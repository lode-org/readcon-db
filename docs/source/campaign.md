# Campaign ops runbook

HPC ingest is **partitioned writers**, not multi-writer inside one LMDB
env. Each `shard_XXXX/` is an independent `ConCorpus`. Route
`traj_id % n_shards` to that shard. One process owns each `shard_id`
across the job. If more writers share a shard id, each node keeps a
private tree, `drain`s to a unique dest, then `join-drained`.

Default shard count is 64 (`DEFAULT_N_SHARDS`). Power-of-two counts
keep routing simple.

This page is the campaign ops runbook. Multi-shard writers are covered
by `src/shard.rs` tests (`parallel_writers_different_shards`,
`join_drained_roots_*`) and `tests/cli_refuse.rs` (mint/overwrite
refuses).

## Layout

```
<root>/
  shards.json          # { "n_shards": 64, "version": 1 }
  shard_0000/data.mdb
  shard_0001/data.mdb
  ...
```

`shards.json` is the manifest. `shard-init` writes it.
`shard-ingest`, `drain`, `join-drained`, `compact-join`, and
`shard-select` require it. Commands that only *read* (`shard-select`,
`compact-join`) use `open_existing` and do **not** mint a missing root.

## One writer per shard (pattern A)

Use this when the job can assign shard ids uniquely (`SLURM_PROCID %
n_shards`, or one rank owns shard \(S\)).

```bash
# once, on a shared or node-local root
readcon-db shard-init /scratch/$USER/campaign --shards 64

# each writer: --start-id must satisfy start-id ≡ shard (mod n_shards)
readcon-db shard-ingest /scratch/$USER/campaign --shard $S --start-id $T \
    --units '{"length":"angstrom","energy":"eV","time":"fs"}' run.con
```

`shard-ingest` refuses a missing manifest (does not mint). It refuses a
`--start-id` that does not route to `--shard`. After the first file it
skips traj ids that would land on another shard so one writer never
touches a foreign env.

Close every writer (drop the process, or `rkrdb_close`) **before**
drain. LMDB does not allow a snapshot while a write txn is open.

If every shard id has exactly one writer, drain to **one** dest that
does not already hold those `data.mdb` files:

```bash
readcon-db drain /scratch/$USER/campaign /lustre/proj/campaign/drained
readcon-db compact-join /lustre/proj/campaign/drained /lustre/proj/campaign_single
```

`drain` compact-snapshots `data.mdb` only (no `lock.mdb`) and refuses
dest overwrite. `compact-join` is the single-root join
(`open_existing`); dest must not exist.

## Overlapping shard ids (pattern B)

Use this when many nodes write the same shard ids (more writers than
shards, or each node owns a full 0..N-1 set on burst-buffer).

Each node keeps a **private** tree, drains to a **unique** dest, then
one join merges those dests:

```bash
# per node, on burst buffer (Frontier /mnt/bb, Aurora /tmp, ...)
readcon-db shard-init /mnt/bb/$USER/campaign --shards 64
readcon-db shard-ingest /mnt/bb/$USER/campaign --shard $S --start-id $T \
    --units '{"length":"angstrom","energy":"eV","time":"fs"}' run.con

# close writers, then drain to a dest no other node uses
readcon-db drain /mnt/bb/$USER/campaign \
    /lustre/orion/proj/campaign/node_$SLURM_NODEID

# once, after every node drained
readcon-db join-drained /lustre/orion/proj/campaign_single \
    /lustre/orion/proj/campaign/node_*
```

`join-drained` requires `shards.json` on every source, refuses an
existing dest, and refuses traj-id collisions **before** creating dest.
Give writers disjoint traj id ranges (or disjoint shard sets) so join
does not collide.

`drain` to a dest that already has `shard_XXXX/data.mdb` is an error:
two node-local trees that share a shard id must not last-writer-wins.

## Select and compact

```bash
# fan-out select; does not mint a missing root
readcon-db shard-select /lustre/orion/proj/campaign/drained --symbol Cu

# one sharded root -> new single-env corpus (dest must not exist)
readcon-db compact-join /lustre/orion/proj/campaign/drained \
    /lustre/orion/proj/campaign_single

# reverse: single-env -> new sharded root
readcon-db compact-split /lustre/orion/proj/campaign_single \
    /lustre/orion/proj/campaign_reshard --shards 64

# extXYZ only when an external tool demands XYZ on disk
readcon-db compact-export-extxyz /lustre/orion/proj/campaign/drained \
    subset.xyz --sharded --symbol Cu
```

`compact-export-extxyz` on a sharded root needs `--sharded`. Without the
flag it refuses (does not open the root as a single-env corpus). Dest
must not exist.

H5MD interchange is on the **single-env** dest after join:

```bash
# Python, dest must not exist
python -c 'from readcon_db import ConCorpus
ConCorpus("/lustre/orion/proj/campaign_single", readonly=True).export_h5md(1, "traj.h5")'
```

## Units

Stamp units on ingest (`--units` JSON, or C `rkrdb_append_trajectory_units`
/ Fortran `db_append_units`). Aliases canonicalize (`A` -> `angstrom`).
Missing `units.time` is CON `fs`. `set_units` converts stored numbers;
append/extend stamp incoming values. CON line-2 `units` is the
authority.

## C / Fortran writers

The C ABI and Fortran module open **one** corpus directory
(`rkrdb_open` / `db_open`). They do not implement shard routing.
Campaign partition stays on the CLI / Rust `ShardedConCorpus`.

A C or Fortran rank that owns shard \(S\) opens the shard directory
after `shard-init`:

```c
/* traj_id % n_shards == 3 */
rkrdb_open("/scratch/campaign/shard_0003", &id);
rkrdb_append_trajectory_units(id, 3, "run.con",
    "{\"length\":\"angstrom\",\"energy\":\"eV\",\"time\":\"fs\"}", &n);
rkrdb_close(id);
```

Then the same drain / join-drained / compact-join sequence as above.

Same-frame MPI: do **not** open the env on every rank. Rank 0 of the
**caller communicator** packs RCSO / RCSB and `MPI_Bcast`s on that
handle (`include/readcon-db-mpi.h`, Fortran INTEGER +
`rkrdb_bcast_packed_frame_f`). Shared `open_readonly` is the other
legal path when ranks touch different keys. See [workflows](workflows.md).

## Refuses (do not fight them)

| Command | Refuse |
|---------|--------|
| `shard-ingest` | missing `shards.json`; `--start-id` not congruent to `--shard` |
| `shard-select` / `compact-join` | missing root / missing `shards.json` (no mint) |
| `drain` | missing source `shards.json`; dest `shard_XXXX/data.mdb` already exists; dest `n_shards` mismatch |
| `join-drained` | dest exists; a source lacks `shards.json`; traj id appears in more than one source |
| `compact-join` / `compact-split` | dest exists; source missing |
| `compact-export-extxyz` | sharded root without `--sharded`; dest exists |

On refuse, dest is not created (or a partial dest is removed). Tests:
`tests/cli_refuse.rs`, `src/shard.rs`.

## CLI from a prebuilt clib prefix

Unpack `readcon-db-clib-$VERSION-$target.tar.gz` (see [install](install.md)).
`bin/readcon-db` is in that archive when the Release build produced it.

```bash
export PATH="$PREFIX/bin:$PATH"
export LD_LIBRARY_PATH="$PREFIX/lib:${LD_LIBRARY_PATH:-}"
readcon-db shard-init /scratch/$USER/campaign --shards 64
```
