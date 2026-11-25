//! CON major-4 files, written by an unreleased readcon-core (see
//! resources/test/con_v4/README.md). The pinned readcon-core reads
//! majors up to 3, so ingest refuses them and stores nothing. A pin that
//! reads major 4 fails these tests, and they then become round-trip tests.

use std::path::PathBuf;

use readcon_core::error::ParseError;
use readcon_core::iterators::ConFrameIterator;
use readcon_db::{ConCorpus, Error};

const FIXTURES: [&str; 2] = ["tiny_cuh2_v4.con", "tiny_cuh2_v4.convel"];

fn dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/test/con_v4")
}

fn assert_unsupported_v4(err: Error) {
    match err {
        Error::Parse(msg) => assert_eq!(msg, ParseError::UnsupportedVersion(4).to_string()),
        other => panic!("expected the UnsupportedVersion parse error, got {other:?}"),
    }
}

#[test]
fn pinned_parser_rejects_major_4() {
    assert_eq!(readcon_core::CON_SPEC_VERSION, 3);
    for name in FIXTURES {
        let text = std::fs::read_to_string(dir().join(name)).unwrap();
        assert!(text.contains("\"con_spec_version\":4"), "{name}");
        let first = ConFrameIterator::new(&text).next();
        assert!(
            matches!(first, Some(Err(ParseError::UnsupportedVersion(4)))),
            "{name}: {first:?}"
        );
    }
}

#[test]
fn append_path_rejects_major_4_and_stores_nothing() {
    for (traj, name) in (1..).zip(FIXTURES) {
        let tmp = tempfile::tempdir().unwrap();
        let db = ConCorpus::open(tmp.path().join("c")).unwrap();
        let err = db
            .append_trajectory_path(traj, dir().join(name))
            .unwrap_err();
        assert_eq!(
            err.to_string(),
            "parse frame: unsupported con_spec_version: 4"
        );
        assert_unsupported_v4(err);
        assert!(db.traj_meta(traj).unwrap().is_none(), "{name}");
    }
}

#[test]
fn ingest_directory_rejects_major_4_and_stores_nothing() {
    let tmp = tempfile::tempdir().unwrap();
    let db = ConCorpus::open(tmp.path().join("c")).unwrap();
    assert_unsupported_v4(db.ingest_directory(dir(), 1).unwrap_err());
    for traj in 1..=FIXTURES.len() as u64 {
        assert!(db.traj_meta(traj).unwrap().is_none());
    }
}
