# readcon-db

**Mmap-backed CON/convel corpus store** (LMDB via [Heed](https://github.com/meilisearch/heed)), **non-SQL selection**, **xxHash3-128 exact match**, and **Rust / C / C++ / Python / Fortran** bindings.

Companion to [`readcon-core`](https://github.com/lode-org/readcon-core) / Python package **`readcon`**:

| Crate / package | Role |
|-----------------|------|
| **readcon-core** / **`readcon`** | CON interchange (parse/write/spec v2–v3). **XYZ/PDB/GRO → `ConFrame` via chemfiles** (`read_chemfiles*`), not ASE. Optional `to_ase` only for calculators. |
| **readcon-db** / **`readcon_db`** | Campaign store: mmap, indexes, multi-reader, dedup. Blobs are **CON text** decoded with readcon-core. |

ASE is **not** on the critical path for reading CON or XYZ in this stack. ASE `.db` is a comparison baseline for CSE metrics, not the recommended store.

## Quick start

```bash
# Python (checkouts side-by-side)
export VIRTUAL_ENV=... && source $VIRTUAL_ENV/bin/activate
cd readcon-core && maturin develop --release --features python
# optional foreign formats:
# maturin develop --release --features python,chemfiles
cd ../readcon-db && maturin develop --release --features python

cargo test -p readcon-db
cargo build --release   # libreadcon_db + CLI readcon-db
```

```rust
use readcon_db::{ConCorpus, Select};
let db = ConCorpus::open("/tmp/corpus")?;
db.append_trajectory_path(1, "run.con")?;
// XYZ in: use readcon-core chemfiles → ConFrame → append (see workflows)
let keys = db.select(&Select::new().require_symbol("Cu"))?;
let h = db.frame_hash(keys[0])?;
```

```bash
./target/release/readcon-db ingest-dir /tmp/corpus /path/to/con_files
./target/release/readcon-db dedup-export /tmp/corpus --symbol Cu -o subset.xyz  # only if a tool demands XYZ on disk
```

Foreign trajectories: **`readcon.read_chemfiles("traj.xyz")` → frames → ingest into readcon-db** (chemfiles-enabled build), not `ase.io.read`.

## Design

- **No SQL** — explicit indexes + in-process intersection.
- **Decode via readcon-core** — CON semantics never fork.
- **xxHash3-128** on canonical blobs — exact dedup / `find_by_hash`.
- **Many readers, one writer** (LMDB).

Full ABI table, logo, Sphinx docs, and site: see `docs/`, `website/`, `assets/logo/`, `CHANGELOG.md`.

## License

MIT
