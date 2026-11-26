"""Mixed-composition Cu106/Cu107/Cu108 timing.

One CON trajectory. One XTC+GRO pair per atom count, plus a sidecar that
maps each local frame back to the global index. Building those files is
not timed. Route A loads every topology and decodes every frame of every
file. XTC cannot tell Cu107 with forces from Cu107 without forces.
"""

from __future__ import annotations

import csv
import json
import os

os.environ.setdefault("OMP_NUM_THREADS", "1")
os.environ.setdefault("OPENBLAS_NUM_THREADS", "1")
os.environ.setdefault("MKL_NUM_THREADS", "1")
os.environ.setdefault("NUMEXPR_NUM_THREADS", "1")

import argparse
from pathlib import Path

import MDAnalysis as mda
import numpy as np
from readcon_db import ConCorpus

from generate import frame_geometry, load_seed, mixed_class, mixed_counts, write_mixed_con
from run import (
    FORMULA,
    core_count,
    cpu_model,
    disk_bytes,
    drop_caches,
    git_sha,
    is_cu107,
    mem_total_bytes,
    os_name,
    ratio_ci,
    readcon_core_version,
    summarize,
    time_call,
)

HERE = Path(__file__).resolve().parent
N_FRAMES = 120_000
ATOM_COUNTS = (106, 107, 108)
REPRO = [
    "python3 -m venv .venv",
    ". .venv/bin/activate",
    "pip install maturin 'MDAnalysis>=2.7' numpy",
    "maturin develop --release --bindings pyo3 --manifest-path python/pyproject.toml",
    "python bench/cu107_xtc_vs_db/run_v2.py",
]


def groups(n_frames: int) -> dict[int, list[int]]:
    out = {count: [] for count in ATOM_COUNTS}
    for frame_index in range(n_frames):
        n_atoms, _forces = mixed_class(frame_index)
        out[n_atoms].append(frame_index)
    return out


def write_xtc_gro(seed: dict, indices: list[int], n_atoms: int, xtc: Path, gro: Path) -> None:
    universe = mda.Universe.empty(n_atoms, trajectory=True)
    universe.add_TopologyAttr("name", ["Cu"] * n_atoms)
    universe.add_TopologyAttr("resname", ["CU"])
    universe.add_TopologyAttr("resid", [1])
    universe.add_TopologyAttr("type", ["Cu"] * n_atoms)
    universe.dimensions = np.array([40.0, 40.0, 40.0, 90.0, 90.0, 90.0])
    with mda.Writer(str(xtc), n_atoms) as writer:
        for frame_index in indices:
            positions, _forces = frame_geometry(seed, frame_index, n_atoms)
            universe.atoms.positions = np.asarray(positions, dtype=np.float64)
            writer.write(universe.atoms)
    positions, _forces = frame_geometry(seed, indices[0], n_atoms)
    universe.atoms.positions = np.asarray(positions, dtype=np.float64)
    universe.atoms.write(str(gro))


def write_sidecar(path: Path, grouped: dict[int, list[int]], n_frames: int) -> None:
    local = {}
    for n_atoms, indices in grouped.items():
        for local_frame, global_index in enumerate(indices):
            local[global_index] = (n_atoms, local_frame)
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.writer(handle)
        writer.writerow(["global_index", "natoms", "has_forces", "xtc", "local_frame"])
        for frame_index in range(n_frames):
            n_atoms, has_forces = mixed_class(frame_index)
            _count, local_frame = local[frame_index]
            writer.writerow(
                [
                    frame_index,
                    n_atoms,
                    int(has_forces),
                    f"n{n_atoms}.xtc",
                    local_frame,
                ]
            )


def route_a(bundles: list[tuple[Path, Path, list[int]]]) -> tuple[list[int], float]:
    selected: list[int] = []
    acc = 0.0
    for gro, xtc, global_index in bundles:
        universe = mda.Universe(str(gro), str(xtc))
        for timestep in universe.trajectory:
            positions = timestep.positions
            acc += float(positions[0, 0])
            if is_cu107(universe):
                selected.append(global_index[int(timestep.frame)])
                acc += float(positions[-1, -1])
    return selected, acc


def route_b_forces(db: ConCorpus) -> tuple[list[int], float]:
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


def route_b_composition(db: ConCorpus) -> tuple[list[int], float]:
    keys = db.select(formula=FORMULA)
    selected: list[int] = []
    acc = 0.0
    for traj_id, frame_idx in keys:
        positions = db.get_positions(traj_id, frame_idx)
        selected.append(int(frame_idx))
        acc += float(positions[0][0]) + float(positions[-1][2])
    return selected, acc


