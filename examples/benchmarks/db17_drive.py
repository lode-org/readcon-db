"""Cold A/B of the db15 binary against this tree on one traj-100k corpus.

The corpus is written by ``9e568240`` into ``build/db17-corpus``. Both
libraries are built from their own worktrees. Selected, range, and metadata
are cold reads, n=12, one thread, alternating which library runs first.
Metadata is repeated with the order reversed. The open and the cache drop
stay outside the clock.
"""

from __future__ import annotations

import hashlib
import json
import os

os.environ["OMP_NUM_THREADS"] = "1"
os.environ["OPENBLAS_NUM_THREADS"] = "1"
os.environ["MKL_NUM_THREADS"] = "1"
os.environ["NUMEXPR_NUM_THREADS"] = "1"

import subprocess
import sys
from pathlib import Path

import numpy as np

import db24_drive as fair

REPO = Path(__file__).resolve().parents[2]
REMOTE_BRANCH = "fix/ingest-species-count"
DB15 = "9e568240a2d734085da2c411be0977e604e12ed6"
N = 12
N_FRAMES = 64
N_ATOMS = 100_000
OPS = ("selected", "range", "metadata")

READER = r"""
import json
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[2])
sys.path.insert(1, sys.argv[1])
import fair_ase_protocol as fair

dest = Path(sys.argv[3])
op = sys.argv[4]
n_frames = 64
mid = n_frames // 2
state = {}


def setup() -> None:
    fair.prepare_cold_files("readcon-db", dest)
    state["db"] = fair.open_readcon(dest / "corpus", warm=False)


def run() -> None:
    state["got"] = fair.fetch_materialized(
        "readcon-db", state["db"], op, mid, n_frames
    )


def teardown() -> None:
    fair.close_reader("readcon-db", state["db"])
    state["db"] = None
    state["got"] = None


row = fair.sample_operation(run, setup=setup, teardown=teardown)
json.dump({"seconds": row["seconds"], "inner_n": row["inner_n"]}, sys.stdout)
sys.stdout.write("\n")
"""

WRITER = r"""
import sys
from pathlib import Path

sys.path.insert(0, sys.argv[2])
sys.path.insert(1, sys.argv[1])
import fair_ase_protocol as fair
from readcon_db import ConCorpus

dest = Path(sys.argv[3])
dest.mkdir(parents=True, exist_ok=True)
text, _records = fair.build_text_records(64, 100_000)
db = ConCorpus(str(dest))
n_frames = db.append_trajectory_str(1, text, source="traj.con")
print(n_frames, (dest / "data.mdb").stat().st_size)
"""


def measure_root() -> Path:
    root = Path(os.environ.get("READCON_MEASURE_ROOT", ""))
    if root.is_dir():
        return root
    return REPO


def git(root: Path, *args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=root, text=True).strip()


def remote_branch_sha(root: Path, branch: str = REMOTE_BRANCH) -> str:
    """Commit named by ``refs/heads/<branch>`` on origin.

    A local ``origin/<branch>`` ref stays stale when ``remote.origin.fetch``
    only covers main. ``ls-remote`` reads the remote ref itself.
    """
    ref = f"refs/heads/{branch}"
    out = git(root, "ls-remote", "origin", ref)
    for line in out.splitlines():
        sha, _sep, name = line.partition("\t")
        if name == ref:
            return sha
    raise SystemExit(f"refusing to measure: origin has no {ref}")


def require_tip(root: Path) -> str:
    """Refuse unless HEAD is the remote branch tip and the tree is clean."""
    tip = remote_branch_sha(root)
    head = git(root, "rev-parse", "HEAD")
    if head != tip:
        raise SystemExit(
            f"refusing to measure: HEAD {head} is not the remote branch tip {tip}"
        )
    dirty = git(root, "status", "--porcelain", "--untracked-files=all", "--ignored=no")
    if dirty.strip():
        raise SystemExit("refusing to measure: working tree is not clean")
    return head


def pinned_env(root: Path, py_path: str) -> dict:
    env = os.environ.copy()
    env["PYTHONPATH"] = py_path
    env["PYTHONUNBUFFERED"] = "1"
    env["TMPDIR"] = str(root / "build" / "tmp")
    env["RAYON_NUM_THREADS"] = "1"
    env["OMP_NUM_THREADS"] = "1"
    env["OPENBLAS_NUM_THREADS"] = "1"
    env["MKL_NUM_THREADS"] = "1"
    env["NUMEXPR_NUM_THREADS"] = "1"
    return env


def so_digest(py_path: str) -> str:
    found = sorted(Path(py_path).glob("readcon_db/*.so"))
    if len(found) != 1:
        raise SystemExit(f"{py_path} has {found}")
    return hashlib.sha256(found[0].read_bytes()).hexdigest()


