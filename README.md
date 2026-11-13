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

Python `append_trajectory_frames` commits 64 frames at a time. `get_frame` returns a `StoredFrame` (`atoms[i]` builds that atom only). Gzip and zstd files are read with `readcon.read_con`, then appended. Python `append_trajectory` and Rust `append_trajectory_path` read UTF-8 and do not decompress. `readcon-db` does not register an `ase.ioformats` entry point. Guide: [`docs/orgmode/howto/streaming.org`](docs/orgmode/howto/streaming.org). Upgrade from 0.2.0: [`docs/orgmode/howto/migration.org`](docs/orgmode/howto/migration.org).

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
[`examples/benchmarks/results/fair/campaign.log`](examples/benchmarks/results/fair/campaign.log).

Both stores get the same records: symbols, positions, cell,
periodicity, masses, forces, energy, and `frame_index`. ASE is SQLite
with `journal_mode=DELETE` and `synchronous=FULL`. Each insert is one
transaction and one commit fsync, on a database file created for that
sample. One warmup is discarded. Six serial samples alternate the two
tools. The summary is the Hyndman–Fan type-7 median and IQR. Every
stored field is checked by its float64 bytes; a mismatch aborts the run.

A cold read calls `posix_fadvise(DONTNEED)` on the closed database
files, then times a readonly open plus the read.
`/proc/sys/vm/drop_caches` was not writable on the measurement host.
A warm read is the next read on the already open store. The timed
close does not call `gc.collect()`. Full reads use `read_columns`.
The single-frame query is `frame_index` of the middle frame.

The published CPC log (`paper/cpc/logs/db.log`, 40 frames of 8 Cu, no
warmup) is readcon-db 4.67 ms and ASE 72.3 ms (IQR 37.7 ms). ASE
committed and fsynced once per frame. On the fair harness that same
40×8 insert, one commit each, is readcon-db 1.97 ms (IQR 0.70 ms) and
ASE 6.52 ms (IQR 1.15 ms).

Type-7 medians from the fair ladder on that host, in seconds:

| workload | op | cache | readcon-db | ASE sqlite |
| --- | --- | --- | --- | --- |
| 10³ atoms, 1 frame | full read | warm | 2.17e-4 | 4.81e-4 |
| 10³ atoms, 1 frame | full read | cold | 4.24e-4 | 9.96e-4 |
| 10⁴ atoms, 1 frame | full read | warm | 1.22e-3 | 2.33e-3 |
| 10⁴ atoms, 1 frame | full read | cold | 1.49e-3 | 3.16e-3 |
| 10⁵ atoms, 1 frame | full read | warm | 1.60e-2 | 3.53e-2 |
| 10⁵ atoms, 1 frame | full read | cold | 1.59e-2 | 2.84e-2 |
| 10⁶ atoms, 1 frame | full read | warm | 1.77e-1 | 2.40e-1 |
| 10⁶ atoms, 1 frame | full read | cold | 1.78e-1 | 2.82e-1 |
| 10⁶ atoms, 1 frame | one frame | cold | 1.80e-1 | 2.83e-1 |
| 8 atoms, 10⁴ frames | full read | warm | 1.12e-1 | 9.62e-1 |
| 8 atoms, 10⁴ frames | full read | cold | 1.21e-1 | 9.77e-1 |
| 8 atoms, 10⁴ frames | one frame | cold | 5.67e-4 | 3.56e-3 |
| 10⁶ atoms, 1 frame | insert | cold-file | 1.65e-1 | 2.40e-1 |
| 8 atoms, 10⁴ frames | insert | cold-file | 3.02e-1 | 3.28e0 |

readcon-db is ahead on every full read, every single-frame query, and
every insert in that log, warm and cold. A text above 512 KiB is indexed
from the stored span, without one object per atom; the 10⁶-atom insert
was 0.349 s before that and is 0.165 s after, against ASE at 0.240 s.
Texts at or below 512 KiB still take the full parse. The one-frame
insert median moved from 0.315 ms to 0.400 ms, which is past the
previous IQR by about 0.05 ms, from a single 1.0 ms fsync sample; the
other five samples are 0.30–0.54 ms. The full ladder, including the
before column, is
[`examples/benchmarks/results/fair/RESULTS.md`](examples/benchmarks/results/fair/RESULTS.md).
Select-count parity on a shared CON ladder remains
`examples/benchmarks/fair_campaign.py`.

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
