//! `hash_con_text` must use the bytes the corpus stores.
//!
//! `append_trajectory_str` keeps each frame's original span. A rewrite of
//! that span is a different xxHash3 key, so `hash-file` would miss the frame.

use readcon_db::{ConCorpus, FrameKey};

fn text(name: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources/test")
        .join(name);
    std::fs::read_to_string(path).unwrap()
}

fn stored_hash_matches(name: &str) {
    let text = text(name);
    let dir = tempfile::tempdir().unwrap();
    let db = ConCorpus::open(dir.path()).unwrap();
    let n = db.append_trajectory_str(1, &text, name).unwrap();
    assert!(n >= 1, "{name}");
    let key = FrameKey {
        traj_id: 1,
        frame_idx: 0,
    };
    let stored = db.frame_hash(key).unwrap();
    let via = ConCorpus::hash_con_text(&text).unwrap();
    assert_eq!(via.to_bytes(), stored.to_bytes(), "{name}");
    assert_eq!(db.find_by_hash(via).unwrap(), Some(key), "{name}");
    if n > 1 {
        let other = db
            .frame_hash(FrameKey {
                traj_id: 1,
                frame_idx: 1,
            })
            .unwrap();
        if other.to_bytes() != stored.to_bytes() {
            assert_ne!(via.to_bytes(), other.to_bytes());
        }
    }
}

#[test]
fn hash_con_text_matches_the_bytes_the_corpus_stores() {
    stored_hash_matches("tiny_cuh2.con");
    stored_hash_matches("tiny_cuh2_vel_forces.con");
    stored_hash_matches("tiny_multi_cuh2.con");
}
