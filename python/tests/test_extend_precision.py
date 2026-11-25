"""Precision of frames extended onto a live trajectory."""

from pathlib import Path

import readcon
from readcon_db import ConCorpus

FIXTURE = Path(__file__).resolve().parents[2] / "resources" / "test"


def _text(name):
    return (FIXTURE / name).read_text()


def _numbers(text):
    frame = readcon.read_con_string(text)[0]
    masses = [atom.mass for atom in frame.atoms]
    coords = [(atom.x, atom.y, atom.z) for atom in frame.atoms]
    return tuple(frame.cell), masses, coords


def _same_generation(db, spans):
    for idx, span in enumerate(spans):
        digest = ConCorpus.xxh3_128(span.encode())
        if db.frame_hash(1, idx) != digest:
            return False
        if (1, idx) not in db.select(exact_hash=digest):
            return False
    return True


def test_extend_trajectory_keeps_source_text_and_generation(tmp_path):
    head = _text("highprec_a.con")
    tail = _text("highprec_b.con")
    root = tmp_path / "db"
    db = ConCorpus(str(root))
    assert db.append_trajectory_str(1, head, source="run.con") == 1
    tail_path = tmp_path / "tail.con"
    tail_path.write_text(tail)
    assert db.extend_trajectory(1, str(tail_path)) == 2
    assert db.get_frame_text(1, 1) == tail
    del db
    db = ConCorpus(str(root))
    assert db.get_frame_text(1, 0) == head
    stored = db.get_frame_text(1, 1)
    assert stored == tail
    assert _numbers(stored) == _numbers(tail)
    assert _numbers(db.get_frame_text(1, 0)) == _numbers(head)
    assert "1.23456789012345" in head
    assert "1000000.123456789" in head
    assert "0.00000000123456789" in head
    assert "15.345600123456789" in head
    assert _same_generation(db, (head, tail))


def test_legacy_six_decimal_text_is_a_different_generation(tmp_path):
    original = _text("highprec_a.con")
    frames = readcon.read_con_string(original)
    legacy = readcon.write_con_string(frames, precision=6)
    assert legacy != original
    assert "1.23456789012345" not in legacy
    assert "0.00000000123456789" not in legacy
    root = tmp_path / "db"
    db = ConCorpus(str(root))
    db.append_trajectory_str(4, legacy, source="old-extend")
    del db
    db = ConCorpus(str(root))
    stored = db.get_frame_text(4, 0)
    assert stored == legacy
    assert db.frame_hash(4, 0) == ConCorpus.xxh3_128(stored.encode())
    assert (4, 0) in db.select(exact_hash=db.frame_hash(4, 0))
    assert db.select(exact_hash=ConCorpus.xxh3_128(original.encode())) == []
    got = _numbers(stored)
    src = _numbers(original)
    assert got != src
    more = _text("highprec_b.con")
    assert db.extend_trajectory(4, str(FIXTURE / "highprec_b.con")) == 2
    assert db.get_frame_text(4, 0) == stored
    assert db.get_frame_text(4, 1) == more
