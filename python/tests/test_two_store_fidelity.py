"""Paper 40×8 frames round-trip the fields both fair read paths return.

Exact equality on the stored CON frames, on ``read_arrays``, and on
``materialize_ase``: symbols, positions, cell lengths, cell angles, the
cell matrix, pbc, masses, fixed flags, velocities, frame index, and
atomic numbers. ``read_arrays`` energy is 0 with ``energy_set`` false,
and ``velocities_set`` and ``pbc_set`` are true. ``materialize_ase``
energy is that same unset pair. CON units metadata matches the written
units. Atom ids are exact on the CON frames and are whatever
``ase_atom_id`` read back.
"""

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


def _bits(left, right) -> bool:
    import numpy as np

    return (
        np.asarray(left, dtype=np.float64).tobytes()
        == np.asarray(right, dtype=np.float64).tobytes()
    )


def test_paper_frames_round_trip_each_field(tmp_path: Path) -> None:
    import numpy as np

    stored = fair_fidelity.write_both(tmp_path)
    try:
        seen = fair_fidelity.views(stored)
        rows = fair_fidelity.compare(stored)
    finally:
        fair_fidelity.fair.close_ase(stored["ase"])
    parsed = seen["parsed"]
    written = seen["written"]
    arrays = seen["arrays"]
    got = seen["got"]
    expect_pos = np.stack([fair_fidelity._positions(frame) for frame in parsed])
    expect_vel = np.stack([fair_fidelity._velocities(frame) for frame in parsed])
    expect_mass = np.stack([fair_fidelity._masses(frame) for frame in parsed])
    expect_fixed = np.stack([fair_fidelity._fixed_rows(frame) for frame in parsed])
    expect_index = np.arange(fair_fidelity.N_FRAMES, dtype=np.float64)
    assert fair_fidelity.CELL_ANGLES == (80.0, 75.0, 70.0)
    assert len(set(fair_fidelity.CELL_ANGLES)) == 3
    assert all(angle != 90.0 for angle in fair_fidelity.CELL_ANGLES)
    assert len(set(fair_fidelity.CELL_LENGTHS)) == 3
    for index, (frame, src) in enumerate(zip(seen["con"], parsed)):
        assert [atom.symbol for atom in frame.atoms] == ["Cu"] * fair_fidelity.N_ATOMS
        assert frame.metadata["units"] == fair_fidelity.UNITS
        assert _bits(fair_fidelity._positions(frame), fair_fidelity._positions(src))
        assert _bits(frame.cell, src.cell)
        assert _bits(frame.angles, src.angles)
        assert tuple(bool(flag) for flag in frame.metadata["pbc"]) == fair_fidelity.PBC
        assert _bits(fair_fidelity._masses(frame), fair_fidelity._masses(src))
        assert (
            fair_fidelity._fixed_rows(frame).tobytes()
            == fair_fidelity._fixed_rows(src).tobytes()
        )
        assert _bits(frame.velocities_array(), fair_fidelity._velocities(src))
        assert [atom.atom_id for atom in frame.atoms] == list(fair_fidelity.ATOM_IDS)
        assert int(frame.frame_index) == index
    assert _bits(arrays["positions"], expect_pos)
    assert _bits(arrays["velocities"], expect_vel)
    assert arrays["velocities_set"] is True
    assert (
        np.asarray(arrays["numbers"], dtype=np.uint32).tobytes()
        == np.full(
            (fair_fidelity.N_FRAMES, fair_fidelity.N_ATOMS), 29, dtype=np.uint32
        ).tobytes()
    )
    assert _bits(arrays["energy"], np.zeros(fair_fidelity.N_FRAMES))
    assert not any(arrays["energy_set"])
    assert all(arrays["pbc_set"])
    assert _bits(arrays["masses"], expect_mass)
    assert arrays["constraints"].tobytes() == expect_fixed.tobytes()
    assert _bits(arrays["frame_index"], expect_index)
    assert all(arrays["frame_index_set"])
    assert all(
        tuple(bool(flag) for flag in flags) == fair_fidelity.PBC
        for flags in arrays["pbc"]
    )
    for matrix, atoms in zip(arrays["cell"], written):
        lengths, angles = fair_fidelity._lengths_angles(matrix)
        assert _bits(lengths, atoms.cell.lengths())
        assert _bits(angles, atoms.cell.angles())
    assert _bits(got["positions"], [atoms.positions for atoms in written])
    assert _bits(got["velocities"], [atoms.get_velocities() for atoms in written])
    assert (
        np.asarray(got["numbers"], dtype=np.uint32).tobytes()
        == np.full(
            (fair_fidelity.N_FRAMES, fair_fidelity.N_ATOMS), 29, dtype=np.uint32
        ).tobytes()
    )
    assert _bits(got["energy"], np.zeros(fair_fidelity.N_FRAMES))
    assert not any(got["energy_set"])
    assert _bits(got["masses"], [atoms.get_masses() for atoms in written])
    assert (
        got["constraints"].tobytes()
        == np.stack(
            [fair_fidelity.fair._fixed_mask(atoms) for atoms in written]
        ).tobytes()
    )
    assert _bits(got["cell"], [atoms.cell.array for atoms in written])
    assert _bits(got["frame_index"], expect_index)
    assert all(got["frame_index_set"])
    assert all(
        tuple(bool(flag) for flag in flags) == fair_fidelity.PBC for flags in got["pbc"]
    )
    for matrix, atoms in zip(got["cell"], written):
        lengths, angles = fair_fidelity._lengths_angles(matrix)
        assert _bits(lengths, atoms.cell.lengths())
        assert _bits(angles, atoms.cell.angles())
    assert all("atom_id" in atoms.arrays for atoms in written)
    assert seen["ase_atom_id"] == [None] * fair_fidelity.N_FRAMES
    by_name = {row["field"]: row for row in rows}
    for name in (
        "symbols",
        "positions",
        "cell lengths",
        "cell angles",
        "pbc",
        "masses",
        "fixed flags",
        "velocities",
        "frame index",
        "atomic numbers",
        "energy",
    ):
        assert by_name[name]["readcon"] is True, name
        assert by_name[name]["ase"] is True, name
    assert by_name["velocities_set"]["readcon"] is True
    assert by_name["velocities_set"]["ase"] is False
    assert by_name["pbc_set"]["readcon"] is True
    assert by_name["pbc_set"]["ase"] is False
    assert by_name["units"]["readcon"] is True
    assert by_name["units"]["ase"] is False
    assert by_name["atom ids"]["readcon"] is True
    assert by_name["atom ids"]["ase"] is False
    assert by_name["float precision"]["readcon_text"] == "bit-exact"
    assert by_name["float precision"]["ase_text"] == "bit-exact"
    text = fair_fidelity.render_markdown(rows)
    assert text == fair_fidelity.FIELDS_PATH.read_text()
