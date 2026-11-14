//! Arbitrary CON, gzip, zstd, and CON-bin bytes through path ingest.

#![no_main]

use libfuzzer_sys::fuzz_target;
use readcon_core::compression::read_file_bytes;
use readcon_core::iterators::frames_from_bytes;
use readcon_db::ConCorpus;

fn ingest_path(bytes: &[u8], name: &str) {
    let Ok(dir) = tempfile::tempdir() else {
        return;
    };
    let file = dir.path().join(name);
    if std::fs::write(&file, bytes).is_err() {
        return;
    }
    let Ok(db_dir) = tempfile::tempdir() else {
        return;
    };
    let Ok(db) = ConCorpus::open_with_map_size(db_dir.path(), 8 * 1024 * 1024) else {
        return;
    };
    let _ = db.append_trajectory_path(1, &file);
    if let Ok(text) = std::str::from_utf8(bytes) {
        let _ = db.append_trajectory_str(2, text, name);
    }
    if let Ok(plain) = read_file_bytes(&file)
        && plain.len() <= 1024 * 1024
        && let Ok(text) = std::str::from_utf8(&plain)
    {
        let _ = db.append_trajectory_str(3, text, name);
        if let Ok(frames) = frames_from_bytes(&plain, Some(1)) {
            let _ = db.append_trajectory_frames(4, &frames, name);
        }
    }
}

fuzz_target!(|data: &[u8]| {
    if data.is_empty() || data.len() > 32 * 1024 {
        return;
    }
    let name = match data[0] % 4 {
        0 => "fuzz.con",
        1 => "fuzz.con.gz",
        2 => "fuzz.con.zst",
        _ => "fuzz.conbin",
    };
    ingest_path(data, name);
});
