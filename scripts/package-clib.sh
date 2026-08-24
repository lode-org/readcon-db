#!/usr/bin/env bash
# Assemble a prebuilt C ABI tarball: shipped headers + libreadcon_db +
# readcon-db.pc. cbindgen is not invoked.
#
# Layout:
#   readcon-db-clib-$VERSION-$TARGET/
#     include/readcon-db.h include/readcon-db-mpi.h
#     lib/libreadcon_db.{so,dylib} | bin/readcon_db.dll + lib/readcon_db.dll.lib
#     lib/pkgconfig/readcon-db.pc
#     bin/readcon-db            (CLI, when the cargo/prefix build produced it)
#     LICENSE README.clib.md
#
# Usage:
#   scripts/package-clib.sh <output-dir> [--root DIR] [--target TRIPLE]
#                           [--features FEATS] [--prefix DIR] [--no-build]
#
# --root is the crate tree (Cargo.toml + include/). Default: this repo.
# --target names the archive; cargo --target is used only when it differs
# from the host triple. --prefix skips cargo and copies an existing install.
# --no-build refuses to invoke cargo (uses --prefix or $root/target/release).
set -euo pipefail

usage() {
    echo "usage: $0 OUTPUT_DIR [--root DIR] [--target TRIPLE] [--features FEATS] [--prefix DIR] [--no-build]" >&2
    exit 2
}

if [[ $# -lt 1 ]]; then
    usage
fi

OUTPUT_DIR="$1"
shift

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
TARGET=""
FEATURES=""
PREFIX=""
NO_BUILD=0

while [[ $# -gt 0 ]]; do
    case "$1" in
        --root)
            [[ $# -ge 2 ]] || usage
            ROOT_DIR="$(cd "$2" && pwd)"
            shift 2
            ;;
        --target)
            [[ $# -ge 2 ]] || usage
            TARGET="$2"
            shift 2
            ;;
        --features)
            [[ $# -ge 2 ]] || usage
            FEATURES="$2"
            shift 2
            ;;
        --prefix|--from-prefix)
            [[ $# -ge 2 ]] || usage
            PREFIX="$(cd "$2" && pwd)"
            shift 2
            ;;
        --no-build)
            NO_BUILD=1
            shift
            ;;
        *)
            usage
            ;;
    esac
done

mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"

if [[ ! -f "$ROOT_DIR/Cargo.toml" ]]; then
    echo "package-clib: no Cargo.toml under $ROOT_DIR" >&2
    exit 1
fi

VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -1)"
if [[ -z "$VERSION" ]]; then
    echo "package-clib: could not parse version from $ROOT_DIR/Cargo.toml" >&2
    exit 1
fi

detect_host() {
    if command -v rustc >/dev/null 2>&1; then
        rustc -vV | sed -n 's/^host: //p'
        return 0
    fi
    local sys mach
    sys="$(uname -s)"
    mach="$(uname -m)"
    case "$sys:$mach" in
        Linux:x86_64) echo x86_64-unknown-linux-gnu ;;
        Linux:aarch64|Linux:arm64) echo aarch64-unknown-linux-gnu ;;
        Darwin:x86_64) echo x86_64-apple-darwin ;;
        Darwin:arm64) echo aarch64-apple-darwin ;;
        MINGW*|MSYS*|CYGWIN*:*|Windows_NT:*) echo x86_64-pc-windows-msvc ;;
        *)
            echo "package-clib: cannot detect host triple; pass --target" >&2
            return 1
            ;;
    esac
}

HOST="$(detect_host)"
if [[ -z "$TARGET" ]]; then
    TARGET="$HOST"
fi

ARCHIVE_NAME="readcon-db-clib-${VERSION}-${TARGET}"
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

DEST="${TMP_DIR}/${ARCHIVE_NAME}"
mkdir -p "$DEST"/{include,lib/pkgconfig}

# Shipped headers: never generate, never ship cbindgen.toml.
for h in readcon-db.h readcon-db-mpi.h; do
    if [[ ! -f "$ROOT_DIR/include/$h" ]]; then
        echo "package-clib: missing shipped header include/$h" >&2
        exit 1
    fi
    cp -a "$ROOT_DIR/include/$h" "$DEST/include/"
done
cp -a "$ROOT_DIR/LICENSE" "$DEST/"

