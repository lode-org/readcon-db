"""Regression tests for db24 rerun/export path handling."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

import pytest

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


def test_bad_sample_count_is_not_parsed_at_import() -> None:
    env = os.environ.copy()
    env["READCON_DB24_N"] = "oops"
    env.pop("READCON_RERUN_RELEASE", None)
    done = subprocess.run(
        [sys.executable, "-c", "import db24_drive"],
        cwd=Path(__file__).resolve().parent,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    assert done.returncode == 0, done.stderr


def test_bad_sample_count_is_a_clean_error_at_main(monkeypatch) -> None:
    monkeypatch.setenv("READCON_DB24_N", "oops")
    with pytest.raises(SystemExit, match="READCON_DB24_N must be an integer"):
        drv.sample_count()


def test_sample_count_below_12_is_allowed_outside_rerun(monkeypatch) -> None:
    monkeypatch.delenv("READCON_RERUN_RELEASE", raising=False)
    monkeypatch.setenv("READCON_DB24_N", "6")
    assert drv.sample_count() == 6


def test_rerun_or_release_requires_n_at_least_12(monkeypatch, tmp_path: Path) -> None:
    monkeypatch.setenv("READCON_RERUN_RELEASE", "1")
    monkeypatch.setenv("READCON_DB24_N", "11")
    monkeypatch.setattr("sys.argv", ["db24_drive.py", "--out", str(tmp_path / "out")])

    def stop() -> None:
        raise RuntimeError("passed-floor")

    monkeypatch.setattr(drv, "require_pushed_commit", stop)
    with pytest.raises(SystemExit, match="at least 12 for a rerun or release"):
        drv.main()
    monkeypatch.setenv("READCON_DB24_N", "12")
    assert drv.sample_count() == 12


def test_main_outside_rerun_does_not_apply_the_floor(
    monkeypatch, tmp_path: Path
) -> None:
    monkeypatch.delenv("READCON_RERUN_RELEASE", raising=False)
    monkeypatch.setenv("READCON_DB24_N", "6")
    monkeypatch.setattr("sys.argv", ["db24_drive.py", "--out", str(tmp_path / "db24")])

    def stop() -> None:
        raise RuntimeError("passed-floor")

    monkeypatch.setattr(drv, "require_pushed_commit", stop)
    with pytest.raises(RuntimeError, match="passed-floor"):
        drv.main()
    assert drv.N == 6


def test_help_says_the_floor_is_only_for_rerun_or_release(monkeypatch, capsys) -> None:
    monkeypatch.setattr("sys.argv", ["db24_drive.py", "--help"])
    with pytest.raises(SystemExit) as caught:
        drv.parse_out(drv.BENCH)
    assert caught.value.code == 0
    text = capsys.readouterr().out
    assert "at least 12" in text
    assert "rerun or release" in text
