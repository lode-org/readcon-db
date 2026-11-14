# db28 realistic arms at lattice scale

Commit `73de73cfd7d83dcb4346143902698bc4f5cb8ec7`, tree `c01190d6eb414be636e1d243af86763bb28f209e`. Host Intel(R) Xeon(R) Processor, 8 CPUs. n=6, serial. The same reads run on every arm. Arm order within a sample is raw, shuffle, reversed on odd indexes. Warm selected seconds are the mean per frame. The median is type-7. Parentheses are the type-7 IQR. The paired CI is a 10000-draw percentile bootstrap, type-7 median, numpy Generator seed 0. The lattice arm is the best case. Quote md and thermal.

md is the CC-BY 4.0 NhaA membrane trajectory (Ian Kenney and Shujie Fan, figshare 7185203): 60702 protein, lipid, and ion atoms, water removed by the authors, 211 strided frames. thermal is seeded 0.08 angstrom noise on that structure, 211 frames.

## Bytes and codec

| arm | role | codec stored | atoms/frame | coord bytes/frame | corpus bytes | frames_soa bytes | stored frame bytes | hit shuffle | peak RSS |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| md | realistic | raw | 60702 | 1456848 | 614641664 | 409898306 | 1942646 | False | 2938327040 |
| md | realistic | shuffle-zstd | 60702 | 1456848 | 416727040 | 212073873 | 1005089 | True | 2569400320 |
| thermal | realistic | raw | 60702 | 1456848 | 779874304 | 409898306 | 1942646 | False | 1787424768 |
| thermal | realistic | shuffle-zstd | 60702 | 1456848 | 722833408 | 353274932 | 1674289 | True | 1674690560 |
| lattice | best case | raw | 100000 | 4800000 | 341966848 | 307208960 | 4800140 | False | 1350877184 |
| lattice | best case | shuffle-zstd | 100000 | 4800000 | 36569088 | 1583727 | 24746 | True | 867655680 |

## Headline ratios

shuffle/raw. File ratio is corpus bytes. Time ratio is the paired median.

| arm | role | file | full cold unset | full warm unset | full cold 1 | full warm 1 |
| --- | --- | --- | --- | --- | --- | --- |
| md | realistic | 0.6780 | 0.5076 [0.3764, 0.9408] 5/6 | 0.5093 [0.4388, 0.7948] 6/6 | 0.6762 [0.5699, 0.8341] 6/6 | 1.2143 [0.8551, 2.2855] 1/6 |
| thermal | realistic | 0.9269 | 0.6237 [0.5484, 0.8142] 6/6 | 0.7112 [0.4425, 0.8392] 6/6 | 0.8863 [0.6384, 1.2067] 4/6 | 1.0737 [0.8900, 1.6800] 2/6 |
| lattice | best case | 0.1069 | 0.1573 [0.1230, 0.2040] 6/6 | 0.2869 [0.2361, 0.3915] 6/6 | 0.3438 [0.2795, 0.5011] 6/6 | 0.6457 [0.4586, 0.7661] 6/6 |

## All reads

