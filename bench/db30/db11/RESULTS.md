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
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.7524e-03 | 4.8030e-03 | 6.5032e-05 | 0.014 | 2.1042e+04 | 1.5492e+01 | 65536 | 113876992 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.1784e-02 | 3.2116e-02 | 4.0485e-04 | 0.013 | 3.1462e+03 | 2.3164e+00 | 151552 | 100585472 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.7540e-03 | 4.9000e-03 | 1.1420e-04 | 0.024 | 2.1035e+04 | 1.5487e+01 | 65536 | 112226304 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.9438e-05 | 7.3428e-05 | 2.4866e-06 | 0.036 |  |  | 65536 | 105189376 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0139e-02 | 1.0196e-02 | 1.7508e-04 | 0.017 |  |  | 151552 | 117170176 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.8177e-05 | 7.7764e-05 | 2.1675e-06 | 0.032 |  |  | 65536 | 105136128 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.3890e-05 | 2.5921e-05 | 1.7581e-06 | 0.074 |  |  | 65536 | 102858752 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 1.3026e-03 | 1.3411e-03 | 1.0331e-04 | 0.079 |  |  | 151552 | 100732928 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.4987e-05 | 2.6212e-05 | 9.2914e-07 | 0.037 |  |  | 65536 | 102735872 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.6387e-05 | 6.7812e-05 | 1.2900e-06 | 0.019 |  |  | 65536 | 105201664 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 9.4046e-03 | 9.6440e-03 | 2.4516e-04 | 0.026 |  |  | 151552 | 119681024 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 6.7246e-05 | 7.1666e-05 | 1.4697e-06 | 0.022 |  |  | 65536 | 105267200 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.2512e-05 | 2.3133e-05 | 6.1608e-07 | 0.027 |  |  | 65536 | 102952960 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 9.5090e-04 | 1.0160e-03 | 1.0006e-04 | 0.105 |  |  | 151552 | 101511168 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.3493e-05 | 2.4156e-05 | 8.7957e-07 | 0.037 |  |  | 65536 | 103002112 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 4.5929e-04 | 5.6399e-04 | 9.6740e-05 | 0.211 |  |  | 65536 | 105381888 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.0468e-02 | 1.0627e-02 | 1.6699e-04 | 0.016 |  |  | 151552 | 116277248 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 6.9567e-04 | 7.7650e-04 | 1.5218e-04 | 0.219 |  |  | 65536 | 105201664 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 4.3806e-04 | 4.9030e-04 | 6.5675e-05 | 0.150 |  |  | 65536 | 102744064 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 1.2932e-03 | 1.4773e-03 | 3.3711e-04 | 0.261 |  |  | 151552 | 100737024 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 3.4613e-04 | 4.2739e-04 | 1.1121e-04 | 0.321 |  |  | 65536 | 102764544 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.2310e-05 | 4.2498e-05 | 1.7248e-07 | 0.004 |  |  | 65536 | 104906752 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 8.7149e-03 | 8.8861e-03 | 5.0208e-04 | 0.058 |  |  | 151552 | 120127488 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.1909e-05 | 4.2075e-05 | 1.3172e-07 | 0.003 |  |  | 65536 | 105025536 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.5674e-06 | 9.6022e-06 | 6.2229e-08 | 0.007 |  |  | 65536 | 102686720 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.6534e-04 | 1.6700e-04 | 5.8724e-07 | 0.004 |  |  | 151552 | 108548096 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.3438e-06 | 9.4132e-06 | 1.2025e-07 | 0.013 |  |  | 65536 | 102592512 | exact |

