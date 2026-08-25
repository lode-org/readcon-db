# Overview

Long-timescale and NEB campaigns produce **corpora**: many trajectories × many frames. Loading every frame as a heap `ConFrame` exhausts RAM; scanning every text file for “Cu and \(N < 200\)” wastes I/O.

**readcon-db** is the **corpus store** in the [readcon ecosystem](https://github.com/lode-org/readcon-core) (interchange = [readcon-core](https://github.com/lode-org/readcon-core) / Python `readcon`). It treats the corpus as an **embedded key-value database**:

1. **mmap** the LMDB environment — hot pages live in the OS page cache (“disk data in RAM” without a second buffer pool).
2. **Many readers, one writer** — analysis threads open read transactions; ingest is serialized.
3. **Secondary indexes** — atom counts (`idx_natoms`), symbols (`idx_symbol`), **per-element counts** (`idx_elem_count`), **exact formula** (`idx_formula`, e.g. `Cu:2|H:2`), **finite energy** (`idx_energy`), **fmax** (`idx_fmax` when forces exist), **section flags** (`idx_flags`), and **xxHash3-128** (`frame_by_hash`). **Reindex** rebuilds all secondary DBs from authoritative blobs. ASE.db **screening columns** (mass, volume, PBC, time/frame_index/NEB, charge/magmom) are indexed when CON-derivable—see `docs/design.md` matrix. Not a SQL engine; not ASE bookkeeping (`user`, `ctime`, calculator).
4. **Decode with readcon-core** — CON semantics never fork; metadata keys such as `energy` and declared `sections` are the same constants as in the CON spec.

**Day-to-day path:** CON (or chemfiles→`ConFrame` in core) → ingest CON blobs → `Select` / CLI / `rkrdb_select_meta`. ASE is **not** on the I/O path; optional `to_ase` is only for calculators. The CPC manuscript is the readcon-core paper; this crate is the companion campaign store, not a second claim. ASE `.db` timings, if the paper includes an appendix table, are the **frozen fair campaign** (`paper/cpc/freeze/`), not a product recommendation and not the legacy Cu2 unequal-workload bench.

Selection is an explicit Rust/`Select` builder (or `rkrdb_select_*` / `rkrdb_select_meta` in C), not SQL. See [architecture](architecture.md) for the query-cost model.

**H5MD interchange:** `export_h5md` / `collect_h5md` write one `[T][N][3]`
trajectory (CON stays authority). **Drain/join:** node-local `shard-ingest`,
`drain` compact-snapshots `data.mdb`, then `join-drained`. `compact-join`
is the single-root join. Ops: [campaign](campaign.md).


**Benchmarks:** fair ASE comparison → `examples/benchmarks/fair_campaign.py` (same CON ladder; not Cu2 stand-ins). Paper appendix freeze: `paper/cpc/freeze/` ([CPC companion](cpc.md)).
