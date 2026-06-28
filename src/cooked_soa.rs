//! Optional **cooked SoA** payload: derived binary numerics beside authoritative CON text.
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

const HEADER_LEN: usize = 4 + 4 + 4 + 4 + 1 + 3 + 4; // 24

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
        let n = frame.atom_data.len();
        if n > u32::MAX as usize {
            return Err(Error::Message("too many atoms for cooked SoA".into()));
        }
        let natoms = n as u32;
        let mut flags = 0u32;
        let has_f = frame.atom_data.iter().any(|a| a.force.is_some());
        let has_v = frame.atom_data.iter().any(|a| a.velocity.is_some());
        if has_f {
            flags |= FLAG_FORCES;
        }
        if has_v {
            flags |= FLAG_VELOCITIES;
        }

        let mut out = Vec::with_capacity(HEADER_LEN + n * 3 * 8 * (1 + has_f as usize + has_v as usize));
        out.extend_from_slice(COOKED_MAGIC);
        out.extend_from_slice(&COOKED_VERSION.to_le_bytes());
        out.extend_from_slice(&natoms.to_le_bytes());
        out.extend_from_slice(&flags.to_le_bytes());
        out.push(DTYPE_F64);
        out.extend_from_slice(&[0u8; 3]);
        out.extend_from_slice(&0u32.to_le_bytes());

        for a in &frame.atom_data {
            for c in [a.x, a.y, a.z] {
                out.extend_from_slice(&c.to_le_bytes());
            }
        }
        if has_f {
            for a in &frame.atom_data {
                let f = a.force.unwrap_or([0.0; 3]);
                for c in f {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
        }
        if has_v {
            for a in &frame.atom_data {
                let v = a.velocity.unwrap_or([0.0; 3]);
                for c in v {
                    out.extend_from_slice(&c.to_le_bytes());
                }
            }
        }
        Ok(out)
    }

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
        let block = n.checked_mul(3).ok_or_else(|| Error::Message("overflow".into()))?;
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
            .join("../readcon-core/resources/test")
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
        assert!(CookedSoa::decode(b"XXXX").is_err());
        assert!(CookedSoa::try_decode(b"XXXX").is_none());
    }
}
