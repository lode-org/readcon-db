//! CLI topology commands. Parser/select/reindex tests live in the library.
//! Seams-backed cases skip at runtime when the engine is absent.

use std::path::{Path, PathBuf};
use std::process::Command;

use readcon_db::{AnnotateTopologyOpts, ConCorpus, FrameKey};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_readcon-db"))
}

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources/test")
        .join(name)
}

fn empty_path_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

fn run_without_seams(args: &[&str]) -> std::process::Output {
    let empty = empty_path_dir();
    bin()
        .args(args)
        .env_remove("SEAMS")
        .env("PATH", empty.path())
        .output()
        .expect("spawn")
}

#[test]
fn cli_annotate_without_seams_errors_clearly() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("c");
    {
        let db = ConCorpus::open(&corpus).unwrap();
        db.append_trajectory_path(1, fixture("tiny_cuh2.con"))
            .unwrap();
        db.close();
    }
    let out = run_without_seams(&[
        "annotate-topology",
        corpus.to_str().unwrap(),
        "--cutoff",
        "3.0",
    ]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("seams fingerprint"),
        "stderr must name the command: {err}"
    );
}

#[test]
fn cli_find_by_topology_without_params_errors_clearly() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("c");
    {
        let db = ConCorpus::open(&corpus).unwrap();
        db.append_trajectory_path(1, fixture("tiny_cuh2.con"))
            .unwrap();
        db.close();
    }
    let out = run_without_seams(&[
        "find-by-topology",
        corpus.to_str().unwrap(),
        fixture("tiny_cuh2.con").to_str().unwrap(),
    ]);
    assert!(!out.status.success());
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(
        err.contains("annotate-topology") || err.contains("seams fingerprint"),
        "stderr must be actionable: {err}"
    );
}

#[test]
fn cli_annotate_missing_corpus_does_not_mint() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("nope");
    let out = run_without_seams(&[
        "annotate-topology",
        corpus.to_str().unwrap(),
        "--cutoff",
        "3.0",
    ]);
    assert!(!out.status.success());
    assert!(!corpus.exists());
}

fn seams_or_skip() -> Option<PathBuf> {
    match which_seams() {
        Some(p) => Some(p),
        None => {
            eprintln!(
                "skipping topology CLI seams test: seams fingerprint binary not found on PATH/SEAMS"
            );
            None
        }
    }
}

fn which_seams() -> Option<PathBuf> {
    if let Ok(s) = std::env::var("SEAMS") {
        let p = PathBuf::from(&s);
        if p.is_file() {
            return Some(p);
        }
    }
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let cand = dir.join("seams");
        if cand.is_file() {
            return Some(cand);
        }
    }
    None
}

fn permute_cu_file(src: &Path, dest: &Path) {
    let text = std::fs::read_to_string(src).unwrap();
    let mut lines: Vec<&str> = text.lines().collect();
    let mut cu = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        if t.starts_with("0.6394") || t.starts_with("3.1969") {
            cu.push(i);
        }
    }
    assert_eq!(cu.len(), 2);
    lines.swap(cu[0], cu[1]);
    let mut out = lines.join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    std::fs::write(dest, out).unwrap();
}

#[test]
fn cli_topo_key_and_find_by_topology_with_seams() {
    let Some(seams) = seams_or_skip() else {
        return;
    };
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("c");
    let hex = {
        let db = ConCorpus::open(&corpus).unwrap();
        db.append_trajectory_path(1, fixture("tiny_cuh2.con"))
            .unwrap();
        db.annotate_topology(AnnotateTopologyOpts::new(3.0).seams(&seams))
            .unwrap();
        let hex = db
            .frame_topo_key(FrameKey {
                traj_id: 1,
                frame_idx: 0,
            })
            .unwrap()
            .expect("hex");
        db.close();
        hex
    };
    let out = bin()
        .args([
            "select",
            corpus.to_str().unwrap(),
            "--topo-key",
            &hex,
        ])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("traj=1"), "{stdout}");

    let perm = dir.path().join("perm.con");
    permute_cu_file(&fixture("tiny_cuh2.con"), &perm);
    let found = bin()
        .env("SEAMS", &seams)
        .args([
            "find-by-topology",
            corpus.to_str().unwrap(),
            perm.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(found.status.success(), "{:?}", found);
    let fo = String::from_utf8_lossy(&found.stdout);
    assert!(fo.contains("traj=1"), "{fo}");
}
