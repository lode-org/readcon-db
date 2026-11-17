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
* ``campaign.json``: ``python examples/benchmarks/fair_ase_protocol.py
  --mode all --out DIR``. ``db30/fair-db30``, ``db31/fair-db31``, and
  ``db32/fair-db32`` are that mode at three commits. One rerun writes one
  campaign into all three paths. ``--mode db8`` is the shorter campaign at
  ``db30/db8/campaign.json``.

``n`` is at least 6. Each record and the ``host.json`` beside it carries the
readcon-db commit and tree, the readcon-core version and git sha linked from
``Cargo.lock`` and the crate, rustc, python, the allocator, and the host
cpu, count, MHz, and kernel. The tree must be clean, and ``git branch -r
--contains HEAD`` must list a remote branch.
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
    "raw.log",
    "campaign.log",
)
SHA1 = 40


@dataclass(frozen=True)
class Step:
    """One producer invocation and the paper directories that receive it."""

    kind: str
    script: str
    papers: tuple[str, ...]
    record_name: str
    how: str
    sample_env: str
    flags: tuple[str, ...] = ()


STEPS: tuple[Step, ...] = (
    Step(
        "ab.json",
        "examples/benchmarks/db24_drive.py",
        ("db31/db24",),
        "ab.json",
        "drive",
        "READCON_DB24_N",
    ),
    Step(
        "ab.json",
        "examples/benchmarks/db25_drive.py",
        ("db31/db25",),
        "ab.json",
        "drive",
        "READCON_DB24_N",
    ),
    Step(
        "ab.json",
        "examples/benchmarks/db26_drive.py",
        ("db31/db26",),
        "ab.json",
        "drive",
        "READCON_DB26_N",
    ),
    Step(
        "ab.json",
        "examples/benchmarks/db28_drive.py",
        ("db31/db28",),
        "ab.json",
        "drive",
        "READCON_DB28_N",
    ),
    Step(
        "ab.json",
        "examples/benchmarks/db29_drive.py",
        ("db31/db29",),
        "ab.json",
        "drive",
        "READCON_DB29_N",
    ),
    Step(
        "readcon_db_bench.json",
        "examples/bench_corpus_ops.rs",
        ("db31/corpus-ops",),
        "readcon_db_bench.json",
        "corpus",
        "",
    ),
    Step(
        "campaign.json",
        "examples/benchmarks/fair_ase_protocol.py",
        ("db30/fair-db30", "db31/fair-db31", "db32/fair-db32"),
        "campaign.json",
        "fair",
        "READCON_DB_N_SAMPLES",
        ("--mode", "all"),
    ),
    Step(
        "campaign.json",
        "examples/benchmarks/fair_ase_protocol.py",
        ("db30/db8",),
        "campaign.json",
        "fair",
        "READCON_DB_N_SAMPLES",
        ("--mode", "db8"),
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
    status = git("status", "--porcelain")
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


def command_line(step: Step, root: Path, n: int) -> str:
    if step.how == "corpus":
        frames = work_dir(root, step)
        out_json = root / step.papers[0] / step.record_name
        argv = corpus_argv(frames, out_json, n)
        prep = "concatenate resources/test/tiny_cuh2.con into " + " ".join(
            f"n{count}.con" for count in LADDER
        )
        return f"{step.script} :: {prep} && " + " ".join(argv)
    if step.how == "fair":
        dest = work_dir(root, step)
    else:
        dest = root / step.papers[0]
    env = f"{step.sample_env}={n} READCON_RERUN_RELEASE=1"
    return f"{step.script} :: {env} " + " ".join(python_argv(step, dest))


def print_plan(root: Path, export: Path, n: int) -> None:
    print(f"n {n}")
    print(f"out {root}")
    print(f"export {export}")
    print("kind host.json")
    for step in STEPS:
        print(f"kind {step.kind}")
        print("producer " + command_line(step, root, n))
        for paper in step.papers:
            print(f"output {paper}/{step.record_name}")
            print(f"output {paper}/host.json")
            print(f"log {paper}/raw.log")
            print(f"export {paper}/{step.record_name}")
            print(f"export {paper}/host.json")


def cpu_model() -> str:
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        return ""
    return ""


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
    return {
        "cpu": cpu_model(),
        "ncpu": count,
        "cpuinfo_mhz": cpu_mhz(),
        "kernel": platform.release(),
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
    source = lock_source_sha()
    if source:
        return source
    if len(sha) != SHA1 or any(ch not in "0123456789abcdef" for ch in sha):
        raise SystemExit(f"readcon-core crate at {root} has no git sha")
    return sha


def lock_source_sha() -> str:
    """A git source in Cargo.lock names the revision that was linked."""
    source = lock_readcon_core().get("source", "")
    if "#" not in source or not source.startswith("git+"):
        return ""
    sha = source.rsplit("#", 1)[-1].split("?")[0].strip().lower()
    if len(sha) == SHA1 and all(ch in "0123456789abcdef" for ch in sha):
        return sha
    return ""


def allocator_in_use(root: Path) -> str:
    """The allocator these campaigns link.

    The rlib, the cdylib, and ``bench_corpus_ops`` link readcon-core without
    ``static_system_alloc``. That feature is the static archive only. When
    the crate installs mimalloc as the global allocator, that is the
    allocator in use.
    """
    text = (root / "src" / "lib.rs").read_text()
    if "mimalloc::MiMalloc" in text and "global_allocator" in text:
        return "mimalloc"
    return "system"


def provenance(commit: str, tree: str) -> dict[str, object]:
    locked = lock_readcon_core()
    root = crate_root(locked)
    sha = git_sha_of_crate(root)
    rustc = subprocess.check_output(["rustc", "--version"], text=True).strip()
    return {
        "commit": commit,
        "tree": tree,
        "readcon_core": locked["version"],
        "readcon_sha": sha,
        "rustc": rustc,
        "python": f"Python {platform.python_version()}",
        "allocator": allocator_in_use(root),
        "host": host_facts(),
    }


def stamp(record_path: Path, prov: dict[str, object]) -> None:
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


def run_command(argv: list[str], env: dict[str, str], log_path: Path) -> str:
    log_path.parent.mkdir(parents=True, exist_ok=True)
    proc = subprocess.run(
        argv,
        cwd=REPO,
        env=env,
        capture_output=True,
        text=True,
        check=False,
    )
    body = "command " + " ".join(argv) + "\n" + proc.stdout + proc.stderr
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
    for step in STEPS:
        body: dict[str, object] = {
            "synthetic": True,
            "n": n,
            "commit": prov["commit"],
            "tree": prov["tree"],
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
        raw = "synthetic\n" + command_line(step, root, n) + "\n"
        for paper in step.papers:
            directory = root / paper
            directory.mkdir(parents=True, exist_ok=True)
            record = directory / step.record_name
            refuse_existing(record)
            record.write_text(json.dumps(body, indent=2) + "\n")
            (directory / "raw.log").write_text(raw)
            stamp(record, prov)


def run_measurements(root: Path, prov: dict[str, object], n: int) -> None:
    for step in STEPS:
        for paper in step.papers:
            refuse_existing(root / paper / step.record_name)
        env = child_env(step, n)
        if step.how == "corpus":
            frames = work_dir(root, step)
            prepare_frames(frames)
            record = root / step.papers[0] / step.record_name
            record.parent.mkdir(parents=True, exist_ok=True)
            raw = run_command(
                corpus_argv(frames, record, n), env, record.parent / "raw.log"
            )
            if not record.is_file():
                raise SystemExit(f"corpus producer wrote no {record}")
            stamp(record, prov)
            continue
        if step.how == "fair":
            dest = work_dir(root, step)
            dest.mkdir(parents=True, exist_ok=True)
            raw = run_command(python_argv(step, dest), env, dest / "raw.log")
            produced = dest / step.record_name
            if not produced.is_file():
                raise SystemExit(f"fair producer wrote no {produced}")
            for paper in step.papers:
                target = root / paper
                publish_tree(dest, target, raw)
                stamp(target / step.record_name, prov)
            continue
        dest = root / step.papers[0]
        dest.mkdir(parents=True, exist_ok=True)
        record = dest / step.record_name
        raw = run_command(python_argv(step, dest), env, dest / "raw.log")
        if not record.is_file():
            raise SystemExit(f"producer wrote no {record}")
        if not (dest / "raw.log").is_file():
            (dest / "raw.log").write_text(raw)
        stamp(record, prov)


def export_records(root: Path, export: Path) -> None:
    refuse_saved(export)
    for step in STEPS:
        for paper in step.papers:
            src = root / paper
            dest = export / paper
            dest.mkdir(parents=True, exist_ok=True)
            for name in EXPORT_NAMES:
                path = src / name
                if path.is_file():
                    shutil.copy2(path, dest / name)


def parse_args(argv: list[str]) -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument("--synthetic", action="store_true")
    parser.add_argument("--n", type=int, default=12)
    parser.add_argument("--out", type=Path, default=None)
    parser.add_argument("--export", type=Path, default=None)
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
        for paper in step.papers:
            refuse_saved(root / paper)
    if args.dry_run:
        print_plan(root, export, args.n)
        return
    prov = provenance(commit, tree)
    if prov["commit"] != commit or prov["tree"] != tree:
        raise SystemExit("provenance commit does not match HEAD")
    if args.synthetic:
        write_synthetic(root, prov, args.n)
    else:
        run_measurements(root, prov, args.n)
    export_records(root, export)
    print(f"wrote {export}")


if __name__ == "__main__":
    main()
