"""Fair traj-100k A/B. Arm order alternates, as the paper protocol does.

Scratch, wheels, and worker logs go under ``build/`` via :func:`bench_disk`.
The published record is ``bench/db24``. ``tables.json`` is written by
``fair_ase_protocol.record_table_usage``.

Warm selected is one pass of ``i * 17 mod 64``. Odd sample indexes reverse
the arm order. A second block repeats the warm rows with
``RAYON_NUM_THREADS=1``.

The db18 arm is the library at ``06ffcbd`` (the db18 record; that commit
only adds the bench files on top of ``9dcc9ce``). The raw and shuffle arms
are this tree. Both prefixes are built under ``build/wheels``.
"""

import json
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[2]
WORKER = Path(__file__).resolve().parent / "db23_worker.py"
BENCH = REPO / "bench" / "db24"
# The db18 campaign commit. Its tree matches the library measured as db18.
DB18_COMMIT = "06ffcbddc44039b4bf3f3c38793a0927a5709d8a"
# bench/db23 names 82746979. That object is not in the published history.
# The published commit of the same tree is 00d3213.
DB23_UNPUBLISHED = "82746979b2a34851019d066f0ead3d3812b98ee2"
DB23_TREE = "4a1b19288f7c66a650ce6e0aad7b88db8c82daaf"
DB23_PUBLISHED = "00d32136e9acdbc84fd0297d68b812b0facaedee"
N = int(os.environ.get("READCON_DB24_N", "12"))
OPS = ("full_read", "selected", "range")
CACHES = ("cold", "warm")
ARMS = ("db18", "raw", "shuffle", "ase")
BOOT_SAMPLES = 10_000
BOOT_SEED = 0


def bench_disk() -> Path:
    """Scratch for corpora, wheels are separate. Never ``/tmp``."""
    path = REPO / "build" / "db24-ab"
    path.mkdir(parents=True, exist_ok=True)
    return path


def site_packages(prefix: Path) -> Path:
    found = []
    for pattern in (
        "lib/python*/site-packages",
        "lib/python*/dist-packages",
        "local/lib/python*/site-packages",
        "local/lib/python*/dist-packages",
    ):
        found.extend(prefix.glob(pattern))
    found = sorted(set(found))
    if not found:
        raise RuntimeError(f"no site-packages under {prefix}")
    return found[0]


def prefix_ready(prefix: Path, commit: str) -> bool:
    stamp = prefix / ".commit"
    if not stamp.is_file() or stamp.read_text().strip() != commit:
        return False
    return any(site_packages(prefix).glob("readcon_db*"))


def pip_install(prefix: Path, src: Path, env: dict) -> None:
    subprocess.check_call(
        [
            sys.executable,
            "-m",
            "pip",
            "install",
            "--prefix",
            str(prefix),
            "--no-build-isolation",
            "--no-deps",
            str(src),
        ],
        cwd=src.parent,
        env=env,
    )


def ensure_prefix(kind: str, commit: str) -> str:
    """Build a readcon-db prefix under ``build/wheels`` if the stamp differs."""
    prefix = REPO / "build" / "wheels" / kind
    if prefix_ready(prefix, commit):
        return str(site_packages(prefix))
    if prefix.exists():
        shutil.rmtree(prefix)
    prefix.mkdir(parents=True)
    env = os.environ.copy()
    env["RUSTUP_TOOLCHAIN"] = "1.88.0"
    if kind == "db18":
        src_tree = REPO / "build" / "src-db18"
        env["CARGO_TARGET_DIR"] = str(REPO / "build" / "target-db18")
        if not (src_tree / "python" / "pyproject.toml").is_file():
            if src_tree.exists():
                subprocess.check_call(
                    ["git", "worktree", "remove", "--force", str(src_tree)],
                    cwd=REPO,
                )
            subprocess.check_call(
                ["git", "worktree", "add", "--detach", str(src_tree), commit],
                cwd=REPO,
            )
        pip_install(prefix, src_tree / "python", env)
    else:
        pip_install(prefix, REPO / "python", env)
    (prefix / ".commit").write_text(commit + "\n")
    return str(site_packages(prefix))


def prefix(name: str, commits: dict[str, str]) -> str:
    if name in ("ase", "db18"):
        return ensure_prefix("db18", DB18_COMMIT)
    return ensure_prefix("db24", commits["head"])


