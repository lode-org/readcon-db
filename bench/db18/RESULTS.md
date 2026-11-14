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
| frames-1000 | insert | cold-file | readcon-db | 12 | 2.9982e-02 | 3.1496e-02 | 4.6681e-04 | 0.016 | 3.3353e+04 | 2.5065e+01 | 356352 | 122728448 | exact |
| frames-1000 | insert | cold-file | ase.db | 12 | 2.7370e-01 | 2.8093e-01 | 8.1356e-03 | 0.030 | 3.6536e+03 | 2.7456e+00 | 1146880 | 106180608 | exact |
| frames-1000 | full_read | cold | readcon-db | 12 | 4.1320e-04 | 4.2625e-04 | 6.5989e-06 | 0.016 |  |  | 356352 | 113188864 | exact |
| frames-1000 | full_read | cold | ase.db | 12 | 8.4242e-02 | 8.5067e-02 | 1.1020e-03 | 0.013 |  |  | 1146880 | 137719808 | exact |
| frames-1000 | selected | cold | readcon-db | 12 | 4.1834e-05 | 4.4534e-05 | 1.2188e-06 | 0.029 |  |  | 356352 | 100323328 | exact |
| frames-1000 | selected | cold | ase.db | 12 | 1.4350e-03 | 1.7805e-03 | 3.0797e-04 | 0.215 |  |  | 1146880 | 95657984 | exact |
| frames-1000 | range | cold | readcon-db | 12 | 1.6030e-04 | 1.6354e-04 | 2.8112e-06 | 0.018 |  |  | 356352 | 101998592 | exact |
| frames-1000 | range | cold | ase.db | 12 | 2.4791e-02 | 2.5748e-02 | 1.2764e-03 | 0.051 |  |  | 1146880 | 119238656 | exact |
| frames-1000 | metadata | cold | readcon-db | 12 | 3.5980e-05 | 3.7316e-05 | 1.6564e-06 | 0.046 |  |  | 356352 | 100331520 | exact |
| frames-1000 | metadata | cold | ase.db | 12 | 4.5627e-03 | 4.7361e-03 | 3.3343e-04 | 0.073 |  |  | 1146880 | 94990336 | exact |
| frames-1000 | full_read | warm | readcon-db | 12 | 3.4944e-04 | 3.5171e-04 | 4.6256e-06 | 0.013 |  |  | 356352 | 105586688 | exact |
| frames-1000 | full_read | warm | ase.db | 12 | 7.9586e-02 | 8.0776e-02 | 1.0575e-03 | 0.013 |  |  | 1146880 | 139345920 | exact |
| frames-1000 | selected | warm | readcon-db | 12 | 1.0535e-05 | 1.0713e-05 | 7.4050e-08 | 0.007 |  |  | 356352 | 100466688 | exact |
| frames-1000 | selected | warm | ase.db | 12 | 2.2526e-04 | 2.2766e-04 | 1.9049e-06 | 0.008 |  |  | 1146880 | 101076992 | exact |
| frames-1000 | range | warm | readcon-db | 12 | 9.6116e-05 | 9.7022e-05 | 8.8132e-07 | 0.009 |  |  | 356352 | 100622336 | exact |
| frames-1000 | range | warm | ase.db | 12 | 2.0494e-02 | 3.1382e-02 | 2.8693e-03 | 0.140 |  |  | 1146880 | 120487936 | exact |
| frames-1000 | metadata | warm | readcon-db | 12 | 1.2690e-05 | 1.2877e-05 | 1.7683e-07 | 0.014 |  |  | 356352 | 100368384 | exact |
| frames-1000 | metadata | warm | ase.db | 12 | 2.5128e-03 | 2.5368e-03 | 1.7814e-05 | 0.007 |  |  | 1146880 | 94625792 | exact |
| frames-10000 | insert | cold-file | readcon-db | 12 | 2.6629e-01 | 2.7238e-01 | 9.8599e-03 | 0.037 | 3.7553e+04 | 2.8820e+01 | 3362816 | 196059136 | exact |
| frames-10000 | insert | cold-file | ase.db | 12 | 2.6325e+00 | 2.6772e+00 | 4.0609e-02 | 0.015 | 3.7987e+03 | 2.9153e+00 | 10817536 | 196186112 | exact |
| frames-10000 | full_read | cold | readcon-db | 12 | 5.3160e-03 | 6.9052e-03 | 5.9536e-04 | 0.112 |  |  | 3362816 | 183472128 | exact |
| frames-10000 | full_read | cold | ase.db | 12 | 9.2724e-01 | 9.6685e-01 | 2.4840e-02 | 0.027 |  |  | 10817536 | 527319040 | exact |
| frames-10000 | selected | cold | readcon-db | 12 | 5.0593e-05 | 5.1503e-05 | 7.4373e-07 | 0.015 |  |  | 3362816 | 112672768 | exact |
| frames-10000 | selected | cold | ase.db | 12 | 3.4882e-03 | 4.7623e-03 | 1.0001e-03 | 0.287 |  |  | 10817536 | 94928896 | exact |
| frames-10000 | range | cold | readcon-db | 12 | 1.6480e-03 | 2.1862e-03 | 1.7941e-04 | 0.109 |  |  | 3362816 | 138862592 | exact |
| frames-10000 | range | cold | ase.db | 12 | 2.2767e-01 | 2.5220e-01 | 1.9782e-02 | 0.087 |  |  | 10817536 | 203456512 | exact |
| frames-10000 | metadata | cold | readcon-db | 12 | 1.5097e-04 | 1.5596e-04 | 3.9695e-06 | 0.026 |  |  | 3362816 | 112648192 | exact |
| frames-10000 | metadata | cold | ase.db | 12 | 3.7219e-02 | 4.7221e-02 | 4.1481e-03 | 0.111 |  |  | 10817536 | 98873344 | exact |
| frames-10000 | full_read | warm | readcon-db | 12 | 5.2633e-03 | 5.6746e-03 | 1.1673e-04 | 0.022 |  |  | 3362816 | 159404032 | exact |
| frames-10000 | full_read | warm | ase.db | 12 | 9.1584e-01 | 9.2683e-01 | 8.7382e-03 | 0.010 |  |  | 10817536 | 545562624 | exact |
| frames-10000 | selected | warm | readcon-db | 12 | 1.0510e-05 | 1.0915e-05 | 2.2691e-07 | 0.022 |  |  | 3362816 | 108716032 | exact |
| frames-10000 | selected | warm | ase.db | 12 | 7.3021e-04 | 7.4634e-04 | 1.1445e-05 | 0.016 |  |  | 10817536 | 96366592 | exact |
| frames-10000 | range | warm | readcon-db | 12 | 1.0837e-03 | 1.1088e-03 | 1.6468e-05 | 0.015 |  |  | 3362816 | 126287872 | exact |
| frames-10000 | range | warm | ase.db | 12 | 2.1521e-01 | 2.2136e-01 | 3.8374e-03 | 0.018 |  |  | 10817536 | 208150528 | exact |
| frames-10000 | metadata | warm | readcon-db | 12 | 1.0018e-04 | 1.0150e-04 | 4.7955e-07 | 0.005 |  |  | 3362816 | 108908544 | exact |
| frames-10000 | metadata | warm | ase.db | 12 | 2.5708e-02 | 2.5856e-02 | 2.0537e-04 | 0.008 |  |  | 10817536 | 98844672 | exact |
| traj-100k | insert | cold-file | readcon-db | 12 | 1.2290e+00 | 1.4198e+00 | 1.3053e-01 | 0.106 | 5.2074e+01 | 2.6898e+02 | 341966848 | 2817257472 | exact |
| traj-100k | insert | cold-file | ase.db | 12 | 1.3944e+00 | 1.4383e+00 | 3.8018e-02 | 0.027 | 4.5898e+01 | 2.3708e+02 | 384503808 | 2177728512 | exact |
| traj-100k | full_read | cold | readcon-db | 12 | 6.3785e-01 | 7.4383e-01 | 6.7841e-02 | 0.106 |  |  | 341966848 | 1931800576 | exact |
| traj-100k | full_read | cold | ase.db | 12 | 2.4140e+00 | 2.5348e+00 | 1.7470e-01 | 0.072 |  |  | 384503808 | 7852355584 | exact |
| traj-100k | selected | cold | readcon-db | 12 | 6.7791e-03 | 8.3249e-03 | 7.5198e-04 | 0.111 |  |  | 341966848 | 130117632 | exact |
| traj-100k | selected | cold | ase.db | 12 | 3.6274e-02 | 3.9621e-02 | 8.9825e-04 | 0.025 |  |  | 384503808 | 298496000 | exact |
| traj-100k | range | cold | readcon-db | 12 | 1.7474e-01 | 2.1360e-01 | 3.7546e-02 | 0.215 |  |  | 341966848 | 693956608 | exact |
| traj-100k | range | cold | ase.db | 12 | 6.0247e-01 | 6.7584e-01 | 2.6850e-02 | 0.045 |  |  | 384503808 | 2182221824 | exact |
| traj-100k | metadata | cold | readcon-db | 12 | 1.5063e-05 | 1.6103e-05 | 1.3844e-06 | 0.092 |  |  | 341966848 | 93786112 | exact |
| traj-100k | metadata | cold | ase.db | 12 | 2.8549e-01 | 3.4557e-01 | 3.9543e-02 | 0.139 |  |  | 384503808 | 143216640 | exact |
| traj-100k | full_read | warm | readcon-db | 12 | 3.6144e-01 | 3.9268e-01 | 4.2843e-02 | 0.119 |  |  | 341966848 | 1528553472 | exact |
| traj-100k | full_read | warm | ase.db | 12 | 1.9284e+00 | 1.9676e+00 | 2.3640e-02 | 0.012 |  |  | 384503808 | 7744528384 | exact |
| traj-100k | selected | warm | readcon-db | 12 | 2.7266e-03 | 2.8070e-03 | 4.3073e-05 | 0.016 |  |  | 341966848 | 124174336 | exact |
| traj-100k | selected | warm | ase.db | 12 | 3.0134e-02 | 3.0424e-02 | 5.4572e-04 | 0.018 |  |  | 384503808 | 296435712 | exact |
| traj-100k | range | warm | readcon-db | 12 | 9.3126e-02 | 1.0280e-01 | 6.9191e-03 | 0.074 |  |  | 341966848 | 586838016 | exact |
| traj-100k | range | warm | ase.db | 12 | 4.9302e-01 | 4.9694e-01 | 5.2838e-03 | 0.011 |  |  | 384503808 | 2151145472 | exact |
| traj-100k | metadata | warm | readcon-db | 12 | 2.4335e-06 | 2.4481e-06 | 1.6378e-08 | 0.007 |  |  | 341966848 | 93712384 | exact |
| traj-100k | metadata | warm | ase.db | 12 | 8.5897e-02 | 8.7668e-02 | 1.7527e-03 | 0.020 |  |  | 384503808 | 143179776 | exact |

## File size against db16

The scalar indexes are one `idx_cols` blob. `hash_by_frame` is empty.
`frame_by_hash` is unchanged. n=12, same harness as db16.

| workload | db16 file B | db18 file B | db16 readcon metadata cold | db18 |
| --- | --- | --- | --- | --- |
| frames-1000 | 700416 | 356352 | 5.4755e-05 | 3.5980e-05 |
| frames-10000 | 6266880 | 3362816 | 3.2679e-04 | 1.5097e-04 |
| traj-100k | 342003712 | 341966848 | 2.8889e-04 | 1.5063e-05 |

