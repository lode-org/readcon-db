"""Paper 40×8 frames round-trip every field the two fair stores can hold."""

from __future__ import annotations

import sys
from pathlib import Path

import pytest

pytest.importorskip("readcon")
pytest.importorskip("readcon_db")

BENCH = Path(__file__).resolve().parents[2] / "examples" / "benchmarks"
sys.path.insert(0, str(BENCH))

import fair_fidelity  # noqa: E402
import readcon  # noqa: E402


def test_distinct_cu_masses_cannot_be_written_as_con(tmp_path: Path) -> None:
    import numpy as np
    from ase import Atoms

    import fair_ase_protocol as fair

    atoms = [
        readcon.Atom("Cu", 0.0, 0.0, 0.0, mass=62.5, atom_id=0),
        readcon.Atom("Cu", 1.0, 0.0, 0.0, mass=63.5, atom_id=1),
    ]
    frame = readcon.ConFrame((10.0, 10.0, 10.0), (90.0, 90.0, 90.0), atoms)
    with pytest.raises(readcon.MassMismatch):
        readcon.write_con_string([frame])
    written = Atoms(
        "Cu2",
        positions=[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
        masses=[62.5, 63.5],
    )
    db, _settings = fair.connect_fair(tmp_path / "atoms.db")
    db.connection = db._connect()
    db.change_count = 0
    fair.reset_commits(db)
    db.write(written, frame_index=0)
    db.connection.commit()
    try:
        got = next(db.select()).toatoms().get_masses()
    finally:
        fair.close_ase(db)
    assert (
        np.asarray(got, dtype=np.float64).tobytes()
        == np.asarray(written.get_masses(), dtype=np.float64).tobytes()
    )


def test_paper_frames_round_trip_each_field(tmp_path: Path) -> None:
    stored = fair_fidelity.write_both(tmp_path)
    try:
        rows = fair_fidelity.compare(stored)
    finally:
        fair_fidelity.fair.close_ase(stored["ase"])
    by_name = {row["field"]: row for row in rows}
    for name in (
        "symbols",
        "positions",
        "cell",
        "pbc",
        "masses",
        "fixed flags",
        "velocities",
    ):
        assert by_name[name]["readcon"] is True, name
        assert by_name[name]["ase"] is True, name
    assert by_name["atom ids"]["readcon"] is True
    assert by_name["atom ids"]["ase"] is False
    assert by_name["float precision"]["readcon_text"] == "bit-exact"
    assert by_name["float precision"]["ase_text"] == "bit-exact"
    text = fair_fidelity.render_markdown(rows)
    assert text == fair_fidelity.FIELDS_PATH.read_text()
