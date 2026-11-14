"""append_trajectory_frames commits every group or none of them."""

from pathlib import Path

import readcon
from readcon_db import ConCorpus

FIXTURE = Path(__file__).resolve().parents[2] / "resources" / "test" / "tiny_cuh2.con"


def _frames(n: int):
    one = readcon.read_con(str(FIXTURE))[0]
    return [one] * n


def test_later_group_failure_publishes_nothing(tmp_path, monkeypatch):
    frames = _frames(70)
    db = ConCorpus(str(tmp_path / "db"))
    monkeypatch.setenv("READCON_DB_FAIL_AFTER_GROUPS", "2")
    try:
        db.append_trajectory_frames(1, frames, source="partial")
    except RuntimeError as err:
        assert "injected" in str(err)
    else:
        raise AssertionError("the injected failure did not fire")
    assert db.select(traj_id=1) == []
    assert db.append_trajectory_frames(1, frames, source="full") == 70
    assert len(db.select(traj_id=1)) == 70
