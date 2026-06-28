# readcon-db

**Mmap-backed CON/convel corpus store** (LMDB via [Heed](https://github.com/meilisearch/heed)), **non-SQL selection**, **xxHash3-128 exact match**, and **Rust / C / C++ / Python / Fortran** bindings.

Part of the **readcon ecosystem** with [`readcon-core`](https://github.com/lode-org/readcon-core) (Python package **`readcon`**):

| Crate / package | Role | Docs |
|-----------------|------|------|
| **[readcon-core](https://github.com/lode-org/readcon-core)** / **`readcon`** | CON interchange (parse/write/spec v2–v3). **XYZ/PDB/GRO → `ConFrame` via chemfiles** (`read_chemfiles*`), not ASE. Optional `to_ase` only for calculators. | Core README, `docs/orgmode/` |
| **readcon-db** / **`readcon_db`** (this repo) | Campaign store: mmap, indexes (natoms, symbols, **energy range**, **forces/velocities/energy flags**), multi-reader, dedup. Blobs are **CON text** decoded with readcon-core. | [`docs/design.md`](docs/design.md), Sphinx `docs/source/`, `website/` |

ASE is **not** on the critical path for reading CON or XYZ in this stack. ASE `.db` may appear in CSE **timing** tables; it is not the recommended store.

## Install

```bash
cargo add readcon-db
cargo install readcon-db --locked   # CLI
pip install readcon-db             # module readcon_db (PyPI)
```

Docs: <https://lode-org.github.io/readcon-db/> · API: <https://docs.rs/readcon-db> · crate: <https://crates.io/crates/readcon-db>

## Quick start (from source)

```bash
# Optional: sibling checkouts under LODE/ (path dep on readcon-core)
export VIRTUAL_ENV=... && source $VIRTUAL_ENV/bin/activate
cd readcon-core && maturin develop --release --features python
# optional foreign formats:
# maturin develop --release --features python,chemfiles
cd ../readcon-db && maturin develop --release --features python

cargo test --locked
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
- **Many readers, one writer** (LMDB).

Full ABI table, logo, Sphinx docs, and site: see `docs/`, `website/`, `assets/logo/`, `CHANGELOG.md`. Fortran module notes: `fortran/ReadConDb/`.

## License

MIT

## Cooked SoA tier

Optional RCSO numerics in `frames_soa` (opt-in cook). CON text in `frames` stays authoritative. User doc: [`docs/orgmode/cooked-soa.org`](docs/orgmode/cooked-soa.org).
