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
clock. Each row is its own process. A short read repeats to 50 ms
and a short commit to 2 s. Six samples are kept. A row whose
IQR/median is still above 0.05 takes six more. frames-100
selected cold stays wider than that: its timed query mixes a
cached call with a fault, and that mix changes between samples.

| workload | op | cache | tool | n | median s | IQR s | IQR/median | disk B | peak RSS B | fidelity |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| atoms-100000 | insert | cold-file | readcon-db | 6 | 1.9837e-02 | 3.3982e-05 | 0.002 | 5435392 | 171892736 | exact |
| atoms-100000 | insert | cold-file | ase.db | 6 | 2.4285e-02 | 8.2021e-04 | 0.034 | 6074368 | 197275648 | exact |
| atoms-100000 | full_read | cold | readcon-db | 8 | 7.5987e-03 | 3.1649e-04 | 0.042 | 5435392 | 129720320 | exact |
| atoms-100000 | full_read | cold | ase.db | 6 | 3.6687e-02 | 1.2535e-03 | 0.034 | 6074368 | 208416768 | exact |
| atoms-100000 | selected | cold | readcon-db | 12 | 6.4459e-03 | 8.1848e-04 | 0.127 | 5435392 | 129687552 | exact |
| atoms-100000 | selected | cold | ase.db | 12 | 3.6925e-02 | 6.4423e-03 | 0.174 | 6074368 | 216866816 | exact |
| atoms-100000 | full_read | warm | readcon-db | 6 | 2.7524e-03 | 1.5917e-05 | 0.006 | 5435392 | 123146240 | exact |
| atoms-100000 | full_read | warm | ase.db | 6 | 3.0115e-02 | 9.5798e-04 | 0.032 | 6074368 | 211959808 | exact |
| atoms-100000 | selected | warm | readcon-db | 6 | 2.6773e-03 | 9.8164e-06 | 0.004 | 5435392 | 123187200 | exact |
| atoms-100000 | selected | warm | ase.db | 6 | 3.0678e-02 | 7.0807e-04 | 0.023 | 6074368 | 216944640 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 6 | 1.8607e-01 | 2.9981e-03 | 0.016 | 52637696 | 564789248 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 6 | 2.2488e-01 | 3.8416e-03 | 0.017 | 60125184 | 1110016000 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 12 | 6.5334e-02 | 5.3780e-03 | 0.082 | 52637696 | 422346752 | exact |
| atoms-1000000 | full_read | cold | ase.db | 7 | 3.1483e-01 | 1.3248e-02 | 0.042 | 60125184 | 995766272 | exact |
| atoms-1000000 | selected | cold | readcon-db | 6 | 6.6783e-02 | 2.7404e-03 | 0.041 | 52637696 | 421715968 | exact |
| atoms-1000000 | selected | cold | ase.db | 6 | 3.1668e-01 | 1.2843e-02 | 0.041 | 60125184 | 931041280 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 6 | 2.7800e-02 | 1.3465e-03 | 0.048 | 52637696 | 358576128 | exact |
| atoms-1000000 | full_read | warm | ase.db | 6 | 2.7658e-01 | 3.1973e-03 | 0.012 | 60125184 | 905019392 | exact |
| atoms-1000000 | selected | warm | readcon-db | 7 | 2.7266e-02 | 1.1738e-03 | 0.043 | 52637696 | 358875136 | exact |
| atoms-1000000 | selected | warm | ase.db | 6 | 2.7707e-01 | 1.8943e-03 | 0.007 | 60125184 | 905101312 | exact |
| frames-100 | insert | cold-file | readcon-db | 6 | 4.3462e-03 | 1.7989e-04 | 0.041 | 180224 | 103202816 | exact |
| frames-100 | insert | cold-file | ase.db | 6 | 2.7296e-02 | 1.9197e-04 | 0.007 | 151552 | 95547392 | exact |
| frames-100 | full_read | cold | readcon-db | 12 | 2.7612e-04 | 7.6582e-05 | 0.277 | 180224 | 95891456 | exact |
| frames-100 | full_read | cold | ase.db | 6 | 9.0665e-03 | 2.8378e-04 | 0.031 | 151552 | 94752768 | exact |
| frames-100 | selected | cold | readcon-db | 12 | 1.5552e-04 | 3.9011e-05 | 0.251 | 180224 | 93630464 | exact |
| frames-100 | selected | cold | ase.db | 11 | 1.0461e-03 | 5.1399e-05 | 0.049 | 151552 | 93917184 | exact |
| frames-100 | full_read | warm | readcon-db | 6 | 5.5659e-05 | 1.0824e-07 | 0.002 | 180224 | 96157696 | exact |
| frames-100 | full_read | warm | ase.db | 6 | 7.6809e-03 | 1.0683e-05 | 0.001 | 151552 | 94736384 | exact |
| frames-100 | selected | warm | readcon-db | 6 | 1.3447e-05 | 4.5451e-08 | 0.003 | 180224 | 93880320 | exact |
| frames-100 | selected | warm | ase.db | 6 | 1.6559e-04 | 1.3434e-06 | 0.008 | 151552 | 94117888 | exact |
| frames-10000 | insert | cold-file | readcon-db | 7 | 5.4166e-01 | 2.6710e-02 | 0.049 | 13176832 | 175984640 | exact |
| frames-10000 | insert | cold-file | ase.db | 6 | 2.6078e+00 | 1.9824e-02 | 0.008 | 10825728 | 194072576 | exact |
| frames-10000 | full_read | cold | readcon-db | 6 | 2.8168e-02 | 4.0561e-04 | 0.014 | 13176832 | 152883200 | exact |
| frames-10000 | full_read | cold | ase.db | 6 | 7.6346e-01 | 2.4874e-03 | 0.003 | 10809344 | 190795776 | exact |
| frames-10000 | selected | cold | readcon-db | 6 | 6.5329e-04 | 6.9119e-06 | 0.011 | 13176832 | 93519872 | exact |
| frames-10000 | selected | cold | ase.db | 12 | 4.2589e-03 | 4.8857e-04 | 0.115 | 10809344 | 94105600 | exact |
| frames-10000 | full_read | warm | readcon-db | 6 | 7.2477e-03 | 6.8731e-06 | 0.001 | 13176832 | 145014784 | exact |
| frames-10000 | full_read | warm | ase.db | 6 | 7.5735e-01 | 1.1153e-02 | 0.015 | 10809344 | 182222848 | exact |
| frames-10000 | selected | warm | readcon-db | 6 | 1.1752e-05 | 1.4585e-07 | 0.012 | 13176832 | 93839360 | exact |
| frames-10000 | selected | warm | ase.db | 6 | 7.2130e-04 | 3.3953e-06 | 0.005 | 10809344 | 94294016 | exact |

