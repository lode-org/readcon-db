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
