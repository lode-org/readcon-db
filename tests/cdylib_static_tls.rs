//! The db cdylib must dlopen after static TLS surplus is spent.

use std::path::PathBuf;
use std::process::Command;

fn cdylib() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let target = manifest.join("target").join("tls-cdylib");
    let status = Command::new("cargo")
        .args([
            "rustc",
            "--locked",
            "--release",
            "--lib",
            "--crate-type",
            "cdylib",
            "--features",
            "local_dynamic_tls",
            "--target-dir",
        ])
        .arg(&target)
        .arg("--manifest-path")
        .arg(manifest.join("Cargo.toml"))
        .status()
        .expect("cargo rustc");
    assert!(status.success(), "cdylib build failed");
    let so = target.join("release").join("libreadcon_db.so");
    assert!(so.is_file(), "missing {}", so.display());
    so
}

#[cfg(target_os = "linux")]
#[test]
fn a_late_dlopen_has_no_initial_exec_tls() {
    let so = cdylib();
    let dynamic = Command::new("readelf")
        .args(["-d", so.to_str().unwrap()])
        .output()
        .expect("readelf -d");
    assert!(dynamic.status.success());
    let dynamic = String::from_utf8_lossy(&dynamic.stdout);
    assert!(
        !dynamic.contains("STATIC_TLS"),
        "DF_STATIC_TLS still set:\n{dynamic}"
    );
    let relocs = Command::new("readelf")
        .args(["-r", so.to_str().unwrap()])
        .output()
        .expect("readelf -r");
    assert!(relocs.status.success());
    let relocs = String::from_utf8_lossy(&relocs.stdout);
    assert!(
        !relocs.contains("TPOFF"),
        "initial-exec TLS relocation remains:\n{relocs}"
    );

    let script = r#"
import ctypes, hashlib, sqlite3, ssl, sys
from pathlib import Path

so = Path(sys.argv[1])
work = Path(sys.argv[2])
try:
    import numpy
except ImportError:
    numpy = None
src = work / "filler.c"
src.write_text("__thread char slot[65536]; int bump(void) { slot[0] += 1; return slot[0]; }\n")
import subprocess
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
lib.rkrdb_abi_version_major.restype = ctypes.c_uint32
got = lib.rkrdb_abi_version_major()
if got == 0:
    raise SystemExit("abi major is 0")
print(f"dlopen-ok fillers={len(fillers)} major={got} numpy={numpy is not None}")
"#;
    let dir = tempfile::tempdir().expect("tempdir");
    let output = Command::new("python3")
        .arg("-c")
        .arg(script)
        .arg(&so)
        .arg(dir.path())
        .output()
        .expect("python");
    assert!(
        output.status.success(),
        "late dlopen failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
