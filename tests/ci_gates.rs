//! Structural gates for language-package version lockstep.

use std::path::PathBuf;
use std::process::Command;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run_gate(rel: &str) {
    let script = repo_root().join(rel);
    assert!(script.is_file(), "missing {}", script.display());
    let status = Command::new("bash")
        .arg(&script)
        .current_dir(repo_root())
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn {}: {e}", script.display()));
    assert!(
        status.success(),
        "{rel} failed (exit {:?})",
        status.code()
    );
}

#[test]
fn language_package_versions_match_cargo() {
    run_gate("scripts/check_version_lockstep.sh");
}
