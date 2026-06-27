# Architecture

## Environment layout

```
Environment (Heed / LMDB)
├── frames          : FrameKey → CON text blob
├── traj_meta       : traj_id → { n_frames, source }
├── idx_natoms      : (n_atoms BE, FrameKey) → ()
├── idx_symbol      : (symbol ‖ 0xFF ‖ FrameKey) → ()
├── frame_by_hash   : xxh3-128 → FrameKey (first wins)
└── hash_by_frame   : FrameKey → xxh3-128
```

`FrameKey` is 12 bytes: `traj_id` (BE u64) + `frame_idx` (BE u32) so lexicographic order matches numeric order.

## Ingest path

1. `ConFrameIterator` over file text (readcon-core).
2. Re-serialize each frame with `ConFrameWriter` (canonical blob).
3. Store blob; compute **xxHash3-128**; update indexes and dedup map.

## Selection

Postings lists from secondary DBs are intersected in-process (`BTreeSet`). Optional `exact_hash` is a point lookup into `frame_by_hash`.

## Bindings hourglass

```
  Python / Fortran / C++ apps
            │
            ▼
     C ABI (rkrdb_*)  ◄── cdylib / staticlib
            │
            ▼
   Rust ConCorpus (Heed + readcon-core)
```
