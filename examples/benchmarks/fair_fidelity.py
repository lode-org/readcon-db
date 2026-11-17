"""Round-trip the paper 40×8 Cu frames through both fair stores.

The geometry is ``paper_con_text``: eight Cu atoms, forty frames, cell
10 Å. Each atom also carries an explicit mass, a fixed-flag mask, a
non-zero velocity, and an atom id that is a permutation of ``0..7``.
CON stores one mass per symbol, so that mass is the same on every Cu
atom and is not the elemental default.

``compare`` checks, with exact equality, symbols, positions, cell
lengths, cell angles, pbc, masses, fixed flags, velocities, and
frame_index on the stored CON frames, on ``read_arrays``, and on
``materialize_ase``. Atom ids are checked on the CON frames only.
ASE keeps no atom id.
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
FIELDS_PATH = Path(__file__).resolve().parent / "fair-fidelity" / "FIELDS.md"


def _fixed(index: int) -> tuple[bool, bool, bool]:
    if index % 2 == 0:
        return (True, False, True)
    return (False, False, False)


def _velocity(index: int) -> tuple[float, float, float]:
    return (0.01 * (index + 1), -0.02 * (index + 1), 0.125)


def build_frames() -> list:
    """Paper coordinates, plus the fields the fair comparison was not given."""
    frames = []
    for frame_index in range(N_FRAMES):
        atoms = []
        for index, atom_id in enumerate(ATOM_IDS):
            vx, vy, vz = _velocity(index)
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
                )
            )
        frame = readcon.ConFrame((10.0, 10.0, 10.0), (90.0, 90.0, 90.0), atoms)
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
        "ase_atom_id": ["atom_id" in row.toatoms().arrays for row in rows],
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
    lengths_rdb = all(
        _bits(frame.cell, src.cell) for frame, src in zip(con_frames, parsed)
    )
    angles_rdb = all(
        _bits(frame.angles, src.angles) for frame, src in zip(con_frames, parsed)
    )
    lengths_ase = all(
        _bits(_lengths_angles(matrix)[0], atoms.cell.lengths())
        for matrix, atoms in zip(got["cell"], written)
    )
    angles_ase = all(
        _bits(_lengths_angles(matrix)[1], atoms.cell.angles())
        for matrix, atoms in zip(got["cell"], written)
    )
    cell_ase = _bits(got["cell"], expect_ase_cell) and lengths_ase and angles_ase
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
    index_rdb = (
        all(int(frame.frame_index) == index for index, frame in enumerate(con_frames))
        and _bits(arrays["frame_index"], expect_index)
        and all(arrays["frame_index_set"])
    )
    index_ase = _bits(got["frame_index"], expect_index) and all(got["frame_index_set"])
    ids_rdb = all(
        [atom.atom_id for atom in frame.atoms] == list(ATOM_IDS) for frame in con_frames
    )
    float_rdb = pos_rdb and lengths_rdb and angles_rdb and mass_rdb and vel_rdb
    float_ase = pos_ase and cell_ase and mass_ase and vel_ase
    return [
        {"field": "symbols", "readcon": symbols_rdb, "ase": symbols_ase},
        {"field": "positions", "readcon": pos_rdb, "ase": pos_ase},
        {"field": "cell lengths", "readcon": lengths_rdb, "ase": lengths_ase},
        {"field": "cell angles", "readcon": angles_rdb, "ase": angles_ase},
        {"field": "pbc", "readcon": pbc_rdb, "ase": pbc_ase},
        {"field": "masses", "readcon": mass_rdb, "ase": mass_ase},
        {"field": "fixed flags", "readcon": fixed_rdb, "ase": fixed_ase},
        {"field": "velocities", "readcon": vel_rdb, "ase": vel_ase},
        {"field": "frame index", "readcon": index_rdb, "ase": index_ase},
        {"field": "atom ids", "readcon": ids_rdb, "ase": False},
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
        "Forty frames of eight Cu atoms, the paper cell and coordinates,",
        "an explicit mass of 62.5 on every atom, mixed fixed flags,",
        "non-zero velocities, frame indexes, and atom ids",
        "`(3, 0, 7, 1, 5, 2, 6, 4)`.",
        "Checked on the stored CON frames, on readcon-db `read_arrays`,",
        "and on `materialize_ase`. Atom ids are a readcon-db field.",
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
