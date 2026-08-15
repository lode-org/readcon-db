#!/usr/bin/env bash
# Assemble a C/C++ source tarball that CMake FetchContent and Meson wrap
# can consume without cbindgen, Corrosion, or git.
#
# Usage:
#   scripts/package-cxx.sh <output-dir> [--vendor]
set -euo pipefail

if [[ $# -lt 1 ]]; then
    echo "usage: $0 OUTPUT_DIR [--vendor]" >&2
    exit 2
fi

OUTPUT_DIR="$1"
shift
VENDOR=0
if [[ "${1:-}" == "--vendor" ]]; then
    VENDOR=1
fi

mkdir -p "$OUTPUT_DIR"
OUTPUT_DIR="$(cd "$OUTPUT_DIR" && pwd)"
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

VERSION="$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$ROOT_DIR/Cargo.toml" | head -1)"
if [[ "$VENDOR" -eq 1 ]]; then
    ARCHIVE_NAME="readcon-db-cxx-${VERSION}-vendor"
else
    ARCHIVE_NAME="readcon-db-cxx-${VERSION}"
fi

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

DEST="${TMP_DIR}/${ARCHIVE_NAME}"
mkdir -p "$DEST"/{cmake,include,src,scripts}

cd "$ROOT_DIR"
cargo package --allow-dirty --no-verify --package readcon-db
CRATE_TAR="$(ls -1 "$ROOT_DIR"/target/package/readcon-db-"${VERSION}".crate)"
mkdir -p "${TMP_DIR}/crate"
tar -C "${TMP_DIR}/crate" -xf "$CRATE_TAR"
CRATE_DIR="$(find "${TMP_DIR}/crate" -mindepth 1 -maxdepth 1 -type d | head -1)"
cp -a "${CRATE_DIR}/." "$DEST/"

cp -a "$ROOT_DIR/CMakeLists.txt" "$DEST/"
cp -a "$ROOT_DIR/cmake/." "$DEST/cmake/"
cp -a "$ROOT_DIR/meson.build" "$DEST/"
cp -a "$ROOT_DIR/meson_options.txt" "$DEST/"
cp -a "$ROOT_DIR/include/." "$DEST/include/"
cp -a "$ROOT_DIR/scripts/meson_cargo_build.py" "$DEST/scripts/"
cp -a "$ROOT_DIR/LICENSE" "$DEST/"
if [[ -f "$ROOT_DIR/Cargo.lock" ]]; then
    cp -a "$ROOT_DIR/Cargo.lock" "$DEST/"
fi

if ! grep -q '^\[workspace\]' "$DEST/Cargo.toml"; then
    printf '\n[workspace]\n' >> "$DEST/Cargo.toml"
fi

if [[ ! -f "$DEST/Cargo.lock" ]]; then
    cargo generate-lockfile --manifest-path "$DEST/Cargo.toml"
fi

cat > "$DEST/README.cxx.md" <<EOF
# readcon-db ${VERSION} (C/C++ source tarball)

Headers in \`include/\` are shipped. **cbindgen is not required.**

## CMake (FetchContent)

\`\`\`cmake
include(FetchContent)
FetchContent_Declare(
  readcon-db
  URL      https://github.com/lode-org/readcon-db/releases/download/v${VERSION}/readcon-db-cxx-${VERSION}.tar.gz
  URL_HASH SHA256=<sha256 of this file>
)
FetchContent_MakeAvailable(readcon-db)
target_link_libraries(app PRIVATE readcon-db::shared)
\`\`\`

Requires rustc/cargo. It does **not** require cbindgen or Corrosion.

## Meson (wrap)

\`\`\`meson
readcon_db_dep = dependency('readcon-db')
\`\`\`

## pkg-config

\`\`\`
pkg-config --cflags --libs readcon-db
\`\`\`
EOF

if [[ "$VENDOR" -eq 1 ]]; then
    mkdir -p "$DEST/.cargo"
    (
        cd "$DEST"
        cargo vendor --locked vendor
    )
    cat > "$DEST/.cargo/config.toml" <<'EOF'
[source.crates-io]
replace-with = "vendored-sources"

[source.vendored-sources]
directory = "vendor"
EOF
fi

rm -rf "$DEST/tests" "$DEST/benches" "$DEST/docs" "$DEST/python" "$DEST/fortran" || true
mkdir -p "$DEST/tests"
cp -a "$ROOT_DIR/tests/cmake-project" "$DEST/tests/"
cp -a "$ROOT_DIR/tests/meson-wrap" "$DEST/tests/"

tar -C "$TMP_DIR" -cf "${TMP_DIR}/${ARCHIVE_NAME}.tar" "$ARCHIVE_NAME"
gzip -9 "${TMP_DIR}/${ARCHIVE_NAME}.tar"
cp "${TMP_DIR}/${ARCHIVE_NAME}.tar.gz" "${OUTPUT_DIR}/${ARCHIVE_NAME}.tar.gz"

SHA="$(sha256sum "${OUTPUT_DIR}/${ARCHIVE_NAME}.tar.gz" | awk '{print $1}')"
echo "${OUTPUT_DIR}/${ARCHIVE_NAME}.tar.gz"
echo "sha256:${SHA}"
echo "${SHA}" > "${OUTPUT_DIR}/${ARCHIVE_NAME}.tar.gz.sha256"

# wrapdb wrap points at the slim tarball URL; do not overwrite it from --vendor.
if [[ "$VENDOR" -eq 0 && -f "$ROOT_DIR/packaging/wrapdb/readcon-db.wrap.in" ]]; then
    sed -e "s/@VERSION@/${VERSION}/g" -e "s/@SHA256@/${SHA}/g" \
        "$ROOT_DIR/packaging/wrapdb/readcon-db.wrap.in" \
        > "${OUTPUT_DIR}/readcon-db.wrap"
    echo "${OUTPUT_DIR}/readcon-db.wrap"
fi
