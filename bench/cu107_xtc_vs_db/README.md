# Cu107: XTC+GRO against readcon-db

One trajectory, two reads, one process. There is no Cu107-with-forces
trajectory on `readcon-core` `paper/reframe-v2`
(`52268690c7b671053ce997b10a26cf5a30edefdb`) or `readcon-db`
`eval/cpc-paper` (`6146c3be9cde2d35f78cc540dedd978d995976b8`).
`generate.py` builds 120000 frames from `resources/test/tiny_cuh2_forces.con`.

The two copper atoms in that fixture supply the coordinates, forces, mass,
header lines, and metadata. They are tiled on a 5x5x5 lattice and the first
107 sites are kept. Frame `k` adds `k * 1e-4` angstrom to every x coordinate
and multiplies forces by `1 + k * 1e-6`. Every frame is `Cu:107` and has
forces. The large CON, XTC, GRO, and store files are written under `work/`
and are not part of the commit.

## Routes

Route A, not including the one-time XTC/GRO conversion. Load the GRO
topology, decode every XTC frame, and keep frames whose 107 atom names are
`Cu`. XTC holds no forces. That half is `unanswerable_from_xtc`.

Route B, not including the one-time ingest. `ConCorpus.select(formula="Cu:107",
require_forces=True)`, then `get_positions` and `get_forces` for each
selected key. Ingest calls `recook_all` so those reads use the cooked
per-frame records.

The script checks that both routes return frame indices `0 .. n-1` and that
route B forces match the decimals written into the CON file.

## Reproduce

From the repository root, with a toolchain that satisfies `rust-version` in
`Cargo.toml`:

```sh
python3 -m venv .venv
. .venv/bin/activate
pip install maturin 'MDAnalysis>=2.7' numpy
maturin develop --release --bindings pyo3 --manifest-path python/pyproject.toml
python bench/cu107_xtc_vs_db/run.py
```

`run.py` sets `OMP_NUM_THREADS`, `OPENBLAS_NUM_THREADS`, `MKL_NUM_THREADS`,
and `NUMEXPR_NUM_THREADS` to 1. The default is 120000 frames, 2 discarded
ABAB warmups, then 30 timed repeats of each route. Warm-cache repeats run
first. Cold repeats call `sync` and `echo 3 > /proc/sys/vm/drop_caches`
before each route. One invocation measures both caches.

`results.csv` has one timed row: `route`, `cache`, `repeat`, `wall_s`,
`n_frames_selected`. `repeat` starts at 1. Routes are `A` and `B`.

`results.json` holds medians, the interquartile range (75th minus 25th,
NumPy percentile method `linear`), min, max, and a paired bootstrap 95%
interval of `median(B) / median(A)`. Each cache reseeds
`numpy.random.default_rng(20261123)` and draws 10000 resamples of the
repeat index. The file also records frame counts, the CPU model, the core
count, RAM, the OS, and the Python, MDAnalysis, NumPy, readcon-db, and
readcon-core versions. readcon-core `0.23.2` is commit
`8c2d9984c40aedfb2e0084fedefaa02e930957f2`. File sizes are allocated blocks
(`st_blocks * 512`) for the XTC, the GRO, and the store directory.
