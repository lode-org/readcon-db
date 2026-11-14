# db24 fair comparison

Commit `67dc0235955e6c0f50a8e814d4d489fbb387ec65`, tree `99addc975a22943f44aa024dc4b718d6d5052579`. Host Linux 6.12.113+, Intel(R) Xeon(R) Processor, 8 CPUs. `RAYON_NUM_THREADS` was unset for the first block. n=12, serial. Arm order is db18, raw, shuffle, ase; reversed on odd sample indexes. Warm selected is one pass of i*17 mod 64, each frame once; seconds are the mean per frame. There is no frame cache. The median is type-7. Each time cell is that median, and the parentheses are the type-7 IQR. Both percentiles use `numpy` method `linear`.

`bench/db23` names unpublished commit `82746979b2a34851019d066f0ead3d3812b98ee2` for tree `4a1b19288f7c66a650ce6e0aad7b88db8c82daaf`. That commit is not in the published history. The published commit for the same tree is `00d32136e9acdbc84fd0297d68b812b0facaedee`. db23 and db22 are unchanged.

db22 lists a warm-selected shuffle/raw ratio of 0.573. That figure is superseded. The raw arm copied the blob before it looked at the tag, which inflated raw. The corrected comparisons are the db23 and db24 warm-selected rows.

## Level and window

`bench/db24/levers.json` is the stdout of `record_db24_levers` (release, ignored). One 100k-atom frame, positions and forces, six scalar columns. Each time is the mean of 20 rounds after 3 untimed rounds. The same rounds are in `samples_us`, and `iqr_us` is their type-7 IQR. Long-distance matching is off. `tables.json` is written by `fair_ase_protocol.record_table_usage`.

Two empty `thread::scope` workers cost 110 µs (IQR 61 µs).

| level | window | bytes | one thread µs (IQR) | two threads µs (IQR) |
| --- | --- | --- | --- | --- |
| 3 | 20 | 12471 | 575 (27) | 364 (46) |
| 3 | 12 | 24418 | 413 (18) | 292 (136) |
| 1 | 12 | 24651 | 418 (16) | 306 (89) |
| -5 | 12 | 36107 | 363 (13) | 323 (204) |
| -1 | 12 | 24562 | 429 (46) | 276 (39) |

Level 3 and a 4 KiB window are the parameters these fair rows used. Level -5 at that window is 50 µs under level 3 on the mean, and the wider of those two IQRs is 18 µs, so that gap is outside the lever's own spread. The compressed columns grow from 24418 bytes to 36107 bytes. The level stays 3. The comparison is against this lever IQR, not a selected-row IQR.

Leaving forces uncompressed would store 2400000 bytes a frame. Shuffling only the high four bytes of each `f64` is 22219 bytes of zstd plus 2400000 raw bytes, and that decode plus the copy is 419 µs (IQR 28 µs). Both stay fully shuffled.

## How the two arms run

A single frame is decoded on the calling thread for raw and for shuffle. The shuffled path decompresses the three lanes into reused buffers and unshuffles them straight into `x, y, z` order. It does not scatter one lane at a time. A range or a full shuffle read splits frames across `RAYON_NUM_THREADS` workers, or across the process thread count when that variable is unset. Raw range and raw full stay on one thread. The second table is the warm rows again with `RAYON_NUM_THREADS=1`.

## Sizes

| | db18 | raw | shuffle-zstd | ASE |
| --- | --- | --- | --- | --- |
| traj-100k file | 341966848 | 341966848 | 36569088 | 384503808 |
| `frames_soa` values | 307208960 | 307208960 | 1583727 | |

`frames_soa` values are the `value_bytes` rows in `bench/db24/tables.json`, written by `fair_ase_protocol.record_table_usage`.

## Times

