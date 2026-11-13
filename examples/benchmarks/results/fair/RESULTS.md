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
| readcon-db | 1.7138e-02 | 4.2416e-03 | 1.3837e-02 | 1.8079e-02 | 1.1829e-02 | 2.0492e-02 |
| ase.db | 5.0747e-02 | 6.6903e-03 | 4.9323e-02 | 5.6013e-02 | 4.7465e-02 | 6.1656e-02 |

## This machine, fair harness

Insert unlinks and creates the database file, then times one
transaction and one commit fsync. Full read and the selected-row
query (`frame_index` of the middle frame) are cold and warm.

| workload | frames | atoms | op | cache | tool | median s | IQR s |
| --- | --- | --- | --- | --- | --- | --- | --- |
| paper40x8 | 40 | 8 | insert | cold-file | readcon-db | 1.9852e-03 | 7.8139e-04 |
| paper40x8 | 40 | 8 | insert | cold-file | ase.db | 7.9432e-03 | 3.9098e-03 |
| atoms-1000 | 1 | 1000 | insert | cold-file | readcon-db | 1.0523e-03 | 7.8090e-05 |
| atoms-1000 | 1 | 1000 | insert | cold-file | ase.db | 2.8696e-03 | 1.2830e-03 |
| atoms-1000 | 1 | 1000 | full_read | cold | readcon-db | 1.0352e-02 | 4.9443e-04 |
| atoms-1000 | 1 | 1000 | full_read | cold | ase.db | 1.4585e-03 | 1.1214e-03 |
| atoms-1000 | 1 | 1000 | full_read | warm | readcon-db | 2.1944e-04 | 4.1734e-05 |
| atoms-1000 | 1 | 1000 | full_read | warm | ase.db | 4.6657e-04 | 3.1641e-05 |
| atoms-1000 | 1 | 1000 | selected | cold | readcon-db | 1.0016e-02 | 1.0709e-03 |
| atoms-1000 | 1 | 1000 | selected | cold | ase.db | 1.5442e-03 | 1.9250e-04 |
| atoms-1000 | 1 | 1000 | selected | warm | readcon-db | 2.2162e-04 | 3.4955e-05 |
| atoms-1000 | 1 | 1000 | selected | warm | ase.db | 5.1128e-04 | 5.6153e-05 |
| atoms-10000 | 1 | 10000 | insert | cold-file | readcon-db | 3.3361e-03 | 4.3024e-04 |
| atoms-10000 | 1 | 10000 | insert | cold-file | ase.db | 4.4083e-03 | 6.8256e-04 |
| atoms-10000 | 1 | 10000 | full_read | cold | readcon-db | 1.2040e-02 | 8.6349e-04 |
| atoms-10000 | 1 | 10000 | full_read | cold | ase.db | 4.4894e-03 | 1.8285e-04 |
| atoms-10000 | 1 | 10000 | full_read | warm | readcon-db | 1.3977e-03 | 9.2255e-05 |
| atoms-10000 | 1 | 10000 | full_read | warm | ase.db | 2.7190e-03 | 9.6142e-05 |
| atoms-10000 | 1 | 10000 | selected | cold | readcon-db | 1.1553e-02 | 6.6796e-04 |
| atoms-10000 | 1 | 10000 | selected | cold | ase.db | 3.8639e-03 | 1.0059e-03 |
| atoms-10000 | 1 | 10000 | selected | warm | readcon-db | 1.3587e-03 | 1.0336e-04 |
| atoms-10000 | 1 | 10000 | selected | warm | ase.db | 2.5731e-03 | 5.5674e-05 |
| atoms-100000 | 1 | 100000 | insert | cold-file | readcon-db | 3.3861e-02 | 1.5067e-03 |
| atoms-100000 | 1 | 100000 | insert | cold-file | ase.db | 2.9629e-02 | 2.5337e-03 |
| atoms-100000 | 1 | 100000 | full_read | cold | readcon-db | 2.5869e-02 | 3.5029e-03 |
| atoms-100000 | 1 | 100000 | full_read | cold | ase.db | 2.9155e-02 | 3.3003e-03 |
| atoms-100000 | 1 | 100000 | full_read | warm | readcon-db | 1.4358e-02 | 1.9908e-03 |
| atoms-100000 | 1 | 100000 | full_read | warm | ase.db | 2.3353e-02 | 4.4686e-04 |
| atoms-100000 | 1 | 100000 | selected | cold | readcon-db | 2.8269e-02 | 1.7500e-03 |
| atoms-100000 | 1 | 100000 | selected | cold | ase.db | 2.9031e-02 | 3.9289e-03 |
| atoms-100000 | 1 | 100000 | selected | warm | readcon-db | 1.4679e-02 | 1.1570e-03 |
| atoms-100000 | 1 | 100000 | selected | warm | ase.db | 2.2999e-02 | 9.8354e-04 |
| atoms-1000000 | 1 | 1000000 | insert | cold-file | readcon-db | 3.2750e-01 | 7.0429e-03 |
| atoms-1000000 | 1 | 1000000 | insert | cold-file | ase.db | 2.4427e-01 | 9.5138e-03 |
| atoms-1000000 | 1 | 1000000 | full_read | cold | readcon-db | 1.6847e-01 | 2.8408e-03 |
| atoms-1000000 | 1 | 1000000 | full_read | cold | ase.db | 2.6691e-01 | 6.9426e-03 |
| atoms-1000000 | 1 | 1000000 | full_read | warm | readcon-db | 1.6536e-01 | 4.4278e-02 |
| atoms-1000000 | 1 | 1000000 | full_read | warm | ase.db | 2.4948e-01 | 4.8680e-02 |
| atoms-1000000 | 1 | 1000000 | selected | cold | readcon-db | 1.6437e-01 | 2.7028e-03 |
| atoms-1000000 | 1 | 1000000 | selected | cold | ase.db | 2.4946e-01 | 7.5303e-03 |
| atoms-1000000 | 1 | 1000000 | selected | warm | readcon-db | 1.7379e-01 | 6.2728e-02 |
| atoms-1000000 | 1 | 1000000 | selected | warm | ase.db | 2.3311e-01 | 3.8441e-02 |
| frames-1 | 1 | 8 | insert | cold-file | readcon-db | 3.5571e-04 | 1.1707e-04 |
| frames-1 | 1 | 8 | insert | cold-file | ase.db | 1.1348e-03 | 4.3335e-04 |
| frames-1 | 1 | 8 | full_read | cold | readcon-db | 8.4241e-03 | 3.1774e-04 |
| frames-1 | 1 | 8 | full_read | cold | ase.db | 9.7972e-04 | 1.3635e-04 |
| frames-1 | 1 | 8 | full_read | warm | readcon-db | 6.1564e-05 | 1.3487e-05 |
| frames-1 | 1 | 8 | full_read | warm | ase.db | 1.7300e-04 | 2.9173e-05 |
| frames-1 | 1 | 8 | selected | cold | readcon-db | 8.5378e-03 | 3.0843e-04 |
| frames-1 | 1 | 8 | selected | cold | ase.db | 1.5509e-03 | 8.1205e-04 |
| frames-1 | 1 | 8 | selected | warm | readcon-db | 5.8339e-05 | 1.7870e-05 |
| frames-1 | 1 | 8 | selected | warm | ase.db | 1.7928e-04 | 2.1548e-05 |
| frames-100 | 100 | 8 | insert | cold-file | readcon-db | 4.7056e-03 | 1.9971e-04 |
| frames-100 | 100 | 8 | insert | cold-file | ase.db | 3.4570e-02 | 8.1342e-04 |
| frames-100 | 100 | 8 | full_read | cold | readcon-db | 9.7872e-03 | 1.6856e-04 |
| frames-100 | 100 | 8 | full_read | cold | ase.db | 9.5810e-03 | 3.3369e-04 |
| frames-100 | 100 | 8 | full_read | warm | readcon-db | 8.3453e-04 | 6.9746e-05 |
| frames-100 | 100 | 8 | full_read | warm | ase.db | 8.1677e-03 | 7.9202e-05 |
| frames-100 | 100 | 8 | selected | cold | readcon-db | 8.8870e-03 | 1.5432e-04 |
| frames-100 | 100 | 8 | selected | cold | ase.db | 1.5927e-03 | 6.2370e-05 |
| frames-100 | 100 | 8 | selected | warm | readcon-db | 7.7299e-05 | 1.7626e-05 |
| frames-100 | 100 | 8 | selected | warm | ase.db | 1.9005e-04 | 1.7452e-05 |
| frames-10000 | 10000 | 8 | insert | cold-file | readcon-db | 3.3290e-01 | 8.2125e-03 |
| frames-10000 | 10000 | 8 | insert | cold-file | ase.db | 3.1056e+00 | 7.3061e-02 |
| frames-10000 | 10000 | 8 | full_read | cold | readcon-db | 1.6728e-01 | 1.3815e-02 |
| frames-10000 | 10000 | 8 | full_read | cold | ase.db | 8.8791e-01 | 2.6524e-02 |
| frames-10000 | 10000 | 8 | full_read | warm | readcon-db | 9.5647e-02 | 1.3507e-03 |
| frames-10000 | 10000 | 8 | full_read | warm | ase.db | 8.7250e-01 | 1.1586e-02 |
| frames-10000 | 10000 | 8 | selected | cold | readcon-db | 2.0524e-02 | 9.4525e-04 |
| frames-10000 | 10000 | 8 | selected | cold | ase.db | 6.8365e-03 | 4.1083e-03 |
| frames-10000 | 10000 | 8 | selected | warm | readcon-db | 8.6441e-05 | 3.3902e-05 |
| frames-10000 | 10000 | 8 | selected | warm | ase.db | 7.8028e-04 | 5.4528e-05 |

