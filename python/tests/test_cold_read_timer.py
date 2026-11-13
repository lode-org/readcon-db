"""A cold read must not collect the whole Python heap inside the timer."""

import gc
import os
import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "examples" / "benchmarks"))
import fair_ase_protocol as fair


def test_cold_read_does_not_collect_inside_the_timer(tmp_path, monkeypatch):
    text, atoms, records = fair.build_records(1, 8)
    fair.stamp_frame_index(atoms)
    payload = {
        "text": text,
        "atoms": atoms,
        "records": records,
        "n_frames": 1,
        "geometry_only": False,
    }
    dest = tmp_path / "readcon"
    fair.insert_fair("readcon-db", payload, dest)
    calls = []

    def spy(*_args, **_kwargs):
        calls.append(1)
        raise AssertionError("gc.collect ran inside the cold read")

    monkeypatch.setattr(gc, "collect", spy)
    monkeypatch.setattr(fair.gc, "collect", spy)
    fair.drop_files(fair.lmdb_files(dest / "corpus"))
    fair.time_call(
        lambda: fair.measure_read("readcon-db", dest, "full_read", 1, records, 0)
    )
    assert calls == []
    # The environment was released, so another readonly open works.
    fair.measure_read("readcon-db", dest, "selected", 1, records, 0)


def test_one_frame_selected_columns_match_the_full_read(tmp_path):
    """A one-frame selected read is the same column scan as the full read."""
    text, atoms, records = fair.build_records(1, 8)
    fair.stamp_frame_index(atoms)
    payload = {
        "text": text,
        "atoms": atoms,
        "records": records,
        "n_frames": 1,
        "geometry_only": False,
    }
    dest = tmp_path / "readcon"
    fair.insert_fair("readcon-db", payload, dest)
    db = fair.open_readcon(dest / "corpus")
    try:
        full = db.read_columns(1)
        one = db.read_columns(1, 0)
    finally:
        fair.close_lmdb(db)
    assert int(one["n_frames"]) == int(full["n_frames"]) == 1
    assert bytes(np.asarray(one["positions"])) == bytes(np.asarray(full["positions"]))
    assert bytes(np.asarray(one["forces"])) == bytes(np.asarray(full["forces"]))
    assert bytes(np.asarray(one["masses"])) == bytes(np.asarray(full["masses"]))
    assert bytes(np.asarray(one["cell"])) == bytes(np.asarray(full["cell"]))


def test_mincore_sees_a_resident_file_and_labels_advice(tmp_path):
    page = os.sysconf("SC_PAGE_SIZE")
    path = tmp_path / "pages.bin"
    path.write_bytes(b"x" * (page * 4))
    assert path.read_bytes()[0] == ord("x")
    held = fair.resident_fraction(path)
    assert held > 0.0
    fraction, label = fair.advise_cold(path)
    assert label in ("verified", "resident")
    if label == "verified":
        assert fraction == 0.0
        assert fair.resident_fraction(path) == 0.0
    else:
        assert fraction > 0.0
