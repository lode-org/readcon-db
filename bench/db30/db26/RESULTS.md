# db26 selected, cold, and raw across commits

Commit `043c3091c9606293a920311a73a8a622d4638746`, tree `a201b635a8bfaa444623c2530e6c10592a423c3f`. Raw comparison base `aedcbda468d208278b8699b80c6db743edcdc21e` tree `4ec91d84f06cb9dada3326231d811d9ea0063534`. Host Linux 6.12.113+, Intel(R) Xeon(R) Processor, 8 CPUs. n=12, serial. The median is type-7. Parentheses are the type-7 IQR. The paired CI is a 10000-draw percentile bootstrap of the ratio at the same sample index (numpy Generator seed 0). Odd sample indexes reverse the arm order. Load average at the start of the timed block was 0.49, 0.68, 0.72.

db24 and db25 are unchanged. db25 did not repeat selected or cold. Those rows are here because `d671c32` unshuffles into the caller buffer. That commit is in this history, and this measurement is `20a5f64e`.

## Summary

| comparison | op | threads | cache | ratio of medians | paired median | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- |
| shuffle/raw | selected | unset | cold | 0.2470 | 0.2216 | [0.2138, 0.2663] | 12/12 |
| shuffle/raw | full_read | unset | cold | 0.1688 | 0.1676 | [0.1413, 0.1888] | 12/12 |
| shuffle/raw | range | unset | cold | 0.1653 | 0.1626 | [0.1417, 0.1766] | 12/12 |
| shuffle/raw | selected | unset | warm | 0.7854 | 0.7684 | [0.7497, 0.8120] | 12/12 |
| 20a5f64e/aedcbda4 raw | full_read | unset | warm | 0.9815 | 0.9994 | [0.8323, 1.2318] | 6/12 |
| 20a5f64e/aedcbda4 raw | range | unset | warm | 1.0231 | 1.0051 | [0.8337, 1.2052] | 6/12 |
| 20a5f64e/aedcbda4 raw | full_read | 1 | warm | 0.9908 | 0.9583 | [0.8634, 1.2181] | 7/12 |
| 20a5f64e/aedcbda4 raw | range | 1 | warm | 1.0546 | 1.1284 | [0.8774, 1.2956] | 4/12 |

## Shuffle versus raw

Arm order is raw, shuffle, reversed on odd indexes. Warm selected is one pass of i*17 mod 64, and the seconds are the mean per frame. Cold advises `DONTNEED` outside the clock, then opens and reads inside it. There is no frame cache.

| op | cache | raw | shuffle-zstd | shuffle/raw | paired median | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- |
| full_read | cold | 5.0199e-01 (3.5474e-02) | 8.4714e-02 (1.0048e-02) | 0.1688 | 0.1676 | [0.1413, 0.1888] | 12/12 |
| selected | cold | 5.5641e-03 (7.9498e-04) | 1.3745e-03 (1.7028e-04) | 0.2470 | 0.2216 | [0.2138, 0.2663] | 12/12 |
| range | cold | 1.3941e-01 (2.1516e-02) | 2.3045e-02 (6.8546e-03) | 0.1653 | 0.1626 | [0.1417, 0.1766] | 12/12 |
| selected | warm | 2.7332e-03 (1.6772e-04) | 2.1465e-03 (7.1492e-05) | 0.7854 | 0.7684 | [0.7497, 0.8120] | 12/12 |

full_read cold: paired median 0.1676, 95% CI [0.1413, 0.1888]. shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

selected cold: paired median 0.2216, 95% CI [0.2138, 0.2663]. shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

range cold: paired median 0.1626, 95% CI [0.1417, 0.1766]. shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

selected warm: paired median 0.7684, 95% CI [0.7497, 0.8120]. shuffle faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so shuffle is faster.

## Raw full and range, this commit versus aedcbda4

Both arms are `READCON_DB_COOKED_CODEC=raw`. Each arm reads the corpus it wrote. The ratio is this commit over `aedcbda4`. Arm order is tip, aedcbda4, reversed on odd indexes. The load average is the 1-minute value sampled before each pair.

| op | threads | aedcbda4 | 20a5f64e | tip/base | paired median | 95% CI | tip faster | load 1m median |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| full_read | unset | 2.2166e-01 (4.1373e-02) | 2.1756e-01 (3.7001e-02) | 0.9815 | 0.9994 | [0.8323, 1.2318] | 6/12 | 0.69 |
| range | unset | 6.2529e-02 (8.0475e-03) | 6.3972e-02 (1.1449e-02) | 1.0231 | 1.0051 | [0.8337, 1.2052] | 6/12 | 0.69 |
| full_read | 1 | 2.3032e-01 (5.9829e-02) | 2.2821e-01 (5.0561e-02) | 0.9908 | 0.9583 | [0.8634, 1.2181] | 7/12 | 0.74 |
| range | 1 | 6.1758e-02 (8.6173e-03) | 6.5133e-02 (1.2476e-02) | 1.0546 | 1.1284 | [0.8774, 1.2956] | 4/12 | 0.74 |

RAYON=unset raw full_read: paired median 0.9994, 95% CI [0.8323, 1.2318]. 20a5f64e faster in 6 of 12, slower in 6, ties 0. The paired interval includes 1.

RAYON=unset raw range: paired median 1.0051, 95% CI [0.8337, 1.2052]. 20a5f64e faster in 6 of 12, slower in 6, ties 0. The paired interval includes 1.

RAYON=1 raw full_read: paired median 0.9583, 95% CI [0.8634, 1.2181]. 20a5f64e faster in 7 of 12, slower in 5, ties 0. The paired interval includes 1.

RAYON=1 raw range: paired median 1.1284, 95% CI [0.8774, 1.2956]. 20a5f64e faster in 4 of 12, slower in 8, ties 0. The paired interval includes 1.

Per-sample 1, 5, and 15 minute load averages are in `ab.json` under `load_raw_unset` and `load_raw_rayon_1`.

## High byte, same columns on both sides

`high_byte.json` is one 100k-atom frame, positions `(i * 0.125, 0.5, 1.0)` and forces `(x * 0.5, 0.25, 0.5)`, byte-shuffled per lane. zstd level 3, window log 12, long-distance matching off. Each time is the mean of 20 rounds after 3 untimed rounds, and the parenthesis is the type-7 IQR of those rounds. A row's uncompressed size, full-shuffle size, and high-four size are the same lanes. The db24 sentence compared uncompressed forces with a high-four figure that also included positions. That mix is not repeated.

| columns | lanes | uncompressed | full shuffle zstd | full decode µs | high-four zstd | low-four raw | high-four decode+copy µs |
| --- | --- | --- | --- | --- | --- | --- | --- |
| positions_and_forces | 6 | 4800000 | 24418 | 355 (1) | 22219 | 2400000 | 343 (1) |
| forces | 3 | 2400000 | 12206 | 161 (1) | 11109 | 1200000 | 167 (0) |

Full shuffle stores only the zstd bytes. The high-four scheme stores the zstd of the high four bytes plus the low four bytes raw. On both column sets the full-shuffle blob is the smaller of the two.

## Sizes

| corpus | data.mdb |
| --- | --- |
| raw | 341966848 |
| shuffle | 36569088 |
| aedcbda4 | 341966848 |

`tables.json` is `fair_ase_protocol.record_table_usage`.
