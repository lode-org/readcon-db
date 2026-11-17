"""Round-trip the paper 40×8 Cu frames through both fair stores.

The geometry is ``paper_con_text``: eight Cu atoms, forty frames, cell
10 Å. Each atom also carries an explicit mass, a fixed-flag mask, a
non-zero velocity, and an atom id that is a permutation of ``0..7``.
CON stores one mass per symbol, so that mass is the same on every Cu
atom and is not the elemental default.
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


def compare(stored: dict) -> list[dict]:
    """One row per field. ``kept`` is whether the store returned the written value."""
    parsed = stored["parsed"]
    written = stored["atoms"]
    rdb = ConCorpus(str(stored["corpus"]), readonly=True)
    try:
        con_frames = [
            readcon.read_con_string(rdb.get_frame_text(1, index))[0]
            for index in range(N_FRAMES)
        ]
    finally:
        fair.close_lmdb(rdb)
    rows = fair.read_ase_rows(stored["ase"], N_FRAMES)
    got = fair.materialize_ase(rows)
    back = [row.toatoms() for row in rows]

    symbols_rdb = all(
        [atom.symbol for atom in frame.atoms] == ["Cu"] * N_ATOMS
        for frame in con_frames
    )
    symbols_ase = all(
        atoms.get_chemical_symbols() == ["Cu"] * N_ATOMS for atoms in back
    )
    pos_rdb = all(
        _bits(_positions(frame), _positions(src))
        for frame, src in zip(con_frames, parsed)
    )
    pos_ase = all(
        _bits(atoms.positions, src.positions) for atoms, src in zip(back, written)
    )
    cell_rdb = all(
        _bits(frame.cell, src.cell) for frame, src in zip(con_frames, parsed)
    )
    cell_ase = all(
        _bits(atoms.cell.array, src.cell.array) for atoms, src in zip(back, written)
    )
    pbc_rdb = all(tuple(frame.metadata["pbc"]) == PBC for frame in con_frames)
    pbc_ase = all(tuple(bool(flag) for flag in atoms.pbc) == PBC for atoms in back)
    mass_rdb = all(
        _bits(_masses(frame), _masses(src)) for frame, src in zip(con_frames, parsed)
    )
    mass_ase = all(
        _bits(atoms.get_masses(), src.get_masses()) for atoms, src in zip(back, written)
    )
    fixed_rdb = all(
        _fixed_rows(frame).tobytes() == _fixed_rows(src).tobytes()
        for frame, src in zip(con_frames, parsed)
    )
    fixed_ase = all(
        fair._fixed_mask(atoms).tobytes() == fair._fixed_mask(src).tobytes()
        for atoms, src in zip(back, written)
    )
    # The fair reader must report the mask the database stored.
    fixed_reader = (
        got["constraints"].tobytes()
        == np.stack([fair._fixed_mask(atoms) for atoms in back]).tobytes()
    )
    vel_rdb = all(
        _bits(_velocities(frame), _velocities(src))
        for frame, src in zip(con_frames, parsed)
    )
    vel_ase = all(
        _bits(atoms.get_velocities(), src.get_velocities())
        for atoms, src in zip(back, written)
    )
    ids_rdb = all(
        [atom.atom_id for atom in frame.atoms] == list(ATOM_IDS) for frame in con_frames
    )
    ids_ase = all("atom_id" in atoms.arrays for atoms in back)
    float_rdb = pos_rdb and cell_rdb and mass_rdb and vel_rdb
    float_ase = pos_ase and cell_ase and mass_ase and vel_ase
    rows_out = [
        {"field": "symbols", "readcon": symbols_rdb, "ase": symbols_ase},
        {"field": "positions", "readcon": pos_rdb, "ase": pos_ase},
        {"field": "cell", "readcon": cell_rdb, "ase": cell_ase},
        {"field": "pbc", "readcon": pbc_rdb, "ase": pbc_ase},
        {"field": "masses", "readcon": mass_rdb, "ase": mass_ase},
        {
            "field": "fixed flags",
            "readcon": fixed_rdb,
            "ase": fixed_ase and fixed_reader,
        },
        {"field": "velocities", "readcon": vel_rdb, "ase": vel_ase},
        {"field": "atom ids", "readcon": ids_rdb, "ase": ids_ase},
        {
            "field": "float precision",
            "readcon": float_rdb,
            "ase": float_ase,
            "readcon_text": "bit-exact" if float_rdb else "not bit-exact",
            "ase_text": "bit-exact" if float_ase else "not bit-exact",
        },
    ]
    return rows_out


def render_markdown(rows: list[dict]) -> str:
    lines = [
        "# Fair store fields",
        "",
        "Generated by `examples/benchmarks/fair-fidelity/write_fields.py`.",
        "Forty frames of eight Cu atoms, the paper cell and coordinates,",
        "an explicit mass of 62.5 on every atom, mixed fixed flags,",
        "non-zero velocities, and atom ids `(3, 0, 7, 1, 5, 2, 6, 4)`.",
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
