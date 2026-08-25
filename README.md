# readcon-db

**Mmap-backed CON/convel corpus store** (LMDB via [Heed](https://github.com/meilisearch/heed)), **non-SQL selection**, **xxHash3-128 exact match**, and **Rust / C / C++ / Python / Fortran** bindings.

Part of the **readcon ecosystem** with [`readcon-core`](https://github.com/lode-org/readcon-core) (Python package **`readcon`**):

| Crate / package | Role | Docs |
|-----------------|------|------|
| **[readcon-core](https://github.com/lode-org/readcon-core)** / **`readcon`** | CON interchange (parse/write/spec v2–v3). **XYZ/PDB/GRO → `ConFrame` via chemfiles** (`read_chemfiles*`), not ASE. Optional `to_ase` only for calculators. | Core README, `docs/orgmode/` |
| **readcon-db** / **`readcon_db`** (this repo) | mmap CON corpus: indexes (natoms, symbols, **energy range**, **forces/velocities/energy flags**), multi-reader, dedup. Blobs are **CON text** decoded with readcon-core. | [docs](https://lode-org.github.io/readcon-db/docs/), [`docs/design.md`](docs/design.md) |

ASE is not on the CON or XYZ read path. Optional `to_ase` is calculator hand-off only.

## Install

```bash
cargo add readcon-db
cargo install readcon-db --locked   # CLI
pip install readcon-db             # module readcon_db (PyPI)
# C/C++: FetchContent / meson dependency('readcon-db') / pkg-config
# headers in include/ are shipped; cbindgen is not required
# Prebuilt C ABI (no cargo): readcon-db-clib-$VER-$target.tar.gz on the GitHub Release
```

Site: <https://lode-org.github.io/readcon-db/> · Docs: <https://lode-org.github.io/readcon-db/docs/> · crate: <https://crates.io/crates/readcon-db>

## Quick start (from source)

```bash
git clone https://github.com/lode-org/readcon-db
cd readcon-db
cargo test --locked
cargo build --release   # libreadcon_db + CLI readcon-db
```

Optional LODE sibling checkout (edit core + db together): clone both under the same parent, then create **untracked** `.cargo/config.toml` in `readcon-db`:

```toml
[patch.crates-io]
readcon-core = { path = "../readcon-core" }
```

Python extension from a checkout (`python/` + maturin):

```bash
pip install maturin
maturin develop --release --features python --manifest-path python/pyproject.toml
```

```rust
use readcon_db::{ConCorpus, Select};
let db = ConCorpus::open("/tmp/corpus")?;
db.append_trajectory_path(1, "run.con")?;
// XYZ in: use readcon-core chemfiles → ConFrame → append (see workflows)
let keys = db.select(
    &Select::new()
        .require_symbol("Cu")
        .require_forces()
        .exact_composition("Cu:2|H:2")
        .fmax_range(0.0, 1.0)
        .energy_range(-50.0, 0.0),
)?;
let h = db.frame_hash(keys[0])?;
```

```bash
./target/release/readcon-db ingest-dir /tmp/corpus /path/to/con_files
./target/release/readcon-db select /tmp/corpus --formula 'Cu:2|H:2' --require-forces \
    --fmax-max 1.0 --energy-min -50 --energy-max 0
./target/release/readcon-db reindex /tmp/corpus
./target/release/readcon-db dedup-export /tmp/corpus --symbol Cu -o subset.xyz  # only if a tool demands XYZ on disk
```

Foreign trajectories: **`readcon.read_chemfiles("traj.xyz")` → frames → ingest into readcon-db** (chemfiles-enabled build), not `ase.io.read`.

## Design

- **No SQL engine** — explicit indexes + in-process intersection, with **ASE.db-competitive screening fields** (mass, volume, PBC, reserved metadata, charge/magmom; see [design matrix](docs/design.md)).
- **Decode via readcon-core** — CON semantics never fork.
- **Metadata indexes** — finite `energy` bins; flags for forces, velocities, energy presence.
- **xxHash3-128** on stored blobs — exact dedup / `find_by_hash`.
- **Many readers, one writer** (LMDB). Same-frame MPI: rank 0 of the
  **caller communicator** packs RCSO and `MPI_Bcast` on that handle
  (`include/readcon-db-mpi.h`, Python `bcast_packed_frame` /
  `bcast_packed_frames`). The library never `MPI_Init`s and never names
  the process-wide world communicator; LAMMPS / mpi4py pass the comm they
  already own.
- **H5MD interchange** — `export_h5md` / `collect_h5md` writes one
  `[T][N][3]` trajectory (CON stays authority). Engine dest is Å / ps /
  kJ mol^{-1} Angstrom^{-1}; velocity dest is `Angstrom ps-1`. Callers stamp
  units on ingest; missing `units.time` is CON `fs`.
- **Node-local drain/join** — `shard-ingest` then `drain` to a unique
  dest (`data.mdb` only, refuse overwrite), then `join-drained`.
  `compact-join` joins one sharded root (`open_existing`). Campaign
  ops: [`docs/source/campaign.md`](docs/source/campaign.md).

Full ABI table, logo, Sphinx docs, and site: see `docs/`, `website/`, `assets/logo/`, `CHANGELOG.md`. Fortran module notes: `fortran/README.md`, `fortran/ReadConDb/`.

## License

MIT

## Cooked SoA tier

Optional RCSO numerics in `frames_soa` (opt-in cook). RCSO is
non-authoritative: CON text in `frames` is the sole authority for hash,
dedup, join/split, and reindex. User doc:
[`docs/orgmode/cooked-soa.org`](docs/orgmode/cooked-soa.org).
