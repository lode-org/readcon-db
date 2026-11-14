//! Optional **cooked SoA** payload: derived binary numerics beside authoritative CON text.
//!
//! RCSO is **non-authoritative**. CON text in `frames` is the sole authority.
//!
//! # Why CON text is still required (RCSO is **not** fully equivalent)
//!
//! RCSO stores only POD numerics (positions and optional forces/velocities). It does
//! **not** carry element symbols, masses, cell/angles, constraint masks, JSON metadata,
//! section labels, or exact on-disk CON bytes. Therefore it cannot replace
//! `frames` for xxHash3 dedup, join/split fidelity, `reindex`, formula/symbol indexes,
//! or CON export. The improvement is skipping **CON parse on numeric hot paths** when a
//! valid cooked blob exists—not omitting the text tier from storage.
//!
//! Layout (little-endian, version 1):
//! - magic `RCSO` (4 bytes)
//! - version `u32` (=1)
//! - natoms `u32`
//! - flags `u32` (bit0 = forces block present, bit1 = velocities block present)
//! - pos_dtype `u8` (0 = f64 row-major N×3); pad 3 bytes
//! - reserved `u32` (=0)
//! - positions: `natoms * 3` f64 LE
//! - forces (if flag): `natoms * 3` f64 LE
//! - velocities (if flag): `natoms * 3` f64 LE
//!
//! Never used for xxHash dedup, join-split fidelity, or secondary index rebuild.
//! Rebuild by re-parsing CON text → [`CookedSoa::encode_frame`].

use readcon_core::types::ConFrame;

use crate::error::{Error, Result};

pub const COOKED_MAGIC: &[u8; 4] = b"RCSO";
pub const COOKED_VERSION: u32 = 1;
pub const DTYPE_F64: u8 = 0;

pub const FLAG_FORCES: u32 = 1 << 0;
pub const FLAG_VELOCITIES: u32 = 1 << 1;
/// Per-atom masses, atomic numbers, constraint masks, and the 3×3 cell.
/// Written by current ingests so a read can copy them without parsing CON.
pub const FLAG_MASSES: u32 = 1 << 2;
pub const FLAG_NUMBERS: u32 = 1 << 3;
pub const FLAG_CONSTRAINTS: u32 = 1 << 4;
pub const FLAG_CELL: u32 = 1 << 5;
/// One f64 energy, a set flag, frame index, pbc, and symbol runs.
pub const FLAG_META: u32 = 1 << 6;

const HEADER_LEN: usize = 4 + 4 + 4 + 4 + 1 + 3 + 4; // 24

/// Decoded cooked numerics (always f64 for v1).
#[derive(Clone, Debug, PartialEq)]
pub struct CookedSoa {
    /// Atom count. Positions, forces, and velocities use this length.
    pub natoms: u32,
    /// Cartesian coordinates, one row per atom.
    pub positions: Vec<[f64; 3]>,
    /// Forces, present only when the cooked blob stored them.
    pub forces: Option<Vec<[f64; 3]>>,
    /// Velocities, present only when the cooked blob stored them.
    pub velocities: Option<Vec<[f64; 3]>>,
}

