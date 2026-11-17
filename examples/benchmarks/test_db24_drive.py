"""Regression tests for db24 rerun/export path handling."""

from __future__ import annotations

from pathlib import Path

import db24_drive as drv


def test_bench_rel_allows_outside_repo(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.setattr(drv, "BENCH", tmp_path / "outside" / "db24")
    text = drv.bench_rel()
    assert text == str(tmp_path / "outside" / "db24")


def test_parse_out_keeps_a_directory_outside_the_repo(
    monkeypatch, tmp_path: Path
) -> None:
    outside = tmp_path / "abs-out"
    monkeypatch.setattr(drv, "BENCH", outside)
    monkeypatch.setattr("sys.argv", ["db24_drive.py"])
    assert drv.parse_out(outside) == outside
    monkeypatch.setattr("sys.argv", ["db24_drive.py", "--out", str(outside / "nested")])
    assert drv.parse_out(outside) == outside / "nested"
    monkeypatch.setattr("sys.argv", ["db24_drive.py", "--out", "bench/custom"])
    assert drv.parse_out(outside) == drv.REPO / "bench" / "custom"
