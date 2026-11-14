//! cargo run --example ingest_select -- [corpus_dir] [file.con]
use readcon_db::{ConCorpus, Select};
use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let dir = args.next().unwrap_or_else(|| {
        let path = env::temp_dir().join(format!("readcon_db_demo_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&path);
        path.display().to_string()
    });
    let file = args.next().unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("resources/test/tiny_cuh2.con")
            .display()
            .to_string()
    });
    let db = ConCorpus::open(&dir)?;
    let n = db.append_trajectory_path(1, &file)?;
    println!("ingested {n} frames into {dir}");
    let keys = db.select(&Select::new().trajectory(1).require_symbol("Cu"))?;
    println!("Cu frames: {}", keys.len());
    if let Some(k) = keys.first() {
        let h = db.frame_hash(*k)?;
        println!("hash[0] = {}", h.to_hex());
        println!("find_by_hash = {:?}", db.find_by_hash(h)?);
    }
    Ok(())
}
