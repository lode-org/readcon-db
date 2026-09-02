# Changelog

## v0.1.6 - 2026-09-02
#### Benchmarks
- (**cpc**) restamp the fair campaign freeze with host and commit - (4a85c51) - *HaoZeke*
- (**cpc**) stamp host, date, and commit on the fair campaign JSON - (fdd5697) - *HaoZeke*
#### Features
- add topology-key secondary index - (0963b8d) - *HaoZeke*
#### Bug Fixes
- (**docs**) keep the hero snippet on one raw-html line - (94de5d9) - *HaoZeke*
- annotate TrajMeta in commit_prepared - (e7f209e) - *HaoZeke*
#### Documentation
- document topology keys and the kinetic ART catalogue test - (79c8542) - *HaoZeke*
- drop the CPC positioning page from the public site - (84bc2b1) - *HaoZeke*
- restore sibling docs sites in Ecosystem - (d4376cf) - *HaoZeke*
- Ecosystem is only the sibling docs site - (face455) - *HaoZeke*
- Ecosystem nav lists docs sites only - (1964885) - *HaoZeke*
- switch Pages to Shibuya with Diátaxis pages like core - (c944f98) - *HaoZeke*
- drop the second-crate card copy from the landing - (9b24526) - *HaoZeke*
- export the landing from org and point Docs at /docs/ - (c32f07c) - *HaoZeke*
#### Tests
- break the Cu-Cu cutoff bond in the topology identity check - (2b2bee4) - *HaoZeke*
#### Style
- (**lychee**) apply taplo format - (98016d2) - *HaoZeke*



