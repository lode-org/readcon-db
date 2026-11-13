//! Ingest CON text into an LMDB corpus and run a `Select` query.

#![no_main]

use libfuzzer_sys::fuzz_target;
use readcon_db::{ConCorpus, Select};

fn symbol_of(byte: u8) -> &'static str {
    match byte % 4 {
        0 => "Cu",
        1 => "H",
        2 => "O",
        _ => "He",
    }
}

fuzz_target!(|data: &[u8]| {
    if data.len() > 8 * 1024 {
        return;
    }
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    let Ok(dir) = tempfile::tempdir() else {
        return;
    };
    let Ok(db) = ConCorpus::open_with_map_size(dir.path(), 8 * 1024 * 1024) else {
        return;
    };
    db.set_compress_frames(data.first().copied().unwrap_or(0) & 1 == 1);
    let ingested = match db.append_trajectory_str(1, text, "fuzz") {
        Ok(count) => count,
        Err(_) => return,
    };
    assert!((ingested as usize) <= text.len().saturating_add(1));
    let mid = text.len() / 2;
    let _ = db.extend_trajectory_str(1, &text[mid..], "fuzz-ext");

    let mut query = Select::new().trajectory(1).limit(4);
    match data.get(1).copied().unwrap_or(0) % 6 {
        0 => query = query.require_symbol(symbol_of(data.get(2).copied().unwrap_or(0))),
        1 => query = query.natoms_range(0, 16),
        2 => query = query.require_forces(),
        3 => query = query.require_velocities(),
        4 => query = query.energy_range(-1.0e6, 1.0e6),
        _ => query = query.exact_composition("CuH2"),
    }
    let Ok(keys) = db.select(&query) else {
        return;
    };
    assert!(keys.len() <= (ingested as usize).saturating_add(text.len()));
    for key in keys.iter().take(2) {
        if let Ok(frame) = db.get_frame(*key) {
            std::hint::black_box(frame.atom_data.len());
        }
        let _ = db.get_positions(*key);
        let _ = db.frame_formula(*key);
        if let Ok(hash) = db.frame_hash(*key) {
            let _ = db.find_by_hash(hash);
        }
    }
});
