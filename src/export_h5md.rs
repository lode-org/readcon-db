//! Cooked H5MD arrays collected from CON/RCSO. The HDF5 file is written
//! by the Python binding (h5py). This module owns layout so tests can
//! check `[T][N][3]` without h5py.

use crate::corpus::ConCorpus;
use crate::error::Result;
use crate::select::Select;

/// Time-dependent cooked trajectory for one `traj_id`.
#[derive(Clone, Debug)]
pub struct H5mdArrays {
    pub n_frames: usize,
    pub natoms: usize,
    /// `[T][N][3]` row-major f64.
    pub positions: Vec<f64>,
    /// `[T][3][3]` H5MD box/edges (diagonal from CON `boxl`).
    pub edges: Vec<f64>,
    /// Integer Z, length `N`.
    pub species_z: Vec<i32>,
    /// `[T][N][3]` if any frame has forces; frames without forces are zeros.
    pub forces: Option<Vec<f64>>,
    /// H5MD `box` boundary strings, from CON `pbc` (periodic when absent).
    pub boundary: [String; 3],
}

fn boundary_from_pbc(pbc: Option<[bool; 3]>) -> [String; 3] {
    let p = pbc.unwrap_or([true, true, true]);
    std::array::from_fn(|i| {
        if p[i] {
            "periodic".into()
        } else {
            "none".into()
        }
    })
}

fn boxl_to_edges33(boxl: &[f64; 3]) -> [f64; 9] {
    [
        boxl[0], 0.0, 0.0, 0.0, boxl[1], 0.0, 0.0, 0.0, boxl[2],
    ]
}

impl ConCorpus {
    /// Collect one trajectory as H5MD-shaped arrays (fixed `N`).
    pub fn collect_h5md(&self, traj_id: u64) -> Result<H5mdArrays> {
        let keys = self.select(&Select::new().trajectory(traj_id))?;
        if keys.is_empty() {
            return Err(crate::error::Error::Message("no frames for traj".into()));
        }
        let first = self.get_frame(keys[0])?;
        let natoms = first.atom_data.len();
        let species_z: Vec<i32> = first
            .atom_data
            .iter()
            .map(|a| readcon_core::helpers::symbol_to_atomic_number(a.symbol.as_ref()) as i32)
            .collect();
        let n_frames = keys.len();
        let boundary = boundary_from_pbc(first.header.pbc());
        let mut positions = Vec::with_capacity(n_frames * natoms * 3);
        let mut edges = Vec::with_capacity(n_frames * 9);
        let mut force_rows: Vec<Option<Vec<[f64; 3]>>> = Vec::with_capacity(n_frames);
        for k in &keys {
            let fr = self.get_frame(*k)?;
            edges.extend_from_slice(&boxl_to_edges33(&fr.header.boxl));
            let packed = self.pack_frame(*k)?;
            let cooked = crate::cooked_soa::CookedSoa::decode(&packed)?;
            if cooked.natoms as usize != natoms {
                return Err(crate::error::Error::Message(
                    "H5MD export needs fixed natoms in the trajectory".into(),
                ));
            }
            for p in &cooked.positions {
                positions.extend_from_slice(p);
            }
            force_rows.push(cooked.forces);
        }
        let forces = if force_rows.iter().any(|f| f.is_some()) {
            let mut fbuf = vec![0.0f64; n_frames * natoms * 3];
            for (ti, fo) in force_rows.iter().enumerate() {
                if let Some(rows) = fo {
                    let off = ti * natoms * 3;
                    for (i, row) in rows.iter().enumerate() {
                        fbuf[off + i * 3] = row[0];
                        fbuf[off + i * 3 + 1] = row[1];
                        fbuf[off + i * 3 + 2] = row[2];
                    }
                }
            }
            Some(fbuf)
        } else {
            None
        };
        Ok(H5mdArrays {
            n_frames,
            natoms,
            positions,
            edges,
            species_z,
            forces,
            boundary,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test")
            .join(name)
    }

    #[test]
    fn collect_h5md_tn3_from_con() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        let n = db
            .append_trajectory_path(1, fixture("tiny_multi_cuh2.con"))
            .unwrap();
        assert!(n >= 2);
        let a = db.collect_h5md(1).unwrap();
        assert_eq!(a.n_frames, n as usize);
        assert!(a.natoms >= 1);
        assert_eq!(a.positions.len(), a.n_frames * a.natoms * 3);
        assert_eq!(a.edges.len(), a.n_frames * 9);
        assert_eq!(a.species_z.len(), a.natoms);
        assert!(a.species_z.iter().all(|&z| z > 0));
        assert_eq!(a.boundary.len(), 3);
    }

    #[test]
    fn collect_h5md_pads_mixed_forces() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        db.append_trajectory_path(1, fixture("tiny_cuh2.con"))
            .unwrap();
        db.extend_trajectory_path(1, fixture("tiny_cuh2_forces.con"))
            .unwrap();
        let a = db.collect_h5md(1).unwrap();
        let f = a.forces.expect("second frame carries forces");
        assert_eq!(f.len(), a.n_frames * a.natoms * 3);
        assert!(a.n_frames >= 2);
        assert!(f[..a.natoms * 3].iter().all(|&x| x == 0.0));
        assert!(f[a.natoms * 3..].iter().any(|&x| x != 0.0));
    }
}
