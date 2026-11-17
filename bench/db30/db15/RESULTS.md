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
| frames-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.3658e-02 | 3.4285e-02 | 5.8191e-04 | 0.017 | 2.9710e+04 | 2.2327e+01 | 356352 | 124051456 | exact |
| frames-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.0844e-01 | 3.2748e-01 | 1.4767e-02 | 0.048 | 3.2421e+03 | 2.4364e+00 | 1138688 | 110854144 | exact |
| frames-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.3712e-02 | 3.3954e-02 | 4.3817e-04 | 0.013 | 2.9663e+04 | 2.2291e+01 | 356352 | 124551168 | exact |
| frames-1000 | full_read | cold | readcon-db | 1 | 1 | 12 | 4.7063e-04 | 5.1317e-04 | 5.1726e-05 | 0.110 |  |  | 356352 | 117923840 | exact |
| frames-1000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.0209e-02 | 9.2938e-02 | 2.3231e-03 | 0.026 |  |  | 1146880 | 142618624 | exact |
| frames-1000 | full_read | cold | readcon-db | unset | 8 | 12 | 4.5163e-04 | 4.6819e-04 | 9.7777e-06 | 0.022 |  |  | 356352 | 117989376 | exact |
| frames-1000 | selected | cold | readcon-db | 1 | 1 | 12 | 3.6592e-05 | 3.7420e-05 | 9.0635e-07 | 0.025 |  |  | 356352 | 109252608 | exact |
| frames-1000 | selected | cold | ase.db | 1 | 1 | 12 | 1.0526e-03 | 1.2065e-03 | 1.7636e-04 | 0.168 |  |  | 1146880 | 101183488 | exact |
| frames-1000 | selected | cold | readcon-db | unset | 8 | 12 | 3.6881e-05 | 3.7389e-05 | 8.6020e-07 | 0.023 |  |  | 356352 | 109428736 | exact |
| frames-1000 | range | cold | readcon-db | 1 | 1 | 12 | 1.6665e-04 | 1.8501e-04 | 7.7444e-06 | 0.046 |  |  | 356352 | 108249088 | exact |
| frames-1000 | range | cold | ase.db | 1 | 1 | 12 | 2.6373e-02 | 2.7945e-02 | 1.8256e-03 | 0.069 |  |  | 1146880 | 119824384 | exact |
| frames-1000 | range | cold | readcon-db | unset | 8 | 12 | 1.6595e-04 | 1.6748e-04 | 9.9107e-07 | 0.006 |  |  | 356352 | 108118016 | exact |
| frames-1000 | metadata | cold | readcon-db | 1 | 1 | 12 | 3.3944e-05 | 3.6093e-05 | 1.1861e-06 | 0.035 |  |  | 356352 | 104976384 | exact |
| frames-1000 | metadata | cold | ase.db | 1 | 1 | 12 | 4.7428e-03 | 5.0618e-03 | 8.4206e-04 | 0.178 |  |  | 1146880 | 99635200 | exact |
| frames-1000 | metadata | cold | readcon-db | unset | 8 | 12 | 3.2784e-05 | 3.3690e-05 | 8.2961e-07 | 0.025 |  |  | 356352 | 105123840 | exact |
| frames-1000 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.1744e-04 | 4.5907e-04 | 3.1948e-05 | 0.077 |  |  | 356352 | 110522368 | exact |
| frames-1000 | full_read | warm | ase.db | 1 | 1 | 12 | 9.2627e-02 | 9.6386e-02 | 3.4048e-03 | 0.037 |  |  | 1146880 | 144265216 | exact |
| frames-1000 | full_read | warm | readcon-db | unset | 8 | 12 | 4.5253e-04 | 4.7005e-04 | 1.1225e-05 | 0.025 |  |  | 356352 | 110690304 | exact |
| frames-1000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.5636e-06 | 9.9912e-06 | 4.7902e-07 | 0.050 |  |  | 356352 | 105086976 | exact |
| frames-1000 | selected | warm | ase.db | 1 | 1 | 12 | 2.2858e-04 | 2.4175e-04 | 6.8249e-06 | 0.030 |  |  | 1146880 | 105779200 | exact |
| frames-1000 | selected | warm | readcon-db | unset | 8 | 12 | 9.4434e-06 | 9.6309e-06 | 1.0952e-07 | 0.012 |  |  | 356352 | 104980480 | exact |
| frames-1000 | range | warm | readcon-db | 1 | 1 | 12 | 1.1174e-04 | 1.1334e-04 | 6.4622e-07 | 0.006 |  |  | 356352 | 105398272 | exact |
| frames-1000 | range | warm | ase.db | 1 | 1 | 12 | 2.2053e-02 | 2.3077e-02 | 2.7529e-04 | 0.012 |  |  | 1146880 | 127303680 | exact |
| frames-1000 | range | warm | readcon-db | unset | 8 | 12 | 1.1083e-04 | 1.1281e-04 | 1.2764e-06 | 0.012 |  |  | 356352 | 105168896 | exact |
| frames-1000 | metadata | warm | readcon-db | 1 | 1 | 12 | 1.2145e-05 | 1.2223e-05 | 4.0925e-08 | 0.003 |  |  | 356352 | 105115648 | exact |
| frames-1000 | metadata | warm | ase.db | 1 | 1 | 12 | 2.7863e-03 | 2.8409e-03 | 2.2113e-05 | 0.008 |  |  | 1146880 | 99430400 | exact |
| frames-1000 | metadata | warm | readcon-db | unset | 8 | 12 | 1.2546e-05 | 1.3679e-05 | 7.8177e-07 | 0.062 |  |  | 356352 | 104935424 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.3023e-01 | 3.3527e-01 | 5.6355e-03 | 0.017 | 3.0282e+04 | 2.3240e+01 | 3362816 | 194629632 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.0057e+00 | 3.0981e+00 | 9.3293e-02 | 0.031 | 3.3270e+03 | 2.5533e+00 | 10809344 | 200892416 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2686e-01 | 3.2798e-01 | 1.9581e-03 | 0.006 | 3.0594e+04 | 2.3480e+01 | 3362816 | 189673472 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.2346e-03 | 6.5585e-03 | 1.5656e-04 | 0.025 |  |  | 3362816 | 162861056 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.8297e-01 | 1.0122e+00 | 3.5512e-02 | 0.036 |  |  | 10792960 | 533897216 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.0607e-03 | 6.3439e-03 | 7.8271e-05 | 0.013 |  |  | 3362816 | 163020800 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.5213e-05 | 4.6108e-05 | 7.0770e-07 | 0.016 |  |  | 3362816 | 117379072 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 3.4724e-03 | 4.0797e-03 | 4.8472e-04 | 0.140 |  |  | 10792960 | 99545088 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.4161e-05 | 4.5917e-05 | 8.3999e-07 | 0.019 |  |  | 3362816 | 117395456 | exact |
| frames-10000 | range | cold | readcon-db | 1 | 1 | 12 | 1.7705e-03 | 1.8442e-03 | 6.9489e-05 | 0.039 |  |  | 3362816 | 143888384 | exact |
| frames-10000 | range | cold | ase.db | 1 | 1 | 12 | 2.3232e-01 | 2.3687e-01 | 4.3693e-03 | 0.019 |  |  | 10792960 | 209104896 | exact |
| frames-10000 | range | cold | readcon-db | unset | 8 | 12 | 1.7815e-03 | 1.8879e-03 | 5.8783e-05 | 0.033 |  |  | 3362816 | 143704064 | exact |
| frames-10000 | metadata | cold | readcon-db | 1 | 1 | 12 | 1.4614e-04 | 1.4763e-04 | 2.3744e-06 | 0.016 |  |  | 3362816 | 117608448 | exact |
| frames-10000 | metadata | cold | ase.db | 1 | 1 | 12 | 3.7805e-02 | 4.5962e-02 | 2.7245e-03 | 0.072 |  |  | 10792960 | 104189952 | exact |
| frames-10000 | metadata | cold | readcon-db | unset | 8 | 12 | 1.4460e-04 | 1.5340e-04 | 4.6783e-06 | 0.032 |  |  | 3362816 | 117690368 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.4510e-03 | 6.7839e-03 | 2.3906e-04 | 0.037 |  |  | 3362816 | 177668096 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 9.2996e-01 | 9.5149e-01 | 9.6217e-03 | 0.010 |  |  | 10792960 | 552308736 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.2657e-03 | 6.7535e-03 | 1.7843e-04 | 0.028 |  |  | 3362816 | 177889280 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.4863e-06 | 9.5829e-06 | 3.9688e-08 | 0.004 |  |  | 3362816 | 113266688 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.3365e-04 | 7.4542e-04 | 8.3044e-06 | 0.011 |  |  | 10792960 | 101003264 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 1.0113e-05 | 1.0170e-05 | 9.6588e-08 | 0.010 |  |  | 3362816 | 113274880 | exact |
| frames-10000 | range | warm | readcon-db | 1 | 1 | 12 | 1.6755e-03 | 1.7060e-03 | 2.7163e-05 | 0.016 |  |  | 3362816 | 128634880 | exact |
| frames-10000 | range | warm | ase.db | 1 | 1 | 12 | 2.1603e-01 | 2.1956e-01 | 2.7081e-03 | 0.013 |  |  | 10792960 | 213434368 | exact |
| frames-10000 | range | warm | readcon-db | unset | 8 | 12 | 1.6713e-03 | 1.6843e-03 | 1.9705e-05 | 0.012 |  |  | 3362816 | 128565248 | exact |
| frames-10000 | metadata | warm | readcon-db | 1 | 1 | 12 | 9.5736e-05 | 9.6175e-05 | 4.2368e-07 | 0.004 |  |  | 3362816 | 113479680 | exact |
| frames-10000 | metadata | warm | ase.db | 1 | 1 | 12 | 2.8745e-02 | 2.9438e-02 | 5.2110e-04 | 0.018 |  |  | 10792960 | 103350272 | exact |
| frames-10000 | metadata | warm | readcon-db | unset | 8 | 12 | 9.5807e-05 | 9.7845e-05 | 1.2993e-06 | 0.014 |  |  | 3362816 | 113528832 | exact |
| traj-100k | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.3232e+00 | 2.4228e+00 | 7.0926e-02 | 0.031 | 2.7549e+01 | 1.4230e+02 | 341966848 | 2660773888 | exact |
| traj-100k | insert | cold-file | ase.db | 1 | 1 | 12 | 1.4369e+00 | 1.4677e+00 | 5.6279e-02 | 0.039 | 4.4539e+01 | 2.3006e+02 | 384503808 | 2142130176 | exact |
| traj-100k | insert | cold-file | readcon-db | unset | 8 | 12 | 2.2625e+00 | 2.3071e+00 | 7.4880e-02 | 0.033 | 2.8287e+01 | 1.4611e+02 | 341966848 | 2613215232 | exact |
| traj-100k | full_read | cold | readcon-db | 1 | 1 | 12 | 4.8190e-01 | 7.1398e-01 | 7.6298e-02 | 0.158 |  |  | 341966848 | 1935851520 | exact |
| traj-100k | full_read | cold | ase.db | 1 | 1 | 12 | 2.1765e+00 | 2.3243e+00 | 2.7681e-01 | 0.127 |  |  | 384503808 | 7869247488 | exact |
| traj-100k | full_read | cold | readcon-db | unset | 8 | 12 | 4.9743e-01 | 6.1802e-01 | 5.1463e-02 | 0.103 |  |  | 341966848 | 1935626240 | exact |
| traj-100k | selected | cold | readcon-db | 1 | 1 | 12 | 7.4035e-03 | 8.3774e-03 | 1.9199e-03 | 0.259 |  |  | 341966848 | 135028736 | exact |
| traj-100k | selected | cold | ase.db | 1 | 1 | 12 | 3.3973e-02 | 3.6972e-02 | 2.0031e-03 | 0.059 |  |  | 384503808 | 297652224 | exact |
| traj-100k | selected | cold | readcon-db | unset | 8 | 12 | 7.7951e-03 | 8.5039e-03 | 8.2565e-04 | 0.106 |  |  | 341966848 | 134983680 | exact |
| traj-100k | range | cold | readcon-db | 1 | 1 | 12 | 1.2272e-01 | 1.6729e-01 | 1.9769e-02 | 0.161 |  |  | 341966848 | 701169664 | exact |
| traj-100k | range | cold | ase.db | 1 | 1 | 12 | 5.3232e-01 | 5.8391e-01 | 1.7480e-02 | 0.033 |  |  | 384503808 | 2183852032 | exact |
| traj-100k | range | cold | readcon-db | unset | 8 | 12 | 1.3452e-01 | 1.4677e-01 | 1.4979e-02 | 0.111 |  |  | 341966848 | 701186048 | exact |
| traj-100k | metadata | cold | readcon-db | 1 | 1 | 12 | 1.7786e-05 | 2.0126e-05 | 1.7210e-06 | 0.097 |  |  | 341966848 | 98512896 | exact |
| traj-100k | metadata | cold | ase.db | 1 | 1 | 12 | 2.0752e-01 | 2.3375e-01 | 3.1306e-02 | 0.151 |  |  | 384503808 | 148045824 | exact |
| traj-100k | metadata | cold | readcon-db | unset | 8 | 12 | 1.5996e-05 | 1.6910e-05 | 1.0630e-06 | 0.066 |  |  | 341966848 | 98590720 | exact |
| traj-100k | full_read | warm | readcon-db | 1 | 1 | 12 | 2.5311e-01 | 2.8733e-01 | 5.6599e-02 | 0.224 |  |  | 341966848 | 1532788736 | exact |
| traj-100k | full_read | warm | ase.db | 1 | 1 | 12 | 2.0730e+00 | 2.2262e+00 | 2.0431e-01 | 0.099 |  |  | 384503808 | 7751614464 | exact |
| traj-100k | full_read | warm | readcon-db | unset | 8 | 12 | 2.5698e-01 | 2.9510e-01 | 5.3830e-02 | 0.209 |  |  | 341966848 | 1534889984 | exact |
| traj-100k | selected | warm | readcon-db | 1 | 1 | 12 | 2.4635e-03 | 2.5589e-03 | 3.6333e-05 | 0.015 |  |  | 341966848 | 128798720 | exact |
| traj-100k | selected | warm | ase.db | 1 | 1 | 12 | 3.0924e-02 | 3.1642e-02 | 8.4100e-04 | 0.027 |  |  | 384503808 | 307195904 | exact |
| traj-100k | selected | warm | readcon-db | unset | 8 | 12 | 2.5618e-03 | 2.6364e-03 | 5.5275e-05 | 0.022 |  |  | 341966848 | 128815104 | exact |
| traj-100k | range | warm | readcon-db | 1 | 1 | 12 | 6.5562e-02 | 8.8409e-02 | 4.7408e-03 | 0.072 |  |  | 341966848 | 594178048 | exact |
| traj-100k | range | warm | ase.db | 1 | 1 | 12 | 4.9354e-01 | 4.9818e-01 | 6.4513e-03 | 0.013 |  |  | 384503808 | 2153574400 | exact |
| traj-100k | range | warm | readcon-db | unset | 8 | 12 | 6.5079e-02 | 8.8150e-02 | 8.4529e-03 | 0.130 |  |  | 341966848 | 594259968 | exact |
| traj-100k | metadata | warm | readcon-db | 1 | 1 | 12 | 2.4406e-06 | 2.5625e-06 | 6.3752e-08 | 0.026 |  |  | 341966848 | 98316288 | exact |
| traj-100k | metadata | warm | ase.db | 1 | 1 | 12 | 6.0053e-02 | 6.1287e-02 | 6.2677e-04 | 0.010 |  |  | 384503808 | 148148224 | exact |
| traj-100k | metadata | warm | readcon-db | unset | 8 | 12 | 2.4514e-06 | 2.4592e-06 | 1.6479e-08 | 0.007 |  |  | 341966848 | 98308096 | exact |

