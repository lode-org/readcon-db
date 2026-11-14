"""Realistic arms are large enough to take the shuffle path."""

from __future__ import annotations

import shutil
from collections import Counter
from pathlib import Path

import numpy as np
import pytest

import fair_ase_protocol as fair
from readcon_db import ConCorpus

# Species counts of the pinned NhaA gro, in the order the arm writes.
NHA_SPECIES_COUNTS = {
    "C": 19020,
    "H": 35642,
    "N": 1230,
    "O": 4222,
    "P": 390,
    "S": 26,
    "Na": 121,
    "Cl": 51,
}


def test_lattice_frames_are_distinct() -> None:
    _text, records = fair.build_text_records(4, 8)
    fair.refuse_identical_frames([rec["positions"] for rec in records])
    assert len(records) == 4


def test_repeated_coordinates_are_refused() -> None:
    pos = np.zeros((3, 3), dtype=np.float64)
    with pytest.raises(RuntimeError, match="byte-identical"):
        fair.refuse_identical_frames([pos, pos.copy()])


def test_fetch_pinned_refuses_a_bad_sha256() -> None:
    """A file already in build/ with the wrong digest is refused, offline."""
    name = "pin-test-bad.bin"
    dest = fair._FETCH_DIR / name
    dest.parent.mkdir(parents=True, exist_ok=True)
    dest.write_bytes(b"not the pinned bytes")
    with pytest.raises(RuntimeError, match="sha256"):
        fair.fetch_pinned("http://127.0.0.1/unused", "0" * 64, name)


def test_md_records_keep_the_expected_species_list() -> None:
    _text, records = fair.build_md_records(1)
    symbols = records[0]["symbols"]
    assert symbols == fair.nha_topology().symbols
    assert len(symbols) == fair.NHA_ATOMS
    assert dict(Counter(symbols)) == NHA_SPECIES_COUNTS


def test_thermal_frames_are_seeded_and_distinct() -> None:
    first = fair.thermal_positions(0)
    again = fair.thermal_positions(0)
    assert first.tobytes() == again.tobytes()
    assert first.tobytes() != fair.thermal_positions(1).tobytes()
    _text, records = fair.build_thermal_records(2)
    assert len(records) == 2
    fair.refuse_identical_frames([rec["positions"] for rec in records])


def test_build_thermal_records_refuses_identical_frames(monkeypatch) -> None:
    base = fair.nha_topology().base
    monkeypatch.setattr(fair, "thermal_positions", lambda _index: base.copy())
    with pytest.raises(RuntimeError, match="byte-identical"):
        fair.build_thermal_records(2)


def test_realistic_arms_match_the_lattice_scale_and_clear_the_shuffle_floor() -> None:
    lattice = fair.arm_profile("lattice")
    assert lattice["role"] == "best case"
    assert lattice["expects_shuffle"]
    for arm in ("md", "thermal"):
        profile = fair.arm_profile(arm)
        assert profile["role"] == "realistic"
        assert profile["atoms_per_frame"] >= 1000
        assert profile["frame_coord_bytes"] >= fair.SHUFFLE_MIN_COOKED_BYTES
        assert profile["expects_shuffle"]
        assert (
            profile["n_frames"] * profile["frame_coord_bytes"]
            >= fair.LATTICE_COORD_BYTES
        )


def _write(monkeypatch, arm: str, codec: str, dest: Path, n_frames: int) -> dict:
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    monkeypatch.setenv("READCON_DB_COOKED_CODEC", codec)
    text, records = fair.build_arm(arm, n_frames)
    fair.refuse_identical_frames([rec["positions"] for rec in records])
    db = ConCorpus(str(dest))
    n = db.append_trajectory_str(1, text, source="traj.con")
    assert int(n) == n_frames
    del db
    got = fair.corpus_arm_record(dest, n_frames)
    got["atoms_per_frame"] = records[0]["positions"].shape[0]
    got["frame_coord_bytes"] = fair.frame_coord_bytes(
        got["atoms_per_frame"], has_forces=arm == "lattice"
    )
    return got


def test_cooked_codec_binding_returns_raw_and_shuffle(monkeypatch) -> None:
    """The binding reports the codec the write actually stored."""
    text, _records = fair.build_text_records(1, 3000)
    assert (
        fair.frame_coord_bytes(3000, has_forces=True) >= fair.SHUFFLE_MIN_COOKED_BYTES
    )
    root = Path(__file__).resolve().parents[2] / "build" / "db28-codec"
    for codec in ("raw", "shuffle-zstd"):
        dest = root / codec
        if dest.exists():
            shutil.rmtree(dest)
        dest.mkdir(parents=True)
        monkeypatch.setenv("READCON_DB_COOKED_CODEC", codec)
        db = ConCorpus(str(dest))
        db.append_trajectory_str(1, text, source="traj.con")
        del db
        got = fair.corpus_arm_record(dest, 1)
        assert got["cooked_codec"] == codec
        assert got["hit_shuffle"] is (codec == "shuffle-zstd")


def test_shuffle_arms_do_not_store_the_raw_bytes(monkeypatch) -> None:
    """An arm that should shuffle must not leave the raw and shuffle files equal."""
    root = Path(__file__).resolve().parents[2] / "build" / "db28-shuffle-test"
    for arm in ("thermal", "md"):
        raw = _write(monkeypatch, arm, "raw", root / arm / "raw", 2)
        shuffled = _write(monkeypatch, arm, "shuffle-zstd", root / arm / "shuffle", 2)
        assert raw["atoms_per_frame"] == shuffled["atoms_per_frame"]
        assert shuffled["frame_coord_bytes"] >= fair.SHUFFLE_MIN_COOKED_BYTES
        assert shuffled["hit_shuffle"], shuffled
        assert raw["hit_shuffle"] is False
        assert raw["cooked_codec"] == "raw"
        assert shuffled["cooked_codec"] == "shuffle-zstd"
        assert raw["corpus_bytes"] != shuffled["corpus_bytes"]
        assert raw["frames_soa_value_bytes"] != shuffled["frames_soa_value_bytes"]
