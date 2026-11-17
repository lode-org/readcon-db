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
| readcon-db | 4.3172e-03 | 8.9627e-04 | 3.8090e-03 | 4.7053e-03 | 2.8217e-03 | 9.1591e-03 |
| ase.db | 9.5657e-02 | 3.4100e-02 | 7.2294e-02 | 1.0639e-01 | 4.8785e-02 | 1.1584e-01 |

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
| paper40x8 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.6304e-03 | 2.7118e-03 | 8.5791e-05 | 0.033 | 1.5207e+04 | 4.7446e+00 | 49152 | 117788672 | exact |
| paper40x8 | insert | cold-file | ase.db | 1 | 1 | 12 | 5.7855e-03 | 6.2719e-03 | 2.3837e-04 | 0.041 | 6.9138e+03 | 2.1571e+00 | 90112 | 104730624 | exact |
| paper40x8 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.6381e-03 | 2.6909e-03 | 9.1988e-05 | 0.035 | 1.5163e+04 | 4.7308e+00 | 49152 | 117637120 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 1.6545e-03 | 1.7083e-03 | 1.0455e-04 | 0.063 | 6.0440e+02 | 2.5719e+01 | 98304 | 102248448 | exact |
| atoms-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.8740e-03 | 2.0736e-03 | 2.9920e-04 | 0.160 | 5.3361e+02 | 2.2706e+01 | 126976 | 102428672 | exact |
| atoms-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 1.6876e-03 | 1.8058e-03 | 1.3034e-04 | 0.077 | 5.9257e+02 | 2.5215e+01 | 98304 | 102436864 | exact |
| atoms-1000 | full_read | cold | readcon-db | 1 | 1 | 12 | 3.3845e-05 | 3.4812e-05 | 1.2084e-06 | 0.036 |  |  | 98304 | 100655104 | exact |
| atoms-1000 | full_read | cold | ase.db | 1 | 1 | 12 | 1.4213e-03 | 1.7026e-03 | 2.4900e-04 | 0.175 |  |  | 126976 | 131084288 | exact |
| atoms-1000 | full_read | cold | readcon-db | unset | 8 | 12 | 3.4651e-05 | 3.5485e-05 | 6.9040e-07 | 0.020 |  |  | 98304 | 100503552 | exact |
| atoms-1000 | selected | cold | readcon-db | 1 | 1 | 12 | 3.1961e-05 | 3.2214e-05 | 6.1832e-07 | 0.019 |  |  | 98304 | 100524032 | exact |
| atoms-1000 | selected | cold | ase.db | 1 | 1 | 12 | 2.1282e-03 | 2.5175e-03 | 6.6196e-04 | 0.311 |  |  | 126976 | 123088896 | exact |
| atoms-1000 | selected | cold | readcon-db | unset | 8 | 12 | 3.2284e-05 | 3.3484e-05 | 1.2000e-06 | 0.037 |  |  | 98304 | 100495360 | exact |
| atoms-1000 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.7051e-05 | 1.7251e-05 | 1.2864e-07 | 0.008 |  |  | 98304 | 100528128 | exact |
| atoms-1000 | full_read | warm | ase.db | 1 | 1 | 12 | 4.3564e-04 | 5.3347e-04 | 1.1189e-04 | 0.257 |  |  | 126976 | 195035136 | exact |
| atoms-1000 | full_read | warm | readcon-db | unset | 8 | 12 | 1.6404e-05 | 1.6485e-05 | 6.8609e-08 | 0.004 |  |  | 98304 | 100429824 | exact |
| atoms-1000 | selected | warm | readcon-db | 1 | 1 | 12 | 1.6442e-05 | 1.6533e-05 | 1.2828e-07 | 0.008 |  |  | 98304 | 100597760 | exact |
| atoms-1000 | selected | warm | ase.db | 1 | 1 | 12 | 5.4550e-04 | 5.5358e-04 | 1.3157e-04 | 0.241 |  |  | 126976 | 189730816 | exact |
| atoms-1000 | selected | warm | readcon-db | unset | 8 | 12 | 1.6215e-05 | 1.6244e-05 | 6.3169e-08 | 0.004 |  |  | 98304 | 100528128 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.8824e-03 | 3.9306e-03 | 1.5974e-04 | 0.041 | 2.5757e+02 | 1.1882e+02 | 589824 | 120520704 | exact |
| atoms-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 4.3068e-03 | 4.3849e-03 | 2.2392e-04 | 0.052 | 2.3219e+02 | 1.0712e+02 | 667648 | 111751168 | exact |
| atoms-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.7737e-03 | 3.9969e-03 | 2.0139e-04 | 0.053 | 2.6499e+02 | 1.2225e+02 | 589824 | 120619008 | exact |
| atoms-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.8900e-04 | 3.0027e-04 | 1.1741e-05 | 0.041 |  |  | 589824 | 104693760 | exact |
| atoms-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 5.2199e-03 | 5.2846e-03 | 8.2382e-05 | 0.016 |  |  | 667648 | 188579840 | exact |
| atoms-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 2.8277e-04 | 2.8618e-04 | 1.0655e-06 | 0.004 |  |  | 589824 | 104841216 | exact |
| atoms-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.9054e-04 | 2.9928e-04 | 2.8132e-06 | 0.010 |  |  | 589824 | 104603648 | exact |
| atoms-10000 | selected | cold | ase.db | 1 | 1 | 12 | 5.4756e-03 | 5.7932e-03 | 6.5451e-04 | 0.120 |  |  | 667648 | 189018112 | exact |
| atoms-10000 | selected | cold | readcon-db | unset | 8 | 12 | 2.9996e-04 | 3.0636e-04 | 1.4281e-05 | 0.048 |  |  | 589824 | 104722432 | exact |
| atoms-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.6768e-04 | 2.7194e-04 | 1.4710e-06 | 0.005 |  |  | 589824 | 103993344 | exact |
| atoms-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.1900e-03 | 3.5208e-03 | 1.1166e-04 | 0.035 |  |  | 667648 | 238206976 | exact |
| atoms-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.5177e-04 | 2.5378e-04 | 5.7374e-07 | 0.002 |  |  | 589824 | 104206336 | exact |
| atoms-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.6378e-04 | 2.7557e-04 | 4.0417e-06 | 0.015 |  |  | 589824 | 103923712 | exact |
| atoms-10000 | selected | warm | ase.db | 1 | 1 | 12 | 3.5895e-03 | 3.7475e-03 | 4.9491e-04 | 0.138 |  |  | 667648 | 230973440 | exact |
| atoms-10000 | selected | warm | readcon-db | unset | 8 | 12 | 2.4681e-04 | 2.4754e-04 | 8.7467e-07 | 0.004 |  |  | 589824 | 103845888 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.6884e-02 | 2.7395e-02 | 6.7531e-04 | 0.025 | 3.7197e+01 | 1.8632e+02 | 5390336 | 197001216 | exact |
| atoms-100000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.6265e-02 | 2.7268e-02 | 1.6167e-03 | 0.062 | 3.8073e+01 | 1.9071e+02 | 6074368 | 208592896 | exact |
| atoms-100000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.8034e-02 | 2.8351e-02 | 4.4522e-04 | 0.016 | 3.5671e+01 | 1.7868e+02 | 5390336 | 197255168 | exact |
| atoms-100000 | full_read | cold | readcon-db | 1 | 1 | 12 | 7.3700e-03 | 7.8862e-03 | 4.1411e-04 | 0.056 |  |  | 5390336 | 135098368 | exact |
| atoms-100000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.4458e-02 | 3.6439e-02 | 1.8757e-03 | 0.054 |  |  | 6074368 | 297246720 | exact |
| atoms-100000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.4826e-03 | 7.3654e-03 | 6.1488e-04 | 0.095 |  |  | 5390336 | 135061504 | exact |
| atoms-100000 | selected | cold | readcon-db | 1 | 1 | 12 | 6.6177e-03 | 7.6463e-03 | 1.1457e-03 | 0.173 |  |  | 5390336 | 135098368 | exact |
| atoms-100000 | selected | cold | ase.db | 1 | 1 | 12 | 3.5254e-02 | 3.7303e-02 | 2.2210e-03 | 0.063 |  |  | 6074368 | 297664512 | exact |
| atoms-100000 | selected | cold | readcon-db | unset | 8 | 12 | 6.6070e-03 | 7.1535e-03 | 4.3269e-04 | 0.065 |  |  | 5390336 | 135020544 | exact |
| atoms-100000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.6651e-03 | 2.7388e-03 | 3.1432e-05 | 0.012 |  |  | 5390336 | 128843776 | exact |
| atoms-100000 | full_read | warm | ase.db | 1 | 1 | 12 | 2.9276e-02 | 2.9500e-02 | 2.5309e-04 | 0.009 |  |  | 6074368 | 306311168 | exact |
| atoms-100000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.6634e-03 | 2.7422e-03 | 5.8911e-05 | 0.022 |  |  | 5390336 | 128790528 | exact |
| atoms-100000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.6816e-03 | 2.7473e-03 | 3.5857e-05 | 0.013 |  |  | 5390336 | 128876544 | exact |
| atoms-100000 | selected | warm | ase.db | 1 | 1 | 12 | 3.3418e-02 | 3.4178e-02 | 1.0823e-03 | 0.032 |  |  | 6074368 | 301146112 | exact |
| atoms-100000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5944e-03 | 2.6583e-03 | 2.1362e-05 | 0.008 |  |  | 5390336 | 128704512 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.8609e-01 | 2.9003e-01 | 3.9717e-03 | 0.014 | 3.4954e+00 | 1.8905e+02 | 52592640 | 733442048 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.6014e-01 | 2.6884e-01 | 1.4964e-02 | 0.058 | 3.8441e+00 | 2.0792e+02 | 60125184 | 1127813120 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.8259e-01 | 2.8528e-01 | 3.4073e-03 | 0.012 | 3.5387e+00 | 1.9139e+02 | 52592640 | 732909568 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 1 | 1 | 12 | 5.5064e-02 | 9.1043e-02 | 2.3629e-02 | 0.429 |  |  | 52592640 | 427364352 | exact |
| atoms-1000000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.3917e-01 | 3.4426e-01 | 1.0108e-02 | 0.030 |  |  | 60125184 | 1321443328 | exact |
| atoms-1000000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.4910e-02 | 6.0741e-02 | 1.1403e-02 | 0.208 |  |  | 52592640 | 427352064 | exact |
| atoms-1000000 | selected | cold | readcon-db | 1 | 1 | 12 | 5.3529e-02 | 6.4561e-02 | 5.0931e-03 | 0.095 |  |  | 52592640 | 427433984 | exact |
| atoms-1000000 | selected | cold | ase.db | 1 | 1 | 12 | 3.3159e-01 | 3.6343e-01 | 1.6596e-02 | 0.050 |  |  | 60125184 | 1320521728 | exact |
| atoms-1000000 | selected | cold | readcon-db | unset | 8 | 12 | 5.5692e-02 | 6.6374e-02 | 9.8181e-03 | 0.176 |  |  | 52592640 | 427503616 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.8238e-02 | 3.0332e-02 | 2.8995e-03 | 0.103 |  |  | 52592640 | 364470272 | exact |
| atoms-1000000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.1668e-01 | 3.2379e-01 | 9.6586e-03 | 0.030 |  |  | 60125184 | 1330593792 | exact |
| atoms-1000000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.4980e-02 | 2.6074e-02 | 9.6772e-04 | 0.039 |  |  | 52592640 | 364556288 | exact |
| atoms-1000000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.4885e-02 | 2.7624e-02 | 1.8205e-03 | 0.073 |  |  | 52592640 | 364425216 | exact |
| atoms-1000000 | selected | warm | ase.db | 1 | 1 | 12 | 2.9087e-01 | 3.1749e-01 | 2.2406e-02 | 0.077 |  |  | 60125184 | 1330565120 | exact |
| atoms-1000000 | selected | warm | readcon-db | unset | 8 | 12 | 2.4643e-02 | 2.5672e-02 | 1.4654e-03 | 0.059 |  |  | 52592640 | 364466176 | exact |
| frames-1 | insert | cold-file | readcon-db | 1 | 1 | 12 | 1.1907e-03 | 1.3808e-03 | 1.9890e-04 | 0.167 | 8.3987e+02 | 6.0890e-01 | 40960 | 99872768 | exact |
| frames-1 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.3612e-03 | 1.4424e-03 | 1.1520e-04 | 0.085 | 7.3467e+02 | 5.3264e-01 | 69632 | 105299968 | exact |
| frames-1 | insert | cold-file | readcon-db | unset | 8 | 12 | 1.1437e-03 | 1.2858e-03 | 1.6227e-04 | 0.142 | 8.7434e+02 | 6.3390e-01 | 40960 | 100290560 | exact |
| frames-1 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.3972e-05 | 2.4690e-05 | 4.0662e-07 | 0.017 |  |  | 40960 | 98336768 | exact |
| frames-1 | full_read | cold | ase.db | 1 | 1 | 12 | 5.9927e-04 | 6.5331e-04 | 5.1188e-05 | 0.085 |  |  | 69632 | 103067648 | exact |
| frames-1 | full_read | cold | readcon-db | unset | 8 | 12 | 2.3773e-05 | 2.8577e-05 | 7.0173e-07 | 0.030 |  |  | 40960 | 98521088 | exact |
| frames-1 | selected | cold | readcon-db | 1 | 1 | 12 | 2.2273e-05 | 2.2854e-05 | 4.4006e-07 | 0.020 |  |  | 40960 | 98369536 | exact |
| frames-1 | selected | cold | ase.db | 1 | 1 | 12 | 1.1240e-03 | 1.1446e-03 | 2.0483e-05 | 0.018 |  |  | 69632 | 100818944 | exact |
| frames-1 | selected | cold | readcon-db | unset | 8 | 12 | 2.2607e-05 | 2.3165e-05 | 6.4070e-07 | 0.028 |  |  | 40960 | 98078720 | exact |
| frames-1 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.1628e-05 | 1.1660e-05 | 4.0834e-08 | 0.004 |  |  | 40960 | 98299904 | exact |
| frames-1 | full_read | warm | ase.db | 1 | 1 | 12 | 1.5247e-04 | 1.5522e-04 | 1.7402e-06 | 0.011 |  |  | 69632 | 109154304 | exact |
| frames-1 | full_read | warm | readcon-db | unset | 8 | 12 | 1.1733e-05 | 1.1806e-05 | 7.6188e-08 | 0.006 |  |  | 40960 | 98344960 | exact |
| frames-1 | selected | warm | readcon-db | 1 | 1 | 12 | 1.1446e-05 | 1.1540e-05 | 1.0362e-07 | 0.009 |  |  | 40960 | 98058240 | exact |
| frames-1 | selected | warm | ase.db | 1 | 1 | 12 | 1.6462e-04 | 1.6899e-04 | 3.2890e-06 | 0.020 |  |  | 69632 | 108433408 | exact |
| frames-1 | selected | warm | readcon-db | unset | 8 | 12 | 1.1547e-05 | 1.1660e-05 | 9.1278e-08 | 0.008 |  |  | 40960 | 98099200 | exact |
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.6695e-03 | 4.7234e-03 | 5.9831e-05 | 0.013 | 2.1416e+04 | 1.5767e+01 | 65536 | 116334592 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.1129e-02 | 3.2410e-02 | 1.1663e-03 | 0.037 | 3.2124e+03 | 2.3651e+00 | 151552 | 100597760 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.7175e-03 | 4.8242e-03 | 6.9253e-05 | 0.015 | 2.1198e+04 | 1.5607e+01 | 65536 | 115101696 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.7615e-05 | 6.9020e-05 | 1.3933e-06 | 0.021 |  |  | 65536 | 105377792 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0572e-02 | 1.0808e-02 | 3.4285e-04 | 0.032 |  |  | 151552 | 116277248 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.8545e-05 | 6.9430e-05 | 1.1714e-06 | 0.017 |  |  | 65536 | 104947712 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.3308e-05 | 2.3704e-05 | 2.3089e-07 | 0.010 |  |  | 65536 | 102858752 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 7.9218e-04 | 1.0156e-03 | 2.2815e-04 | 0.288 |  |  | 151552 | 101806080 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.2629e-05 | 2.3146e-05 | 7.4937e-07 | 0.033 |  |  | 65536 | 102817792 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.7053e-05 | 7.6693e-05 | 1.8571e-06 | 0.028 |  |  | 65536 | 105156608 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 1.0285e-02 | 1.0485e-02 | 2.4876e-04 | 0.024 |  |  | 151552 | 116629504 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 7.1151e-05 | 7.2182e-05 | 8.0479e-07 | 0.011 |  |  | 65536 | 105115648 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.2889e-05 | 2.4610e-05 | 1.3673e-06 | 0.060 |  |  | 65536 | 102957056 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 9.1140e-04 | 1.0051e-03 | 1.1799e-04 | 0.129 |  |  | 151552 | 101646336 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.4212e-05 | 2.5895e-05 | 9.9959e-07 | 0.041 |  |  | 65536 | 102887424 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 5.1139e-04 | 6.1135e-04 | 9.5741e-05 | 0.187 |  |  | 65536 | 105295872 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 9.7368e-03 | 1.0154e-02 | 4.8955e-04 | 0.050 |  |  | 151552 | 118452224 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 5.5923e-04 | 6.8631e-04 | 9.2777e-05 | 0.166 |  |  | 65536 | 105123840 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 3.7770e-04 | 4.4558e-04 | 1.4369e-04 | 0.380 |  |  | 65536 | 102760448 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 1.2054e-03 | 1.2507e-03 | 4.5917e-05 | 0.038 |  |  | 151552 | 100737024 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.5407e-04 | 3.3645e-04 | 3.3501e-05 | 0.132 |  |  | 65536 | 102907904 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.2293e-05 | 4.3053e-05 | 3.1029e-07 | 0.007 |  |  | 65536 | 104783872 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 8.4738e-03 | 8.5206e-03 | 2.1845e-05 | 0.003 |  |  | 151552 | 119595008 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.1703e-05 | 4.1797e-05 | 1.2843e-07 | 0.003 |  |  | 65536 | 105156608 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.6234e-06 | 9.7955e-06 | 2.1957e-07 | 0.023 |  |  | 65536 | 102780928 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.6533e-04 | 1.6629e-04 | 5.0569e-07 | 0.003 |  |  | 151552 | 108597248 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.6386e-06 | 9.7742e-06 | 1.1625e-07 | 0.012 |  |  | 65536 | 102670336 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.3373e-01 | 3.3792e-01 | 3.9542e-03 | 0.012 | 2.9965e+04 | 2.2996e+01 | 3362816 | 184225792 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.9153e+00 | 2.9839e+00 | 3.4809e-02 | 0.012 | 3.4302e+03 | 2.6325e+00 | 10825728 | 200994816 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2904e-01 | 3.3154e-01 | 4.4320e-03 | 0.013 | 3.0392e+04 | 2.3324e+01 | 3362816 | 195002368 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.0543e-03 | 6.1721e-03 | 1.0278e-04 | 0.017 |  |  | 3362816 | 161017856 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.2259e-01 | 9.2863e-01 | 1.0581e-02 | 0.011 |  |  | 10825728 | 533884928 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.0269e-03 | 6.1491e-03 | 1.0973e-04 | 0.018 |  |  | 3362816 | 161046528 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.2756e-05 | 4.4277e-05 | 9.0586e-07 | 0.021 |  |  | 3362816 | 117534720 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 6.2424e-03 | 8.9509e-03 | 4.0119e-03 | 0.643 |  |  | 10825728 | 99340288 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.3181e-05 | 4.3761e-05 | 4.3031e-07 | 0.010 |  |  | 3362816 | 117334016 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.0159e-03 | 6.8315e-03 | 1.7970e-04 | 0.030 |  |  | 3362816 | 179859456 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 8.7775e-01 | 8.8753e-01 | 1.4726e-02 | 0.017 |  |  | 10825728 | 552116224 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.0865e-03 | 6.5513e-03 | 1.7246e-04 | 0.028 |  |  | 3362816 | 179671040 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.3627e-06 | 9.4371e-06 | 3.5882e-08 | 0.004 |  |  | 3362816 | 113111040 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.1618e-04 | 7.2134e-04 | 4.7220e-06 | 0.007 |  |  | 10825728 | 101044224 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 9.5034e-06 | 9.5811e-06 | 8.1310e-08 | 0.009 |  |  | 3362816 | 113246208 | exact |

