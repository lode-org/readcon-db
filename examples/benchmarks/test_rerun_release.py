"""Dry-run, refusal, and synthetic provenance tests for the release rerun."""

from __future__ import annotations

import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from dataclasses import replace
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = "examples/benchmarks/rerun_release.py"
KINDS = ("ab.json", "readcon_db_bench.json", "campaign.json", "host.json")
PRODUCERS = (
    "examples/benchmarks/db24_drive.py",
    "examples/benchmarks/db25_drive.py",
    "examples/benchmarks/db26_drive.py",
    "examples/benchmarks/db28_drive.py",
    "examples/benchmarks/db29_drive.py",
    "examples/bench_corpus_ops.rs",
    "examples/benchmarks/fair_ase_protocol.py",
)
OUTPUTS = (
    "db33/db24/ab.json",
    "db33/db25/ab.json",
    "db33/db26/ab.json",
    "db33/db28/ab.json",
    "db33/db29/ab.json",
    "db33/corpus-ops/readcon_db_bench.json",
    "db33/fair/campaign.json",
    "db33/db8/campaign.json",
)
SUPERSEDED = {
    "db30/fair-db30": "db33/fair/campaign.json",
    "db31/fair-db31": "db33/fair/campaign.json",
    "db32/fair-db32": "db33/fair/campaign.json",
    "db30/db8": "db33/db8/campaign.json",
    "db31/db24": "db33/db24/ab.json",
    "db31/db25": "db33/db25/ab.json",
    "db31/db26": "db33/db26/ab.json",
    "db31/db28": "db33/db28/ab.json",
    "db31/db29": "db33/db29/ab.json",
    "db31/corpus-ops": "db33/corpus-ops/readcon_db_bench.json",
}
PROVENANCE = (
    "commit",
    "tree",
    "readcon_core",
    "readcon_sha",
    "rustc",
    "python",
    "allocator",
)
HOST = ("cpu", "ncpu", "cpuinfo_mhz", "kernel")
HOST += ("family", "model", "mhz", "nproc")
_GIT_ENV = {
    "GIT_AUTHOR_NAME": "Rohit Goswami",
    "GIT_AUTHOR_EMAIL": "rgoswami@ieee.org",
    "GIT_COMMITTER_NAME": "Rohit Goswami",
    "GIT_COMMITTER_EMAIL": "rgoswami@ieee.org",
}


def _git(repo: Path, *args: str) -> subprocess.CompletedProcess[str]:
    env = {**os.environ, **_GIT_ENV}
    return subprocess.run(
        ["git", "-C", str(repo), *args],
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )


def _sync_uncommitted(src: Path, dst: Path) -> None:
    status = _git(src, "status", "--porcelain")
    for line in status.stdout.splitlines():
        if not line.strip():
            continue
        code = line[:2]
        rel = line[3:].split(" -> ")[-1]
        target = dst / rel
        if "D" in code:
            target.unlink(missing_ok=True)
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(src / rel, target)


def _isolated_clean_repo(tmp_path: Path) -> Path:
    repo = tmp_path / "repo"
    done = subprocess.run(
        ["git", "clone", "--shared", "--quiet", str(ROOT), str(repo)],
        capture_output=True,
        text=True,
        check=False,
    )
    assert done.returncode == 0, done.stderr
    _sync_uncommitted(ROOT, repo)
    if _git(repo, "status", "--porcelain").stdout.strip():
        add = _git(repo, "add", "-A")
        assert add.returncode == 0, add.stderr
        commit = _git(
            repo,
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "test: stage candidate changes",
        )
        assert commit.returncode == 0, commit.stderr
    updated = _git(repo, "update-ref", "refs/remotes/origin/release-test", "HEAD")
    assert updated.returncode == 0, updated.stderr
    return repo


def _run(repo: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [sys.executable, str(repo / SCRIPT), *args],
        cwd=repo,
        capture_output=True,
        text=True,
        check=False,
    )


def _saved_digest(repo: Path) -> str:
    return hashlib.sha256(
        (repo / "bench" / "db24" / "ab.json").read_bytes()
    ).hexdigest()


