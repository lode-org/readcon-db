# readcon-db vs ASE sqlite

Type-7 median and IQR. One discarded warmup and six serial samples
for every fair row. The published log has no discarded warmup.
A mismatch of any stored field aborts the run, so every sample below
passed an element-wise check (fidelity exact).
Cold medians use samples whose mincore resident fraction is 0.

## Published ingest (`paper/cpc/logs/db.log`)

40 frames, 8 Cu atoms. ASE commits every row. No warmup.

| tool | median s | IQR s | q1 s | q3 s | min s | max s |
| --- | --- | --- | --- | --- | --- | --- |
| readcon-db | 4.6724e-03 | 2.0083e-03 | 3.6932e-03 | 5.7016e-03 | 3.3584e-03 | 6.2406e-03 |
| ase.db | 7.2318e-02 | 3.7693e-02 | 6.0244e-02 | 9.7937e-02 | 5.4239e-02 | 1.0352e-01 |

## This machine, published harness

| tool | median s | IQR s | q1 s | q3 s | min s | max s |
| --- | --- | --- | --- | --- | --- | --- |
| readcon-db | 2.7092e-03 | 4.4367e-04 | 2.4336e-03 | 2.8773e-03 | 1.7226e-03 | 4.3947e-03 |
| ase.db | 5.2341e-02 | 1.6237e-03 | 5.1764e-02 | 5.3388e-02 | 4.3007e-02 | 6.3197e-02 |

## This machine, fair harness

Insert unlinks and creates the database file, then times one
transaction and one commit fsync. Full read and the selected-row
query (`frame_index` of the middle frame) are cold and warm.

