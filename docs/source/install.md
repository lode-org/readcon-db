# Install

## Rust (crates.io)

```bash
cargo add readcon-db
# library + CLI binary from the same crate:
cargo install readcon-db --locked
```

Requires [readcon-core](https://crates.io/crates/readcon-core) **^0.14** (pulled automatically).

## Python (PyPI)

```bash
pip install readcon-db
python -c "import readcon_db"
```

Install [readcon](https://pypi.org/project/readcon/) (core) as well when you need CON parse/write outside the corpus API.

## From source (developers)

```bash
git clone https://github.com/lode-org/readcon-db
cd readcon-db
cargo test --locked
cargo build --release   # libreadcon_db + CLI readcon-db
```

`readcon-core` comes from crates.io (`^0.14`). To develop against a local core tree, add an **untracked** `.cargo/config.toml`:

```toml
[patch.crates-io]
readcon-core = { path = "../readcon-core" }
```

Python extension from a checkout:

```bash
pip install maturin
maturin develop --release --features python --manifest-path python/pyproject.toml
```

## C / C++ (no cbindgen)

`include/readcon-db.h` is hand-written and shipped. Optional
`include/readcon-db-mpi.h` takes the caller's `MPI_Comm` (not linked into
the library). CMake FetchContent, Meson wrap, and `pkg-config` do **not**
run cbindgen.

CMake:

```cmake
include(FetchContent)
FetchContent_Declare(
  readcon-db
  URL https://github.com/lode-org/readcon-db/releases/download/v0.1.6/readcon-db-cxx-0.1.6.tar.gz
  URL_HASH SHA256=<sha256 from the .sha256 sidecar on the GitHub Release>
)
FetchContent_MakeAvailable(readcon-db)
target_link_libraries(app PRIVATE readcon-db::shared)
```

From a checkout:

```bash
cmake -S . -B build -DCMAKE_INSTALL_PREFIX=$PWD/prefix
cmake --build build && cmake --install build
export PKG_CONFIG_PATH=$PWD/prefix/lib/pkgconfig
pkg-config --cflags --libs readcon-db
```

Meson: `dependency('readcon-db')` with a wrap pointing at
`readcon-db-cxx-$VERSION.tar.gz` (see `packaging/wrapdb/`).

A process that uses both C APIs must link the **shared** objects
(`libreadcon_db.so` and `libreadcon_core.so`). Do not static-link
`libreadcon_db.a` together with `libreadcon_core.a`: the db archive
already contains the Rust core and rust-std, and the duplicate
symbols fail at link.

## Prebuilt C ABI (no local cargo)

Unpack `readcon-db-clib-$VERSION-$target.tar.gz` from the GitHub
Release, then `export READCON_DB_LIB` / `PKG_CONFIG_PATH`. Attach
assets to an already-published tag with Actions → **C ABI library
tarball** → `tag=vX.Y.Z`.

```bash
VER=0.1.6
TARGET=x86_64-unknown-linux-gnu
curl -fsSL -O "https://github.com/lode-org/readcon-db/releases/download/v${VER}/readcon-db-clib-${VER}-${TARGET}.tar.gz"
tar -xzf "readcon-db-clib-${VER}-${TARGET}.tar.gz"
PREFIX="$PWD/readcon-db-clib-${VER}-${TARGET}"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
export LD_LIBRARY_PATH="$PREFIX/lib:${LD_LIBRARY_PATH:-}"
export READCON_DB_LIB="$PREFIX/lib/libreadcon_db.so"
pkg-config --cflags --libs readcon-db
```

The cxx tarball still needs rustc/cargo. The clib tarball is the
Fortran / C path that does not.

## Fortran

`fortran/ReadConDb` (`bind(C)` against the C ABI). Notes:
`fortran/README.md`. Smoke after unpacking the clib prefix:

```bash
cd fortran/ReadConDb
fpm build --flag "$(pkg-config --cflags readcon-db)" \
  --link-flag "$(pkg-config --libs readcon-db) -ldl -lpthread -lm"
```

From a checkout (after a release build of the cdylib):

```bash
cd fortran/ReadConDb && fpm build --flag "-I../../include" \
  --link-flag "-L../../target/release -lreadcon_db -ldl -lpthread -lm"
```

Campaign shard/drain/join is the `readcon-db` CLI (in `bin/` of the
clib tarball when the Release build produced it). See
[campaign ops](campaign.md).

## Documentation

- Landing page: <https://lode-org.github.io/readcon-db/>
- Full docs: <https://lode-org.github.io/readcon-db/docs/>
- Design notes: [`docs/design.md`](https://github.com/lode-org/readcon-db/blob/main/docs/design.md) in the repository

```bash
pixi r -e docs docbld
```

`pixi.lock` pins Sphinx, Furo, and MyST to the same set Pages publishes. Refresh with `pixi lock` after editing `pixi.toml` `[feature.docs]`.

## Static marketing page

Open `website/index.html` (or serve `website/`). Logos under `assets/logo/`.
