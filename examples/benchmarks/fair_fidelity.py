"""Round-trip the paper 40×8 Cu frames through both fair stores.

Eight Cu atoms, forty frames, on a triclinic cell with lengths
``(10, 11.5, 12.25)`` and angles ``(80, 75, 70)``. Each atom also
carries an explicit mass, a fixed-flag mask, a non-zero velocity, and
an atom id that is a permutation of ``0..7``. CON stores one mass per
symbol, so that mass is the same on every Cu atom and is not the
elemental default.

``compare`` checks exact equality of symbols, positions, cell lengths,
cell angles, the ``read_arrays`` and ``materialize_ase`` cell matrices
(byte-for-byte, not only lengths and angles), pbc, masses, fixed flags,
velocities, forces, frame index, and atomic numbers. Even frames carry
non-zero forces and odd frames carry zeros. ``read_arrays`` energy is 0
with ``energy_set`` false, ``velocities_set`` and ``pbc_set`` are true,
and ``n_frames`` and ``n_atoms`` match the fixture. ``materialize_ase``
energy is the same unset pair. CON units metadata is the written units
dict. Atom ids are exact on the CON frames and are whatever
``views()["ase_atom_id"]`` read back. A forces path that returns nothing
is recorded as not kept.
"""

from __future__ import annotations

import json
from pathlib import Path

import numpy as np

import fair_ase_protocol as fair
import readcon
from readcon_db import ConCorpus

N_FRAMES = 40
N_ATOMS = 8
MASS = np.float64(62.5)
ATOM_IDS = (3, 0, 7, 1, 5, 2, 6, 4)
PBC = (True, True, False)
# Distinct lengths and angles, none of them 90. A valid CON cell.
CELL_LENGTHS = (10.0, 11.5, 12.25)
CELL_ANGLES = (80.0, 75.0, 70.0)
UNITS = {
    "energy": "eV",
    "length": "angstrom",
    "mass": "amu",
    "time": "(amu*angstrom^2/eV)^0.5",
}
FIELDS_PATH = Path(__file__).resolve().parent / "fair-fidelity" / "FIELDS.md"


def _fixed(index: int) -> tuple[bool, bool, bool]:
    if index % 2 == 0:
        return (True, False, True)
    return (False, False, False)


def _velocity(index: int) -> tuple[float, float, float]:
    return (0.01 * (index + 1), -0.02 * (index + 1), 0.125)


def _force(frame_index: int, index: int) -> tuple[float, float, float]:
    """Non-zero on even frames. Odd frames still carry an explicit zero."""
    if frame_index % 2 == 1:
        return (0.0, 0.0, 0.0)
    return (0.01 * (index + 1), -0.02 * (index + 1), 0.125)


def build_frames() -> list:
    """Paper coordinates, plus the fields the fair comparison was not given."""
    frames = []
    for frame_index in range(N_FRAMES):
        atoms = []
        for index, atom_id in enumerate(ATOM_IDS):
            vx, vy, vz = _velocity(index)
            fx, fy, fz = _force(frame_index, index)
            atoms.append(
                readcon.Atom(
                    "Cu",
                    float(index) + frame_index * 0.01,
                    0.1,
                    0.2,
                    fixed=_fixed(index),
                    atom_id=atom_id,
                    mass=float(MASS),
                    vx=vx,
                    vy=vy,
                    vz=vz,
                    fx=fx,
                    fy=fy,
                    fz=fz,
                )
            )
        frame = readcon.ConFrame(CELL_LENGTHS, CELL_ANGLES, atoms)
        frame.set_metadata_json(json.dumps({"pbc": list(PBC)}))
        frame.set_frame_index(frame_index)
        frames.append(frame)
    return frames


def _positions(frame) -> np.ndarray:
    return np.array(
        [[atom.x, atom.y, atom.z] for atom in frame.atoms], dtype=np.float64
    )


def _velocities(frame) -> np.ndarray:
    return np.array(
        [[atom.vx, atom.vy, atom.vz] for atom in frame.atoms], dtype=np.float64
    )


def _forces(frame) -> np.ndarray:
    return np.array(frame.forces_array(), dtype=np.float64)


def _masses(frame) -> np.ndarray:
    return np.array([atom.mass for atom in frame.atoms], dtype=np.float64)


