# db28 realistic arms at lattice scale

Commit `043c3091c9606293a920311a73a8a622d4638746`, tree `a201b635a8bfaa444623c2530e6c10592a423c3f`. Host Intel(R) Xeon(R) Processor, 8 CPUs. n=12, serial. The same reads run on every arm. Arm order within a sample is raw, shuffle, reversed on odd indexes. Warm selected seconds are the mean per frame. The median is type-7. Parentheses are the type-7 IQR. The paired CI is a 10000-draw percentile bootstrap, type-7 median, numpy Generator seed 0. The lattice arm is the best case. Quote md and thermal.

md is the CC-BY 4.0 NhaA membrane trajectory (Ian Kenney and Shujie Fan, figshare 7185203): 60702 protein, lipid, and ion atoms, water removed by the authors, 211 strided frames. thermal is seeded 0.08 angstrom noise on that structure, 211 frames.

## Bytes and codec

| arm | role | codec stored | atoms/frame | coord bytes/frame | corpus bytes | frames_soa bytes | stored frame bytes | hit shuffle | peak RSS |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| md | realistic | raw | 60702 | 1456848 | 614641664 | 409898306 | 1942646 | False | 2936991744 |
| md | realistic | shuffle-zstd | 60702 | 1456848 | 416727040 | 212073873 | 1005089 | True | 2583625728 |
| thermal | realistic | raw | 60702 | 1456848 | 779874304 | 409898306 | 1942646 | False | 1786232832 |
| thermal | realistic | shuffle-zstd | 60702 | 1456848 | 722833408 | 353274932 | 1674289 | True | 1707507712 |
| lattice | best case | raw | 100000 | 4800000 | 341966848 | 307208960 | 4800140 | False | 1349955584 |
| lattice | best case | shuffle-zstd | 100000 | 4800000 | 36569088 | 1583727 | 24746 | True | 860028928 |

## Headline ratios

shuffle/raw. File ratio is corpus bytes. Time ratio is the paired median.

| arm | role | file | full cold unset | full warm unset | full cold 1 | full warm 1 |
| --- | --- | --- | --- | --- | --- | --- |
| md | realistic | 0.6780 | 0.4290 [0.3856, 0.5079] 12/12 | 0.6681 [0.5238, 0.7814] 11/12 | 0.6850 [0.6264, 0.7952] 10/12 | 1.2718 [1.0548, 1.6001] 2/12 |
| thermal | realistic | 0.9269 | 0.6204 [0.4653, 0.7516] 12/12 | 0.6738 [0.4985, 0.8160] 12/12 | 0.9149 [0.7004, 1.0708] 8/12 | 1.0325 [0.9084, 1.4689] 6/12 |
| lattice | best case | 0.1069 | 0.1511 [0.1369, 0.2153] 12/12 | 0.2653 [0.2452, 0.3474] 12/12 | 0.3944 [0.3289, 0.4796] 12/12 | 0.6793 [0.6089, 0.7356] 12/12 |

## All reads

