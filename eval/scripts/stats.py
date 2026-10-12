"""Median, minimum, and maximum of one repeat series."""

from __future__ import annotations

import numpy as np


def spread(values: list[float]) -> dict:
    x = np.asarray(list(values), dtype=float)
    x = x[np.isfinite(x)]
    if x.size == 0:
        return {"n": 0, "median": None, "minimum": None, "maximum": None}
    return {
        "n": int(x.size),
        "median": float(np.median(x)),
        "minimum": float(np.min(x)),
        "maximum": float(np.max(x)),
    }
