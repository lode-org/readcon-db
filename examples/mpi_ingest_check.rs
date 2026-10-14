//! `mpirun -n P mpi_ingest_check <file.con> <outdir>`: ingest with P ranks
//! into `<outdir>/mpi`, ingest serially into `<outdir>/serial` (rank 0),
//! and print the sha256 of both canonical dumps plus the ingest wall time.
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
    let corpus = (world.rank() == 0).then(|| ConCorpus::open(out.join("mpi")).unwrap());
    world.barrier();
    let t = std::time::Instant::now();
    let n = readcon_db::mpi_ingest::append_trajectory_path(&world, corpus.as_ref(), 7, file, batch)
        .unwrap();
    world.barrier();
    let dt = t.elapsed().as_secs_f64();
    if let Some(c) = corpus {
        let mpi_sha = hex(&Sha256::digest(c.canonical_dump().unwrap()));
        let s = ConCorpus::open(out.join("serial")).unwrap();
        let t0 = std::time::Instant::now();
        s.append_trajectory_path(7, file).unwrap();
        let ds = t0.elapsed().as_secs_f64();
        let ser_sha = hex(&Sha256::digest(s.canonical_dump().unwrap()));
        println!("ranks={} frames={n} mpi_s={dt:.3} serial_s={ds:.3} mpi_sha256={mpi_sha} serial_sha256={ser_sha} match={}", world.size(), mpi_sha == ser_sha);
    }
}
