# readcon-db vs ASE sqlite

Type-7 median and IQR. One discarded warmup and six serial samples
for every fair row. The published log has no discarded warmup.
A mismatch of any stored field aborts the run, so every sample below
passed an element-wise check (fidelity exact).
Cold medians use samples whose mincore resident fraction is 0.
Full read and the selected frame share each sample index.

## Published ingest (`paper/cpc/logs/db.log`)

40 frames, 8 Cu atoms. ASE commits every row. No warmup.

| tool | median s | IQR s | q1 s | q3 s | min s | max s |
| --- | --- | --- | --- | --- | --- | --- |
| readcon-db | 4.6724e-03 | 2.0083e-03 | 3.6932e-03 | 5.7016e-03 | 3.3584e-03 | 6.2406e-03 |
| ase.db | 7.2318e-02 | 3.7693e-02 | 6.0244e-02 | 9.7937e-02 | 5.4239e-02 | 1.0352e-01 |

## This machine, published harness

| tool | median s | IQR s | q1 s | q3 s | min s | max s |
| --- | --- | --- | --- | --- | --- | --- |
| readcon-db | 3.0322e-03 | 1.6155e-03 | 2.4871e-03 | 4.1025e-03 | 2.2256e-03 | 4.6883e-03 |
| ase.db | 7.5529e-02 | 9.3763e-03 | 7.0502e-02 | 7.9879e-02 | 5.6474e-02 | 9.2539e-02 |

## This machine, fair harness

Insert creates the database outside the clock and times the commit.
A full read and the selected-row query (`frame_index` of the middle
frame) copy positions, cell, masses, atomic numbers, constraints,
and velocities into NumPy arrays. ASE does the same via toatoms().
db5 timed only the sqlite fetch, which is why its 1M-atom warm
full read was 0.0166 s against db4's 0.226 s. Open, directory
fsync, fadvise, mincore, and the field check stay outside the
clock. Each row is its own process. A short read repeats to 50 ms
and a short commit to 2 s. A row that stayed above an IQR/median
of 0.15 was measured again at a longer span; that budget is
`span_s` on the row.

frames-100 selected cold readcon-db stayed at an IQR/median of 0.172
after a 4 s span. Every other fair row is at or under 0.15.

