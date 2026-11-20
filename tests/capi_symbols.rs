//! The C ABI is compiled only with the `capi` feature. A default build must
//! not export unmangled `rkrdb_*` symbols, which clash when two readcon-db
//! versions link into one binary. This test builds the library both ways in a
//! nested cargo and reads the symbol tables of every artifact.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;

use object::read::archive::ArchiveFile;
use object::{Object, ObjectSymbol};

const EXPECTED_WITH_CAPI: &[&str] = &[
    "rkrdb_abi_version_major",
    "rkrdb_open",
    "rkrdb_close",
    "rkrdb_archive_open",
    "rkrdb_archive_close",
];

// One target dir per feature set. The cdylib and staticlib paths carry no
// feature hash, so builds with different features in a shared target dir
// overwrite each other while cargo still reports the other one as fresh.
fn target_dir(features: &[&str]) -> PathBuf {
    let name = if features.is_empty() {
        "default".to_string()
    } else {
        features.join("-")
    };
    Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("capi_symbols")
        .join(name)
}

fn artifacts(features: &[&str]) -> Vec<PathBuf> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let mut cmd = Command::new(cargo);
    cmd.args(["build", "--lib", "--locked", "--message-format=json"])
        .arg("--manifest-path")
        .arg(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
        .arg("--target-dir")
        .arg(target_dir(features));
    if !features.is_empty() {
        cmd.arg("--features").arg(features.join(","));
    }
    let out = cmd.output().expect("run cargo build");
    assert!(
        out.status.success(),
        "cargo build {features:?} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let mut files = Vec::new();
    for line in String::from_utf8(out.stdout).unwrap().lines() {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if v["reason"] != "compiler-artifact" || v["target"]["name"] != "readcon_db" {
            continue;
        }
        if !v["target"]["kind"]
            .as_array()
            .is_some_and(|k| k.iter().any(|k| k == "lib"))
        {
            continue;
        }
        for f in v["filenames"].as_array().unwrap() {
            let f = PathBuf::from(f.as_str().unwrap());
            let ext = f.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ["a", "rlib", "lib", "so", "dylib", "dll"].contains(&ext) {
                files.push(f);
            }
        }
    }
    files
}

fn defined_rkrdb(path: &Path) -> BTreeSet<String> {
    let data = std::fs::read(path).unwrap();
    let mut names = BTreeSet::new();
    let mut scan = |file: &object::File| {
        for sym in file.symbols().chain(file.dynamic_symbols()) {
            if !sym.is_definition() {
                continue;
            }
            if let Ok(name) = sym.name() {
                // Mach-O prefixes C symbols with an underscore.
                let bare = name.strip_prefix('_').unwrap_or(name);
                if bare.starts_with("rkrdb_") {
                    names.insert(bare.to_string());
                }
            }
        }
    };
    match path.extension().and_then(|e| e.to_str()) {
        Some("a") | Some("rlib") | Some("lib") => {
            let ar = ArchiveFile::parse(&*data).unwrap();
            for member in ar.members() {
                let member = member.unwrap();
                // rlibs also carry non-object members such as lib.rmeta.
                if let Ok(file) = object::File::parse(member.data(&*data).unwrap()) {
                    scan(&file);
                }
            }
        }
        _ => scan(&object::File::parse(&*data).unwrap()),
    }
    names
}

fn exported(features: &[&str]) -> BTreeSet<String> {
    let files = artifacts(features);
    assert!(
        files
            .iter()
            .any(|f| f.extension().is_some_and(|e| e == "rlib")),
        "no rlib artifact in {files:?}"
    );
    assert!(
        files.len() >= 3,
        "expected rlib, staticlib and cdylib artifacts, got {files:?}"
    );
    files.iter().flat_map(|f| defined_rkrdb(f)).collect()
}

#[test]
fn default_build_exports_no_rkrdb_symbols() {
    let got = exported(&[]);
    assert!(got.is_empty(), "default build exports C symbols: {got:?}");
}

#[test]
fn capi_build_exports_rkrdb_symbols() {
    let got = exported(&["capi"]);
    for want in EXPECTED_WITH_CAPI {
        assert!(got.contains(*want), "capi build is missing {want}: {got:?}");
    }
}
