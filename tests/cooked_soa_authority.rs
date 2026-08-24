//! CON text is the sole authority. RCSO is derived and non-authoritative.
//!
//! Numeric extract coverage lives with the unit tests
//! `numeric_extract_prefers_con_when_rcso_disagrees` and
//! `collect_h5md_prefers_con_when_rcso_disagrees` (poisoned RCSO must lose).
//! This file locks the published-page wording and the parse-vs-cook match.

use std::fs;
use std::path::PathBuf;

use readcon_db::{ConCorpus, FrameKey};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn read(rel: &str) -> String {
    let p = root().join(rel);
    fs::read_to_string(&p).unwrap_or_else(|e| panic!("missing {}: {e}", p.display()))
}

fn fixture(name: &str) -> PathBuf {
    root().join("resources/test").join(name)
}

fn assert_contains(hay: &str, needle: &str, file: &str) {
    assert!(
        hay.contains(needle),
        "{file} must state the CON authority contract ({needle:?})"
    );
}

/// Published Sphinx pages and rustdoc must keep CON-text authority unmissable.
#[test]
fn published_pages_state_con_text_is_sole_authority() {
    let page = read("docs/source/cooked_soa.md");
    assert_contains(&page, "non-authoritative", "docs/source/cooked_soa.md");
    assert_contains(&page, "sole authority", "docs/source/cooked_soa.md");
    assert_contains(&page, "disagrees with CON", "docs/source/cooked_soa.md");

    let toc = read("docs/source/index.md");
    assert_contains(&toc, "cooked_soa", "docs/source/index.md");

    let arch = read("docs/source/architecture.md");
    assert_contains(&arch, "non-authoritative", "docs/source/architecture.md");
    assert_contains(&arch, "sole authority", "docs/source/architecture.md");

    let overview = read("docs/source/overview.md");
    assert_contains(&overview, "non-authoritative", "docs/source/overview.md");
    assert_contains(&overview, "sole authority", "docs/source/overview.md");

    let org = read("docs/orgmode/cooked-soa.org");
    assert_contains(&org, "non-authoritative", "docs/orgmode/cooked-soa.org");
    assert_contains(&org, "sole authority", "docs/orgmode/cooked-soa.org");

    let rustdoc = read("src/cooked_soa.rs");
    assert_contains(&rustdoc, "non-authoritative", "src/cooked_soa.rs");
    assert_contains(&rustdoc, "sole authority", "src/cooked_soa.rs");
}

/// Parse path and cooked path must agree. Cook / delete must not change CON.
#[test]
fn numeric_extract_parse_and_cooked_match() {
    let dir = tempfile::tempdir().unwrap();
    let db = ConCorpus::open(dir.path()).unwrap();
    let key = FrameKey {
        traj_id: 1,
        frame_idx: 0,
    };
    db.append_trajectory_path(1, fixture("tiny_cuh2_vel_forces.con"))
        .unwrap();
    assert!(!db.has_valid_cooked_soa(key).unwrap());

    let pos_parse = db.get_positions(key).unwrap();
    let f_parse = db.get_forces(key).unwrap().expect("forces");
    let v_parse = db.get_velocities(key).unwrap().expect("velocities");
    assert_eq!(pos_parse.len(), 4);
    assert!((pos_parse[0][0] - 0.6394).abs() < 1e-9);
    assert!((v_parse[0][0] - 0.001234).abs() < 1e-12);
    assert!((f_parse[0][0] - 0.123456).abs() < 1e-12);

    let text = db.get_frame_text(key).unwrap();
    let h = db.frame_hash(key).unwrap();

    db.cook_frame(key).unwrap();
    assert!(db.has_valid_cooked_soa(key).unwrap());
    assert_eq!(db.get_positions(key).unwrap(), pos_parse);
    assert_eq!(db.get_forces(key).unwrap().unwrap(), f_parse);
    assert_eq!(db.get_velocities(key).unwrap().unwrap(), v_parse);
    assert_eq!(db.get_frame_text(key).unwrap(), text);
    assert_eq!(db.frame_hash(key).unwrap().to_bytes(), h.to_bytes());
    assert_eq!(db.find_by_hash(h).unwrap(), Some(key));

    db.delete_cooked_soa(key).unwrap();
    assert!(!db.has_valid_cooked_soa(key).unwrap());
    assert_eq!(db.get_positions(key).unwrap(), pos_parse);
    assert_eq!(db.get_forces(key).unwrap().unwrap(), f_parse);
    assert_eq!(db.get_velocities(key).unwrap().unwrap(), v_parse);
    assert_eq!(db.get_frame_text(key).unwrap(), text);
    assert_eq!(db.frame_hash(key).unwrap().to_bytes(), h.to_bytes());
}
