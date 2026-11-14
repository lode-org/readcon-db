"""db26 records. Selected and cold, raw tip versus aedcbda4, same-column high byte.

The geometry is the lattice arm, labelled as the best case in
``fair_ase_protocol.synthetic_arrays``. Scratch stays under ``build/``.
This script does not write ``bench/db24`` or ``bench/db25``. The published
files are ``bench/db26``, and ``db26_highbyte`` writes ``high_byte.json``.
"""

from __future__ import annotations

import json
import os
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path

import numpy as np

import db24_drive as fair
import db26_highbyte

REPO = fair.REPO
BENCH = REPO / "bench" / "db26"
# The bench/db26 files in the repository were produced by this driver at
# this commit. A later run records whatever pushed HEAD it is started on.
PUBLISHED_TIP = "20a5f64e37d9a0892e913289d39bcbe71d7718f8"
BASE = "aedcbda4"
N = 12
BOOT_SAMPLES = 10_000
BOOT_SEED = 0
WORKER = fair.WORKER


def env_base() -> dict:
    env = os.environ.copy()
    home = str(Path.home() / ".local" / "bin")
    env["PATH"] = home + os.pathsep + env.get("PATH", "")
    env["TMPDIR"] = str(REPO / "build" / "tmp")
    env["RUSTUP_TOOLCHAIN"] = "1.88.0"
    env.pop("RAYON_NUM_THREADS", None)
    return env


def site_packages(prefix: Path) -> Path:
    return fair.site_packages(prefix)


def ensure_tree_prefix(kind: str, commit: str) -> str:
    """Install ``commit`` from a detached worktree. The tip uses the checkout."""
    prefix = REPO / "build" / "wheels" / kind
    if fair.prefix_ready(prefix, commit):
        return str(site_packages(prefix))
    if prefix.exists():
        shutil.rmtree(prefix)
    prefix.mkdir(parents=True)
    env = env_base()
    if kind == "tip":
        fair.pip_install(prefix, REPO / "python", env)
    else:
        src = REPO / "build" / "src-aedcbda4"
        env["CARGO_TARGET_DIR"] = str(REPO / "build" / "target-aedcbda4")
        if not (src / "python" / "pyproject.toml").is_file():
            if src.exists():
                subprocess.check_call(
                    ["git", "worktree", "remove", "--force", str(src)], cwd=REPO
                )
            subprocess.check_call(
                ["git", "worktree", "add", "--detach", str(src), commit], cwd=REPO
            )
        fair.pip_install(prefix, src / "python", env)
    (prefix / ".commit").write_text(commit + "\n")
    return str(site_packages(prefix))


def run_py(py_path: str, code: str, args: list[str], codec: str | None) -> str:
    env = env_base()
    env["PYTHONPATH"] = py_path
    env["PYTHONUNBUFFERED"] = "1"
    if codec:
        env["READCON_DB_COOKED_CODEC"] = codec
    else:
        env.pop("READCON_DB_COOKED_CODEC", None)
    proc = subprocess.run(
        [sys.executable, "-c", code, *args],
        check=True,
        capture_output=True,
        text=True,
        env=env,
    )
    if proc.stderr.strip():
        print(proc.stderr[-1500:], file=sys.stderr, flush=True)
    return proc.stdout.strip()


def write_corpus(name: str, root: Path, py_path: str, codec: str) -> int:
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
    out = run_py(
        py_path,
        code,
        [str(corpus), str(BENCH), name, str(WORKER.parent), str(root)],
        codec,
    )
    n_s, size_s = out.split()
    if n_s != "64":
        raise SystemExit(out)
    return int(size_s)


