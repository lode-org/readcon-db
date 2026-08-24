"""In-memory CON text and frame ingest (no temp file)."""

from pathlib import Path

from readcon_db import ConCorpus

FIXTURE = Path(__file__).resolve().parents[2] / "resources" / "test"


def test_append_trajectory_str(tmp_path):
    text = (FIXTURE / "tiny_cuh2.con").read_text()
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory_str(1, text, source="tiny_cuh2.con")
    assert n >= 1
    keys = db.select(traj_id=1)
    assert len(keys) == n


def test_append_trajectory_frames(tmp_path):
    import readcon

    frames = readcon.read_con(str(FIXTURE / "tiny_cuh2.con"))
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory_frames(1, frames, source="tiny_cuh2")
    assert n == len(frames)
    keys = db.select(traj_id=1)
    assert len(keys) == n
