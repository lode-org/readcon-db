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

use readcon_db::{join_corpus_dirs, ConCorpus, Select, ShardedConCorpus, DEFAULT_N_SHARDS};

fn usage() -> ExitCode {
    eprintln!(
        "Usage:
  readcon-db ingest <corpus_dir> [--start-id N] <file.con>...
  readcon-db ingest-dir <corpus_dir> <dir_with_con_files>
  readcon-db select <corpus_dir> [--traj N] [--symbol S] [--natoms-min A] [--natoms-max B]
                     [--energy-min E] [--energy-max E] [--fmax-min F] [--fmax-max F]
                     [--elem SYM:COUNT] [--elem-min SYM:COUNT] [--formula Cu:2|H:2]
                     [--mass-min M] [--mass-max M] [--volume-min V] [--volume-max V]
                     [--pbc X,Y,Z] [--time-min T] [--time-max T] [--timestep-min DT] [--timestep-max DT]
                     [--frame-index-min I] [--frame-index-max I]
                     [--neb-bead-min N] [--neb-bead-max N] [--neb-band-min B] [--neb-band-max B]
                     [--charge-min C] [--charge-max C] [--magmom-min M] [--magmom-max M]
                     [--require-forces] [--require-velocities] [--require-energy]
                     [--export out.xyz]
  readcon-db dedup-export <corpus_dir> [same filters as select] -o out.xyz
  readcon-db reindex <corpus_dir>
  readcon-db shard-init <root> [--shards N]
  readcon-db shard-ingest <root> --shard S --start-id T <file.con>...
  readcon-db shard-select <root> [--symbol S] ...
  readcon-db compact-join <sharded_root> <single_dst>
  readcon-db compact-split <single_src> <sharded_dst> [--shards N]
  readcon-db compact-export-extxyz <corpus_or_shard_root> <out.xyz> [--sharded] [--symbol S]
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
                let mut mass_min: Option<f64> = None;
                let mut mass_max: Option<f64> = None;
                let mut vol_min: Option<f64> = None;
                let mut vol_max: Option<f64> = None;
                let mut pbc: Option<[bool; 3]> = None;
                let mut time_min: Option<f64> = None;
                let mut time_max: Option<f64> = None;
                let mut timestep_min: Option<f64> = None;
                let mut timestep_max: Option<f64> = None;
                let mut fi_min: Option<f64> = None;
                let mut fi_max: Option<f64> = None;
                let mut bead_min: Option<f64> = None;
                let mut bead_max: Option<f64> = None;
                let mut band_min: Option<f64> = None;
                let mut band_max: Option<f64> = None;
                let mut charge_min: Option<f64> = None;
                let mut charge_max: Option<f64> = None;
                let mut mag_min: Option<f64> = None;
                let mut mag_max: Option<f64> = None;
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

                        "--mass-min" => { mass_min = Some(args.get(i + 1).ok_or("m")?.parse()?); i += 2; }
                        "--mass-max" => { mass_max = Some(args.get(i + 1).ok_or("m")?.parse()?); i += 2; }
                        "--volume-min" => { vol_min = Some(args.get(i + 1).ok_or("v")?.parse()?); i += 2; }
                        "--volume-max" => { vol_max = Some(args.get(i + 1).ok_or("v")?.parse()?); i += 2; }
                        "--pbc" => {
                            let s = args.get(i + 1).ok_or("x,y,z")?;
                            let parts: Vec<_> = s.split(',').collect();
                            if parts.len() != 3 { return Err("pbc needs X,Y,Z as 0/1 or true/false".into()); }
                            let parse_b = |x: &str| -> Result<bool, Box<dyn std::error::Error>> {
                                Ok(matches!(x.trim().to_ascii_lowercase().as_str(), "1" | "true" | "t" | "yes"))
                            };
                            pbc = Some([parse_b(parts[0])?, parse_b(parts[1])?, parse_b(parts[2])?]);
                            i += 2;
                        }
                        "--time-min" => { time_min = Some(args.get(i + 1).ok_or("t")?.parse()?); i += 2; }
                        "--time-max" => { time_max = Some(args.get(i + 1).ok_or("t")?.parse()?); i += 2; }
                        "--timestep-min" => { timestep_min = Some(args.get(i + 1).ok_or("dt")?.parse()?); i += 2; }
                        "--timestep-max" => { timestep_max = Some(args.get(i + 1).ok_or("dt")?.parse()?); i += 2; }
                        "--frame-index-min" => { fi_min = Some(args.get(i + 1).ok_or("i")?.parse()?); i += 2; }
                        "--frame-index-max" => { fi_max = Some(args.get(i + 1).ok_or("i")?.parse()?); i += 2; }
                        "--neb-bead-min" => { bead_min = Some(args.get(i + 1).ok_or("n")?.parse()?); i += 2; }
                        "--neb-bead-max" => { bead_max = Some(args.get(i + 1).ok_or("n")?.parse()?); i += 2; }
                        "--neb-band-min" => { band_min = Some(args.get(i + 1).ok_or("b")?.parse()?); i += 2; }
                        "--neb-band-max" => { band_max = Some(args.get(i + 1).ok_or("b")?.parse()?); i += 2; }
                        "--charge-min" => { charge_min = Some(args.get(i + 1).ok_or("c")?.parse()?); i += 2; }
                        "--charge-max" => { charge_max = Some(args.get(i + 1).ok_or("c")?.parse()?); i += 2; }
                        "--magmom-min" => { mag_min = Some(args.get(i + 1).ok_or("m")?.parse()?); i += 2; }
                        "--magmom-max" => { mag_max = Some(args.get(i + 1).ok_or("m")?.parse()?); i += 2; }
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
                if mass_min.is_some() || mass_max.is_some() {
                    sel = sel.mass_range(mass_min.unwrap_or(f64::NEG_INFINITY), mass_max.unwrap_or(f64::INFINITY));
                }
                if vol_min.is_some() || vol_max.is_some() {
                    sel = sel.volume_range(vol_min.unwrap_or(f64::NEG_INFINITY), vol_max.unwrap_or(f64::INFINITY));
                }
                if let Some(p) = pbc {
                    sel = sel.pbc(p);
                }
                if time_min.is_some() || time_max.is_some() {
                    sel = sel.time_range(time_min.unwrap_or(f64::NEG_INFINITY), time_max.unwrap_or(f64::INFINITY));
                }
                if timestep_min.is_some() || timestep_max.is_some() {
                    sel = sel.timestep_range(
                        timestep_min.unwrap_or(f64::NEG_INFINITY),
                        timestep_max.unwrap_or(f64::INFINITY),
                    );
                }
                if fi_min.is_some() || fi_max.is_some() {
                    sel = sel.frame_index_range(fi_min.unwrap_or(f64::NEG_INFINITY), fi_max.unwrap_or(f64::INFINITY));
                }
                if bead_min.is_some() || bead_max.is_some() {
                    sel = sel.neb_bead_range(bead_min.unwrap_or(f64::NEG_INFINITY), bead_max.unwrap_or(f64::INFINITY));
                }
                if band_min.is_some() || band_max.is_some() {
                    sel = sel.neb_band_range(
                        band_min.unwrap_or(f64::NEG_INFINITY),
                        band_max.unwrap_or(f64::INFINITY),
                    );
                }
                if charge_min.is_some() || charge_max.is_some() {
                    sel = sel.charge_range(charge_min.unwrap_or(f64::NEG_INFINITY), charge_max.unwrap_or(f64::INFINITY));
                }
                if mag_min.is_some() || mag_max.is_some() {
                    sel = sel.magmom_range(mag_min.unwrap_or(f64::NEG_INFINITY), mag_max.unwrap_or(f64::INFINITY));
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

            "shard-init" => {
                let root = args.first().ok_or("root")?.clone();
                let mut ns = DEFAULT_N_SHARDS;
                let mut i = 1;
                while i < args.len() {
                    if args[i] == "--shards" {
                        ns = args.get(i + 1).ok_or("n")?.parse()?;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                let _ = ShardedConCorpus::open(&root, ns)?;
                println!("initialized {root} with {ns} shards");
            }
            "shard-ingest" => {
                let root = args.first().ok_or("root")?.clone();
                let mut shard = None::<u32>;
                let mut start = 1u64;
                let mut files = Vec::new();
                let mut i = 1;
                while i < args.len() {
                    match args[i].as_str() {
                        "--shard" => {
                            shard = Some(args.get(i + 1).ok_or("shard")?.parse()?);
                            i += 2;
                        }
                        "--start-id" => {
                            start = args.get(i + 1).ok_or("id")?.parse()?;
                            i += 2;
                        }
                        _ => {
                            files.push(args[i].clone());
                            i += 1;
                        }
                    }
                }
                let sid = shard.ok_or("--shard required (HPC: use $SLURM_PROCID % n_shards)")?;
                let db = ShardedConCorpus::open_shard(&root, sid)?;
                let mut tid = start;
                for f in files {
                    // Ensure traj routes to this shard
                    let routed = ShardedConCorpus::shard_for_traj(tid, {
                        let m: readcon_db::ShardManifest = serde_json::from_str(
                            &std::fs::read_to_string(std::path::Path::new(&root).join("shards.json"))?,
                        )?;
                        m.n_shards
                    });
                    if routed != sid {
                        return Err(format!(
                            "traj {tid} routes to shard {routed}, not {sid}; choose start-id ≡ {sid} (mod n_shards)"
                        )
                        .into());
                    }
                    let n = db.append_trajectory_path(tid, &f)?;
                    println!("shard {sid} traj {tid}: {n} frames from {f}");
                    tid += 1;
                    // skip ids that don't map to this shard
                    while ShardedConCorpus::shard_for_traj(tid, {
                        let m: readcon_db::ShardManifest = serde_json::from_str(
                            &std::fs::read_to_string(std::path::Path::new(&root).join("shards.json"))?,
                        )?;
                        m.n_shards
                    }) != sid
                    {
                        tid += 1;
                    }
                }
            }
            "shard-select" => {
                let root = args.first().ok_or("root")?.clone();
                let mut symbol = None;
                let mut i = 1;
                while i < args.len() {
                    if args[i] == "--symbol" {
                        symbol = Some(args.get(i + 1).ok_or("sym")?.clone());
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                let mut db = ShardedConCorpus::open(&root, DEFAULT_N_SHARDS)?;
                let mut sel = Select::new();
                if let Some(s) = symbol {
                    sel = sel.require_symbol(s);
                }
                let keys = db.select(&sel)?;
                println!("{} keys across {} shards", keys.len(), db.n_shards());
                for k in keys.iter().take(20) {
                    println!("  traj={} frame={}", k.traj_id, k.frame_idx);
                }
            }

            "compact-join" => {
                let src = args.first().ok_or("sharded_root")?.clone();
                let dst = args.get(1).ok_or("single_dst")?.clone();
                let mut s = ShardedConCorpus::open(&src, DEFAULT_N_SHARDS)?;
                let n = s.join_to_single_env(&dst)?;
                println!("joined {n} frames -> {dst} (single-env-lmdb)");
            }
            "compact-split" => {
                let src = args.first().ok_or("single_src")?.clone();
                let dst = args.get(1).ok_or("sharded_dst")?.clone();
                let mut ns = DEFAULT_N_SHARDS;
                let mut i = 2;
                while i < args.len() {
                    if args[i] == "--shards" {
                        ns = args.get(i + 1).ok_or("n")?.parse()?;
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                let single = ConCorpus::open(&src)?;
                let n = ShardedConCorpus::split_single_to_sharded(&single, &dst, ns)?;
                println!("split {n} frames -> {dst} ({ns} shards, sharded-lmdb)");
            }
            "compact-export-extxyz" => {
                let src = args.first().ok_or("src")?.clone();
                let out = args.get(1).ok_or("out.xyz")?.clone();
                let mut sharded = false;
                let mut symbol = None;
                let mut i = 2;
                while i < args.len() {
                    match args[i].as_str() {
                        "--sharded" => {
                            sharded = true;
                            i += 1;
                        }
                        "--symbol" => {
                            symbol = Some(args.get(i + 1).ok_or("sym")?.clone());
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }
                let mut sel = Select::new();
                if let Some(s) = symbol {
                    sel = sel.require_symbol(s);
                }
                let n = if sharded {
                    let tmp = tempfile::tempdir()?;
                    let joined = tmp.path().join("j");
                    let mut sc2 = ShardedConCorpus::open(&src, DEFAULT_N_SHARDS)?;
                    sc2.join_to_single_env(&joined)?;
                    let db = ConCorpus::open(&joined)?;
                    let keys = db.select(&sel)?;
                    db.export_extxyz(&keys, &out, "energy")?
                } else {
                    let db = ConCorpus::open(&src)?;
                    let keys = db.select(&sel)?;
                    db.export_extxyz(&keys, &out, "energy")?
                };
                println!("wrote {n} frames extxyz -> {out} (analysis export)");
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
