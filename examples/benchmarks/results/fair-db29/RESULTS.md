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
| readcon-db | 5.1147e-03 | 3.5265e-03 | 3.6188e-03 | 7.1452e-03 | 2.2913e-03 | 9.8156e-03 |
| ase.db | 5.3779e-02 | 2.5180e-03 | 5.3524e-02 | 5.6042e-02 | 5.1810e-02 | 8.2114e-02 |

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

| workload | op | cache | tool | n | median s | p90 s | IQR s | IQR/median | frames/s | MB/s | disk B | peak RSS B | fidelity |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| paper40x8 | insert | cold-file | readcon-db | 6 | 2.2871e-03 | 2.3595e-03 | 4.8894e-05 | 0.021 | 1.7489e+04 | 5.4566e+00 | 49152 | 118722560 | exact |
| paper40x8 | insert | cold-file | ase.db | 6 | 5.6017e-03 | 5.7794e-03 | 3.8257e-05 | 0.007 | 7.1406e+03 | 2.2279e+00 | 90112 | 106143744 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 12 | 1.2876e-03 | 1.4302e-03 | 2.1989e-04 | 0.171 | 7.7662e+02 | 3.3047e+01 | 98304 | 103665664 | exact |
| atoms-1000 | insert | cold-file | ase.db | 6 | 1.5427e-03 | 1.6082e-03 | 7.5555e-05 | 0.049 | 6.4823e+02 | 2.7583e+01 | 126976 | 103755776 | exact |
| atoms-1000 | full_read | cold | readcon-db | 6 | 3.1883e-05 | 3.2617e-05 | 7.5871e-07 | 0.024 |  |  | 98304 | 101556224 | exact |
| atoms-1000 | full_read | cold | ase.db | 12 | 2.0304e-03 | 2.2734e-03 | 2.2930e-04 | 0.113 |  |  | 126976 | 123650048 | exact |
| atoms-1000 | selected | cold | readcon-db | 6 | 3.1240e-05 | 3.2758e-05 | 1.1226e-06 | 0.036 |  |  | 98304 | 101629952 | exact |
| atoms-1000 | selected | cold | ase.db | 6 | 1.7569e-03 | 1.8480e-03 | 4.9916e-05 | 0.028 |  |  | 126976 | 113258496 | exact |
| atoms-1000 | full_read | warm | readcon-db | 6 | 1.6555e-05 | 1.6631e-05 | 4.5535e-08 | 0.003 |  |  | 98304 | 101613568 | exact |
| atoms-1000 | full_read | warm | ase.db | 12 | 7.9115e-04 | 1.0032e-03 | 2.7546e-04 | 0.348 |  |  | 126976 | 164720640 | exact |
| atoms-1000 | selected | warm | readcon-db | 6 | 1.6256e-05 | 1.6333e-05 | 6.9104e-08 | 0.004 |  |  | 98304 | 101453824 | exact |
| atoms-1000 | selected | warm | ase.db | 9 | 6.5317e-04 | 6.8847e-04 | 3.1560e-05 | 0.048 |  |  | 126976 | 153387008 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 6 | 3.8324e-03 | 3.8563e-03 | 3.2526e-05 | 0.008 | 2.6093e+02 | 1.2037e+02 | 589824 | 119361536 | exact |
| atoms-10000 | insert | cold-file | ase.db | 12 | 3.6268e-03 | 3.8118e-03 | 2.1967e-04 | 0.061 | 2.7573e+02 | 1.2720e+02 | 667648 | 112640000 | exact |
| atoms-10000 | full_read | cold | readcon-db | 6 | 2.9294e-04 | 2.9347e-04 | 2.5557e-07 | 0.001 |  |  | 589824 | 105779200 | exact |
| atoms-10000 | full_read | cold | ase.db | 12 | 5.2235e-03 | 8.2836e-03 | 1.6776e-03 | 0.321 |  |  | 667648 | 185946112 | exact |
| atoms-10000 | selected | cold | readcon-db | 6 | 2.9313e-04 | 2.9486e-04 | 9.0480e-07 | 0.003 |  |  | 589824 | 105934848 | exact |
| atoms-10000 | selected | cold | ase.db | 6 | 5.5761e-03 | 5.8329e-03 | 4.6624e-05 | 0.008 |  |  | 667648 | 143872000 | exact |
| atoms-10000 | full_read | warm | readcon-db | 6 | 2.7152e-04 | 2.7809e-04 | 4.5056e-06 | 0.017 |  |  | 589824 | 105009152 | exact |
| atoms-10000 | full_read | warm | ase.db | 6 | 4.3440e-03 | 4.5191e-03 | 1.5921e-04 | 0.037 |  |  | 667648 | 153772032 | exact |
| atoms-10000 | selected | warm | readcon-db | 6 | 2.6502e-04 | 2.6786e-04 | 2.3145e-06 | 0.009 |  |  | 589824 | 104665088 | exact |
| atoms-10000 | selected | warm | ase.db | 12 | 4.1227e-03 | 4.3614e-03 | 6.1393e-04 | 0.149 |  |  | 667648 | 220504064 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 6 | 2.9094e-02 | 2.9746e-02 | 6.1822e-04 | 0.021 | 3.4371e+01 | 1.7217e+02 | 5390336 | 177799168 | exact |
| atoms-100000 | insert | cold-file | ase.db | 6 | 2.7197e-02 | 2.7573e-02 | 3.8874e-04 | 0.014 | 3.6769e+01 | 1.8418e+02 | 6074368 | 205238272 | exact |
| atoms-100000 | full_read | cold | readcon-db | 12 | 6.2759e-03 | 6.6832e-03 | 4.7511e-04 | 0.076 |  |  | 5390336 | 136347648 | exact |
| atoms-100000 | full_read | cold | ase.db | 6 | 3.3129e-02 | 3.5481e-02 | 5.4640e-04 | 0.016 |  |  | 6074368 | 221646848 | exact |
| atoms-100000 | selected | cold | readcon-db | 9 | 7.4136e-03 | 8.3009e-03 | 2.3707e-04 | 0.032 |  |  | 5390336 | 136400896 | exact |
| atoms-100000 | selected | cold | ase.db | 6 | 3.9297e-02 | 3.9903e-02 | 1.4581e-03 | 0.037 |  |  | 6074368 | 221777920 | exact |
| atoms-100000 | full_read | warm | readcon-db | 6 | 2.5745e-03 | 2.6467e-03 | 2.5824e-05 | 0.010 |  |  | 5390336 | 129769472 | exact |
| atoms-100000 | full_read | warm | ase.db | 12 | 3.4530e-02 | 3.9898e-02 | 6.6212e-03 | 0.192 |  |  | 6074368 | 308342784 | exact |
| atoms-100000 | selected | warm | readcon-db | 6 | 2.5844e-03 | 2.6967e-03 | 5.0441e-05 | 0.020 |  |  | 5390336 | 129548288 | exact |
| atoms-100000 | selected | warm | ase.db | 6 | 3.0846e-02 | 3.4364e-02 | 1.5246e-03 | 0.049 |  |  | 6074368 | 221540352 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 6 | 3.0821e-01 | 3.1056e-01 | 2.2261e-03 | 0.007 | 3.2446e+00 | 1.7549e+02 | 52592640 | 547909632 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 10 | 2.5567e-01 | 2.6163e-01 | 1.2635e-02 | 0.049 | 3.9114e+00 | 2.1155e+02 | 60125184 | 1144741888 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 12 | 6.7073e-02 | 8.7282e-02 | 1.7835e-02 | 0.266 |  |  | 52592640 | 428371968 | exact |
| atoms-1000000 | full_read | cold | ase.db | 12 | 3.5340e-01 | 3.7648e-01 | 1.6976e-02 | 0.048 |  |  | 60125184 | 1319174144 | exact |
| atoms-1000000 | selected | cold | readcon-db | 12 | 6.7658e-02 | 8.0040e-02 | 8.1683e-03 | 0.121 |  |  | 52592640 | 428552192 | exact |
| atoms-1000000 | selected | cold | ase.db | 6 | 3.5220e-01 | 3.7361e-01 | 4.4922e-03 | 0.013 |  |  | 60125184 | 937242624 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 6 | 2.2889e-02 | 2.3977e-02 | 1.1139e-03 | 0.049 |  |  | 52592640 | 365719552 | exact |
| atoms-1000000 | full_read | warm | ase.db | 6 | 3.0143e-01 | 3.0717e-01 | 6.8194e-03 | 0.023 |  |  | 60125184 | 915226624 | exact |
| atoms-1000000 | selected | warm | readcon-db | 12 | 2.3715e-02 | 2.6113e-02 | 1.9571e-03 | 0.083 |  |  | 52592640 | 365432832 | exact |
| atoms-1000000 | selected | warm | ase.db | 12 | 2.9518e-01 | 3.3472e-01 | 1.6780e-02 | 0.057 |  |  | 60125184 | 1340051456 | exact |
| frames-1 | insert | cold-file | readcon-db | 12 | 8.5110e-04 | 1.0309e-03 | 1.7371e-04 | 0.204 | 1.1750e+03 | 8.5184e-01 | 40960 | 101412864 | exact |
| frames-1 | insert | cold-file | ase.db | 12 | 1.2480e-03 | 1.3438e-03 | 7.5744e-05 | 0.061 | 8.0127e+02 | 5.8092e-01 | 69632 | 106635264 | exact |
| frames-1 | full_read | cold | readcon-db | 12 | 2.3269e-05 | 2.8845e-05 | 2.7168e-06 | 0.117 |  |  | 40960 | 99962880 | exact |
| frames-1 | full_read | cold | ase.db | 8 | 5.7678e-04 | 6.0444e-04 | 2.6182e-05 | 0.045 |  |  | 69632 | 103120896 | exact |
| frames-1 | selected | cold | readcon-db | 6 | 2.1031e-05 | 2.2107e-05 | 5.3183e-07 | 0.025 |  |  | 40960 | 99885056 | exact |
| frames-1 | selected | cold | ase.db | 12 | 6.4928e-04 | 8.8495e-04 | 2.1996e-04 | 0.339 |  |  | 69632 | 104210432 | exact |
| frames-1 | full_read | warm | readcon-db | 6 | 1.1674e-05 | 1.1756e-05 | 1.0527e-07 | 0.009 |  |  | 40960 | 100069376 | exact |
| frames-1 | full_read | warm | ase.db | 6 | 1.5213e-04 | 1.5349e-04 | 1.5497e-06 | 0.010 |  |  | 69632 | 105508864 | exact |
| frames-1 | selected | warm | readcon-db | 6 | 1.1273e-05 | 1.1351e-05 | 8.3926e-09 | 0.001 |  |  | 40960 | 99852288 | exact |
| frames-1 | selected | warm | ase.db | 6 | 1.6624e-04 | 1.7315e-04 | 6.0531e-06 | 0.036 |  |  | 69632 | 104914944 | exact |
| frames-100 | insert | cold-file | readcon-db | 6 | 4.5919e-03 | 4.6644e-03 | 7.7419e-05 | 0.017 | 2.1777e+04 | 1.6034e+01 | 65536 | 115122176 | exact |
| frames-100 | insert | cold-file | ase.db | 6 | 3.2527e-02 | 3.3737e-02 | 8.9644e-04 | 0.028 | 3.0744e+03 | 2.2635e+00 | 151552 | 102125568 | exact |
| frames-100 | full_read | cold | readcon-db | 6 | 6.8926e-05 | 7.0387e-05 | 2.0723e-06 | 0.030 |  |  | 65536 | 106450944 | exact |
| frames-100 | full_read | cold | ase.db | 6 | 1.0129e-02 | 1.0466e-02 | 2.1323e-04 | 0.021 |  |  | 151552 | 109764608 | exact |
| frames-100 | selected | cold | readcon-db | 12 | 2.2400e-05 | 2.4312e-05 | 1.3044e-06 | 0.058 |  |  | 65536 | 104587264 | exact |
| frames-100 | selected | cold | ase.db | 12 | 9.0448e-04 | 1.0105e-03 | 1.9590e-04 | 0.217 |  |  | 151552 | 103448576 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 6 | 7.4076e-05 | 7.7019e-05 | 3.3842e-06 | 0.046 |  |  | 65536 | 106045440 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 6 | 1.0142e-02 | 1.0535e-02 | 3.3397e-04 | 0.033 |  |  | 151552 | 110080000 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 6 | 2.1352e-05 | 2.1431e-05 | 3.4157e-07 | 0.016 |  |  | 65536 | 104456192 | exact |
| frames-100 | selected | warm-after-open | ase.db | 12 | 5.3360e-04 | 7.3355e-04 | 1.7651e-04 | 0.331 |  |  | 151552 | 104873984 | exact |
| frames-100 | full_read | cold-open | readcon-db | 12 | 3.6883e-04 | 4.4317e-04 | 8.0462e-05 | 0.218 |  |  | 65536 | 106553344 | exact |
| frames-100 | full_read | cold-open | ase.db | 6 | 1.0318e-02 | 1.0741e-02 | 3.7110e-04 | 0.036 |  |  | 151552 | 109670400 | exact |
| frames-100 | selected | cold-open | readcon-db | 12 | 2.4742e-04 | 2.6910e-04 | 2.7827e-05 | 0.112 |  |  | 65536 | 104574976 | exact |
| frames-100 | selected | cold-open | ase.db | 12 | 1.2322e-03 | 1.4299e-03 | 2.6141e-04 | 0.212 |  |  | 151552 | 102440960 | exact |
| frames-100 | full_read | warm | readcon-db | 6 | 4.4052e-05 | 4.4926e-05 | 3.2829e-07 | 0.007 |  |  | 65536 | 106668032 | exact |
| frames-100 | full_read | warm | ase.db | 12 | 9.2584e-03 | 1.0014e-02 | 5.9069e-04 | 0.064 |  |  | 151552 | 121348096 | exact |
| frames-100 | selected | warm | readcon-db | 6 | 9.6710e-06 | 9.7972e-06 | 1.0957e-07 | 0.011 |  |  | 65536 | 104591360 | exact |
| frames-100 | selected | warm | ase.db | 6 | 1.7242e-04 | 1.7425e-04 | 9.8084e-07 | 0.006 |  |  | 151552 | 104902656 | exact |
| frames-10000 | insert | cold-file | readcon-db | 6 | 2.6721e-01 | 2.7021e-01 | 7.0224e-03 | 0.026 | 3.7423e+04 | 2.8720e+01 | 3362816 | 200048640 | exact |
| frames-10000 | insert | cold-file | ase.db | 6 | 3.0784e+00 | 3.1236e+00 | 2.5678e-02 | 0.008 | 3.2484e+03 | 2.4930e+00 | 10809344 | 200736768 | exact |
| frames-10000 | full_read | cold | readcon-db | 6 | 6.4131e-03 | 1.0153e-02 | 2.7198e-04 | 0.042 |  |  | 3362816 | 189001728 | exact |
| frames-10000 | full_read | cold | ase.db | 6 | 1.0325e+00 | 1.0651e+00 | 2.8273e-02 | 0.027 |  |  | 10825728 | 366612480 | exact |
| frames-10000 | selected | cold | readcon-db | 6 | 4.6303e-05 | 4.6938e-05 | 7.3401e-07 | 0.016 |  |  | 3362816 | 118366208 | exact |
| frames-10000 | selected | cold | ase.db | 12 | 5.1330e-03 | 5.7910e-03 | 8.8345e-04 | 0.172 |  |  | 10825728 | 100978688 | exact |
| frames-10000 | full_read | warm | readcon-db | 12 | 6.3478e-03 | 7.0010e-03 | 4.5833e-04 | 0.072 |  |  | 3362816 | 178905088 | exact |
| frames-10000 | full_read | warm | ase.db | 6 | 1.0671e+00 | 1.0955e+00 | 1.0136e-02 | 0.009 |  |  | 10825728 | 385404928 | exact |
| frames-10000 | selected | warm | readcon-db | 6 | 9.8331e-06 | 9.9618e-06 | 2.2272e-08 | 0.002 |  |  | 3362816 | 115068928 | exact |
| frames-10000 | selected | warm | ase.db | 6 | 7.3189e-04 | 8.0819e-04 | 1.4015e-05 | 0.019 |  |  | 10825728 | 101457920 | exact |

