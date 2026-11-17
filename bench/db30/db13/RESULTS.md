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
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.7105e-03 | 4.7619e-03 | 7.1940e-05 | 0.015 | 2.1229e+04 | 1.5630e+01 | 65536 | 113913856 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.1569e-02 | 3.1829e-02 | 2.7489e-04 | 0.009 | 3.1677e+03 | 2.3322e+00 | 151552 | 100552704 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.7767e-03 | 4.8674e-03 | 5.3760e-05 | 0.011 | 2.0935e+04 | 1.5413e+01 | 65536 | 115970048 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 7.2300e-05 | 7.2938e-05 | 1.3959e-06 | 0.019 |  |  | 65536 | 105160704 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0026e-02 | 1.0110e-02 | 1.5089e-04 | 0.015 |  |  | 151552 | 117731328 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 7.0319e-05 | 7.3366e-05 | 2.9038e-06 | 0.041 |  |  | 65536 | 105193472 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.5687e-05 | 2.6741e-05 | 1.9489e-06 | 0.076 |  |  | 65536 | 102854656 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 1.0973e-03 | 1.2626e-03 | 2.1377e-04 | 0.195 |  |  | 151552 | 101158912 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.5926e-05 | 2.6681e-05 | 7.9463e-07 | 0.031 |  |  | 65536 | 102969344 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 7.2680e-05 | 7.4189e-05 | 1.3250e-06 | 0.018 |  |  | 65536 | 105545728 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 9.9265e-03 | 1.0032e-02 | 1.4698e-04 | 0.015 |  |  | 151552 | 118743040 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 7.2929e-05 | 7.5046e-05 | 9.3972e-07 | 0.013 |  |  | 65536 | 105201664 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.5600e-05 | 2.6563e-05 | 1.2691e-06 | 0.050 |  |  | 65536 | 102944768 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 1.1028e-03 | 1.4586e-03 | 2.7260e-04 | 0.247 |  |  | 151552 | 101203968 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.4378e-05 | 2.5392e-05 | 5.6241e-07 | 0.023 |  |  | 65536 | 102760448 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 4.8022e-04 | 5.5887e-04 | 8.9263e-05 | 0.186 |  |  | 65536 | 105132032 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.0349e-02 | 1.0444e-02 | 3.7923e-04 | 0.037 |  |  | 151552 | 117452800 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 5.3797e-04 | 7.1834e-04 | 1.6720e-04 | 0.311 |  |  | 65536 | 105164800 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 2.7139e-04 | 3.0587e-04 | 5.8883e-05 | 0.217 |  |  | 65536 | 102735872 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 1.7537e-03 | 1.8626e-03 | 1.5770e-04 | 0.090 |  |  | 151552 | 100069376 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 3.6578e-04 | 4.7119e-04 | 1.6490e-04 | 0.451 |  |  | 65536 | 102707200 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.1345e-05 | 4.1551e-05 | 1.2894e-07 | 0.003 |  |  | 65536 | 105046016 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 9.2234e-03 | 9.4138e-03 | 3.7898e-04 | 0.041 |  |  | 151552 | 119648256 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.2631e-05 | 5.2087e-05 | 9.8344e-07 | 0.023 |  |  | 65536 | 104972288 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.7454e-06 | 9.8021e-06 | 1.3489e-07 | 0.014 |  |  | 65536 | 102486016 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.6350e-04 | 1.6429e-04 | 5.2588e-07 | 0.003 |  |  | 151552 | 108662784 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.1084e-06 | 9.2546e-06 | 1.0355e-07 | 0.011 |  |  | 65536 | 102637568 | exact |

