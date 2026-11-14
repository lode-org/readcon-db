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
| frames-100 | insert | cold-file | readcon-db | 12 | 4.5333e-03 | 4.6711e-03 | 2.4065e-04 | 0.053 | 180224 | 105570304 | exact |
| frames-100 | insert | cold-file | ase.db | 12 | 2.7056e-02 | 2.7308e-02 | 2.4607e-04 | 0.009 | 151552 | 95391744 | exact |
| frames-100 | full_read | cold | readcon-db | 12 | 8.2491e-05 | 8.3802e-05 | 1.8783e-06 | 0.023 | 180224 | 97153024 | exact |
| frames-100 | full_read | cold | ase.db | 12 | 9.2523e-03 | 9.3533e-03 | 1.5838e-04 | 0.017 | 151552 | 114704384 | exact |
| frames-100 | selected | cold | readcon-db | 12 | 2.6409e-05 | 2.7150e-05 | 4.0633e-07 | 0.015 | 180224 | 95580160 | exact |
| frames-100 | selected | cold | ase.db | 12 | 5.2777e-04 | 5.6945e-04 | 2.6850e-05 | 0.051 | 151552 | 98623488 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 12 | 8.3189e-05 | 8.5061e-05 | 1.6465e-06 | 0.020 | 180224 | 97574912 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 12 | 9.1804e-03 | 9.4053e-03 | 2.9622e-04 | 0.032 | 151552 | 114561024 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 12 | 2.6779e-05 | 2.7382e-05 | 6.9282e-07 | 0.026 | 180224 | 95567872 | exact |
| frames-100 | selected | warm-after-open | ase.db | 12 | 8.8366e-04 | 9.7194e-04 | 1.0773e-04 | 0.122 | 151552 | 96800768 | exact |
| frames-100 | full_read | cold-open | readcon-db | 12 | 3.6734e-04 | 3.9414e-04 | 2.2588e-05 | 0.061 | 180224 | 97529856 | exact |
| frames-100 | full_read | cold-open | ase.db | 12 | 9.7446e-03 | 9.8334e-03 | 1.4307e-04 | 0.015 | 151552 | 114343936 | exact |
| frames-100 | selected | cold-open | readcon-db | 12 | 2.6662e-04 | 2.8148e-04 | 1.7235e-05 | 0.065 | 180224 | 95383552 | exact |
| frames-100 | selected | cold-open | ase.db | 12 | 1.1149e-03 | 1.1837e-03 | 7.3327e-05 | 0.066 | 151552 | 95887360 | exact |
| frames-100 | full_read | warm | readcon-db | 12 | 5.4558e-05 | 5.4674e-05 | 6.1025e-08 | 0.001 | 180224 | 97308672 | exact |
| frames-100 | full_read | warm | ase.db | 12 | 8.3201e-03 | 8.3663e-03 | 1.0504e-04 | 0.013 | 151552 | 117071872 | exact |
| frames-100 | selected | warm | readcon-db | 12 | 1.3182e-05 | 1.3240e-05 | 6.2173e-08 | 0.005 | 180224 | 95363072 | exact |
| frames-100 | selected | warm | ase.db | 12 | 1.6188e-04 | 1.6217e-04 | 9.3030e-07 | 0.006 | 151552 | 103710720 | exact |

