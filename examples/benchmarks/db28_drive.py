"""db28 records. Same settings on md, thermal, and the lattice best case.

Scratch stays under build/. This does not write bench/db24 through bench/db27.
bench/db28 is created only after the realistic arms are distinct and shuffled.
"""

from __future__ import annotations

import json
import os

os.environ["OMP_NUM_THREADS"] = "1"
os.environ["OPENBLAS_NUM_THREADS"] = "1"
os.environ["MKL_NUM_THREADS"] = "1"
os.environ["NUMEXPR_NUM_THREADS"] = "1"

import argparse
import platform
import sys
import time
from pathlib import Path

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parent))
import db26_drive as drv

REPO = Path(__file__).resolve().parents[2]
BENCH = REPO / "bench" / "db28"
OUTPUTS = ("RESULTS.md", "ab.json", "host.json", "campaign.log")
NEUTRAL_HOST = "host"


def host_label(name: str) -> str:
    """Records use this label. A machine hostname is refused."""
    if name != NEUTRAL_HOST:
        raise ValueError(f"refusing non-neutral host label {name!r}")
    return NEUTRAL_HOST


def _json_hostname(line: str) -> str | None:
    key = '"hostname"'
    if key not in line:
        return None
    after = line.split(key, 1)[1].lstrip()
    if not after.startswith(":"):
        return None
    after = after[1:].lstrip()
    if len(after) < 2 or after[0] != '"':
        return None
    end = after.find('"', 1)
    if end < 0:
        return None
    return after[1:end]


def _finished_record(directory: Path) -> bool:
    """A finished record has its own log and result file, both non-empty."""
    for name in ("campaign.log", "RESULTS.md"):
        path = directory / name
        if not path.is_file() or path.stat().st_size == 0:
            return False
    return True


def replacement_directory(record_dir: Path, root: Path) -> Path | None:
    """The finished directory named by this record's own ``SUPERSEDED.md``.

    A missing path, an empty directory, or a directory that holds only a
    result note does not hide the record. A replacement equal to the
    record root, or nested inside that root, does not hide it either.
    """
    note = record_dir / "SUPERSEDED.md"
    if not note.is_file():
        return None
    named = None
    for line in note.read_text(encoding="utf-8", errors="replace").splitlines():
        text = line.strip()
        prefix = "Replacement:"
        if text.startswith(prefix):
            named = text[len(prefix) :].strip()
            break
    if not named:
        return None
    root_abs = root.resolve()
    dest = (root / named).resolve()
    try:
        dest.relative_to(root_abs)
    except ValueError:
        return None
    # Outside means outside the record root (bench/<record>), including
    # when the note itself sits in a subdirectory of that record.
    bound = _record_root(note, root)
    if bound is None:
        bound = record_dir
    try:
        dest.relative_to(bound.resolve())
    except ValueError:
        pass
    else:
        return None
    if not _finished_record(dest):
        return None
    return dest


def _record_root(path: Path, root: Path) -> Path | None:
    """The top ``bench/<record>`` or ``examples/benchmarks/<record>`` directory."""
    for parts in (("bench",), ("examples", "benchmarks")):
        base = root.joinpath(*parts)
        try:
            rel = path.relative_to(base)
        except ValueError:
            continue
        if len(rel.parts) < 2:
            return None
        return base / rel.parts[0]
    return None


def _superseded(path: Path, root: Path) -> bool:
    record_root = _record_root(path, root)
    if record_root is None:
        return False
    current = path.parent
    while True:
        if replacement_directory(current, root) is not None:
            return True
        if current == record_root:
            return False
        parent = current.parent
        if parent == current:
            return False
        current = parent