def test_dry_run_lists_every_record_kind(tmp_path: Path) -> None:
    """``--dry-run`` names every record kind, producer, and paper path."""
    repo = _isolated_clean_repo(tmp_path)
    saved = _saved_digest(repo)
    out = repo / "build" / "rerun-dry"
    done = _run(repo, "--dry-run", "--n", "6", "--out", str(out))
    output = done.stdout + done.stderr
    assert done.returncode == 0, output
    assert not out.exists()
    missing_kinds = [kind for kind in KINDS if f"kind {kind}" not in output]
    assert not missing_kinds, output
    missing_producers = [name for name in PRODUCERS if name not in output]
    assert not missing_producers, output
    missing_outputs = [path for path in OUTPUTS if path not in output]
    assert not missing_outputs, output
    for path in OUTPUTS:
        parent = str(Path(path).parent)
        assert f"{parent}/host.json" in output
        assert f"{parent}/raw.log" in output
        assert f"export {path}" in output
    assert "--mode all" in output
    assert "--mode db8" in output
    assert "export db30/fair-db30/campaign.json" not in output
    assert "export db31/fair-db31/campaign.json" not in output
    assert "export db32/fair-db32/campaign.json" not in output
    assert "export db30/db8/campaign.json" not in output
    for old, new in SUPERSEDED.items():
        assert f"supersede {old} -> {new}" in output
    assert "bench/db24/ab.json" not in output
    assert _saved_digest(repo) == saved

    short = _run(repo, "--dry-run", "--n", "5", "--out", str(out))
    assert short.returncode != 0
    assert "at least 6" in (short.stdout + short.stderr)

    saved_out = _run(repo, "--dry-run", "--out", "bench/db24")
    assert saved_out.returncode != 0
    assert "saved record" in (saved_out.stdout + saved_out.stderr)
    assert _saved_digest(repo) == saved


def test_refuses_unpushed_or_dirty_tree(tmp_path: Path) -> None:
    """A dirty tree or a commit on no remote branch is refused."""
    repo = _isolated_clean_repo(tmp_path)
    readme = repo / "README.md"
    readme.write_text(readme.read_text(encoding="utf-8") + "\n", encoding="utf-8")
    dirty = _run(repo, "--dry-run")
    assert dirty.returncode != 0
    assert "dirty tree" in (dirty.stdout + dirty.stderr)
    _git(repo, "checkout", "--", "README.md")

    unpushed = _git(
        repo,
        "-c",
        "core.hooksPath=/dev/null",
        "-c",
        "commit.gpgsign=false",
        "commit",
        "--allow-empty",
        "-m",
        "test: unpushed commit",
    )
    assert unpushed.returncode == 0, unpushed.stderr
    refused = _run(repo, "--dry-run")
    assert refused.returncode != 0
    assert "not on any remote branch" in (refused.stdout + refused.stderr)


def _assert_provenance(path: Path) -> None:
    data = json.loads(path.read_text())
    missing = [key for key in PROVENANCE if not data.get(key)]
    assert not missing, (path, missing)
    nested = data.get("provenance")
    assert isinstance(nested, dict)
    nested_missing = [key for key in PROVENANCE if not nested.get(key)]
    assert not nested_missing, (path, nested_missing)
    host = data["host"]
    assert isinstance(host, dict)
    host_missing = [key for key in HOST if host.get(key) in (None, "")]
    assert not host_missing, (path, host_missing)
    assert len(data["commit"]) == 40
    assert len(data["tree"]) == 40
    assert len(data["readcon_sha"]) == 40
    beside = json.loads((path.parent / "host.json").read_text())
    flat_missing = [
        key for key in (*PROVENANCE, *HOST) if beside.get(key) in (None, "")
    ]
    for key in ("readcon_core", "readcon_sha", "rustc", "python"):
        assert beside.get(key), (path.parent / "host.json", key)
    assert not flat_missing, (path.parent / "host.json", flat_missing)
    assert (path.parent / "raw.log").is_file()
    assert path.read_bytes().endswith(b"\n")


def test_synthetic_run_carries_provenance(tmp_path: Path) -> None:
    """A tiny synthetic run writes every record kind with provenance."""
    repo = _isolated_clean_repo(tmp_path)
    saved = _saved_digest(repo)
    out = repo / "build" / "rerun-synthetic"
    export = repo / "build" / "rerun-synthetic-export"
    done = _run(
        repo,
        "--synthetic",
        "--n",
        "6",
        "--out",
        str(out),
        "--export",
        str(export),
    )
    output = done.stdout + done.stderr
    assert done.returncode == 0, output
    for rel in OUTPUTS:
        _assert_provenance(out / rel)
        exported = export / rel
        assert exported.is_file(), rel
        _assert_provenance(exported)
        record = json.loads((out / rel).read_text())
        assert record["synthetic"] is True
        assert record["n"] == 6
        if rel.endswith("campaign.json"):
            assert record["fair"] == []
            assert record["head"] == record["commit"]
        if rel.endswith("readcon_db_bench.json"):
            assert record["rows"] == []
    assert _saved_digest(repo) == saved


