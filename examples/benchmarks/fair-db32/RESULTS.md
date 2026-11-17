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
| readcon-db | 4.1602e-03 | 8.1670e-04 | 3.9403e-03 | 4.7570e-03 | 2.6967e-03 | 1.1320e-02 |
| ase.db | 7.7911e-02 | 6.4512e-03 | 7.3653e-02 | 8.0104e-02 | 6.8207e-02 | 9.2594e-02 |

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
transaction still commits once. n is 12 serial samples
(READCON_DB_N_SAMPLES, never below 12). p90 is
the type-7 90th percentile of those means. A range read
materializes an inclusive frame-index span. A metadata read
returns the frame indexes whose energy is in the last tenth.
Ingest rows report frames/s and MB/s of the CON text. disk B
is the database file after that ingest.

| workload | op | cache | tool | RAYON_NUM_THREADS | threads | n | median s | p90 s | IQR s | IQR/median | frames/s | MB/s | disk B | peak RSS B | fidelity |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| paper40x8 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.2603e-03 | 2.3167e-03 | 3.9362e-05 | 0.017 | 1.7697e+04 | 5.5215e+00 | 49152 | 117878784 | exact |
| paper40x8 | insert | cold-file | ase.db | 1 | 1 | 12 | 5.7336e-03 | 5.9858e-03 | 3.2455e-04 | 0.057 | 6.9764e+03 | 2.1766e+00 | 90112 | 104890368 | exact |
| paper40x8 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.2091e-03 | 2.2850e-03 | 8.1085e-05 | 0.037 | 1.8107e+04 | 5.6493e+00 | 49152 | 117903360 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 1.3366e-03 | 1.5241e-03 | 1.4107e-04 | 0.106 | 7.4816e+02 | 3.1836e+01 | 98304 | 104538112 | exact |
| atoms-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.7707e-03 | 1.8518e-03 | 1.2812e-04 | 0.072 | 5.6475e+02 | 2.4031e+01 | 126976 | 102780928 | exact |
| atoms-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 1.3312e-03 | 1.4922e-03 | 1.8353e-04 | 0.138 | 7.5118e+02 | 3.1964e+01 | 98304 | 104407040 | exact |
| atoms-1000 | full_read | cold | readcon-db | 1 | 1 | 12 | 3.0909e-05 | 3.2638e-05 | 1.2617e-06 | 0.041 |  |  | 98304 | 100679680 | exact |
| atoms-1000 | full_read | cold | ase.db | 1 | 1 | 12 | 1.4131e-03 | 1.7028e-03 | 3.9366e-04 | 0.279 |  |  | 126976 | 131084288 | exact |
| atoms-1000 | full_read | cold | readcon-db | unset | 8 | 12 | 3.1426e-05 | 3.3007e-05 | 1.1389e-06 | 0.036 |  |  | 98304 | 100704256 | exact |
| atoms-1000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.9382e-05 | 3.0146e-05 | 4.8768e-07 | 0.017 |  |  | 98304 | 100683776 | exact |
| atoms-1000 | selected | cold | ase.db | 1 | 1 | 12 | 1.4830e-03 | 1.6742e-03 | 2.3614e-04 | 0.159 |  |  | 126976 | 130547712 | exact |
| atoms-1000 | selected | cold | readcon-db | unset | 8 | 12 | 2.8406e-05 | 2.9416e-05 | 1.1761e-06 | 0.041 |  |  | 98304 | 100478976 | exact |
| atoms-1000 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.6831e-05 | 1.6873e-05 | 4.9646e-08 | 0.003 |  |  | 98304 | 100560896 | exact |
| atoms-1000 | full_read | warm | ase.db | 1 | 1 | 12 | 4.3995e-04 | 5.1167e-04 | 7.9556e-05 | 0.181 |  |  | 126976 | 197865472 | exact |
| atoms-1000 | full_read | warm | readcon-db | unset | 8 | 12 | 1.6463e-05 | 1.6646e-05 | 1.4579e-07 | 0.009 |  |  | 98304 | 100331520 | exact |
| atoms-1000 | selected | warm | readcon-db | 1 | 1 | 12 | 1.5847e-05 | 1.5885e-05 | 6.1311e-08 | 0.004 |  |  | 98304 | 100577280 | exact |
| atoms-1000 | selected | warm | ase.db | 1 | 1 | 12 | 4.8342e-04 | 5.0356e-04 | 2.4773e-05 | 0.051 |  |  | 126976 | 192258048 | exact |
| atoms-1000 | selected | warm | readcon-db | unset | 8 | 12 | 1.6424e-05 | 1.6513e-05 | 6.3955e-08 | 0.004 |  |  | 98304 | 100450304 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.6756e-03 | 3.7549e-03 | 1.3545e-04 | 0.037 | 2.7206e+02 | 1.2551e+02 | 589824 | 118759424 | exact |
| atoms-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.7001e-03 | 3.8387e-03 | 1.3974e-04 | 0.038 | 2.7026e+02 | 1.2468e+02 | 667648 | 111464448 | exact |
| atoms-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.6716e-03 | 3.7208e-03 | 7.9613e-05 | 0.022 | 2.7236e+02 | 1.2565e+02 | 589824 | 118870016 | exact |
| atoms-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.8230e-04 | 2.8600e-04 | 2.6829e-06 | 0.010 |  |  | 589824 | 104787968 | exact |
| atoms-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 4.9253e-03 | 5.0300e-03 | 1.9398e-04 | 0.039 |  |  | 667648 | 196091904 | exact |
| atoms-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 2.8484e-04 | 2.9355e-04 | 3.1071e-06 | 0.011 |  |  | 589824 | 104701952 | exact |
| atoms-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.7642e-04 | 2.8265e-04 | 7.7021e-06 | 0.028 |  |  | 589824 | 105029632 | exact |
| atoms-10000 | selected | cold | ase.db | 1 | 1 | 12 | 4.8785e-03 | 4.9999e-03 | 1.7808e-04 | 0.037 |  |  | 667648 | 197705728 | exact |
| atoms-10000 | selected | cold | readcon-db | unset | 8 | 12 | 2.7573e-04 | 2.8061e-04 | 5.0342e-06 | 0.018 |  |  | 589824 | 104747008 | exact |
| atoms-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.5438e-04 | 2.5665e-04 | 3.8475e-06 | 0.015 |  |  | 589824 | 104112128 | exact |
| atoms-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.1741e-03 | 3.3954e-03 | 2.3719e-04 | 0.075 |  |  | 667648 | 238198784 | exact |
| atoms-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.6438e-04 | 2.7488e-04 | 2.8955e-06 | 0.011 |  |  | 589824 | 103985152 | exact |
| atoms-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.4136e-04 | 2.4493e-04 | 2.9177e-06 | 0.012 |  |  | 589824 | 103903232 | exact |
| atoms-10000 | selected | warm | ase.db | 1 | 1 | 12 | 3.1764e-03 | 3.3404e-03 | 1.5042e-05 | 0.005 |  |  | 667648 | 239140864 | exact |
| atoms-10000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5178e-04 | 2.5365e-04 | 2.0007e-06 | 0.008 |  |  | 589824 | 103841792 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.4768e-02 | 2.5548e-02 | 8.4247e-04 | 0.034 | 4.0375e+01 | 2.0224e+02 | 5390336 | 197365760 | exact |
| atoms-100000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.4911e-02 | 2.5600e-02 | 9.4480e-04 | 0.038 | 4.0142e+01 | 2.0108e+02 | 6074368 | 205299712 | exact |
| atoms-100000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.5578e-02 | 2.6215e-02 | 9.8442e-04 | 0.038 | 3.9096e+01 | 1.9583e+02 | 5390336 | 197255168 | exact |
| atoms-100000 | full_read | cold | readcon-db | 1 | 1 | 12 | 5.6241e-03 | 6.2761e-03 | 8.3797e-04 | 0.149 |  |  | 5390336 | 135200768 | exact |
| atoms-100000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.4032e-02 | 3.8802e-02 | 5.1596e-03 | 0.152 |  |  | 6074368 | 308928512 | exact |
| atoms-100000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.8981e-03 | 7.4536e-03 | 6.9423e-04 | 0.101 |  |  | 5390336 | 135278592 | exact |
| atoms-100000 | selected | cold | readcon-db | 1 | 1 | 12 | 5.2856e-03 | 5.8738e-03 | 1.0285e-03 | 0.195 |  |  | 5390336 | 135122944 | exact |
| atoms-100000 | selected | cold | ase.db | 1 | 1 | 12 | 3.3449e-02 | 3.3855e-02 | 6.9410e-04 | 0.021 |  |  | 6074368 | 297664512 | exact |
| atoms-100000 | selected | cold | readcon-db | unset | 8 | 12 | 5.3162e-03 | 5.9155e-03 | 8.0823e-04 | 0.152 |  |  | 5390336 | 135225344 | exact |
| atoms-100000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.5267e-03 | 2.5532e-03 | 1.2938e-05 | 0.005 |  |  | 5390336 | 128933888 | exact |
| atoms-100000 | full_read | warm | ase.db | 1 | 1 | 12 | 2.8448e-02 | 2.8713e-02 | 3.6187e-04 | 0.013 |  |  | 6074368 | 307007488 | exact |
| atoms-100000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.5256e-03 | 2.5827e-03 | 7.4366e-06 | 0.003 |  |  | 5390336 | 128811008 | exact |
| atoms-100000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.4532e-03 | 2.5159e-03 | 1.5289e-05 | 0.006 |  |  | 5390336 | 128856064 | exact |
| atoms-100000 | selected | warm | ase.db | 1 | 1 | 12 | 2.8712e-02 | 3.0886e-02 | 1.1894e-03 | 0.041 |  |  | 6074368 | 301744128 | exact |
| atoms-100000 | selected | warm | readcon-db | unset | 8 | 12 | 2.4845e-03 | 2.7224e-03 | 1.1174e-04 | 0.045 |  |  | 5390336 | 128815104 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.8619e-01 | 2.9110e-01 | 6.9317e-03 | 0.024 | 3.4942e+00 | 1.8899e+02 | 52592640 | 683171840 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.3656e-01 | 2.5316e-01 | 6.9426e-03 | 0.029 | 4.2273e+00 | 2.2864e+02 | 60125184 | 1135890432 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.8093e-01 | 3.1233e-01 | 1.4512e-02 | 0.052 | 3.5596e+00 | 1.9253e+02 | 52592640 | 683134976 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 1 | 1 | 12 | 5.7936e-02 | 8.2807e-02 | 9.2595e-03 | 0.160 |  |  | 52592640 | 427765760 | exact |
| atoms-1000000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.4425e-01 | 3.7091e-01 | 2.1407e-02 | 0.062 |  |  | 60125184 | 1337700352 | exact |
| atoms-1000000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.5767e-02 | 6.5715e-02 | 1.2393e-02 | 0.222 |  |  | 52592640 | 427479040 | exact |
| atoms-1000000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.7590e-02 | 6.0509e-02 | 1.0547e-02 | 0.222 |  |  | 52592640 | 427720704 | exact |
| atoms-1000000 | selected | cold | ase.db | 1 | 1 | 12 | 3.2115e-01 | 3.7948e-01 | 3.2902e-02 | 0.102 |  |  | 60125184 | 1320800256 | exact |
| atoms-1000000 | selected | cold | readcon-db | unset | 8 | 12 | 5.2326e-02 | 6.1383e-02 | 1.0723e-02 | 0.205 |  |  | 52592640 | 427687936 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.7261e-02 | 2.0332e-02 | 2.1871e-03 | 0.127 |  |  | 52592640 | 364675072 | exact |
| atoms-1000000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.0055e-01 | 3.5404e-01 | 2.6034e-02 | 0.087 |  |  | 60125184 | 1330696192 | exact |
| atoms-1000000 | full_read | warm | readcon-db | unset | 8 | 12 | 1.8564e-02 | 2.0415e-02 | 1.2841e-03 | 0.069 |  |  | 52592640 | 364769280 | exact |
| atoms-1000000 | selected | warm | readcon-db | 1 | 1 | 12 | 1.8337e-02 | 1.9416e-02 | 9.0106e-04 | 0.049 |  |  | 52592640 | 364691456 | exact |
| atoms-1000000 | selected | warm | ase.db | 1 | 1 | 12 | 2.8082e-01 | 2.9856e-01 | 9.9661e-03 | 0.035 |  |  | 60125184 | 1330630656 | exact |
| atoms-1000000 | selected | warm | readcon-db | unset | 8 | 12 | 1.8018e-02 | 1.9331e-02 | 1.8320e-03 | 0.102 |  |  | 52592640 | 364482560 | exact |
| frames-1 | insert | cold-file | readcon-db | 1 | 1 | 12 | 7.8042e-04 | 9.4498e-04 | 1.2806e-04 | 0.164 | 1.2814e+03 | 9.2899e-01 | 40960 | 102240256 | exact |
| frames-1 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.0789e-03 | 1.1487e-03 | 7.1385e-05 | 0.066 | 9.2689e+02 | 6.7200e-01 | 69632 | 105111552 | exact |
| frames-1 | insert | cold-file | readcon-db | unset | 8 | 12 | 7.9065e-04 | 9.6593e-04 | 1.8260e-04 | 0.231 | 1.2648e+03 | 9.1697e-01 | 40960 | 102109184 | exact |
| frames-1 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.2806e-05 | 2.3179e-05 | 3.5247e-07 | 0.015 |  |  | 40960 | 98512896 | exact |
| frames-1 | full_read | cold | ase.db | 1 | 1 | 12 | 5.8094e-04 | 6.1418e-04 | 5.9478e-05 | 0.102 |  |  | 69632 | 103374848 | exact |
| frames-1 | full_read | cold | readcon-db | unset | 8 | 12 | 2.2754e-05 | 2.3559e-05 | 8.8683e-07 | 0.039 |  |  | 40960 | 98566144 | exact |
| frames-1 | selected | cold | readcon-db | 1 | 1 | 12 | 2.1025e-05 | 2.1861e-05 | 7.2921e-07 | 0.035 |  |  | 40960 | 98557952 | exact |
| frames-1 | selected | cold | ase.db | 1 | 1 | 12 | 8.2697e-04 | 9.1595e-04 | 1.5127e-04 | 0.183 |  |  | 69632 | 102031360 | exact |
| frames-1 | selected | cold | readcon-db | unset | 8 | 12 | 2.0524e-05 | 2.1135e-05 | 5.3742e-07 | 0.026 |  |  | 40960 | 98406400 | exact |
| frames-1 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.1489e-05 | 1.1637e-05 | 1.3894e-07 | 0.012 |  |  | 40960 | 98422784 | exact |
| frames-1 | full_read | warm | ase.db | 1 | 1 | 12 | 1.5351e-04 | 1.5474e-04 | 1.9252e-06 | 0.013 |  |  | 69632 | 109518848 | exact |
| frames-1 | full_read | warm | readcon-db | unset | 8 | 12 | 1.1698e-05 | 1.2350e-05 | 2.7705e-07 | 0.024 |  |  | 40960 | 98238464 | exact |
| frames-1 | selected | warm | readcon-db | 1 | 1 | 12 | 1.1053e-05 | 1.1159e-05 | 1.1058e-07 | 0.010 |  |  | 40960 | 98267136 | exact |
| frames-1 | selected | warm | ase.db | 1 | 1 | 12 | 1.6939e-04 | 1.7683e-04 | 3.7593e-06 | 0.022 |  |  | 69632 | 108269568 | exact |
| frames-1 | selected | warm | readcon-db | unset | 8 | 12 | 1.0962e-05 | 1.1064e-05 | 8.8739e-08 | 0.008 |  |  | 40960 | 98316288 | exact |
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.5340e-03 | 4.6042e-03 | 6.5834e-05 | 0.015 | 2.2055e+04 | 1.6238e+01 | 65536 | 113664000 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.1978e-02 | 3.4549e-02 | 1.0269e-03 | 0.032 | 3.1271e+03 | 2.3024e+00 | 151552 | 100802560 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.5252e-03 | 4.6006e-03 | 1.0634e-04 | 0.023 | 2.2098e+04 | 1.6270e+01 | 65536 | 114061312 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.7944e-05 | 7.0199e-05 | 1.9736e-06 | 0.029 |  |  | 65536 | 105443328 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0095e-02 | 1.0331e-02 | 2.3175e-04 | 0.023 |  |  | 151552 | 117391360 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.4534e-05 | 6.7860e-05 | 1.3387e-06 | 0.021 |  |  | 65536 | 105467904 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.0075e-05 | 2.0540e-05 | 6.7301e-07 | 0.034 |  |  | 65536 | 103014400 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 1.1083e-03 | 1.1722e-03 | 1.5217e-04 | 0.137 |  |  | 151552 | 101294080 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.1041e-05 | 2.1577e-05 | 4.8180e-07 | 0.023 |  |  | 65536 | 102998016 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.6728e-05 | 6.8008e-05 | 1.1933e-06 | 0.018 |  |  | 65536 | 105115648 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 9.9581e-03 | 1.0801e-02 | 6.7034e-04 | 0.067 |  |  | 151552 | 118349824 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 6.9019e-05 | 7.0792e-05 | 1.3587e-06 | 0.020 |  |  | 65536 | 105385984 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 1.9440e-05 | 2.0443e-05 | 8.4166e-07 | 0.043 |  |  | 65536 | 103112704 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 1.0905e-03 | 1.1605e-03 | 1.0141e-04 | 0.093 |  |  | 151552 | 101281792 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.1172e-05 | 2.1563e-05 | 3.8818e-07 | 0.018 |  |  | 65536 | 103260160 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 2.6551e-04 | 2.9932e-04 | 2.9160e-05 | 0.110 |  |  | 65536 | 105459712 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.0147e-02 | 1.0512e-02 | 4.3770e-04 | 0.043 |  |  | 151552 | 117919744 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 3.6460e-04 | 4.8551e-04 | 1.2269e-04 | 0.337 |  |  | 65536 | 105099264 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 2.6473e-04 | 3.0754e-04 | 1.1607e-04 | 0.438 |  |  | 65536 | 102907904 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 1.0224e-03 | 1.4832e-03 | 2.4408e-04 | 0.239 |  |  | 151552 | 101376000 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.2876e-04 | 2.7406e-04 | 4.8510e-05 | 0.212 |  |  | 65536 | 102916096 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.3081e-05 | 4.3528e-05 | 3.2386e-07 | 0.008 |  |  | 65536 | 104931328 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 8.6057e-03 | 8.8186e-03 | 8.4469e-05 | 0.010 |  |  | 151552 | 119906304 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.3789e-05 | 4.4086e-05 | 2.3925e-07 | 0.005 |  |  | 65536 | 104951808 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.1505e-06 | 9.2060e-06 | 6.3150e-08 | 0.007 |  |  | 65536 | 102834176 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.6930e-04 | 1.7025e-04 | 2.0317e-06 | 0.012 |  |  | 151552 | 108171264 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.1608e-06 | 9.2656e-06 | 5.7543e-08 | 0.006 |  |  | 65536 | 102920192 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.2523e-01 | 3.2714e-01 | 2.5188e-03 | 0.008 | 3.0748e+04 | 2.3597e+01 | 3362816 | 184205312 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.9959e+00 | 3.0208e+00 | 4.1710e-02 | 0.014 | 3.3379e+03 | 2.5617e+00 | 10801152 | 201162752 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.1988e-01 | 3.2607e-01 | 5.0302e-03 | 0.016 | 3.1261e+04 | 2.3991e+01 | 3362816 | 196075520 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.0512e-03 | 7.1507e-03 | 6.1615e-04 | 0.102 |  |  | 3362816 | 161062912 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.2588e-01 | 1.0078e+00 | 8.4847e-02 | 0.092 |  |  | 10792960 | 533827584 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.1802e-03 | 6.3860e-03 | 2.7827e-04 | 0.045 |  |  | 3362816 | 161091584 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.3335e-05 | 4.5018e-05 | 6.7904e-07 | 0.016 |  |  | 3362816 | 117530624 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 4.5176e-03 | 5.2465e-03 | 2.0475e-03 | 0.453 |  |  | 10792960 | 99500032 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.5206e-05 | 4.5697e-05 | 6.7804e-07 | 0.015 |  |  | 3362816 | 117510144 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.6220e-03 | 7.8464e-03 | 7.6266e-04 | 0.115 |  |  | 3362816 | 179707904 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 9.0089e-01 | 9.2632e-01 | 1.9171e-02 | 0.021 |  |  | 10792960 | 552587264 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.0040e-03 | 6.4173e-03 | 1.3461e-04 | 0.022 |  |  | 3362816 | 179908608 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.4034e-06 | 9.5194e-06 | 7.6322e-08 | 0.008 |  |  | 3362816 | 113373184 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.3323e-04 | 7.4468e-04 | 8.7686e-06 | 0.012 |  |  | 10792960 | 101142528 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 9.4194e-06 | 9.5027e-06 | 9.9771e-08 | 0.011 |  |  | 3362816 | 113381376 | exact |
