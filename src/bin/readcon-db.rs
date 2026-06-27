//! CLI for corpus ingest / select / export (metatrain XYZ, etc.)
//!
//! ```text
//! readcon-db ingest <corpus_dir> --start-id 1 <file.con>...
//! readcon-db ingest-dir <corpus_dir> <con_directory>
//! readcon-db select <corpus_dir> [--symbol Cu] [--traj N] [--export out.xyz]
//! readcon-db dedup-export <corpus_dir> --symbol Cu -o train.xyz
//! ```

use std::env;
use std::process::ExitCode;

use readcon_db::{ConCorpus, Select};

fn usage() -> ExitCode {
    eprintln!(
        "Usage:
  readcon-db ingest <corpus_dir> [--start-id N] <file.con>...
  readcon-db ingest-dir <corpus_dir> <dir_with_con_files>
  readcon-db select <corpus_dir> [--traj N] [--symbol S] [--natoms-min A] [--natoms-max B] [--export out.xyz]
  readcon-db dedup-export <corpus_dir> [--symbol S] -o out.xyz
  readcon-db hash-file <file.con>   # print xxh3-128 hex of first frame (canonical)
"
    );
    ExitCode::from(2)
}

fn main() -> ExitCode {
    let mut args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        return usage();
    }
    let cmd = args.remove(0);
    let run = (|| -> Result<(), Box<dyn std::error::Error>> {
        match cmd.as_str() {
            "ingest" => {
                let corpus = args.first().ok_or("corpus_dir")?.clone();
                let mut start = 1u64;
                let mut files = Vec::new();
                let mut i = 1;
                while i < args.len() {
                    if args[i] == "--start-id" {
                        start = args.get(i + 1).ok_or("id")?.parse()?;
                        i += 2;
                        continue;
                    }
                    files.push(args[i].clone());
                    i += 1;
                }
                let db = ConCorpus::open(&corpus)?;
                let mut tid = start;
                for f in files {
                    let n = db.append_trajectory_path(tid, &f)?;
                    println!("traj {tid}: {n} frames from {f}");
                    tid += 1;
                }
            }
            "ingest-dir" => {
                let corpus = args.first().ok_or("corpus")?.clone();
                let dir = args.get(1).ok_or("dir")?.clone();
                let db = ConCorpus::open(&corpus)?;
                for (tid, n, p) in db.ingest_directory(&dir, 1)? {
                    println!("traj {tid}: {n} frames <- {p}");
                }
            }
            "select" | "dedup-export" => {
                let corpus = args.first().ok_or("corpus")?.clone();
                let mut traj = None;
                let mut symbol = None;
                let mut nmin = 0u32;
                let mut nmax = u32::MAX;
                let mut export = None;
                let mut i = 1;
                while i < args.len() {
                    match args[i].as_str() {
                        "--traj" => {
                            traj = Some(args.get(i + 1).ok_or("traj")?.parse()?);
                            i += 2;
                        }
                        "--symbol" => {
                            symbol = Some(args.get(i + 1).ok_or("sym")?.clone());
                            i += 2;
                        }
                        "--natoms-min" => {
                            nmin = args.get(i + 1).ok_or("n")?.parse()?;
                            i += 2;
                        }
                        "--natoms-max" => {
                            nmax = args.get(i + 1).ok_or("n")?.parse()?;
                            i += 2;
                        }
                        "--export" | "-o" => {
                            export = Some(args.get(i + 1).ok_or("path")?.clone());
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }
                let db = ConCorpus::open(&corpus)?;
                let mut sel = Select::new().natoms_range(nmin, nmax);
                if let Some(t) = traj {
                    sel = sel.trajectory(t);
                }
                if let Some(s) = symbol.clone() {
                    sel = sel.require_symbol(s);
                }
                let keys = if cmd == "dedup-export" {
                    db.unique_frame_keys(&sel)?
                } else {
                    db.select(&sel)?
                };
                println!("{} keys", keys.len());
                if let Some(path) = export {
                    let n = db.export_extxyz(&keys, &path, "energy")?;
                    println!("wrote {n} frames -> {path}");
                } else {
                    for k in keys.iter().take(20) {
                        println!("  traj={} frame={}", k.traj_id, k.frame_idx);
                    }
                    if keys.len() > 20 {
                        println!("  ...");
                    }
                }
            }
            "hash-file" => {
                let f = args.first().ok_or("file")?;
                let text = std::fs::read_to_string(f)?;
                let h = ConCorpus::hash_con_text(&text)?;
                println!("{}", h.to_hex());
            }
            _ => return Err("unknown command".into()),
        }
        Ok(())
    })();
    match run {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e}");
            usage()
        }
    }
}
