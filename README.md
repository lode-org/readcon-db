# readcon-db

**Mmap-backed CON/convel corpus store** (LMDB via [Heed](https://github.com/meilisearch/heed)), **non-SQL selection**, **xxHash3-128 exact match**, and **Rust / C / C++ / Python / Fortran** bindings.

Part of the **readcon ecosystem** with [`readcon-core`](https://github.com/lode-org/readcon-core) (Python package **`readcon`**):

| Crate / package | Role | Docs |
|-----------------|------|------|
| **[readcon-core](https://github.com/lode-org/readcon-core)** / **`readcon`** | CON interchange (parse/write/spec v2–v3). **XYZ/PDB/GRO → `ConFrame` via chemfiles** (`read_chemfiles*`), not ASE. Optional `to_ase` only for calculators. | Core README, `docs/orgmode/` |
| **readcon-db** / **`readcon_db`** (this repo) | mmap CON corpus: indexes (natoms, symbols, **energy range**, **forces/velocities/energy flags**), multi-reader, dedup. Blobs are **CON text** decoded with readcon-core. | [docs](https://lode-org.github.io/readcon-db/docs/), [`docs/orgmode/explanation/design.org`](docs/orgmode/explanation/design.org) |

ASE is not on the CON or XYZ read path. Optional `to_ase` is calculator hand-off only.

## Install

```bash
cargo add readcon-db
cargo install readcon-db --locked   # CLI
pip install readcon-db             # module readcon_db (PyPI)
# C/C++: FetchContent / meson dependency('readcon-db') / pkg-config
# headers in include/ are shipped; cbindgen is not required
# Prebuilt C ABI (no cargo): readcon-db-clib-$VER-$target.tar.gz on the GitHub Release
```

Docs: <https://lode-org.github.io/readcon-db/> · crate: <https://crates.io/crates/readcon-db>

## Quick start (from source)

```bash
git clone https://github.com/lode-org/readcon-db
cd readcon-db
cargo test --locked
cargo build --release   # libreadcon_db + CLI readcon-db
```

Optional LODE sibling checkout (edit core + db together): clone both under the same parent, then create **untracked** `.cargo/config.toml` in `readcon-db`:

```toml
[patch.crates-io]
readcon-core = { path = "../readcon-core" }
```

Python extension from a checkout (`python/` + maturin):

```bash
pip install maturin
maturin develop --release --features python --manifest-path python/pyproject.toml
```

```rust
use readcon_db::{ConCorpus, Select};
let db = ConCorpus::open("/tmp/corpus")?;
db.append_trajectory_path(1, "run.con")?;
// XYZ in: use readcon-core chemfiles → ConFrame → append (see workflows)
let keys = db.select(
    &Select::new()
        .require_symbol("Cu")
        .require_forces()
        .exact_composition("Cu:2|H:2")
        .fmax_range(0.0, 1.0)
        .energy_range(-50.0, 0.0),
)?;
let h = db.frame_hash(keys[0])?;
```

```bash
./target/release/readcon-db ingest-dir /tmp/corpus /path/to/con_files
./target/release/readcon-db select /tmp/corpus --formula 'Cu:2|H:2' --require-forces \
    --fmax-max 1.0 --energy-min -50 --energy-max 0
./target/release/readcon-db reindex /tmp/corpus
./target/release/readcon-db dedup-export /tmp/corpus --symbol Cu -o subset.xyz  # only if a tool demands XYZ on disk
```

Foreign trajectories: **`readcon.read_chemfiles("traj.xyz")` → frames → ingest into readcon-db** (chemfiles-enabled build), not `ase.io.read`.

`append_trajectory`, `append_trajectory_str`, and `append_trajectory_frames` each commit the whole trajectory once. A failure aborts that transaction, so no frame from it is visible. `append_trajectory_frames` still serializes 64 frames at a time and drops that group after the put, so the scratch stays one group. `get_frame` returns a `StoredFrame` (`atoms[i]` builds that atom only). `select` returns keys. `read_arrays` copies the cooked columns into NumPy. Gzip and zstd files are read with `readcon.read_con`, then appended. Python `append_trajectory` and Rust `append_trajectory_path` read UTF-8 and do not decompress. `readcon-db` does not register an `ase.ioformats` entry point. Guide: [`docs/orgmode/howto/streaming.org`](docs/orgmode/howto/streaming.org). The CLI tutorial is [`docs/orgmode/tutorials/tutorial.org`](docs/orgmode/tutorials/tutorial.org). Upgrade from 0.2.0: [`docs/orgmode/howto/migration.org`](docs/orgmode/howto/migration.org).

## Design

- **No SQL engine** — explicit indexes + in-process intersection, with **ASE.db-competitive screening fields** (mass, volume, PBC, reserved metadata, charge/magmom; see [design matrix](docs/orgmode/explanation/design.org)).
- **Decode via readcon-core** — CON semantics never fork.
- **Metadata indexes** — finite `energy` bins; flags for forces, velocities, energy presence.
- **xxHash3-128** on the CON text. A new `frames` value is `0xFD 0x01` plus one zstd level-1 frame of that UTF-8, written outside the LMDB transaction. An aligned run of 64 frames shares one `0xFD 0x03` zstd frame when that group is shorter than the per-frame values, and the other keys store a `0xFD 0x02` redirect. Untagged UTF-8 still reads as a 0.1.8 span. `readcon-db migrate` rewrites legacy values once and leaves an already packed value unchanged. `--no-compress` on `ingest`, `ingest-dir`, and `shard-ingest` stores the UTF-8. There is no dictionary. Both MPI ingest routes send the stored bytes. The hash is the decompressed text.
- **Many readers, one writer** (LMDB). Same-frame MPI: rank 0 of the
  **caller communicator** packs RCSO and `MPI_Bcast` on that handle
  (`include/readcon-db-mpi.h`, Python `bcast_packed_frame` /
  `bcast_packed_frames`). The library never `MPI_Init`s and never names
  the process-wide world communicator; LAMMPS / mpi4py pass the comm they
  already own.
- **H5MD interchange** — `export_h5md` / `collect_h5md` writes one
  `[T][N][3]` trajectory (CON stays authority). Engine dest is Å / ps /
  kJ mol^{-1} Angstrom^{-1}; velocity dest is `Angstrom ps-1`.
  Velocities follow D10: v1, v2, and v3.0 frames use the natural unit
  `(amu*angstrom^2/eV)^0.5` whatever the time label says; v3.1 frames use
  `units.time` and fail when a velocities section has no `units.time`.
  `--velocity-time-unit`, Rust `H5mdCollectOpts`, the Python
  `velocity_time_unit` argument, and C `rkrdb_set_velocity_time_unit`
  override that rule for a file that breaks it.
  The simulation clock uses `units.time` when that key is present.
  A v3.1 frame with `time` or `timestep` and no `units.time` is an error.
  The same override supplies that missing clock unit.
  A present `units.time` still scales the clock.
  v1, v2, and v3.0 clocks with no `units.time` are read as `fs`.
- **Node-local drain/join** — `shard-ingest` then `drain` to a unique
  dest (`data.mdb` only, refuse overwrite), then `join-drained`.
  `compact-join` joins one sharded root (`open_existing`). Campaign
  ops: [`docs/orgmode/howto/campaign.org`](docs/orgmode/howto/campaign.org).

Full ABI table, logo, Sphinx docs, and site: see `docs/`, `website/`, `assets/logo/`, `CHANGELOG.md`. Fortran module notes: `fortran/README.md`, `fortran/ReadConDb/`.

## Benchmarks

ASE sqlite is a measurement target, not a storage path. The fair
comparison is
[`examples/benchmarks/fair_ase_protocol.py`](examples/benchmarks/fair_ase_protocol.py).
Why the published CPC ingest is not a like-for-like timing, and the
rules this tree uses instead, are in
[`examples/benchmarks/FAIRNESS.md`](examples/benchmarks/FAIRNESS.md).
Raw samples are
[`examples/benchmarks/results/fair-db29/campaign.log`](examples/benchmarks/results/fair-db29/campaign.log).

Both stores get the same records: symbols, positions, cell,
periodicity, masses, forces, energy, and `frame_index`. ASE is SQLite
with `journal_mode=DELETE` and `synchronous=FULL`. Each insert is one
transaction and one commit fsync, on a database file created for that
sample. One warmup is discarded. Six serial samples alternate the two
tools. The summary is the Hyndman–Fan type-7 median and IQR. Every
stored field is checked by its float64 bytes; a mismatch aborts the run.

`open` reads a corpus under 1 MiB with one `pread` before it returns.
Selected cold is the query after that prefetching open. `cold-open`
drops the cache outside the clock and times the open together with
the query, for both tools. The harness does not read the file.
`/proc/sys/vm/drop_caches` was not writable on the measurement host.
A warm read is the next read on the already open store. The timed
close does not call `gc.collect()`. Full reads and the selected frame
copy the same materialized columns. The single-frame query is
`frame_index` of the middle frame. The rules are in `FAIRNESS.md`.

The published CPC log (`paper/cpc/logs/db.log`, 40 frames of 8 Cu, no
warmup) is readcon-db 4.67 ms and ASE 72.3 ms (IQR 37.7 ms). ASE
committed and fsynced once per frame.

<!-- fair-docs:begin readme-fair -->
Type-7 medians from `examples/benchmarks/results/fair-db29/campaign.json`, in seconds. n is the stored sample count.

| workload | op | cache | readcon-db | ASE sqlite | faster |
| --- | --- | --- | --- | --- | --- |
| 10^3 atoms, 1 frame | full read | warm | 1.6555e-05 (n=6) | 7.9115e-04 (n=12) | readcon-db |
| 10^3 atoms, 1 frame | full read | cold | 3.1883e-05 (n=6) | 2.0304e-03 (n=12) | readcon-db |
| 10^4 atoms, 1 frame | full read | warm | 2.7152e-04 (n=6) | 4.3440e-03 (n=6) | readcon-db |
| 10^4 atoms, 1 frame | full read | cold | 2.9294e-04 (n=6) | 5.2235e-03 (n=12) | readcon-db |
| 10^5 atoms, 1 frame | full read | warm | 2.5745e-03 (n=6) | 3.4530e-02 (n=12) | readcon-db |
| 10^5 atoms, 1 frame | full read | cold | 6.2759e-03 (n=12) | 3.3129e-02 (n=6) | readcon-db |
| 10^6 atoms, 1 frame | full read | warm | 2.2889e-02 (n=6) | 3.0143e-01 (n=6) | readcon-db |
| 10^6 atoms, 1 frame | full read | cold | 6.7073e-02 (n=12) | 3.5340e-01 (n=12) | readcon-db |
| 10^6 atoms, 1 frame | one frame | cold | 6.7658e-02 (n=12) | 3.5220e-01 (n=6) | readcon-db |
| 10^6 atoms, 1 frame | insert | cold-file | 3.0821e-01 (n=6) | 2.5567e-01 (n=10) | ASE |
| 8 atoms, 10^4 frames | full read | warm | 6.3478e-03 (n=12) | 1.0671e+00 (n=6) | readcon-db |
| 8 atoms, 10^4 frames | full read | cold | 6.4131e-03 (n=6) | 1.0325e+00 (n=6) | readcon-db |
| 8 atoms, 10^4 frames | one frame | cold | 4.6303e-05 (n=6) | 5.1330e-03 (n=12) | readcon-db |
| 8 atoms, 10^4 frames | insert | cold-file | 2.6721e-01 (n=6) | 3.0784e+00 (n=6) | readcon-db |

ASE is faster on the 10^4-atom, 10^5-atom, and 10^6-atom inserts (10^4 atoms, 1 frame: readcon-db 3.8324e-03 (n=6), ASE 3.6268e-03 (n=12); 10^5 atoms, 1 frame: readcon-db 2.9094e-02 (n=6), ASE 2.7197e-02 (n=6); 10^6 atoms, 1 frame: readcon-db 3.0821e-01 (n=6), ASE 2.5567e-01 (n=10)).
readcon-db is faster on every full read and single-frame query in this table.
The full ladder is [`examples/benchmarks/results/fair-db29/RESULTS.md`](examples/benchmarks/results/fair-db29/RESULTS.md).
<!-- fair-docs:end readme-fair -->

## Questions and bug reports

Ask questions and report bugs on the [GitHub issue tracker](https://github.com/lode-org/readcon-db/issues).
Report a security problem by email instead, as [`SECURITY.md`](SECURITY.md) describes.

## License

MIT

## Cooked SoA tier

Optional RCSO numerics in `frames_soa` (opt-in cook). Ingest and `migrate` do not cook. RCSO is
non-authoritative: CON text in `frames` is the sole authority for hash,
dedup, join/split, and reindex. User doc:
[`docs/orgmode/cooked-soa.org`](docs/orgmode/cooked-soa.org).
