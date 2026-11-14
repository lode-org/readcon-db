# db22 shuffle-zstd is the default

Warm selected on the fair pass (`i * 17 mod 64`, each frame once) was
2.01 ms for shuffle and 1.67 ms for raw. Four changes were timed on one
100k-atom frame, positions and forces, 4.80 MB, release build.

Two empty `thread::scope` workers cost 70 µs. Decoding the six columns
as two groups (positions, then forces) is the single-frame parallel
path. Long-distance matching is off in every row below.

| level | window | bytes | one thread µs | two threads µs |
| --- | --- | --- | --- | --- |
| 3 | 20 | 12572 | 761 | 457 |
| 3 | 12 | 24551 | 494 | 291 |
| 1 | 12 | 24784 | 490 | 309 |
| -5 | 12 | 36307 | 470 | 245 |
| -1 | 12 | 24695 | 494 | 386 |

Level 3 with a 4 KiB window is the one that landed. It is 200 µs faster
than the same level at window 20 even before the second thread, and the
two-thread row is another 200 µs, both outside the noise. Level -5 is
46 µs faster than level 3 at the same window and about 12 KB larger per
frame, inside the noise, so the level stays 3. Two threads run only
when the frame has at least 16384 atoms and both positions and forces.
Below that the 70 µs startup is the whole column.

Leaving forces uncompressed would store 2400000 bytes a frame, 154 MB
across 64 frames. Shuffling only the high four bytes of each `f64` and
leaving the low four raw is the same 2.4 MB of raw bytes plus 22 KB of
zstd, and the decode was 359 µs. Neither is a size win. Both stay
shuffled.

The single-frame read allocates the NumPy arrays first and unshuffles
into them. The previous breakdown's second 4.80 MB copy, about 0.3 ms,
is not on that path. Range and full still decode in the Rust buffers
and then copy out. There is no frame cache.

n=12, serial, same host. At each index the order is the db18 binary,
this tree with `READCON_DB_COOKED_CODEC=raw`, this tree with
`shuffle-zstd`, then ASE. Cold drops the file until mincore is 0 and
opens outside the clock. Every warm shuffle row is faster than raw, so
the codec is the default. `READCON_DB_COOKED_CODEC=raw` keeps RCSO.
`FB 03` from db20 still decodes. Blobs under 64 KiB and cooked groups
stay raw.

| | db18 | raw | shuffle-zstd | ASE |
| --- | --- | --- | --- | --- |
| traj-100k file | 341966848 | 341966848 | 36569088 | 384503808 |
| `frames_soa` values | 307208960 | 307208960 | 1583727 |  |

| op | cache | db18 s | raw s | shuffle s | ASE s | shuffle/raw |
| --- | --- | --- | --- | --- | --- | --- |
| insert | | 1.3642e+00 | 1.2746e+00 | 1.1367e+00 | 1.4018e+00 | 0.892 |
| full_read | cold | 6.5975e-01 | 6.1690e-01 | 2.8582e-01 | 2.1520e+00 | 0.463 |
| selected | cold | 6.2080e-03 | 5.8248e-03 | 1.8622e-03 | 3.1112e-02 | 0.320 |
| range | cold | 1.8614e-01 | 1.8479e-01 | 7.3832e-02 | 4.9851e-01 | 0.400 |
| full_read | warm | 3.6879e-01 | 3.6274e-01 | 2.4976e-01 | 1.7858e+00 | 0.689 |
| selected | warm | 1.6515e-03 | 2.0600e-03 | 1.1805e-03 | 2.3661e-02 | 0.573 |
| range | warm | 1.0477e-01 | 1.0411e-01 | 6.5526e-02 | 4.3361e-01 | 0.629 |

Warm selected is the mean of one pass over 64 distinct frames.
