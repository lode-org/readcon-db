# readcon-db vs ASE sqlite

Type-7 median and IQR. One discarded warmup and twelve serial samples
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
| readcon-db | 2.9553e-03 | 1.2673e-03 | 2.1231e-03 | 3.3904e-03 | 1.7705e-03 | 1.2668e-02 |
| ase.db | 4.4172e-02 | 7.1141e-03 | 4.1140e-02 | 4.8254e-02 | 3.6309e-02 | 6.1843e-02 |

## This machine, fair harness

Insert creates the database outside the clock and times the commit.
A full read and the selected-row query (`frame_index` of the middle
frame) copy positions, cell, masses, atomic numbers, constraints,
and velocities into NumPy arrays. ASE does the same via toatoms().
db5 timed only the sqlite fetch, which is why its 1M-atom warm
full read was 0.0166 s against db4's 0.226 s. Open, directory
fsync, fadvise, mincore, and the field check stay outside the
clock. cold-open times the open and the query together, for
both tools, so neither keeps readahead from an open outside
the clock. The cold row still opens outside the clock. Each
row is its own process. A short read repeats to 50 ms and a
short commit to 2 s. Both inserts are one transaction and one
commit fsync. A long readcon-db trajectory puts one 64-frame
group at a time and drops that scratch after the put; the
transaction still commits once. Six samples are kept. A row
whose IQR/median is still above 0.05 takes six more. p90 is
the type-7 90th percentile of those means. A range read
materializes an inclusive frame-index span. A metadata read
returns the frame indexes whose energy is in the last tenth.
Ingest rows report frames/s and MB/s of the CON text. disk B
is the database file after that ingest.

