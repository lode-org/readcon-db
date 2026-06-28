# readcon-db design notes

## Problem

`readcon-core` streams and validates CON/convel **files**. NEB and long-timescale campaigns produce **corpora**: \(10^3\)–\(10^6\) frames across many trajectories. Loading every frame as a `ConFrame` exceeds RAM; scanning every file for “all frames with H and Cu and \(N<200\)” wastes I/O.

## Approach

Treat the corpus as an **embedded key-value database** with **hand-built secondary indexes**, backed by **LMDB via Heed** so the OS page cache supplies RAM residency without a custom buffer pool.

## Embedded multi-process SOTA patterns (what we implement)

These are standard patterns for **local** high-performance multi-process KV stores
(LMDB / Kyoto Cabinet / RocksDB-class embedded engines; Gray & Reuter MVCC lineage)—
**not** distributed cluster consensus (Raft/Paxos), which is out of scope for a
workstation campaign corpus.

| Pattern | Chemistry-store gap (ASE.db / SQLite) | readcon-db |
|---------|----------------------------------------|------------|
| **Mmap primary storage** | Row pages + BLOB deserialize | OS page cache holds CON text |
| **MVCC multi-reader, single-writer** | Writers/readers contend on SQLite locks | LMDB COW pages; `max_readers=512` |
| **Cross-process shared env** | One connection model in Python | Separate OS processes each `open()` same dir |
| **Authoritative value + secondary indexes** | Pickled `Atoms` + ad hoc KV | CON blob authoritative; B-tree indexes rebuild via `reindex` |
| **Selective index scan + set intersection** | Often full table / formula scan | Smallest-first `BTreeSet` intersect |
| **Batch read txn for trajectory extract** | Per-row `toatoms()` | `touch_trajectory_blobs` / `get_frame_texts` one `RoTxn` |
| **Exact content addressing** | Row id / UUID | xxHash3-128 on stored blob |

Cluster / network patterns (**not** implemented): multi-node sharding, Raft leader
election, gRPC query plane. Those do not match the NEB-on-one-host threat model.

## Why Heed/LMDB (not SQL, not SQLite)

- Mmap-first; readers do not copy the whole DB into a heap arena.
- Concurrent read transactions are first-class.
- Predictable latency for point lookups and ordered scans.
- We control indexes (natoms, symbols, energy, composition, fmax, section flags) for exact access patterns of MD post-processing.
- SQL would invite ad hoc joins that encourage copies and planner variance.

## Secondary indexes

| DB name | Key layout | Select predicates |
|---------|------------|-------------------|
| `idx_natoms` | BE `u32` n_atoms ‖ `FrameKey` | `natoms_range` |
| `idx_symbol` | symbol UTF-8 ‖ `0xff` ‖ `FrameKey` | `require_symbol` |
| `idx_elem_count` | symbol ‖ `0xff` ‖ BE count ‖ `FrameKey` | `element_exact` / `element_min` |
| `idx_formula` | canonical `Sym:count\|...` ‖ `0xff` ‖ `FrameKey` | `exact_composition` |
| `idx_energy` | order-preserving BE bits of finite energy ‖ `FrameKey` | `energy_range` |
| `idx_fmax` | order-preserving max \(\|F_i\|\) ‖ `FrameKey` (forces only) | `fmax_range` |
| `idx_flags` | `flag_id` (u8) ‖ `FrameKey` | `require_forces` / `require_velocities` / `require_energy` |
| `frame_by_hash` / `hash_by_frame` | xxHash3-128 | `exact_hash`, dedup |

Formula encoding: sorted non-empty symbols, `Sym:count` joined by `|` (e.g. `Cu:2|H:2`). Finite energies and fmax only; frames without forces never satisfy a finite `fmax_range`.

**Query cost model:** each predicate materializes a `BTreeSet<FrameKey>` from one index scan (or hash point lookup). Final result is set intersection, then sort + optional `limit`. Cost tracks selective indexes—not full corpus decode. Full-table fallback only when no indexed predicate is set.

## Ingest paths

1. Path / multi-frame CON text (`append_trajectory_path` / `append_trajectory_str`) with `next_with_raw_span` when possible.
2. **In-memory frames** (`append_trajectory_frames` / `extend_trajectory_frames`) for chemfiles → `ConFrame` → corpus without a temp file.
3. Directory ingest (`ingest_directory`).

