"""Paired bisect summaries are computed from the samples."""

from __future__ import annotations

import json
from pathlib import Path

import pytest
import raw_full_bisect as bisect


def test_short_pairs_are_refused() -> None:
    with pytest.raises(ValueError, match="n>=30"):
        bisect.summarize_pair([1.0] * 29, [1.0] * 29)


def test_ratio_median_and_win_count_follow_the_samples() -> None:
    summary = bisect.summarize_pair([1.0] * 30, [0.5] * 30)
    assert summary["n"] == 30
    assert summary["left_median_s"] == pytest.approx(1.0)
    assert summary["right_median_s"] == pytest.approx(0.5)
    assert summary["paired_median"] == pytest.approx(0.5)
    assert summary["right_faster"] == 30
    lo, hi = summary["ci95"]
    assert lo <= summary["paired_median"] <= hi
    assert hi < 1.0


def test_equal_samples_sit_on_one() -> None:
    summary = bisect.summarize_pair([2.0] * 30, [2.0] * 30)
    assert summary["paired_median"] == pytest.approx(1.0)
    assert summary["right_faster"] == 0
    lo, hi = summary["ci95"]
    assert lo <= 1.0 <= hi
    assert bisect.intervals_include_one(
        [
            {
                "ci95": summary["ci95"],
            }
        ]
    )


def test_steps_are_loaded_from_the_record(tmp_path: Path) -> None:
    record = {
        "left": "left",
        "right": "right",
        "left_sha": "a" * 40,
        "right_sha": "b" * 40,
        "threads": "1",
        "samples": {"left": [1.0] * 30, "right": [2.0] * 30},
    }
    path = tmp_path / "samples.json"
    path.write_text(json.dumps([record]))
    steps = bisect.load_steps(path)
    assert bisect.SAMPLES_PATH == (
        Path(bisect.__file__).resolve().parents[2]
        / "bench"
        / "db29"
        / "bisect"
        / "samples.json"
    )
    assert len(steps) == 1
    step = steps[0]
    assert step["left_sha"] == record["left_sha"]
    assert step["right_sha"] == record["right_sha"]
    assert step["threads"] == "1"
    assert step["n"] == 30
    assert step["right_faster"] == 0
    assert step["paired_median"] == pytest.approx(2.0)
    lo, hi = step["ci95"]
    assert lo <= step["paired_median"] <= hi
