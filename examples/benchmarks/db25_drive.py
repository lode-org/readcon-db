"""Affected rows after unshuffling straight into the NumPy buffer.

Full and range reads of a component-major trajectory no longer copy the
coordinates out of a second buffer. Selected already decoded into the
caller array, and cold reads are the file size, so this pass repeats
only the warm full and range rows. n=12, serial, alternating arms, the
same settings on both codecs. The second block sets
``RAYON_NUM_THREADS=1``.

The pushed base is ``28dbf15e``. Scratch stays under ``build/``.
"""

import json
import os
import platform
import shutil
import sys
from pathlib import Path

import db24_drive as fair_drive

REPO = fair_drive.REPO
BENCH = REPO / "bench" / "db25"
PUSHED_BASE = "28dbf15e8ff2cecdff429bcedbd5c9e2e3a60b59"
OPS = ("full_read", "range")
ARMS = fair_drive.ARMS


def write_readcon(name: str, root: Path, commits: dict[str, str]) -> int:
    dest = root / name
    if dest.exists():
        shutil.rmtree(dest)
    corpus = dest / "corpus"
    corpus.mkdir(parents=True)
    code = r"""
import sys
sys.path.insert(0, sys.argv[4])
import fair_ase_protocol as fair
from readcon_db import ConCorpus
from pathlib import Path
text, _ = fair.build_text_records(64, 100_000)
dest = Path(sys.argv[1])
bench = Path(sys.argv[2])
parent = Path(sys.argv[5])
db = ConCorpus(str(dest))
n = db.append_trajectory_str(1, text, source="traj.con")
del db
fair.record_table_usage(bench, "traj-100k-" + sys.argv[3], parent / sys.argv[3])
print(n, (dest / "data.mdb").stat().st_size)
"""
    out = fair_drive.run_py(
        name,
        code,
        [str(corpus), str(BENCH), name, str(fair_drive.WORKER.parent), str(root)],
        commits,
    )
    n_s, size_s = out.split()
    assert n_s == "64", out
    return int(size_s)


def write_ase(root: Path, commits: dict[str, str]) -> int:
    dest = root / "ase"
    if dest.exists():
        shutil.rmtree(dest)
    dest.mkdir(parents=True)
    code = r"""
import sys
sys.path.insert(0, sys.argv[2])
import fair_ase_protocol as fair
from pathlib import Path
atoms, records = fair.build_atom_records(64, 100_000)
dest = Path(sys.argv[1])
payload = {"atoms": atoms, "records": records, "n_frames": 64, "geometry_only": False}
fair.insert_fair("ase.db", payload, dest)
print(fair.disk_bytes("ase.db", dest))
"""
    return int(fair_drive.run_py("ase", code, [str(dest), str(fair_drive.WORKER.parent)], commits))


