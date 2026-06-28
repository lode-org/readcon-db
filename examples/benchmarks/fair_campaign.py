#!/usr/bin/env python3
"""
Fair ASE.db vs readcon-db campaign on the **same CON-derived structures**.

Prep: multi-frame CON ladder from a real fixture (default tiny_cuh2.con), not Cu2 stand-ins.
ASE rows are built from readcon frame geometry (symbols, positions, cell, masses) plus
optional energy/charge in ASE key_value_pairs / info so competitive selects work on both sides.

Usage:
  python fair_campaign.py [--fixture PATH] [--ladder 10,50,100,200,500] [--out DIR] [--run-id 1]

Emits JSON + select-parity block. Methodology is measurement-only; ASE is not the product store.
"""
from __future__ import annotations

import argparse
import json
import shutil
import tempfile
import threading
import time
from pathlib import Path

import numpy as np

import readcon
from ase import Atoms
from ase.db import connect
from readcon_db import ConCorpus

REPO_DB = Path(__file__).resolve().parents[2]
CORE_TEST = REPO_DB.parent / "readcon-core" / "resources" / "test"
DEFAULT_FIXTURE = CORE_TEST / "tiny_cuh2.con"
DEFAULT_LADDER = (10, 50, 100, 200, 500)


def frame_to_atoms(fr) -> Atoms:
    """CON-faithful Atoms without relying on to_ase() calculator path."""
    pos = np.asarray(fr.coords_array(), dtype=float)
    # symbols: list of str from atoms attribute (PyO3 list of dict-like or strings)
    atoms_attr = fr.atoms
    symbols = []
    for a in atoms_attr:
        if isinstance(a, str):
            symbols.append(a)
        elif isinstance(a, dict):
            symbols.append(a.get("symbol") or a.get("element") or "X")
        else:
            # PyO3 object with .symbol
            symbols.append(getattr(a, "symbol", str(a)))
    cell = list(fr.cell)
    atoms = Atoms(symbols=symbols, positions=pos, cell=cell, pbc=True)
    # masses from ASE element table already set by symbols
    if fr.energy is not None:
        atoms.info["energy"] = float(fr.energy)
    # optional ASE query keys
    atoms.info["mass"] = float(atoms.get_masses().sum())
    vol = float(abs(np.linalg.det(atoms.cell.array))) if atoms.cell.rank == 3 else 0.0
    atoms.info["volume"] = vol
    return atoms


def write_ladder_con(fixture: Path, n_frames: int, out_path: Path) -> None:
    text = fixture.read_text()
    # ensure multi-frame by concatenation (fixture is one frame)
    out_path.write_text(text * n_frames)


def prep_ladder(fixture: Path, ladder: list[int], work: Path) -> Path:
    frames_dir = work / "con_ladder"
    frames_dir.mkdir(parents=True, exist_ok=True)
    for n in ladder:
        write_ladder_con(fixture, n, frames_dir / f"n{n}.con")
    return frames_dir