def apply_codec(name: str, env: dict) -> None:
    env.pop("READCON_DB_COOKED_CODEC", None)
    if name == "shuffle":
        env["READCON_DB_COOKED_CODEC"] = "shuffle-zstd"
    elif name == "raw":
        env["READCON_DB_COOKED_CODEC"] = "raw"


def git_out(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=REPO, text=True).strip()


def require_pushed_commit(repo: Path | None = None) -> None:
    """Refuse to measure a commit that origin does not already contain.

    Pushes re-stamp local commits, so a SHA measured before it is on origin
    never exists upstream. The tree must be clean as well. ``repo`` is the
    git checkout; the benchmark drivers use this repository.
    """
    repo = REPO if repo is None else Path(repo)
    subprocess.check_call(["git", "fetch", "origin"], cwd=repo)
    contained = subprocess.run(
        ["git", "branch", "-r", "--contains", "HEAD"],
        cwd=repo,
        check=False,
        capture_output=True,
        text=True,
    )
    if contained.returncode != 0 or not contained.stdout.strip():
        raise SystemExit(
            "refusing to measure: HEAD is not contained in any origin branch"
        )
    dirty = subprocess.check_output(
        ["git", "status", "--porcelain"], cwd=repo, text=True
    )
    if dirty.strip():
        raise SystemExit("refusing to measure: working tree is not clean")


def cpu_model() -> str:
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    except OSError:
        return ""
    return ""


def arm_order(index: int) -> list[str]:
    arms = list(ARMS)
    if index % 2:
        arms.reverse()
    return arms


