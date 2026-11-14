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
the type-7 90th percentile of those means. A range read
materializes an inclusive frame-index span. A metadata read
returns the frame indexes whose energy is in the last tenth.
Ingest rows report frames/s and MB/s of the CON text. disk B
is the database file after that ingest.

| workload | op | cache | tool | n | median s | p90 s | IQR s | IQR/median | frames/s | MB/s | disk B | peak RSS B | fidelity |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| paper40x8 | insert | cold-file | readcon-db | 6 | 2.2947e-03 | 2.3763e-03 | 7.1278e-05 | 0.031 | 1.7431e+04 | 5.4386e+00 | 49152 | 118382592 | exact |
| paper40x8 | insert | cold-file | ase.db | 6 | 5.4583e-03 | 5.7746e-03 | 2.4571e-04 | 0.045 | 7.3283e+03 | 2.2864e+00 | 90112 | 106172416 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 12 | 1.2104e-03 | 1.4028e-03 | 1.8916e-04 | 0.156 | 8.2618e+02 | 3.5156e+01 | 98304 | 105734144 | exact |
| atoms-1000 | insert | cold-file | ase.db | 12 | 1.5255e-03 | 1.5696e-03 | 8.4119e-05 | 0.055 | 6.5554e+02 | 2.7895e+01 | 126976 | 107171840 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 6 | 3.8553e-03 | 3.9108e-03 | 8.6962e-05 | 0.023 | 2.5938e+02 | 1.1966e+02 | 589824 | 119320576 | exact |
| atoms-10000 | insert | cold-file | ase.db | 9 | 3.6801e-03 | 3.8666e-03 | 1.7045e-04 | 0.046 | 2.7173e+02 | 1.2536e+02 | 667648 | 112685056 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 6 | 2.7648e-02 | 2.8180e-02 | 7.2461e-04 | 0.026 | 3.6169e+01 | 1.8117e+02 | 5390336 | 179765248 | exact |
| atoms-100000 | insert | cold-file | ase.db | 6 | 2.7813e-02 | 2.8588e-02 | 6.1587e-04 | 0.022 | 3.5955e+01 | 1.8010e+02 | 6074368 | 208191488 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 6 | 2.9634e-01 | 3.0144e-01 | 7.7160e-03 | 0.026 | 3.3745e+00 | 1.8252e+02 | 52592640 | 564674560 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 6 | 2.5418e-01 | 2.6248e-01 | 1.0841e-02 | 0.043 | 3.9342e+00 | 2.1279e+02 | 60125184 | 1128894464 | exact |
| frames-1 | insert | cold-file | readcon-db | 12 | 8.5488e-04 | 1.0630e-03 | 9.7451e-05 | 0.114 | 1.1698e+03 | 8.4807e-01 | 40960 | 101527552 | exact |
| frames-1 | insert | cold-file | ase.db | 6 | 1.1049e-03 | 1.1177e-03 | 2.2987e-05 | 0.021 | 9.0509e+02 | 6.5619e-01 | 69632 | 102879232 | exact |
| frames-100 | insert | cold-file | readcon-db | 6 | 4.4986e-03 | 4.5462e-03 | 7.3436e-05 | 0.016 | 2.2229e+04 | 1.6366e+01 | 65536 | 115036160 | exact |
| frames-100 | insert | cold-file | ase.db | 6 | 3.3411e-02 | 3.3739e-02 | 5.0946e-04 | 0.015 | 2.9930e+03 | 2.2036e+00 | 151552 | 102301696 | exact |
| frames-10000 | insert | cold-file | readcon-db | 6 | 2.5727e-01 | 2.6148e-01 | 3.6733e-03 | 0.014 | 3.8870e+04 | 2.9831e+01 | 3362816 | 199933952 | exact |
| frames-10000 | insert | cold-file | ase.db | 6 | 3.0873e+00 | 3.1041e+00 | 3.7306e-02 | 0.012 | 3.2391e+03 | 2.4859e+00 | 10817536 | 201678848 | exact |

