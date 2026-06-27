#!/usr/bin/env python3
"""ASE .db insert/extract/select timings — compare to readcon-db JSON."""
from __future__ import annotations
import json, sys, time
from pathlib import Path

try:
    from ase.db import connect
    from ase import Atoms
    import numpy as np
except ImportError:
    print("ASE not installed", file=sys.stderr)
    sys.exit(1)

# Minimal CON-like structures without readcon-core Python: build Atoms from known tiny_cuh2 geometry
# For fairness use same N frames as bench files — replicate one Atoms N times.

def make_atoms():
    # 2 Cu + H-like from tiny fixture pattern — use 2 Cu only for speed match order of magnitude
    a = Atoms("Cu2", positions=[[0.64, 0.90, 0.0], [3.20, 0.90, 0.0]], cell=[15.35, 21.70, 100.0], pbc=True)
    return a

def bench_one(n_frames: int, db_path: Path) -> dict:
    if db_path.exists():
        db_path.unlink()
    atoms0 = make_atoms()
    db = connect(str(db_path))

    t0 = time.perf_counter()
    with db:
        for i in range(n_frames):
            db.write(atoms0, traj_id=1, frame_idx=i)
    insert_s = time.perf_counter() - t0

    t1 = time.perf_counter()
    rounds = 20
    for _ in range(rounds):
        for row in db.select():
            _ = row.toatoms().positions.sum()
    extract_s = (time.perf_counter() - t1) / rounds

    t2 = time.perf_counter()
    sel_rounds = 50
    for _ in range(sel_rounds):
        # ASE db has limited query; scan all (worst case comparable to full select)
        list(db.select())
    select_s = (time.perf_counter() - t2) / sel_rounds

    # "Concurrent" readers — ASE sqlite often serializes; still measure threads
    import threading
    def reader():
        d = connect(str(db_path))
        for row in d.select():
            _ = row.toatoms().positions.sum()

    t3 = time.perf_counter()
    threads = [threading.Thread(target=reader) for _ in range(8)]
    for th in threads:
        th.start()
    for th in threads:
        th.join()
    concurrent_s = time.perf_counter() - t3

    return {
        "backend": "ase.db",
        "n_frames": n_frames,
        "insert_s": insert_s,
        "extract_all_mean_s": extract_s,
        "select_cu_mean_s": select_s,
        "concurrent_8readers_extract_s": concurrent_s,
        "insert_frames_per_s": n_frames / insert_s if insert_s else None,
        "extract_frames_per_s": n_frames / extract_s if extract_s else None,
        "note": "ASE Atoms Cu2 stand-in; full scan select (no symbol index)",
    }

def main():
    out = Path(sys.argv[1] if len(sys.argv) > 1 else "/tmp/ase_db_bench.json")
    work = Path("/tmp/ase_db_bench_work")
    work.mkdir(exist_ok=True)
    results = []
    for n in [10, 50, 100, 200, 500]:
        r = bench_one(n, work / f"n{n}.db")
        results.append(r)
        print(r)
    out.write_text(json.dumps(results, indent=2))
    print("wrote", out)

if __name__ == "__main__":
    main()
