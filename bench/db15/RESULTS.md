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
| frames-1000 | insert | cold-file | readcon-db | 12 | 3.3525e-02 | 3.4492e-02 | 1.4926e-03 | 0.045 | 2.9828e+04 | 2.2416e+01 | 1343488 | 123977728 | exact |
| frames-1000 | insert | cold-file | ase.db | 12 | 2.6754e-01 | 2.7335e-01 | 9.0236e-03 | 0.034 | 3.7378e+03 | 2.8089e+00 | 1146880 | 106106880 | exact |
| frames-1000 | full_read | cold | readcon-db | 12 | 1.7089e-03 | 1.7808e-03 | 7.7374e-05 | 0.045 |  |  | 1343488 | 105156608 | exact |
| frames-1000 | full_read | cold | ase.db | 12 | 8.5297e-02 | 9.9481e-02 | 4.3285e-03 | 0.051 |  |  | 1146880 | 137494528 | exact |
| frames-1000 | selected | cold | readcon-db | 12 | 2.8389e-04 | 3.1668e-04 | 5.4442e-05 | 0.192 |  |  | 1343488 | 93552640 | exact |
| frames-1000 | selected | cold | ase.db | 12 | 1.1323e-03 | 1.4613e-03 | 1.5087e-04 | 0.133 |  |  | 1146880 | 96165888 | exact |
| frames-1000 | range | cold | readcon-db | 12 | 1.3244e-03 | 1.5614e-03 | 1.3648e-04 | 0.103 |  |  | 1343488 | 98562048 | exact |
| frames-1000 | range | cold | ase.db | 12 | 2.6401e-02 | 2.7561e-02 | 1.4316e-03 | 0.054 |  |  | 1146880 | 114970624 | exact |
| frames-1000 | metadata | cold | readcon-db | 12 | 8.0470e-04 | 9.0544e-04 | 7.4249e-05 | 0.092 |  |  | 1343488 | 93794304 | exact |
| frames-1000 | metadata | cold | ase.db | 12 | 4.5259e-03 | 5.3687e-03 | 4.0975e-04 | 0.091 |  |  | 1146880 | 94736384 | exact |
| frames-1000 | full_read | warm | readcon-db | 12 | 6.7190e-04 | 6.7303e-04 | 1.6562e-06 | 0.002 |  |  | 1343488 | 104148992 | exact |
| frames-1000 | full_read | warm | ase.db | 12 | 8.0053e-02 | 8.0407e-02 | 5.3615e-04 | 0.007 |  |  | 1146880 | 139251712 | exact |
| frames-1000 | selected | warm | readcon-db | 12 | 1.1744e-05 | 1.1893e-05 | 6.7303e-08 | 0.006 |  |  | 1343488 | 93548544 | exact |
| frames-1000 | selected | warm | ase.db | 12 | 2.1890e-04 | 2.2433e-04 | 2.4519e-06 | 0.011 |  |  | 1146880 | 101048320 | exact |
| frames-1000 | range | warm | readcon-db | 12 | 1.3264e-04 | 1.3453e-04 | 7.4230e-07 | 0.006 |  |  | 1343488 | 98770944 | exact |
| frames-1000 | range | warm | ase.db | 12 | 2.0350e-02 | 2.0671e-02 | 1.8342e-04 | 0.009 |  |  | 1146880 | 122294272 | exact |
| frames-1000 | metadata | warm | readcon-db | 12 | 2.5824e-05 | 2.6900e-05 | 3.5097e-07 | 0.014 |  |  | 1343488 | 93814784 | exact |
| frames-1000 | metadata | warm | ase.db | 12 | 2.5371e-03 | 2.6333e-03 | 4.8611e-05 | 0.019 |  |  | 1146880 | 94556160 | exact |
| frames-10000 | insert | cold-file | readcon-db | 12 | 3.2143e-01 | 3.2282e-01 | 2.5921e-03 | 0.008 | 3.1111e+04 | 2.3876e+01 | 12517376 | 204898304 | exact |
| frames-10000 | insert | cold-file | ase.db | 12 | 2.8178e+00 | 3.1905e+00 | 3.0377e-01 | 0.108 | 3.5488e+03 | 2.7235e+00 | 10809344 | 195915776 | exact |
| frames-10000 | full_read | cold | readcon-db | 12 | 2.1961e-02 | 2.5322e-02 | 2.5979e-03 | 0.118 |  |  | 12517376 | 153088000 | exact |
| frames-10000 | full_read | cold | ase.db | 12 | 9.8210e-01 | 1.0840e+00 | 6.8031e-02 | 0.069 |  |  | 10792960 | 527007744 | exact |
| frames-10000 | selected | cold | readcon-db | 12 | 3.1086e-04 | 4.0024e-04 | 6.6236e-05 | 0.213 |  |  | 12517376 | 93310976 | exact |
| frames-10000 | selected | cold | ase.db | 12 | 3.7899e-03 | 4.6201e-03 | 1.0557e-03 | 0.279 |  |  | 10792960 | 94564352 | exact |
| frames-10000 | range | cold | readcon-db | 12 | 1.3570e-02 | 1.5062e-02 | 3.3893e-03 | 0.250 |  |  | 12517376 | 134537216 | exact |
| frames-10000 | range | cold | ase.db | 12 | 2.2312e-01 | 2.3537e-01 | 4.8496e-03 | 0.022 |  |  | 10792960 | 203317248 | exact |
| frames-10000 | metadata | cold | readcon-db | 12 | 6.1861e-03 | 6.6725e-03 | 4.0227e-04 | 0.065 |  |  | 12517376 | 97869824 | exact |
| frames-10000 | metadata | cold | ase.db | 12 | 4.3470e-02 | 4.7224e-02 | 4.8119e-03 | 0.111 |  |  | 10792960 | 98672640 | exact |
| frames-10000 | full_read | warm | readcon-db | 12 | 7.6256e-03 | 8.1245e-03 | 4.0618e-04 | 0.053 |  |  | 12517376 | 144859136 | exact |
| frames-10000 | full_read | warm | ase.db | 12 | 9.4250e-01 | 9.6258e-01 | 1.9132e-02 | 0.020 |  |  | 10792960 | 545460224 | exact |
| frames-10000 | selected | warm | readcon-db | 12 | 1.6396e-05 | 2.0142e-05 | 6.6375e-06 | 0.405 |  |  | 12517376 | 93761536 | exact |
| frames-10000 | selected | warm | ase.db | 12 | 7.3095e-04 | 7.5418e-04 | 1.0780e-05 | 0.015 |  |  | 10792960 | 96292864 | exact |
| frames-10000 | range | warm | readcon-db | 12 | 1.9462e-03 | 1.9747e-03 | 1.9758e-05 | 0.010 |  |  | 12517376 | 132640768 | exact |
| frames-10000 | range | warm | ase.db | 12 | 2.2092e-01 | 2.3102e-01 | 1.2678e-02 | 0.057 |  |  | 10792960 | 207806464 | exact |
| frames-10000 | metadata | warm | readcon-db | 12 | 2.4903e-04 | 2.5695e-04 | 1.5334e-06 | 0.006 |  |  | 12517376 | 98209792 | exact |
| frames-10000 | metadata | warm | ase.db | 12 | 2.7210e-02 | 3.1008e-02 | 1.2386e-03 | 0.046 |  |  | 10792960 | 98553856 | exact |
| traj-100k | insert | cold-file | readcon-db | 12 | 1.3316e+00 | 1.4045e+00 | 6.5813e-02 | 0.049 | 4.8063e+01 | 2.4826e+02 | 342003712 | 2360737792 | exact |
| traj-100k | insert | cold-file | ase.db | 12 | 1.4945e+00 | 1.5124e+00 | 3.6644e-02 | 0.025 | 4.2824e+01 | 2.2120e+02 | 384503808 | 2175569920 | exact |
| traj-100k | full_read | cold | readcon-db | 12 | 6.5138e-01 | 7.8815e-01 | 9.7851e-02 | 0.150 |  |  | 342003712 | 1929674752 | exact |
| traj-100k | full_read | cold | ase.db | 12 | 2.5388e+00 | 2.8995e+00 | 3.0328e-01 | 0.119 |  |  | 384503808 | 7852228608 | exact |
| traj-100k | selected | cold | readcon-db | 12 | 7.2613e-03 | 1.2467e-02 | 4.6657e-04 | 0.064 |  |  | 342003712 | 129802240 | exact |
| traj-100k | selected | cold | ase.db | 12 | 3.7703e-02 | 3.8825e-02 | 1.2393e-03 | 0.033 |  |  | 384503808 | 292786176 | exact |
| traj-100k | range | cold | readcon-db | 12 | 1.7021e-01 | 1.9354e-01 | 2.4773e-02 | 0.146 |  |  | 342003712 | 693313536 | exact |
| traj-100k | range | cold | ase.db | 12 | 6.0231e-01 | 6.3040e-01 | 3.8964e-02 | 0.065 |  |  | 384503808 | 2186866688 | exact |
| traj-100k | metadata | cold | readcon-db | 12 | 2.6138e-04 | 2.9557e-04 | 4.4511e-05 | 0.170 |  |  | 342003712 | 93351936 | exact |
| traj-100k | metadata | cold | ase.db | 12 | 2.5949e-01 | 2.8660e-01 | 4.2341e-02 | 0.163 |  |  | 384503808 | 143228928 | exact |
| traj-100k | full_read | warm | readcon-db | 12 | 4.0475e-01 | 4.3338e-01 | 1.9105e-02 | 0.047 |  |  | 342003712 | 1526538240 | exact |
| traj-100k | full_read | warm | ase.db | 12 | 1.9826e+00 | 2.0813e+00 | 7.6674e-02 | 0.039 |  |  | 384503808 | 7742566400 | exact |
| traj-100k | selected | warm | readcon-db | 12 | 2.7676e-03 | 2.8641e-03 | 3.4680e-05 | 0.013 |  |  | 342003712 | 123752448 | exact |
| traj-100k | selected | warm | ase.db | 12 | 3.1585e-02 | 3.4815e-02 | 2.1776e-03 | 0.069 |  |  | 384503808 | 296402944 | exact |
| traj-100k | range | warm | readcon-db | 12 | 1.0057e-01 | 1.2386e-01 | 1.3953e-02 | 0.139 |  |  | 342003712 | 586682368 | exact |
| traj-100k | range | warm | ase.db | 12 | 5.1816e-01 | 5.5634e-01 | 2.2444e-02 | 0.043 |  |  | 384503808 | 2152546304 | exact |
| traj-100k | metadata | warm | readcon-db | 12 | 3.3256e-06 | 3.4413e-06 | 6.7555e-08 | 0.020 |  |  | 342003712 | 93220864 | exact |
| traj-100k | metadata | warm | ase.db | 12 | 8.2540e-02 | 8.9211e-02 | 4.8166e-03 | 0.058 |  |  | 384503808 | 143036416 | exact |

