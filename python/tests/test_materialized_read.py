"""A fair read materializes the same arrays, from the cooked blob."""

import sys
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "examples" / "benchmarks"))
import fair_ase_protocol as fair


def test_both_tools_materialize_the_same_arrays(tmp_path):
    text, atoms, records = fair.build_records(1, 4)
    fair.stamp_frame_index(atoms)
    payload = {
        "text": text,
        "atoms": atoms,
        "records": records,
        "n_frames": 1,
        "geometry_only": False,
    }
    for kind in ("readcon-db", "ase.db"):
        dest = fair.fresh_dir(tmp_path, kind)
        fair.insert_fair(kind, payload, dest)
        if kind == "readcon-db":
            db = fair.open_readcon(dest / "corpus")
        else:
            db = fair.open_ase_reader(dest / "atoms.db")
        got = fair.fetch_materialized(kind, db, "full_read", 0, 1)
        fair.check_materialized(got, records)
        assert got["positions"].dtype == np.float64
        assert got["masses"].dtype == np.float64
        assert got["numbers"].dtype == np.uint32
        assert got["constraints"].dtype == np.uint8
        assert got["velocities"].dtype == np.float64
        assert got["cell"].shape == (1, 3, 3)
        fair.close_reader(kind, db)


def test_row_peak_rss_is_not_the_parent_high_water(tmp_path):
    text, atoms, records = fair.build_records(1, 4)
    fair.stamp_frame_index(atoms)
    payload = {
        "text": text,
        "atoms": atoms,
        "records": records,
        "n_frames": 1,
        "geometry_only": False,
    }
    dest = fair.fresh_dir(tmp_path, "rc")
    fair.insert_fair("readcon-db", payload, dest)
    held = bytearray(200 * 1024 * 1024)
    used = records
    probe = {
        "positions": np.stack([rec["positions"] for rec in used]),
        "masses": np.stack([rec["masses"] for rec in used]),
        "cell": np.stack([rec["cell"] for rec in used]),
        "velocities": np.zeros((1, 4, 3), dtype=np.float64),
        "constraints": np.zeros((1, 4, 3), dtype=np.uint8),
        "numbers": np.full((1, 4), 29, dtype=np.uint32),
        "energy": np.array([rec["energy"] for rec in used], dtype=np.float64),
        "energy_set": [True],
        "frame_index": np.array([0.0], dtype=np.float64),
        "frame_index_set": [True],
        "pbc": [tuple(rec["pbc"]) for rec in used],
        "symbol_runs": [
            [[sym, count] for sym, count in rec["symbol_runs"]] for rec in used
        ],
        "forces": np.stack([rec["forces"] for rec in used]),
    }
    row = fair.spawn_row(
        {
            "what": "read",
            "kind": "readcon-db",
            "op": "full_read",
            "cache": "warm",
            "name": "peak",
            "dest": str(dest),
            "n_frames": 1,
            "n_atoms": 4,
            "digests": fair.digests_of(probe),
        }
    )
    assert row["peak_rss_bytes"] < 150 * 1024 * 1024
    assert len(held) == 200 * 1024 * 1024
