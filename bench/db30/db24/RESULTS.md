# db24 fair comparison

Commit `043c3091c9606293a920311a73a8a622d4638746`, tree `a201b635a8bfaa444623c2530e6c10592a423c3f`. Host Linux 6.12.113+, Intel(R) Xeon(R) Processor, 8 CPUs. `RAYON_NUM_THREADS` was unset for the first block. n=12, serial. Arm order is db18, raw, shuffle, ase; reversed on odd sample indexes. Warm selected is one pass of i*17 mod 64, each frame once; seconds are the mean per frame. There is no frame cache. The median is type-7. Each time cell is that median, and the parentheses are the type-7 IQR. Both percentiles use `numpy` method `linear`.

`bench/db23` names unpublished commit `82746979b2a34851019d066f0ead3d3812b98ee2` for tree `4a1b19288f7c66a650ce6e0aad7b88db8c82daaf`. That commit is not in the published history. The published commit for the same tree is `00d32136e9acdbc84fd0297d68b812b0facaedee`. db23 and db22 are unchanged.

db22 lists a warm-selected shuffle/raw ratio of 0.573. That figure is superseded. The raw arm copied the blob before it looked at the tag, which inflated raw. The corrected comparisons are the db23 and db24 warm-selected rows.

## Level and window

`bench/db30/db24/levers.json` is the stdout of `record_db24_levers` (release, ignored). One 100k-atom frame, positions and forces, six scalar columns. Each time is the mean of 20 rounds after 3 untimed rounds. The same rounds are in `samples_us`, and `iqr_us` is their type-7 IQR. Long-distance matching is off. `tables.json` is written by `fair_ase_protocol.record_table_usage`.

Two empty `thread::scope` workers cost 152 µs (IQR 195 µs).

| level | window | bytes | one thread µs (IQR) | two threads µs (IQR) |
| --- | --- | --- | --- | --- |
| 3 | 20 | 12471 | 519 (5) | 443 (252) |
| 3 | 12 | 24418 | 361 (1) | 405 (274) |
| 1 | 12 | 24651 | 358 (1) | 340 (228) |
| -5 | 12 | 36107 | 318 (1) | 337 (172) |
| -1 | 12 | 24562 | 359 (1) | 431 (114) |

Level 3 and a 4 KiB window are the parameters these fair rows used. Level -5 at that window is 43 µs under level 3 on the mean, and the wider of those two IQRs is 1 µs, so that gap is outside the lever's own spread. The compressed columns grow from 24418 bytes to 36107 bytes. The level stays 3. The comparison is against this lever IQR, not a selected-row IQR.

Leaving forces uncompressed would store 2400000 bytes a frame. Shuffling only the high four bytes of each `f64` is 22219 bytes of zstd plus 2400000 raw bytes, and that decode plus the copy is 340 µs (IQR 1 µs). Both stay fully shuffled.

## How the two arms run

A single frame is decoded on the calling thread for raw and for shuffle. The shuffled path decompresses the three lanes into reused buffers and unshuffles them straight into `x, y, z` order. It does not scatter one lane at a time. A range or a full shuffle read splits frames across `RAYON_NUM_THREADS` workers, or across the process thread count when that variable is unset. Raw range and raw full stay on one thread. The second table is the warm rows again with `RAYON_NUM_THREADS=1`.

## Sizes

| | db18 | raw | shuffle-zstd | ASE |
| --- | --- | --- | --- | --- |
| traj-100k file | 341966848 | 341966848 | 36569088 | 384503808 |
| `frames_soa` values | 307208960 | 307208960 | 1583727 | |

`frames_soa` values are the `value_bytes` rows in `bench/db30/db24/tables.json`, written by `fair_ase_protocol.record_table_usage`.

## Times

