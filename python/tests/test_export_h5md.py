"""H5MD 1.1 interchange from export_h5md (h5py)."""

from pathlib import Path

import h5py
import numpy as np
from readcon_db import ConCorpus, canonicalize_unit, unit_conversion_factor

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
        assert _as_str(f["particles/all/position/time"].attrs["unit"]) == "ps"
        assert _as_str(pos.attrs["unit"]) == "Angstrom"
        edges = f["particles/all/box/edges/value"]
        assert edges.shape == (n, 3, 3)
        assert _as_str(edges.attrs["unit"]) == "Angstrom"
        z = f["particles/all/species"]
        assert z.shape == (pos.shape[1],)
        assert np.issubdtype(z.dtype, np.integer)
        assert np.all(z[:] > 0)
        assert 29 in z[:]
        assert 1 in z[:]
        assert "force" not in f["particles/all"]
        assert f["particles/all/box/edges/step"].shape == (n,)
        assert f["particles/all/box"].attrs["dimension"] == 3
        _assert_fixed_ascii(f["h5md/author"].attrs, "name")
        _assert_fixed_ascii(f["h5md/creator"].attrs, "name")
        _assert_fixed_ascii(f["h5md/creator"].attrs, "version")
        _assert_fixed_ascii(f["particles/all/box"].attrs, "boundary")
        assert _as_str(f["particles/all/position/value"].attrs["unit"]) == "Angstrom"
        assert _as_str(f["particles/all/position/time"].attrs["unit"]) == "ps"
        assert _as_str(f["particles/all/box/edges/value"].attrs["unit"]) == "Angstrom"


def _as_str(x):
    if isinstance(x, (bytes, np.bytes_)):
        return x.decode("ascii", "replace").rstrip("\x00")
    return str(x)


def _assert_fixed_ascii(attrs, key):
    dt = np.asarray(attrs[key]).dtype
    info = h5py.check_string_dtype(dt)
    if info is not None:
        assert info.length is not None
        assert info.encoding in (None, "ascii", "utf-8")
        return
    assert dt.kind in ("S", "U") and dt.itemsize >= 1


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
        assert _as_str(force.attrs["unit"]) == "kJ mol-1 Angstrom-1"
        assert f["particles/all/force/time"].shape == (n,)
        assert _as_str(f["particles/all/force/time"].attrs["unit"]) == "ps"
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
        e_j = unit_conversion_factor("eV", "J")
        l_m = unit_conversion_factor("angstrom", "m")
        na = 6.02214076e23
        scale = (e_j / l_m) / ((1000.0 / na) / 1e-10)
        np.testing.assert_allclose(force[idx], native * scale)
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


