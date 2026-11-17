#!/usr/bin/env python3
"""Rerun the readcon-db records copied into the CPC paper.

The paper copies these records verbatim under ``paper/cpc/data/db30``,
``db31``, and ``db32``. This entry point runs the producers again, serially,
into a new directory named by the readcon-db commit. It does not edit a
saved record. ``--dry-run`` prints the plan. ``--synthetic`` writes the same
paths with provenance and no timings, for the guard tests.

Producers, and the commands that wrote the saved records:

* ``ab.json`` for db24: ``python examples/benchmarks/db24_drive.py --out DIR``
  with ``READCON_DB24_N`` (the saved runs use 12).
* ``ab.json`` for db25: ``python examples/benchmarks/db25_drive.py --out DIR``.
  The sample count is db24's ``READCON_DB24_N``.
* ``ab.json`` for db26: ``python examples/benchmarks/db26_drive.py``. The
  saved run writes ``bench/db26``. The rerun passes ``--out``.
* ``ab.json`` for db28: ``python examples/benchmarks/db28_drive.py``, which
  writes ``ab.json`` and ``host.json``. The saved run writes ``bench/db28``.
* ``ab.json`` for db29: ``python examples/benchmarks/db29_drive.py``, same
  layout, saved under ``bench/db29``.
* ``readcon_db_bench.json``: concatenate ``resources/test/tiny_cuh2.con`` into
  ``n10.con``, ``n50.con``, ``n100.con``, ``n200.con``, and ``n500.con`` (the
  ladder ``fair_campaign.prep_ladder`` uses), then
  ``cargo run --release --example bench_corpus_ops -- FRAMES OUT [repeats]``.
* ``campaign.json`` for the full fair protocol:
  ``python examples/benchmarks/fair_ase_protocol.py --mode all --out DIR``.
  The saved paper copies are ``db30/fair-db30``, ``db31/fair-db31``, and
  ``db32/fair-db32``. The rerun writes that one run to ``db33/fair`` and a
  ``SUPERSEDED.md`` beside each saved copy.
* ``campaign.json`` for the db8 workload list:
  ``python examples/benchmarks/fair_ase_protocol.py --mode db8 --out DIR``.
  The saved copy is ``db30/db8``. The rerun writes ``db33/db8``.

Each producer has one export path. A new run is ``db33/<name>/``. The saved
``db30``, ``db31``, and ``db32`` records are not rewritten. The export writes
a ``SUPERSEDED.md`` beside each of those paths, naming the new path and the
readcon-db commit.

``n`` is at least 6. Each record and the ``host.json`` beside it carries the
readcon-db commit and tree, the readcon-core version and git sha linked from
``Cargo.lock`` and the crate, rustc, python, the allocator, and the host
cpu, count, MHz, and kernel. The tree must be clean, and ``git branch -r
--contains HEAD`` must list a remote branch. Export also requires the linked
readcon-core to be a crates.io release or a git commit on a remote branch or
tag. ``--readcon-core-ref`` names the version or sha the lock must match.
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import os
import platform
import shutil
import subprocess
import sys
import tarfile
import urllib.request
from dataclasses import dataclass
from datetime import datetime, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
FIXTURE = REPO / "resources" / "test" / "tiny_cuh2.con"
LADDER = (10, 50, 100, 200, 500)
SAVED_PREFIXES = (
    "bench",
    "examples/benchmarks/fair-db30",
    "examples/benchmarks/fair-db31",
    "examples/benchmarks/fair-db32",
    "examples/benchmarks/results",
)
EXPORT_NAMES = (
    "ab.json",
    "campaign.json",
    "readcon_db_bench.json",
    "host.json",
    "RESULTS.md",
    "noise.json",
    "tables.json",
    "levers.json",
    "high_byte.json",
    "raw.log",
    "campaign.log",
)
SHA1 = 40
CORE_GIT = "https://github.com/lode-org/readcon-core.git"


@dataclass(frozen=True)
class Step:
    """One producer invocation and the single paper directory that receives it."""

    kind: str
    script: str
    paper: str
    record_name: str
    how: str
    sample_env: str
    flags: tuple[str, ...] = ()
    supersedes: tuple[str, ...] = ()


STEPS: tuple[Step, ...] = (
    Step(
        "ab.json",
        "examples/benchmarks/db24_drive.py",
        "db33/db24",
        "ab.json",
        "drive",
        "READCON_DB24_N",
        supersedes=("db31/db24",),
    ),
    Step(
        "ab.json",
        "examples/benchmarks/db25_drive.py",
        "db33/db25",
        "ab.json",
        "drive",
        "READCON_DB24_N",
        supersedes=("db31/db25",),
    ),
    Step(
        "ab.json",
        "examples/benchmarks/db26_drive.py",
        "db33/db26",
        "ab.json",
        "drive",
        "READCON_DB26_N",
        supersedes=("db31/db26",),
    ),
    Step(
        "ab.json",
        "examples/benchmarks/db28_drive.py",
        "db33/db28",
        "ab.json",
        "drive",
        "READCON_DB28_N",
        supersedes=("db31/db28",),
    ),
    Step(
        "ab.json",
        "examples/benchmarks/db29_drive.py",
        "db33/db29",
        "ab.json",
        "drive",
        "READCON_DB29_N",
        supersedes=("db31/db29",),
    ),
    Step(
        "readcon_db_bench.json",
        "examples/bench_corpus_ops.rs",
        "db33/corpus-ops",
        "readcon_db_bench.json",
        "corpus",
        "",
        supersedes=("db31/corpus-ops",),
    ),
    Step(
        "campaign.json",
        "examples/benchmarks/fair_ase_protocol.py",
        "db33/fair",
        "campaign.json",
        "fair",
        "READCON_DB_N_SAMPLES",
        ("--mode", "all"),
        ("db30/fair-db30", "db31/fair-db31", "db32/fair-db32"),
    ),
    Step(
        "campaign.json",
        "examples/benchmarks/fair_ase_protocol.py",
        "db33/db8",
        "campaign.json",
        "fair",
        "READCON_DB_N_SAMPLES",
        ("--mode", "db8"),
        ("db30/db8",),
    ),
)


def git(*args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", "-C", str(REPO), *args],
        capture_output=True,
        text=True,
        check=False,
    )


def require_clean_and_remote() -> tuple[str, str]:
    """Refuse a dirty tree or a commit no remote branch contains."""
    status = git("status", "--porcelain", "--untracked-files=all")
    if status.returncode != 0:
        raise SystemExit(status.stderr.strip() or "git status failed")
    if status.stdout.strip():
        raise SystemExit(
            "refusing release rerun on a dirty tree:\n" + status.stdout.strip()
        )
    head = git("rev-parse", "HEAD")
    if head.returncode != 0:
        raise SystemExit(head.stderr.strip() or "git rev-parse HEAD failed")
    sha = head.stdout.strip()
    branches = git("branch", "-r", "--contains", "HEAD")
    if branches.returncode != 0 or not branches.stdout.strip():
        raise SystemExit(
            f"refusing release rerun: {sha} is not on any remote branch "
            "(git branch -r --contains HEAD returned none)"
        )
    tree = git("rev-parse", "HEAD^{tree}")
    if tree.returncode != 0:
        raise SystemExit(tree.stderr.strip() or "git rev-parse tree failed")
    return sha, tree.stdout.strip()


def repo_relative(path: Path) -> str | None:
    try:
        return path.resolve().relative_to(REPO.resolve()).as_posix()
    except ValueError:
        return None


def refuse_saved(path: Path) -> None:
    rel = repo_relative(path)
    if rel is None:
        return
    for prefix in SAVED_PREFIXES:
        if rel == prefix or rel.startswith(prefix + "/"):
            raise SystemExit(f"refusing to write a saved record directory: {rel}")


def refuse_existing(path: Path) -> None:
    if path.is_file():
        raise SystemExit(f"refusing to edit an existing record: {path}")


def default_out(commit: str) -> Path:
    stamp = datetime.now(timezone.utc).strftime("%Y%m%dT%H%M%SZ")
    return REPO / "build" / "rerun" / commit / stamp


def child_env(step: Step, n: int) -> dict[str, str]:
    env = os.environ.copy()
    env["READCON_RERUN_RELEASE"] = "1"
    env.pop("READCON_DB_N_SAMPLES", None)
    env["OMP_NUM_THREADS"] = "1"
    env["OPENBLAS_NUM_THREADS"] = "1"
    env["MKL_NUM_THREADS"] = "1"
    env["NUMEXPR_NUM_THREADS"] = "1"
    if step.sample_env:
        env[step.sample_env] = str(n)
    return env


def mode_of(step: Step) -> str:
    if "--mode" not in step.flags:
        return "all"
    return step.flags[step.flags.index("--mode") + 1]


def work_dir(root: Path, step: Step) -> Path:
    if step.how == "corpus":
        return root / "work" / "corpus-frames"
    return root / "work" / f"fair-{mode_of(step)}"


def python_argv(step: Step, dest: Path) -> list[str]:
    return [sys.executable, str(REPO / step.script), *step.flags, "--out", str(dest)]


def corpus_argv(frames: Path, out_json: Path, n: int) -> list[str]:
    return [
        "cargo",
        "run",
        "--release",
        "--locked",
        "--example",
        "bench_corpus_ops",
        "--",
        str(frames),
        str(out_json),
        str(n),
    ]


def source_run(step: Step) -> str:
    flags = " ".join(step.flags)
    return f"{step.script} {flags}".strip()


def record_rel(step: Step) -> str:
    return f"{step.paper}/{step.record_name}"


def command_line(step: Step, root: Path, n: int) -> str:
    if step.how == "corpus":
        frames = work_dir(root, step)
        out_json = root / step.paper / step.record_name
        argv = corpus_argv(frames, out_json, n)
        prep = "concatenate resources/test/tiny_cuh2.con into " + " ".join(
            f"n{count}.con" for count in LADDER
        )
        return f"{step.script} :: {prep} && " + " ".join(argv)
    if step.how == "fair":
        dest = work_dir(root, step)
    else:
        dest = root / step.paper
    env = f"{step.sample_env}={n} READCON_RERUN_RELEASE=1"
    return f"{step.script} :: {env} " + " ".join(python_argv(step, dest))


def require_one_export_path(steps: tuple[Step, ...] = STEPS) -> None:
    """Each run has one destination, and no two runs share it."""
    seen_path: dict[str, str] = {}
    seen_run: dict[str, str] = {}
    seen_old: set[str] = set()
    for step in steps:
        run = source_run(step)
        rel = record_rel(step)
        if rel in seen_path:
            raise SystemExit(
                f"refusing to export: {seen_path[rel]} and {run} both write {rel}"
            )
        if run in seen_run:
            raise SystemExit(
                f"refusing to export: {run} writes {seen_run[run]} and {rel}"
            )
        seen_path[rel] = run
        seen_run[run] = rel
        if step.paper in step.supersedes:
            raise SystemExit(f"refusing to export {rel} onto a path it supersedes")
        for old in step.supersedes:
            if old in seen_old:
                raise SystemExit(f"refusing to export: {old} is named twice")
            seen_old.add(old)
            old_rel = f"{old}/{step.record_name}"
            if old_rel in seen_path:
                raise SystemExit(f"refusing to export a second run to {old_rel}")


def print_plan(root: Path, export: Path, n: int) -> None:
    require_one_export_path()
    print(f"n {n}")
    print(f"out {root}")
    print(f"export {export}")
    print("kind host.json")
    for step in STEPS:
        rel = record_rel(step)
        print(f"kind {step.kind}")
        print("producer " + command_line(step, root, n))
        print(f"output {rel}")
        print(f"output {step.paper}/host.json")
        print(f"log {step.paper}/raw.log")
        print(f"export {rel}")
        print(f"export {step.paper}/host.json")
        for old in step.supersedes:
            print(f"supersede {old} -> {rel}")


def cpu_model() -> str:
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        return ""
    return ""


def cpu_family_and_model(cpuinfo: str | None = None) -> tuple[str, str]:
    """Family and model. The key is ``model``, not ``model name``."""
    if cpuinfo is None:
        try:
            cpuinfo = Path("/proc/cpuinfo").read_text()
        except OSError:
            return "", ""
    family = ""
    model = ""
    for line in cpuinfo.splitlines():
        if ":" not in line:
            continue
        key, value = line.split(":", 1)
        key = key.strip()
        if key == "cpu family":
            family = value.strip()
        elif key == "model":
            model = value.strip()
        if family and model:
            break
    return family, model


def cpu_mhz() -> float:
    try:
        lines = Path("/proc/cpuinfo").read_text().splitlines()
    except OSError:
        lines = []
    for line in lines:
        if line.lower().startswith("cpu mhz"):
            return float(line.split(":", 1)[1].strip())
    proc = subprocess.run(["lscpu"], capture_output=True, text=True, check=False)
    for line in proc.stdout.splitlines():
        if "MHz" not in line or ":" not in line:
            continue
        token = line.split(":", 1)[1].strip().split()[0]
        try:
            return float(token)
        except ValueError:
            continue
    raise SystemExit("host cpuinfo has no MHz")


def host_facts() -> dict[str, object]:
    count = os.cpu_count()
    if not count:
        raise SystemExit("host cpu count is missing")
    mhz = cpu_mhz()
    family, model = cpu_family_and_model()
    return {
        "cpu": cpu_model(),
        "ncpu": count,
        "cpuinfo_mhz": mhz,
        "kernel": platform.release(),
        "family": family,
        "model": model,
        "mhz": mhz,
        "nproc": count,
    }


def lock_readcon_core() -> dict[str, str]:
    fields: dict[str, str] = {}
    for block in (REPO / "Cargo.lock").read_text().split("[[package]]"):
        parsed: dict[str, str] = {}
        for line in block.splitlines():
            if " = " not in line:
                continue
            key, value = line.split(" = ", 1)
            parsed[key.strip()] = value.strip().strip('"')
        if parsed.get("name") == "readcon-core":
            fields = parsed
            break
    if not fields.get("version"):
        raise SystemExit("Cargo.lock has no readcon-core package")
    return fields


def registry_crate(version: str) -> Path | None:
    home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    matches = sorted((home / "registry" / "src").glob(f"*/readcon-core-{version}"))
    return matches[-1] if matches else None


def download_crate(version: str, checksum: str) -> Path:
    cache = REPO / "build" / "crate-src"
    root = cache / f"readcon-core-{version}"
    if (root / ".cargo_vcs_info.json").is_file():
        return root
    url = f"https://static.crates.io/crates/readcon-core/readcon-core-{version}.crate"
    with urllib.request.urlopen(url, timeout=60) as response:
        blob = response.read()
    digest = hashlib.sha256(blob).hexdigest()
    if checksum and digest != checksum:
        raise SystemExit(
            "readcon-core crate checksum does not match Cargo.lock: "
            f"{digest} != {checksum}"
        )
    cache.mkdir(parents=True, exist_ok=True)
    with tarfile.open(fileobj=io.BytesIO(blob), mode="r:gz") as archive:
        archive.extractall(cache, filter="data")
    if not root.is_dir():
        raise SystemExit(f"crate archive did not contain {root.name}")
    return root


def crate_root(locked: dict[str, str]) -> Path:
    source = locked.get("source", "")
    if not source.startswith("registry+"):
        raise SystemExit(
            "registry crate source inspection was requested for a non-registry source"
        )
    version = locked["version"]
    found = registry_crate(version)
    if found is not None and (found / ".cargo_vcs_info.json").is_file():
        return found
    return download_crate(version, locked.get("checksum", ""))


def git_sha_of_crate(root: Path) -> str:
    info = json.loads((root / ".cargo_vcs_info.json").read_text())
    git_info = info.get("git") if isinstance(info.get("git"), dict) else {}
    sha = git_info.get("sha1") or info.get("sha1") or ""
    if not isinstance(sha, str):
        sha = ""
    sha = sha.strip().lower()
    if len(sha) != SHA1 or any(ch not in "0123456789abcdef" for ch in sha):
        raise SystemExit(f"readcon-core crate at {root} has no git sha")
    return sha


def lock_source_sha(locked: dict[str, str] | None = None) -> str:
    """A git source in Cargo.lock names the revision that was linked."""
    source = (locked or lock_readcon_core()).get("source", "")
    if "#" not in source or not source.startswith("git+"):
        return ""
    sha = source.rsplit("#", 1)[-1].split("?")[0].strip().lower()
    if len(sha) == SHA1 and all(ch in "0123456789abcdef" for ch in sha):
        return sha
    return ""


def allocator_from_source(text: str) -> str:
    """The allocator named by ``src/lib.rs``.

    The rlib, the cdylib, and ``bench_corpus_ops`` link readcon-core without
    ``static_system_alloc``. That feature is the static archive only. When
    the crate installs mimalloc as the global allocator, that is the
    allocator in use.
    """
    if "mimalloc::MiMalloc" in text and "global_allocator" in text:
        return "mimalloc"
    return "system"


def allocator_in_use(root: Path) -> str:
    return allocator_from_source((root / "src" / "lib.rs").read_text())


def git_pin_source(sha: str) -> str:
    """``src/lib.rs`` at ``sha`` from the local core checkout or cargo's cache."""
    checkout = REPO / "build" / "readcon-core-git"
    if (checkout / ".git").exists():
        shown = subprocess.run(
            ["git", "-C", str(checkout), "show", f"{sha}:src/lib.rs"],
            capture_output=True,
            text=True,
            check=False,
        )
        if shown.returncode == 0:
            return shown.stdout
    home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    checkouts = home / "git" / "checkouts"
    if checkouts.is_dir():
        for root in sorted(checkouts.glob("readcon-core-*/*")):
            lib = root / "src" / "lib.rs"
            if not lib.is_file():
                continue
            head = subprocess.run(
                ["git", "-C", str(root), "rev-parse", "HEAD"],
                capture_output=True,
                text=True,
                check=False,
            )
            if head.returncode == 0 and head.stdout.strip().lower() == sha.lower():
                return lib.read_text()
    raise SystemExit(
        "refusing to stamp provenance: no local readcon-core checkout "
        f"contains src/lib.rs at {sha}"
    )


