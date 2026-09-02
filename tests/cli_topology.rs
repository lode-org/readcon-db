//! CLI topology commands. Parser/select tests live in-unit; these cover the binary.

use std::path::PathBuf;
use std::process::Command;

use readcon_db::{AnnotateTopologyOpts, ConCorpus, Select};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_readcon-db"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources/test")
        .join(name)
}

fn seams_or_skip() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("SEAMS") {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let cand = dir.join("seams");
            if cand.is_file() {
                return Some(cand);
            }
        }
    }
    eprintln!("skipping CLI topology seams test: seams fingerprint not on PATH or SEAMS");
    None
}

#[test]
fn annotate_without_seams_errors() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("c");
    {
        let db = ConCorpus::open(&corpus).unwrap();
        db.append_trajectory_path(1, fixture("tiny_cuh2.con"))
            .unwrap();
    }
    let out = bin()
        .args([
            "annotate-topology",
            corpus.to_str().unwrap(),
            "--cutoff",
            "3.0",
        ])
        .env_remove("SEAMS")
        .env("PATH", "/usr/bin")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("seams fingerprint"),
        "stderr should name seams fingerprint: {err}"
    );
}

#[test]
fn find_by_topology_without_seams_errors() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("c");
    {
        let db = ConCorpus::open(&corpus).unwrap();
        db.append_trajectory_path(1, fixture("tiny_cuh2.con"))
            .unwrap();
    }
    let out = bin()
        .args([
            "find-by-topology",
            corpus.to_str().unwrap(),
            fixture("tiny_cuh2.con").to_str().unwrap(),
        ])
        .env_remove("SEAMS")
        .env("PATH", "/usr/bin")
        .output()
        .unwrap();
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("seams fingerprint"),
        "stderr should name seams fingerprint: {err}"
    );
}

#[test]
fn cli_topo_key_lists_annotated_frame() {
    let Some(seams) = seams_or_skip() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("c");
    let hex = {
        let db = ConCorpus::open(&corpus).unwrap();
        db.append_trajectory_path(1, fixture("tiny_cuh2.con"))
            .unwrap();
        db.annotate_topology(&readcon_db::AnnotateTopologyOpts {
            cutoff: 3.0,
            graph: Some("cutoff".into()),
            hops: Some(2),
            seams: Some(seams),
        })
        .unwrap();
        let keys = db.select(&Select::new()).unwrap();
        db.frame_topo_key(keys[0]).unwrap().expect("hex")
    };
    let out = bin()
        .args(["select", corpus.to_str().unwrap(), "--topo-key", &hex])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("traj=1"), "{stdout}");
}
