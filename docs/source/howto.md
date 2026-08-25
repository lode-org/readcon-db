# How-to — corpus I/O by language

```{note}
Diátaxis *how-to* (task-oriented). Learning path: {doc}`tutorial`.
Full tables: {doc}`api_rust`, {doc}`api_c`, {doc}`api_python`,
{doc}`api_fortran`.
```

Same job in every language: open a corpus, ingest a `.con`, select,
read a hash.

## Rust

```rust
use readcon_db::{ConCorpus, Select};

let db = ConCorpus::open("/tmp/corpus")?;
db.append_trajectory_path(1, "run.con")?;
let keys = db.select(
    &Select::new()
        .trajectory(1)
        .require_symbol("Cu")
        .natoms_range(1, 10_000)
        .limit(100),
)?;
let h = db.frame_hash(keys[0])?;
let frame = db.get_frame(keys[0])?;
```

Exact match: `Select::new().exact_hash(h.to_bytes())`.

## Python

```python
import readcon_db

db = readcon_db.ConCorpus.open("/tmp/corpus")
db.append_trajectory_path(1, "run.con")
sel = readcon_db.Select().require_symbol("Cu")
keys = db.select(sel)
h = db.frame_hash(keys[0])
text = db.get_frame_text(keys[0])
```

`maturin develop --features python` from a checkout. Module name is
`readcon_db`.

## C / C++

```c
#include "readcon-db.h"

RkrdbCorpus *db = rkrdb_open("/tmp/corpus");
rkrdb_append_trajectory_path(db, 1, "run.con");
/* Select via rkrdb_select_* / rkrdb_select_meta; see api_c */
rkrdb_close(db);
```

C++ RAII: `readcon_db::Corpus`. MPI pack/bcast takes the **caller**
communicator (`readcon-db-mpi.h`). The library does not call
`MPI_Init`. Details: {doc}`api_c`.

## Fortran

`fortran/ReadConDb` is `bind(C)` over the same `rkrdb_*` ABI. Build
with `pkg-config --cflags --libs readcon-db` after a prefix install, or
the clib tarball. Details: {doc}`api_fortran`.

## Many writers

Do not open one LMDB env from two writers. Partition with
{doc}`campaign` (`shard-init`, `shard-ingest`, `drain`, `join-drained`).

## Units and H5MD

Stamp incoming units on append; `collect_h5md` / `export_h5md` emit one
`[T][N][3]` trajectory. CON text stays authoritative. {doc}`workflows`.
