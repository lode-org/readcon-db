"""H5MD 1.1 interchange from export_h5md (h5py)."""

from pathlib import Path

import h5py
import numpy as np
from readcon_db import ConCorpus

FIXTURE = Path(__file__).resolve().parents[2] / "resources" / "test"


def test_export_h5md_tn3_and_units(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory(1, str(FIXTURE / "tiny_multi_cuh2.con"))
    assert n >= 2
    out = tmp_path / "traj.h5"
    written = db.export_h5md(1, str(out))
    assert written == n
    with h5py.File(out, "r") as f:
        assert tuple(f["h5md"].attrs["version"]) == (1, 1)
        assert f["h5md/author"].attrs["name"] == "readcon-db"
        assert f["h5md/creator"].attrs["name"] == "readcon-db"
        assert "version" in f["h5md/creator"].attrs
        pos = f["particles/all/position/value"]
        assert pos.shape[0] == n
        assert pos.shape[2] == 3
        assert pos.ndim == 3
        assert "step" in f["particles/all/position"]
        assert f["particles/all/position/step"].shape == (n,)
        assert f["particles/all/position/time"].shape == (n,)
        assert f["particles/all/position/time"].attrs["unit"] == "ps"
        assert pos.attrs["unit"] == "Angstrom"
        edges = f["particles/all/box/edges/value"]
        assert edges.shape == (n, 3, 3)
        assert edges.attrs["unit"] == "Angstrom"
        z = f["particles/all/species"]
        assert z.shape == (pos.shape[1],)
        assert np.issubdtype(z.dtype, np.integer)
        assert np.all(z[:] > 0)
        assert "force" not in f["particles/all"]


def test_export_h5md_mixed_forces_zero_pad(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    db.append_trajectory(1, str(FIXTURE / "tiny_cuh2.con"))
    n = db.extend_trajectory(1, str(FIXTURE / "tiny_cuh2_forces.con"))
    out = tmp_path / "mixed.h5"
    db.export_h5md(1, str(out))
    with h5py.File(out, "r") as f:
        force = f["particles/all/force/value"]
        assert force.shape[0] == n
        assert force.shape[2] == 3
        assert force.attrs["unit"] == "kJ mol-1 Angstrom-1"
        assert f["particles/all/force/time"].shape == (n,)
        assert f["particles/all/force/time"].attrs["unit"] == "ps"
        first = force[0]
        rest = force[1:]
        assert np.all(first == 0.0)
        assert np.any(rest != 0.0)


def test_export_h5md_con_fallback_matches_rcso(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory(1, str(FIXTURE / "tiny_multi_cuh2.con"))
    assert n >= 2
    assert not db.has_valid_cooked_soa(1, 0)
    out_con = tmp_path / "from_con.h5"
    db.export_h5md(1, str(out_con))
    db.recook_all()
    assert db.has_valid_cooked_soa(1, 0)
    out_rcso = tmp_path / "from_rcso.h5"
    db.export_h5md(1, str(out_rcso))
    with h5py.File(out_con, "r") as a, h5py.File(out_rcso, "r") as b:
        np.testing.assert_allclose(
            a["particles/all/position/value"][:],
            b["particles/all/position/value"][:],
        )
        np.testing.assert_allclose(
            a["particles/all/box/edges/value"][:],
            b["particles/all/box/edges/value"][:],
        )
        assert a["particles/all/position/value"].shape[0] == n
        assert a["particles/all/position/value"].ndim == 3
