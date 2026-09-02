# Getting started

```{tip}
Install one language, then run the {doc}`tutorial`.
```

## Install

Pick **one** language. Pins match this tree (`0.1.6`). Core is `readcon-core` ^0.14 (pulled automatically for Rust).

| Package | Install | Destination |
|---------|---------|-------------|
| Rust | `cargo add readcon-db` | this site · [crates.io](https://crates.io/crates/readcon-db) |
| CLI | `cargo install readcon-db --locked` | `readcon-db` binary |
| Python | `pip install readcon-db` | [PyPI](https://pypi.org/project/readcon-db/) |
| C / C++ | CMake FetchContent, Meson wrap, or `pkg-config readcon-db` | {doc}`install` |
| Fortran | `fortran/ReadConDb` after a prefix or clib install | {doc}`api_fortran` |
| Prebuilt C ABI | `readcon-db-clib-$VER-$target.tar.gz` on the GitHub Release | {doc}`install` |

CON parse/write outside the corpus API is [readcon-core](https://lode-org.github.io/readcon-core/getting-started.html) (`pip install readcon` / `cargo add readcon-core`).

### Rust

```bash
cargo add readcon-db
```

### Python

```bash
pip install readcon-db
python -c "import readcon_db"
```

### C / C++ / Fortran

Headers in `include/` are shipped. cbindgen is not required. Full FetchContent / Meson / clib matrix: {doc}`install`.

## Smoke test

From a checkout (fixture under `resources/test/`):

```python
import tempfile
from pathlib import Path
import readcon_db

root = Path(tempfile.mkdtemp())
db = readcon_db.ConCorpus.open(str(root))
db.append_trajectory_path(1, "resources/test/tiny_cuh2.con")
keys = db.select(readcon_db.Select().require_symbol("Cu"))
print(len(keys), db.frame_hash(keys[0]))
```

```rust
use readcon_db::{ConCorpus, Select};
let db = ConCorpus::open("/tmp/readcon-db-smoke")?;
db.append_trajectory_path(1, "resources/test/tiny_cuh2.con")?;
let keys = db.select(&Select::new().require_symbol("Cu"))?;
println!("{} {:?}", keys.len(), db.frame_hash(keys[0])?);
```

## Where to go next

| Goal | Page | Kind |
|------|------|------|
| Learn open / ingest / select | {doc}`tutorial` | Tutorial |
| Same tasks in C / Fortran | {doc}`howto` | How-to |
| Many writers, shards | {doc}`campaign` | How-to |
| MPI, H5MD, ASE.db comparison | {doc}`workflows` | How-to |
| Why mmap / indexes | {doc}`architecture` | Explanation |
| Rust / C / Python / Fortran API | {doc}`api_rust` | Reference |

## Scope

| Task | Path |
|------|------|
| Parse or write one `.con` | [readcon-core](https://lode-org.github.io/readcon-core/) |
| Many frames, filter, hash | `readcon-db` |
| Field meanings (energy, formula) | `readcon_core::index_proj` (same keys as db indexes) |
| Foreign XYZ / PDB / GRO | core chemfiles path, then ingest CON |