def vm_host_hits(root: Path) -> list[str]:
    """Benchmark and bench files that name a VM host or a local path.

    A record is skipped when its own ``SUPERSEDED.md`` names a finished
    replacement outside that directory. The note covers files nested under
    the record. A note above the record directory does not hide it.
    """
    hits = []
    host_line = "host " + "cursor"
    marked = "`" + "cursor" + "`"
    tmp_path = "/" + "tmp" + "/"
    home_path = "/" + "home"
    agent_path = "/" + "agent"
    for base in (root / "examples" / "benchmarks", root / "bench"):
        if not base.is_dir():
            continue
        for path in sorted(base.rglob("*")):
            if not path.is_file() or path.suffix not in {".py", ".md", ".log", ".json"}:
                continue
            if "__pycache__" in path.parts or path.name.startswith("test_"):
                continue
            if _superseded(path, root):
                continue
            text = path.read_text(encoding="utf-8", errors="replace")
            named = False
            for line in text.splitlines():
                host = _json_hostname(line)
                if (
                    line.strip() == host_line
                    or marked in line
                    or tmp_path in line
                    or home_path in line
                    or agent_path in line
                    or (host is not None and host != NEUTRAL_HOST)
                ):
                    named = True
                    break
            if named:
                hits.append(path.relative_to(root).as_posix())
    return hits


N = 12


def require_recorded_n(n: int) -> int:
    """A saved record uses at least 12 paired samples.

    A release rerun (``READCON_RERUN_RELEASE=1``) may use 6.
    """
    floor = 6 if os.environ.get("READCON_RERUN_RELEASE") == "1" else 12
    if n < floor:
        raise ValueError(f"a recorded run needs n>={floor}, not {n}")
    return n


OPS = ("full_read", "selected", "range")
CACHES = ("cold", "warm")
ARMS = ("md", "thermal", "lattice")
BOOT_NOTE = "10000-draw percentile bootstrap, type-7 median, numpy Generator seed 0"


def fast_md_positions(n_frames: int) -> list[np.ndarray]:
    """One strided read of the same frame indices ``_load_md_positions`` uses."""
    import mdtraj

    import fair_ase_protocol as fair

    topo = fair.nha_topology()
    gro = fair.fetch_pinned(fair.NHA_GRO_URL, fair.NHA_GRO_SHA256, "NhaA_non_water.gro")
    xtc = fair.fetch_pinned(fair.NHA_XTC_URL, fair.NHA_XTC_SHA256, "NhaA_non_water.xtc")
    indices = fair._md_source_indices(n_frames)
    stride = indices[1] - indices[0] if n_frames > 1 else 1
    if [i * stride for i in range(n_frames)] != indices:
        raise SystemExit(f"stride {stride} does not match {indices[:4]}")
    traj = mdtraj.load(str(xtc), top=str(gro), stride=stride)
    if traj.n_frames < n_frames:
        raise SystemExit(
            f"strided trajectory has {traj.n_frames} frames, need {n_frames}"
        )
    check = mdtraj.load_frame(str(xtc), indices[0], top=str(gro)).xyz[0]
    frames = []
    for i in range(n_frames):
        angstrom = np.asarray(traj.xyz[i], dtype=np.float64) * 10.0
        frames.append(angstrom[topo.order])
    direct = np.asarray(check, dtype=np.float64) * 10.0
    if not np.array_equal(direct[topo.order], frames[0]):
        raise SystemExit("strided frame 0 does not match load_frame")
    fair.refuse_identical_frames(frames)
    print(f"md frames {n_frames} stride {stride} distinct", flush=True)
    return frames


def hwm(pid: int) -> int:
    try:
        for line in Path(f"/proc/{pid}/status").read_text().splitlines():
            if line.startswith("VmHWM:"):
                return int(line.split()[1]) * 1024
    except OSError:
        return 0
    return 0


