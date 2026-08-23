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
        assert _as_str(f["h5md/author"].attrs["name"]) == "readcon-db"
        assert _as_str(f["h5md/creator"].attrs["name"]) == "readcon-db"
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
        assert f["particles/all/box/edges/step"].shape == (n,)
        assert f["particles/all/box"].attrs["dimension"] == 3
        _assert_fixed_ascii(f["h5md/author"].attrs, "name")
        _assert_fixed_ascii(f["h5md/creator"].attrs, "name")
        _assert_fixed_ascii(f["h5md/creator"].attrs, "version")
        _assert_fixed_ascii(f["particles/all/box"].attrs, "boundary")


def _as_str(x):
    if isinstance(x, (bytes, np.bytes_)):
        return x.decode("ascii", "replace").rstrip("\x00")
    return str(x)


def _assert_fixed_ascii(attrs, key):
    dt = attrs[key].dtype
    info = h5py.check_string_dtype(dt)
    assert info is not None
    assert info.length is not None
    assert not info.encoding or info.encoding in ("ascii", "utf-8")


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
        native = None
        idx = None
        for i in range(n):
            fo = db.get_forces(1, i)
            if fo is not None:
                native = np.asarray(fo, dtype=np.float64)
                idx = i
                break
        assert native is not None
        np.testing.assert_allclose(force[idx], native * 96.48533212331002)
        bnd = f["particles/all/box"].attrs["boundary"]
        assert len(bnd) == 3


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


def test_pack_frames_unpack_batch(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory(1, str(FIXTURE / "tiny_multi_cuh2.con"))
    blob = db.pack_frames([(1, i) for i in range(n)])
    frames = ConCorpus.unpack_batch(blob)
    assert len(frames) == n
    pos0 = db.get_positions(1, 0)
    np.testing.assert_allclose(np.asarray(frames[0]), np.asarray(pos0))


def test_export_h5md_mdanalysis_reader(tmp_path):
    from MDAnalysis.coordinates.H5MD import H5MDReader

    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory(1, str(FIXTURE / "tiny_multi_cuh2.con"))
    out = tmp_path / "mda.h5"
    db.export_h5md(1, str(out))
    reader = H5MDReader(str(out), convert_units=True)
    assert reader.n_frames == n
    assert reader.n_atoms >= 1
