"""Rerun drivers compare HEAD with the remote branch tip."""

from __future__ import annotations

import os
import subprocess
from pathlib import Path

import db17_drive
import legacy_campaign_drive as legacy
import pytest

BRANCH = "fix/ingest-species-count"


def git(repo: Path, *args: str) -> str:
    env = os.environ.copy()
    env["GIT_AUTHOR_NAME"] = "Test"
    env["GIT_AUTHOR_EMAIL"] = "test@example.com"
    env["GIT_COMMITTER_NAME"] = "Test"
    env["GIT_COMMITTER_EMAIL"] = "test@example.com"
    return subprocess.check_output(["git", *args], cwd=repo, text=True, env=env).strip()


def measurement_repo(tmp_path: Path) -> tuple[Path, str, str]:
    """Origin tip is newer than the local ``origin/<branch>`` ref.

    ``remote.origin.fetch`` covers main only, so a fetch of the branch
    would leave that tracking ref where it is.
    """
    origin = tmp_path / "origin.git"
    repo = tmp_path / "checkout"
    subprocess.check_call(["git", "init", "--bare", "-b", "main", str(origin)])
    subprocess.check_call(["git", "init", "-b", "main", str(repo)])
    git(repo, "remote", "add", "origin", str(origin))
    git(
        repo,
        "config",
        "remote.origin.fetch",
        "+refs/heads/main:refs/remotes/origin/main",
    )
    (repo / "readme").write_text("one\n")
    (repo / ".gitignore").write_text("secret.ignored\n")
    git(repo, "add", "readme", ".gitignore")
    git(repo, "commit", "-m", "one")
    old = git(repo, "rev-parse", "HEAD")
    git(repo, "branch", BRANCH)
    git(repo, "push", "origin", "main")
    git(repo, "push", "origin", BRANCH)
    git(repo, "checkout", BRANCH)
    (repo / "readme").write_text("two\n")
    git(repo, "commit", "-am", "two")
    tip = git(repo, "rev-parse", "HEAD")
    git(repo, "push", "origin", f"HEAD:refs/heads/{BRANCH}")
    git(repo, "update-ref", f"refs/remotes/origin/{BRANCH}", old)
    git(repo, "checkout", "--detach", old)
    return repo, old, tip


def require(driver: str, repo: Path, monkeypatch: pytest.MonkeyPatch) -> str:
    if driver == "db17":
        return db17_drive.require_tip(repo)
    monkeypatch.setenv("READCON_MEASURE_ROOT", str(repo))
    return legacy.require_tip()


@pytest.mark.parametrize("driver", ["db17", "legacy"])
def test_stale_tracking_ref_refuses_an_old_detached_head(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, driver: str
) -> None:
    repo, old, tip = measurement_repo(tmp_path)
    assert git(repo, "rev-parse", "HEAD") == old
    assert git(repo, "rev-parse", f"refs/remotes/origin/{BRANCH}") == old
    assert git(repo, "ls-remote", "origin", f"refs/heads/{BRANCH}").split()[0] == tip
    with pytest.raises(SystemExit, match="refusing to measure") as caught:
        require(driver, repo, monkeypatch)
    assert tip in str(caught.value)
    assert old in str(caught.value)
    git(repo, "checkout", "--detach", tip)
    assert require(driver, repo, monkeypatch) == tip


@pytest.mark.parametrize("driver", ["db17", "legacy"])
def test_untracked_file_hidden_by_status_config_is_refused(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch, driver: str
) -> None:
    repo, _old, tip = measurement_repo(tmp_path)
    git(repo, "checkout", "--detach", tip)
    git(repo, "config", "status.showUntrackedFiles", "no")
    (repo / "secret.ignored").write_text("skip\n")
    assert git(repo, "status", "--porcelain").strip() == ""
    assert require(driver, repo, monkeypatch) == tip
    (repo / "notes.txt").write_text("local\n")
    assert git(repo, "status", "--porcelain").strip() == ""
    with pytest.raises(SystemExit, match="not clean"):
        require(driver, repo, monkeypatch)
