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
| atoms-100000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.5706e-02 | 2.6363e-02 | 8.2430e-04 | 0.032 | 3.8901e+01 | 1.9486e+02 | 5390336 | 197111808 | exact |
| atoms-100000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.5477e-02 | 2.6491e-02 | 7.7845e-04 | 0.031 | 3.9251e+01 | 1.9661e+02 | 6074368 | 208248832 | exact |
| atoms-100000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.5488e-02 | 2.5860e-02 | 3.9868e-04 | 0.016 | 3.9234e+01 | 1.9652e+02 | 5390336 | 197255168 | exact |
| atoms-100000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.5954e-03 | 6.9896e-03 | 1.0143e-03 | 0.154 |  |  | 5390336 | 134983680 | exact |
| atoms-100000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.4026e-02 | 3.5324e-02 | 1.3151e-03 | 0.039 |  |  | 6074368 | 307273728 | exact |
| atoms-100000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.0709e-03 | 5.7717e-03 | 8.3636e-04 | 0.165 |  |  | 5390336 | 135012352 | exact |
| atoms-100000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.9141e-03 | 5.2410e-03 | 4.4429e-04 | 0.090 |  |  | 5390336 | 134983680 | exact |
| atoms-100000 | selected | cold | ase.db | 1 | 1 | 12 | 3.6003e-02 | 3.7070e-02 | 1.2084e-03 | 0.034 |  |  | 6074368 | 310521856 | exact |
| atoms-100000 | selected | cold | readcon-db | unset | 8 | 12 | 5.9597e-03 | 6.4989e-03 | 6.4029e-04 | 0.107 |  |  | 5390336 | 135032832 | exact |
| atoms-100000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.6147e-03 | 2.6798e-03 | 6.8399e-05 | 0.026 |  |  | 5390336 | 128724992 | exact |
| atoms-100000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.0593e-02 | 3.1429e-02 | 6.4078e-04 | 0.021 |  |  | 6074368 | 307003392 | exact |
| atoms-100000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.5982e-03 | 2.6663e-03 | 3.7292e-05 | 0.014 |  |  | 5390336 | 128626688 | exact |
| atoms-100000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.5008e-03 | 2.7626e-03 | 1.9634e-04 | 0.079 |  |  | 5390336 | 128675840 | exact |
| atoms-100000 | selected | warm | ase.db | 1 | 1 | 12 | 3.0295e-02 | 3.1535e-02 | 1.0421e-03 | 0.034 |  |  | 6074368 | 301395968 | exact |
| atoms-100000 | selected | warm | readcon-db | unset | 8 | 12 | 2.4721e-03 | 2.5282e-03 | 2.9920e-05 | 0.012 |  |  | 5390336 | 128638976 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.8844e-01 | 2.9198e-01 | 2.1966e-03 | 0.008 | 3.4669e+00 | 1.8751e+02 | 52592640 | 733421568 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.3866e-01 | 2.4750e-01 | 2.9454e-03 | 0.012 | 4.1900e+00 | 2.2662e+02 | 60125184 | 1135677440 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.8651e-01 | 2.9126e-01 | 3.6959e-03 | 0.013 | 3.4903e+00 | 1.8878e+02 | 52592640 | 733515776 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 1 | 1 | 12 | 5.5671e-02 | 6.8807e-02 | 9.5870e-03 | 0.172 |  |  | 52592640 | 427331584 | exact |
| atoms-1000000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.4156e-01 | 3.5254e-01 | 1.3308e-02 | 0.039 |  |  | 60125184 | 1321459712 | exact |
| atoms-1000000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.3269e-02 | 5.8268e-02 | 3.1318e-03 | 0.059 |  |  | 52592640 | 427560960 | exact |
| atoms-1000000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.8542e-02 | 5.2574e-02 | 3.7507e-03 | 0.077 |  |  | 52592640 | 427405312 | exact |
| atoms-1000000 | selected | cold | ase.db | 1 | 1 | 12 | 3.3337e-01 | 3.7314e-01 | 1.7543e-02 | 0.053 |  |  | 60125184 | 1321721856 | exact |
| atoms-1000000 | selected | cold | readcon-db | unset | 8 | 12 | 5.0238e-02 | 5.5376e-02 | 3.1321e-03 | 0.062 |  |  | 52592640 | 427511808 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.9395e-02 | 2.0942e-02 | 1.5249e-03 | 0.079 |  |  | 52592640 | 364527616 | exact |
| atoms-1000000 | full_read | warm | ase.db | 1 | 1 | 12 | 2.8636e-01 | 3.0928e-01 | 7.1091e-03 | 0.025 |  |  | 60125184 | 1330446336 | exact |
| atoms-1000000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.1571e-02 | 2.4359e-02 | 2.8486e-03 | 0.132 |  |  | 52592640 | 364503040 | exact |
| atoms-1000000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.5326e-02 | 2.8196e-02 | 2.9093e-03 | 0.115 |  |  | 52592640 | 364404736 | exact |
| atoms-1000000 | selected | warm | ase.db | 1 | 1 | 12 | 2.8363e-01 | 2.9612e-01 | 1.1934e-02 | 0.042 |  |  | 60125184 | 1330622464 | exact |
| atoms-1000000 | selected | warm | readcon-db | unset | 8 | 12 | 1.9206e-02 | 2.0164e-02 | 5.2568e-04 | 0.027 |  |  | 52592640 | 364335104 | exact |
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.5308e-03 | 4.5650e-03 | 5.3919e-05 | 0.012 | 2.2071e+04 | 1.6250e+01 | 65536 | 114069504 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.2484e-02 | 3.2908e-02 | 5.8524e-04 | 0.018 | 3.0784e+03 | 2.2665e+00 | 151552 | 100798464 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.4545e-03 | 4.5887e-03 | 1.1615e-04 | 0.026 | 2.2449e+04 | 1.6528e+01 | 65536 | 114196480 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.4767e-05 | 6.7942e-05 | 1.0414e-06 | 0.016 |  |  | 65536 | 105209856 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 9.6514e-03 | 1.0146e-02 | 8.2035e-04 | 0.085 |  |  | 151552 | 118087680 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.7772e-05 | 7.0073e-05 | 1.3112e-06 | 0.019 |  |  | 65536 | 105193472 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.1041e-05 | 2.1558e-05 | 2.8264e-07 | 0.013 |  |  | 65536 | 103096320 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 7.6490e-04 | 8.7359e-04 | 2.1031e-04 | 0.275 |  |  | 151552 | 102518784 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.1464e-05 | 2.2920e-05 | 9.7025e-07 | 0.045 |  |  | 65536 | 102961152 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.5874e-05 | 6.7375e-05 | 9.1677e-07 | 0.014 |  |  | 65536 | 105263104 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 1.0228e-02 | 1.0592e-02 | 3.9403e-04 | 0.039 |  |  | 151552 | 117215232 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 6.8925e-05 | 7.2299e-05 | 4.4757e-06 | 0.065 |  |  | 65536 | 105209856 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.1473e-05 | 2.2766e-05 | 1.0678e-06 | 0.050 |  |  | 65536 | 103051264 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 7.4224e-04 | 8.9743e-04 | 1.8626e-04 | 0.251 |  |  | 151552 | 102580224 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.1634e-05 | 2.3308e-05 | 1.1943e-06 | 0.055 |  |  | 65536 | 102658048 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 4.3713e-04 | 5.0296e-04 | 1.1989e-04 | 0.274 |  |  | 65536 | 105185280 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.1587e-02 | 1.3371e-02 | 1.2459e-03 | 0.108 |  |  | 151552 | 115265536 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 3.7457e-04 | 4.0734e-04 | 6.5506e-05 | 0.175 |  |  | 65536 | 105140224 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 2.2461e-04 | 3.0078e-04 | 6.1472e-05 | 0.274 |  |  | 65536 | 102879232 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 9.2863e-04 | 1.2429e-03 | 2.7866e-04 | 0.300 |  |  | 151552 | 101523456 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.6355e-04 | 3.2079e-04 | 5.3575e-05 | 0.203 |  |  | 65536 | 102801408 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.3243e-05 | 4.3766e-05 | 2.5177e-07 | 0.006 |  |  | 65536 | 104833024 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 9.2646e-03 | 9.3510e-03 | 9.9209e-05 | 0.011 |  |  | 151552 | 119648256 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.3210e-05 | 4.3575e-05 | 3.6083e-07 | 0.008 |  |  | 65536 | 104755200 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.2220e-06 | 9.2641e-06 | 9.8487e-08 | 0.011 |  |  | 65536 | 102653952 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.7575e-04 | 1.9238e-04 | 6.3111e-06 | 0.036 |  |  | 151552 | 107999232 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.4823e-06 | 9.5558e-06 | 1.4818e-07 | 0.016 |  |  | 65536 | 102576128 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.2488e-01 | 3.3348e-01 | 4.4696e-03 | 0.014 | 3.0780e+04 | 2.3622e+01 | 3362816 | 199196672 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.0410e+00 | 3.1117e+00 | 1.0357e-01 | 0.034 | 3.2883e+03 | 2.5236e+00 | 10833920 | 200998912 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2480e-01 | 3.3324e-01 | 6.6447e-03 | 0.020 | 3.0789e+04 | 2.3629e+01 | 3362816 | 194871296 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.1857e-03 | 6.4341e-03 | 1.9183e-04 | 0.031 |  |  | 3362816 | 160894976 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.4714e-01 | 9.6639e-01 | 1.5806e-02 | 0.017 |  |  | 10817536 | 533729280 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.2107e-03 | 6.7556e-03 | 2.7842e-04 | 0.045 |  |  | 3362816 | 160968704 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.4659e-05 | 4.5739e-05 | 1.1671e-06 | 0.026 |  |  | 3362816 | 117182464 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 4.2435e-03 | 4.7039e-03 | 7.3159e-04 | 0.172 |  |  | 10817536 | 99594240 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.5351e-05 | 4.6057e-05 | 6.2642e-07 | 0.014 |  |  | 3362816 | 117383168 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.3576e-03 | 7.5077e-03 | 2.8926e-04 | 0.045 |  |  | 3362816 | 179646464 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 9.7498e-01 | 1.0122e+00 | 4.5098e-02 | 0.046 |  |  | 10817536 | 552259584 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.2698e-03 | 6.8352e-03 | 3.0309e-04 | 0.048 |  |  | 3362816 | 179752960 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.3800e-06 | 9.5462e-06 | 1.2692e-07 | 0.014 |  |  | 3362816 | 113291264 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.5481e-04 | 7.6916e-04 | 1.0548e-05 | 0.014 |  |  | 10817536 | 100913152 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 9.2783e-06 | 9.4921e-06 | 8.8263e-08 | 0.010 |  |  | 3362816 | 113278976 | exact |

