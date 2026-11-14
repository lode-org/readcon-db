"""db29 records on the pushed tip. Scratch stays under build/.

Warm full, range, and selected, plus the cold bulk read, at default
threads and at one thread. Raw versus shuffle-zstd, then the same reads
on corpora written with set_cooked_access. n is the committed default.
"""

from __future__ import annotations

import json
import os
import platform
import shutil
import sys
import time
from pathlib import Path

import numpy as np

REPO = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(Path(__file__).resolve().parent))
import db26_drive as drv
import db28_drive as d28
import fair_ase_protocol as fair

OUT = REPO / "bench" / "db29"
ROOT = REPO / "build" / "db29-ab"
STATE_PATH = REPO / "build" / "db29-state.json"
N = d28.require_recorded_n(d28.N)
MODES = ("raw", "shuffle", "warm-point", "cold-bulk")
EXPECT = {
    "raw": "raw",
    "shuffle": "shuffle-zstd",
    "warm-point": "raw",
    "cold-bulk": "shuffle-zstd",
}
PLAN = (
    ("cold", "full_read"),
    ("warm", "full_read"),
    ("warm", "range"),
    ("warm", "selected"),
)
BOOT_NOTE = d28.BOOT_NOTE


def load_state() -> dict:
    if STATE_PATH.is_file():
        return json.loads(STATE_PATH.read_text())
    return {
        "codecs": {},
        "reads": {},
        "policy": {},
        "bits": {},
        "commit": "",
        "tree": "",
    }


def save_state(state: dict) -> None:
    STATE_PATH.write_text(json.dumps(state) + "\n")


def wait_quiet() -> None:
    for _ in range(18):
        load = os.getloadavg()[0]
        print(f"load {load:.2f}", flush=True)
        if load <= 0.5:
            return
        time.sleep(10)


def low_bit_report(frames: list[np.ndarray]) -> dict:
    values = np.concatenate(
        [np.ascontiguousarray(frame, dtype=np.float64).ravel() for frame in frames]
    )
    bits = values.view(np.uint64)
    lanes = np.ascontiguousarray(bits).view(np.uint8).reshape(-1, 8)
    report = {"n": int(bits.size)}
    for width in (8, 16, 24, 29, 32):
        mask = np.uint64((1 << width) - 1)
        report[f"low{width}_zero_frac"] = float(np.mean((bits & mask) == 0))
    report["byte_lane_zero_frac"] = [float(np.mean(lanes[:, i] == 0)) for i in range(8)]
    return report


def source_widen_report() -> dict:
    import mdtraj

    topo = fair.nha_topology()
    gro = fair.fetch_pinned(fair.NHA_GRO_URL, fair.NHA_GRO_SHA256, "NhaA_non_water.gro")
    xtc = fair.fetch_pinned(fair.NHA_XTC_URL, fair.NHA_XTC_SHA256, "NhaA_non_water.xtc")
    n_frames = fair.arm_n_frames("md")
    indices = fair._md_source_indices(n_frames)
    stride = indices[1] - indices[0]
    traj = mdtraj.load(str(xtc), top=str(gro), stride=stride)
    xyz = np.asarray(traj.xyz[:n_frames], dtype=np.float32)
    widened = xyz.astype(np.float64)
    stored = (widened * 10.0)[:, topo.order, :]
    return {
        "widened_nm": low_bit_report([widened.reshape(-1, 3)]),
        "times_10_angstrom": low_bit_report([stored]),
        "n_frames": int(n_frames),
        "dtype_source": "float32",
    }


