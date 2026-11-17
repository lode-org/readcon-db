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
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.7744e-03 | 4.8533e-03 | 9.2793e-05 | 0.019 | 2.0945e+04 | 1.5421e+01 | 65536 | 114040832 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.1856e-02 | 3.2602e-02 | 3.8037e-04 | 0.012 | 3.1391e+03 | 2.3112e+00 | 151552 | 100700160 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.8269e-03 | 4.8845e-03 | 9.6875e-05 | 0.020 | 2.0717e+04 | 1.5253e+01 | 65536 | 116547584 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.6510e-05 | 6.7462e-05 | 1.3432e-06 | 0.020 |  |  | 65536 | 105369600 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 9.9864e-03 | 1.0239e-02 | 3.7087e-04 | 0.037 |  |  | 151552 | 118153216 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.8089e-05 | 7.2675e-05 | 3.4537e-06 | 0.051 |  |  | 65536 | 105078784 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.3208e-05 | 2.5272e-05 | 1.4540e-06 | 0.063 |  |  | 65536 | 102912000 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 1.4017e-03 | 1.5084e-03 | 9.7950e-05 | 0.070 |  |  | 151552 | 100462592 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.4221e-05 | 2.5229e-05 | 9.2850e-07 | 0.038 |  |  | 65536 | 102936576 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 7.0434e-05 | 7.1413e-05 | 2.0351e-06 | 0.029 |  |  | 65536 | 105123840 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 9.5053e-03 | 9.7917e-03 | 4.8229e-04 | 0.051 |  |  | 151552 | 119586816 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 6.8126e-05 | 7.4405e-05 | 2.0416e-06 | 0.030 |  |  | 65536 | 105066496 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.3486e-05 | 2.3820e-05 | 3.9528e-07 | 0.017 |  |  | 65536 | 102944768 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 1.2262e-03 | 1.3115e-03 | 1.8296e-04 | 0.149 |  |  | 151552 | 100732928 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.3262e-05 | 2.3488e-05 | 2.3128e-07 | 0.010 |  |  | 65536 | 102887424 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 5.7298e-04 | 7.2962e-04 | 1.0245e-04 | 0.179 |  |  | 65536 | 105316352 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.0494e-02 | 1.0622e-02 | 2.0756e-04 | 0.020 |  |  | 151552 | 116547584 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 4.4600e-04 | 4.9456e-04 | 6.2478e-05 | 0.140 |  |  | 65536 | 105275392 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 4.1597e-04 | 4.4084e-04 | 2.9075e-05 | 0.070 |  |  | 65536 | 102727680 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 1.6083e-03 | 1.7636e-03 | 2.2909e-04 | 0.142 |  |  | 151552 | 100216832 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 3.4569e-04 | 4.7984e-04 | 1.7119e-04 | 0.495 |  |  | 65536 | 102739968 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.1949e-05 | 4.2095e-05 | 1.2329e-07 | 0.003 |  |  | 65536 | 104947712 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 8.8504e-03 | 8.8720e-03 | 2.4774e-05 | 0.003 |  |  | 151552 | 119545856 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.2066e-05 | 4.2300e-05 | 1.3001e-07 | 0.003 |  |  | 65536 | 104677376 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.1300e-06 | 9.2484e-06 | 1.2305e-07 | 0.013 |  |  | 65536 | 102686720 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.6176e-04 | 1.6329e-04 | 9.2435e-07 | 0.006 |  |  | 151552 | 108687360 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.2958e-06 | 9.3863e-06 | 8.9324e-08 | 0.010 |  |  | 65536 | 102686720 | exact |

