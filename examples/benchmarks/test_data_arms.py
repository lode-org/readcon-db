"""Lattice is the best case. The MD and thermal arms are distinct frames."""

from __future__ import annotations

import hashlib

import numpy as np
import pytest

import fair_ase_protocol as fair


def test_lattice_frames_are_distinct() -> None:
    _text, records = fair.build_text_records(4, 8)
    fair.refuse_identical_frames([rec["positions"] for rec in records])
    assert len(records) == 4


def test_repeated_coordinates_are_refused() -> None:
    pos = np.zeros((3, 3), dtype=np.float64)
    with pytest.raises(RuntimeError, match="byte-identical"):
        fair.refuse_identical_frames([pos, pos.copy()])


def test_md_trajectory_is_pinned_and_distinct() -> None:
    text = fair.md_trajectory_text()
    assert hashlib.sha256(text.encode()).hexdigest() == fair.MD_TRAJECTORY_SHA256
    _text, records = fair.build_md_records()
    assert len(records) == fair.MD_N_FRAMES
    fair.refuse_identical_frames([rec["positions"] for rec in records])
    assert records[0]["symbols"] == fair.MD_SYMBOLS


def test_thermal_frames_are_seeded_and_distinct() -> None:
    first = fair.thermal_positions(0)
    again = fair.thermal_positions(0)
    assert first.tobytes() == again.tobytes()
    assert first.tobytes() != fair.thermal_positions(1).tobytes()
    _text, records = fair.build_thermal_records(8)
    assert len(records) == 8
    fair.refuse_identical_frames([rec["positions"] for rec in records])
