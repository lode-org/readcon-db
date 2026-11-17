# db29 realistic arms

Commit `043c3091c9606293a920311a73a8a622d4638746`, tree `a201b635a8bfaa444623c2530e6c10592a423c3f`. Host label `host`, Intel(R) Xeon(R) Processor, 8 CPUs. n=12, serial. Warm reads are full, range, and selected. Cold bulk is the full read. Threads are the process default and `RAYON_NUM_THREADS=1`. The paired CI is a 10000-draw percentile bootstrap, type-7 median, numpy Generator seed 0. The ratio is the right-hand layout over the left. Faster counts the right-hand layout.

raw and shuffle-zstd are `READCON_DB_COOKED_CODEC`. warm-point and cold-bulk are `set_cooked_access` with that variable unset. The lattice arm is the best case. Quote md and thermal.

## Bytes and peak RSS

| arm | layout | codec stored | atoms/frame | corpus bytes | frames_soa bytes | hit shuffle | peak RSS |
| --- | --- | --- | --- | --- | --- | --- | --- |
| md | raw | raw | 60702 | 614641664 | 409898306 | False | 2937327616 |
| md | shuffle | shuffle-zstd | 60702 | 416727040 | 212073873 | True | 2580508672 |
| md | warm-point | raw | 60702 | 614641664 | 409898306 | False | 2937225216 |
| md | cold-bulk | shuffle-zstd | 60702 | 416727040 | 212073873 | True | 2584698880 |
| thermal | raw | raw | 60702 | 779874304 | 409898306 | False | 1785905152 |
| thermal | shuffle | shuffle-zstd | 60702 | 722833408 | 353274932 | True | 1699500032 |
| thermal | warm-point | raw | 60702 | 779874304 | 409898306 | False | 1786298368 |
| thermal | cold-bulk | shuffle-zstd | 60702 | 722833408 | 353274932 | True | 1701003264 |
| lattice | raw | raw | 100000 | 341966848 | 307208960 | False | 1350123520 |
| lattice | shuffle | shuffle-zstd | 100000 | 36569088 | 1583727 | True | 919478272 |
| lattice | warm-point | raw | 100000 | 341966848 | 307208960 | False | 1349988352 |
| lattice | cold-bulk | shuffle-zstd | 100000 | 36569088 | 1583727 | True | 861237248 |

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
| md | full_read | cold | unset | 8.4149e-01 | 3.5787e-01 | 0.4262 | [0.3642, 0.4870] | 12/12 |
| md | full_read | warm | unset | 3.5666e-01 | 2.0615e-01 | 0.5488 | [0.4426, 0.7020] | 12/12 |
| md | range | warm | unset | 6.7010e-02 | 3.3720e-02 | 0.5244 | [0.4303, 0.6192] | 12/12 |
| md | selected | warm | unset | 1.2439e-03 | 1.7326e-03 | 1.3684 | [1.3252, 1.4718] | 0/12 |
| md | full_read | cold | 1 | 8.6482e-01 | 5.8230e-01 | 0.6761 | [0.5791, 0.8594] | 11/12 |
| md | full_read | warm | 1 | 3.5405e-01 | 4.3880e-01 | 1.3279 | [1.0748, 1.5875] | 2/12 |
| md | range | warm | 1 | 6.1268e-02 | 8.9953e-02 | 1.4420 | [1.3321, 1.7611] | 1/12 |
| md | selected | warm | 1 | 1.1679e-03 | 1.6073e-03 | 1.3744 | [1.3544, 1.3807] | 0/12 |
| thermal | full_read | cold | unset | 9.0178e-01 | 4.8492e-01 | 0.5666 | [0.4590, 0.6022] | 12/12 |
| thermal | full_read | warm | unset | 3.8580e-01 | 2.0514e-01 | 0.5350 | [0.4639, 0.7906] | 11/12 |
| thermal | range | warm | unset | 6.7312e-02 | 3.7158e-02 | 0.5951 | [0.5208, 0.6869] | 12/12 |
| thermal | selected | warm | unset | 1.2550e-03 | 1.4520e-03 | 1.1583 | [1.1258, 1.1878] | 1/12 |
| thermal | full_read | cold | 1 | 8.6302e-01 | 7.3131e-01 | 0.9132 | [0.7754, 1.1212] | 8/12 |
| thermal | full_read | warm | 1 | 3.6377e-01 | 3.4606e-01 | 1.0511 | [0.8899, 1.3815] | 6/12 |
| thermal | range | warm | 1 | 6.0146e-02 | 7.4057e-02 | 1.2111 | [1.0438, 1.4725] | 2/12 |
| thermal | selected | warm | 1 | 1.2398e-03 | 1.4104e-03 | 1.1447 | [1.0862, 1.2062] | 1/12 |
| lattice | full_read | cold | unset | 4.7913e-01 | 8.3446e-02 | 0.1850 | [0.1540, 0.2168] | 12/12 |
| lattice | full_read | warm | unset | 1.9675e-01 | 6.4505e-02 | 0.3176 | [0.2663, 0.3810] | 12/12 |
| lattice | range | warm | unset | 6.0746e-02 | 1.8426e-02 | 0.2688 | [0.2424, 0.3711] | 12/12 |
| lattice | selected | warm | unset | 2.3959e-03 | 1.9777e-03 | 0.8291 | [0.8010, 0.8454] | 12/12 |
| lattice | full_read | cold | 1 | 4.8006e-01 | 1.8530e-01 | 0.4257 | [0.3234, 0.4632] | 12/12 |
| lattice | full_read | warm | 1 | 2.0599e-01 | 1.4470e-01 | 0.7086 | [0.6482, 0.7971] | 10/12 |
| lattice | range | warm | 1 | 5.9857e-02 | 3.4678e-02 | 0.5249 | [0.4480, 0.6594] | 12/12 |
| lattice | selected | warm | 1 | 2.4650e-03 | 2.0434e-03 | 0.8295 | [0.8007, 0.8506] | 12/12 |