def ref_matches(version: str, sha: str, ref: str) -> bool:
    if ref == version or ref == sha:
        return True
    return bool(
        7 <= len(ref) < SHA1
        and sha.startswith(ref)
        and all(ch in "0123456789abcdef" for ch in ref)
    )


def _git_lines(checkout: Path, *args: str) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        ["git", "-C", str(checkout), *args],
        capture_output=True,
        text=True,
        check=False,
    )


def _object_exists(checkout: Path, sha: str) -> bool:
    found = _git_lines(checkout, "cat-file", "-e", f"{sha}^{{commit}}")
    return found.returncode == 0


def _configured_remotes(checkout: Path) -> list[str]:
    remotes = _git_lines(checkout, "remote")
    if remotes.returncode != 0:
        return []
    return [line.strip() for line in remotes.stdout.splitlines() if line.strip()]


def _ls_remote_refs(checkout: Path, remote: str) -> dict[str, str]:
    env = os.environ.copy()
    env["GIT_TERMINAL_PROMPT"] = "0"
    proc = subprocess.run(
        ["git", "-C", str(checkout), "ls-remote", "--heads", "--tags", remote],
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    if proc.returncode != 0:
        raise SystemExit(
            "refusing to export: could not query remote "
            f"{remote} for readcon-core "
            f"(git ls-remote failed with status {proc.returncode})"
        )
    refs: dict[str, str] = {}
    for line in proc.stdout.splitlines():
        if "\t" not in line:
            continue
        oid, refname = line.split("\t", 1)
        oid = oid.strip().lower()
        refname = refname.strip()
        if refname.endswith("^{}"):
            refs[refname[:-3]] = oid
        elif refname not in refs:
            refs[refname] = oid
    return refs


def _remote_contains(checkout: Path, sha: str) -> bool:
    """True when a remote branch or remote tag on a configured remote contains ``sha``."""
    remotes = _configured_remotes(checkout)
    if not remotes:
        return False
    target = sha.strip().lower()
    has_target = _object_exists(checkout, target)
    for remote in remotes:
        refs = _ls_remote_refs(checkout, remote)
        for tip in set(refs.values()):
            if tip == target:
                return True
            if has_target and _object_exists(checkout, tip):
                anc = _git_lines(checkout, "merge-base", "--is-ancestor", target, tip)
                if anc.returncode == 0:
                    return True
    return False


def _remote_tracking_contains(checkout: Path, sha: str) -> bool:
    """True when fetched remote-tracking refs (branches or remote tags) contain ``sha``."""
    remotes = _configured_remotes(checkout)
    if not remotes:
        return False
    for remote in remotes:
        refs = _git_lines(
            checkout, "for-each-ref", "--contains", sha, f"refs/remotes/{remote}/"
        )
        if refs.returncode == 0 and refs.stdout.strip():
            return True
    return False


def core_checkout() -> Path:
    """Clone or fetch readcon-core. A network failure is a clean refusal."""
    dest = REPO / "build" / "readcon-core-git"
    dest.parent.mkdir(parents=True, exist_ok=True)
    env = os.environ.copy()
    env["GIT_TERMINAL_PROMPT"] = "0"
    if not (dest / ".git").exists():
        argv = [
            "git",
            "clone",
            "--quiet",
            "--no-tags",
            "-c",
            "remote.origin.fetch=+refs/tags/*:refs/remotes/origin/tags/*",
            CORE_GIT,
            str(dest),
        ]
        action = "clone"
    else:
        argv = [
            "git",
            "-C",
            str(dest),
            "fetch",
            "--quiet",
            "--no-tags",
            "--prune",
            "origin",
            "+refs/heads/*:refs/remotes/origin/*",
            "+refs/tags/*:refs/remotes/origin/tags/*",
        ]
        action = "fetch"
    try:
        subprocess.check_call(argv, env=env)
    except subprocess.CalledProcessError as exc:
        raise SystemExit(
            "refusing to export: could not "
            f"{action} readcon-core to verify the pin is pushed "
            f"(git {action} failed with status {exc.returncode})"
        ) from exc
    return dest


def core_sha_is_pushed(sha: str, repo: Path | None = None) -> bool:
    """True when ``sha`` is on a remote branch or remote tag of readcon-core.

    When ``repo`` is given, its configured remotes are queried directly via
    ``git ls-remote``. Otherwise, a commit already in the local checkout is
    judged from fetched remote-tracking refs under ``refs/remotes/*`` without
    hitting the network; fetching is only for a sha the checkout does not have
    yet, and a fetch or clone failure is a clean refusal.
    """
    if repo is not None:
        return _remote_contains(repo, sha)
    dest = REPO / "build" / "readcon-core-git"
    if (dest / ".git").exists() and _object_exists(dest, sha):
        return _remote_tracking_contains(dest, sha)
    core_checkout()
    return _remote_tracking_contains(dest, sha)


def pin_instructions(version: str) -> str:
    """How to point Cargo at a release or a pushed commit."""
    return (
        f'The pin is `readcon-core = "={version}"` in Cargo.toml. '
        'For a crates.io release, set `readcon-core = "=<version>"` and run '
        "`cargo update -p readcon-core --precise <version>`. "
        "For a pushed commit, set "
        '`readcon-core = { git = "https://github.com/lode-org/readcon-core", '
        'rev = "<sha>" }` and run `cargo update -p readcon-core`. '
        "Export accepts that registry source, or a git rev on a remote "
        "branch or remote tag."
    )


def pin_change(version: str, sha: str, ref: str) -> str:
    """How to point the rerun at ``ref`` instead of the lock's pin."""
    shown = sha or version
    return (
        f"refusing to export: Cargo.lock links readcon-core {version} ({shown}), "
        f"not {ref}. {pin_instructions(version)}"
    )


def _looks_like_git_sha(ref: str) -> bool:
    text = ref.lower()
    return 7 <= len(text) <= SHA1 and all(ch in "0123456789abcdef" for ch in text)


def linked_sha(locked: dict[str, str], ref: str | None) -> str:
    """Revision compared with ``--readcon-core-ref``.

    The help text accepts a version or a git sha. A registry pin's git sha
    is the one in the crate, and only a sha-shaped ref needs that file.
    Matching the version does not download the crate.
    """
    source = locked.get("source", "")
    version = locked["version"]
    if source.startswith("git+"):
        return lock_source_sha(locked)
    if (
        source.startswith("registry+")
        and ref
        and ref != version
        and _looks_like_git_sha(ref)
    ):
        return git_sha_of_crate(crate_root(locked))
    return version


def require_exportable_core(
    locked: dict[str, str],
    ref: str | None,
    repo: Path | None = None,
) -> None:
    """Refuse a link that is not a release or a pushed commit, or not ``ref``."""
    source = locked.get("source", "")
    version = locked["version"]
    sha = linked_sha(locked, ref)
    if ref and not ref_matches(version, sha, ref):
        raise SystemExit(pin_change(version, sha, ref))
    if source.startswith("registry+"):
        return
    if source.startswith("git+"):
        if not sha:
            raise SystemExit("Cargo.lock readcon-core git source has no pinned sha")
        if core_sha_is_pushed(sha, repo):
            return
        shown = source or "a path"
        raise SystemExit(
            f"refusing to export: linked readcon-core {version} ({sha}) from {shown} "
            "is not a crates.io release or a pushed commit. "
            + pin_instructions(version)
        )
    if source.startswith(("path+", "path:")) or source == "":
        shown = source or "a path"
        raise SystemExit(
            f"refusing to export: linked readcon-core {version} from {shown}. "
            "Path dependencies are not exportable release inputs. "
            + pin_instructions(version)
        )
    shown = source or "a path"
    raise SystemExit(
        f"refusing to export: linked readcon-core {version} from {shown}. "
        "Only registry releases and pushed git pins are exportable."
    )


def configured_toolchain() -> str | None:
    env_toolchain = os.environ.get("RUSTUP_TOOLCHAIN")
    if env_toolchain:
        return env_toolchain.strip() or None
    toml = REPO / "rust-toolchain.toml"
    if toml.is_file():
        for line in toml.read_text().splitlines():
            line = line.strip()
            if line.startswith("channel") and "=" in line:
                return line.split("=", 1)[1].strip().strip('"').strip("'")
    text = REPO / "rust-toolchain"
    if text.is_file():
        value = text.read_text().strip()
        return value or None
    return None


def rustc_version() -> str:
    toolchain = configured_toolchain()
    if toolchain:
        return subprocess.check_output(
            ["rustc", f"+{toolchain}", "--version"], text=True
        ).strip()
    return subprocess.check_output(["rustc", "--version"], text=True).strip()


def provenance(commit: str, tree: str, locked: dict[str, str]) -> dict[str, object]:
    source = locked.get("source", "")
    if source.startswith("registry+"):
        root = crate_root(locked)
        sha = git_sha_of_crate(root)
        allocator = allocator_in_use(root)
    elif source.startswith("git+"):
        sha = lock_source_sha(locked)
        if not sha:
            raise SystemExit("Cargo.lock readcon-core git source has no pinned sha")
        allocator = allocator_from_source(git_pin_source(sha))
    else:
        raise SystemExit(
            f"refusing to stamp provenance for non-exportable readcon-core source: {source or 'path'}"
        )
    rustc = rustc_version()
    return {
        "commit": commit,
        "tree": tree,
        "readcon_core": locked["version"],
        "readcon_sha": sha,
        "rustc": rustc,
        "python": f"Python {platform.python_version()}",
        "allocator": allocator,
        "host": host_facts(),
    }


def stamp(record_path: Path, prov: dict[str, object], source: str) -> None:
    data = json.loads(record_path.read_text())
    if not isinstance(data, dict):
        raise SystemExit(f"{record_path} is not a JSON object")
    host = data.get("host")
    if not isinstance(host, dict):
        host = {}
    host.update(prov["host"])
    data.update(
        {
            "commit": prov["commit"],
            "tree": prov["tree"],
            "readcon_core": prov["readcon_core"],
            "readcon_sha": prov["readcon_sha"],
            "rustc": prov["rustc"],
            "python": prov["python"],
            "allocator": prov["allocator"],
            "host": host,
            "provenance": prov,
            "source_run": source,
        }
    )
    if record_path.name == "campaign.json":
        data["head"] = prov["commit"]
    record_path.write_text(json.dumps(data, indent=2) + "\n")
    beside = record_path.parent / "host.json"
    previous: dict[str, object] = {}
    if beside.is_file():
        loaded = json.loads(beside.read_text())
        if isinstance(loaded, dict):
            previous = loaded
    previous.update(
        {
            "commit": prov["commit"],
            "tree": prov["tree"],
            "readcon_core": prov["readcon_core"],
            "readcon_sha": prov["readcon_sha"],
            "rustc": prov["rustc"],
            "python": prov["python"],
            "allocator": prov["allocator"],
            "host": prov["host"],
        }
    )
    previous.update(prov["host"])
    beside.write_text(json.dumps(previous, indent=2) + "\n")


def _shown_arg(text: str, root: Path | None) -> str:
    """Show ``text`` relative to the longer of the output root and the repo.

    The output root is checked before the repository when it is inside the
    repository. The string is not resolved, so a symlink under the prefix
    stays a relative path.
    """
    if not text:
        return text
    prefixes: list[tuple[str, str]] = []
    if root is not None:
        prefixes.append((str(root.resolve()), "root"))
    prefixes.append((str(REPO.resolve()), "repo"))
    prefixes.sort(key=lambda item: (len(item[0]), item[1] == "root"), reverse=True)
    for prefix, kind in prefixes:
        if text == prefix:
            return "."
        head = prefix + os.sep
        if text.startswith(head):
            rel = text[len(head) :]
            if kind == "root":
                return f"./{rel}"
            return rel
    return text


def run_command(
    argv: list[str], env: dict[str, str], log_path: Path, root: Path | None = None
) -> str:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    proc = subprocess.run(
        argv,
        cwd=REPO,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    shown = [_shown_arg(arg, root) for arg in argv]
    body = "command " + " ".join(shown) + "\n" + proc.stdout + proc.stderr
    if not body.endswith("\n"):
        body += "\n"
    body += f"exit {proc.returncode}\n"
    log_path.write_text(body)
    if proc.returncode != 0:
        tail = body[-2000:]
        raise SystemExit(
            f"command failed ({proc.returncode}): {' '.join(argv)}\n{tail}"
        )
    return body


def prepare_frames(dest: Path) -> None:
    text = FIXTURE.read_text()
    dest.mkdir(parents=True, exist_ok=True)
    for count in LADDER:
        (dest / f"n{count}.con").write_text(text * count)


def publish_tree(src: Path, dest: Path, raw: str) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    if src.resolve() == dest.resolve():
        (dest / "raw.log").write_text(raw)
        return
    for item in src.iterdir():
        if item.name == "work" or not item.is_file():
            continue
        shutil.copy2(item, dest / item.name)
    (dest / "raw.log").write_text(raw)


def write_synthetic(root: Path, prov: dict[str, object], n: int) -> None:
    require_one_export_path()
    for step in STEPS:
        body: dict[str, object] = {
            "synthetic": True,
            "n": n,
            "commit": prov["commit"],
            "tree": prov["tree"],
            "source_run": source_run(step),
        }
        if step.record_name == "campaign.json":
            body.update(
                {
                    "head": prov["commit"],
                    "published_db_v2": {},
                    "local_paper_harness": {},
                    "fair": [],
                    "settings": {},
                }
            )
        elif step.record_name == "readcon_db_bench.json":
            body["rows"] = []
        directory = root / step.paper
        directory.mkdir(parents=True, exist_ok=True)
        record = directory / step.record_name
        refuse_existing(record)
        record.write_text(json.dumps(body, indent=2) + "\n")
        raw = "synthetic\n" + command_line(step, root, n) + "\n"
        (directory / "raw.log").write_text(raw)
        stamp(record, prov, source_run(step))


def run_measurements(root: Path, prov: dict[str, object], n: int) -> None:
    require_one_export_path()
    for step in STEPS:
        record = root / step.paper / step.record_name
        refuse_existing(record)
        env = child_env(step, n)
        if step.how == "corpus":
            frames = work_dir(root, step)
            prepare_frames(frames)
            record.parent.mkdir(parents=True, exist_ok=True)
            run_command(
                corpus_argv(frames, record, n), env, record.parent / "raw.log", root
            )
            if not record.is_file():
                raise SystemExit(f"corpus producer wrote no {record}")
            stamp(record, prov, source_run(step))
            continue
        if step.how == "fair":
            work = work_dir(root, step)
            work.mkdir(parents=True, exist_ok=True)
            raw = run_command(python_argv(step, work), env, work / "raw.log", root)
            produced = work / step.record_name
            if not produced.is_file():
                raise SystemExit(f"fair producer wrote no {produced}")
            publish_tree(work, record.parent, raw)
            stamp(record, prov, source_run(step))
            continue
        dest = record.parent
        dest.mkdir(parents=True, exist_ok=True)
        raw = run_command(python_argv(step, dest), env, dest / "raw.log", root)
        if not record.is_file():
            raise SystemExit(f"producer wrote no {record}")
        if not (dest / "raw.log").is_file():
            (dest / "raw.log").write_text(raw)
        stamp(record, prov, source_run(step))


def superseded_note(old: str, new: str, commit: str) -> str:
    return (
        f"Replacement: {new}\n"
        "\n"
        f"The record in `{old}` is unchanged. "
        f"The rerun on `{commit}` is in `{new}`.\n"
    )


def export_records(root: Path, export: Path, commit: str) -> None:
    refuse_saved(export)
    require_one_export_path()
    written: list[str] = []
    for step in STEPS:
        rel = record_rel(step)
        src = root / step.paper
        extras = sorted(
            item.name
            for item in src.iterdir()
            if item.is_file() and item.name not in EXPORT_NAMES
        )
        if extras:
            listed = ", ".join(f"{step.paper}/{name}" for name in extras)
            raise SystemExit(
                f"refusing to export: {listed} would be dropped; "
                "add the name to EXPORT_NAMES"
            )
        dest = export / step.paper
        dest.mkdir(parents=True, exist_ok=True)
        record = dest / step.record_name
        refuse_existing(record)
        for name in EXPORT_NAMES:
            path = src / name
            if path.is_file():
                target = dest / name
                refuse_existing(target)
                shutil.copy2(path, target)
        if not record.is_file():
            raise SystemExit(f"export is missing {rel}")
        written.append(rel)
        for old in step.supersedes:
            old_record = export / old / step.record_name
            if old_record.is_file():
                raise SystemExit(f"refusing to leave a second copy at {old_record}")
            note = export / old / "SUPERSEDED.md"
            refuse_existing(note)
            note.parent.mkdir(parents=True, exist_ok=True)
            note.write_text(superseded_note(old, rel, commit))
    if len(written) != len(set(written)):
        raise SystemExit(
            "refusing to export: one run was written to more than one path"
        )


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--synthetic", action="store_true")
    parser.add_argument("--n", type=int, default=12)
    parser.add_argument("--out", type=Path, default=None)
    parser.add_argument("--export", type=Path, default=None)
    parser.add_argument(
        "--readcon-core-ref",
        default=None,
        help=(
            "version or git sha the Cargo.lock readcon-core pin must match; "
            "omit to use the lock's version"
        ),
    )
    return parser.parse_args(argv)


def main(argv: list[str] | None = None) -> None:
    args = parse_args(sys.argv[1:] if argv is None else argv)
    if args.dry_run and args.synthetic:
        raise SystemExit("pass either --dry-run or --synthetic")
    if args.n < 6:
        raise SystemExit(f"n must be at least 6, not {args.n}")
    commit, tree = require_clean_and_remote()
    root = args.out if args.out is not None else default_out(commit)
    if not root.is_absolute():
        root = REPO / root
    export = args.export if args.export is not None else root / "export"
    if not export.is_absolute():
        export = REPO / export
    refuse_saved(root)
    refuse_saved(export)
    for step in STEPS:
        refuse_saved(root / step.paper)
    if args.dry_run and not args.readcon_core_ref:
        print_plan(root, export, args.n)
        return
    locked = lock_readcon_core()
    require_exportable_core(locked, args.readcon_core_ref)
    prov = provenance(commit, tree, locked)
    if prov["commit"] != commit or prov["tree"] != tree:
        raise SystemExit("provenance commit does not match HEAD")
    if args.dry_run:
        print(f"readcon-core {prov['readcon_core']} {prov['readcon_sha']}")
        print_plan(root, export, args.n)
        return
    if args.synthetic:
        write_synthetic(root, prov, args.n)
    else:
        run_measurements(root, prov, args.n)
    export_records(root, export, commit)
    print(f"wrote {export}")


if __name__ == "__main__":
    main()