def write_corpus(
    arm: str, codec_name: str, py_note: str, root: Path
) -> tuple[dict, list]:
    import fair_ase_protocol as fair

    dest = root / arm / codec_name
    corpus = dest / "corpus"
    if corpus.exists():
        import shutil

        shutil.rmtree(dest)
    corpus.mkdir(parents=True)
    os.environ["READCON_DB_COOKED_CODEC"] = (
        "raw" if codec_name == "raw" else "shuffle-zstd"
    )
    t0 = time.perf_counter()
    text, records = fair.build_arm(arm)
    built_s = time.perf_counter() - t0
    fair.refuse_identical_frames([rec["positions"] for rec in records])
    n_expect = fair.arm_n_frames(arm)
    if len(records) != n_expect:
        raise SystemExit(f"{arm} produced {len(records)} frames, expected {n_expect}")
    from readcon_db import ConCorpus

    db = ConCorpus(str(corpus))
    t1 = time.perf_counter()
    n = db.append_trajectory_str(1, text, source="traj.con")
    append_s = time.perf_counter() - t1
    del db
    if int(n) != n_expect:
        raise SystemExit(f"{arm} {codec_name} stored {n} frames")
    facts = fair.corpus_arm_record(corpus, n_expect)
    profile = fair.arm_profile(arm)
    facts.update(profile)
    facts["build_s"] = built_s
    facts["append_s"] = append_s
    facts["codec_requested"] = os.environ["READCON_DB_COOKED_CODEC"]
    print(
        f"wrote {arm} {codec_name} corpus={facts['corpus_bytes']} "
        f"codec={facts['cooked_codec']} hit={facts['hit_shuffle']} "
        f"atoms={facts['atoms_per_frame']} frame_coord={facts['frame_coord_bytes']}",
        flush=True,
    )
    del py_note
    return facts, records


def run_reads(arm: str, root: Path, py_path: str, threads: str | None, log) -> dict:
    extra = {"READCON_BENCH_ARM": arm}
    if threads == "1":
        extra["RAYON_NUM_THREADS"] = "1"
    label = threads or "unset"
    workers = {
        "raw": drv.start_worker(
            "raw", root / arm / "raw", py_path, "raw", dict(extra), root
        ),
        "shuffle": drv.start_worker(
            "shuffle",
            root / arm / "shuffle",
            py_path,
            "shuffle-zstd",
            dict(extra),
            root,
        ),
    }
    series = {(cache, op): {"raw": [], "shuffle": []} for cache in CACHES for op in OPS}
    try:
        for cache in CACHES:
            for op in OPS:
                for i in range(N):
                    order = ["raw", "shuffle"] if i % 2 == 0 else ["shuffle", "raw"]
                    for name in order:
                        row = drv.ask(name, workers[name], f"{cache} {op}", root)
                        series[(cache, op)][name].append(row["seconds"])
                    line = (
                        f"{arm} threads={label} {cache} {op} {i} "
                        f"raw={series[(cache, op)]['raw'][-1]:.6e} "
                        f"shuffle={series[(cache, op)]['shuffle'][-1]:.6e}"
                    )
                    print(line, flush=True)
                    log.write(line + "\n")
                    log.flush()
        peaks = {name: hwm(proc.pid) for name, proc in workers.items()}
    finally:
        drv.stop(workers)
    packed = {}
    for cache, op in ((c, o) for c in CACHES for o in OPS):
        block = {}
        for name in ("raw", "shuffle"):
            vals = series[(cache, op)][name]
            block[name] = {
                "median_s": drv.median(vals),
                "iqr_s": drv.iqr(vals),
                "samples_s": vals,
                "peak_rss_bytes": peaks[name],
            }
        raw = np.asarray(block["raw"]["samples_s"], dtype=np.float64)
        shuffle = np.asarray(block["shuffle"]["samples_s"], dtype=np.float64)
        ratios = shuffle / raw
        block["ratio_shuffle_over_raw"] = (
            block["shuffle"]["median_s"] / block["raw"]["median_s"]
            if block["raw"]["median_s"]
            else None
        )
        block["paired"] = {
            "median": float(np.percentile(ratios, 50, method="linear")),
            "ci95": drv.bootstrap(ratios),
            "shuffle_faster": int(np.sum(shuffle < raw)),
            "shuffle_slower": int(np.sum(shuffle > raw)),
            "ties": int(np.sum(shuffle == raw)),
            "n": int(ratios.size),
            "ci_method": BOOT_NOTE,
        }
        packed[f"{op}:{cache}"] = block
    return packed


