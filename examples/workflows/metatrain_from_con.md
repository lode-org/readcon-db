# Workflows without ASE on the I/O path

**Principle:** CON and XYZ (and other chemfiles formats) enter through
**readcon-core / `readcon`**. ASE is optional for calculators only.

## A. Optimizer CON → readcon-db (native)

```bash
readcon-db ingest-dir /data/corpus /data/neb_runs
readcon-db select /data/corpus --symbol Cu --require-forces \
    --energy-min -50 --energy-max 0   # keys only; decode with readcon
```

Python:

```python
import readcon
from readcon_db import ConCorpus

# CON
frames = readcon.read_all_frames("saddle.con")

# XYZ / PDB / … → ConFrame (requires chemfiles-linked readcon)
# frames = readcon.read_chemfiles("structures.xyz")

db = ConCorpus("/data/corpus")
# prefer writing CON then ingest, or extend API to append frames directly later
db.append_trajectory(1, "saddle.con")
keys = db.select(
    symbol="Cu",
    require_forces=True,
    energy_min=-50.0,
    energy_max=0.0,
)
text = db.get_frame_text(*keys[0])  # still CON
```

Stay in CON for analysis that uses `readcon` / C / Fortran APIs. Secondary
indexes (`idx_natoms`, `idx_symbol`, `idx_energy`, `idx_flags`) are documented in
[`docs/design.md`](../../docs/design.md) and in the companion
[readcon-core](https://github.com/lode-org/readcon-core) README ecosystem table.

## B. Only if an external tool requires XYZ on disk

Some training front-ends (e.g. metatrain YAML `read_from: train.xyz`) want ASE-style
extXYZ **files**. That is an **export** concern, not an import dependency on ASE:

```bash
readcon-db dedup-export /data/corpus --symbol Cu -o train_cu.xyz
```

Internally this uses `readcon-core` decode + a small XYZ writer in `readcon-db`
(`export_extxyz`) — **no `ase.io`**. Prefer teaching consumers CON+`readcon`
when you control the stack.

## C. Why not ASE `.db` as the campaign store?

Even though ASE can open many legacy `.con` files, `.db` is SQLite + `Atoms`:
weaker multi-reader behavior, no CON-native symbol index, and forces/convel/spec
v3 gaps on the ASE CON reader. CSE benchmarks (insert/extract/concurrency) are
in `examples/benchmarks/` and the CPC paper § on ASE `.db` vs readcon-db.
