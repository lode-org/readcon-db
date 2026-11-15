"""The db28 driver writes the four record files."""

from __future__ import annotations

import json
import socket

import pytest

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


def test_fresh_fair_logs_use_the_neutral_label() -> None:
    fresh = drv.REPO / "examples" / "benchmarks" / "results" / "fair-db29"
    for name in ("campaign.log", "insert-span.log", "insert-validate.log"):
        assert (fresh / name).read_text().splitlines()[2] == "host host"


def _finished_replacement(directory) -> None:
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "campaign.log").write_text("done\n")
    (directory / "RESULTS.md").write_text("# results\n")


def test_superseded_directory_needs_a_real_replacement(tmp_path) -> None:
    old = tmp_path / "bench" / "db1"
    old.mkdir(parents=True)
    (old / "campaign.log").write_text('{"corpus": "' + "/" + "tmp" + '/ab"}\n')
    (old / "SUPERSEDED.md").write_text("Replacement: bench/db30/missing\n")
    assert drv.vm_host_hits(tmp_path) == ["bench/db1/campaign.log"]
    (tmp_path / "bench" / "db30" / "db1-rerun").mkdir(parents=True)
    (old / "SUPERSEDED.md").write_text("Replacement: bench/db30/db1-rerun\n")
    assert drv.vm_host_hits(tmp_path) == ["bench/db1/campaign.log"]


def test_partial_replacement_does_not_hide_the_record(tmp_path) -> None:
    old = tmp_path / "bench" / "db1"
    old.mkdir(parents=True)
    (old / "campaign.log").write_text('{"corpus": "' + "/" + "tmp" + '/ab"}\n')
    (old / "SUPERSEDED.md").write_text("Replacement: bench/db30\n")
    dest = tmp_path / "bench" / "db30"
    dest.mkdir(parents=True)
    assert drv.vm_host_hits(tmp_path) == ["bench/db1/campaign.log"]
    (dest / "RESULTS.md").write_text("# only\n")
    assert drv.vm_host_hits(tmp_path) == ["bench/db1/campaign.log"]
    (dest / "campaign.log").write_text("")
    (dest / "RESULTS.md").write_text("# results\n")
    assert drv.vm_host_hits(tmp_path) == ["bench/db1/campaign.log"]
    (dest / "campaign.log").write_text("done\n")
    (dest / "RESULTS.md").write_text("")
    assert drv.vm_host_hits(tmp_path) == ["bench/db1/campaign.log"]
    _finished_replacement(dest)
    assert drv.vm_host_hits(tmp_path) == []


def test_replacement_equal_to_or_inside_the_record_is_refused(tmp_path) -> None:
    record = tmp_path / "bench" / "db1"
    _finished_replacement(record)
    (record / "campaign.log").write_text('{"corpus": "' + "/" + "tmp" + '/ab"}\n')
    for named in ("bench/db1", "bench/db1/../db1", "bench/db1/nested"):
        if named.endswith("nested"):
            _finished_replacement(record / "nested")
        (record / "SUPERSEDED.md").write_text("Replacement: " + named + "\n")
        assert drv.vm_host_hits(tmp_path) == ["bench/db1/campaign.log"]


def test_record_note_hides_a_nested_file(tmp_path) -> None:
    record = tmp_path / "bench" / "db1"
    nested = record / "sub"
    nested.mkdir(parents=True)
    (nested / "hit.json").write_text('{"corpus": "' + "/" + "tmp" + '/ab"}\n')
    _finished_replacement(tmp_path / "bench" / "db30")
    (tmp_path / "bench" / "SUPERSEDED.md").write_text("Replacement: bench/db30\n")
    assert drv.vm_host_hits(tmp_path) == ["bench/db1/sub/hit.json"]
    (record / "SUPERSEDED.md").write_text("Replacement: bench/db30\n")
    assert drv.vm_host_hits(tmp_path) == []


def test_ancestor_note_does_not_hide_the_record(tmp_path) -> None:
    record = tmp_path / "bench" / "db1"
    record.mkdir(parents=True)
    (record / "campaign.log").write_text('{"corpus": "' + "/" + "tmp" + '/ab"}\n')
    _finished_replacement(tmp_path / "bench" / "db30")
    (tmp_path / "bench" / "SUPERSEDED.md").write_text("Replacement: bench/db30\n")
    assert drv.vm_host_hits(tmp_path) == ["bench/db1/campaign.log"]
    (record / "SUPERSEDED.md").write_text("Replacement: bench/db30\n")
    assert drv.vm_host_hits(tmp_path) == []


def test_vm_host_hits_reads_logs_notes_and_json(tmp_path) -> None:
    fair = tmp_path / "examples" / "benchmarks" / "results" / "fair"
    fair.mkdir(parents=True)
    (fair / "campaign.log").write_text("rev\ndate\nhost " + "cursor" + "\n")
    (tmp_path / "examples" / "benchmarks" / "notes.md").write_text(
        "Host `" + "cursor" + "`\n"
    )
    (fair / "host.json").write_text('{"hostname": "' + "cursor" + '"}\n')
    (fair / "ok.json").write_text('{"hostname": "host"}\n')
    bench = tmp_path / "bench" / "old"
    bench.mkdir(parents=True)
    (bench / "ab.json").write_text('{"corpus": "' + "/" + "tmp" + '/ab"}\n')
    (bench / "home.json").write_text('{"path": "' + "/" + "home" + '/x"}\n')
    (bench / "agent.json").write_text('{"path": "' + "/" + "agent" + '/x"}\n')
    assert drv.vm_host_hits(tmp_path) == [
        "examples/benchmarks/notes.md",
        "examples/benchmarks/results/fair/campaign.log",
        "examples/benchmarks/results/fair/host.json",
        "bench/old/ab.json",
        "bench/old/agent.json",
        "bench/old/home.json",
    ]


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