def test_export_writes_each_run_to_one_path(tmp_path: Path) -> None:
    """One source run is exported to one path, and saved records stay put."""
    repo = _isolated_clean_repo(tmp_path)
    saved_fair = (
        repo / "examples" / "benchmarks" / "fair-db30" / "SUPERSEDED.md"
    ).read_bytes()
    saved = _saved_digest(repo)
    out = repo / "build" / "rerun-one-path"
    export = repo / "build" / "rerun-one-path-export"
    done = _run(
        repo,
        "--synthetic",
        "--n",
        "6",
        "--readcon-core-ref",
        "0.23.2",
        "--out",
        str(out),
        "--export",
        str(export),
    )
    output = done.stdout + done.stderr
    assert done.returncode == 0, output
    found: dict[str, list[str]] = {}
    for path in export.rglob("*.json"):
        if path.name not in {"ab.json", "campaign.json", "readcon_db_bench.json"}:
            continue
        data = json.loads(path.read_text())
        rel = path.relative_to(export).as_posix()
        found.setdefault(data["source_run"], []).append(rel)
    assert len(found) == len(OUTPUTS)
    for run, paths in found.items():
        assert paths == [paths[0]], (run, paths)
    assert sorted(path for paths in found.values() for path in paths) == sorted(OUTPUTS)
    fair_runs = [run for run in found if "fair_ase_protocol.py" in run]
    assert len(fair_runs) == 2
    assert found[next(run for run in fair_runs if "--mode all" in run)] == [
        "db33/fair/campaign.json"
    ]
    commit = json.loads((export / "db33/fair/campaign.json").read_text())["commit"]
    for old, new in SUPERSEDED.items():
        assert not (export / old / Path(new).name).is_file()
        note = (export / old / "SUPERSEDED.md").read_text()
        assert new in note
        assert commit in note
    assert (
        repo / "examples" / "benchmarks" / "fair-db30" / "SUPERSEDED.md"
    ).read_bytes() == saved_fair
    assert _saved_digest(repo) == saved

    mismatch = _run(
        repo,
        "--synthetic",
        "--n",
        "6",
        "--readcon-core-ref",
        "0.23.3",
        "--out",
        str(repo / "build" / "rerun-wrong-core"),
        "--export",
        str(repo / "build" / "rerun-wrong-core-export"),
    )
    refused = mismatch.stdout + mismatch.stderr
    assert mismatch.returncode != 0
    assert "0.23.2" in refused
    assert "0.23.3" in refused
    assert "cargo update -p readcon-core" in refused
    assert not (repo / "build" / "rerun-wrong-core-export").exists()


def _bare_and_clone(tmp_path: Path) -> tuple[Path, Path, str]:
    bare = tmp_path / "bare.git"
    init_bare = _git(tmp_path, "init", "--bare", "-b", "main", str(bare))
    assert init_bare.returncode == 0, init_bare.stderr
    core = tmp_path / "core"
    core.mkdir()
    assert _git(core, "init", "-b", "main").returncode == 0
    (core / "readme").write_text("one\n")
    assert _git(core, "add", "readme").returncode == 0
    assert _git(core, "commit", "-m", "one").returncode == 0
    assert _git(core, "remote", "add", "origin", str(bare)).returncode == 0
    pushed = _git(core, "push", "-u", "origin", "main")
    assert pushed.returncode == 0, pushed.stderr
    return bare, core, _git(core, "rev-parse", "HEAD").stdout.strip()


def test_local_tag_only_is_refused(tmp_path: Path, monkeypatch) -> None:
    """A commit tagged only locally does not count as pushed."""
    import rerun_release as rerun

    bare, core, _main_sha = _bare_and_clone(tmp_path)
    assert _git(core, "checkout", "--detach").returncode == 0
    (core / "readme").write_text("local-tag-only\n")
    assert _git(core, "add", "readme").returncode == 0
    assert _git(core, "commit", "-m", "local tag only").returncode == 0
    local_sha = _git(core, "rev-parse", "HEAD").stdout.strip()
    assert _git(core, "tag", "v0.23.9-local", local_sha).returncode == 0
    assert not rerun.core_sha_is_pushed(local_sha, core)
    with pytest.raises(
        SystemExit, match=r"not a crates\.io release or a pushed commit"
    ):
        rerun.require_exportable_core(
            {
                "name": "readcon-core",
                "version": "0.23.3",
                "source": "git+https://github.com/lode-org/readcon-core#" + local_sha,
            },
            "0.23.3",
            core,
        )
    # Also verify the cloned checkout path rejects a local-only tag on an unpushed commit.
    monkeypatch.setattr(rerun, "REPO", tmp_path / "work")
    monkeypatch.setattr(rerun, "CORE_GIT", str(bare))
    dest = rerun.core_checkout()
    assert _git(dest, "checkout", "--detach").returncode == 0
    (dest / "readme").write_text("dest-local\n")
    assert _git(dest, "add", "readme").returncode == 0
    assert _git(dest, "commit", "-m", "dest local").returncode == 0
    dest_sha = _git(dest, "rev-parse", "HEAD").stdout.strip()
    assert _git(dest, "tag", "local-only", dest_sha).returncode == 0
    assert not rerun.core_sha_is_pushed(dest_sha)


def test_pushed_branch_is_accepted(tmp_path: Path, monkeypatch) -> None:
    """A commit on a pushed branch is accepted."""
    import rerun_release as rerun

    bare, core, _main_sha = _bare_and_clone(tmp_path)
    assert _git(core, "checkout", "-b", "feature").returncode == 0
    (core / "readme").write_text("feature\n")
    assert _git(core, "add", "readme").returncode == 0
    assert _git(core, "commit", "-m", "feature").returncode == 0
    feature = _git(core, "rev-parse", "HEAD").stdout.strip()
    assert _git(core, "push", "origin", "feature").returncode == 0
    assert rerun.core_sha_is_pushed(feature, core)

    monkeypatch.setattr(rerun, "REPO", tmp_path / "work")
    monkeypatch.setattr(rerun, "CORE_GIT", str(bare))
    assert rerun.core_sha_is_pushed(feature)


