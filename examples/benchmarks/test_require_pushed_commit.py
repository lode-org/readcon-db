"""require_pushed_commit refuses a dirty tree and an unpushed commit."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

import db24_drive


def git(repo: Path, *args: str) -> None:
    env = os.environ.copy()
    env["GIT_AUTHOR_NAME"] = "Test"
    env["GIT_AUTHOR_EMAIL"] = "test@example.com"
    env["GIT_COMMITTER_NAME"] = "Test"
    env["GIT_COMMITTER_EMAIL"] = "test@example.com"
    subprocess.check_call(["git", *args], cwd=repo, env=env)


def test_require_pushed_commit_rejects_unpushed_and_dirty() -> None:
    root = Path(__file__).resolve().parents[2] / "build" / "require-pushed-test"
    if root.exists():
        subprocess.check_call(["rm", "-rf", str(root)])
    origin = root / "origin.git"
    repo = root / "checkout"
    origin.mkdir(parents=True)
    subprocess.check_call(["git", "init", "--bare", str(origin)])
    repo.mkdir()
    git(repo, "init", "-b", "main")
    git(repo, "remote", "add", "origin", str(origin))
    (repo / "readme").write_text("one\n")
    git(repo, "add", "readme")
    git(repo, "commit", "-m", "one")
    git(repo, "push", "-u", "origin", "HEAD")
    db24_drive.require_pushed_commit(repo)

    (repo / "readme").write_text("two\n")
    git(repo, "add", "readme")
    git(repo, "commit", "-m", "two")
    try:
        db24_drive.require_pushed_commit(repo)
    except SystemExit as exc:
        assert "not contained" in str(exc)
    else:
        raise AssertionError("unpushed HEAD was accepted")

    git(repo, "push", "origin", "HEAD")
    (repo / "readme").write_text("dirty\n")
    try:
        db24_drive.require_pushed_commit(repo)
    except SystemExit as exc:
        assert "not clean" in str(exc)
    else:
        raise AssertionError("dirty tree was accepted")
