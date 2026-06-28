/// Stable trajectory identifier (user-assigned).
pub type TrajId = u64;
/// Zero-based frame index within a trajectory.
pub type FrameIdx = u32;

/// xxHash3 128-bit content fingerprint (exact match / dedup).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContentHash(pub [u8; 16]);

impl ContentHash {
    pub fn to_bytes(self) -> [u8; 16] {
        self.0
    }
    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() != 16 {
            return None;
        }
        let mut a = [0u8; 16];
        a.copy_from_slice(b);
        Some(Self(a))
    }
    pub fn to_hex(self) -> String {
        self.0.iter().map(|b| format!("{b:02x}")).collect()
    }
}

/// Hash canonical frame blob bytes (UTF-8 CON text as stored).
pub fn hash_frame_bytes(blob: &[u8]) -> ContentHash {
    use xxhash_rust::xxh3::xxh3_128;
    let h = xxh3_128(blob);
    ContentHash(h.to_le_bytes())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameKey {
    pub traj_id: TrajId,
    pub frame_idx: FrameIdx,
}

impl FrameKey {
    pub fn to_bytes(self) -> [u8; 12] {
        let mut out = [0u8; 12];
        out[..8].copy_from_slice(&self.traj_id.to_be_bytes());
        out[8..].copy_from_slice(&self.frame_idx.to_be_bytes());
        out
    }

    pub fn from_bytes(b: &[u8]) -> Option<Self> {
        if b.len() != 12 {
            return None;
        }
        let mut t = [0u8; 8];
        let mut f = [0u8; 4];
        t.copy_from_slice(&b[..8]);
        f.copy_from_slice(&b[8..]);
        Some(Self {
            traj_id: u64::from_be_bytes(t),
            frame_idx: u32::from_be_bytes(f),
        })
    }
}

/// Secondary index key: n_atoms (BE u32) || FrameKey bytes
pub(crate) fn natoms_key(n_atoms: u32, fk: FrameKey) -> [u8; 16] {
    let mut out = [0u8; 16];
    out[..4].copy_from_slice(&n_atoms.to_be_bytes());
    out[4..].copy_from_slice(&fk.to_bytes());
    out
}

/// symbol (utf-8) || 0xff || FrameKey
pub(crate) fn symbol_key(symbol: &str, fk: FrameKey) -> Vec<u8> {
    let mut v = symbol.as_bytes().to_vec();
    v.push(0xff);
    v.extend_from_slice(&fk.to_bytes());
    v
}

pub(crate) fn symbol_prefix(symbol: &str) -> Vec<u8> {
    let mut v = symbol.as_bytes().to_vec();
    v.push(0xff);
    v
}

/// Order-preserving map of finite f64 → u64 for BE key prefixes.
pub(crate) fn ordered_f64_bits(x: f64) -> Option<u64> {
    if !x.is_finite() {
        return None;
    }
    let bits = x.to_bits();
    Some(if x.is_sign_negative() {
        !bits
    } else {
        bits ^ (1u64 << 63)
    })
}

/// Quantize finite energy for ordered range scans (f64 bits, BE; NaN/Inf skipped at index time).
pub(crate) fn energy_bin_key(energy: f64, fk: FrameKey) -> Option<[u8; 20]> {
    let ordered = ordered_f64_bits(energy)?;
    let mut out = [0u8; 20];
    out[..8].copy_from_slice(&ordered.to_be_bytes());
    out[8..].copy_from_slice(&fk.to_bytes());
    Some(out)
}

/// Max force magnitude bin (only when forces exist and fmax is finite).
pub(crate) fn fmax_bin_key(fmax: f64, fk: FrameKey) -> Option<[u8; 20]> {
    energy_bin_key(fmax, fk)
}

/// Section / capability flag key: flag_id (u8) || FrameKey
pub(crate) fn flag_key(flag: u8, fk: FrameKey) -> [u8; 13] {
    let mut out = [0u8; 13];
    out[0] = flag;
    out[1..].copy_from_slice(&fk.to_bytes());
    out
}

pub(crate) const FLAG_HAS_FORCES: u8 = 1;
pub(crate) const FLAG_HAS_VELOCITIES: u8 = 2;
pub(crate) const FLAG_HAS_ENERGY: u8 = 3;

/// Per-element count: symbol || 0xff || BE u32 count || FrameKey
pub(crate) fn elem_count_key(symbol: &str, count: u32, fk: FrameKey) -> Vec<u8> {
    let mut v = symbol.as_bytes().to_vec();
    v.push(0xff);
    v.extend_from_slice(&count.to_be_bytes());
    v.extend_from_slice(&fk.to_bytes());
    v
}

pub(crate) fn elem_count_symbol_prefix(symbol: &str) -> Vec<u8> {
    let mut v = symbol.as_bytes().to_vec();
    v.push(0xff);
    v
}

/// Parse count and FrameKey from an `elem_count_key` byte key (after verifying prefix).
pub(crate) fn parse_elem_count_key(k: &[u8], symbol: &str) -> Option<(u32, FrameKey)> {
    let pref = elem_count_symbol_prefix(symbol);
    if !k.starts_with(&pref) || k.len() < pref.len() + 4 + 12 {
        return None;
    }
    let mut cb = [0u8; 4];
    cb.copy_from_slice(&k[pref.len()..pref.len() + 4]);
    let count = u32::from_be_bytes(cb);
    let fk = FrameKey::from_bytes(&k[pref.len() + 4..pref.len() + 4 + 12])?;
    Some((count, fk))
}

/// Canonical formula string: sorted `Sym:count` joined by `|` (empty symbols skipped).
/// Example: Cu₂H₂ → `Cu:2|H:2`. Deterministic for exact composition index.
pub fn composition_formula(counts: &[(String, u32)]) -> String {
    let mut parts: Vec<(String, u32)> = counts
        .iter()
        .filter(|(s, c)| !s.is_empty() && *c > 0)
        .cloned()
        .collect();
    parts.sort_by(|a, b| a.0.cmp(&b.0));
    parts
        .into_iter()
        .map(|(s, c)| format!("{s}:{c}"))
        .collect::<Vec<_>>()
        .join("|")
}

/// Species multiset from atom symbols (non-empty only).
pub fn species_counts_from_symbols<'a>(symbols: impl Iterator<Item = &'a str>) -> Vec<(String, u32)> {
    use std::collections::BTreeMap;
    let mut m = BTreeMap::new();
    for s in symbols {
        if s.is_empty() {
            continue;
        }
        *m.entry(s.to_string()).or_insert(0u32) += 1;
    }
    m.into_iter().collect()
}