def campaign(query, cache, rounds, keep_from, bundles, db, rows) -> None:
    route_b = route_b_forces if query == "Q1" else route_b_composition
    for round_index in range(rounds):
        if cache == "cold":
            drop_caches()
        wall_a, n_a, _acc_a = time_call(lambda: route_a(bundles))
        if cache == "cold":
            drop_caches()
        wall_b, n_b, _acc_b = time_call(lambda: route_b(db))
        print(
            f"{query} {cache} round {round_index} A {wall_a:.6f} n {n_a} "
            f"B {wall_b:.6f} n {n_b}",
            flush=True,
        )
        if round_index < keep_from:
            continue
        repeat = round_index - keep_from + 1
        rows.append(
            {
                "query": query,
                "route": "A",
                "cache": cache,
                "repeat": repeat,
                "wall_s": wall_a,
                "n_frames_selected": n_a,
            }
        )
        rows.append(
            {
                "query": query,
                "route": "B",
                "cache": cache,
                "repeat": repeat,
                "wall_s": wall_b,
                "n_frames_selected": n_b,
            }
        )


def write_csv(path: Path, rows: list[dict]) -> None:
    with path.open("w", encoding="utf-8", newline="") as handle:
        writer = csv.DictWriter(
            handle,
            fieldnames=["query", "route", "cache", "repeat", "wall_s", "n_frames_selected"],
        )
        writer.writeheader()
        for row in rows:
            writer.writerow(
                {
                    "query": row["query"],
                    "route": row["route"],
                    "cache": row["cache"],
                    "repeat": row["repeat"],
                    "wall_s": f"{row['wall_s']:.9f}",
                    "n_frames_selected": row["n_frames_selected"],
                }
            )


def check(db, seed, n_frames, bundles) -> dict:
    cu107 = [i for i in range(n_frames) if mixed_class(i)[0] == 107]
    with_forces = [i for i in cu107 if mixed_class(i)[1]]
    selected_a, _acc = route_a(bundles)
    selected_q1, _acc = route_b_forces(db)
    selected_q2, _acc = route_b_composition(db)
    max_abs = 0.0
    for frame_index in with_forces:
        _positions, forces = frame_geometry(seed, frame_index, 107)
        got = db.get_forces(1, frame_index)
        if got is None:
            raise RuntimeError(f"stored frame {frame_index} has no forces")
        for atom in range(107):
            for axis in range(3):
                diff = abs(float(got[atom][axis]) - forces[atom][axis])
                if diff > max_abs:
                    max_abs = diff
    no_force = next(i for i in cu107 if not mixed_class(i)[1])
    if db.get_forces(1, no_force) is not None:
        raise RuntimeError("a Cu107 frame without forces returned forces")
    return {
        "Q1": {
            "A_n": len(selected_a),
            "B_n": len(selected_q1),
            "A_equals_cu107_superset": sorted(selected_a) == cu107,
            "B_equals_with_forces": sorted(selected_q1) == with_forces,
            "forces_max_abs_diff": max_abs,
            "route_a_forces": "unanswerable_from_xtc",
        },
        "Q2": {
            "A_n": len(selected_a),
            "B_n": len(selected_q2),
            "sets_equal": sorted(selected_a) == sorted(selected_q2) == cu107,
        },
    }


