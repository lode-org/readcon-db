# readcon-db vs ASE sqlite

Type-7 median and IQR. One discarded warmup and six serial samples
for every fair row. The published log has no discarded warmup.
A mismatch of any stored field aborts the run, so every sample below
passed an element-wise check.

## Published ingest (`paper/cpc/logs/db.log`)

40 frames, 8 Cu atoms. ASE commits every row. No warmup.

| tool | median s | IQR s | q1 s | q3 s | min s | max s |
| --- | --- | --- | --- | --- | --- | --- |
| readcon-db | 4.6724e-03 | 2.0083e-03 | 3.6932e-03 | 5.7016e-03 | 3.3584e-03 | 6.2406e-03 |
| ase.db | 7.2318e-02 | 3.7693e-02 | 6.0244e-02 | 9.7937e-02 | 5.4239e-02 | 1.0352e-01 |

## This machine, published harness

| tool | median s | IQR s | q1 s | q3 s | min s | max s |
| --- | --- | --- | --- | --- | --- | --- |
| readcon-db | 2.6550e-03 | 1.0223e-03 | 2.1206e-03 | 3.1430e-03 | 1.8677e-03 | 3.7165e-03 |
| ase.db | 5.5272e-02 | 1.5970e-02 | 4.7022e-02 | 6.2992e-02 | 3.9345e-02 | 6.9438e-02 |

## This machine, fair harness

Insert unlinks and creates the database file, then times one
transaction and one commit fsync. Full read and the selected-row
query (`frame_index` of the middle frame) are cold and warm.

