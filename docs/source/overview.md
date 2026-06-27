# Overview

Long-timescale and NEB campaigns produce **corpora**: many trajectories × many frames. Loading every frame as a heap `ConFrame` exhausts RAM; scanning every text file for “Cu and \(N < 200\)” wastes I/O.

**readcon-db** is the **campaign store** in the [readcon ecosystem](https://github.com/lode-org/readcon-core) (interchange = [readcon-core](https://github.com/lode-org/readcon-core) / Python `readcon`). It treats the corpus as an **embedded key-value database**:

1. **mmap** the LMDB environment — hot pages live in the OS page cache (“disk data in RAM” without a second buffer pool).
2. **Many readers, one writer** — analysis threads open read transactions; ingest is serialized.
3. **Secondary indexes** — atom count ranges (`idx_natoms`), required symbols (`idx_symbol`), **finite energy ranges** (`idx_energy`), **section / capability flags** (`idx_flags`: forces, velocities, energy present), and exact **xxHash3-128** of the stored blob (`frame_by_hash`).
4. **Decode with readcon-core** — CON semantics never fork; metadata keys such as `energy` and declared `sections` are the same constants as in the CON spec.

**Day-to-day path:** CON (or chemfiles→`ConFrame` in core) → ingest CON blobs → `Select` / CLI / `rkrdb_select_meta`. ASE is **not** on the I/O path; optional `to_ase` is only for calculators. ASE `.db` timings in the CPC paper are **unequal-workload CSE baselines**, not a product recommendation.

Selection is an explicit Rust/`Select` builder (or `rkrdb_select_*` / `rkrdb_select_meta` in C), not SQL. See [architecture](architecture.md) for the query-cost model.
