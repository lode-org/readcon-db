//! `mpirun -n P mpi_ingest_check <file.con> <outdir> [batch] [traj_id]`
//!
//! Ingests with P ranks into `<outdir>/mpi`, repeats the ingest serially on
//! rank 0 into `<outdir>/serial`, and prints the sha256 of both canonical
//! dumps. `outdir` must be empty: a second run with the same traj id returns
//! `TrajExists`.
use mpi::traits::*;
use readcon_db::ConCorpus;
use sha2::{Digest, Sha256};

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn main() {
    let universe = mpi::initialize().expect("MPI init");
    let world = universe.world();
    let a: Vec<String> = std::env::args().collect();
    let (file, out) = (&a[1], std::path::Path::new(&a[2]));
    let batch: usize = a
        .get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(readcon_db::mpi_ingest::DEFAULT_BATCH_FRAMES);
    let traj_id: u64 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(7);
    let corpus = (world.rank() == 0).then(|| ConCorpus::open(out.join("mpi")).unwrap());
    world.barrier();
    let t = std::time::Instant::now();
    let n = readcon_db::mpi_ingest::append_trajectory_path(
        &world,
        corpus.as_ref(),
        traj_id,
        file,
        batch,
    )
    .unwrap();
    world.barrier();
    let dt = t.elapsed().as_secs_f64();
    if let Some(c) = corpus {
        let mpi_sha = hex(&Sha256::digest(c.canonical_dump().unwrap()));
        let s = ConCorpus::open(out.join("serial")).unwrap();
        let t0 = std::time::Instant::now();
        s.append_trajectory_path(traj_id, file).unwrap();
        let ds = t0.elapsed().as_secs_f64();
        let ser_sha = hex(&Sha256::digest(s.canonical_dump().unwrap()));
        println!(
            "ranks={} frames={n} mpi_s={dt:.3} serial_s={ds:.3} mpi_sha256={mpi_sha} serial_sha256={ser_sha} match={}",
            world.size(),
            mpi_sha == ser_sha
        );
    }
}
