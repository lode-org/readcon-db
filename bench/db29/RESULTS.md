# db29 realistic arms

Commit `67dc0235955e6c0f50a8e814d4d489fbb387ec65`, tree `99addc975a22943f44aa024dc4b718d6d5052579`. Host label `host`, Intel(R) Xeon(R) Processor, 8 CPUs. n=12, serial. Warm reads are full, range, and selected. Cold bulk is the full read. Threads are the process default and `RAYON_NUM_THREADS=1`. The paired CI is a 10000-draw percentile bootstrap, type-7 median, numpy Generator seed 0. The ratio is the right-hand layout over the left. Faster counts the right-hand layout.

raw and shuffle-zstd are `READCON_DB_COOKED_CODEC`. warm-point and cold-bulk are `set_cooked_access` with that variable unset. The lattice arm is the best case. Quote md and thermal.

## Bytes and peak RSS

| arm | layout | codec stored | atoms/frame | corpus bytes | frames_soa bytes | hit shuffle | peak RSS |
| --- | --- | --- | --- | --- | --- | --- | --- |
| md | raw | raw | 60702 | 614641664 | 409898306 | False | 2938003456 |
| md | shuffle | shuffle-zstd | 60702 | 416727040 | 212073873 | True | 2587848704 |
| md | warm-point | raw | 60702 | 614641664 | 409898306 | False | 2938195968 |
| md | cold-bulk | shuffle-zstd | 60702 | 416727040 | 212073873 | True | 2579668992 |
| thermal | raw | raw | 60702 | 779874304 | 409898306 | False | 1760858112 |
| thermal | shuffle | shuffle-zstd | 60702 | 722833408 | 353274932 | True | 1704255488 |
| thermal | warm-point | raw | 60702 | 779874304 | 409898306 | False | 1775996928 |
| thermal | cold-bulk | shuffle-zstd | 60702 | 722833408 | 353274932 | True | 1667837952 |
| lattice | raw | raw | 100000 | 341966848 | 307208960 | False | 1351311360 |
| lattice | shuffle | shuffle-zstd | 100000 | 36569088 | 1583727 | True | 921178112 |
| lattice | warm-point | raw | 100000 | 341966848 | 307208960 | False | 1351729152 |
| lattice | cold-bulk | shuffle-zstd | 100000 | 36569088 | 1583727 | True | 904183808 |

## Float32 XTC widened to float64

The NhaA XTC stores coordinates as float32 nanometres. The arm widens them to float64 and multiplies by 10. A widened float32 keeps zeros in the low bytes. Thermal noise fills those bytes, so the same atoms compress less. Lane 0 is the least-significant byte.

- md stored angstrom: n=38424366 low32=0.027212706645569636 low29=0.21440957542409417 low24=1.0 lanes=[1.000, 1.000, 1.000, 0.027, 0.040, 0.008, 0.001, 0.000]
- thermal stored angstrom: n=38424366 low32=0.0 low29=0.0 low24=1.0410061157547791e-07 lanes=[0.004, 0.004, 0.004, 0.004, 0.004, 0.004, 0.001, 0.000]
- lattice stored angstrom: n=19200000 low32=0.9999933333333333 low29=0.9999933333333333 low24=0.9999933333333333 lanes=[1.000, 1.000, 1.000, 1.000, 0.742, 0.585, 0.005, 0.000]
- md widened nm, before times 10: n=38424366 low32=0.12535514574267798 low29=1.0 low24=1.0 lanes=[1.000, 1.000, 1.000, 0.125, 0.008, 0.005, 0.007, 0.000]
- md times 10 angstrom, from the XTC: n=38424366 low32=0.027212706645569636 low29=0.21440957542409417 low24=1.0 lanes=[1.000, 1.000, 1.000, 0.027, 0.040, 0.008, 0.001, 0.000]

## raw vs shuffle-zstd

| arm | op | cache | threads | raw s | shuffle s | paired | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| md | full_read | cold | unset | 7.6382e-01 | 4.1213e-01 | 0.5271 | [0.4934, 0.5850] | 11/12 |
| md | full_read | warm | unset | 3.6771e-01 | 2.0489e-01 | 0.6831 | [0.4502, 0.8064] | 11/12 |
| md | range | warm | unset | 5.9293e-02 | 3.4448e-02 | 0.6034 | [0.5006, 0.7480] | 10/12 |
| md | selected | warm | unset | 1.2154e-03 | 1.6895e-03 | 1.3918 | [1.3264, 1.4555] | 0/12 |
| md | full_read | cold | 1 | 7.5348e-01 | 6.3851e-01 | 0.7710 | [0.7199, 0.8562] | 11/12 |
| md | full_read | warm | 1 | 3.1888e-01 | 4.3302e-01 | 1.3963 | [1.0597, 1.7198] | 1/12 |
| md | range | warm | 1 | 5.9227e-02 | 8.8018e-02 | 1.3960 | [1.2045, 1.8276] | 0/12 |
| md | selected | warm | 1 | 1.1860e-03 | 1.6239e-03 | 1.3763 | [1.3379, 1.4102] | 0/12 |
| thermal | full_read | cold | unset | 7.1725e-01 | 3.9662e-01 | 0.5295 | [0.4739, 0.5906] | 12/12 |
| thermal | full_read | warm | unset | 2.7960e-01 | 1.6445e-01 | 0.5899 | [0.4780, 0.6797] | 12/12 |
| thermal | range | warm | unset | 5.7684e-02 | 3.4514e-02 | 0.5811 | [0.5388, 0.7201] | 12/12 |
| thermal | selected | warm | unset | 1.2140e-03 | 1.4651e-03 | 1.2026 | [1.1590, 1.2695] | 0/12 |
| thermal | full_read | cold | 1 | 8.0900e-01 | 6.1010e-01 | 0.7879 | [0.6833, 0.9359] | 11/12 |
| thermal | full_read | warm | 1 | 3.1675e-01 | 3.4596e-01 | 1.0511 | [0.8757, 1.5254] | 6/12 |
| thermal | range | warm | 1 | 6.0051e-02 | 6.9936e-02 | 1.1484 | [1.0126, 1.4957] | 2/12 |
| thermal | selected | warm | 1 | 1.1824e-03 | 1.3825e-03 | 1.1543 | [1.1400, 1.1913] | 0/12 |
| lattice | full_read | cold | unset | 4.7405e-01 | 8.1552e-02 | 0.1680 | [0.1408, 0.1820] | 12/12 |
| lattice | full_read | warm | unset | 1.9756e-01 | 6.0050e-02 | 0.2958 | [0.2636, 0.3275] | 12/12 |
| lattice | range | warm | unset | 5.8897e-02 | 1.6510e-02 | 0.2644 | [0.2555, 0.3506] | 12/12 |
| lattice | selected | warm | unset | 2.3710e-03 | 1.8673e-03 | 0.7888 | [0.7793, 0.8040] | 12/12 |
| lattice | full_read | cold | 1 | 5.1188e-01 | 1.7158e-01 | 0.3212 | [0.2744, 0.3839] | 12/12 |
| lattice | full_read | warm | 1 | 2.0018e-01 | 1.3451e-01 | 0.6728 | [0.6082, 0.7209] | 12/12 |
| lattice | range | warm | 1 | 5.7044e-02 | 2.7186e-02 | 0.5069 | [0.3950, 0.6207] | 12/12 |
| lattice | selected | warm | 1 | 2.3576e-03 | 1.8544e-03 | 0.7778 | [0.7675, 0.8137] | 12/12 |