def test_missing_remote_tip_reports_the_actual_cause(tmp_path: Path) -> None:
    """Reachability cannot be checked until a newly advertised tip is local."""
    import rerun_release as rerun

    bare, core, main_sha = _bare_and_clone(tmp_path)
    publisher = tmp_path / "publisher"
    cloned = _git(tmp_path, "clone", str(bare), str(publisher))
    assert cloned.returncode == 0, cloned.stderr
    assert _git(publisher, "checkout", "-b", "feature").returncode == 0
    (publisher / "readme").write_text("remote-only-tip\n")
    assert _git(publisher, "add", "readme").returncode == 0
    assert _git(publisher, "commit", "-m", "remote only tip").returncode == 0
    assert _git(publisher, "push", "origin", "feature").returncode == 0
    assert _git(bare, "symbolic-ref", "HEAD", "refs/heads/feature").returncode == 0
    deleted = _git(publisher, "push", "origin", "--delete", "main")
    assert deleted.returncode == 0, deleted.stderr

    with pytest.raises(SystemExit) as caught:
        rerun.core_sha_is_pushed(main_sha, core)
    message = str(caught.value)
    assert "remote origin tip" in message
    assert "is not present in the local checkout" in message
    assert "not a crates.io release or a pushed commit" not in message


def test_commit_reachable_only_from_pushed_tag_is_accepted(
    tmp_path: Path, monkeypatch
) -> None:
    """A commit reachable only from a pushed tag (not on any branch) is accepted."""
    import rerun_release as rerun

    bare, core, _main_sha = _bare_and_clone(tmp_path)
    assert _git(core, "checkout", "--detach").returncode == 0
    (core / "readme").write_text("ancestor\n")
    assert _git(core, "add", "readme").returncode == 0
    assert _git(core, "commit", "-m", "ancestor").returncode == 0
    ancestor_sha = _git(core, "rev-parse", "HEAD").stdout.strip()
    (core / "readme").write_text("tagged-tip\n")
    assert _git(core, "add", "readme").returncode == 0
    assert _git(core, "commit", "-m", "tagged tip").returncode == 0
    tip_sha = _git(core, "rev-parse", "HEAD").stdout.strip()
    assert _git(core, "tag", "-a", "v0.23.9", "-m", "release", tip_sha).returncode == 0
    assert _git(core, "push", "origin", "v0.23.9").returncode == 0

    assert rerun.core_sha_is_pushed(tip_sha, core)
    assert rerun.core_sha_is_pushed(ancestor_sha, core)

    monkeypatch.setattr(rerun, "REPO", tmp_path / "work")
    monkeypatch.setattr(rerun, "CORE_GIT", str(bare))
    assert rerun.core_sha_is_pushed(ancestor_sha)


def test_cached_checkout_prunes_a_deleted_remote_branch(
    tmp_path: Path, monkeypatch
) -> None:
    """A stale remote-tracking branch must not make a deleted ref look pushed."""
    import rerun_release as rerun

    bare, core, _main_sha = _bare_and_clone(tmp_path)
    assert _git(core, "checkout", "-b", "feature").returncode == 0
    (core / "readme").write_text("feature\n")
    assert _git(core, "add", "readme").returncode == 0
    assert _git(core, "commit", "-m", "feature").returncode == 0
    feature = _git(core, "rev-parse", "HEAD").stdout.strip()
    assert _git(core, "push", "origin", "feature").returncode == 0

    monkeypatch.setattr(rerun, "REPO", tmp_path / "work")
    monkeypatch.setattr(rerun, "CORE_GIT", str(bare))
    assert rerun.core_sha_is_pushed(feature)
    cached = rerun.REPO / "build" / "readcon-core-git"
    assert (
        _git(cached, "show-ref", "--verify", "refs/remotes/origin/feature").returncode
        == 0
    )

    deleted = _git(core, "push", "origin", "--delete", "feature")
    assert deleted.returncode == 0, deleted.stderr
    assert not rerun.core_sha_is_pushed(feature)
    assert (
        _git(cached, "show-ref", "--verify", "refs/remotes/origin/feature").returncode
        != 0
    )


