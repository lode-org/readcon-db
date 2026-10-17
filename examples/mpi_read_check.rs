//! `mpirun -n P mpi_read_check <outdir> <file.con>...`
//!
//! Rank 0 ingests the files serially. Then every rank checks that its
//! scattered RCSO block equals a local `pack_frames` of the same keys, and
//! rank 0 compares the sha256 of both collective XYZ exports with the
//! serial `export_extxyz`.
use std::path::PathBuf;

use mpi::collective::SystemOperation;
use mpi::traits::*;
use readcon_db::mpi_read::{export_extxyz_collective, scatter_packed_frames, WriteMode};
use readcon_db::{ConCorpus, Select};
use sha2::{Digest, Sha256};

fn sha(p: &std::path::Path) -> String {
    Sha256::digest(std::fs::read(p).unwrap())
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

fn main() {
    let universe = mpi::initialize().expect("MPI init");
    let world = universe.world();
    let a: Vec<String> = std::env::args().collect();
    let out = PathBuf::from(&a[1]);
    let db = out.join("db");
    let mut keys = Vec::new();
    if world.rank() == 0 {
        let c = ConCorpus::open(&db).unwrap();
        for (i, f) in a[2..].iter().enumerate() {
            c.append_trajectory_path(i as u64 + 1, f).unwrap();
        }
        keys = c.select(&Select::new()).unwrap();
        c.export_extxyz(&keys, out.join("serial.xyz"), "energy")
            .unwrap();
    }
    world.barrier();
    let (range, blob) = scatter_packed_frames(&world, &db, &keys).unwrap();
    let all = ConCorpus::open_readonly(&db)
        .unwrap()
        .select(&Select::new())
        .unwrap();
    let local = ConCorpus::open_readonly(&db)
        .unwrap()
        .pack_frames(&all[range])
        .unwrap();
    let mut ok = 0i32;
    world.all_reduce_into(&i32::from(local == blob), &mut ok, SystemOperation::min());
    for (m, name) in [
        (WriteMode::MpiIo, "mpiio.xyz"),
        (WriteMode::Gather, "gather.xyz"),
    ] {
        export_extxyz_collective(&world, &db, &keys, out.join(name), "energy", m).unwrap();
    }
    if world.rank() == 0 {
        let s = sha(&out.join("serial.xyz"));
        let (m, g) = (sha(&out.join("mpiio.xyz")), sha(&out.join("gather.xyz")));
        println!(
            "ranks={} frames={} scatter_match={} serial_sha256={s} mpiio_match={} gather_match={}",
            world.size(),
            keys.len(),
            ok == 1,
            m == s,
            g == s
        );
    }
}
