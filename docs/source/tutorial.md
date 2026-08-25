# Tutorial — first corpus

```{note}
Diátaxis *tutorial* (learning-oriented). Recipes by language:
{doc}`howto`. Ops at scale: {doc}`campaign`.
```

This walk-through opens an empty corpus directory, ingest the in-repo
CuH2 fixture, selects copper-containing frames, and prints a content
hash. Use a throwaway directory.

## Open

```python
from pathlib import Path
import tempfile
import readcon_db

root = Path(tempfile.mkdtemp())
db = readcon_db.ConCorpus.open(str(root))
```

```rust
use readcon_db::{ConCorpus, Select};
let db = ConCorpus::open("/tmp/readcon-db-tutorial")?;
```

`open` creates the LMDB environment if the directory is new. One writer
at a time; many readers may mmap the same tree.

## Ingest

```python
db.append_trajectory_path(1, "resources/test/tiny_cuh2.con")
```

```rust
db.append_trajectory_path(1, "resources/test/tiny_cuh2.con")?;
```

The blob on disk is CON text. `traj_id` `1` is yours to assign; later
appends to the same id extend that trajectory.

## Select and hash

```python
keys = db.select(readcon_db.Select().require_symbol("Cu"))
h = db.frame_hash(keys[0])
print(len(keys), h)
text = db.get_frame_text(keys[0])
```

```rust
let keys = db.select(&Select::new().require_symbol("Cu"))?;
let h = db.frame_hash(keys[0])?;
let text = db.get_frame_text(keys[0])?;
assert_eq!(db.find_by_hash(h)?, Some(keys[0]));
```

`Select` is an explicit filter (symbol, atom count, energy, section
flags, exact hash). It is not SQL. Hits are keys; decode a key when you
need a `ConFrame` or the raw CON blob.

## What you just did

1. Created a corpus directory (mmap LMDB).
2. Stored a CON fixture under `traj_id = 1`.
3. Filtered without loading every atom of a large tree.
4. Fingerprinted the hit with xxHash3-128.

Next: {doc}`howto` for C and Fortran, {doc}`campaign` for sharded
writers, {doc}`architecture` for the index layout.