def write_mode(text: str, dest: Path, mode: str, n_expect: int) -> dict:
    corpus = dest / "corpus"
    if (corpus / "data.mdb").is_file():
        facts = fair.corpus_arm_record(corpus, n_expect)
        if facts["cooked_codec"] == EXPECT[mode]:
            print(f"reuse {dest} codec={facts['cooked_codec']}", flush=True)
            return facts
        shutil.rmtree(dest)
    if dest.exists():
        shutil.rmtree(dest)
    corpus.mkdir(parents=True)
    os.environ.pop("READCON_DB_COOKED_CODEC", None)
    from readcon_db import ConCorpus

    db = ConCorpus(str(corpus))
    if mode in ("raw", "shuffle"):
        os.environ["READCON_DB_COOKED_CODEC"] = EXPECT[mode]
    else:
        db.set_cooked_access(mode)
    t0 = time.perf_counter()
    n = db.append_trajectory_str(1, text, source="traj.con")
    append_s = time.perf_counter() - t0
    del db
    os.environ.pop("READCON_DB_COOKED_CODEC", None)
    if int(n) != n_expect:
        raise SystemExit(f"{dest} stored {n}")
    facts = fair.corpus_arm_record(corpus, n_expect)
    facts["append_s"] = append_s
    if facts["cooked_codec"] != EXPECT[mode]:
        raise SystemExit(f"{dest} stored {facts['cooked_codec']}")
    print(
        f"wrote {dest.parent.name}/{mode} corpus={facts['corpus_bytes']} "
        f"hit={facts['hit_shuffle']} append_s={append_s:.1f}",
        flush=True,
    )
    return facts


def ensure_corpora(state: dict) -> None:
    fair._load_md_positions = d28.fast_md_positions
    for arm in ("lattice", "md", "thermal"):
        state["codecs"].setdefault(arm, {})
        if all(mode in state["codecs"][arm] for mode in MODES):
            continue
        t0 = time.perf_counter()
        text, records = fair.build_arm(arm)
        print(f"built {arm} in {time.perf_counter() - t0:.1f}s", flush=True)
        fair.refuse_identical_frames([rec["positions"] for rec in records])
        n_expect = fair.arm_n_frames(arm)
        if len(records) != n_expect:
            raise SystemExit(f"{arm} frames {len(records)}")
        state["bits"][arm] = low_bit_report([rec["positions"] for rec in records])
        state["bits"][arm]["energy0"] = all(
            float(rec["energy"]) == 0.0 for rec in records
        )
        profile = fair.arm_profile(arm)
        for mode in MODES:
            if mode in state["codecs"][arm]:
                continue
            facts = write_mode(text, ROOT / arm / mode, mode, n_expect)
            facts.update(profile)
            state["codecs"][arm][mode] = facts
            save_state(state)
        del text, records
        if arm in ("md", "thermal"):
            for mode in ("shuffle", "cold-bulk"):
                if not state["codecs"][arm][mode]["hit_shuffle"]:
                    raise SystemExit(f"{arm} {mode} did not shuffle")
    if "source" not in state["bits"]:
        print("counting XTC low bits", flush=True)
        state["bits"]["source"] = source_widen_report()
        save_state(state)


def pack_samples(left, right, peaks, left_name, right_name) -> dict:
    block = {}
    for name, vals in ((left_name, left), (right_name, right)):
        block[name] = {
            "median_s": drv.median(vals),
            "iqr_s": drv.iqr(vals),
            "samples_s": vals,
            "peak_rss_bytes": peaks[name],
        }
    a = np.asarray(left, dtype=np.float64)
    b = np.asarray(right, dtype=np.float64)
    ratios = b / a
    block["ratio_right_over_left"] = (
        block[right_name]["median_s"] / block[left_name]["median_s"]
        if block[left_name]["median_s"]
        else None
    )
    block["paired"] = {
        "median": float(np.percentile(ratios, 50, method="linear")),
        "ci95": drv.bootstrap(ratios),
        "right_faster": int(np.sum(b < a)),
        "right_slower": int(np.sum(b > a)),
        "ties": int(np.sum(b == a)),
        "n": int(ratios.size),
        "ci_method": BOOT_NOTE,
    }
    return block