| arm | op | cache | threads | raw s | shuffle s | shuffle/raw | paired median | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| md | full_read | cold | unset | 9.1955e-01 (2.2054e-01) | 3.8129e-01 (1.4583e-01) | 0.4146 | 0.5076 | [0.3764, 0.9408] | 5/6 |
| md | selected | cold | unset | 3.1480e-03 (1.7135e-04) | 2.4457e-03 (2.4640e-04) | 0.7769 | 0.7776 | [0.7390, 0.8517] | 6/6 |
| md | range | cold | unset | 1.4263e-01 (3.7016e-02) | 8.4516e-02 (1.6675e-02) | 0.5925 | 0.5027 | [0.4067, 0.6423] | 6/6 |
| md | full_read | warm | unset | 3.5216e-01 (1.0927e-01) | 1.9308e-01 (9.2010e-03) | 0.5483 | 0.5093 | [0.4388, 0.7948] | 6/6 |
| md | selected | warm | unset | 1.1729e-03 (2.9015e-05) | 1.6011e-03 (2.3456e-05) | 1.3651 | 1.3768 | [1.2605, 1.3897] | 0/6 |
| md | range | warm | unset | 6.0636e-02 (6.5124e-03) | 3.4724e-02 (4.2227e-03) | 0.5727 | 0.5047 | [0.4510, 0.6527] | 6/6 |
| md | full_read | cold | 1 | 1.0178e+00 (1.8125e-01) | 6.3227e-01 (7.2924e-02) | 0.6212 | 0.6762 | [0.5699, 0.8341] | 6/6 |
| md | selected | cold | 1 | 3.8672e-03 (7.4019e-04) | 2.9125e-03 (1.4446e-04) | 0.7531 | 0.7335 | [0.5774, 0.9340] | 5/6 |
| md | range | cold | 1 | 2.0721e-01 (3.7170e-03) | 1.4833e-01 (5.9471e-03) | 0.7158 | 0.7003 | [0.6531, 0.7259] | 6/6 |
| md | full_read | warm | 1 | 3.1981e-01 (9.3385e-02) | 3.8910e-01 (4.6797e-02) | 1.2166 | 1.2143 | [0.8551, 2.2855] | 1/6 |
| md | selected | warm | 1 | 1.1942e-03 (2.7977e-05) | 1.5943e-03 (9.0381e-05) | 1.3350 | 1.3659 | [1.3202, 1.4233] | 0/6 |
| md | range | warm | 1 | 6.3138e-02 (2.8516e-02) | 8.9683e-02 (1.9793e-02) | 1.4204 | 1.4305 | [1.0610, 1.8484] | 1/6 |
| thermal | full_read | cold | unset | 9.1625e-01 (3.2136e-01) | 5.8869e-01 (1.6267e-01) | 0.6425 | 0.6237 | [0.5484, 0.8142] | 6/6 |
| thermal | selected | cold | unset | 3.7327e-03 (1.1311e-03) | 3.6843e-03 (3.6792e-04) | 0.9870 | 0.9983 | [0.6267, 1.1387] | 3/6 |
| thermal | range | cold | unset | 1.9060e-01 (2.3331e-02) | 1.0634e-01 (2.6040e-02) | 0.5579 | 0.5941 | [0.5145, 0.7433] | 6/6 |
| thermal | full_read | warm | unset | 3.1515e-01 (1.0029e-01) | 2.0027e-01 (4.9040e-02) | 0.6355 | 0.7112 | [0.4425, 0.8392] | 6/6 |
| thermal | selected | warm | unset | 1.2608e-03 (1.7408e-04) | 1.4222e-03 (1.4351e-05) | 1.1280 | 1.1288 | [0.9943, 1.2288] | 2/6 |
| thermal | range | warm | unset | 6.0151e-02 (2.8785e-02) | 3.8180e-02 (7.7095e-03) | 0.6347 | 0.5206 | [0.4832, 0.7743] | 6/6 |
| thermal | full_read | cold | 1 | 1.0133e+00 (3.0190e-01) | 8.5053e-01 (3.4001e-01) | 0.8393 | 0.8863 | [0.6384, 1.2067] | 4/6 |
| thermal | selected | cold | 1 | 4.1015e-03 (6.7043e-05) | 3.6507e-03 (8.2864e-04) | 0.8901 | 0.8906 | [0.8582, 1.5743] | 4/6 |
| thermal | range | cold | 1 | 1.9248e-01 (3.9110e-02) | 1.7332e-01 (4.5625e-02) | 0.9005 | 0.8898 | [0.7835, 1.1283] | 5/6 |
| thermal | full_read | warm | 1 | 3.5670e-01 (6.3641e-02) | 3.8669e-01 (6.4432e-02) | 1.0841 | 1.0737 | [0.8900, 1.6800] | 2/6 |
| thermal | selected | warm | 1 | 1.2348e-03 (1.2000e-04) | 1.4242e-03 (1.3825e-04) | 1.1534 | 1.1760 | [1.1341, 1.2482] | 0/6 |
| thermal | range | warm | 1 | 6.0553e-02 (2.5342e-02) | 6.9468e-02 (1.1436e-02) | 1.1472 | 1.2296 | [0.8964, 1.7726] | 1/6 |
| lattice | full_read | cold | unset | 4.7480e-01 (5.1767e-02) | 7.2429e-02 (2.2488e-02) | 0.1525 | 0.1573 | [0.1230, 0.2040] | 6/6 |
| lattice | selected | cold | unset | 5.8245e-03 (9.4985e-04) | 1.3175e-03 (1.1661e-04) | 0.2262 | 0.2355 | [0.1933, 0.3035] | 6/6 |
| lattice | range | cold | unset | 1.3379e-01 (2.2671e-02) | 1.9316e-02 (6.7523e-03) | 0.1444 | 0.1423 | [0.1179, 0.1714] | 6/6 |
| lattice | full_read | warm | unset | 2.0068e-01 (2.0942e-02) | 5.7682e-02 (1.6636e-02) | 0.2874 | 0.2869 | [0.2361, 0.3915] | 6/6 |
| lattice | selected | warm | unset | 2.3838e-03 (4.2586e-05) | 1.8686e-03 (9.4171e-05) | 0.7839 | 0.7800 | [0.7594, 0.8541] | 6/6 |
| lattice | range | warm | unset | 5.9643e-02 (6.5348e-03) | 1.4596e-02 (3.1625e-03) | 0.2447 | 0.2735 | [0.2109, 0.3264] | 6/6 |
| lattice | full_read | cold | 1 | 5.3261e-01 (2.7126e-02) | 1.7165e-01 (8.7873e-02) | 0.3223 | 0.3438 | [0.2795, 0.5011] | 6/6 |
| lattice | selected | cold | 1 | 5.5508e-03 (1.6212e-03) | 1.3905e-03 (1.2512e-04) | 0.2505 | 0.2505 | [0.1689, 0.2679] | 6/6 |
| lattice | range | cold | 1 | 1.3785e-01 (2.3710e-02) | 3.6110e-02 (9.0104e-03) | 0.2619 | 0.2630 | [0.2198, 0.3383] | 6/6 |
| lattice | full_read | warm | 1 | 2.0015e-01 (4.1736e-02) | 1.3099e-01 (6.0329e-03) | 0.6545 | 0.6457 | [0.4586, 0.7661] | 6/6 |
| lattice | selected | warm | 1 | 2.3049e-03 (6.2662e-05) | 1.8261e-03 (2.3982e-05) | 0.7923 | 0.7917 | [0.7715, 0.8008] | 6/6 |
| lattice | range | warm | 1 | 5.5709e-02 (7.9692e-03) | 2.7187e-02 (9.5860e-03) | 0.4880 | 0.4457 | [0.4233, 0.6970] | 6/6 |
