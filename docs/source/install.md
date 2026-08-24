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
  URL https://github.com/lode-org/readcon-db/releases/download/v0.1.4/readcon-db-cxx-0.1.4.tar.gz
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
Release (headers + `libreadcon_db` + `readcon-db.pc`). cbindgen is
not required. CMake FetchContent / Meson wrap still use the *source*
tarball `readcon-db-cxx-$VERSION.tar.gz`.

```bash
tar -xzf readcon-db-clib-0.1.4-x86_64-unknown-linux-gnu.tar.gz
cd readcon-db-clib-0.1.4-x86_64-unknown-linux-gnu
export PKG_CONFIG_PATH="$PWD/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
export LD_LIBRARY_PATH="$PWD/lib:${LD_LIBRARY_PATH:-}"
pkg-config --cflags --libs readcon-db
```

Attach assets to an already-published tag with Actions → **C ABI
library tarball** → `tag=vX.Y.Z` (packager from the workflow ref;
sources from the tag).

Fortran: `fortran/ReadConDb` (`bind(C)` against the C ABI). Point
fpm at this prefix instead of `target/release`:

```bash
cd fortran/ReadConDb
fpm test --flag "$(pkg-config --cflags readcon-db) -cpp" \
  --link-flag "$(pkg-config --libs readcon-db) -ldl -lpthread -lm"
```

## Documentation

- User site: <https://lode-org.github.io/readcon-db/>
- Rust API: <https://docs.rs/readcon-db>
- Design notes: [`docs/design.md`](https://github.com/lode-org/readcon-db/blob/main/docs/design.md) in the repository

```bash
pixi r -e docs docbld
```

`pixi.lock` pins Sphinx, Furo, and MyST to the same set Pages publishes. Refresh with `pixi lock` after editing `pixi.toml` `[feature.docs]`.

## Static marketing page

Open `website/index.html` (or serve `website/`). Logos under `assets/logo/`.
