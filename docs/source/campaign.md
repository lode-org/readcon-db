# Campaign ops runbook

Operator path for a multi-writer CON campaign. Multi-shard writers
(`parallel_writers_different_shards`, `join_drained_roots_*`) are
covered by `src/shard.rs` tests. This page is the runbook, not a
second store design.

Single-env LMDB has **one writer**. Site-scale ingest uses
**partitioned writers**: one independent env per shard
(`root/shard_XXXX/`). That is not multi-writer inside one env.

## Layout

```
campaign_root/
  shards.json          # n_shards, version
  shard_0000/data.mdb
  shard_0001/data.mdb
  ...
```

Routing is `traj_id % n_shards`. CLI `shard-ingest --start-id T`
refuses a start id that does not land on `--shard`, then advances
`tid` so each next file stays on that shard.

Two compact layouts after ingest:

| Mode | CLI | Use |
|------|-----|-----|
| **sharded-lmdb** | `shard-init` / `compact-split` | HPC multi-writer |
| **single-env-lmdb** | `compact-join` / `join-drained` | Laptop analysis, H5MD, `open_readonly` |

## Path A: one writer per shard id

Use this when the job can assign each `shard_id` to exactly one writer
(rank or node). Writers on different shards never share a write lock.

```bash
# once, on the shared filesystem
readcon-db shard-init /scratch/campaign --shards 64

# each writer: only its shard. start-id ≡ shard (mod n_shards)
readcon-db shard-ingest /scratch/campaign --shard $S --start-id $S \
    --units '{"length":"angstrom","energy":"eV","time":"fs"}' run.con

# close writers, then optional compact snapshot (data.mdb only)
readcon-db drain /scratch/campaign /lustre/proj/campaign_pfs

# fan-out select across shards
readcon-db shard-select /scratch/campaign --symbol Cu

# laptop analysis: one env
readcon-db compact-join /scratch/campaign /data/campaign_single
```

`drain` refuses a dest shard that already has `data.mdb`. Path A can
drain to one dest because shard ids do not overlap.

## Path B: overlapping shard ids (more writers than shards)

If many ranks share a `shard_id`, do **not** open the same shard env
from two writers. Each node keeps a **private** tree, drains to a
**unique** dest, then `join-drained` merges trajs.

```bash
# per node, burst buffer / local disk
readcon-db shard-init /mnt/bb/$USER/campaign --shards 64
readcon-db shard-ingest /mnt/bb/$USER/campaign --shard $S --start-id $T \
    --units '{"length":"angstrom","energy":"eV","time":"fs"}' run.con

# ranks close the env
# unique dest per node (node id, not shard id)
readcon-db drain /mnt/bb/$USER/campaign \
    /lustre/proj/campaign/node_$SLURM_NODEID

# one join after every node has drained
readcon-db join-drained /lustre/proj/campaign_single \
    /lustre/proj/campaign/node_*
```

`join-drained` requires `shards.json` on every source, refuses an
existing dest, and refuses duplicate `traj_id` across sources (dest
is not created on collision). Assign traj ids so they are unique
across the job, not only inside one node.

## Drain contract

- Close writers first. `drain` compact-snapshots `data.mdb` only
  (no `lock.mdb`).
- Dest shard that already exists: refuse. Drain each overlapping
  tree to a unique dest, then `join-drained`.
- Dest `shards.json` `n_shards` must match src.
- Failure rolls back shards created by that call.

`compact-join` joins **one** sharded root (`open_existing`) into a
single env. `join-drained` joins **several** drained roots. Do not
swap them.

## After join

```bash
readcon-db select /lustre/proj/campaign_single --symbol Cu --require-forces
# optional interchange (CON stays authority in the corpus)
# Python: ConCorpus(...).export_h5md(traj_id=1, path="traj.h5")
```

Same-frame MPI (every rank needs one key): pack on rank 0 of the
**caller communicator**, `MPI_Bcast` on that handle. See
[workflows](workflows.md). Do not open LMDB on every rank for that
pattern.

## Do not

- Two writers on the same `shard_id` in one tree.
- `drain` onto a dest that already holds those shards.
- Reuse `traj_id` across nodes that will `join-drained`.
- Static-link `libreadcon_db.a` with `libreadcon_core.a` (duplicate
  rust-std). Shared objects only when both C APIs are in one process.
- Treat cooked RCSO or H5MD as authority. CON text in `frames` is.
