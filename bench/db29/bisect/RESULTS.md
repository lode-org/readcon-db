# db29 raw full-read bisect

Commit `67dc0235955e6c0f50a8e814d4d489fbb387ec65`. Taken from `raw_full_bisect.py`. Warm full reads, raw codec, lattice arm, n=30, paired, 95% CI. Host label `host`.

| left | right | threads | n | paired | 95% CI | right faster |
| --- | --- | --- | --- | --- | --- | --- |
| aedcbda4 | 750b197 | unset | 30 | 0.9908 | [0.9366, 1.2785] | 16/30 |
| aedcbda4 | 750b197 | 1 | 30 | 1.1169 | [0.9890, 1.2829] | 11/30 |
| 750b197 | d671c32 | unset | 30 | 1.0365 | [0.9612, 1.1562] | 13/30 |
| 750b197 | d671c32 | 1 | 30 | 1.0023 | [0.9044, 1.1178] | 15/30 |
| d671c32 | 20a5f64 | unset | 30 | 1.0089 | [0.9674, 1.0537] | 14/30 |
| d671c32 | 20a5f64 | 1 | 30 | 0.9957 | [0.8876, 1.2003] | 15/30 |

Every interval includes 1. The largest step is aedcbda4 to 750b197 at one thread, 1.1169 [0.9890, 1.2829], and the newer side was faster on 11 of 30 pairs. There is no commit to patch.
