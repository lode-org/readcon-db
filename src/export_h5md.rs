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
    /// `[T]` CON `header.time()` when present, else frame index.
    pub times: Vec<f64>,
    /// H5MD time unit (`fs` when CON units.time is fs; `ps` for dummy index).
    pub time_unit: String,
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

fn normalize_time_unit(raw: &str) -> String {
    let s = raw.trim();
    if s.eq_ignore_ascii_case("fs")
        || s.eq_ignore_ascii_case("femtosecond")
        || s.eq_ignore_ascii_case("femtoseconds")
    {
        "fs".into()
    } else if s.eq_ignore_ascii_case("ns") || s.eq_ignore_ascii_case("nanosecond") {
        "ns".into()
    } else {
        "ps".into()
    }
}

fn time_unit_of(header: &readcon_core::types::ConFrameHeader) -> String {
    header
        .units()
        .and_then(|u| u.get("time"))
        .and_then(|v| v.as_str())
        .map(normalize_time_unit)
        .unwrap_or_else(|| "ps".into())
}

fn edges33_from_header(h: &readcon_core::types::ConFrameHeader) -> [f64; 9] {
    if let Some(lat) = h.lattice_vectors() {
        return [
            lat[0][0], lat[0][1], lat[0][2], lat[1][0], lat[1][1], lat[1][2], lat[2][0],
            lat[2][1], lat[2][2],
        ];
    }
    boxl_angles_to_edges33(&h.boxl, &h.angles)
}

fn boxl_angles_to_edges33(boxl: &[f64; 3], angles: &[f64; 3]) -> [f64; 9] {
    let ortho = angles.iter().all(|&a| a == 0.0 || (a - 90.0).abs() < 1e-9);
    if ortho {
        return boxl_to_edges33(boxl);
    }
    let deg = |a: f64| a * std::f64::consts::PI / 180.0;
    let (lx, ly, lz) = (boxl[0], boxl[1], boxl[2]);
    let (alpha, beta, gamma) = (deg(angles[0]), deg(angles[1]), deg(angles[2]));
    let ax = lx;
    let bx = ly * gamma.cos();
    let by = ly * gamma.sin();
    let cx = lz * beta.cos();
    let cy = lz * (alpha.cos() - beta.cos() * gamma.cos()) / gamma.sin();
    let cz = (lz * lz - cx * cx - cy * cy).max(0.0).sqrt();
    [ax, 0.0, 0.0, bx, by, 0.0, cx, cy, cz]
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
        let time_unit = time_unit_of(&first.header);
        let mut positions = Vec::with_capacity(n_frames * natoms * 3);
        let mut edges = Vec::with_capacity(n_frames * 9);
        let mut times = Vec::with_capacity(n_frames);
        let mut force_rows: Vec<Option<Vec<[f64; 3]>>> = Vec::with_capacity(n_frames);
        for k in &keys {
            let fr = self.get_frame(*k)?;
            times.push(fr.header.time().unwrap_or(f64::from(k.frame_idx)));
            edges.extend_from_slice(&edges33_from_header(&fr.header));
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
            times,
            time_unit,
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
        assert_eq!(a.times.len(), a.n_frames);
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

    #[test]
    fn collect_h5md_con_fallback_matches_rcso() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        db.append_trajectory_path(1, fixture("tiny_multi_cuh2.con"))
            .unwrap();
        let key = crate::keys::FrameKey {
            traj_id: 1,
            frame_idx: 0,
        };
        assert!(!db.has_valid_cooked_soa(key).unwrap());
        let from_con = db.collect_h5md(1).unwrap();
        db.recook_all().unwrap();
        assert!(db.has_valid_cooked_soa(key).unwrap());
        let from_rcso = db.collect_h5md(1).unwrap();
        assert_eq!(from_con.n_frames, from_rcso.n_frames);
        assert_eq!(from_con.positions, from_rcso.positions);
        assert_eq!(from_con.edges, from_rcso.edges);
        assert_eq!(from_con.species_z, from_rcso.species_z);
    }

    #[test]
    fn collect_h5md_uses_con_time_and_fs_unit() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        let text = std::fs::read_to_string(fixture("tiny_cuh2.con")).unwrap();
        let mut frames = Vec::new();
        for item in readcon_core::iterators::ConFrameIterator::new(&text) {
            frames.push(item.unwrap());
        }
        frames[0].header.set_time(12.5);
        frames[0].header.metadata.insert(
            "units".into(),
            serde_json::json!({"length":"angstrom","energy":"eV","mass":"amu","time":"fs"}),
        );
        db.append_trajectory_frames(1, &frames, "t").unwrap();
        let a = db.collect_h5md(1).unwrap();
        assert_eq!(a.times[0], 12.5);
        assert_eq!(a.time_unit, "fs");
    }

    #[test]
    fn collect_h5md_triclinic_edges_from_angles() {
        let dir = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(dir.path()).unwrap();
        let text = std::fs::read_to_string(fixture("tiny_cuh2.con")).unwrap();
        let mut frames = Vec::new();
        for item in readcon_core::iterators::ConFrameIterator::new(&text) {
            frames.push(item.unwrap());
        }
        frames[0].header.angles = [60.0, 90.0, 70.0];
        db.append_trajectory_frames(1, &frames, "t").unwrap();
        let a = db.collect_h5md(1).unwrap();
        assert_eq!(a.edges.len(), 9);
        assert!(a.edges[3].abs() > 1e-9, "b_x from gamma != 90");
        let ortho = boxl_to_edges33(&frames[0].header.boxl);
        assert_ne!(a.edges, ortho.to_vec());
    }
}
