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


def test_append_trajectory_frames_stores_round_trip_text(tmp_path):
    import readcon

    pair = (FIXTURE / "highprec_a.con").read_text()
    pair += (FIXTURE / "highprec_b.con").read_text()
    frames = readcon.read_con_string(pair * 35)
    assert len(frames) == 70
    db = ConCorpus(str(tmp_path / "corpus"))
    assert db.append_trajectory_frames(1, frames, source="highprec") == 70
    for idx in (0, 1, 64, 69):
        stored = db.get_frame_text(1, idx)
        assert stored == readcon.write_con_string(frames, start=idx, stop=idx + 1)
        assert db.frame_hash(1, idx) == ConCorpus.hash_con_text(stored)
        got = readcon.read_con_string(stored)[0]
        want = frames[idx]
        assert tuple(got.cell) == tuple(want.cell)
        assert [(a.x, a.y, a.z, a.mass) for a in got.atoms] == [
            (a.x, a.y, a.z, a.mass) for a in want.atoms
        ]
    head = db.get_frame_text(1, 64)
    assert "21.702000987654323" in head
    assert "1.23456789e-9" in head