## set_cooked_access

warm-point against cold-bulk. The ratio is cold-bulk / warm-point.

| arm | op | cache | threads | warm-point s | cold-bulk s | paired | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| md | full_read | cold | unset | 8.4659e-01 | 3.3988e-01 | 0.4161 | [0.3737, 0.4352] | 12/12 |
| md | full_read | warm | unset | 3.7507e-01 | 1.8850e-01 | 0.4660 | [0.4046, 0.7075] | 12/12 |
| md | range | warm | unset | 6.5089e-02 | 3.2617e-02 | 0.5444 | [0.4327, 0.6504] | 12/12 |
| md | selected | warm | unset | 1.1983e-03 | 1.6320e-03 | 1.3520 | [1.3401, 1.3928] | 0/12 |
| md | full_read | cold | 1 | 8.1895e-01 | 6.6073e-01 | 0.7645 | [0.7119, 0.8831] | 10/12 |
| md | full_read | warm | 1 | 3.3407e-01 | 4.3012e-01 | 1.2615 | [1.0857, 1.8014] | 1/12 |
| md | range | warm | 1 | 5.9596e-02 | 9.2215e-02 | 1.3766 | [1.3207, 1.8987] | 0/12 |
| md | selected | warm | 1 | 1.1928e-03 | 1.6090e-03 | 1.3438 | [1.3304, 1.3780] | 0/12 |
| thermal | full_read | cold | unset | 7.6564e-01 | 4.5166e-01 | 0.5524 | [0.4795, 0.6128] | 12/12 |
| thermal | full_read | warm | unset | 2.7008e-01 | 1.6990e-01 | 0.5994 | [0.4899, 0.6708] | 12/12 |
| thermal | range | warm | unset | 6.0412e-02 | 3.3524e-02 | 0.5922 | [0.4784, 0.6659] | 10/12 |
| thermal | selected | warm | unset | 1.1646e-03 | 1.3874e-03 | 1.1896 | [1.1780, 1.2000] | 0/12 |
| thermal | full_read | cold | 1 | 8.2133e-01 | 6.7931e-01 | 0.8567 | [0.7992, 0.9408] | 12/12 |
| thermal | full_read | warm | 1 | 3.1989e-01 | 3.2176e-01 | 1.0185 | [0.8647, 1.4931] | 6/12 |
| thermal | range | warm | 1 | 5.9902e-02 | 7.1174e-02 | 1.2413 | [1.0330, 1.4520] | 1/12 |
| thermal | selected | warm | 1 | 1.1662e-03 | 1.3898e-03 | 1.1789 | [1.1719, 1.2124] | 0/12 |
| lattice | full_read | cold | unset | 5.0562e-01 | 8.8630e-02 | 0.1918 | [0.1442, 0.2087] | 12/12 |
| lattice | full_read | warm | unset | 2.0688e-01 | 6.1609e-02 | 0.2959 | [0.2466, 0.3686] | 12/12 |
| lattice | range | warm | unset | 6.2032e-02 | 1.7165e-02 | 0.2857 | [0.2179, 0.3640] | 12/12 |
| lattice | selected | warm | unset | 2.5191e-03 | 2.0202e-03 | 0.8194 | [0.7669, 0.8325] | 12/12 |
| lattice | full_read | cold | 1 | 4.9203e-01 | 1.7809e-01 | 0.3636 | [0.2961, 0.4648] | 12/12 |
| lattice | full_read | warm | 1 | 2.0494e-01 | 1.4350e-01 | 0.6667 | [0.6498, 0.7640] | 12/12 |
| lattice | range | warm | 1 | 5.7512e-02 | 3.4397e-02 | 0.6079 | [0.4389, 0.7168] | 11/12 |
| lattice | selected | warm | 1 | 2.4418e-03 | 2.0066e-03 | 0.8184 | [0.7925, 0.8517] | 12/12 |
