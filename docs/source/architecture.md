# Architecture

## Ecosystem

| Layer | Crate | Role |
|-------|-------|------|
| Interchange | [**readcon-core**](https://github.com/lode-org/readcon-core) ([docs](https://lode-org.github.io/readcon-core/)) | CON/convel parse/write, chemfiles→`ConFrame`, typed `energy` / `sections`, hourglass ABI |
| Corpus | **readcon-db** (this project) | LMDB/Heed store, secondary indexes, xxHash3 dedup, multi-reader selects |

Frame **blobs are CON text**; indexes are derived at ingest from the same
`ConFrame` readcon-core already built—no second metadata schema.

## Environment layout

```
Environment (Heed / LMDB)
├── frames          : FrameKey → CON text blob (source span at ingest)
├── traj_meta       : traj_id → { n_frames, source }
├── idx_natoms      : (n_atoms BE, FrameKey) → ()
├── idx_symbol      : (symbol ‖ 0xFF ‖ FrameKey) → ()
├── idx_energy      : (ord(E) BE, FrameKey) → ()   # finite energy only
├── idx_flags       : (flag_id ‖ FrameKey) → ()    # forces / velocities / has_energy
├── frame_by_hash   : xxh3-128 → FrameKey (first wins)
└── hash_by_frame   : FrameKey → xxh3-128
```

`FrameKey` is 12 bytes: `traj_id` (BE u64) + `frame_idx` (BE u32) so lexicographic order matches numeric order.

**Flag ids** (u8): `1` = has forces, `2` = has velocities, `3` = has finite energy.
Energy order uses an order-preserving map of finite `f64` bits so range scans
match IEEE order. Energy comes from `FrameHeader::energy()` / metadata key
`energy` (readcon-core constants). Forces/velocities from declared `sections`
or per-atom data.

## Ingest path

1. `ConFrameIterator::next_with_raw_span` over file text (readcon-core)—store the **original** substring when possible (no hot-path re-serialize).
2. Parse once for indexes (natoms, symbols, energy, flags).
3. Store blob; compute **xxHash3-128**; update hash maps and secondary B-trees.

## Selection and query costs

Postings lists from secondary DBs are intersected in-process (`BTreeSet`):

| Predicate | Index | Cost sketch |
|-----------|-------|-------------|
| `exact_hash` | `frame_by_hash` | Point lookup |
| `require_symbol` | `idx_symbol` | Prefix walk for that element |
| `natoms_range` | `idx_natoms` | Ordered scan, early stop |
| `energy_range` | `idx_energy` | Ordered scan on finite energies |
| `require_forces` / `require_velocities` / `require_energy` | `idx_flags` | Prefix walk on flag id |
| traj filter alone | `frames` keys | Full key scan if no other index |

Decode (`get_frame`) is separate: one LMDB get + readcon-core parse. Keys-only
select avoids decode.

## Bindings hourglass

```
  Python / Fortran / C++ apps
            │
            ▼
     C ABI (rkrdb_*, incl. rkrdb_select_meta)  ◄── cdylib / staticlib
            │
            ▼
   Rust ConCorpus (Heed + readcon-core)
```

Fortran uses `bind(C)` wrappers under `fortran/ReadConDb/` (docs-level snippet;
not a long CPC listing).
