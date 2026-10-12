# readcon-db evaluation

Reproducible checks and timings for the corpus store. This directory is
measurements, not paper text.

## What is checked

Correctness (harness `correctness`): identical CON bytes share one xxHash3-128
index entry and a one-byte edit does not; `reindex` rebuilds secondary indexes
from stored CON text; stored blobs match the readcon-core raw spans;
`frames_soa` is ignored by the hash and by `reindex`; frames with no forces or
no velocities stay out of those indexes; a cell without a `pbc` metadata key is
not treated as periodic.

Workloads (`scripts/generate_workloads.py`): ASE EMT saddle searches (HCN/HNC
NEB, Cu adatom NEB, Pt–H NEB, Pt adatom dimer) and an aKMC trace that reuses
those states. eOn is not required. Scale corpora repeat that ensemble out to
the requested frame counts, with fresh EMT single points once the search tape
is exhausted.

Timings (`scripts/run_eval.py`): five paired, interleaved repeats of ingest and of
composition, energy-range, and max-force queries against `ase.db` (SQLite) and
a directory of CON files scanned with readcon-core. Each series is summarized
by its median, minimum, and maximum. Thread caps are set to 1.

## Run

From the repository root, with the release profile:

```bash
python3 eval/scripts/generate_workloads.py --sizes 1000,10000
cargo build --release --manifest-path eval/harness/Cargo.toml
python3 eval/scripts/run_eval.py --sizes 1000,10000 --repeats 5 --child-timeout 90
python3 eval/scripts/plot_results.py
```

A full interleaved repeat at 100000 frames does not finish in two minutes, because
`ase.db` ingest is about two hundred frames per second. One million frames does
not fit in the 2 GiB LMDB map (`MDB_MAP_FULL`). Those sizes are not in the
timed table.

Numbers land in `eval/results/`. `eval/RESULTS.md` states them.
Scale CON trees under `eval/data/scale/` are rebuilt by the generator and are
not stored in git. `eval/data/manifest.json` records their byte counts and
SHA-256 digests.
