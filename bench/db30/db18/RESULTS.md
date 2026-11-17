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
| frames-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.3653e-02 | 3.4005e-02 | 3.5481e-04 | 0.011 | 2.9715e+04 | 2.2331e+01 | 356352 | 124428288 | exact |
| frames-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.9621e-01 | 3.0840e-01 | 4.6827e-03 | 0.016 | 3.3760e+03 | 2.5371e+00 | 1146880 | 110968832 | exact |
| frames-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.3269e-02 | 3.4044e-02 | 6.9398e-04 | 0.021 | 3.0058e+04 | 2.2589e+01 | 356352 | 124309504 | exact |
| frames-1000 | full_read | cold | readcon-db | 1 | 1 | 12 | 4.3626e-04 | 4.5679e-04 | 1.7664e-05 | 0.040 |  |  | 356352 | 117977088 | exact |
| frames-1000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.0224e-02 | 9.2458e-02 | 2.3309e-03 | 0.026 |  |  | 1146880 | 142532608 | exact |
| frames-1000 | full_read | cold | readcon-db | unset | 8 | 12 | 4.4132e-04 | 4.6237e-04 | 1.6563e-05 | 0.038 |  |  | 356352 | 117989376 | exact |
| frames-1000 | selected | cold | readcon-db | 1 | 1 | 12 | 3.6178e-05 | 3.7334e-05 | 1.3898e-06 | 0.038 |  |  | 356352 | 109240320 | exact |
| frames-1000 | selected | cold | ase.db | 1 | 1 | 12 | 1.6604e-03 | 2.3398e-03 | 2.9130e-04 | 0.175 |  |  | 1146880 | 99958784 | exact |
| frames-1000 | selected | cold | readcon-db | unset | 8 | 12 | 3.7001e-05 | 3.7881e-05 | 1.5812e-06 | 0.043 |  |  | 356352 | 109215744 | exact |
| frames-1000 | range | cold | readcon-db | 1 | 1 | 12 | 1.6671e-04 | 1.6867e-04 | 1.7901e-06 | 0.011 |  |  | 356352 | 108052480 | exact |
| frames-1000 | range | cold | ase.db | 1 | 1 | 12 | 2.7534e-02 | 2.8483e-02 | 1.9386e-03 | 0.070 |  |  | 1146880 | 119050240 | exact |
| frames-1000 | range | cold | readcon-db | unset | 8 | 12 | 1.6645e-04 | 1.6865e-04 | 2.1671e-06 | 0.013 |  |  | 356352 | 108167168 | exact |
| frames-1000 | metadata | cold | readcon-db | 1 | 1 | 12 | 3.3512e-05 | 3.4274e-05 | 9.7471e-07 | 0.029 |  |  | 356352 | 105091072 | exact |
| frames-1000 | metadata | cold | ase.db | 1 | 1 | 12 | 4.4756e-03 | 5.3813e-03 | 7.5913e-04 | 0.170 |  |  | 1146880 | 99586048 | exact |
| frames-1000 | metadata | cold | readcon-db | unset | 8 | 12 | 3.2578e-05 | 3.3067e-05 | 6.4315e-07 | 0.020 |  |  | 356352 | 105160704 | exact |
| frames-1000 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.0788e-04 | 6.0993e-04 | 1.6839e-05 | 0.041 |  |  | 356352 | 110616576 | exact |
| frames-1000 | full_read | warm | ase.db | 1 | 1 | 12 | 8.5127e-02 | 8.7493e-02 | 2.0750e-03 | 0.024 |  |  | 1146880 | 144269312 | exact |
| frames-1000 | full_read | warm | readcon-db | unset | 8 | 12 | 4.0882e-04 | 4.1469e-04 | 3.9302e-06 | 0.010 |  |  | 356352 | 110530560 | exact |
| frames-1000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.3465e-06 | 9.6042e-06 | 2.2598e-07 | 0.024 |  |  | 356352 | 105095168 | exact |
| frames-1000 | selected | warm | ase.db | 1 | 1 | 12 | 2.2870e-04 | 2.3917e-04 | 6.0837e-06 | 0.027 |  |  | 1146880 | 105668608 | exact |
| frames-1000 | selected | warm | readcon-db | unset | 8 | 12 | 9.3304e-06 | 9.4129e-06 | 1.1509e-07 | 0.012 |  |  | 356352 | 105074688 | exact |
| frames-1000 | range | warm | readcon-db | 1 | 1 | 12 | 1.1097e-04 | 1.1475e-04 | 2.7817e-06 | 0.025 |  |  | 356352 | 105492480 | exact |
| frames-1000 | range | warm | ase.db | 1 | 1 | 12 | 2.4936e-02 | 2.7837e-02 | 4.7608e-03 | 0.191 |  |  | 1146880 | 123064320 | exact |
| frames-1000 | range | warm | readcon-db | unset | 8 | 12 | 1.1287e-04 | 1.1552e-04 | 1.8039e-06 | 0.016 |  |  | 356352 | 105496576 | exact |
| frames-1000 | metadata | warm | readcon-db | 1 | 1 | 12 | 1.2164e-05 | 1.2323e-05 | 1.7929e-07 | 0.015 |  |  | 356352 | 105107456 | exact |
| frames-1000 | metadata | warm | ase.db | 1 | 1 | 12 | 3.2687e-03 | 3.3422e-03 | 1.7716e-04 | 0.054 |  |  | 1146880 | 99373056 | exact |
| frames-1000 | metadata | warm | readcon-db | unset | 8 | 12 | 1.3029e-05 | 1.5708e-05 | 2.7272e-06 | 0.209 |  |  | 356352 | 105250816 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.2836e-01 | 3.3124e-01 | 6.4529e-03 | 0.020 | 3.0454e+04 | 2.3372e+01 | 3362816 | 184401920 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.9663e+00 | 3.0304e+00 | 4.0388e-02 | 0.014 | 3.3712e+03 | 2.5872e+00 | 10825728 | 200925184 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2381e-01 | 3.2689e-01 | 2.6991e-03 | 0.008 | 3.0883e+04 | 2.3701e+01 | 3362816 | 184205312 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.0660e-03 | 6.3667e-03 | 1.4062e-04 | 0.023 |  |  | 3362816 | 162938880 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.4471e-01 | 9.8224e-01 | 3.5822e-02 | 0.038 |  |  | 10801152 | 533811200 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.0199e-03 | 6.1925e-03 | 5.4575e-05 | 0.009 |  |  | 3362816 | 162942976 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.4315e-05 | 4.5169e-05 | 1.3604e-06 | 0.031 |  |  | 3362816 | 117456896 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 4.3821e-03 | 5.1480e-03 | 6.0668e-04 | 0.138 |  |  | 10801152 | 99524608 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.4296e-05 | 4.5536e-05 | 7.1271e-07 | 0.016 |  |  | 3362816 | 117293056 | exact |
| frames-10000 | range | cold | readcon-db | 1 | 1 | 12 | 1.7100e-03 | 1.7764e-03 | 9.3675e-05 | 0.055 |  |  | 3362816 | 143835136 | exact |
| frames-10000 | range | cold | ase.db | 1 | 1 | 12 | 2.3712e-01 | 2.4141e-01 | 8.6922e-03 | 0.037 |  |  | 10801152 | 209182720 | exact |
| frames-10000 | range | cold | readcon-db | unset | 8 | 12 | 1.7464e-03 | 1.9117e-03 | 5.9564e-05 | 0.034 |  |  | 3362816 | 143826944 | exact |
| frames-10000 | metadata | cold | readcon-db | 1 | 1 | 12 | 1.4398e-04 | 1.4709e-04 | 2.8892e-06 | 0.020 |  |  | 3362816 | 117727232 | exact |
| frames-10000 | metadata | cold | ase.db | 1 | 1 | 12 | 4.0038e-02 | 4.2201e-02 | 1.8124e-03 | 0.045 |  |  | 10801152 | 104226816 | exact |
| frames-10000 | metadata | cold | readcon-db | unset | 8 | 12 | 1.4550e-04 | 1.5051e-04 | 2.4356e-06 | 0.017 |  |  | 3362816 | 117649408 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.3289e-03 | 6.9633e-03 | 5.4676e-04 | 0.086 |  |  | 3362816 | 177684480 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 9.3449e-01 | 9.6316e-01 | 2.7232e-02 | 0.029 |  |  | 10801152 | 552484864 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.1452e-03 | 6.8122e-03 | 2.2213e-04 | 0.036 |  |  | 3362816 | 177704960 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.9697e-06 | 1.0058e-05 | 3.0955e-08 | 0.003 |  |  | 3362816 | 113283072 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.4459e-04 | 7.6204e-04 | 8.9503e-06 | 0.012 |  |  | 10801152 | 100913152 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 9.4029e-06 | 9.5204e-06 | 7.2947e-08 | 0.008 |  |  | 3362816 | 113307648 | exact |
| frames-10000 | range | warm | readcon-db | 1 | 1 | 12 | 1.6600e-03 | 1.6733e-03 | 1.9160e-05 | 0.012 |  |  | 3362816 | 128663552 | exact |
| frames-10000 | range | warm | ase.db | 1 | 1 | 12 | 2.2257e-01 | 2.5447e-01 | 1.5572e-02 | 0.070 |  |  | 10801152 | 213610496 | exact |
| frames-10000 | range | warm | readcon-db | unset | 8 | 12 | 1.6758e-03 | 1.6893e-03 | 1.1770e-05 | 0.007 |  |  | 3362816 | 128512000 | exact |
| frames-10000 | metadata | warm | readcon-db | 1 | 1 | 12 | 9.4907e-05 | 9.5296e-05 | 3.4848e-07 | 0.004 |  |  | 3362816 | 113475584 | exact |
| frames-10000 | metadata | warm | ase.db | 1 | 1 | 12 | 2.8249e-02 | 2.8687e-02 | 1.8063e-04 | 0.006 |  |  | 10801152 | 104148992 | exact |
| frames-10000 | metadata | warm | readcon-db | unset | 8 | 12 | 9.5583e-05 | 9.7425e-05 | 1.4221e-06 | 0.015 |  |  | 3362816 | 113537024 | exact |
| traj-100k | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.3019e+00 | 2.5216e+00 | 2.2567e-01 | 0.098 | 2.7803e+01 | 1.4361e+02 | 341966848 | 2659766272 | exact |
| traj-100k | insert | cold-file | ase.db | 1 | 1 | 12 | 1.4059e+00 | 1.4917e+00 | 7.7174e-02 | 0.055 | 4.5522e+01 | 2.3513e+02 | 384503808 | 2138968064 | exact |
| traj-100k | insert | cold-file | readcon-db | unset | 8 | 12 | 2.2505e+00 | 2.4202e+00 | 7.6765e-02 | 0.034 | 2.8439e+01 | 1.4689e+02 | 341966848 | 2797092864 | exact |
| traj-100k | full_read | cold | readcon-db | 1 | 1 | 12 | 4.9413e-01 | 5.1675e-01 | 2.0183e-02 | 0.041 |  |  | 341966848 | 1935740928 | exact |
| traj-100k | full_read | cold | ase.db | 1 | 1 | 12 | 2.1280e+00 | 2.3394e+00 | 2.1226e-01 | 0.100 |  |  | 384503808 | 7876829184 | exact |
| traj-100k | full_read | cold | readcon-db | unset | 8 | 12 | 5.4177e-01 | 6.1508e-01 | 8.9841e-02 | 0.166 |  |  | 341966848 | 1935400960 | exact |
| traj-100k | selected | cold | readcon-db | 1 | 1 | 12 | 7.0809e-03 | 7.6465e-03 | 7.2322e-04 | 0.102 |  |  | 341966848 | 134942720 | exact |
| traj-100k | selected | cold | ase.db | 1 | 1 | 12 | 3.6715e-02 | 4.0093e-02 | 3.3494e-03 | 0.091 |  |  | 384503808 | 303230976 | exact |
| traj-100k | selected | cold | readcon-db | unset | 8 | 12 | 8.2432e-03 | 8.7794e-03 | 8.0850e-04 | 0.098 |  |  | 341966848 | 135049216 | exact |
| traj-100k | range | cold | readcon-db | 1 | 1 | 12 | 1.3439e-01 | 1.5753e-01 | 2.3032e-02 | 0.171 |  |  | 341966848 | 701079552 | exact |
| traj-100k | range | cold | ase.db | 1 | 1 | 12 | 5.3593e-01 | 5.5658e-01 | 2.8453e-02 | 0.053 |  |  | 384503808 | 2188677120 | exact |
| traj-100k | range | cold | readcon-db | unset | 8 | 12 | 1.3676e-01 | 1.5842e-01 | 2.8950e-02 | 0.212 |  |  | 341966848 | 701198336 | exact |
| traj-100k | metadata | cold | readcon-db | 1 | 1 | 12 | 1.6754e-05 | 2.2341e-05 | 2.0750e-06 | 0.124 |  |  | 341966848 | 98394112 | exact |
| traj-100k | metadata | cold | ase.db | 1 | 1 | 12 | 1.8512e-01 | 2.7913e-01 | 2.1578e-02 | 0.117 |  |  | 384503808 | 148172800 | exact |
| traj-100k | metadata | cold | readcon-db | unset | 8 | 12 | 1.6178e-05 | 1.8451e-05 | 2.1244e-06 | 0.131 |  |  | 341966848 | 98365440 | exact |
| traj-100k | full_read | warm | readcon-db | 1 | 1 | 12 | 3.0132e-01 | 4.0628e-01 | 7.3335e-02 | 0.243 |  |  | 341966848 | 1532497920 | exact |
| traj-100k | full_read | warm | ase.db | 1 | 1 | 12 | 1.9888e+00 | 2.4078e+00 | 3.0354e-01 | 0.153 |  |  | 384503808 | 7737270272 | exact |
| traj-100k | full_read | warm | readcon-db | unset | 8 | 12 | 3.0505e-01 | 3.4655e-01 | 6.3835e-02 | 0.209 |  |  | 341966848 | 1534910464 | exact |
| traj-100k | selected | warm | readcon-db | 1 | 1 | 12 | 2.4598e-03 | 2.5548e-03 | 2.6342e-05 | 0.011 |  |  | 341966848 | 128925696 | exact |
| traj-100k | selected | warm | ase.db | 1 | 1 | 12 | 2.8523e-02 | 2.9006e-02 | 2.8174e-04 | 0.010 |  |  | 384503808 | 307023872 | exact |
| traj-100k | selected | warm | readcon-db | unset | 8 | 12 | 2.5444e-03 | 2.6334e-03 | 4.8019e-05 | 0.019 |  |  | 341966848 | 128782336 | exact |
| traj-100k | range | warm | readcon-db | 1 | 1 | 12 | 5.7996e-02 | 6.2594e-02 | 3.9433e-03 | 0.068 |  |  | 341966848 | 594305024 | exact |
| traj-100k | range | warm | ase.db | 1 | 1 | 12 | 5.3555e-01 | 6.0050e-01 | 3.2827e-02 | 0.061 |  |  | 384503808 | 2157412352 | exact |
| traj-100k | range | warm | readcon-db | unset | 8 | 12 | 6.6226e-02 | 8.4978e-02 | 1.0955e-02 | 0.165 |  |  | 341966848 | 594083840 | exact |
| traj-100k | metadata | warm | readcon-db | 1 | 1 | 12 | 2.4261e-06 | 2.4522e-06 | 1.4359e-08 | 0.006 |  |  | 341966848 | 98349056 | exact |
| traj-100k | metadata | warm | ase.db | 1 | 1 | 12 | 5.4697e-02 | 5.8183e-02 | 1.0534e-03 | 0.019 |  |  | 384503808 | 148168704 | exact |
| traj-100k | metadata | warm | readcon-db | unset | 8 | 12 | 2.4842e-06 | 2.5717e-06 | 7.7437e-08 | 0.031 |  |  | 341966848 | 98406400 | exact |

