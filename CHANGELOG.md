# Changelog

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