def write_results(directory: Path, record: dict) -> None:
    lines = [
        "# db28 realistic arms at lattice scale",
        "",
        (
            f"Commit `{record['commit']}`, tree `{record['tree']}`. "
            f"Host {record['host']['cpu']}, {record['host']['ncpu']} CPUs. "
            f"{drv.fair.protocol_header(record['host'])} "
            f"n={record['n']}, serial. The same reads run on every arm. "
            "Arm order within a sample is raw, shuffle, reversed on odd indexes. "
            "Warm selected seconds are the mean per frame. "
            "The median is type-7. Parentheses are the type-7 IQR. "
            f"The paired CI is a {BOOT_NOTE}. "
            "The lattice arm is the best case. Quote md and thermal."
        ),
        "",
        (
            "md is the CC-BY 4.0 NhaA membrane trajectory "
            "(Ian Kenney and Shujie Fan, figshare 7185203): 60702 protein, lipid, "
            "and ion atoms, water removed by the authors, 211 strided frames. "
            "thermal is seeded 0.08 angstrom noise on that structure, 211 frames."
        ),
        "",
        "## Bytes and codec",
        "",
        "| arm | role | codec stored | atoms/frame | coord bytes/frame | corpus bytes | frames_soa bytes | stored frame bytes | hit shuffle | peak RSS |",
        "| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |",
    ]
    for arm in ARMS:
        role = "best case" if arm == "lattice" else "realistic"
        for codec in ("raw", "shuffle"):
            facts = record["codecs"][arm][codec]
            peak = record["peak_rss_bytes"][arm][codec]
            lines.append(
                f"| {arm} | {role} | {facts['cooked_codec']} | "
                f"{facts['atoms_per_frame']} | {facts['frame_coord_bytes']} | "
                f"{facts['corpus_bytes']} | {facts['frames_soa_value_bytes']} | "
                f"{facts['stored_frame_bytes']:.0f} | {facts['hit_shuffle']} | {peak} |"
            )
    lines.append("")
    lines.append("## Headline ratios")
    lines.append("")
    lines.append(
        "shuffle/raw. File ratio is corpus bytes. Time ratio is the paired median."
    )
    lines.append("")
    lines.append(
        "| arm | role | file | full cold unset | full warm unset | full cold 1 | full warm 1 |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- | --- |")
    for arm in ARMS:
        role = "best case" if arm == "lattice" else "realistic"
        raw_b = record["codecs"][arm]["raw"]["corpus_bytes"]
        sh_b = record["codecs"][arm]["shuffle"]["corpus_bytes"]
        file_ratio = sh_b / raw_b if raw_b else None

        def cell(group, key):
            paired = group[key]["paired"]
            lo, hi = paired["ci95"]
            return (
                f"{paired['median']:.4f} [{lo:.4f}, {hi:.4f}] "
                f"{paired['shuffle_faster']}/{paired['n']}"
            )

        unset = record["reads"][arm]["threads_unset"]
        one = record["reads"][arm]["threads_1"]
        lines.append(
            f"| {arm} | {role} | {file_ratio:.4f} | "
            f"{cell(unset, 'full_read:cold')} | {cell(unset, 'full_read:warm')} | "
            f"{cell(one, 'full_read:cold')} | {cell(one, 'full_read:warm')} |"
        )
    lines.append("")
    lines.append("## All reads")
    lines.append("")
    lines.append(
        "| arm | op | cache | threads | raw s | shuffle s | shuffle/raw | paired median | 95% CI | faster |"
    )
    lines.append("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |")
    for arm in ARMS:
        for threads, group in (
            ("unset", record["reads"][arm]["threads_unset"]),
            ("1", record["reads"][arm]["threads_1"]),
        ):
            for cache in CACHES:
                for op in OPS:
                    block = group[f"{op}:{cache}"]
                    lo, hi = block["paired"]["ci95"]
                    lines.append(
                        f"| {arm} | {op} | {cache} | {threads} | "
                        f"{block['raw']['median_s']:.4e} ({block['raw']['iqr_s']:.4e}) | "
                        f"{block['shuffle']['median_s']:.4e} ({block['shuffle']['iqr_s']:.4e}) | "
                        f"{block['ratio_shuffle_over_raw']:.4f} | "
                        f"{block['paired']['median']:.4f} | [{lo:.4f}, {hi:.4f}] | "
                        f"{block['paired']['shuffle_faster']}/{block['paired']['n']} |"
                    )
    lines.append("")
    (directory / "RESULTS.md").write_text("\n".join(lines))


