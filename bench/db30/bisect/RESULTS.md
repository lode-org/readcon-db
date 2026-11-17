# raw full-read bisect

Commit `043c3091c9606293a920311a73a8a622d4638746`. n=30. Each CI is computed from the paired samples in `samples.json`. Host label `host`.

| left | right | threads | n | paired | 95% CI | right faster |
| --- | --- | --- | --- | --- | --- | --- |
| `aedcbda468d208278b8699b80c6db743edcdc21e` | `750b1970b2ddc9cc0bb4547d3918024d67d7f6a6` | unset | 30 | 1.0391 | [0.9949, 1.5561] | 12/30 |
| `aedcbda468d208278b8699b80c6db743edcdc21e` | `750b1970b2ddc9cc0bb4547d3918024d67d7f6a6` | 1 | 30 | 0.9673 | [0.9248, 1.0966] | 18/30 |
| `750b1970b2ddc9cc0bb4547d3918024d67d7f6a6` | `d671c327375f588044b6f4953314cd80ddddf356` | unset | 30 | 1.0788 | [0.9333, 1.4666] | 14/30 |
| `750b1970b2ddc9cc0bb4547d3918024d67d7f6a6` | `d671c327375f588044b6f4953314cd80ddddf356` | 1 | 30 | 1.0438 | [0.7967, 1.2092] | 14/30 |
| `d671c327375f588044b6f4953314cd80ddddf356` | `20a5f64e37d9a0892e913289d39bcbe71d7718f8` | unset | 30 | 0.9961 | [0.7833, 1.1773] | 15/30 |
| `d671c327375f588044b6f4953314cd80ddddf356` | `20a5f64e37d9a0892e913289d39bcbe71d7718f8` | 1 | 30 | 0.9979 | [0.8971, 1.1675] | 15/30 |
