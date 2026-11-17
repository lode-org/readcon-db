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
| paper40x8 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.2547e-03 | 2.2901e-03 | 7.8818e-05 | 0.035 | 1.7741e+04 | 5.5352e+00 | 49152 | 117624832 | exact |
| paper40x8 | insert | cold-file | ase.db | 1 | 1 | 12 | 5.3998e-03 | 5.4695e-03 | 2.0586e-04 | 0.038 | 7.4077e+03 | 2.3112e+00 | 90112 | 104886272 | exact |
| paper40x8 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.2168e-03 | 2.2961e-03 | 8.3496e-05 | 0.038 | 1.8044e+04 | 5.6296e+00 | 49152 | 117612544 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 1.2379e-03 | 1.3069e-03 | 7.9828e-05 | 0.064 | 8.0783e+02 | 3.4375e+01 | 98304 | 104501248 | exact |
| atoms-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.4898e-03 | 1.7463e-03 | 1.1405e-04 | 0.077 | 6.7125e+02 | 2.8563e+01 | 126976 | 105287680 | exact |
| atoms-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 1.2490e-03 | 1.3594e-03 | 1.3773e-04 | 0.110 | 8.0064e+02 | 3.4069e+01 | 98304 | 104419328 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.6602e-03 | 3.8320e-03 | 2.0238e-04 | 0.055 | 2.7321e+02 | 1.2604e+02 | 589824 | 118648832 | exact |
| atoms-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.9492e-03 | 4.0393e-03 | 1.0078e-04 | 0.026 | 2.5322e+02 | 1.1682e+02 | 667648 | 111448064 | exact |
| atoms-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.6955e-03 | 3.8026e-03 | 1.4118e-04 | 0.038 | 2.7060e+02 | 1.2483e+02 | 589824 | 118595584 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.5401e-02 | 2.5969e-02 | 4.7284e-04 | 0.019 | 3.9368e+01 | 1.9720e+02 | 5390336 | 196886528 | exact |
| atoms-100000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.5845e-02 | 2.8989e-02 | 3.2291e-03 | 0.125 | 3.8693e+01 | 1.9382e+02 | 6074368 | 204423168 | exact |
| atoms-100000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.5254e-02 | 2.7010e-02 | 1.5274e-03 | 0.060 | 3.9598e+01 | 1.9835e+02 | 5390336 | 199737344 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.1664e-01 | 3.2217e-01 | 8.5183e-03 | 0.027 | 3.1582e+00 | 1.7082e+02 | 52592640 | 693600256 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.5721e-01 | 2.7041e-01 | 7.7304e-03 | 0.030 | 3.8878e+00 | 2.1028e+02 | 60125184 | 1119596544 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.8954e-01 | 2.9199e-01 | 4.3143e-03 | 0.015 | 3.4538e+00 | 1.8680e+02 | 52592640 | 693583872 | exact |
| frames-1 | insert | cold-file | readcon-db | 1 | 1 | 12 | 7.1552e-04 | 8.6665e-04 | 1.1396e-04 | 0.159 | 1.3976e+03 | 1.0133e+00 | 40960 | 99799040 | exact |
| frames-1 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.0422e-03 | 1.0668e-03 | 3.6455e-05 | 0.035 | 9.5954e+02 | 6.9566e-01 | 69632 | 105185280 | exact |
| frames-1 | insert | cold-file | readcon-db | unset | 8 | 12 | 6.6392e-04 | 7.5042e-04 | 9.8678e-05 | 0.149 | 1.5062e+03 | 1.0920e+00 | 40960 | 100093952 | exact |
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.5394e-03 | 4.6403e-03 | 1.0128e-04 | 0.022 | 2.2030e+04 | 1.6219e+01 | 65536 | 116232192 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.3687e-02 | 3.4675e-02 | 6.0087e-04 | 0.018 | 2.9685e+03 | 2.1856e+00 | 151552 | 100564992 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.5473e-03 | 4.6086e-03 | 5.3578e-05 | 0.012 | 2.1991e+04 | 1.6191e+01 | 65536 | 113774592 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.2644e-01 | 3.3625e-01 | 1.0715e-02 | 0.033 | 3.0634e+04 | 2.3510e+01 | 3362816 | 192720896 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.1035e+00 | 3.2111e+00 | 9.0343e-02 | 0.029 | 3.2222e+03 | 2.4729e+00 | 10825728 | 200986624 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2881e-01 | 3.4240e-01 | 9.6184e-03 | 0.029 | 3.0413e+04 | 2.3340e+01 | 3362816 | 201154560 | exact |

