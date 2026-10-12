#!/usr/bin/env python3
"""Five paired, interleaved repeats of ingest and queries.

Thread environment variables are pinned to 1 before any child starts.
Each timed child checks hit counts and index sums before it returns a time.
The summary of each series is the median, the minimum, and the maximum.
"""

from __future__ import annotations

import argparse
import csv
import gzip
import json
import os
import platform
import subprocess
import sys
import time
from pathlib import Path

import zstandard

from stats import spread

ROOT = Path(__file__).resolve().parents[2]
EVAL = ROOT / "eval"
HARNESS = EVAL / "harness" / "target" / "release" / "cpc-eval-harness"
RESULTS = EVAL / "results"
WORK = EVAL / "work"
SYSTEMS = ("readcon-db", "ase.db", "con-directory")
INGEST_SYSTEMS = ("readcon-db", "ase.db", "ase.db-txn")
QUERIES = ("composition", "energy", "fmax")

ENV = os.environ.copy()
ENV.update(
    {
        "OMP_NUM_THREADS": "1",
        "OPENBLAS_NUM_THREADS": "1",
        "MKL_NUM_THREADS": "1",
        "NUMEXPR_NUM_THREADS": "1",
        "RAYON_NUM_THREADS": "1",
        "VECLIB_MAXIMUM_THREADS": "1",
    }
)


def scrub(text: str) -> str:
    return " ".join("PATH" if word.startswith("/") else word for word in text.split())


def rel(path: Path) -> str:
    return str(path.relative_to(ROOT))


def rotate(items, shift):
    k = shift % len(items)
    return items[k:] + items[:k]


