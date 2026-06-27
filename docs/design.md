# readcon-db design notes

## Problem

`readcon-core` streams and validates CON/convel **files**. NEB and long-timescale campaigns produce **corpora**: \(10^3\)–\(10^6\) frames across many trajectories. Loading every frame as a `ConFrame` exceeds RAM; scanning every file for “all frames with H and Cu and \(N<200\)” wastes I/O.

## Approach

Treat the corpus as an **embedded key-value database** with **hand-built secondary indexes**, backed by **LMDB via Heed** so the OS page cache supplies RAM residency without a custom buffer pool.

## Why Heed/LMDB (not SQL, not SQLite)

- Mmap-first; readers do not copy the whole DB into a heap arena.
- Concurrent read transactions are first-class.
- Predictable latency for point lookups and ordered scans.
- We control indexes (natoms, symbols, energy bins, section flags) for exact access patterns of MD post-processing.
- SQL would invite ad hoc joins (frames ⨝ trajectories ⨝ species) that encourage copies and planner variance.

## Secondary indexes (v1)

| DB name | Key layout | Select predicates |
|---------|------------|-------------------|
| `idx_natoms` | BE `u32` n_atoms ‖ `FrameKey` | `natoms_range` |
| `idx_symbol` | symbol UTF-8 ‖ `0xff` ‖ `FrameKey` | `require_symbol` (AND over symbols) |
| `idx_energy` | order-preserving BE bits of finite energy ‖ `FrameKey` | `energy_range` |
| `idx_flags` | `flag_id` (u8) ‖ `FrameKey` | `require_forces` / `require_velocities` / `require_energy` |
| `frame_by_hash` / `hash_by_frame` | xxHash3-128 | `exact_hash`, dedup |

Energy is taken from `FrameHeader::energy()` (spec key `energy`). Forces/velocities from declared `sections` or per-atom data. Finite energies only enter `idx_energy`; missing energy is not a range miss—use `require_energy` when presence matters.

**Query cost model:** each predicate materializes a `BTreeSet<FrameKey>` from one index scan (or hash point lookup). Final result is set intersection, then sort + optional `limit`. Cost is proportional to the **smallest** selective index when predicates are independent—not to full corpus decode. Full-table fallback only when no predicate uses an index (traj filter alone still scans `frames` keys).

## Invariants

1. **Frame blob is authoritative** for fidelity; indexes are derived and rebuildable.
2. **Single writer** for ingest; analysis is read-only.
3. **Decode with `readcon-core`** so CON semantics never fork.
4. **Selection returns keys first**; callers decode lazily.

## Ecosystem

| Crate | Responsibility |
|-------|----------------|
| [readcon-core](https://github.com/lode-org/readcon-core) | CON/convel interchange, chemfiles ingress, multi-language hourglass ABI |
| **readcon-db** (this repo) | Campaign corpora, indexes, mmap multi-reader, exact dedup |

Foreign formats (XYZ/PDB/…) enter via **readcon-core chemfiles → `ConFrame` → ingest**; ASE is optional for calculators only.

## Rebuild indexes

`readcon-db reindex` (planned) walks `frames` and regenerates secondary DBs — required after corruption or schema evolution. Until then, recreate the corpus directory from source CON files.

## Security / multi-tenant

Single trusted user on local disk for v1 (same threat model as CON files on a workstation). No network protocol in v1.

## Status / roadmap

1. ~~Heed env + `frames` + `traj_meta` + ingest from path.~~
2. ~~`idx_natoms` + `idx_symbol` + `Select` intersection.~~
3. ~~`idx_energy` + `idx_flags` (forces/velocities/energy presence).~~
4. Optional cooked SoA blobs.
5. Python / C / Fortran bindings returning keys; decode in `readcon` Python package.
6. Parallel reindex with rayon (still one writer txn at a time, chunked commits).