def test_append_and_set_units_canonical(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    db.append_trajectory(
        1,
        str(FIXTURE / "tiny_cuh2.con"),
        units={"length": "A", "energy": "ev", "time": "femtosecond"},
    )
    raw = db.get_units(1, 0)
    assert raw is not None
    assert "angstrom" in raw
    assert "eV" in raw
    assert "fs" in raw
    pos_a = np.asarray(db.get_positions(1, 0))
    db.set_units(1, {"length": "nm", "energy": "hartree", "time": "ps"})
    raw2 = db.get_units(1, 0)
    assert "nm" in raw2
    pos_nm = np.asarray(db.get_positions(1, 0))
    np.testing.assert_allclose(pos_nm, pos_a * 0.1)
    assert canonicalize_unit("A") == "angstrom"
    out = tmp_path / "alias.h5"
    db.export_h5md(1, str(out))
    with h5py.File(out, "r") as f:
        exported = f["particles/all/position/value"][0]
        np.testing.assert_allclose(exported, pos_a, atol=1e-6)
        assert _as_str(f["particles/all/position/time"].attrs["unit"]) == "ps"


def test_unit_conversion_factor_length():
    assert abs(unit_conversion_factor("angstrom", "nm") - 0.1) < 1e-12
    assert abs(unit_conversion_factor("eV", "meV") - 1000.0) < 1e-6


def test_pack_frames_unpack_batch(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory(1, str(FIXTURE / "tiny_multi_cuh2.con"))
    blob = db.pack_frames([(1, i) for i in range(n)])
    frames = ConCorpus.unpack_batch(blob)
    assert len(frames) == n
    pos0 = db.get_positions(1, 0)
    np.testing.assert_allclose(np.asarray(frames[0]), np.asarray(pos0))


def test_export_h5md_mdanalysis_reader_with_velocity(tmp_path):
    from MDAnalysis.coordinates.H5MD import H5MDReader

    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory(1, str(FIXTURE / "tiny_cuh2.convel"))
    out = tmp_path / "mda_v.h5"
    db.export_h5md(1, str(out))
    reader = H5MDReader(str(out), convert_units=True)
    assert reader.n_frames == n
    ts = reader.ts
    assert ts.has_velocities
    assert abs(float(ts.velocities[0, 0]) - 1.234) < 1e-3


def test_export_h5md_mdanalysis_reader(tmp_path):
    from MDAnalysis.coordinates.H5MD import H5MDReader

    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory(1, str(FIXTURE / "tiny_multi_cuh2.con"))
    out = tmp_path / "mda.h5"
    db.export_h5md(1, str(out))
    with h5py.File(out, "r") as f:
        assert _as_str(f["particles/all/position/time"].attrs["unit"]) == "ps"
    reader = H5MDReader(str(out), convert_units=True)
    assert reader.n_frames == n
    assert reader.n_atoms >= 1


def test_export_h5md_mdanalysis_reader_with_forces(tmp_path):
    from MDAnalysis.coordinates.H5MD import H5MDReader

    db = ConCorpus(str(tmp_path / "corpus"))
    db.append_trajectory(1, str(FIXTURE / "tiny_cuh2.con"))
    n = db.extend_trajectory(
        1,
        str(FIXTURE / "tiny_cuh2_forces.con"),
        units={"length": "angstrom", "energy": "eV"},
    )
    out = tmp_path / "mda_f.h5"
    db.export_h5md(1, str(out))
    reader = H5MDReader(str(out), convert_units=True)
    assert reader.n_frames == n
    with h5py.File(out, "r") as f:
        assert "force" in f["particles/all"]
        assert _as_str(f["particles/all/force/value"].attrs["unit"]) == "kJ mol-1 Angstrom-1"


def test_export_h5md_append_nm_scales_positions(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    db.append_trajectory(
        1,
        str(FIXTURE / "tiny_cuh2.con"),
        units={"length": "nm", "energy": "eV"},
    )
    native = np.asarray(db.get_positions(1, 0))
    out = tmp_path / "nm.h5"
    db.export_h5md(1, str(out))
    with h5py.File(out, "r") as f:
        t = f["particles/all/position/time"][:]
        pos = f["particles/all/position/value"][0]
        np.testing.assert_allclose(pos, native * 10.0)
        assert t.shape[0] == 1


def test_export_h5md_writes_velocity(tmp_path):
    db = ConCorpus(str(tmp_path / "corpus"))
    db.append_trajectory(1, str(FIXTURE / "tiny_cuh2.convel"))
    out = tmp_path / "vel.h5"
    db.export_h5md(1, str(out))
    with h5py.File(out, "r") as f:
        v = f["particles/all/velocity/value"]
        assert v.ndim == 3
        assert v.shape[2] == 3
        assert np.any(v[:] != 0.0)
        assert _as_str(v.attrs["unit"]) == "Angstrom ps-1"
        assert abs(float(v[0, 0, 0]) - 1.234) < 1e-9


def test_ingest_directory_units(tmp_path):
    src = tmp_path / "cons"
    src.mkdir()
    (src / "a.con").write_text((FIXTURE / "tiny_cuh2.con").read_text())
    db = ConCorpus(str(tmp_path / "corpus"))
    rows = db.ingest_directory(str(src), units={"length": "A", "energy": "ev"})
    assert len(rows) == 1
    raw = db.get_units(1, 0)
    assert "angstrom" in raw
    assert "eV" in raw


def test_export_h5md_i_times_timestep(tmp_path):
    src = (FIXTURE / "tiny_multi_cuh2.con").read_text()
    lines = src.splitlines()
    meta = (
        '{"con_spec_version":3,"timestep":10.0,'
        '"units":{"length":"angstrom","energy":"eV","time":"fs"}}'
    )
    lines = [meta if ln.strip().startswith("{") else ln for ln in lines]
    con = tmp_path / "dt.con"
    con.write_text("\n".join(lines) + "\n")
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory(1, str(con))
    assert n >= 2
    out = tmp_path / "dt.h5"
    db.export_h5md(1, str(out))
    with h5py.File(out, "r") as f:
        t = f["particles/all/position/time"][:]
        assert abs(float(t[0]) - 0.0) < 1e-12
        assert abs(float(t[1]) - 0.01) < 1e-12


def test_export_h5md_writes_header_time(tmp_path):
    src = (FIXTURE / "tiny_cuh2.con").read_text()
    lines = src.splitlines()
    lines[1] = (
        '{"con_spec_version":3,"time":12.5,'
        '"units":{"length":"angstrom","energy":"eV","time":"fs"}}'
    )
    con = tmp_path / "timed.con"
    con.write_text("\n".join(lines) + "\n")
    db = ConCorpus(str(tmp_path / "corpus"))
    db.append_trajectory(1, str(con))
    out = tmp_path / "t.h5"
    db.export_h5md(1, str(out))
    with h5py.File(out, "r") as f:
        t = f["particles/all/position/time"][:]
        assert t.shape == (1,)
        assert abs(float(t[0]) - 0.0125) < 1e-12
        et = f["particles/all/box/edges/time"][:]
        assert abs(float(et[0]) - 0.0125) < 1e-12
        assert f["particles/all/box/edges/value"].shape[0] == 1