def _fixed_rows(frame) -> np.ndarray:
    return np.array(
        [[int(flag) for flag in atom.fixed] for atom in frame.atoms], dtype=np.uint8
    )


def _bits(left, right) -> bool:
    return (
        np.asarray(left, dtype=np.float64).tobytes()
        == np.asarray(right, dtype=np.float64).tobytes()
    )


def write_both(tmp: Path) -> dict:
    """Insert one trajectory the way the fair protocol inserts paper frames."""
    text = readcon.write_con_string(build_frames())
    parsed = readcon.read_con_string(text)
    corpus = tmp / "corpus"
    db = ConCorpus(str(corpus))
    n = db.append_trajectory_str(1, text, source="paper40x8.con")
    if int(n) != N_FRAMES:
        raise AssertionError(n)
    fair.close_lmdb(db)

    atoms_list = []
    for index, frame in enumerate(parsed):
        atoms = frame.to_ase()
        atoms.info["frame_index"] = index
        atoms_list.append(atoms)
    ase_path = tmp / "atoms.db"
    ase = fair.connect_fair(ase_path)[0]
    ase.connection = ase._connect()
    ase.change_count = 0
    fair.reset_commits(ase)
    for atoms in atoms_list:
        ase.write(atoms, frame_index=int(atoms.info["frame_index"]))
    ase.connection.commit()
    fair.assert_one_commit(ase)
    return {
        "text": text,
        "parsed": parsed,
        "corpus": corpus,
        "ase": ase,
        "atoms": atoms_list,
    }


def _yes(kept: bool) -> str:
    return "yes" if kept else "no"


def views(stored: dict) -> dict:
    """Stored CON frames, cooked arrays, and the ASE materializer output."""
    parsed = stored["parsed"]
    rdb = ConCorpus(str(stored["corpus"]), readonly=True)
    try:
        con_frames = [
            readcon.read_con_string(rdb.get_frame_text(1, index))[0]
            for index in range(N_FRAMES)
        ]
        arrays = rdb.read_arrays(1)
    finally:
        fair.close_lmdb(rdb)
    rows = fair.read_ase_rows(stored["ase"], N_FRAMES)
    return {
        "parsed": parsed,
        "written": stored["atoms"],
        "con": con_frames,
        "arrays": arrays,
        "got": fair.materialize_ase(rows),
        "ase_atom_id": [
            None
            if "atom_id" not in row.toatoms().arrays
            else [int(value) for value in row.toatoms().arrays["atom_id"]]
            for row in rows
        ],
    }


def _u8(left, right) -> bool:
    return (
        np.asarray(left, dtype=np.uint8).tobytes()
        == np.asarray(right, dtype=np.uint8).tobytes()
    )


def _lengths_angles(matrix) -> tuple[np.ndarray, np.ndarray]:
    from ase.cell import Cell

    cell = Cell(np.asarray(matrix, dtype=np.float64))
    return (
        np.asarray(cell.lengths(), dtype=np.float64),
        np.asarray(cell.angles(), dtype=np.float64),
    )


