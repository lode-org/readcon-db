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
| frames-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.3803e-02 | 3.4083e-02 | 6.4076e-04 | 0.019 | 2.9583e+04 | 2.2232e+01 | 356352 | 124551168 | exact |
| frames-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.9999e-01 | 3.0160e-01 | 5.8363e-03 | 0.019 | 3.3335e+03 | 2.5051e+00 | 1146880 | 110817280 | exact |
| frames-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.3280e-02 | 3.3532e-02 | 3.3341e-04 | 0.010 | 3.0048e+04 | 2.2581e+01 | 356352 | 124391424 | exact |
| frames-1000 | full_read | cold | readcon-db | 1 | 1 | 12 | 4.4284e-04 | 4.4727e-04 | 6.9870e-06 | 0.016 |  |  | 356352 | 118054912 | exact |
| frames-1000 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0068e-01 | 1.0568e-01 | 5.0293e-03 | 0.050 |  |  | 1146880 | 142692352 | exact |
| frames-1000 | full_read | cold | readcon-db | unset | 8 | 12 | 4.4534e-04 | 4.5073e-04 | 3.3148e-06 | 0.007 |  |  | 356352 | 117981184 | exact |
| frames-1000 | selected | cold | readcon-db | 1 | 1 | 12 | 3.6322e-05 | 3.6869e-05 | 7.0134e-07 | 0.019 |  |  | 356352 | 109350912 | exact |
| frames-1000 | selected | cold | ase.db | 1 | 1 | 12 | 1.3164e-03 | 1.7438e-03 | 2.3069e-04 | 0.175 |  |  | 1146880 | 100397056 | exact |
| frames-1000 | selected | cold | readcon-db | unset | 8 | 12 | 3.6072e-05 | 3.6766e-05 | 7.1255e-07 | 0.020 |  |  | 356352 | 109170688 | exact |
| frames-1000 | range | cold | readcon-db | 1 | 1 | 12 | 1.6772e-04 | 1.7003e-04 | 4.5522e-06 | 0.027 |  |  | 356352 | 108208128 | exact |
| frames-1000 | range | cold | ase.db | 1 | 1 | 12 | 2.8558e-02 | 3.0205e-02 | 2.1369e-03 | 0.075 |  |  | 1146880 | 118272000 | exact |
| frames-1000 | range | cold | readcon-db | unset | 8 | 12 | 1.6802e-04 | 1.7611e-04 | 5.0723e-06 | 0.030 |  |  | 356352 | 108081152 | exact |
| frames-1000 | metadata | cold | readcon-db | 1 | 1 | 12 | 3.3337e-05 | 3.4097e-05 | 6.7884e-07 | 0.020 |  |  | 356352 | 105119744 | exact |
| frames-1000 | metadata | cold | ase.db | 1 | 1 | 12 | 5.5537e-03 | 6.6860e-03 | 1.1380e-03 | 0.205 |  |  | 1146880 | 99524608 | exact |
| frames-1000 | metadata | cold | readcon-db | unset | 8 | 12 | 3.2932e-05 | 3.4654e-05 | 1.0479e-06 | 0.032 |  |  | 356352 | 105086976 | exact |
| frames-1000 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.2744e-04 | 6.5833e-04 | 4.5631e-05 | 0.107 |  |  | 356352 | 110559232 | exact |
| frames-1000 | full_read | warm | ase.db | 1 | 1 | 12 | 9.0838e-02 | 9.2493e-02 | 2.7058e-03 | 0.030 |  |  | 1146880 | 144183296 | exact |
| frames-1000 | full_read | warm | readcon-db | unset | 8 | 12 | 4.0223e-04 | 4.0825e-04 | 2.7564e-06 | 0.007 |  |  | 356352 | 110575616 | exact |
| frames-1000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.3556e-06 | 9.5461e-06 | 1.3147e-07 | 0.014 |  |  | 356352 | 105111552 | exact |
| frames-1000 | selected | warm | ase.db | 1 | 1 | 12 | 2.2813e-04 | 2.3251e-04 | 4.1291e-06 | 0.018 |  |  | 1146880 | 105734144 | exact |
| frames-1000 | selected | warm | readcon-db | unset | 8 | 12 | 9.4293e-06 | 1.1005e-05 | 1.9868e-07 | 0.021 |  |  | 356352 | 105013248 | exact |
| frames-1000 | range | warm | readcon-db | 1 | 1 | 12 | 1.1122e-04 | 1.1356e-04 | 9.1384e-07 | 0.008 |  |  | 356352 | 105340928 | exact |
| frames-1000 | range | warm | ase.db | 1 | 1 | 12 | 2.1538e-02 | 2.1729e-02 | 2.2361e-04 | 0.010 |  |  | 1146880 | 127340544 | exact |
| frames-1000 | range | warm | readcon-db | unset | 8 | 12 | 1.1345e-04 | 1.2280e-04 | 6.1346e-06 | 0.054 |  |  | 356352 | 105549824 | exact |
| frames-1000 | metadata | warm | readcon-db | 1 | 1 | 12 | 1.2295e-05 | 1.2622e-05 | 1.7238e-07 | 0.014 |  |  | 356352 | 104976384 | exact |
| frames-1000 | metadata | warm | ase.db | 1 | 1 | 12 | 2.7941e-03 | 2.8737e-03 | 2.8903e-05 | 0.010 |  |  | 1146880 | 99454976 | exact |
| frames-1000 | metadata | warm | readcon-db | unset | 8 | 12 | 1.2281e-05 | 1.2350e-05 | 9.2175e-08 | 0.008 |  |  | 356352 | 105119744 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.2502e-01 | 3.3344e-01 | 5.8300e-03 | 0.018 | 3.0767e+04 | 2.3612e+01 | 3362816 | 206532608 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.1151e+00 | 3.2109e+00 | 1.1349e-01 | 0.036 | 3.2102e+03 | 2.4637e+00 | 10825728 | 201019392 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2350e-01 | 3.2585e-01 | 3.0174e-03 | 0.009 | 3.0912e+04 | 2.3724e+01 | 3362816 | 200740864 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.1316e-03 | 6.8644e-03 | 2.6445e-04 | 0.043 |  |  | 3362816 | 162820096 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0230e+00 | 1.0396e+00 | 3.0622e-02 | 0.030 |  |  | 10850304 | 533815296 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.1735e-03 | 6.4004e-03 | 6.4971e-05 | 0.011 |  |  | 3362816 | 162988032 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.5582e-05 | 4.6063e-05 | 8.4077e-07 | 0.018 |  |  | 3362816 | 117370880 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 5.4173e-03 | 6.9894e-03 | 2.1095e-03 | 0.389 |  |  | 10850304 | 99295232 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.5021e-05 | 4.5834e-05 | 7.0476e-07 | 0.016 |  |  | 3362816 | 117485568 | exact |
| frames-10000 | range | cold | readcon-db | 1 | 1 | 12 | 1.7861e-03 | 1.8429e-03 | 5.3554e-05 | 0.030 |  |  | 3362816 | 143749120 | exact |
| frames-10000 | range | cold | ase.db | 1 | 1 | 12 | 2.5182e-01 | 2.6467e-01 | 5.5161e-03 | 0.022 |  |  | 10850304 | 209166336 | exact |
| frames-10000 | range | cold | readcon-db | unset | 8 | 12 | 1.7941e-03 | 2.0389e-03 | 8.1473e-05 | 0.045 |  |  | 3362816 | 143831040 | exact |
| frames-10000 | metadata | cold | readcon-db | 1 | 1 | 12 | 1.4863e-04 | 1.5286e-04 | 6.3129e-06 | 0.042 |  |  | 3362816 | 117800960 | exact |
| frames-10000 | metadata | cold | ase.db | 1 | 1 | 12 | 4.1027e-02 | 4.3108e-02 | 2.3219e-03 | 0.057 |  |  | 10850304 | 104210432 | exact |
| frames-10000 | metadata | cold | readcon-db | unset | 8 | 12 | 1.4991e-04 | 1.5277e-04 | 6.1642e-06 | 0.041 |  |  | 3362816 | 117690368 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.3155e-03 | 7.0154e-03 | 2.6385e-04 | 0.042 |  |  | 3362816 | 177750016 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 9.6737e-01 | 9.8273e-01 | 1.8647e-02 | 0.019 |  |  | 10850304 | 552570880 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.1495e-03 | 6.4629e-03 | 1.5136e-04 | 0.025 |  |  | 3362816 | 177422336 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 1.0183e-05 | 1.0796e-05 | 1.2161e-07 | 0.012 |  |  | 3362816 | 113262592 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.3179e-04 | 7.3596e-04 | 2.4801e-06 | 0.003 |  |  | 10850304 | 101175296 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 9.5027e-06 | 9.8285e-06 | 1.3092e-07 | 0.014 |  |  | 3362816 | 113164288 | exact |
| frames-10000 | range | warm | readcon-db | 1 | 1 | 12 | 1.3046e-03 | 1.6551e-03 | 7.4502e-05 | 0.057 |  |  | 3362816 | 129118208 | exact |
| frames-10000 | range | warm | ase.db | 1 | 1 | 12 | 2.3395e-01 | 2.3798e-01 | 4.9123e-03 | 0.021 |  |  | 10850304 | 213508096 | exact |
| frames-10000 | range | warm | readcon-db | unset | 8 | 12 | 1.6676e-03 | 1.6920e-03 | 2.0944e-05 | 0.013 |  |  | 3362816 | 128745472 | exact |
| frames-10000 | metadata | warm | readcon-db | 1 | 1 | 12 | 9.6186e-05 | 1.0978e-04 | 4.5395e-06 | 0.047 |  |  | 3362816 | 113614848 | exact |
| frames-10000 | metadata | warm | ase.db | 1 | 1 | 12 | 2.8499e-02 | 2.8629e-02 | 1.5215e-04 | 0.005 |  |  | 10850304 | 103219200 | exact |
| frames-10000 | metadata | warm | readcon-db | unset | 8 | 12 | 9.5765e-05 | 9.6470e-05 | 9.1023e-07 | 0.010 |  |  | 3362816 | 113631232 | exact |
| traj-100k | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.3928e+00 | 2.5125e+00 | 2.0507e-01 | 0.086 | 2.6746e+01 | 1.3815e+02 | 341966848 | 2712272896 | exact |
| traj-100k | insert | cold-file | ase.db | 1 | 1 | 12 | 1.3797e+00 | 1.3954e+00 | 2.6224e-02 | 0.019 | 4.6389e+01 | 2.3961e+02 | 384503808 | 2140696576 | exact |
| traj-100k | insert | cold-file | readcon-db | unset | 8 | 12 | 2.2459e+00 | 2.3842e+00 | 9.3832e-02 | 0.042 | 2.8497e+01 | 1.4719e+02 | 341966848 | 2610089984 | exact |
| traj-100k | full_read | cold | readcon-db | 1 | 1 | 12 | 5.1748e-01 | 6.3685e-01 | 3.0129e-02 | 0.058 |  |  | 341966848 | 1935695872 | exact |
| traj-100k | full_read | cold | ase.db | 1 | 1 | 12 | 2.1266e+00 | 2.2802e+00 | 1.2189e-01 | 0.057 |  |  | 384503808 | 7866773504 | exact |
| traj-100k | full_read | cold | readcon-db | unset | 8 | 12 | 5.0528e-01 | 5.8166e-01 | 8.7188e-02 | 0.173 |  |  | 341966848 | 1935732736 | exact |
| traj-100k | selected | cold | readcon-db | 1 | 1 | 12 | 6.6804e-03 | 7.1775e-03 | 7.1637e-04 | 0.107 |  |  | 341966848 | 135061504 | exact |
| traj-100k | selected | cold | ase.db | 1 | 1 | 12 | 3.6683e-02 | 3.7414e-02 | 1.2864e-03 | 0.035 |  |  | 384503808 | 298598400 | exact |
| traj-100k | selected | cold | readcon-db | unset | 8 | 12 | 8.0215e-03 | 8.3972e-03 | 4.6959e-04 | 0.059 |  |  | 341966848 | 135348224 | exact |
| traj-100k | range | cold | readcon-db | 1 | 1 | 12 | 1.3298e-01 | 1.5323e-01 | 1.7630e-02 | 0.133 |  |  | 341966848 | 701243392 | exact |
| traj-100k | range | cold | ase.db | 1 | 1 | 12 | 5.4117e-01 | 5.5764e-01 | 2.0688e-02 | 0.038 |  |  | 384503808 | 2180669440 | exact |
| traj-100k | range | cold | readcon-db | unset | 8 | 12 | 1.3502e-01 | 1.5912e-01 | 9.0194e-03 | 0.067 |  |  | 341966848 | 701292544 | exact |
| traj-100k | metadata | cold | readcon-db | 1 | 1 | 12 | 1.4325e-05 | 1.4829e-05 | 5.5753e-07 | 0.039 |  |  | 341966848 | 98746368 | exact |
| traj-100k | metadata | cold | ase.db | 1 | 1 | 12 | 2.4478e-01 | 2.8899e-01 | 4.5637e-02 | 0.186 |  |  | 384503808 | 148160512 | exact |
| traj-100k | metadata | cold | readcon-db | unset | 8 | 12 | 1.6227e-05 | 1.8304e-05 | 2.9319e-06 | 0.181 |  |  | 341966848 | 98533376 | exact |
| traj-100k | full_read | warm | readcon-db | 1 | 1 | 12 | 2.4801e-01 | 2.8203e-01 | 5.0277e-02 | 0.203 |  |  | 341966848 | 1532911616 | exact |
| traj-100k | full_read | warm | ase.db | 1 | 1 | 12 | 1.8919e+00 | 2.2073e+00 | 3.0633e-02 | 0.016 |  |  | 384503808 | 7759364096 | exact |
| traj-100k | full_read | warm | readcon-db | unset | 8 | 12 | 2.4772e-01 | 3.0947e-01 | 4.4144e-02 | 0.178 |  |  | 341966848 | 1534885888 | exact |
| traj-100k | selected | warm | readcon-db | 1 | 1 | 12 | 2.4915e-03 | 2.5801e-03 | 3.3926e-05 | 0.014 |  |  | 341966848 | 128942080 | exact |
| traj-100k | selected | warm | ase.db | 1 | 1 | 12 | 3.1814e-02 | 3.3211e-02 | 3.6091e-03 | 0.113 |  |  | 384503808 | 307064832 | exact |
| traj-100k | selected | warm | readcon-db | unset | 8 | 12 | 2.4185e-03 | 2.4834e-03 | 2.4930e-05 | 0.010 |  |  | 341966848 | 128933888 | exact |
| traj-100k | range | warm | readcon-db | 1 | 1 | 12 | 6.1827e-02 | 7.9836e-02 | 1.6905e-03 | 0.027 |  |  | 341966848 | 594112512 | exact |
| traj-100k | range | warm | ase.db | 1 | 1 | 12 | 4.9314e-01 | 5.2360e-01 | 2.3416e-02 | 0.047 |  |  | 384503808 | 2155253760 | exact |
| traj-100k | range | warm | readcon-db | unset | 8 | 12 | 6.4580e-02 | 8.3134e-02 | 2.7775e-03 | 0.043 |  |  | 341966848 | 594051072 | exact |
| traj-100k | metadata | warm | readcon-db | 1 | 1 | 12 | 2.4022e-06 | 2.4199e-06 | 1.5578e-08 | 0.006 |  |  | 341966848 | 98467840 | exact |
| traj-100k | metadata | warm | ase.db | 1 | 1 | 12 | 6.2908e-02 | 6.6771e-02 | 1.0195e-03 | 0.016 |  |  | 384503808 | 148140032 | exact |
| traj-100k | metadata | warm | readcon-db | unset | 8 | 12 | 2.4189e-06 | 2.4497e-06 | 2.3191e-08 | 0.010 |  |  | 341966848 | 98344960 | exact |

