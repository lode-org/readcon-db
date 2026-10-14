//! The canonical dump is the determinism check for parallel ingest: equal
//! corpora give equal bytes, and any changed frame changes them.
use readcon_db::{ConCorpus, ShardedConCorpus};
use std::path::PathBuf;

fn fx(n: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("resources/test")
        .join(n)
}

fn build(dir: &std::path::Path, files: &[&str]) -> Vec<u8> {
    let c = ConCorpus::open(dir).unwrap();
    for (i, f) in files.iter().enumerate() {
        c.append_trajectory_path(i as u64 + 1, fx(f)).unwrap();
    }
    c.canonical_dump().unwrap()
}

#[test]
fn equal_corpora_equal_dumps() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let files = [
        "tiny_multi_cuh2.con",
        "sulfolene.con",
        "tiny_multi_cuh2.con",
    ];
    assert_eq!(build(a.path(), &files), build(b.path(), &files));
}

#[test]
fn different_corpora_different_dumps() {
    let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let x = build(a.path(), &["tiny_multi_cuh2.con", "sulfolene.con"]);
    let y = build(b.path(), &["sulfolene.con", "tiny_multi_cuh2.con"]);
    assert_ne!(x, y);
}

#[test]
fn sharded_dump_covers_every_shard() {
    let d = tempfile::tempdir().unwrap();
    let mut s = ShardedConCorpus::open(d.path(), 3).unwrap();
    for t in 1..=4u64 {
        s.append_trajectory_path(t, fx("tiny_cuh2.con")).unwrap();
    }
    let dump = s.canonical_dump().unwrap();
    drop(s);
    let one = ConCorpus::open(d.path().join("shard_0001"))
        .unwrap()
        .canonical_dump()
        .unwrap();
    assert!(dump.len() > 2 * one.len());
}
