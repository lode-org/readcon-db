# Fortran `ReadConDb` (fpm)

Production **ISO_C_BINDING** bindings over `include/readcon-db.h`, managed
with [fpm](https://fpm.fortran-lang.org/). Module source:
`fortran/ReadConDb/src/readcon_db.f90`.

## Prebuilt C ABI (no local cargo)

`fpm.toml` already has `link = ["readcon_db"]`. Point the linker at a
Release tarball instead of `target/release`:

```bash
VER=0.1.4
TARGET=x86_64-unknown-linux-gnu
curl -fsSL -O "https://github.com/lode-org/readcon-db/releases/download/v${VER}/readcon-db-clib-${VER}-${TARGET}.tar.gz"
tar -xzf "readcon-db-clib-${VER}-${TARGET}.tar.gz"
PREFIX="$PWD/readcon-db-clib-${VER}-${TARGET}"
export PKG_CONFIG_PATH="$PREFIX/lib/pkgconfig:${PKG_CONFIG_PATH:-}"
export LD_LIBRARY_PATH="$PREFIX/lib:${LD_LIBRARY_PATH:-}"
export READCON_DB_LIB="$PREFIX/lib/libreadcon_db.so"
cd fortran/ReadConDb
fpm build --flag "$(pkg-config --cflags readcon-db)" \
  --link-flag "$(pkg-config --libs readcon-db) -ldl -lpthread -lm"
```

Dispatch `.github/workflows/c_lib_tarball.yml` with `tag=vX.Y.Z` to attach
those assets to an existing GitHub Release (same hook as `cxx_tarball.yml`).

A process that also uses the readcon-core C API must link the **shared**
objects (`libreadcon_db.so` and `libreadcon_core.so`). Do not static-link
`libreadcon_db.a` together with `libreadcon_core.a`.

## From a checkout

```bash
cargo build --release
cd fortran/ReadConDb
fpm build --flag "-I../../include" \
  --link-flag "-L../../target/release -lreadcon_db -ldl -lpthread -lm"
```

API notes: Sphinx [Fortran API](../docs/source/api_fortran.md). Campaign
shard/drain/join stays on the `readcon-db` CLI; a Fortran rank that owns
one shard opens `root/shard_XXXX` with `db_open`. See
[campaign ops](../docs/source/campaign.md).
