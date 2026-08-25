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
├── idx_fmax        : (ord(fmax) BE, FrameKey) → ()  # forces present only
├── idx_mass        : (ord(mass) BE, FrameKey) → ()
├── idx_volume      : (ord(V) BE, FrameKey) → ()
├── idx_pbc         : mask ‖ FrameKey  # metadata pbc only
├── idx_meta        : channel ‖ ord(value) ‖ FrameKey  # time, NEB, charge, …
├── idx_elem_count  : symbol ‖ 0xFF ‖ BE count ‖ FrameKey
├── idx_formula     : `Cu:2|H:2` ‖ 0xFF ‖ FrameKey
├── idx_flags       : (flag_id ‖ FrameKey) → ()    # forces / velocities / has_energy
├── frame_by_hash   : xxh3-128 → FrameKey (first wins)
├── frames_soa      : FrameKey → RCSO cooked numerics (optional, derived)
└── hash_by_frame   : FrameKey → xxh3-128
```

**H5MD interchange:** `collect_h5md` / `export_h5md` emit one `[T][N][3]`
trajectory (position, optional force and velocity). CON text stays
authority. Dest units are Å / ps / kJ mol^{-1} Å^{-1}; velocity dest is
`Angstrom ps-1`.

**Drain/join:** node-local `shard-ingest`, `drain_to` compact-snapshots
`data.mdb` (refuse overwrite), then `join-drained`. `compact-join` is
the single-root join (`open_existing`). Ops: [campaign](campaign.md).

**Cooked SoA tier:** optional RCSO in `frames_soa` accelerates `get_positions` / `get_forces` / `get_velocities` without CON parse when valid. RCSO is non-authoritative: CON text in `frames` remains sole authority for hash/dedup/join/reindex. RCSO is not fully equivalent (no symbols/metadata/exact bytes). See `docs/orgmode/cooked-soa.org`.


`FrameKey` is 12 bytes: `traj_id` (BE u64) + `frame_idx` (BE u32) so lexicographic order matches numeric order.

**Flag ids** (u8): `1` = has forces, `2` = has velocities, `3` = has finite energy.
Energy / fmax use an order-preserving map of finite `f64` bits. Formula is
sorted `Sym:count` joined by `|`. Forces/velocities from declared `sections`
or per-atom data.

## Ingest path

1. Path / CON text: `ConFrameIterator::next_with_raw_span` when possible.
2. **In-memory** `append_trajectory_frames` / `extend_trajectory_frames` for chemfiles → corpus.
3. Parse once for indexes (natoms, symbols, composition, energy, fmax, flags).
4. Store blob; compute **xxHash3-128**; update secondary B-trees.

## Reindex

`ConCorpus::reindex` / CLI `readcon-db reindex` clears secondary DBs and rebuilds
from authoritative `frames` (schema evolution without delete+re-ingest).

## Selection and query costs

Postings lists from secondary DBs are intersected in-process (`BTreeSet`):

| Predicate | Index | Cost sketch |
|-----------|-------|-------------|
| `exact_hash` | `frame_by_hash` | Point lookup |
| `require_symbol` | `idx_symbol` | Prefix walk for that element |
| `element_exact` / `element_min` | `idx_elem_count` | Prefix + count filter |
| `exact_composition` | `idx_formula` | Prefix on canonical formula |
| `natoms_range` | `idx_natoms` | Ordered scan, early stop |
| `energy_range` | `idx_energy` | Ordered scan on finite energies |
| `fmax_range` | `idx_fmax` | Ordered scan (no forces ⇒ not indexed) |
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
     optional readcon-db-mpi.h (caller MPI_Comm only)
            │
            ▼
   Rust ConCorpus (Heed + readcon-core)
```

Fortran uses `bind(C)` wrappers under `fortran/ReadConDb/`. Prebuilt
`libreadcon_db` is `readcon-db-clib-$VERSION-$target.tar.gz` on the
GitHub Release ([install](install.md)).
