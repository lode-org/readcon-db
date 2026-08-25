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
  if grep -qE "$pat" "$f"; then
    ok "$rel matches $cargo_ver"
  else
    die "$rel does not contain version $cargo_ver (pattern $pat)"
  fi
}

check_contains "python/pyproject.toml" "version = \"${cargo_ver}\""
check_contains "meson.build" "version: '${cargo_ver}'"
check_contains "pixi.toml" "version = \"${cargo_ver}\""
check_contains "fortran/ReadConDb/fpm.toml" "^version = \"${cargo_ver}\""
check_contains "CITATION.cff" "^version: ${cargo_ver}$"
check_contains "docs/source/conf.py" "release = \"${cargo_ver}\""

if grep -qE '^readcon-core = "=0\.14\.8"$' "$ROOT/Cargo.toml"; then
  ok "Cargo.toml pins readcon-core =0.14.8"
else
  die "Cargo.toml must pin readcon-core = \"=0.14.8\""
fi

if [[ "$fail" -ne 0 ]]; then
  echo "check_version_lockstep: FAILED" >&2
  exit 1
fi
echo "check_version_lockstep: all checks passed"
