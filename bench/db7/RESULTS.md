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
| readcon-db | 2.5906e-03 | 1.5194e-03 | 2.1684e-03 | 3.6879e-03 | 2.0851e-03 | 5.1632e-03 |
| ase.db | 5.0790e-02 | 7.1047e-03 | 4.4892e-02 | 5.1996e-02 | 4.0872e-02 | 6.1476e-02 |

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
| paper40x8 | insert | cold-file | readcon-db | 6 | 1.9984e-03 | 8.9454e-05 | 0.045 | 81920 | 103555072 | exact |
| paper40x8 | insert | cold-file | ase.db | 6 | 5.4800e-03 | 1.7036e-04 | 0.031 | 90112 | 97456128 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 12 | 1.4770e-03 | 1.6639e-04 | 0.113 | 143360 | 99540992 | exact |
| atoms-1000 | insert | cold-file | ase.db | 6 | 1.5312e-03 | 2.9423e-05 | 0.019 | 126976 | 98496512 | exact |
| atoms-1000 | full_read | cold | readcon-db | 6 | 6.5812e-05 | 2.8488e-06 | 0.043 | 143360 | 93097984 | exact |
| atoms-1000 | full_read | cold | ase.db | 6 | 1.4175e-03 | 6.6639e-05 | 0.047 | 126976 | 95965184 | exact |
| atoms-1000 | selected | cold | readcon-db | 7 | 6.3669e-05 | 2.8993e-06 | 0.046 | 143360 | 93003776 | exact |
| atoms-1000 | selected | cold | ase.db | 12 | 1.6982e-03 | 1.3535e-04 | 0.080 | 126976 | 95928320 | exact |
| atoms-1000 | full_read | warm | readcon-db | 6 | 1.6755e-05 | 3.9612e-08 | 0.002 | 143360 | 93200384 | exact |
| atoms-1000 | full_read | warm | ase.db | 6 | 3.7118e-04 | 1.3661e-06 | 0.004 | 126976 | 99364864 | exact |
| atoms-1000 | selected | warm | readcon-db | 6 | 1.6334e-05 | 6.1557e-08 | 0.004 | 143360 | 93208576 | exact |
| atoms-1000 | selected | warm | ase.db | 6 | 3.9276e-04 | 9.0843e-06 | 0.023 | 126976 | 98795520 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 6 | 3.6770e-03 | 1.1609e-04 | 0.032 | 634880 | 121380864 | exact |
| atoms-10000 | insert | cold-file | ase.db | 9 | 3.7480e-03 | 1.2616e-04 | 0.034 | 667648 | 112246784 | exact |
| atoms-10000 | full_read | cold | readcon-db | 6 | 1.1776e-03 | 2.4564e-05 | 0.021 | 634880 | 100913152 | exact |
| atoms-10000 | full_read | cold | ase.db | 12 | 4.5261e-03 | 3.5606e-04 | 0.079 | 667648 | 118771712 | exact |
| atoms-10000 | selected | cold | readcon-db | 6 | 1.1812e-03 | 4.2854e-05 | 0.036 | 634880 | 101187584 | exact |
| atoms-10000 | selected | cold | ase.db | 8 | 4.9932e-03 | 2.1434e-04 | 0.043 | 667648 | 118583296 | exact |
| atoms-10000 | full_read | warm | readcon-db | 6 | 3.0099e-04 | 1.2165e-07 | 0.000 | 634880 | 100118528 | exact |
| atoms-10000 | full_read | warm | ase.db | 12 | 2.6097e-03 | 6.0881e-04 | 0.233 | 667648 | 145743872 | exact |
| atoms-10000 | selected | warm | readcon-db | 6 | 3.1123e-04 | 3.9907e-07 | 0.001 | 634880 | 100220928 | exact |
| atoms-10000 | selected | warm | ase.db | 12 | 2.6725e-03 | 6.8484e-04 | 0.256 | 667648 | 146501632 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 6 | 1.7484e-02 | 6.0176e-04 | 0.034 | 5435392 | 251207680 | exact |
| atoms-100000 | insert | cold-file | ase.db | 6 | 2.5422e-02 | 4.4997e-04 | 0.018 | 6074368 | 250970112 | exact |
| atoms-100000 | full_read | cold | readcon-db | 6 | 6.9350e-03 | 1.8400e-04 | 0.027 | 5435392 | 142286848 | exact |
| atoms-100000 | full_read | cold | ase.db | 10 | 3.5580e-02 | 1.5385e-03 | 0.043 | 6074368 | 214634496 | exact |
| atoms-100000 | selected | cold | readcon-db | 6 | 7.1029e-03 | 3.2743e-04 | 0.046 | 5435392 | 142204928 | exact |
| atoms-100000 | selected | cold | ase.db | 6 | 3.4956e-02 | 1.5291e-03 | 0.044 | 6074368 | 214876160 | exact |
| atoms-100000 | full_read | warm | readcon-db | 6 | 3.6270e-03 | 1.6313e-05 | 0.004 | 5435392 | 133337088 | exact |
| atoms-100000 | full_read | warm | ase.db | 6 | 2.9274e-02 | 2.2934e-04 | 0.008 | 6074368 | 211890176 | exact |
| atoms-100000 | selected | warm | readcon-db | 6 | 3.5696e-03 | 2.6833e-05 | 0.008 | 5435392 | 133378048 | exact |
| atoms-100000 | selected | warm | ase.db | 6 | 3.0081e-02 | 1.8494e-04 | 0.006 | 6074368 | 214872064 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 6 | 1.7160e-01 | 4.5286e-03 | 0.026 | 52637696 | 1639284736 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 6 | 2.3362e-01 | 5.3489e-03 | 0.023 | 60125184 | 1639682048 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 12 | 7.6251e-02 | 8.3736e-03 | 0.110 | 52637696 | 516624384 | exact |
| atoms-1000000 | full_read | cold | ase.db | 9 | 3.1884e-01 | 8.1550e-03 | 0.026 | 60125184 | 1132404736 | exact |
| atoms-1000000 | selected | cold | readcon-db | 6 | 7.8086e-02 | 3.5864e-03 | 0.046 | 52637696 | 516591616 | exact |
| atoms-1000000 | selected | cold | ase.db | 6 | 3.1313e-01 | 9.2157e-03 | 0.029 | 60125184 | 931000320 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 7 | 4.0462e-02 | 1.9284e-03 | 0.048 | 52637696 | 429731840 | exact |
| atoms-1000000 | full_read | warm | ase.db | 6 | 2.7745e-01 | 3.8059e-03 | 0.014 | 60125184 | 904749056 | exact |
| atoms-1000000 | selected | warm | readcon-db | 8 | 3.9416e-02 | 1.6175e-03 | 0.041 | 52637696 | 429727744 | exact |
| atoms-1000000 | selected | warm | ase.db | 6 | 2.7739e-01 | 2.1308e-03 | 0.008 | 60125184 | 908853248 | exact |
| frames-1 | insert | cold-file | readcon-db | 12 | 9.2044e-04 | 1.3418e-04 | 0.146 | 86016 | 96350208 | exact |
| frames-1 | insert | cold-file | ase.db | 6 | 1.0444e-03 | 1.9318e-05 | 0.018 | 69632 | 97378304 | exact |
| frames-1 | full_read | cold | readcon-db | 6 | 4.0163e-05 | 4.5891e-07 | 0.011 | 86016 | 93421568 | exact |
| frames-1 | full_read | cold | ase.db | 6 | 4.6135e-04 | 1.4058e-05 | 0.030 | 69632 | 93487104 | exact |
| frames-1 | selected | cold | readcon-db | 6 | 4.0157e-05 | 2.6039e-07 | 0.006 | 86016 | 93175808 | exact |
| frames-1 | selected | cold | ase.db | 6 | 7.1232e-04 | 2.0412e-05 | 0.029 | 69632 | 93589504 | exact |
| frames-1 | full_read | warm | readcon-db | 6 | 1.0988e-05 | 5.4332e-08 | 0.005 | 86016 | 93184000 | exact |
| frames-1 | full_read | warm | ase.db | 6 | 1.4632e-04 | 1.0494e-06 | 0.007 | 69632 | 93585408 | exact |
| frames-1 | selected | warm | readcon-db | 6 | 1.0752e-05 | 7.2022e-08 | 0.007 | 86016 | 93327360 | exact |
| frames-1 | selected | warm | ase.db | 6 | 1.5695e-04 | 1.0456e-06 | 0.007 | 69632 | 93732864 | exact |
| frames-100 | insert | cold-file | readcon-db | 6 | 4.4316e-03 | 1.4350e-04 | 0.032 | 180224 | 105754624 | exact |
| frames-100 | insert | cold-file | ase.db | 6 | 2.7568e-02 | 1.9374e-04 | 0.007 | 151552 | 96694272 | exact |
| frames-100 | full_read | cold | readcon-db | 12 | 2.7769e-04 | 3.2548e-05 | 0.117 | 180224 | 95518720 | exact |
| frames-100 | full_read | cold | ase.db | 6 | 8.5839e-03 | 6.7894e-05 | 0.008 | 151552 | 94285824 | exact |
| frames-100 | selected | cold | readcon-db | 12 | 1.4161e-04 | 2.1656e-05 | 0.153 | 180224 | 93265920 | exact |
| frames-100 | selected | cold | ase.db | 6 | 1.0316e-03 | 3.0281e-05 | 0.029 | 151552 | 93499392 | exact |
| frames-100 | full_read | warm | readcon-db | 6 | 5.5139e-05 | 1.7618e-07 | 0.003 | 180224 | 95559680 | exact |
| frames-100 | full_read | warm | ase.db | 6 | 7.7548e-03 | 2.5473e-05 | 0.003 | 151552 | 94425088 | exact |
| frames-100 | selected | warm | readcon-db | 6 | 1.0871e-05 | 5.2540e-08 | 0.005 | 180224 | 93442048 | exact |
| frames-100 | selected | warm | ase.db | 6 | 1.6475e-04 | 1.1162e-06 | 0.007 | 151552 | 93671424 | exact |
| frames-10000 | insert | cold-file | readcon-db | 6 | 2.4663e-01 | 5.9947e-03 | 0.024 | 12517376 | 363356160 | exact |
| frames-10000 | insert | cold-file | ase.db | 6 | 2.5726e+00 | 3.3183e-02 | 0.013 | 10825728 | 239017984 | exact |
| frames-10000 | full_read | cold | readcon-db | 6 | 1.8036e-02 | 2.2603e-04 | 0.013 | 12517376 | 157732864 | exact |
| frames-10000 | full_read | cold | ase.db | 6 | 7.6357e-01 | 3.4632e-03 | 0.005 | 10825728 | 190681088 | exact |
| frames-10000 | selected | cold | readcon-db | 12 | 8.4066e-04 | 6.3230e-05 | 0.075 | 12517376 | 93364224 | exact |
| frames-10000 | selected | cold | ase.db | 6 | 4.0596e-03 | 1.3706e-04 | 0.034 | 10825728 | 93945856 | exact |
| frames-10000 | full_read | warm | readcon-db | 6 | 8.1850e-03 | 2.5140e-05 | 0.003 | 12517376 | 147042304 | exact |
| frames-10000 | full_read | warm | ase.db | 6 | 7.7235e-01 | 2.8860e-03 | 0.004 | 10825728 | 181764096 | exact |
| frames-10000 | selected | warm | readcon-db | 6 | 1.1117e-05 | 4.2396e-08 | 0.004 | 12517376 | 93528064 | exact |
| frames-10000 | selected | warm | ase.db | 6 | 7.1936e-04 | 8.0586e-07 | 0.001 | 10825728 | 94031872 | exact |

