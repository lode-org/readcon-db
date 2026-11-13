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
| readcon-db | 1.3798e-02 | 5.0171e-03 | 1.1341e-02 | 1.6358e-02 | 1.1101e-02 | 1.6891e-02 |
| ase.db | 4.2128e-02 | 2.8031e-03 | 3.9763e-02 | 4.2566e-02 | 3.8626e-02 | 4.2624e-02 |

## This machine, fair harness

Insert unlinks and creates the database file, then times one
transaction and one commit fsync. Full read and the selected-row
query (`frame_index` of the middle frame) are cold and warm.

| workload | frames | atoms | op | cache | tool | median s | IQR s |
| --- | --- | --- | --- | --- | --- | --- | --- |
| paper40x8 | 40 | 8 | insert | cold-file | readcon-db | 1.7944e-03 | 9.2241e-04 |
| paper40x8 | 40 | 8 | insert | cold-file | ase.db | 5.2650e-03 | 3.7063e-04 |
| atoms-1000 | 1 | 1000 | insert | cold-file | readcon-db | 6.8513e-04 | 1.2617e-04 |
| atoms-1000 | 1 | 1000 | insert | cold-file | ase.db | 1.3787e-03 | 1.3303e-04 |
| atoms-1000 | 1 | 1000 | full_read | cold | readcon-db | 9.2259e-03 | 2.6530e-04 |
| atoms-1000 | 1 | 1000 | full_read | cold | ase.db | 1.4321e-03 | 4.8376e-04 |
| atoms-1000 | 1 | 1000 | full_read | warm | readcon-db | 6.6131e-04 | 6.1528e-05 |
| atoms-1000 | 1 | 1000 | full_read | warm | ase.db | 4.8160e-04 | 5.0889e-05 |
| atoms-1000 | 1 | 1000 | selected | cold | readcon-db | 9.1746e-03 | 4.4204e-04 |
| atoms-1000 | 1 | 1000 | selected | cold | ase.db | 1.6660e-03 | 1.1176e-03 |
| atoms-1000 | 1 | 1000 | selected | warm | readcon-db | 6.5642e-04 | 2.1447e-05 |
| atoms-1000 | 1 | 1000 | selected | warm | ase.db | 4.7164e-04 | 7.0456e-05 |
| atoms-10000 | 1 | 10000 | insert | cold-file | readcon-db | 3.0491e-03 | 2.2860e-04 |
| atoms-10000 | 1 | 10000 | insert | cold-file | ase.db | 3.6738e-03 | 2.2047e-04 |
| atoms-10000 | 1 | 10000 | full_read | cold | readcon-db | 1.6012e-02 | 1.0744e-03 |
| atoms-10000 | 1 | 10000 | full_read | cold | ase.db | 4.6453e-03 | 5.1313e-04 |
| atoms-10000 | 1 | 10000 | full_read | warm | readcon-db | 6.0218e-03 | 8.3783e-04 |
| atoms-10000 | 1 | 10000 | full_read | warm | ase.db | 2.4521e-03 | 4.7136e-05 |
| atoms-10000 | 1 | 10000 | selected | cold | readcon-db | 1.6006e-02 | 5.8817e-04 |
| atoms-10000 | 1 | 10000 | selected | cold | ase.db | 5.0094e-03 | 1.7636e-03 |
| atoms-10000 | 1 | 10000 | selected | warm | readcon-db | 6.3948e-03 | 6.2026e-04 |
| atoms-10000 | 1 | 10000 | selected | warm | ase.db | 2.5068e-03 | 7.9439e-05 |
| atoms-100000 | 1 | 100000 | insert | cold-file | readcon-db | 2.8704e-02 | 5.4049e-04 |
| atoms-100000 | 1 | 100000 | insert | cold-file | ase.db | 2.7718e-02 | 2.0285e-03 |
| atoms-100000 | 1 | 100000 | full_read | cold | readcon-db | 1.2432e-01 | 8.6152e-03 |
| atoms-100000 | 1 | 100000 | full_read | cold | ase.db | 3.0000e-02 | 3.3149e-03 |
| atoms-100000 | 1 | 100000 | full_read | warm | readcon-db | 1.2150e-01 | 8.5275e-03 |
| atoms-100000 | 1 | 100000 | full_read | warm | ase.db | 2.4595e-02 | 9.1596e-04 |
| atoms-100000 | 1 | 100000 | selected | cold | readcon-db | 1.2845e-01 | 1.1056e-02 |
| atoms-100000 | 1 | 100000 | selected | cold | ase.db | 2.9494e-02 | 1.1240e-03 |
| atoms-100000 | 1 | 100000 | selected | warm | readcon-db | 1.0964e-01 | 1.2254e-02 |
| atoms-100000 | 1 | 100000 | selected | warm | ase.db | 2.3778e-02 | 6.9030e-04 |
| atoms-1000000 | 1 | 1000000 | insert | cold-file | readcon-db | 3.1545e-01 | 1.2328e-02 |
| atoms-1000000 | 1 | 1000000 | insert | cold-file | ase.db | 2.5145e-01 | 8.6345e-03 |
| atoms-1000000 | 1 | 1000000 | full_read | cold | readcon-db | 1.2666e+00 | 3.3388e-01 |
| atoms-1000000 | 1 | 1000000 | full_read | cold | ase.db | 2.4705e-01 | 2.6221e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | warm | readcon-db | 1.2245e+00 | 3.2619e-01 |
| atoms-1000000 | 1 | 1000000 | full_read | warm | ase.db | 2.2126e-01 | 1.0778e-02 |
| atoms-1000000 | 1 | 1000000 | selected | cold | readcon-db | 1.1804e+00 | 2.2482e-01 |
| atoms-1000000 | 1 | 1000000 | selected | cold | ase.db | 2.4335e-01 | 1.1861e-02 |
| atoms-1000000 | 1 | 1000000 | selected | warm | readcon-db | 1.1355e+00 | 2.2311e-01 |
| atoms-1000000 | 1 | 1000000 | selected | warm | ase.db | 2.1889e-01 | 2.5740e-03 |
| frames-1 | 1 | 8 | insert | cold-file | readcon-db | 3.1920e-04 | 3.7074e-05 |
| frames-1 | 1 | 8 | insert | cold-file | ase.db | 1.0438e-03 | 2.1090e-04 |
| frames-1 | 1 | 8 | full_read | cold | readcon-db | 9.1298e-03 | 1.0819e-03 |
| frames-1 | 1 | 8 | full_read | cold | ase.db | 1.1221e-03 | 2.3558e-04 |
| frames-1 | 1 | 8 | full_read | warm | readcon-db | 7.0298e-05 | 3.0964e-05 |
| frames-1 | 1 | 8 | full_read | warm | ase.db | 1.7620e-04 | 3.5635e-05 |
| frames-1 | 1 | 8 | selected | cold | readcon-db | 8.4534e-03 | 1.7448e-04 |
| frames-1 | 1 | 8 | selected | cold | ase.db | 1.0460e-03 | 3.8289e-04 |
| frames-1 | 1 | 8 | selected | warm | readcon-db | 7.4864e-05 | 2.5901e-05 |
| frames-1 | 1 | 8 | selected | warm | ase.db | 1.8658e-04 | 2.8508e-05 |
| frames-100 | 100 | 8 | insert | cold-file | readcon-db | 4.2981e-03 | 1.1035e-04 |
| frames-100 | 100 | 8 | insert | cold-file | ase.db | 3.2309e-02 | 4.3528e-04 |
| frames-100 | 100 | 8 | full_read | cold | readcon-db | 1.3207e-02 | 1.7821e-04 |
| frames-100 | 100 | 8 | full_read | cold | ase.db | 9.7315e-03 | 2.0643e-04 |
| frames-100 | 100 | 8 | full_read | warm | readcon-db | 4.1531e-03 | 8.7913e-05 |
| frames-100 | 100 | 8 | full_read | warm | ase.db | 8.0336e-03 | 1.2645e-04 |
| frames-100 | 100 | 8 | selected | cold | readcon-db | 8.7295e-03 | 2.6316e-04 |
| frames-100 | 100 | 8 | selected | cold | ase.db | 1.7431e-03 | 5.1106e-04 |
| frames-100 | 100 | 8 | selected | warm | readcon-db | 1.0605e-04 | 3.3817e-05 |
| frames-100 | 100 | 8 | selected | warm | ase.db | 2.0625e-04 | 3.3997e-05 |
| frames-10000 | 10000 | 8 | insert | cold-file | readcon-db | 3.3014e-01 | 1.4177e-02 |
| frames-10000 | 10000 | 8 | insert | cold-file | ase.db | 3.1405e+00 | 1.0686e-01 |
| frames-10000 | 10000 | 8 | full_read | cold | readcon-db | 6.2228e-01 | 4.5779e-02 |
| frames-10000 | 10000 | 8 | full_read | cold | ase.db | 8.8536e-01 | 2.8274e-02 |
| frames-10000 | 10000 | 8 | full_read | warm | readcon-db | 5.9593e-01 | 2.1868e-02 |
| frames-10000 | 10000 | 8 | full_read | warm | ase.db | 8.9342e-01 | 4.1913e-02 |
| frames-10000 | 10000 | 8 | selected | cold | readcon-db | 3.1035e-02 | 1.0322e-02 |
| frames-10000 | 10000 | 8 | selected | cold | ase.db | 6.0739e-03 | 1.3620e-03 |
| frames-10000 | 10000 | 8 | selected | warm | readcon-db | 2.8559e-04 | 6.4759e-05 |
| frames-10000 | 10000 | 8 | selected | warm | ase.db | 8.5477e-04 | 1.2888e-04 |

