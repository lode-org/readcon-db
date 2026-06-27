# readcon-db

**Embedded frame store for large CON/convel corpora** — mmap-backed, concurrent readers, selection without SQL.

Companion to [`readcon-core`](https://github.com/lode-org/readcon-core): the core library owns *format fidelity* (parse one stream well); this crate owns *corpus scale* (many trajectories, selective access, OS-level “disk as RAM”).

## Why not SQL?

Optimizer corpora are append-mostly sequences of frames with secondary filters (atom count, symbols present, energy metadata, trajectory id). A general SQL engine optimizes joins and transactions we do not need, at the cost of copies and planner overhead on hot paths. We want:

1. **mmap / page-cache residency** — OS keeps hot pages in RAM without a second buffer pool.
2. **Many concurrent readers, one writer** — classic LMDB/Heed topology for analysis jobs.
3. **Zero-copy reads into `readcon-core` types** — deserialize only selected frames.
4. **Explicit indexes** — we define the access patterns; no query planner surprises.

## Storage engine: Heed (LMDB)

[Heed](https://github.com/meilisearch/heed) is the Rust LMDB wrapper used successfully in Meilisearch. LMDB properties we rely on:

| Property | Use for CON corpora |
|----------|---------------------|
| Single-level store, B+ trees in a file | One `*.rdb` (or env dir) per project/corpus |
| Memory-map entire environment | “Disk data in RAM” when working set fits; otherwise demand paging |
| MVCC readers without blocking writers long | Parallel analysis threads open read txns |
| Single writer | Ingestion / append trajectory is serialized (acceptable for MD post-processing) |
| Multiple named databases in one env | Separate spaces: frames, indexes, trajectory meta |

Alternatives considered and rejected for v1: SQLite (SQL tax, less predictable mmap semantics for blobs), Sled (different durability model), custom append-only log alone (no secondary indexes without reinventing B-trees).

## Data model

```
Environment (heed::Env)
├── db "traj_meta"     : traj_id -> TrajMeta { path_hint, n_frames, flags, created }
├── db "frames"        : FrameKey { traj_id, frame_idx } -> FrameBlob
├── db "frame_by_hash" : content_hash -> FrameKey (optional dedup)
├── db "idx_natoms"    : (n_atoms, traj_id, frame_idx) -> ()   # ordered for range scans
├── db "idx_symbol"    : (symbol, traj_id, frame_idx) -> ()    # multi-entry per frame
└── db "idx_energy"    : (energy_bin, traj_id, frame_idx) -> () # optional, from metadata
```

**FrameBlob** encodings (feature-negotiated):

1. **Raw CON text** (default ingest) — maximal fidelity; decode with `readcon-core` on read.
2. **Postcard/bincode of SoA** (optional “cooked” path) — faster re-read when format is trusted; still produced *by* `readcon-core` so semantics match.

Keys are fixed-width big-endian tuples so LMDB lexicographic order matches numeric order for range queries.

## Selection API (no SQL)

```rust
// Pseudocode
let hits = db.select(
    Select::new()
        .trajectory(traj_id)           // optional
        .natoms_range(50..=200)        // uses idx_natoms
        .require_symbols(&["Cu", "H"]) // intersection via idx_symbol
        .energy_ev_range(-10.0..0.0)   // if indexed
        .limit(10_000),
)?;
for key in hits {
    let frame: ConFrame = db.get_frame(key)?; // decode blob
}
```

Implementation strategy: intersect postings lists from secondary DBs (sort-merge), then fetch blobs. No boolean SQL; composition is explicit in Rust.

## Concurrency model

- **Readers**: unlimited `RoTxn` — analysis threads, Python GIL release around decode.
- **Writer**: one `RwTxn` for `append_trajectory` / `ingest_path`.
- **Never hold write txn across slow decode of unrelated work.**
- Prefer **bulk ingest** (one write txn per trajectory file) over per-frame commits.

## Optimal speed checklist

1. Ingest once as raw CON bytes; cook SoA offline if re-read dominates.
2. Keep environment on fast local NVMe; LMDB map size set at create (grow policy documented).
3. Secondary indexes only for filters that appear in real workloads (natoms, symbols, energy).
4. Stream selection results; do not materialize all `ConFrame`s when only keys are needed.
5. Align with `readcon-core` iterators for single-file workflows; use `readcon-db` only when \(N_{\mathrm{files}}\times N_{\mathrm{frames}}\) exceeds comfortable RAM *as decoded frames* but fits as mmap.

## Relation to readcon-core paper

The CPC manuscript argues format fidelity and a multi-language ABI. **readcon-db** is deliberately a *second* repository so CPC scope stays “format + library,” while corpus-scale concerns (indexes, mmap, multi-reader) do not inflate the interchange story. Cite this design when discussing future work / large NEB ensembles.

## Status

Design + skeleton only. Implementation proceeds behind feature flags; API is unstable until `readcon-core` 0.14+ ABI is frozen for blob decode.
