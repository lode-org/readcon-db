# readcon-db vs ASE sqlite

Type-7 median and IQR. One discarded warmup and six serial samples
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
| readcon-db | 2.8338e-03 | 9.6109e-04 | 2.0362e-03 | 2.9973e-03 | 1.7945e-03 | 3.1198e-03 |
| ase.db | 5.0200e-02 | 5.5446e-03 | 4.7250e-02 | 5.2795e-02 | 4.3035e-02 | 6.0056e-02 |

## This machine, fair harness

Insert creates the database outside the clock and times the commit.
A full read and the selected-row query (`frame_index` of the middle
frame) time the query only: open, directory fsync, fadvise,
mincore, the field check, and the collector stay outside. A read
shorter than 50 ms repeats until the timed spans add to 50 ms. A
commit shorter than 2 s repeats until they add to 2 s, because
those fsyncs stay correlated well past 250 ms. The sample is that
mean. A row that stayed above an IQR/median of 0.15 was measured
again at a longer span; that budget is `span_s` on the row.

| workload | op | cache | tool | n | median s | IQR s | IQR/median | disk B | peak RSS B | fidelity |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| paper40x8 | insert | cold-file | readcon-db | 6 | 2.0211e-03 | 1.1670e-04 | 0.058 | 61440 | 110448640 | exact |
| paper40x8 | insert | cold-file | ase.db | 6 | 5.8846e-03 | 3.7633e-04 | 0.064 | 90112 | 110383104 | exact |
| atoms-1000 | insert | cold-file | readcon-db | 6 | 1.1704e-03 | 1.7292e-04 | 0.148 | 90112 | 4573847552 | exact |
| atoms-1000 | insert | cold-file | ase.db | 6 | 1.3820e-03 | 9.7669e-06 | 0.007 | 126976 | 4573753344 | exact |
| atoms-1000 | full_read | cold | readcon-db | 6 | 1.6622e-04 | 5.3498e-06 | 0.032 | 90112 | 112709632 | exact |
| atoms-1000 | full_read | cold | ase.db | 6 | 8.3866e-04 | 3.6776e-05 | 0.044 | 126976 | 97243136 | exact |
| atoms-1000 | selected | cold | readcon-db | 6 | 1.6579e-04 | 5.9576e-06 | 0.036 | 90112 | 112709632 | exact |
| atoms-1000 | selected | cold | ase.db | 6 | 1.3186e-03 | 1.9061e-04 | 0.145 | 126976 | 110981120 | exact |
| atoms-1000 | full_read | warm | readcon-db | 6 | 1.1258e-04 | 6.6291e-07 | 0.006 | 90112 | 110944256 | exact |
| atoms-1000 | full_read | warm | ase.db | 6 | 1.0640e-04 | 8.4008e-07 | 0.008 | 126976 | 110944256 | exact |
| atoms-1000 | selected | warm | readcon-db | 6 | 1.1249e-04 | 3.0254e-07 | 0.003 | 90112 | 110944256 | exact |
| atoms-1000 | selected | warm | ase.db | 6 | 1.2337e-04 | 1.9203e-07 | 0.002 | 126976 | 110944256 | exact |
| atoms-10000 | insert | cold-file | readcon-db | 6 | 2.8556e-03 | 7.5266e-05 | 0.026 | 147456 | 137695232 | exact |
| atoms-10000 | insert | cold-file | ase.db | 6 | 3.7029e-03 | 1.5648e-04 | 0.042 | 667648 | 134574080 | exact |
| atoms-10000 | full_read | cold | readcon-db | 6 | 1.2286e-03 | 5.2453e-05 | 0.043 | 147456 | 143233024 | exact |
| atoms-10000 | full_read | cold | ase.db | 6 | 1.6440e-03 | 7.2033e-05 | 0.044 | 667648 | 143085568 | exact |
| atoms-10000 | selected | cold | readcon-db | 6 | 1.2331e-03 | 2.6167e-05 | 0.021 | 147456 | 143233024 | exact |
| atoms-10000 | selected | cold | ase.db | 6 | 1.2640e-03 | 2.4529e-05 | 0.019 | 667648 | 131850240 | exact |
| atoms-10000 | full_read | warm | readcon-db | 6 | 9.9408e-04 | 1.2546e-06 | 0.001 | 147456 | 185143296 | exact |
| atoms-10000 | full_read | warm | ase.db | 6 | 2.9596e-04 | 8.1333e-06 | 0.027 | 667648 | 185143296 | exact |
| atoms-10000 | selected | warm | readcon-db | 6 | 9.9791e-04 | 4.8369e-06 | 0.005 | 147456 | 185143296 | exact |
| atoms-10000 | selected | warm | ase.db | 6 | 3.1439e-04 | 4.3105e-06 | 0.014 | 667648 | 185143296 | exact |
| atoms-100000 | insert | cold-file | readcon-db | 6 | 1.5684e-02 | 1.3843e-04 | 0.009 | 630784 | 318488576 | exact |
| atoms-100000 | insert | cold-file | ase.db | 6 | 2.6887e-02 | 3.9611e-04 | 0.015 | 6074368 | 316211200 | exact |
| atoms-100000 | full_read | cold | readcon-db | 6 | 1.1524e-02 | 1.5578e-04 | 0.014 | 630784 | 330567680 | exact |
| atoms-100000 | full_read | cold | ase.db | 6 | 6.0513e-03 | 6.7258e-04 | 0.111 | 6074368 | 327389184 | exact |
| atoms-100000 | selected | cold | readcon-db | 6 | 1.2310e-02 | 1.4404e-03 | 0.117 | 630784 | 330567680 | exact |
| atoms-100000 | selected | cold | ase.db | 6 | 6.3000e-03 | 7.1038e-04 | 0.113 | 6074368 | 339099648 | exact |
| atoms-100000 | full_read | warm | readcon-db | 6 | 1.1061e-02 | 6.2141e-05 | 0.006 | 630784 | 741646336 | exact |
| atoms-100000 | full_read | warm | ase.db | 6 | 1.4006e-03 | 1.1420e-05 | 0.008 | 6074368 | 741711872 | exact |
| atoms-100000 | selected | warm | readcon-db | 6 | 1.1889e-02 | 1.6016e-03 | 0.135 | 630784 | 741711872 | exact |
| atoms-100000 | selected | warm | ase.db | 6 | 1.4194e-03 | 2.9231e-05 | 0.021 | 6074368 | 735559680 | exact |
| atoms-1000000 | insert | cold-file | readcon-db | 6 | 1.5505e-01 | 3.9738e-03 | 0.026 | 4632576 | 1753894912 | exact |
| atoms-1000000 | insert | cold-file | ase.db | 6 | 2.3012e-01 | 3.4480e-03 | 0.015 | 60125184 | 1342455808 | exact |
| atoms-1000000 | full_read | cold | readcon-db | 6 | 1.2566e-01 | 5.2347e-03 | 0.042 | 4632576 | 1142415360 | exact |
| atoms-1000000 | full_read | cold | ase.db | 6 | 5.0512e-02 | 3.9912e-03 | 0.079 | 60125184 | 2608807936 | exact |
| atoms-1000000 | selected | cold | readcon-db | 6 | 1.3964e-01 | 1.8198e-02 | 0.130 | 4632576 | 1301233664 | exact |
| atoms-1000000 | selected | cold | ase.db | 6 | 4.9601e-02 | 2.8590e-03 | 0.058 | 60125184 | 1160601600 | exact |
| atoms-1000000 | full_read | warm | readcon-db | 6 | 1.2263e-01 | 2.0746e-03 | 0.017 | 4632576 | 2669858816 | exact |
| atoms-1000000 | full_read | warm | ase.db | 6 | 1.6551e-02 | 1.4735e-03 | 0.089 | 60125184 | 6990233600 | exact |
| atoms-1000000 | selected | warm | readcon-db | 6 | 1.3431e-01 | 1.9292e-02 | 0.144 | 4632576 | 2669858816 | exact |
| atoms-1000000 | selected | warm | ase.db | 6 | 3.4999e-02 | 1.8788e-03 | 0.054 | 60125184 | 2501861376 | exact |
| frames-1 | insert | cold-file | readcon-db | 6 | 1.0847e-03 | 5.8365e-05 | 0.054 | 81920 | 2144227328 | exact |
| frames-1 | insert | cold-file | ase.db | 6 | 1.0328e-03 | 5.7590e-05 | 0.056 | 69632 | 2144141312 | exact |
| frames-1 | full_read | cold | readcon-db | 6 | 5.9231e-05 | 8.9574e-07 | 0.015 | 81920 | 2120204288 | exact |
| frames-1 | full_read | cold | ase.db | 6 | 4.5351e-04 | 3.5868e-05 | 0.079 | 69632 | 2119090176 | exact |
| frames-1 | selected | cold | readcon-db | 6 | 5.7697e-05 | 2.1743e-06 | 0.038 | 81920 | 2120204288 | exact |
| frames-1 | selected | cold | ase.db | 6 | 6.0964e-04 | 4.9756e-05 | 0.082 | 69632 | 91836416 | exact |
| frames-1 | full_read | warm | readcon-db | 6 | 2.1034e-05 | 1.7554e-07 | 0.008 | 81920 | 2113687552 | exact |
| frames-1 | full_read | warm | ase.db | 6 | 8.2758e-05 | 1.3083e-06 | 0.016 | 69632 | 2113687552 | exact |
| frames-1 | selected | warm | readcon-db | 6 | 2.0373e-05 | 8.2517e-08 | 0.004 | 81920 | 2113687552 | exact |
| frames-1 | selected | warm | ase.db | 6 | 9.5134e-05 | 1.1246e-06 | 0.012 | 69632 | 2113687552 | exact |
| frames-100 | insert | cold-file | readcon-db | 6 | 4.3064e-03 | 1.0876e-04 | 0.025 | 106496 | 2110087168 | exact |
| frames-100 | insert | cold-file | ase.db | 6 | 3.0623e-02 | 2.0925e-04 | 0.007 | 151552 | 2109919232 | exact |
| frames-100 | full_read | cold | readcon-db | 6 | 3.7272e-04 | 6.4447e-06 | 0.017 | 106496 | 434892800 | exact |
| frames-100 | full_read | cold | ase.db | 6 | 4.1059e-03 | 3.0670e-04 | 0.075 | 151552 | 434638848 | exact |
| frames-100 | selected | cold | readcon-db | 6 | 8.4162e-05 | 3.2683e-06 | 0.039 | 106496 | 434745344 | exact |
| frames-100 | selected | cold | ase.db | 6 | 7.2864e-04 | 8.0866e-05 | 0.111 | 151552 | 93544448 | exact |
| frames-100 | full_read | warm | readcon-db | 6 | 3.0361e-04 | 3.5323e-06 | 0.012 | 106496 | 434769920 | exact |
| frames-100 | full_read | warm | ase.db | 6 | 2.9491e-03 | 1.7393e-05 | 0.006 | 151552 | 434671616 | exact |
| frames-100 | selected | warm | readcon-db | 6 | 3.3791e-05 | 2.6753e-07 | 0.008 | 106496 | 434671616 | exact |
| frames-100 | selected | warm | ase.db | 6 | 1.0269e-04 | 5.2159e-07 | 0.005 | 151552 | 434671616 | exact |
| frames-10000 | insert | cold-file | readcon-db | 6 | 2.4112e-01 | 6.3474e-04 | 0.003 | 5644288 | 571432960 | exact |
| frames-10000 | insert | cold-file | ase.db | 6 | 2.8250e+00 | 4.0341e-02 | 0.014 | 10809344 | 558702592 | exact |
| frames-10000 | full_read | cold | readcon-db | 6 | 4.5189e-02 | 1.9005e-03 | 0.042 | 5644288 | 546725888 | exact |
| frames-10000 | full_read | cold | ase.db | 6 | 2.7928e-01 | 7.6824e-03 | 0.028 | 10809344 | 540270592 | exact |
| frames-10000 | selected | cold | readcon-db | 6 | 5.7377e-04 | 2.1871e-05 | 0.038 | 5644288 | 314781696 | exact |
| frames-10000 | selected | cold | ase.db | 6 | 4.0383e-03 | 4.6546e-04 | 0.115 | 10809344 | 540270592 | exact |
| frames-10000 | full_read | warm | readcon-db | 6 | 3.0590e-02 | 1.2127e-03 | 0.040 | 5644288 | 546467840 | exact |
| frames-10000 | full_read | warm | ase.db | 6 | 2.6984e-01 | 8.6454e-03 | 0.032 | 10809344 | 546467840 | exact |
| frames-10000 | selected | warm | readcon-db | 6 | 3.3698e-05 | 1.1082e-06 | 0.033 | 5644288 | 546467840 | exact |
| frames-10000 | selected | warm | ase.db | 6 | 6.5830e-04 | 9.0681e-06 | 0.014 | 10809344 | 546467840 | exact |

