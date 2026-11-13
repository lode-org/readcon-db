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
| readcon-db | 2.6874e-03 | 2.1827e-03 | 1.9534e-03 | 4.1361e-03 | 1.7727e-03 | 4.6722e-03 |
| ase.db | 5.6197e-02 | 9.4726e-03 | 5.2101e-02 | 6.1573e-02 | 4.5377e-02 | 7.3677e-02 |

## This machine, fair harness

Insert unlinks and creates the database file, then times one
transaction and one commit fsync. Full read and the selected-row
query (`frame_index` of the middle frame) are cold and warm.

| workload | frames | atoms | op | cache | tool | median s | IQR s |
| --- | --- | --- | --- | --- | --- | --- | --- |
| paper40x8 | 40 | 8 | insert | cold-file | readcon-db | 1.7620e-03 | 7.7161e-04 |
| paper40x8 | 40 | 8 | insert | cold-file | ase.db | 5.1976e-03 | 1.0613e-03 |
| atoms-1000 | 1 | 1000 | insert | cold-file | readcon-db | 8.1422e-04 | 6.7161e-04 |
| atoms-1000 | 1 | 1000 | insert | cold-file | ase.db | 2.0257e-03 | 1.1206e-03 |
| atoms-1000 | 1 | 1000 | full_read | cold | readcon-db | 5.8942e-04 | 1.2249e-04 |
| atoms-1000 | 1 | 1000 | full_read | cold | ase.db | 9.7351e-04 | 5.0341e-04 |
| atoms-1000 | 1 | 1000 | full_read | warm | readcon-db | 1.6906e-04 | 3.3751e-05 |
| atoms-1000 | 1 | 1000 | full_read | warm | ase.db | 3.9013e-04 | 3.9870e-05 |
| atoms-1000 | 1 | 1000 | selected | cold | readcon-db | 6.2709e-04 | 1.2324e-04 |
| atoms-1000 | 1 | 1000 | selected | cold | ase.db | 2.1578e-03 | 2.0367e-04 |
| atoms-1000 | 1 | 1000 | selected | warm | readcon-db | 1.7313e-04 | 2.8277e-05 |
| atoms-1000 | 1 | 1000 | selected | warm | ase.db | 3.8997e-04 | 3.9160e-05 |
| atoms-10000 | 1 | 10000 | insert | cold-file | readcon-db | 2.8798e-03 | 7.9794e-04 |
| atoms-10000 | 1 | 10000 | insert | cold-file | ase.db | 3.8478e-03 | 8.8794e-04 |
| atoms-10000 | 1 | 10000 | full_read | cold | readcon-db | 1.4807e-03 | 3.5574e-05 |
| atoms-10000 | 1 | 10000 | full_read | cold | ase.db | 3.2137e-03 | 1.1254e-03 |
| atoms-10000 | 1 | 10000 | full_read | warm | readcon-db | 1.1502e-03 | 2.6113e-05 |
| atoms-10000 | 1 | 10000 | full_read | warm | ase.db | 2.2294e-03 | 3.8222e-05 |
| atoms-10000 | 1 | 10000 | selected | cold | readcon-db | 1.8654e-03 | 2.6235e-04 |
| atoms-10000 | 1 | 10000 | selected | cold | ase.db | 4.6426e-03 | 1.9284e-04 |
| atoms-10000 | 1 | 10000 | selected | warm | readcon-db | 1.1492e-03 | 1.5145e-05 |
| atoms-10000 | 1 | 10000 | selected | warm | ase.db | 2.2151e-03 | 6.5279e-05 |
| atoms-100000 | 1 | 100000 | insert | cold-file | readcon-db | 1.7227e-02 | 7.9403e-04 |
| atoms-100000 | 1 | 100000 | insert | cold-file | ase.db | 2.8879e-02 | 1.3601e-03 |
| atoms-100000 | 1 | 100000 | full_read | cold | readcon-db | 1.3636e-02 | 5.0774e-04 |
| atoms-100000 | 1 | 100000 | full_read | cold | ase.db | 2.6575e-02 | 1.6049e-03 |
| atoms-100000 | 1 | 100000 | full_read | warm | readcon-db | 1.2698e-02 | 3.1625e-04 |
| atoms-100000 | 1 | 100000 | full_read | warm | ase.db | 2.0858e-02 | 4.7933e-04 |
| atoms-100000 | 1 | 100000 | selected | cold | readcon-db | 1.3707e-02 | 4.2755e-04 |
| atoms-100000 | 1 | 100000 | selected | cold | ase.db | 2.5965e-02 | 8.8113e-04 |
| atoms-100000 | 1 | 100000 | selected | warm | readcon-db | 1.2635e-02 | 1.4449e-04 |
| atoms-100000 | 1 | 100000 | selected | warm | ase.db | 2.0653e-02 | 9.9012e-05 |
| atoms-1000000 | 1 | 1000000 | insert | cold-file | readcon-db | 1.5473e-01 | 5.2366e-03 |
| atoms-1000000 | 1 | 1000000 | insert | cold-file | ase.db | 2.4229e-01 | 3.5085e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | cold | readcon-db | 1.7644e-01 | 3.0657e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | cold | ase.db | 2.6709e-01 | 2.1135e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | warm | readcon-db | 1.4985e-01 | 1.2222e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | warm | ase.db | 2.2180e-01 | 1.0867e-03 |
| atoms-1000000 | 1 | 1000000 | selected | cold | readcon-db | 1.5683e-01 | 7.9770e-03 |
| atoms-1000000 | 1 | 1000000 | selected | cold | ase.db | 2.5138e-01 | 2.1830e-02 |
| atoms-1000000 | 1 | 1000000 | selected | warm | readcon-db | 1.5379e-01 | 1.5787e-02 |
| atoms-1000000 | 1 | 1000000 | selected | warm | ase.db | 2.1767e-01 | 2.9532e-03 |
| frames-1 | 1 | 8 | insert | cold-file | readcon-db | 1.1870e-03 | 8.3831e-04 |
| frames-1 | 1 | 8 | insert | cold-file | ase.db | 1.3959e-03 | 1.1171e-03 |
| frames-1 | 1 | 8 | full_read | cold | readcon-db | 5.4662e-04 | 3.4817e-04 |
| frames-1 | 1 | 8 | full_read | cold | ase.db | 9.8512e-04 | 1.3685e-04 |
| frames-1 | 1 | 8 | full_read | warm | readcon-db | 6.1511e-05 | 2.7700e-05 |
| frames-1 | 1 | 8 | full_read | warm | ase.db | 1.5628e-04 | 2.8732e-05 |
| frames-1 | 1 | 8 | selected | cold | readcon-db | 4.9757e-04 | 1.1769e-04 |
| frames-1 | 1 | 8 | selected | cold | ase.db | 1.2420e-03 | 1.5726e-04 |
| frames-1 | 1 | 8 | selected | warm | readcon-db | 6.4638e-05 | 2.5294e-05 |
| frames-1 | 1 | 8 | selected | warm | ase.db | 1.6664e-04 | 9.6607e-06 |
| frames-100 | 100 | 8 | insert | cold-file | readcon-db | 4.5526e-03 | 1.6760e-04 |
| frames-100 | 100 | 8 | insert | cold-file | ase.db | 2.8538e-02 | 7.1607e-04 |
| frames-100 | 100 | 8 | full_read | cold | readcon-db | 1.4113e-03 | 1.6795e-04 |
| frames-100 | 100 | 8 | full_read | cold | ase.db | 8.7110e-03 | 4.1440e-04 |
| frames-100 | 100 | 8 | full_read | warm | readcon-db | 7.8865e-04 | 4.5814e-05 |
| frames-100 | 100 | 8 | full_read | warm | ase.db | 7.3090e-03 | 8.1895e-05 |
| frames-100 | 100 | 8 | selected | cold | readcon-db | 6.9678e-04 | 1.0542e-04 |
| frames-100 | 100 | 8 | selected | cold | ase.db | 1.5901e-03 | 3.3135e-04 |
| frames-100 | 100 | 8 | selected | warm | readcon-db | 7.5478e-05 | 2.1379e-05 |
| frames-100 | 100 | 8 | selected | warm | ase.db | 1.8021e-04 | 3.4707e-05 |
| frames-10000 | 10000 | 8 | insert | cold-file | readcon-db | 2.4749e-01 | 4.2821e-04 |
| frames-10000 | 10000 | 8 | insert | cold-file | ase.db | 2.6212e+00 | 2.4060e-02 |
| frames-10000 | 10000 | 8 | full_read | cold | readcon-db | 1.0045e-01 | 7.4485e-03 |
| frames-10000 | 10000 | 8 | full_read | cold | ase.db | 7.8797e-01 | 1.5549e-02 |
| frames-10000 | 10000 | 8 | full_read | warm | readcon-db | 9.0311e-02 | 6.2910e-03 |
| frames-10000 | 10000 | 8 | full_read | warm | ase.db | 7.7972e-01 | 1.6789e-02 |
| frames-10000 | 10000 | 8 | selected | cold | readcon-db | 4.8986e-04 | 4.0691e-05 |
| frames-10000 | 10000 | 8 | selected | cold | ase.db | 3.0725e-03 | 1.5973e-04 |
| frames-10000 | 10000 | 8 | selected | warm | readcon-db | 9.1688e-05 | 3.6548e-05 |
| frames-10000 | 10000 | 8 | selected | warm | ase.db | 7.5990e-04 | 3.6806e-05 |

