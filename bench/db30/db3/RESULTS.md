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
| readcon-db | 3.4836e-03 | 1.1798e-03 | 3.1753e-03 | 4.3551e-03 | 2.7074e-03 | 1.0349e-02 |
| ase.db | 6.9853e-02 | 1.0110e-02 | 6.7450e-02 | 7.7560e-02 | 6.2933e-02 | 1.0136e-01 |

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
| paper40x8 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.5486e-03 | 2.5913e-03 | 6.2919e-05 | 0.025 | 1.5695e+04 | 4.8968e+00 | 49152 | 117755904 | exact |
| paper40x8 | insert | cold-file | ase.db | 1 | 1 | 12 | 5.7382e-03 | 6.0899e-03 | 3.7868e-04 | 0.066 | 6.9709e+03 | 2.1749e+00 | 90112 | 104755200 | exact |
| paper40x8 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.6016e-03 | 2.6324e-03 | 9.5967e-05 | 0.037 | 1.5375e+04 | 4.7971e+00 | 49152 | 119664640 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 1.5783e-03 | 1.7387e-03 | 1.8313e-04 | 0.116 | 6.3358e+02 | 2.6960e+01 | 98304 | 102432768 | exact |
| atoms-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.8524e-03 | 1.9764e-03 | 1.2614e-04 | 0.068 | 5.3984e+02 | 2.2971e+01 | 126976 | 102629376 | exact |
| atoms-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 1.5346e-03 | 1.8032e-03 | 1.3121e-04 | 0.086 | 6.5162e+02 | 2.7728e+01 | 98304 | 102432768 | exact |
| atoms-1000 | full_read | cold | readcon-db | 1 | 1 | 12 | 3.4609e-05 | 3.5367e-05 | 8.5775e-07 | 0.025 |  |  | 98304 | 100503552 | exact |
| atoms-1000 | full_read | cold | ase.db | 1 | 1 | 12 | 1.8690e-03 | 1.9713e-03 | 1.2737e-04 | 0.068 |  |  | 126976 | 123793408 | exact |
| atoms-1000 | full_read | cold | readcon-db | unset | 8 | 12 | 3.3671e-05 | 3.4401e-05 | 6.7902e-07 | 0.020 |  |  | 98304 | 100569088 | exact |
| atoms-1000 | selected | cold | readcon-db | 1 | 1 | 12 | 3.1968e-05 | 3.2936e-05 | 1.6001e-06 | 0.050 |  |  | 98304 | 100622336 | exact |
| atoms-1000 | selected | cold | ase.db | 1 | 1 | 12 | 1.2289e-03 | 1.3207e-03 | 1.7035e-04 | 0.139 |  |  | 126976 | 137093120 | exact |
| atoms-1000 | selected | cold | readcon-db | unset | 8 | 12 | 3.1957e-05 | 3.2848e-05 | 8.3139e-07 | 0.026 |  |  | 98304 | 100388864 | exact |
| atoms-1000 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.6946e-05 | 1.7057e-05 | 5.3693e-08 | 0.003 |  |  | 98304 | 100524032 | exact |
| atoms-1000 | full_read | warm | ase.db | 1 | 1 | 12 | 5.3194e-04 | 5.5853e-04 | 1.2077e-04 | 0.227 |  |  | 126976 | 189030400 | exact |
| atoms-1000 | full_read | warm | readcon-db | unset | 8 | 12 | 1.6532e-05 | 1.6606e-05 | 1.1190e-07 | 0.007 |  |  | 98304 | 100569088 | exact |
| atoms-1000 | selected | warm | readcon-db | 1 | 1 | 12 | 1.6296e-05 | 1.6356e-05 | 4.8437e-08 | 0.003 |  |  | 98304 | 100442112 | exact |
| atoms-1000 | selected | warm | ase.db | 1 | 1 | 12 | 4.1739e-04 | 5.3811e-04 | 9.1929e-05 | 0.220 |  |  | 126976 | 198696960 | exact |
| atoms-1000 | selected | warm | readcon-db | unset | 8 | 12 | 1.6314e-05 | 1.7221e-05 | 3.3822e-07 | 0.021 |  |  | 98304 | 100536320 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.8335e-03 | 3.9017e-03 | 8.8284e-05 | 0.023 | 2.6086e+02 | 1.2034e+02 | 589824 | 120860672 | exact |
| atoms-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 4.0452e-03 | 4.4621e-03 | 3.2506e-04 | 0.080 | 2.4721e+02 | 1.1404e+02 | 667648 | 111648768 | exact |
| atoms-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.8806e-03 | 3.9609e-03 | 1.4876e-04 | 0.038 | 2.5769e+02 | 1.1888e+02 | 589824 | 120635392 | exact |
| atoms-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.9558e-04 | 2.9814e-04 | 3.3982e-06 | 0.011 |  |  | 589824 | 104718336 | exact |
| atoms-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 5.2340e-03 | 5.3313e-03 | 9.0708e-05 | 0.017 |  |  | 667648 | 188571648 | exact |
| atoms-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 3.0235e-04 | 3.0495e-04 | 1.5707e-06 | 0.005 |  |  | 589824 | 104845312 | exact |
| atoms-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.8885e-04 | 2.9655e-04 | 5.8903e-06 | 0.020 |  |  | 589824 | 104615936 | exact |
| atoms-10000 | selected | cold | ase.db | 1 | 1 | 12 | 5.1349e-03 | 5.3477e-03 | 4.5298e-04 | 0.088 |  |  | 667648 | 194002944 | exact |
| atoms-10000 | selected | cold | readcon-db | unset | 8 | 12 | 2.8636e-04 | 3.0031e-04 | 1.0813e-05 | 0.038 |  |  | 589824 | 104796160 | exact |
| atoms-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.6132e-04 | 2.6240e-04 | 8.7149e-07 | 0.003 |  |  | 589824 | 103874560 | exact |
| atoms-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.1351e-03 | 3.9277e-03 | 7.8443e-04 | 0.250 |  |  | 667648 | 234360832 | exact |
| atoms-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.4802e-04 | 2.4895e-04 | 4.3042e-07 | 0.002 |  |  | 589824 | 104026112 | exact |
| atoms-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.4347e-04 | 2.4395e-04 | 5.4874e-07 | 0.002 |  |  | 589824 | 103895040 | exact |
| atoms-10000 | selected | warm | ase.db | 1 | 1 | 12 | 3.5460e-03 | 3.8963e-03 | 7.3945e-04 | 0.209 |  |  | 667648 | 230301696 | exact |
| atoms-10000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5645e-04 | 2.5728e-04 | 5.1881e-07 | 0.002 |  |  | 589824 | 103854080 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.5892e-02 | 2.6661e-02 | 6.7720e-04 | 0.026 | 3.8621e+01 | 1.9346e+02 | 5390336 | 196964352 | exact |
| atoms-100000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.5682e-02 | 2.7515e-02 | 1.4392e-03 | 0.056 | 3.8938e+01 | 1.9504e+02 | 6074368 | 207904768 | exact |
| atoms-100000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.9019e-02 | 3.1763e-02 | 2.7147e-03 | 0.094 | 3.4460e+01 | 1.7261e+02 | 5390336 | 197128192 | exact |
| atoms-100000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.1810e-03 | 6.6484e-03 | 6.2448e-04 | 0.101 |  |  | 5390336 | 135081984 | exact |
| atoms-100000 | full_read | cold | ase.db | 1 | 1 | 12 | 4.1899e-02 | 4.3723e-02 | 1.7024e-03 | 0.041 |  |  | 6074368 | 297377792 | exact |
| atoms-100000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.2243e-03 | 6.6643e-03 | 1.1203e-03 | 0.214 |  |  | 5390336 | 135155712 | exact |
| atoms-100000 | selected | cold | readcon-db | 1 | 1 | 12 | 6.3160e-03 | 6.5623e-03 | 2.9110e-04 | 0.046 |  |  | 5390336 | 134934528 | exact |
| atoms-100000 | selected | cold | ase.db | 1 | 1 | 12 | 3.9288e-02 | 4.1166e-02 | 1.3936e-03 | 0.035 |  |  | 6074368 | 310452224 | exact |
| atoms-100000 | selected | cold | readcon-db | unset | 8 | 12 | 5.2633e-03 | 5.7366e-03 | 3.4392e-04 | 0.065 |  |  | 5390336 | 134823936 | exact |
| atoms-100000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.4761e-03 | 2.5816e-03 | 5.6937e-05 | 0.023 |  |  | 5390336 | 128770048 | exact |
| atoms-100000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.3077e-02 | 3.3860e-02 | 4.5576e-04 | 0.014 |  |  | 6074368 | 300396544 | exact |
| atoms-100000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.5809e-03 | 2.7152e-03 | 9.3271e-05 | 0.036 |  |  | 5390336 | 128765952 | exact |
| atoms-100000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.5292e-03 | 2.6351e-03 | 4.0922e-05 | 0.016 |  |  | 5390336 | 128839680 | exact |
| atoms-100000 | selected | warm | ase.db | 1 | 1 | 12 | 3.2055e-02 | 3.4581e-02 | 2.7386e-03 | 0.085 |  |  | 6074368 | 301445120 | exact |
| atoms-100000 | selected | warm | readcon-db | unset | 8 | 12 | 2.4835e-03 | 2.5853e-03 | 9.2672e-05 | 0.037 |  |  | 5390336 | 128831488 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.2354e-01 | 3.3170e-01 | 1.1890e-02 | 0.037 | 3.0908e+00 | 1.6717e+02 | 52592640 | 733437952 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.7009e-01 | 2.8943e-01 | 3.2216e-02 | 0.119 | 3.7025e+00 | 2.0026e+02 | 60125184 | 1123733504 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.0217e-01 | 3.1776e-01 | 1.5749e-02 | 0.052 | 3.3094e+00 | 1.7899e+02 | 52592640 | 733224960 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.0445e-02 | 7.4132e-02 | 9.9597e-03 | 0.165 |  |  | 52592640 | 427487232 | exact |
| atoms-1000000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.5956e-01 | 3.7281e-01 | 1.6510e-02 | 0.046 |  |  | 60125184 | 1353625600 | exact |
| atoms-1000000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.4044e-02 | 7.2361e-02 | 6.9223e-03 | 0.108 |  |  | 52592640 | 427335680 | exact |
| atoms-1000000 | selected | cold | readcon-db | 1 | 1 | 12 | 5.5339e-02 | 6.8044e-02 | 4.2250e-03 | 0.076 |  |  | 52592640 | 427171840 | exact |
| atoms-1000000 | selected | cold | ase.db | 1 | 1 | 12 | 3.4983e-01 | 3.5889e-01 | 1.1007e-02 | 0.031 |  |  | 60125184 | 1321410560 | exact |
| atoms-1000000 | selected | cold | readcon-db | unset | 8 | 12 | 5.7146e-02 | 7.3412e-02 | 1.0765e-02 | 0.188 |  |  | 52592640 | 427298816 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.8644e-02 | 3.1318e-02 | 2.8379e-03 | 0.099 |  |  | 52592640 | 364724224 | exact |
| atoms-1000000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.1891e-01 | 3.3842e-01 | 2.4405e-02 | 0.077 |  |  | 60125184 | 1330597888 | exact |
| atoms-1000000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.7497e-02 | 3.0189e-02 | 1.3170e-03 | 0.048 |  |  | 52592640 | 364568576 | exact |
| atoms-1000000 | selected | warm | readcon-db | 1 | 1 | 12 | 3.0960e-02 | 3.5598e-02 | 2.2920e-03 | 0.074 |  |  | 52592640 | 364593152 | exact |
| atoms-1000000 | selected | warm | ase.db | 1 | 1 | 12 | 3.0957e-01 | 3.2296e-01 | 1.9126e-02 | 0.062 |  |  | 60125184 | 1330532352 | exact |
| atoms-1000000 | selected | warm | readcon-db | unset | 8 | 12 | 2.6469e-02 | 3.0793e-02 | 2.2516e-03 | 0.085 |  |  | 52592640 | 364556288 | exact |
| frames-1 | insert | cold-file | readcon-db | 1 | 1 | 12 | 7.7852e-04 | 8.6563e-04 | 8.1032e-05 | 0.104 | 1.2845e+03 | 9.3126e-01 | 40960 | 100569088 | exact |
| frames-1 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.0622e-03 | 1.1328e-03 | 3.6029e-05 | 0.034 | 9.4143e+02 | 6.8254e-01 | 69632 | 105168896 | exact |
| frames-1 | insert | cold-file | readcon-db | unset | 8 | 12 | 7.7400e-04 | 8.1273e-04 | 7.6657e-05 | 0.099 | 1.2920e+03 | 9.3669e-01 | 40960 | 99999744 | exact |
| frames-1 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.2708e-05 | 2.6146e-05 | 1.6973e-06 | 0.075 |  |  | 40960 | 98328576 | exact |
| frames-1 | full_read | cold | ase.db | 1 | 1 | 12 | 5.3174e-04 | 5.5575e-04 | 5.3239e-05 | 0.100 |  |  | 69632 | 103768064 | exact |
| frames-1 | full_read | cold | readcon-db | unset | 8 | 12 | 2.2981e-05 | 2.3638e-05 | 4.7439e-07 | 0.021 |  |  | 40960 | 98488320 | exact |
| frames-1 | selected | cold | readcon-db | 1 | 1 | 12 | 2.1426e-05 | 2.2637e-05 | 8.7352e-07 | 0.041 |  |  | 40960 | 98447360 | exact |
| frames-1 | selected | cold | ase.db | 1 | 1 | 12 | 7.6609e-04 | 8.3052e-04 | 1.1319e-04 | 0.148 |  |  | 69632 | 102260736 | exact |
| frames-1 | selected | cold | readcon-db | unset | 8 | 12 | 2.1374e-05 | 2.2294e-05 | 6.7329e-07 | 0.031 |  |  | 40960 | 98279424 | exact |
| frames-1 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.2513e-05 | 1.2586e-05 | 5.7260e-08 | 0.005 |  |  | 40960 | 98185216 | exact |
| frames-1 | full_read | warm | ase.db | 1 | 1 | 12 | 1.6111e-04 | 1.6568e-04 | 2.5426e-06 | 0.016 |  |  | 69632 | 108638208 | exact |
| frames-1 | full_read | warm | readcon-db | unset | 8 | 12 | 1.1864e-05 | 1.2094e-05 | 2.6995e-07 | 0.023 |  |  | 40960 | 98332672 | exact |
| frames-1 | selected | warm | readcon-db | 1 | 1 | 12 | 1.1773e-05 | 1.4719e-05 | 9.5014e-07 | 0.081 |  |  | 40960 | 98201600 | exact |
| frames-1 | selected | warm | ase.db | 1 | 1 | 12 | 1.6416e-04 | 1.6671e-04 | 1.3990e-06 | 0.009 |  |  | 69632 | 108621824 | exact |
| frames-1 | selected | warm | readcon-db | unset | 8 | 12 | 1.2073e-05 | 1.2210e-05 | 1.4525e-07 | 0.012 |  |  | 40960 | 98230272 | exact |
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.4693e-03 | 4.5886e-03 | 1.1113e-04 | 0.025 | 2.2375e+04 | 1.6474e+01 | 65536 | 114085888 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.2197e-02 | 3.2502e-02 | 2.1819e-04 | 0.007 | 3.1058e+03 | 2.2867e+00 | 151552 | 100671488 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.5325e-03 | 4.5841e-03 | 4.4710e-05 | 0.010 | 2.2063e+04 | 1.6244e+01 | 65536 | 116121600 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.6001e-05 | 6.9276e-05 | 1.6818e-06 | 0.025 |  |  | 65536 | 105193472 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0127e-02 | 1.0298e-02 | 4.0130e-04 | 0.040 |  |  | 151552 | 117743616 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.7599e-05 | 7.2737e-05 | 4.2634e-06 | 0.063 |  |  | 65536 | 105119744 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.1032e-05 | 2.2536e-05 | 1.0966e-06 | 0.052 |  |  | 65536 | 103051264 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 8.7002e-04 | 1.0890e-03 | 1.6692e-04 | 0.192 |  |  | 151552 | 101642240 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.0892e-05 | 2.2090e-05 | 5.6877e-07 | 0.027 |  |  | 65536 | 102879232 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.8026e-05 | 7.0469e-05 | 1.8660e-06 | 0.027 |  |  | 65536 | 105189376 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 1.0044e-02 | 1.0277e-02 | 2.4738e-04 | 0.025 |  |  | 151552 | 117493760 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 6.8468e-05 | 6.9814e-05 | 1.9564e-06 | 0.029 |  |  | 65536 | 105287680 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.0589e-05 | 2.2310e-05 | 1.8365e-06 | 0.089 |  |  | 65536 | 102690816 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 8.2189e-04 | 9.0906e-04 | 6.9512e-05 | 0.085 |  |  | 151552 | 101892096 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.0462e-05 | 2.3118e-05 | 1.3212e-06 | 0.065 |  |  | 65536 | 102903808 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 3.6539e-04 | 4.0529e-04 | 5.3819e-05 | 0.147 |  |  | 65536 | 105189376 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.0577e-02 | 1.0753e-02 | 1.8384e-04 | 0.017 |  |  | 151552 | 116396032 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 3.4027e-04 | 4.0660e-04 | 7.0311e-05 | 0.207 |  |  | 65536 | 105132032 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 2.6952e-04 | 3.0578e-04 | 3.5524e-05 | 0.132 |  |  | 65536 | 102678528 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 1.1773e-03 | 1.2915e-03 | 1.7538e-04 | 0.149 |  |  | 151552 | 100888576 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.4187e-04 | 2.6595e-04 | 2.5277e-05 | 0.105 |  |  | 65536 | 102727680 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.3575e-05 | 4.5131e-05 | 9.8015e-07 | 0.022 |  |  | 65536 | 104898560 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 8.8882e-03 | 1.1023e-02 | 1.8513e-04 | 0.021 |  |  | 151552 | 119185408 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.3371e-05 | 4.3659e-05 | 1.5828e-07 | 0.004 |  |  | 65536 | 105029632 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.3340e-06 | 9.4574e-06 | 1.3537e-07 | 0.015 |  |  | 65536 | 102789120 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.7208e-04 | 1.7426e-04 | 1.5510e-06 | 0.009 |  |  | 151552 | 108134400 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.9193e-06 | 9.9727e-06 | 1.1647e-07 | 0.012 |  |  | 65536 | 102735872 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.2894e-01 | 3.3297e-01 | 5.4344e-03 | 0.017 | 3.0401e+04 | 2.3331e+01 | 3362816 | 186679296 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.0555e+00 | 3.0813e+00 | 2.9820e-02 | 0.010 | 3.2728e+03 | 2.5117e+00 | 10825728 | 200863744 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2480e-01 | 3.2882e-01 | 1.6913e-03 | 0.005 | 3.0788e+04 | 2.3628e+01 | 3362816 | 194789376 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.2274e-03 | 6.4417e-03 | 1.9179e-04 | 0.031 |  |  | 3362816 | 160960512 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.6941e-01 | 9.9976e-01 | 5.3567e-02 | 0.055 |  |  | 10801152 | 533880832 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.2729e-03 | 6.5568e-03 | 2.5043e-04 | 0.040 |  |  | 3362816 | 160927744 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.4308e-05 | 4.5422e-05 | 1.0293e-06 | 0.023 |  |  | 3362816 | 117391360 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 7.3766e-03 | 8.2838e-03 | 8.7331e-04 | 0.118 |  |  | 10801152 | 99233792 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.5835e-05 | 4.9820e-05 | 3.0920e-06 | 0.067 |  |  | 3362816 | 117358592 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.2466e-03 | 7.2327e-03 | 5.1202e-04 | 0.082 |  |  | 3362816 | 179847168 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 1.0110e+00 | 1.0998e+00 | 5.4839e-02 | 0.054 |  |  | 10801152 | 552456192 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.3345e-03 | 6.8677e-03 | 3.4289e-04 | 0.054 |  |  | 3362816 | 179662848 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.5209e-06 | 9.6547e-06 | 1.6445e-07 | 0.017 |  |  | 3362816 | 113201152 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.5807e-04 | 7.7856e-04 | 1.6900e-05 | 0.022 |  |  | 10801152 | 101093376 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 9.5831e-06 | 9.7889e-06 | 1.3063e-07 | 0.014 |  |  | 3362816 | 113213440 | exact |

