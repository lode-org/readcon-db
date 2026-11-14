# db26 selected, cold, and raw across commits

Commit `20a5f64e37d9a0892e913289d39bcbe71d7718f8`, tree `3c611a734dfe1f60715aa88ee3eb214d1997f806`. Raw comparison base `aedcbda468d208278b8699b80c6db743edcdc21e` tree `4ec91d84f06cb9dada3326231d811d9ea0063534`. Host Linux 6.12.113+, Intel(R) Xeon(R) Processor, 8 CPUs. n=12, serial. The median is type-7. Parentheses are the type-7 IQR. The paired CI is a 10000-draw percentile bootstrap of the ratio at the same sample index (numpy Generator seed 0). Odd sample indexes reverse the arm order. Load average at the start of the timed block was 0.46, 0.65, 0.42.

db24 and db25 are unchanged. db25 did not repeat selected or cold. Those rows are here because `d671c32` unshuffles into the caller buffer. That commit is in this history, and this measurement is `20a5f64e`.

## Summary

| comparison | op | threads | cache | ratio of medians | paired median | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- |
| shuffle/raw | selected | unset | cold | 0.2350 | 0.2273 | [0.2120, 0.2488] | 12/12 |
| shuffle/raw | full_read | unset | cold | 0.1593 | 0.1544 | [0.1405, 0.1834] | 12/12 |
| shuffle/raw | range | unset | cold | 0.1793 | 0.1678 | [0.1449, 0.2223] | 12/12 |
| shuffle/raw | selected | unset | warm | 0.6738 | 0.6701 | [0.6620, 0.6799] | 12/12 |
| 20a5f64e/aedcbda4 raw | full_read | unset | warm | 1.0900 | 1.0977 | [0.9128, 1.2952] | 4/12 |
| 20a5f64e/aedcbda4 raw | range | unset | warm | 1.0752 | 1.1855 | [0.9022, 1.3230] | 4/12 |
| 20a5f64e/aedcbda4 raw | full_read | 1 | warm | 0.9693 | 0.9855 | [0.8080, 1.1856] | 6/12 |
| 20a5f64e/aedcbda4 raw | range | 1 | warm | 1.0478 | 1.0806 | [0.7845, 1.1941] | 6/12 |

## Shuffle versus raw

Arm order is raw, shuffle, reversed on odd indexes. Warm selected is one pass of i*17 mod 64, and the seconds are the mean per frame. Cold advises `DONTNEED` outside the clock, then opens and reads inside it. There is no frame cache.

| op | cache | raw | shuffle-zstd | shuffle/raw | paired median | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- |
| full_read | cold | 5.4373e-01 (5.8805e-02) | 8.6634e-02 (2.3587e-02) | 0.1593 | 0.1544 | [0.1405, 0.1834] | 12/12 |
| selected | cold | 6.7582e-03 (7.0990e-04) | 1.5879e-03 (2.5577e-04) | 0.2350 | 0.2273 | [0.2120, 0.2488] | 12/12 |
| range | cold | 1.4260e-01 (1.2907e-02) | 2.5570e-02 (6.2114e-03) | 0.1793 | 0.1678 | [0.1449, 0.2223] | 12/12 |
| selected | warm | 1.3702e-03 (3.2944e-05) | 9.2325e-04 (2.3273e-05) | 0.6738 | 0.6701 | [0.6620, 0.6799] | 12/12 |

full_read cold: paired median 0.1544, 95% CI [0.1405, 0.1834]. shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

selected cold: paired median 0.2273, 95% CI [0.2120, 0.2488]. shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

range cold: paired median 0.1678, 95% CI [0.1449, 0.2223]. shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

selected warm: paired median 0.6701, 95% CI [0.6620, 0.6799]. shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

## Raw full and range, this commit versus aedcbda4

Both arms are `READCON_DB_COOKED_CODEC=raw`. Each arm reads the corpus it wrote. The ratio is this commit over `aedcbda4`. Arm order is tip, aedcbda4, reversed on odd indexes. The load average is the 1-minute value sampled before each pair.

| op | threads | aedcbda4 | 20a5f64e | tip/base | paired median | 95% CI | tip faster | load 1m median |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| full_read | unset | 1.9372e-01 (5.5205e-02) | 2.1115e-01 (4.6214e-02) | 1.0900 | 1.0977 | [0.9128, 1.2952] | 4/12 | 0.65 |
| range | unset | 5.7880e-02 (1.0519e-02) | 6.2235e-02 (1.0753e-02) | 1.0752 | 1.1855 | [0.9022, 1.3230] | 4/12 | 0.65 |
| full_read | 1 | 2.0143e-01 (3.7965e-02) | 1.9525e-01 (4.4610e-02) | 0.9693 | 0.9855 | [0.8080, 1.1856] | 6/12 | 0.69 |
| range | 1 | 5.6855e-02 (1.0798e-02) | 5.9571e-02 (1.1856e-02) | 1.0478 | 1.0806 | [0.7845, 1.1941] | 6/12 | 0.69 |

RAYON=unset raw full_read: paired median 1.0977, 95% CI [0.9128, 1.2952]. 20a5f64e faster in 4 of 12, slower in 8, ties 0. The paired interval includes 1.

RAYON=unset raw range: paired median 1.1855, 95% CI [0.9022, 1.3230]. 20a5f64e faster in 4 of 12, slower in 8, ties 0. The paired interval includes 1.

RAYON=1 raw full_read: paired median 0.9855, 95% CI [0.8080, 1.1856]. 20a5f64e faster in 6 of 12, slower in 6, ties 0. The paired interval includes 1.

RAYON=1 raw range: paired median 1.0806, 95% CI [0.7845, 1.1941]. 20a5f64e faster in 6 of 12, slower in 6, ties 0. The paired interval includes 1.

Per-sample 1, 5, and 15 minute load averages are in `ab.json` under `load_raw_unset` and `load_raw_rayon_1`.

## High byte, same columns on both sides

`high_byte.json` is one 100k-atom frame, positions `(i * 0.125, 0.5, 1.0)` and forces `(x * 0.5, 0.25, 0.5)`, byte-shuffled per lane. zstd level 3, window log 12, long-distance matching off. Each time is the mean of 20 rounds after 3 untimed rounds, and the parenthesis is the type-7 IQR of those rounds. A row's uncompressed size, full-shuffle size, and high-four size are the same lanes. The db24 sentence compared uncompressed forces with a high-four figure that also included positions. That mix is not repeated.

| columns | lanes | uncompressed | full shuffle zstd | full decode µs | high-four zstd | low-four raw | high-four decode+copy µs |
| --- | --- | --- | --- | --- | --- | --- | --- |
| positions_and_forces | 6 | 4800000 | 24418 | 354 (1) | 22219 | 2400000 | 330 (1) |
| forces | 3 | 2400000 | 12206 | 161 (1) | 11109 | 1200000 | 164 (1) |

Full shuffle stores only the zstd bytes. The high-four scheme stores the zstd of the high four bytes plus the low four bytes raw. On both column sets the full-shuffle blob is the smaller of the two.

## Sizes

| corpus | data.mdb |
| --- | --- |
| raw | 341966848 |
| shuffle | 36569088 |
| aedcbda4 | 341966848 |

`tables.json` is `fair_ase_protocol.record_table_usage`.
