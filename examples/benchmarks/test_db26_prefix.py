"""Each measured commit is installed from its own worktree."""

from __future__ import annotations

import pytest

pytest.importorskip("readcon")
pytest.importorskip("readcon_db")

import db26_drive as drv


def test_two_commits_build_from_two_worktrees(tmp_path, monkeypatch) -> None:
    monkeypatch.setattr(drv, "REPO", tmp_path)
    monkeypatch.setattr(drv.fair, "prefix_ready", lambda prefix, commit: False)
    monkeypatch.setattr(drv, "site_packages", lambda prefix: prefix)
    added = []

    def check_call(cmd, cwd=None):
        if cmd[:3] == ["git", "worktree", "add"]:
            added.append((cmd[-2], cmd[-1]))
        return 0

    installed = []

    def pip_install(prefix, src, env):
        installed.append((str(src), env["CARGO_TARGET_DIR"]))

    monkeypatch.setattr(drv.subprocess, "check_call", check_call)
    monkeypatch.setattr(drv.fair, "pip_install", pip_install)
    first = "a" * 40
    second = "b" * 40
    drv.ensure_tree_prefix("left", first)
    drv.ensure_tree_prefix("right", second)
    assert [commit for _src, commit in added] == [first, second]
    assert len({src for src, _commit in added}) == 2
    assert len({src for src, _target in installed}) == 2
    assert len({target for _src, target in installed}) == 2
