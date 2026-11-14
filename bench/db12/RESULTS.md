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
| frames-100 | insert | cold-file | readcon-db | 12 | 4.2923e-03 | 4.3750e-03 | 7.6736e-05 | 0.018 | 180224 | 103731200 | exact |
| frames-100 | insert | cold-file | ase.db | 12 | 2.6885e-02 | 2.7035e-02 | 1.7047e-04 | 0.006 | 151552 | 95391744 | exact |
| frames-100 | full_read | cold | readcon-db | 12 | 2.2105e-04 | 2.5050e-04 | 3.7349e-05 | 0.169 | 180224 | 95555584 | exact |
| frames-100 | full_read | cold | ase.db | 12 | 8.7407e-03 | 8.7861e-03 | 7.7671e-05 | 0.009 | 151552 | 94269440 | exact |
| frames-100 | selected | cold | readcon-db | 12 | 2.3257e-04 | 2.5730e-04 | 1.0722e-04 | 0.461 | 180224 | 93310976 | exact |
| frames-100 | selected | cold | ase.db | 12 | 6.5105e-04 | 7.0639e-04 | 3.4018e-05 | 0.052 | 151552 | 93540352 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 12 | 1.2536e-04 | 1.3401e-04 | 1.1855e-05 | 0.095 | 180224 | 95965184 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 12 | 7.8542e-03 | 7.8703e-03 | 1.6019e-05 | 0.002 | 151552 | 94257152 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 12 | 5.0295e-05 | 5.1437e-05 | 1.3945e-06 | 0.028 | 180224 | 93741056 | exact |
| frames-100 | selected | warm-after-open | ase.db | 12 | 4.2633e-04 | 4.3012e-04 | 4.3412e-06 | 0.010 | 151552 | 93687808 | exact |
| frames-100 | full_read | cold-open | readcon-db | 12 | 1.3802e-03 | 1.4710e-03 | 1.8563e-04 | 0.134 | 180224 | 95797248 | exact |
| frames-100 | full_read | cold-open | ase.db | 12 | 8.8835e-03 | 8.9355e-03 | 5.6970e-05 | 0.006 | 151552 | 94486528 | exact |
| frames-100 | selected | cold-open | readcon-db | 12 | 1.3028e-03 | 1.3379e-03 | 9.5899e-05 | 0.074 | 180224 | 93655040 | exact |
| frames-100 | selected | cold-open | ase.db | 12 | 1.1600e-03 | 1.2827e-03 | 1.0458e-04 | 0.090 | 151552 | 93577216 | exact |
| frames-100 | full_read | warm | readcon-db | 12 | 5.5122e-05 | 5.5212e-05 | 6.1186e-08 | 0.001 | 180224 | 95870976 | exact |
| frames-100 | full_read | warm | ase.db | 12 | 7.6341e-03 | 7.6541e-03 | 1.6716e-05 | 0.002 | 151552 | 94298112 | exact |
| frames-100 | selected | warm | readcon-db | 12 | 1.3048e-05 | 1.3093e-05 | 9.0189e-08 | 0.007 | 180224 | 93827072 | exact |
| frames-100 | selected | warm | ase.db | 12 | 1.6229e-04 | 1.6331e-04 | 5.0225e-07 | 0.003 | 151552 | 93802496 | exact |