/// formula || 0xff || FrameKey
pub(crate) fn formula_key(formula: &str, fk: FrameKey) -> Vec<u8> {
    let mut v = formula.as_bytes().to_vec();
    v.push(0xff);
    v.extend_from_slice(&fk.to_bytes());
    v
}

pub(crate) fn formula_prefix(formula: &str) -> Vec<u8> {
    let mut v = formula.as_bytes().to_vec();
    v.push(0xff);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formula_canonical_order() {
        let f1 = composition_formula(&[("H".into(), 2), ("Cu".into(), 2)]);
        let f2 = composition_formula(&[("Cu".into(), 2), ("H".into(), 2)]);
        assert_eq!(f1, "Cu:2|H:2");
        assert_eq!(f1, f2);
    }

    #[test]
    fn ordered_bits_monotone() {
        let a = ordered_f64_bits(-1.0).unwrap();
        let b = ordered_f64_bits(0.0).unwrap();
        let c = ordered_f64_bits(1.0).unwrap();
        assert!(a < b && b < c);
    }

    #[test]
    fn elem_count_roundtrip_parse() {
        let fk = FrameKey {
            traj_id: 3,
            frame_idx: 7,
        };
        let k = elem_count_key("Cu", 2, fk);
        let (c, fk2) = parse_elem_count_key(&k, "Cu").unwrap();
        assert_eq!(c, 2);
        assert_eq!(fk2, fk);
    }
}

/// Meta scalar channel ids for `idx_meta` (u8 prefix).
pub(crate) const META_TIME: u8 = 1;
pub(crate) const META_TIMESTEP: u8 = 2;
pub(crate) const META_FRAME_INDEX: u8 = 3;
pub(crate) const META_NEB_BEAD: u8 = 4;
pub(crate) const META_NEB_BAND: u8 = 5;
pub(crate) const META_CHARGE: u8 = 6;
pub(crate) const META_MAGMOM: u8 = 7;

/// Ordered scalar bin: channel_id || ord(f64) BE || FrameKey (21 bytes)
pub(crate) fn meta_scalar_key(channel: u8, value: f64, fk: FrameKey) -> Option<[u8; 21]> {
    let ordered = ordered_f64_bits(value)?;
    let mut out = [0u8; 21];
    out[0] = channel;
    out[1..9].copy_from_slice(&ordered.to_be_bytes());
    out[9..].copy_from_slice(&fk.to_bytes());
    Some(out)
}

pub(crate) fn meta_channel_prefix(channel: u8) -> [u8; 1] {
    [channel]
}

/// PBC mask key: bit0=x, bit1=y, bit2=z (true=1); only written when metadata has pbc.
pub(crate) fn pbc_key(mask: u8, fk: FrameKey) -> [u8; 13] {
    let mut out = [0u8; 13];
    out[0] = mask & 0x07;
    out[1..].copy_from_slice(&fk.to_bytes());
    out
}

pub(crate) fn pbc_mask_from_bools(p: [bool; 3]) -> u8 {
    (p[0] as u8) | ((p[1] as u8) << 1) | ((p[2] as u8) << 2)
}

/// Mass / volume use same layout as energy (20 bytes, no channel).
pub(crate) fn mass_bin_key(mass: f64, fk: FrameKey) -> Option<[u8; 20]> {
    energy_bin_key(mass, fk)
}

pub(crate) fn volume_bin_key(vol: f64, fk: FrameKey) -> Option<[u8; 20]> {
    energy_bin_key(vol, fk)
}
