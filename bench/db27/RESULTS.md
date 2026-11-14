# db27 realistic arms

Commit `a68606ec979bcb9dc22a17f1f1da6680aea5ed2f`, tree `81762171013a6278986d1d7bb4b930d3de1a2297`. Host Intel(R) Xeon(R) Processor, 8 CPUs. n=12, serial. Arm order is raw, shuffle, reversed on odd sample indexes. Warm selected is one pass of i*17 mod 64, and those seconds are the mean per frame. Cold advises DONTNEED outside the clock. The median is type-7. Parentheses are the type-7 IQR. The paired CI is a 10000-draw percentile bootstrap of the shuffle/raw ratio at the same sample index (numpy Generator seed 0). The lattice arm is the best case. The ratios to quote are md and thermal.

A frame under 64 KiB is not byte-shuffled. The md and thermal frames are 10 atoms, so `shuffle-zstd` stores a cooked group rather than component-major columns. The lattice frames are 100000 atoms and do shuffle.

## Sizes

| arm | role | raw data.mdb | shuffle data.mdb | shuffle/raw file | raw frames_soa | shuffle frames_soa | shuffle entries |
| --- | --- | --- | --- | --- | --- | --- | --- |
| md | realistic | 86016 | 86016 | 1.0000 | 8693 | 8693 | 1 |
| thermal | realistic | 90112 | 90112 | 1.0000 | 15044 | 15044 | 1 |
| lattice | best case | 341966848 | 36569088 | 0.1069 | 307208960 | 1583727 | 64 |

`tables.json` is `fair_ase_protocol.record_table_usage`.

## Realistic arms

| arm | op | cache | threads | raw | shuffle setting | shuffle/raw | paired median | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| md | full_read | cold | unset | 5.2343e-05 (1.5732e-06) | 5.1994e-05 (1.0293e-06) | 0.9933 | 0.9941 | [0.9707, 1.0284] | 7/12 |
| md | selected | cold | unset | 1.9143e-05 (4.7670e-07) | 1.9361e-05 (3.8097e-07) | 1.0114 | 1.0023 | [0.9872, 1.0318] | 5/12 |
| md | range | cold | unset | 3.3573e-05 (1.5267e-06) | 3.2675e-05 (1.0858e-06) | 0.9733 | 0.9746 | [0.9243, 1.0001] | 9/12 |
| md | full_read | warm | unset | 3.3340e-05 (1.3129e-07) | 3.3461e-05 (8.4743e-08) | 1.0036 | 1.0015 | [0.9995, 1.0058] | 4/12 |
| md | selected | warm | unset | 1.0698e-05 (3.7643e-07) | 1.0455e-05 (1.0969e-06) | 0.9773 | 0.9697 | [0.9257, 1.0348] | 7/12 |
| md | range | warm | unset | 1.7129e-05 (1.3969e-07) | 1.7164e-05 (9.3441e-08) | 1.0020 | 0.9989 | [0.9963, 1.0076] | 8/12 |
| md | full_read | cold | 1 | 5.2214e-05 (2.6279e-06) | 5.1758e-05 (3.1379e-06) | 0.9913 | 0.9842 | [0.9680, 1.0299] | 8/12 |
| md | selected | cold | 1 | 1.8960e-05 (2.2049e-06) | 2.0119e-05 (9.3447e-07) | 1.0611 | 1.0517 | [1.0124, 1.0705] | 2/12 |
| md | range | cold | 1 | 3.3293e-05 (1.1063e-06) | 3.5280e-05 (2.5916e-06) | 1.0597 | 1.0597 | [0.9859, 1.0974] | 4/12 |
| md | full_read | warm | 1 | 3.3679e-05 (7.1611e-07) | 3.3696e-05 (4.6005e-07) | 1.0005 | 0.9978 | [0.9823, 1.0052] | 7/12 |
| md | selected | warm | 1 | 1.0824e-05 (7.8343e-07) | 1.1160e-05 (4.3284e-07) | 1.0310 | 1.0521 | [1.0047, 1.1266] | 2/12 |
| md | range | warm | 1 | 1.7134e-05 (6.4394e-08) | 1.7203e-05 (1.9713e-07) | 1.0040 | 1.0025 | [0.9991, 1.0074] | 3/12 |
| thermal | full_read | cold | unset | 5.9558e-05 (5.7110e-06) | 5.8407e-05 (4.1071e-06) | 0.9807 | 0.9903 | [0.9144, 1.0554] | 7/12 |
| thermal | selected | cold | unset | 2.4727e-05 (1.1473e-06) | 2.3774e-05 (1.8107e-06) | 0.9615 | 0.9805 | [0.9104, 1.0159] | 9/12 |
| thermal | range | cold | unset | 3.9615e-05 (5.4346e-06) | 4.0832e-05 (5.7253e-06) | 1.0307 | 1.0016 | [0.9638, 1.0848] | 6/12 |
| thermal | full_read | warm | unset | 3.4645e-05 (6.2921e-06) | 3.3443e-05 (3.4411e-07) | 0.9653 | 0.9706 | [0.8154, 0.9854] | 11/12 |
| thermal | selected | warm | unset | 1.1950e-05 (1.6315e-06) | 1.0881e-05 (7.1894e-07) | 0.9105 | 0.9357 | [0.8902, 0.9631] | 12/12 |
| thermal | range | warm | unset | 1.8182e-05 (1.0478e-06) | 1.7601e-05 (1.9189e-07) | 0.9680 | 0.9666 | [0.9478, 0.9775] | 12/12 |
| thermal | full_read | cold | 1 | 5.4997e-05 (7.7804e-07) | 5.5954e-05 (1.2626e-06) | 1.0174 | 1.0224 | [1.0120, 1.0313] | 2/12 |
| thermal | selected | cold | 1 | 2.3288e-05 (1.3184e-06) | 2.3417e-05 (1.7994e-06) | 1.0055 | 0.9976 | [0.9749, 1.0674] | 6/12 |
| thermal | range | cold | 1 | 3.7201e-05 (1.4055e-06) | 3.7612e-05 (1.5125e-06) | 1.0111 | 1.0166 | [0.9876, 1.0325] | 5/12 |
| thermal | full_read | warm | 1 | 3.3787e-05 (7.1205e-07) | 3.4056e-05 (4.1911e-07) | 1.0080 | 1.0019 | [0.9895, 1.0109] | 5/12 |
| thermal | selected | warm | 1 | 1.0920e-05 (3.3359e-07) | 1.0482e-05 (3.0256e-07) | 0.9599 | 0.9715 | [0.9514, 0.9986] | 9/12 |
| thermal | range | warm | 1 | 1.7750e-05 (2.8017e-07) | 1.7747e-05 (1.3041e-07) | 0.9998 | 1.0001 | [0.9875, 1.0028] | 6/12 |

