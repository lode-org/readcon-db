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
| paper40x8 | insert | cold-file | readcon-db | 7 | 2.3240e-03 | 2.4343e-03 | 8.7069e-05 | 0.037 | 1.7212e+04 | 5.3701e+00 | 49152 | 118538240 | exact |
| paper40x8 | insert | cold-file | ase.db | 6 | 5.8179e-03 | 6.0778e-03 | 2.4323e-04 | 0.042 | 6.8754e+03 | 2.1451e+00 | 90112 | 106127360 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 12 | 1.2740e-03 | 1.3605e-03 | 1.5757e-04 | 0.124 | 7.8490e+02 | 3.3399e+01 | 98304 | 105635840 | exact |
| atoms-1000 | insert | cold-file | ase.db | 6 | 1.5186e-03 | 1.6177e-03 | 2.8031e-05 | 0.018 | 6.5848e+02 | 2.8020e+01 | 126976 | 103718912 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 6 | 3.9015e-03 | 4.1049e-03 | 1.2569e-04 | 0.032 | 2.5631e+02 | 1.1824e+02 | 589824 | 119398400 | exact |
| atoms-10000 | insert | cold-file | ase.db | 7 | 3.9635e-03 | 4.0508e-03 | 1.8919e-04 | 0.048 | 2.5230e+02 | 1.1639e+02 | 667648 | 112549888 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 6 | 3.0548e-02 | 3.0676e-02 | 3.1763e-04 | 0.010 | 3.2735e+01 | 1.6397e+02 | 5390336 | 177467392 | exact |
| atoms-100000 | insert | cold-file | ase.db | 6 | 2.7659e-02 | 2.7762e-02 | 3.4475e-04 | 0.012 | 3.6155e+01 | 1.8110e+02 | 6074368 | 205262848 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 6 | 3.2110e-01 | 3.4644e-01 | 1.0095e-02 | 0.031 | 3.1143e+00 | 1.6844e+02 | 52592640 | 547680256 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 6 | 2.6634e-01 | 2.8453e-01 | 9.9031e-03 | 0.037 | 3.7547e+00 | 2.0308e+02 | 60125184 | 1104744448 | exact |
| frames-1 | insert | cold-file | readcon-db | 12 | 8.1611e-04 | 1.0592e-03 | 2.0890e-04 | 0.256 | 1.2253e+03 | 8.8836e-01 | 40960 | 101347328 | exact |
| frames-1 | insert | cold-file | ase.db | 6 | 1.1933e-03 | 1.3130e-03 | 5.8045e-05 | 0.049 | 8.3802e+02 | 6.0756e-01 | 69632 | 102912000 | exact |
| frames-100 | insert | cold-file | readcon-db | 6 | 4.6020e-03 | 4.6387e-03 | 9.1535e-05 | 0.020 | 2.1729e+04 | 1.5998e+01 | 65536 | 117006336 | exact |
| frames-100 | insert | cold-file | ase.db | 6 | 3.3493e-02 | 3.4165e-02 | 7.5014e-04 | 0.022 | 2.9857e+03 | 2.1982e+00 | 151552 | 102125568 | exact |
| frames-10000 | insert | cold-file | readcon-db | 6 | 2.6754e-01 | 2.7890e-01 | 4.8823e-03 | 0.018 | 3.7378e+04 | 2.8686e+01 | 3362816 | 199733248 | exact |
| frames-10000 | insert | cold-file | ase.db | 6 | 3.2155e+00 | 3.2729e+00 | 9.1838e-02 | 0.029 | 3.1100e+03 | 2.3867e+00 | 10833920 | 201621504 | exact |