def start_worker(name: str, dest: Path, py_path: str, codec: str, extra: dict | None, root: Path):
    env = env_base()
    env["PYTHONPATH"] = py_path
    env["PYTHONUNBUFFERED"] = "1"
    env["READCON_DB_COOKED_CODEC"] = codec
    if extra:
        env.update(extra)
    err_path = root / f"{name}.worker.err"
    err = open(err_path, "w")
    proc = subprocess.Popen(
        [sys.executable, str(WORKER), str(dest), "readcon-db"],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=err,
        text=True,
        env=env,
    )
    ready = proc.stdout.readline().strip()
    if ready != "READY":
        raise RuntimeError(err_path.read_text()[-2000:] or ready)
    return proc


def ask(name: str, proc, command: str, root: Path) -> dict:
    proc.stdin.write(command + "\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        err = (root / f"{name}.worker.err").read_text()[-2000:]
        raise RuntimeError(err or f"{name} closed stdout")
    return json.loads(line)


def stop(workers: dict) -> None:
    fair.stop(workers)


def median(values) -> float:
    return fair.median(values)


def iqr(values) -> float:
    return fair.iqr(values)


def bootstrap(ratios: np.ndarray) -> list[float]:
    rng = np.random.default_rng(BOOT_SEED)
    n = int(ratios.size)
    boots = np.empty(BOOT_SAMPLES, dtype=np.float64)
    for i in range(BOOT_SAMPLES):
        idx = rng.integers(0, n, n)
        boots[i] = np.percentile(ratios[idx], 50, method="linear")
    lo, hi = np.percentile(boots, [2.5, 97.5], method="linear")
    return [float(lo), float(hi)]


def cell_block(samples) -> dict:
    med = median(samples)
    spread = iqr(samples)
    return {
        "median_s": med,
        "iqr_s": spread,
        "iqr_over_median": (spread / med) if med else None,
        "samples_s": samples,
    }


def load_now() -> dict:
    one, five, fifteen = os.getloadavg()
    return {"1m": one, "5m": five, "15m": fifteen}


def wait_quiet(log, limit: float = 0.5, timeout: float = 180.0) -> dict:
    t0 = time.time()
    last = load_now()
    while time.time() - t0 < timeout:
        last = load_now()
        line = f"load {last['1m']:.2f} {last['5m']:.2f} {last['15m']:.2f}"
        print(line, flush=True)
        log.write(line + "\n")
        log.flush()
        if last["1m"] <= limit:
            return last
        time.sleep(10)
    return last


def run_pairs(workers, root, keys, order, log, label: str) -> tuple[dict, list]:
    """``keys`` is (cache, op). ``order`` is a function of the sample index."""
    series = {key: {name: [] for name in workers} for key in keys}
    loads = []
    for cache, op in keys:
        for i in range(N):
            loads.append({"i": i, "op": f"{op}:{cache}", "before": load_now()})
            for name in order(i):
                row = ask(name, workers[name], f"{cache} {op}", root)
                series[(cache, op)][name].append(row["seconds"])
                line = (
                    f"{label} {cache} {op} {i} {name} {row['seconds']:.6e} "
                    f"load={loads[-1]['before']['1m']:.2f}"
                )
                print(line, flush=True)
                log.write(line + "\n")
                log.flush()
    return series, loads


def pack_shuffle(series, keys) -> dict:
    out = {}
    for cache, op in keys:
        block = {}
        for name in ("raw", "shuffle"):
            block[name] = cell_block(series[(cache, op)][name])
        raw = block["raw"]["median_s"]
        shuffle = block["shuffle"]["median_s"]
        ratios = np.asarray(block["shuffle"]["samples_s"]) / np.asarray(
            block["raw"]["samples_s"]
        )
        faster = int(np.sum(np.asarray(block["shuffle"]["samples_s"]) < np.asarray(block["raw"]["samples_s"])))
        slower = int(np.sum(np.asarray(block["shuffle"]["samples_s"]) > np.asarray(block["raw"]["samples_s"])))
        ties = int(np.sum(np.asarray(block["shuffle"]["samples_s"]) == np.asarray(block["raw"]["samples_s"])))
        block["ratio_shuffle_over_raw"] = (shuffle / raw) if raw else None
        block["paired"] = {
            "median": float(np.percentile(ratios, 50, method="linear")),
            "ci95": bootstrap(ratios),
            "shuffle_faster": faster,
            "shuffle_slower": slower,
            "ties": ties,
            "n": int(ratios.size),
            "ratios": [float(v) for v in ratios],
        }
        out[f"{op}:{cache}"] = block
    return out


def pack_tip(series, keys) -> dict:
    out = {}
    for cache, op in keys:
        block = {}
        for name in ("aedcbda4", "tip"):
            block[name] = cell_block(series[(cache, op)][name])
        base = block["aedcbda4"]["median_s"]
        tip = block["tip"]["median_s"]
        ratios = np.asarray(block["tip"]["samples_s"]) / np.asarray(
            block["aedcbda4"]["samples_s"]
        )
        faster = int(np.sum(np.asarray(block["tip"]["samples_s"]) < np.asarray(block["aedcbda4"]["samples_s"])))
        slower = int(np.sum(np.asarray(block["tip"]["samples_s"]) > np.asarray(block["aedcbda4"]["samples_s"])))
        ties = N - faster - slower
        block["ratio_tip_over_aedcbda4"] = (tip / base) if base else None
        block["paired"] = {
            "median": float(np.percentile(ratios, 50, method="linear")),
            "ci95": bootstrap(ratios),
            "tip_faster": faster,
            "tip_slower": slower,
            "ties": ties,
            "n": int(ratios.size),
            "ratios": [float(v) for v in ratios],
        }
        out[f"{op}:{cache}"] = block
    return out


def fmt_cell(block, name) -> str:
    return f"{block[name]['median_s']:.4e} ({block[name]['iqr_s']:.4e})"


def paired_sentence(label: str, paired: dict, left: str, faster_key: str, slower_key: str) -> str:
    lo, hi = paired["ci95"]
    if hi < 1.0:
        verdict = f"The paired interval is below 1, so {left} is faster."
    elif lo > 1.0:
        verdict = f"The paired interval is above 1, so {left} is slower."
    else:
        verdict = "The paired interval includes 1."
    return (
        f"{label}: paired median {paired['median']:.4f}, "
        f"95% CI [{lo:.4f}, {hi:.4f}]. "
        f"{left} faster in {paired[faster_key]} of {paired['n']}, "
        f"slower in {paired[slower_key]}, ties {paired['ties']}. {verdict}"
    )


def write_results(record: dict) -> None:
    host = record["host"]
    lines = [
        "# db26 selected, cold, and raw across commits",
        "",
        f"Commit `{record['commit']}`, tree `{record['tree']}`. "
        f"Raw comparison base `{record['base_commit']}` tree `{record['base_tree']}`. "
        f"Host {host['system']} {host['release']}, {host['cpu']}, {host['ncpu']} CPUs. "
        f"n={record['n']}, serial. The median is type-7. Parentheses are the type-7 IQR. "
        "The paired CI is a 10000-draw percentile bootstrap of the ratio at the same "
        f"sample index (numpy Generator seed {BOOT_SEED}). "
        "Odd sample indexes reverse the arm order. "
        f"Load average at the start of the timed block was {record['load_before_timed']['1m']:.2f}, "
        f"{record['load_before_timed']['5m']:.2f}, {record['load_before_timed']['15m']:.2f}.",
        "",
        "db24 and db25 are unchanged. db25 did not repeat selected or cold. "
        "Those rows are here because `d671c32` unshuffles into the caller buffer. "
        "That commit is in this history, and this measurement is `20a5f64e`.",
        "",
        "## Summary",
        "",
        "| comparison | op | threads | cache | ratio of medians | paired median | 95% CI | faster |",
        "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for key, op, cache in (
        ("selected:cold", "selected", "cold"),
        ("full_read:cold", "full_read", "cold"),
        ("range:cold", "range", "cold"),
        ("selected:warm", "selected", "warm"),
    ):
        block = record["shuffle_vs_raw"][key]
        lo, hi = block["paired"]["ci95"]
        lines.append(
            f"| shuffle/raw | {op} | unset | {cache} | "
            f"{block['ratio_shuffle_over_raw']:.4f} | {block['paired']['median']:.4f} | "
            f"[{lo:.4f}, {hi:.4f}] | "
            f"{block['paired']['shuffle_faster']}/{block['paired']['n']} |"
        )
    for threads, group in (
        ("unset", record["raw_tip_vs_aedcbda4"]),
        ("1", record["raw_tip_vs_aedcbda4_rayon_1"]),
    ):
        for key, op in (("full_read:warm", "full_read"), ("range:warm", "range")):
            block = group[key]
            lo, hi = block["paired"]["ci95"]
            lines.append(
                f"| 20a5f64e/aedcbda4 raw | {op} | {threads} | warm | "
                f"{block['ratio_tip_over_aedcbda4']:.4f} | {block['paired']['median']:.4f} | "
                f"[{lo:.4f}, {hi:.4f}] | "
                f"{block['paired']['tip_faster']}/{block['paired']['n']} |"
            )
    lines.append("")
    lines.append("## Shuffle versus raw")
    lines.append("")
    lines.append(
        "Arm order is raw, shuffle, reversed on odd indexes. "
        "Warm selected is one pass of i*17 mod 64, and the seconds are the mean per frame. "
        "Cold advises `DONTNEED` outside the clock, then opens and reads inside it. "
        "There is no frame cache."
    )
    lines.append("")
    lines.append("| op | cache | raw | shuffle-zstd | shuffle/raw | paired median | 95% CI | faster |")
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- |")
    for key, op, cache in (
        ("full_read:cold", "full_read", "cold"),
        ("selected:cold", "selected", "cold"),
        ("range:cold", "range", "cold"),
        ("selected:warm", "selected", "warm"),
    ):
        block = record["shuffle_vs_raw"][key]
        lo, hi = block["paired"]["ci95"]
        lines.append(
            f"| {op} | {cache} | {fmt_cell(block, 'raw')} | {fmt_cell(block, 'shuffle')} | "
            f"{block['ratio_shuffle_over_raw']:.4f} | {block['paired']['median']:.4f} | "
            f"[{lo:.4f}, {hi:.4f}] | "
            f"{block['paired']['shuffle_faster']}/{block['paired']['n']} |"
        )
    lines.append("")
    for key, op, cache in (
        ("full_read:cold", "full_read", "cold"),
        ("selected:cold", "selected", "cold"),
        ("range:cold", "range", "cold"),
        ("selected:warm", "selected", "warm"),
    ):
        lines.append(
            paired_sentence(
                f"{op} {cache}",
                record["shuffle_vs_raw"][key]["paired"],
                "shuffle",
                "shuffle_faster",
                "shuffle_slower",
            )
        )
        lines.append("")
    lines.append("## Raw full and range, this commit versus aedcbda4")
    lines.append("")
    lines.append(
        "Both arms are `READCON_DB_COOKED_CODEC=raw`. Each arm reads the corpus "
        "it wrote. The ratio is this commit over `aedcbda4`. "
        "Arm order is tip, aedcbda4, reversed on odd indexes. "
        "The load average is the 1-minute value sampled before each pair."
    )
    lines.append("")
    lines.append("| op | threads | aedcbda4 | 20a5f64e | tip/base | paired median | 95% CI | tip faster | load 1m median |")
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for threads, group, load_key in (
        ("unset", record["raw_tip_vs_aedcbda4"], "load_raw_unset"),
        ("1", record["raw_tip_vs_aedcbda4_rayon_1"], "load_raw_rayon_1"),
    ):
        loads = [row["before"]["1m"] for row in record[load_key]]
        load_med = float(np.percentile(np.asarray(loads), 50, method="linear"))
        for key, op in (("full_read:warm", "full_read"), ("range:warm", "range")):
            block = group[key]
            lo, hi = block["paired"]["ci95"]
            lines.append(
                f"| {op} | {threads} | {fmt_cell(block, 'aedcbda4')} | {fmt_cell(block, 'tip')} | "
                f"{block['ratio_tip_over_aedcbda4']:.4f} | {block['paired']['median']:.4f} | "
                f"[{lo:.4f}, {hi:.4f}] | "
                f"{block['paired']['tip_faster']}/{block['paired']['n']} | {load_med:.2f} |"
            )
    lines.append("")
    for threads, group in (
        ("unset", record["raw_tip_vs_aedcbda4"]),
        ("1", record["raw_tip_vs_aedcbda4_rayon_1"]),
    ):
        for key, op in (("full_read:warm", "full_read"), ("range:warm", "range")):
            lines.append(
                paired_sentence(
                    f"RAYON={threads} raw {op}",
                    group[key]["paired"],
                    "20a5f64e",
                    "tip_faster",
                    "tip_slower",
                )
            )
            lines.append("")
    lines.append(
        "Per-sample 1, 5, and 15 minute load averages are in `ab.json` "
        "under `load_raw_unset` and `load_raw_rayon_1`."
    )
    lines.append("")
    lines.append("## High byte, same columns on both sides")
    lines.append("")
    lines.append(
        "`high_byte.json` is one 100k-atom frame, positions `(i * 0.125, 0.5, 1.0)` "
        "and forces `(x * 0.5, 0.25, 0.5)`, byte-shuffled per lane. "
        "zstd level 3, window log 12, long-distance matching off. "
        "Each time is the mean of 20 rounds after 3 untimed rounds, and the "
        "parenthesis is the type-7 IQR of those rounds. "
        "A row's uncompressed size, full-shuffle size, and high-four size are "
        "the same lanes. The db24 sentence compared uncompressed forces with a "
        "high-four figure that also included positions. That mix is not repeated."
    )
    lines.append("")
    lines.append("| columns | lanes | uncompressed | full shuffle zstd | full decode µs | high-four zstd | low-four raw | high-four decode+copy µs |")
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- |")
    for row in record["high_byte"]["sets"]:
        full = row["full_shuffle_decode"]
        high = row["high_four_decode_copy"]
        lines.append(
            f"| {row['columns']} | {row['n_columns']} | {row['uncompressed_bytes']} | "
            f"{row['full_shuffle_zstd_bytes']} | {full['mean_us']:.0f} ({full['iqr_us']:.0f}) | "
            f"{row['high_four_zstd_bytes']} | {row['low_four_raw_bytes']} | "
            f"{high['mean_us']:.0f} ({high['iqr_us']:.0f}) |"
        )
    lines.append("")
    lines.append(
        "Full shuffle stores only the zstd bytes. The high-four scheme stores "
        "the zstd of the high four bytes plus the low four bytes raw. "
        "On both column sets the full-shuffle blob is the smaller of the two."
    )
    lines.append("")
    lines.append("## Sizes")
    lines.append("")
    lines.append("| corpus | data.mdb |")
    lines.append("| --- | --- |")
    for name, size in record["sizes"].items():
        lines.append(f"| {name} | {size} |")
    lines.append("")
    lines.append("`tables.json` is `fair_ase_protocol.record_table_usage`.")
    lines.append("")
    (BENCH / "RESULTS.md").write_text("\n".join(lines))


def high_byte(commit: str) -> dict:
    os.environ["DB26_COMMIT"] = commit
    return db26_highbyte.main()


def main() -> None:
    os.environ.update(env_base())
    fair.require_pushed_commit()
    commit = fair.git_out("rev-parse", "HEAD")
    print(f"published db26 records are {PUBLISHED_TIP}; this run records {commit}", flush=True)
    tree = fair.git_out("rev-parse", f"{commit}^{{tree}}")
    base_tree = fair.git_out("rev-parse", f"{BASE}^{{tree}}")
    BENCH.mkdir(parents=True, exist_ok=True)
    root = REPO / "build" / "db26-ab"
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    log = open(BENCH / "campaign.log", "w")
    print("building tip and aedcbda4 prefixes", flush=True)
    tip_py = ensure_tree_prefix("tip", commit)
    base_py = ensure_tree_prefix("aedcbda4", BASE)
    print("high-byte columns", flush=True)
    high = high_byte(commit)
    (BENCH / "high_byte.json").write_text(json.dumps(high, indent=2) + "\n")
    log.write("high-byte written\n")
    sizes = {}
    for name, py_path, codec in (
        ("raw", tip_py, "raw"),
        ("shuffle", tip_py, "shuffle-zstd"),
        ("aedcbda4", base_py, "raw"),
    ):
        sizes[name] = write_corpus(name, root, py_path, codec)
        line = f"wrote {name} {sizes[name]}"
        print(line, flush=True)
        log.write(line + "\n")
        log.flush()
    before = wait_quiet(log)
    log.write(f"timed block starts at load {before}\n")

    def shuffle_order(i: int) -> list[str]:
        arms = ["raw", "shuffle"]
        if i % 2:
            arms.reverse()
        return arms

    workers = {
        "raw": start_worker("raw", root / "raw", tip_py, "raw", None, root),
        "shuffle": start_worker("shuffle", root / "shuffle", tip_py, "shuffle-zstd", None, root),
    }
    keys_shuffle = (
        ("cold", "full_read"),
        ("cold", "selected"),
        ("cold", "range"),
        ("warm", "selected"),
    )
    try:
        series, _loads = run_pairs(workers, root, keys_shuffle, shuffle_order, log, "shuffle")
    finally:
        stop(workers)
    shuffle_pack = pack_shuffle(series, keys_shuffle)

    def tip_order(i: int) -> list[str]:
        arms = ["tip", "aedcbda4"]
        if i % 2:
            arms.reverse()
        return arms

    keys_raw = (("warm", "full_read"), ("warm", "range"))
    workers = {
        "tip": start_worker("tip", root / "raw", tip_py, "raw", None, root),
        "aedcbda4": start_worker("aedcbda4", root / "aedcbda4", base_py, "raw", None, root),
    }
    try:
        raw_series, raw_loads = run_pairs(workers, root, keys_raw, tip_order, log, "raw-unset")
    finally:
        stop(workers)
    workers = {
        "tip": start_worker(
            "tip", root / "raw", tip_py, "raw", {"RAYON_NUM_THREADS": "1"}, root
        ),
        "aedcbda4": start_worker(
            "aedcbda4",
            root / "aedcbda4",
            base_py,
            "raw",
            {"RAYON_NUM_THREADS": "1"},
            root,
        ),
    }
    try:
        one_series, one_loads = run_pairs(workers, root, keys_raw, tip_order, log, "raw-1")
    finally:
        stop(workers)
    out = {
        "commit": commit,
        "tree": tree,
        "base_commit": fair.git_out("rev-parse", BASE),
        "base_tree": base_tree,
        "unshuffle_commit": "d671c32",
        "host": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "cpu": fair.cpu_model(),
            "ncpu": os.cpu_count(),
        },
        "n": N,
        "load_before_timed": before,
        "sizes": sizes,
        "shuffle_vs_raw": shuffle_pack,
        "raw_tip_vs_aedcbda4": pack_tip(raw_series, keys_raw),
        "raw_tip_vs_aedcbda4_rayon_1": pack_tip(one_series, keys_raw),
        "load_raw_unset": raw_loads,
        "load_raw_rayon_1": one_loads,
        "high_byte": high,
        "tables_json": "fair_ase_protocol.record_table_usage",
        "scratch": "build/db26-ab",
    }
    (BENCH / "ab.json").write_text(json.dumps(out, indent=2) + "\n")
    write_results(out)
    log.write("wrote bench/db26\n")
    log.close()
    print("wrote", BENCH / "ab.json", flush=True)


if __name__ == "__main__":
    main()