def test_cached_checkout_refreshes_missing_remote_tag_refs(
    tmp_path: Path, monkeypatch
) -> None:
    """Caches made before remote tag tracking was added are repaired."""
    import rerun_release as rerun

    bare, core, _main_sha = _bare_and_clone(tmp_path)
    assert _git(core, "checkout", "--detach").returncode == 0
    (core / "readme").write_text("tagged\n")
    assert _git(core, "add", "readme").returncode == 0
    assert _git(core, "commit", "-m", "tagged").returncode == 0
    tagged = _git(core, "rev-parse", "HEAD").stdout.strip()
    assert _git(core, "tag", "v0.23.9", tagged).returncode == 0
    assert _git(core, "push", "origin", "v0.23.9").returncode == 0

    monkeypatch.setattr(rerun, "REPO", tmp_path / "work")
    monkeypatch.setattr(rerun, "CORE_GIT", str(bare))
    cached = rerun.core_checkout()
    removed = _git(cached, "update-ref", "-d", "refs/remotes/origin/tags/v0.23.9")
    assert removed.returncode == 0, removed.stderr
    assert (
        _git(
            cached, "show-ref", "--verify", "refs/remotes/origin/tags/v0.23.9"
        ).returncode
        != 0
    )
    assert rerun.core_sha_is_pushed(tagged)


def test_no_remotes_is_refused(tmp_path: Path) -> None:
    """A repo with no configured remotes never counts local branches or tags."""
    import rerun_release as rerun

    core = tmp_path / "no-remotes"
    core.mkdir()
    assert _git(core, "init", "-b", "main").returncode == 0
    (core / "readme").write_text("one\n")
    assert _git(core, "add", "readme").returncode == 0
    assert _git(core, "commit", "-m", "one").returncode == 0
    sha = _git(core, "rev-parse", "HEAD").stdout.strip()
    assert _git(core, "tag", "v0.23.9", sha).returncode == 0
    assert _git(core, "update-ref", "refs/remotes/origin/main", sha).returncode == 0
    assert not rerun.core_sha_is_pushed(sha, core)
    with pytest.raises(
        SystemExit, match=r"not a crates\.io release or a pushed commit"
    ):
        rerun.require_exportable_core(
            {
                "name": "readcon-core",
                "version": "0.23.3",
                "source": "git+https://github.com/lode-org/readcon-core#" + sha,
            },
            "0.23.3",
            core,
        )


def _core_repo(path: Path, remote: str) -> str:
    path.mkdir(parents=True)
    init = _git(path, "init", "-b", "main")
    assert init.returncode == 0, init.stderr
    (path / "readme").write_text("one\n")
    added = _git(path, "add", "readme")
    assert added.returncode == 0, added.stderr
    commit = _git(path, "commit", "-m", "one")
    assert commit.returncode == 0, commit.stderr
    origin = _git(path, "remote", "add", "origin", remote)
    assert origin.returncode == 0, origin.stderr
    return _git(path, "rev-parse", "HEAD").stdout.strip()


def test_unpushed_pin_with_unreachable_remote_is_refused_cleanly(
    tmp_path: Path, monkeypatch
) -> None:
    """A cached unpushed commit is refused cleanly with the remote down."""
    import rerun_release as rerun

    monkeypatch.setattr(rerun, "REPO", tmp_path)
    dest = tmp_path / "build" / "readcon-core-git"
    sha = _core_repo(dest, "https://127.0.0.1:9/readcon-core.git")
    assert rerun.core_sha_is_pushed(sha) is False


def test_unreachable_core_remote_is_a_clean_error(tmp_path: Path, monkeypatch) -> None:
    """Cloning a dead remote is a refusal, not an uncaught git error."""
    import rerun_release as rerun

    monkeypatch.setattr(rerun, "REPO", tmp_path)
    monkeypatch.setattr(rerun, "CORE_GIT", "https://127.0.0.1:9/readcon-core.git")
    with pytest.raises(SystemExit, match="could not clone readcon-core"):
        rerun.core_sha_is_pushed("ab" * 20)


def test_dry_run_without_ref_does_not_fetch_or_refuse(
    tmp_path: Path, monkeypatch, capsys: pytest.CaptureFixture[str]
) -> None:
    """``--dry-run`` with no ref prints the plan and does not touch the pin."""
    import rerun_release as rerun

    monkeypatch.setattr(rerun, "require_clean_and_remote", lambda: ("a" * 40, "b" * 40))
    monkeypatch.setattr(rerun, "REPO", tmp_path)
    monkeypatch.setattr(rerun, "CORE_GIT", "https://127.0.0.1:9/readcon-core.git")
    out = tmp_path / "plan"
    pins = (
        {
            "name": "readcon-core",
            "version": "0.23.2",
            "source": "git+https://github.com/lode-org/readcon-core#" + ("ab" * 20),
        },
        {"name": "readcon-core", "version": "0.23.2"},
    )
    for locked in pins:
        monkeypatch.setattr(
            rerun, "lock_readcon_core", lambda locked=locked: dict(locked)
        )
        rerun.main(["--dry-run", "--n", "6", "--out", str(out)])
        captured = capsys.readouterr()
        assert "kind ab.json" in captured.out
        assert "refusing to export" not in captured.out + captured.err
        assert "could not clone" not in captured.out + captured.err
        assert not out.exists()