## v0.1.5 - 2026-08-24
#### Features
- (**abi**) rkrdb_get_frame returns a parsed ConFrame - (3802c6f) - *HaoZeke*
- (**archive**) appended counter distinct from committed - (2db2a34) - *HaoZeke*
- (**archive**) async fixed-composition observation ledger with C ABI - (384c95b) - *HaoZeke*
- (**bindings**) ingest CON text and frames without a temp file - (030d5ce) - *HaoZeke*
- (**bindings**) ingest CON text and frames without a temp file - (c25da0c) - *HaoZeke*
- (**capi**) rkrdb_extend_trajectory creates or appends frames - (c865a19) - *HaoZeke*
- prebuilt libreadcon_db C ABI and campaign ops runbook - (6640266) - *HaoZeke*
- export Fortran rkrdb_not_found next to ok/err - (b8b25ae) - *HaoZeke*
- Fortran get_positions and get_forces next to velocities - (1f8bf9b) - *HaoZeke*
- H5MD species C collect and test velocity/force getters - (f3a15dc) - *HaoZeke*
- H5MD velocities; extend-units tests; C++ frame_units - (72d7898) - *HaoZeke*
- Fortran db_extend and db_extend_units - (d68a265) - *HaoZeke*
- C++ and Fortran h5md_shape and h5md_positions - (7dd2cc7) - *HaoZeke*
- C h5md_shape and [T][N][3] positions from collect_h5md - (b8a2b52) - *HaoZeke*
- Fortran db_h5md_times and db_append_units on the public module - (4597380) - *HaoZeke*
- C collect_h5md times; use header.timestep for dest ps - (759786c) - *HaoZeke*
- stamp caller units from the C, C++, and Fortran ABI - (47ca812) - *HaoZeke*
- stamp caller units into CON metadata as canonical names - (d44bf20) - *HaoZeke*
- batched RCSO pack, H5MD export, and shard drain - (2444895) - *HaoZeke*
- MPI Bcast on the caller sub-communicator - (2014d60) - *HaoZeke*
- read-only corpus open and RCSO pack for MPI_Bcast - (0345946) - *HaoZeke*
#### Bug Fixes
- (**cxx**) write the wrapdb wrap only for the slim tarball - (f71d189) - *HaoZeke*
- (**drain**) make dest_man copy test hook thread-local - (a967240) - *HaoZeke*
- (**drain**) dest_was_new rollback covers dest_man copy and parse - (e001158) - *HaoZeke*
- (**rcsb**) name the batch envelope in decode_batch errors - (7f559f9) - *HaoZeke*
- dest_was_new drain rollback; dest force after set_units - (874596d) - *HaoZeke*
- dest xyz on T>1 collect and join; dest force after set_units - (fe4ee84) - *HaoZeke*
- drain rolls back failed dest; dest xyz on item 1 - (8823654) - *HaoZeke*
- collect_h5md prefers stored RCSO; leftover pack tests - (637292e) - *HaoZeke*
- drop leftover 1 MiB pack caps; size-query examples - (1955119) - *HaoZeke*
- pack_frame size query; bcast no 1 MiB cap - (286663d) - *HaoZeke*
- pack_frames unpack; readonly export; 1-frame MPI batch - (fb0ed3d) - *HaoZeke*
- sharded select/join are readonly; shard-ingest routes first - (77c8d7f) - *HaoZeke*
- rollback join dest; docs dest refuse; CLI compact-join - (e2b9737) - *HaoZeke*
- unlink extxyz dest; no mint on empty shard or CLI - (10aaa05) - *HaoZeke*
- join preview is per-traj; extxyz dest/pbc/lattice - (eaa6cf4) - *HaoZeke*
- refuse dest mutation; 17-digit set_units; H5MD dest checks - (b362d3e) - *HaoZeke*
- compact-join open_existing; none boundary; dest positions - (8ce6d48) - *HaoZeke*
- join-drained requires shards.json before dest create - (52b12b2) - *HaoZeke*
- drain refuse before shards.json; velocity has-flag - (c72573f) - *HaoZeke*
- MDA velocity unit Angstrom ps-1; close H5MD File; get_velocities - (53ccc20) - *HaoZeke*
- drop stale indexes on set_units; stamp units on ingest_directory - (7c27388) - *HaoZeke*
- ingest-dir units, named dest-refuse test, Fortran batch pack - (8e67552) - *HaoZeke*
- write H5MD physical unit attrs as strings MDA can index - (8f37004) - *HaoZeke*
- CON time default fs; set_units converts; stamp units on extend/CLI - (e2a8f2d) - *HaoZeke*
- store H5MD string attrs as fixed-width Unicode for MDA - (0a600f5) - *HaoZeke*
- write H5MD unit attrs as fixed utf-8 so MDA can read them - (c537919) - *HaoZeke*
- BARMA-4 nits for unit attrs, force scale, and docs - (c480263) - *HaoZeke*
- H5MD engine units are always Angstrom, ps, kJ/mol/A - (e138152) - *HaoZeke*
- H5MD force scale via SI unit_conversion_factor - (2749926) - *HaoZeke*
- convert H5MD units with core unit_conversion_factor - (bd92d3e) - *HaoZeke*
- FrameHeader type name for readcon-core 0.14 - (bcb2ff1) - *HaoZeke*
- BARMA-3 nits for H5MD, join overwrite, and docs - (84b5561) - *HaoZeke*
- H5MD time and MDA force units; join-drained overlapping shards - (fa63d61) - *HaoZeke*
- H5MD 1.1 collector with units, CON pbc, and force pad - (b2f6810) - *HaoZeke*
- heed 0.21 copy_to_file takes a path - (60927e9) - *HaoZeke*
- snapshot via heed copy_to_file - (811602f) - *HaoZeke*
- H5MD 1.1 interchange and compact shard drain - (d482bd2) - *HaoZeke*
- H5MD export PyO3 dataset kwargs lifetime - (e6ee917) - *HaoZeke*
#### Documentation
- (**brand**) stacked CON frame, pair with readcon-core - (2e978bf) - *HaoZeke*
- (**cpc**) position companion store and freeze fair campaign table - (f10e294) - *HaoZeke*
- add SECURITY, CONTRIBUTING, and Code of Conduct - (d140038) - *HaoZeke*
- publish objects.inv at the site root - (0d91bf4) - *HaoZeke*
- chemfiles skip/nth and line-2 units on ingest path - (95ab97f) - *HaoZeke*
- workflows drain comment says join-drained - (7b30cf2) - *HaoZeke*
#### Tests
- (**ci**) gate language-package versions and the core pin - (92bcfdc) - *HaoZeke*
- (**clib**) unpack gate requires the campaign CLI - (8f2ec29) - *HaoZeke*
- (**cooked**) valid RCSO must skip CON parse on getters - (ce1c7b2) - *HaoZeke*
- (**drain**) dest leftover dest_man copy dest_was_new dest - (3693da2) - *HaoZeke*
- (**drain**) register dest_was_new rollback cases as tests - (88f3456) - *HaoZeke*
- (**h5md**) C dest time 0.0125 after set_units time - (a9f8b7e) - *HaoZeke*
- (**h5md**) dest vel, time, edges, and force stay after set_units - (49aff10) - *HaoZeke*
- (**h5md**) dest time after set_units stays 0.0125 ps - (47f4016) - *HaoZeke*
- lock CON authority for cooked SoA in CI - (53683ff) - *HaoZeke*
- dest-A positions, ingest_directory units, i*timestep on H5MD - (29f5674) - *HaoZeke*
- compare H5MD unit attrs after decoding fixed ASCII - (ecfeb08) - *HaoZeke*
- MDA H5MDReader convert_units with fixed-ASCII unit attrs - (584ee00) - *HaoZeke*
- decode fixed-ASCII H5MD attrs in export checks - (9bc1ae3) - *HaoZeke*
- H5MD export matches on CON fallback and after RCSO cook - (33894d2) - *HaoZeke*
- drain dest reopens and compact-join matches membership - (be8ee32) - *HaoZeke*
- assert compact drain dest stays far below the 2 GiB map - (bcd120f) - *HaoZeke*
#### CI
- (**docs**) gate the CPC fair-campaign freeze - (ed349ea) - *HaoZeke*
- (**docs**) lock Sphinx toolchain with pixi - (6bbd0ae) - *HaoZeke*
#### Chores
- (**deps**) pin readcon-core to 0.14.7 - (1646f26) - *HaoZeke*
#### Style
- (**h5md**) rename dest-force binding that codespell flags - (ebeb5da) - *HaoZeke*
- apply prek format and ignore codespell fo - (cf19430) - *HaoZeke*
- drop unused FrameKey import in collect_h5md - (3765955) - *HaoZeke*