copy_cli_from() {
    local src="$1"
    if [[ -f "$src/bin/readcon-db" ]]; then
        mkdir -p "$DEST/bin"
        cp -a "$src/bin/readcon-db" "$DEST/bin/"
    elif [[ -f "$src/bin/readcon-db.exe" ]]; then
        mkdir -p "$DEST/bin"
        cp -a "$src/bin/readcon-db.exe" "$DEST/bin/"
    elif [[ -x "$src/readcon-db" ]]; then
        mkdir -p "$DEST/bin"
        cp -a "$src/readcon-db" "$DEST/bin/"
    elif [[ -f "$src/readcon-db.exe" ]]; then
        mkdir -p "$DEST/bin"
        cp -a "$src/readcon-db.exe" "$DEST/bin/"
    fi
}

copy_shared_from() {
    local src="$1"
    local copied=0
    if [[ -f "$src/lib/libreadcon_db.so" ]]; then
        cp -a "$src/lib/libreadcon_db.so" "$DEST/lib/"
        copied=1
    fi
    if [[ -f "$src/lib/libreadcon_db.dylib" ]]; then
        cp -a "$src/lib/libreadcon_db.dylib" "$DEST/lib/"
        copied=1
    fi
    if [[ -f "$src/lib/libreadcon_db.a" ]]; then
        cp -a "$src/lib/libreadcon_db.a" "$DEST/lib/"
    fi
    if [[ -f "$src/bin/readcon_db.dll" ]]; then
        mkdir -p "$DEST/bin"
        cp -a "$src/bin/readcon_db.dll" "$DEST/bin/"
        copied=1
    fi
    if [[ -f "$src/lib/readcon_db.dll.lib" ]]; then
        cp -a "$src/lib/readcon_db.dll.lib" "$DEST/lib/"
        copied=1
    fi
    if [[ -f "$src/lib/pkgconfig/readcon-db.pc" ]]; then
        cp -a "$src/lib/pkgconfig/readcon-db.pc" "$DEST/lib/pkgconfig/"
    fi
    copy_cli_from "$src"
    [[ "$copied" -eq 1 ]]
}

copy_shared_from_cargo() {
    local src="$1"
    local copied=0
    if [[ -f "$src/libreadcon_db.so" ]]; then
        cp -a "$src/libreadcon_db.so" "$DEST/lib/"
        copied=1
    fi
    if [[ -f "$src/libreadcon_db.dylib" ]]; then
        cp -a "$src/libreadcon_db.dylib" "$DEST/lib/"
        copied=1
    fi
    if [[ -f "$src/libreadcon_db.a" ]]; then
        cp -a "$src/libreadcon_db.a" "$DEST/lib/"
    fi
    if [[ -f "$src/readcon_db.dll" ]]; then
        mkdir -p "$DEST/bin"
        cp -a "$src/readcon_db.dll" "$DEST/bin/"
        copied=1
    fi
    if [[ -f "$src/readcon_db.dll.lib" ]]; then
        cp -a "$src/readcon_db.dll.lib" "$DEST/lib/"
        copied=1
    elif [[ -f "$src/readcon_db.lib" ]]; then
        cp -a "$src/readcon_db.lib" "$DEST/lib/readcon_db.dll.lib"
        copied=1
    fi
    copy_cli_from "$src"
    [[ "$copied" -eq 1 ]]
}

if [[ -n "$PREFIX" ]]; then
    copy_shared_from "$PREFIX" || {
        echo "package-clib: no libreadcon_db under --prefix $PREFIX" >&2
        exit 1
    }
else
    if [[ "$TARGET" == "$HOST" ]]; then
        LIB_DIR="$ROOT_DIR/target/release"
        CARGO_TARGET_ARGS=()
    else
        LIB_DIR="$ROOT_DIR/target/${TARGET}/release"
        CARGO_TARGET_ARGS=(--target "$TARGET")
    fi

    if [[ "$NO_BUILD" -eq 0 ]]; then
        if ! command -v cargo >/dev/null 2>&1; then
            echo "package-clib: cargo is required unless --prefix or --no-build is set" >&2
            exit 1
        fi
        (
            cd "$ROOT_DIR"
            feat_args=()
            if [[ -n "$FEATURES" ]]; then
                feat_args=(--features "$FEATURES")
            fi
            cargo build --release --locked --package readcon-db \
                ${CARGO_TARGET_ARGS[@]+"${CARGO_TARGET_ARGS[@]}"} \
                ${feat_args[@]+"${feat_args[@]}"}
        )
    fi

    copy_shared_from_cargo "$LIB_DIR" || {
        echo "package-clib: no libreadcon_db in $LIB_DIR (build first or pass --prefix)" >&2
        exit 1
    }
