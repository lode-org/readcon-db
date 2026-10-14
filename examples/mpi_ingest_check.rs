//! `mpirun -n P mpi_ingest_check <single|sharded> <outdir> <batch> <n_shards> <first_traj_id> <file.con>...`
//!
//! Ingests the files as `first_traj_id`, `first_traj_id + 1`, ... with P ranks
//! into `<outdir>/mpi`, repeats the ingest serially on rank 0 into
//! `<outdir>/serial`, and prints the sha256 of both canonical dumps. `outdir`
//! must be empty: a second run that reuses a traj id returns `TrajExists`.
use std::path::PathBuf;

use mpi::traits::*;
use readcon_db::mpi_ingest;
use readcon_db::{ConCorpus, ShardedConCorpus};
use sha2::{Digest, Sha256};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() {
    let universe = mpi::initialize().expect("MPI init");
    let world = universe.world();
    let a: Vec<String> = std::env::args().collect();
    let sharded = a[1] == "sharded";
    let out = PathBuf::from(&a[2]);
    let batch: usize = a[3].parse().unwrap();
    let n_shards: u32 = a[4].parse().unwrap();
    let first_id: u64 = a[5].parse().unwrap();
    let files: Vec<(u64, PathBuf)> = a[6..]
        .iter()
        .enumerate()
        .map(|(i, f)| (first_id + i as u64, PathBuf::from(f)))
        .collect();
    world.barrier();
    let t = std::time::Instant::now();
    let counts = if sharded {
        mpi_ingest::append_trajectory_paths_sharded(
            &world,
            out.join("mpi"),
            n_shards,
            &files,
            batch,
        )
    } else {
        mpi_ingest::append_trajectory_paths(&world, out.join("mpi"), &files, batch)
    }
    .unwrap();
    world.barrier();
    let dt = t.elapsed().as_secs_f64();
    if world.rank() != 0 {
        return;
    }
    let (mpi_dump, ser_dump, ds) = if sharded {
        let mpi_dump = ShardedConCorpus::open_existing(out.join("mpi"))
            .unwrap()
            .canonical_dump()
            .unwrap();
        let mut s = ShardedConCorpus::open(out.join("serial"), n_shards).unwrap();
        let t0 = std::time::Instant::now();
        for (id, f) in &files {
            s.append_trajectory_path(*id, f).unwrap();
        }
        (
            mpi_dump,
            s.canonical_dump().unwrap(),
            t0.elapsed().as_secs_f64(),
        )
    } else {
        let mpi_dump = ConCorpus::open(out.join("mpi"))
            .unwrap()
            .canonical_dump()
            .unwrap();
        let s = ConCorpus::open(out.join("serial")).unwrap();
        let t0 = std::time::Instant::now();
        for (id, f) in &files {
            s.append_trajectory_path(*id, f).unwrap();
        }
        (
            mpi_dump,
            s.canonical_dump().unwrap(),
            t0.elapsed().as_secs_f64(),
        )
    };
    let (m, s) = (
        hex(&Sha256::digest(&mpi_dump)),
        hex(&Sha256::digest(&ser_dump)),
    );
    let n: u32 = counts.iter().sum();
    println!(
        "mode={} ranks={} files={} frames={n} mpi_s={dt:.3} serial_s={ds:.3} mpi_sha256={m} serial_sha256={s} match={}",
        a[1],
        world.size(),
        files.len(),
        m == s
    );
}