- - -

## v0.1.4 - 2026-08-15
#### Maintenance
- bump to v0.1.4 - (a31f192) - *HaoZeke*
#### Bug Fixes
- (**ci**) rename wheel workflow to python-wheels.yml - (c126648) - *HaoZeke*
- (**python**) ship README metadata so twine --strict accepts the sdist - (c376665) - *HaoZeke*

- - -

## v0.1.3 - 2026-08-15
#### Buildsystem
- add prek, cog, lychee, and docs CI - (5153f0a) - *HaoZeke*
#### Documentation
- (**bench**) refresh fair campaign provenance from re-run - (4bef664) - *HaoZeke*
- add the release checklist and lock Sphinx to 0.1.3 - (d4234e4) - *HaoZeke*
- CMake FetchContent, Meson wrap, and shipped C header - (4c56bca) - *HaoZeke*
#### Maintenance
- bump to v0.1.3 - (5a31bca) - *HaoZeke*
- bump to v0.1.3 - (cee1efb) - *HaoZeke*
#### Features
- (**cxx**) CMake FetchContent and Meson wrap without cbindgen - (72c6be4) - *HaoZeke*
- (**dist**) cxx source tarball, wrapdb wrap, and cargo-c metadata - (3cd0671) - *HaoZeke*
#### Bug Fixes
- (**ci**) build wheels on native runners like featomic - (e2d5f4c) - *HaoZeke*
- (**ci**) stop applying Linux bfd RUSTFLAGS on macOS wheels - (4f733ba) - *HaoZeke*
- (**ci**) drop large-file-auditor from lint - (3f93e96) - *HaoZeke*
- (**ci**) check cog from the latest tag and emit -L in the meson .pc - (ca9104e) - *HaoZeke*
- (**ci**) make prek.toml taplo-safe - (81c92d0) - *HaoZeke*
- (**ci**) run maturin from python/ for wheels workflow - (892135c) - *HaoZeke*
- (**ci**) depend on readcon-core from crates.io only - (d5c7a70) - *HaoZeke*
- (**cmake**) pass --lib to cargo rustc for the C ABI - (73268ec) - *HaoZeke*
- (**cxx**) C-only CMake, slim/vendor tarballs, dual-link docs - (4e04496) - *HaoZeke*
- (**meson**) rename cargo_features; rust_features is reserved - (a7c9d6c) - *HaoZeke*
#### Tests
- (**hpc**) multi-shard writers match single-env select baseline - (59de66b) - *HaoZeke*
- vendor CON fixtures so CI does not need sibling core - (9e7e873) - *HaoZeke*
#### CI
- (**cxx**) do not fail the job if cbindgen happens to be on PATH - (46b165d) - *HaoZeke*
- gate on cargo test/clippy/docs only - (c00d02e) - *HaoZeke*
- maturin develop smoke in venv - (f8a1b87) - *HaoZeke*
- stabilize clippy and maturin smoke job - (de88660) - *HaoZeke*
#### Chores
- (**fortran**) lock fpm package version to 0.1.2 - (714fbca) - *HaoZeke*
#### Style
- apply prek whitespace, ruff, and taplo - (41b6467) - *HaoZeke*

- - -

## v0.1.2 - 2026-06-28
#### Releases
- (**0.1.2**) CI, install docs, homepage metadata - (3bdc013) - *HaoZeke*

- - -

