//! One columnar blob per trajectory for the scalar secondary indexes.
//!
//! The old `idx_*` tables store the frame key inside every B-tree key, so a
//! 1 000-frame file spends a page header on each copy. Energy, mass, volume
//! and the other scalars are fixed-width, so they pack as parallel arrays.
//! The energy array is the first payload: a metadata query reads that prefix
//! and does not touch the later columns.
//!
//! `hash_by_frame` is not stored. The content hash sits in this blob, and a
//! missing hash is still the hash of the CON text.

use crate::error::{Error, Result};

const MAGIC: &[u8; 4] = b"IXC1";
const META_CHANNELS: usize = 7;

/// One frame's scalar index fields. Zero `u64` columns mean "not present".
/// `pbc == 0xff` means the frame stored no PBC mask.
#[derive(Clone, Debug)]
pub struct ScalarSlot {
    /// Atom count.
    pub natoms: u32,
    /// Ordered bits of a finite energy, or 0.
    pub energy: u64,
    /// Ordered bits of a finite max force, or 0.
    pub fmax: u64,
    /// Ordered bits of a finite total mass, or 0.
    pub mass: u64,
    /// Ordered bits of a finite cell volume, or 0.
    pub volume: u64,
    /// Low bits: forces, velocities, energy. `0x80` marks a real frame.
    pub flags: u8,
    /// PBC mask, or `0xff` when the frame has no mask.
    pub pbc: u8,
    /// Ordered bits for meta channels 1..=7, or 0.
    pub meta: [u64; META_CHANNELS],
    /// xxHash3-128 of the CON text.
    pub hash: [u8; 16],
    /// Element symbol and count, in index order.
    pub symbols: Vec<(String, u32)>,
}

impl Default for ScalarSlot {
    fn default() -> Self {
        Self {
            natoms: 0,
            energy: 0,
            fmax: 0,
            mass: 0,
            volume: 0,
            flags: 0,
            pbc: 0xff,
            meta: [0; META_CHANNELS],
            hash: [0; 16],
            symbols: Vec::new(),
        }
    }
}

/// In-memory columns for one trajectory. Slot `i` is frame index `i`.
#[derive(Clone, Debug)]
pub struct Columns {
    n: usize,
    energy: Vec<u64>,
    fmax: Vec<u64>,
    mass: Vec<u64>,
    volume: Vec<u64>,
    natoms: Vec<u32>,
    flags: Vec<u8>,
    pbc: Vec<u8>,
    meta: Vec<[u64; META_CHANNELS]>,
    hash: Vec<[u8; 16]>,
    symbols: Vec<Vec<(String, u32)>>,
}

impl Columns {
    /// No frames yet.
    pub fn new() -> Self {
        Self::empty()
    }

    fn empty() -> Self {
        Self {
            n: 0,
            energy: Vec::new(),
            fmax: Vec::new(),
            mass: Vec::new(),
            volume: Vec::new(),
            natoms: Vec::new(),
            flags: Vec::new(),
            pbc: Vec::new(),
            meta: Vec::new(),
            hash: Vec::new(),
            symbols: Vec::new(),
        }
    }

    fn grow(&mut self, n: usize) {
        if n <= self.n {
            return;
        }
        self.energy.resize(n, 0);
        self.fmax.resize(n, 0);
        self.mass.resize(n, 0);
        self.volume.resize(n, 0);
        self.natoms.resize(n, 0);
        self.flags.resize(n, 0);
        self.pbc.resize(n, 0xff);
        self.meta.resize(n, [0; META_CHANNELS]);
        self.hash.resize(n, [0; 16]);
        self.symbols.resize(n, Vec::new());
        self.n = n;
    }

    /// Drop one frame. A later [`Self::set`] can fill the slot again.
    pub fn clear_slot(&mut self, frame_idx: u32) {
        let i = frame_idx as usize;
        if i >= self.n {
            return;
        }
        self.energy[i] = 0;
        self.fmax[i] = 0;
        self.mass[i] = 0;
        self.volume[i] = 0;
        self.natoms[i] = 0;
        self.flags[i] = 0;
        self.pbc[i] = 0xff;
        self.meta[i] = [0; META_CHANNELS];
        self.hash[i] = [0; 16];
        self.symbols[i].clear();
    }

    /// Replace the content hash of one slot. The slot must already be occupied.
    #[cfg(test)]
    pub fn set_hash(&mut self, frame_idx: u32, hash: [u8; 16]) {
        self.hash[frame_idx as usize] = hash;
    }

