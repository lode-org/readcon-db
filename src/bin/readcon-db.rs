//! CLI for corpus ingest / select / export / reindex
//!
//! ```text
//! readcon-db ingest <corpus_dir> --start-id 1 <file.con>...
//! readcon-db ingest-dir <corpus_dir> <con_directory>
//! readcon-db select <corpus_dir> [filters...] [--export out.xyz]
//! readcon-db dedup-export <corpus_dir> [filters...] -o train.xyz
//! readcon-db reindex <corpus_dir>
//! readcon-db hash-file <file.con>
//! ```

use std::env;
use std::process::ExitCode;

use readcon_db::{ConCorpus, Select};

fn usage() -> ExitCode {
    eprintln!(
        "Usage:
  readcon-db ingest <corpus_dir> [--start-id N] <file.con>...
  readcon-db ingest-dir <corpus_dir> <dir_with_con_files>
  readcon-db select <corpus_dir> [--traj N] [--symbol S] [--natoms-min A] [--natoms-max B]
                     [--energy-min E] [--energy-max E] [--fmax-min F] [--fmax-max F]
                     [--elem SYM:COUNT] [--elem-min SYM:COUNT] [--formula Cu:2|H:2]
                     [--require-forces] [--require-velocities] [--require-energy]
                     [--export out.xyz]
  readcon-db dedup-export <corpus_dir> [same filters as select] -o out.xyz
  readcon-db reindex <corpus_dir>
  readcon-db hash-file <file.con>
"
    );
    ExitCode::from(2)
}

fn parse_sym_count(s: &str) -> Result<(String, u32), Box<dyn std::error::Error>> {
    let (sym, cnt) = s
        .split_once(':')
        .ok_or("expected SYM:COUNT")?;
    Ok((sym.to_string(), cnt.parse()?))
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
            "reindex" => {
                let corpus = args.first().ok_or("corpus")?.clone();
                let db = ConCorpus::open(&corpus)?;
                let n = db.reindex()?;
                println!("reindexed {n} frames");
            }
            "select" | "dedup-export" => {
                let corpus = args.first().ok_or("corpus")?.clone();
                let mut traj = None;
                let mut symbol = None;
                let mut nmin = 0u32;
                let mut nmax = u32::MAX;
                let mut emin: Option<f64> = None;
                let mut emax: Option<f64> = None;
                let mut fmin: Option<f64> = None;
                let mut fmax: Option<f64> = None;
                let mut elem_exact = Vec::new();
                let mut elem_min = Vec::new();
                let mut formula = None;
                let mut req_forces = false;
                let mut req_vels = false;
                let mut req_energy = false;
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
                        "--energy-min" => {
                            emin = Some(args.get(i + 1).ok_or("e")?.parse()?);
                            i += 2;
                        }
                        "--energy-max" => {
                            emax = Some(args.get(i + 1).ok_or("e")?.parse()?);
                            i += 2;
                        }
                        "--fmax-min" => {
                            fmin = Some(args.get(i + 1).ok_or("f")?.parse()?);
                            i += 2;
                        }
                        "--fmax-max" => {
                            fmax = Some(args.get(i + 1).ok_or("f")?.parse()?);
                            i += 2;
                        }
                        "--elem" => {
                            elem_exact.push(parse_sym_count(args.get(i + 1).ok_or("SYM:COUNT")?)?);
                            i += 2;
                        }
                        "--elem-min" => {
                            elem_min.push(parse_sym_count(args.get(i + 1).ok_or("SYM:COUNT")?)?);
                            i += 2;
                        }
                        "--formula" => {
                            formula = Some(args.get(i + 1).ok_or("formula")?.clone());
                            i += 2;
                        }
                        "--require-forces" => {
                            req_forces = true;
                            i += 1;
                        }
                        "--require-velocities" => {
                            req_vels = true;
                            i += 1;
                        }
                        "--require-energy" => {
                            req_energy = true;
                            i += 1;
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
                if emin.is_some() || emax.is_some() {
                    sel = sel.energy_range(
                        emin.unwrap_or(f64::NEG_INFINITY),
                        emax.unwrap_or(f64::INFINITY),
                    );
                }
                if fmin.is_some() || fmax.is_some() {
                    sel = sel.fmax_range(fmin.unwrap_or(0.0), fmax.unwrap_or(f64::INFINITY));
                }
                for (sym, c) in elem_exact {
                    sel = sel.element_exact(sym, c);
                }
                for (sym, c) in elem_min {
                    sel = sel.element_min(sym, c);
                }
                if let Some(f) = formula {
                    sel = sel.exact_composition(f);
                }
                if req_forces {
                    sel = sel.require_forces();
                }
                if req_vels {
                    sel = sel.require_velocities();
                }
                if req_energy {
                    sel = sel.require_energy();
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