def bench_readcon_db(con_path: Path, n_frames: int, corpus_dir: Path) -> dict:
    if corpus_dir.exists():
        shutil.rmtree(corpus_dir)
    db = ConCorpus(str(corpus_dir))
    t0 = time.perf_counter()
    nf = db.append_trajectory(1, str(con_path))
    insert_s = time.perf_counter() - t0
    assert int(nf) == n_frames, (nf, n_frames)

    # extract: materialize every CON blob (owned copy + byte fold) in one txn per pass
    rounds = 20
    t1 = time.perf_counter()
    last_ck = 0
    last_total = 0
    for _ in range(rounds):
        total, ck = db.touch_trajectory(1, n_frames)
        assert total > 0 and ck != 0  # payload fold; not length-only
        last_total, last_ck = total, ck
    extract_s = (time.perf_counter() - t1) / rounds
    assert last_total > 0 and last_ck != 0

    # competitive selects (mean over rounds)
    sel_rounds = 50
    t2 = time.perf_counter()
    for _ in range(sel_rounds):
        _ = db.select(symbol="Cu")
    select_cu_s = (time.perf_counter() - t2) / sel_rounds

    t2b = time.perf_counter()
    for _ in range(sel_rounds):
        _ = db.select(natoms_min=1, natoms_max=10_000)
    select_natoms_s = (time.perf_counter() - t2b) / sel_rounds

    # energy if any frames have it — may be 0 hits on tiny_cuh2 without energy
    # Mass / volume windows derived from first frame (all ladder frames identical).
    fr0 = readcon.read_con(str(con_path))[0]
    atoms0 = frame_to_atoms(fr0)
    mass0 = float(atoms0.info["mass"])
    vol0 = float(atoms0.info["volume"])
    mass_lo, mass_hi = mass0 * 0.99, mass0 * 1.01
    vol_lo, vol_hi = vol0 * 0.99, vol0 * 1.01

    t2m = time.perf_counter()
    for _ in range(sel_rounds):
        _ = db.select(mass_min=mass_lo, mass_max=mass_hi)
    select_mass_s = (time.perf_counter() - t2m) / sel_rounds

    t2v = time.perf_counter()
    for _ in range(sel_rounds):
        _ = db.select(volume_min=vol_lo, volume_max=vol_hi)
    select_volume_s = (time.perf_counter() - t2v) / sel_rounds

    # Energy timing only if corpus has energy metadata (tiny_cuh2 has none → omit, not full-scan).
    has_energy = any(fr.energy is not None for fr in readcon.read_con(str(con_path))[:1])
    select_energy_s = None
    if has_energy:
        t2c = time.perf_counter()
        for _ in range(sel_rounds):
            _ = db.select(energy_min=-1e9, energy_max=1e9, require_energy=True)
        select_energy_s = (time.perf_counter() - t2c) / sel_rounds

    # Multi-reader: share one ConCorpus; each thread fully materializes all blobs (one txn).
    def reader():
        total, ck = db.touch_trajectory(1, n_frames)
        assert total > 0 and ck != 0

    t3 = time.perf_counter()
    threads = [threading.Thread(target=reader) for _ in range(8)]
    for th in threads:
        th.start()
    for th in threads:
        th.join()
    concurrent_s = time.perf_counter() - t3

    hit_cu = len(db.select(symbol="Cu"))
    hit_natoms = len(db.select(natoms_min=1, natoms_max=10_000))
    hit_formula = len(db.select(formula="Cu:2|H:2"))
    hit_mass = len(db.select(mass_min=mass_lo, mass_max=mass_hi))
    hit_volume = len(db.select(volume_min=vol_lo, volume_max=vol_hi))

    return {
        "backend": "readcon-db",
        "n_frames": n_frames,
        "insert_s": insert_s,
        "extract_all_mean_s": extract_s,
        "select_cu_mean_s": select_cu_s,
        "select_natoms_mean_s": select_natoms_s,
        "select_mass_mean_s": select_mass_s,
        "select_volume_mean_s": select_volume_s,
        "select_energy_present_mean_s": select_energy_s,
        "energy_select_skipped": not has_energy,
        "mass_window": [mass_lo, mass_hi],
        "volume_window": [vol_lo, vol_hi],
        "concurrent_8readers_extract_s": concurrent_s,
        "insert_frames_per_s": n_frames / insert_s if insert_s else None,
        "extract_frames_per_s": n_frames / extract_s if extract_s else None,
        "hit_symbol_Cu": hit_cu,
        "hit_natoms_1_10000": hit_natoms,
        "hit_formula_Cu2H2": hit_formula,
        "hit_mass_window": hit_mass,
        "hit_volume_window": hit_volume,
        "methodology": "CON ladder append; Select via indexes; 8 threads share one ConCorpus (LMDB multi-reader)",
    }


