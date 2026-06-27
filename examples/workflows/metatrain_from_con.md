# Workflow: CON campaign → readcon-db → metatrain XYZ

metatrain loads ASE systems from **extXYZ** (or ASE `.db`, DiskDataset, MemmapDataset).
`readcon-db` sits **upstream**: keep optimizers’ CON checkpoints in an mmap corpus,
filter/dedup, then export only the training subset.

## 1. Ingest optimizer outputs

```bash
cargo build --release
./target/release/readcon-db ingest-dir /data/corpus /data/neb_runs/con_files
# or per file:
./target/release/readcon-db ingest /data/corpus --start-id 1 run_a.con run_b.convel
```

## 2. Select / dedup (common ML hygiene)

```bash
# all Cu-containing frames, write metatrain-ready XYZ
./target/release/readcon-db dedup-export /data/corpus --symbol Cu -o train_cu.xyz

# explicit select without dedup
./target/release/readcon-db select /data/corpus --symbol H --natoms-max 200 --export val.xyz
```

Exact geometry duplicates (re-ingested trajectories) share **xxHash3-128**;
`dedup-export` keeps the **first** representative key per hash.

## 3. Point metatrain at the XYZ

Minimal `options.yaml` fragment (see metatrain docs for full schema):

```yaml
training_set:
  systems:
    read_from: train_cu.xyz
    length_unit: angstrom
  targets:
    energy:
      key: energy
      unit: eV
    # forces:
    #   key: forces
    #   unit: eV/A
```

If CON files lacked energies/forces, add them in ASE after export, or ingest
`.con` files that include force sections (`tiny_cuh2_forces.con` style).

## 4. Other database-shaped use cases

| Use case | How |
|----------|-----|
| **Train/val split by trajectory** | `select --traj 1 --export train.xyz` vs `--traj 2` |
| **Composition filter** | `--symbol Cu` (and chain symbols via repeated ingest filters in Rust API) |
| **Size filter** | `--natoms-min` / `--natoms-max` (surface slabs vs clusters) |
| **Exact dedup** | `unique_frame_keys` / `dedup-export` / `find_by_hash` |
| **Cross-check blob** | `hash-file run.con` vs `frame_hash` in DB |
| **ASE / analysis** | Export XYZ → `ase.io.read(..., index=':')` |
| **LAMMPS / GROMACS** | Out of scope; export XYZ then convert with Atomsk/ASE |

## 5. Python (optional extension module)

```bash
cd python && maturin develop --features python
```

```python
from readcon_db import ConCorpus
db = ConCorpus("/data/corpus")
keys = db.select(symbol="Cu")
# write XYZ via CLI or extend Py API to call export in a follow-up
```

## 6. Why not ASE `.db` alone?

ASE databases are convenient for **small** sets metatrain already loads.
`readcon-db` targets **optimizer-native CON** at campaign scale with mmap,
content-addressed dedup, and multi-language ingest without forcing an ASE
dependency in Fortran/C pipelines.
