"""Paired raw full-read bisect, n=30, 95% CI.

Warm full reads of the lattice arm (100000 atoms, 64 frames), raw codec,
same settings on both sides of each step. The median is type-7. The CI is
a 10000-draw percentile bootstrap of the paired ratio at one sample index,
numpy Generator seed 0.

Every interval includes 1. The largest point estimate is aedcbda4 to
750b197 at one thread, 1.1169 [0.9890, 1.2829], and the newer side was
faster on 11 of 30 pairs. The two later steps sit on 1.0. The raw full-read
path is unchanged from 20a5f64e to this tip (the later source edits are a
shuffle-decode test and a cfg on the shuffled blob copy), so the same
intervals cover HEAD. There is no commit whose removal would put the
slowdown outside the noise.
"""

from __future__ import annotations

STEPS = (
    {
        "left": "aedcbda4",
        "right": "750b197",
        "threads": "unset",
        "n": 30,
        "paired_median": 0.9908,
        "ci95": (0.9366, 1.2785),
        "right_faster": 16,
    },
    {
        "left": "aedcbda4",
        "right": "750b197",
        "threads": "1",
        "n": 30,
        "paired_median": 1.1169,
        "ci95": (0.9890, 1.2829),
        "right_faster": 11,
    },
    {
        "left": "750b197",
        "right": "d671c32",
        "threads": "unset",
        "n": 30,
        "paired_median": 1.0365,
        "ci95": (0.9612, 1.1562),
        "right_faster": 13,
    },
    {
        "left": "750b197",
        "right": "d671c32",
        "threads": "1",
        "n": 30,
        "paired_median": 1.0023,
        "ci95": (0.9044, 1.1178),
        "right_faster": 15,
    },
    {
        "left": "d671c32",
        "right": "20a5f64",
        "threads": "unset",
        "n": 30,
        "paired_median": 1.0089,
        "ci95": (0.9674, 1.0537),
        "right_faster": 14,
    },
    {
        "left": "d671c32",
        "right": "20a5f64",
        "threads": "1",
        "n": 30,
        "paired_median": 0.9957,
        "ci95": (0.8876, 1.2003),
        "right_faster": 15,
    },
)


def intervals_include_one() -> bool:
    return all(lo <= 1.0 <= hi for step in STEPS for lo, hi in (step["ci95"],))
