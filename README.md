# readcon-db

**Mmap-backed CON/convel corpus store** (LMDB via [Heed](https://github.com/meilisearch/heed)), **non-SQL selection**, **xxHash3-128 exact match**, and **Rust / C / C++ / Python / Fortran** bindings.

Part of the **readcon ecosystem** with [`readcon-core`](https://github.com/lode-org/readcon-core) (Python package **`readcon`**):

| Crate / package | Role | Docs |
|-----------------|------|------|
| **[readcon-core](https://github.com/lode-org/readcon-core)** / **`readcon`** | CON interchange (parse/write/spec v2–v3). **XYZ/PDB/GRO → `ConFrame` via chemfiles** (`read_chemfiles*`), not ASE. Optional `to_ase` only for calculators. | Core README, `docs/orgmode/` |
| **readcon-db** / **`readcon_db`** (this repo) | Campaign store: mmap, indexes (natoms, symbols, **energy range**, **forces/velocities/energy flags**), multi-reader, dedup. Blobs are **CON text** decoded with readcon-core. | [`docs/design.md`](docs/design.md), Sphinx `docs/source/`, `website/` |

ASE is **not** on the critical path for reading CON or XYZ in this stack. ASE `.db` may appear in CSE **timing** tables; it is not the recommended store.

## Quick start

```bash
# Python (checkouts side-by-side)
export VIRTUAL_ENV=... && source $VIRTUAL_ENV/bin/activate
cd readcon-core && maturin develop --release --features python
# optional foreign formats:
# maturin develop --release --features python,chemfiles
cd ../readcon-db && maturin develop --release --features python

cargo test
cargo build --release   # libreadcon_db + CLI readcon-db
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
        .energy_range(-50.0, 0.0),
)?;
let h = db.frame_hash(keys[0])?;
```

```bash
./target/release/readcon-db ingest-dir /tmp/corpus /path/to/con_files
./target/release/readcon-db select /tmp/corpus --symbol Cu --require-forces \
    --energy-min -50 --energy-max 0
./target/release/readcon-db dedup-export /tmp/corpus --symbol Cu -o subset.xyz  # only if a tool demands XYZ on disk
```

Foreign trajectories: **`readcon.read_chemfiles("traj.xyz")` → frames → ingest into readcon-db** (chemfiles-enabled build), not `ase.io.read`.

## Design

- **No SQL** — explicit indexes + in-process intersection ([query cost model](docs/design.md)).
- **Decode via readcon-core** — CON semantics never fork.
- **Metadata indexes** — finite `energy` bins; flags for forces, velocities, energy presence.
- **xxHash3-128** on stored blobs — exact dedup / `find_by_hash`.
- **Many readers, one writer** (LMDB).

Full ABI table, logo, Sphinx docs, and site: see `docs/`, `website/`, `assets/logo/`, `CHANGELOG.md`. Fortran module notes: `fortran/ReadConDb/`.

## License

MIT
