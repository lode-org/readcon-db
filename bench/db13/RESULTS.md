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
| frames-100 | insert | cold-file | readcon-db | 12 | 4.4765e-03 | 4.6557e-03 | 2.3272e-04 | 0.052 | 180224 | 102813696 | exact |
| frames-100 | insert | cold-file | ase.db | 12 | 2.7281e-02 | 2.7675e-02 | 5.3578e-04 | 0.020 | 151552 | 95404032 | exact |
| frames-100 | full_read | cold | readcon-db | 12 | 1.1165e-04 | 1.1258e-04 | 2.6333e-06 | 0.024 | 180224 | 97525760 | exact |
| frames-100 | full_read | cold | ase.db | 12 | 8.4960e-03 | 8.5333e-03 | 7.6843e-05 | 0.009 | 151552 | 94461952 | exact |
| frames-100 | selected | cold | readcon-db | 12 | 4.7868e-05 | 4.8489e-05 | 3.4894e-07 | 0.007 | 180224 | 95166464 | exact |
| frames-100 | selected | cold | ase.db | 12 | 1.0399e-03 | 1.0924e-03 | 8.8348e-05 | 0.085 | 151552 | 93466624 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 12 | 1.3494e-04 | 1.4235e-04 | 1.0692e-05 | 0.079 | 180224 | 95440896 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 12 | 7.9025e-03 | 7.9138e-03 | 2.6679e-05 | 0.003 | 151552 | 94363648 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 12 | 4.5419e-05 | 4.6880e-05 | 8.8033e-07 | 0.019 | 180224 | 93130752 | exact |
| frames-100 | selected | warm-after-open | ase.db | 12 | 4.3109e-04 | 4.3271e-04 | 1.1072e-06 | 0.003 | 151552 | 93466624 | exact |
| frames-100 | full_read | cold-open | readcon-db | 12 | 5.9477e-04 | 7.6221e-04 | 1.9137e-04 | 0.322 | 180224 | 95760384 | exact |
| frames-100 | full_read | cold-open | ase.db | 12 | 8.8615e-03 | 8.9039e-03 | 7.2054e-05 | 0.008 | 151552 | 94441472 | exact |
| frames-100 | selected | cold-open | readcon-db | 12 | 8.6600e-04 | 1.0215e-03 | 1.1898e-04 | 0.137 | 180224 | 93216768 | exact |
| frames-100 | selected | cold-open | ase.db | 12 | 1.1522e-03 | 1.2523e-03 | 1.2566e-04 | 0.109 | 151552 | 93540352 | exact |
| frames-100 | full_read | warm | readcon-db | 12 | 5.5019e-05 | 5.5153e-05 | 1.3519e-07 | 0.002 | 180224 | 95555584 | exact |
| frames-100 | full_read | warm | ase.db | 12 | 7.6480e-03 | 7.6636e-03 | 1.7338e-05 | 0.002 | 151552 | 94285824 | exact |
| frames-100 | selected | warm | readcon-db | 12 | 1.3029e-05 | 1.3140e-05 | 1.6906e-07 | 0.013 | 180224 | 93474816 | exact |
| frames-100 | selected | warm | ase.db | 12 | 1.6359e-04 | 1.6401e-04 | 7.4841e-07 | 0.005 | 151552 | 93655040 | exact |

