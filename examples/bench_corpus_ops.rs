//! Wall-clock insert / point extract / select vs workload size.
//! Usage: cargo run --release --example bench_corpus_ops -- <frames_dir> <out.json>
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

use readcon_db::{ConCorpus, FrameKey, Select};
use serde_json::{json, Value};

fn git_out(args: &[&str]) -> String {
    Command::new("git")
        .args(args)
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|text| text.trim().to_string())
        .unwrap_or_default()
}

fn read_trim(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty())
}

fn host_record() -> Value {
    let mut host = json!({
        "hostname": "host",
        "kernel": read_trim("/proc/sys/kernel/osrelease").unwrap_or_default(),
        "serial": true,
        "quantile": "type-7",
        "omp_num_threads": "1",
        "openblas_num_threads": "1",
        "mkl_num_threads": "1",
        "numexpr_num_threads": "1",
    });
    if let Some(cpu) = fs::read_to_string("/proc/cpuinfo").ok().and_then(|text| {
        text.lines()
            .find(|line| line.starts_with("model name"))
            .and_then(|line| line.split_once(':'))
            .map(|(_, value)| value.trim().to_string())
    }) {
        host["cpu"] = json!(cpu);
    }
    if let Some(mhz) = fs::read_to_string("/proc/cpuinfo").ok().and_then(|text| {
        text.lines()
            .find(|line| line.to_ascii_lowercase().starts_with("cpu mhz"))
            .and_then(|line| line.split_once(':'))
            .and_then(|(_, value)| value.trim().parse::<f64>().ok())
    }) {
        host["cpuinfo_mhz"] = json!(mhz);
    }
    let cpu0 = "/sys/devices/system/cpu/cpu0/cpufreq";
    if let Some(governor) = read_trim(&format!("{cpu0}/scaling_governor")) {
        host["scaling_governor"] = json!(governor);
    }
    if let Some(freq) =
        read_trim(&format!("{cpu0}/scaling_cur_freq")).and_then(|text| text.parse::<u64>().ok())
    {
        host["scaling_cur_freq_khz"] = json!(freq);
    }
    if let Some(no_turbo) = read_trim("/sys/devices/system/cpu/intel_pstate/no_turbo") {
        host["turbo"] = json!(if no_turbo == "1" { "off" } else { "on" });
    }
    if let Some(boost) = read_trim("/sys/devices/system/cpu/cpufreq/boost") {
        host["boost"] = json!(if boost == "1" { "on" } else { "off" });
    }
    host
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let frames_dir = PathBuf::from(env::args().nth(1).expect("frames_dir"));
    let out_path = env::args().nth(2).unwrap_or_else(|| {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("build/readcon_db_bench.json")
            .display()
            .to_string()
    });
    if let Some(parent) = PathBuf::from(&out_path).parent() {
        fs::create_dir_all(parent)?;
    }
    let work = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("build/bench_corpus_ops");
    if work.exists() {
        fs::remove_dir_all(&work)?;
    }
    fs::create_dir_all(&work)?;
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
        let corpus_path = work.join(&name);
        let _ = fs::remove_dir_all(&corpus_path);
        let db = ConCorpus::open(&corpus_path)?;

        let t0 = Instant::now();
        let nf = db.append_trajectory_path(1, f)?;
        let insert_s = t0.elapsed().as_secs_f64();

        let mut extract_sum = 0.0;
        let rounds = 20usize;
        let mut extract_samples_s = Vec::with_capacity(rounds);
        for _ in 0..rounds {
            let t_round = Instant::now();
            for i in 0..nf {
                let fr = db.get_frame(FrameKey {
                    traj_id: 1,
                    frame_idx: i,
                })?;
                extract_sum += fr.atom_data.len() as f64; // touch
            }
            extract_samples_s.push(t_round.elapsed().as_secs_f64());
        }
        std::hint::black_box(extract_sum);
        let extract_s = extract_samples_s.iter().sum::<f64>() / rounds as f64;

        let sel_rounds = 50usize;
        let mut select_samples_s = Vec::with_capacity(sel_rounds);
        for _ in 0..sel_rounds {
            let t_round = Instant::now();
            let _ = db.select(&Select::new().trajectory(1).require_symbol("Cu"))?;
            select_samples_s.push(t_round.elapsed().as_secs_f64());
        }
        let select_s = select_samples_s.iter().sum::<f64>() / sel_rounds as f64;

        // concurrent readers: N threads each doing full point extracts
        let readers = 8usize;
        let t3 = Instant::now();
        std::thread::scope(|s| {
            for _ in 0..readers {
                let db = &db;
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
            "extract_samples_s": extract_samples_s,
            "select_cu_mean_s": select_s,
            "select_cu_samples_s": select_samples_s,
            "concurrent_8readers_extract_s": concurrent_s,
            "insert_frames_per_s": nf as f64 / insert_s,
            "extract_frames_per_s": nf as f64 / extract_s,
        }));
        eprintln!(
            "readcon-db n={nf} insert={insert_s:.4}s extract={extract_s:.4}s sel={select_s:.5}s conc8={concurrent_s:.4}s"
        );
    }

    let payload = json!({
        "commit": git_out(&["rev-parse", "HEAD"]),
        "tree": git_out(&["rev-parse", "HEAD^{tree}"]),
        "host": host_record(),
        "rows": results,
    });
    fs::write(&out_path, serde_json::to_string_pretty(&payload)?)?;
    println!("wrote {out_path}");
    Ok(())
}