impl CookedSoa {
    /// Encode one frame as an RCSO blob. CON text stays the authority.
    pub fn encode_frame(frame: &ConFrame) -> Result<Vec<u8>> {
        let n = frame.atom_data.len();
        if n > u32::MAX as usize {
            return Err(Error::Message("too many atoms for cooked SoA".into()));
        }
        let natoms = n as u32;
        let mut flags = 0u32;
        let has_f = frame.atom_data.iter().any(|a| a.force().is_some());
        let has_v = frame.atom_data.iter().any(|a| a.velocity().is_some());
        if has_f {
            flags |= FLAG_FORCES;
        }
        if has_v {
            flags |= FLAG_VELOCITIES;
        }

        let mut out = Vec::with_capacity(
            HEADER_LEN + n * (3 * 8 * (1 + has_f as usize + has_v as usize) + 8 + 4 + 3),
        );
        out.extend_from_slice(COOKED_MAGIC);
        out.extend_from_slice(&COOKED_VERSION.to_le_bytes());
        out.extend_from_slice(&natoms.to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.push(DTYPE_F64);
        out.extend_from_slice(&[0u8; 3]);
        out.extend_from_slice(&0u32.to_le_bytes());

        push_coord_block(
            &mut out,
            frame.positions.f64_slice(),
            &frame.atom_data,
            crate::atom_coord::atom_xyz,
        )?;
        if has_f {
            push_coord_block(&mut out, frame.forces.f64_slice(), &frame.atom_data, |a| {
                a.force().unwrap_or([0.0; 3])
            })?;
        }
        if has_v {
            push_coord_block(
                &mut out,
                frame.velocities.f64_slice(),
                &frame.atom_data,
                |a| a.velocity().unwrap_or([0.0; 3]),
            )?;
        }
        // These blocks sit after the numeric columns. A reader that does not
        // know the flags still decodes positions, forces, and velocities.
        flags |= FLAG_MASSES | FLAG_NUMBERS | FLAG_CONSTRAINTS | FLAG_CELL | FLAG_META;
        out[12..16].copy_from_slice(&flags.to_le_bytes());
        if frame.masses.len() != n {
            return Err(Error::Message("cooked masses length".into()));
        }
        let mass_slice = frame.masses.f64_slice();
        let uniform_mass = mass_slice
            .filter(|xs| xs.len() == n && xs.iter().all(|m| m.to_bits() == xs[0].to_bits()));
        if let Some(xs) = uniform_mass {
            out.push(1);
            push_f64s(&mut out, &xs[..1]);
        } else {
            out.push(0);
            if let Some(xs) = mass_slice {
                push_f64s(&mut out, xs);
            } else {
                for i in 0..n {
                    push_f64s(&mut out, &[frame.masses.get_f64(i)]);
                }
            }
        }
        // Species runs follow the header's type groups, so this does not
        // walk every atom. Constraints are a dense mask only when one axis
        // is fixed; the common case stores a single zero byte.
        let mut runs: Vec<(&str, u32)> = Vec::new();
        let mut offset = 0usize;
        if frame.header.natms_per_type.is_empty() {
            for a in &frame.atom_data {
                let sym = a.symbol.as_ref();
                if let Some(last) = runs.last_mut() {
                    if last.0 == sym {
                        last.1 += 1;
                        continue;
                    }
                }
                runs.push((sym, 1));
            }
        } else {
            for &count in &frame.header.natms_per_type {
                if count == 0 || offset >= n {
                    continue;
                }
                let sym = frame.atom_data[offset].symbol.as_ref();
                runs.push((sym, count as u32));
                offset += count;
            }
        }
        let any_fixed = frame
            .atom_data
            .iter()
            .any(|a| a.fixed() != [false, false, false]);
        if any_fixed {
            out.push(1);
            for a in &frame.atom_data {
                for flag in a.fixed() {
                    out.push(u8::from(flag));
                }
            }
        } else {
            out.push(0);
        }
        for row in frame.header.cell_matrix() {
            for c in row {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        let energy = frame.header.energy().unwrap_or(0.0);
        out.extend_from_slice(&energy.to_le_bytes());
        out.push(u8::from(frame.header.energy().is_some()));
        let frame_index = frame.header.frame_index().map(|i| i as f64).unwrap_or(0.0);
        out.extend_from_slice(&frame_index.to_le_bytes());
        out.push(u8::from(frame.header.frame_index().is_some()));
        let pbc = frame.header.pbc().unwrap_or([false, false, false]);
        out.push(u8::from(pbc[0]));
        out.push(u8::from(pbc[1]));
        out.push(u8::from(pbc[2]));
        out.push(u8::from(frame.header.pbc().is_some()));
        out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
        for (sym, count) in runs {
            let bytes = sym.as_bytes();
            let len = u16::try_from(bytes.len())
                .map_err(|_| Error::Message("symbol longer than a cooked run allows".into()))?;
            out.extend_from_slice(&count.to_le_bytes());
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(bytes);
        }
        Ok(out)
    }

    /// RCSO bytes from columns the span walk already parsed.
    ///
    /// `forces` and `velocities` are empty when that block is absent.
    /// `mass_uniform` stores one mass for every atom. Otherwise `masses`
    /// has one value per atom. `constraints` is `None` when every axis is
    /// free, and `natoms * 3` bytes otherwise.
    ///
    /// The column vectors are dropped as soon as their bytes are in `out`,
    /// so the scratch and the blob are not resident together.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_columns(
        natoms: u32,
        positions: Vec<f64>,
        forces: Vec<f64>,
        velocities: Vec<f64>,
        mass_uniform: Option<f64>,
        masses: &[f64],
        constraints: Option<Vec<u8>>,
        cell: [f64; 9],
        energy: f64,
        energy_set: bool,
        frame_index: f64,
        frame_index_set: bool,
        pbc: [bool; 3],
        pbc_set: bool,
        runs: &[(&str, u32)],
    ) -> Result<Vec<u8>> {
        let n = natoms as usize;
        if positions.len() != n.saturating_mul(3) {
            return Err(Error::Message("cooked positions length".into()));
        }
        let has_f = !forces.is_empty();
        let has_v = !velocities.is_empty();
        if has_f && forces.len() != positions.len() {
            return Err(Error::Message("cooked forces length".into()));
        }
        if has_v && velocities.len() != positions.len() {
            return Err(Error::Message("cooked velocities length".into()));
        }
        if mass_uniform.is_none() && masses.len() != n {
            return Err(Error::Message("cooked masses length".into()));
        }
        if let Some(mask) = constraints.as_ref() {
            if mask.len() != positions.len() {
                return Err(Error::Message("cooked constraints length".into()));
            }
        }
        let mut flags = FLAG_MASSES | FLAG_NUMBERS | FLAG_CONSTRAINTS | FLAG_CELL | FLAG_META;
        if has_f {
            flags |= FLAG_FORCES;
        }
        if has_v {
            flags |= FLAG_VELOCITIES;
        }
        let mut out = Vec::with_capacity(
            HEADER_LEN + positions.len() * 8 * (1 + has_f as usize + has_v as usize) + n * 8,
        );
        out.extend_from_slice(COOKED_MAGIC);
        out.extend_from_slice(&COOKED_VERSION.to_le_bytes());
        out.extend_from_slice(&natoms.to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.push(DTYPE_F64);
        out.extend_from_slice(&[0u8; 3]);
        out.extend_from_slice(&0u32.to_le_bytes());
        push_f64s(&mut out, &positions);
        drop(positions);
        if has_f {
            push_f64s(&mut out, &forces);
        }
        drop(forces);
        if has_v {
            push_f64s(&mut out, &velocities);
        }
        drop(velocities);
        if let Some(mass) = mass_uniform {
            out.push(1);
            push_f64s(&mut out, &[mass]);
        } else {
            out.push(0);
            push_f64s(&mut out, masses);
        }
        match constraints {
            Some(mask) => {
                out.push(1);
                out.extend_from_slice(&mask);
            }
            None => out.push(0),
        }
        push_f64s(&mut out, &cell);
        push_f64s(&mut out, &[energy]);
        out.push(u8::from(energy_set));
        push_f64s(&mut out, &[frame_index]);
        out.push(u8::from(frame_index_set));
        out.push(u8::from(pbc[0]));
        out.push(u8::from(pbc[1]));
        out.push(u8::from(pbc[2]));
        out.push(u8::from(pbc_set));
        out.extend_from_slice(&(runs.len() as u32).to_le_bytes());
        for (sym, count) in runs {
            let bytes = sym.as_bytes();
            let len = u16::try_from(bytes.len())
                .map_err(|_| Error::Message("symbol longer than a cooked run allows".into()))?;
            out.extend_from_slice(&count.to_le_bytes());
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(bytes);
        }
        Ok(out)
    }

    /// Decode an RCSO blob. A bad magic, version, or length is an error.
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() < HEADER_LEN {
            return Err(Error::Message("cooked SoA truncated header".into()));
        }
        if &bytes[0..4] != COOKED_MAGIC {
            return Err(Error::Message("cooked SoA bad magic".into()));
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != COOKED_VERSION {
            return Err(Error::Message(format!(
                "cooked SoA unsupported version {version}"
            )));
        }
        let natoms = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
        let flags = u32::from_le_bytes(bytes[12..16].try_into().unwrap());
        let dtype = bytes[16];
        if dtype != DTYPE_F64 {
            return Err(Error::Message(format!(
                "cooked SoA unsupported dtype {dtype}"
            )));
        }
        let n = natoms as usize;
        let block = n
            .checked_mul(3)
            .ok_or_else(|| Error::Message("overflow".into()))?;
        let block_bytes = block
            .checked_mul(8)
            .ok_or_else(|| Error::Message("overflow".into()))?;
        let mut need = HEADER_LEN + block_bytes;
        let has_f = flags & FLAG_FORCES != 0;
        let has_v = flags & FLAG_VELOCITIES != 0;
        if has_f {
            need = need
                .checked_add(block_bytes)
                .ok_or_else(|| Error::Message("overflow".into()))?;
        }
        if has_v {
            need = need
                .checked_add(block_bytes)
                .ok_or_else(|| Error::Message("overflow".into()))?;
        }
        if bytes.len() < need {
            return Err(Error::Message("cooked SoA truncated body".into()));
        }

        let mut off = HEADER_LEN;
        let positions = read_vec3_block(&bytes[off..off + block_bytes], n)?;
        off += block_bytes;
        let forces = if has_f {
            let f = read_vec3_block(&bytes[off..off + block_bytes], n)?;
            off += block_bytes;
            Some(f)
        } else {
            None
        };
        let velocities = if has_v {
            let v = read_vec3_block(&bytes[off..off + block_bytes], n)?;
            Some(v)
        } else {
            None
        };
        let _ = off;
        Ok(Self {
            natoms,
            positions,
            forces,
            velocities,
        })
    }

    /// Prefer cooked bytes; return None if missing/invalid (caller parses CON).
    pub fn try_decode(bytes: &[u8]) -> Option<Self> {
        Self::decode(bytes).ok()
    }

    /// Append the position block to `out` as row-major f64. `None` when the
    /// blob is not a v1 f64 RCSO with a complete position block.
    pub fn copy_positions(bytes: &[u8], out: &mut Vec<f64>) -> Option<u32> {
        if bytes.len() < HEADER_LEN || &bytes[0..4] != COOKED_MAGIC {
            return None;
        }
        let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
        if version != COOKED_VERSION || bytes[16] != DTYPE_F64 {
            return None;
        }
        let natoms = u32::from_le_bytes(bytes[8..12].try_into().ok()?);
        let n = natoms as usize;
        let nbytes = n.checked_mul(3)?.checked_mul(8)?;
        let end = HEADER_LEN.checked_add(nbytes)?;
        if bytes.len() < end {
            return None;
        }
        out.reserve(n.saturating_mul(3));
        for chunk in bytes[HEADER_LEN..end].chunks_exact(8) {
            let raw: [u8; 8] = chunk.try_into().ok()?;
            out.push(f64::from_le_bytes(raw));
        }
        Some(natoms)
    }
}

/// One frame of the arrays a fair read has to materialize.
///
/// Positions, velocities, masses, and the cell are row-major `f64`.
/// `numbers` are atomic numbers. `constraints` is `natoms * 3` bytes,
/// 1 where that axis is fixed. Velocities are zeros when the frame has none.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterializedFrame {
    /// Atom count.
    pub n_atoms: u32,
    /// Row-major coordinates, `n_atoms * 3`.
    pub positions: Vec<f64>,
    /// Row-major forces, empty when the frame has none.
    pub forces: Vec<f64>,
    /// Row-major velocities, `n_atoms * 3`, zeros when the frame has none.
    pub velocities: Vec<f64>,
    /// True when the velocity block was stored, not filled with zeros.
    pub velocities_set: bool,
    /// Per-atom masses.
    pub masses: Vec<f64>,
    /// Atomic numbers.
    pub numbers: Vec<u32>,
    /// Fixed mask, `n_atoms * 3` bytes.
    pub constraints: Vec<u8>,
    /// Row-major 3×3 cell.
    pub cell: [f64; 9],
    /// Frame energy. Zero when `energy_set` is false.
    pub energy: f64,
    /// Whether the CON frame stored an energy.
    pub energy_set: bool,
    /// Stored frame index, or 0 when unset.
    pub frame_index: f64,
    /// Whether the CON frame stored a frame index.
    pub frame_index_set: bool,
    /// Periodic flags.
    pub pbc: [bool; 3],
    /// Whether the CON frame stored pbc.
    pub pbc_set: bool,
    /// `(symbol, count)` runs in file order.
    pub symbol_runs: Vec<(String, u32)>,
}

impl MaterializedFrame {
    /// Copy a current RCSO blob. `None` when the extended blocks are absent,
    /// which is how an older blob asks the caller to parse CON instead.
    pub fn from_blob(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < HEADER_LEN || &bytes[0..4] != COOKED_MAGIC {
            return None;
        }
        let version = u32::from_le_bytes(bytes.get(4..8)?.try_into().ok()?);
        if version != COOKED_VERSION || bytes.get(16)? != &DTYPE_F64 {
            return None;
        }
        let natoms = u32::from_le_bytes(bytes.get(8..12)?.try_into().ok()?);
        let flags = u32::from_le_bytes(bytes.get(12..16)?.try_into().ok()?);
        let need = FLAG_MASSES | FLAG_NUMBERS | FLAG_CONSTRAINTS | FLAG_CELL | FLAG_META;
        if flags & need != need {
            return None;
        }
        let n = natoms as usize;
        let mut off = HEADER_LEN;
        let npos = n.checked_mul(3)?;
        let positions = take_f64(bytes, &mut off, npos)?;
        let forces = if flags & FLAG_FORCES != 0 {
            take_f64(bytes, &mut off, npos)?
        } else {
            Vec::new()
        };
        let (velocities, velocities_set) = if flags & FLAG_VELOCITIES != 0 {
            (take_f64(bytes, &mut off, n.checked_mul(3)?)?, true)
        } else {
            // Zeros are filled by the caller. A million-atom frame would
            // otherwise keep 24 MiB of zeros beside the position columns.
            (Vec::new(), false)
        };
        Self::finish(
            natoms,
            positions,
            forces,
            velocities,
            velocities_set,
            &bytes[off..],
        )
    }
}

impl MaterializedFrame {
    fn finish(
        natoms: u32,
        positions: Vec<f64>,
        forces: Vec<f64>,
        velocities: Vec<f64>,
        velocities_set: bool,
        bytes: &[u8],
    ) -> Option<Self> {
        let n = natoms as usize;
        let mut off = 0usize;
        let mass_mode = *bytes.get(off)?;
        off += 1;
        let masses = if mass_mode == 1 {
            let m = take_f64(bytes, &mut off, 1)?[0];
            vec![m; n]
        } else {
            take_f64(bytes, &mut off, n)?
        };
        let constraint_mode = *bytes.get(off)?;
        off += 1;
        let constraints = if constraint_mode == 0 {
            vec![0u8; n.checked_mul(3)?]
        } else {
            take_bytes(bytes, &mut off, n.checked_mul(3)?)?
        };
        let cell_v = take_f64(bytes, &mut off, 9)?;
        let mut cell = [0.0; 9];
        cell.copy_from_slice(&cell_v);
        let energy = take_f64(bytes, &mut off, 1)?[0];
        let energy_set = *bytes.get(off)? != 0;
        off += 1;
        let frame_index = take_f64(bytes, &mut off, 1)?[0];
        let frame_index_set = *bytes.get(off)? != 0;
        off += 1;
        let pbc = [
            *bytes.get(off)? != 0,
            *bytes.get(off + 1)? != 0,
            *bytes.get(off + 2)? != 0,
        ];
        let pbc_set = *bytes.get(off + 3)? != 0;
        off += 4;
        let nruns = take_u32(bytes, &mut off, 1)?[0] as usize;
        let mut symbol_runs = Vec::with_capacity(nruns);
        let mut numbers = Vec::with_capacity(n);
        for _ in 0..nruns {
            let count = take_u32(bytes, &mut off, 1)?[0];
            if off + 2 > bytes.len() {
                return None;
            }
            let len = u16::from_le_bytes(bytes[off..off + 2].try_into().ok()?) as usize;
            off += 2;
            let sym = bytes.get(off..off + len)?;
            off += len;
            let sym = String::from_utf8(sym.to_vec()).ok()?;
            let z =
                u32::try_from(readcon_core::helpers::symbol_to_atomic_number(&sym)).unwrap_or(0);
            numbers.extend(std::iter::repeat_n(z, count as usize));
            symbol_runs.push((sym, count));
        }
        if numbers.len() != n {
            return None;
        }
        Some(Self {
            n_atoms: natoms,
            positions,
            forces,
            velocities,
            velocities_set,
            masses,
            numbers,
            constraints,
            cell,
            energy,
            energy_set,
            frame_index,
            frame_index_set,
            pbc,
            pbc_set,
            symbol_runs,
        })
    }
}

/// Tag for an interleaved byte-shuffled zstd frame (db20). Not `RCSO`.
pub const SHUFFLE_ZSTD_TAG: [u8; 2] = [0xFB, 0x03];

/// Tag for a component-major shuffled frame. Each scalar column is its own
/// zstd frame. `FB 03` still decodes.
pub const SHUFFLE_ZSTD_SOA_TAG: [u8; 2] = [0xFB, 0x04];

/// Level chosen on a 100k-atom frame: it was smaller than level 1 and the
/// decode was faster. Negative levels were larger and no faster.
const SHUFFLE_LEVEL: i32 = 3;

/// True when `stored` is a shuffled zstd frame rather than raw RCSO.
pub fn is_shuffled_zstd(stored: &[u8]) -> bool {
    stored.len() >= 2 && stored[0] == 0xFB && (stored[1] == 0x03 || stored[1] == 0x04)
}

/// Atom count and which coordinate blocks a component-major frame carries.
/// `None` for raw RCSO and for the older interleaved tag.
#[cfg(feature = "python")]
pub fn shuffled_column_layout(stored: &[u8]) -> Option<(u32, bool, bool)> {
    if stored.len() < 14
        || stored[0] != SHUFFLE_ZSTD_SOA_TAG[0]
        || stored[1] != SHUFFLE_ZSTD_SOA_TAG[1]
    {
        return None;
    }
    let natoms = u32::from_le_bytes(stored.get(2..6)?.try_into().ok()?);
    let flags = u32::from_le_bytes(stored.get(6..10)?.try_into().ok()?);
    if natoms == 0 {
        return None;
    }
    Some((
        natoms,
        flags & FLAG_FORCES != 0,
        flags & FLAG_VELOCITIES != 0,
    ))
}

struct ZstdScratch {
    dec: zstd::bulk::Decompressor<'static>,
    /// One decompressed column, reused by the interleaved db20 tag.
    buf: Vec<u8>,
}

fn with_zstd_scratch<T>(body: impl FnOnce(&mut ZstdScratch) -> Result<T>) -> Result<T> {
    std::thread_local! {
        static SCRATCH: std::cell::RefCell<Option<ZstdScratch>> = const { std::cell::RefCell::new(None) };
    }
    SCRATCH.with(|slot| {
        let mut guard = slot.borrow_mut();
        if guard.is_none() {
            let dec = zstd::bulk::Decompressor::new()
                .map_err(|e| Error::Message(format!("zstd decode: {e}")))?;
            *guard = Some(ZstdScratch {
                dec,
                buf: Vec::new(),
            });
        }
        body(guard.as_mut().unwrap())
    })
}

fn coord_f64s(raw: &[u8]) -> Option<usize> {
    if raw.len() < HEADER_LEN || &raw[0..4] != COOKED_MAGIC {
        return None;
    }
    let natoms = u32::from_le_bytes(raw.get(8..12)?.try_into().ok()?) as usize;
    let flags = u32::from_le_bytes(raw.get(12..16)?.try_into().ok()?);
    let mut n = natoms.checked_mul(3)?;
    if flags & FLAG_FORCES != 0 {
        n = n.checked_add(natoms.checked_mul(3)?)?;
    }
    if flags & FLAG_VELOCITIES != 0 {
        n = n.checked_add(natoms.checked_mul(3)?)?;
    }
    let end = HEADER_LEN.checked_add(n.checked_mul(8)?)?;
    if raw.len() < end {
        return None;
    }
    Some(n)
}

fn coord_layout(raw: &[u8]) -> Result<(usize, u32, usize, usize, usize)> {
    let n = coord_f64s(raw).ok_or_else(|| Error::Message("cooked frame is not RCSO".into()))?;
    let natoms = u32::from_le_bytes(raw[8..12].try_into().unwrap());
    let flags = u32::from_le_bytes(raw[12..16].try_into().unwrap());
    let atoms = natoms as usize;
    let npos = atoms * 3;
    let nfor = if flags & FLAG_FORCES != 0 { npos } else { 0 };
    let nvel = if flags & FLAG_VELOCITIES != 0 {
        npos
    } else {
        0
    };
    if npos + nfor + nvel != n {
        return Err(Error::Message(
            "shuffled cooked frame length mismatch".into(),
        ));
    }
    Ok((n, flags, npos, nfor, nvel))
}

/// Byte-shuffle each scalar column and zstd it at level 3.
///
/// `x`, `y`, and `z` are separate frames, and the same for forces and
/// velocities. An interleaved shuffle of `xyz` does not decode in the
/// noise of a raw copy. The header and the trailing metadata stay
/// uncompressed. The db20 interleaved tag still decodes.
pub fn compress_shuffled(raw: &[u8]) -> Result<Vec<u8>> {
    compress_components(raw, SHUFFLE_ZSTD_SOA_TAG)
}

/// db20 layout: one zstd frame of the interleaved coordinate bytes.
#[cfg(test)]
fn compress_interleaved_shuffle(raw: &[u8]) -> Result<Vec<u8>> {
    let (n, _, _, _, _) = coord_layout(raw)?;
    let coord_bytes = n * 8;
    let coords = &raw[HEADER_LEN..HEADER_LEN + coord_bytes];
    let mut shuffled = vec![0u8; coord_bytes];
    let mut off = 0usize;
    let natoms = u32::from_le_bytes(raw[8..12].try_into().unwrap()) as usize;
    let flags = u32::from_le_bytes(raw[12..16].try_into().unwrap());
    let npos = natoms * 3;
    let nfor = if flags & FLAG_FORCES != 0 { npos } else { 0 };
    let nvel = if flags & FLAG_VELOCITIES != 0 {
        npos
    } else {
        0
    };
    for count in [npos, nfor, nvel] {
        if count == 0 {
            continue;
        }
        let bytes = count * 8;
        crate::byte_shuffle::shuffle_bytes(
            &coords[off..off + bytes],
            &mut shuffled[off..off + bytes],
        );
        off += bytes;
    }
    let comp = zstd::bulk::compress(&shuffled, 1)
        .map_err(|e| Error::Message(format!("zstd encode: {e}")))?;
    let suffix = &raw[HEADER_LEN + coord_bytes..];
    let mut out = Vec::with_capacity(2 + 4 + 4 + comp.len() + HEADER_LEN + suffix.len());
    out.extend_from_slice(&SHUFFLE_ZSTD_TAG);
    out.extend_from_slice(&(n as u32).to_le_bytes());
    out.extend_from_slice(&(comp.len() as u32).to_le_bytes());
    out.extend_from_slice(&comp);
    out.extend_from_slice(&raw[..HEADER_LEN]);
    out.extend_from_slice(suffix);
    Ok(out)
}

fn compress_components(raw: &[u8], tag: [u8; 2]) -> Result<Vec<u8>> {
    let (_n, _flags, npos, nfor, nvel) = coord_layout(raw)?;
    let natoms = u32::from_le_bytes(raw[8..12].try_into().unwrap());
    let atoms = natoms as usize;
    let coord_bytes = (npos + nfor + nvel) * 8;
    let coords = &raw[HEADER_LEN..HEADER_LEN + coord_bytes];
    let mut compressor = zstd::bulk::Compressor::new(SHUFFLE_LEVEL)
        .map_err(|e| Error::Message(format!("zstd encode: {e}")))?;
    // A window of 4 KiB decodes a 100k-atom column faster than the
    // default. Long-distance matching does not.
    let window = (atoms * 8)
        .next_power_of_two()
        .trailing_zeros()
        .clamp(10, 12);
    compressor
        .set_parameter(zstd::zstd_safe::CParameter::WindowLog(window))
        .map_err(|e| Error::Message(format!("zstd encode: {e}")))?;
    compressor
        .set_parameter(zstd::zstd_safe::CParameter::EnableLongDistanceMatching(
            false,
        ))
        .map_err(|e| Error::Message(format!("zstd encode: {e}")))?;
    let mut columns = Vec::new();
    let mut gather = vec![0f64; atoms];
    let mut shuffled = vec![0u8; atoms * 8];
    for (count, base) in [(npos, 0usize), (nfor, npos), (nvel, npos + nfor)] {
        if count == 0 {
            continue;
        }
        let f64s = le_f64s(&coords[base * 8..base * 8 + count * 8]);
        for lane in 0..3 {
            for (i, slot) in gather.iter_mut().enumerate() {
                *slot = f64s[i * 3 + lane];
            }
            let bytes = le_f64_bytes(&gather);
            crate::byte_shuffle::shuffle_bytes(bytes, &mut shuffled);
            columns.push(
                compressor
                    .compress(&shuffled)
                    .map_err(|e| Error::Message(format!("zstd encode: {e}")))?,
            );
        }
    }
    let suffix = &raw[HEADER_LEN + coord_bytes..];
    let mut out = Vec::new();
    out.extend_from_slice(&tag);
    out.extend_from_slice(&natoms.to_le_bytes());
    out.extend_from_slice(&u32::from_le_bytes(raw[12..16].try_into().unwrap()).to_le_bytes());
    out.extend_from_slice(&(columns.len() as u32).to_le_bytes());
    for column in &columns {
        out.extend_from_slice(&(column.len() as u32).to_le_bytes());
        out.extend_from_slice(column);
    }
    out.extend_from_slice(&raw[..HEADER_LEN]);
    out.extend_from_slice(suffix);
    Ok(out)
}

fn le_f64s(bytes: &[u8]) -> Vec<f64> {
    if cfg!(target_endian = "little") {
        let n = bytes.len() / 8;
        let slice = unsafe { std::slice::from_raw_parts(bytes.as_ptr().cast::<f64>(), n) };
        slice.to_vec()
    } else {
        bytes
            .chunks_exact(8)
            .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
            .collect()
    }
}

fn le_f64_bytes(xs: &[f64]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(xs.as_ptr().cast::<u8>(), xs.len() * 8) }
}

/// Unshuffle one compressed frame straight into its column buffers.
pub fn decompress_shuffled(stored: &[u8]) -> Result<MaterializedFrame> {
    if stored.len() >= 2 && stored[1] == SHUFFLE_ZSTD_TAG[1] && stored[0] == 0xFB {
        return decompress_interleaved(stored);
    }
    decompress_components(stored)
}

fn decompress_interleaved(stored: &[u8]) -> Result<MaterializedFrame> {
    if stored.len() < 10 {
        return Err(Error::Message("not a shuffled cooked frame".into()));
    }
    let n = u32::from_le_bytes(stored[2..6].try_into().unwrap()) as usize;
    let comp_len = u32::from_le_bytes(stored[6..10].try_into().unwrap()) as usize;
    if stored.len() < 10 + comp_len + HEADER_LEN {
        return Err(Error::Message("truncated shuffled cooked frame".into()));
    }
    let comp = &stored[10..10 + comp_len];
    let shuffled = with_zstd_scratch(|scratch| {
        scratch.buf.clear();
        scratch.buf.reserve(n * 8);
        scratch
            .dec
            .decompress_to_buffer(comp, &mut scratch.buf)
            .map_err(|e| Error::Message(format!("zstd decode: {e}")))?;
        if scratch.buf.len() != n * 8 {
            return Err(Error::Message(
                "shuffled cooked frame has the wrong length".into(),
            ));
        }
        Ok(scratch.buf.clone())
    })?;
    let rest = &stored[10 + comp_len..];
    if rest.len() < HEADER_LEN || &rest[..4] != COOKED_MAGIC {
        return Err(Error::Message(
            "shuffled cooked frame is missing its header".into(),
        ));
    }
    let natoms = u32::from_le_bytes(rest[8..12].try_into().unwrap());
    let flags = u32::from_le_bytes(rest[12..16].try_into().unwrap());
    let atoms = natoms as usize;
    let npos = atoms * 3;
    let nfor = if flags & FLAG_FORCES != 0 { npos } else { 0 };
    let nvel = if flags & FLAG_VELOCITIES != 0 {
        npos
    } else {
        0
    };
    if npos + nfor + nvel != n {
        return Err(Error::Message(
            "shuffled cooked frame length mismatch".into(),
        ));
    }
    let mut positions = uninit_f64(npos);
    unshuffle_into(&shuffled[..npos * 8], &mut positions);
    let forces = if nfor == 0 {
        Vec::new()
    } else {
        let mut forces = uninit_f64(nfor);
        let start = npos * 8;
        unshuffle_into(&shuffled[start..start + nfor * 8], &mut forces);
        forces
    };
    let (velocities, velocities_set) = if nvel == 0 {
        (Vec::new(), false)
    } else {
        let mut velocities = uninit_f64(nvel);
        let start = (npos + nfor) * 8;
        unshuffle_into(&shuffled[start..start + nvel * 8], &mut velocities);
        (velocities, true)
    };
    let suffix = &rest[HEADER_LEN..];
    MaterializedFrame::finish(
        natoms,
        positions,
        forces,
        velocities,
        velocities_set,
        suffix,
    )
    .ok_or_else(|| Error::Message("shuffled cooked frame metadata is corrupt".into()))
}

fn decompress_components(stored: &[u8]) -> Result<MaterializedFrame> {
    if stored.len() < 14
        || stored[0] != SHUFFLE_ZSTD_SOA_TAG[0]
        || stored[1] != SHUFFLE_ZSTD_SOA_TAG[1]
    {
        return Err(Error::Message("not a shuffled cooked frame".into()));
    }
    let natoms = u32::from_le_bytes(stored[2..6].try_into().unwrap());
    let flags = u32::from_le_bytes(stored[6..10].try_into().unwrap());
    let ncomp = u32::from_le_bytes(stored[10..14].try_into().unwrap()) as usize;
    let atoms = natoms as usize;
    let npos = atoms * 3;
    let nfor = if flags & FLAG_FORCES != 0 { npos } else { 0 };
    let nvel = if flags & FLAG_VELOCITIES != 0 {
        npos
    } else {
        0
    };
    let expect = [npos, nfor, nvel].iter().filter(|n| **n > 0).count() * 3;
    if ncomp != expect || atoms == 0 {
        return Err(Error::Message(
            "shuffled cooked frame length mismatch".into(),
        ));
    }
    let off = 14usize;
    let mut positions = uninit_f64(npos);
    let mut forces = if nfor == 0 {
        Vec::new()
    } else {
        uninit_f64(nfor)
    };
    let mut velocities = if nvel == 0 {
        Vec::new()
    } else {
        uninit_f64(nvel)
    };
    let suffix_at = fill_component_columns(
        stored,
        off,
        ncomp,
        &mut positions,
        &mut forces,
        &mut velocities,
    )?;
    let suffix = &stored[suffix_at + HEADER_LEN..];
    MaterializedFrame::finish(natoms, positions, forces, velocities, nvel != 0, suffix)
        .ok_or_else(|| Error::Message("shuffled cooked frame metadata is corrupt".into()))
}

/// Decode coordinate columns into caller buffers. The returned frame carries
/// the metadata only; its position and force vectors are empty.
pub(crate) fn decompress_shuffled_into(
    stored: &[u8],
    positions: &mut [f64],
    forces: &mut [f64],
    velocities: &mut [f64],
) -> Result<MaterializedFrame> {
    if stored.len() >= 2 && stored[0] == 0xFB && stored[1] == SHUFFLE_ZSTD_TAG[1] {
        let frame = decompress_interleaved(stored)?;
        if positions.len() != frame.positions.len()
            || forces.len() != frame.forces.len()
            || velocities.len() != frame.velocities.len()
        {
            return Err(Error::Message(
                "shuffled cooked frame length mismatch".into(),
            ));
        }
        positions.copy_from_slice(&frame.positions);
        if !forces.is_empty() {
            forces.copy_from_slice(&frame.forces);
        }
        if !velocities.is_empty() {
            velocities.copy_from_slice(&frame.velocities);
        }
        return Ok(frame);
    }
    if stored.len() < 14
        || stored[0] != SHUFFLE_ZSTD_SOA_TAG[0]
        || stored[1] != SHUFFLE_ZSTD_SOA_TAG[1]
    {
        return Err(Error::Message("not a shuffled cooked frame".into()));
    }
    let natoms = u32::from_le_bytes(stored[2..6].try_into().unwrap());
    let flags = u32::from_le_bytes(stored[6..10].try_into().unwrap());
    let ncomp = u32::from_le_bytes(stored[10..14].try_into().unwrap()) as usize;
    let atoms = natoms as usize;
    let npos = atoms * 3;
    let nfor = if flags & FLAG_FORCES != 0 { npos } else { 0 };
    let nvel = if flags & FLAG_VELOCITIES != 0 {
        npos
    } else {
        0
    };
    if positions.len() != npos || forces.len() != nfor || velocities.len() != nvel {
        return Err(Error::Message(
            "shuffled cooked frame length mismatch".into(),
        ));
    }
    let expect = [npos, nfor, nvel].iter().filter(|n| **n > 0).count() * 3;
    if ncomp != expect || atoms == 0 {
        return Err(Error::Message(
            "shuffled cooked frame length mismatch".into(),
        ));
    }
    let suffix_at = fill_component_columns(stored, 14, ncomp, positions, forces, velocities)?;
    if stored.len() < suffix_at + HEADER_LEN || &stored[suffix_at..suffix_at + 4] != COOKED_MAGIC {
        return Err(Error::Message(
            "shuffled cooked frame is missing its header".into(),
        ));
    }
    MaterializedFrame::finish(
        natoms,
        Vec::new(),
        Vec::new(),
        Vec::new(),
        nvel != 0,
        &stored[suffix_at + HEADER_LEN..],
    )
    .ok_or_else(|| Error::Message("shuffled cooked frame metadata is corrupt".into()))
}

fn fill_component_columns(
    stored: &[u8],
    mut off: usize,
    ncomp: usize,
    positions: &mut [f64],
    forces: &mut [f64],
    velocities: &mut [f64],
) -> Result<usize> {
    let mut parts = Vec::with_capacity(ncomp);
    for _ in 0..ncomp {
        if off + 4 > stored.len() {
            return Err(Error::Message("truncated shuffled cooked frame".into()));
        }
        let comp_len = u32::from_le_bytes(stored[off..off + 4].try_into().unwrap()) as usize;
        off += 4;
        if off + comp_len > stored.len() {
            return Err(Error::Message("truncated shuffled cooked frame".into()));
        }
        parts.push((off, comp_len));
        off += comp_len;
    }
    if stored.len() < off + HEADER_LEN || &stored[off..off + 4] != COOKED_MAGIC {
        return Err(Error::Message(
            "shuffled cooked frame is missing its header".into(),
        ));
    }
    let declared =
        3 + usize::from(!forces.is_empty()) * 3 + usize::from(!velocities.is_empty()) * 3;
    if parts.len() != declared {
        return Err(Error::Message(
            "shuffled cooked frame length mismatch".into(),
        ));
    }
    if !positions.len().is_multiple_of(3) {
        return Err(Error::Message(
            "shuffled cooked frame length mismatch".into(),
        ));
    }
    let atoms = positions.len() / 3;
    // Every lane is decompressed before any caller buffer is written. A short
    // part, including one past the position columns, leaves those buffers as
    // the caller left them.
    let decoded = decompress_component_columns(stored, &parts, atoms * 8)?;
    let mut i = 0usize;
    for dst in [positions, forces, velocities] {
        if dst.is_empty() {
            continue;
        }
        if i + 3 > decoded.len() || !dst.len().is_multiple_of(3) {
            return Err(Error::Message(
                "shuffled cooked frame length mismatch".into(),
            ));
        }
        crate::byte_shuffle::unshuffle_interleave(
            &decoded[i],
            &decoded[i + 1],
            &decoded[i + 2],
            dst,
        );
        i += 3;
    }
    Ok(off)
}

fn decompress_component_columns(
    stored: &[u8],
    parts: &[(usize, usize)],
    expect: usize,
) -> Result<Vec<Vec<u8>>> {
    with_zstd_scratch(|scratch| {
        let mut decoded = Vec::with_capacity(parts.len());
        for &(start, comp_len) in parts {
            let comp = stored
                .get(start..start + comp_len)
                .ok_or_else(|| Error::Message("truncated shuffled cooked frame".into()))?;
            let mut col = Vec::with_capacity(expect);
            scratch
                .dec
                .decompress_to_buffer(comp, &mut col)
                .map_err(|e| Error::Message(format!("zstd decode: {e}")))?;
            if col.len() != expect {
                return Err(Error::Message(
                    "shuffled cooked frame has the wrong length".into(),
                ));
            }
            decoded.push(col);
        }
        Ok(decoded)
    })
}

#[allow(clippy::uninit_vec)]
fn uninit_f64(n: usize) -> Vec<f64> {
    let mut out = Vec::with_capacity(n);
    // SAFETY: `f64` has no destructor. Every slot is written before a read.
    unsafe { out.set_len(n) };
    out
}

fn unshuffle_into(src: &[u8], dst: &mut [f64]) {
    let out =
        unsafe { std::slice::from_raw_parts_mut(dst.as_mut_ptr().cast::<u8>(), dst.len() * 8) };
    crate::byte_shuffle::unshuffle_bytes(src, out);
}

fn push_f64s(out: &mut Vec<u8>, xs: &[f64]) {
    if cfg!(target_endian = "little") {
        let nbytes = xs.len() * 8;
        let old = out.len();
        out.resize(old + nbytes, 0);
        unsafe {
            std::ptr::copy_nonoverlapping(
                xs.as_ptr().cast::<u8>(),
                out.as_mut_ptr().add(old),
                nbytes,
            );
        }
    } else {
        for x in xs {
            out.extend_from_slice(&x.to_le_bytes());
        }
    }
}

fn push_coord_block(
    out: &mut Vec<u8>,
    slice: Option<&[f64]>,
    atoms: &[readcon_core::types::AtomDatum],
    fallback: impl Fn(&readcon_core::types::AtomDatum) -> [f64; 3],
) -> Result<()> {
    if let Some(xs) = slice {
        if xs.len() == atoms.len() * 3 {
            push_f64s(out, xs);
            return Ok(());
        }
    }
    for atom in atoms {
        push_f64s(out, &fallback(atom));
    }
    Ok(())
}

fn copy_f64_block(bytes: &[u8], off: usize, n: usize) -> Option<Vec<f64>> {
    let nbytes = n.checked_mul(8)?;
    let raw = bytes.get(off..off + nbytes)?;
    let mut out = uninit_f64(n);
    if cfg!(target_endian = "little") {
        unsafe {
            std::ptr::copy_nonoverlapping(raw.as_ptr(), out.as_mut_ptr().cast::<u8>(), nbytes);
        }
    } else {
        for (dst, chunk) in out.iter_mut().zip(raw.chunks_exact(8)) {
            *dst = f64::from_le_bytes(chunk.try_into().ok()?);
        }
    }
    Some(out)
}

fn take_f64(bytes: &[u8], off: &mut usize, n: usize) -> Option<Vec<f64>> {
    let out = copy_f64_block(bytes, *off, n)?;
    *off += n * 8;
    Some(out)
}

fn take_u32(bytes: &[u8], off: &mut usize, n: usize) -> Option<Vec<u32>> {
    let nbytes = n.checked_mul(4)?;
    let raw = bytes.get(*off..*off + nbytes)?;
    *off += nbytes;
    let mut out = Vec::with_capacity(n);
    for chunk in raw.chunks_exact(4) {
        out.push(u32::from_le_bytes(chunk.try_into().ok()?));
    }
    Some(out)
}

fn take_bytes(bytes: &[u8], off: &mut usize, n: usize) -> Option<Vec<u8>> {
    let raw = bytes.get(*off..*off + n)?;
    *off += n;
    Some(raw.to_vec())
}

/// Length-prefixed RCSO blobs for one collective on the caller comm.
/// Grain: many frames per Bcast (ADIOS BP5 `MinDeferredSize` is 4 MiB).
pub const BATCH_MAGIC: &[u8; 4] = b"RCSB";
pub const BATCH_VERSION: u32 = 1;

pub fn encode_batch(blobs: &[Vec<u8>]) -> Result<Vec<u8>> {
    if blobs.len() > u32::MAX as usize {
        return Err(Error::Message("too many frames in pack batch".into()));
    }
    let mut out = Vec::new();
    out.extend_from_slice(BATCH_MAGIC);
    out.extend_from_slice(&BATCH_VERSION.to_le_bytes());
    out.extend_from_slice(&(blobs.len() as u32).to_le_bytes());
    for b in blobs {
        if b.len() > u32::MAX as usize {
            return Err(Error::Message("RCSO blob exceeds u32 length".into()));
        }
        out.extend_from_slice(&(b.len() as u32).to_le_bytes());
        out.extend_from_slice(b);
    }
    Ok(out)
}

pub fn decode_batch(bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
    if bytes.len() < 12 {
        return Err(Error::Message("RCSB truncated header".into()));
    }
    if &bytes[0..4] != BATCH_MAGIC {
        return Err(Error::Message("RCSB bad magic".into()));
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    if version != BATCH_VERSION {
        return Err(Error::Message(format!(
            "RCSB unsupported version {version}"
        )));
    }
    let n = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
    // Each entry is a length prefix plus a blob. Reserving `n` before that
    // check turns a 12-byte header into a multi-gigabyte `Vec`.
    let rest = bytes.len().saturating_sub(12);
    if n > rest / 4 {
        return Err(Error::Message("RCSB count exceeds the buffer".into()));
    }
    let mut off = 12usize;
    let mut out = Vec::with_capacity(n);
    for _ in 0..n {
        if off + 4 > bytes.len() {
            return Err(Error::Message("RCSB truncated length".into()));
        }
        let ln = u32::from_le_bytes(bytes[off..off + 4].try_into().unwrap()) as usize;
        off += 4;
        if off + ln > bytes.len() {
            return Err(Error::Message("RCSB truncated blob".into()));
        }
        out.push(bytes[off..off + ln].to_vec());
        off += ln;
    }
    Ok(out)
}

fn read_vec3_block(bytes: &[u8], n: usize) -> Result<Vec<[f64; 3]>> {
    let mut out = Vec::with_capacity(n);
    let mut i = 0;
    for _ in 0..n {
        let mut row = [0.0f64; 3];
        for slot in &mut row {
            let start = i;
            let end = start + 8;
            if end > bytes.len() {
                return Err(Error::Message("cooked SoA short block".into()));
            }
            *slot = f64::from_le_bytes(bytes[start..end].try_into().unwrap());
            i = end;
        }
        out.push(row);
    }
    Ok(out)
}

/// Lead bytes of one zstd frame holding several small RCSO blobs.
/// `0xFC` cannot start `RCSO`, so a per-frame blob is not read as a group.
pub const COOKED_GROUP_TAG: [u8; 2] = [0xFC, 0x01];

pub fn is_cooked_group(stored: &[u8]) -> bool {
    stored.starts_with(&COOKED_GROUP_TAG)
}

/// One zstd frame of length-prefixed RCSO blobs, in frame order.
pub fn pack_cooked_group(blobs: &[&[u8]]) -> Result<Vec<u8>> {
    let mut payload = Vec::new();
    for blob in blobs {
        if blob.len() > u32::MAX as usize {
            return Err(Error::Message("cooked blob exceeds u32".into()));
        }
        payload.extend_from_slice(&(blob.len() as u32).to_le_bytes());
        payload.extend_from_slice(blob);
    }
    let compressed = zstd::stream::encode_all(&payload[..], 1)
        .map_err(|e| Error::Message(format!("zstd encode: {e}")))?;
    let mut out = Vec::with_capacity(6 + compressed.len());
    out.extend_from_slice(&COOKED_GROUP_TAG);
    out.extend_from_slice(&(blobs.len() as u32).to_le_bytes());
    out.extend_from_slice(&compressed);
    Ok(out)
}

/// Decompressed RCSO blobs in pack order. `buf` is the plain payload;
/// `ranges` index it, so a single-frame read copies one blob.
pub struct CookedGroup {
    buf: Vec<u8>,
    ranges: Vec<std::ops::Range<usize>>,
}

impl CookedGroup {
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ranges.len()
    }

    pub fn member(&self, slot: usize) -> Option<&[u8]> {
        self.ranges.get(slot).map(|range| &self.buf[range.clone()])
    }
}

/// RCSO blobs in the order they were packed.
pub fn unpack_cooked_group(stored: &[u8]) -> Result<CookedGroup> {
    if stored.len() < 6 || !is_cooked_group(stored) {
        return Err(Error::Message("corrupt cooked group".into()));
    }
    let count = u32::from_le_bytes(stored[2..6].try_into().unwrap()) as usize;
    let buf = zstd::stream::decode_all(&stored[6..])
        .map_err(|e| Error::Message(format!("corrupt cooked group: {e}")))?;
    let mut ranges = Vec::with_capacity(count);
    let mut i = 0usize;
    while ranges.len() < count {
        if i + 4 > buf.len() {
            return Err(Error::Message("corrupt cooked group".into()));
        }
        let n = u32::from_le_bytes(buf[i..i + 4].try_into().unwrap()) as usize;
        i += 4;
        if i + n > buf.len() {
            return Err(Error::Message("corrupt cooked group".into()));
        }
        ranges.push(i..i + n);
        i += n;
    }
    if ranges.len() != count || i != buf.len() {
        return Err(Error::Message("corrupt cooked group".into()));
    }
    Ok(CookedGroup { buf, ranges })
}

#[cfg(test)]
fn resident_bytes() -> usize {
    let status = std::fs::read_to_string("/proc/self/status").expect("status");
    for line in status.lines() {
        let Some(rest) = line.strip_prefix("VmRSS:") else {
            continue;
        };
        let kb: usize = rest.split_whitespace().next().unwrap().parse().unwrap();
        return kb.saturating_mul(1024);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::*;
    use readcon_core::iterators::ConFrameIterator;
    use std::path::PathBuf;

    fn fixture(name: &str) -> String {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test")
            .join(name);
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn parsed_columns_are_contiguous_f64() {
        let text = fixture("tiny_cuh2_forces.con");
        let fr = ConFrameIterator::new(&text).next().unwrap().unwrap();
        assert!(fr.positions.f64_slice().is_some());
        assert_eq!(
            fr.positions.f64_slice().unwrap().len(),
            fr.atom_data.len() * 3
        );
        assert!(fr.forces.f64_slice().is_some());
        assert!(fr.masses.f64_slice().is_some());
    }

    #[test]
    fn encode_decode_positions_fixture() {
        let text = fixture("tiny_cuh2.con");
        let fr = ConFrameIterator::new(&text).next().unwrap().unwrap();
        let bytes = CookedSoa::encode_frame(&fr).unwrap();
        assert!(bytes.len() > HEADER_LEN);
        let cooked = CookedSoa::decode(&bytes).unwrap();
        assert_eq!(cooked.natoms as usize, fr.atom_data.len());
        for (i, a) in fr.atom_data.iter().enumerate() {
            assert_eq!(cooked.positions[i], crate::atom_coord::atom_xyz(a));
        }
        assert!(cooked.forces.is_none());
    }

    #[test]
    fn shuffled_forces_round_trip_every_bit() {
        let text = fixture("tiny_cuh2_vel_forces.con");
        let fr = ConFrameIterator::new(&text).next().unwrap().unwrap();
        let raw = CookedSoa::encode_frame(&fr).unwrap();
        let packed = compress_shuffled(&raw).unwrap();
        assert!(is_shuffled_zstd(&packed));
        let plain = MaterializedFrame::from_blob(&raw).unwrap();
        let back = decompress_shuffled(&packed).unwrap();
        assert_eq!(back.positions, plain.positions);
        assert_eq!(back.forces, plain.forces);
        assert_eq!(back.velocities, plain.velocities);
        assert_eq!(back.energy.to_bits(), plain.energy.to_bits());
        assert_eq!(&packed[..2], &SHUFFLE_ZSTD_SOA_TAG);
    }

    #[test]
    fn db20_interleaved_shuffle_still_round_trips() {
        let text = fixture("tiny_cuh2_vel_forces.con");
        let fr = ConFrameIterator::new(&text).next().unwrap().unwrap();
        let raw = CookedSoa::encode_frame(&fr).unwrap();
        let packed = compress_interleaved_shuffle(&raw).unwrap();
        assert_eq!(&packed[..2], &SHUFFLE_ZSTD_TAG);
        let plain = MaterializedFrame::from_blob(&raw).unwrap();
        let back = decompress_shuffled(&packed).unwrap();
        assert_eq!(back.positions, plain.positions);
        assert_eq!(back.forces, plain.forces);
        assert_eq!(back.velocities, plain.velocities);
        assert_eq!(back.energy.to_bits(), plain.energy.to_bits());
        assert!(MaterializedFrame::from_blob(&raw).is_some());
        assert!(!is_shuffled_zstd(&raw));
    }

    #[test]
    #[ignore]
    fn profile_component_decode() {
        let n = 100_000usize;
        let mut builder =
            readcon_core::types::ConFrameBuilder::new([10.5, 12.25, 8.0], [90.0, 90.0, 90.0]);
        let mut forces = Vec::with_capacity(n * 3);
        for i in 0..n {
            let x = i as f64 * 0.125;
            let y = if i == 0 { std::f64::consts::PI } else { 0.5 };
            let z = 1.0;
            builder.add_atom("Cu", x, y, z, [false, false, false], i as u64, 63.546);
            forces.extend_from_slice(&[x * 0.5, y * 0.5, z * 0.5]);
        }
        builder.set_forces_from_flat(&forces).unwrap();
        let mut fr = builder.build().unwrap();
        fr.header.set_energy(-42.5);
        let raw = CookedSoa::encode_frame(&fr).unwrap();
        let packed = compress_shuffled(&raw).unwrap();
        let rounds = 20u32;
        for _ in 0..3 {
            std::hint::black_box(decompress_shuffled(&packed).unwrap());
            std::hint::black_box(MaterializedFrame::from_blob(&raw).unwrap());
        }
        let t0 = std::time::Instant::now();
        for _ in 0..rounds {
            std::hint::black_box(decompress_shuffled(&packed).unwrap());
        }
        let dec = t0.elapsed().as_micros() / u128::from(rounds);
        let t1 = std::time::Instant::now();
        for _ in 0..rounds {
            std::hint::black_box(MaterializedFrame::from_blob(&raw).unwrap());
        }
        let blob = t1.elapsed().as_micros() / u128::from(rounds);
        eprintln!(
            "component decode {dec} us | from_blob {blob} us | packed {}",
            packed.len()
        );
    }

    #[test]
    fn encode_decode_forces_fixture() {
        let text = fixture("tiny_cuh2_forces.con");
        let fr = ConFrameIterator::new(&text).next().unwrap().unwrap();
        let bytes = CookedSoa::encode_frame(&fr).unwrap();
        let cooked = CookedSoa::decode(&bytes).unwrap();
        assert!(cooked.forces.is_some());
        let forces = cooked.forces.as_ref().unwrap();
        for (i, a) in fr.atom_data.iter().enumerate() {
            assert_eq!(cooked.positions[i], crate::atom_coord::atom_xyz(a));
            if let Some(f) = a.force() {
                assert_eq!(forces[i], f);
            }
        }
    }

    #[test]
    fn encode_decode_velocities_fixture() {
        let text = fixture("tiny_cuh2_vel_forces.con");
        let fr = ConFrameIterator::new(&text).next().unwrap().unwrap();
        let bytes = CookedSoa::encode_frame(&fr).unwrap();
        let cooked = CookedSoa::decode(&bytes).unwrap();
        assert!(cooked.forces.is_some());
        assert!(cooked.velocities.is_some());
        let forces = cooked.forces.as_ref().unwrap();
        let vels = cooked.velocities.as_ref().unwrap();
        for (i, a) in fr.atom_data.iter().enumerate() {
            assert_eq!(cooked.positions[i], crate::atom_coord::atom_xyz(a));
            if let Some(f) = a.force() {
                assert_eq!(forces[i], f);
            }
            if let Some(v) = a.velocity() {
                assert_eq!(vels[i], v);
            }
        }
        assert!((vels[0][0] - 0.001234).abs() < 1e-12);
        assert!((forces[0][0] - 0.123456).abs() < 1e-12);
    }

    #[test]
    fn bad_magic_rejected() {
        assert!(CookedSoa::decode(b"XXXX").is_err());
        assert!(CookedSoa::try_decode(b"XXXX").is_none());
    }

    #[test]
    fn rcsb_batch_roundtrip() {
        let a = vec![1u8, 2, 3];
        let b = vec![4u8, 5];
        let enc = encode_batch(&[a.clone(), b.clone()]).unwrap();
        let dec = decode_batch(&enc).unwrap();
        assert_eq!(dec, vec![a, b]);
        assert!(decode_batch(&enc[..8]).is_err());
    }

    #[test]
    fn declared_batch_count_does_not_allocate() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(BATCH_MAGIC);
        bytes.extend_from_slice(&BATCH_VERSION.to_le_bytes());
        bytes.extend_from_slice(&8_000_000u32.to_le_bytes());
        let before = resident_bytes();
        let err = decode_batch(&bytes).unwrap_err();
        let after = resident_bytes();
        assert!(err.to_string().contains("exceeds the buffer"), "{err}");
        assert!(
            after < before.saturating_add(8 << 20),
            "resident {before} -> {after}"
        );
    }

    fn synthetic_rcso(n: usize, velocities: bool) -> Vec<u8> {
        let mut builder =
            readcon_core::types::ConFrameBuilder::new([10.5, 12.25, 8.0], [90.0, 90.0, 90.0]);
        for i in 0..n {
            builder.add_atom(
                "Cu",
                i as f64 * 0.125,
                0.5,
                1.0,
                [false, false, false],
                i as u64,
                63.546,
            );
        }
        let forces: Vec<f64> = (0..n)
            .flat_map(|i| {
                let x = i as f64 * 0.125;
                [x * 0.5, 0.25, 0.5]
            })
            .collect();
        builder.set_forces_from_flat(&forces).unwrap();
        if velocities {
            for (i, slot) in builder.velocities_mut().iter_mut().enumerate() {
                *slot = i as f64 * 0.01;
            }
        }
        let mut fr = builder.build().unwrap();
        fr.header.set_energy(-42.5);
        CookedSoa::encode_frame(&fr).unwrap()
    }

    #[test]
    fn column_counts_around_the_parallel_threshold_round_trip() {
        for (n, velocities) in [
            (16_383usize, false),
            (16_384, false),
            (16_385, false),
            (16_384, true),
        ] {
            let raw = synthetic_rcso(n, velocities);
            let packed = compress_shuffled(&raw).unwrap();
            let plain = MaterializedFrame::from_blob(&raw).unwrap();
            let back = decompress_shuffled(&packed).unwrap();
            assert_eq!(back.positions, plain.positions, "n={n} vel={velocities}");
            assert_eq!(back.forces, plain.forces, "n={n}");
            if velocities {
                assert_eq!(back.velocities, plain.velocities, "n={n}");
                let n3 = n * 3;
                let mut positions = vec![0f64; n3];
                let mut forces = vec![0f64; n3];
                let mut vels = vec![0f64; n3];
                let meta =
                    decompress_shuffled_into(&packed, &mut positions, &mut forces, &mut vels)
                        .unwrap();
                assert!(meta.positions.is_empty());
                assert_eq!(positions, plain.positions);
                assert_eq!(forces, plain.forces);
                assert_eq!(vels, plain.velocities);
                assert!(meta.velocities_set);
            }
        }
    }

    #[test]
    fn dropped_truncated_and_corrupt_components_are_rejected() {
        let raw = synthetic_rcso(16_384, true);
        let packed = compress_shuffled(&raw).unwrap();
        let n3 = 16_384 * 3;
        let mut positions = vec![0f64; n3];
        let mut forces = vec![0f64; n3];
        let mut vels = vec![1f64; n3];

        let mut dropped = packed.clone();
        dropped[10..14].copy_from_slice(&6u32.to_le_bytes());
        let err =
            decompress_shuffled_into(&dropped, &mut positions, &mut forces, &mut vels).unwrap_err();
        assert!(err.to_string().contains("length mismatch"), "{err}");
        assert!(decompress_shuffled(&dropped).is_err());
        assert_eq!(vels[0], 1.0, "a rejected frame must not look decoded");

        assert!(decompress_shuffled(&packed[..packed.len() / 2]).is_err());
        assert!(
            decompress_shuffled_into(&packed[..80], &mut positions, &mut forces, &mut vels)
                .is_err()
        );

        let mut corrupt = packed.clone();
        let magic = corrupt
            .windows(4)
            .rposition(|window| window == COOKED_MAGIC)
            .unwrap();
        corrupt[magic] = b'X';
        assert!(decompress_shuffled(&corrupt).is_err());
        assert!(
            decompress_shuffled_into(&corrupt, &mut positions, &mut forces, &mut vels).is_err()
        );
    }

    /// Length of each component, then the offset of the trailing RCSO header.
    fn component_parts(packed: &[u8]) -> (Vec<(usize, usize)>, usize) {
        let ncomp = u32::from_le_bytes(packed[10..14].try_into().unwrap()) as usize;
        let mut off = 14usize;
        let mut parts = Vec::with_capacity(ncomp);
        for _ in 0..ncomp {
            let comp_len = u32::from_le_bytes(packed[off..off + 4].try_into().unwrap()) as usize;
            off += 4;
            parts.push((off, comp_len));
            off += comp_len;
        }
        (parts, off)
    }

    /// Shorten one component's payload and its length prefix. Later parts stay aligned.
    fn cut_component(packed: &[u8], part: usize, new_len: usize) -> Vec<u8> {
        let (parts, _) = component_parts(packed);
        let (start, comp_len) = parts[part];
        assert!(
            new_len < comp_len,
            "cut {new_len} is not shorter than {comp_len}"
        );
        let mut out = Vec::with_capacity(packed.len() - (comp_len - new_len));
        out.extend_from_slice(&packed[..start - 4]);
        out.extend_from_slice(&(new_len as u32).to_le_bytes());
        out.extend_from_slice(&packed[start..start + new_len]);
        out.extend_from_slice(&packed[start + comp_len..]);
        out
    }

    /// One more length-prefixed part, and a component count that includes it.
    fn extra_component(packed: &[u8]) -> Vec<u8> {
        let (_, header) = component_parts(packed);
        let mut out = packed.to_vec();
        let ncomp = u32::from_le_bytes(out[10..14].try_into().unwrap());
        out[10..14].copy_from_slice(&(ncomp + 1).to_le_bytes());
        let dummy = [0u8, 1, 2, 3];
        let mut inserted = Vec::with_capacity(4 + dummy.len());
        inserted.extend_from_slice(&(dummy.len() as u32).to_le_bytes());
        inserted.extend_from_slice(&dummy);
        out.splice(header..header, inserted);
        out
    }

    #[test]
    fn cut_and_extra_components_around_the_threshold_are_rejected() {
        for n in [16_383usize, 16_385] {
            let raw = synthetic_rcso(n, true);
            let packed = compress_shuffled(&raw).unwrap();
            let n3 = n * 3;
            let mut positions = vec![0f64; n3];
            let mut forces = vec![0f64; n3];
            let mut vels = vec![7f64; n3];
            for cut in [3usize, 8] {
                let bad = cut_component(&packed, 0, cut);
                let err = decompress_shuffled_into(&bad, &mut positions, &mut forces, &mut vels)
                    .unwrap_err();
                assert!(
                    err.to_string().contains("zstd") || err.to_string().contains("length"),
                    "n={n} cut={cut}: {err}"
                );
                assert!(decompress_shuffled(&bad).is_err(), "n={n} cut={cut}");
                assert_eq!(positions[0], 0.0, "n={n} cut={cut} wrote positions");
                assert_eq!(forces[0], 0.0, "n={n} cut={cut} wrote forces");
                assert_eq!(vels[0], 7.0, "n={n} cut={cut} wrote velocities");
            }
            let extra = extra_component(&packed);
            let err = decompress_shuffled_into(&extra, &mut positions, &mut forces, &mut vels)
                .unwrap_err();
            assert!(err.to_string().contains("length mismatch"), "n={n}: {err}");
            assert!(decompress_shuffled(&extra).is_err(), "n={n} extra");
            assert_eq!(positions[0], 0.0, "n={n} extra wrote positions");
            assert_eq!(vels[0], 7.0, "n={n} extra wrote velocities");
        }
    }

    fn replace_component(packed: &[u8], part: usize, plain: &[u8]) -> Vec<u8> {
        let (parts, _) = component_parts(packed);
        let (start, comp_len) = parts[part];
        let compressed = zstd::bulk::compress(plain, 3).unwrap();
        let mut out = Vec::new();
        out.extend_from_slice(&packed[..start - 4]);
        out.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        out.extend_from_slice(&compressed);
        out.extend_from_slice(&packed[start + comp_len..]);
        out
    }

    #[test]
    fn valid_zstd_with_the_wrong_decoded_length_is_rejected() {
        let n = 64usize;
        let raw = synthetic_rcso(n, false);
        let packed = compress_shuffled(&raw).unwrap();
        let (parts, _) = component_parts(&packed);
        let (start, comp_len) = parts[0];
        let plain = zstd::stream::decode_all(&packed[start..start + comp_len]).unwrap();
        assert_eq!(plain.len(), n * 8);
        let bad = replace_component(&packed, 0, &plain[..plain.len() - 8]);
        let n3 = n * 3;
        let mut positions = vec![3f64; n3];
        let mut forces = vec![3f64; n3];
        let err = decompress_shuffled_into(&bad, &mut positions, &mut forces, &mut []).unwrap_err();
        assert!(err.to_string().contains("wrong length"), "{err}");
        assert_eq!(positions[0], 3.0);
        assert_eq!(forces[0], 3.0);
    }

    #[test]
    fn a_short_later_part_does_not_write_earlier_columns() {
        for n in [16_383usize, 16_384, 16_385] {
            let raw = synthetic_rcso(n, true);
            let packed = compress_shuffled(&raw).unwrap();
            let (parts, _) = component_parts(&packed);
            assert!(parts.len() > 6, "n={n} parts {}", parts.len());
            let n3 = n * 3;
            for part in [3usize, 6] {
                let bad = cut_component(&packed, part, 3);
                let mut positions = vec![5f64; n3];
                let mut forces = vec![5f64; n3];
                let mut vels = vec![5f64; n3];
                let err = decompress_shuffled_into(&bad, &mut positions, &mut forces, &mut vels)
                    .unwrap_err();
                assert!(
                    err.to_string().contains("zstd") || err.to_string().contains("length"),
                    "n={n} part={part}: {err}"
                );
                assert_eq!(positions[0], 5.0, "n={n} part={part} wrote positions");
                assert_eq!(forces[n3 - 1], 5.0, "n={n} part={part} wrote forces");
                assert_eq!(vels[0], 5.0, "n={n} part={part} wrote velocities");
            }
        }
    }

    #[test]
    #[ignore]
    fn record_db23_levers() {
        let n = 100_000usize;
        let raw = synthetic_rcso(n, false);
        let (_ntot, _flags, npos, nfor, _nvel) = coord_layout(&raw).unwrap();
        let coords = &raw[HEADER_LEN..HEADER_LEN + (npos + nfor) * 8];
        let pos = le_f64s(&coords[..npos * 8]);
        let frc = le_f64s(&coords[npos * 8..]);
        let mut cols = Vec::new();
        for src in [&pos, &frc] {
            for lane in 0..3 {
                let mut gather = vec![0f64; n];
                for i in 0..n {
                    gather[i] = src[i * 3 + lane];
                }
                let mut shuffled = vec![0u8; n * 8];
                crate::byte_shuffle::shuffle_bytes(le_f64_bytes(&gather), &mut shuffled);
                cols.push(shuffled);
            }
        }
        let rounds = 20u32;
        let time = |body: &mut dyn FnMut()| {
            for _ in 0..3 {
                body();
            }
            let t0 = std::time::Instant::now();
            for _ in 0..rounds {
                body();
            }
            t0.elapsed().as_micros() / u128::from(rounds)
        };
        let scope_us = time(&mut || {
            std::thread::scope(|scope| {
                scope.spawn(|| {});
                scope.spawn(|| {});
            });
        });
        let mut rows = Vec::new();
        for (level, window) in [(3, 20u32), (3, 12), (1, 12), (-5, 12), (-1, 12)] {
            let mut compressor = zstd::bulk::Compressor::new(level).unwrap();
            compressor
                .set_parameter(zstd::zstd_safe::CParameter::WindowLog(window))
                .unwrap();
            compressor
                .set_parameter(zstd::zstd_safe::CParameter::EnableLongDistanceMatching(
                    false,
                ))
                .unwrap();
            let mut parts = Vec::new();
            let mut bytes = 0usize;
            for col in &cols {
                let part = compressor.compress(col).unwrap();
                bytes += part.len();
                parts.push(part);
            }
            let mut decs: Vec<_> = (0..6)
                .map(|_| zstd::bulk::Decompressor::new().unwrap())
                .collect();
            let mut bufs = vec![vec![0u8; n * 8]; 6];
            let seq = time(&mut || {
                for i in 0..6 {
                    decs[i]
                        .decompress_to_buffer(&parts[i], &mut bufs[i])
                        .unwrap();
                }
                std::hint::black_box(bufs[0][0]);
            });
            let par = time(&mut || {
                std::thread::scope(|scope| {
                    let (left, right) = parts.split_at(3);
                    let (buf_l, buf_r) = bufs.split_at_mut(3);
                    let (dec_l, dec_r) = decs.split_at_mut(3);
                    scope.spawn(move || {
                        for i in 0..3 {
                            dec_l[i]
                                .decompress_to_buffer(&left[i], &mut buf_l[i])
                                .unwrap();
                        }
                    });
                    scope.spawn(move || {
                        for i in 0..3 {
                            dec_r[i]
                                .decompress_to_buffer(&right[i], &mut buf_r[i])
                                .unwrap();
                        }
                    });
                });
                std::hint::black_box(bufs[0][0]);
            });
            rows.push(format!(
                "{{\"level\":{level},\"window\":{window},\"ldm\":false,\"bytes\":{bytes},\"seq_us\":{seq},\"two_thread_us\":{par}}}"
            ));
        }
        let mut compressor = zstd::bulk::Compressor::new(3).unwrap();
        compressor
            .set_parameter(zstd::zstd_safe::CParameter::WindowLog(12))
            .unwrap();
        let mut high_bytes = 0usize;
        let mut high_parts = Vec::new();
        for col in &cols {
            let mut high = vec![0u8; n * 4];
            high[..n].copy_from_slice(&col[4 * n..5 * n]);
            high[n..2 * n].copy_from_slice(&col[5 * n..6 * n]);
            high[2 * n..3 * n].copy_from_slice(&col[6 * n..7 * n]);
            high[3 * n..].copy_from_slice(&col[7 * n..]);
            let part = compressor.compress(&high).unwrap();
            high_bytes += part.len();
            high_parts.push(part);
        }
        let mut dec = zstd::bulk::Decompressor::new().unwrap();
        let mut hbuf = vec![0u8; n * 4];
        let low = vec![0u8; n * 4];
        let mut sink = vec![0u8; n * 8];
        let high_us = time(&mut || {
            for part in &high_parts {
                dec.decompress_to_buffer(part, &mut hbuf).unwrap();
                sink[..n * 4].copy_from_slice(&low);
                sink[n * 4..].copy_from_slice(&hbuf);
            }
            std::hint::black_box(sink[0]);
        });
        println!(
            "{{\"scope_two_threads_us\":{scope_us},\"raw_force_bytes\":{},\"high_byte_zstd_bytes\":{high_bytes},\"high_byte_raw_low_bytes\":{},\"high_byte_decode_copy_us\":{high_us},\"rows\":[{}]}}",
            nfor * 8,
            cols.len() * n * 4,
            rows.join(",")
        );
    }

    /// Same rows as [`record_db23_levers`], with the 20 timed rounds kept.
    /// The mean alone cannot be compared with an IQR from another row.
    #[test]
    #[ignore]
    fn record_db24_levers() {
        fn type7(samples: &[u128], p: f64) -> f64 {
            let mut vals = samples.to_vec();
            vals.sort_unstable();
            let n = vals.len();
            if n == 1 {
                return vals[0] as f64;
            }
            let h = (n - 1) as f64 * p;
            let lo = h.floor() as usize;
            let hi = h.ceil() as usize;
            let w = h - lo as f64;
            vals[lo] as f64 * (1.0 - w) + vals[hi] as f64 * w
        }
        fn summarize(samples: &[u128]) -> String {
            let mean = samples.iter().sum::<u128>() / samples.len() as u128;
            let iqr = type7(samples, 0.75) - type7(samples, 0.25);
            let list = samples
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",");
            format!("\"mean_us\":{mean},\"iqr_us\":{iqr},\"samples_us\":[{list}]")
        }
        let n = 100_000usize;
        let raw = synthetic_rcso(n, false);
        let (_ntot, _flags, npos, nfor, _nvel) = coord_layout(&raw).unwrap();
        let coords = &raw[HEADER_LEN..HEADER_LEN + (npos + nfor) * 8];
        let pos = le_f64s(&coords[..npos * 8]);
        let frc = le_f64s(&coords[npos * 8..]);
        let mut cols = Vec::new();
        for src in [&pos, &frc] {
            for lane in 0..3 {
                let mut gather = vec![0f64; n];
                for i in 0..n {
                    gather[i] = src[i * 3 + lane];
                }
                let mut shuffled = vec![0u8; n * 8];
                crate::byte_shuffle::shuffle_bytes(le_f64_bytes(&gather), &mut shuffled);
                cols.push(shuffled);
            }
        }
        let sample = |body: &mut dyn FnMut()| -> Vec<u128> {
            for _ in 0..3 {
                body();
            }
            let mut out = Vec::with_capacity(20);
            for _ in 0..20 {
                let t0 = std::time::Instant::now();
                body();
                out.push(t0.elapsed().as_micros());
            }
            out
        };
        let scope = sample(&mut || {
            std::thread::scope(|scope| {
                scope.spawn(|| {});
                scope.spawn(|| {});
            });
        });
        let mut rows = Vec::new();
        for (level, window) in [(3, 20u32), (3, 12), (1, 12), (-5, 12), (-1, 12)] {
            let mut compressor = zstd::bulk::Compressor::new(level).unwrap();
            compressor
                .set_parameter(zstd::zstd_safe::CParameter::WindowLog(window))
                .unwrap();
            compressor
                .set_parameter(zstd::zstd_safe::CParameter::EnableLongDistanceMatching(
                    false,
                ))
                .unwrap();
            let mut parts = Vec::new();
            let mut bytes = 0usize;
            for col in &cols {
                let part = compressor.compress(col).unwrap();
                bytes += part.len();
                parts.push(part);
            }
            let mut decs: Vec<_> = (0..6)
                .map(|_| zstd::bulk::Decompressor::new().unwrap())
                .collect();
            let mut bufs = vec![vec![0u8; n * 8]; 6];
            let seq = sample(&mut || {
                for i in 0..6 {
                    decs[i]
                        .decompress_to_buffer(&parts[i], &mut bufs[i])
                        .unwrap();
                }
                std::hint::black_box(bufs[0][0]);
            });
            let par = sample(&mut || {
                std::thread::scope(|scope| {
                    let (left, right) = parts.split_at(3);
                    let (buf_l, buf_r) = bufs.split_at_mut(3);
                    let (dec_l, dec_r) = decs.split_at_mut(3);
                    scope.spawn(move || {
                        for i in 0..3 {
                            dec_l[i]
                                .decompress_to_buffer(&left[i], &mut buf_l[i])
                                .unwrap();
                        }
                    });
                    scope.spawn(move || {
                        for i in 0..3 {
                            dec_r[i]
                                .decompress_to_buffer(&right[i], &mut buf_r[i])
                                .unwrap();
                        }
                    });
                });
                std::hint::black_box(bufs[0][0]);
            });
            rows.push(format!(
                "{{\"level\":{level},\"window\":{window},\"ldm\":false,\"bytes\":{bytes},\"seq\":{{{}}},\"two_threads\":{{{}}}}}",
                summarize(&seq),
                summarize(&par)
            ));
        }
        let mut compressor = zstd::bulk::Compressor::new(3).unwrap();
        compressor
            .set_parameter(zstd::zstd_safe::CParameter::WindowLog(12))
            .unwrap();
        let mut high_bytes = 0usize;
        let mut high_parts = Vec::new();
        for col in &cols {
            let mut high = vec![0u8; n * 4];
            high[..n].copy_from_slice(&col[4 * n..5 * n]);
            high[n..2 * n].copy_from_slice(&col[5 * n..6 * n]);
            high[2 * n..3 * n].copy_from_slice(&col[6 * n..7 * n]);
            high[3 * n..].copy_from_slice(&col[7 * n..]);
            let part = compressor.compress(&high).unwrap();
            high_bytes += part.len();
            high_parts.push(part);
        }
        let mut dec = zstd::bulk::Decompressor::new().unwrap();
        let mut hbuf = vec![0u8; n * 4];
        let low = vec![0u8; n * 4];
        let mut sink = vec![0u8; n * 8];
        let high = sample(&mut || {
            for part in &high_parts {
                dec.decompress_to_buffer(part, &mut hbuf).unwrap();
                sink[..n * 4].copy_from_slice(&low);
                sink[n * 4..].copy_from_slice(&hbuf);
            }
            std::hint::black_box(sink[0]);
        });
        println!(
            "{{\"rounds\":20,\"untimed\":3,\"scope_two_threads\":{{{}}},\"raw_force_bytes\":{},\"high_byte_zstd_bytes\":{high_bytes},\"high_byte_raw_low_bytes\":{},\"high_byte_decode_copy\":{{{}}},\"rows\":[{}]}}",
            summarize(&scope),
            nfor * 8,
            cols.len() * n * 4,
            summarize(&high),
            rows.join(",")
        );
    }
}
