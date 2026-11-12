"""Oxygen-first rows match a stored frame after the canonical index is built."""

from pathlib import Path

import readcon
from readcon_db import ConCorpus


def _pair():
    c1 = (3.0, 4.0, 4.0)
    c2 = (4.336, 4.0, 4.0)
    oxygen = (5.016, 5.178, 4.0)
    carbon_first = readcon.ConFrame(
        (18.0, 18.0, 18.0),
        (90.0, 90.0, 90.0),
        [
            readcon.Atom("C", *c1, atom_id=0, mass=12.011),
            readcon.Atom("C", *c2, atom_id=1, mass=12.011),
            readcon.Atom("O", *oxygen, atom_id=2, mass=15.999),
        ],
    )
    oxygen_first = readcon.ConFrame(
        (18.0, 18.0, 18.0),
        (90.0, 90.0, 90.0),
        [
            readcon.Atom("O", *oxygen, atom_id=2, mass=15.999),
            readcon.Atom("C", *c1, atom_id=0, mass=12.011),
            readcon.Atom("C", *c2, atom_id=1, mass=12.011),
        ],
    )
    return carbon_first, oxygen_first


def test_oxygen_first_matches_and_reports_the_permutation(tmp_path: Path):
    carbon_first, oxygen_first = _pair()
    text = readcon.write_con_string([oxygen_first])
    db = ConCorpus(str(tmp_path / "corpus"))
    db.append_trajectory_frames(1, [carbon_first], source="reactant")
    content = db.frame_hash(1, 0)
    assert content != db.xxh3_128(text.encode())
    try:
        db.find_by_atom_order(text)
    except RuntimeError as err:
        assert "index-canonical" in str(err)
    else:
        raise AssertionError("unindexed lookup should fail")
    assert db.index_canonical_identity() == 1
    assert db.frame_hash(1, 0) == content
    version, digest = db.frame_canonical_identity(1, 0)
    assert version == 1
    assert len(digest) == 16
    assert digest != db.xxh3_128(text.encode())
    hits = db.find_by_atom_order(text)
    assert len(hits) == 1
    assert hits[0].query_index == 0
    assert hits[0].traj_id == 1
    assert hits[0].frame_idx == 0
    assert list(hits[0].permutation) == [1, 2, 0]
