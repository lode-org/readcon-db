# readcon-db

**Mmap-backed CON/convel corpus store** (LMDB via [Heed](https://github.com/meilisearch/heed)), **non-SQL selection**, **xxHash3-128 exact match**, and **Rust / C / C++ / Python / Fortran** bindings.

Companion to [`readcon-core`](https://github.com/lode-org/readcon-core): core owns **format fidelity**; this crate owns **corpus scale** (many trajectories, selective access, OS page-cache residency).

## Features

| Layer | Capability |
|-------|------------|
| Storage | Heed/LMDB env: `frames`, `traj_meta`, `idx_natoms`, `idx_symbol`, `frame_by_hash`, `hash_by_frame` |
| Exact match | **xxHash3-128** of canonical re-serialized CON blob (`xxhash-rust`) |
| Selection | `Select` builder — trajectory, atom-count range, required symbols, exact hash, limit |
| Concurrency | Many readers (`RoTxn`), **one writer** for ingest (LMDB) |
| Rust | `ConCorpus` API |
| C | `include/readcon-db.h` — `rkrdb_*` (`cdylib` / `staticlib`) |
| C++ | RAII `readcon_db::Corpus` in the same header (`extern "C"` + thin class) |
| Python | `maturin` / PyO3 feature `python` → module `readcon_db.ConCorpus` |
| Fortran | `fortran/ReadConDb` `bind(C)` wrappers |

## Quick start (Rust)

```rust
use readcon_db::{ConCorpus, Select};

let db = ConCorpus::open("/tmp/my_corpus")?;
db.append_trajectory_path(1, "run.con")?;
let keys = db.select(&Select::new().require_symbol("Cu").natoms_range(1, 500))?;
let h = db.frame_hash(keys[0])?;
assert_eq!(db.find_by_hash(h)?, Some(keys[0]));
```

```bash
cargo test -p readcon-db
cargo build --release   # libreadcon_db.so / .a
```

## C

```c
#include "readcon-db.h"
size_t id;
rkrdb_open("/tmp/corpus", &id);
uint32_t n;
rkrdb_append_trajectory(id, 1, "run.con", &n);
rkrdb_select_basic(id, 1, "Cu", 1, 100000, 0);
int m = rkrdb_result_count(id);
uint8_t hash[16];
rkrdb_frame_hash(id, 1, 0, hash);
rkrdb_close(id);
```

Link `-lreadcon_db` (and transitive deps from `cargo` staticlib as needed). Header: `include/readcon-db.h`.

## Python

```bash
cd python && maturin develop --features python
```

```python
from readcon_db import ConCorpus
db = ConCorpus("/tmp/corpus")
db.append_trajectory(1, "run.con")
keys = db.select(traj_id=1, symbol="Cu")
h = db.frame_hash(1, 0)
assert db.find_by_hash(h) == (1, 0)
```

## Fortran

Build the shared library first (`cargo build --release`), then point `fpm` / your compiler at `include/` and `target/release/libreadcon_db.so` (or static). Module: `fortran/ReadConDb/src/readcon_db.f90`.

## Design notes

- **No SQL** — access patterns are explicit indexes + in-process intersection.
- **Decode via readcon-core** — CON semantics never fork.
- **Dedup map** `frame_by_hash` keeps the **first** key for a given content hash (stable representative).
- Default map size **2 GiB**; create a larger env or reopen with a bigger map for huge corpora.

## License

MIT

## Documentation & website

| Artifact | Path |
|----------|------|
| **Logo (SVG)** | [`assets/logo/readcon-db-logo.svg`](assets/logo/readcon-db-logo.svg) |
| **Wordmark** | [`assets/logo/readcon-db-wordmark.svg`](assets/logo/readcon-db-wordmark.svg) |
| **Brand notes** | [`assets/logo/BRAND.md`](assets/logo/BRAND.md) |
| **Marketing site** | open [`website/index.html`](website/index.html) (teal LODE-style landing) |
| **Sphinx docs** | `docs/source/` — `cd docs && pip install -r requirements.txt && make html` |
| **CI Pages** | `.github/workflows/pages.yml` publishes `website/` + Sphinx under `/docs` |

Local preview:

```bash
cd docs && pip install -r requirements.txt && make html
# open docs/_build/html/index.html
python3 -m http.server 8765 --directory website   # marketing page
```
