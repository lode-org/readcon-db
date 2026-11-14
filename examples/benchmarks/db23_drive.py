"""Fair traj-100k A/B. Arm order alternates, as the paper protocol does.

Warm selected is one pass of ``i * 17 mod 64``. Odd sample indexes reverse
the arm order. A second block repeats the warm rows with
``RAYON_NUM_THREADS=1``.
"""

import json
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path

import numpy as np

ROOT = Path("/tmp/db23-ab")
REPO = Path(__file__).resolve().parents[2]
WORKER = Path(__file__).resolve().parent / "db23_worker.py"
N = 12
OPS = ("full_read", "selected", "range")
CACHES = ("cold", "warm")
ARMS = ("db18", "raw", "shuffle", "ase")


def prefix(name: str) -> str:
    wheel = "db18" if name in ("ase", "db18") else "db23"
    return f"/tmp/py-{wheel}/local/lib/python3.12/dist-packages"


def apply_codec(name: str, env: dict) -> None:
    env.pop("READCON_DB_COOKED_CODEC", None)
    if name == "shuffle":
        env["READCON_DB_COOKED_CODEC"] = "shuffle-zstd"
    elif name == "raw":
        env["READCON_DB_COOKED_CODEC"] = "raw"


def git_out(*args: str) -> str:
    return subprocess.check_output(["git", *args], cwd=REPO, text=True).strip()


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


def run_py(name: str, code: str, args: list[str], extra: dict | None = None) -> str:
    env = os.environ.copy()
    env["PYTHONPATH"] = prefix(name)
    env["PYTHONUNBUFFERED"] = "1"
    apply_codec(name, env)
    if extra:
        env.update(extra)
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


def write_readcon(name: str) -> int:
    dest = ROOT / name
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
parent = Path(sys.argv[2])
db = ConCorpus(str(dest))
n = db.append_trajectory_str(1, text, source="traj.con")
del db
fair.record_table_usage(parent, "traj-100k-" + sys.argv[3], parent / sys.argv[3])
print(n, (dest / "data.mdb").stat().st_size)
"""
    bench = str(WORKER.parent)
    out = run_py(name, code, [str(corpus), str(ROOT), name, bench])
    n_s, size_s = out.split()
    assert n_s == "64", out
    return int(size_s)


def write_ase() -> int:
    dest = ROOT / "ase"
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
    return int(run_py("ase", code, [str(dest), str(WORKER.parent)]))


def start_worker(name: str, extra: dict | None = None):
    env = os.environ.copy()
    env["PYTHONPATH"] = prefix(name)
    env["PYTHONUNBUFFERED"] = "1"
    apply_codec(name, env)
    if extra:
        env.update(extra)
    kind = "ase.db" if name == "ase" else "readcon-db"
    err_path = ROOT / f"{name}.worker.err"
    err = open(err_path, "w")
    proc = subprocess.Popen(
        [sys.executable, str(WORKER), str(ROOT / name), kind],
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


def ask(name: str, proc, command: str) -> dict:
    proc.stdin.write(command + "\n")
    proc.stdin.flush()
    line = proc.stdout.readline()
    if not line:
        err = (ROOT / f"{name}.worker.err").read_text()[-2000:]
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
    return float(
        np.percentile(np.asarray(values, dtype=np.float64), 50, method="linear")
    )


def iqr(values):
    arr = np.asarray(values, dtype=np.float64)
    return float(
        np.percentile(arr, 75, method="linear")
        - np.percentile(arr, 25, method="linear")
    )


def run_block(workers: dict, caches, ops) -> dict:
    series = {}
    for cache in caches:
        for op in ops:
            series[(cache, op)] = {name: [] for name in ARMS}
            for i in range(N):
                for name in arm_order(i):
                    row = ask(name, workers[name], f"{cache} {op}")
                    series[(cache, op)][name].append(row["seconds"])
                    print(
                        f"{cache} {op} {i} {name} {row['seconds']:.6e} inner={row['inner_n']}",
                        flush=True,
                    )
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
        label = "insert" if cache == "ingest" else f"{op}:{cache}"
        out[label] = block
        print(
            f"{label} raw={raw:.6e} shuffle={shuffle:.6e} ratio={shuffle / raw:.4f}",
            flush=True,
        )
    return out


def main() -> None:
    ROOT.mkdir(parents=True, exist_ok=True)
    commit = git_out("rev-parse", "HEAD")
    tree = git_out("rev-parse", f"{commit}^{{tree}}")
    sizes = {}
    for name in ("db18", "raw", "shuffle"):
        sizes[name] = write_readcon(name)
        print(f"wrote {name} {sizes[name]}", flush=True)
    sizes["ase"] = write_ase()
    print(f"wrote ase {sizes['ase']}", flush=True)
    workers = {name: start_worker(name) for name in ARMS}
    try:
        read = run_block(workers, CACHES, OPS)
        ingest = {("ingest", "insert"): {name: [] for name in ARMS}}
        disks = {name: [] for name in ARMS}
        for i in range(N):
            for name in arm_order(i):
                row = ask(name, workers[name], "ingest")
                ingest[("ingest", "insert")][name].append(row["seconds"])
                disks[name].append(row["disk_bytes"])
                print(
                    f"ingest {i} {name} {row['seconds']:.6e} disk={row['disk_bytes']}",
                    flush=True,
                )
    finally:
        stop(workers)
    one = {}
    workers = {name: start_worker(name, {"RAYON_NUM_THREADS": "1"}) for name in ARMS}
    try:
        one = run_block(workers, ("warm",), OPS)
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
        "sizes": sizes,
        "ingest_disk_bytes": disks,
        "ops": {},
        "ops_rayon_num_threads_1": {},
    }
    out["ops"] = pack(
        read | ingest, [(c, o) for c in CACHES for o in OPS] + [("ingest", "insert")]
    )
    out["ops_rayon_num_threads_1"] = pack(one, [("warm", op) for op in OPS])
    dest = Path("/tmp/db23-ab.json")
    dest.write_text(json.dumps(out, indent=2) + "\n")
    print("wrote", dest, flush=True)


if __name__ == "__main__":
    main()
