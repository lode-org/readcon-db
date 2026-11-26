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


def test_extend_trajectory_str(tmp_path):
    head = (FIXTURE / "highprec_a.con").read_text()
    tail = (FIXTURE / "highprec_b.con").read_text()
    db = ConCorpus(str(tmp_path / "corpus"))
    assert db.append_trajectory_str(1, head, source="run.con") == 1
    assert db.extend_trajectory_str(1, tail, source="run.con") == 2
    assert db.get_frame_text(1, 0) == head
    assert db.get_frame_text(1, 1) == tail
    assert db.frame_hash(1, 0) == ConCorpus.hash_con_text(head)
    assert db.frame_hash(1, 1) == ConCorpus.hash_con_text(tail)
    assert len(db.select(traj_id=1)) == 2


def test_append_trajectory_frames(tmp_path):
    import readcon

    frames = readcon.read_con(str(FIXTURE / "tiny_cuh2.con"))
    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory_frames(1, frames, source="tiny_cuh2")
    assert n == len(frames)
    keys = db.select(traj_id=1)
    assert len(keys) == n


def test_append_trajectory_frames_stores_round_trip_text(tmp_path):
    import readcon

    pair = (FIXTURE / "highprec_a.con").read_text()
    pair += (FIXTURE / "highprec_b.con").read_text()
    frames = readcon.read_con_string(pair * 35)
    assert len(frames) == 70
    db = ConCorpus(str(tmp_path / "corpus"))
    assert db.append_trajectory_frames(1, frames, source="highprec") == 70
    for idx in (0, 1, 64, 69):
        stored = db.get_frame_text(1, idx)
        assert stored == readcon.write_con_string(frames, start=idx, stop=idx + 1)
        assert db.frame_hash(1, idx) == ConCorpus.hash_con_text(stored)
        got = readcon.read_con_string(stored)[0]
        want = frames[idx]
        assert tuple(got.cell) == tuple(want.cell)
        assert [(a.x, a.y, a.z, a.mass) for a in got.atoms] == [
            (a.x, a.y, a.z, a.mass) for a in want.atoms
        ]
    head = db.get_frame_text(1, 64)
    assert "21.702000987654323" in head
    assert "1.23456789e-9" in head


def _written(tmp_path, frames):
    import readcon

    db = ConCorpus(str(tmp_path / "corpus"))
    n = db.append_trajectory_frames(1, frames, source="time-units")
    assert n == len(frames)
    text = db.get_frame_text(1, 0)
    header = json.loads(text.splitlines()[1])
    # get_frame returns positions only. The stored CON text is the frame.
    back = readcon.read_con_string(text)[0]
    return header, text, back


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


def _header_line(spec):
    lines = (FIXTURE / "tiny_cuh2.convel").read_text().splitlines()
    lines[1] = spec
    return "\n".join(lines) + "\n"


def _append_read_con(tmp_path, text):
    import readcon

    path = tmp_path / "direct.con"
    path.write_text(text)
    frames = readcon.read_con(str(path))
    assert type(frames).__name__ == "ConFrameList"
    assert not isinstance(frames, (list, tuple))
    original = _velocities(readcon.read_con(str(path))[0])
    db = ConCorpus(str(tmp_path / "corpus"))
    assert db.append_trajectory_frames(1, frames, source="direct") == len(frames)
    stored = db.get_frame_text(1, 0)
    header = json.loads(stored.splitlines()[1])
    back = readcon.read_con_string(stored)[0]
    return header, stored, back, original


def test_direct_read_con_pre31_clock_with_velocities(tmp_path):
    text = _header_line('{"con_spec_version":2,"time":12.5,"timestep":1.25}')
    header, text_out, back, original = _append_read_con(tmp_path, text)
    scale = unit_conversion_factor("fs", NATURAL_TIME)
    assert header["con_spec_version"] == 3
    assert header["con_spec_minor"] == 1
    assert header["units"]["time"] == "fs"
    assert header["time"] == 12.5
    assert header["timestep"] == 1.25
    assert "velocities" in header["sections"]
    expected = [(vx * scale, vy * scale, vz * scale) for vx, vy, vz in original]
    assert _component_rows(text_out, "Velocities of Component") == expected
    assert _velocities(back) == expected
    assert back.time == 12.5
    assert back.timestep == 1.25


def test_direct_read_con_v31_clock_and_velocities(tmp_path):
    text = _header_line(
        '{"con_spec_version":3,"con_spec_minor":1,"time":12.5,"timestep":1.25,'
        '"units":{"length":"angstrom","energy":"eV","mass":"amu","time":"ps"}}'
    )
    header, text_out, back, original = _append_read_con(tmp_path, text)
    assert header["con_spec_version"] == 3
    assert header["con_spec_minor"] == 1
    assert header["units"]["time"] == "ps"
    assert header["time"] == 12.5
    assert header["timestep"] == 1.25
    assert "velocities" in header["sections"]
    assert _component_rows(text_out, "Velocities of Component") == original
    assert _velocities(back) == original
    assert back.time == 12.5
    assert back.timestep == 1.25