def write_results(record: dict) -> None:
    lines = [
        "# db25 warm full and range",
        "",
        f"Commit `{record['commit']}`, tree `{record['tree']}`. "
        f"Pushed base `{record['pushed_base']}`. "
        f"Host {record['host']['cpu']}, {record['host']['ncpu']} CPUs. "
        "n=12, serial. Arm order is db18, raw, shuffle, ase; reversed on odd "
        "sample indexes. These rows are the warm full and range reads. "
        "A multi-frame shuffle read unshuffles into the NumPy buffer allocated "
        "up front. Selected and the cold rows are unchanged from db24 and are "
        "not repeated. The median is type-7. Parentheses are the type-7 IQR. "
        "The paired CI is a 10000-draw percentile bootstrap of the shuffle/raw "
        "ratio at the same sample index (numpy Generator seed 0).",
        "",
        "| op | threads | raw | shuffle-zstd | shuffle/raw | paired median | 95% CI | faster |",
        "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    blocks = [
        ("full_read", "unset", record["ops"]["full_read:warm"]),
        ("range", "unset", record["ops"]["range:warm"]),
        ("full_read", "1", record["ops_rayon_num_threads_1"]["full_read:warm"]),
        ("range", "1", record["ops_rayon_num_threads_1"]["range:warm"]),
    ]
    for op, threads, block in blocks:
        paired = block["paired"]
        lo, hi = paired["ci95"]
        lines.append(
            f"| {op} | {threads} | {block['raw']['median_s']:.4e} "
            f"({block['raw']['iqr_s']:.4e}) | {block['shuffle']['median_s']:.4e} "
            f"({block['shuffle']['iqr_s']:.4e}) | {block['ratio_shuffle_over_raw']:.4f} | "
            f"{paired['median']:.4f} | [{lo:.4f}, {hi:.4f}] | "
            f"{paired['shuffle_faster']}/{paired['n']} |"
        )
    lines.append("")
    full = record["ops_rayon_num_threads_1"]["full_read:warm"]["paired"]
    lo, hi = full["ci95"]
    if hi < 1.0:
        lines.append(
            "Single-thread warm full reads are faster: the paired interval is below 1."
        )
    else:
        lines.append(
            "Single-thread warm full reads are not faster on this paired interval. "
            f"Paired median {full['median']:.4f}, 95% CI [{lo:.4f}, {hi:.4f}], "
            f"shuffle faster in {full['shuffle_faster']} of {full['n']}."
        )
    lines.append("")
    (BENCH / "RESULTS.md").write_text("\n".join(lines))


def main() -> None:
    scratch_tmp = REPO / "build" / "tmp"
    scratch_tmp.mkdir(parents=True, exist_ok=True)
    os.environ["TMPDIR"] = str(scratch_tmp)
    root = REPO / "build" / "db25-ab"
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    BENCH.mkdir(parents=True, exist_ok=True)
    log = open(BENCH / "campaign.log", "w")
    commit = fair_drive.git_out("rev-parse", "HEAD")
    tree = fair_drive.git_out("rev-parse", f"{commit}^{{tree}}")
    commits = {"head": commit}
    print(f"building wheels for {commit} on {PUSHED_BASE}", flush=True)
    fair_drive.ensure_prefix("db18", fair_drive.DB18_COMMIT)
    fair_drive.ensure_prefix("db24", commit)
    sizes = {}
    for name in ("db18", "raw", "shuffle"):
        sizes[name] = write_readcon(name, root, commits)
        print(f"wrote {name} {sizes[name]}", flush=True)
    sizes["ase"] = write_ase(root, commits)
    print(f"wrote ase {sizes['ase']}", flush=True)
    workers = {name: fair_drive.start_worker(name, root, commits) for name in ARMS}
    try:
        read = fair_drive.run_block(workers, root, ("warm",), OPS, log)
    finally:
        fair_drive.stop(workers)
    workers = {
        name: fair_drive.start_worker(name, root, commits, {"RAYON_NUM_THREADS": "1"})
        for name in ARMS
    }
    try:
        one = fair_drive.run_block(workers, root, ("warm",), OPS, log)
    finally:
        fair_drive.stop(workers)
    out = {
        "commit": commit,
        "tree": tree,
        "pushed_base": PUSHED_BASE,
        "host": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "cpu": fair_drive.cpu_model(),
            "ncpu": os.cpu_count(),
        },
        "n": fair_drive.N,
        "rows": "warm full_read and warm range; then the same with RAYON_NUM_THREADS=1",
        "arm_order": "db18, raw, shuffle, ase; reversed on odd sample indexes",
        "sizes": sizes,
        "tables_json": "fair_ase_protocol.record_table_usage",
        "ops": fair_drive.pack(read, [("warm", op) for op in OPS]),
        "ops_rayon_num_threads_1": fair_drive.pack(one, [("warm", op) for op in OPS]),
    }
    (BENCH / "ab.json").write_text(json.dumps(out, indent=2) + "\n")
    write_results(out)
    log.close()
    print("wrote", BENCH / "ab.json", flush=True)


if __name__ == "__main__":
    main()
