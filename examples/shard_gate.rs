//! `mpiexec -n P shard_gate <outdir> <n_shards> <file.con>...`
//!
//! Sharded ingest of the files as trajectory ids 0, 1, ... then rank 0
//! drops the page cache on every shard `data.mdb` and times point reads
//! of the first and last frame of each file.
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::time::Instant;

use mpi::traits::*;
use readcon_db::mpi_ingest;
use readcon_db::{ConCorpus, FrameKey};

fn drop_cache(path: &std::path::Path) {
    let Ok(file) = File::open(path) else {
        return;
    };
    let _ = file.sync_all();
    unsafe {
        libc::posix_fadvise(file.as_raw_fd(), 0, 0, libc::POSIX_FADV_DONTNEED);
    }
}

fn main() {
    let universe = mpi::initialize().expect("MPI init");
    let world = universe.world();
    let args: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(&args[1]);
    let n_shards: u32 = args[2].parse().unwrap();
    let files: Vec<(u64, PathBuf)> = args[3..]
        .iter()
        .enumerate()
        .map(|(i, f)| (i as u64, PathBuf::from(f)))
        .collect();
    world.barrier();
    let t = Instant::now();
    let counts = mpi_ingest::append_trajectory_paths_sharded(
        &world,
        out.join("mpi"),
        n_shards,
        &files,
        4096,
    )
    .unwrap();
    world.barrier();
    let dt = t.elapsed().as_secs_f64();
    if world.rank() != 0 {
        return;
    }
    let n: u32 = counts.iter().sum();
    let root = out.join("mpi");
    let mut opened = Vec::new();
    for sid in 0..n_shards {
        let dir = root.join(format!("shard_{sid:04}"));
        let data = dir.join("data.mdb");
        if data.is_file() {
            opened.push((sid, ConCorpus::open_readonly(&dir).unwrap()));
        }
    }
    for (sid, _) in &opened {
        drop_cache(&root.join(format!("shard_{sid:04}/data.mdb")));
    }
    let t0 = Instant::now();
    let mut nreads = 0usize;
    let mut nbytes = 0usize;
    for (sid, db) in &opened {
        let nframes = counts[*sid as usize];
        if nframes == 0 {
            continue;
        }
        let step = (nframes / 32).max(1);
        let mut idx = 0u32;
        while idx < nframes {
            let text = db
                .get_frame_text(FrameKey {
                    traj_id: *sid as u64,
                    frame_idx: idx,
                })
                .unwrap();
            nbytes += text.len();
            nreads += 1;
            idx += step;
        }
    }
    let cold_s = t0.elapsed().as_secs_f64();
    let us = if nreads == 0 {
        0.0
    } else {
        cold_s * 1e6 / nreads as f64
    };
    let mut shard_bytes = 0u64;
    for sid in 0..n_shards {
        if let Ok(meta) = std::fs::metadata(root.join(format!("shard_{sid:04}/data.mdb"))) {
            shard_bytes += meta.len();
        }
    }
    println!(
        "ranks={} shards={n_shards} frames={n} files={} ingest_s={dt:.4} frames_per_s={:.1} cold_reads={nreads} cold_s={cold_s:.4} cold_us_per_frame={us:.1} cold_bytes={nbytes} shard_mdb_bytes={shard_bytes}",
        world.size(),
        files.len(),
        n as f64 / dt,
    );
}
