# db23 fair comparison

Commit `82746979b2a34851019d066f0ead3d3812b98ee2`, tree `4a1b19288f7c66a650ce6e0aad7b88db8c82daaf`. Host Linux 6.12.113+, Intel(R) Xeon(R) Processor, 8 CPUs. `RAYON_NUM_THREADS` was unset for the first block (the JSON records null). n=12, serial. Arm order is db18, raw, shuffle, ase; reversed on odd sample indexes. Warm selected is one pass of i*17 mod 64, each frame once; seconds are the mean per frame. There is no frame cache. The median is type-7. Each time cell is that median, and the parentheses are the type-7 IQR (75th percentile minus the 25th). Both percentiles use `numpy` method `linear`.

## Level and window

`bench/db23/levers.json` is the stdout of `record_db23_levers` (release, ignored). One 100k-atom frame, positions and forces, six scalar columns. Each time is the mean of 20 rounds after 3 untimed rounds. Long-distance matching is off. The probe does not record an IQR.

Two empty `thread::scope` workers cost 106 µs.

| level | window | bytes | one thread µs | two threads µs |
| --- | --- | --- | --- | --- |
| 3 | 20 | 12471 | 520 | 542 |
| 3 | 12 | 24418 | 361 | 545 |
| 1 | 12 | 24651 | 358 | 477 |
| -5 | 12 | 36107 | 321 | 467 |
| -1 | 12 | 24562 | 361 | 515 |

Two decode threads are slower than one thread on every row, so a single frame stays on the calling thread. Level 3 and a 4 KiB window stay. Window 20 is 520 µs and 12471 bytes; window 12 is 361 µs and 24418 bytes. Level -5 at that window is 321 µs and 36107 bytes, 40 µs under level 3. The warm-selected raw IQR is 108 µs, so the level stays 3.

Leaving forces uncompressed would store 2400000 bytes a frame. Shuffling only the high four bytes of each `f64` is 22219 bytes of zstd plus 2400000 raw bytes, and that decode plus the copy is 359 µs. Both stay fully shuffled.

## How the two arms run

A single frame is decoded on the calling thread for raw and for shuffle. A range or a full shuffle read splits frames across `RAYON_NUM_THREADS` workers, or across the process thread count when that variable is unset. Raw range and raw full stay on one thread. The second table is the warm rows again with `RAYON_NUM_THREADS=1`.

The shuffled single-frame read fills the NumPy arrays in place. The raw path copies each column once into a writable array. Both paths return writable arrays.

Raw warm selected is 1.537 ms. The db22 record in `bench/db22/RESULTS.md` lists 2.0600 ms for that cell; that run copied the raw blob before it looked at the tag. The db21 fair record in `bench/db21/RESULTS.md` lists 1.6704 ms for the same cell.

## Sizes

| | db18 | raw | shuffle-zstd | ASE |
| --- | --- | --- | --- | --- |
| traj-100k file | 341966848 | 341966848 | 36569088 | 384503808 |
| `frames_soa` values | 307208960 | 307208960 | 1583727 | |

`frames_soa` values are from `bench/db23/tables.json`. File bytes are `sizes` in `bench/db23/ab.json`.

## Times

| op | cache | db18 | raw | shuffle-zstd | ASE | shuffle/raw |
| --- | --- | --- | --- | --- | --- | --- |
| insert |  | 1.2822e+00 (1.1462e-01) | 1.3431e+00 (1.4665e-01) | 1.1145e+00 (5.6421e-02) | 1.4123e+00 (2.7167e-02) | 0.8298 |
| full_read | cold | 6.8959e-01 (1.0403e-01) | 5.4202e-01 (6.6197e-02) | 2.0436e-01 (5.3766e-02) | 2.0520e+00 (1.7731e-01) | 0.3770 |
| selected | cold | 7.0396e-03 (8.9200e-04) | 6.4280e-03 (1.8964e-03) | 1.8329e-03 (1.7643e-04) | 3.1014e-02 (1.8939e-03) | 0.2851 |
| range | cold | 1.9582e-01 (4.2369e-02) | 1.7242e-01 (2.8466e-02) | 6.8475e-02 (2.9086e-02) | 5.2798e-01 (1.7886e-02) | 0.3971 |
| full_read | warm | 3.8750e-01 (7.8097e-02) | 2.8736e-01 (6.9182e-02) | 1.8474e-01 (2.0609e-02) | 1.8003e+00 (1.4345e-01) | 0.6429 |
| selected | warm | 1.6902e-03 (1.3984e-04) | 1.5371e-03 (1.0827e-04) | 1.3245e-03 (2.1282e-05) | 2.4262e-02 (2.9336e-04) | 0.8617 |
| range | warm | 1.1617e-01 (3.7284e-02) | 9.1304e-02 (2.8144e-02) | 4.3023e-02 (1.2822e-02) | 4.5060e-01 (4.3355e-03) | 0.4712 |

Cold ratios are mostly the file size, 36.6 MB against 342 MB (36569088 bytes against 341966848).

A slower row is inside the noise when the relative gap is no larger than the wider of the two IQR/median ratios, and no larger than 0.05 when both spreads are tighter than that. With `RAYON_NUM_THREADS` unset, every warm row is faster than raw, so byte-shuffled zstd stays the default. `READCON_DB_COOKED_CODEC=raw` keeps RCSO. The name is case-sensitive; the only other accepted value is `shuffle-zstd`. A reader from before db20 cannot open a corpus written with this default.

### `RAYON_NUM_THREADS=1`

| op | cache | db18 | raw | shuffle-zstd | ASE | shuffle/raw |
| --- | --- | --- | --- | --- | --- | --- |
| full_read | warm | 3.5839e-01 (6.2272e-02) | 2.9815e-01 (8.7131e-02) | 3.3848e-01 (1.3828e-02) | 1.9213e+00 (9.4166e-02) | 1.1353 |
| selected | warm | 1.6693e-03 (1.6627e-04) | 1.5746e-03 (1.2267e-04) | 1.3342e-03 (7.4635e-05) | 2.4471e-02 (4.6603e-04) | 0.8473 |
| range | warm | 1.1397e-01 (3.6496e-02) | 8.9014e-02 (3.0104e-02) | 8.1285e-02 (1.7274e-02) | 4.4487e-01 (5.1838e-03) | 0.9132 |

Warm full is 1.1353×. The relative gap is 0.135 and the wider IQR/median is 0.292 (raw 0.2922, shuffle 0.0409), so that row is inside the noise. Warm selected and warm range stay faster.
