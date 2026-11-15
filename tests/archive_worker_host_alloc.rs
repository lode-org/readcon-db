//! Linking this crate's static archive beside readcon-core's must not
//! install mimalloc. The archive worker then frees with the host allocator.

use std::path::{Path, PathBuf};
use std::process::Command;

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn defined_mimalloc(archive: &Path) -> String {
    let output = Command::new("nm")
        .args(["--defined-only"])
        .arg(archive)
        .output()
        .expect("nm");
    assert!(output.status.success(), "nm failed on {}", archive.display());
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| {
            let mut parts = line.split_whitespace();
            let _addr = parts.next();
            let kind = parts.next().unwrap_or("");
            let name = parts.next().unwrap_or("");
            kind == "T" && (name == "mi_malloc" || name == "mi_free")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn staticlib(
    manifest: &Path,
    target: &Path,
    features: &str,
    archive: &str,
    patch_core: bool,
) -> PathBuf {
    let mut cmd = Command::new("cargo");
    cmd.args([
        "rustc",
        "--release",
        "--lib",
        "--crate-type",
        "staticlib",
        "--target-dir",
    ])
    .arg(target)
    .arg("--manifest-path")
    .arg(manifest);
    if !features.is_empty() {
        cmd.args(["--features", features]);
    }
    if patch_core {
        if let Ok(core) = std::env::var("READCON_CORE_SRC") {
            cmd.args([
                "--config",
                &format!("patch.crates-io.readcon-core.path=\"{core}\""),
            ]);
        }
    }
    // Match scripts/meson_cargo_build.py: static archives leave LTO off
    // so a host can link more than one of them.
    cmd.env("CARGO_PROFILE_RELEASE_LTO", "off");
    let status = cmd.status().expect("cargo rustc");
    assert!(
        status.success(),
        "staticlib build failed for {}",
        manifest.display()
    );
    let built = target.join("release").join(archive);
    assert!(built.is_file(), "missing {}", built.display());
    built
}

#[cfg(target_os = "linux")]
#[test]
fn archive_worker_frees_with_the_host_allocator() {
    let root = manifest_dir();
    let work = root.join("target").join("archive-worker-host-alloc");
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).expect("workdir");

    let core_src = std::env::var("READCON_CORE_SRC").unwrap_or_else(|_| {
        panic!("READCON_CORE_SRC must point at the readcon-core checkout")
    });
    // The staticlib feature is what scripts/meson_cargo_build.py passes.
    // Before that change these strings are empty and mimalloc is installed.
    let core_features = "static_system_alloc";
    let db_features = "readcon-core/static_system_alloc";
    let core = staticlib(
        &PathBuf::from(core_src).join("Cargo.toml"),
        &work.join("core"),
        core_features,
        "libreadcon_core.a",
        false,
    );
    let db = staticlib(
        &root.join("Cargo.toml"),
        &work.join("db"),
        db_features,
        "libreadcon_db.a",
        true,
    );
    for archive in [&core, &db] {
        let hits = defined_mimalloc(archive);
        assert!(
            hits.is_empty(),
            "{} still installs mimalloc:\n{hits}",
            archive.display()
        );
    }

    let driver = work.join("archive_worker_host_alloc");
    let compile = Command::new("cc")
        .arg("-o")
        .arg(&driver)
        .arg(root.join("tests/archive_worker_host_alloc.c"))
        .arg("-I")
        .arg(root.join("include"))
        .arg("-Wl,--allow-multiple-definition")
        .arg("-Wl,--start-group")
        .arg(&db)
        .arg(&core)
        .arg("-Wl,--end-group")
        .args(["-lpthread", "-ldl", "-lm", "-lgcc_s"])
        .status()
        .expect("cc");
    assert!(compile.success(), "link failed");

    let archive = work.join("archive");
    let run = Command::new(&driver)
        .env("ARCHIVE_DIR", &archive)
        .status()
        .expect("run");
    assert!(
        run.success(),
        "archive worker did not free with the host allocator: {run}"
    );
}
