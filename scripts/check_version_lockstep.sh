#!/usr/bin/env bash
# Structural gate: every language package reports the Cargo.toml version.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
fail=0

die() { echo "ERROR: $*" >&2; fail=1; }
ok() { echo "OK: $*"; }

cargo_ver="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT/Cargo.toml" | head -1)"
[[ -n "$cargo_ver" ]] || die "could not read Cargo.toml version"
ok "Cargo.toml $cargo_ver"

check_contains() {
  local rel="$1"
  local pat="$2"
  local f="$ROOT/$rel"
  [[ -f "$f" ]] || { die "missing $rel"; return; }
  core_revision=2fd79dddb948e22e4a74b2e85de98a3ae4c3d08b
if grep -qF 'version = "=0.14.10"' "$ROOT/Cargo.toml" &&
   grep -qF 'git = "https://github.com/lode-org/readcon-core"' "$ROOT/Cargo.toml" &&
   grep -qF "rev = \"$core_revision\"" "$ROOT/Cargo.toml"; then
  ok "Cargo.toml pins readcon-core 0.14.10 at $core_revision"
else
  die "Cargo.toml must pin the exact round-trip readcon-core revision"
fi

if [[ "$fail" -ne 0 ]]; then
  echo "check_version_lockstep: FAILED" >&2
  exit 1
fi
echo "check_version_lockstep: all checks passed"
