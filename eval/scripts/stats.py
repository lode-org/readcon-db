"""Median and percentile-bootstrap intervals. One implementation for the tables and the figures."""

from __future__ import annotations

import numpy as np

SEED = 20261011
N_BOOT = 10_000
ALPHA = 0.05


def bootstrap_median(values: list[float], seed: int = SEED, n_boot: int = N_BOOT) -> dict:
    x = np.asarray(list(values), dtype=float)
    x = x[np.isfinite(x)]
    if x.size == 0:
        return {"n": 0, "median": None, "ci_lo": None, "ci_hi": None}
    med = float(np.median(x))
    if x.size == 1:
        return {"n": 1, "median": med, "ci_lo": med, "ci_hi": med}
    rng = np.random.default_rng(seed)
    picks = rng.integers(0, x.size, size=(n_boot, x.size))
    samples = np.median(x[picks], axis=1)
    lo, hi = np.quantile(samples, [ALPHA / 2.0, 1.0 - ALPHA / 2.0])
    return {"n": int(x.size), "median": med, "ci_lo": float(lo), "ci_hi": float(hi)}
