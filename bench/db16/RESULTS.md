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
| frames-1000 | insert | cold-file | readcon-db | 12 | 3.1329e-02 | 3.2395e-02 | 1.3075e-03 | 0.042 | 3.1920e+04 | 2.3988e+01 | 700416 | 123817984 | exact |
| frames-1000 | insert | cold-file | ase.db | 12 | 2.6398e-01 | 2.6739e-01 | 3.1734e-03 | 0.012 | 3.7881e+03 | 2.8468e+00 | 1138688 | 105959424 | exact |
| frames-1000 | full_read | cold | readcon-db | 12 | 3.9267e-04 | 3.9629e-04 | 3.8301e-06 | 0.010 |  |  | 700416 | 110813184 | exact |
| frames-1000 | full_read | cold | ase.db | 12 | 8.2973e-02 | 8.4212e-02 | 1.2814e-03 | 0.015 |  |  | 1146880 | 137416704 | exact |
| frames-1000 | selected | cold | readcon-db | 12 | 4.5421e-05 | 4.6670e-05 | 1.3917e-06 | 0.031 |  |  | 700416 | 102354944 | exact |
| frames-1000 | selected | cold | ase.db | 12 | 1.0461e-03 | 1.1843e-03 | 1.4956e-04 | 0.143 |  |  | 1146880 | 96272384 | exact |
| frames-1000 | range | cold | readcon-db | 12 | 1.6311e-04 | 1.6532e-04 | 2.2786e-06 | 0.014 |  |  | 700416 | 103104512 | exact |
| frames-1000 | range | cold | ase.db | 12 | 2.5221e-02 | 2.6104e-02 | 1.3150e-03 | 0.052 |  |  | 1146880 | 118489088 | exact |
| frames-1000 | metadata | cold | readcon-db | 12 | 5.4755e-05 | 5.5406e-05 | 1.1010e-06 | 0.020 |  |  | 700416 | 102440960 | exact |
| frames-1000 | metadata | cold | ase.db | 12 | 4.4187e-03 | 5.0931e-03 | 1.1113e-03 | 0.251 |  |  | 1146880 | 94879744 | exact |
| frames-1000 | full_read | warm | readcon-db | 12 | 3.4252e-04 | 3.4485e-04 | 2.9585e-06 | 0.009 |  |  | 700416 | 105967616 | exact |
| frames-1000 | full_read | warm | ase.db | 12 | 8.2618e-02 | 8.3202e-02 | 1.6458e-03 | 0.020 |  |  | 1146880 | 138903552 | exact |
| frames-1000 | selected | warm | readcon-db | 12 | 1.0759e-05 | 1.0874e-05 | 8.5295e-08 | 0.008 |  |  | 700416 | 98418688 | exact |
| frames-1000 | selected | warm | ase.db | 12 | 2.1985e-04 | 2.2114e-04 | 1.8301e-06 | 0.008 |  |  | 1146880 | 101322752 | exact |
| frames-1000 | range | warm | readcon-db | 12 | 9.8599e-05 | 1.0081e-04 | 1.3202e-06 | 0.013 |  |  | 700416 | 100892672 | exact |
| frames-1000 | range | warm | ase.db | 12 | 2.0519e-02 | 2.1104e-02 | 3.8407e-04 | 0.019 |  |  | 1146880 | 122220544 | exact |
| frames-1000 | metadata | warm | readcon-db | 12 | 2.5909e-05 | 2.6083e-05 | 8.1186e-08 | 0.003 |  |  | 700416 | 98664448 | exact |
| frames-1000 | metadata | warm | ase.db | 12 | 3.1820e-03 | 3.6974e-03 | 9.2714e-04 | 0.291 |  |  | 1146880 | 94654464 | exact |
| frames-10000 | insert | cold-file | readcon-db | 12 | 3.0823e-01 | 3.1605e-01 | 8.5867e-03 | 0.028 | 3.2444e+04 | 2.4899e+01 | 6266880 | 200548352 | exact |
| frames-10000 | insert | cold-file | ase.db | 12 | 2.6783e+00 | 2.7059e+00 | 3.7192e-02 | 0.014 | 3.7337e+03 | 2.8654e+00 | 10792960 | 195915776 | exact |
| frames-10000 | full_read | cold | readcon-db | 12 | 6.1364e-03 | 8.0564e-03 | 2.3122e-03 | 0.377 |  |  | 6266880 | 186851328 | exact |
| frames-10000 | full_read | cold | ase.db | 12 | 9.6037e-01 | 9.7796e-01 | 3.7544e-02 | 0.039 |  |  | 10801152 | 526925824 | exact |
| frames-10000 | selected | cold | readcon-db | 12 | 5.0816e-05 | 5.1244e-05 | 6.0038e-07 | 0.012 |  |  | 6266880 | 115802112 | exact |
| frames-10000 | selected | cold | ase.db | 12 | 5.2024e-03 | 5.8622e-03 | 1.1079e-03 | 0.213 |  |  | 10801152 | 94691328 | exact |
| frames-10000 | range | cold | readcon-db | 12 | 1.6194e-03 | 1.7184e-03 | 1.2151e-04 | 0.075 |  |  | 6266880 | 142344192 | exact |
| frames-10000 | range | cold | ase.db | 12 | 2.3584e-01 | 2.4051e-01 | 7.9558e-03 | 0.034 |  |  | 10801152 | 203689984 | exact |
| frames-10000 | metadata | cold | readcon-db | 12 | 3.2679e-04 | 3.4269e-04 | 1.6154e-05 | 0.049 |  |  | 6266880 | 116215808 | exact |
| frames-10000 | metadata | cold | ase.db | 12 | 4.4722e-02 | 4.8902e-02 | 5.9974e-03 | 0.134 |  |  | 10801152 | 98770944 | exact |
| frames-10000 | full_read | warm | readcon-db | 12 | 5.2466e-03 | 5.6983e-03 | 9.0559e-05 | 0.017 |  |  | 6266880 | 164052992 | exact |
| frames-10000 | full_read | warm | ase.db | 12 | 9.2368e-01 | 9.4704e-01 | 2.2803e-02 | 0.025 |  |  | 10801152 | 545329152 | exact |
| frames-10000 | selected | warm | readcon-db | 12 | 1.0773e-05 | 1.0855e-05 | 4.7335e-08 | 0.004 |  |  | 6266880 | 112508928 | exact |
| frames-10000 | selected | warm | ase.db | 12 | 7.2724e-04 | 7.4186e-04 | 1.7741e-05 | 0.024 |  |  | 10801152 | 96411648 | exact |
| frames-10000 | range | warm | readcon-db | 12 | 1.0551e-03 | 1.0738e-03 | 6.0186e-06 | 0.006 |  |  | 6266880 | 129875968 | exact |
| frames-10000 | range | warm | ase.db | 12 | 2.1742e-01 | 2.2619e-01 | 7.5169e-03 | 0.035 |  |  | 10801152 | 208019456 | exact |
| frames-10000 | metadata | warm | readcon-db | 12 | 2.4941e-04 | 2.5122e-04 | 1.1301e-06 | 0.005 |  |  | 6266880 | 112574464 | exact |
| frames-10000 | metadata | warm | ase.db | 12 | 2.5901e-02 | 2.6253e-02 | 4.6527e-04 | 0.018 |  |  | 10801152 | 98570240 | exact |
| traj-100k | insert | cold-file | readcon-db | 12 | 1.1973e+00 | 1.2524e+00 | 6.3248e-02 | 0.053 | 5.3455e+01 | 2.7611e+02 | 342003712 | 2371624960 | exact |
| traj-100k | insert | cold-file | ase.db | 12 | 1.4282e+00 | 1.4753e+00 | 3.5459e-02 | 0.025 | 4.4810e+01 | 2.3146e+02 | 384503808 | 2176212992 | exact |
| traj-100k | full_read | cold | readcon-db | 12 | 6.2669e-01 | 6.8044e-01 | 8.0982e-02 | 0.129 |  |  | 342003712 | 1931218944 | exact |
| traj-100k | full_read | cold | ase.db | 12 | 2.1841e+00 | 2.3060e+00 | 1.9412e-01 | 0.089 |  |  | 384503808 | 7860563968 | exact |
| traj-100k | selected | cold | readcon-db | 12 | 7.6325e-03 | 8.2194e-03 | 6.6936e-04 | 0.088 |  |  | 342003712 | 130240512 | exact |
| traj-100k | selected | cold | ase.db | 12 | 3.9290e-02 | 4.0322e-02 | 1.1023e-03 | 0.028 |  |  | 384503808 | 305639424 | exact |
| traj-100k | range | cold | readcon-db | 12 | 2.0671e-01 | 2.4219e-01 | 4.6291e-02 | 0.224 |  |  | 342003712 | 693903360 | exact |
| traj-100k | range | cold | ase.db | 12 | 5.5706e-01 | 5.9830e-01 | 2.4356e-02 | 0.044 |  |  | 384503808 | 2179706880 | exact |
| traj-100k | metadata | cold | readcon-db | 12 | 2.8889e-04 | 3.2962e-04 | 2.9552e-05 | 0.102 |  |  | 342003712 | 93433856 | exact |
| traj-100k | metadata | cold | ase.db | 12 | 2.3226e-01 | 3.0097e-01 | 8.2580e-02 | 0.356 |  |  | 384503808 | 143269888 | exact |
| traj-100k | full_read | warm | readcon-db | 12 | 3.7355e-01 | 4.0419e-01 | 4.7146e-02 | 0.126 |  |  | 342003712 | 1527427072 | exact |
| traj-100k | full_read | warm | ase.db | 12 | 2.0383e+00 | 2.1721e+00 | 7.4519e-02 | 0.037 |  |  | 384503808 | 7752949760 | exact |
| traj-100k | selected | warm | readcon-db | 12 | 2.7337e-03 | 2.8296e-03 | 5.9297e-05 | 0.022 |  |  | 342003712 | 123645952 | exact |
| traj-100k | selected | warm | ase.db | 12 | 3.1978e-02 | 3.2702e-02 | 4.9529e-04 | 0.015 |  |  | 384503808 | 303718400 | exact |
| traj-100k | range | warm | readcon-db | 12 | 9.9949e-02 | 1.6315e-01 | 7.2860e-03 | 0.073 |  |  | 342003712 | 587051008 | exact |
| traj-100k | range | warm | ase.db | 12 | 5.3286e-01 | 5.4588e-01 | 8.9391e-03 | 0.017 |  |  | 384503808 | 2150981632 | exact |
| traj-100k | metadata | warm | readcon-db | 12 | 3.2773e-06 | 3.3009e-06 | 1.5082e-08 | 0.005 |  |  | 342003712 | 93528064 | exact |
| traj-100k | metadata | warm | ase.db | 12 | 7.4348e-02 | 7.6829e-02 | 2.3075e-03 | 0.031 |  |  | 384503808 | 143007744 | exact |

