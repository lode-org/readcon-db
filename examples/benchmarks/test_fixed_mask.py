"""Fixed-mask reader: known constraints, and the ones it must not invent."""

from __future__ import annotations

import sys
from pathlib import Path

import numpy as np
import pytest
from ase import Atoms
from ase.constraints import FixAtoms, FixBondLengths, FixCartesian, FixScaled

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fair_ase_protocol as fair  # noqa: E402


def _atoms(cell) -> Atoms:
    return Atoms(
        "Cu2",
        positions=[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
        cell=cell,
        pbc=True,
    )


def test_fix_atoms_fixes_every_axis() -> None:
    atoms = _atoms([2.0, 3.0, 4.0])
    atoms.set_constraint(FixAtoms(indices=[1]))
    mask = fair._fixed_mask(atoms)
    assert mask[0].tolist() == [0, 0, 0]
    assert mask[1].tolist() == [1, 1, 1]


def test_fix_cartesian_uses_its_mask() -> None:
    atoms = _atoms([2.0, 3.0, 4.0])
    atoms.set_constraint(FixCartesian(0, mask=(False, True, False)))
    mask = fair._fixed_mask(atoms)
    assert mask[0].tolist() == [0, 1, 0]
    assert mask[1].tolist() == [0, 0, 0]


def test_fix_scaled_follows_an_axis_aligned_cell() -> None:
    atoms = _atoms([2.0, 3.0, 4.0])
    atoms.set_constraint(FixScaled(a=[0], mask=(True, False, True)))
    mask = fair._fixed_mask(atoms)
    assert mask[0].tolist() == [1, 0, 1]
    assert int(mask.sum()) == 2


def test_fix_scaled_rejects_a_tilted_cell_vector() -> None:
    atoms = _atoms([[1.0, 0.2, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]])
    atoms.set_constraint(FixScaled(a=[0], mask=(True, False, False)))
    with pytest.raises(ValueError, match="cartesian axis"):
        fair._fixed_mask(atoms)


def test_fix_bond_lengths_is_left_unset() -> None:
    atoms = _atoms([2.0, 3.0, 4.0])
    atoms.set_constraint(FixBondLengths([(0, 1)]))
    mask = fair._fixed_mask(atoms)
    assert mask.dtype == np.uint8
    assert int(mask.sum()) == 0
