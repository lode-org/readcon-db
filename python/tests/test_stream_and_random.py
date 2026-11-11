"""Append stays near one chunk, and a random read does not build every atom."""

import time
from pathlib import Path

import readcon
from readcon_db import ConCorpus


def _hwm_kib() -> int:
    for line in Path("/proc/self/status").read_text(encoding="utf-8").splitlines():
        if line.startswith("VmHWM:"):
            return int(line.split()[1])
    return 0


def _frames(n: int):
    atoms = [readcon.Atom("Cu", float(i % 8), 0.0, 1.0, atom_id=i) for i in range(32)]
    frame = readcon.ConFrame((12.0, 12.0, 12.0), (90.0, 90.0, 90.0), atoms)
    return [frame] * n


def test_append_frames_high_water_stays_near_one_chunk(tmp_path):
    frames = _frames(4000)
    db = ConCorpus(str(tmp_path / "corpus"))
    before = _hwm_kib()
    n = db.append_trajectory_frames(1, frames, source="chunked")
    grew = _hwm_kib() - before
    assert n == 4000
    # Converting every frame at once is tens of MiB. One chunk of 64 is not.
    assert grew < 16 * 1024, grew
    got = db.get_frame(1, 3999)
    assert got.atoms[3].x == 3.0
    assert got.xyz[0] == (0.0, 0.0, 1.0)


def test_append_read_con_does_not_build_every_python_frame(tmp_path):
    atoms = [readcon.Atom("Cu", float(i % 8), 0.0, 1.0, atom_id=i) for i in range(32)]
    frame = readcon.ConFrame((12.0, 12.0, 12.0), (90.0, 90.0, 90.0), atoms)
    src = tmp_path / "src.con"
    readcon.write_con(str(src), [frame] * 20000)
    loaded = readcon.read_con(str(src), n_threads=1)
    db = ConCorpus(str(tmp_path / "corpus"))
    before = _hwm_kib()
    n = db.append_trajectory_frames(1, loaded, source="read-con")
    grew = _hwm_kib() - before
    assert n == 20000
    # Indexing the read result builds a Python frame per frame and keeps it.
    assert grew < 8 * 1024, grew
    assert db.get_frame(1, 19999).atoms[3].x == 3.0


def test_random_get_frame_does_not_rebuild_a_python_frame(tmp_path):
    frames = _frames(64)
    db = ConCorpus(str(tmp_path / "corpus"))
    db.append_trajectory_frames(1, frames, source="random")
    got = db.get_frame(1, 3)
    # read_con_string returns a ConFrame whose atoms are a Python list.
    # The stored frame builds one atom on index.
    assert type(got).__name__ == "StoredFrame", type(got).__name__
    assert type(got.atoms).__name__ == "StoredAtoms", type(got.atoms).__name__
    assert got.atoms[0].x == 0.0
    assert got.xyz[1][0] == 1.0
    started = time.perf_counter()
    sink = 0.0
    for i in range(1000):
        sink += db.get_frame(1, i % 64).atoms[0].x
    assert sink == 0.0
    assert time.perf_counter() - started < 0.2
