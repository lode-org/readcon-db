"""The db28 driver writes the four record files."""

from __future__ import annotations

import json
import socket

import pytest

pytest.importorskip("readcon")
pytest.importorskip("readcon_db")

import db28_drive as drv


def test_driver_writes_the_record_layout(tmp_path) -> None:
    record = drv.example_record()
    drv.write_outputs(tmp_path, record, "commit sample\n")
    for name in drv.OUTPUTS:
        path = tmp_path / name
        assert path.is_file(), name
        assert path.stat().st_size > 0
        assert path.read_bytes().endswith(b"\n")
    text = (tmp_path / "RESULTS.md").read_text()
    assert text.startswith("# db28")
    assert "| md |" in text
    assert "| thermal |" in text
    assert "| lattice |" in text
    ab = json.loads((tmp_path / "ab.json").read_text())
    assert set(ab) >= {"commit", "tree", "host", "n", "codecs", "reads"}
    assert set(ab["codecs"]) == {"md", "thermal", "lattice"}
    host = json.loads((tmp_path / "host.json").read_text())
    assert host["commit"] == record["commit"]
    assert "hostname" in host
    assert (tmp_path / "campaign.log").read_text() == "commit sample\n"
    assert host["hostname"] == "host"


def test_recorded_run_refuses_n_below_12(tmp_path) -> None:
    assert drv.N >= 12
    with pytest.raises(ValueError, match="n>=12"):
        drv.require_recorded_n(6)
    with pytest.raises(ValueError, match="n>=12"):
        drv.require_recorded_n(11)
    assert drv.require_recorded_n(12) == 12
    record = drv.example_record()
    record["n"] = 6
    with pytest.raises(ValueError, match="n>=12"):
        drv.write_outputs(tmp_path, record, "short\n")


def test_host_label_rejects_cursor_and_any_other_hostname(tmp_path) -> None:
    assert drv.host_label("host") == "host"
    for name in ("cursor", "Cursor", "localhost", "ip-10-0-0-1", socket.gethostname()):
        if name == "host":
            continue
        with pytest.raises(ValueError):
            drv.host_label(name)
    record = drv.example_record()
    record["host"]["hostname"] = "cursor"
    with pytest.raises(ValueError):
        drv.write_outputs(tmp_path, record, "nope\n")