    /// Replace the slot for `frame_idx`.
    pub fn set(&mut self, frame_idx: u32, slot: ScalarSlot) {
        let i = frame_idx as usize;
        self.grow(i + 1);
        self.energy[i] = slot.energy;
        self.fmax[i] = slot.fmax;
        self.mass[i] = slot.mass;
        self.volume[i] = slot.volume;
        self.natoms[i] = slot.natoms;
        self.flags[i] = slot.flags | 0x80;
        self.pbc[i] = slot.pbc;
        self.meta[i] = slot.meta;
        self.hash[i] = slot.hash;
        self.symbols[i] = slot.symbols;
    }

    /// Encode. The energy array starts at byte 8.
    pub fn encode(&self) -> Result<Vec<u8>> {
        let n = self.n;
        let symbols_at = symbols_offset(n);
        let mut out = Vec::with_capacity(symbols_at + n * 8);
        out.extend_from_slice(MAGIC);
        out.extend_from_slice(&(n as u32).to_le_bytes());
        push_u64s(&mut out, &self.energy);
        push_u64s(&mut out, &self.fmax);
        push_u64s(&mut out, &self.mass);
        push_u64s(&mut out, &self.volume);
        for v in &self.natoms {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&self.flags);
        out.extend_from_slice(&self.pbc);
        for slot in &self.meta {
            push_u64s(&mut out, slot);
        }
        for h in &self.hash {
            out.extend_from_slice(h);
        }
        debug_assert_eq!(out.len(), symbols_at);
        for syms in &self.symbols {
            let count = u16::try_from(syms.len())
                .map_err(|_| Error::Message("too many symbols in one frame".into()))?;
            out.extend_from_slice(&count.to_le_bytes());
            for (sym, n_atoms) in syms {
                let bytes = sym.as_bytes();
                if bytes.len() > u8::MAX as usize {
                    return Err(Error::Message("symbol longer than 255 bytes".into()));
                }
                out.push(bytes.len() as u8);
                out.extend_from_slice(bytes);
                out.extend_from_slice(&n_atoms.to_le_bytes());
            }
        }
        Ok(out)
    }
}

fn push_u64s(out: &mut Vec<u8>, vals: &[u64]) {
    for v in vals {
        out.extend_from_slice(&v.to_le_bytes());
    }
}

fn symbols_offset(n: usize) -> usize {
    8 + n * 8 * 4 + n * 4 + n + n + n * 8 * META_CHANNELS + n * 16
}

fn header_n(blob: &[u8]) -> Result<usize> {
    if blob.len() < 8 || &blob[..4] != MAGIC {
        return Err(Error::Message("corrupt scalar columns".into()));
    }
    let n = u32::from_le_bytes(blob[4..8].try_into().unwrap()) as usize;
    if blob.len() < symbols_offset(n) {
        return Err(Error::Message("corrupt scalar columns".into()));
    }
    Ok(n)
}

/// Frame indexes whose energy bits lie in `lo..=hi`.
///
/// Reads only the energy prefix. A short or non-column blob is an error.
pub fn energy_frame_indexes(blob: &[u8], lo: u64, hi: u64) -> Result<Vec<u32>> {
    let n = header_n(blob)?;
    let mut out = Vec::new();
    for i in 0..n {
        let start = 8 + i * 8;
        let bits = u64::from_le_bytes(blob[start..start + 8].try_into().unwrap());
        if bits != 0 && bits >= lo && bits <= hi {
            out.push(i as u32);
        }
    }
    Ok(out)
}

fn u64_at(blob: &[u8], off: usize, i: usize) -> u64 {
    let start = off + i * 8;
    u64::from_le_bytes(blob[start..start + 8].try_into().unwrap())
}