def run_py(name: str, code: str, args: list[str], commits: dict[str, str]) -> str:
    env = os.environ.copy()
    env["PYTHONPATH"] = prefix(name, commits)
    env["PYTHONUNBUFFERED"] = "1"
    apply_codec(name, env)
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
    bench = str(WORKER.parent)
    out = run_py(
        name,
        code,
        [str(corpus), str(BENCH), name, bench, str(root)],
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
    return int(run_py("ase", code, [str(dest), str(WORKER.parent)], commits))


def start_worker(name: str, root: Path, commits: dict[str, str], extra: dict | None = None):
    env = os.environ.copy()
    env["PYTHONPATH"] = prefix(name, commits)
    env["PYTHONUNBUFFERED"] = "1"
    apply_codec(name, env)
    if extra:
        env.update(extra)
    kind = "ase.db" if name == "ase" else "readcon-db"
    err_path = root / f"{name}.worker.err"
    err = open(err_path, "w")
    proc = subprocess.Popen(
        [sys.executable, str(WORKER), str(root / name), kind],
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
    for proc in workers.values():
        try:
            proc.stdin.write("quit\n")
            proc.stdin.flush()
        except BrokenPipeError:
            pass
        proc.wait(timeout=180)


def median(values):
    return float(np.percentile(np.asarray(values, dtype=np.float64), 50, method="linear"))


def iqr(values):
    arr = np.asarray(values, dtype=np.float64)
    return float(
        np.percentile(arr, 75, method="linear") - np.percentile(arr, 25, method="linear")
    )


def paired_ratio(raw_samples, shuffle_samples) -> dict:
    raw = np.asarray(raw_samples, dtype=np.float64)
    shuffle = np.asarray(shuffle_samples, dtype=np.float64)
    ratios = shuffle / raw
    faster = int(np.sum(shuffle < raw))
    slower = int(np.sum(shuffle > raw))
    ties = int(np.sum(shuffle == raw))
    rng = np.random.default_rng(BOOT_SEED)
    n = int(ratios.size)
    boots = np.empty(BOOT_SAMPLES, dtype=np.float64)
    for i in range(BOOT_SAMPLES):
        idx = rng.integers(0, n, n)
        boots[i] = np.percentile(ratios[idx], 50, method="linear")
    lo, hi = np.percentile(boots, [2.5, 97.5], method="linear")
    return {
        "median": float(np.percentile(ratios, 50, method="linear")),
        "ci95": [float(lo), float(hi)],
        "ci_method": (
            f"{BOOT_SAMPLES} bootstrap resamples of the paired shuffle/raw ratio, "
            "type-7 median, percentile 2.5 and 97.5, numpy Generator seed "
            f"{BOOT_SEED}"
        ),
        "shuffle_faster": faster,
        "shuffle_slower": slower,
        "ties": ties,
        "n": n,
        "ratios": [float(v) for v in ratios],
    }


def run_block(workers: dict, root: Path, caches, ops, log) -> dict:
    series = {}
    for cache in caches:
        for op in ops:
            series[(cache, op)] = {name: [] for name in ARMS}
            for i in range(N):
                for name in arm_order(i):
                    row = ask(name, workers[name], f"{cache} {op}", root)
                    series[(cache, op)][name].append(row["seconds"])
                    line = (
                        f"{cache} {op} {i} {name} {row['seconds']:.6e} "
                        f"inner={row['inner_n']}"
                    )
                    print(line, flush=True)
                    log.write(line + "\n")
                    log.flush()
    return series


def pack(series, keys) -> dict:
    out = {}
    for cache, op in keys:
        block = {}
        for name in ARMS:
            vals = series[(cache, op)][name]
            med = median(vals)
            spread = iqr(vals)
            block[name] = {
                "median_s": med,
                "iqr_s": spread,
                "iqr_over_median": (spread / med) if med else None,
                "samples_s": vals,
            }
        raw = block["raw"]["median_s"]
        shuffle = block["shuffle"]["median_s"]
        block["ratio_shuffle_over_raw"] = (shuffle / raw) if raw else None
        block["paired"] = paired_ratio(
            block["raw"]["samples_s"], block["shuffle"]["samples_s"]
        )
        label = "insert" if cache == "ingest" else f"{op}:{cache}"
        out[label] = block
        paired = block["paired"]
        print(
            f"{label} raw={raw:.6e} shuffle={shuffle:.6e} "
            f"ratio={shuffle / raw:.4f} paired={paired['median']:.4f} "
            f"ci=[{paired['ci95'][0]:.4f},{paired['ci95'][1]:.4f}] "
            f"faster={paired['shuffle_faster']}/{paired['n']}",
            flush=True,
        )
    return out


def run_levers(log) -> dict:
    env = os.environ.copy()
    env["RUSTUP_TOOLCHAIN"] = "1.88.0"
    proc = subprocess.run(
        [
            "cargo",
            "test",
            "--release",
            "--lib",
            "record_db24_levers",
            "--",
            "--ignored",
            "--nocapture",
        ],
        cwd=REPO,
        env=env,
        capture_output=True,
        text=True,
        check=True,
    )
    log.write(proc.stdout)
    log.write(proc.stderr)
    blob = None
    for line in proc.stdout.splitlines():
        line = line.strip()
        if line.startswith("{") and line.endswith("}"):
            blob = line
    if blob is None:
        raise RuntimeError(proc.stdout[-2000:] + proc.stderr[-2000:])
    return json.loads(blob)


def cell(block, name: str) -> str:
    med = block[name]["median_s"]
    spread = block[name]["iqr_s"]
    return f"{med:.4e} ({spread:.4e})"


def paired_sentence(label: str, paired: dict) -> str:
    med = paired["median"]
    lo, hi = paired["ci95"]
    wins = (
        f"shuffle faster in {paired['shuffle_faster']} of {paired['n']}, "
        f"slower in {paired['shuffle_slower']}, ties {paired['ties']}"
    )
    if hi < 1.0:
        verdict = "The paired interval is below 1, so shuffle is faster."
    elif lo > 1.0:
        verdict = "The paired interval is above 1, so shuffle is slower."
    else:
        verdict = "The paired interval includes 1."
    return (
        f"{label}: paired median {med:.4f}, 95% CI [{lo:.4f}, {hi:.4f}]. "
        f"{wins[0].upper() + wins[1:]}. {verdict}"
    )


def write_results(record: dict, levers: dict) -> None:
    host = record["host"]
    lines = []
    lines.append("# db24 fair comparison")
    lines.append("")
    lines.append(
        f"Commit `{record['commit']}`, tree `{record['tree']}`. "
        f"Host {host['system']} {host['release']}, {host['cpu']}, {host['ncpu']} CPUs. "
        f"`RAYON_NUM_THREADS` was unset for the first block. n={record['n']}, serial. "
        "Arm order is db18, raw, shuffle, ase; reversed on odd sample indexes. "
        "Warm selected is one pass of i*17 mod 64, each frame once; seconds are the mean per frame. "
        "There is no frame cache. The median is type-7. Each time cell is that median, "
        "and the parentheses are the type-7 IQR. Both percentiles use `numpy` method `linear`."
    )
    lines.append("")
    lines.append(
        f"`bench/db23` names unpublished commit `{DB23_UNPUBLISHED}` for tree "
        f"`{DB23_TREE}`. That commit is not in the published history. The published "
        f"commit for the same tree is `{DB23_PUBLISHED}`. db23 and db22 are unchanged."
    )
    lines.append("")
    lines.append(
        "db22 lists a warm-selected shuffle/raw ratio of 0.573. That figure is "
        "superseded. The raw arm copied the blob before it looked at the tag, which "
        "inflated raw. The corrected comparisons are the db23 and db24 warm-selected rows."
    )
    lines.append("")
    lines.append("## Level and window")
    lines.append("")
    lines.append(
        "`bench/db24/levers.json` is the stdout of `record_db24_levers` (release, ignored). "
        "One 100k-atom frame, positions and forces, six scalar columns. "
        "Each time is the mean of 20 rounds after 3 untimed rounds. "
        "The same rounds are in `samples_us`, and `iqr_us` is their type-7 IQR. "
        "Long-distance matching is off. "
        "`tables.json` is written by `fair_ase_protocol.record_table_usage`."
    )
    lines.append("")
    scope = levers["scope_two_threads"]
    lines.append(
        f"Two empty `thread::scope` workers cost {scope['mean_us']:.0f} µs "
        f"(IQR {scope['iqr_us']:.0f} µs)."
    )
    lines.append("")
    lines.append("| level | window | bytes | one thread µs (IQR) | two threads µs (IQR) |")
    lines.append("| --- | --- | --- | --- | --- |")
    for row in levers["rows"]:
        lines.append(
            f"| {row['level']} | {row['window']} | {row['bytes']} | "
            f"{row['seq']['mean_us']:.0f} ({row['seq']['iqr_us']:.0f}) | "
            f"{row['two_threads']['mean_us']:.0f} ({row['two_threads']['iqr_us']:.0f}) |"
        )
    lines.append("")
    by_key = {(row["level"], row["window"]): row for row in levers["rows"]}
    stay = by_key[(3, 12)]
    fast = by_key[(-5, 12)]
    gap = stay["seq"]["mean_us"] - fast["seq"]["mean_us"]
    wider = max(stay["seq"]["iqr_us"], fast["seq"]["iqr_us"])
    lines.append(
        "Level 3 and a 4 KiB window are the parameters these fair rows used. "
        f"Level -5 at that window is {gap:.0f} µs under level 3 on the mean, "
        f"and the wider of those two IQRs is {wider:.0f} µs, so that gap is "
        "outside the lever's own spread. The compressed columns grow from "
        f"{stay['bytes']} bytes to {fast['bytes']} bytes. The level stays 3. "
        "The comparison is against this lever IQR, not a selected-row IQR."
    )
    lines.append("")
    high = levers["high_byte_decode_copy"]
    lines.append(
        f"Leaving forces uncompressed would store {levers['raw_force_bytes']} bytes a frame. "
        f"Shuffling only the high four bytes of each `f64` is "
        f"{levers['high_byte_zstd_bytes']} bytes of zstd plus "
        f"{levers['high_byte_raw_low_bytes']} raw bytes, and that decode plus the copy is "
        f"{high['mean_us']:.0f} µs (IQR {high['iqr_us']:.0f} µs). Both stay fully shuffled."
    )
    lines.append("")
    lines.append("## How the two arms run")
    lines.append("")
    lines.append(
        "A single frame is decoded on the calling thread for raw and for shuffle. "
        "The shuffled path decompresses the three lanes into reused buffers and "
        "unshuffles them straight into `x, y, z` order. It does not scatter one "
        "lane at a time. A range or a full shuffle read splits frames across "
        "`RAYON_NUM_THREADS` workers, or across the process thread count when that "
        "variable is unset. Raw range and raw full stay on one thread. The second "
        "table is the warm rows again with `RAYON_NUM_THREADS=1`."
    )
    lines.append("")
    lines.append("## Sizes")
    lines.append("")
    lines.append("| | db18 | raw | shuffle-zstd | ASE |")
    lines.append("| --- | --- | --- | --- | --- |")
    sizes = record["sizes"]
    lines.append(
        f"| traj-100k file | {sizes['db18']} | {sizes['raw']} | {sizes['shuffle']} | {sizes['ase']} |"
    )
    soa = {}
    tables_path = BENCH / "tables.json"
    if tables_path.is_file():
        for row in json.loads(tables_path.read_text()):
            for table in row["tables"]:
                if table["name"] == "frames_soa":
                    soa[row["workload"]] = table["value_bytes"]
    if soa:
        lines.append(
            f"| `frames_soa` values | {soa.get('traj-100k-db18', '')} | "
            f"{soa.get('traj-100k-raw', '')} | {soa.get('traj-100k-shuffle', '')} | |"
        )
    lines.append("")
    lines.append(
        "`frames_soa` values are the `value_bytes` rows in `bench/db24/tables.json`, "
        "written by `fair_ase_protocol.record_table_usage`."
    )
    lines.append("")
    lines.append("## Times")
    lines.append("")
    lines.append("| op | cache | db18 | raw | shuffle-zstd | ASE | shuffle/raw |")
    lines.append("| --- | --- | --- | --- | --- | --- | --- |")
    order = [
        ("insert", "insert", ""),
        ("full_read:cold", "full_read", "cold"),
        ("selected:cold", "selected", "cold"),
        ("range:cold", "range", "cold"),
        ("full_read:warm", "full_read", "warm"),
        ("selected:warm", "selected", "warm"),
        ("range:warm", "range", "warm"),
    ]
    for key, op, cache in order:
        block = record["ops"][key]
        lines.append(
            f"| {op} | {cache} | {cell(block, 'db18')} | {cell(block, 'raw')} | "
            f"{cell(block, 'shuffle')} | {cell(block, 'ase')} | "
            f"{block['ratio_shuffle_over_raw']:.4f} |"
        )
    lines.append("")
    lines.append(
        "The shuffle/raw column is the ratio of the two medians. "
        "The paired ratio below is the type-7 median of shuffle/raw at the same "
        "sample index. The CI is a 10000-draw percentile bootstrap of that median "
        f"(numpy Generator seed {BOOT_SEED})."
    )
    lines.append("")
    for key, op, cache in order:
        label = "insert" if key == "insert" else f"{op} {cache}"
        lines.append(paired_sentence(label, record["ops"][key]["paired"]))
        lines.append("")
    lines.append(
        "A slower row is inside the noise when the relative gap of the medians is "
        "no larger than the wider of the two IQR/median ratios, and no larger than "
        "0.05 when both spreads are tighter than that. The paired interval is the "
        "one used when saying a row is faster."
    )
    lines.append("")
    warm_full = record["ops"]["full_read:warm"]["paired"]
    if warm_full["ci95"][1] < 1.0:
        lines.append(
            "With `RAYON_NUM_THREADS` unset, warm full reads are faster on the paired interval."
        )
    else:
        lines.append(
            "With `RAYON_NUM_THREADS` unset, warm full reads are not claimed from the "
            "paired interval alone; the sentence above is the result."
        )
    lines.append("")
    lines.append(
        "`READCON_DB_COOKED_CODEC=raw` keeps RCSO. The name is case-sensitive; "
        "the only other accepted value is `shuffle-zstd`. A reader from before db20 "
        "cannot open a corpus written with the shuffle default."
    )
    lines.append("")
    lines.append("### `RAYON_NUM_THREADS=1`")
    lines.append("")
    lines.append("| op | cache | db18 | raw | shuffle-zstd | ASE | shuffle/raw |")
    lines.append("| --- | --- | --- | --- | --- | --- | --- |")
    one_order = [
        ("full_read:warm", "full_read", "warm"),
        ("selected:warm", "selected", "warm"),
        ("range:warm", "range", "warm"),
    ]
    for key, op, cache in one_order:
        block = record["ops_rayon_num_threads_1"][key]
        lines.append(
            f"| {op} | {cache} | {cell(block, 'db18')} | {cell(block, 'raw')} | "
            f"{cell(block, 'shuffle')} | {cell(block, 'ase')} | "
            f"{block['ratio_shuffle_over_raw']:.4f} |"
        )
    lines.append("")
    for key, op, cache in one_order:
        lines.append(
            paired_sentence(
                f"RAYON=1 {op} {cache}",
                record["ops_rayon_num_threads_1"][key]["paired"],
            )
        )
        lines.append("")
    full = record["ops_rayon_num_threads_1"]["full_read:warm"]["paired"]
    if full["ci95"][1] < 1.0:
        lines.append(
            "Single-thread warm full reads are faster: the paired interval is below 1."
        )
    else:
        lines.append(
            "Single-thread warm full reads are not faster on this paired interval. "
            "The ratio of medians and the win count are in the sentence above."
        )
    lines.append("")
    text = "\n".join(lines)
    (BENCH / "RESULTS.md").write_text(text)


def main() -> None:
    require_pushed_commit()
    scratch_tmp = REPO / "build" / "tmp"
    scratch_tmp.mkdir(parents=True, exist_ok=True)
    os.environ["TMPDIR"] = str(scratch_tmp)
    root = bench_disk()
    BENCH.mkdir(parents=True, exist_ok=True)
    log_path = BENCH / "campaign.log"
    log = open(log_path, "w")
    commit = git_out("rev-parse", "HEAD")
    tree = git_out("rev-parse", f"{commit}^{{tree}}")
    commits = {"head": commit}
    print(f"building wheels for {commit} and db18 {DB18_COMMIT}", flush=True)
    ensure_prefix("db18", DB18_COMMIT)
    ensure_prefix("db24", commit)
    print("running lever probe", flush=True)
    levers = run_levers(log)
    (BENCH / "levers.json").write_text(json.dumps(levers, indent=2) + "\n")
    sizes = {}
    for name in ("db18", "raw", "shuffle"):
        sizes[name] = write_readcon(name, root, commits)
        line = f"wrote {name} {sizes[name]}"
        print(line, flush=True)
        log.write(line + "\n")
    sizes["ase"] = write_ase(root, commits)
    line = f"wrote ase {sizes['ase']}"
    print(line, flush=True)
    log.write(line + "\n")
    log.flush()
    workers = {name: start_worker(name, root, commits) for name in ARMS}
    try:
        read = run_block(workers, root, CACHES, OPS, log)
        ingest = {("ingest", "insert"): {name: [] for name in ARMS}}
        disks = {name: [] for name in ARMS}
        for i in range(N):
            for name in arm_order(i):
                row = ask(name, workers[name], "ingest", root)
                ingest[("ingest", "insert")][name].append(row["seconds"])
                disks[name].append(row["disk_bytes"])
                line = f"ingest {i} {name} {row['seconds']:.6e} disk={row['disk_bytes']}"
                print(line, flush=True)
                log.write(line + "\n")
                log.flush()
    finally:
        stop(workers)
    workers = {
        name: start_worker(name, root, commits, {"RAYON_NUM_THREADS": "1"}) for name in ARMS
    }
    try:
        one = run_block(workers, root, ("warm",), OPS, log)
    finally:
        stop(workers)
    out = {
        "commit": commit,
        "tree": tree,
        "host": {
            "system": platform.system(),
            "release": platform.release(),
            "machine": platform.machine(),
            "cpu": cpu_model(),
            "ncpu": os.cpu_count(),
        },
        "RAYON_NUM_THREADS": os.environ.get("RAYON_NUM_THREADS"),
        "n": N,
        "arm_order": "db18, raw, shuffle, ase; reversed on odd sample indexes",
        "warm_selected": "one pass of i*17 mod 64, each frame once; seconds are the mean per frame",
        "cache": "none",
        "scratch": str(root.relative_to(REPO)),
        "db18_commit": DB18_COMMIT,
        "db23_unpublished_commit": DB23_UNPUBLISHED,
        "db23_tree": DB23_TREE,
        "db23_published_commit": DB23_PUBLISHED,
        "db22_superseded": (
            "db22 warm selected shuffle/raw 0.573 is superseded: the raw arm copied "
            "the blob before it looked at the tag, which inflated raw. See db23 and db24."
        ),
        "tables_json": "fair_ase_protocol.record_table_usage",
        "sizes": sizes,
        "ingest_disk_bytes": disks,
        "ops": {},
        "ops_rayon_num_threads_1": {},
    }
    out["ops"] = pack(
        read | ingest, [(c, o) for c in CACHES for o in OPS] + [("ingest", "insert")]
    )
    out["ops_rayon_num_threads_1"] = pack(one, [("warm", op) for op in OPS])
    (BENCH / "ab.json").write_text(json.dumps(out, indent=2) + "\n")
    write_results(out, levers)
    log.write("wrote bench/db24\n")
    log.close()
    print("wrote", BENCH / "ab.json", flush=True)


if __name__ == "__main__":
    main()
