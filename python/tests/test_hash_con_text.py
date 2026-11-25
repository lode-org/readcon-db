"""Content hash of CON text against the hash the corpus stores."""

from pathlib import Path

import pytest
from readcon_db import ConCorpus

FIXTURE = Path(__file__).resolve().parents[2] / "resources" / "test"


def test_hash_con_text_matches_stored_hash_of_high_precision_file(tmp_path):
    head = (FIXTURE / "highprec_a.con").read_text()
    tail = (FIXTURE / "highprec_b.con").read_text()
    db = ConCorpus(str(tmp_path / "db"))
    assert db.append_trajectory(1, str(FIXTURE / "highprec_a.con")) == 1
    assert db.extend_trajectory(1, str(FIXTURE / "highprec_b.con")) == 2
    for idx, text in enumerate((head, tail)):
        digest = ConCorpus.hash_con_text(text)
        assert digest == db.frame_hash(1, idx)
        assert db.find_by_hash(digest) == (1, idx)
        assert db.select(exact_hash=digest) == [(1, idx)]
    assert ConCorpus.hash_con_text(head + tail) == db.frame_hash(1, 0)
    with pytest.raises(RuntimeError):
        ConCorpus.hash_con_text("")
