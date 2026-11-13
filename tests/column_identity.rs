//! Per-frame column identity from readcon-core 0.22.0.
//!
//! `CoordColumns` in 0.21.0 has positions, masses, and cells. It has no
//! column 5 and no element runs, so this test does not build on that pin.
//! 0.22.0 returns those for a later frame whose element order is not the
//! first frame's. The corpus stores that order: the oxygen keeps its mass,
//! and a velocity that is not on the first atom stays indexed.

use std::fs;
use std::path::Path;

use readcon_core::iterators::{read_coord_columns, ConFrameIterator};
use readcon_core::types::ConFrameBuilder;
use readcon_core::writer::write_frames_to_string;
use readcon_db::{ConCorpus, FrameKey, Select};

fn pair() -> (readcon_core::types::ConFrame, readcon_core::types::ConFrame) {
    let c1 = [3.0, 4.0, 4.0];
    let c2 = [4.336, 4.0, 4.0];
    let oxygen = [5.016, 5.178, 4.0];
    let mut carbon_first = ConFrameBuilder::new([18.0, 18.0, 18.0], [90.0, 90.0, 90.0]);
    carbon_first.prebox_header("claisen");
    carbon_first.add_atom("C", c1[0], c1[1], c1[2], [false; 3], 0, 12.011);
    carbon_first.add_atom("C", c2[0], c2[1], c2[2], [false; 3], 1, 12.011);
    carbon_first.add_atom("O", oxygen[0], oxygen[1], oxygen[2], [false; 3], 2, 15.999);

    let mut oxygen_first = ConFrameBuilder::new([18.0, 18.0, 18.0], [90.0, 90.0, 90.0]);
    oxygen_first.prebox_header("claisen");
    oxygen_first.add_atom("O", oxygen[0], oxygen[1], oxygen[2], [false; 3], 2, 15.999);
    oxygen_first.add_atom("C", c1[0], c1[1], c1[2], [false; 3], 0, 12.011);
    oxygen_first.with_velocity([0.1, -0.2, 0.3]);
    oxygen_first.add_atom("C", c2[0], c2[1], c2[2], [false; 3], 1, 12.011);
    oxygen_first
        .set_units(serde_json::json!({
            "length": "angstrom",
            "energy": "eV",
            "mass": "amu",
            "time": "fs"
        }))
        .expect("units");

    (
        carbon_first.build().expect("carbon-first"),
        oxygen_first.build().expect("oxygen-first"),
    )
}

#[test]
fn later_frame_keeps_its_element_order_and_a_later_velocity() {
    let (carbon_first, oxygen_first) = pair();
    let text = write_frames_to_string(&[carbon_first, oxygen_first]).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("pair.con");
    fs::write(&path, &text).unwrap();

    let columns = read_coord_columns(Path::new(&path), Some(1)).unwrap();
    assert_eq!(columns.block.n_frames, 2);
    assert_eq!(columns.block.n_atoms, 3);
    assert_eq!(columns.atom_ids, vec![0, 1, 2, 2, 0, 1]);
    assert_eq!(
        columns.species,
        vec![
            vec![("C".to_string(), 2), ("O".to_string(), 1)],
            vec![("O".to_string(), 1), ("C".to_string(), 2)],
        ]
    );
    // File order, not the first frame's element order. The oxygen is atom 0
    // of the second frame and keeps the oxygen mass.
    assert!((columns.masses[3] - 15.999).abs() < 1e-9);
    assert!((columns.masses[3] - 12.011).abs() > 1.0);

    let parsed: Vec<_> = ConFrameIterator::new(&text)
        .map(|item| item.unwrap())
        .collect();
    assert_eq!(parsed[1].atom_data[0].symbol.as_ref(), "O");
    assert_eq!(parsed[1].atom_data[0].atom_id, 2);
    assert!((parsed[1].masses.get_f64(0) - 15.999).abs() < 1e-9);
    assert_eq!(parsed[1].atom_data[1].symbol.as_ref(), "C");
    assert_eq!(parsed[1].atom_data[1].velocity(), Some([0.1, -0.2, 0.3]));

    let db = ConCorpus::open(dir.path().join("corpus")).unwrap();
    assert_eq!(db.append_trajectory_str(1, &text, "pair").unwrap(), 2);
    let stored = db
        .get_frame(FrameKey {
            traj_id: 1,
            frame_idx: 1,
        })
        .unwrap();
    assert_eq!(stored.atom_data[0].symbol.as_ref(), "O");
    assert_eq!(stored.atom_data[0].atom_id, 2);
    assert!((stored.masses.get_f64(0) - 15.999).abs() < 1e-9);
    let velocities = db
        .get_velocities(FrameKey {
            traj_id: 1,
            frame_idx: 1,
        })
        .unwrap()
        .expect("later-atom velocity");
    assert_eq!(velocities[0], [0.0, 0.0, 0.0]);
    assert_eq!(velocities[1], [0.1, -0.2, 0.3]);
    let hits = db
        .select(&Select::new().trajectory(1).require_velocities())
        .unwrap();
    assert!(hits.iter().any(|key| key.frame_idx == 1));
}