/// Decode one blob. Used when a query needs a column other than energy,
/// and when a later append extends the trajectory.
pub fn decode(blob: &[u8]) -> Result<Columns> {
    let n = header_n(blob)?;
    let mut c = Columns::empty();
    c.grow(n);
    let mut off = 8;
    for i in 0..n {
        c.energy[i] = u64_at(blob, off, i);
    }
    off += n * 8;
    for i in 0..n {
        c.fmax[i] = u64_at(blob, off, i);
    }
    off += n * 8;
    for i in 0..n {
        c.mass[i] = u64_at(blob, off, i);
    }
    off += n * 8;
    for i in 0..n {
        c.volume[i] = u64_at(blob, off, i);
    }
    off += n * 8;
    for i in 0..n {
        let start = off + i * 4;
        c.natoms[i] = u32::from_le_bytes(blob[start..start + 4].try_into().unwrap());
    }
    off += n * 4;
    c.flags.copy_from_slice(&blob[off..off + n]);
    off += n;
    c.pbc.copy_from_slice(&blob[off..off + n]);
    off += n;
    for ch in 0..META_CHANNELS {
        for i in 0..n {
            c.meta[i][ch] = u64_at(blob, off, i);
        }
        off += n * 8;
    }
    for i in 0..n {
        let start = off + i * 16;
        c.hash[i].copy_from_slice(&blob[start..start + 16]);
    }
    off += n * 16;
    for i in 0..n {
        if off + 2 > blob.len() {
            return Err(Error::Message("corrupt scalar columns".into()));
        }
        let count = u16::from_le_bytes(blob[off..off + 2].try_into().unwrap()) as usize;
        off += 2;
        let mut syms = Vec::with_capacity(count);
        for _ in 0..count {
            if off >= blob.len() {
                return Err(Error::Message("corrupt scalar columns".into()));
            }
            let len = blob[off] as usize;
            off += 1;
            if off + len + 4 > blob.len() {
                return Err(Error::Message("corrupt scalar columns".into()));
            }
            let sym = std::str::from_utf8(&blob[off..off + len])
                .map_err(|_| Error::Message("corrupt scalar columns".into()))?
                .to_string();
            off += len;
            let n_atoms = u32::from_le_bytes(blob[off..off + 4].try_into().unwrap());
            off += 4;
            syms.push((sym, n_atoms));
        }
        c.symbols[i] = syms;
    }
    Ok(c)
}

/// Content hash stored for `frame_idx`, if that slot is occupied.
pub fn stored_hash(blob: &[u8], frame_idx: u32) -> Result<Option<[u8; 16]>> {
    let n = header_n(blob)?;
    let i = frame_idx as usize;
    if i >= n {
        return Ok(None);
    }
    let flags_off = 8 + n * 8 * 4 + n * 4;
    if blob[flags_off + i] & 0x80 == 0 {
        return Ok(None);
    }
    let hash_off = symbols_offset(n) - n * 16;
    let start = hash_off + i * 16;
    let mut hash = [0u8; 16];
    hash.copy_from_slice(&blob[start..start + 16]);
    Ok(Some(hash))
}

/// Occupied frame indexes whose stored hash equals `want`.
pub fn frames_with_hash(blob: &[u8], want: &[u8; 16]) -> Result<Vec<u32>> {
    let n = header_n(blob)?;
    let flags_off = 8 + n * 8 * 4 + n * 4;
    let hash_off = symbols_offset(n) - n * 16;
    let mut out = Vec::new();
    for i in 0..n {
        if blob[flags_off + i] & 0x80 == 0 {
            continue;
        }
        let start = hash_off + i * 16;
        if &blob[start..start + 16] == want {
            out.push(i as u32);
        }
    }
    Ok(out)
}

/// A borrowed view of one slot. Symbol pairs are parsed only when asked.
pub struct SlotView<'a> {
    /// Atom count.
    pub natoms: u32,
    /// Ordered energy bits, or 0.
    pub energy: u64,
    /// Ordered max-force bits, or 0.
    pub fmax: u64,
    /// Ordered mass bits, or 0.
    pub mass: u64,
    /// Ordered volume bits, or 0.
    pub volume: u64,
    /// Flag byte, including the occupied bit.
    pub flags: u8,
    /// PBC mask or `0xff`.
    pub pbc: u8,
    /// xxHash3-128 of the CON text.
    pub hash: [u8; 16],
    meta: &'a [u8],
    symbols: &'a [u8],
}

impl<'a> SlotView<'a> {
    /// Ordered bits of meta channel `channel` (1..=7), or 0.
    pub fn meta(&self, channel: u8) -> u64 {
        if channel == 0 || channel as usize > META_CHANNELS {
            return 0;
        }
        let start = (channel as usize - 1) * 8;
        u64::from_le_bytes(self.meta[start..start + 8].try_into().unwrap())
    }

