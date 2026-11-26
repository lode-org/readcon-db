"""Wall time for one Cu107 trajectory, two routes, one process.

Route A loads the GRO topology, decodes every XTC frame, and keeps frames
whose composition is Cu107. XTC has no forces; that half is
unanswerable_from_xtc.

Route B calls ConCorpus.select(formula="Cu:107", require_forces=True) and
materialises positions and forces from the cooked per-frame records.

Conversion and ingest are not timed. Repeats alternate A, B. Two warmups
are discarded, then 30 timed repeats per route. Warm-cache repeats are
measured first. Cold repeats drop the page cache before each call.
"""

from __future__ import annotations

import argparse
import csv
import gc
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path

os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", "1")
os.environ.setdefault("NUMEXPR_NUM_THREADS", "1")

import numpy as np

import MDAnalysis as mda

from readcon_db import ConCorpus

from generate import FIXTURE, N_ATOMS, frame_geometry, load_seed, write_con

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
FORMULA = "Cu:107"
BOOTSTRAP_SEED = 20261123
BOOTSTRAP_DRAWS = 10000
DEFAULT_FRAMES = 120_000


def drop_caches() -> None:
    subprocess.run(["sync"], check=True)
    proc = subprocess.run(
        ["sudo", "tee", "/proc/sys/vm/drop_caches"],
        input=b"3\n",
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
        check=False,
    )
    if proc.returncode != 0:
        detail = proc.stderr.decode("utf-8", "replace").strip()
        raise SystemExit(f"drop_caches not permitted: {detail}")


def cpu_model() -> str:
    with open("/proc/cpuinfo", encoding="utf-8") as handle:
        for line in handle:
            if line.startswith("model name"):
                return line.split(":", 1)[1].strip()
    return "unknown"


def core_count() -> int:
    count = 0
    with open("/proc/cpuinfo", encoding="utf-8") as handle:
        for line in handle:
            if line.startswith("processor"):
                count += 1
    return count


def mem_total_bytes() -> int:
    with open("/proc/meminfo", encoding="utf-8") as handle:
        for line in handle:
            if line.startswith("MemTotal:"):
                return int(line.split()[1]) * 1024
    raise RuntimeError("MemTotal missing")


