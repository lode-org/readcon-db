"""Type-7 quantiles used for the ASE comparison."""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "examples" / "benchmarks"))
import fair_ase_protocol as fair


def test_type7_matches_numpy_linear_on_the_published_ase_log():
    stats = fair.summarize(fair.PUBLISHED_ASE)
    expect = np.percentile(np.asarray(fair.PUBLISHED_ASE), [25, 50, 75], method="linear")
    assert stats["q1_s"] == float(expect[0])
    assert stats["median_s"] == float(expect[1])
    assert stats["q3_s"] == float(expect[2])
    assert stats["iqr_s"] == float(expect[2] - expect[0])
    assert stats["n"] == 6
    assert abs(stats["median_s"] - 0.07231816899911792) < 1e-15
    assert abs(stats["iqr_s"] - 0.03769298000224808) < 1e-15


def test_type7_matches_numpy_linear_on_the_published_readcon_log():
    stats = fair.summarize(fair.PUBLISHED_READCON)
    assert abs(stats["median_s"] - 0.0046723915002075955) < 1e-15
