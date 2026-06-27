# Python API

Build the extension (from a checkout next to `readcon-core`):

```bash
cd python
maturin develop --features python
```

```python
from readcon_db import ConCorpus

db = ConCorpus("/tmp/corpus")
db.append_trajectory(1, "run.con")
keys = db.select(traj_id=1, symbol="Cu", natoms_min=1, natoms_max=10_000)
h = db.frame_hash(1, 0)          # bytes(16)
assert db.find_by_hash(h) == (1, 0)
text = db.get_frame_text(1, 0)
raw = ConCorpus.xxh3_128(b"blob")
```

`exact_hash=` accepts a 16-byte `bytes` object (LE xxh3-128 as stored).
