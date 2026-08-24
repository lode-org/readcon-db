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
rkrdb_bcast_packed_frames(lmp->world, 0, dir, trajs, frames, nkeys, &buf, &n);
```

```python
from readcon_db import bcast_packed_frame, bcast_packed_frames
blob = bcast_packed_frame(lmp.world, dir, traj, frame)  # mpi4py Comm
batch = bcast_packed_frames(lmp.world, dir, [(traj, 0), (traj, 1)])
```

The other legal path: every rank `open_readonly` (shared mmap) when ranks
touch **different** keys.

Many frames on one collective: `rkrdb_bcast_packed_frames` / Python
`bcast_packed_frames` (RCSB envelope). Grain is a NEB band or dump
window, not one EndStep per image.

Node-local ingest, then drain to the PFS: see the
[campaign ops runbook](campaign.md). One writer per shard id can
drain to a shared dest. Overlapping shard ids (more writers than
shards): unique dest per node, then `join-drained`.

## H5MD interchange

Cooked `[T][N][3]` export (CON stays authority in the corpus):

```python
from readcon_db import ConCorpus
db = ConCorpus("/lustre/orion/proj/campaign_single", readonly=True)
db.export_h5md(traj_id=1, path="traj.h5")
```

Dest must not exist (`File` `"x"`). A write failure removes the dest.

Time on the file is dest `ps`: CON `header.time()`, or `i * timestep`,
else the frame index. Missing `units.time` is CON `fs`. Optional force
and velocity groups are dest `kJ mol-1 Angstrom-1` and `Angstrom ps-1`.
`readcon-db compact-join` is the single-root join (`open_existing`).

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

Use **readcon-core chemfiles ingress** (`read_chemfiles`, `read_chemfiles_nth`,
Rust/C `rkr_read_chemfiles*`) to obtain `ConFrame`s, write CON if needed, then
ingest. Chemfiles converts format units on read (GRO nm → Å) and stamps line-2
`units` as Å / ps / amu; use `skip` / `step` / `read_step` rather than loading
every frame. Do **not** use ASE as the XYZ reader for this stack. Peer docs:
[readcon-core](https://lode-org.github.io/readcon-core/).

## Optional XYZ *export*

`export_extxyz` / CLI `dedup-export` / `compact-export-extxyz` only for
external tools that demand XYZ on disk. Dest must not exist (`create_new`;
a write failure removes the dest). `Lattice` is the same triclinic
`[3][3]` as H5MD (`boxl`+angles or `lattice_vectors`). `pbc` is CON
`header.pbc()` (`T`/`F`, default `T T T`). Implementation does not call ASE.
`compact-export-extxyz --sharded` joins through a temp dest that is removed.

## ASE `.db` comparison (measurement only)

The CPC manuscript is the readcon-core paper. This crate is the companion
campaign store. A store-comparison appendix, if used, is the frozen **fair**
campaign in `paper/cpc/freeze/` ([CPC companion](cpc.md)), not a product path
and not “store `Atoms` in ASE.db”.

Legacy `examples/benchmarks/bench_ase_db.py` Cu2 stand-ins are **unequal
workloads** (lightweight ASE rows vs full CON parse+index). Keep them as
artifacts only; do not promote them into the paper table.

## Fair ASE.db vs readcon-db campaign

Use **`examples/benchmarks/fair_campaign.py`**: builds a multi-frame CON ladder from a real fixture,
loads **the same frames** into ASE `.db` (via readcon geometry → `Atoms`) and **readcon-db**,
records insert/extract/competitive select/8-reader timings, and checks **hit-count agreement**
for symbol `Cu` and `natoms` range. Live results: JSON `ase_fair_campaign_{run}.json` and
markdown table. The paper freeze is a copy under `paper/cpc/freeze/`; re-running this
script does not move that freeze.

```bash
# venv with ase + maturin-developed readcon / readcon_db
python examples/benchmarks/fair_campaign.py --out /tmp/fair_out --run-id 1
python examples/benchmarks/test_fair_select_parity.py
python paper/cpc/scripts/gen_fair_table.py --check
```

Legacy `bench_ase_db.py` Cu2 timings are **unequal-workload** artifacts only.
Interchange axis (parse CON): campaign JSON field `interchange` (readcon vs `ase.io.read`).