def compare(stored: dict) -> list[dict]:
    """One row per asserted field. Equality is byte-for-byte on the arrays."""
    seen = views(stored)
    parsed = seen["parsed"]
    written = seen["written"]
    con_frames = seen["con"]
    arrays = seen["arrays"]
    got = seen["got"]
    symbols = ["Cu"] * N_ATOMS
    expect_pos = np.stack([_positions(frame) for frame in parsed])
    expect_vel = np.stack([_velocities(frame) for frame in parsed])
    expect_forces = np.stack([_forces(frame) for frame in parsed])
    expect_mass = np.stack([_masses(frame) for frame in parsed])
    expect_fixed = np.stack([_fixed_rows(frame) for frame in parsed])
    expect_index = np.arange(N_FRAMES, dtype=np.float64)
    expect_ase_pos = np.stack([atoms.positions for atoms in written])
    expect_ase_vel = np.stack([atoms.get_velocities() for atoms in written])
    expect_ase_mass = np.stack([atoms.get_masses() for atoms in written])
    expect_ase_cell = np.stack([atoms.cell.array for atoms in written])
    expect_ase_fixed = np.stack([fair._fixed_mask(atoms) for atoms in written])

    symbols_rdb = all(
        [atom.symbol for atom in frame.atoms] == symbols for frame in con_frames
    ) and all(
        [run[0] for run in frame] == symbols[:1] and frame[0][1] == N_ATOMS
        for frame in arrays["symbol_runs"]
    )
    symbols_ase = all(
        [run[0] for run in frame] == symbols[:1] and frame[0][1] == N_ATOMS
        for frame in got["symbol_runs"]
    )
    pos_rdb = _bits(expect_pos, [_positions(frame) for frame in con_frames]) and _bits(
        arrays["positions"], expect_pos
    )
    pos_ase = _bits(got["positions"], expect_ase_pos)
    array_lengths = np.stack([_lengths_angles(matrix)[0] for matrix in arrays["cell"]])
    array_angles = np.stack([_lengths_angles(matrix)[1] for matrix in arrays["cell"]])
    written_lengths = np.stack([atoms.cell.lengths() for atoms in written])
    written_angles = np.stack([atoms.cell.angles() for atoms in written])
    matrix_rdb = _bits(arrays["cell"], expect_ase_cell)
    lengths_rdb = (
        all(_bits(frame.cell, src.cell) for frame, src in zip(con_frames, parsed))
        and matrix_rdb
        and _bits(array_lengths, written_lengths)
    )
    angles_rdb = (
        all(_bits(frame.angles, src.angles) for frame, src in zip(con_frames, parsed))
        and matrix_rdb
        and _bits(array_angles, written_angles)
    )
    got_lengths = np.stack([_lengths_angles(matrix)[0] for matrix in got["cell"]])
    got_angles = np.stack([_lengths_angles(matrix)[1] for matrix in got["cell"]])
    matrix_ase = _bits(got["cell"], expect_ase_cell)
    lengths_ase = matrix_ase and _bits(got_lengths, written_lengths)
    angles_ase = matrix_ase and _bits(got_angles, written_angles)
    pbc_rdb = all(
        tuple(bool(flag) for flag in frame.metadata["pbc"]) == PBC
        for frame in con_frames
    ) and all(tuple(bool(flag) for flag in flags) == PBC for flags in arrays["pbc"])
    pbc_ase = all(tuple(bool(flag) for flag in flags) == PBC for flags in got["pbc"])
    mass_rdb = _bits([_masses(frame) for frame in con_frames], expect_mass) and _bits(
        arrays["masses"], expect_mass
    )
    mass_ase = _bits(got["masses"], expect_ase_mass)
    fixed_rdb = _u8([_fixed_rows(frame) for frame in con_frames], expect_fixed) and _u8(
        arrays["constraints"], expect_fixed
    )
    fixed_ase = _u8(got["constraints"], expect_ase_fixed)
    vel_rdb = _bits(
        [frame.velocities_array() for frame in con_frames], expect_vel
    ) and _bits(arrays["velocities"], expect_vel)
    vel_ase = _bits(got["velocities"], expect_ase_vel)
    forces_rdb = (
        arrays["forces"] is not None
        and all(frame.has_forces for frame in con_frames)
        and _bits([_forces(frame) for frame in con_frames], expect_forces)
        and _bits(arrays["forces"], expect_forces)
    )
    # ``get_forces`` zeros fixed axes. The stored row keeps the written forces.
    forces_ase = got["forces"] is not None and _bits(got["forces"], expect_forces)
    index_rdb = (
        all(int(frame.frame_index) == index for index, frame in enumerate(con_frames))
        and _bits(arrays["frame_index"], expect_index)
        and all(arrays["frame_index_set"])
    )
    index_ase = _bits(got["frame_index"], expect_index) and all(got["frame_index_set"])
    ids_rdb = all(
        [atom.atom_id for atom in frame.atoms] == list(ATOM_IDS) for frame in con_frames
    )
    ids_ase = seen["ase_atom_id"] == [list(ATOM_IDS)] * N_FRAMES
    numbers = np.full((N_FRAMES, N_ATOMS), 29, dtype=np.uint32)
    numbers_rdb = (
        np.asarray(arrays["numbers"], dtype=np.uint32).tobytes() == numbers.tobytes()
    )
    numbers_ase = (
        np.asarray(got["numbers"], dtype=np.uint32).tobytes() == numbers.tobytes()
    )
    energy_rdb = _bits(arrays["energy"], np.zeros(N_FRAMES)) and not any(
        arrays["energy_set"]
    )
    energy_ase = _bits(got["energy"], np.zeros(N_FRAMES)) and not any(got["energy_set"])
    velocities_set = arrays["velocities_set"] is True
    pbc_set = all(arrays["pbc_set"])
    if int(arrays["n_frames"]) != N_FRAMES or int(arrays["n_atoms"]) != N_ATOMS:
        raise AssertionError((arrays["n_frames"], arrays["n_atoms"]))
    units_rdb = all(frame.metadata.get("units") == UNITS for frame in con_frames)
    units_ase = "units" in got
    float_rdb = (
        pos_rdb
        and lengths_rdb
        and angles_rdb
        and mass_rdb
        and vel_rdb
        and forces_rdb
        and energy_rdb
    )
    float_ase = (
        pos_ase
        and lengths_ase
        and angles_ase
        and mass_ase
        and vel_ase
        and forces_ase
        and energy_ase
    )
    return [
        {"field": "symbols", "readcon": symbols_rdb, "ase": symbols_ase},
        {"field": "positions", "readcon": pos_rdb, "ase": pos_ase},
        {"field": "cell lengths", "readcon": lengths_rdb, "ase": lengths_ase},
        {"field": "cell angles", "readcon": angles_rdb, "ase": angles_ase},
        {"field": "pbc", "readcon": pbc_rdb, "ase": pbc_ase},
        {"field": "masses", "readcon": mass_rdb, "ase": mass_ase},
        {"field": "fixed flags", "readcon": fixed_rdb, "ase": fixed_ase},
        {"field": "velocities", "readcon": vel_rdb, "ase": vel_ase},
        {"field": "forces", "readcon": forces_rdb, "ase": forces_ase},
        {"field": "frame index", "readcon": index_rdb, "ase": index_ase},
        {"field": "atom ids", "readcon": ids_rdb, "ase": ids_ase},
        {"field": "atomic numbers", "readcon": numbers_rdb, "ase": numbers_ase},
        {"field": "energy", "readcon": energy_rdb, "ase": energy_ase},
        {
            "field": "velocities_set",
            "readcon": velocities_set,
            "ase": "velocities_set" in got,
        },
        {"field": "pbc_set", "readcon": pbc_set, "ase": "pbc_set" in got},
        {"field": "units", "readcon": units_rdb, "ase": units_ase},
        {
            "field": "float precision",
            "readcon": float_rdb,
            "ase": float_ase,
            "readcon_text": "bit-exact" if float_rdb else "not bit-exact",
            "ase_text": "bit-exact" if float_ase else "not bit-exact",
        },
    ]