| workload | frames | atoms | op | cache | tool | median s | IQR s |
| --- | --- | --- | --- | --- | --- | --- | --- |
| paper40x8 | 40 | 8 | insert | cold-file | readcon-db | 1.5142e-03 | 8.3146e-04 |
| paper40x8 | 40 | 8 | insert | cold-file | ase.db | 5.1692e-03 | 6.1745e-04 |
| atoms-1000 | 1 | 1000 | insert | cold-file | readcon-db | 5.5414e-04 | 9.4438e-05 |
| atoms-1000 | 1 | 1000 | insert | cold-file | ase.db | 1.2715e-03 | 1.1174e-04 |
| atoms-1000 | 1 | 1000 | full_read | cold | readcon-db | 3.9200e-04 | 1.4137e-05 |
| atoms-1000 | 1 | 1000 | full_read | cold | ase.db | 8.6882e-04 | 4.1372e-05 |
| atoms-1000 | 1 | 1000 | full_read | warm | readcon-db | 1.7570e-04 | 2.2524e-05 |
| atoms-1000 | 1 | 1000 | full_read | warm | ase.db | 3.7631e-04 | 3.3253e-05 |
| atoms-1000 | 1 | 1000 | selected | cold | readcon-db | 4.8401e-04 | 2.0727e-04 |
| atoms-1000 | 1 | 1000 | selected | cold | ase.db | 1.4388e-03 | 1.1220e-03 |
| atoms-1000 | 1 | 1000 | selected | warm | readcon-db | 1.7330e-04 | 2.7723e-05 |
| atoms-1000 | 1 | 1000 | selected | warm | ase.db | 3.8449e-04 | 3.6073e-05 |
| atoms-10000 | 1 | 10000 | insert | cold-file | readcon-db | 3.5916e-03 | 2.6856e-04 |
| atoms-10000 | 1 | 10000 | insert | cold-file | ase.db | 3.5786e-03 | 8.4666e-05 |
| atoms-10000 | 1 | 10000 | full_read | cold | readcon-db | 1.8302e-03 | 1.2114e-04 |
| atoms-10000 | 1 | 10000 | full_read | cold | ase.db | 4.1270e-03 | 1.1361e-04 |
| atoms-10000 | 1 | 10000 | full_read | warm | readcon-db | 1.1346e-03 | 1.5136e-05 |
| atoms-10000 | 1 | 10000 | full_read | warm | ase.db | 2.1549e-03 | 2.0708e-05 |
| atoms-10000 | 1 | 10000 | selected | cold | readcon-db | 1.4306e-03 | 3.4943e-04 |
| atoms-10000 | 1 | 10000 | selected | cold | ase.db | 3.0609e-03 | 1.2246e-04 |
| atoms-10000 | 1 | 10000 | selected | warm | readcon-db | 1.1426e-03 | 2.9941e-05 |
| atoms-10000 | 1 | 10000 | selected | warm | ase.db | 2.1797e-03 | 2.8806e-05 |
| atoms-100000 | 1 | 100000 | insert | cold-file | readcon-db | 1.6631e-02 | 7.8670e-04 |
| atoms-100000 | 1 | 100000 | insert | cold-file | ase.db | 2.9169e-02 | 1.1200e-03 |
| atoms-100000 | 1 | 100000 | full_read | cold | readcon-db | 1.4071e-02 | 2.1661e-03 |
| atoms-100000 | 1 | 100000 | full_read | cold | ase.db | 2.8405e-02 | 2.3270e-03 |
| atoms-100000 | 1 | 100000 | full_read | warm | readcon-db | 1.3002e-02 | 2.9977e-04 |
| atoms-100000 | 1 | 100000 | full_read | warm | ase.db | 1.9939e-02 | 4.3838e-04 |
| atoms-100000 | 1 | 100000 | selected | cold | readcon-db | 2.1120e-02 | 4.7004e-03 |
| atoms-100000 | 1 | 100000 | selected | cold | ase.db | 4.6338e-02 | 5.1601e-03 |
| atoms-100000 | 1 | 100000 | selected | warm | readcon-db | 1.2702e-02 | 1.3390e-04 |
| atoms-100000 | 1 | 100000 | selected | warm | ase.db | 2.0065e-02 | 2.7279e-04 |
| atoms-1000000 | 1 | 1000000 | insert | cold-file | readcon-db | 1.5567e-01 | 9.1950e-03 |
| atoms-1000000 | 1 | 1000000 | insert | cold-file | ase.db | 2.3873e-01 | 1.3329e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | cold | readcon-db | 1.6045e-01 | 3.2362e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | cold | ase.db | 2.8136e-01 | 1.7122e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | warm | readcon-db | 1.4194e-01 | 1.6995e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | warm | ase.db | 2.1450e-01 | 3.2098e-03 |
| atoms-1000000 | 1 | 1000000 | selected | cold | readcon-db | 1.5712e-01 | 2.6864e-02 |
| atoms-1000000 | 1 | 1000000 | selected | cold | ase.db | 2.7020e-01 | 2.0939e-02 |
| atoms-1000000 | 1 | 1000000 | selected | warm | readcon-db | 1.6062e-01 | 3.3210e-02 |
| atoms-1000000 | 1 | 1000000 | selected | warm | ase.db | 2.3914e-01 | 4.3214e-02 |
| frames-1 | 1 | 8 | insert | cold-file | readcon-db | 3.4469e-04 | 9.3988e-05 |
| frames-1 | 1 | 8 | insert | cold-file | ase.db | 1.1501e-03 | 5.3805e-04 |
| frames-1 | 1 | 8 | full_read | cold | readcon-db | 2.8170e-04 | 1.3642e-05 |
| frames-1 | 1 | 8 | full_read | cold | ase.db | 5.5604e-04 | 6.8539e-05 |
| frames-1 | 1 | 8 | full_read | warm | readcon-db | 5.5273e-05 | 1.7054e-05 |
| frames-1 | 1 | 8 | full_read | warm | ase.db | 1.6221e-04 | 2.0420e-05 |
| frames-1 | 1 | 8 | selected | cold | readcon-db | 3.7157e-04 | 2.1371e-04 |
| frames-1 | 1 | 8 | selected | cold | ase.db | 7.8706e-04 | 3.4974e-04 |
| frames-1 | 1 | 8 | selected | warm | readcon-db | 5.6500e-05 | 1.7448e-05 |
| frames-1 | 1 | 8 | selected | warm | ase.db | 1.6734e-04 | 6.1292e-06 |
| frames-100 | 100 | 8 | insert | cold-file | readcon-db | 4.2209e-03 | 1.4303e-04 |
| frames-100 | 100 | 8 | insert | cold-file | ase.db | 2.7929e-02 | 2.8020e-04 |
| frames-100 | 100 | 8 | full_read | cold | readcon-db | 1.4220e-03 | 4.4169e-05 |
| frames-100 | 100 | 8 | full_read | cold | ase.db | 8.7578e-03 | 2.3943e-04 |
| frames-100 | 100 | 8 | full_read | warm | readcon-db | 8.1728e-04 | 5.3438e-05 |
| frames-100 | 100 | 8 | full_read | warm | ase.db | 7.2710e-03 | 7.5633e-05 |
| frames-100 | 100 | 8 | selected | cold | readcon-db | 6.8535e-04 | 7.0630e-05 |
| frames-100 | 100 | 8 | selected | cold | ase.db | 1.6577e-03 | 1.8473e-04 |
| frames-100 | 100 | 8 | selected | warm | readcon-db | 7.6439e-05 | 2.1768e-05 |
| frames-100 | 100 | 8 | selected | warm | ase.db | 1.7587e-04 | 9.6928e-06 |
| frames-10000 | 10000 | 8 | insert | cold-file | readcon-db | 2.5627e-01 | 2.7660e-03 |
| frames-10000 | 10000 | 8 | insert | cold-file | ase.db | 2.6262e+00 | 3.2509e-02 |
| frames-10000 | 10000 | 8 | full_read | cold | readcon-db | 9.9207e-02 | 7.0655e-03 |
| frames-10000 | 10000 | 8 | full_read | cold | ase.db | 7.4750e-01 | 2.6690e-03 |
| frames-10000 | 10000 | 8 | full_read | warm | readcon-db | 9.3433e-02 | 1.0488e-02 |
| frames-10000 | 10000 | 8 | full_read | warm | ase.db | 7.8066e-01 | 2.0144e-02 |
| frames-10000 | 10000 | 8 | selected | cold | readcon-db | 7.7101e-04 | 3.2431e-04 |
| frames-10000 | 10000 | 8 | selected | cold | ase.db | 4.1423e-03 | 1.1046e-03 |
| frames-10000 | 10000 | 8 | selected | warm | readcon-db | 9.3104e-05 | 2.8495e-05 |
| frames-10000 | 10000 | 8 | selected | warm | ase.db | 7.5932e-04 | 1.5485e-05 |

