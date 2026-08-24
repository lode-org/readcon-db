//! CLI dest/mint refuses. Integration test so `CARGO_BIN_EXE_readcon-db` is set.

use std::process::Command;

use readcon_db::ShardedConCorpus;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_readcon-db"))
}

#[test]
fn shard_select_missing_root_does_not_mint() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("nope");
    let st = bin()
        .args(["shard-select", root.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(!st.success());
    assert!(!root.join("shards.json").exists());
    assert!(!root.exists());
}

#[test]
fn compact_split_missing_src_does_not_mint() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("missing_src");
    let dest = dir.path().join("split_out");
    let st = bin()
        .args([
            "compact-split",
            src.to_str().unwrap(),
            dest.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(!st.success());
    assert!(!src.exists());
    assert!(!dest.exists());
}

#[test]
fn compact_export_refuses_sharded_without_flag() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("sharded");
    ShardedConCorpus::open(&root, 2).unwrap();
    let out = dir.path().join("out.xyz");
    let st = bin()
        .args([
            "compact-export-extxyz",
            root.to_str().unwrap(),
            out.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(!st.success());
    assert!(!root.join("data.mdb").exists());
    assert!(!out.exists());
}

#[test]
fn select_missing_path_does_not_mint() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("nope");
    let st = bin()
        .args(["select", corpus.to_str().unwrap(), "--symbol", "Cu"])
        .status()
        .unwrap();
    assert!(!st.success());
    assert!(!corpus.exists());
}

#[test]
fn cook_missing_path_does_not_mint() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("nope");
    let st = bin()
        .args([
            "cook-frame",
            corpus.to_str().unwrap(),
            "--traj",
            "1",
            "--frame",
            "0",
        ])
        .status()
        .unwrap();
    assert!(!st.success());
    assert!(!corpus.exists());
}

#[test]
fn compact_join_missing_src_does_not_mint() {
    let dir = tempfile::tempdir().unwrap();
    let src = dir.path().join("nope");
    let dest = dir.path().join("joined");
    let st = bin()
        .args([
            "compact-join",
            src.to_str().unwrap(),
            dest.to_str().unwrap(),
        ])
        .status()
        .unwrap();
    assert!(!st.success());
    assert!(!src.join("shards.json").exists());
    assert!(!src.exists());
    assert!(!dest.exists());
}

#[test]
fn reindex_missing_path_does_not_mint() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("nope");
    let st = bin()
        .args(["reindex", corpus.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(!st.success());
    assert!(!corpus.exists());
}

#[test]
fn recook_all_missing_path_does_not_mint() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("nope");
    let st = bin()
        .args(["recook-all", corpus.to_str().unwrap()])
        .status()
        .unwrap();
    assert!(!st.success());
    assert!(!corpus.exists());
}

#[test]
fn delete_cooked_missing_path_does_not_mint() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("nope");
    let st = bin()
        .args([
            "delete-cooked",
            corpus.to_str().unwrap(),
            "--traj",
            "1",
            "--frame",
            "0",
        ])
        .status()
        .unwrap();
    assert!(!st.success());
    assert!(!corpus.exists());
}

#[test]
fn positions_missing_path_does_not_mint() {
    let dir = tempfile::tempdir().unwrap();
    let corpus = dir.path().join("nope");
    let st = bin()
        .args([
            "positions",
            corpus.to_str().unwrap(),
            "--traj",
            "1",
            "--frame",
            "0",
        ])
        .status()
        .unwrap();
    assert!(!st.success());
    assert!(!corpus.exists());
}
