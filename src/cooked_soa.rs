//! Optional **cooked SoA** payload: derived binary numerics beside authoritative CON text.
//!
//! Layout and encode/decode live in `readcon_core::rcso` (RCSO/RCSB v1).
//! This module is the corpus-facing wrapper. CON text remains authority.

use readcon_core::rcso::{self, Rcso};
use readcon_core::types::ConFrame;

use crate::error::{Error, Result};

pub const COOKED_MAGIC: &[u8; 4] = rcso::RCSO_MAGIC;
pub const COOKED_VERSION: u32 = rcso::RCSO_VERSION;
pub const DTYPE_F64: u8 = rcso::RCSO_DTYPE_F64;
pub const FLAG_FORCES: u32 = rcso::RCSO_FLAG_FORCES;
pub const FLAG_VELOCITIES: u32 = rcso::RCSO_FLAG_VELOCITIES;

const HEADER_LEN: usize = 24;

/// Decoded cooked numerics (always f64 for v1).
#[derive(Clone, Debug, PartialEq)]
pub struct CookedSoa {
    pub natoms: u32,
    pub positions: Vec<[f64; 3]>,
    pub forces: Option<Vec<[f64; 3]>>,
    pub velocities: Option<Vec<[f64; 3]>>,
}

impl CookedSoa {
    pub fn encode_frame(frame: &ConFrame) -> Result<Vec<u8>> {
        Rcso::encode_frame(frame).map_err(|e| Error::Message(e.to_string()))
    }

    pub fn decode(bytes: &[u8]) -> Result<Self> {
        let s = Rcso::decode(bytes).map_err(|e| Error::Message(e.to_string()))?;
        Ok(Self {
            natoms: s.natoms,
            positions: s.positions,
            forces: s.forces,
            velocities: s.velocities,
        })
    }

    /// Prefer cooked bytes; return None if missing/invalid (caller parses CON).
    pub fn try_decode(bytes: &[u8]) -> Option<Self> {
        Self::decode(bytes).ok()
    }
}

/// Length-prefixed RCSO blobs for one collective on the caller comm.
pub const BATCH_MAGIC: &[u8; 4] = rcso::RCSB_MAGIC;
pub const BATCH_VERSION: u32 = rcso::RCSB_VERSION;

pub fn encode_batch(blobs: &[Vec<u8>]) -> Result<Vec<u8>> {
    rcso::encode_batch(blobs).map_err(|e| Error::Message(e.to_string()))
}

pub fn decode_batch(bytes: &[u8]) -> Result<Vec<Vec<u8>>> {
    rcso::decode_batch(bytes).map_err(|e| Error::Message(e.to_string()))
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
    fn encode_decode_positions_fixture() {
        let text = fixture("tiny_cuh2.con");
        let fr = ConFrameIterator::new(&text).next().unwrap().unwrap();
        let bytes = CookedSoa::encode_frame(&fr).unwrap();
        assert!(bytes.len() > HEADER_LEN);
        let cooked = CookedSoa::decode(&bytes).unwrap();
        assert_eq!(cooked.natoms as usize, fr.atom_data.len());
        for (i, a) in fr.atom_data.iter().enumerate() {
            assert_eq!(cooked.positions[i], [a.x, a.y, a.z]);
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
            assert_eq!(cooked.positions[i], [a.x, a.y, a.z]);
            if let Some(f) = a.force {
                assert_eq!(forces[i], f);
            }
        }
    }

    #[test]
    fn bad_magic_rejected() {
        assert_eq!(COOKED_MAGIC, b"RCSO");
        assert_eq!(COOKED_VERSION, 1);
        assert_eq!(DTYPE_F64, 0);
        assert_eq!(FLAG_FORCES, 1);
        assert_eq!(FLAG_VELOCITIES, 2);
        assert_eq!(BATCH_MAGIC, b"RCSB");
        assert_eq!(BATCH_VERSION, 1);
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
}