def time_pair(arm, left, right, py_path, threads, log) -> dict:
    extra = {"READCON_BENCH_ARM": arm}
    if threads == "1":
        extra["RAYON_NUM_THREADS"] = "1"
    label = threads or "unset"
    workers = {
        left: drv.start_worker(
            left, ROOT / arm / left, py_path, EXPECT[left], dict(extra), ROOT
        ),
        right: drv.start_worker(
            right, ROOT / arm / right, py_path, EXPECT[right], dict(extra), ROOT
        ),
    }
    series = {(cache, op): {left: [], right: []} for cache, op in PLAN}
    try:
        for cache, op in PLAN:
            for i in range(N):
                order = [left, right] if i % 2 == 0 else [right, left]
                for name in order:
                    row = drv.ask(name, workers[name], f"{cache} {op}", ROOT)
                    series[(cache, op)][name].append(row["seconds"])
                line = (
                    f"{arm} {left}/{right} threads={label} {cache} {op} {i} "
                    f"{left}={series[(cache, op)][left][-1]:.6e} "
                    f"{right}={series[(cache, op)][right][-1]:.6e}"
                )
                print(line, flush=True)
                log.write(line + "\n")
                log.flush()
        peaks = {name: d28.hwm(proc.pid) for name, proc in workers.items()}
    finally:
        drv.stop(workers)
    return {
        f"{op}:{cache}": pack_samples(
            series[(cache, op)][left], series[(cache, op)][right], peaks, left, right
        )
        for cache, op in PLAN
    }


def peak_rss(state, arm, mode) -> int:
    group = state["reads"][arm] if mode in ("raw", "shuffle") else state["policy"][arm]
    peaks = [
        block[mode]["peak_rss_bytes"]
        for key in ("threads_unset", "threads_1")
        for block in group[key].values()
    ]
    return max(peaks) if peaks else 0


def _fmt_bits(report: dict) -> str:
    lanes = ", ".join(f"{v:.3f}" for v in report.get("byte_lane_zero_frac", []))
    return (
        f"n={report.get('n')} low32={report.get('low32_zero_frac')} "
        f"low29={report.get('low29_zero_frac')} low24={report.get('low24_zero_frac')} "
        f"lanes=[{lanes}]"
    )


