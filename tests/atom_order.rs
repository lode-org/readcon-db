//! Atom-id row order and the two identities.
//!
//! CON groups atoms by element in first-encounter order. A Claisen product
//! written oxygen-first keeps the same `atom_id`s and changes the xxHash3
//! content hash. The opt-in canonical index hashes the frame after those
//! rows are put back into atom-id order.

use std::process::Command;

use readcon_core::types::ConFrameBuilder;
use readcon_core::writer::write_frames_to_string;
use readcon_db::{
    atom_order_permutation, canonical_identity, hash_frame_bytes, AtomOrderHit, ConCorpus,
    FrameKey, CANONICAL_IDENTITY_VERSION,
};

fn frame(order: &[(&str, u64, [f64; 3], f64)]) -> readcon_core::types::ConFrame {
    let mut builder = ConFrameBuilder::new([18.0, 18.0, 18.0], [90.0, 90.0, 90.0]);
    builder.prebox_header("claisen");
    for (symbol, id, pos, mass) in order {
        builder.add_atom(symbol, pos[0], pos[1], pos[2], [false; 3], *id, *mass);
    }
    builder.build().expect("frame")
}

fn claisen_pair() -> (readcon_core::types::ConFrame, readcon_core::types::ConFrame) {
    let c1 = [3.0, 4.0, 4.0];
    let c2 = [4.336, 4.0, 4.0];
    let o = [5.016, 5.178, 4.0];
    let carbon_first = frame(&[
        ("C", 0, c1, 12.011),
        ("C", 1, c2, 12.011),
        ("O", 2, o, 15.999),
    ]);
    let oxygen_first = frame(&[
        ("O", 2, o, 15.999),
        ("C", 0, c1, 12.011),
        ("C", 1, c2, 12.011),
    ]);
    (carbon_first, oxygen_first)
}

