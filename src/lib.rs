//! readcon-db — embedded CON corpus store (design skeleton).
//!
//! See `README.md` and `docs/design.md` for the Heed/LMDB rationale.
//! Implementation is intentionally minimal until the on-disk key layout is frozen.

#![allow(dead_code)]

/// Stable trajectory identifier (user-assigned or content-derived).
pub type TrajId = u64;

/// Zero-based frame index within a trajectory.
pub type FrameIdx = u32;

/// Primary key for a stored frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FrameKey {
    pub traj_id: TrajId,
    pub frame_idx: FrameIdx,
}

impl FrameKey {
    /// Big-endian key bytes for LMDB lexicographic order.
    pub fn to_bytes(self) -> [u8; 12] {
        let mut out = [0u8; 12];
        out[..8].copy_from_slice(&self.traj_id.to_be_bytes());
        out[8..].copy_from_slice(&self.frame_idx.to_be_bytes());
        out
    }

    pub fn from_bytes(b: [u8; 12]) -> Self {
        let mut t = [0u8; 8];
        let mut f = [0u8; 4];
        t.copy_from_slice(&b[..8]);
        f.copy_from_slice(&b[8..]);
        Self {
            traj_id: u64::from_be_bytes(t),
            frame_idx: u32::from_be_bytes(f),
        }
    }
}

/// Non-SQL selection builder (filters composed in process, executed via indexes).
#[derive(Clone, Debug, Default)]
pub struct Select {
    pub traj_id: Option<TrajId>,
    pub natoms_min: Option<u32>,
    pub natoms_max: Option<u32>,
    pub symbols_all: Vec<String>,
    pub limit: Option<usize>,
}

impl Select {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn trajectory(mut self, id: TrajId) -> Self {
        self.traj_id = Some(id);
        self
    }
    pub fn natoms_range(mut self, min: u32, max: u32) -> Self {
        self.natoms_min = Some(min);
        self.natoms_max = Some(max);
        self
    }
    pub fn require_symbol(mut self, s: impl Into<String>) -> Self {
        self.symbols_all.push(s.into());
        self
    }
    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }
}

/// Placeholder database handle — wire Heed `Env` in a follow-up commit.
pub struct ConCorpus {
    _path: std::path::PathBuf,
}

impl ConCorpus {
    pub fn open(_path: impl Into<std::path::PathBuf>) -> Result<Self, String> {
        Err("readcon-db: Heed environment not wired yet; see README.md".into())
    }

    pub fn select(&self, _sel: &Select) -> Result<Vec<FrameKey>, String> {
        Err("not implemented".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_key_roundtrip_order() {
        let a = FrameKey {
            traj_id: 1,
            frame_idx: 2,
        };
        let b = FrameKey {
            traj_id: 1,
            frame_idx: 10,
        };
        assert!(a.to_bytes() < b.to_bytes());
        assert_eq!(FrameKey::from_bytes(a.to_bytes()), a);
    }
}
