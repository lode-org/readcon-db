"""Every stored field of a frame comes back with the same bits."""

from pathlib import Path

import numpy as np
import readcon
from readcon_db import ConCorpus

FIXTURE = Path(__file__).resolve().parents[2] / "resources" / "test"


def _f64(value):
    if value is None:
        return None
    return np.float64(value).tobytes()


def _arr(value):
    if value is None:
        return None
    return np.asarray(value, dtype=np.float64).tobytes()


def snapshot(frame):
    meta = frame.metadata or {}
    return {
        "symbols": [atom.symbol for atom in frame.atoms],
        "positions": _arr(frame.coords_array()),
        "masses": _arr([atom.mass for atom in frame.atoms]),
        "atom_id": [atom.atom_id for atom in frame.atoms],
        "fixed": [tuple(bool(flag) for flag in atom.fixed) for atom in frame.atoms],
        "forces": _arr(frame.forces_array()) if frame.has_forces else None,
        "velocities": _arr(frame.velocities_array()) if frame.has_velocities else None,
        "energy": _f64(frame.energy),
        "cell": _arr(frame.cell),
        "angles": _arr(frame.angles),
        "frame_index": None if frame.frame_index is None else int(frame.frame_index),
        "time": _f64(frame.time),
        "charge": _f64(meta.get("charge")),
        "magmom": _f64(meta.get("magmom")),
        "pbc": None if "pbc" not in meta else tuple(bool(flag) for flag in meta["pbc"]),
    }


def test_fixture_fields_round_trip(tmp_path):
    text = (FIXTURE / "tiny_cuh2_vel_forces.con").read_text()
    original = [snapshot(frame) for frame in readcon.read_con_string(text)]
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory_str(1, text, source="tiny_cuh2_vel_forces.con")
    assert n == len(original)
    stored = []
    for idx in range(n):
        blob = db.get_frame_text(1, idx)
        assert list(db.xxh3_128(blob.encode())) == list(db.frame_hash(1, idx))
        got = readcon.read_con_string(blob)
        assert len(got) == 1
        stored.append(snapshot(got[0]))
    assert stored == original
    for field in original[0]:
        assert stored[0][field] == original[0][field], field


def test_shared_fields_round_trip_both_stores(tmp_path):
    import sys

    bench = Path(__file__).resolve().parents[2] / "examples" / "benchmarks"
    sys.path.insert(0, str(bench))
    import fair_ase_protocol as fair

    text, atoms, records = fair.build_records(2, 3)
    fair.stamp_frame_index(atoms)
    payload = {
        "text": text,
        "atoms": atoms,
        "records": records,
        "n_frames": 2,
        "geometry_only": False,
    }
    fair.insert_fair("readcon-db", payload, tmp_path / "rc")
    fair.insert_fair("ase.db", payload, tmp_path / "ase")


def test_default_select_keeps_every_frame_and_a_narrow_window_filters(tmp_path):
    text = (FIXTURE / "tiny_cuh2.con").read_text()
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory_str(1, text, source="tiny_cuh2.con")
    assert len(db.select()) == n
    assert len(db.select(natoms_min=0, natoms_max=2**32 - 1)) == n
    assert db.select(natoms_min=1, natoms_max=1) == []
    assert len(db.select(natoms_min=4, natoms_max=4)) == n
