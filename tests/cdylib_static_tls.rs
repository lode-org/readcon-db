//! Linux cdylibs must not request initial-exec TLS.
//!
//! `readelf -d` FLAGS must not include STATIC_TLS, and the dynamic
//! relocations must not include TPOFF (DTPMOD and DTPOFF are the
//! local-dynamic forms and are allowed). A dlopen after filler
//! libraries have spent the static TLS surplus must succeed.

#![cfg(target_os = "linux")]

use std::path::{Path, PathBuf};
use std::process::Command;

fn build_cdylib(target: &Path, features: &str) -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let status = Command::new(cargo)
        .args([
            "rustc",
            "--locked",
            "--release",
            "--lib",
            "--crate-type",
            "cdylib",
            "--features",
            features,
            "--target-dir",
        ])
        .arg(target)
        .arg("--manifest-path")
        .arg(manifest.join("Cargo.toml"))
        .status()
        .expect("cargo rustc");
    assert!(status.success(), "cdylib build failed for {features}");
    let so = target.join("release").join("libreadcon_db.so");
    assert!(so.is_file(), "missing {}", so.display());
    so
}

fn flags_line(dynamic: &str) -> String {
    dynamic
        .lines()
        .find(|line| line.contains("(FLAGS)") && !line.contains("FLAGS_"))
        .unwrap_or("")
        .to_string()
}

fn tpoff_lines(relocs: &str) -> Vec<String> {
    relocs
        .lines()
        .filter(|line| line.contains("TPOFF") && !line.contains("DTPOFF"))
        .map(str::to_string)
        .collect()
}

fn assert_no_static_tls(so: &Path) {
    let dynamic = Command::new("readelf")
        .args(["-d"])
        .arg(so)
        .output()
        .expect("readelf -d");
    assert!(
        dynamic.status.success(),
        "readelf -d failed\n{}",
        String::from_utf8_lossy(&dynamic.stderr)
    );
    let dynamic = String::from_utf8_lossy(&dynamic.stdout);
    let flags = flags_line(&dynamic);
    assert!(
        !flags.contains("STATIC_TLS"),
        "DF_STATIC_TLS still set:\n{flags}\n{dynamic}"
    );
    let relocs = Command::new("readelf")
        .args(["-W", "-r"])
        .arg(so)
        .output()
        .expect("readelf -r");
    assert!(
        relocs.status.success(),
        "readelf -r failed\n{}",
        String::from_utf8_lossy(&relocs.stderr)
    );
    let relocs = String::from_utf8_lossy(&relocs.stdout);
    let tpoff = tpoff_lines(&relocs);
    assert!(
        tpoff.is_empty(),
        "TPOFF relocations remain ({}):\n{}",
        tpoff.len(),
        tpoff.join("\n")
    );
}

fn assert_late_dlopen(so: &Path, symbol: &str) {
    let script = r#"
import ctypes, subprocess, sys
from pathlib import Path

so = Path(sys.argv[1])
work = Path(sys.argv[2])
symbol = sys.argv[3]
src = work / "filler.c"
src.write_text(
    "__thread char slot[32]; int bump(void) { slot[0] += 1; return slot[0]; }\n"
)
fillers = []
for i in range(64):
    path = work / f"filler-{i}.so"
    built = subprocess.run(
        ["gcc", "-shared", "-fPIC", "-ftls-model=initial-exec", "-o", str(path), str(src)],
        check=False,
    )
    if built.returncode != 0:
        raise SystemExit("filler compile failed")
    try:
        fillers.append(ctypes.CDLL(str(path)))
    except OSError as err:
        if "static TLS" not in str(err):
            raise
        break
else:
    raise SystemExit("64 initial-exec objects did not exhaust static TLS")
lib = ctypes.CDLL(str(so))
if symbol:
    fn = getattr(lib, symbol)
    fn.restype = ctypes.c_uint32
    got = fn()
    if got == 0:
        raise SystemExit("symbol returned 0")
    print(f"dlopen-ok fillers={len(fillers)} {symbol}={got}")
else:
    print(f"dlopen-ok fillers={len(fillers)}")
"#;
    let dir = tempfile::tempdir().expect("tempdir");
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(so)
        .arg(dir.path())
        .arg(symbol)
        .output()
        .expect("python");
    assert!(
        output.status.success(),
        "late dlopen failed for {}\nstdout:\n{}\nstderr:\n{}",
        so.display(),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn capi_and_python_cdylibs_have_no_static_tls() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/tls-cdylib");
    let capi_dir = tempfile::tempdir().expect("capi copy");
    let capi = build_cdylib(&root, "capi");
    let capi_copy = capi_dir.path().join("libreadcon_db.so");
    std::fs::copy(&capi, &capi_copy).expect("copy capi cdylib");
    assert_no_static_tls(&capi_copy);
    assert_late_dlopen(&capi_copy, "rkrdb_abi_version_major");

    let python = build_cdylib(&root, "python");
    assert_no_static_tls(&python);
    assert_late_dlopen(&python, "");
}
