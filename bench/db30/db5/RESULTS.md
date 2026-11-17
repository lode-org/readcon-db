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
| readcon-db | 3.3224e-03 | 6.8681e-04 | 2.9580e-03 | 3.6448e-03 | 2.2350e-03 | 8.8612e-03 |
| ase.db | 5.7118e-02 | 1.6901e-02 | 5.0953e-02 | 6.7854e-02 | 4.4442e-02 | 7.1204e-02 |

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
| paper40x8 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.2078e-03 | 2.2620e-03 | 6.3137e-05 | 0.029 | 1.8118e+04 | 5.6527e+00 | 49152 | 119955456 | exact |
| paper40x8 | insert | cold-file | ase.db | 1 | 1 | 12 | 5.4063e-03 | 5.8444e-03 | 2.5855e-04 | 0.048 | 7.3988e+03 | 2.3084e+00 | 90112 | 104767488 | exact |
| paper40x8 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.1565e-03 | 2.2592e-03 | 7.8709e-05 | 0.036 | 1.8548e+04 | 5.7871e+00 | 49152 | 119799808 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 1.2004e-03 | 1.3041e-03 | 1.0063e-04 | 0.084 | 8.3304e+02 | 3.5447e+01 | 98304 | 102649856 | exact |
| atoms-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.5743e-03 | 1.6801e-03 | 1.5181e-04 | 0.096 | 6.3519e+02 | 2.7029e+01 | 126976 | 104194048 | exact |
| atoms-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 1.1837e-03 | 1.2649e-03 | 5.5767e-05 | 0.047 | 8.4480e+02 | 3.5948e+01 | 98304 | 102297600 | exact |
| atoms-1000 | full_read | cold | readcon-db | 1 | 1 | 12 | 3.3831e-05 | 3.5254e-05 | 1.5184e-06 | 0.045 |  |  | 98304 | 100773888 | exact |
| atoms-1000 | full_read | cold | ase.db | 1 | 1 | 12 | 1.5901e-03 | 1.8629e-03 | 3.6204e-04 | 0.228 |  |  | 126976 | 128073728 | exact |
| atoms-1000 | full_read | cold | readcon-db | unset | 8 | 12 | 3.2239e-05 | 3.5987e-05 | 4.9437e-07 | 0.015 |  |  | 98304 | 100659200 | exact |
| atoms-1000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.9984e-05 | 3.0770e-05 | 6.6621e-07 | 0.022 |  |  | 98304 | 100474880 | exact |
| atoms-1000 | selected | cold | ase.db | 1 | 1 | 12 | 1.4061e-03 | 1.5827e-03 | 2.7853e-04 | 0.198 |  |  | 126976 | 132419584 | exact |
| atoms-1000 | selected | cold | readcon-db | unset | 8 | 12 | 3.0077e-05 | 3.1026e-05 | 1.3813e-06 | 0.046 |  |  | 98304 | 100474880 | exact |
| atoms-1000 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.6660e-05 | 1.6971e-05 | 1.6554e-07 | 0.010 |  |  | 98304 | 100245504 | exact |
| atoms-1000 | full_read | warm | ase.db | 1 | 1 | 12 | 5.0958e-04 | 6.1013e-04 | 1.4144e-04 | 0.278 |  |  | 126976 | 187154432 | exact |
| atoms-1000 | full_read | warm | readcon-db | unset | 8 | 12 | 1.6735e-05 | 1.7400e-05 | 3.0870e-07 | 0.018 |  |  | 98304 | 100507648 | exact |
| atoms-1000 | selected | warm | readcon-db | 1 | 1 | 12 | 1.6350e-05 | 1.6747e-05 | 7.9289e-08 | 0.005 |  |  | 98304 | 100397056 | exact |
| atoms-1000 | selected | warm | ase.db | 1 | 1 | 12 | 5.1486e-04 | 6.0126e-04 | 3.3689e-05 | 0.065 |  |  | 126976 | 182861824 | exact |
| atoms-1000 | selected | warm | readcon-db | unset | 8 | 12 | 1.6430e-05 | 1.6642e-05 | 1.6883e-07 | 0.010 |  |  | 98304 | 100352000 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.7782e-03 | 3.9323e-03 | 1.7941e-04 | 0.047 | 2.6467e+02 | 1.2210e+02 | 589824 | 120750080 | exact |
| atoms-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.8690e-03 | 4.0221e-03 | 2.3584e-04 | 0.061 | 2.5846e+02 | 1.1924e+02 | 667648 | 114114560 | exact |
| atoms-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.6083e-03 | 3.6649e-03 | 8.2081e-05 | 0.023 | 2.7714e+02 | 1.2785e+02 | 589824 | 120664064 | exact |
| atoms-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.9340e-04 | 2.9778e-04 | 3.5173e-06 | 0.012 |  |  | 589824 | 104759296 | exact |
| atoms-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 5.1537e-03 | 5.3561e-03 | 2.3272e-04 | 0.045 |  |  | 667648 | 190251008 | exact |
| atoms-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 2.9294e-04 | 3.0543e-04 | 1.2658e-05 | 0.043 |  |  | 589824 | 104656896 | exact |
| atoms-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.9003e-04 | 2.9625e-04 | 7.4213e-06 | 0.026 |  |  | 589824 | 104742912 | exact |
| atoms-10000 | selected | cold | ase.db | 1 | 1 | 12 | 4.8656e-03 | 5.3708e-03 | 5.3976e-04 | 0.111 |  |  | 667648 | 195624960 | exact |
| atoms-10000 | selected | cold | readcon-db | unset | 8 | 12 | 2.8815e-04 | 2.9850e-04 | 8.7824e-06 | 0.030 |  |  | 589824 | 104624128 | exact |
| atoms-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.7288e-04 | 2.7765e-04 | 2.6173e-06 | 0.010 |  |  | 589824 | 103997440 | exact |
| atoms-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.3496e-03 | 3.5796e-03 | 2.1863e-04 | 0.065 |  |  | 667648 | 232296448 | exact |
| atoms-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.5725e-04 | 2.6071e-04 | 8.4790e-07 | 0.003 |  |  | 589824 | 103997440 | exact |
| atoms-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.4381e-04 | 2.4631e-04 | 1.0372e-06 | 0.004 |  |  | 589824 | 103903232 | exact |
| atoms-10000 | selected | warm | ase.db | 1 | 1 | 12 | 3.3673e-03 | 3.6856e-03 | 2.9948e-04 | 0.089 |  |  | 667648 | 231956480 | exact |
| atoms-10000 | selected | warm | readcon-db | unset | 8 | 12 | 2.6588e-04 | 2.6778e-04 | 4.3325e-06 | 0.016 |  |  | 589824 | 103952384 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.0705e-02 | 3.1252e-02 | 1.1887e-03 | 0.039 | 3.2569e+01 | 1.6314e+02 | 5390336 | 197304320 | exact |
| atoms-100000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.7482e-02 | 2.8870e-02 | 1.5136e-03 | 0.055 | 3.6387e+01 | 1.8227e+02 | 6074368 | 206442496 | exact |
| atoms-100000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.7247e-02 | 2.8901e-02 | 8.4006e-04 | 0.031 | 3.6701e+01 | 1.8384e+02 | 5390336 | 197365760 | exact |
| atoms-100000 | full_read | cold | readcon-db | 1 | 1 | 12 | 7.1752e-03 | 8.1215e-03 | 1.3332e-03 | 0.186 |  |  | 5390336 | 135057408 | exact |
| atoms-100000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.6375e-02 | 3.7769e-02 | 2.1788e-03 | 0.060 |  |  | 6074368 | 298442752 | exact |
| atoms-100000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.5004e-03 | 6.5114e-03 | 1.1365e-03 | 0.207 |  |  | 5390336 | 135192576 | exact |
| atoms-100000 | selected | cold | readcon-db | 1 | 1 | 12 | 6.7258e-03 | 7.1734e-03 | 6.7169e-04 | 0.100 |  |  | 5390336 | 135036928 | exact |
| atoms-100000 | selected | cold | ase.db | 1 | 1 | 12 | 3.9013e-02 | 4.2351e-02 | 3.1651e-03 | 0.081 |  |  | 6074368 | 297902080 | exact |
| atoms-100000 | selected | cold | readcon-db | unset | 8 | 12 | 6.9738e-03 | 8.3295e-03 | 1.3668e-03 | 0.196 |  |  | 5390336 | 134938624 | exact |
| atoms-100000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.5938e-03 | 2.6804e-03 | 3.2312e-05 | 0.012 |  |  | 5390336 | 128745472 | exact |
| atoms-100000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.0865e-02 | 3.4180e-02 | 3.0749e-03 | 0.100 |  |  | 6074368 | 306163712 | exact |
| atoms-100000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.4548e-03 | 2.5273e-03 | 1.9187e-05 | 0.008 |  |  | 5390336 | 128856064 | exact |
| atoms-100000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.5251e-03 | 2.5859e-03 | 7.8284e-05 | 0.031 |  |  | 5390336 | 128823296 | exact |
| atoms-100000 | selected | warm | ase.db | 1 | 1 | 12 | 3.0139e-02 | 3.1339e-02 | 9.2482e-04 | 0.031 |  |  | 6074368 | 301527040 | exact |
| atoms-100000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5922e-03 | 2.6817e-03 | 3.8157e-05 | 0.015 |  |  | 5390336 | 128856064 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.0421e-01 | 3.2159e-01 | 1.8557e-02 | 0.061 | 3.2872e+00 | 1.7779e+02 | 52592640 | 733421568 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.5593e-01 | 2.6528e-01 | 1.8072e-02 | 0.071 | 3.9073e+00 | 2.1133e+02 | 60125184 | 1119895552 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.8538e-01 | 2.9227e-01 | 3.7536e-03 | 0.013 | 3.5041e+00 | 1.8952e+02 | 52592640 | 733487104 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 1 | 1 | 12 | 5.7462e-02 | 6.9969e-02 | 1.6435e-02 | 0.286 |  |  | 52592640 | 427577344 | exact |
| atoms-1000000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.4517e-01 | 4.1784e-01 | 3.6994e-02 | 0.107 |  |  | 60125184 | 1329647616 | exact |
| atoms-1000000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.4953e-02 | 7.5547e-02 | 1.3052e-02 | 0.238 |  |  | 52592640 | 427327488 | exact |
| atoms-1000000 | selected | cold | readcon-db | 1 | 1 | 12 | 5.2748e-02 | 6.4616e-02 | 5.4276e-03 | 0.103 |  |  | 52592640 | 427380736 | exact |
| atoms-1000000 | selected | cold | ase.db | 1 | 1 | 12 | 3.2559e-01 | 3.3805e-01 | 1.7219e-02 | 0.053 |  |  | 60125184 | 1320247296 | exact |
| atoms-1000000 | selected | cold | readcon-db | unset | 8 | 12 | 5.0525e-02 | 5.5210e-02 | 4.7054e-03 | 0.093 |  |  | 52592640 | 427503616 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.0516e-02 | 2.6133e-02 | 7.3224e-04 | 0.036 |  |  | 52592640 | 364470272 | exact |
| atoms-1000000 | full_read | warm | ase.db | 1 | 1 | 12 | 2.9201e-01 | 3.0436e-01 | 1.1059e-02 | 0.038 |  |  | 60125184 | 1330569216 | exact |
| atoms-1000000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.3943e-02 | 2.8642e-02 | 5.4429e-03 | 0.227 |  |  | 52592640 | 364552192 | exact |
| atoms-1000000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.1602e-02 | 2.2605e-02 | 8.5420e-04 | 0.040 |  |  | 52592640 | 364638208 | exact |
| atoms-1000000 | selected | warm | ase.db | 1 | 1 | 12 | 2.9718e-01 | 3.2421e-01 | 7.3498e-03 | 0.025 |  |  | 60125184 | 1330655232 | exact |
| atoms-1000000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5798e-02 | 3.0493e-02 | 2.5602e-03 | 0.099 |  |  | 52592640 | 364482560 | exact |
| frames-1 | insert | cold-file | readcon-db | 1 | 1 | 12 | 6.5357e-04 | 7.2184e-04 | 1.0765e-04 | 0.165 | 1.5301e+03 | 1.1093e+00 | 40960 | 99909632 | exact |
| frames-1 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.0230e-03 | 1.0468e-03 | 2.4764e-05 | 0.024 | 9.7748e+02 | 7.0867e-01 | 69632 | 105168896 | exact |
| frames-1 | insert | cold-file | readcon-db | unset | 8 | 12 | 7.1316e-04 | 8.1291e-04 | 1.5873e-04 | 0.223 | 1.4022e+03 | 1.0166e+00 | 40960 | 99545088 | exact |
| frames-1 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.2542e-05 | 2.3303e-05 | 5.4945e-07 | 0.024 |  |  | 40960 | 98234368 | exact |
| frames-1 | full_read | cold | ase.db | 1 | 1 | 12 | 5.3512e-04 | 5.7501e-04 | 1.9772e-05 | 0.037 |  |  | 69632 | 103710720 | exact |
| frames-1 | full_read | cold | readcon-db | unset | 8 | 12 | 2.2582e-05 | 2.5271e-05 | 1.5877e-06 | 0.070 |  |  | 40960 | 98402304 | exact |
| frames-1 | selected | cold | readcon-db | 1 | 1 | 12 | 2.0763e-05 | 2.1344e-05 | 4.7977e-07 | 0.023 |  |  | 40960 | 98365440 | exact |
| frames-1 | selected | cold | ase.db | 1 | 1 | 12 | 6.9073e-04 | 7.4824e-04 | 6.7256e-05 | 0.097 |  |  | 69632 | 102735872 | exact |
| frames-1 | selected | cold | readcon-db | unset | 8 | 12 | 2.0908e-05 | 2.1271e-05 | 5.1358e-07 | 0.025 |  |  | 40960 | 98353152 | exact |
| frames-1 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.1852e-05 | 1.2074e-05 | 7.7119e-08 | 0.007 |  |  | 40960 | 98201600 | exact |
| frames-1 | full_read | warm | ase.db | 1 | 1 | 12 | 1.5124e-04 | 1.5184e-04 | 8.0324e-07 | 0.005 |  |  | 69632 | 109412352 | exact |
| frames-1 | full_read | warm | readcon-db | unset | 8 | 12 | 1.1854e-05 | 1.2061e-05 | 6.6316e-08 | 0.006 |  |  | 40960 | 98189312 | exact |
| frames-1 | selected | warm | readcon-db | 1 | 1 | 12 | 1.1545e-05 | 1.1765e-05 | 7.6498e-08 | 0.007 |  |  | 40960 | 98160640 | exact |
| frames-1 | selected | warm | ase.db | 1 | 1 | 12 | 1.6653e-04 | 1.8138e-04 | 1.2431e-05 | 0.075 |  |  | 69632 | 108363776 | exact |
| frames-1 | selected | warm | readcon-db | unset | 8 | 12 | 1.1460e-05 | 1.1636e-05 | 1.3847e-07 | 0.012 |  |  | 40960 | 98160640 | exact |
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.5690e-03 | 4.6399e-03 | 1.0799e-04 | 0.024 | 2.1886e+04 | 1.6114e+01 | 65536 | 115736576 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.2231e-02 | 3.4145e-02 | 9.1750e-04 | 0.028 | 3.1026e+03 | 2.2843e+00 | 151552 | 100786176 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.5421e-03 | 4.5915e-03 | 7.1476e-05 | 0.016 | 2.2016e+04 | 1.6209e+01 | 65536 | 114118656 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.6185e-05 | 6.9232e-05 | 1.1901e-06 | 0.018 |  |  | 65536 | 104955904 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0249e-02 | 1.0468e-02 | 2.7384e-04 | 0.027 |  |  | 151552 | 116801536 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.6817e-05 | 7.1406e-05 | 4.5117e-06 | 0.068 |  |  | 65536 | 105066496 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.0682e-05 | 2.1892e-05 | 1.2657e-06 | 0.061 |  |  | 65536 | 102977536 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 8.5610e-04 | 1.0037e-03 | 1.3081e-04 | 0.153 |  |  | 151552 | 101822464 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.0926e-05 | 2.2836e-05 | 1.4268e-06 | 0.068 |  |  | 65536 | 102809600 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.6362e-05 | 6.9927e-05 | 1.3822e-06 | 0.021 |  |  | 65536 | 105357312 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 1.0196e-02 | 1.0798e-02 | 7.6718e-04 | 0.075 |  |  | 151552 | 117391360 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 7.0279e-05 | 7.7647e-05 | 8.8105e-06 | 0.125 |  |  | 65536 | 105172992 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.0915e-05 | 2.1693e-05 | 1.0447e-06 | 0.050 |  |  | 65536 | 102973440 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 6.6685e-04 | 7.4833e-04 | 7.3616e-05 | 0.110 |  |  | 151552 | 102715392 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.1463e-05 | 2.5340e-05 | 2.1343e-06 | 0.099 |  |  | 65536 | 102625280 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 3.2524e-04 | 3.9878e-04 | 5.6883e-05 | 0.175 |  |  | 65536 | 105058304 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.0006e-02 | 1.0419e-02 | 4.7340e-04 | 0.047 |  |  | 151552 | 118050816 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 3.5295e-04 | 4.2234e-04 | 3.8135e-05 | 0.108 |  |  | 65536 | 105316352 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 2.5735e-04 | 2.9879e-04 | 3.4957e-05 | 0.136 |  |  | 65536 | 102768640 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 9.6034e-04 | 1.3561e-03 | 2.5225e-04 | 0.263 |  |  | 151552 | 101138432 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.2671e-04 | 2.5302e-04 | 3.4932e-05 | 0.154 |  |  | 65536 | 102805504 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.3588e-05 | 4.3988e-05 | 2.2919e-07 | 0.005 |  |  | 65536 | 104816640 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 8.7130e-03 | 8.7607e-03 | 5.9828e-05 | 0.007 |  |  | 151552 | 119635968 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.3146e-05 | 4.4061e-05 | 7.8300e-07 | 0.018 |  |  | 65536 | 104960000 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.5111e-06 | 1.2003e-05 | 2.0094e-07 | 0.021 |  |  | 65536 | 102572032 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.7197e-04 | 1.7607e-04 | 1.3823e-06 | 0.008 |  |  | 151552 | 108339200 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.2635e-06 | 9.3612e-06 | 1.2226e-07 | 0.013 |  |  | 65536 | 102809600 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.3031e-01 | 3.3393e-01 | 4.0836e-03 | 0.012 | 3.0274e+04 | 2.3234e+01 | 3362816 | 194441216 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.0550e+00 | 3.1210e+00 | 5.2437e-02 | 0.017 | 3.2733e+03 | 2.5121e+00 | 10833920 | 200962048 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2988e-01 | 3.4014e-01 | 7.1684e-03 | 0.022 | 3.0314e+04 | 2.3264e+01 | 3362816 | 194605056 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.1114e-03 | 6.3908e-03 | 1.6081e-04 | 0.026 |  |  | 3362816 | 161071104 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.9637e-01 | 1.0168e+00 | 2.1496e-02 | 0.022 |  |  | 10817536 | 533798912 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.2233e-03 | 6.6695e-03 | 2.0984e-04 | 0.034 |  |  | 3362816 | 160993280 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.5728e-05 | 4.7296e-05 | 1.0373e-06 | 0.023 |  |  | 3362816 | 117411840 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 4.9779e-03 | 5.5922e-03 | 4.6942e-04 | 0.094 |  |  | 10817536 | 99352576 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.5768e-05 | 4.7964e-05 | 1.4682e-06 | 0.032 |  |  | 3362816 | 117268480 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.3072e-03 | 7.2555e-03 | 3.2963e-04 | 0.052 |  |  | 3362816 | 179838976 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 9.6484e-01 | 1.0625e+00 | 6.3040e-02 | 0.065 |  |  | 10817536 | 552329216 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.3452e-03 | 6.6810e-03 | 3.6030e-04 | 0.057 |  |  | 3362816 | 179847168 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.6025e-06 | 1.0895e-05 | 1.4343e-07 | 0.015 |  |  | 3362816 | 113168384 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.4692e-04 | 7.7821e-04 | 2.5108e-05 | 0.034 |  |  | 10817536 | 100945920 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 9.2806e-06 | 9.3990e-06 | 5.4896e-08 | 0.006 |  |  | 3362816 | 113201152 | exact |

