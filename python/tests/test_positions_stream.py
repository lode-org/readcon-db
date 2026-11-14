"""Positions come back as one NumPy array, cooked or parsed."""

from pathlib import Path

import numpy as np
import readcon
from readcon_db import ConCorpus

FIXTURE = Path(__file__).resolve().parents[2] / "resources" / "test"


def test_stream_matches_each_frame_and_the_con_bytes(tmp_path):
    text = (FIXTURE / "tiny_cuh2_vel_forces.con").read_text()
    original = readcon.read_con_string(text)
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory_str(1, text, source="tiny_cuh2_vel_forces.con")
    assert n == len(original)
    assert db.has_valid_cooked_soa(1, 0)
    whole = np.asarray(db.get_positions_all(1))
    assert whole.dtype == np.float64
    assert whole.shape == (n, len(original[0].atoms), 3)
    for i, frame in enumerate(original):
        one = np.asarray(db.get_positions(1, i))
        expect = np.asarray(frame.coords_array(), dtype=np.float64)
        assert one.shape == expect.shape
        assert one.tobytes() == expect.tobytes()
        assert whole[i].tobytes() == expect.tobytes()
    cooked = whole.tobytes()
    for i in range(n):
        db.delete_cooked_soa(1, i)
    assert not db.has_valid_cooked_soa(1, 0)
    parsed = np.asarray(db.get_positions_all(1))
    assert parsed.tobytes() == cooked
    assert np.asarray(db.get_positions(1, 0)).tobytes() == cooked[: parsed[0].nbytes]
