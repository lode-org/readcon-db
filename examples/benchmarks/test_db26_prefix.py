"""Each measured commit is installed from its own worktree."""

from __future__ import annotations

import pytest

pytest.importorskip("readcon")
pytest.importorskip("readcon_db")

import db26_drive as drv


def test_highbyte_keeps_an_explicit_rustup_toolchain(monkeypatch, tmp_path) -> None:
    import db26_highbyte as high

    monkeypatch.setattr(high.fair, "require_pushed_commit", lambda repo=None: None)
    monkeypatch.setattr(high.fair, "git_out", lambda *_args: "a" * 40)
    monkeypatch.setenv("RUSTUP_TOOLCHAIN", "1.91.0")
    seen: dict[str, str] = {}

    def run(*_args, **kwargs):
        seen["toolchain"] = kwargs["env"]["RUSTUP_TOOLCHAIN"]

        class Proc:
            stdout = '{"ok": true}\n'
            stderr = ""

        return Proc()

    monkeypatch.setattr(high.subprocess, "run", run)
    high.main(tmp_path / "high_byte.json")
    assert seen["toolchain"] == "1.91.0"


def test_env_base_keeps_an_explicit_rustup_toolchain(monkeypatch) -> None:
    monkeypatch.setenv("RUSTUP_TOOLCHAIN", "1.91.0")
    assert drv.env_base()["RUSTUP_TOOLCHAIN"] == "1.91.0"
    monkeypatch.delenv("RUSTUP_TOOLCHAIN")
    assert drv.env_base()["RUSTUP_TOOLCHAIN"] == "1.88.0"


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
    monkeypatch.setattr(drv, "worktree_head", lambda src: "")
    monkeypatch.setattr(drv.fair, "pip_install", pip_install)
    first = "a" * 40
    second = "b" * 40
    drv.ensure_tree_prefix("left", first)
    drv.ensure_tree_prefix("right", second)
    assert [commit for _src, commit in added] == [first, second]
    assert len({src for src, _commit in added}) == 2
    assert len({src for src, _target in installed}) == 2
    assert len({target for _src, target in installed}) == 2


def test_a_stamp_from_before_per_commit_worktrees_is_rebuilt(
    tmp_path, monkeypatch
) -> None:
    commit = "c" * 40
    prefix = tmp_path / "build" / "wheels" / "label"
    prefix.mkdir(parents=True)
    (prefix / ".commit").write_text(commit + "\n")
    monkeypatch.setattr(drv, "REPO", tmp_path)
    monkeypatch.setattr(drv.fair, "prefix_ready", lambda prefix, commit: True)
    monkeypatch.setattr(drv, "site_packages", lambda prefix: prefix)
    monkeypatch.setattr(drv.subprocess, "check_call", lambda cmd, cwd=None: 0)
    monkeypatch.setattr(drv, "worktree_head", lambda src: commit)
    installed = []
    monkeypatch.setattr(
        drv.fair, "pip_install", lambda prefix, src, env: installed.append(src)
    )
    drv.ensure_tree_prefix("label", commit)
    assert installed == [tmp_path / "build" / f"src-{commit[:12]}" / "python"]
    assert (prefix / ".source").read_text().strip() == f"src-{commit[:12]}"
    drv.ensure_tree_prefix("label", commit)
    assert len(installed) == 1


def test_a_worktree_on_another_commit_is_replaced(tmp_path, monkeypatch) -> None:
    commit = "d" * 40
    monkeypatch.setattr(drv, "REPO", tmp_path)
    monkeypatch.setattr(drv.fair, "prefix_ready", lambda prefix, commit: False)
    monkeypatch.setattr(drv, "site_packages", lambda prefix: prefix)
    monkeypatch.setattr(drv, "worktree_head", lambda src: "e" * 40)
    src = tmp_path / "build" / f"src-{commit[:12]}"
    (src / "python").mkdir(parents=True)
    (src / "python" / "pyproject.toml").write_text("")
    calls = []
    monkeypatch.setattr(
        drv.subprocess, "check_call", lambda cmd, cwd=None: calls.append(cmd[:3])
    )
    monkeypatch.setattr(drv.fair, "pip_install", lambda prefix, src, env: None)
    drv.ensure_tree_prefix("label", commit)
    assert calls == [["git", "worktree", "remove"], ["git", "worktree", "add"]]