fi

if [[ ! -f "$DEST/lib/pkgconfig/readcon-db.pc" ]]; then
    PRIVATE=""
    case "$TARGET" in
        *linux*) PRIVATE="-ldl -lpthread -lm" ;;
        *apple*) PRIVATE="-lresolv" ;;
        *windows*) PRIVATE="" ;;
    esac
    cat > "$DEST/lib/pkgconfig/readcon-db.pc" <<EOF
prefix=\${pcfiledir}/../..
exec_prefix=\${prefix}
libdir=\${prefix}/lib
includedir=\${prefix}/include

Name: readcon-db
Description: Mmap-backed CON frame corpus (LMDB) with C/C++ FFI
Version: ${VERSION}
URL: https://github.com/lode-org/readcon-db
Libs: -L\${libdir} -lreadcon_db
Libs.private: ${PRIVATE}
Cflags: -I\${includedir}
EOF
fi

if find "$DEST" -name 'cbindgen.toml' -o -name 'cbindgen' | grep -q .; then
    echo "package-clib: tarball must not contain cbindgen" >&2
    exit 1
fi

cat > "$DEST/README.clib.md" <<EOF
# readcon-db ${VERSION} (prebuilt C ABI, ${TARGET})

Shared library plus shipped headers and pkg-config. **cbindgen is not
required** and must not be invoked to consume this tarball.

This archive is the Fortran / C consumer path (no local cargo). CMake
FetchContent and Meson wrap still use the *source* tarball
\`readcon-db-cxx-${VERSION}.tar.gz\`.

\`libreadcon_db\` already contains the Rust readcon-core and rust-std.
A process that also uses the readcon-core C API must link the **shared**
objects (\`libreadcon_db.so\` and \`libreadcon_core.so\`). Do not
static-link \`libreadcon_db.a\` together with \`libreadcon_core.a\`.

## Unpack

\`\`\`bash
tar -xzf readcon-db-clib-${VERSION}-${TARGET}.tar.gz
cd readcon-db-clib-${VERSION}-${TARGET}
export PKG_CONFIG_PATH="\$PWD/lib/pkgconfig:\${PKG_CONFIG_PATH:-}"
export LD_LIBRARY_PATH="\$PWD/lib:\${LD_LIBRARY_PATH:-}"   # Linux
export DYLD_LIBRARY_PATH="\$PWD/lib:\${DYLD_LIBRARY_PATH:-}" # macOS
export READCON_DB_LIB="\$PWD/lib/libreadcon_db.so"         # .dylib on macOS
pkg-config --cflags --libs readcon-db
\`\`\`

The CLI (\`bin/readcon-db\`) is included when the build produced it.
Campaign ops (shard-init / shard-ingest / drain / join-drained) use that
binary; see \`docs/source/campaign.md\`.

## Fortran (fpm)

\`\`\`bash
export PKG_CONFIG_PATH="\$PWD/lib/pkgconfig:\${PKG_CONFIG_PATH:-}"
export LD_LIBRARY_PATH="\$PWD/lib:\${LD_LIBRARY_PATH:-}"
cd fortran/ReadConDb
fpm build --flag "\$(pkg-config --cflags readcon-db)" \\
  --link-flag "\$(pkg-config --libs readcon-db) -ldl -lpthread -lm"
\`\`\`

\`fpm.toml\` already lists \`link = ["readcon_db"]\`. Point the linker at
this prefix instead of a local \`target/release\`.
EOF

tar -C "$TMP_DIR" -cf "${TMP_DIR}/${ARCHIVE_NAME}.tar" "$ARCHIVE_NAME"
gzip -9 "${TMP_DIR}/${ARCHIVE_NAME}.tar"
cp "${TMP_DIR}/${ARCHIVE_NAME}.tar.gz" "${OUTPUT_DIR}/${ARCHIVE_NAME}.tar.gz"

SHA="$(sha256sum "${OUTPUT_DIR}/${ARCHIVE_NAME}.tar.gz" | awk '{print $1}')"
echo "${OUTPUT_DIR}/${ARCHIVE_NAME}.tar.gz"
echo "sha256:${SHA}"
echo "${SHA}" > "${OUTPUT_DIR}/${ARCHIVE_NAME}.tar.gz.sha256"