    /// Element counts for this frame.
    pub fn symbols(&self) -> Result<Vec<(String, u32)>> {
        if self.symbols.len() < 2 {
            return Err(Error::Message("corrupt scalar columns".into()));
        }
        let count = u16::from_le_bytes(self.symbols[..2].try_into().unwrap()) as usize;
        let mut off = 2usize;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            if off >= self.symbols.len() {
                return Err(Error::Message("corrupt scalar columns".into()));
            }
            let len = self.symbols[off] as usize;
            off += 1;
            if off + len + 4 > self.symbols.len() {
                return Err(Error::Message("corrupt scalar columns".into()));
            }
            let sym = std::str::from_utf8(&self.symbols[off..off + len])
                .map_err(|_| Error::Message("corrupt scalar columns".into()))?
                .to_string();
            off += len;
            let n_atoms = u32::from_le_bytes(self.symbols[off..off + 4].try_into().unwrap());
            off += 4;
            out.push((sym, n_atoms));
        }
        Ok(out)
    }
}

/// Visit every occupied slot. `symbols` on the view is that frame's record only.
pub fn for_each_slot(blob: &[u8], mut body: impl FnMut(u32, SlotView<'_>)) -> Result<()> {
    let n = header_n(blob)?;
    let cols = decode(blob)?;
    let mut sym_off = symbols_offset(n);
    for i in 0..n {
        if sym_off + 2 > blob.len() {
            return Err(Error::Message("corrupt scalar columns".into()));
        }
        let count = u16::from_le_bytes(blob[sym_off..sym_off + 2].try_into().unwrap()) as usize;
        let mut end = sym_off + 2;
        for _ in 0..count {
            if end >= blob.len() {
                return Err(Error::Message("corrupt scalar columns".into()));
            }
            let len = blob[end] as usize;
            end += 1 + len + 4;
            if end > blob.len() {
                return Err(Error::Message("corrupt scalar columns".into()));
            }
        }
        if cols.flags[i] & 0x80 != 0 {
            let meta_off = 8 + n * 8 * 4 + n * 4 + n + n + i * 8 * META_CHANNELS;
            body(
                i as u32,
                SlotView {
                    natoms: cols.natoms[i],
                    energy: cols.energy[i],
                    fmax: cols.fmax[i],
                    mass: cols.mass[i],
                    volume: cols.volume[i],
                    flags: cols.flags[i],
                    pbc: cols.pbc[i],
                    hash: cols.hash[i],
                    meta: &blob[meta_off..meta_off + 8 * META_CHANNELS],
                    symbols: &blob[sym_off..end],
                },
            );
        }
        sym_off = end;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn energy_prefix_ignores_a_broken_tail() {
        let mut cols = Columns::empty();
        cols.set(
            1,
            ScalarSlot {
                energy: 0x1111,
                flags: 0x80,
                ..ScalarSlot::default()
            },
        );
        let mut blob = cols.encode().unwrap();
        let last = blob.len() - 1;
        blob[last] ^= 0xff;
        let hits = energy_frame_indexes(&blob, 0x1111, 0x1111).unwrap();
        assert_eq!(hits, vec![1]);
    }

    #[test]
    fn round_trip_keeps_symbols_and_hash() {
        let mut cols = Columns::empty();
        let mut slot = ScalarSlot {
            natoms: 8,
            energy: 42,
            fmax: 7,
            mass: 9,
            volume: 11,
            flags: 0x80 | 0x05,
            pbc: 0x05,
            meta: [0, 0, 3, 0, 0, 0, 0],
            hash: [9; 16],
            symbols: vec![("Cu".into(), 8)],
        };
        cols.set(0, slot.clone());
        slot.energy = 0;
        slot.symbols = vec![("H".into(), 2), ("Cu".into(), 1)];
        cols.set(3, slot);
        let blob = cols.encode().unwrap();
        let back = decode(&blob).unwrap();
        assert_eq!(back.n, 4);
        assert_eq!(back.energy[0], 42);
        assert_eq!(back.symbols[0], vec![("Cu".into(), 8)]);
        assert_eq!(back.symbols[3][0].0, "H");
        assert_eq!(back.hash[0], [9; 16]);
        assert_eq!(back.pbc[1], 0xff);
        assert!(back.flags[1] & 0x80 == 0);
        assert_eq!(stored_hash(&blob, 0).unwrap().unwrap(), [9; 16]);
        assert!(stored_hash(&blob, 1).unwrap().is_none());
    }
}
