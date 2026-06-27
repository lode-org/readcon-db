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
keys = db.select(
    traj_id=1,
    symbol="Cu",
    natoms_min=1,
    natoms_max=10_000,
    energy_min=-50.0,
    energy_max=0.0,
    require_forces=True,
)
h = db.frame_hash(1, 0)          # bytes(16)
assert db.find_by_hash(h) == (1, 0)
text = db.get_frame_text(1, 0)
raw = ConCorpus.xxh3_128(b"blob")
```

Optional `select` kwargs: `exact_hash=` (16-byte LE xxh3-128), `energy_min` /
`energy_max`, `require_forces`, `require_velocities`, `require_energy`, `limit`.