| arm | op | cache | threads | raw s | shuffle s | shuffle/raw | paired median | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| md | full_read | cold | unset | 7.5717e-01 (2.3349e-01) | 3.4919e-01 (6.1722e-02) | 0.4612 | 0.4290 | [0.3856, 0.5079] | 12/12 |
| md | selected | cold | unset | 3.6297e-03 (6.3494e-04) | 2.7250e-03 (2.7874e-04) | 0.7508 | 0.8003 | [0.6950, 0.9149] | 11/12 |
| md | range | cold | unset | 1.8918e-01 (3.3840e-02) | 9.6596e-02 (1.6181e-02) | 0.5106 | 0.5088 | [0.4500, 0.5808] | 12/12 |
| md | full_read | warm | unset | 3.6397e-01 (1.1366e-01) | 2.2637e-01 (5.7300e-02) | 0.6219 | 0.6681 | [0.5238, 0.7814] | 11/12 |
| md | selected | warm | unset | 1.1994e-03 (8.4827e-05) | 1.6010e-03 (4.1646e-05) | 1.3348 | 1.3663 | [1.2696, 1.4097] | 1/12 |
| md | range | warm | unset | 5.9092e-02 (1.7644e-02) | 3.5202e-02 (6.6586e-03) | 0.5957 | 0.5774 | [0.5083, 0.7049] | 12/12 |
| md | full_read | cold | 1 | 8.2661e-01 (1.8054e-01) | 5.8997e-01 (2.3277e-01) | 0.7137 | 0.6850 | [0.6264, 0.7952] | 10/12 |
| md | selected | cold | 1 | 3.5965e-03 (4.6145e-04) | 2.9358e-03 (2.5411e-04) | 0.8163 | 0.8148 | [0.7026, 0.8404] | 12/12 |
| md | range | cold | 1 | 1.8768e-01 (2.7734e-02) | 1.3911e-01 (1.9254e-02) | 0.7412 | 0.7019 | [0.6275, 0.7511] | 12/12 |
| md | full_read | warm | 1 | 3.4743e-01 (8.8435e-02) | 4.2972e-01 (6.8112e-02) | 1.2368 | 1.2718 | [1.0548, 1.6001] | 2/12 |
| md | selected | warm | 1 | 1.1673e-03 (2.3673e-05) | 1.6030e-03 (1.4619e-05) | 1.3733 | 1.3702 | [1.3455, 1.4005] | 0/12 |
| md | range | warm | 1 | 5.9842e-02 (1.5473e-02) | 8.3253e-02 (7.4639e-03) | 1.3912 | 1.3432 | [1.0834, 1.6116] | 3/12 |
| thermal | full_read | cold | unset | 7.7831e-01 (1.1312e-01) | 4.5282e-01 (1.7636e-01) | 0.5818 | 0.6204 | [0.4653, 0.7516] | 12/12 |
| thermal | selected | cold | unset | 3.7199e-03 (4.5220e-04) | 3.4533e-03 (6.8149e-04) | 0.9283 | 0.9313 | [0.8140, 0.9822] | 10/12 |
| thermal | range | cold | unset | 1.7420e-01 (3.6560e-02) | 9.9255e-02 (8.5496e-03) | 0.5698 | 0.5871 | [0.5564, 0.6432] | 12/12 |
| thermal | full_read | warm | unset | 3.2543e-01 (9.0912e-02) | 2.0257e-01 (5.3449e-02) | 0.6225 | 0.6738 | [0.4985, 0.8160] | 12/12 |
| thermal | selected | warm | unset | 1.1636e-03 (3.2121e-05) | 1.3859e-03 (3.3189e-05) | 1.1910 | 1.1933 | [1.1730, 1.2222] | 0/12 |
| thermal | range | warm | unset | 5.9427e-02 (1.2823e-02) | 3.6167e-02 (7.0208e-03) | 0.6086 | 0.6347 | [0.5321, 0.7028] | 12/12 |
| thermal | full_read | cold | 1 | 7.2283e-01 (6.4718e-02) | 7.1280e-01 (2.7982e-01) | 0.9861 | 0.9149 | [0.7004, 1.0708] | 8/12 |
| thermal | selected | cold | 1 | 3.5044e-03 (3.4818e-04) | 3.0268e-03 (7.8766e-04) | 0.8637 | 0.8870 | [0.7775, 1.0334] | 9/12 |
| thermal | range | cold | 1 | 1.7878e-01 (3.3464e-02) | 1.4807e-01 (1.3298e-02) | 0.8282 | 0.8338 | [0.7091, 0.8595] | 12/12 |
| thermal | full_read | warm | 1 | 3.1567e-01 (7.3985e-02) | 3.3533e-01 (6.5866e-02) | 1.0623 | 1.0325 | [0.9084, 1.4689] | 6/12 |
| thermal | selected | warm | 1 | 1.1419e-03 (1.4680e-05) | 1.3626e-03 (1.7066e-05) | 1.1933 | 1.1950 | [1.1894, 1.2101] | 1/12 |
| thermal | range | warm | 1 | 5.8786e-02 (1.5347e-02) | 7.0352e-02 (8.8235e-03) | 1.1967 | 1.1200 | [1.0636, 1.4057] | 2/12 |
| lattice | full_read | cold | unset | 4.7501e-01 (1.2952e-01) | 8.2639e-02 (2.0222e-02) | 0.1740 | 0.1511 | [0.1369, 0.2153] | 12/12 |
| lattice | selected | cold | unset | 5.2156e-03 (7.1780e-04) | 1.4136e-03 (9.1172e-05) | 0.2710 | 0.2816 | [0.2491, 0.2880] | 12/12 |
| lattice | range | cold | unset | 1.0440e-01 (2.7445e-02) | 1.8639e-02 (3.4055e-03) | 0.1785 | 0.1747 | [0.1469, 0.2050] | 12/12 |
| lattice | full_read | warm | unset | 2.0996e-01 (1.8173e-02) | 5.8947e-02 (1.9292e-02) | 0.2808 | 0.2653 | [0.2452, 0.3474] | 12/12 |
| lattice | selected | warm | unset | 2.4340e-03 (7.9831e-05) | 1.9674e-03 (8.6391e-05) | 0.8083 | 0.8209 | [0.7843, 0.8298] | 12/12 |
| lattice | range | warm | unset | 6.1599e-02 (1.1089e-02) | 1.4994e-02 (2.3754e-03) | 0.2434 | 0.2411 | [0.2126, 0.2949] | 12/12 |
| lattice | full_read | cold | 1 | 4.7894e-01 (7.4059e-02) | 1.8106e-01 (5.0364e-02) | 0.3780 | 0.3944 | [0.3289, 0.4796] | 12/12 |
| lattice | selected | cold | 1 | 5.5631e-03 (8.5126e-04) | 1.3930e-03 (1.9512e-04) | 0.2504 | 0.2457 | [0.2084, 0.2562] | 12/12 |
| lattice | range | cold | 1 | 1.2993e-01 (2.1285e-02) | 3.4457e-02 (5.1490e-03) | 0.2652 | 0.2537 | [0.2406, 0.2850] | 12/12 |
| lattice | full_read | warm | 1 | 2.0277e-01 (3.0801e-02) | 1.3828e-01 (5.5175e-03) | 0.6819 | 0.6793 | [0.6089, 0.7356] | 12/12 |
| lattice | selected | warm | 1 | 2.4425e-03 (8.1412e-05) | 1.9508e-03 (1.1543e-04) | 0.7987 | 0.7934 | [0.7633, 0.8399] | 12/12 |
| lattice | range | warm | 1 | 6.0511e-02 (1.6866e-02) | 3.4016e-02 (6.7105e-03) | 0.5621 | 0.5061 | [0.4270, 0.6697] | 11/12 |