def render_markdown(rows: list[dict]) -> str:
    lines = [
        "# Fair store fields",
        "",
        "Generated by `examples/benchmarks/fair-fidelity/write_fields.py`.",
        "Forty frames of eight Cu atoms on a triclinic cell, lengths",
        "`(10, 11.5, 12.25)` and angles `(80, 75, 70)`, with an explicit",
        "mass of 62.5, mixed fixed flags, non-zero velocities, non-zero",
        "forces on even frames, frame indexes, and atom ids",
        "`(3, 0, 7, 1, 5, 2, 6, 4)`.",
        "Checked on the stored CON frames, on readcon-db `read_arrays`,",
        "and on `materialize_ase`. The cell matrix is compared as stored",
        "bytes. Atomic numbers, forces, the unset energy, and the",
        "read_arrays set-flags are included. Atom ids and CON units are",
        "reported from the values read back.",
        "CON keeps one mass per symbol, so the mass does not vary inside the frame.",
        "",
        "| field | readcon-db kept? | ASE kept? |",
        "| --- | --- | --- |",
    ]
    for row in rows:
        if row["field"] == "float precision":
            left = row["readcon_text"]
            right = row["ase_text"]
        else:
            left = _yes(row["readcon"])
            right = _yes(row["ase"])
        lines.append(f"| {row['field']} | {left} | {right} |")
    lines.append("")
    return "\n".join(lines)


def write_fields(path: Path = FIELDS_PATH) -> str:
    import tempfile

    with tempfile.TemporaryDirectory() as tmp:
        stored = write_both(Path(tmp))
        try:
            text = render_markdown(compare(stored))
        finally:
            fair.close_ase(stored["ase"])
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    return text