def write_outputs(directory: Path, record: dict, log_text: str) -> None:
    """Write the four files a db28 record is made of."""
    require_recorded_n(int(record["n"]))
    record["host"]["hostname"] = host_label(record["host"]["hostname"])
    directory.mkdir(parents=True, exist_ok=True)
    (directory / "host.json").write_text(json.dumps(record["host"], indent=2) + "\n")
    (directory / "ab.json").write_text(json.dumps(record, indent=2) + "\n")
    if not log_text.endswith("\n"):
        log_text += "\n"
    (directory / "campaign.log").write_text(log_text)
    write_results(directory, record)


def _paired(median: float) -> dict:
    return {
        "median": median,
        "ci95": [median, median],
        "shuffle_faster": 0,
        "shuffle_slower": N,
        "ties": 0,
        "n": N,
        "ci_method": BOOT_NOTE,
    }


def example_record() -> dict:
    """A record with the keys ``write_outputs`` lays out. Not a measurement."""
    group = {}
    for cache in CACHES:
        for op in OPS:
            group[f"{op}:{cache}"] = {
                "raw": {
                    "median_s": 1.0,
                    "iqr_s": 0.0,
                    "samples_s": [1.0] * N,
                    "peak_rss_bytes": 1,
                },
                "shuffle": {
                    "median_s": 1.0,
                    "iqr_s": 0.0,
                    "samples_s": [1.0] * N,
                    "peak_rss_bytes": 1,
                },
                "ratio_shuffle_over_raw": 1.0,
                "paired": _paired(1.0),
            }
    codecs = {}
    for arm in ARMS:
        codecs[arm] = {}
        for codec in ("raw", "shuffle"):
            codecs[arm][codec] = {
                "cooked_codec": "raw" if codec == "raw" else "shuffle-zstd",
                "atoms_per_frame": 60702 if arm != "lattice" else 100000,
                "frame_coord_bytes": 1456848,
                "corpus_bytes": 1000,
                "frames_soa_value_bytes": 800,
                "stored_frame_bytes": 100.0,
                "hit_shuffle": codec == "shuffle",
            }
    host = {
        "commit": "0" * 40,
        "tree": "1" * 40,
        "hostname": "host",
        "system": "Linux",
        "release": "0",
        "machine": "x86_64",
        "cpu": "test",
        "ncpu": 1,
        "loadavg": [0.0, 0.0, 0.0],
        "n": N,
        "serial": True,
        "settings": "full, selected, range; cold and warm; threads unset and 1",
    }
    return {
        "commit": host["commit"],
        "tree": host["tree"],
        "host": host,
        "n": N,
        "codecs": codecs,
        "peak_rss_bytes": {arm: {"raw": 1, "shuffle": 1} for arm in ARMS},
        "reads": {arm: {"threads_unset": group, "threads_1": group} for arm in ARMS},
    }