## set_cooked_access

warm-point against cold-bulk. The ratio is cold-bulk / warm-point.

| arm | op | cache | threads | warm-point s | cold-bulk s | paired | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| md | full_read | cold | unset | 7.8582e-01 | 3.5930e-01 | 0.4249 | [0.3914, 0.5167] | 12/12 |
| md | full_read | warm | unset | 3.5326e-01 | 1.9740e-01 | 0.5529 | [0.4834, 0.6990] | 12/12 |
| md | range | warm | unset | 6.4868e-02 | 3.4038e-02 | 0.5398 | [0.4481, 0.5900] | 12/12 |
| md | selected | warm | unset | 1.1937e-03 | 1.6757e-03 | 1.4068 | [1.3131, 1.4959] | 0/12 |
| md | full_read | cold | 1 | 7.1785e-01 | 5.7643e-01 | 0.7848 | [0.6980, 0.8295] | 11/12 |
| md | full_read | warm | 1 | 3.7216e-01 | 4.0968e-01 | 1.0841 | [0.9471, 1.5704] | 4/12 |
| md | range | warm | 1 | 5.8398e-02 | 8.2884e-02 | 1.4220 | [1.1629, 1.7201] | 1/12 |
| md | selected | warm | 1 | 1.1973e-03 | 1.6309e-03 | 1.3553 | [1.3463, 1.3836] | 0/12 |
| thermal | full_read | cold | unset | 7.1278e-01 | 4.1271e-01 | 0.5584 | [0.4714, 0.8564] | 11/12 |
| thermal | full_read | warm | unset | 3.6004e-01 | 2.0644e-01 | 0.6238 | [0.4919, 0.7838] | 12/12 |
| thermal | range | warm | unset | 6.2019e-02 | 3.5162e-02 | 0.5220 | [0.4739, 0.6441] | 12/12 |
| thermal | selected | warm | unset | 1.1724e-03 | 1.3893e-03 | 1.1878 | [1.1766, 1.2291] | 0/12 |
| thermal | full_read | cold | 1 | 7.4607e-01 | 6.2169e-01 | 0.8997 | [0.6930, 0.9810] | 11/12 |
| thermal | full_read | warm | 1 | 3.3827e-01 | 3.3675e-01 | 0.9767 | [0.9305, 1.6773] | 7/12 |
| thermal | range | warm | 1 | 5.8497e-02 | 6.9879e-02 | 1.2613 | [1.0512, 1.4481] | 1/12 |
| thermal | selected | warm | 1 | 1.1757e-03 | 1.3910e-03 | 1.1765 | [1.1460, 1.1874] | 0/12 |
| lattice | full_read | cold | unset | 4.6882e-01 | 8.6902e-02 | 0.1668 | [0.1560, 0.2256] | 12/12 |
| lattice | full_read | warm | unset | 2.2073e-01 | 6.4036e-02 | 0.2906 | [0.2474, 0.3428] | 12/12 |
| lattice | range | warm | unset | 6.3807e-02 | 1.8042e-02 | 0.2800 | [0.2377, 0.3360] | 12/12 |
| lattice | selected | warm | unset | 2.5958e-03 | 2.0330e-03 | 0.7883 | [0.7734, 0.8182] | 12/12 |
| lattice | full_read | cold | 1 | 4.8322e-01 | 1.9382e-01 | 0.4022 | [0.3147, 0.5658] | 12/12 |
| lattice | full_read | warm | 1 | 2.0945e-01 | 1.4335e-01 | 0.6749 | [0.5534, 0.7102] | 11/12 |
| lattice | range | warm | 1 | 6.2284e-02 | 3.3041e-02 | 0.4796 | [0.4403, 0.6011] | 12/12 |
| lattice | selected | warm | 1 | 2.5318e-03 | 2.0256e-03 | 0.7967 | [0.7881, 0.8148] | 12/12 |
