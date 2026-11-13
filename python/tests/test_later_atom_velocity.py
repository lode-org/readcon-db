"""A velocity that is not on the first atom stays visible.

readcon 0.21.0 reports ``has_velocities`` from the first atom only, and
``velocities_array`` is ``None`` in that case, so ``to_ase`` drops the
later atom's velocity. 0.22.0 reports the section when any atom carries
one. The corpus still stores the oxygen-first order and that velocity.
"""

from pathlib import Path

import readcon
from readcon_db import ConCorpus


def _oxygen_first():
    c1 = (3.0, 4.0, 4.0)
    c2 = (4.336, 4.0, 4.0)
    oxygen = (5.016, 5.178, 4.0)
    return readcon.ConFrame(
        (18.0, 18.0, 18.0),
        (90.0, 90.0, 90.0),
        [
            readcon.Atom("O", *oxygen, atom_id=2, mass=15.999),
            readcon.Atom("C", *c1, atom_id=0, mass=12.011, vx=0.1, vy=-0.2, vz=0.3),
            readcon.Atom("C", *c2, atom_id=1, mass=12.011),
        ],
    )


def test_later_atom_velocity_reaches_ase_and_the_corpus(tmp_path: Path):
    frame = _oxygen_first()
    assert frame.has_velocities
    velocities = frame.velocities_array()
    assert velocities is not None
    assert velocities.shape == (3, 3)
    assert list(velocities[1]) == [0.1, -0.2, 0.3]

    atoms = frame.to_ase()
    assert [atom.symbol for atom in atoms] == ["O", "C", "C"]
    assert abs(float(atoms.get_masses()[0]) - 15.999) < 1e-9
    ase_velocities = atoms.get_velocities()
    assert ase_velocities is not None
    assert abs(float(ase_velocities[1][0])) > 0.0

    db = ConCorpus(str(tmp_path / "corpus"))
    assert db.append_trajectory_frames(1, [frame], source="oxygen-first") == 1
    stored = db.get_frame(1, 0)
    assert [atom.symbol for atom in stored.atoms] == ["O", "C", "C"]
    got = db.get_velocities(1, 0)
    assert got is not None
    assert list(got[1]) == [0.1, -0.2, 0.3]
    recovered = readcon.read_con_string(db.get_frame_text(1, 0))[0]
    assert recovered.has_velocities
    assert recovered.atoms[0].symbol == "O"
    assert recovered.atoms[0].mass == 15.999
    assert recovered.atoms[1].vx == 0.1
