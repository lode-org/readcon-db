//! Corrupted and truncated RCSO position blobs and RCSB batches.

#![no_main]

use libfuzzer_sys::fuzz_target;
use readcon_db::CookedSoa;

fuzz_target!(|data: &[u8]| {
    if data.len() > 64 * 1024 {
        return;
    }
    let _ = CookedSoa::decode(data);
    let _ = CookedSoa::try_decode(data);
    let mut flat = Vec::new();
    let _ = CookedSoa::copy_positions(data, &mut flat);
    if data.len() > 16 {
        let _ = CookedSoa::decode(&data[..data.len() / 2]);
        let _ = CookedSoa::copy_positions(&data[..data.len() / 2], &mut flat);
    }
});