def bench_ase_db(con_path: Path, n_frames: int, db_path: Path) -> dict:
    if db_path.exists():
        db_path.unlink()
    frames = readcon.read_con(str(con_path))
    assert len(frames) == n_frames
    atoms_list = [frame_to_atoms(fr) for fr in frames]

    db = connect(str(db_path))
    t0 = time.perf_counter()
    with db:
        for i, atoms in enumerate(atoms_list):
            # energy as kwarg for ASE select(energy=...) when present
            kw = {"traj_id": 1, "frame_idx": i, "mass": atoms.info["mass"], "volume": atoms.info["volume"]}
            if "energy" in atoms.info:
                kw["energy"] = atoms.info["energy"]
            db.write(atoms, **kw)
    insert_s = time.perf_counter() - t0

    rounds = 20
    t1 = time.perf_counter()
    for _ in range(rounds):
        for row in db.select():
            _ = row.toatoms().positions.sum()
    extract_s = (time.perf_counter() - t1) / rounds

    sel_rounds = 50
    # ASE formula / symbol: use formula string or natural_formula
    t2 = time.perf_counter()
    for _ in range(sel_rounds):
        list(db.select("Cu"))  # ASE: element presence query
    select_cu_s = (time.perf_counter() - t2) / sel_rounds

    t2b = time.perf_counter()
    for _ in range(sel_rounds):
        list(db.select("natoms>=1,natoms<=10000"))
    select_natoms_s = (time.perf_counter() - t2b) / sel_rounds

    mass0 = float(atoms_list[0].info["mass"])
    vol0 = float(atoms_list[0].info["volume"])
    mass_lo, mass_hi = mass0 * 0.99, mass0 * 1.01
    vol_lo, vol_hi = vol0 * 0.99, vol0 * 1.01

    t2m = time.perf_counter()
    for _ in range(sel_rounds):
        list(db.select(f"mass>={mass_lo},mass<={mass_hi}"))
    select_mass_s = (time.perf_counter() - t2m) / sel_rounds

    t2v = time.perf_counter()
    for _ in range(sel_rounds):
        list(db.select(f"volume>={vol_lo},volume<={vol_hi}"))
    select_volume_s = (time.perf_counter() - t2v) / sel_rounds

    # Energy: only time a fixed no-hit or real energy query — never fall back to full scan.
    has_energy = any("energy" in a.info for a in atoms_list[:1])
    select_energy_s = None
    if has_energy:
        t2c = time.perf_counter()
        for _ in range(sel_rounds):
            list(db.select("energy"))
        select_energy_s = (time.perf_counter() - t2c) / sel_rounds

    # Share one ASE connection for threaded reads (SQLite serializes; still CSE-comparable load).
    def reader():
        for row in db.select():
            _ = row.toatoms().positions.sum()

    t3 = time.perf_counter()
    threads = [threading.Thread(target=reader) for _ in range(8)]
    for th in threads:
        th.start()
    for th in threads:
        th.join()
    concurrent_s = time.perf_counter() - t3

    hit_cu = len(list(db.select("Cu")))
    hit_natoms = len(list(db.select("natoms>=1,natoms<=10000")))
    # formula: ASE uses reduced formula e.g. CuH
    hit_formula = len(list(db.select(formula="CuH")))
    hit_mass = len(list(db.select(f"mass>={mass_lo},mass<={mass_hi}")))
    hit_volume = len(list(db.select(f"volume>={vol_lo},volume<={vol_hi}")))

    return {
        "backend": "ase.db",
        "n_frames": n_frames,
        "insert_s": insert_s,
        "extract_all_mean_s": extract_s,
        "select_cu_mean_s": select_cu_s,
        "select_natoms_mean_s": select_natoms_s,
        "select_mass_mean_s": select_mass_s,
        "select_volume_mean_s": select_volume_s,
        "select_energy_present_mean_s": select_energy_s,
        "energy_select_skipped": not has_energy,
        "mass_window": [mass_lo, mass_hi],
        "volume_window": [vol_lo, vol_hi],
        "concurrent_8readers_extract_s": concurrent_s,
        "insert_frames_per_s": n_frames / insert_s if insert_s else None,
        "extract_frames_per_s": n_frames / extract_s if extract_s else None,
        "hit_symbol_Cu": hit_cu,
        "hit_natoms_1_10000": hit_natoms,
        "hit_formula_CuH_ase": hit_formula,
        "hit_mass_window": hit_mass,
        "hit_volume_window": hit_volume,
        "hit_formula_note": "ASE reduced formula CuH vs readcon multiset Cu:2|H:2 — count agreement uses symbol+natoms+mass+volume",
        "methodology": "Same CON frames via readcon→Atoms (no Cu2 stand-in); 8 threads share one ase.db connection",
    }


