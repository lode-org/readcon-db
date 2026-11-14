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
| atoms-1000000 | insert | cold-file | readcon-db | 6 | 1.8928e-01 | 1.9353e-01 | 3.7451e-03 | 0.020 | 52637696 | 601739264 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 6 | 2.2832e-01 | 2.3084e-01 | 1.1731e-03 | 0.005 | 60125184 | 1102393344 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 6 | 6.8578e-02 | 7.3372e-02 | 3.4173e-03 | 0.050 | 52637696 | 421691392 | exact |
| atoms-1000000 | full_read | cold | ase.db | 6 | 3.3769e-01 | 3.6648e-01 | 1.1955e-02 | 0.035 | 60125184 | 928047104 | exact |
| atoms-1000000 | selected | cold | readcon-db | 6 | 6.8016e-02 | 7.0449e-02 | 1.3216e-03 | 0.019 | 52637696 | 422203392 | exact |
| atoms-1000000 | selected | cold | ase.db | 6 | 3.3928e-01 | 3.7028e-01 | 8.2694e-03 | 0.024 | 60125184 | 931008512 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 6 | 2.8910e-02 | 3.0105e-02 | 1.1590e-03 | 0.040 | 52637696 | 359002112 | exact |
| atoms-1000000 | full_read | warm | ase.db | 6 | 3.0642e-01 | 3.1307e-01 | 1.3095e-02 | 0.043 | 60125184 | 909103104 | exact |
| atoms-1000000 | selected | warm | readcon-db | 6 | 2.8016e-02 | 3.0572e-02 | 1.1790e-03 | 0.042 | 52637696 | 358785024 | exact |
| atoms-1000000 | selected | warm | ase.db | 6 | 2.8535e-01 | 2.8869e-01 | 8.0790e-03 | 0.028 | 60125184 | 905289728 | exact |
| frames-10000 | insert | cold-file | readcon-db | 6 | 2.8261e-01 | 2.8380e-01 | 1.0862e-03 | 0.004 | 12517376 | 189030400 | exact |
| frames-10000 | insert | cold-file | ase.db | 6 | 2.5240e+00 | 2.5963e+00 | 4.0383e-02 | 0.016 | 10817536 | 194351104 | exact |
| frames-10000 | full_read | cold | readcon-db | 12 | 1.8905e-02 | 2.1144e-02 | 1.2438e-03 | 0.066 | 12517376 | 153268224 | exact |
| frames-10000 | full_read | cold | ase.db | 7 | 7.8217e-01 | 8.4998e-01 | 3.8194e-02 | 0.049 | 10817536 | 193212416 | exact |
| frames-10000 | selected | cold | readcon-db | 12 | 5.0615e-04 | 5.9233e-04 | 6.3609e-05 | 0.126 | 12517376 | 93573120 | exact |
| frames-10000 | selected | cold | ase.db | 12 | 4.0003e-03 | 4.5435e-03 | 5.8519e-04 | 0.146 | 10817536 | 94265344 | exact |
| frames-10000 | full_read | warm | readcon-db | 6 | 7.2021e-03 | 7.5638e-03 | 4.4650e-05 | 0.006 | 12517376 | 144314368 | exact |
| frames-10000 | full_read | warm | ase.db | 6 | 7.5523e-01 | 7.6890e-01 | 9.6169e-03 | 0.013 | 10817536 | 180293632 | exact |
| frames-10000 | selected | warm | readcon-db | 6 | 1.1347e-05 | 1.1415e-05 | 7.4684e-08 | 0.007 | 12517376 | 93839360 | exact |
| frames-10000 | selected | warm | ase.db | 6 | 7.1378e-04 | 7.1876e-04 | 4.2237e-06 | 0.006 | 10817536 | 94285824 | exact |
| frames-100 | insert | cold-file | readcon-db | 6 | 4.3536e-03 | 4.4749e-03 | 1.3339e-04 | 0.031 | 180224 | 103497728 | exact |
| frames-100 | insert | cold-file | ase.db | 6 | 2.6960e-02 | 2.7180e-02 | 1.9329e-04 | 0.007 | 151552 | 95739904 | exact |
| frames-100 | full_read | cold | readcon-db | 12 | 2.7447e-04 | 3.1771e-04 | 8.7276e-05 | 0.318 | 180224 | 96030720 | exact |
| frames-100 | full_read | cold | ase.db | 6 | 8.9500e-03 | 9.0306e-03 | 1.0469e-04 | 0.012 | 151552 | 94781440 | exact |
| frames-100 | selected | cold | readcon-db | 12 | 1.6099e-04 | 2.2579e-04 | 8.9426e-05 | 0.555 | 180224 | 93585408 | exact |
| frames-100 | selected | cold | ase.db | 12 | 8.9746e-04 | 9.7682e-04 | 7.9090e-05 | 0.088 | 151552 | 94052352 | exact |
| frames-100 | full_read | cold-open | readcon-db | 12 | 5.1086e-04 | 7.2279e-04 | 1.4959e-04 | 0.293 | 180224 | 96100352 | exact |
| frames-100 | full_read | cold-open | ase.db | 6 | 9.2476e-03 | 9.2932e-03 | 4.8232e-05 | 0.005 | 151552 | 94810112 | exact |
| frames-100 | selected | cold-open | readcon-db | 12 | 8.3818e-04 | 8.9658e-04 | 1.8086e-04 | 0.216 | 180224 | 93650944 | exact |
| frames-100 | selected | cold-open | ase.db | 12 | 1.0509e-03 | 1.1461e-03 | 1.0863e-04 | 0.103 | 151552 | 93843456 | exact |
| frames-100 | full_read | warm | readcon-db | 6 | 5.6252e-05 | 5.6525e-05 | 1.1866e-07 | 0.002 | 180224 | 96186368 | exact |
| frames-100 | full_read | warm | ase.db | 6 | 7.6573e-03 | 7.6624e-03 | 5.4557e-06 | 0.001 | 151552 | 94781440 | exact |
| frames-100 | selected | warm | readcon-db | 6 | 1.2859e-05 | 1.3027e-05 | 1.3611e-07 | 0.011 | 180224 | 93827072 | exact |
| frames-100 | selected | warm | ase.db | 6 | 1.6465e-04 | 1.6578e-04 | 1.5476e-06 | 0.009 | 151552 | 94191616 | exact |

