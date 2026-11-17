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
| readcon-db | 2.7794e-03 | 1.1396e-03 | 2.4030e-03 | 3.5427e-03 | 1.7478e-03 | 1.2191e-02 |
| ase.db | 4.6461e-02 | 7.2408e-03 | 4.0863e-02 | 4.8104e-02 | 3.6057e-02 | 5.5829e-02 |

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
| paper40x8 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.2440e-03 | 2.2677e-03 | 5.7185e-05 | 0.025 | 1.7825e+04 | 5.5615e+00 | 49152 | 118013952 | exact |
| paper40x8 | insert | cold-file | ase.db | 1 | 1 | 12 | 5.2710e-03 | 5.6121e-03 | 3.1419e-04 | 0.060 | 7.5887e+03 | 2.3677e+00 | 90112 | 104816640 | exact |
| paper40x8 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.2029e-03 | 2.3033e-03 | 6.8593e-05 | 0.031 | 1.8158e+04 | 5.6653e+00 | 49152 | 117768192 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 1.1596e-03 | 1.2648e-03 | 1.4893e-04 | 0.128 | 8.6234e+02 | 3.6694e+01 | 98304 | 102264832 | exact |
| atoms-1000 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.4491e-03 | 1.6255e-03 | 1.3333e-04 | 0.092 | 6.9010e+02 | 2.9365e+01 | 126976 | 106160128 | exact |
| atoms-1000 | insert | cold-file | readcon-db | unset | 8 | 12 | 1.0882e-03 | 1.1700e-03 | 7.7170e-05 | 0.071 | 9.1893e+02 | 3.9102e+01 | 98304 | 102473728 | exact |
| atoms-1000 | full_read | cold | readcon-db | 1 | 1 | 12 | 3.1582e-05 | 3.2923e-05 | 1.7453e-06 | 0.055 |  |  | 98304 | 100188160 | exact |
| atoms-1000 | full_read | cold | ase.db | 1 | 1 | 12 | 1.4765e-03 | 1.6763e-03 | 1.7324e-04 | 0.117 |  |  | 126976 | 129617920 | exact |
| atoms-1000 | full_read | cold | readcon-db | unset | 8 | 12 | 3.2193e-05 | 3.4115e-05 | 1.7431e-06 | 0.054 |  |  | 98304 | 100409344 | exact |
| atoms-1000 | selected | cold | readcon-db | 1 | 1 | 12 | 3.0359e-05 | 3.3204e-05 | 3.0684e-06 | 0.101 |  |  | 98304 | 100642816 | exact |
| atoms-1000 | selected | cold | ase.db | 1 | 1 | 12 | 1.4076e-03 | 1.7331e-03 | 2.2305e-04 | 0.158 |  |  | 126976 | 132243456 | exact |
| atoms-1000 | selected | cold | readcon-db | unset | 8 | 12 | 3.2738e-05 | 3.5382e-05 | 2.7410e-06 | 0.084 |  |  | 98304 | 100528128 | exact |
| atoms-1000 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.7391e-05 | 1.7670e-05 | 1.7266e-07 | 0.010 |  |  | 98304 | 100454400 | exact |
| atoms-1000 | full_read | warm | ase.db | 1 | 1 | 12 | 4.4648e-04 | 6.1648e-04 | 1.1503e-04 | 0.258 |  |  | 126976 | 191758336 | exact |
| atoms-1000 | full_read | warm | readcon-db | unset | 8 | 12 | 1.7143e-05 | 1.8839e-05 | 4.7869e-07 | 0.028 |  |  | 98304 | 100528128 | exact |
| atoms-1000 | selected | warm | readcon-db | 1 | 1 | 12 | 1.8024e-05 | 1.8746e-05 | 1.0624e-06 | 0.059 |  |  | 98304 | 100499456 | exact |
| atoms-1000 | selected | warm | ase.db | 1 | 1 | 12 | 5.3276e-04 | 6.8601e-04 | 1.1881e-04 | 0.223 |  |  | 126976 | 178720768 | exact |
| atoms-1000 | selected | warm | readcon-db | unset | 8 | 12 | 1.7360e-05 | 1.8013e-05 | 9.7232e-07 | 0.056 |  |  | 98304 | 100495360 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.6403e-03 | 3.6988e-03 | 1.0405e-04 | 0.029 | 2.7470e+02 | 1.2673e+02 | 589824 | 120422400 | exact |
| atoms-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.6051e-03 | 3.7175e-03 | 1.2912e-04 | 0.036 | 2.7738e+02 | 1.2796e+02 | 667648 | 111677440 | exact |
| atoms-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.5329e-03 | 3.7095e-03 | 1.6192e-04 | 0.046 | 2.8306e+02 | 1.3058e+02 | 589824 | 120692736 | exact |
| atoms-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.9847e-04 | 3.0169e-04 | 5.3541e-06 | 0.018 |  |  | 589824 | 104742912 | exact |
| atoms-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 5.0719e-03 | 5.1952e-03 | 3.0612e-04 | 0.060 |  |  | 667648 | 193093632 | exact |
| atoms-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 3.0051e-04 | 3.1420e-04 | 1.2762e-05 | 0.042 |  |  | 589824 | 104747008 | exact |
| atoms-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 2.8542e-04 | 2.9384e-04 | 3.6408e-06 | 0.013 |  |  | 589824 | 104796160 | exact |
| atoms-10000 | selected | cold | ase.db | 1 | 1 | 12 | 4.1326e-03 | 4.2073e-03 | 7.2077e-05 | 0.017 |  |  | 667648 | 212185088 | exact |
| atoms-10000 | selected | cold | readcon-db | unset | 8 | 12 | 2.9374e-04 | 3.0060e-04 | 5.0038e-06 | 0.017 |  |  | 589824 | 104534016 | exact |
| atoms-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.6324e-04 | 2.7217e-04 | 4.4044e-06 | 0.017 |  |  | 589824 | 103911424 | exact |
| atoms-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.2555e-03 | 3.3259e-03 | 7.0188e-05 | 0.022 |  |  | 667648 | 238534656 | exact |
| atoms-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.6123e-04 | 2.6467e-04 | 1.3722e-06 | 0.005 |  |  | 589824 | 103919616 | exact |
| atoms-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.7114e-04 | 2.7634e-04 | 4.1021e-06 | 0.015 |  |  | 589824 | 103915520 | exact |
| atoms-10000 | selected | warm | ase.db | 1 | 1 | 12 | 3.2871e-03 | 3.5567e-03 | 2.0927e-04 | 0.064 |  |  | 667648 | 235982848 | exact |
| atoms-10000 | selected | warm | readcon-db | unset | 8 | 12 | 2.6297e-04 | 2.7050e-04 | 8.2237e-06 | 0.031 |  |  | 589824 | 104013824 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 2.6674e-02 | 2.7894e-02 | 1.1950e-03 | 0.045 | 3.7490e+01 | 1.8779e+02 | 5390336 | 197074944 | exact |
| atoms-100000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.6201e-02 | 2.6932e-02 | 1.1739e-03 | 0.045 | 3.8166e+01 | 1.9118e+02 | 6074368 | 206045184 | exact |
| atoms-100000 | insert | cold-file | readcon-db | unset | 8 | 12 | 2.7361e-02 | 2.8612e-02 | 1.0190e-03 | 0.037 | 3.6548e+01 | 1.8307e+02 | 5390336 | 196935680 | exact |
| atoms-100000 | full_read | cold | readcon-db | 1 | 1 | 12 | 5.9445e-03 | 6.7111e-03 | 6.7426e-04 | 0.113 |  |  | 5390336 | 135204864 | exact |
| atoms-100000 | full_read | cold | ase.db | 1 | 1 | 12 | 4.2738e-02 | 4.3749e-02 | 1.4822e-03 | 0.035 |  |  | 6074368 | 303337472 | exact |
| atoms-100000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.5986e-03 | 6.6734e-03 | 1.0910e-03 | 0.195 |  |  | 5390336 | 134864896 | exact |
| atoms-100000 | selected | cold | readcon-db | 1 | 1 | 12 | 5.2646e-03 | 5.7672e-03 | 2.8625e-04 | 0.054 |  |  | 5390336 | 134885376 | exact |
| atoms-100000 | selected | cold | ase.db | 1 | 1 | 12 | 3.9797e-02 | 4.1858e-02 | 2.0438e-03 | 0.051 |  |  | 6074368 | 310489088 | exact |
| atoms-100000 | selected | cold | readcon-db | unset | 8 | 12 | 5.9200e-03 | 6.5357e-03 | 4.1009e-04 | 0.069 |  |  | 5390336 | 134893568 | exact |
| atoms-100000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.7593e-03 | 2.8457e-03 | 6.2632e-05 | 0.023 |  |  | 5390336 | 128765952 | exact |
| atoms-100000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.2714e-02 | 3.5044e-02 | 2.5667e-03 | 0.078 |  |  | 6074368 | 295047168 | exact |
| atoms-100000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.6196e-03 | 2.7465e-03 | 1.4670e-04 | 0.056 |  |  | 5390336 | 128753664 | exact |
| atoms-100000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.6140e-03 | 2.8026e-03 | 1.3920e-04 | 0.053 |  |  | 5390336 | 128688128 | exact |
| atoms-100000 | selected | warm | ase.db | 1 | 1 | 12 | 3.0443e-02 | 3.2548e-02 | 1.3832e-03 | 0.045 |  |  | 6074368 | 306900992 | exact |
| atoms-100000 | selected | warm | readcon-db | unset | 8 | 12 | 2.5816e-03 | 2.7307e-03 | 6.3057e-05 | 0.024 |  |  | 5390336 | 128716800 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.0068e-01 | 3.0461e-01 | 4.8413e-03 | 0.016 | 3.3258e+00 | 1.7988e+02 | 52592640 | 733409280 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 1 | 1 | 12 | 2.7241e-01 | 2.8690e-01 | 1.8798e-02 | 0.069 | 3.6709e+00 | 1.9855e+02 | 60125184 | 1107730432 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.1317e-01 | 3.2308e-01 | 1.2437e-02 | 0.040 | 3.1932e+00 | 1.7271e+02 | 52592640 | 733609984 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.7906e-02 | 7.1581e-02 | 3.6651e-03 | 0.054 |  |  | 52592640 | 427413504 | exact |
| atoms-1000000 | full_read | cold | ase.db | 1 | 1 | 12 | 3.6691e-01 | 4.2669e-01 | 2.0974e-02 | 0.057 |  |  | 60125184 | 1321361408 | exact |
| atoms-1000000 | full_read | cold | readcon-db | unset | 8 | 12 | 5.7103e-02 | 6.4102e-02 | 5.2301e-03 | 0.092 |  |  | 52592640 | 427376640 | exact |
| atoms-1000000 | selected | cold | readcon-db | 1 | 1 | 12 | 6.2959e-02 | 7.6040e-02 | 1.8639e-02 | 0.296 |  |  | 52592640 | 427413504 | exact |
| atoms-1000000 | selected | cold | ase.db | 1 | 1 | 12 | 3.4778e-01 | 3.5273e-01 | 6.9687e-03 | 0.020 |  |  | 60125184 | 1320554496 | exact |
| atoms-1000000 | selected | cold | readcon-db | unset | 8 | 12 | 6.4208e-02 | 6.9725e-02 | 3.9638e-03 | 0.062 |  |  | 52592640 | 427261952 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 1 | 1 | 12 | 2.6729e-02 | 2.9962e-02 | 1.8853e-03 | 0.071 |  |  | 52592640 | 364539904 | exact |
| atoms-1000000 | full_read | warm | ase.db | 1 | 1 | 12 | 3.0543e-01 | 3.1430e-01 | 1.1571e-02 | 0.038 |  |  | 60125184 | 1330630656 | exact |
| atoms-1000000 | full_read | warm | readcon-db | unset | 8 | 12 | 2.7089e-02 | 3.1864e-02 | 3.6506e-03 | 0.135 |  |  | 52592640 | 364310528 | exact |
| atoms-1000000 | selected | warm | readcon-db | 1 | 1 | 12 | 2.6929e-02 | 2.9593e-02 | 2.2352e-03 | 0.083 |  |  | 52592640 | 364662784 | exact |
| atoms-1000000 | selected | warm | ase.db | 1 | 1 | 12 | 3.2162e-01 | 3.4409e-01 | 1.3521e-02 | 0.042 |  |  | 60125184 | 1330528256 | exact |
| atoms-1000000 | selected | warm | readcon-db | unset | 8 | 12 | 2.8744e-02 | 3.9537e-02 | 6.0615e-03 | 0.211 |  |  | 52592640 | 364482560 | exact |
| frames-1 | insert | cold-file | readcon-db | 1 | 1 | 12 | 6.9127e-04 | 8.6272e-04 | 1.1903e-04 | 0.172 | 1.4466e+03 | 1.0488e+00 | 40960 | 100048896 | exact |
| frames-1 | insert | cold-file | ase.db | 1 | 1 | 12 | 1.0541e-03 | 1.1500e-03 | 6.9889e-05 | 0.066 | 9.4868e+02 | 6.8780e-01 | 69632 | 105000960 | exact |
| frames-1 | insert | cold-file | readcon-db | unset | 8 | 12 | 6.6048e-04 | 7.8872e-04 | 1.4080e-04 | 0.213 | 1.5141e+03 | 1.0977e+00 | 40960 | 99663872 | exact |
| frames-1 | full_read | cold | readcon-db | 1 | 1 | 12 | 2.1923e-05 | 2.2252e-05 | 2.4990e-07 | 0.011 |  |  | 40960 | 98353152 | exact |
| frames-1 | full_read | cold | ase.db | 1 | 1 | 12 | 4.8326e-04 | 4.9745e-04 | 1.4687e-05 | 0.030 |  |  | 69632 | 104185856 | exact |
| frames-1 | full_read | cold | readcon-db | unset | 8 | 12 | 2.2086e-05 | 2.3230e-05 | 5.9589e-07 | 0.027 |  |  | 40960 | 98578432 | exact |
| frames-1 | selected | cold | readcon-db | 1 | 1 | 12 | 2.1800e-05 | 2.4000e-05 | 1.4898e-06 | 0.068 |  |  | 40960 | 98222080 | exact |
| frames-1 | selected | cold | ase.db | 1 | 1 | 12 | 6.8373e-04 | 8.0608e-04 | 1.1467e-04 | 0.168 |  |  | 69632 | 102621184 | exact |
| frames-1 | selected | cold | readcon-db | unset | 8 | 12 | 2.1749e-05 | 2.3494e-05 | 8.2413e-07 | 0.038 |  |  | 40960 | 98271232 | exact |
| frames-1 | full_read | warm | readcon-db | 1 | 1 | 12 | 1.1853e-05 | 1.1984e-05 | 1.5993e-07 | 0.013 |  |  | 40960 | 98279424 | exact |
| frames-1 | full_read | warm | ase.db | 1 | 1 | 12 | 1.5349e-04 | 1.5782e-04 | 3.4835e-06 | 0.023 |  |  | 69632 | 109187072 | exact |
| frames-1 | full_read | warm | readcon-db | unset | 8 | 12 | 1.1863e-05 | 1.1975e-05 | 1.3129e-07 | 0.011 |  |  | 40960 | 98037760 | exact |
| frames-1 | selected | warm | readcon-db | 1 | 1 | 12 | 1.1631e-05 | 1.1809e-05 | 1.6672e-07 | 0.014 |  |  | 40960 | 98209792 | exact |
| frames-1 | selected | warm | ase.db | 1 | 1 | 12 | 1.6983e-04 | 1.7757e-04 | 2.0046e-06 | 0.012 |  |  | 69632 | 108191744 | exact |
| frames-1 | selected | warm | readcon-db | unset | 8 | 12 | 1.1557e-05 | 1.1707e-05 | 1.9128e-07 | 0.017 |  |  | 40960 | 98107392 | exact |
| frames-100 | insert | cold-file | readcon-db | 1 | 1 | 12 | 4.5581e-03 | 4.6839e-03 | 1.1924e-04 | 0.026 | 2.1939e+04 | 1.6152e+01 | 65536 | 113094656 | exact |
| frames-100 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.2205e-02 | 3.3762e-02 | 1.5224e-03 | 0.047 | 3.1051e+03 | 2.2862e+00 | 151552 | 100659200 | exact |
| frames-100 | insert | cold-file | readcon-db | unset | 8 | 12 | 4.4942e-03 | 4.5411e-03 | 5.0200e-05 | 0.011 | 2.2251e+04 | 1.6382e+01 | 65536 | 114081792 | exact |
| frames-100 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.8206e-05 | 7.0138e-05 | 1.0012e-06 | 0.015 |  |  | 65536 | 105345024 | exact |
| frames-100 | full_read | cold | ase.db | 1 | 1 | 12 | 9.9214e-03 | 1.0177e-02 | 3.1618e-04 | 0.032 |  |  | 151552 | 118136832 | exact |
| frames-100 | full_read | cold | readcon-db | unset | 8 | 12 | 6.8188e-05 | 7.2795e-05 | 3.5048e-06 | 0.051 |  |  | 65536 | 105299968 | exact |
| frames-100 | selected | cold | readcon-db | 1 | 1 | 12 | 2.1195e-05 | 2.5753e-05 | 1.0800e-06 | 0.051 |  |  | 65536 | 103026688 | exact |
| frames-100 | selected | cold | ase.db | 1 | 1 | 12 | 8.1694e-04 | 9.7579e-04 | 1.7060e-04 | 0.209 |  |  | 151552 | 101662720 | exact |
| frames-100 | selected | cold | readcon-db | unset | 8 | 12 | 2.0787e-05 | 2.3274e-05 | 9.0727e-07 | 0.044 |  |  | 65536 | 102731776 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | 1 | 1 | 12 | 6.7925e-05 | 6.9350e-05 | 1.6068e-06 | 0.024 |  |  | 65536 | 105361408 | exact |
| frames-100 | full_read | warm-after-open | ase.db | 1 | 1 | 12 | 9.8950e-03 | 1.0169e-02 | 3.5816e-04 | 0.036 |  |  | 151552 | 118435840 | exact |
| frames-100 | full_read | warm-after-open | readcon-db | unset | 8 | 12 | 7.2692e-05 | 7.4202e-05 | 3.8394e-06 | 0.053 |  |  | 65536 | 105340928 | exact |
| frames-100 | selected | warm-after-open | readcon-db | 1 | 1 | 12 | 2.1732e-05 | 2.2647e-05 | 1.4350e-06 | 0.066 |  |  | 65536 | 102838272 | exact |
| frames-100 | selected | warm-after-open | ase.db | 1 | 1 | 12 | 8.2554e-04 | 9.5953e-04 | 1.3989e-04 | 0.169 |  |  | 151552 | 101883904 | exact |
| frames-100 | selected | warm-after-open | readcon-db | unset | 8 | 12 | 2.0597e-05 | 2.1219e-05 | 7.2719e-07 | 0.035 |  |  | 65536 | 103014400 | exact |
| frames-100 | full_read | cold-open | readcon-db | 1 | 1 | 12 | 3.5096e-04 | 3.9025e-04 | 5.2025e-05 | 0.148 |  |  | 65536 | 105320448 | exact |
| frames-100 | full_read | cold-open | ase.db | 1 | 1 | 12 | 1.0469e-02 | 1.0881e-02 | 4.1075e-04 | 0.039 |  |  | 151552 | 116359168 | exact |
| frames-100 | full_read | cold-open | readcon-db | unset | 8 | 12 | 3.2218e-04 | 3.5717e-04 | 3.6775e-05 | 0.114 |  |  | 65536 | 105361408 | exact |
| frames-100 | selected | cold-open | readcon-db | 1 | 1 | 12 | 2.6680e-04 | 3.1178e-04 | 2.1383e-05 | 0.080 |  |  | 65536 | 102780928 | exact |
| frames-100 | selected | cold-open | ase.db | 1 | 1 | 12 | 9.7806e-04 | 1.1389e-03 | 1.1942e-04 | 0.122 |  |  | 151552 | 101343232 | exact |
| frames-100 | selected | cold-open | readcon-db | unset | 8 | 12 | 2.4988e-04 | 2.9672e-04 | 5.0429e-05 | 0.202 |  |  | 65536 | 102670336 | exact |
| frames-100 | full_read | warm | readcon-db | 1 | 1 | 12 | 4.3289e-05 | 4.3871e-05 | 2.9613e-07 | 0.007 |  |  | 65536 | 105005056 | exact |
| frames-100 | full_read | warm | ase.db | 1 | 1 | 12 | 8.4406e-03 | 8.5158e-03 | 6.1431e-05 | 0.007 |  |  | 151552 | 119627776 | exact |
| frames-100 | full_read | warm | readcon-db | unset | 8 | 12 | 4.3834e-05 | 4.4095e-05 | 1.6558e-07 | 0.004 |  |  | 65536 | 104935424 | exact |
| frames-100 | selected | warm | readcon-db | 1 | 1 | 12 | 9.2653e-06 | 9.3108e-06 | 5.0275e-08 | 0.005 |  |  | 65536 | 102715392 | exact |
| frames-100 | selected | warm | ase.db | 1 | 1 | 12 | 1.7285e-04 | 1.7426e-04 | 1.1605e-06 | 0.007 |  |  | 151552 | 108105728 | exact |
| frames-100 | selected | warm | readcon-db | unset | 8 | 12 | 9.9498e-06 | 1.0092e-05 | 1.4110e-07 | 0.014 |  |  | 65536 | 102694912 | exact |
| frames-10000 | insert | cold-file | readcon-db | 1 | 1 | 12 | 3.2505e-01 | 3.2686e-01 | 2.0537e-03 | 0.006 | 3.0765e+04 | 2.3610e+01 | 3362816 | 196784128 | exact |
| frames-10000 | insert | cold-file | ase.db | 1 | 1 | 12 | 3.0336e+00 | 3.1277e+00 | 9.5014e-02 | 0.031 | 3.2965e+03 | 2.5299e+00 | 10817536 | 201084928 | exact |
| frames-10000 | insert | cold-file | readcon-db | unset | 8 | 12 | 3.2602e-01 | 3.2988e-01 | 4.2284e-03 | 0.013 | 3.0673e+04 | 2.3540e+01 | 3362816 | 187109376 | exact |
| frames-10000 | full_read | cold | readcon-db | 1 | 1 | 12 | 6.1076e-03 | 6.6539e-03 | 1.1173e-04 | 0.018 |  |  | 3362816 | 160907264 | exact |
| frames-10000 | full_read | cold | ase.db | 1 | 1 | 12 | 9.9917e-01 | 1.0446e+00 | 3.2415e-02 | 0.032 |  |  | 10801152 | 533704704 | exact |
| frames-10000 | full_read | cold | readcon-db | unset | 8 | 12 | 6.1848e-03 | 6.4091e-03 | 2.1188e-04 | 0.034 |  |  | 3362816 | 160862208 | exact |
| frames-10000 | selected | cold | readcon-db | 1 | 1 | 12 | 4.4600e-05 | 4.6427e-05 | 1.4547e-06 | 0.033 |  |  | 3362816 | 117346304 | exact |
| frames-10000 | selected | cold | ase.db | 1 | 1 | 12 | 3.7136e-03 | 4.1137e-03 | 4.3851e-04 | 0.118 |  |  | 10801152 | 99471360 | exact |
| frames-10000 | selected | cold | readcon-db | unset | 8 | 12 | 4.6375e-05 | 4.9855e-05 | 3.6366e-06 | 0.078 |  |  | 3362816 | 117444608 | exact |
| frames-10000 | full_read | warm | readcon-db | 1 | 1 | 12 | 6.4245e-03 | 6.9563e-03 | 3.1097e-04 | 0.048 |  |  | 3362816 | 179933184 | exact |
| frames-10000 | full_read | warm | ase.db | 1 | 1 | 12 | 1.0022e+00 | 1.0358e+00 | 3.5242e-02 | 0.035 |  |  | 10801152 | 552251392 | exact |
| frames-10000 | full_read | warm | readcon-db | unset | 8 | 12 | 6.2887e-03 | 6.5041e-03 | 3.1529e-04 | 0.050 |  |  | 3362816 | 179703808 | exact |
| frames-10000 | selected | warm | readcon-db | 1 | 1 | 12 | 9.8536e-06 | 9.9609e-06 | 1.4301e-07 | 0.015 |  |  | 3362816 | 113393664 | exact |
| frames-10000 | selected | warm | ase.db | 1 | 1 | 12 | 7.4347e-04 | 7.7537e-04 | 1.7866e-05 | 0.024 |  |  | 10801152 | 101072896 | exact |
| frames-10000 | selected | warm | readcon-db | unset | 8 | 12 | 1.0021e-05 | 1.0169e-05 | 6.2614e-08 | 0.006 |  |  | 3362816 | 113303552 | exact |