| workload | op | cache | tool | RAYON_NUM_THREADS | threads | n | median s | p90 s | IQR s | IQR/median | frames/s | MB/s | disk B | peak RSS B | fidelity |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| paper40x8 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.1808e-03 | 2.2171e-03 | 3.0354e-05 | 0.014 | 1.8342e+04 | 5.7226e+00 | 49152 | 119738368 | exact |
| paper40x8 | insert | cold-file | ase.db | 1 | 1 | 12 | 5.2827e-03 | 5.5465e-03 | 1.6842e-04 | 0.032 | 7.5719e+03 | 2.3624e+00 | 90112 | 104771584 | exact |
| paper40x8 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.1686e-03 | 2.2041e-03 | 6.7637e-05 | 0.031 | 1.8445e+04 | 5.7549e+00 | 49152 | 119754752 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 1.0519e-03 | 1.1926e-03 | 5.2166e-05 | 0.050 | 9.5065e+02 | 4.0452e+01 | 98304 | 102408192 | exact |
| atoms-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.4045e-03 | 1.4704e-03 | 7.1079e-05 | 0.051 | 7.1197e+02 | 3.0296e+01 | 126976 | 106029056 | exact |
| atoms-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 1.1248e-03 | 1.2438e-03 | 1.3290e-04 | 0.118 | 8.8907e+02 | 3.7832e+01 | 98304 | 102612992 | exact |
| atoms-1000 | full_read | cold | readcon-db | 1 | 1 | 12 | 3.2520e-05 | 3.3496e-05 | 8.6167e-07 | 0.026 |  |  | 98304 | 100651008 | exact |
| atoms-1000 | full_read | cold | ase.db | 1 | 1 | 12 | 1.2277e-03 | 1.4289e-03 | 1.1918e-04 | 0.097 |  |  | 126976 | 135389184 | exact |
| atoms-1000 | full_read | cold | readcon-db | unset | 8 | 12 | 3.2505e-05 | 3.4569e-05 | 2.3936e-06 | 0.074 |  |  | 98304 | 100511744 | exact |
| atoms-1000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.9449e-05 | 3.0498e-05 | 8.7659e-07 | 0.030 |  |  | 98304 | 100757504 | exact |
| atoms-1000 | selected | cold | ase.db | 1 | 1 | 12 | 1.7649e-03 | 2.0009e-03 | 2.5853e-04 | 0.146 |  |  | 126976 | 124715008 | exact |
| atoms-1000 | selected | cold | readcon-db | unset | 8 | 12 | 2.9641e-05 | 3.0763e-05 | 1.0212e-06 | 0.034 |  |  | 98304 | 100503552 | exact |
| atoms-1000 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.7581e-05 | 1.7940e-05 | 4.3775e-07 | 0.025 |  |  | 98304 | 100458496 | exact |
| atoms-1000 | full_read | warm | ase.db | 1 | 1 | 12 | 5.3151e-04 | 5.5900e-04 | 2.4617e-05 | 0.046 |  |  | 126976 | 182968320 | exact |
| atoms-1000 | full_read | warm | readcon-db | unset | 8 | 12 | 1.7008e-05 | 1.7711e-05 | 3.8527e-07 | 0.023 |  |  | 98304 | 100270080 | exact |
| atoms-1000 | selected | warm | readcon-db | 1 | 1 | 12 | 1.6689e-05 | 2.1232e-05 | 7.8793e-07 | 0.047 |  |  | 98304 | 100585472 | exact |
| atoms-1000 | selected | warm | ase.db | 1 | 1 | 12 | 5.2760e-04 | 5.4861e-04 | 2.5350e-05 | 0.048 |  |  | 126976 | 185135104 | exact |
| atoms-1000 | selected | warm | readcon-db | unset | 8 | 12 | 1.7033e-05 | 1.7837e-05 | 5.5644e-07 | 0.033 |  |  | 98304 | 100397056 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.6795e-03 | 3.8117e-03 | 2.0374e-04 | 0.055 | 2.7177e+02 | 1.2538e+02 | 589824 | 120754176 | exact |
| atoms-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.5453e-03 | 3.6085e-03 | 1.3921e-04 | 0.039 | 2.8206e+02 | 1.3012e+02 | 667648 | 111472640 | exact |
| atoms-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.6281e-03 | 3.7646e-03 | 1.6522e-04 | 0.046 | 2.7563e+02 | 1.2715e+02 | 589824 | 120598528 | exact |
| atoms-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.8994e-04 | 2.9657e-04 | 9.3362e-06 | 0.032 |  |  | 589824 | 104562688 | exact |
| atoms-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 5.3241e-03 | 5.4028e-03 | 1.2238e-04 | 0.023 |  |  | 667648 | 188866560 | exact |
| atoms-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 2.8908e-04 | 3.0174e-04 | 1.1617e-05 | 0.040 |  |  | 589824 | 104812544 | exact |
| atoms-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.8218e-04 | 2.8504e-04 | 2.6106e-06 | 0.009 |  |  | 589824 | 104669184 | exact |
| atoms-10000 | selected | cold | ase.db | 1 | 1 | 12 | 4.7139e-03 | 5.6694e-03 | 7.2898e-04 | 0.155 |  |  | 667648 | 198475776 | exact |
| atoms-10000 | selected | cold | readcon-db | unset | 8 | 12 | 2.8501e-04 | 2.9071e-04 | 5.5080e-06 | 0.019 |  |  | 589824 | 104620032 | exact |
| atoms-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.6085e-04 | 2.6167e-04 | 6.5568e-07 | 0.003 |  |  | 589824 | 104189952 | exact |
| atoms-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.4067e-03 | 3.8948e-03 | 6.3060e-04 | 0.185 |  |  | 667648 | 229429248 | exact |
| atoms-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.4437e-04 | 2.4696e-04 | 1.4539e-06 | 0.006 |  |  | 589824 | 104026112 | exact |
| atoms-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.6989e-04 | 2.7463e-04 | 6.4985e-06 | 0.024 |  |  | 589824 | 103907328 | exact |
| atoms-10000 | selected | warm | ase.db | 1 | 1 | 12 | 3.2545e-03 | 3.3030e-03 | 2.3308e-05 | 0.007 |  |  | 667648 | 240414720 | exact |
| atoms-10000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5680e-04 | 2.5944e-04 | 6.4170e-06 | 0.025 |  |  | 589824 | 103997440 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.6101e-02 | 2.7094e-02 | 7.6141e-04 | 0.029 | 3.8313e+01 | 1.9191e+02 | 5390336 | 196964352 | exact |
| atoms-100000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.5454e-02 | 2.5884e-02 | 8.6760e-04 | 0.034 | 3.9287e+01 | 1.9679e+02 | 6074368 | 203661312 | exact |
| atoms-100000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.6072e-02 | 2.6980e-02 | 1.0872e-03 | 0.042 | 3.8356e+01 | 1.9213e+02 | 5390336 | 197038080 | exact |
| atoms-100000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.1369e-03 | 7.0893e-03 | 8.1565e-04 | 0.133 |  |  | 5390336 | 135086080 | exact |
| atoms-100000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.7851e-02 | 3.9684e-02 | 2.2079e-03 | 0.058 |  |  | 6074368 | 310550528 | exact |
| atoms-100000 | full_read | cold | readcon-db | unset | 8 | 12 | 7.3380e-03 | 7.5974e-03 | 2.6368e-04 | 0.036 |  |  | 5390336 | 134963200 | exact |
| atoms-100000 | selected | cold | readcon-db | 1 | 1 | 12 | 5.4479e-03 | 6.8633e-03 | 1.3375e-03 | 0.246 |  |  | 5390336 | 134971392 | exact |
| atoms-100000 | selected | cold | ase.db | 1 | 1 | 12 | 3.7833e-02 | 3.8810e-02 | 1.9497e-03 | 0.052 |  |  | 6074368 | 297533440 | exact |
| atoms-100000 | selected | cold | readcon-db | unset | 8 | 12 | 6.3657e-03 | 6.8400e-03 | 6.6149e-04 | 0.104 |  |  | 5390336 | 134889472 | exact |
| atoms-100000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.6388e-03 | 2.7305e-03 | 4.0967e-05 | 0.016 |  |  | 5390336 | 128774144 | exact |
| atoms-100000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.2215e-02 | 3.4279e-02 | 3.2393e-03 | 0.101 |  |  | 6074368 | 307077120 | exact |
| atoms-100000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.6471e-03 | 2.7279e-03 | 3.4815e-05 | 0.013 |  |  | 5390336 | 128823296 | exact |
| atoms-100000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.4994e-03 | 2.5486e-03 | 4.7900e-05 | 0.019 |  |  | 5390336 | 128708608 | exact |
| atoms-100000 | selected | warm | ase.db | 1 | 1 | 12 | 2.9723e-02 | 3.0055e-02 | 5.6233e-04 | 0.019 |  |  | 6074368 | 307093504 | exact |
| atoms-100000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5137e-03 | 2.5770e-03 | 4.5899e-05 | 0.018 |  |  | 5390336 | 128745472 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.9016e-01 | 3.0010e-01 | 1.0258e-02 | 0.035 | 3.4464e+00 | 1.8640e+02 | 52592640 | 733392896 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.6335e-01 | 2.7431e-01 | 1.2921e-02 | 0.049 | 3.7972e+00 | 2.0538e+02 | 60125184 | 1110716416 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.1465e-01 | 3.3690e-01 | 1.1190e-02 | 0.036 | 3.1781e+00 | 1.7190e+02 | 52592640 | 733401088 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.3300e-02 | 7.9542e-02 | 1.9088e-02 | 0.302 |  |  | 52592640 | 427618304 | exact |
| atoms-1000000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.7534e-01 | 4.5703e-01 | 1.5392e-02 | 0.041 |  |  | 60125184 | 1345617920 | exact |
| atoms-1000000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.3975e-02 | 6.9727e-02 | 3.9594e-03 | 0.062 |  |  | 52592640 | 427253760 | exact |
| atoms-1000000 | selected | cold | readcon-db | 1 | 1 | 12 | 6.7564e-02 | 9.3570e-02 | 9.6523e-03 | 0.143 |  |  | 52592640 | 427212800 | exact |
| atoms-1000000 | selected | cold | ase.db | 1 | 1 | 12 | 3.8461e-01 | 4.1993e-01 | 1.7200e-02 | 0.045 |  |  | 60125184 | 1320628224 | exact |
| atoms-1000000 | selected | cold | readcon-db | unset | 8 | 12 | 6.2192e-02 | 8.3551e-02 | 1.0337e-02 | 0.166 |  |  | 52592640 | 427352064 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 1 | 1 | 12 | 3.0862e-02 | 3.3361e-02 | 1.9836e-03 | 0.064 |  |  | 52592640 | 364523520 | exact |
| atoms-1000000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.0968e-01 | 3.2053e-01 | 9.3412e-03 | 0.030 |  |  | 60125184 | 1330544640 | exact |
| atoms-1000000 | full_read | warm | readcon-db | unset | 8 | 12 | 3.1747e-02 | 3.6594e-02 | 3.9145e-03 | 0.123 |  |  | 52592640 | 364748800 | exact |
| atoms-1000000 | selected | warm | readcon-db | 1 | 1 | 12 | 3.5209e-02 | 4.2020e-02 | 4.5406e-03 | 0.129 |  |  | 52592640 | 364433408 | exact |
| atoms-1000000 | selected | warm | ase.db | 1 | 1 | 12 | 3.0755e-01 | 3.3585e-01 | 9.7969e-03 | 0.032 |  |  | 60125184 | 1330491392 | exact |
| atoms-1000000 | selected | warm | readcon-db | unset | 8 | 12 | 2.9263e-02 | 3.2444e-02 | 1.2396e-03 | 0.042 |  |  | 52592640 | 364371968 | exact |
| frames-1 | insert | cold-file | readcon-db | 1 | 1 | 12 | 6.0672e-04 | 7.1802e-04 | 6.9399e-05 | 0.114 | 1.6482e+03 | 1.1949e+00 | 40960 | 99954688 | exact |
| frames-1 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.0380e-03 | 1.0474e-03 | 3.0759e-05 | 0.030 | 9.6341e+02 | 6.9847e-01 | 69632 | 105291776 | exact |
| frames-1 | insert | cold-file | readcon-db | unset | 8 | 12 | 7.5456e-04 | 8.5395e-04 | 1.2558e-04 | 0.166 | 1.3253e+03 | 9.6083e-01 | 40960 | 99807232 | exact |
| frames-1 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.2843e-05 | 2.5101e-05 | 7.9906e-07 | 0.035 |  |  | 40960 | 98693120 | exact |
| frames-1 | full_read | cold | ase.db | 1 | 1 | 12 | 5.4246e-04 | 5.8150e-04 | 4.7617e-05 | 0.088 |  |  | 69632 | 103501824 | exact |
| frames-1 | full_read | cold | readcon-db | unset | 8 | 12 | 2.2540e-05 | 2.3022e-05 | 8.2126e-07 | 0.036 |  |  | 40960 | 98430976 | exact |
| frames-1 | selected | cold | readcon-db | 1 | 1 | 12 | 2.1244e-05 | 2.2430e-05 | 1.0457e-06 | 0.049 |  |  | 40960 | 98369536 | exact |
| frames-1 | selected | cold | ase.db | 1 | 1 | 12 | 7.7404e-04 | 8.6444e-04 | 8.1449e-05 | 0.105 |  |  | 69632 | 102162432 | exact |
| frames-1 | selected | cold | readcon-db | unset | 8 | 12 | 2.1893e-05 | 2.2766e-05 | 1.5475e-06 | 0.071 |  |  | 40960 | 98177024 | exact |
| frames-1 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.2012e-05 | 1.2298e-05 | 1.3129e-07 | 0.011 |  |  | 40960 | 98402304 | exact |
| frames-1 | full_read | warm | ase.db | 1 | 1 | 12 | 1.5389e-04 | 1.5991e-04 | 3.5135e-06 | 0.023 |  |  | 69632 | 109051904 | exact |
| frames-1 | full_read | warm | readcon-db | unset | 8 | 12 | 1.1781e-05 | 1.1824e-05 | 5.8761e-08 | 0.005 |  |  | 40960 | 98316288 | exact |
| frames-1 | selected | warm | readcon-db | 1 | 1 | 12 | 1.1605e-05 | 1.1678e-05 | 5.0748e-08 | 0.004 |  |  | 40960 | 98164736 | exact |
| frames-1 | selected | warm | ase.db | 1 | 1 | 12 | 1.7713e-04 | 1.7875e-04 | 1.4200e-06 | 0.008 |  |  | 69632 | 107933696 | exact |
| frames-1 | selected | warm | readcon-db | unset | 8 | 12 | 1.1547e-05 | 1.1836e-05 | 2.1876e-07 | 0.019 |  |  | 40960 | 98111488 | exact |
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.5011e-03 | 4.5457e-03 | 4.3450e-05 | 0.010 | 2.2217e+04 | 1.6357e+01 | 65536 | 114212864 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.2813e-02 | 3.3960e-02 | 8.5012e-04 | 0.026 | 3.0476e+03 | 2.2438e+00 | 151552 | 100589568 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.4951e-03 | 4.5651e-03 | 6.2378e-05 | 0.014 | 2.2246e+04 | 1.6379e+01 | 65536 | 113905664 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.5756e-05 | 6.6642e-05 | 2.0530e-06 | 0.031 |  |  | 65536 | 105275392 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0165e-02 | 1.0408e-02 | 2.6984e-04 | 0.027 |  |  | 151552 | 117121024 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.7290e-05 | 7.1389e-05 | 2.9416e-06 | 0.044 |  |  | 65536 | 105074688 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.2664e-05 | 2.3685e-05 | 9.6576e-07 | 0.043 |  |  | 65536 | 102711296 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 8.3254e-04 | 1.0479e-03 | 1.3775e-04 | 0.165 |  |  | 151552 | 101662720 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.1865e-05 | 2.3236e-05 | 1.5973e-06 | 0.073 |  |  | 65536 | 102830080 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.7828e-05 | 7.0986e-05 | 1.2853e-06 | 0.019 |  |  | 65536 | 105275392 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 9.9729e-03 | 1.3957e-02 | 2.2793e-03 | 0.229 |  |  | 151552 | 117329920 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 6.6570e-05 | 6.8315e-05 | 2.1917e-06 | 0.033 |  |  | 65536 | 105115648 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.0851e-05 | 2.1247e-05 | 4.0071e-07 | 0.019 |  |  | 65536 | 102797312 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 1.0346e-03 | 1.1930e-03 | 1.5644e-04 | 0.151 |  |  | 151552 | 101457920 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.1721e-05 | 2.2710e-05 | 9.7625e-07 | 0.045 |  |  | 65536 | 103026688 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 3.8627e-04 | 4.6767e-04 | 1.0459e-04 | 0.271 |  |  | 65536 | 105226240 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.0401e-02 | 1.0805e-02 | 3.1479e-04 | 0.030 |  |  | 151552 | 116654080 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 3.2036e-04 | 4.3580e-04 | 3.1890e-05 | 0.100 |  |  | 65536 | 105197568 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 2.4885e-04 | 2.7757e-04 | 4.3579e-05 | 0.175 |  |  | 65536 | 102752256 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 1.2717e-03 | 1.4084e-03 | 2.1742e-04 | 0.171 |  |  | 151552 | 100859904 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.5064e-04 | 3.0473e-04 | 5.0507e-05 | 0.202 |  |  | 65536 | 102744064 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.3339e-05 | 4.4182e-05 | 4.0250e-07 | 0.009 |  |  | 65536 | 104927232 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 9.1650e-03 | 9.3498e-03 | 3.5600e-04 | 0.039 |  |  | 151552 | 119816192 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.4013e-05 | 4.4163e-05 | 2.1424e-07 | 0.005 |  |  | 65536 | 105074688 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.4491e-06 | 9.5209e-06 | 1.1975e-07 | 0.013 |  |  | 65536 | 102723584 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.7376e-04 | 1.7461e-04 | 1.3166e-06 | 0.008 |  |  | 151552 | 107999232 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.3146e-06 | 9.3435e-06 | 5.7664e-08 | 0.006 |  |  | 65536 | 102887424 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.2660e-01 | 3.2896e-01 | 2.9165e-03 | 0.009 | 3.0618e+04 | 2.3498e+01 | 3362816 | 201732096 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.0424e+00 | 3.1023e+00 | 5.4468e-02 | 0.018 | 3.2868e+03 | 2.5225e+00 | 10809344 | 201125888 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2378e-01 | 3.2634e-01 | 2.0141e-03 | 0.006 | 3.0885e+04 | 2.3703e+01 | 3362816 | 185081856 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.4324e-03 | 7.2485e-03 | 3.9981e-04 | 0.062 |  |  | 3362816 | 160886784 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.8859e-01 | 1.0013e+00 | 1.7244e-02 | 0.017 |  |  | 10809344 | 533848064 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.2112e-03 | 6.5608e-03 | 1.7425e-04 | 0.028 |  |  | 3362816 | 160903168 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.4642e-05 | 4.5164e-05 | 1.1033e-06 | 0.025 |  |  | 3362816 | 117620736 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 3.4976e-03 | 3.9814e-03 | 8.0911e-04 | 0.231 |  |  | 10809344 | 99491840 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.4303e-05 | 4.6960e-05 | 1.8330e-06 | 0.041 |  |  | 3362816 | 117411840 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.4488e-03 | 8.5909e-03 | 4.1729e-04 | 0.065 |  |  | 3362816 | 179699712 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 1.0119e+00 | 1.0618e+00 | 3.5638e-02 | 0.035 |  |  | 10809344 | 552407040 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.4955e-03 | 7.3998e-03 | 4.6103e-04 | 0.071 |  |  | 3362816 | 179822592 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.4088e-06 | 9.5258e-06 | 7.1461e-08 | 0.008 |  |  | 3362816 | 113295360 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.4215e-04 | 1.2501e-03 | 4.3867e-05 | 0.059 |  |  | 10809344 | 100753408 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 9.9726e-06 | 1.0040e-05 | 9.1176e-08 | 0.009 |  |  | 3362816 | 113225728 | exact |

