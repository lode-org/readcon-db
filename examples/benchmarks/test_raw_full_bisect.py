"""The n=30 raw full-read bisect does not isolate a commit."""

from __future__ import annotations

import raw_full_bisect as bisect


def test_every_step_interval_includes_one() -> None:
    assert len(bisect.STEPS) == 6
    for step in bisect.STEPS:
        assert step["n"] >= 30
        lo, hi = step["ci95"]
        assert lo <= 1.0 <= hi
        assert 0 <= step["right_faster"] <= step["n"]
    assert bisect.intervals_include_one()
    largest = max(bisect.STEPS, key=lambda step: step["paired_median"])
    assert largest["left"] == "aedcbda4"
    assert largest["right"] == "750b197"
    assert largest["threads"] == "1"
    assert largest["right_faster"] == 11