| op | cache | db18 | raw | shuffle-zstd | ASE | shuffle/raw |
| --- | --- | --- | --- | --- | --- | --- |
| insert |  | 1.2613e+00 (1.0517e-01) | 1.2840e+00 (1.0693e-01) | 1.2163e+00 (5.7745e-02) | 1.4214e+00 (1.1323e-01) | 0.9472 |
| full_read | cold | 5.9839e-01 (6.9832e-02) | 5.5265e-01 (6.2129e-02) | 9.1962e-02 (1.1637e-02) | 1.9790e+00 (7.1768e-02) | 0.1664 |
| selected | cold | 6.3752e-03 (1.5092e-03) | 5.5706e-03 (4.8489e-04) | 1.4516e-03 (1.1403e-04) | 2.7958e-02 (1.9848e-03) | 0.2606 |
| range | cold | 1.6857e-01 (2.7461e-02) | 1.5292e-01 (2.0451e-02) | 3.0244e-02 (5.0591e-03) | 4.8276e-01 (2.7479e-02) | 0.1978 |
| full_read | warm | 3.4732e-01 (7.6309e-02) | 2.3207e-01 (5.3871e-02) | 9.8123e-02 (3.0142e-02) | 1.6775e+00 (6.3778e-02) | 0.4228 |
| selected | warm | 2.6173e-03 (2.2593e-04) | 2.4678e-03 (2.1925e-04) | 2.1428e-03 (7.6065e-05) | 2.3405e-02 (1.5115e-04) | 0.8683 |
| range | warm | 1.1304e-01 (3.7877e-02) | 8.9238e-02 (3.3133e-02) | 2.7464e-02 (7.0526e-03) | 4.2155e-01 (9.1377e-03) | 0.3078 |

The shuffle/raw column is the ratio of the two medians. The paired ratio below is the type-7 median of shuffle/raw at the same sample index. The CI is a 10000-draw percentile bootstrap of that median (numpy Generator seed 0).

insert: paired median 0.9557, 95% CI [0.8883, 1.0030]. Shuffle faster in 9 of 12, slower in 3, ties 0. The paired interval includes 1.

full_read cold: paired median 0.1705, 95% CI [0.1562, 0.1867]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

selected cold: paired median 0.2652, 95% CI [0.2474, 0.2883]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

range cold: paired median 0.2101, 95% CI [0.1935, 0.2188]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

full_read warm: paired median 0.4531, 95% CI [0.3854, 0.5117]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

selected warm: paired median 0.8783, 95% CI [0.8432, 0.9181]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

range warm: paired median 0.3324, 95% CI [0.3112, 0.3683]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

A slower row is inside the noise when the relative gap of the medians is no larger than the wider of the two IQR/median ratios, and no larger than 0.05 when both spreads are tighter than that. The paired interval is the one used when saying a row is faster.

With `RAYON_NUM_THREADS` unset, warm full reads are faster on the paired interval.

`READCON_DB_COOKED_CODEC=raw` keeps RCSO. The name is case-sensitive; the only other accepted value is `shuffle-zstd`. A reader from before db20 cannot open a corpus written with the shuffle default.

### `RAYON_NUM_THREADS=1`

| op | cache | db18 | raw | shuffle-zstd | ASE | shuffle/raw |
| --- | --- | --- | --- | --- | --- | --- |
| full_read | warm | 3.5689e-01 (6.4731e-02) | 3.2206e-01 (2.4344e-02) | 1.7180e-01 (1.1474e-02) | 1.6955e+00 (1.0818e-01) | 0.5334 |
| selected | warm | 2.7209e-03 (1.7587e-04) | 2.5581e-03 (9.5030e-05) | 2.0966e-03 (3.8517e-05) | 2.3405e-02 (4.5310e-05) | 0.8196 |
| range | warm | 1.1076e-01 (3.4948e-02) | 9.2516e-02 (3.2513e-02) | 5.0358e-02 (1.5037e-02) | 4.2757e-01 (6.2672e-03) | 0.5443 |

RAYON=1 full_read warm: paired median 0.5485, 95% CI [0.5221, 0.5806]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

RAYON=1 selected warm: paired median 0.8178, 95% CI [0.8018, 0.8310]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

RAYON=1 range warm: paired median 0.5525, 95% CI [0.5401, 0.5762]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

Single-thread warm full reads are faster: the paired interval is below 1.
