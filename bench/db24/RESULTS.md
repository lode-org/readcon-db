# db24 fair comparison

Commit `466c480450feddff032de8364e2a58ec85545e95`, tree `1ce09ecda3c7f67d535de71a868a3f8394a85e7e`. Host Linux 6.12.113+, Intel(R) Xeon(R) Processor, 8 CPUs. `RAYON_NUM_THREADS` was unset for the first block. n=12, serial. Arm order is db18, raw, shuffle, ase; reversed on odd sample indexes. Warm selected is one pass of i*17 mod 64, each frame once; seconds are the mean per frame. There is no frame cache. The median is type-7. Each time cell is that median, and the parentheses are the type-7 IQR. Both percentiles use `numpy` method `linear`.

`bench/db23` names unpublished commit `82746979b2a34851019d066f0ead3d3812b98ee2` for tree `4a1b19288f7c66a650ce6e0aad7b88db8c82daaf`. That commit is not in the published history. The published commit for the same tree is `00d32136e9acdbc84fd0297d68b812b0facaedee`. db23 and db22 are unchanged.

db22 lists a warm-selected shuffle/raw ratio of 0.573. That figure is superseded. The raw arm copied the blob before it looked at the tag, which inflated raw. The corrected comparisons are the db23 and db24 warm-selected rows.

## Level and window

`bench/db24/levers.json` is the stdout of `record_db24_levers` (release, ignored). One 100k-atom frame, positions and forces, six scalar columns. Each time is the mean of 20 rounds after 3 untimed rounds. The same rounds are in `samples_us`, and `iqr_us` is their type-7 IQR. Long-distance matching is off. `tables.json` is written by `fair_ase_protocol.record_table_usage`.

Two empty `thread::scope` workers cost 144 µs (IQR 47 µs).

| level | window | bytes | one thread µs (IQR) | two threads µs (IQR) |
| --- | --- | --- | --- | --- |
| 3 | 20 | 12471 | 522 (10) | 536 (248) |
| 3 | 12 | 24418 | 357 (0) | 398 (229) |
| 1 | 12 | 24651 | 358 (1) | 409 (276) |
| -5 | 12 | 36107 | 319 (1) | 311 (100) |
| -1 | 12 | 24562 | 358 (0) | 361 (180) |

Level 3 and a 4 KiB window are the parameters these fair rows used. Level -5 at that window is 38 µs under level 3 on the mean, and the wider of those two IQRs is 1 µs, so that gap is outside the lever's own spread. The compressed columns grow from 24418 bytes to 36107 bytes. The level stays 3. The comparison is against this lever IQR, not a selected-row IQR.

Leaving forces uncompressed would store 2400000 bytes a frame. Shuffling only the high four bytes of each `f64` is 22219 bytes of zstd plus 2400000 raw bytes, and that decode plus the copy is 352 µs (IQR 3 µs). Both stay fully shuffled.

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
| insert |  | 1.2166e+00 (5.2723e-02) | 1.1805e+00 (7.5149e-02) | 1.0013e+00 (1.7067e-02) | 1.3020e+00 (7.8068e-02) | 0.8481 |
| full_read | cold | 5.9479e-01 (1.0887e-01) | 5.5022e-01 (1.2970e-01) | 2.1892e-01 (8.8795e-02) | 1.9880e+00 (1.6691e-01) | 0.3979 |
| selected | cold | 6.1015e-03 (1.1549e-03) | 7.2286e-03 (7.3913e-04) | 1.3466e-03 (1.6506e-04) | 2.9976e-02 (1.7365e-03) | 0.1863 |
| range | cold | 1.7645e-01 (3.8748e-02) | 1.6773e-01 (5.2932e-02) | 5.7541e-02 (1.6431e-02) | 4.8125e-01 (2.9807e-02) | 0.3431 |
| full_read | warm | 3.3588e-01 (7.0698e-02) | 2.7264e-01 (5.2221e-02) | 1.5214e-01 (2.2195e-02) | 1.6927e+00 (1.3721e-01) | 0.5580 |
| selected | warm | 1.5916e-03 (1.0242e-04) | 1.4543e-03 (1.0356e-04) | 9.0981e-04 (3.6046e-05) | 2.2507e-02 (4.8060e-05) | 0.6256 |
| range | warm | 1.0992e-01 (3.6546e-02) | 9.6104e-02 (3.7419e-02) | 4.0522e-02 (1.2800e-02) | 4.1008e-01 (7.5652e-03) | 0.4216 |

The shuffle/raw column is the ratio of the two medians. The paired ratio below is the type-7 median of shuffle/raw at the same sample index. The CI is a 10000-draw percentile bootstrap of that median (numpy Generator seed 0).

insert: paired median 0.8628, 95% CI [0.8490, 0.9075]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

full_read cold: paired median 0.3183, 95% CI [0.2788, 0.4350]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

selected cold: paired median 0.1839, 95% CI [0.1733, 0.2019]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

range cold: paired median 0.3887, 95% CI [0.2557, 0.4676]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

full_read warm: paired median 0.5609, 95% CI [0.5060, 0.6813]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

selected warm: paired median 0.6385, 95% CI [0.6233, 0.6507]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

range warm: paired median 0.4898, 95% CI [0.4030, 0.6915]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

A slower row is inside the noise when the relative gap of the medians is no larger than the wider of the two IQR/median ratios, and no larger than 0.05 when both spreads are tighter than that. The paired interval is the one used when saying a row is faster.

With `RAYON_NUM_THREADS` unset, warm full reads are faster on the paired interval.

`READCON_DB_COOKED_CODEC=raw` keeps RCSO. The name is case-sensitive; the only other accepted value is `shuffle-zstd`. A reader from before db20 cannot open a corpus written with the shuffle default.

### `RAYON_NUM_THREADS=1`

| op | cache | db18 | raw | shuffle-zstd | ASE | shuffle/raw |
| --- | --- | --- | --- | --- | --- | --- |
| full_read | warm | 3.2212e-01 (6.2991e-02) | 2.5810e-01 (5.3951e-02) | 2.5004e-01 (1.1321e-02) | 1.6009e+00 (4.4102e-02) | 0.9688 |
| selected | warm | 1.6222e-03 (1.1358e-04) | 1.4343e-03 (9.9953e-05) | 9.2537e-04 (4.4614e-05) | 2.2646e-02 (1.0846e-04) | 0.6452 |
| range | warm | 1.0966e-01 (3.6164e-02) | 9.1128e-02 (3.8796e-02) | 6.5002e-02 (1.3732e-02) | 4.1403e-01 (1.0438e-02) | 0.7133 |

RAYON=1 full_read warm: paired median 0.9430, 95% CI [0.8048, 1.0895]. Shuffle faster in 8 of 12, slower in 4, ties 0. The paired interval includes 1.

RAYON=1 selected warm: paired median 0.6532, 95% CI [0.6331, 0.6818]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

RAYON=1 range warm: paired median 0.7895, 95% CI [0.7064, 0.8836]. Shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

Single-thread warm full reads are not faster on this paired interval. The ratio of medians and the win count are in the sentence above.
