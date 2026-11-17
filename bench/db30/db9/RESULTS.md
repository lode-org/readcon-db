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
| atoms-1000000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.9589e-01 | 3.0292e-01 | 8.2231e-03 | 0.028 | 3.3796e+00 | 1.8279e+02 | 52592640 | 733454336 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.5413e-01 | 2.5755e-01 | 7.1786e-03 | 0.028 | 3.9350e+00 | 2.1283e+02 | 60125184 | 1135702016 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.9649e-01 | 3.0391e-01 | 5.8060e-03 | 0.020 | 3.3728e+00 | 1.8242e+02 | 52592640 | 733425664 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 1 | 1 | 12 | 5.1620e-02 | 5.6220e-02 | 6.0665e-03 | 0.118 |  |  | 52592640 | 427560960 | exact |
| atoms-1000000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.2911e-01 | 3.4191e-01 | 1.1768e-02 | 0.036 |  |  | 60125184 | 1321521152 | exact |
| atoms-1000000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.1535e-02 | 5.3530e-02 | 3.7535e-03 | 0.073 |  |  | 52592640 | 427196416 | exact |
| atoms-1000000 | selected | cold | readcon-db | 1 | 1 | 12 | 6.1957e-02 | 7.0694e-02 | 8.1721e-03 | 0.132 |  |  | 52592640 | 427491328 | exact |
| atoms-1000000 | selected | cold | ase.db | 1 | 1 | 12 | 3.3572e-01 | 3.5288e-01 | 1.5868e-02 | 0.047 |  |  | 60125184 | 1320468480 | exact |
| atoms-1000000 | selected | cold | readcon-db | unset | 8 | 12 | 5.4440e-02 | 5.9629e-02 | 7.7842e-03 | 0.143 |  |  | 52592640 | 427503616 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.3009e-02 | 3.4283e-02 | 2.9720e-03 | 0.129 |  |  | 52592640 | 364470272 | exact |
| atoms-1000000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.0058e-01 | 3.0496e-01 | 3.6855e-03 | 0.012 |  |  | 60125184 | 1330659328 | exact |
| atoms-1000000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.4636e-02 | 2.8951e-02 | 4.2694e-03 | 0.173 |  |  | 52592640 | 364609536 | exact |
| atoms-1000000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.1804e-02 | 2.3149e-02 | 1.8376e-03 | 0.084 |  |  | 52592640 | 364498944 | exact |
| atoms-1000000 | selected | warm | ase.db | 1 | 1 | 12 | 2.9638e-01 | 2.9968e-01 | 6.5699e-03 | 0.022 |  |  | 60125184 | 1330827264 | exact |
| atoms-1000000 | selected | warm | readcon-db | unset | 8 | 12 | 2.2537e-02 | 2.3598e-02 | 1.2169e-03 | 0.054 |  |  | 52592640 | 364511232 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.2263e-01 | 3.2568e-01 | 2.7701e-03 | 0.009 | 3.0996e+04 | 2.3787e+01 | 3362816 | 200286208 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.9792e+00 | 3.0613e+00 | 4.1484e-02 | 0.014 | 3.3566e+03 | 2.5760e+00 | 10817536 | 200933376 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2805e-01 | 3.3039e-01 | 3.3351e-03 | 0.010 | 3.0483e+04 | 2.3394e+01 | 3362816 | 188497920 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.1628e-03 | 6.4384e-03 | 1.9022e-04 | 0.031 |  |  | 3362816 | 160980992 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.6294e-01 | 1.0008e+00 | 3.5100e-02 | 0.036 |  |  | 10833920 | 533893120 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.1890e-03 | 6.4026e-03 | 1.1956e-04 | 0.019 |  |  | 3362816 | 160874496 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.5317e-05 | 4.6356e-05 | 1.2389e-06 | 0.027 |  |  | 3362816 | 117374976 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 3.7896e-03 | 4.2633e-03 | 9.8712e-04 | 0.260 |  |  | 10833920 | 99627008 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.5404e-05 | 4.6019e-05 | 1.2990e-06 | 0.029 |  |  | 3362816 | 117248000 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 7.1197e-03 | 9.0393e-03 | 2.0661e-03 | 0.290 |  |  | 3362816 | 179724288 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 1.0025e+00 | 1.0758e+00 | 8.6129e-02 | 0.086 |  |  | 10833920 | 552374272 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.2110e-03 | 6.8864e-03 | 2.4054e-04 | 0.039 |  |  | 3362816 | 179503104 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.6459e-06 | 9.7622e-06 | 1.7915e-07 | 0.019 |  |  | 3362816 | 113094656 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.3160e-04 | 7.3694e-04 | 2.9670e-06 | 0.004 |  |  | 10833920 | 101031936 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 9.3702e-06 | 9.5076e-06 | 7.6449e-08 | 0.008 |  |  | 3362816 | 113491968 | exact |
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.5659e-03 | 4.6025e-03 | 5.0355e-05 | 0.011 | 2.1901e+04 | 1.6125e+01 | 65536 | 113999872 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.1927e-02 | 3.2199e-02 | 3.5062e-04 | 0.011 | 3.1322e+03 | 2.3061e+00 | 151552 | 100745216 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.5372e-03 | 4.5934e-03 | 7.1638e-05 | 0.016 | 2.2040e+04 | 1.6227e+01 | 65536 | 116019200 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.6992e-05 | 6.7654e-05 | 2.3108e-06 | 0.034 |  |  | 65536 | 105119744 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0227e-02 | 1.0515e-02 | 3.2091e-04 | 0.031 |  |  | 151552 | 116998144 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.4668e-05 | 6.6313e-05 | 1.7497e-06 | 0.027 |  |  | 65536 | 105123840 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.1174e-05 | 2.2964e-05 | 7.8725e-07 | 0.037 |  |  | 65536 | 102952960 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 7.7051e-04 | 8.5223e-04 | 1.6070e-04 | 0.209 |  |  | 151552 | 102232064 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.0771e-05 | 2.1483e-05 | 8.8603e-07 | 0.043 |  |  | 65536 | 103018496 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.5867e-05 | 6.9511e-05 | 2.4888e-06 | 0.038 |  |  | 65536 | 105164800 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 9.9711e-03 | 1.0265e-02 | 4.1534e-04 | 0.042 |  |  | 151552 | 118001664 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 6.6213e-05 | 6.7407e-05 | 1.5976e-06 | 0.024 |  |  | 65536 | 105242624 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.0542e-05 | 2.1783e-05 | 7.2218e-07 | 0.035 |  |  | 65536 | 102834176 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 1.0679e-03 | 1.1623e-03 | 1.9827e-04 | 0.186 |  |  | 151552 | 101195776 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 1.9873e-05 | 2.0696e-05 | 6.6633e-07 | 0.034 |  |  | 65536 | 102871040 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 4.3607e-04 | 5.3771e-04 | 1.4189e-04 | 0.325 |  |  | 65536 | 105361408 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.0158e-02 | 1.0496e-02 | 5.1429e-04 | 0.051 |  |  | 151552 | 117735424 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 4.2913e-04 | 4.5850e-04 | 2.8663e-05 | 0.067 |  |  | 65536 | 105271296 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 2.4616e-04 | 2.9273e-04 | 6.0640e-05 | 0.246 |  |  | 65536 | 102928384 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 1.0832e-03 | 1.2879e-03 | 1.4202e-04 | 0.131 |  |  | 151552 | 100917248 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.5886e-04 | 2.8941e-04 | 4.7889e-05 | 0.185 |  |  | 65536 | 102522880 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.4645e-05 | 4.5195e-05 | 2.5301e-07 | 0.006 |  |  | 65536 | 104800256 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 9.1717e-03 | 9.2179e-03 | 9.0158e-05 | 0.010 |  |  | 151552 | 119767040 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.3616e-05 | 4.3958e-05 | 3.6364e-07 | 0.008 |  |  | 65536 | 104853504 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.2117e-06 | 9.3402e-06 | 9.5909e-08 | 0.010 |  |  | 65536 | 102526976 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.7474e-04 | 1.7635e-04 | 1.8043e-06 | 0.010 |  |  | 151552 | 107986944 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.3051e-06 | 9.5015e-06 | 1.5584e-07 | 0.017 |  |  | 65536 | 102797312 | exact |

