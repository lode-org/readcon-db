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
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.6526e-03 | 4.8431e-03 | 1.6100e-04 | 0.035 | 2.1493e+04 | 1.5824e+01 | 65536 | 116056064 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.1475e-02 | 3.1821e-02 | 5.4763e-04 | 0.017 | 3.1772e+03 | 2.3392e+00 | 151552 | 100659200 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.6795e-03 | 4.7431e-03 | 9.7437e-05 | 0.021 | 2.1370e+04 | 1.5733e+01 | 65536 | 114982912 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 7.3516e-05 | 8.6420e-05 | 1.1480e-05 | 0.156 |  |  | 65536 | 105230336 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0209e-02 | 1.0304e-02 | 2.3075e-04 | 0.023 |  |  | 151552 | 117379072 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.7657e-05 | 6.9807e-05 | 1.2095e-06 | 0.018 |  |  | 65536 | 105062400 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.4512e-05 | 2.5410e-05 | 2.2972e-06 | 0.094 |  |  | 65536 | 102789120 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 1.0454e-03 | 1.3795e-03 | 4.4180e-04 | 0.423 |  |  | 151552 | 101425152 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.2800e-05 | 2.3294e-05 | 6.1400e-07 | 0.027 |  |  | 65536 | 102985728 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.6570e-05 | 6.7463e-05 | 9.4754e-07 | 0.014 |  |  | 65536 | 105226240 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 9.6573e-03 | 9.8069e-03 | 1.9750e-04 | 0.020 |  |  | 151552 | 119345152 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 6.8007e-05 | 6.8869e-05 | 7.9976e-07 | 0.012 |  |  | 65536 | 105439232 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.3886e-05 | 2.5637e-05 | 1.1873e-06 | 0.050 |  |  | 65536 | 102977536 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 1.2870e-03 | 1.3354e-03 | 6.7695e-05 | 0.053 |  |  | 151552 | 100405248 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.3338e-05 | 2.3837e-05 | 5.0591e-07 | 0.022 |  |  | 65536 | 102871040 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 3.9412e-04 | 4.7235e-04 | 7.5338e-05 | 0.191 |  |  | 65536 | 105299968 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.0193e-02 | 1.0368e-02 | 1.8360e-04 | 0.018 |  |  | 151552 | 116563968 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 4.6554e-04 | 4.8545e-04 | 3.5153e-05 | 0.076 |  |  | 65536 | 105287680 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 2.7315e-04 | 3.2192e-04 | 4.7826e-05 | 0.175 |  |  | 65536 | 102580224 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 9.8767e-04 | 1.2857e-03 | 1.9152e-04 | 0.194 |  |  | 151552 | 101281792 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.9316e-04 | 4.4003e-04 | 1.6657e-04 | 0.568 |  |  | 65536 | 102678528 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.2143e-05 | 4.2413e-05 | 2.2747e-07 | 0.005 |  |  | 65536 | 104902656 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 8.4418e-03 | 8.5446e-03 | 6.3023e-05 | 0.007 |  |  | 151552 | 119623680 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.2835e-05 | 4.3351e-05 | 2.1006e-07 | 0.005 |  |  | 65536 | 104914944 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.0886e-06 | 9.1562e-06 | 2.7373e-08 | 0.003 |  |  | 65536 | 102617088 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.6877e-04 | 1.7158e-04 | 1.2382e-06 | 0.007 |  |  | 151552 | 108273664 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 8.9780e-06 | 9.0551e-06 | 6.2371e-08 | 0.007 |  |  | 65536 | 102645760 | exact |