| op | cache | db18 | raw | shuffle-zstd | ASE | shuffle/raw |
| --- | --- | --- | --- | --- | --- | --- |
| insert |  | 1.2265e+00 (1.0541e-01) | 2.4483e+00 (3.5218e-02) | 2.2228e+00 (1.5914e-01) | 1.3792e+00 (8.7334e-02) | 0.9079 |
| full_read | cold | 5.5470e-01 (6.8224e-02) | 5.1327e-01 (3.9578e-02) | 1.1230e-01 (6.4880e-02) | 1.9668e+00 (1.3209e-01) | 0.2188 |
| selected | cold | 3.8417e-03 (1.5866e-03) | 6.1397e-03 (4.7290e-04) | 1.4389e-03 (2.4431e-04) | 2.8024e-02 (3.0775e-03) | 0.2344 |
| range | cold | 1.6056e-01 (2.8982e-02) | 1.6779e-01 (2.6231e-02) | 3.0471e-02 (1.8051e-03) | 4.7883e-01 (1.3297e-02) | 0.1816 |
| full_read | warm | 3.5949e-01 (5.5151e-02) | 2.6235e-01 (4.6173e-02) | 8.6203e-02 (1.3043e-02) | 1.7021e+00 (1.5150e-01) | 0.3286 |
| selected | warm | 2.6567e-03 (1.6069e-04) | 2.5362e-03 (1.4971e-04) | 2.0947e-03 (5.3841e-05) | 2.3225e-02 (1.6552e-04) | 0.8259 |
| range | warm | 1.1220e-01 (3.6087e-02) | 9.0095e-02 (2.8631e-02) | 2.1947e-02 (2.0311e-03) | 4.1480e-01 (5.9296e-03) | 0.2436 |

The shuffle/raw column is the ratio of the two medians. The paired ratio below is the type-7 median of shuffle/raw at the same sample index. The CI is a 10000-draw percentile bootstrap of that median (numpy Generator seed 0).

insert: paired median 0.9272, 95% CI [0.8931, 0.9370]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

full_read cold: paired median 0.2234, 95% CI [0.1702, 0.2675]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

selected cold: paired median 0.2423, 95% CI [0.2163, 0.2616]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

range cold: paired median 0.1899, 95% CI [0.1792, 0.2060]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

full_read warm: paired median 0.3106, 95% CI [0.2845, 0.3680]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

selected warm: paired median 0.8228, 95% CI [0.7971, 0.8416]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

range warm: paired median 0.2530, 95% CI [0.2334, 0.3412]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

A slower row is inside the noise when the relative gap of the medians is no larger than the wider of the two IQR/median ratios, and no larger than 0.05 when both spreads are tighter than that. The paired interval is the one used when saying a row is faster.

With `RAYON_NUM_THREADS` unset, warm full reads are faster on the paired interval.

`READCON_DB_COOKED_CODEC=raw` keeps RCSO. The name is case-sensitive; the only other accepted value is `shuffle-zstd`. A reader from before db20 cannot open a corpus written with the shuffle default.

### `RAYON_NUM_THREADS=1`

| op | cache | db18 | raw | shuffle-zstd | ASE | shuffle/raw |
| --- | --- | --- | --- | --- | --- | --- |
| full_read | warm | 3.3691e-01 (6.0402e-02) | 2.6955e-01 (2.9598e-02) | 1.7536e-01 (4.1160e-02) | 1.6612e+00 (9.9680e-02) | 0.6505 |
| selected | warm | 2.5403e-03 (8.7136e-05) | 2.4613e-03 (1.0135e-04) | 1.9997e-03 (1.3959e-04) | 2.3168e-02 (1.9487e-04) | 0.8124 |
| range | warm | 1.0722e-01 (3.5609e-02) | 9.2566e-02 (4.1682e-02) | 5.0739e-02 (1.5791e-02) | 4.1377e-01 (1.0030e-02) | 0.5481 |

RAYON=1 full_read warm: paired median 0.6596, 95% CI [0.5985, 0.8184]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

RAYON=1 selected warm: paired median 0.8013, 95% CI [0.7975, 0.8181]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

RAYON=1 range warm: paired median 0.5592, 95% CI [0.4664, 0.6109]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

Single-thread warm full reads are faster: the paired interval is below 1.
