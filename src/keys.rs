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
