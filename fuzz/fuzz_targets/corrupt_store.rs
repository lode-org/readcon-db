//! Truncated and corrupted LMDB environments, plus stored frame blobs.

#![no_main]

use libfuzzer_sys::fuzz_target;
use readcon_db::ConCorpus;

fn touch(db: &ConCorpus) {
    let Ok(keys) = db.list_frame_keys() else {
        return;
    };
    for key in keys.iter().take(8) {
        let _ = db.get_frame_text(*key);
        let _ = db.get_frame(*key);
        let _ = db.get_positions(*key);
        let _ = db.get_velocities(*key);
        let _ = db.get_forces(*key);
        let _ = db.trajectory_positions(key.traj_id);
        let _ = db.get_cooked_soa(*key);
    }
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 64 * 1024 {
        return;
    }
    let Ok(dir) = tempfile::tempdir() else {
        return;
    };
    let mdb = dir.path().join("data.mdb");
    if std::fs::write(&mdb, data).is_err() {
        return;
    }
    if let Ok(db) = ConCorpus::open_readonly(dir.path()) {
        touch(&db);
    }

    // A real corpus, then the fuzzer's bytes overwrite a prefix or a suffix.
    let Ok(fresh) = tempfile::tempdir() else {
        return;
    };
    if let Ok(db) = ConCorpus::open_with_map_size(fresh.path(), 8 * 1024 * 1024) {
        let _ = db.append_trajectory_str(1, "\n\n1 1 1\n90 90 90\n\n\n1\n1\n1\nH\nCoordinates of Component 1\n0 0 0 0 1\n", "seed");
        drop(db);
        let path = fresh.path().join("data.mdb");
        if let Ok(mut bytes) = std::fs::read(&path) {
            let n = data.len().min(bytes.len());
            if data[0] & 1 == 0 {
                bytes[..n].copy_from_slice(&data[..n]);
            } else {
                let start = bytes.len() - n;
                bytes[start..].copy_from_slice(&data[..n]);
            }
            if std::fs::write(&path, &bytes).is_ok()
                && let Ok(db) = ConCorpus::open_readonly(fresh.path())
            {
                touch(&db);
            }
        }
    }
});