def run_py(root: Path, py_path: str, bench: Path, code: str, args: list[str]) -> str:
    proc = subprocess.run(
        [sys.executable, "-c", code, str(bench), py_path, *args],
        check=False,
        capture_output=True,
        text=True,
        env=pinned_env(root, py_path),
    )
    if proc.returncode != 0:
        raise SystemExit(proc.stderr[-4000:] or proc.stdout[-2000:])
    return proc.stdout.strip()


def write_corpus(root: Path, py_path: str, bench: Path, dest: Path) -> int:
    if dest.exists():
        import shutil

        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    out = run_py(root, py_path, bench, WRITER, [str(dest)])
    n_frames, size = out.split()
    if n_frames != str(N_FRAMES):
        raise SystemExit(f"corpus wrote {n_frames} frames")
    (dest.parent / "corpus.commit").write_text(DB15 + "\n")
    return int(size)


def one_sample(root: Path, py_path: str, bench: Path, dest: Path, op: str) -> dict:
    out = run_py(root, py_path, bench, READER, [str(dest.parent), op])
    return json.loads(out)


def median(values: list[float]) -> float:
    return float(np.percentile(np.asarray(values), 50, method="linear"))


def summarize(samples: list[dict]) -> dict:
    seconds = [row["seconds"] for row in samples]
    return {
        "n": len(seconds),
        "median_s": median(seconds),
        "samples": samples,
    }


def write_record(out: Path, payload: dict) -> None:
    out.mkdir(parents=True, exist_ok=True)
    (out / "ab.json").write_text(json.dumps(payload, indent=2) + "\n")
    (out / "host.json").write_text(json.dumps(payload["host"], indent=2) + "\n")
    lines = [
        "# traj-100k cold A/B",
        "",
        (
            f"Commit `{payload['commit']}`. Corpus written by `{DB15}` "
            f"({payload['corpus_bytes']} bytes) under `{payload['corpus']}`. "
            f"n={N}. Host label `host`. "
            f"{fair.protocol_header(payload['host'])}"
        ),
        "",
        "| op | order | db15 median s | tip median s | tip/db15 |",
        "| --- | --- | --- | --- | --- |",
    ]
    for name, block in payload["ops"].items():
        left = block["db15"]["median_s"]
        right = block["tip"]["median_s"]
        lines.append(
            f"| {name} | {block['order']} | {left:.6e} | {right:.6e} | {right / left:.4f} |"
        )
    lines.append("")
    (out / "RESULTS.md").write_text("\n".join(lines))


def main() -> None:
    root = measure_root()
    sys.path.insert(0, str(root / "examples" / "benchmarks"))
    import db26_drive as drv

    tip = require_tip(root)
    (root / "build" / "tmp").mkdir(parents=True, exist_ok=True)
    wheels = {
        "db15": drv.ensure_tree_prefix("db15", DB15),
        "tip": drv.ensure_tree_prefix("tip", tip),
    }
    digests = {label: so_digest(path) for label, path in wheels.items()}
    if digests["db15"] == digests["tip"]:
        raise SystemExit(f"libraries are the same build: {digests}")
    print(f"digest db15 {digests['db15']}", flush=True)
    print(f"digest tip {digests['tip']}", flush=True)
    benches = {
        "db15": root / "build" / f"src-{DB15[:12]}" / "examples" / "benchmarks",
        "tip": root / "examples" / "benchmarks",
    }
    corpus = root / "build" / "db17-corpus" / "corpus"
    size = write_corpus(root, wheels["db15"], benches["db15"], corpus)
    print(f"corpus bytes {size}", flush=True)
    ops = {}
    orders = [("db15", "tip")]
    for op in OPS:
        block_orders = orders + ([("tip", "db15")] if op == "metadata" else [])
        for order in block_orders:
            key = op if order == ("db15", "tip") else f"{op}_reverse"
            samples = {label: [] for label in order}
            for index in range(N):
                for label in order:
                    row = one_sample(root, wheels[label], benches[label], corpus, op)
                    samples[label].append(row)
                    print(
                        f"{key} {index} {label} {row['seconds']:.6e}",
                        flush=True,
                    )
            ops[key] = {
                "order": " then ".join(order),
                "db15": summarize(samples["db15"]),
                "tip": summarize(samples["tip"]),
            }
    payload = {
        "commit": tip,
        "db15": DB15,
        "n": N,
        "corpus": "build/db17-corpus/corpus",
        "corpus_bytes": size,
        "host": {
            "commit": tip,
            "hostname": "host",
            "n": N,
            "rayon_num_threads": "1",
            "omp_num_threads": "1",
            "openblas_num_threads": "1",
            "mkl_num_threads": "1",
            "numexpr_num_threads": "1",
            "library_sha256": digests,
            **fair.host_facts(),
        },
        "ops": ops,
    }
    write_record(root / "build" / "db30-db17", payload)
    print("DB17_OK", flush=True)


if __name__ == "__main__":
    main()
