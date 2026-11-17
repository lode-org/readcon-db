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
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.7335e-03 | 4.7817e-03 | 8.2587e-05 | 0.017 | 2.1126e+04 | 1.5554e+01 | 65536 | 113987584 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.1175e-02 | 3.1703e-02 | 7.2988e-04 | 0.023 | 3.2077e+03 | 2.3617e+00 | 151552 | 100671488 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.7850e-03 | 4.8544e-03 | 1.8007e-04 | 0.038 | 2.0899e+04 | 1.5387e+01 | 65536 | 114147328 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.9050e-05 | 7.0469e-05 | 3.3268e-06 | 0.048 |  |  | 65536 | 105263104 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 9.8076e-03 | 9.8580e-03 | 2.3907e-04 | 0.024 |  |  | 151552 | 119672832 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.7449e-05 | 6.9789e-05 | 9.4383e-07 | 0.014 |  |  | 65536 | 105234432 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.3784e-05 | 2.4232e-05 | 4.8311e-07 | 0.020 |  |  | 65536 | 102862848 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 1.3715e-03 | 1.4212e-03 | 7.7600e-05 | 0.057 |  |  | 151552 | 100577280 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.2456e-05 | 2.2802e-05 | 4.3070e-07 | 0.019 |  |  | 65536 | 103026688 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.9487e-05 | 7.0214e-05 | 2.7793e-06 | 0.040 |  |  | 65536 | 105222144 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 9.4404e-03 | 9.6115e-03 | 1.1059e-04 | 0.012 |  |  | 151552 | 119746560 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 6.8241e-05 | 6.9956e-05 | 1.4892e-06 | 0.022 |  |  | 65536 | 105238528 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.3832e-05 | 2.4154e-05 | 3.7274e-07 | 0.016 |  |  | 65536 | 102883328 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 8.7781e-04 | 1.3777e-03 | 2.1712e-04 | 0.247 |  |  | 151552 | 101625856 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.3040e-05 | 2.3801e-05 | 5.2527e-07 | 0.023 |  |  | 65536 | 102727680 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 5.1781e-04 | 5.9578e-04 | 4.6214e-05 | 0.089 |  |  | 65536 | 105177088 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 9.6289e-03 | 9.7148e-03 | 8.9110e-05 | 0.009 |  |  | 151552 | 119595008 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 5.2253e-04 | 6.5059e-04 | 1.1961e-04 | 0.229 |  |  | 65536 | 105160704 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 4.3221e-04 | 4.7655e-04 | 8.9660e-05 | 0.207 |  |  | 65536 | 102850560 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 1.4801e-03 | 1.6891e-03 | 3.0222e-04 | 0.204 |  |  | 151552 | 100343808 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.9725e-04 | 5.0402e-04 | 1.3421e-04 | 0.452 |  |  | 65536 | 102584320 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.2410e-05 | 4.2706e-05 | 2.2857e-07 | 0.005 |  |  | 65536 | 104951808 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 8.4588e-03 | 8.5029e-03 | 4.0481e-05 | 0.005 |  |  | 151552 | 120131584 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.1520e-05 | 4.1576e-05 | 7.2653e-08 | 0.002 |  |  | 65536 | 105033728 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.3591e-06 | 9.5305e-06 | 5.5325e-08 | 0.006 |  |  | 65536 | 102690816 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.6284e-04 | 1.6456e-04 | 1.3417e-06 | 0.008 |  |  | 151552 | 108883968 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.1473e-06 | 9.1887e-06 | 4.2062e-08 | 0.005 |  |  | 65536 | 102621184 | exact |