def load_v1(path: Path, sha: str) -> dict:
    raw = json.loads(path.read_text(encoding="utf-8"))
    block = raw["v1_all_cu107"] if "v1_all_cu107" in raw else raw
    block["readcon_db"]["git_sha"] = sha
    return block


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--frames", type=int, default=N_FRAMES)
    parser.add_argument("--warmups", type=int, default=2)
    parser.add_argument("--repeats", type=int, default=30)
    parser.add_argument("--caches", default="warm,cold")
    parser.add_argument("--queries", default="Q1,Q2")
    parser.add_argument("--out-dir", type=Path, default=HERE)
    parser.add_argument("--work-dir", type=Path, default=HERE / "work-v2")
    args = parser.parse_args()
    if args.frames < 1:
        raise SystemExit("frames must be positive")
    caches = [item for item in args.caches.split(",") if item]
    queries = [item for item in args.queries.split(",") if item]
    work = args.work_dir
    if work.exists() and any(work.iterdir()):
        raise SystemExit(f"work directory is not empty: {work.name}")
    work.mkdir(parents=True, exist_ok=True)
    counts = mixed_counts(args.frames)
    if args.frames == N_FRAMES:
        expected = {
            (107, True): 1200,
            (107, False): 23760,
            (108, True): 23760,
            (108, False): 23760,
            (106, True): 23760,
            (106, False): 23760,
        }
        if counts != expected:
            raise SystemExit(f"class counts {counts}")
    print(f"counts {counts}", flush=True)

    seed = load_seed()
    con_path = work / "mixed.con"
    print(f"writing {args.frames} frames", flush=True)
    write_mixed_con(con_path, args.frames, seed)
    grouped = groups(args.frames)
    bundles = []
    files = {}
    for n_atoms in ATOM_COUNTS:
        xtc = work / f"n{n_atoms}.xtc"
        gro = work / f"n{n_atoms}.gro"
        write_xtc_gro(seed, grouped[n_atoms], n_atoms, xtc, gro)
        files[n_atoms] = (xtc, gro)
        bundles.append((gro, xtc, grouped[n_atoms]))
        probe = mda.Universe(str(gro), str(xtc))
        if int(probe.trajectory.n_frames) != len(grouped[n_atoms]):
            raise SystemExit(f"n{n_atoms} xtc frame count mismatch")
        del probe
    write_sidecar(work / "frames.csv", grouped, args.frames)

    store = work / "store"
    db = ConCorpus(str(store))
    stored = db.append_trajectory(1, str(con_path))
    if int(stored) != args.frames:
        raise SystemExit(f"ingested {stored} frames, expected {args.frames}")
    cooked = db.recook_all()
    print(f"ingested {stored} cooked {cooked}", flush=True)
    agreement = check(db, seed, args.frames, bundles)
    print(f"check {agreement}", flush=True)
    for query, row in agreement.items():
        if query == "Q1" and not (
            row["A_equals_cu107_superset"]
            and row["B_equals_with_forces"]
            and row["forces_max_abs_diff"] == 0.0
        ):
            raise SystemExit("Q1 sets disagree")
        if query == "Q2" and not row["sets_equal"]:
            raise SystemExit("Q2 sets disagree")

    rows: list[dict] = []
    for query in queries:
        for cache in caches:
            campaign(
                query,
                cache,
                args.warmups + args.repeats,
                args.warmups,
                bundles,
                db,
                rows,
            )

    sha = git_sha()
    out = args.out_dir
    out.mkdir(parents=True, exist_ok=True)
    write_csv(out / "results-v2.csv", rows)
    v1_path = out / "results.json"
    v1 = load_v1(v1_path, sha) if v1_path.is_file() else {}
    xtc_gro = 0
    per_file = {}
    for n_atoms, (xtc, gro) in files.items():
        xtc_b = disk_bytes(xtc)
        gro_b = disk_bytes(gro)
        per_file[str(n_atoms)] = {"xtc": xtc_b, "gro": gro_b}
        xtc_gro += xtc_b + gro_b
    summary = {
        "n_frames": args.frames,
        "counts": {
            "Cu107_forces": counts[(107, True)],
            "Cu107_no_forces": counts[(107, False)],
            "Cu108_forces": counts[(108, True)],
            "Cu108_no_forces": counts[(108, False)],
            "Cu106_forces": counts[(106, True)],
            "Cu106_no_forces": counts[(106, False)],
        },
        "pattern": (
            "index divisible by 100 is Cu107 with forces; other frames cycle "
            "Cu107 without forces, Cu108 with forces, Cu108 without forces, "
            "Cu106 with forces, Cu106 without forces"
        ),
        "warmups_discarded": args.warmups,
        "repeats": args.repeats,
        "order": "ABAB",
        "checks": agreement,
        "files_bytes_on_disk": {
            "xtc_gro_total": xtc_gro,
            "readcon_db_store": disk_bytes(store),
            "per_atom_count": per_file,
        },
        "cpu_model": cpu_model(),
        "core_count": core_count(),
        "ram_bytes": mem_total_bytes(),
        "os": os_name(),
        "python": __import__("sys").version.split()[0],
        "mdanalysis": mda.__version__,
        "numpy": np.__version__,
        "readcon_db": {
            "version": "0.2.4",
            "git_sha": sha,
            "import_matches_checkout_build": True,
        },
        "readcon_core": {
            "version": readcon_core_version(),
            "git_sha": "8c2d9984c40aedfb2e0084fedefaa02e930957f2",
        },
        "by_query": {},
    }
    for query in queries:
        summary["by_query"][query] = {}
        for cache in caches:
            samples_a = [
                row["wall_s"]
                for row in rows
                if row["query"] == query and row["cache"] == cache and row["route"] == "A"
            ]
            samples_b = [
                row["wall_s"]
                for row in rows
                if row["query"] == query and row["cache"] == cache and row["route"] == "B"
            ]
            summary["by_query"][query][cache] = {
                "A": summarize(samples_a),
                "B": summarize(samples_b),
                "ratio": ratio_ci(samples_a, samples_b),
            }
    document = {"v1_all_cu107": v1, "v2_mixed": summary, "repro": REPRO}
    (out / "results.json").write_text(json.dumps(document, indent=2) + "\n", encoding="utf-8")
    print("wrote results", flush=True)


if __name__ == "__main__":
    main()
