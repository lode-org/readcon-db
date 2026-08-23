#!/usr/bin/env bash
# Structural gate: C/C++ consumers must never be told to run cbindgen.
# Does not compile. Run from the repository root.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
fail=0

die() { echo "check-cxx-dist: $*" >&2; fail=1; }

# Shipped headers exist
for h in include/readcon-db.h include/readcon-db-mpi.h; do
    [[ -f "$h" ]] || die "missing shipped header $h"
done

# MPI helper: caller comm only. Never Init; never name the process-wide world handle.
if grep -vE '^[[:space:]]*(\*|//|/\*)' include/readcon-db-mpi.h \
    | grep -nE 'MPI_COMM_WORLD|MPI_Init[[:space:]]*\('; then
    die "readcon-db-mpi.h must not call MPI_Init or name MPI_COMM_WORLD"
fi

# CMake must not require cbindgen or Corrosion
if grep -nE 'find_program[[:space:]]*\([[:space:]]*CBINDGEN|cbindgen[[:space:]]+REQUIRED|Corrosion' CMakeLists.txt; then
    die "CMakeLists.txt still requires cbindgen or Corrosion"
fi
grep -q 'readcon-db.h' CMakeLists.txt || die "CMakeLists.txt does not reference the shipped C header"
grep -q 'readcon-db-mpi.h' CMakeLists.txt || die "CMakeLists.txt does not reference the MPI helper header"
grep -q 'Name: readcon-db' cmake/readcon-db.pc.in || die "missing pkg-config template name"
grep -q 'readcon-db::shared' CMakeLists.txt || die "CMakeLists.txt missing readcon-db::shared"
grep -q 'FetchContent' cmake/readcon-db-config.in.cmake && die "installed cmake config must not FetchContent"

# Meson must not require cbindgen
if grep -nE "find_program\('cbindgen'|cbindgen_prog" meson.build; then
    die "meson.build still requires cbindgen"
fi
grep -q "filebase: 'readcon-db'" meson.build || die "meson pkg-config filebase must be readcon-db"
grep -q "meson.override_dependency('readcon-db'" meson.build || die "meson.build must override_dependency('readcon-db')"
if grep -nE "filebase: 'meson-readcon-db'|version: f'@pkg_ver@_meson'" meson.build; then
    die "meson.build still emits the non-standard meson-readcon-db.pc"
fi

# cargo-c metadata if present
if grep -q 'package.metadata.capi' Cargo.toml; then
    grep -q 'generation = false' Cargo.toml || die "Cargo.toml capi.header.generation must stay false"
    grep -q 'filename = "readcon-db"' Cargo.toml || die "cargo-c pkg-config filename must be readcon-db"
fi

# Tarball assembler exists
[[ -x scripts/package-cxx.sh ]] || die "scripts/package-cxx.sh must be executable"
[[ -f scripts/meson_cargo_build.py ]] || die "missing scripts/meson_cargo_build.py"

# CMake version is not hardcoded to a stale release
if grep -nE 'project\(readcon-db VERSION 0\.13' CMakeLists.txt; then
    die "CMakeLists.txt still hardcodes a stale project version"
fi

if [[ "$fail" -ne 0 ]]; then
    echo "check-cxx-dist: FAILED" >&2
    exit 1
fi
echo "check-cxx-dist: ok"