def os_name() -> str:
    name = "unknown"
    with open("/etc/os-release", encoding="utf-8") as handle:
        for line in handle:
            if line.startswith("PRETTY_NAME="):
                name = line.split("=", 1)[1].strip().strip('"')
    return name


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def git_sha() -> str:
    """`git rev-parse` of the commit that was HEAD when this checkout's build was imported.

    The extension must be the editable maturin build of this checkout, and
    its bytes must match `target/release/libreadcon_db.so`. The recorded
    commit is the latest one whose committer time is not after that file's
    mtime, which is HEAD at build time.
    """
    import readcon_db

    installed_dir = Path(readcon_db.__file__).resolve().parent
    installed = sorted(installed_dir.glob("*.so"))
    if len(installed) != 1:
        raise SystemExit("imported readcon_db does not contain one extension")
    built = ROOT / "target" / "release" / "libreadcon_db.so"
    if not built.is_file():
        raise SystemExit("this checkout has no release build of the extension")
    if _sha256(installed[0]) != _sha256(built):
        raise SystemExit("imported extension does not match this checkout's release build")
    dist = installed_dir.parent / "readcon_db-0.2.4.dist-info" / "direct_url.json"
    info = json.loads(dist.read_text(encoding="utf-8"))
    if not info.get("dir_info", {}).get("editable"):
        raise SystemExit("imported readcon_db is not an editable maturin build")
    url = str(info.get("url", ""))
    if not url.startswith("file://"):
        raise SystemExit("imported readcon_db does not point at a local checkout")
    if Path(url[7:]).resolve() != (ROOT / "python").resolve():
        raise SystemExit("imported readcon_db is not this checkout")
    when = time.strftime("%Y-%m-%d %H:%M:%S +0000", time.gmtime(installed[0].stat().st_mtime))
    listed = subprocess.run(
        ["git", "rev-list", "-1", f"--before={when}", "HEAD"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    resolved = subprocess.run(
        ["git", "rev-parse", listed.stdout.strip()],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return resolved.stdout.strip()


def readcon_core_version() -> str:
    lock = (ROOT / "Cargo.lock").read_text(encoding="utf-8")
    marker = 'name = "readcon-core"\n'
    start = lock.find(marker)
    if start < 0:
        raise RuntimeError("readcon-core is not in Cargo.lock")
    window = lock[start : start + 400]
    for line in window.splitlines():
        if line.startswith("version = "):
            return line.split("=", 1)[1].strip().strip('"')
    raise RuntimeError("readcon-core version missing")


def disk_bytes(path: Path) -> int:
    if path.is_file():
        return path.stat().st_blocks * 512
    total = 0
    for item in path.rglob("*"):
        if item.is_file():
            total += item.stat().st_blocks * 512
    return total


def write_xtc_gro(seed: dict, n_frames: int, xtc: Path, gro: Path) -> None:
    universe = mda.Universe.empty(N_ATOMS, trajectory=True)
    universe.add_TopologyAttr("name", ["Cu"] * N_ATOMS)
    universe.add_TopologyAttr("resname", ["CU"])
    universe.add_TopologyAttr("resid", [1])
    universe.add_TopologyAttr("type", ["Cu"] * N_ATOMS)
    positions, _forces = frame_geometry(seed, 0)
    universe.atoms.positions = np.asarray(positions, dtype=np.float64)
    universe.dimensions = np.array([40.0, 40.0, 40.0, 90.0, 90.0, 90.0])
    with mda.Writer(str(xtc), N_ATOMS) as writer:
        for frame_index in range(n_frames):
            positions, _forces = frame_geometry(seed, frame_index)
            universe.atoms.positions = np.asarray(positions, dtype=np.float64)
            writer.write(universe.atoms)
    universe.atoms.positions = np.asarray(frame_geometry(seed, 0)[0], dtype=np.float64)
    universe.atoms.write(str(gro))


def is_cu107(universe: mda.Universe) -> bool:
    # GRO stores atom names. Cu107 is 107 atoms whose name is Cu.
    names = universe.atoms.names
    if len(names) != N_ATOMS:
        return False
    for name in names:
        if name != "Cu":
            return False
    return True


def route_a(gro: Path, xtc: Path) -> tuple[list[int], float]:
    universe = mda.Universe(str(gro), str(xtc))
    selected: list[int] = []
    acc = 0.0
    for timestep in universe.trajectory:
        positions = timestep.positions
        if is_cu107(universe):
            selected.append(int(timestep.frame))
            acc += float(positions[0, 0]) + float(positions[-1, -1])
    return selected, acc


def route_b(db: ConCorpus) -> tuple[list[int], float]:
    keys = db.select(formula=FORMULA, require_forces=True)
    selected: list[int] = []
    acc = 0.0
    for traj_id, frame_idx in keys:
        positions = db.get_positions(traj_id, frame_idx)
        forces = db.get_forces(traj_id, frame_idx)
        if forces is None:
            raise RuntimeError(f"frame {frame_idx} has no forces")
        selected.append(int(frame_idx))
        acc += float(positions[0][0]) + float(forces[0][0])
        acc += float(positions[-1][2]) + float(forces[-1][2])
    return selected, acc


def check_agreement(db: ConCorpus, seed: dict, n_frames: int, gro: Path, xtc: Path) -> dict:
    selected_a, _acc = route_a(gro, xtc)
    keys = db.select(formula=FORMULA, require_forces=True)
    selected_b = [int(frame_idx) for _traj, frame_idx in keys]
    expected = list(range(n_frames))
    max_abs = 0.0
    for frame_index in range(n_frames):
        _positions, forces = frame_geometry(seed, frame_index)
        got = db.get_forces(1, frame_index)
        if got is None:
            raise RuntimeError(f"stored frame {frame_index} has no forces")
        for atom in range(N_ATOMS):
            for axis in range(3):
                diff = abs(float(got[atom][axis]) - forces[atom][axis])
                if diff > max_abs:
                    max_abs = diff
    return {
        "frame_set_equal": selected_a == selected_b == expected,
        "n_frames_selected": len(selected_b),
        "forces_max_abs_diff": max_abs,
        "route_a_forces": "unanswerable_from_xtc",
    }


def summarize(samples: list[float]) -> dict:
    values = np.asarray(samples, dtype=np.float64)
    q1, median, q3 = np.percentile(values, [25, 50, 75], method="linear")
    return {
        "n": int(values.size),
        "median_s": float(median),
        "q1_s": float(q1),
        "q3_s": float(q3),
        "iqr_s": float(q3 - q1),
        "min_s": float(values.min()),
        "max_s": float(values.max()),
    }


def ratio_ci(samples_a: list[float], samples_b: list[float]) -> dict:
    left = np.asarray(samples_a, dtype=np.float64)
    right = np.asarray(samples_b, dtype=np.float64)
    rng = np.random.default_rng(BOOTSTRAP_SEED)
    draws = np.empty(BOOTSTRAP_DRAWS, dtype=np.float64)
    width = left.size
    for draw in range(BOOTSTRAP_DRAWS):
        index = rng.integers(0, width, size=width)
        draws[draw] = float(np.median(right[index]) / np.median(left[index]))
    low, high = np.percentile(draws, [2.5, 97.5], method="linear")
    return {
        "median_ratio_B_over_A": float(np.median(right) / np.median(left)),
        "ci95_low": float(low),
        "ci95_high": float(high),
        "resamples": BOOTSTRAP_DRAWS,
        "seed": BOOTSTRAP_SEED,
        "paired": True,
    }


def time_call(fn) -> tuple[float, int, float]:
    gc.collect()
    started = time.perf_counter()
    selected, acc = fn()
    elapsed = time.perf_counter() - started
    return elapsed, len(selected), acc


def campaign(
    cache: str,
    rounds: int,
    keep_from: int,
    gro: Path,
    xtc: Path,
    db: ConCorpus,
    rows: list[dict],
) -> None:
    for round_index in range(rounds):
        if cache == "cold":
            drop_caches()
        wall_a, n_a, acc_a = time_call(lambda: route_a(gro, xtc))
        if cache == "cold":
            drop_caches()
        wall_b, n_b, acc_b = time_call(lambda: route_b(db))
        print(
            f"{cache} round {round_index} A {wall_a:.6f} n {n_a} B {wall_b:.6f} n {n_b}",
            flush=True,
        )
        if round_index < keep_from:
            continue
        repeat = round_index - keep_from + 1
        rows.append(
            {
                "route": "A",
                "cache": cache,
                "repeat": repeat,
                "wall_s": wall_a,
                "n_frames_selected": n_a,
                "checksum": acc_a,
            }
        )
        rows.append(
            {
                "route": "B",
                "cache": cache,
                "repeat": repeat,
                "wall_s": wall_b,
                "n_frames_selected": n_b,
                "checksum": acc_b,
            }
        )


def write_csv(path: Path, rows: list[dict]) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=["route", "cache", "repeat", "wall_s", "n_frames_selected"],
        )
        writer.writeheader()
        for row in rows:
            writer.writerow(
                {
                    "route": row["route"],
                    "cache": row["cache"],
                    "repeat": row["repeat"],
                    "wall_s": f"{row['wall_s']:.9f}",
                    "n_frames_selected": row["n_frames_selected"],
                }
            )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--frames", type=int, default=DEFAULT_FRAMES)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--repeats", type=int, default=30)
    parser.add_argument("--caches", default="warm,cold")
    parser.add_argument("--out-dir", type=Path, default=HERE)
    parser.add_argument("--work-dir", type=Path, default=HERE / "work")
    args = parser.parse_args()
    if args.frames < 1:
        raise SystemExit("set --frames or DEFAULT_FRAMES")
    if args.warmups < 0 or args.repeats < 1:
        raise SystemExit("repeats must be positive")
    caches = [item for item in args.caches.split(",") if item]
    work = args.work_dir
    if work.exists() and any(work.iterdir()):
        raise SystemExit(f"work directory is not empty: {work.name}")
    work.mkdir(parents=True, exist_ok=True)
    con_path = work / "cu107.con"
    xtc_path = work / "cu107.xtc"
    gro_path = work / "cu107.gro"
    store = work / "store"

    seed = load_seed()
    print(f"writing {args.frames} frames", flush=True)
    write_con(con_path, args.frames, seed)
    write_xtc_gro(seed, args.frames, xtc_path, gro_path)
    probe = mda.Universe(str(gro_path), str(xtc_path))
    if int(probe.trajectory.n_frames) != args.frames:
        raise SystemExit(
            f"xtc frames {probe.trajectory.n_frames} != {args.frames}"
        )
    del probe

    db = ConCorpus(str(store))
    stored = db.append_trajectory(1, str(con_path))
    if int(stored) != args.frames:
        raise SystemExit(f"ingested {stored} frames, expected {args.frames}")
    cooked = db.recook_all()
    print(f"ingested {stored} cooked {cooked}", flush=True)
    agreement = check_agreement(db, seed, args.frames, gro_path, xtc_path)
    print(
        f"check equal {agreement['frame_set_equal']} "
        f"force_diff {agreement['forces_max_abs_diff']}",
        flush=True,
    )
    if not agreement["frame_set_equal"]:
        raise SystemExit("frame sets differ")
    if agreement["forces_max_abs_diff"] != 0.0:
        raise SystemExit("forces do not match the written source")

    rows: list[dict] = []
    for cache in caches:
        campaign(
            cache,
            args.warmups + args.repeats,
            args.warmups,
            gro_path,
            xtc_path,
            db,
            rows,
        )

    out = args.out_dir
    out.mkdir(parents=True, exist_ok=True)
    write_csv(out / "results.csv", rows)
    summary = {
        "n_frames": args.frames,
        "n_atoms": N_ATOMS,
        "formula": FORMULA,
        "fixture": "resources/test/tiny_cuh2_forces.con",
        "fixture_present": FIXTURE.is_file(),
        "warmups_discarded": args.warmups,
        "repeats": args.repeats,
        "order": "ABAB",
        "checks": agreement,
        "files_bytes_on_disk": {
            "xtc": disk_bytes(xtc_path),
            "gro": disk_bytes(gro_path),
            "readcon_db_store": disk_bytes(store),
        },
        "cpu_model": cpu_model(),
        "core_count": core_count(),
        "ram_bytes": mem_total_bytes(),
        "os": os_name(),
        "python": sys.version.split()[0],
        "mdanalysis": mda.__version__,
        "numpy": np.__version__,
        "readcon_db": {"version": "0.2.4", "git_sha": git_sha()},
        "readcon_core": {
            "version": readcon_core_version(),
            "git_sha": "8c2d9984c40aedfb2e0084fedefaa02e930957f2",
        },
        "by_cache": {},
    }
    for cache in caches:
        samples_a = [row["wall_s"] for row in rows if row["cache"] == cache and row["route"] == "A"]
        samples_b = [row["wall_s"] for row in rows if row["cache"] == cache and row["route"] == "B"]
        summary["by_cache"][cache] = {
            "A": summarize(samples_a),
            "B": summarize(samples_b),
            "ratio": ratio_ci(samples_a, samples_b),
        }
    (out / "results.json").write_text(
        json.dumps(summary, indent=2) + "\n", encoding="utf-8"
    )
    print("wrote results", flush=True)


if __name__ == "__main__":
    main()
