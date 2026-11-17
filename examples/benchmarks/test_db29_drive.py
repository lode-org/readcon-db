"""The committed db29 driver can write the record layout."""

from __future__ import annotations

import json
from pathlib import Path

import pytest

pytest.importorskip("readcon")
pytest.importorskip("readcon_db")

import db29_drive as drv


def _side(median: float, name: str) -> dict:
    return {
        name: {
            "median_s": median,
            "iqr_s": 0.0,
            "samples_s": [median] * 12,
            "peak_rss_bytes": 1,
        }
    }


def _block(left: str, right: str) -> dict:
    block = {}
    block.update(_side(1.0, left))
    block.update(_side(0.5, right))
    block["paired"] = {
        "median": 0.5,
        "ci95": [0.4, 0.6],
        "right_faster": 12,
        "n": 12,
        "ci_method": drv.BOOT_NOTE,
    }
    return block


def test_driver_writes_policy_and_float32_sections(tmp_path) -> None:
    reads = {}
    policy = {}
    codecs = {}
    for arm in ("md", "thermal", "lattice"):
        group = {f"{op}:{cache}": _block("raw", "shuffle") for cache, op in drv.PLAN}
        reads[arm] = {"threads_unset": group, "threads_1": group}
        policy[arm] = {
            "threads_unset": {
                f"{op}:{cache}": _block("warm-point", "cold-bulk")
                for cache, op in drv.PLAN
            },
            "threads_1": {
                f"{op}:{cache}": _block("warm-point", "cold-bulk")
                for cache, op in drv.PLAN
            },
        }
        codecs[arm] = {
            mode: {
                "cooked_codec": "raw"
                if mode in ("raw", "warm-point")
                else "shuffle-zstd",
                "atoms_per_frame": 8,
                "corpus_bytes": 10,
                "frames_soa_value_bytes": 4,
                "hit_shuffle": mode in ("shuffle", "cold-bulk"),
            }
            for mode in drv.MODES
        }
    bits = {
        "low32_zero_frac": 0.0,
        "low29_zero_frac": 1.0,
        "low24_zero_frac": 1.0,
        "n": 3,
        "byte_lane_zero_frac": [1.0] * 8,
    }
    state = {
        "commit": "a" * 40,
        "tree": "b" * 40,
        "codecs": codecs,
        "reads": reads,
        "policy": policy,
        "bits": {
            "md": bits,
            "thermal": bits,
            "lattice": bits,
            "source": {"widened_nm": bits, "times_10_angstrom": bits},
        },
    }
    drv.write_record(state, "commit sample\n", tmp_path)
    text = (tmp_path / "RESULTS.md").read_text()
    assert "set_cooked_access" in text
    assert "Float32" in text
    assert "| md |" in text
    ab = json.loads((tmp_path / "ab.json").read_text())
    assert ab["n"] >= 12
    assert ab["host"]["hostname"] == "host"
    assert "policy" in ab and "bits" in ab


def test_campaign_log_is_replaced_not_appended(tmp_path: Path) -> None:
    path = tmp_path / "campaign.log"
    path.write_text("previous run\n")
    with drv.open_campaign_log(path) as log:
        log.write("commit sample\n")
    assert path.read_text() == "commit sample\n"


def test_state_path_includes_out(tmp_path, monkeypatch) -> None:
    monkeypatch.setattr(drv, "REPO", tmp_path)
    monkeypatch.setattr(drv, "_readcon_core_lock_identity", lambda: "c" * 40)
    (tmp_path / "build").mkdir()
    left = drv.state_path(drv.state_scope(tmp_path / "one", 12))
    right = drv.state_path(drv.state_scope(tmp_path / "two", 12))
    assert left != right


def test_state_path_includes_core_identity(tmp_path, monkeypatch) -> None:
    monkeypatch.setattr(drv, "REPO", tmp_path)
    identities = iter(["c" * 40, "d" * 40])
    monkeypatch.setattr(drv, "_readcon_core_lock_identity", lambda: next(identities))
    out = tmp_path / "bench" / "db29"
    assert drv.state_path(drv.state_scope(out, 12)) != drv.state_path(
        drv.state_scope(out, 12)
    )


def test_load_state_rejects_a_mismatched_scope(tmp_path, monkeypatch) -> None:
    monkeypatch.setattr(drv, "REPO", tmp_path)
    monkeypatch.setattr(drv, "_readcon_core_lock_identity", lambda: "c" * 40)
    (tmp_path / "build").mkdir()
    out = tmp_path / "bench" / "db29"
    scope = drv.state_scope(out, 12)
    path = drv.state_path(scope)
    drv.save_state(drv.load_state(path, scope), path, scope)
    tampered = json.loads(path.read_text())
    tampered["out"] = str((tmp_path / "other").resolve())
    tampered["readcon_core"] = "d" * 40
    path.write_text(json.dumps(tampered))
    with pytest.raises(SystemExit, match="state mismatch"):
        drv.load_state(path, scope)


def test_state_is_scoped_by_out_n_and_core(tmp_path, monkeypatch) -> None:
    monkeypatch.setattr(drv, "REPO", tmp_path)
    monkeypatch.setattr(drv, "_readcon_core_lock_identity", lambda: "c" * 40)
    build = tmp_path / "build"
    build.mkdir(parents=True, exist_ok=True)
    out = tmp_path / "bench" / "db29"
    scope_12 = drv.state_scope(out, 12)
    path_12 = drv.state_path(scope_12)
    state = drv.load_state(path_12, scope_12)
    state["commit"] = "a" * 40
    drv.save_state(state, path_12, scope_12)
    scope_6 = drv.state_scope(out, 6)
    path_6 = drv.state_path(scope_6)
    assert path_6 != path_12
    fresh = drv.load_state(path_6, scope_6)
    assert fresh["reads"] == {}
    assert fresh["policy"] == {}


def test_release_env_n_must_be_integer(monkeypatch) -> None:
    monkeypatch.setenv("READCON_RERUN_RELEASE", "1")
    monkeypatch.setenv("READCON_DB29_N", "oops")
    monkeypatch.setattr("sys.argv", ["db29_drive.py"])
    with pytest.raises(SystemExit, match="READCON_DB29_N must be an integer"):
        drv.main()