def main() -> None:
    global BENCH, N
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", type=Path, default=None)
    args = ap.parse_args()
    if args.out is not None:
        BENCH = args.out if args.out.is_absolute() else REPO / args.out
    if os.environ.get("READCON_RERUN_RELEASE") == "1":
        raw_n = os.environ.get("READCON_DB28_N", str(N))
        try:
            N = int(raw_n)
        except ValueError as exc:
            raise SystemExit(
                f"READCON_DB28_N must be an integer, not {raw_n!r}"
            ) from exc
        if N < 6:
            raise SystemExit(f"n must be at least 6, not {N}")
    os.environ.update(drv.env_base())
    os.environ.pop("READCON_DB_COOKED_CODEC", None)
    import fair_ase_protocol as fair

    require_recorded_n(N)
    drv.fair.require_pushed_commit()
    commit = drv.fair.git_out("rev-parse", "HEAD")
    contained = drv.fair.git_out("branch", "-r", "--contains", "HEAD")
    if not contained.strip():
        raise SystemExit("HEAD is not contained in any origin branch")
    tree = drv.fair.git_out("rev-parse", f"{commit}^{{tree}}")
    print(f"building wheel for {commit}", flush=True)
    py_path = drv.ensure_tree_prefix("tip", commit)
    fair._load_md_positions = fast_md_positions
    root = REPO / "build" / "db28-ab"
    root.mkdir(parents=True, exist_ok=True)
    codecs = {arm: {} for arm in ARMS}
    for arm in ARMS:
        for codec in ("raw", "shuffle"):
            facts, records = write_corpus(arm, codec, py_path, root)
            codecs[arm][codec] = facts
            if arm in ("md", "thermal"):
                if not facts["hit_shuffle"] and codec == "shuffle":
                    raise SystemExit(
                        f"{arm} shuffle did not hit the shuffle path: {facts}"
                    )
                if facts["cooked_codec"] != (
                    "raw" if codec == "raw" else "shuffle-zstd"
                ):
                    raise SystemExit(f"{arm} {codec} stored {facts['cooked_codec']}")
            del records
    host = {
        "commit": commit,
        "tree": tree,
        "hostname": host_label(NEUTRAL_HOST),
        "system": platform.system(),
        "release": platform.release(),
        "machine": platform.machine(),
        "cpu": drv.fair.cpu_model(),
        "ncpu": os.cpu_count(),
        "loadavg": list(os.getloadavg()),
        "n": N,
        "serial": True,
        "settings": "full, selected, range; cold and warm; threads unset and 1",
        **drv.fair.host_facts(),
    }
    BENCH.mkdir(parents=True, exist_ok=args.out is not None)
    log_path = BENCH / "campaign.log"
    reads = {}
    peaks = {arm: {} for arm in ARMS}
    with log_path.open("w") as log:
        log.write(f"commit {commit}\n")
        for arm in ARMS:
            for codec in ("raw", "shuffle"):
                log.write(f"codec {arm} {codec} {json.dumps(codecs[arm][codec])}\n")
        log.flush()
        for arm in ARMS:
            print(f"measuring {arm}", flush=True)
            unset = run_reads(arm, root, py_path, None, log)
            one = run_reads(arm, root, py_path, "1", log)
            reads[arm] = {"threads_unset": unset, "threads_1": one}
            for codec, key in (("raw", "raw"), ("shuffle", "shuffle")):
                peaks[arm][codec] = max(
                    block[key]["peak_rss_bytes"]
                    for group in (unset, one)
                    for block in group.values()
                )
        record = {
            "commit": commit,
            "tree": tree,
            "host": host,
            "n": N,
            "codecs": codecs,
            "peak_rss_bytes": peaks,
            "reads": reads,
        }
        log.write("wrote bench/db28\n")
    write_outputs(BENCH, record, log_path.read_text())
    print("wrote", BENCH, flush=True)


if __name__ == "__main__":
    main()
