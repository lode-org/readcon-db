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
| readcon-db | 2.4108e-03 | 1.8950e-03 | 2.0313e-03 | 3.9262e-03 | 1.9564e-03 | 5.2398e-03 |
| ase.db | 5.5651e-02 | 9.1565e-03 | 5.2877e-02 | 6.2034e-02 | 4.1248e-02 | 7.5531e-02 |

## This machine, fair harness

Insert unlinks and creates the database file, then times one
transaction and one commit fsync. Full read and the selected-row
query (`frame_index` of the middle frame) are cold and warm.

| workload | frames | atoms | op | cache | tool | median s | IQR s |
| --- | --- | --- | --- | --- | --- | --- | --- |
| paper40x8 | 40 | 8 | insert | cold-file | readcon-db | 2.3848e-03 | 1.0760e-03 |
| paper40x8 | 40 | 8 | insert | cold-file | ase.db | 5.9432e-03 | 1.2624e-03 |
| atoms-1000 | 1 | 1000 | insert | cold-file | readcon-db | 7.6093e-04 | 3.6910e-04 |
| atoms-1000 | 1 | 1000 | insert | cold-file | ase.db | 1.3459e-03 | 3.2727e-05 |
| atoms-1000 | 1 | 1000 | full_read | cold | readcon-db | 4.2446e-04 | 5.8111e-05 |
| atoms-1000 | 1 | 1000 | full_read | cold | ase.db | 9.9647e-04 | 3.6663e-05 |
| atoms-1000 | 1 | 1000 | full_read | warm | readcon-db | 2.1700e-04 | 3.8931e-05 |
| atoms-1000 | 1 | 1000 | full_read | warm | ase.db | 4.8107e-04 | 2.2647e-05 |
| atoms-1000 | 1 | 1000 | selected | cold | readcon-db | 5.9337e-04 | 3.2915e-04 |
| atoms-1000 | 1 | 1000 | selected | cold | ase.db | 1.4612e-03 | 1.3306e-03 |
| atoms-1000 | 1 | 1000 | selected | warm | readcon-db | 1.9729e-04 | 1.8408e-05 |
| atoms-1000 | 1 | 1000 | selected | warm | ase.db | 4.6503e-04 | 5.0630e-05 |
| atoms-10000 | 1 | 10000 | insert | cold-file | readcon-db | 2.9770e-03 | 3.2712e-04 |
| atoms-10000 | 1 | 10000 | insert | cold-file | ase.db | 3.5304e-03 | 3.0187e-04 |
| atoms-10000 | 1 | 10000 | full_read | cold | readcon-db | 1.4941e-03 | 6.5517e-05 |
| atoms-10000 | 1 | 10000 | full_read | cold | ase.db | 3.1582e-03 | 8.9471e-05 |
| atoms-10000 | 1 | 10000 | full_read | warm | readcon-db | 1.2232e-03 | 6.7054e-05 |
| atoms-10000 | 1 | 10000 | full_read | warm | ase.db | 2.3334e-03 | 6.9269e-05 |
| atoms-10000 | 1 | 10000 | selected | cold | readcon-db | 2.0125e-03 | 5.2204e-04 |
| atoms-10000 | 1 | 10000 | selected | cold | ase.db | 4.5985e-03 | 8.9325e-04 |
| atoms-10000 | 1 | 10000 | selected | warm | readcon-db | 1.1730e-03 | 2.0287e-05 |
| atoms-10000 | 1 | 10000 | selected | warm | ase.db | 2.2662e-03 | 1.9533e-04 |
| atoms-100000 | 1 | 100000 | insert | cold-file | readcon-db | 2.8036e-02 | 5.5468e-04 |
| atoms-100000 | 1 | 100000 | insert | cold-file | ase.db | 2.8140e-02 | 3.3208e-03 |
| atoms-100000 | 1 | 100000 | full_read | cold | readcon-db | 1.5886e-02 | 5.3359e-04 |
| atoms-100000 | 1 | 100000 | full_read | cold | ase.db | 2.8399e-02 | 4.1466e-04 |
| atoms-100000 | 1 | 100000 | full_read | warm | readcon-db | 1.6013e-02 | 7.0280e-03 |
| atoms-100000 | 1 | 100000 | full_read | warm | ase.db | 3.5312e-02 | 1.2470e-02 |
| atoms-100000 | 1 | 100000 | selected | cold | readcon-db | 1.4135e-02 | 4.5791e-04 |
| atoms-100000 | 1 | 100000 | selected | cold | ase.db | 2.4610e-02 | 2.4379e-03 |
| atoms-100000 | 1 | 100000 | selected | warm | readcon-db | 1.3255e-02 | 6.7611e-04 |
| atoms-100000 | 1 | 100000 | selected | warm | ase.db | 2.1317e-02 | 7.8537e-04 |
| atoms-1000000 | 1 | 1000000 | insert | cold-file | readcon-db | 3.4903e-01 | 4.7691e-02 |
| atoms-1000000 | 1 | 1000000 | insert | cold-file | ase.db | 2.3947e-01 | 2.0073e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | cold | readcon-db | 1.7780e-01 | 4.2700e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | cold | ase.db | 2.8180e-01 | 4.3527e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | warm | readcon-db | 1.7728e-01 | 5.3152e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | warm | ase.db | 2.4047e-01 | 3.4558e-02 |
| atoms-1000000 | 1 | 1000000 | selected | cold | readcon-db | 1.7978e-01 | 4.6918e-02 |
| atoms-1000000 | 1 | 1000000 | selected | cold | ase.db | 2.8319e-01 | 3.2095e-02 |
| atoms-1000000 | 1 | 1000000 | selected | warm | readcon-db | 1.7316e-01 | 6.0414e-02 |
| atoms-1000000 | 1 | 1000000 | selected | warm | ase.db | 2.5136e-01 | 3.0992e-02 |
| frames-1 | 1 | 8 | insert | cold-file | readcon-db | 3.1470e-04 | 3.0663e-05 |
| frames-1 | 1 | 8 | insert | cold-file | ase.db | 1.0633e-03 | 4.4492e-05 |
| frames-1 | 1 | 8 | full_read | cold | readcon-db | 2.8701e-04 | 5.2285e-05 |
| frames-1 | 1 | 8 | full_read | cold | ase.db | 6.5542e-04 | 9.5599e-05 |
| frames-1 | 1 | 8 | full_read | warm | readcon-db | 5.9349e-05 | 2.0021e-05 |
| frames-1 | 1 | 8 | full_read | warm | ase.db | 1.8208e-04 | 2.5226e-05 |
| frames-1 | 1 | 8 | selected | cold | readcon-db | 2.7838e-04 | 4.9126e-05 |
| frames-1 | 1 | 8 | selected | cold | ase.db | 7.3228e-04 | 6.0154e-05 |
| frames-1 | 1 | 8 | selected | warm | readcon-db | 6.7252e-05 | 1.9264e-05 |
| frames-1 | 1 | 8 | selected | warm | ase.db | 2.0876e-04 | 1.9872e-05 |
| frames-100 | 100 | 8 | insert | cold-file | readcon-db | 4.8247e-03 | 7.2606e-04 |
| frames-100 | 100 | 8 | insert | cold-file | ase.db | 3.9384e-02 | 4.1867e-03 |
| frames-100 | 100 | 8 | full_read | cold | readcon-db | 1.5287e-03 | 1.9101e-04 |
| frames-100 | 100 | 8 | full_read | cold | ase.db | 1.0617e-02 | 6.8461e-04 |
| frames-100 | 100 | 8 | full_read | warm | readcon-db | 9.1998e-04 | 1.4288e-05 |
| frames-100 | 100 | 8 | full_read | warm | ase.db | 9.1299e-03 | 1.8769e-04 |
| frames-100 | 100 | 8 | selected | cold | readcon-db | 3.3744e-04 | 5.7358e-05 |
| frames-100 | 100 | 8 | selected | cold | ase.db | 7.8504e-04 | 9.7169e-04 |
| frames-100 | 100 | 8 | selected | warm | readcon-db | 9.2170e-05 | 2.6599e-05 |
| frames-100 | 100 | 8 | selected | warm | ase.db | 2.2264e-04 | 2.8856e-05 |
| frames-10000 | 10000 | 8 | insert | cold-file | readcon-db | 3.7281e-01 | 1.3681e-02 |
| frames-10000 | 10000 | 8 | insert | cold-file | ase.db | 3.4295e+00 | 1.1939e-01 |
| frames-10000 | 10000 | 8 | full_read | cold | readcon-db | 1.2092e-01 | 1.5586e-02 |
| frames-10000 | 10000 | 8 | full_read | cold | ase.db | 9.7684e-01 | 1.6769e-02 |
| frames-10000 | 10000 | 8 | full_read | warm | readcon-db | 1.1167e-01 | 2.3777e-03 |
| frames-10000 | 10000 | 8 | full_read | warm | ase.db | 9.6229e-01 | 1.4599e-02 |
| frames-10000 | 10000 | 8 | selected | cold | readcon-db | 5.6732e-04 | 9.0463e-05 |
| frames-10000 | 10000 | 8 | selected | cold | ase.db | 3.5585e-03 | 1.6221e-04 |
| frames-10000 | 10000 | 8 | selected | warm | readcon-db | 9.3291e-05 | 2.9083e-05 |
| frames-10000 | 10000 | 8 | selected | warm | ase.db | 8.5196e-04 | 4.1678e-05 |

