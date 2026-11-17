"""Dry-run, refusal, and synthetic provenance tests for the release rerun."""

from __future__ import annotations

import hashlib
import json
import os
import shutil
import subprocess
import sys
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


def test_unpushed_readcon_core_refuses_export(tmp_path: Path) -> None:
    """A git pin that no remote branch or tag contains is not exported."""
    import rerun_release as rerun

    core = tmp_path / "core"
    core.mkdir()
    _git(core, "init", "-b", "main")
    (core / "readme").write_text("one\n")
    _git(core, "add", "readme")
    _git(core, "commit", "-m", "one")
    _git(core, "update-ref", "refs/remotes/origin/main", "HEAD")
    pushed = _git(core, "rev-parse", "HEAD").stdout.strip()
    assert rerun.core_sha_is_pushed(pushed, core)
    tagged = _git(core, "tag", "v0.23.9", pushed)
    assert tagged.returncode == 0, tagged.stderr
    assert rerun.core_sha_is_pushed(pushed, core)
    branch_only = _git(core, "checkout", "-b", "feature")
    assert branch_only.returncode == 0, branch_only.stderr
    (core / "readme").write_text("two\n")
    _git(core, "add", "readme")
    _git(core, "commit", "-m", "two")
    feature = _git(core, "rev-parse", "HEAD").stdout.strip()
    _git(core, "update-ref", "refs/remotes/origin/feature", feature)
    assert not rerun.core_sha_is_pushed(feature, core)
    missing = "b" * 40
    assert not rerun.core_sha_is_pushed(missing, core)
    try:
        try:
            rerun.require_exportable_core(
                {
                    "name": "readcon-core",
                    "version": "0.23.3",
                    "source": "git+https://github.com/lode-org/readcon-core#" + missing,
                },
                "0.23.3",
                core,
            )
        except SystemExit as exc:
            assert "not a crates.io release or a pushed commit" in str(exc)
        else:
            raise AssertionError("unpushed readcon-core was accepted")
    finally:
        _git(core, "checkout", "main")


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


def test_unpushed_pin_is_refused_without_fetching(tmp_path: Path, monkeypatch) -> None:
    """A commit already in the local checkout is refused with the remote down."""
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
    monkeypatch.setattr(rerun, "allocator_in_use", lambda _root: "mimalloc")
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


def test_export_includes_ab_sidecar_files() -> None:
    import rerun_release as rerun

    for name in (
        "RESULTS.md",
        "noise.json",
        "tables.json",
        "levers.json",
        "high_byte.json",
    ):
        assert name in rerun.EXPORT_NAMES


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


def test_configured_toolchain_prefers_rustup_env(monkeypatch) -> None:
    import rerun_release as rerun

    monkeypatch.setenv("RUSTUP_TOOLCHAIN", "1.88.0")
    assert rerun.configured_toolchain() == "1.88.0"
