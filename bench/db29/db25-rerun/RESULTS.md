# db25 warm full and range

Commit `67dc0235955e6c0f50a8e814d4d489fbb387ec65`, tree `99addc975a22943f44aa024dc4b718d6d5052579`. Pushed base `28dbf15e8ff2cecdff429bcedbd5c9e2e3a60b59`. Host Intel(R) Xeon(R) Processor, 8 CPUs. n=12, serial. Arm order is db18, raw, shuffle, ase; reversed on odd sample indexes. These rows are the warm full and range reads. A multi-frame shuffle read unshuffles into the NumPy buffer allocated up front. Selected and the cold rows are unchanged from db24 and are not repeated. The median is type-7. Parentheses are the type-7 IQR. The paired CI is a 10000-draw percentile bootstrap of the shuffle/raw ratio at the same sample index (numpy Generator seed 0).

| op | threads | raw | shuffle-zstd | shuffle/raw | paired median | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- |
| full_read | unset | 3.5461e-01 (4.0891e-01) | 1.2132e-01 (9.5283e-02) | 0.3421 | 0.3085 | [0.2328, 0.3595] | 12/12 |
| range | unset | 9.8874e-02 (4.1280e-02) | 2.4688e-02 (8.3616e-03) | 0.2497 | 0.2898 | [0.2641, 0.3134] | 12/12 |
| full_read | 1 | 2.8296e-01 (6.8064e-02) | 2.1436e-01 (4.3881e-02) | 0.7576 | 0.7349 | [0.6258, 0.8076] | 11/12 |
| range | 1 | 7.7451e-02 (3.4036e-02) | 4.7864e-02 (1.5239e-02) | 0.6180 | 0.5855 | [0.5487, 0.6097] | 12/12 |

Single-thread warm full reads are faster: the paired interval is below 1.