def write_rows(lines, state, section, left_name, right_name) -> None:
    lines.append(
        f"| arm | op | cache | threads | {left_name} s | {right_name} s | paired | 95% CI | faster |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for arm in d28.ARMS:
        for threads, key in (("unset", "threads_unset"), ("1", "threads_1")):
            group = state[section][arm][key]
            for cache, op in PLAN:
                block = group[f"{op}:{cache}"]
                lo, hi = block["paired"]["ci95"]
                lines.append(
                    f"| {arm} | {op} | {cache} | {threads} | "
                    f"{block[left_name]['median_s']:.4e} | {block[right_name]['median_s']:.4e} | "
                    f"{block['paired']['median']:.4f} | [{lo:.4f}, {hi:.4f}] | "
                    f"{block['paired']['right_faster']}/{block['paired']['n']} |"
                )


def write_record(state: dict, log_text: str, dest: Path | None = None) -> None:
    host = {
        "commit": state["commit"],
        "tree": state["tree"],
        "hostname": d28.host_label(d28.NEUTRAL_HOST),
        "system": platform.system(),
        "release": platform.release(),
        "machine": platform.machine(),
        "cpu": drv.fair.cpu_model(),
        "ncpu": os.cpu_count(),
        "loadavg": list(os.getloadavg()),
        "n": N,
        "serial": True,
        "settings": (
            "warm full, range, selected; cold bulk full_read; "
            "threads unset and 1; raw vs shuffle-zstd; "
            "set_cooked_access warm-point vs cold-bulk"
        ),
    }
    record = {
        "commit": state["commit"],
        "tree": state["tree"],
        "host": host,
        "n": N,
        "codecs": state["codecs"],
        "bits": state["bits"],
        "reads": state["reads"],
        "policy": state["policy"],
    }
    dest = OUT if dest is None else dest
    dest.mkdir(parents=True, exist_ok=True)
    (dest / "host.json").write_text(json.dumps(host, indent=2) + "\n")
    (dest / "ab.json").write_text(json.dumps(record, indent=2) + "\n")
    (dest / "campaign.log").write_text(
        log_text if log_text.endswith("\n") else log_text + "\n"
    )
    lines = [
        "# db29 realistic arms",
        "",
        (
            f"Commit `{state['commit']}`, tree `{state['tree']}`. "
            f"Host label `{host['hostname']}`, {host['cpu']}, {host['ncpu']} CPUs. "
            f"n={N}, serial. Warm reads are full, range, and selected. "
            "Cold bulk is the full read. "
            "Threads are the process default and `RAYON_NUM_THREADS=1`. "
            f"The paired CI is a {BOOT_NOTE}. "
            "The ratio is the right-hand layout over the left. Faster counts the right-hand layout."
        ),
        "",
        (
            "raw and shuffle-zstd are `READCON_DB_COOKED_CODEC`. "
            "warm-point and cold-bulk are `set_cooked_access` with that variable unset. "
            "The lattice arm is the best case. Quote md and thermal."
        ),
        "",
        "## Bytes and peak RSS",
        "",
        "| arm | layout | codec stored | atoms/frame | corpus bytes | frames_soa bytes | hit shuffle | peak RSS |",
        "| --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for arm in d28.ARMS:
        for mode in MODES:
            facts = state["codecs"][arm][mode]
            lines.append(
                f"| {arm} | {mode} | {facts['cooked_codec']} | {facts['atoms_per_frame']} | "
                f"{facts['corpus_bytes']} | {facts['frames_soa_value_bytes']} | "
                f"{facts['hit_shuffle']} | {peak_rss(state, arm, mode)} |"
            )
    lines += [
        "",
        "## Float32 XTC widened to float64",
        "",
        (
            "The NhaA XTC stores coordinates as float32 nanometres. The arm widens "
            "them to float64 and multiplies by 10. A widened float32 keeps zeros in "
            "the low bytes. Thermal noise fills those bytes, so the same atoms "
            "compress less. Lane 0 is the least-significant byte."
        ),
        "",
    ]
    bits = state["bits"]
    src = bits.get("source", {})
    for label, report in (
        ("md stored angstrom", bits.get("md", {})),
        ("thermal stored angstrom", bits.get("thermal", {})),
        ("lattice stored angstrom", bits.get("lattice", {})),
        ("md widened nm, before times 10", src.get("widened_nm", {})),
        ("md times 10 angstrom, from the XTC", src.get("times_10_angstrom", {})),
    ):
        if report:
            lines.append(f"- {label}: {_fmt_bits(report)}")
    lines += ["", "## raw vs shuffle-zstd", ""]
    write_rows(lines, state, "reads", "raw", "shuffle")
    lines += [
        "",
        "## set_cooked_access",
        "",
        "warm-point against cold-bulk. The ratio is cold-bulk / warm-point.",
        "",
    ]
    write_rows(lines, state, "policy", "warm-point", "cold-bulk")
    lines.append("")
    (dest / "RESULTS.md").write_text("\n".join(lines))
    print("wrote", dest, flush=True)


def main() -> None:
    os.environ["TMPDIR"] = str(REPO / "build" / "tmp")
    (REPO / "build" / "tmp").mkdir(parents=True, exist_ok=True)
    d28.require_recorded_n(N)
    drv.fair.require_pushed_commit()
    commit = drv.fair.git_out("rev-parse", "HEAD")
    contained = drv.fair.git_out("branch", "-r", "--contains", "HEAD")
    if not contained.strip():
        raise SystemExit("HEAD is not on an origin branch")
    tree = drv.fair.git_out("rev-parse", f"{commit}^{{tree}}")
    py_path = drv.ensure_tree_prefix("tip", commit)
    state = load_state()
    if state.get("commit") not in ("", commit):
        raise SystemExit(f"state is for {state.get('commit')}")
    state["commit"] = commit
    state["tree"] = tree
    print(f"corpora for {commit}", flush=True)
    ensure_corpora(state)
    log_path = REPO / "build" / "db29-campaign.log"
    log = log_path.open("a")
    log.write(f"commit {commit}\n")
    wait_quiet()
    for arm in d28.ARMS:
        state["reads"].setdefault(arm, {})
        state["policy"].setdefault(arm, {})
        for threads, key in ((None, "threads_unset"), ("1", "threads_1")):
            if key not in state["reads"][arm]:
                print(f"timing {arm} raw/shuffle {key}", flush=True)
                state["reads"][arm][key] = time_pair(
                    arm, "raw", "shuffle", py_path, threads, log
                )
                save_state(state)
            if key not in state["policy"][arm]:
                print(f"timing {arm} policy {key}", flush=True)
                state["policy"][arm][key] = time_pair(
                    arm, "warm-point", "cold-bulk", py_path, threads, log
                )
                save_state(state)
    log.write("wrote db29\n")
    log.close()
    write_record(state, log_path.read_text())


if __name__ == "__main__":
    main()