## Reindex

`ConCorpus::reindex` (CLI: `readcon-db reindex <corpus_dir>`) clears secondary DBs and rebuilds them from authoritative `frames` blobs—**schema upgrade path** after adding indexes. Safe to run twice (idempotent key sets). Frames and `traj_meta` are not deleted.

## Invariants

1. **Frame blob is authoritative** for fidelity; indexes are derived and rebuildable.
2. **Single writer** for ingest/reindex; analysis is read-only.
3. **Decode with `readcon-core`** so CON semantics never fork.
4. **Selection returns keys first**; callers decode lazily.

## Core contracts (`readcon-core::index_proj`)

Screening scalars and ingest rules live in **readcon-core** so this crate does not fork CON meaning:

| Contract | API | Role |
|----------|-----|------|
| Index projection | `FrameIndexProjection::from_frame` | natoms, formula, finite energy, fmax, mass, volume, sections mask, meta channels |
| Formula encoding | `composition_formula` / `frame_composition_formula` | `idx_formula` keys (`Cu:2|H:2`) |
| Finite policy | `finite_energy`, mass/volume/fmax | Non-finite scalars omitted from ordered indexes |
| Sections mask | `sections_present_mask` / `SECTIONS_MASK_*` | forces / velocities / energies flags |
| Span ingest | `ConFrameIterator::next_with_raw_span`, `frame_byte_spans` | Store exact multi-frame substrings; no hot-path re-serialize |
| Canonical write | `ConFrameWriter::canonical(true)` | Opt-in stable JSON key order for materialize-from-frames |

`frame_scalars` and `corpus` prepare/reindex call into these APIs (thin wrappers / delegates).

## Ecosystem

