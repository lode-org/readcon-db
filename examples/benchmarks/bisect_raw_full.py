"""Measure the raw full-read bisect and write one sample per pair.

The record's medians and intervals are computed by
``raw_full_bisect.summarize_pair``. ``host.json`` names the full SHAs
that were timed.
"""

from __future__ import annotations

import json
import os
import shutil
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import db26_drive as drv
import db28_drive as d28
import raw_full_bisect as bisect

REPO = Path(__file__).resolve().parents[2]
# Short names are the labels. The full SHA is what host.json records.
MEASURED = (
    ("aedcbda4", "aedcbda468d208278b8699b80c6db743edcdc21e"),
    ("750b197", "750b1970b2ddc9cc0bb4547d3918024d67d7f6a6"),
    ("d671c32", "d671c327375f588044b6f4953314cd80ddddf356"),
    ("20a5f64", "20a5f64e37d9a0892e913289d39bcbe71d7718f8"),
)
N = 30
PAIRS = (
    ("aedcbda4", "750b197"),
    ("750b197", "d671c32"),
    ("d671c32", "20a5f64"),
)


def write_record(dest: Path, steps: list[dict], commit: str, tree: str) -> None:
    dest.mkdir(parents=True, exist_ok=True)
    host = {
        "commit": commit,
        "tree": tree,
        "hostname": d28.host_label(d28.NEUTRAL_HOST),
        "cpu": drv.fair.cpu_model(),
        "ncpu": os.cpu_count(),
        "n": N,
        "measured": [{"label": label, "sha": sha} for label, sha in MEASURED],
    }
    (dest / "host.json").write_text(json.dumps(host, indent=2) + "\n")
    samples = []
    for step in steps:
        samples.append(
            {
                "left": step["left"],
                "right": step["right"],
                "left_sha": step["left_sha"],
                "right_sha": step["right_sha"],
                "threads": step["threads"],
                "samples": step["samples"],
            }
        )
    (dest / "samples.json").write_text(json.dumps(samples) + "\n")
    computed = bisect.steps_from_samples(samples)
    record = {"commit": commit, "tree": tree, "host": host, "steps": computed}
    (dest / "ab.json").write_text(json.dumps(record, indent=2) + "\n")
    lines = [
        "# raw full-read bisect",
        "",
        (
            f"Commit `{commit}`. n={N}. Each CI is computed from the paired samples "
            f"in `samples.json`. Host label `{host['hostname']}`."
        ),
        "",
        "| left | right | threads | n | paired | 95% CI | right faster |",
        "| --- | --- | --- | --- | --- | --- | --- |",
    ]
    for step in computed:
        lo, hi = step["ci95"]
        lines.append(
            f"| `{step['left_sha']}` | `{step['right_sha']}` | {step['threads']} | {step['n']} | "
            f"{step['paired_median']:.4f} | [{lo:.4f}, {hi:.4f}] | "
            f"{step['right_faster']}/{step['n']} |"
        )
    lines.append("")
    (dest / "RESULTS.md").write_text("\n".join(lines))


def corpus_ready(dest: Path, sha: str) -> bool:
    """A corpus is reused only when the commit that wrote it is ``sha``."""
    stamp = dest.parent / "corpus.commit"
    return (
        (dest / "data.mdb").is_file()
        and stamp.is_file()
        and stamp.read_text().strip() == sha
    )


def _sha(label: str) -> str:
    return dict(MEASURED)[label]


def measure_pair(
    left: str, right: str, wheels: dict[str, str], threads: str | None
) -> dict:
    """One paired setting. Returns samples only; the summary is computed later."""
    extra = {"RAYON_NUM_THREADS": "1"} if threads == "1" else None
    root = REPO / "build" / "bisect-raw"
    workers = {
        left: drv.start_worker(left, root / left, wheels[left], "raw", extra, root),
        right: drv.start_worker(right, root / right, wheels[right], "raw", extra, root),
    }
    samples = {left: [], right: []}
    try:
        for i in range(N):
            order = [left, right] if i % 2 == 0 else [right, left]
            for name in order:
                row = drv.ask(name, workers[name], "warm full_read", root)
                samples[name].append(row["seconds"])
            print(
                f"{threads or 'unset'} {left}/{right} {i} "
                f"{left}={samples[left][-1]:.4e} {right}={samples[right][-1]:.4e}",
                flush=True,
            )
    finally:
        drv.stop(workers)
    return {
        "left": left,
        "right": right,
        "left_sha": _sha(left),
        "right_sha": _sha(right),
        "threads": threads or "unset",
        "samples": samples,
    }


def main() -> None:
    os.environ["TMPDIR"] = str(REPO / "build" / "tmp")
    (REPO / "build" / "tmp").mkdir(parents=True, exist_ok=True)
    drv.fair.require_pushed_commit()
    commit = drv.fair.git_out("rev-parse", "HEAD")
    tree = drv.fair.git_out("rev-parse", f"{commit}^{{tree}}")
    wheels = {label: drv.ensure_tree_prefix(label, sha) for label, sha in MEASURED}
    root = REPO / "build" / "bisect-raw"
    for label, sha in MEASURED:
        # Corpus write lives with the wheel that will read it.
        dest = root / label / "corpus"
        if corpus_ready(dest, sha):
            continue
        if dest.exists():
            shutil.rmtree(dest)
        dest.mkdir(parents=True)
        code = r"""
import sys
sys.path.insert(0, sys.argv[2])
import fair_ase_protocol as fair
from readcon_db import ConCorpus
text, _ = fair.build_text_records(64, 100_000)
db = ConCorpus(sys.argv[1])
print(db.append_trajectory_str(1, text, source="traj.con"))
"""
        drv.run_py(
            wheels[label],
            code,
            [str(dest), str(REPO / "examples" / "benchmarks")],
            "raw",
        )
        (dest.parent / "corpus.commit").write_text(sha + "\n")
    for _ in range(18):
        load = os.getloadavg()[0]
        if load <= 0.5:
            break
        time.sleep(10)
    records = []
    for left, right in PAIRS:
        for threads in (None, "1"):
            records.append(measure_pair(left, right, wheels, threads))
    write_record(REPO / "bench" / "db29" / "bisect", records, commit, tree)


if __name__ == "__main__":
    main()
