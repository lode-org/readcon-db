"""frames-100 noise keeps the one-thread row and the unset row."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fair_ase_protocol as fair


def _row(rayon: str, median: float, cache: str = "cold") -> dict:
    return {
        "workload": "frames-100",
        "op": "selected",
        "cache": cache,
        "tool": "readcon-db",
        "rayon_num_threads": rayon,
        "n": 12,
        "median_s": median,
        "iqr_s": 0.1 * median,
        "samples_s": [median] * 12,
    }


def test_unset_thread_does_not_overwrite_the_one_thread_row() -> None:
    noise = fair.collect_frames100_noise(
        [_row("1", 1.0), _row("unset", 2.0), _row("1", 3.0, cache="cold-open")]
    )
    assert set(noise) == {
        "readcon-db:cold:rayon=1",
        "readcon-db:cold:rayon=unset",
        "readcon-db:cold-open:rayon=1",
    }
    assert noise["readcon-db:cold:rayon=1"]["median_s"] == 1.0
    assert noise["readcon-db:cold:rayon=unset"]["median_s"] == 2.0
    assert noise["readcon-db:cold:rayon=1"]["n"] == 12