| Crate | Responsibility |
|-------|----------------|
| [readcon-core](https://github.com/lode-org/readcon-core) | CON/convel interchange, chemfiles ingress, multi-language hourglass ABI |
| **readcon-db** (this repo) | Campaign corpora, indexes, mmap multi-reader, exact dedup, reindex |

ASE is calculator-only; not the campaign store.

## Security / multi-tenant

Single trusted user on local disk for v1. No network protocol in v1.

## Status

Shipped: frames, traj_meta, composition/energy/fmax/flags/natoms/symbol/hash indexes, `Select`, reindex, append frames, CLI/Python/C campaign select.
## Optional cooked SoA tier (`frames_soa`)

**Shipped (derived, non-authoritative):** each `FrameKey` may have a binary
payload in LMDB DB `frames_soa` (magic `RCSO`, v1 LE POD header + f64 N×3
positions and optional forces/velocities). Encode/decode: `cooked_soa::CookedSoa`
from a parsed `ConFrame`.

| Rule | Behavior |
|------|----------|
| Authority | UTF-8 CON text in `frames` only; xxHash3 / dedup / join-split / `reindex` ignore SoA |
| Opt-in | Default ingest does **not** cook; `cook_frame` / `recook_all` / `append_trajectory_path_cook(..., true)` |
| Hot path | `get_positions` / `get_forces` prefer valid cooked; corrupt/missing → parse CON |
| Discard | `delete_cooked_soa`; reindex and select unaffected |
| DLPack | Still **ephemeral** in-process views on `ConFrame`—not an LMDB value format |

Roadmap (unchanged): parallel chunked reindex; optional multi-dtype SoA matrix.

## ASE.db column ↔ readcon-db (competitive screening set)

Speed only matters if filters users already have in ASE.db exist. Architecture stays **non-SQL** (secondary LMDB DBs + intersection); the **feature claim** is campaign-column parity for CON-derivable fields.

| ASE.db / common filter | CON source | readcon-db predicate / index |
|------------------------|------------|------------------------------|
| `natoms` | atom count | `natoms_range` / `idx_natoms` |
| `formula` / species | multiset | `exact_composition`, `element_*` / `idx_formula`, `idx_elem_count` |
| symbol presence | symbols | `require_symbol` / `idx_symbol` |
| `energy` | metadata `energy` | `energy_range` / `idx_energy` |
| forces present / `fmax` | forces section / ‖F‖ | `require_forces`, `fmax_range` / `idx_flags`, `idx_fmax` |
| velocities | section / data | `require_velocities` |
| total mass | `masses_per_type` × counts | `mass_range` / `idx_mass` |
| cell volume | `lattice_vectors` or `boxl`+`angles` | `volume_range` / `idx_volume` |
| `pbc` | metadata `pbc` (explicit only; missing ≠ match) | `pbc([x,y,z])` / `idx_pbc` |
| `time`, `timestep` | reserved metadata | `time_range`, `timestep_range` / `idx_meta` |
| `frame_index` | reserved metadata | `frame_index_range` / `idx_meta` |
| NEB bead/band | `neb_bead`, `neb_band` | `neb_bead_range`, `neb_band_range` / `idx_meta` |
| `charge`, `magmom` | optional JSON numbers | `charge_range`, `magmom_range` / `idx_meta` |
| exact structure id | CON blob | `exact_hash` / xxHash3 |
| `id` / row id | — | `FrameKey` (traj_id, frame_idx) |
| `unique_id` UUID | — | **N/A (ASE bookkeeping)**; use content hash |
| `ctime` / `mtime` / `age` | — | **N/A (ASE bookkeeping)** |
| `user` / `calculator` | — | **N/A** unless stored in CON metadata (not competitive set) |
| arbitrary `key_value_pairs` DSL | — | **N/A (no SQL DSL)**; reserved + charge/magmom cover screening |
| SQL `SELECT` | — | **N/A (architecture)**; use `Select` / CLI / Python |

`reindex` rebuilds **all** secondary indexes including mass/volume/pbc/meta.

## Writer concurrency

CPU-bound prepare (parse CON spans / serialize ConFrames) runs **outside** the exclusive LMDB `write_txn`. Concurrent threads may prepare in parallel; only commits serialize at the engine (single active write txn). FFI handle-table locks do not cover ingest.


## HPC multi-writer (millions of ranks)

A **single** LMDB environment cannot run concurrent `write_txn`s. For site-scale
ingest (many SLURM tasks uploading CON), use **`ShardedConCorpus`**:

1. `readcon-db shard-init /scratch/campaign --shards 256` once on the shared FS.
2. Each rank opens **only its shard**: `shard_id = $SLURM_PROCID % n_shards` (or
   `traj_id % n_shards`) via `ShardedConCorpus::open_shard` / CLI `shard-ingest --shard S`.
3. Writers on **different shards never share a write lock** — up to `n_shards`
   parallel commits on one filesystem (bounded by FS, not one LMDB mutex).
4. Global queries: `shard-select` / `ShardedConCorpus::select` fans out read-only
   across shards (multi-reader MVCC per shard).

Assign trajectory IDs so `traj_id % n_shards == shard_id` (CLI `shard-ingest`
advances start-id accordingly). This is **partitioned embedded writers**, not
Raft multi-master — the right pattern for campaign uploads on Lustre/GPFS.

## LMDB model decision (HPC)

**KEEP sharded LMDB (Heed).** Single-env multi-writer contradicts LMDB SWMR; HPC-scale
uploads use **independent envs per shard** (MDHIM-style local backends / industry
sharded-LMDB practice). Full rationale, citations (LMDB docs, MDHIM HotStorage’15,
PapyrusKV, RocksDB contrast, HDF5/Zarr/ADIOS2/DAOS contrast), risks, and falsifiers:
see decision artifact from the storage-model review (session SCRATCH
`lmdb-model-decision.md`) or regenerate from team notes. Do **not** interpret
“one write_txn per env” as “LMDB cannot do multi-process writes”—only that
**partitioning is mandatory** for concurrent commits.

## Compaction: reversible join / split for analysis

| Mode | CLI | Use |
|------|-----|-----|
| **sharded-lmdb** | `shard-init` / `compact-split` | HPC multi-writer; parallel rank ingest |
| **single-env-lmdb** | `compact-join` / ordinary `ingest` | Laptop analysis; one `ConCorpus::open` |
| **extxyz** | `compact-export-extxyz` [--sharded] | External ML tools (non-LMDB) |

Join copies CON blobs by traj (ids preserved; duplicate traj across shards errors).
Split routes `traj_id % n_shards` into a new manifest root. Membership is reversible
under that routing (indexes rebuilt via normal prepare/commit ingest).
