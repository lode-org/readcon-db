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
| readcon-db | 3.9350e-03 | 1.0317e-03 | 3.4868e-03 | 4.5185e-03 | 2.0405e-03 | 9.9054e-03 |
| ase.db | 9.9898e-02 | 2.0215e-02 | 8.3863e-02 | 1.0408e-01 | 6.4319e-02 | 1.1592e-01 |

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
| paper40x8 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.5086e-03 | 2.5533e-03 | 5.6152e-05 | 0.022 | 1.5945e+04 | 4.9748e+00 | 49152 | 119930880 | exact |
| paper40x8 | insert | cold-file | ase.db | 1 | 1 | 12 | 5.6518e-03 | 5.9837e-03 | 2.8051e-04 | 0.050 | 7.0774e+03 | 2.2081e+00 | 90112 | 104787968 | exact |
| paper40x8 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.5728e-03 | 2.6566e-03 | 6.2929e-05 | 0.024 | 1.5547e+04 | 4.8508e+00 | 49152 | 117817344 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 1.6721e-03 | 1.7632e-03 | 1.8243e-04 | 0.109 | 5.9806e+02 | 2.5449e+01 | 98304 | 102477824 | exact |
| atoms-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.1277e-03 | 2.3616e-03 | 2.1654e-04 | 0.102 | 4.7000e+02 | 1.9999e+01 | 126976 | 102490112 | exact |
| atoms-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 1.6114e-03 | 1.7383e-03 | 1.4361e-04 | 0.089 | 6.2059e+02 | 2.6407e+01 | 98304 | 102682624 | exact |
| atoms-1000 | full_read | cold | readcon-db | 1 | 1 | 12 | 3.7248e-05 | 4.1005e-05 | 3.0743e-06 | 0.083 |  |  | 98304 | 100524032 | exact |
| atoms-1000 | full_read | cold | ase.db | 1 | 1 | 12 | 1.7322e-03 | 1.8124e-03 | 1.3485e-04 | 0.078 |  |  | 126976 | 125652992 | exact |
| atoms-1000 | full_read | cold | readcon-db | unset | 8 | 12 | 3.8331e-05 | 4.0303e-05 | 1.8671e-06 | 0.049 |  |  | 98304 | 100507648 | exact |
| atoms-1000 | selected | cold | readcon-db | 1 | 1 | 12 | 3.3363e-05 | 3.6139e-05 | 1.2865e-06 | 0.039 |  |  | 98304 | 100376576 | exact |
| atoms-1000 | selected | cold | ase.db | 1 | 1 | 12 | 1.3357e-03 | 1.5743e-03 | 2.4694e-04 | 0.185 |  |  | 126976 | 132837376 | exact |
| atoms-1000 | selected | cold | readcon-db | unset | 8 | 12 | 3.3470e-05 | 3.6984e-05 | 2.2830e-06 | 0.068 |  |  | 98304 | 100605952 | exact |
| atoms-1000 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.6729e-05 | 1.6921e-05 | 1.4042e-07 | 0.008 |  |  | 98304 | 100368384 | exact |
| atoms-1000 | full_read | warm | ase.db | 1 | 1 | 12 | 5.2507e-04 | 5.4659e-04 | 2.0361e-05 | 0.039 |  |  | 126976 | 183742464 | exact |
| atoms-1000 | full_read | warm | readcon-db | unset | 8 | 12 | 1.6602e-05 | 1.6683e-05 | 3.6928e-08 | 0.002 |  |  | 98304 | 100511744 | exact |
| atoms-1000 | selected | warm | readcon-db | 1 | 1 | 12 | 1.6039e-05 | 1.6105e-05 | 6.1612e-08 | 0.004 |  |  | 98304 | 100716544 | exact |
| atoms-1000 | selected | warm | ase.db | 1 | 1 | 12 | 5.8200e-04 | 5.9922e-04 | 1.4125e-04 | 0.243 |  |  | 126976 | 182640640 | exact |
| atoms-1000 | selected | warm | readcon-db | unset | 8 | 12 | 1.7327e-05 | 1.7488e-05 | 2.9708e-07 | 0.017 |  |  | 98304 | 100605952 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.0327e-03 | 4.3295e-03 | 3.4417e-04 | 0.085 | 2.4797e+02 | 1.1440e+02 | 589824 | 120795136 | exact |
| atoms-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 4.5493e-03 | 4.7345e-03 | 2.7396e-04 | 0.060 | 2.1981e+02 | 1.0141e+02 | 667648 | 111251456 | exact |
| atoms-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.8724e-03 | 3.9782e-03 | 1.5961e-04 | 0.041 | 2.5824e+02 | 1.1913e+02 | 589824 | 120815616 | exact |
| atoms-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.8967e-04 | 2.9908e-04 | 6.9494e-06 | 0.024 |  |  | 589824 | 104951808 | exact |
| atoms-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 5.8242e-03 | 6.0940e-03 | 3.4727e-04 | 0.060 |  |  | 667648 | 181678080 | exact |
| atoms-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 2.8985e-04 | 2.9789e-04 | 9.2117e-06 | 0.032 |  |  | 589824 | 104726528 | exact |
| atoms-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.8508e-04 | 2.8799e-04 | 2.3961e-06 | 0.008 |  |  | 589824 | 104628224 | exact |
| atoms-10000 | selected | cold | ase.db | 1 | 1 | 12 | 5.1136e-03 | 5.6104e-03 | 2.5595e-04 | 0.050 |  |  | 667648 | 189788160 | exact |
| atoms-10000 | selected | cold | readcon-db | unset | 8 | 12 | 2.8788e-04 | 2.9019e-04 | 3.1436e-06 | 0.011 |  |  | 589824 | 104734720 | exact |
| atoms-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.7382e-04 | 2.7507e-04 | 5.9434e-07 | 0.002 |  |  | 589824 | 104038400 | exact |
| atoms-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.1491e-03 | 3.7184e-03 | 5.1151e-05 | 0.016 |  |  | 667648 | 238669824 | exact |
| atoms-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.7268e-04 | 2.7557e-04 | 3.9652e-06 | 0.015 |  |  | 589824 | 103780352 | exact |
| atoms-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.5869e-04 | 2.5892e-04 | 4.8285e-07 | 0.002 |  |  | 589824 | 104116224 | exact |
| atoms-10000 | selected | warm | ase.db | 1 | 1 | 12 | 3.8947e-03 | 3.9646e-03 | 1.9157e-04 | 0.049 |  |  | 667648 | 219516928 | exact |
| atoms-10000 | selected | warm | readcon-db | unset | 8 | 12 | 2.3623e-04 | 2.3965e-04 | 1.7321e-06 | 0.007 |  |  | 589824 | 103948288 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.6328e-02 | 2.6840e-02 | 3.1026e-04 | 0.012 | 3.7983e+01 | 1.9026e+02 | 5390336 | 197107712 | exact |
| atoms-100000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.6358e-02 | 2.7370e-02 | 8.9085e-04 | 0.034 | 3.7939e+01 | 1.9004e+02 | 6074368 | 206536704 | exact |
| atoms-100000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.6139e-02 | 2.6476e-02 | 1.6637e-03 | 0.064 | 3.8256e+01 | 1.9163e+02 | 5390336 | 197066752 | exact |
| atoms-100000 | full_read | cold | readcon-db | 1 | 1 | 12 | 7.0740e-03 | 7.4321e-03 | 3.4601e-04 | 0.049 |  |  | 5390336 | 134950912 | exact |
| atoms-100000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.6247e-02 | 3.6852e-02 | 5.5697e-04 | 0.015 |  |  | 6074368 | 305799168 | exact |
| atoms-100000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.8207e-03 | 7.4644e-03 | 6.6377e-04 | 0.097 |  |  | 5390336 | 135094272 | exact |
| atoms-100000 | selected | cold | readcon-db | 1 | 1 | 12 | 6.5783e-03 | 7.0802e-03 | 2.1221e-03 | 0.323 |  |  | 5390336 | 135008256 | exact |
| atoms-100000 | selected | cold | ase.db | 1 | 1 | 12 | 3.9500e-02 | 4.0823e-02 | 1.8697e-03 | 0.047 |  |  | 6074368 | 297955328 | exact |
| atoms-100000 | selected | cold | readcon-db | unset | 8 | 12 | 6.9601e-03 | 7.7207e-03 | 7.3147e-04 | 0.105 |  |  | 5390336 | 135061504 | exact |
| atoms-100000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.6978e-03 | 2.7620e-03 | 1.8409e-05 | 0.007 |  |  | 5390336 | 128864256 | exact |
| atoms-100000 | full_read | warm | ase.db | 1 | 1 | 12 | 2.8028e-02 | 2.8448e-02 | 5.1604e-04 | 0.018 |  |  | 6074368 | 296542208 | exact |
| atoms-100000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.6624e-03 | 2.7274e-03 | 1.6431e-05 | 0.006 |  |  | 5390336 | 128872448 | exact |
| atoms-100000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.7028e-03 | 2.7856e-03 | 5.4644e-05 | 0.020 |  |  | 5390336 | 128602112 | exact |
| atoms-100000 | selected | warm | ase.db | 1 | 1 | 12 | 2.9217e-02 | 3.3880e-02 | 2.2170e-03 | 0.076 |  |  | 6074368 | 301445120 | exact |
| atoms-100000 | selected | warm | readcon-db | unset | 8 | 12 | 2.6639e-03 | 2.7299e-03 | 2.1105e-05 | 0.008 |  |  | 5390336 | 128638976 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.8850e-01 | 2.9431e-01 | 7.3900e-03 | 0.026 | 3.4662e+00 | 1.8748e+02 | 52592640 | 733282304 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.5939e-01 | 2.7564e-01 | 1.3094e-02 | 0.050 | 3.8552e+00 | 2.0851e+02 | 60125184 | 1119678464 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.8003e-01 | 2.8251e-01 | 2.4860e-03 | 0.009 | 3.5710e+00 | 1.9314e+02 | 52592640 | 733429760 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 1 | 1 | 12 | 5.5031e-02 | 5.8642e-02 | 3.2636e-03 | 0.059 |  |  | 52592640 | 427278336 | exact |
| atoms-1000000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.4960e-01 | 4.0646e-01 | 1.7637e-02 | 0.050 |  |  | 60125184 | 1321529344 | exact |
| atoms-1000000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.7096e-02 | 6.3684e-02 | 3.2693e-03 | 0.057 |  |  | 52592640 | 427433984 | exact |
| atoms-1000000 | selected | cold | readcon-db | 1 | 1 | 12 | 5.8077e-02 | 7.0252e-02 | 4.3140e-03 | 0.074 |  |  | 52592640 | 427499520 | exact |
| atoms-1000000 | selected | cold | ase.db | 1 | 1 | 12 | 3.3760e-01 | 3.5276e-01 | 3.5202e-02 | 0.104 |  |  | 60125184 | 1321865216 | exact |
| atoms-1000000 | selected | cold | readcon-db | unset | 8 | 12 | 6.2785e-02 | 7.1327e-02 | 6.9343e-03 | 0.110 |  |  | 52592640 | 427253760 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.6417e-02 | 2.7784e-02 | 1.2031e-03 | 0.046 |  |  | 52592640 | 364425216 | exact |
| atoms-1000000 | full_read | warm | ase.db | 1 | 1 | 12 | 2.9306e-01 | 3.0944e-01 | 2.2409e-02 | 0.076 |  |  | 60125184 | 1330405376 | exact |
| atoms-1000000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.5278e-02 | 2.8826e-02 | 7.3804e-04 | 0.029 |  |  | 52592640 | 364425216 | exact |
| atoms-1000000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.6049e-02 | 3.2333e-02 | 3.4753e-03 | 0.133 |  |  | 52592640 | 364322816 | exact |
| atoms-1000000 | selected | warm | ase.db | 1 | 1 | 12 | 2.7896e-01 | 3.0449e-01 | 9.6648e-03 | 0.035 |  |  | 60125184 | 1330614272 | exact |
| atoms-1000000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5423e-02 | 2.8984e-02 | 5.1660e-04 | 0.020 |  |  | 52592640 | 364621824 | exact |
| frames-1 | insert | cold-file | readcon-db | 1 | 1 | 12 | 1.1451e-03 | 1.3786e-03 | 2.3221e-04 | 0.203 | 8.7332e+02 | 6.3315e-01 | 40960 | 100110336 | exact |
| frames-1 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.5282e-03 | 1.5850e-03 | 1.7608e-04 | 0.115 | 6.5436e+02 | 4.7441e-01 | 69632 | 105017344 | exact |
| frames-1 | insert | cold-file | readcon-db | unset | 8 | 12 | 1.4763e-03 | 1.6548e-03 | 2.5807e-04 | 0.175 | 6.7737e+02 | 4.9109e-01 | 40960 | 99799040 | exact |
| frames-1 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.7414e-05 | 2.8792e-05 | 7.7305e-07 | 0.028 |  |  | 40960 | 98328576 | exact |
| frames-1 | full_read | cold | ase.db | 1 | 1 | 12 | 7.0226e-04 | 7.2426e-04 | 3.8176e-05 | 0.054 |  |  | 69632 | 102256640 | exact |
| frames-1 | full_read | cold | readcon-db | unset | 8 | 12 | 2.7280e-05 | 2.8774e-05 | 2.3110e-06 | 0.085 |  |  | 40960 | 98312192 | exact |
| frames-1 | selected | cold | readcon-db | 1 | 1 | 12 | 2.4204e-05 | 2.4763e-05 | 6.6975e-07 | 0.028 |  |  | 40960 | 98091008 | exact |
| frames-1 | selected | cold | ase.db | 1 | 1 | 12 | 8.7868e-04 | 1.0711e-03 | 1.7472e-04 | 0.199 |  |  | 69632 | 101625856 | exact |
| frames-1 | selected | cold | readcon-db | unset | 8 | 12 | 2.3717e-05 | 2.5013e-05 | 1.1717e-06 | 0.049 |  |  | 40960 | 98250752 | exact |
| frames-1 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.2496e-05 | 1.2797e-05 | 1.5801e-07 | 0.013 |  |  | 40960 | 98193408 | exact |
| frames-1 | full_read | warm | ase.db | 1 | 1 | 12 | 1.5243e-04 | 1.6555e-04 | 1.0124e-05 | 0.066 |  |  | 69632 | 109010944 | exact |
| frames-1 | full_read | warm | readcon-db | unset | 8 | 12 | 1.1850e-05 | 1.1997e-05 | 9.2244e-08 | 0.008 |  |  | 40960 | 98164736 | exact |
| frames-1 | selected | warm | readcon-db | 1 | 1 | 12 | 1.1419e-05 | 1.1585e-05 | 1.0795e-07 | 0.009 |  |  | 40960 | 98127872 | exact |
| frames-1 | selected | warm | ase.db | 1 | 1 | 12 | 1.7037e-04 | 1.7200e-04 | 3.2398e-06 | 0.019 |  |  | 69632 | 108355584 | exact |
| frames-1 | selected | warm | readcon-db | unset | 8 | 12 | 1.1942e-05 | 1.2085e-05 | 6.7600e-08 | 0.006 |  |  | 40960 | 97918976 | exact |
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 5.0423e-03 | 5.1375e-03 | 1.2020e-04 | 0.024 | 1.9832e+04 | 1.4602e+01 | 65536 | 115847168 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.1632e-02 | 3.2058e-02 | 1.7545e-04 | 0.006 | 3.1614e+03 | 2.3276e+00 | 151552 | 100679680 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 5.1593e-03 | 5.3054e-03 | 2.5906e-04 | 0.050 | 1.9382e+04 | 1.4270e+01 | 65536 | 114024448 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 7.2844e-05 | 7.5156e-05 | 2.3473e-06 | 0.032 |  |  | 65536 | 105127936 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0281e-02 | 1.0419e-02 | 1.6720e-04 | 0.016 |  |  | 151552 | 116310016 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 7.3314e-05 | 7.6850e-05 | 2.7547e-06 | 0.038 |  |  | 65536 | 105369600 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.5983e-05 | 2.6771e-05 | 1.2240e-06 | 0.047 |  |  | 65536 | 102703104 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 1.2435e-03 | 1.6356e-03 | 4.3092e-04 | 0.347 |  |  | 151552 | 100823040 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.3903e-05 | 2.6008e-05 | 1.3927e-06 | 0.058 |  |  | 65536 | 102797312 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 7.2767e-05 | 7.5015e-05 | 2.1731e-06 | 0.030 |  |  | 65536 | 105254912 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 9.9928e-03 | 1.0226e-02 | 5.1676e-04 | 0.052 |  |  | 151552 | 117927936 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 7.3962e-05 | 7.5652e-05 | 3.8270e-06 | 0.052 |  |  | 65536 | 105246720 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.4621e-05 | 2.5648e-05 | 7.3335e-07 | 0.030 |  |  | 65536 | 102838272 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 1.4309e-03 | 1.7224e-03 | 3.7261e-04 | 0.260 |  |  | 151552 | 100560896 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.4940e-05 | 2.6052e-05 | 9.3908e-07 | 0.038 |  |  | 65536 | 102715392 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 6.2251e-04 | 9.4397e-04 | 3.4488e-04 | 0.554 |  |  | 65536 | 105369600 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.0648e-02 | 1.0761e-02 | 2.2492e-04 | 0.021 |  |  | 151552 | 116224000 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 5.5802e-04 | 6.8381e-04 | 1.6018e-04 | 0.287 |  |  | 65536 | 105189376 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 2.7566e-04 | 3.4609e-04 | 6.4934e-05 | 0.236 |  |  | 65536 | 102768640 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 1.1819e-03 | 1.4282e-03 | 4.5923e-04 | 0.389 |  |  | 151552 | 100872192 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.6412e-04 | 3.0744e-04 | 4.6413e-05 | 0.176 |  |  | 65536 | 102928384 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.2160e-05 | 4.2752e-05 | 1.6055e-07 | 0.004 |  |  | 65536 | 104972288 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 8.3842e-03 | 8.4129e-03 | 3.2536e-05 | 0.004 |  |  | 151552 | 119894016 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.2466e-05 | 4.3104e-05 | 2.0908e-07 | 0.005 |  |  | 65536 | 104988672 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.1353e-06 | 9.1980e-06 | 1.0328e-07 | 0.011 |  |  | 65536 | 102723584 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.6296e-04 | 2.4460e-04 | 1.1866e-05 | 0.073 |  |  | 151552 | 107991040 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.6629e-06 | 9.8263e-06 | 5.2392e-08 | 0.005 |  |  | 65536 | 102699008 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.4019e-01 | 3.5010e-01 | 4.5389e-03 | 0.013 | 2.9395e+04 | 2.2559e+01 | 3362816 | 186007552 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.9555e+00 | 3.1997e+00 | 1.0058e-01 | 0.034 | 3.3836e+03 | 2.5967e+00 | 10809344 | 200753152 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2967e-01 | 3.3704e-01 | 6.6548e-03 | 0.020 | 3.0333e+04 | 2.3279e+01 | 3362816 | 185180160 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 5.9988e-03 | 6.3552e-03 | 2.3498e-04 | 0.039 |  |  | 3362816 | 160923648 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.2778e-01 | 9.4031e-01 | 1.3806e-02 | 0.015 |  |  | 10801152 | 534126592 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.9996e-03 | 6.2500e-03 | 1.8702e-04 | 0.031 |  |  | 3362816 | 160960512 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.5246e-05 | 4.5914e-05 | 6.6172e-07 | 0.015 |  |  | 3362816 | 117288960 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 4.1711e-03 | 5.0370e-03 | 1.4116e-03 | 0.338 |  |  | 10801152 | 99311616 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.4260e-05 | 4.6066e-05 | 9.3806e-07 | 0.021 |  |  | 3362816 | 117481472 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.0180e-03 | 6.6590e-03 | 3.3012e-04 | 0.055 |  |  | 3362816 | 179494912 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 9.2376e-01 | 9.6364e-01 | 5.0405e-02 | 0.055 |  |  | 10801152 | 552525824 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 5.9318e-03 | 6.5158e-03 | 1.8370e-04 | 0.031 |  |  | 3362816 | 179785728 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.7766e-06 | 9.8761e-06 | 9.6440e-08 | 0.010 |  |  | 3362816 | 113168384 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.1652e-04 | 7.1920e-04 | 6.0620e-06 | 0.008 |  |  | 10801152 | 101117952 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 9.7935e-06 | 9.9460e-06 | 1.0879e-07 | 0.011 |  |  | 3362816 | 113098752 | exact |

