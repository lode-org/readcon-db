"""In-memory CON text and frame ingest (no temp file)."""

import json
from pathlib import Path

from readcon_db import ConCorpus, unit_conversion_factor

FIXTURE = Path(__file__).resolve().parents[2] / "resources" / "test"
NATURAL_TIME = "(amu*angstrom^2/eV)^0.5"


def test_append_trajectory_str(tmp_path):
    text = (FIXTURE / "tiny_cuh2.con").read_text()
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory_str(1, text, source="tiny_cuh2.con")
    assert n >= 1
    keys = db.select(traj_id=1)
    assert len(keys) == n


def test_append_trajectory_frames(tmp_path):
    import readcon

    frames = readcon.read_con(str(FIXTURE / "tiny_cuh2.con"))
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory_frames(1, frames, source="tiny_cuh2")
    assert n == len(frames)
    keys = db.select(traj_id=1)
    assert len(keys) == n


def _written(tmp_path, frames):
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory_frames(1, frames, source="time-units")
    assert n == len(frames)
    text = db.get_frame_text(1, 0)
    header = json.loads(text.splitlines()[1])
    return header, text, db.get_frame(1, 0)


def _component_rows(text, title):
    rows = []
    lines = text.splitlines()
    i = 0
    while i < len(lines):
        if lines[i].startswith(title):
            i += 1
            while i < len(lines):
                parts = lines[i].split()
                if len(parts) < 3:
                    break
                try:
                    row = (float(parts[0]), float(parts[1]), float(parts[2]))
                except ValueError:
                    break
                rows.append(row)
                i += 1
            continue
        i += 1
    return rows


def _velocities(frame):
    return [(atom.vx, atom.vy, atom.vz) for atom in frame.atoms]


def test_pre31_velocities_with_fs_clock(tmp_path):
    import readcon

    frame = readcon.read_con(str(FIXTURE / "tiny_cuh2.convel"))[0]
    assert frame.spec_version < 3
    frame.set_time(12.5)
    frame.set_timestep(1.25)
    original = _velocities(frame)
    scale = unit_conversion_factor("fs", NATURAL_TIME)
    header, text, back = _written(tmp_path, [frame])
    assert header["con_spec_version"] == 3
    assert header["con_spec_minor"] == 1
    assert header["units"]["time"] == "fs"
    assert header["time"] == 12.5
    assert header["timestep"] == 1.25
    assert "velocities" in header["sections"]
    expected = [(vx * scale, vy * scale, vz * scale) for vx, vy, vz in original]
    assert _component_rows(text, "Velocities of Component") == expected
    assert _velocities(back) == expected
    assert back.time == 12.5
    assert back.timestep == 1.25


def test_pre31_clock_without_velocities(tmp_path):
    import readcon

    frame = readcon.read_con(str(FIXTURE / "tiny_cuh2.con"))[0]
    assert frame.spec_version < 3
    assert not frame.has_velocities
    frame.set_time(12.5)
    frame.set_timestep(1.25)
    header, text, back = _written(tmp_path, [frame])
    assert header["con_spec_version"] == 3
    assert header["con_spec_minor"] == 1
    assert header["units"]["time"] == "fs"
    assert header["time"] == 12.5
    assert header["timestep"] == 1.25
    assert "velocities" not in header.get("sections", [])
    assert _component_rows(text, "Velocities of Component") == []
    assert back.time == 12.5
    assert back.timestep == 1.25
    assert not back.has_velocities


def test_pre31_velocities_without_clock(tmp_path):
    import readcon

    frame = readcon.read_con(str(FIXTURE / "tiny_cuh2.convel"))[0]
    assert frame.spec_version < 3
    assert frame.time is None
    original = _velocities(frame)
    header, text, back = _written(tmp_path, [frame])
    assert header["con_spec_version"] == 3
    assert header["con_spec_minor"] == 1
    assert header["units"]["time"] == NATURAL_TIME
    assert "time" not in header
    assert "timestep" not in header
    assert "velocities" in header["sections"]
    assert _component_rows(text, "Velocities of Component") == original
    assert _velocities(back) == original


def test_v31_time_unit_round_trip(tmp_path):
    import readcon

    src = readcon.read_con(str(FIXTURE / "tiny_cuh2.con"))[0]
    atom = readcon.Atom(
        "Cu",
        1.0,
        2.0,
        3.0,
        atom_id=1,
        mass=63.546,
        vx=0.25,
        vy=-0.5,
        vz=1.5,
    )
    frame = readcon.ConFrame(
        list(src.cell),
        list(src.angles),
        [atom],
        prebox_header=list(src.prebox_header),
        postbox_header=list(src.postbox_header),
        metadata={
            "con_spec_minor": 1,
            "time": 12.5,
            "timestep": 1.25,
            "units": {
                "length": "angstrom",
                "energy": "eV",
                "mass": "amu",
                "time": "ps",
            },
        },
    )
    assert frame.spec_version == 3
    header, text, back = _written(tmp_path, [frame])
    assert header["con_spec_version"] == 3
    assert header["con_spec_minor"] == 1
    assert header["units"]["time"] == "ps"
    assert header["time"] == 12.5
    assert header["timestep"] == 1.25
    assert "velocities" in header["sections"]
    expected = [(0.25, -0.5, 1.5)]
    assert _component_rows(text, "Velocities of Component") == expected
    assert _velocities(back) == expected
    assert back.time == 12.5
    assert back.timestep == 1.25
