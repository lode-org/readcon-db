"""Both read paths return writable NumPy arrays."""

from pathlib import Path

import numpy as np

from readcon_db import ConCorpus


def test_raw_and_shuffled_arrays_are_writable(tmp_path, monkeypatch):
    import sys

    sys.path.insert(
        0, str(Path(__file__).resolve().parents[2] / "examples" / "benchmarks")
    )
    import fair_ase_protocol as fair

    text, _records = fair.build_text_records(1, 8)
    monkeypatch.delenv("READCON_DB_COOKED_CODEC", raising=False)
    raw_dir = tmp_path / "raw"
    raw_dir.mkdir()
    db = ConCorpus(str(raw_dir / "corpus"))
    assert db.append_trajectory_str(1, text, source="t.con") == 1
    got = db.read_arrays(1, 0)
    _assert_writable(got)

    wide, _ = fair.build_text_records(1, 4096)
    monkeypatch.setenv("READCON_DB_COOKED_CODEC", "shuffle-zstd")
    sh_dir = tmp_path / "shuffle"
    sh_dir.mkdir()
    db = ConCorpus(str(sh_dir / "corpus"))
    assert db.append_trajectory_str(1, wide, source="t.con") == 1
    got = db.read_arrays(1, 0)
    _assert_writable(got)
    got["positions"][0, 0, 0] = 1.0


def _assert_writable(got) -> None:
    for key in (
        "positions",
        "forces",
        "velocities",
        "masses",
        "numbers",
        "constraints",
        "cell",
        "energy",
    ):
        value = got[key]
        if value is None:
            continue
        assert isinstance(value, np.ndarray), key
        assert value.flags.writeable, key
        value.flat[0] = value.flat[0]
