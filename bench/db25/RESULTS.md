# db25 warm full and range

Commit `1e9cd7454a2530f6486aec1e6c2dac74564946a0`, tree `19f64411adeeb918444f7f2074ce3ce052937fd3`. Pushed base `28dbf15e8ff2cecdff429bcedbd5c9e2e3a60b59`. Host Intel(R) Xeon(R) Processor, 8 CPUs. n=12, serial. Arm order is db18, raw, shuffle, ase; reversed on odd sample indexes. These rows are the warm full and range reads. A multi-frame shuffle read unshuffles into the NumPy buffer allocated up front. Selected and the cold rows are unchanged from db24 and are not repeated. The median is type-7. Parentheses are the type-7 IQR. The paired CI is a 10000-draw percentile bootstrap of the shuffle/raw ratio at the same sample index (numpy Generator seed 0).

| op | threads | raw | shuffle-zstd | shuffle/raw | paired median | 95% CI | faster |
| --- | --- | --- | --- | --- | --- | --- | --- |
| full_read | unset | 2.4199e-01 (2.9439e-02) | 9.1513e-02 (2.2089e-02) | 0.3782 | 0.3579 | [0.3231, 0.4292] | 12/12 |
| range | unset | 7.6381e-02 (3.1617e-02) | 1.9897e-02 (2.7239e-03) | 0.2605 | 0.2653 | [0.2298, 0.2998] | 12/12 |
| full_read | 1 | 2.5085e-01 (2.9795e-02) | 1.8158e-01 (2.5151e-02) | 0.7239 | 0.7713 | [0.7407, 0.8336] | 12/12 |
| range | 1 | 7.3786e-02 (3.1201e-02) | 4.6800e-02 (1.5493e-02) | 0.6343 | 0.6626 | [0.4129, 0.9176] | 12/12 |

Single-thread warm full reads are faster: the paired interval is below 1.