## Insert rerun after the span index

Same host as the table above (`42bd893`). `before` is that table's
insert median. `after` is a later insert-only run (one warmup, n = 6,
type-7) with texts above 512 KiB indexed from the stored span. Raw
lines: `insert-span.log`. Reads were not retimed.

| workload | tool | before s | before IQR s | after s | after IQR s |
| --- | --- | --- | --- | --- | --- |
| paper40x8 | readcon-db | 2.3848e-03 | 1.0760e-03 | 1.9719e-03 | 6.5999e-04 |
| paper40x8 | ase.db | 5.9432e-03 | 1.2624e-03 | 6.5242e-03 | 1.2866e-03 |
| atoms-1000 | readcon-db | 7.6093e-04 | 3.6910e-04 | 6.7260e-04 | 3.9172e-04 |
| atoms-1000 | ase.db | 1.3459e-03 | 3.2727e-05 | 1.8801e-03 | 2.8034e-04 |
| atoms-10000 | readcon-db | 2.9770e-03 | 3.2712e-04 | 3.1342e-03 | 7.3484e-05 |
| atoms-10000 | ase.db | 3.5304e-03 | 3.0187e-04 | 3.8584e-03 | 1.7635e-04 |
| atoms-100000 | readcon-db | 2.8036e-02 | 5.5468e-04 | 1.6272e-02 | 8.2277e-04 |
| atoms-100000 | ase.db | 2.8140e-02 | 3.3208e-03 | 2.9640e-02 | 1.7345e-03 |
| atoms-1000000 | readcon-db | 3.4903e-01 | 4.7691e-02 | 1.6484e-01 | 4.5250e-03 |
| atoms-1000000 | ase.db | 2.3947e-01 | 2.0073e-02 | 2.3951e-01 | 1.4315e-02 |
| frames-1 | readcon-db | 3.1470e-04 | 3.0663e-05 | 4.0001e-04 | 1.8750e-04 |
| frames-1 | ase.db | 1.0633e-03 | 4.4492e-05 | 1.1397e-03 | 2.1766e-04 |
| frames-100 | readcon-db | 4.8247e-03 | 7.2606e-04 | 4.6217e-03 | 9.0406e-04 |
| frames-100 | ase.db | 3.9384e-02 | 4.1867e-03 | 3.4705e-02 | 7.7993e-04 |
| frames-10000 | readcon-db | 3.7281e-01 | 1.3681e-02 | 3.0166e-01 | 9.0925e-03 |
| frames-10000 | ase.db | 3.4295e+00 | 1.1939e-01 | 3.2814e+00 | 9.6273e-02 |

readcon-db is ahead of ASE on every insert row. The million-atom row
was the one ASE won before (0.349 s vs 0.239 s) and is now 0.165 s vs
0.240 s. Texts at or below 512 KiB still use the previous parser. The
one-frame median, 0.400 ms, is about 0.05 ms past the previous median
plus IQR; five of the six samples are 0.299–0.537 ms and one sample is
1.02 ms.

