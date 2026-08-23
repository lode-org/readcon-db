# Workflows

## MPI: one rank packs, the rest receive

When every rank needs the **same** frame, do not open LMDB on every rank.
Rank 0 of the communicator the host already owns (`lmp->world`, an mpi4py
`Comm`, a Fortran INTEGER handle) packs RCSO; `MPI_Bcast` on **that**
handle; workers unpack with no corpus. The library does not call
`MPI_Init` and does not name the process-wide world communicator.

```c
#include "readcon-db-mpi.h"
rkrdb_bcast_packed_frame(lmp->world, 0, dir, traj, frame, &buf, &n);
```

```python
from readcon_db import bcast_packed_frame
blob = bcast_packed_frame(lmp.world, dir, traj, frame)  # mpi4py Comm
```

The other legal path: every rank `open_readonly` (shared mmap) when ranks
touch **different** keys.

Many frames on one collective: `rkrdb_bcast_packed_frames` / Python
`bcast_packed_frames` (RCSB envelope). Grain is a NEB band or dump
window, not one EndStep per image.

Node-local ingest, then drain to the PFS (Frontier `/mnt/bb`, Aurora
`/tmp`):

```bash
readcon-db shard-init /mnt/bb/$USER/campaign --shards 64
readcon-db shard-ingest /mnt/bb/$USER/campaign --shard $S --start-id $T run.con
# ranks close
# One writer per shard id can drain to a shared dest.
# Overlapping shard ids (more writers than shards): unique dest per node, then join.
readcon-db drain /mnt/bb/$USER/campaign /lustre/orion/proj/campaign/node_$SLURM_NODEID
readcon-db join-drained /lustre/orion/proj/campaign_single \
    /lustre/orion/proj/campaign/node_*
```

## H5MD interchange

Cooked `[T][N][3]` export (CON stays authority in the corpus):

```python
from readcon_db import ConCorpus
db = ConCorpus("/lustre/orion/proj/campaign_single")
db.export_h5md(traj_id=1, path="traj.h5")
```

## CON-native (default)

Optimizers → **CON files** → `readcon-db` ingest → `select` / `get_frame` / C/`readcon` decode.
No ASE on this path.

```bash
readcon-db ingest-dir /data/corpus /data/neb_runs
readcon-db select /data/corpus --symbol Cu --require-forces \
  --energy-min -50 --energy-max 0
```

Metadata predicates use secondary indexes documented in [architecture](architecture.md)
(`idx_energy`, `idx_flags` alongside `idx_natoms` / `idx_symbol`).

## XYZ and other formats

Use **readcon-core chemfiles ingress** (`read_chemfiles`, Rust/C equivalents) to obtain
`ConFrame`s, write CON if needed, then ingest. Do **not** use ASE as the XYZ reader
for this stack. Peer docs: [readcon-core](https://lode-org.github.io/readcon-core/).

## Optional XYZ *export*

`export_extxyz` / CLI `dedup-export` only for external tools that demand XYZ on disk.
Implementation does not call ASE.

## ASE `.db` comparison (measurement only)

See repository `examples/benchmarks/` and the CPC manuscript CSE section. Those timings
are **unequal workloads** (lightweight ASE `Cu2` stand-ins vs full CON parse+index on
readcon-db)—CSE orientation for multi-reader behaviour, **not** a fair store-vs-store
parity claim and **not** “store Atoms in ASE.db” as the product path.


## Fair ASE.db vs readcon-db campaign

Use **`examples/benchmarks/fair_campaign.py`**: builds a multi-frame CON ladder from a real fixture,
loads **the same frames** into ASE `.db` (via readcon geometry → `Atoms`) and **readcon-db**,
records insert/extract/competitive select/8-reader timings, and checks **hit-count agreement**
for symbol `Cu` and `natoms` range. Results: JSON `ase_fair_campaign_{run}.json` and markdown table.

```bash
# venv with ase + maturin-developed readcon / readcon_db
python examples/benchmarks/fair_campaign.py --out /tmp/fair_out --run-id 1
python examples/benchmarks/test_fair_select_parity.py
```

Legacy `bench_ase_db.py` Cu2 timings are **unequal-workload** artifacts only.
Interchange axis (parse CON): campaign JSON field `interchange` (readcon vs `ase.io.read`).