def interchange_parse(fixture: Path, n_frames: int = 100, repeats: int = 5) -> dict:
    """Parse throughput: readcon vs ASE on multi-frame CON text file."""
    with tempfile.TemporaryDirectory() as td:
        p = Path(td) / "traj.con"
        write_ladder_con(fixture, n_frames, p)
        # warm
        readcon.read_con(str(p))
        try:
            import ase.io

            ase.io.read(str(p), index=":")
        except Exception as e:
            return {"error": f"ase.io failed: {e}", "n_frames": n_frames}

        def time_rc():
            t0 = time.perf_counter()
            for _ in range(repeats):
                frs = readcon.read_con(str(p))
                assert len(frs) == n_frames
            return (time.perf_counter() - t0) / repeats

        def time_ase():
            t0 = time.perf_counter()
            for _ in range(repeats):
                frs = ase.io.read(str(p), index=":")
                if not isinstance(frs, list):
                    frs = [frs]
                assert len(frs) == n_frames
            return (time.perf_counter() - t0) / repeats

        rc_s = time_rc()
        ase_s = time_ase()
        return {
            "n_frames": n_frames,
            "repeats": repeats,
            "readcon_mean_s": rc_s,
            "ase_io_mean_s": ase_s,
            "readcon_frames_per_s": n_frames / rc_s if rc_s else None,
            "ase_io_frames_per_s": n_frames / ase_s if ase_s else None,
        }


def run_campaign(fixture: Path, ladder: list[int], out_dir: Path, run_id: int) -> dict:
    out_dir.mkdir(parents=True, exist_ok=True)
    work = out_dir / f"work_run{run_id}"
    if work.exists():
        shutil.rmtree(work)
    work.mkdir(parents=True)
    frames_dir = prep_ladder(fixture, ladder, work)

    rdb_results = []
    ase_results = []
    parity_rows = []

    for n in ladder:
        con_path = frames_dir / f"n{n}.con"
        rdb = bench_readcon_db(con_path, n, work / f"rdb_n{n}")
        ase = bench_ase_db(con_path, n, work / f"ase_n{n}.db")
        rdb_results.append(rdb)
        ase_results.append(ase)
        # Agreement: Cu, natoms, mass window, volume window (both sides carry scalars)
        parity_rows.append(
            {
                "n_frames": n,
                "hit_symbol_Cu_rdb": rdb["hit_symbol_Cu"],
                "hit_symbol_Cu_ase": ase["hit_symbol_Cu"],
                "symbol_Cu_agree": rdb["hit_symbol_Cu"] == ase["hit_symbol_Cu"],
                "hit_natoms_rdb": rdb["hit_natoms_1_10000"],
                "hit_natoms_ase": ase["hit_natoms_1_10000"],
                "natoms_agree": rdb["hit_natoms_1_10000"] == ase["hit_natoms_1_10000"],
                "hit_mass_rdb": rdb["hit_mass_window"],
                "hit_mass_ase": ase["hit_mass_window"],
                "mass_agree": rdb["hit_mass_window"] == ase["hit_mass_window"],
                "hit_volume_rdb": rdb["hit_volume_window"],
                "hit_volume_ase": ase["hit_volume_window"],
                "volume_agree": rdb["hit_volume_window"] == ase["hit_volume_window"],
            }
        )

    interchange = interchange_parse(fixture, n_frames=min(100, max(ladder)), repeats=5)

    all_agree = all(
        r["symbol_Cu_agree"]
        and r["natoms_agree"]
        and r["mass_agree"]
        and r["volume_agree"]
        for r in parity_rows
    )
    payload = {
        "run_id": run_id,
        "fixture": str(fixture),
        "ladder": ladder,
        "fair": True,
        "note": "Shared CON ladder; ASE Atoms from readcon geometry (not legacy Cu2 stand-in)",
        "readcon_db": rdb_results,
        "ase_db": ase_results,
        "select_parity": parity_rows,
        "interchange": interchange,
        "all_symbol_natoms_agree": all_agree,  # name kept; includes mass+volume
        "all_competitive_selects_agree": all_agree,
    }
    out_json = out_dir / f"ase_fair_campaign_{run_id}.json"
    out_json.write_text(json.dumps(payload, indent=2))
    return payload


