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
            |a| crate::atom_coord::atom_xyz(a),
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
        let positions = take_f64(bytes, &mut off, n.checked_mul(3)?)?;
        let forces = if flags & FLAG_FORCES != 0 {
            take_f64(bytes, &mut off, n.checked_mul(3)?)?
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
            numbers.extend(std::iter::repeat(z).take(count as usize));
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

fn take_f64(bytes: &[u8], off: &mut usize, n: usize) -> Option<Vec<f64>> {
    let nbytes = n.checked_mul(8)?;
    let raw = bytes.get(*off..*off + nbytes)?;
    *off += nbytes;
    let mut out = vec![0.0; n];
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
        for c in 0..3 {
            let start = i;
            let end = start + 8;
            if end > bytes.len() {
                return Err(Error::Message("cooked SoA short block".into()));
            }
            row[c] = f64::from_le_bytes(bytes[start..end].try_into().unwrap());
            i = end;
        }
        out.push(row);
    }
    Ok(out)
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
