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
the type-7 90th percentile of those means.

| workload | op | cache | tool | n | median s | p90 s | IQR s | IQR/median | disk B | peak RSS B | fidelity |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| frames-100 | insert | cold-file | readcon-db | 12 | 4.4446e-03 | 4.5436e-03 | 1.1241e-04 | 0.025 | 180224 | 103493632 | exact |
| frames-100 | insert | cold-file | ase.db | 12 | 2.6790e-02 | 2.7060e-02 | 1.8322e-04 | 0.007 | 151552 | 95203328 | exact |
| frames-100 | full_read | cold | readcon-db | 12 | 2.5435e-04 | 3.3847e-04 | 1.1127e-04 | 0.437 | 180224 | 95318016 | exact |
| frames-100 | full_read | cold | ase.db | 12 | 8.9829e-03 | 9.0091e-03 | 1.1228e-04 | 0.012 | 151552 | 94285824 | exact |
| frames-100 | selected | cold | readcon-db | 12 | 1.7629e-04 | 2.2812e-04 | 7.3539e-05 | 0.417 | 180224 | 93057024 | exact |
| frames-100 | selected | cold | ase.db | 12 | 1.0131e-03 | 1.0840e-03 | 1.2113e-04 | 0.120 | 151552 | 93495296 | exact |
| frames-100 | full_read | cold-open | readcon-db | 12 | 8.9940e-04 | 9.0901e-04 | 1.0906e-04 | 0.121 | 180224 | 95277056 | exact |
| frames-100 | full_read | cold-open | ase.db | 12 | 9.1270e-03 | 9.2206e-03 | 1.7176e-04 | 0.019 | 151552 | 94285824 | exact |
| frames-100 | selected | cold-open | readcon-db | 12 | 6.2041e-04 | 7.0378e-04 | 9.6021e-05 | 0.155 | 180224 | 93102080 | exact |
| frames-100 | selected | cold-open | ase.db | 12 | 1.0573e-03 | 1.1906e-03 | 9.8541e-05 | 0.093 | 151552 | 93319168 | exact |
| frames-100 | full_read | warm | readcon-db | 12 | 5.5161e-05 | 5.5250e-05 | 7.5649e-08 | 0.001 | 180224 | 95428608 | exact |
| frames-100 | full_read | warm | ase.db | 12 | 7.6124e-03 | 7.6319e-03 | 1.0670e-05 | 0.001 | 151552 | 94175232 | exact |
| frames-100 | selected | warm | readcon-db | 12 | 1.2990e-05 | 1.3016e-05 | 2.5783e-08 | 0.002 | 180224 | 93249536 | exact |
| frames-100 | selected | warm | ase.db | 12 | 1.6080e-04 | 1.6208e-04 | 7.4307e-07 | 0.005 | 151552 | 93532160 | exact |

