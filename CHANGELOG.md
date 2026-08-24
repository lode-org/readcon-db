# Changelog

## Unreleased

### Tests
- Cargo tests lock CON-text authority for cooked RCSO and cover
  numeric extract (positions, forces, velocities) on parse and cooked paths

### Features
- Batched RCSO pack (`RCSB`): `pack_frames` / `rkrdb_pack_frames` / `bcast_packed_frames` on the caller comm
- `readcon-db drain <local_root> <pfs_root>` compact-snapshots `data.mdb`
  (no `lock.mdb`) after node-local ingest and refuses dest overwrite
- Callers write units (`append_trajectory(..., units=)`,
  `extend_trajectory(..., units=)`, `set_units`, CLI `--units`,
  C `rkrdb_append_trajectory_units` / `rkrdb_extend_trajectory_units` /
  `rkrdb_set_units`, Fortran `db_append_units` / `db_extend_units` /
  `db_set_units`);
  aliases are canonicalized into CON metadata (`A` → `angstrom`).
  `set_units` converts stored numbers; append/extend stamp incoming
  values.
- compact-join uses `open_existing`; join-drained checks traj collisions
  before creating dest. `pbc` false writes H5MD boundary `none`.
- Fortran `db_get_positions` / `db_get_forces` / `db_get_velocities`.
- `join_drained_roots` requires `shards.json`; `open_shard_for_traj`
  writes the manifest so drain can run.
- `rkrdb_get_velocities` matches `get_forces` (`out_has_velocities`; no
  throw on absence). Drain refuses dest shards before writing `shards.json`.
- C/C++/Fortran `rkrdb_h5md_species` copies collect integer Z; Corpus
  wraps `get_positions` / `get_forces` / `get_velocities`.
- H5MD export uses one engine unit system (Å, ps, kJ mol^{-1} Å^{-1});
  CON units convert through `unit_conversion_factor`. Python exposes that
  function. Time is CON time or `i * timestep`, else the frame index, in
  dest `ps`. Missing `units.time` is CON default `fs` (not dest `ps`).
- Edges from `lattice_vectors` or boxl+angles; author/creator/boundary
  attrs are fixed ASCII. Physical `unit` attrs are short H5MD strings
  so MDAnalysis 2.10 `convert_units=True` can index them.
- `join_to_single_env` / `compact-join` / `join_corpus_dirs` refuse dest overwrite
- `join-drained` merges unique-dest drained roots when shard ids overlap
- `drain_to` compact-snapshots `data.mdb` only and refuses dest overwrite
- Python `export_h5md` writes H5MD 1.1 interchange via h5py (CON stays authority):
  `/h5md` author/creator, `position/value` `[T][N][3]`, `position/step` and
  `position/time` `[T]`, `box/edges/value` `[T][3][3]`, integer-Z species,
  CON `pbc` on `box/boundary`. Mixed-force frames pad zeros. CON
  velocities write `particles/all/velocity` `[T][N][3]` dest Å/ps.
  `ConCorpus::collect_h5md` owns the arrays. `extend_trajectory` is on
  Rust, Python, C (`rkrdb_extend_trajectory_units`), C++, and Fortran.
- MPI pack/Bcast helper takes the **caller communicator** (`include/readcon-db-mpi.h`,
  Python `bcast_packed_frame(comm, ...)`, Fortran pack + `MPI_Bcast` on the INTEGER
  handle). Never `MPI_Init` if the host already did; never names the process-wide
  world communicator inside the helper. LAMMPS / mpi4py pass the comm they own.

## 0.1.4 - 2026-08-15

### Packaging
- `python/pyproject.toml` ships the repo README so `twine check --strict` accepts the sdist

### Continuous integration
- Wheel workflow filename is `python-wheels.yml` (PyPI trusted publisher)

## 0.1.3 - 2026-08-15

### Packaging
- CMake FetchContent / `find_package(readcon-db)` and Meson wrap without cbindgen
- cargo-c metadata: shipped `include/readcon-db.h`, `readcon-db.pc`
- cxx source tarball (`readcon-db-cxx-0.1.3.tar.gz` and `-vendor`) on the GitHub Release

## 0.1.2 - 2026-06-28

### Documentation
- Install docs reflect crates.io / PyPI / Pages (no path-only wording)
- Package metadata: homepage and documentation URLs

### Continuous integration
- CI (test, clippy, maturin smoke), crates.io publish workflow, Python wheels + PyPI on tags

### Packaging
- Align `python/pyproject.toml` version with crate **0.1.2**

## 0.1.1 - 2026-06-28
### Features
- Optional RCSO cooked SoA tier (`frames_soa`) with cook/delete/numeric getters
- C/Python/CLI/Fortran exposure for cook and positions/forces fast path
- User docs: `docs/orgmode/cooked-soa.org` (CON authority; RCSO not fully equivalent)

### Documentation
- Dual-tier rules in architecture Sphinx summaries and README pointer


## 0.1.0 — 2026-06-27

### Added
- Heed/LMDB `ConCorpus` with trajectory ingest, SoA-agnostic CON text blobs via `readcon-core`
- Secondary indexes: `idx_natoms`, `idx_symbol`
- **xxHash3-128** exact content identity: `frame_by_hash` / `hash_by_frame`, `Select::exact_hash`, `find_by_hash`
- C ABI (`rkrdb_*`) in `cdylib`/`staticlib`; `include/readcon-db.h` with C++ `readcon_db::Corpus`
- Optional PyO3 module (`--features python`), maturin project under `python/`
- Fortran `bind(C)` module under `fortran/ReadConDb`
- Example `ingest_select`
- **CLI** `readcon-db` (`ingest`, `ingest-dir`, `select`, `dedup-export`, `hash-file`)
- **`export_extxyz`** / **`ingest_directory`** / **`unique_frame_keys`** for metatrain-style pipelines
- Workflow docs: `examples/workflows/metatrain_from_con.md` + YAML snippet
- Sphinx docs, marketing `website/`, logo kit under `assets/logo/`