def test_dry_run_with_a_ref_still_checks_the_pin(tmp_path: Path, monkeypatch) -> None:
    import rerun_release as rerun

    monkeypatch.setattr(rerun, "require_clean_and_remote", lambda: ("a" * 40, "b" * 40))
    monkeypatch.setattr(rerun, "REPO", tmp_path)
    monkeypatch.setattr(
        rerun,
        "lock_readcon_core",
        lambda: {"name": "readcon-core", "version": "0.23.2"},
    )
    with pytest.raises(SystemExit, match="refusing to export"):
        rerun.main(
            [
                "--dry-run",
                "--n",
                "6",
                "--readcon-core-ref",
                "0.23.9",
                "--out",
                str(tmp_path / "plan"),
            ]
        )


def test_registry_pin_accepts_a_git_sha_ref(tmp_path: Path, monkeypatch) -> None:
    """``--readcon-core-ref`` may be the crate git sha, not only the version."""
    import rerun_release as rerun

    sha = "27df3974a8038f567e0d21ae25f404ce32534a7d"
    root = tmp_path / "readcon-core-0.23.2"
    root.mkdir()
    (root / ".cargo_vcs_info.json").write_text(
        json.dumps({"git": {"sha1": sha}, "path_in_vcs": ""}) + "\n"
    )
    calls = {"root": 0}

    def crate_root(_locked: dict[str, str]) -> Path:
        calls["root"] += 1
        return root

    monkeypatch.setattr(rerun, "crate_root", crate_root)
    locked = {
        "name": "readcon-core",
        "version": "0.23.2",
        "source": "registry+https://github.com/rust-lang/crates.io-index",
        "checksum": "abc",
    }
    rerun.require_exportable_core(locked, "0.23.2")
    assert calls["root"] == 0
    rerun.require_exportable_core(locked, sha)
    rerun.require_exportable_core(locked, sha[:12])
    with pytest.raises(SystemExit, match="ab" * 20):
        rerun.require_exportable_core(locked, "ab" * 20)


def test_path_pin_refusal_does_not_contradict_itself() -> None:
    import rerun_release as rerun

    with pytest.raises(SystemExit) as caught:
        rerun.require_exportable_core(
            {"name": "readcon-core", "version": "0.23.99"},
            None,
        )
    text = str(caught.value)
    assert text.count("refusing to export:") == 1
    assert "not 0.23.99" not in text
    assert "(0.23.99)" not in text
    assert "0.23.99" in text
    assert "Path dependencies" in text


def test_main_refuses_git_and_path_pins_before_registry_download(monkeypatch) -> None:
    import rerun_release as rerun

    called = {"urlopen": 0}

    def _no_network(*_args, **_kwargs):
        called["urlopen"] += 1
        raise AssertionError("network must not be used for git/path pins")

    monkeypatch.setattr(rerun.urllib.request, "urlopen", _no_network)
    monkeypatch.setattr(rerun, "require_clean_and_remote", lambda: ("a" * 40, "b" * 40))
    monkeypatch.setattr(
        rerun,
        "core_sha_is_pushed",
        lambda _sha, _repo=None: False,
    )
    pins = (
        {
            "name": "readcon-core",
            "version": "0.23.3",
            "source": "git+https://github.com/lode-org/readcon-core#" + ("1" * 40),
        },
        # A path dependency has no source line in Cargo.lock.
        {"name": "readcon-core", "version": "0.23.3"},
    )
    for locked in pins:
        monkeypatch.setattr(
            rerun,
            "lock_readcon_core",
            lambda locked=locked: dict(locked),
        )
        try:
            rerun.main(["--synthetic", "--n", "6"])
        except SystemExit as exc:
            text = str(exc)
            assert "refusing to export" in text
        else:
            raise AssertionError(f"main accepted pin {locked!r}")
    assert called["urlopen"] == 0


def test_provenance_uses_lock_sha_for_git_source(monkeypatch) -> None:
    import rerun_release as rerun

    locked = {
        "name": "readcon-core",
        "version": "0.23.3",
        "source": "git+https://github.com/lode-org/readcon-core#" + ("f" * 40),
    }
    monkeypatch.setattr(
        rerun,
        "crate_root",
        lambda _locked: (_ for _ in ()).throw(AssertionError("no crate source")),
    )
    monkeypatch.setattr(rerun, "rustc_version", lambda: "rustc 1.88.0")
    monkeypatch.setattr(
        rerun,
        "git_pin_source",
        lambda _sha: "fn campaign() {}\n",
    )
    monkeypatch.setattr(
        rerun,
        "host_facts",
        lambda: {
            "cpu": "x",
            "ncpu": 1,
            "cpuinfo_mhz": 1.0,
            "kernel": "k",
            "family": "6",
            "model": "0",
            "mhz": 1.0,
            "nproc": 1,
        },
    )
    prov = rerun.provenance("a" * 40, "b" * 40, locked)
    assert prov["readcon_core"] == "0.23.3"
    assert prov["readcon_sha"] == "f" * 40
    assert prov["allocator"] == "system"


