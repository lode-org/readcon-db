#!/usr/bin/env python3
"""One ase.db (SQLite) ingest or a cold-then-warm query.

The timed region includes the hit-count and frame-index-sum check.
"""

from __future__ import annotations

import argparse
import json
import os
import struct
import time
import warnings
from pathlib import Path

import numpy as np
from ase import Atoms
from ase.calculators.singlepoint import SinglePointCalculator
from ase.db import connect
from ase.geometry import cellpar_to_cell

import readcon

warnings.filterwarnings("ignore")


def bits_to_float(text: str) -> float:
    return struct.unpack(">d", int(text, 16).to_bytes(8, "big"))[0]


def fmt_bound(value: float) -> str:
    text = repr(float(value))
    if float(text) != float(value):
        raise SystemExit("query bound does not round-trip through decimal text")
    return text


def formula_query(formula: str) -> str:
    parts = []
    for token in formula.split("|"):
        symbol, count = token.split(":")
        parts.append(f"{symbol}={count}")
    return ",".join(parts)


def load_oracle(path: Path) -> dict:
    return json.loads(path.read_text())


def symbols_of(frame) -> list[str]:
    return [atom.symbol for atom in frame.atoms]


def frame_to_atoms(frame) -> Atoms:
    positions = np.array(frame.coords_array(), dtype=float)
    # CON stores lengths and angles. ASE's cell is the 3x3 matrix of those.
    cellpar = [float(v) for v in list(frame.cell) + list(frame.angles)]
    meta = dict(frame.metadata)
    pbc = meta.get("pbc", [False, False, False])
    atoms = Atoms(
        symbols=symbols_of(frame),
        positions=positions,
        cell=cellpar_to_cell(cellpar),
        pbc=pbc,
    )
    forces = np.array(frame.forces_array(), dtype=float)
    energy = float(frame.energy)
    atoms.calc = SinglePointCalculator(atoms, energy=energy, forces=forces)
    return atoms


def file_sizes(path: Path) -> tuple[int, int]:
    st = path.stat()
    return st.st_size, st.st_blocks * 512


def select_checked(db, query: str, expect_n: int, expect_sum: int) -> float:
    t0 = time.perf_counter()
    hits = 0
    total = 0
    for row in db.select(query):
        hits += 1
        total += int(row.frame_index)
    if hits != expect_n or total != expect_sum:
        raise SystemExit(
            f"ase.db check failed hits {hits} expected {expect_n} sum {total} expected {expect_sum}"
        )
    return time.perf_counter() - t0


def do_ingest(con: Path, db_path: Path, oracle: dict) -> dict:
    if db_path.exists():
        db_path.unlink()
    # ASE's Lock waits forever, doubling the sleep, if a previous process
    # died and left database.lock behind.
    lock = db_path.with_name(db_path.name + ".lock")
    if lock.exists():
        lock.unlink()
    raw_bytes = con.stat().st_size
    comp = formula_query(oracle["composition"]["formula"])
    expect_n = int(oracle["composition"]["n"])
    expect_sum = int(oracle["composition"]["index_sum"])
    t0 = time.perf_counter()
    db = connect(str(db_path))
    n = 0
    for frame in readcon.iter_con(str(con)):
        db.write(frame_to_atoms(frame), frame_index=n)
        n += 1
    seconds_check = select_checked(db, comp, expect_n, expect_sum)
    # select_checked has its own timer; the ingest timer is still running.
    del seconds_check
    if n != int(oracle["n_frames"]):
        raise SystemExit(f"ase.db wrote {n} frames, oracle says {oracle['n_frames']}")
    seconds = time.perf_counter() - t0
    apparent, allocated = file_sizes(db_path)
    return {
        "ok": True,
        "seconds": seconds,
        "frames": n,
        "raw_bytes": raw_bytes,
        "frames_per_s": n / seconds,
        "mb_per_s": (raw_bytes / 1.0e6) / seconds,
        "hits": expect_n,
        "index_sum": expect_sum,
        "apparent_bytes": apparent,
        "allocated_bytes": allocated,
    }


def query_for(oracle: dict, name: str) -> tuple[str, int, int]:
    if name == "composition":
        return (
            formula_query(oracle["composition"]["formula"]),
            int(oracle["composition"]["n"]),
            int(oracle["composition"]["index_sum"]),
        )
    if name == "energy":
        lo = bits_to_float(oracle["energy"]["min_bits"])
        hi = bits_to_float(oracle["energy"]["max_bits"])
        return (
            f"energy>={fmt_bound(lo)},energy<={fmt_bound(hi)}",
            int(oracle["energy"]["n"]),
            int(oracle["energy"]["index_sum"]),
        )
    if name == "fmax":
        lo = bits_to_float(oracle["fmax"]["min_bits"])
        hi = bits_to_float(oracle["fmax"]["max_bits"])
        return (
            f"fmax>={fmt_bound(lo)},fmax<={fmt_bound(hi)}",
            int(oracle["fmax"]["n"]),
            int(oracle["fmax"]["index_sum"]),
        )
    raise SystemExit(f"unknown query {name}")


def do_query(db_path: Path, oracle: dict, only: str) -> dict:
    db = connect(str(db_path))
    query, expect_n, expect_sum = query_for(oracle, only)
    cold = select_checked(db, query, expect_n, expect_sum)
    warm = select_checked(db, query, expect_n, expect_sum)
    payload = {"seconds": None, "hits": expect_n, "index_sum": expect_sum}
    return {
        "ok": True,
        "cold": {only: {**payload, "seconds": cold}},
        "warm": {only: {**payload, "seconds": warm}},
    }


def main():
    parser = argparse.ArgumentParser()
    sub = parser.add_subparsers(dest="cmd", required=True)
    ing = sub.add_parser("ingest")
    ing.add_argument("--con", type=Path, required=True)
    ing.add_argument("--db", type=Path, required=True)
    ing.add_argument("--oracle", type=Path, required=True)
    qry = sub.add_parser("query")
    qry.add_argument("--db", type=Path, required=True)
    qry.add_argument("--oracle", type=Path, required=True)
    qry.add_argument("--only", required=True, choices=("composition", "energy", "fmax"))
    args = parser.parse_args()
    oracle = load_oracle(args.oracle)
    if args.cmd == "ingest":
        args.db.parent.mkdir(parents=True, exist_ok=True)
        result = do_ingest(args.con, args.db, oracle)
    else:
        result = do_query(args.db, oracle, args.only)
    print(json.dumps(result))


if __name__ == "__main__":
    main()
