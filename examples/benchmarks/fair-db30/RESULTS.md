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
| readcon-db | 3.8582e-03 | 7.3271e-04 | 3.4848e-03 | 4.2175e-03 | 2.2172e-03 | 7.4782e-03 |
| ase.db | 6.1116e-02 | 9.2140e-03 | 5.6925e-02 | 6.6139e-02 | 5.1811e-02 | 8.1576e-02 |

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
| paper40x8 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.2122e-03 | 2.2628e-03 | 6.1646e-05 | 0.028 | 1.8081e+04 | 5.6413e+00 | 49152 | 117624832 | exact |
| paper40x8 | insert | cold-file | ase.db | 1 | 1 | 12 | 5.2033e-03 | 5.4513e-03 | 2.8947e-04 | 0.056 | 7.6874e+03 | 2.3985e+00 | 90112 | 104812544 | exact |
| paper40x8 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.2686e-03 | 2.3565e-03 | 1.0091e-04 | 0.044 | 1.7632e+04 | 5.5012e+00 | 49152 | 117657600 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 1.1782e-03 | 1.3919e-03 | 1.8339e-04 | 0.156 | 8.4878e+02 | 3.6117e+01 | 98304 | 104337408 | exact |
| atoms-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.4735e-03 | 1.6586e-03 | 1.1308e-04 | 0.077 | 6.7864e+02 | 2.8877e+01 | 126976 | 106045440 | exact |
| atoms-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 1.2398e-03 | 1.3620e-03 | 1.3640e-04 | 0.110 | 8.0656e+02 | 3.4321e+01 | 98304 | 104370176 | exact |
| atoms-1000 | full_read | cold | readcon-db | 1 | 1 | 12 | 3.3484e-05 | 3.5239e-05 | 1.1915e-06 | 0.036 |  |  | 98304 | 100388864 | exact |
| atoms-1000 | full_read | cold | ase.db | 1 | 1 | 12 | 1.1799e-03 | 1.2582e-03 | 1.8804e-04 | 0.159 |  |  | 126976 | 139669504 | exact |
| atoms-1000 | full_read | cold | readcon-db | unset | 8 | 12 | 3.1911e-05 | 3.4523e-05 | 1.6244e-06 | 0.051 |  |  | 98304 | 100601856 | exact |
| atoms-1000 | selected | cold | readcon-db | 1 | 1 | 12 | 3.0199e-05 | 3.1233e-05 | 6.8424e-07 | 0.023 |  |  | 98304 | 100528128 | exact |
| atoms-1000 | selected | cold | ase.db | 1 | 1 | 12 | 1.7898e-03 | 1.9967e-03 | 2.7606e-04 | 0.154 |  |  | 126976 | 125210624 | exact |
| atoms-1000 | selected | cold | readcon-db | unset | 8 | 12 | 3.0311e-05 | 3.3951e-05 | 1.4838e-06 | 0.049 |  |  | 98304 | 100581376 | exact |
| atoms-1000 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.6890e-05 | 1.8436e-05 | 8.6240e-07 | 0.051 |  |  | 98304 | 100274176 | exact |
| atoms-1000 | full_read | warm | ase.db | 1 | 1 | 12 | 5.2462e-04 | 5.4365e-04 | 1.9236e-05 | 0.037 |  |  | 126976 | 184311808 | exact |
| atoms-1000 | full_read | warm | readcon-db | unset | 8 | 12 | 1.6873e-05 | 1.7088e-05 | 8.1681e-08 | 0.005 |  |  | 98304 | 100298752 | exact |
| atoms-1000 | selected | warm | readcon-db | 1 | 1 | 12 | 1.6297e-05 | 1.6414e-05 | 1.8152e-07 | 0.011 |  |  | 98304 | 100458496 | exact |
| atoms-1000 | selected | warm | ase.db | 1 | 1 | 12 | 4.3542e-04 | 5.2744e-04 | 4.3231e-05 | 0.099 |  |  | 126976 | 196325376 | exact |
| atoms-1000 | selected | warm | readcon-db | unset | 8 | 12 | 1.6200e-05 | 1.6643e-05 | 1.3869e-07 | 0.009 |  |  | 98304 | 100380672 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.7192e-03 | 3.8089e-03 | 1.0044e-04 | 0.027 | 2.6887e+02 | 1.2404e+02 | 589824 | 118542336 | exact |
| atoms-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.5448e-03 | 3.6039e-03 | 9.7154e-05 | 0.027 | 2.8210e+02 | 1.3014e+02 | 667648 | 111464448 | exact |
| atoms-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.7568e-03 | 3.8648e-03 | 2.0156e-04 | 0.054 | 2.6618e+02 | 1.2280e+02 | 589824 | 118808576 | exact |
| atoms-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.9667e-04 | 3.3833e-04 | 1.3034e-05 | 0.044 |  |  | 589824 | 104738816 | exact |
| atoms-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 5.4029e-03 | 6.4160e-03 | 3.9561e-04 | 0.073 |  |  | 667648 | 185675776 | exact |
| atoms-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 2.9511e-04 | 2.9901e-04 | 7.8160e-06 | 0.026 |  |  | 589824 | 104910848 | exact |
| atoms-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.8607e-04 | 2.9716e-04 | 4.5953e-06 | 0.016 |  |  | 589824 | 104636416 | exact |
| atoms-10000 | selected | cold | ase.db | 1 | 1 | 12 | 4.8624e-03 | 5.7162e-03 | 1.2608e-03 | 0.259 |  |  | 667648 | 196370432 | exact |
| atoms-10000 | selected | cold | readcon-db | unset | 8 | 12 | 2.9139e-04 | 2.9964e-04 | 3.1974e-06 | 0.011 |  |  | 589824 | 104493056 | exact |
| atoms-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.7837e-04 | 2.9844e-04 | 2.4363e-05 | 0.088 |  |  | 589824 | 103919616 | exact |
| atoms-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.3116e-03 | 3.4324e-03 | 1.2713e-04 | 0.038 |  |  | 667648 | 237367296 | exact |
| atoms-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.6565e-04 | 2.7307e-04 | 5.0520e-06 | 0.019 |  |  | 589824 | 104034304 | exact |
| atoms-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.4964e-04 | 2.5305e-04 | 1.4379e-06 | 0.006 |  |  | 589824 | 103923712 | exact |
| atoms-10000 | selected | warm | ase.db | 1 | 1 | 12 | 3.8635e-03 | 3.9923e-03 | 1.5250e-04 | 0.039 |  |  | 667648 | 219746304 | exact |
| atoms-10000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5993e-04 | 2.6248e-04 | 3.2609e-06 | 0.013 |  |  | 589824 | 103972864 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.5689e-02 | 2.6367e-02 | 5.8720e-04 | 0.023 | 3.8927e+01 | 1.9499e+02 | 5390336 | 199983104 | exact |
| atoms-100000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.6448e-02 | 2.7251e-02 | 8.1274e-04 | 0.031 | 3.7810e+01 | 1.8940e+02 | 6074368 | 207151104 | exact |
| atoms-100000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.7506e-02 | 2.7954e-02 | 1.0147e-03 | 0.037 | 3.6356e+01 | 1.8211e+02 | 5390336 | 197574656 | exact |
| atoms-100000 | full_read | cold | readcon-db | 1 | 1 | 12 | 5.7563e-03 | 6.2410e-03 | 5.6307e-04 | 0.098 |  |  | 5390336 | 135106560 | exact |
| atoms-100000 | full_read | cold | ase.db | 1 | 1 | 12 | 4.2325e-02 | 4.3935e-02 | 2.2932e-03 | 0.054 |  |  | 6074368 | 308109312 | exact |
| atoms-100000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.0816e-03 | 7.0850e-03 | 1.0897e-03 | 0.179 |  |  | 5390336 | 134901760 | exact |
| atoms-100000 | selected | cold | readcon-db | 1 | 1 | 12 | 5.5898e-03 | 6.7038e-03 | 1.1279e-03 | 0.202 |  |  | 5390336 | 134930432 | exact |
| atoms-100000 | selected | cold | ase.db | 1 | 1 | 12 | 3.9638e-02 | 4.2598e-02 | 3.6984e-03 | 0.093 |  |  | 6074368 | 297832448 | exact |
| atoms-100000 | selected | cold | readcon-db | unset | 8 | 12 | 5.3154e-03 | 6.2633e-03 | 8.5901e-04 | 0.162 |  |  | 5390336 | 134897664 | exact |
| atoms-100000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.5096e-03 | 2.5411e-03 | 2.0671e-05 | 0.008 |  |  | 5390336 | 128770048 | exact |
| atoms-100000 | full_read | warm | ase.db | 1 | 1 | 12 | 2.9690e-02 | 3.0557e-02 | 5.3423e-04 | 0.018 |  |  | 6074368 | 294445056 | exact |
| atoms-100000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.5638e-03 | 2.7175e-03 | 3.7155e-05 | 0.014 |  |  | 5390336 | 128851968 | exact |
| atoms-100000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.5984e-03 | 2.6649e-03 | 6.5803e-05 | 0.025 |  |  | 5390336 | 128757760 | exact |
| atoms-100000 | selected | warm | ase.db | 1 | 1 | 12 | 3.0116e-02 | 3.0889e-02 | 8.4868e-04 | 0.028 |  |  | 6074368 | 305446912 | exact |
| atoms-100000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5343e-03 | 2.6080e-03 | 7.2027e-05 | 0.028 |  |  | 5390336 | 128823296 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.1385e-01 | 3.2366e-01 | 6.2489e-03 | 0.020 | 3.1863e+00 | 1.7233e+02 | 52592640 | 693944320 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.6371e-01 | 2.8635e-01 | 1.2647e-02 | 0.048 | 3.7921e+00 | 2.0510e+02 | 60125184 | 1103728640 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.1686e-01 | 3.3724e-01 | 9.1016e-03 | 0.029 | 3.1559e+00 | 1.7069e+02 | 52592640 | 693161984 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 1 | 1 | 12 | 5.7957e-02 | 7.9578e-02 | 2.0540e-02 | 0.354 |  |  | 52592640 | 431726592 | exact |
| atoms-1000000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.6177e-01 | 4.5871e-01 | 5.7471e-02 | 0.159 |  |  | 60125184 | 1353588736 | exact |
| atoms-1000000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.9383e-02 | 7.1469e-02 | 1.4380e-02 | 0.242 |  |  | 52592640 | 431812608 | exact |
| atoms-1000000 | selected | cold | readcon-db | 1 | 1 | 12 | 6.3359e-02 | 7.7824e-02 | 9.2215e-03 | 0.146 |  |  | 52592640 | 431562752 | exact |
| atoms-1000000 | selected | cold | ase.db | 1 | 1 | 12 | 3.5050e-01 | 3.6696e-01 | 1.6250e-02 | 0.046 |  |  | 60125184 | 1321586688 | exact |
| atoms-1000000 | selected | cold | readcon-db | unset | 8 | 12 | 6.4814e-02 | 7.7979e-02 | 1.9185e-02 | 0.296 |  |  | 52592640 | 431603712 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 1 | 1 | 12 | 3.4467e-02 | 3.6110e-02 | 6.7593e-03 | 0.196 |  |  | 52592640 | 368611328 | exact |
| atoms-1000000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.2899e-01 | 3.4861e-01 | 1.4594e-02 | 0.044 |  |  | 60125184 | 1330585600 | exact |
| atoms-1000000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.9046e-02 | 3.5049e-02 | 3.3315e-03 | 0.115 |  |  | 52592640 | 368664576 | exact |
| atoms-1000000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.7261e-02 | 3.0345e-02 | 2.3155e-03 | 0.085 |  |  | 52592640 | 368738304 | exact |
| atoms-1000000 | selected | warm | ase.db | 1 | 1 | 12 | 3.0045e-01 | 3.0759e-01 | 9.0796e-03 | 0.030 |  |  | 60125184 | 1330544640 | exact |
| atoms-1000000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5323e-02 | 2.6565e-02 | 2.4615e-03 | 0.097 |  |  | 52592640 | 368414720 | exact |
| frames-1 | insert | cold-file | readcon-db | 1 | 1 | 12 | 7.3143e-04 | 8.3169e-04 | 7.5074e-05 | 0.103 | 1.3672e+03 | 9.9121e-01 | 40960 | 101998592 | exact |
| frames-1 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.0909e-03 | 1.1673e-03 | 7.6140e-05 | 0.070 | 9.1672e+02 | 6.6462e-01 | 69632 | 105234432 | exact |
| frames-1 | insert | cold-file | readcon-db | unset | 8 | 12 | 7.6212e-04 | 9.1328e-04 | 1.2745e-04 | 0.167 | 1.3121e+03 | 9.5130e-01 | 40960 | 101847040 | exact |
| frames-1 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.2817e-05 | 2.4003e-05 | 1.2988e-06 | 0.057 |  |  | 40960 | 98537472 | exact |
| frames-1 | full_read | cold | ase.db | 1 | 1 | 12 | 5.1652e-04 | 5.5819e-04 | 3.2836e-05 | 0.064 |  |  | 69632 | 103952384 | exact |
| frames-1 | full_read | cold | readcon-db | unset | 8 | 12 | 2.2676e-05 | 2.2803e-05 | 3.3769e-07 | 0.015 |  |  | 40960 | 98242560 | exact |
| frames-1 | selected | cold | readcon-db | 1 | 1 | 12 | 2.1115e-05 | 2.1973e-05 | 4.1382e-07 | 0.020 |  |  | 40960 | 98459648 | exact |
| frames-1 | selected | cold | ase.db | 1 | 1 | 12 | 7.3251e-04 | 8.0339e-04 | 6.6550e-05 | 0.091 |  |  | 69632 | 102342656 | exact |
| frames-1 | selected | cold | readcon-db | unset | 8 | 12 | 2.1819e-05 | 2.2302e-05 | 6.6807e-07 | 0.031 |  |  | 40960 | 98344960 | exact |
| frames-1 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.2718e-05 | 1.2828e-05 | 1.1804e-07 | 0.009 |  |  | 40960 | 98226176 | exact |
| frames-1 | full_read | warm | ase.db | 1 | 1 | 12 | 1.6297e-04 | 1.6929e-04 | 3.2817e-06 | 0.020 |  |  | 69632 | 108650496 | exact |
| frames-1 | full_read | warm | readcon-db | unset | 8 | 12 | 1.2003e-05 | 1.2153e-05 | 1.0040e-07 | 0.008 |  |  | 40960 | 98197504 | exact |
| frames-1 | selected | warm | readcon-db | 1 | 1 | 12 | 1.1889e-05 | 1.2087e-05 | 1.8736e-07 | 0.016 |  |  | 40960 | 98115584 | exact |
| frames-1 | selected | warm | ase.db | 1 | 1 | 12 | 1.7383e-04 | 1.8208e-04 | 1.2509e-05 | 0.072 |  |  | 69632 | 108163072 | exact |
| frames-1 | selected | warm | readcon-db | unset | 8 | 12 | 1.1501e-05 | 1.1678e-05 | 1.7491e-07 | 0.015 |  |  | 40960 | 98119680 | exact |
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.4647e-03 | 4.5386e-03 | 5.2294e-05 | 0.012 | 2.2398e+04 | 1.6491e+01 | 65536 | 113213440 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.2154e-02 | 3.2880e-02 | 6.6820e-04 | 0.021 | 3.1100e+03 | 2.2898e+00 | 151552 | 100708352 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.4840e-03 | 4.5284e-03 | 5.6559e-05 | 0.013 | 2.2301e+04 | 1.6419e+01 | 65536 | 115036160 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.6763e-05 | 6.8210e-05 | 1.5625e-06 | 0.023 |  |  | 65536 | 105197568 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 1.0042e-02 | 1.0398e-02 | 5.0462e-04 | 0.050 |  |  | 151552 | 117624832 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.6995e-05 | 7.0738e-05 | 2.4259e-06 | 0.036 |  |  | 65536 | 105144320 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.0812e-05 | 2.1745e-05 | 6.1007e-07 | 0.029 |  |  | 65536 | 102883328 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 7.0241e-04 | 8.1767e-04 | 1.3866e-04 | 0.197 |  |  | 151552 | 102629376 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 1.9965e-05 | 2.1001e-05 | 7.0938e-07 | 0.036 |  |  | 65536 | 102756352 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.7068e-05 | 6.8588e-05 | 1.2643e-06 | 0.019 |  |  | 65536 | 105267200 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 9.6876e-03 | 1.0010e-02 | 3.9934e-04 | 0.041 |  |  | 151552 | 119091200 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 6.5671e-05 | 6.6252e-05 | 8.0720e-07 | 0.012 |  |  | 65536 | 105136128 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.0306e-05 | 2.1049e-05 | 5.4283e-07 | 0.027 |  |  | 65536 | 102924288 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 6.9281e-04 | 9.6351e-04 | 1.7263e-04 | 0.249 |  |  | 151552 | 102543360 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.0702e-05 | 2.2023e-05 | 6.4689e-07 | 0.031 |  |  | 65536 | 102957056 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 2.9637e-04 | 3.4278e-04 | 4.0197e-05 | 0.136 |  |  | 65536 | 105222144 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 9.7568e-03 | 1.0208e-02 | 4.1364e-04 | 0.042 |  |  | 151552 | 118472704 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 3.6344e-04 | 4.1275e-04 | 1.5990e-05 | 0.044 |  |  | 65536 | 105127936 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 3.0429e-04 | 3.3121e-04 | 5.9522e-05 | 0.196 |  |  | 65536 | 102801408 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 1.3086e-03 | 1.4342e-03 | 2.1176e-04 | 0.162 |  |  | 151552 | 100728832 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.7498e-04 | 2.8670e-04 | 1.8871e-05 | 0.069 |  |  | 65536 | 102744064 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.3380e-05 | 4.3806e-05 | 3.8251e-07 | 0.009 |  |  | 65536 | 104972288 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 8.5503e-03 | 8.6723e-03 | 7.5440e-05 | 0.009 |  |  | 151552 | 119853056 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.3630e-05 | 4.6962e-05 | 2.9580e-06 | 0.068 |  |  | 65536 | 105041920 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.9851e-06 | 1.0156e-05 | 1.3373e-07 | 0.013 |  |  | 65536 | 102653952 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.6987e-04 | 1.7572e-04 | 2.0048e-06 | 0.012 |  |  | 151552 | 108134400 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.3851e-06 | 9.6922e-06 | 1.0017e-07 | 0.011 |  |  | 65536 | 102834176 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.2544e-01 | 3.2730e-01 | 6.3424e-03 | 0.019 | 3.0727e+04 | 2.3582e+01 | 3362816 | 196857856 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.9928e+00 | 3.0593e+00 | 3.6634e-02 | 0.012 | 3.3413e+03 | 2.5643e+00 | 10833920 | 200966144 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2548e-01 | 3.3189e-01 | 5.3518e-03 | 0.016 | 3.0724e+04 | 2.3579e+01 | 3362816 | 202792960 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.2129e-03 | 6.7292e-03 | 1.5986e-04 | 0.026 |  |  | 3362816 | 188239872 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.3665e-01 | 9.4774e-01 | 6.7493e-03 | 0.007 |  |  | 10817536 | 533798912 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.2194e-03 | 6.3650e-03 | 1.6366e-04 | 0.026 |  |  | 3362816 | 188346368 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.3610e-05 | 4.4326e-05 | 8.8336e-07 | 0.020 |  |  | 3362816 | 117321728 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 3.8396e-03 | 4.4381e-03 | 4.4074e-04 | 0.115 |  |  | 10817536 | 99491840 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.3914e-05 | 4.5765e-05 | 8.7499e-07 | 0.020 |  |  | 3362816 | 117420032 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.3953e-03 | 6.8811e-03 | 3.7664e-04 | 0.059 |  |  | 3362816 | 177754112 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 9.5912e-01 | 9.8958e-01 | 3.5076e-02 | 0.037 |  |  | 10817536 | 552509440 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.1789e-03 | 6.7210e-03 | 3.9522e-04 | 0.064 |  |  | 3362816 | 177860608 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.4040e-06 | 9.4802e-06 | 3.0272e-08 | 0.003 |  |  | 3362816 | 113123328 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.3598e-04 | 7.4024e-04 | 3.8321e-06 | 0.005 |  |  | 10817536 | 100999168 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 9.4357e-06 | 9.5207e-06 | 1.1498e-07 | 0.012 |  |  | 3362816 | 113233920 | exact |

