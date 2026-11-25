//! `readcon-db hash-file` against the hash the corpus stores. Integration
//! test so `CARGO_BIN_EXE_readcon-db` is set.

use std::path::{Path, PathBuf};
use std::process::Command;

use readcon_db::{ConCorpus, FrameKey};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_readcon-db"))
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("resources")
        .join("test")
        .join(name)
}

fn hash_file(path: &Path) -> String {
    let out = bin().arg("hash-file").arg(path).output().unwrap();
    assert!(
        out.status.success(),
        "hash-file failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8(out.stdout).unwrap();
    text.trim_end().to_owned()
}

#[test]
fn hash_file_matches_stored_hash_of_high_precision_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("db");
    let head = fixture("highprec_a.con");
    let tail = fixture("highprec_b.con");
    let out = bin()
        .arg("ingest")
        .arg(&root)
        .args(["--start-id", "7"])
        .arg(&head)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "ingest failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    {
        let db = ConCorpus::open(&root).unwrap();
        db.extend_trajectory_path(7, &tail).unwrap();
    }
    let db = ConCorpus::open_readonly(&root).unwrap();
    for (idx, path) in [(0, &head), (1, &tail)] {
        let key = FrameKey {
            traj_id: 7,
            frame_idx: idx,
        };
        let stored = db.frame_hash(key).unwrap();
        assert_eq!(hash_file(path), stored.to_hex(), "{}", path.display());
        assert_eq!(db.find_by_hash(stored).unwrap(), Some(key));
    }
}
