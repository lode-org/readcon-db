"""The committed db29 driver can write the record layout."""

from __future__ import annotations

import json

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