## v0.1.1 - 2026-06-28
#### Benchmarks
- CON-native insert/extract/concurrency vs ASE.db - (ea456e2) - *HaoZeke*
#### Maintenance
- (**db**) bump to 0.1.1 (cooked SoA tier) - (b8a9dd6) - *HaoZeke*
#### Features
- (**bench**) fair ASE.db vs readcon-db campaign on shared CON ladder - (c41e333) - *HaoZeke*
- (**db**) wire cooked SoA through C/Python/CLI/Fortran and org docs - (7bee032) - *HaoZeke*
- (**db**) optional cooked SoA tier beside authoritative CON text - (852788a) - *HaoZeke*
- (**db**) reversible shard join/split compaction and analysis exports - (9e79f03) - *HaoZeke*
- (**db**) sharded corpus for HPC multi-writer ingest - (91ca0d1) - *HaoZeke*
- (**db**) LMDB multi-process reader slots and embedded SOTA docs - (6e5ac43) - *HaoZeke*
- (**db**) ASE.db-competitive screening fields (mass, volume, PBC, meta) - (84c2a46) - *HaoZeke*
- (**db**) composition, fmax indexes, reindex, and in-memory append - (65558da) - *HaoZeke*
- (**db**) energy and section indexes across Select stack - (1d28100) - *HaoZeke*
- (**ffi**) rkrdb_frame_formula and language bindings for projection - (cee1e59) - *HaoZeke*
#### Bug Fixes
- (**bench**) require nonzero payload checksum on fair extract - (eea0499) - *HaoZeke*
- (**bench**) mass/volume select parity; no ASE energy full-scan fallback - (308fa26) - *HaoZeke*
- (**bench**) fair campaign multi-reader uses shared handles - (8d8ad5d) - *HaoZeke*
- (**cli**) compaction usage and temp join for extxyz export - (bd45e55) - *HaoZeke*
- (**db**) AtomDatum x/y/z and force/velocity for cooked SoA - (74b2bc0) - *HaoZeke*
- (**db**) precompute index keys entirely outside write_txn - (acc21b7) - *HaoZeke*
- (**db**) prepare ingest outside exclusive write_txn - (fadeb91) - *HaoZeke*
- (**db**) materialize CON blobs in touch_trajectory_blobs - (4c7bd64) - *HaoZeke*
- (**db**) CLI timestep/neb-band filters and select tests - (944fa4d) - *HaoZeke*
- (**python**) PyO3 0.28 bindings; honest ASE CON vs readcon positioning in docs - (8959812) - *HaoZeke*
#### Performance
- (**db**) batch trajectory touch and smallest-first select intersect - (f885d45) - *HaoZeke*
- (**ingest**) store original CON spans; no re-serialize on hot path - (3357f8e) - *HaoZeke*
#### Documentation
- (**a11y**) wordmark and logo accent meet AA on light canvas - (0ee6eb0) - *HaoZeke*
- (**a11y**) improve logo stroke and dark brand contrast - (b2739d9) - *HaoZeke*
- (**db**) RCSO not fully equivalent; harden numeric fast path - (d31eebc) - *HaoZeke*
- (**db**) record KEEP sharded LMDB decision for HPC writers - (654484e) - *HaoZeke*
- (**sphinx**) document energy/flags indexes and ecosystem links - (2ec563d) - *HaoZeke*
- CON/XYZ via readcon-core chemfiles; ASE not on I/O path - (ae9bfd4) - *HaoZeke*
- ASE legacy CON vs readcon/readcon-db roles - (6ed3a00) - *HaoZeke*
#### Tests
- (**db**) fix sharded corpus Arc path in parallel writer test - (261f17b) - *HaoZeke*
- (**db**) restore multiproc select test attribute - (e00419f) - *HaoZeke*
- (**db**) multiproc CLI concurrent select across OS processes - (1e394ce) - *HaoZeke*
#### Refactoring
- (**db**) delegate screening scalars to readcon-core index_proj - (00e1510) - *HaoZeke*
#### Chores
- (**db**) depend on readcon-core 0.14 for publish alignment - (e3217d4) - *HaoZeke*
- (**release**) pin readcon-core 0.14 for crates.io publish - (afdeb23) - *HaoZeke*
- restore path dep on readcon-core for LODE monorepo checkout - (b0f28bd) - *HaoZeke*
- drop unused iter binding in ingest - (70d98c1) - *HaoZeke*

- - -

## v0.1.0 - 2026-06-27
#### Releases
- (**0.1.0**) Heed corpus, xxHash3 exact match, C/C++/Python/Fortran - (1d23535) - *HaoZeke*
#### Features
- CLI, extXYZ export, metatrain/ASE workflows and tests - (c98a172) - *HaoZeke*
- Heed/LMDB corpus with ingest, get_frame, and Select indexes - (bd5ff1e) - *HaoZeke*
#### Documentation
- Sphinx site, marketing page, logo and brand kit - (b45f75b) - *HaoZeke*
- mark Heed ingest/select as implemented in v0.1 - (cb77643) - *HaoZeke*
- design embedded CON corpus store on Heed/LMDB - (ef78d7a) - *HaoZeke*
#### Chores
- gitignore docs venv and Sphinx build - (1467e10) - *HaoZeke*
- ignore target/ - (218b00e) - *HaoZeke*
