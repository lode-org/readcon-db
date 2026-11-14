# db20 shuffle codec, opt-in

The db19 trial stored a byte-shuffled zstd blob and lost the warm rows.
The unshuffle was 1579 µs for one 2.4 MB coordinate array. This pass
replaces that with an AVX2 transpose of 32 f64 values and unshuffles
each column straight into its buffer. Positions, forces, and velocities
are shuffled separately, so a reader does not have to undo one shuffle
of the concatenated columns.

Release, one core, 600000 f64 (4.80 MB, positions plus forces):

| step | bytes | time |
| --- | --- | --- |
| unshuffle | 4800000 | 296 µs (148 µs for one 2.4 MB column) |
| zstd level 1 of the shuffled bytes | 23858 | 494 µs |
| zstd level -5 | 71479 | 470 µs |
| LZ4 | 107199 | 425 µs, 694 µs with the unshuffle |

Negative zstd barely moves the decode and costs size. LZ4 is a little
faster and about four times larger. Neither one removes the unshuffle,
and a raw `from_blob` of the same frame was 485 µs. The warm selected
row is about 1.4 ms, so an extra several hundred microseconds does not
land inside the noise.

The interleaved A/B is n=12, serial, same host. At each index the order
is the db18 binary, then this codec, then ASE. Each tool reads the
corpus it wrote. Cold drops the file until mincore is 0 and opens
outside the clock. GC is off for both tools. `record_table_usage` runs
when the corpus is built. A row is inside the noise when the relative
median gap is no larger than the wider IQR/median, and no larger than
0.05 when both spreads are tighter than that.

Shuffle is tag `FB 03`. Blobs under 64 KiB and cooked groups stay raw,
so the frames-1000 and frames-10000 layout is unchanged. db15 through
db18 files do not carry the tag and still decode as RCSO or groups.
Selected reads decode the one requested frame. Range and full reads
decode on `RAYON_NUM_THREADS` workers (`std::thread::scope`; unset uses
the available parallelism).

| | db18 | shuffle-zstd |
| --- | --- | --- |
| traj-100k file | 341966848 | 36765696 |
| `frames_soa` values | 307208960 | 1952514 |

The CON text in `frames` stays about 35 MB, so the file does not shrink
by the full column ratio.

| op | cache | db18 median s | shuffle median s | ASE median s | ratio |
| --- | --- | --- | --- | --- | --- |
| insert |  | 1.2688e+00 | 1.0435e+00 | 1.3798e+00 | 0.822 |
| full_read | cold | 6.2266e-01 | 4.0743e-01 | 2.0683e+00 | 0.654 |
| selected | cold | 7.4071e-03 | 3.6794e-03 | 3.2493e-02 | 0.497 |
| range | cold | 1.7946e-01 | 1.3103e-01 | 4.9019e-01 | 0.730 |
| full_read | warm | 3.6109e-01 | 3.9530e-01 | 1.7920e+00 | 1.095 |
| selected | warm | 1.3889e-03 | 2.9237e-03 | 2.3812e-02 | 2.105 |
| range | warm | 1.0405e-01 | 1.2626e-01 | 4.2476e-01 | 1.213 |

Warm selected is 2.10 times slower and its IQR/median is 0.02. Warm
full is 1.09 times slower, IQR/median 0.02. Those are outside the
noise. Warm range is 1.21 and its IQR/median is 0.24, so that one row
sits in the spread. Cold reads and the insert are faster. The codec
still beats ASE on every row. It does not land as the default.

`READCON_DB_COOKED_CODEC=shuffle-zstd` opts in. Unset and `raw` store
RCSO. The choice is recorded on `TrajMeta.cooked_codec`. The same
protocol with the default, this binary against db18:

| | db18 | raw default |
| --- | --- | --- |
| traj-100k file | 341966848 | 341966848 |

| op | cache | db18 median s | raw median s | ASE median s | ratio |
| --- | --- | --- | --- | --- | --- |
| insert |  | 1.1978e+00 | 1.1702e+00 | 1.3632e+00 | 0.977 |
| full_read | cold | 6.3849e-01 | 6.3354e-01 | 2.0785e+00 | 0.992 |
| selected | cold | 6.7188e-03 | 5.3815e-03 | 3.2315e-02 | 0.801 |
| range | cold | 2.0025e-01 | 1.8614e-01 | 5.0316e-01 | 0.930 |
| full_read | warm | 3.8489e-01 | 3.8443e-01 | 1.8963e+00 | 0.999 |
| selected | warm | 1.3821e-03 | 1.4113e-03 | 2.3898e-02 | 1.021 |
| range | warm | 1.0475e-01 | 1.0750e-01 | 4.2976e-01 | 1.026 |

Warm full, warm selected, and warm range are inside the noise (ratios
0.999, 1.021, 1.026). Cold selected's median is lower and its
IQR/median is 0.35, so that gap is the spread, not a slower default.
The raw path stays the default.
