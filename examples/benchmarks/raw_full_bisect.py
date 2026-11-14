"""Paired raw full-read summaries from per-sample times.

Warm full reads of the lattice arm, raw codec, n=30. The median is
type-7. The CI is a 10000-draw percentile bootstrap of the paired
ratio at one sample index, numpy Generator seed 0.

``load_steps`` reads ``bench/db29/bisect/samples.json``. Medians and
intervals are computed from those samples. They are not stored in the
file.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

SAMPLES_PATH = (
    Path(__file__).resolve().parents[2] / "bench" / "db29" / "bisect" / "samples.json"
)
BOOT_DRAWS = 10_000
BOOT_SEED = 0


def bootstrap_ci(
    ratios: np.ndarray, draws: int = BOOT_DRAWS, seed: int = BOOT_SEED
) -> list[float]:
    rng = np.random.default_rng(seed)
    n = int(ratios.size)
    boots = np.empty(draws, dtype=np.float64)
    for i in range(draws):
        idx = rng.integers(0, n, n)
        boots[i] = np.percentile(ratios[idx], 50, method="linear")
    lo, hi = np.percentile(boots, [2.5, 97.5], method="linear")
    return [float(lo), float(hi)]


def summarize_pair(left: list[float], right: list[float]) -> dict:
    """Median, 95% CI, and win count of ``right / left`` at the same index."""
    a = np.asarray(left, dtype=np.float64)
    b = np.asarray(right, dtype=np.float64)
    if a.shape != b.shape or a.size < 30:
        raise ValueError(
            f"a paired bisect needs n>=30 on both sides, got {a.size} and {b.size}"
        )
    ratios = b / a
    return {
        "n": int(a.size),
        "left_median_s": float(np.percentile(a, 50, method="linear")),
        "right_median_s": float(np.percentile(b, 50, method="linear")),
        "paired_median": float(np.percentile(ratios, 50, method="linear")),
        "ci95": bootstrap_ci(ratios),
        "right_faster": int(np.sum(b < a)),
    }


def steps_from_samples(records: list[dict]) -> list[dict]:
    steps = []
    for record in records:
        summary = summarize_pair(
            record["samples"][record["left"]], record["samples"][record["right"]]
        )
        steps.append(
            {
                "left": record["left"],
                "right": record["right"],
                "left_sha": record["left_sha"],
                "right_sha": record["right_sha"],
                "threads": record["threads"],
                **summary,
            }
        )
    return steps


def load_steps(path: Path | None = None) -> list[dict]:
    records = json.loads((path or SAMPLES_PATH).read_text())
    return steps_from_samples(records)


def intervals_include_one(steps: list[dict]) -> bool:
    return all(lo <= 1.0 <= hi for step in steps for lo, hi in (step["ci95"],))
