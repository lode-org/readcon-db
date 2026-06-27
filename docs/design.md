# readcon-db design notes

## Problem

`readcon-core` streams and validates CON/convel **files**. NEB and long-timescale campaigns produce **corpora**: \(10^3\)–\(10^6\) frames across many trajectories. Loading every frame as a `ConFrame` exceeds RAM; scanning every file for “all frames with H and Cu and \(N<200\)” wastes I/O.

## Approach

Treat the corpus as an **embedded key-value database** with **hand-built secondary indexes**, backed by **LMDB via Heed** so the OS page cache supplies RAM residency without a custom buffer pool.

## Why Heed/LMDB (not SQL, not SQLite)

- Mmap-first; readers do not copy the whole DB into a heap arena.
- Concurrent read transactions are first-class.
- Predictable latency for point lookups and ordered scans.
- We control indexes (natoms, symbols, energy bins) for exact access patterns of MD post-processing.
- SQL would invite ad hoc joins (frames ⨝ trajectories ⨝ species) that encourage copies and planner variance.

## Invariants

1. **Frame blob is authoritative** for fidelity; indexes are derived and rebuildable.
2. **Single writer** for ingest; analysis is read-only.
3. **Decode with `readcon-core`** so CON semantics never fork.
4. **Selection returns keys first**; callers decode lazily.

## Rebuild indexes

`readcon-db reindex` walks `frames` and regenerates secondary DBs — required after corruption or schema evolution.

## Security / multi-tenant

Single trusted user on local disk for v1 (same threat model as CON files on a workstation). No network protocol in v1.

## Roadmap

1. Heed env + `frames` + `traj_meta` + ingest from path.
2. `idx_natoms` + `idx_symbol` + `Select` intersection.
3. Optional cooked SoA blobs.
4. Python bindings (PyO3) returning keys; decode in `readcon` Python package.
5. Parallel reindex with rayon (still one writer txn at a time, chunked commits).