| workload | frames | atoms | op | cache | tool | median s | IQR s | fidelity |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| paper40x8 | 40 | 8 | insert | cold-file | readcon-db | 2.2414e-03 | 3.5408e-04 | exact |
| paper40x8 | 40 | 8 | insert | cold-file | ase.db | 6.9013e-03 | 4.1837e-04 | exact |
| atoms-1000 | 1 | 1000 | insert | cold-file | readcon-db | 9.9369e-04 | 9.7296e-04 | exact |
| atoms-1000 | 1 | 1000 | insert | cold-file | ase.db | 1.3630e-03 | 3.6849e-04 | exact |
| atoms-1000 | 1 | 1000 | full_read | cold | readcon-db | 6.3607e-04 | 6.9062e-04 | exact |
| atoms-1000 | 1 | 1000 | full_read | cold | ase.db | 1.6197e-03 | 5.0368e-04 | exact |
| atoms-1000 | 1 | 1000 | full_read | warm | readcon-db | 1.7909e-04 | 3.0712e-05 | exact |
| atoms-1000 | 1 | 1000 | full_read | warm | ase.db | 3.7518e-04 | 3.0656e-05 | exact |
| atoms-1000 | 1 | 1000 | selected | cold | readcon-db | 6.1108e-04 | 5.4787e-04 | exact |
| atoms-1000 | 1 | 1000 | selected | cold | ase.db | 2.0089e-03 | 1.3330e-03 | exact |
| atoms-1000 | 1 | 1000 | selected | warm | readcon-db | 1.7233e-04 | 2.2790e-05 | exact |
| atoms-1000 | 1 | 1000 | selected | warm | ase.db | 3.9533e-04 | 2.9694e-05 | exact |
| atoms-10000 | 1 | 10000 | insert | cold-file | readcon-db | 2.5294e-03 | 4.2428e-04 | exact |
| atoms-10000 | 1 | 10000 | insert | cold-file | ase.db | 3.3720e-03 | 4.7428e-04 | exact |
| atoms-10000 | 1 | 10000 | full_read | cold | readcon-db | 1.8658e-03 | 6.0417e-04 | exact |
| atoms-10000 | 1 | 10000 | full_read | cold | ase.db | 3.9628e-03 | 1.2350e-03 | exact |
| atoms-10000 | 1 | 10000 | full_read | warm | readcon-db | 1.1596e-03 | 2.3902e-05 | exact |
| atoms-10000 | 1 | 10000 | full_read | warm | ase.db | 2.2765e-03 | 6.9241e-05 | exact |
| atoms-10000 | 1 | 10000 | selected | cold | readcon-db | 1.5018e-03 | 1.6326e-04 | exact |
| atoms-10000 | 1 | 10000 | selected | cold | ase.db | 2.9799e-03 | 1.4628e-04 | exact |
| atoms-10000 | 1 | 10000 | selected | warm | readcon-db | 1.1517e-03 | 1.0753e-05 | exact |
| atoms-10000 | 1 | 10000 | selected | warm | ase.db | 2.2120e-03 | 2.4458e-05 | exact |
| atoms-100000 | 1 | 100000 | insert | cold-file | readcon-db | 1.5364e-02 | 5.3769e-05 | exact |
| atoms-100000 | 1 | 100000 | insert | cold-file | ase.db | 2.5796e-02 | 2.2352e-03 | exact |
| atoms-100000 | 1 | 100000 | full_read | cold | readcon-db | 1.3900e-02 | 3.1195e-04 | exact |
| atoms-100000 | 1 | 100000 | full_read | cold | ase.db | 2.4806e-02 | 1.9418e-03 | exact |
| atoms-100000 | 1 | 100000 | full_read | warm | readcon-db | 1.2701e-02 | 2.8253e-04 | exact |
| atoms-100000 | 1 | 100000 | full_read | warm | ase.db | 2.0325e-02 | 2.4041e-04 | exact |
| atoms-100000 | 1 | 100000 | selected | cold | readcon-db | 1.3954e-02 | 4.6287e-04 | exact |
| atoms-100000 | 1 | 100000 | selected | cold | ase.db | 2.5814e-02 | 1.7703e-03 | exact |
| atoms-100000 | 1 | 100000 | selected | warm | readcon-db | 1.2611e-02 | 2.1873e-05 | exact |
| atoms-100000 | 1 | 100000 | selected | warm | ase.db | 1.9965e-02 | 8.7945e-05 | exact |
| atoms-1000000 | 1 | 1000000 | insert | cold-file | readcon-db | 1.4661e-01 | 8.0963e-03 | exact |
| atoms-1000000 | 1 | 1000000 | insert | cold-file | ase.db | 2.3000e-01 | 1.2884e-02 | exact |
| atoms-1000000 | 1 | 1000000 | full_read | cold | readcon-db | 1.7588e-01 | 1.9745e-02 | exact |
| atoms-1000000 | 1 | 1000000 | full_read | cold | ase.db | 2.9023e-01 | 1.9448e-02 | exact |
| atoms-1000000 | 1 | 1000000 | full_read | warm | readcon-db | 1.4267e-01 | 1.9113e-02 | exact |
| atoms-1000000 | 1 | 1000000 | full_read | warm | ase.db | 2.1489e-01 | 1.4270e-03 | exact |
| atoms-1000000 | 1 | 1000000 | selected | cold | readcon-db | 1.6475e-01 | 2.4671e-02 | exact |
| atoms-1000000 | 1 | 1000000 | selected | cold | ase.db | 2.4675e-01 | 2.9473e-02 | exact |
| atoms-1000000 | 1 | 1000000 | selected | warm | readcon-db | 1.5952e-01 | 1.4623e-02 | exact |
| atoms-1000000 | 1 | 1000000 | selected | warm | ase.db | 2.3454e-01 | 2.6475e-02 | exact |
| frames-1 | 1 | 8 | insert | cold-file | readcon-db | 8.4894e-04 | 1.2291e-03 | exact |
| frames-1 | 1 | 8 | insert | cold-file | ase.db | 9.1642e-04 | 9.6942e-06 | exact |
| frames-1 | 1 | 8 | full_read | cold | readcon-db | 3.9799e-04 | 2.6583e-04 | exact |
| frames-1 | 1 | 8 | full_read | cold | ase.db | 7.6060e-04 | 2.3680e-04 | exact |
| frames-1 | 1 | 8 | full_read | warm | readcon-db | 6.1758e-05 | 2.7985e-05 | exact |
| frames-1 | 1 | 8 | full_read | warm | ase.db | 1.6360e-04 | 2.3094e-05 | exact |
| frames-1 | 1 | 8 | selected | cold | readcon-db | 8.8244e-04 | 2.2975e-04 | exact |
| frames-1 | 1 | 8 | selected | cold | ase.db | 1.0489e-03 | 7.3621e-04 | exact |
| frames-1 | 1 | 8 | selected | warm | readcon-db | 5.7076e-05 | 2.4089e-05 | exact |
| frames-1 | 1 | 8 | selected | warm | ase.db | 1.7586e-04 | 1.6922e-05 | exact |
| frames-100 | 100 | 8 | insert | cold-file | readcon-db | 4.4007e-03 | 4.9425e-04 | exact |
| frames-100 | 100 | 8 | insert | cold-file | ase.db | 2.8472e-02 | 8.5226e-04 | exact |
| frames-100 | 100 | 8 | full_read | cold | readcon-db | 1.2653e-03 | 3.5136e-04 | exact |
| frames-100 | 100 | 8 | full_read | cold | ase.db | 9.0371e-03 | 1.0903e-04 | exact |
| frames-100 | 100 | 8 | full_read | warm | readcon-db | 8.0907e-04 | 1.5132e-05 | exact |
| frames-100 | 100 | 8 | full_read | warm | ase.db | 7.5257e-03 | 1.4751e-04 | exact |
| frames-100 | 100 | 8 | selected | cold | readcon-db | 3.2959e-04 | 8.4037e-06 | exact |
| frames-100 | 100 | 8 | selected | cold | ase.db | 6.9088e-04 | 6.1426e-05 | exact |
| frames-100 | 100 | 8 | selected | warm | readcon-db | 7.4132e-05 | 1.8511e-05 | exact |
| frames-100 | 100 | 8 | selected | warm | ase.db | 1.8243e-04 | 2.2937e-05 | exact |
| frames-10000 | 10000 | 8 | insert | cold-file | readcon-db | 2.4507e-01 | 8.2573e-03 | exact |
| frames-10000 | 10000 | 8 | insert | cold-file | ase.db | 2.6766e+00 | 7.4420e-02 | exact |
| frames-10000 | 10000 | 8 | full_read | cold | readcon-db | 1.0334e-01 | 6.1855e-03 | exact |
| frames-10000 | 10000 | 8 | full_read | cold | ase.db | 7.8633e-01 | 3.6672e-02 | exact |
| frames-10000 | 10000 | 8 | full_read | warm | readcon-db | 8.0402e-02 | 1.1447e-03 | exact |
| frames-10000 | 10000 | 8 | full_read | warm | ase.db | 7.4806e-01 | 7.5234e-03 | exact |
| frames-10000 | 10000 | 8 | selected | cold | readcon-db | 4.8899e-04 | 5.6872e-04 | exact |
| frames-10000 | 10000 | 8 | selected | cold | ase.db | 3.6696e-03 | 1.4563e-03 | exact |
| frames-10000 | 10000 | 8 | selected | warm | readcon-db | 8.3625e-05 | 2.5371e-05 | exact |
| frames-10000 | 10000 | 8 | selected | warm | ase.db | 7.4968e-04 | 4.1090e-05 | exact |