def call(argv: list[str], timeout: float) -> dict:
    # The child runs in its own group so a timeout can stop it.
    proc = subprocess.Popen(
        argv,
        cwd=ROOT,
        env=ENV,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    try:
        stdout, stderr = proc.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        os.killpg(proc.pid, 9)
        proc.communicate()
        return {"ok": False, "error": f"timeout after {timeout:.0f}s"}
    lines = [line for line in (stdout or "").splitlines() if line.strip()]
    if not lines:
        return {"ok": False, "error": scrub(stderr or "") or "no output"}
    try:
        data = json.loads(lines[-1])
    except json.JSONDecodeError:
        return {"ok": False, "error": scrub(lines[-1])}
    if proc.returncode != 0:
        data["ok"] = False
        if "error" not in data:
            data["error"] = scrub(stderr or "") or "child failed"
    return data


def fadvise(path: Path):
    fd = os.open(path, os.O_RDONLY)
    try:
        os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
    finally:
        os.close(fd)


def fadvise_tree(directory: Path):
    for path in sorted(directory.glob("*.con")):
        fadvise(path)


def try_drop_caches() -> bool:
    try:
        with open("/proc/sys/vm/drop_caches", "w", encoding="ascii") as handle:
            handle.write("3\n")
        return True
    except OSError:
        return False


def tree_bytes(directory: Path) -> tuple[int, int]:
    apparent = 0
    allocated = 0
    for path in directory.rglob("*"):
        if path.is_file():
            st = path.stat()
            apparent += st.st_size
            allocated += st.st_blocks * 512
    return apparent, allocated


def compress_sizes(path: Path) -> tuple[int, int, int]:
    raw = path.read_bytes()
    gz = gzip.compress(raw, compresslevel=6)
    zstd = zstandard.ZstdCompressor(level=3).compress(raw)
    return len(raw), len(gz), len(zstd)


def read_cpu() -> str:
    name = platform.processor() or "unknown"
    try:
        text = Path("/proc/cpuinfo").read_text(errors="replace")
    except OSError:
        return name
    for line in text.splitlines():
        if line.lower().startswith("model name"):
            return line.split(":", 1)[1].strip()
    return name


def read_mem_gb() -> float:
    try:
        text = Path("/proc/meminfo").read_text()
    except OSError:
        return 0.0
    for line in text.splitlines():
        if line.startswith("MemTotal:"):
            kb = float(line.split()[1])
            return kb / 1024.0 / 1024.0
    return 0.0


def tool_version(argv: list[str]) -> str:
    try:
        proc = subprocess.run(argv, text=True, capture_output=True, check=False)
    except OSError:
        return "missing"
    line = (proc.stdout or proc.stderr).strip().splitlines()
    return line[0] if line else "missing"


def cargo_version(crate: str) -> str:
    text = (ROOT / "Cargo.toml").read_text()
    for line in text.splitlines():
        if line.startswith("version") and crate == "readcon-db":
            return line.split("=", 1)[1].strip().strip('"')
        if crate in line and "path" not in line:
            # dependency pins look like `readcon-core = "=0.16.2"`
            if line.strip().startswith(crate):
                return line.split("=", 1)[1].strip().strip('"')
    return "unknown"


def hardware() -> dict:
    import ase
    import matplotlib
    import numpy
    import readcon

    return {
        "cpu_model": read_cpu(),
        "ncpu": os.cpu_count(),
        "mem_gb": round(read_mem_gb(), 2),
        "python": platform.python_version(),
        "rustc": tool_version(["rustc", "--version"]),
        "readcon_db": cargo_version("readcon-db"),
        "readcon_core": getattr(readcon, "__version__", None) or cargo_version("readcon-core"),
        "ase": ase.__version__,
        "numpy": numpy.__version__,
        "matplotlib": matplotlib.__version__,
        "zstandard": zstandard.__version__,
        "threads": {k: ENV[k] for k in ENV if k.endswith("_THREADS") or k.endswith("_NUM_THREADS")},
        "page_cache_drop": try_drop_caches(),
        "repeats": 5,
        "summary": "median, minimum, and maximum of the paired interleaved repeats",
        "megabyte": "1e6 bytes",
        "repeats_are_paired_and_interleaved": True,
    }


def write_csv(path: Path, rows: list[dict], fieldnames: list[str]):
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", newline="") as handle:
        writer = csv.DictWriter(handle, fieldnames=fieldnames)
        writer.writeheader()
        for row in rows:
            writer.writerow(row)


def add_summary(rows, kind, n_frames, system, query, cache, values):
    stat = spread(values)
    rows.append(
        {
            "kind": kind,
            "n_frames": n_frames,
            "system": system,
            "query": query,
            "cache": cache,
            "n": stat["n"],
            "median": fmt(stat["median"]),
            "minimum": fmt(stat["minimum"]),
            "maximum": fmt(stat["maximum"]),
        }
    )


def fmt(value):
    if value is None:
        return ""
    return f"{value:.8g}"


def ingest_one(system: str, scale: Path, oracle: Path, n_frames: int, timeout: float):
    if system == "readcon-db":
        dest = WORK / f"rdb-{n_frames}"
        data = call(
            [
                str(HARNESS),
                "ingest",
                "--con",
                rel(scale / "all.con"),
                "--dest",
                rel(dest),
                "--oracle",
                rel(oracle),
            ],
            timeout,
        )
        return data, dest
    if system in ("ase.db", "ase.db-txn"):
        db = WORK / (f"ase-txn-{n_frames}.db" if system == "ase.db-txn" else f"ase-{n_frames}.db")
        argv = [
            sys.executable,
            rel(EVAL / "scripts" / "bench_ase.py"),
            "ingest",
            "--con",
            rel(scale / "all.con"),
            "--db",
            rel(db),
            "--oracle",
            rel(oracle),
        ]
        if system == "ase.db-txn":
            argv.append("--transaction")
        data = call(argv, timeout)
        return data, db
    return {"ok": True, "seconds": None}, scale / "files"


def query_one(system: str, store: Path, oracle: Path, only: str, n_frames: int, timeout: float):
    if system == "readcon-db":
        fadvise(store / "data.mdb")
        return call(
            [
                str(HARNESS),
                "query",
                "--corpus",
                rel(store),
                "--oracle",
                rel(oracle),
                "--only",
                only,
            ],
            timeout,
        )
    if system == "ase.db":
        fadvise(store)
        return call(
            [
                sys.executable,
                rel(EVAL / "scripts" / "bench_ase.py"),
                "query",
                "--db",
                rel(store),
                "--oracle",
                rel(oracle),
                "--only",
                only,
            ],
            timeout,
        )
    fadvise_tree(store)
    return call(
        [
            str(HARNESS),
            "scan",
            "--chunks",
            rel(store),
            "--oracle",
            rel(oracle),
            "--only",
            only,
        ],
        timeout,
    )


def cleanup(system: str, store: Path):
    if system == "readcon-db" and store.exists():
        subprocess.run(["rm", "-rf", str(store)], check=False)
    elif system in ("ase.db", "ase.db-txn") and store.exists():
        store.unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--sizes", default="1000,10000")
    parser.add_argument("--repeats", type=int, default=5)
    parser.add_argument("--repeat-start", type=int, default=0)
    parser.add_argument("--append", action="store_true")
    parser.add_argument("--child-timeout", type=float, default=90)
    parser.add_argument("--dedup", action="store_true")
    parser.add_argument("--ingest-only", action="store_true")
    parser.add_argument(
        "--ase-full-through",
        type=int,
        default=10**12,
        help="Frame counts at or below this get ase.db on every repeat",
    )
    parser.add_argument(
        "--ase-repeats-above",
        type=int,
        default=30,
        help="How many leading repeats include ase.db once the frame count is above --ase-full-through",
    )
    args = parser.parse_args()
    if not HARNESS.is_file():
        raise SystemExit("build eval/harness in release mode first")
    sizes = [int(s) for s in args.sizes.split(",") if s]
    RESULTS.mkdir(parents=True, exist_ok=True)
    WORK.mkdir(parents=True, exist_ok=True)

    if not args.ingest_only:
        hw = hardware()
        hw["repeats"] = args.repeats
        (RESULTS / "hardware.json").write_text(json.dumps(hw, indent=2) + "\n")
        correctness = call(
            [
                str(HARNESS),
                "correctness",
                "--work",
                rel(WORK / "correctness"),
                "--out",
                rel(RESULTS / "correctness.json"),
            ],
            args.child_timeout,
        )
        print(
            f"correctness pass={correctness.get('n_pass')} fail={correctness.get('n_fail')}",
            flush=True,
        )

    keep_existing = args.append or args.ingest_only
    ingest_rows = load_rows(RESULTS / "ingest_repeats.csv") if keep_existing else []
    query_rows = load_rows(RESULTS / "query_repeats.csv") if keep_existing else []
    size_rows = load_rows(RESULTS / "sizes.csv") if keep_existing else []
    repeat_ids = list(range(args.repeat_start, args.repeat_start + args.repeats))
    if args.ingest_only:
        ingest_rows = [
            row
            for row in ingest_rows
            if not (int(row["n_frames"]) in sizes and int(row["repeat"]) in repeat_ids)
        ]

    for n_frames in sizes:
        scale = EVAL / "data" / "scale" / f"n{n_frames}"
        oracle_path = scale / "oracle.json"
        if not (scale / "all.con").is_file():
            print(f"missing scale n{n_frames}, skipping", flush=True)
            continue
        if not oracle_path.is_file():
            made = call(
                [
                    str(HARNESS),
                    "oracle",
                    "--chunks",
                    rel(scale / "files"),
                    "--all",
                    rel(scale / "all.con"),
                    "--out",
                    rel(oracle_path),
                ],
                args.child_timeout,
            )
            if made.get("ok") is False or "n_frames" not in made and not oracle_path.is_file():
                print(f"oracle failed for n{n_frames}: {made.get('error')}", flush=True)
                continue
        oracle = json.loads(oracle_path.read_text())
        raw_b, gz_b, zst_b = compress_sizes(scale / "all.con")
        dir_app, dir_alloc = tree_bytes(scale / "files")
        upsert_fixed(size_rows, n_frames, "raw_con", raw_b, raw_b, "multi-frame CON")
        upsert_fixed(size_rows, n_frames, "gzip", gz_b, gz_b, "gzip level 6")
        upsert_fixed(size_rows, n_frames, "zstd", zst_b, zst_b, "zstd level 3")
        upsert_fixed(size_rows, n_frames, "con_directory", dir_app, dir_alloc, "chunk files")
        print(
            f"n{n_frames} oracle frames={oracle['n_frames']} formula={oracle['composition']['formula']} "
            f"comp={oracle['composition']['n']} energy={oracle['energy']['n']} fmax={oracle['fmax']['n']}",
            flush=True,
        )

        for rep in repeat_ids:
            if args.ingest_only:
                order = rotate(INGEST_SYSTEMS, rep)
            else:
                use_ase = n_frames <= args.ase_full_through or rep < args.ase_repeats_above
                base = SYSTEMS if use_ase else ("readcon-db", "con-directory")
                order = rotate(base, rep)
            qord = rotate(QUERIES, rep)
            stores = {}
            wall0 = time.perf_counter()
            print(f"  repeat {rep} order={','.join(order)}", flush=True)
            for position, system in enumerate(order):
                if system == "con-directory":
                    stores[system] = scale / "files"
                    continue
                data, store = ingest_one(system, scale, oracle_path, n_frames, args.child_timeout)
                stores[system] = store
                ok = bool(data.get("ok"))
                ingest_rows.append(
                    {
                        "repeat": rep,
                        "n_frames": n_frames,
                        "system": system,
                        "position": position,
                        "seconds": data.get("seconds", ""),
                        "frames_per_s": data.get("frames_per_s", ""),
                        "mb_per_s": data.get("mb_per_s", ""),
                        "raw_bytes": data.get("raw_bytes", ""),
                        "apparent_bytes": data.get("apparent_bytes", ""),
                        "allocated_bytes": data.get("allocated_bytes", ""),
                        "ok": ok,
                        "error": "" if ok else scrub(str(data.get("error", ""))),
                    }
                )
                if not ok:
                    print(f"    ingest {system} failed: {data.get('error')}", flush=True)
            if not args.ingest_only:
                for query in qord:
                    for system in order:
                        store = stores.get(system)
                        if store is None or (system != "con-directory" and not ingest_ok(ingest_rows, rep, n_frames, system)):
                            continue
                        data = query_one(system, store, oracle_path, query, n_frames, args.child_timeout)
                        if not data.get("ok"):
                            for cache in ("cold", "warm"):
                                query_rows.append(fail_query(rep, n_frames, system, query, cache, data.get("error", "")))
                            print(f"    query {system} {query} failed: {data.get('error')}", flush=True)
                            continue
                        for cache in ("cold", "warm"):
                            block = data[cache][query]
                            query_rows.append(
                                {
                                    "repeat": rep,
                                    "n_frames": n_frames,
                                    "system": system,
                                    "query": query,
                                    "cache": cache,
                                    "seconds": block["seconds"],
                                    "hits": block["hits"],
                                    "index_sum": block["index_sum"],
                                    "ok": True,
                                    "error": "",
                                }
                            )
            for system, store in stores.items():
                if system != "con-directory":
                    cleanup(system, store)
            wall = time.perf_counter() - wall0
            refresh_db_sizes(size_rows, ingest_rows)
            dump_tables(ingest_rows, query_rows, size_rows)
            write_summary(ingest_rows, query_rows)
            print(f"  repeat {rep} n{n_frames} wall {wall:.1f}s", flush=True)
            if wall > 120:
                print(f"  repeat {rep} n{n_frames} exceeded 120s", flush=True)

    if args.dedup:
        dedup_path = EVAL / "data" / "campaign" / "akmc_trace.con"
        if dedup_path.is_file() and dedup_path.stat().st_size > 0:
            dedup = call(
                [
                    str(HARNESS),
                    "dedup",
                    "--con",
                    rel(dedup_path),
                    "--dest",
                    rel(WORK / "dedup-corpus"),
                ],
                args.child_timeout,
            )
            (RESULTS / "dedup.json").write_text(json.dumps(dedup, indent=2) + "\n")
            print(f"dedup ok={dedup.get('ok')} unique={dedup.get('n_unique_hashes')} frames={dedup.get('n_frames')}", flush=True)
        else:
            print("no aKMC trace to measure", flush=True)
    write_summary(ingest_rows, query_rows)
    print("wrote results", flush=True)


def ingest_ok(rows, rep, n_frames, system) -> bool:
    for row in reversed(rows):
        if row["repeat"] == rep and row["n_frames"] == n_frames and row["system"] == system:
            return bool(row["ok"])
    return False


def fail_query(rep, n_frames, system, query, cache, error):
    return {
        "repeat": rep,
        "n_frames": n_frames,
        "system": system,
        "query": query,
        "cache": cache,
        "seconds": "",
        "hits": "",
        "index_sum": "",
        "ok": False,
        "error": scrub(str(error)),
    }


def fixed_size(n_frames, store, apparent, allocated, note):
    return {
        "n_frames": n_frames,
        "store": store,
        "apparent_bytes": apparent,
        "allocated_bytes": allocated,
        "allocated_minimum": allocated,
        "allocated_maximum": allocated,
        "note": note,
    }


def median_size(n_frames, store, apparent, allocated, note):
    ap = spread(apparent)
    al = spread(allocated)
    return {
        "n_frames": n_frames,
        "store": store,
        "apparent_bytes": round(ap["median"]),
        "allocated_bytes": round(al["median"]),
        "allocated_minimum": round(al["minimum"]),
        "allocated_maximum": round(al["maximum"]),
        "note": note,
    }


def load_rows(path: Path) -> list[dict]:
    if not path.is_file():
        return []
    with path.open(newline="") as handle:
        rows = list(csv.DictReader(handle))
    for row in rows:
        if "ok" in row:
            row["ok"] = str(row["ok"]).lower() == "true"
        for key in ("repeat", "n_frames", "position"):
            if key in row and row[key] not in ("", None):
                row[key] = int(row[key])
    return rows


def upsert_fixed(rows, n_frames, store, apparent, allocated, note):
    row = fixed_size(n_frames, store, apparent, allocated, note)
    for i, old in enumerate(rows):
        if int(old["n_frames"]) == int(n_frames) and old["store"] == store:
            rows[i] = row
            return
    rows.append(row)


def refresh_db_sizes(size_rows, ingest_rows):
    kept = [row for row in size_rows if row["store"] not in ("readcon-db", "ase.db", "ase.db-txn")]
    size_rows[:] = kept
    pairs = sorted({(int(row["n_frames"]), row["system"]) for row in ingest_rows})
    for n_frames, system in pairs:
        apparent = []
        allocated = []
        for row in ingest_rows:
            if not row["ok"] or int(row["n_frames"]) != n_frames or row["system"] != system:
                continue
            if row.get("apparent_bytes") in ("", None) or row.get("allocated_bytes") in ("", None):
                continue
            apparent.append(float(row["apparent_bytes"]))
            allocated.append(float(row["allocated_bytes"]))
        if not apparent:
            continue
        if system == "readcon-db":
            note = "median file size over ingests; the map ceiling is 2 GiB and is not the file size"
        elif system == "ase.db-txn":
            note = "median over ingests; writes held in one ASE connection"
        else:
            note = "median over ingests"
        size_rows.append(median_size(n_frames, system, apparent, allocated, note))


def dump_tables(ingest_rows, query_rows, size_rows):
    write_csv(
        RESULTS / "ingest_repeats.csv",
        ingest_rows,
        ["repeat", "n_frames", "system", "position", "seconds", "frames_per_s", "mb_per_s", "raw_bytes", "apparent_bytes", "allocated_bytes", "ok", "error"],
    )
    write_csv(
        RESULTS / "query_repeats.csv",
        query_rows,
        ["repeat", "n_frames", "system", "query", "cache", "seconds", "hits", "index_sum", "ok", "error"],
    )
    write_csv(
        RESULTS / "sizes.csv",
        size_rows,
        ["n_frames", "store", "apparent_bytes", "allocated_bytes", "allocated_minimum", "allocated_maximum", "note"],
    )


def values_where(rows, **want):
    out = []
    for row in rows:
        if not row.get("ok"):
            continue
        if any(row.get(key) != val for key, val in want.items()):
            continue
        if row.get("seconds") in ("", None):
            continue
        out.append(float(row["seconds"]))
    return out


def write_summary(ingest_rows, query_rows):
    summary = []
    sizes = sorted({int(row["n_frames"]) for row in ingest_rows} | {int(row["n_frames"]) for row in query_rows})
    for n_frames in sizes:
        for system in ("readcon-db", "ase.db", "ase.db-txn"):
            seconds = [
                float(row["seconds"])
                for row in ingest_rows
                if row["ok"] and int(row["n_frames"]) == n_frames and row["system"] == system
            ]
            rates = [
                float(row["frames_per_s"])
                for row in ingest_rows
                if row["ok"] and int(row["n_frames"]) == n_frames and row["system"] == system
            ]
            bws = [
                float(row["mb_per_s"])
                for row in ingest_rows
                if row["ok"] and int(row["n_frames"]) == n_frames and row["system"] == system
            ]
            add_summary(summary, "ingest_s", n_frames, system, "", "", seconds)
            add_summary(summary, "frames_per_s", n_frames, system, "", "", rates)
            add_summary(summary, "mb_per_s", n_frames, system, "", "", bws)
            fail_seconds = [
                float(row["seconds"])
                for row in ingest_rows
                if (not row["ok"])
                and int(row["n_frames"]) == n_frames
                and row["system"] == system
                and row.get("seconds") not in ("", None)
            ]
            if fail_seconds:
                add_summary(summary, "ingest_fail_s", n_frames, system, "", "", fail_seconds)
        for query in QUERIES:
            for cache in ("cold", "warm"):
                for system in SYSTEMS:
                    seconds = [
                        float(row["seconds"])
                        for row in query_rows
                        if row["ok"]
                        and int(row["n_frames"]) == n_frames
                        and row["system"] == system
                        and row["query"] == query
                        and row["cache"] == cache
                    ]
                    add_summary(summary, "query_s", n_frames, system, query, cache, seconds)
        for query in QUERIES:
            for cache in ("cold", "warm"):
                paired_ratio(summary, query_rows, n_frames, query, cache, "readcon-db", "ase.db")
                paired_ratio(summary, query_rows, n_frames, query, cache, "readcon-db", "con-directory")
        paired_ingest(summary, ingest_rows, n_frames, "readcon-db", "ase.db")
        paired_ingest(summary, ingest_rows, n_frames, "readcon-db", "ase.db-txn")
    write_csv(
        RESULTS / "summary.csv",
        summary,
        ["kind", "n_frames", "system", "query", "cache", "n", "median", "minimum", "maximum"],
    )


def paired_ratio(summary, rows, n_frames, query, cache, left, right):
    by_rep = {}
    for row in rows:
        if not row["ok"] or int(row["n_frames"]) != n_frames or row["query"] != query or row["cache"] != cache:
            continue
        by_rep.setdefault(row["repeat"], {})[row["system"]] = float(row["seconds"])
    ratios = []
    for pair in by_rep.values():
        if left in pair and right in pair and pair[right] > 0:
            ratios.append(pair[left] / pair[right])
    add_summary(summary, "paired_time_ratio", n_frames, f"{left}/{right}", query, cache, ratios)


def paired_ingest(summary, rows, n_frames, left, right):
    by_rep = {}
    for row in rows:
        if not row["ok"] or int(row["n_frames"]) != n_frames:
            continue
        by_rep.setdefault(row["repeat"], {})[row["system"]] = float(row["seconds"])
    ratios = []
    for pair in by_rep.values():
        if left in pair and right in pair and pair[right] > 0:
            ratios.append(pair[left] / pair[right])
    add_summary(summary, "paired_time_ratio", n_frames, f"{left}/{right}", "", "", ratios)


if __name__ == "__main__":
    main()
