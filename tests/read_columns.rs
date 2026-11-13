//! Grouped frames still round-trip through the column scan.

use readcon_db::{ConCorpus, FrameKey};

#[test]
fn grouped_columns_match_get_frame() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources/test/tiny_cuh2_forces.con");
    let text = std::fs::read_to_string(path).unwrap();
    let many = text.repeat(80);
    let dir = tempfile::tempdir().unwrap();
    let db = ConCorpus::open(dir.path()).unwrap();
    let n = db.append_trajectory_str(7, &many, "grouped").unwrap();
    assert!(n >= 80, "{n}");
    let cols = db.read_columns(7).unwrap();
    assert_eq!(cols.n_atoms.len(), n as usize);
    assert!(cols.forces_uniform);
    for idx in [0u32, 1, 63, 64, 79] {
        let key = FrameKey {
            traj_id: 7,
            frame_idx: idx,
        };
        let frame = db.get_frame(key).unwrap();
        let one = db.read_frame_columns(key).unwrap();
        assert_eq!(one.n_atoms, vec![frame.atom_data.len() as u32]);
        for (atom_i, atom) in frame.atom_data.iter().enumerate() {
            assert_eq!(
                &one.positions[atom_i * 3..atom_i * 3 + 3],
                &[atom.x, atom.y, atom.z]
            );
            assert_eq!(
                &one.forces[atom_i * 3..atom_i * 3 + 3],
                &atom.force().unwrap()
            );
        }
        let base = cols.n_atoms[..idx as usize]
            .iter()
            .map(|count| *count as usize)
            .sum::<usize>()
            * 3;
        let width = cols.n_atoms[idx as usize] as usize * 3;
        assert_eq!(
            &cols.positions[base..base + width],
            one.positions.as_slice()
        );
    }
}