### md

alanine dipeptide heavy-atom MD, 64 frames, pinned sha256

md full_read cold threads=unset: paired median 0.9941, 95% CI [0.9707, 1.0284]. Shuffle setting faster in 7 of 12, slower in 5, ties 0. The paired interval includes 1.

md selected cold threads=unset: paired median 1.0023, 95% CI [0.9872, 1.0318]. Shuffle setting faster in 5 of 12, slower in 7, ties 0. The paired interval includes 1.

md range cold threads=unset: paired median 0.9746, 95% CI [0.9243, 1.0001]. Shuffle setting faster in 9 of 12, slower in 3, ties 0. The paired interval includes 1.

md full_read warm threads=unset: paired median 1.0015, 95% CI [0.9995, 1.0058]. Shuffle setting faster in 4 of 12, slower in 8, ties 0. The paired interval includes 1.

md selected warm threads=unset: paired median 0.9697, 95% CI [0.9257, 1.0348]. Shuffle setting faster in 7 of 12, slower in 5, ties 0. The paired interval includes 1.

md range warm threads=unset: paired median 0.9989, 95% CI [0.9963, 1.0076]. Shuffle setting faster in 8 of 12, slower in 4, ties 0. The paired interval includes 1.

md full_read cold threads=1: paired median 0.9842, 95% CI [0.9680, 1.0299]. Shuffle setting faster in 8 of 12, slower in 4, ties 0. The paired interval includes 1.

md selected cold threads=1: paired median 1.0517, 95% CI [1.0124, 1.0705]. Shuffle setting faster in 2 of 12, slower in 10, ties 0. The paired interval is above 1, so the shuffle setting is slower.

md range cold threads=1: paired median 1.0597, 95% CI [0.9859, 1.0974]. Shuffle setting faster in 4 of 12, slower in 8, ties 0. The paired interval includes 1.

md full_read warm threads=1: paired median 0.9978, 95% CI [0.9823, 1.0052]. Shuffle setting faster in 7 of 12, slower in 5, ties 0. The paired interval includes 1.

md selected warm threads=1: paired median 1.0521, 95% CI [1.0047, 1.1266]. Shuffle setting faster in 2 of 12, slower in 10, ties 0. The paired interval is above 1, so the shuffle setting is slower.

md range warm threads=1: paired median 1.0025, 95% CI [0.9991, 1.0074]. Shuffle setting faster in 3 of 12, slower in 9, ties 0. The paired interval includes 1.

### thermal

seeded 0.08 angstrom noise on the first MD frame, 64 frames

thermal full_read cold threads=unset: paired median 0.9903, 95% CI [0.9144, 1.0554]. Shuffle setting faster in 7 of 12, slower in 5, ties 0. The paired interval includes 1.

