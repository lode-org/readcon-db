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
| frames-100 | insert | cold-file | readcon-db | 12 | 4.3740e-03 | 4.5135e-03 | 1.1180e-04 | 0.026 | 180224 | 104648704 | exact |
| frames-100 | insert | cold-file | ase.db | 12 | 2.6972e-02 | 2.7163e-02 | 5.3669e-04 | 0.020 | 151552 | 95604736 | exact |
| frames-100 | full_read | cold | readcon-db | 12 | 1.3635e-04 | 1.4755e-04 | 1.0397e-05 | 0.076 | 180224 | 95453184 | exact |
| frames-100 | full_read | cold | ase.db | 12 | 8.0659e-03 | 8.0844e-03 | 2.7423e-05 | 0.003 | 151552 | 94613504 | exact |
| frames-100 | selected | cold | readcon-db | 12 | 4.7530e-05 | 4.9126e-05 | 1.1336e-06 | 0.024 | 180224 | 93343744 | exact |
| frames-100 | selected | cold | ase.db | 12 | 4.2727e-04 | 4.2904e-04 | 1.2714e-06 | 0.003 | 151552 | 93601792 | exact |
| frames-100 | full_read | cold-open | readcon-db | 12 | 8.0440e-04 | 9.9631e-04 | 1.1878e-04 | 0.148 | 180224 | 95662080 | exact |
| frames-100 | full_read | cold-open | ase.db | 12 | 9.2823e-03 | 9.3007e-03 | 5.1345e-05 | 0.006 | 151552 | 94572544 | exact |
| frames-100 | selected | cold-open | readcon-db | 12 | 7.4204e-04 | 8.2465e-04 | 9.3140e-05 | 0.126 | 180224 | 93208576 | exact |
| frames-100 | selected | cold-open | ase.db | 12 | 1.0074e-03 | 1.0475e-03 | 6.9558e-05 | 0.069 | 151552 | 93724672 | exact |
| frames-100 | full_read | warm | readcon-db | 12 | 5.4411e-05 | 5.4568e-05 | 3.1784e-07 | 0.006 | 180224 | 95682560 | exact |
| frames-100 | full_read | warm | ase.db | 12 | 7.6915e-03 | 7.7134e-03 | 1.3021e-05 | 0.002 | 151552 | 94400512 | exact |
| frames-100 | selected | warm | readcon-db | 12 | 1.3146e-05 | 1.3172e-05 | 3.0007e-08 | 0.002 | 180224 | 93376512 | exact |
| frames-100 | selected | warm | ase.db | 12 | 1.6486e-04 | 1.6610e-04 | 6.2224e-07 | 0.004 | 151552 | 93888512 | exact |