def test_git_pin_allocator_comes_from_lib_rs(tmp_path: Path, monkeypatch) -> None:
    import rerun_release as rerun

    repo = tmp_path / "build" / "readcon-core-git"
    repo.mkdir(parents=True)
    _git(repo, "init", "-b", "main")
    (repo / "src").mkdir()
    (repo / "src" / "lib.rs").write_text("fn campaign() {}\n")
    _git(repo, "add", "src/lib.rs")
    committed = _git(repo, "commit", "-m", "lib")
    assert committed.returncode == 0, committed.stderr
    sha = _git(repo, "rev-parse", "HEAD").stdout.strip()
    monkeypatch.setattr(rerun, "REPO", tmp_path)
    monkeypatch.setattr(rerun, "rustc_version", lambda: "rustc 1.88.0")
    monkeypatch.setattr(
        rerun,
        "host_facts",
        lambda: {
            "cpu": "x",
            "ncpu": 1,
            "cpuinfo_mhz": 1.0,
            "kernel": "k",
            "family": "6",
            "model": "85",
            "mhz": 1.0,
            "nproc": 1,
        },
    )
    locked = {
        "name": "readcon-core",
        "version": "0.23.3",
        "source": "git+https://github.com/lode-org/readcon-core#" + sha,
    }
    prov = rerun.provenance("a" * 40, "b" * 40, locked)
    assert prov["readcon_sha"] == sha
    assert prov["allocator"] == "system"
    (repo / "src" / "lib.rs").write_text(
        "#[global_allocator]\nstatic G: mimalloc::MiMalloc = mimalloc::MiMalloc;\n"
    )
    _git(repo, "add", "src/lib.rs")
    again = _git(repo, "commit", "-m", "mimalloc")
    assert again.returncode == 0, again.stderr
    sha = _git(repo, "rev-parse", "HEAD").stdout.strip()
    locked["source"] = "git+https://github.com/lode-org/readcon-core#" + sha
    prov = rerun.provenance("a" * 40, "b" * 40, locked)
    assert prov["allocator"] == "mimalloc"


def _synthetic_prov() -> dict[str, object]:
    host = {
        "cpu": "x",
        "ncpu": 1,
        "cpuinfo_mhz": 1.0,
        "kernel": "k",
        "family": "6",
        "model": "85",
        "mhz": 1.0,
        "nproc": 1,
    }
    return {
        "commit": "a" * 40,
        "tree": "b" * 40,
        "readcon_core": "0.23.2",
        "readcon_sha": "c" * 40,
        "rustc": "rustc 1.88.0",
        "python": "Python 3.12.0",
        "allocator": "system",
        "host": host,
    }


def test_export_copies_listed_sidecars_and_refuses_the_rest(tmp_path: Path) -> None:
    import rerun_release as rerun

    root = tmp_path / "out"
    rerun.write_synthetic(root, _synthetic_prov(), 6)
    sidecar = root / "db33" / "db24" / "RESULTS.md"
    sidecar.write_text("# db24\n")
    export = tmp_path / "export"
    rerun.export_records(root, export, "a" * 40)
    assert (export / "db33" / "db24" / "RESULTS.md").read_text() == "# db24\n"
    dropped = root / "db33" / "db24" / "notes.txt"
    dropped.write_text("x\n")
    again = tmp_path / "export-again"
    with pytest.raises(SystemExit, match=r"db33/db24/notes\.txt"):
        rerun.export_records(root, again, "a" * 40)
    assert not again.exists()


def test_export_preflight_refuses_last_step_without_writing_anything(
    tmp_path: Path,
) -> None:
    """A disallowed file in the last step refuses before any export file is written."""
    import rerun_release as rerun

    root = tmp_path / "out"
    rerun.write_synthetic(root, _synthetic_prov(), 6)
    last_step = rerun.STEPS[-1]
    dropped = root / last_step.paper / "notes.txt"
    dropped.write_text("x\n")

    fresh = tmp_path / "export-fresh"
    with pytest.raises(SystemExit, match=re.escape(f"{last_step.paper}/notes.txt")):
        rerun.export_records(root, fresh, "a" * 40)
    assert not fresh.exists()

    existing = tmp_path / "export-existing"
    existing.mkdir()
    sentinel = existing / "keep.txt"
    sentinel.write_text("stay\n")
    with pytest.raises(SystemExit, match=re.escape(f"{last_step.paper}/notes.txt")):
        rerun.export_records(root, existing, "a" * 40)
    assert [p.relative_to(existing).as_posix() for p in existing.rglob("*")] == [
        "keep.txt"
    ]
    assert sentinel.read_text() == "stay\n"


def test_export_preflight_refuses_a_duplicate_copy_target_before_writing(
    tmp_path: Path, monkeypatch
) -> None:
    import rerun_release as rerun

    root = tmp_path / "out"
    rerun.write_synthetic(root, _synthetic_prov(), 6)
    first = replace(rerun.STEPS[0], supersedes=())
    duplicate = replace(first, script="examples/benchmarks/duplicate.py")
    monkeypatch.setattr(rerun, "STEPS", (first, duplicate))
    export = tmp_path / "export"

    with pytest.raises(SystemExit, match=r"would be written twice"):
        rerun.export_records(root, export, "a" * 40)
    assert not export.exists()


