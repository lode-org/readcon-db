# db25 warm full and range

Commit `043c3091c9606293a920311a73a8a622d4638746`, tree `a201b635a8bfaa444623c2530e6c10592a423c3f`. Pushed base `28dbf15e8ff2cecdff429bcedbd5c9e2e3a60b59`. Host Intel(R) Xeon(R) Processor, 8 CPUs. n=12, serial. Arm order is db18, raw, shuffle, ase; reversed on odd sample indexes. These rows are the warm full and range reads. A multi-frame shuffle read unshuffles into the NumPy buffer allocated up front. Selected and the cold rows are unchanged from db24 and are not repeated. The median is type-7. Parentheses are the type-7 IQR. The paired CI is a 10000-draw percentile bootstrap of the shuffle/raw ratio at the same sample index (numpy Generator seed 0).

| op | threads | raw | shuffle-zstd | shuffle/raw | paired median | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- |
| full_read | unset | 3.6872e-01 (1.1112e-01) | 8.4336e-02 (1.4704e-02) | 0.2287 | 0.2592 | [0.1832, 0.2867] | 12/12 |
| range | unset | 8.0644e-02 (3.0297e-02) | 2.1777e-02 (6.4439e-03) | 0.2700 | 0.2727 | [0.2350, 0.3092] | 12/12 |
| full_read | 1 | 2.7744e-01 (8.0547e-02) | 2.0559e-01 (5.2863e-02) | 0.7410 | 0.7085 | [0.5467, 0.8149] | 12/12 |
| range | 1 | 1.0357e-01 (3.3366e-02) | 5.4497e-02 (7.4257e-03) | 0.5262 | 0.5246 | [0.4196, 0.6900] | 12/12 |

Single-thread warm full reads are faster: the paired interval is below 1.
