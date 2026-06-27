# Overview

Long-timescale and NEB campaigns produce **corpora**: many trajectories × many frames. Loading every frame as a heap `ConFrame` exhausts RAM; scanning every text file for “Cu and \(N < 200\)” wastes I/O.

**readcon-db** treats the corpus as an **embedded key-value database**:

1. **mmap** the LMDB environment — hot pages live in the OS page cache (“disk data in RAM” without a second buffer pool).
2. **Many readers, one writer** — analysis threads open read transactions; ingest is serialized.
3. **Secondary indexes** — atom count ranges, required symbols, optional exact **xxHash3-128** of the stored blob.
4. **Decode with readcon-core** — CON semantics never fork.

Selection is an explicit Rust/`Select` builder (or `rkrdb_select_*` in C), not SQL.
