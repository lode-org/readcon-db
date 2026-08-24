#!/usr/bin/env bash
# Structural + tarball gate: a prebuilt C ABI archive must ship headers,
# libreadcon_db, and readcon-db.pc. cbindgen is not required.
# Does not compile. Run from the repository root.
#
# Usage:
#   scripts/check-clib-dist.sh                  # source gate + fixture unpack
#   scripts/check-clib-dist.sh TARBALL...       # source gate + unpack each
#   scripts/check-clib-dist.sh --self-test      # fixture prefix -> package -> unpack
#   scripts/check-clib-dist.sh --no-self-test   # source gate only
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
fail=0
SELFTEST=0
TARBALLS=()

die() { echo "check-clib-dist: $*" >&2; fail=1; }

usage() {
    echo "usage: $0 [--self-test|--no-self-test] [TARBALL...]" >&2
    exit 2
}

if [[ $# -eq 0 ]]; then
    SELFTEST=1
fi
while [[ $# -gt 0 ]]; do
    case "$1" in
        --self-test) SELFTEST=1; shift ;;
        --no-self-test) SELFTEST=0; shift ;;
        -h|--help) usage ;;
        -*)
            echo "unknown argument: $1" >&2
            usage
            ;;
        *)
            TARBALLS+=("$1")
            shift
            ;;
    esac
done

# --- source-tree structural gate (peer of check-cxx-dist.sh) ---

for h in include/readcon-db.h include/readcon-db-mpi.h; do
    [[ -f "$h" ]] || die "missing shipped header $h"
done

[[ -x scripts/package-clib.sh ]] || die "scripts/package-clib.sh must be executable"
[[ -f .github/workflows/c_lib_tarball.yml ]] || die "missing .github/workflows/c_lib_tarball.yml"

grep -q 'generation = false' Cargo.toml || die "Cargo.toml capi.header.generation must stay false"
grep -q 'filename = "readcon-db"' Cargo.toml || die "cargo-c pkg-config filename must be readcon-db"

if grep -nE 'cbindgen[[:space:]]+REQUIRED|find_program[[:space:]]*\([[:space:]]*CBINDGEN' CMakeLists.txt; then
    die "CMakeLists.txt still requires cbindgen"
fi
if grep -nE "find_program\('cbindgen'|cbindgen_prog" meson.build; then
    die "meson.build still requires cbindgen"
fi

if grep -nE 'cargo install cbindgen|cbindgen@|uses: .*cbindgen' .github/workflows/c_lib_tarball.yml; then
    die "c_lib_tarball.yml must not install cbindgen"
fi
grep -q 'package-clib.sh' .github/workflows/c_lib_tarball.yml \
    || die "c_lib_tarball.yml must call scripts/package-clib.sh"
if ! grep -q 'workflow_dispatch:' .github/workflows/c_lib_tarball.yml \
    || ! grep -q 'tag:' .github/workflows/c_lib_tarball.yml; then
    die "c_lib_tarball.yml must support workflow_dispatch with a tag input"
fi

# --- unpack one tarball and assert the prefix layout ---

assert_unpacked() {
    local tarpath="$1"
    local unpack
    unpack="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "rm -rf '$unpack'" RETURN

    if [[ ! -f "$tarpath" ]]; then
        die "tarball not found: $tarpath"
        return
    fi

    if ! tar -xzf "$tarpath" -C "$unpack" 2>/dev/null; then
        die "$tarpath: not a gzip tarball"
        return
    fi
    local root
    root="$(find "$unpack" -mindepth 1 -maxdepth 1 -type d | head -1)"
    if [[ -z "$root" ]]; then
        die "$tarpath: empty archive"
        return
    fi

    [[ -f "$root/include/readcon-db.h" ]] \
        || die "$tarpath: missing include/readcon-db.h"
    [[ -f "$root/include/readcon-db-mpi.h" ]] \
        || die "$tarpath: missing include/readcon-db-mpi.h"

    local lib=""
    local candidate
    for candidate in \
        "$root/lib/libreadcon_db.so" \
        "$root/lib/libreadcon_db.dylib" \
        "$root/lib/libreadcon_db.dll" \
        "$root/lib/readcon_db.dll" \
        "$root/bin/readcon_db.dll"
    do
        if [[ -f "$candidate" ]]; then
            lib="$candidate"
            break
        fi
    done
    if [[ -z "$lib" ]]; then
        die "$tarpath: missing libreadcon_db.so or .dylib or .dll"
    fi

    local pc=""
    for candidate in \
        "$root/lib/pkgconfig/readcon-db.pc" \
        "$root/lib64/pkgconfig/readcon-db.pc" \
        "$root/share/pkgconfig/readcon-db.pc"
    do
        if [[ -f "$candidate" ]]; then
            pc="$candidate"
            break
        fi
    done
    if [[ -z "$pc" ]]; then
        die "$tarpath: missing lib/pkgconfig/readcon-db.pc (or Windows equivalent)"
    else
        grep -q 'Name: readcon-db' "$pc" \
            || die "$tarpath: pkg-config Name must be readcon-db"
        grep -q -- '-lreadcon_db' "$pc" \
            || die "$tarpath: pkg-config Libs must name -lreadcon_db"
    fi

    if find "$root" \( -name 'cbindgen' -o -name 'cbindgen.exe' -o -name 'cbindgen.toml' \) \
        | grep -q .; then
        die "$tarpath: must not ship cbindgen or cbindgen.toml"
    fi
}

# --- fixture prefix so the unpack path runs without cargo ---

if [[ "$SELFTEST" -eq 1 ]]; then
    FIX="$(mktemp -d)"
    OUT="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "rm -rf '$FIX' '$OUT'" EXIT
    mkdir -p "$FIX/include" "$FIX/lib/pkgconfig" "$FIX/bin"
    cp -a include/readcon-db.h include/readcon-db-mpi.h "$FIX/include/"
    printf 'stub-libreadcon_db\n' > "$FIX/lib/libreadcon_db.so"
    printf 'stub-readcon-db-cli\n' > "$FIX/bin/readcon-db"
    cat > "$FIX/lib/pkgconfig/readcon-db.pc" <<'EOF'
prefix=/usr
libdir=${prefix}/lib
includedir=${prefix}/include
Name: readcon-db
Description: Mmap-backed CON frame corpus (LMDB) with C/C++ FFI
Version: 0.0.0-fixture
Libs: -L${libdir} -lreadcon_db
Cflags: -I${includedir}
EOF
    bash scripts/package-clib.sh "$OUT" --from-prefix "$FIX" --target fixture
    local_tar="$(find "$OUT" -maxdepth 1 -name 'readcon-db-clib-*.tar.gz' | head -1)"
    if [[ -z "$local_tar" ]]; then
        die "package-clib.sh --from-prefix produced no tarball"
    else
        assert_unpacked "$local_tar"
    fi
fi

for tarpath in "${TARBALLS[@]+"${TARBALLS[@]}"}"; do
    assert_unpacked "$tarpath"
done

if [[ "$fail" -ne 0 ]]; then
    echo "check-clib-dist: FAILED" >&2
    exit 1
fi
echo "check-clib-dist: ok"
