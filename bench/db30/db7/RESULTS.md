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
| readcon-db | 2.7491e-03 | 1.1204e-03 | 2.3529e-03 | 3.4733e-03 | 1.8532e-03 | 9.3074e-03 |
| ase.db | 4.0686e-02 | 6.6324e-03 | 3.8082e-02 | 4.4714e-02 | 3.6135e-02 | 4.8897e-02 |

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
| paper40x8 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.1277e-03 | 2.2047e-03 | 5.0539e-05 | 0.024 | 1.8800e+04 | 5.8655e+00 | 49152 | 119988224 | exact |
| paper40x8 | insert | cold-file | ase.db | 1 | 1 | 12 | 5.3309e-03 | 5.3929e-03 | 2.1234e-04 | 0.040 | 7.5035e+03 | 2.3411e+00 | 90112 | 104787968 | exact |
| paper40x8 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.1523e-03 | 2.2280e-03 | 4.5252e-05 | 0.021 | 1.8585e+04 | 5.7985e+00 | 49152 | 119771136 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 1.0337e-03 | 1.1627e-03 | 5.7441e-05 | 0.056 | 9.6738e+02 | 4.1164e+01 | 98304 | 102273024 | exact |
| atoms-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.3913e-03 | 1.4462e-03 | 6.6952e-05 | 0.048 | 7.1875e+02 | 3.0584e+01 | 126976 | 106119168 | exact |
| atoms-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 1.0584e-03 | 1.2289e-03 | 1.7004e-04 | 0.161 | 9.4485e+02 | 4.0205e+01 | 98304 | 102432768 | exact |
| atoms-1000 | full_read | cold | readcon-db | 1 | 1 | 12 | 3.0946e-05 | 3.1783e-05 | 6.8167e-07 | 0.022 |  |  | 98304 | 100581376 | exact |
| atoms-1000 | full_read | cold | ase.db | 1 | 1 | 12 | 1.1310e-03 | 1.3282e-03 | 1.9231e-04 | 0.170 |  |  | 126976 | 137867264 | exact |
| atoms-1000 | full_read | cold | readcon-db | unset | 8 | 12 | 3.0501e-05 | 3.0988e-05 | 6.0810e-07 | 0.020 |  |  | 98304 | 100597760 | exact |
| atoms-1000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.9192e-05 | 3.0111e-05 | 6.9024e-07 | 0.024 |  |  | 98304 | 100544512 | exact |
| atoms-1000 | selected | cold | ase.db | 1 | 1 | 12 | 1.6633e-03 | 1.8457e-03 | 1.4549e-04 | 0.087 |  |  | 126976 | 127094784 | exact |
| atoms-1000 | selected | cold | readcon-db | unset | 8 | 12 | 2.9033e-05 | 3.0000e-05 | 1.0844e-06 | 0.037 |  |  | 98304 | 100605952 | exact |
| atoms-1000 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.6726e-05 | 1.6898e-05 | 1.1072e-07 | 0.007 |  |  | 98304 | 100515840 | exact |
| atoms-1000 | full_read | warm | ase.db | 1 | 1 | 12 | 4.6703e-04 | 4.8589e-04 | 1.4222e-05 | 0.030 |  |  | 126976 | 192720896 | exact |
| atoms-1000 | full_read | warm | readcon-db | unset | 8 | 12 | 1.6920e-05 | 1.7079e-05 | 1.3691e-07 | 0.008 |  |  | 98304 | 100548608 | exact |
| atoms-1000 | selected | warm | readcon-db | 1 | 1 | 12 | 1.6460e-05 | 1.6646e-05 | 5.5271e-08 | 0.003 |  |  | 98304 | 100409344 | exact |
| atoms-1000 | selected | warm | ase.db | 1 | 1 | 12 | 5.2579e-04 | 5.8198e-04 | 1.0000e-04 | 0.190 |  |  | 126976 | 185536512 | exact |
| atoms-1000 | selected | warm | readcon-db | unset | 8 | 12 | 1.6546e-05 | 1.6772e-05 | 2.0057e-07 | 0.012 |  |  | 98304 | 100413440 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.5428e-03 | 3.6981e-03 | 1.8237e-04 | 0.051 | 2.8226e+02 | 1.3021e+02 | 589824 | 120692736 | exact |
| atoms-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.5056e-03 | 3.5842e-03 | 1.3985e-04 | 0.040 | 2.8526e+02 | 1.3160e+02 | 667648 | 114200576 | exact |
| atoms-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.5628e-03 | 3.7899e-03 | 2.2667e-04 | 0.064 | 2.8068e+02 | 1.2948e+02 | 589824 | 120696832 | exact |
| atoms-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 3.1165e-04 | 3.2087e-04 | 8.0896e-06 | 0.026 |  |  | 589824 | 104796160 | exact |
| atoms-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 5.0483e-03 | 5.5417e-03 | 5.7220e-04 | 0.113 |  |  | 667648 | 191242240 | exact |
| atoms-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 2.9122e-04 | 3.0427e-04 | 1.0509e-05 | 0.036 |  |  | 589824 | 104841216 | exact |
| atoms-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.9409e-04 | 2.9953e-04 | 6.2996e-06 | 0.021 |  |  | 589824 | 104722432 | exact |
| atoms-10000 | selected | cold | ase.db | 1 | 1 | 12 | 5.3564e-03 | 5.7106e-03 | 8.4970e-04 | 0.159 |  |  | 667648 | 192700416 | exact |
| atoms-10000 | selected | cold | readcon-db | unset | 8 | 12 | 2.9381e-04 | 2.9793e-04 | 2.6695e-06 | 0.009 |  |  | 589824 | 104603648 | exact |
| atoms-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.6751e-04 | 2.7465e-04 | 2.6501e-06 | 0.010 |  |  | 589824 | 104079360 | exact |
| atoms-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.1938e-03 | 3.4652e-03 | 2.8054e-04 | 0.088 |  |  | 667648 | 235876352 | exact |
| atoms-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.6344e-04 | 2.7003e-04 | 4.9356e-06 | 0.019 |  |  | 589824 | 103981056 | exact |
| atoms-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.4305e-04 | 2.4735e-04 | 1.6105e-06 | 0.007 |  |  | 589824 | 103866368 | exact |
| atoms-10000 | selected | warm | ase.db | 1 | 1 | 12 | 3.5340e-03 | 3.7831e-03 | 4.1924e-04 | 0.119 |  |  | 667648 | 230600704 | exact |
| atoms-10000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5483e-04 | 2.5755e-04 | 2.3994e-06 | 0.009 |  |  | 589824 | 104058880 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.5793e-02 | 2.7191e-02 | 1.2802e-03 | 0.050 | 3.8770e+01 | 1.9420e+02 | 5390336 | 197328896 | exact |
| atoms-100000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.5831e-02 | 2.6433e-02 | 9.5615e-04 | 0.037 | 3.8714e+01 | 1.9392e+02 | 6074368 | 204242944 | exact |
| atoms-100000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.8259e-02 | 2.9432e-02 | 1.5577e-03 | 0.055 | 3.5387e+01 | 1.7726e+02 | 5390336 | 197169152 | exact |
| atoms-100000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.2458e-03 | 7.2919e-03 | 7.8203e-04 | 0.125 |  |  | 5390336 | 134975488 | exact |
| atoms-100000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.5755e-02 | 4.0601e-02 | 4.6525e-03 | 0.130 |  |  | 6074368 | 310362112 | exact |
| atoms-100000 | full_read | cold | readcon-db | unset | 8 | 12 | 7.4447e-03 | 8.0699e-03 | 3.9581e-04 | 0.053 |  |  | 5390336 | 134901760 | exact |
| atoms-100000 | selected | cold | readcon-db | 1 | 1 | 12 | 7.2277e-03 | 7.8682e-03 | 7.3363e-04 | 0.102 |  |  | 5390336 | 135004160 | exact |
| atoms-100000 | selected | cold | ase.db | 1 | 1 | 12 | 3.9360e-02 | 8.0097e-02 | 1.9044e-02 | 0.484 |  |  | 6074368 | 278638592 | exact |
| atoms-100000 | selected | cold | readcon-db | unset | 8 | 12 | 7.2814e-03 | 9.1784e-03 | 2.0661e-03 | 0.284 |  |  | 5390336 | 134799360 | exact |
| atoms-100000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.5996e-03 | 2.8453e-03 | 6.3340e-05 | 0.024 |  |  | 5390336 | 128864256 | exact |
| atoms-100000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.6028e-02 | 3.9401e-02 | 5.3446e-03 | 0.148 |  |  | 6074368 | 299667456 | exact |
| atoms-100000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.5615e-03 | 2.6677e-03 | 3.4748e-05 | 0.014 |  |  | 5390336 | 128561152 | exact |
| atoms-100000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.5399e-03 | 2.6078e-03 | 4.2927e-05 | 0.017 |  |  | 5390336 | 128712704 | exact |
| atoms-100000 | selected | warm | ase.db | 1 | 1 | 12 | 3.3511e-02 | 3.7726e-02 | 2.4978e-03 | 0.075 |  |  | 6074368 | 307011584 | exact |
| atoms-100000 | selected | warm | readcon-db | unset | 8 | 12 | 2.4930e-03 | 2.5640e-03 | 4.3796e-05 | 0.018 |  |  | 5390336 | 128716800 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.0111e-01 | 3.0767e-01 | 6.0800e-03 | 0.020 | 3.3211e+00 | 1.7962e+02 | 52592640 | 733315072 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.5956e-01 | 2.6662e-01 | 1.9719e-02 | 0.076 | 3.8527e+00 | 2.0838e+02 | 60125184 | 1111535616 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.0999e-01 | 3.1388e-01 | 3.8667e-03 | 0.012 | 3.2259e+00 | 1.7448e+02 | 52592640 | 733548544 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.1557e-02 | 6.9030e-02 | 1.0392e-02 | 0.169 |  |  | 52592640 | 427499520 | exact |
| atoms-1000000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.4413e-01 | 3.5496e-01 | 1.4093e-02 | 0.041 |  |  | 60125184 | 1337430016 | exact |
| atoms-1000000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.5014e-02 | 7.7650e-02 | 8.0494e-03 | 0.124 |  |  | 52592640 | 427368448 | exact |
| atoms-1000000 | selected | cold | readcon-db | 1 | 1 | 12 | 5.8754e-02 | 6.6826e-02 | 4.0050e-03 | 0.068 |  |  | 52592640 | 427331584 | exact |
| atoms-1000000 | selected | cold | ase.db | 1 | 1 | 12 | 3.3568e-01 | 3.4224e-01 | 7.9952e-03 | 0.024 |  |  | 60125184 | 1320513536 | exact |
| atoms-1000000 | selected | cold | readcon-db | unset | 8 | 12 | 6.4042e-02 | 7.6808e-02 | 6.4946e-03 | 0.101 |  |  | 52592640 | 427180032 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 1 | 1 | 12 | 3.0378e-02 | 3.4463e-02 | 3.7242e-03 | 0.123 |  |  | 52592640 | 364425216 | exact |
| atoms-1000000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.0932e-01 | 3.2481e-01 | 9.1840e-03 | 0.030 |  |  | 60125184 | 1330495488 | exact |
| atoms-1000000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.4157e-02 | 2.6477e-02 | 2.5606e-03 | 0.106 |  |  | 52592640 | 364601344 | exact |
| atoms-1000000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.6323e-02 | 2.8500e-02 | 2.4966e-03 | 0.095 |  |  | 52592640 | 364568576 | exact |
| atoms-1000000 | selected | warm | ase.db | 1 | 1 | 12 | 2.9733e-01 | 3.0331e-01 | 5.4107e-03 | 0.018 |  |  | 60125184 | 1330806784 | exact |
| atoms-1000000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5816e-02 | 2.8630e-02 | 2.0931e-03 | 0.081 |  |  | 52592640 | 364343296 | exact |
| frames-1 | insert | cold-file | readcon-db | 1 | 1 | 12 | 7.5823e-04 | 8.7929e-04 | 1.0723e-04 | 0.141 | 1.3189e+03 | 9.5617e-01 | 40960 | 100483072 | exact |
| frames-1 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.1653e-03 | 1.2400e-03 | 1.5235e-04 | 0.131 | 8.5814e+02 | 6.2215e-01 | 69632 | 105058304 | exact |
| frames-1 | insert | cold-file | readcon-db | unset | 8 | 12 | 6.6816e-04 | 7.7521e-04 | 5.9769e-05 | 0.089 | 1.4966e+03 | 1.0851e+00 | 40960 | 99995648 | exact |
| frames-1 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.2194e-05 | 2.4224e-05 | 1.5321e-06 | 0.069 |  |  | 40960 | 98291712 | exact |
| frames-1 | full_read | cold | ase.db | 1 | 1 | 12 | 5.1643e-04 | 5.4402e-04 | 3.0332e-05 | 0.059 |  |  | 69632 | 103927808 | exact |
| frames-1 | full_read | cold | readcon-db | unset | 8 | 12 | 2.3240e-05 | 2.3818e-05 | 8.1961e-07 | 0.035 |  |  | 40960 | 98369536 | exact |
| frames-1 | selected | cold | readcon-db | 1 | 1 | 12 | 2.1685e-05 | 2.2295e-05 | 6.3000e-07 | 0.029 |  |  | 40960 | 98238464 | exact |
| frames-1 | selected | cold | ase.db | 1 | 1 | 12 | 5.9285e-04 | 8.0698e-04 | 2.2691e-04 | 0.383 |  |  | 69632 | 103186432 | exact |
| frames-1 | selected | cold | readcon-db | unset | 8 | 12 | 2.1392e-05 | 2.2029e-05 | 2.6872e-07 | 0.013 |  |  | 40960 | 98369536 | exact |
| frames-1 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.1882e-05 | 1.2215e-05 | 1.7980e-07 | 0.015 |  |  | 40960 | 98320384 | exact |
| frames-1 | full_read | warm | ase.db | 1 | 1 | 12 | 1.6493e-04 | 1.6902e-04 | 2.5405e-06 | 0.015 |  |  | 69632 | 108617728 | exact |
| frames-1 | full_read | warm | readcon-db | unset | 8 | 12 | 1.1835e-05 | 1.1930e-05 | 1.2464e-07 | 0.011 |  |  | 40960 | 98357248 | exact |
| frames-1 | selected | warm | readcon-db | 1 | 1 | 12 | 1.1571e-05 | 1.1721e-05 | 1.8133e-07 | 0.016 |  |  | 40960 | 98111488 | exact |
| frames-1 | selected | warm | ase.db | 1 | 1 | 12 | 1.6455e-04 | 1.7470e-04 | 6.7171e-06 | 0.041 |  |  | 69632 | 108347392 | exact |
| frames-1 | selected | warm | readcon-db | unset | 8 | 12 | 1.1504e-05 | 1.1625e-05 | 8.7137e-08 | 0.008 |  |  | 40960 | 98152448 | exact |
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.4801e-03 | 4.5914e-03 | 1.4734e-04 | 0.033 | 2.2321e+04 | 1.6434e+01 | 65536 | 114188288 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.2730e-02 | 3.3086e-02 | 6.2023e-04 | 0.019 | 3.0553e+03 | 2.2494e+00 | 151552 | 100745216 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.5431e-03 | 4.5808e-03 | 7.8147e-05 | 0.017 | 2.2011e+04 | 1.6206e+01 | 65536 | 116318208 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.9975e-05 | 7.0829e-05 | 9.5359e-07 | 0.014 |  |  | 65536 | 105283584 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0243e-02 | 1.0418e-02 | 1.7153e-04 | 0.017 |  |  | 151552 | 116883456 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.8198e-05 | 7.0182e-05 | 2.2986e-06 | 0.034 |  |  | 65536 | 105308160 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.0630e-05 | 2.1026e-05 | 6.9189e-07 | 0.034 |  |  | 65536 | 102805504 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 6.7835e-04 | 7.9848e-04 | 4.6339e-05 | 0.068 |  |  | 151552 | 102715392 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.1417e-05 | 2.2541e-05 | 1.5317e-06 | 0.072 |  |  | 65536 | 102866944 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.6399e-05 | 6.8683e-05 | 2.4972e-06 | 0.038 |  |  | 65536 | 105066496 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 9.3548e-03 | 9.5683e-03 | 1.6405e-04 | 0.018 |  |  | 151552 | 119644160 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 6.9955e-05 | 7.3608e-05 | 3.5804e-06 | 0.051 |  |  | 65536 | 105275392 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.1308e-05 | 2.2556e-05 | 8.2520e-07 | 0.039 |  |  | 65536 | 102805504 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 6.8762e-04 | 8.6982e-04 | 8.0586e-05 | 0.117 |  |  | 151552 | 102608896 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.1340e-05 | 2.2326e-05 | 6.9156e-07 | 0.032 |  |  | 65536 | 103116800 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 2.7770e-04 | 3.1515e-04 | 3.1784e-05 | 0.114 |  |  | 65536 | 105218048 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.0339e-02 | 1.0767e-02 | 3.8194e-04 | 0.037 |  |  | 151552 | 116822016 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 2.7732e-04 | 3.4968e-04 | 3.6067e-05 | 0.130 |  |  | 65536 | 105181184 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 2.3913e-04 | 2.8059e-04 | 2.6151e-05 | 0.109 |  |  | 65536 | 102621184 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 1.0278e-03 | 1.3035e-03 | 3.1488e-04 | 0.306 |  |  | 151552 | 101298176 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.1883e-04 | 2.4077e-04 | 1.6380e-05 | 0.075 |  |  | 65536 | 102780928 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.3952e-05 | 4.4233e-05 | 1.9027e-07 | 0.004 |  |  | 65536 | 104972288 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 9.1962e-03 | 9.3356e-03 | 1.7623e-04 | 0.019 |  |  | 151552 | 119578624 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.3093e-05 | 4.3431e-05 | 1.6234e-07 | 0.004 |  |  | 65536 | 105013248 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.3110e-06 | 9.5194e-06 | 4.8881e-08 | 0.005 |  |  | 65536 | 102727680 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.7307e-04 | 1.7647e-04 | 2.5676e-06 | 0.015 |  |  | 151552 | 108118016 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.4518e-06 | 9.5419e-06 | 7.2506e-08 | 0.008 |  |  | 65536 | 102649856 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.2555e-01 | 3.3494e-01 | 6.9123e-03 | 0.021 | 3.0717e+04 | 2.3574e+01 | 3362816 | 194768896 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.0421e+00 | 3.1296e+00 | 8.1204e-02 | 0.027 | 3.2872e+03 | 2.5228e+00 | 10825728 | 201003008 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2511e-01 | 3.2881e-01 | 4.1735e-03 | 0.013 | 3.0759e+04 | 2.3606e+01 | 3362816 | 185511936 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.1682e-03 | 6.5461e-03 | 2.1057e-04 | 0.034 |  |  | 3362816 | 160927744 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.6526e-01 | 1.0061e+00 | 2.5733e-02 | 0.027 |  |  | 10817536 | 533835776 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.1862e-03 | 6.4597e-03 | 1.5057e-04 | 0.024 |  |  | 3362816 | 160940032 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.5461e-05 | 4.6354e-05 | 9.0309e-07 | 0.020 |  |  | 3362816 | 117575680 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 3.6978e-03 | 4.2866e-03 | 4.2948e-04 | 0.116 |  |  | 10817536 | 99561472 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.5070e-05 | 4.6051e-05 | 1.6456e-06 | 0.037 |  |  | 3362816 | 117407744 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.1379e-03 | 6.7449e-03 | 3.0146e-04 | 0.049 |  |  | 3362816 | 179703808 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 9.3267e-01 | 9.4176e-01 | 1.6162e-02 | 0.017 |  |  | 10817536 | 552243200 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.1752e-03 | 6.6005e-03 | 3.5022e-04 | 0.057 |  |  | 3362816 | 179953664 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.4418e-06 | 9.8077e-06 | 2.2888e-07 | 0.024 |  |  | 3362816 | 113221632 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.3804e-04 | 7.4912e-04 | 7.6534e-06 | 0.010 |  |  | 10817536 | 101101568 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 9.2786e-06 | 9.3693e-06 | 6.0407e-08 | 0.007 |  |  | 3362816 | 113184768 | exact |