| workload | op | cache | tool | n | median s | IQR s | IQR/median | disk B | peak RSS B | fidelity |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| paper40x8 | insert | cold-file | readcon-db | 6 | 2.0338e-03 | 1.7326e-05 | 0.009 | 81920 | 103190528 | exact |
| paper40x8 | insert | cold-file | ase.db | 6 | 5.7614e-03 | 7.1999e-05 | 0.012 | 90112 | 96858112 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 6 | 1.3842e-03 | 6.7734e-05 | 0.049 | 143360 | 105926656 | exact |
| atoms-1000 | insert | cold-file | ase.db | 6 | 1.4096e-03 | 6.6099e-05 | 0.047 | 126976 | 98385920 | exact |
| atoms-1000 | full_read | cold | readcon-db | 6 | 6.4574e-05 | 2.4672e-06 | 0.038 | 143360 | 92717056 | exact |
| atoms-1000 | full_read | cold | ase.db | 6 | 1.3740e-03 | 5.2859e-05 | 0.038 | 126976 | 97206272 | exact |
| atoms-1000 | selected | cold | readcon-db | 6 | 6.2866e-05 | 7.6532e-07 | 0.012 | 143360 | 92753920 | exact |
| atoms-1000 | selected | cold | ase.db | 6 | 1.6045e-03 | 8.7875e-05 | 0.055 | 126976 | 95346688 | exact |
| atoms-1000 | full_read | warm | readcon-db | 6 | 1.9934e-05 | 1.1056e-07 | 0.006 | 143360 | 92405760 | exact |
| atoms-1000 | full_read | warm | ase.db | 6 | 4.3077e-04 | 7.6702e-06 | 0.018 | 126976 | 99078144 | exact |
| atoms-1000 | selected | warm | readcon-db | 6 | 1.9530e-05 | 1.0346e-07 | 0.005 | 143360 | 92528640 | exact |
| atoms-1000 | selected | warm | ase.db | 6 | 4.4857e-04 | 8.1075e-06 | 0.018 | 126976 | 99164160 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 6 | 3.3362e-03 | 1.4408e-04 | 0.043 | 634880 | 129523712 | exact |
| atoms-10000 | insert | cold-file | ase.db | 6 | 3.5742e-03 | 8.7002e-05 | 0.024 | 667648 | 109608960 | exact |
| atoms-10000 | full_read | cold | readcon-db | 6 | 1.1422e-03 | 8.5434e-05 | 0.075 | 634880 | 101224448 | exact |
| atoms-10000 | full_read | cold | ase.db | 6 | 4.8362e-03 | 2.2629e-04 | 0.047 | 667648 | 115478528 | exact |
| atoms-10000 | selected | cold | readcon-db | 6 | 1.1876e-03 | 5.8111e-05 | 0.049 | 634880 | 101175296 | exact |
| atoms-10000 | selected | cold | ase.db | 6 | 5.0232e-03 | 1.5382e-04 | 0.031 | 667648 | 114302976 | exact |
| atoms-10000 | full_read | warm | readcon-db | 6 | 3.0805e-04 | 4.6447e-07 | 0.002 | 634880 | 100560896 | exact |
| atoms-10000 | full_read | warm | ase.db | 6 | 2.6162e-03 | 2.7594e-05 | 0.011 | 667648 | 156868608 | exact |
| atoms-10000 | selected | warm | readcon-db | 6 | 3.0495e-04 | 6.3260e-07 | 0.002 | 634880 | 100519936 | exact |
| atoms-10000 | selected | warm | ase.db | 6 | 2.5667e-03 | 1.8541e-05 | 0.007 | 667648 | 158060544 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 6 | 1.7228e-02 | 9.7531e-04 | 0.057 | 5435392 | 326291456 | exact |
| atoms-100000 | insert | cold-file | ase.db | 6 | 2.4372e-02 | 5.9397e-04 | 0.024 | 6074368 | 251129856 | exact |
| atoms-100000 | full_read | cold | readcon-db | 6 | 8.6382e-03 | 5.6674e-04 | 0.066 | 5435392 | 142061568 | exact |
| atoms-100000 | full_read | cold | ase.db | 6 | 3.2512e-02 | 6.1998e-04 | 0.019 | 6074368 | 203239424 | exact |
| atoms-100000 | selected | cold | readcon-db | 6 | 7.6406e-03 | 4.6336e-04 | 0.061 | 5435392 | 142311424 | exact |
| atoms-100000 | selected | cold | ase.db | 6 | 3.4517e-02 | 2.7486e-03 | 0.080 | 6074368 | 202469376 | exact |
| atoms-100000 | full_read | warm | readcon-db | 6 | 3.5602e-03 | 7.1530e-06 | 0.002 | 5435392 | 133636096 | exact |
| atoms-100000 | full_read | warm | ase.db | 6 | 2.9670e-02 | 3.0642e-04 | 0.010 | 6074368 | 212054016 | exact |
| atoms-100000 | selected | warm | readcon-db | 6 | 3.4670e-03 | 6.4827e-06 | 0.002 | 5435392 | 133746688 | exact |
| atoms-100000 | selected | warm | ase.db | 6 | 3.0247e-02 | 4.2165e-04 | 0.014 | 6074368 | 213262336 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 6 | 1.5270e-01 | 5.7064e-03 | 0.037 | 52637696 | 1927426048 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 6 | 2.3275e-01 | 2.0639e-03 | 0.009 | 60125184 | 1639514112 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 6 | 8.8179e-02 | 9.6804e-03 | 0.110 | 52637696 | 516751360 | exact |
| atoms-1000000 | full_read | cold | ase.db | 6 | 3.7462e-01 | 4.5135e-02 | 0.120 | 60125184 | 932028416 | exact |
| atoms-1000000 | selected | cold | readcon-db | 6 | 8.0329e-02 | 2.8644e-03 | 0.036 | 52637696 | 516681728 | exact |
| atoms-1000000 | selected | cold | ase.db | 6 | 3.2130e-01 | 7.4629e-03 | 0.023 | 60125184 | 930820096 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 6 | 4.2623e-02 | 2.4000e-03 | 0.056 | 52637696 | 429527040 | exact |
| atoms-1000000 | full_read | warm | ase.db | 6 | 2.8449e-01 | 3.9339e-03 | 0.014 | 60125184 | 904925184 | exact |
| atoms-1000000 | selected | warm | readcon-db | 6 | 4.3505e-02 | 1.6246e-03 | 0.037 | 52637696 | 429768704 | exact |
| atoms-1000000 | selected | warm | ase.db | 6 | 2.7724e-01 | 9.3404e-03 | 0.034 | 60125184 | 908910592 | exact |
| frames-1 | insert | cold-file | readcon-db | 6 | 1.0957e-03 | 4.8967e-05 | 0.045 | 86016 | 98189312 | exact |
| frames-1 | insert | cold-file | ase.db | 6 | 1.1010e-03 | 1.9411e-05 | 0.018 | 69632 | 98930688 | exact |
| frames-1 | full_read | cold | readcon-db | 6 | 3.8760e-05 | 1.0439e-06 | 0.027 | 86016 | 92721152 | exact |
| frames-1 | full_read | cold | ase.db | 6 | 6.3291e-04 | 3.1695e-05 | 0.050 | 69632 | 93126656 | exact |
| frames-1 | selected | cold | readcon-db | 6 | 3.7588e-05 | 9.9520e-07 | 0.026 | 86016 | 92696576 | exact |
| frames-1 | selected | cold | ase.db | 6 | 9.2631e-04 | 3.4037e-05 | 0.037 | 69632 | 93102080 | exact |
| frames-1 | full_read | warm | readcon-db | 6 | 1.3804e-05 | 8.1863e-08 | 0.006 | 86016 | 92807168 | exact |
| frames-1 | full_read | warm | ase.db | 6 | 1.7916e-04 | 5.2518e-07 | 0.003 | 69632 | 93097984 | exact |
| frames-1 | selected | warm | readcon-db | 6 | 1.3582e-05 | 1.5745e-07 | 0.012 | 86016 | 92823552 | exact |
| frames-1 | selected | warm | ase.db | 6 | 1.9699e-04 | 9.5464e-07 | 0.005 | 69632 | 93384704 | exact |
| frames-100 | insert | cold-file | readcon-db | 6 | 4.3276e-03 | 4.8892e-05 | 0.011 | 180224 | 108191744 | exact |
| frames-100 | insert | cold-file | ase.db | 6 | 2.9834e-02 | 1.7458e-04 | 0.006 | 151552 | 96497664 | exact |
| frames-100 | full_read | cold | readcon-db | 6 | 3.2112e-04 | 4.0205e-05 | 0.125 | 180224 | 95318016 | exact |
| frames-100 | full_read | cold | ase.db | 6 | 8.8932e-03 | 2.9417e-04 | 0.033 | 151552 | 93954048 | exact |
| frames-100 | selected | cold | readcon-db | 6 | 1.8157e-04 | 3.1144e-05 | 0.172 | 180224 | 95498240 | exact |
| frames-100 | selected | cold | ase.db | 6 | 9.1887e-04 | 2.6103e-05 | 0.028 | 151552 | 93163520 | exact |
| frames-100 | full_read | warm | readcon-db | 6 | 6.0692e-05 | 2.4487e-07 | 0.004 | 180224 | 95166464 | exact |
| frames-100 | full_read | warm | ase.db | 6 | 7.9390e-03 | 2.1091e-05 | 0.003 | 151552 | 94011392 | exact |
| frames-100 | selected | warm | readcon-db | 6 | 1.3593e-05 | 3.1560e-08 | 0.002 | 180224 | 93040640 | exact |
| frames-100 | selected | warm | ase.db | 6 | 2.0348e-04 | 3.8176e-06 | 0.019 | 151552 | 93331456 | exact |
| frames-10000 | insert | cold-file | readcon-db | 6 | 2.5542e-01 | 3.0764e-03 | 0.012 | 12517376 | 395333632 | exact |
| frames-10000 | insert | cold-file | ase.db | 6 | 2.7876e+00 | 4.0820e-02 | 0.015 | 10817536 | 239280128 | exact |
| frames-10000 | full_read | cold | readcon-db | 6 | 1.9634e-02 | 2.1918e-03 | 0.112 | 12517376 | 158130176 | exact |
| frames-10000 | full_read | cold | ase.db | 6 | 7.8375e-01 | 3.1751e-03 | 0.004 | 10792960 | 190447616 | exact |
| frames-10000 | selected | cold | readcon-db | 6 | 7.5869e-04 | 5.0636e-05 | 0.067 | 12517376 | 92786688 | exact |
| frames-10000 | selected | cold | ase.db | 6 | 5.2721e-03 | 4.8105e-04 | 0.091 | 10792960 | 93302784 | exact |
| frames-10000 | full_read | warm | readcon-db | 6 | 8.0602e-03 | 1.1762e-05 | 0.001 | 12517376 | 147017728 | exact |
| frames-10000 | full_read | warm | ase.db | 6 | 7.8469e-01 | 4.1651e-03 | 0.005 | 10792960 | 180051968 | exact |
| frames-10000 | selected | warm | readcon-db | 6 | 1.3620e-05 | 1.3357e-07 | 0.010 | 12517376 | 92860416 | exact |
| frames-10000 | selected | warm | ase.db | 6 | 7.7103e-04 | 2.0886e-06 | 0.003 | 10792960 | 93536256 | exact |

