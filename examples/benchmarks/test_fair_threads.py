"""Fair rows keep twelve samples and pin threads on both tools."""

from __future__ import annotations

import ast
import inspect
import os
import subprocess
import sys
import textwrap
from pathlib import Path

import pytest

pytest.importorskip("readcon")
pytest.importorskip("readcon_db")

import fair_ase_protocol as fair
import raw_full_bisect as bisect


def test_paired_rows_are_one_thread_on_both_tools() -> None:
    assert ("readcon-db", "1") in fair.PAIRED_THREADS
    assert ("ase.db", "1") in fair.PAIRED_THREADS
    assert ("readcon-db", "unset") in fair.PAIRED_THREADS
    assert all(rayon == "1" for kind, rayon in fair.PAIRED_THREADS if kind == "ase.db")


def test_apply_rayon_sets_and_clears_the_pool() -> None:
    one = fair.apply_rayon({"RAYON_NUM_THREADS": "8"}, "1")
    assert one["RAYON_NUM_THREADS"] == "1"
    unset = fair.apply_rayon({"RAYON_NUM_THREADS": "8"}, "unset")
    assert "RAYON_NUM_THREADS" not in unset
    for env in (one, unset):
        for name in fair.BLAS_THREAD_VARS:
            assert env[name] == "1"


def test_spawned_row_defaults_to_one_thread(monkeypatch) -> None:
    seen = {}

    def run(cmd, **kwargs):
        seen.update(kwargs["env"])
        return subprocess.CompletedProcess(cmd, 0, stdout="{}", stderr="")

    monkeypatch.setattr(fair.subprocess, "run", run)
    monkeypatch.setenv("RAYON_NUM_THREADS", "8")
    assert fair.spawn_row({"what": "read"}) == {}
    assert seen["RAYON_NUM_THREADS"] == "1"


def test_thread_fields_report_the_environment(monkeypatch) -> None:
    monkeypatch.setenv("RAYON_NUM_THREADS", "2")
    assert fair.thread_fields()["rayon_num_threads"] == "2"
    assert fair.thread_fields()["threads"] == 2
    monkeypatch.delenv("RAYON_NUM_THREADS")
    assert fair.thread_fields()["rayon_num_threads"] == "unset"
    assert fair.thread_fields()["threads"] == (os.cpu_count() or 1)


@pytest.mark.parametrize("func", [fair.execute_insert_row, fair._execute_read_row])
def test_sample_loops_have_no_early_stop(func) -> None:
    tree = ast.parse(textwrap.dedent(inspect.getsource(func)))
    loops = [
        node
        for node in ast.walk(tree)
        if isinstance(node, ast.For)
        and isinstance(node.iter, ast.Call)
        and ast.unparse(node.iter) == "range(N_SAMPLES)"
    ]
    assert len(loops) == 1
    assert not any(isinstance(node, ast.While) for node in ast.walk(tree))
    assert not any(
        isinstance(node, (ast.Break, ast.Return)) for node in _loop_body(loops[0])
    )


def _loop_body(loop: ast.For):
    """Nodes of the loop, without the bodies of functions it defines."""
    stack = list(loop.body)
    while stack:
        node = stack.pop()
        yield node
        if not isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef, ast.Lambda)):
            stack.extend(ast.iter_child_nodes(node))


def test_n_below_twelve_is_refused() -> None:
    env = dict(os.environ, READCON_DB_N_SAMPLES="6")
    proc = subprocess.run(
        [sys.executable, "-c", "import fair_ase_protocol"],
        check=False,
        cwd=Path(fair.__file__).resolve().parent,
        env=env,
        capture_output=True,
        text=True,
    )
    assert proc.returncode != 0
    assert "n>=12" in proc.stderr


def test_bisect_refuses_fewer_than_thirty_pairs() -> None:
    with pytest.raises(ValueError, match="n>=30"):
        bisect.summarize_pair([1.0] * 29, [1.0] * 29)
