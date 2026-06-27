//! Wall-clock insert / point extract / select vs workload size.
//! Usage: cargo run --release --example bench_corpus_ops -- <frames_dir> <out.json>
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use readcon_db::{ConCorpus, FrameKey, Select};
use serde_json::json;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let frames_dir = PathBuf::from(env::args().nth(1).expect("frames_dir"));
    let out_path = env::args().nth(2).unwrap_or_else(|| "/tmp/readcon_db_bench.json".into());
    let work = tempfile::tempdir()?;
    let mut results = Vec::new();

    let mut files: Vec<_> = fs::read_dir(&frames_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("con"))
        .collect();
    files.sort();

    for f in &files {
        let name = f.file_name().unwrap().to_string_lossy().to_string();
        let n_frames: usize = name
            .trim_start_matches('n')
            .trim_end_matches(".con")
            .parse()
            .unwrap_or(0);
        if n_frames == 0 {
            continue;
        }
        let corpus_path = work.path().join(&name);
        let _ = fs::remove_dir_all(&corpus_path);
        let db = ConCorpus::open(&corpus_path)?;

        let t0 = Instant::now();
        let nf = db.append_trajectory_path(1, f)?;
        let insert_s = t0.elapsed().as_secs_f64();

        let t1 = Instant::now();
        let mut extract_sum = 0.0;
        let rounds = 20usize;
        for _ in 0..rounds {
            for i in 0..nf {
                let fr = db.get_frame(FrameKey {
                    traj_id: 1,
                    frame_idx: i,
                })?;
                extract_sum += fr.atom_data.len() as f64; // touch
            }
        }
        let extract_s = t1.elapsed().as_secs_f64() / rounds as f64;

        let t2 = Instant::now();
        let sel_rounds = 50usize;
        for _ in 0..sel_rounds {
            let _ = db.select(&Select::new().trajectory(1).require_symbol("Cu"))?;
        }
        let select_s = t2.elapsed().as_secs_f64() / sel_rounds as f64;

        // concurrent readers: N threads each doing full point extracts
        let readers = 8usize;
        let t3 = Instant::now();
        std::thread::scope(|s| {
            for _ in 0..readers {
                let db = &db;
                let nf = nf;
                s.spawn(move || {
                    for i in 0..nf {
                        let _ = db.get_frame(FrameKey {
                            traj_id: 1,
                            frame_idx: i,
                        });
                    }
                });
            }
        });
        let concurrent_s = t3.elapsed().as_secs_f64();

        results.push(json!({
            "backend": "readcon-db",
            "n_frames": nf,
            "insert_s": insert_s,
            "extract_all_mean_s": extract_s,
            "select_cu_mean_s": select_s,
            "concurrent_8readers_extract_s": concurrent_s,
            "insert_frames_per_s": nf as f64 / insert_s,
            "extract_frames_per_s": nf as f64 / extract_s,
        }));
        eprintln!(
            "readcon-db n={nf} insert={insert_s:.4}s extract={extract_s:.4}s sel={select_s:.5}s conc8={concurrent_s:.4}s"
        );
    }

    fs::write(&out_path, serde_json::to_string_pretty(&results)?)?;
    println!("wrote {out_path}");
    Ok(())
}