def write_markdown_table(payload: dict, path: Path) -> None:
    lines = [
        "# Fair ASE.db vs readcon-db (same CON ladder)",
        "",
        f"Fixture: `{payload['fixture']}`  Ladder: {payload['ladder']}",
        "",
        "| N | rdb ins/s | ase ins/s | rdb ext/s | ase ext/s | rdb sel Cu (s) | ase sel Cu (s) | rdb8 (s) | ase8 (s) | Cu hits agree |",
        "|---|-----------|-----------|-----------|-----------|----------------|----------------|----------|----------|---------------|",
    ]
    by_n_rdb = {r["n_frames"]: r for r in payload["readcon_db"]}
    by_n_ase = {r["n_frames"]: r for r in payload["ase_db"]}
    par = {r["n_frames"]: r for r in payload["select_parity"]}
    for n in payload["ladder"]:
        r, a, p = by_n_rdb[n], by_n_ase[n], par[n]
        lines.append(
            "| {n} | {ri:.2e} | {ai:.2e} | {re:.2e} | {ae:.2e} | {rsc:.2e} | {asc:.2e} | {r8:.3f} | {a8:.3f} | {ag} |".format(
                n=n,
                ri=r["insert_frames_per_s"] or 0,
                ai=a["insert_frames_per_s"] or 0,
                re=r["extract_frames_per_s"] or 0,
                ae=a["extract_frames_per_s"] or 0,
                rsc=r["select_cu_mean_s"],
                asc=a["select_cu_mean_s"],
                r8=r["concurrent_8readers_extract_s"],
                a8=a["concurrent_8readers_extract_s"],
                ag="yes"
                if p["symbol_Cu_agree"]
                and p["natoms_agree"]
                and p.get("mass_agree", True)
                and p.get("volume_agree", True)
                else "NO",
            )
        )
    lines.append("")
    lines.append("Interchange (parse multi-frame CON):")
    lines.append("```json")
    lines.append(json.dumps(payload.get("interchange"), indent=2))
    lines.append("```")
    lines.append("")
    lines.append(
        "Legacy `bench_ase_db.py` Cu2 stand-in timings remain **unequal-workload** artifacts; this file is the fair campaign."
    )
    path.write_text("\n".join(lines) + "\n")


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--fixture", type=Path, default=DEFAULT_FIXTURE)
    ap.add_argument("--ladder", type=str, default="10,50,100,200,500")
    ap.add_argument("--out", type=Path, default=Path(__file__).resolve().parent / "fair_out")
    ap.add_argument("--run-id", type=int, default=1)
    args = ap.parse_args()
    ladder = [int(x) for x in args.ladder.split(",") if x.strip()]
    payload = run_campaign(args.fixture, ladder, args.out, args.run_id)
    write_markdown_table(payload, args.out / f"fair_db_vs_ase_table_run{args.run_id}.md")
    print(
        json.dumps(
            {
                "run_id": args.run_id,
                "all_agree": payload["all_competitive_selects_agree"],
                "out": str(args.out),
            },
            indent=2,
        )
    )
    if not payload["all_competitive_selects_agree"]:
        raise SystemExit("select hit counts disagree — see JSON select_parity")


if __name__ == "__main__":
    main()