def test_export_preflight_refuses_a_duplicate_note_target_before_writing(
    tmp_path: Path, monkeypatch
) -> None:
    import rerun_release as rerun

    root = tmp_path / "out"
    rerun.write_synthetic(root, _synthetic_prov(), 6)
    shared = ("db30/shared",)
    first = replace(rerun.STEPS[0], supersedes=shared)
    second = replace(rerun.STEPS[1], supersedes=shared)
    monkeypatch.setattr(rerun, "STEPS", (first, second))
    export = tmp_path / "export"

    with pytest.raises(SystemExit, match=r"db30/shared/SUPERSEDED\.md.*twice"):
        rerun.export_records(root, export, "a" * 40)
    assert not export.exists()


def test_dirty_tree_names_files_inside_untracked_directories(tmp_path: Path) -> None:
    repo = _isolated_clean_repo(tmp_path)
    nested = repo / "scratch" / "notes.txt"
    nested.parent.mkdir()
    nested.write_text("x\n")
    done = _run(repo, "--dry-run")
    text = done.stdout + done.stderr
    assert done.returncode != 0, text
    assert "scratch/notes.txt" in text


def test_run_command_relativizes_repo_paths(tmp_path: Path) -> None:
    import rerun_release as rerun

    log = tmp_path / "raw.log"
    out = tmp_path / "out"
    out.mkdir(parents=True)
    script = rerun.REPO / "examples" / "benchmarks" / "rerun_release.py"
    rerun.run_command(
        [
            "/usr/bin/python3",
            "-c",
            "print('ok')",
            str(script),
            str(out / "x"),
        ],
        os.environ.copy(),
        log,
        out,
    )
    body = log.read_text()
    assert script.as_posix() not in body
    assert "examples/benchmarks/rerun_release.py" in body
    assert str((out / "x").resolve()) not in body
    assert "/usr/bin/python3" in body


def test_stamp_flattens_host_without_the_unused_key_list(tmp_path: Path) -> None:
    import rerun_release as rerun

    assert not hasattr(rerun, "GEN_VERSION_MAP_BUILD_KEYS")
    assert not hasattr(rerun, "GEN_VERSION_MAP_HOST_KEYS")
    record = tmp_path / "ab.json"
    record.write_text("{}\n")
    host = {
        "cpu": "x",
        "ncpu": 1,
        "cpuinfo_mhz": 1.0,
        "kernel": "k",
        "family": "6",
        "model": "85",
        "mhz": 1.0,
        "nproc": 1,
    }
    prov = {
        "commit": "a" * 40,
        "tree": "b" * 40,
        "readcon_core": "0.23.2",
        "readcon_sha": "c" * 40,
        "rustc": "rustc 1.88.0",
        "python": "Python 3.12.0",
        "allocator": "system",
        "host": host,
    }
    rerun.stamp(record, prov, "examples/benchmarks/db24_drive.py")
    beside = json.loads((tmp_path / "host.json").read_text())
    assert beside["model"] == "85"
    assert beside["family"] == "6"
    assert beside["mhz"] == 1.0
    assert beside["nproc"] == 1
    assert beside["host"]["model"] == "85"


def test_cpu_model_key_is_not_model_name() -> None:
    import rerun_release as rerun

    text = (
        "model name\t: Intel(R) Xeon(R) CPU\n"
        "cpu family\t: 6\n"
        "model\t: 85\n"
        "stepping\t: 4\n"
    )
    assert rerun.cpu_family_and_model(text) == ("6", "85")


def test_shown_arg_prefers_the_output_root(tmp_path: Path, monkeypatch) -> None:
    import rerun_release as rerun

    repo = tmp_path / "repo"
    out = repo / "build" / "out"
    out.mkdir(parents=True)
    monkeypatch.setattr(rerun, "REPO", repo)
    assert rerun._shown_arg(str(out / "db33" / "ab.json"), out) == "./db33/ab.json"
    assert (
        rerun._shown_arg(str(repo / "examples" / "bench_corpus_ops.rs"), out)
        == "examples/bench_corpus_ops.rs"
    )
    assert rerun._shown_arg(str(out), out) == "."


def test_shown_arg_keeps_a_symlink_under_the_repo(tmp_path: Path, monkeypatch) -> None:
    import rerun_release as rerun

    repo = tmp_path / "repo"
    link_dir = repo / "bin"
    link_dir.mkdir(parents=True)
    link = link_dir / "python"
    link.symlink_to("/usr/bin/python3")
    monkeypatch.setattr(rerun, "REPO", repo)
    assert rerun._shown_arg(str(link), None) == "bin/python"


def test_configured_toolchain_prefers_rustup_env(monkeypatch) -> None:
    import rerun_release as rerun

    monkeypatch.setenv("RUSTUP_TOOLCHAIN", "1.88.0")
    assert rerun.configured_toolchain() == "1.88.0"