#[test]
fn oxygen_first_claisen_is_the_same_frame() {
    let (carbon_first, oxygen_first) = claisen_pair();
    let oxygen_text = write_frames_to_string(std::slice::from_ref(&oxygen_first)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let db = ConCorpus::open(dir.path()).unwrap();
    db.append_trajectory_frames(1, std::slice::from_ref(&carbon_first), "reactant")
        .unwrap();
    let stored = FrameKey {
        traj_id: 1,
        frame_idx: 0,
    };
    let content = db.frame_hash(stored).unwrap();
    assert_ne!(
        content,
        hash_frame_bytes(oxygen_text.as_bytes()),
        "a row permutation of the same atom ids must not share the content hash"
    );
    assert_eq!(db.find_by_hash(content).unwrap(), Some(stored));
    let err = db.find_by_atom_order_text(&oxygen_text).unwrap_err();
    assert!(
        err.to_string().contains("index-canonical"),
        "unindexed lookup should name the command, got {err}"
    );

    let before = db.frame_hash(stored).unwrap();
    assert_eq!(db.index_canonical_identity().unwrap(), 1);
    assert_eq!(db.frame_hash(stored).unwrap(), before);
    assert_eq!(db.find_by_hash(before).unwrap(), Some(stored));
    let ident = db.frame_canonical_identity(stored).unwrap();
    assert_eq!(ident.version, CANONICAL_IDENTITY_VERSION);
    assert_eq!(ident, canonical_identity(&oxygen_first).unwrap());
    assert_ne!(
        ident.hash,
        hash_frame_bytes(oxygen_text.as_bytes()),
        "canonical identity is the atom-id order, not the oxygen-first bytes"
    );

    let hits = db.find_by_atom_order_text(&oxygen_text).unwrap();
    assert_eq!(
        hits,
        vec![AtomOrderHit {
            query_index: 0,
            key: stored,
            permutation: vec![1, 2, 0],
        }]
    );
    assert_eq!(
        atom_order_permutation(&db.get_frame(stored).unwrap(), &oxygen_first).unwrap(),
        vec![1, 2, 0]
    );

    let moved = frame(&[
        ("O", 2, [5.016 + 1.0, 5.178, 4.0], 15.999),
        ("C", 0, [3.0, 4.0, 4.0], 12.011),
        ("C", 1, [4.336, 4.0, 4.0], 12.011),
    ]);
    assert!(db.find_by_atom_order_frame(&moved).unwrap().is_empty());

    drop(db);
    let db = ConCorpus::open_readonly(dir.path()).unwrap();
    let path = dir.path().join("oxygen-first.con");
    std::fs::write(&path, &oxygen_text).unwrap();
    assert_eq!(
        db.find_by_atom_order_path(&path).unwrap(),
        vec![AtomOrderHit {
            query_index: 0,
            key: stored,
            permutation: vec![1, 2, 0],
        }]
    );
}

#[test]
fn canonical_identity_rejects_duplicate_missing_and_mismatched_species() {
    let (carbon_first, _) = claisen_pair();
    let duplicate = frame(&[
        ("C", 7, [3.0, 4.0, 4.0], 12.011),
        ("O", 7, [5.016, 5.178, 4.0], 15.999),
    ]);
    let dup = canonical_identity(&duplicate).unwrap_err().to_string();
    assert!(dup.contains("duplicate atom_id 7"), "{dup}");

    let missing = frame(&[
        ("C", 0, [3.0, 4.0, 4.0], 12.011),
        ("C", 1, [4.336, 4.0, 4.0], 12.011),
    ]);
    let miss = atom_order_permutation(&carbon_first, &missing)
        .unwrap_err()
        .to_string();
    assert!(miss.contains("missing atom_id 2"), "{miss}");

    let nitrogen = frame(&[
        ("C", 0, [3.0, 4.0, 4.0], 12.011),
        ("C", 1, [4.336, 4.0, 4.0], 12.011),
        ("N", 2, [5.016, 5.178, 4.0], 15.999),
    ]);
    let species = atom_order_permutation(&carbon_first, &nitrogen)
        .unwrap_err()
        .to_string();
    assert!(species.contains("atom_id 2"), "{species}");
    assert!(species.contains("symbol"), "{species}");
    assert!(species.contains('N'), "{species}");
}

#[test]
fn canonical_index_goes_stale_when_frames_change() {
    let (carbon_first, oxygen_first) = claisen_pair();
    let oxygen_text = write_frames_to_string(std::slice::from_ref(&oxygen_first)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let db = ConCorpus::open(dir.path()).unwrap();
    db.append_trajectory_frames(1, std::slice::from_ref(&carbon_first), "reactant")
        .unwrap();
    db.index_canonical_identity().unwrap();
    db.reindex().unwrap();
    assert_eq!(db.find_by_atom_order_text(&oxygen_text).unwrap().len(), 1);

    db.append_trajectory_frames(2, std::slice::from_ref(&carbon_first), "again")
        .unwrap();
    let stale = db.find_by_atom_order_text(&oxygen_text).unwrap_err();
    let stale = stale.to_string();
    assert!(stale.contains("stale"), "{stale}");
    assert!(stale.contains("index-canonical"), "{stale}");

    db.index_canonical_identity().unwrap();
    assert_eq!(db.find_by_atom_order_text(&oxygen_text).unwrap().len(), 2);
    db.set_trajectory_units(1, serde_json::json!({"length": "nm", "energy": "eV"}))
        .unwrap();
    let rewritten = db.find_by_atom_order_text(&oxygen_text).unwrap_err();
    let rewritten = rewritten.to_string();
    assert!(rewritten.contains("stale"), "{rewritten}");
    assert!(rewritten.contains("index-canonical"), "{rewritten}");
}

#[test]
fn cli_index_canonical_and_find_by_atom_order() {
    let (carbon_first, oxygen_first) = claisen_pair();
    let oxygen_text = write_frames_to_string(std::slice::from_ref(&oxygen_first)).unwrap();
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("corpus");
    let query = dir.path().join("oxygen-first.con");
    std::fs::write(&query, oxygen_text).unwrap();
    {
        let db = ConCorpus::open(&corpus).unwrap();
        db.append_trajectory_frames(1, std::slice::from_ref(&carbon_first), "reactant")
            .unwrap();
    }
    let early = Command::new(env!("CARGO_BIN_EXE_readcon-db"))
        .args([
            "find-by-atom-order",
            corpus.to_str().unwrap(),
            query.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(!early.status.success());
    let err = String::from_utf8_lossy(&early.stderr);
    assert!(err.contains("index-canonical"), "{err}");

    let indexed = Command::new(env!("CARGO_BIN_EXE_readcon-db"))
        .args(["index-canonical", corpus.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        indexed.status.success(),
        "{}",
        String::from_utf8_lossy(&indexed.stderr)
    );
    assert!(String::from_utf8_lossy(&indexed.stdout).contains("indexed 1 frames"));

    let found = Command::new(env!("CARGO_BIN_EXE_readcon-db"))
        .args([
            "find-by-atom-order",
            corpus.to_str().unwrap(),
            query.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        found.status.success(),
        "{}",
        String::from_utf8_lossy(&found.stderr)
    );
    let out = String::from_utf8(found.stdout).unwrap();
    assert!(
        out.contains("query 0: traj=1 frame=0 permutation=1,2,0"),
        "{out}"
    );
}