thermal selected cold threads=unset: paired median 0.9805, 95% CI [0.9104, 1.0159]. Shuffle setting faster in 9 of 12, slower in 3, ties 0. The paired interval includes 1.

thermal range cold threads=unset: paired median 1.0016, 95% CI [0.9638, 1.0848]. Shuffle setting faster in 6 of 12, slower in 6, ties 0. The paired interval includes 1.

thermal full_read warm threads=unset: paired median 0.9706, 95% CI [0.8154, 0.9854]. Shuffle setting faster in 11 of 12, slower in 1, ties 0. The paired interval is below 1, so the shuffle setting is faster.

thermal selected warm threads=unset: paired median 0.9357, 95% CI [0.8902, 0.9631]. Shuffle setting faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so the shuffle setting is faster.

thermal range warm threads=unset: paired median 0.9666, 95% CI [0.9478, 0.9775]. Shuffle setting faster in 12 of 12, slower in 0, ties 0. The paired interval is below 1, so the shuffle setting is faster.

thermal full_read cold threads=1: paired median 1.0224, 95% CI [1.0120, 1.0313]. Shuffle setting faster in 2 of 12, slower in 10, ties 0. The paired interval is above 1, so the shuffle setting is slower.

thermal selected cold threads=1: paired median 0.9976, 95% CI [0.9749, 1.0674]. Shuffle setting faster in 6 of 12, slower in 6, ties 0. The paired interval includes 1.

thermal range cold threads=1: paired median 1.0166, 95% CI [0.9876, 1.0325]. Shuffle setting faster in 5 of 12, slower in 7, ties 0. The paired interval includes 1.

thermal full_read warm threads=1: paired median 1.0019, 95% CI [0.9895, 1.0109]. Shuffle setting faster in 5 of 12, slower in 7, ties 0. The paired interval includes 1.

thermal selected warm threads=1: paired median 0.9715, 95% CI [0.9514, 0.9986]. Shuffle setting faster in 9 of 12, slower in 3, ties 0. The paired interval is below 1, so the shuffle setting is faster.

thermal range warm threads=1: paired median 1.0001, 95% CI [0.9875, 1.0028]. Shuffle setting faster in 6 of 12, slower in 6, ties 0. The paired interval includes 1.

## Lattice, best case

best case, not a ratio to quote: x=i*0.125, y and z constant per frame

| op | cache | threads | raw | shuffle-zstd | shuffle/raw | paired median | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| full_read | cold | unset | 5.2272e-01 (5.3129e-02) | 9.5911e-02 (7.1512e-02) | 0.1835 | 0.1522 | [0.1371, 0.2707] | 12/12 |
| selected | cold | unset | 5.6934e-03 (5.1065e-04) | 1.3142e-03 (8.1378e-05) | 0.2308 | 0.2364 | [0.2131, 0.2648] | 12/12 |
| range | cold | unset | 1.4233e-01 (1.0302e-02) | 1.8072e-02 (5.6130e-03) | 0.1270 | 0.1233 | [0.1039, 0.1562] | 12/12 |
| full_read | warm | unset | 1.9684e-01 (6.7872e-02) | 6.0390e-02 (2.1704e-02) | 0.3068 | 0.3010 | [0.2085, 0.4049] | 12/12 |
| selected | warm | unset | 2.4185e-03 (9.4676e-05) | 1.9331e-03 (7.5664e-05) | 0.7993 | 0.7908 | [0.7765, 0.8316] | 12/12 |
| range | warm | unset | 5.8799e-02 (1.4921e-02) | 1.5657e-02 (3.5867e-03) | 0.2663 | 0.3022 | [0.2280, 0.3464] | 12/12 |
| full_read | cold | 1 | 5.3267e-01 (1.2627e-01) | 1.7135e-01 (7.3550e-02) | 0.3217 | 0.3284 | [0.2660, 0.4223] | 12/12 |
| selected | cold | 1 | 7.0090e-03 (1.7371e-03) | 1.3606e-03 (1.9799e-04) | 0.1941 | 0.2003 | [0.1851, 0.2186] | 12/12 |
| range | cold | 1 | 1.4410e-01 (3.7097e-02) | 3.7913e-02 (2.1025e-02) | 0.2631 | 0.2847 | [0.2190, 0.3101] | 12/12 |
| full_read | warm | 1 | 2.0068e-01 (6.1974e-02) | 1.3964e-01 (3.1449e-02) | 0.6959 | 0.6704 | [0.6369, 0.7180] | 11/12 |
| selected | warm | 1 | 2.4853e-03 (1.3847e-04) | 1.9764e-03 (5.5497e-05) | 0.7953 | 0.7989 | [0.7562, 0.8217] | 12/12 |
| range | warm | 1 | 5.9382e-02 (9.8040e-03) | 3.2485e-02 (1.0176e-02) | 0.5471 | 0.4673 | [0.4138, 0.6974] | 12/12 |
