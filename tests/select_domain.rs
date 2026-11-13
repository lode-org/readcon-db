//! The full atom-count domain is not a predicate.
//!
//! Python `select` passes `0..=u32::MAX` whenever the caller sets no
//! atom-count bound. Storing that window makes every query walk `idx_natoms`.

use readcon_db::{ConCorpus, Select};

#[test]
fn full_natoms_window_does_not_install_a_scan() {
    let sel = Select::new().natoms_range(0, u32::MAX);
    assert_eq!(sel.natoms_min, None);
    assert_eq!(sel.natoms_max, None);
}

#[test]
fn narrower_natoms_window_stays_a_predicate() {
    let sel = Select::new().natoms_range(0, u32::MAX).natoms_range(4, 4);
    assert_eq!(sel.natoms_min, Some(4));
    assert_eq!(sel.natoms_max, Some(4));
    let cleared = sel.natoms_range(0, u32::MAX);
    assert_eq!(cleared.natoms_min, None);
    assert_eq!(cleared.natoms_max, None);
}

#[test]
fn full_window_returns_the_same_frames_as_no_predicate() {
    let path =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("resources/test/tiny_cuh2.con");
    let text = std::fs::read_to_string(path).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let db = ConCorpus::open(dir.path()).unwrap();
    db.append_trajectory_str(1, &text, "tiny").unwrap();
    let all = db.select(&Select::new()).unwrap();
    let wide = db.select(&Select::new().natoms_range(0, u32::MAX)).unwrap();
    assert_eq!(wide, all);
    assert_eq!(all.len(), 1);
    assert!(db
        .select(&Select::new().natoms_range(1, 1))
        .unwrap()
        .is_empty());
    assert_eq!(db.select(&Select::new().natoms_range(4, 4)).unwrap(), all);
}
