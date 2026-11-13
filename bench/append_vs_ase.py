#!/usr/bin/env python3
"""Append one CON file into readcon-db and into ASE's database.

One warmup, then six serial samples. The median and the interquartile
range use the type-7 percentile (NumPy ``linear``, Hyndman and Fan).
Parsing the file and building the ASE rows happens before the clock.
The clock is the append itself. Each sample uses a fresh store.

The ladder is 100 copies of ``resources/test/tiny_cuh2.con``, the same
file the fair campaign times at N=100.
"""

from __future__ import annotations

import json
import os
import shutil
import tempfile
import time
from pathlib import Path


def _threads() -> None:
    for key in (
        "OMP_NUM_THREADS",
        "OPENBLAS_NUM_THREADS",
        "MKL_NUM_THREADS",
        "NUMEXPR_NUM_THREADS",
        "RAYON_NUM_THREADS",
    ):
        os.environ[key] = "1"


_threads()

import numpy as np  # noqa: E402
from ase import Atoms  # noqa: E402
from ase.db import connect  # noqa: E402

import readcon  # noqa: E402
from readcon_db import ConCorpus  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "resources" / "test" / "tiny_cuh2.con"
RESULTS = ROOT / "bench" / "results"
N_FRAMES = 100
WARMUP = 1
SAMPLES = 6


def type7(values: list[float], probability: float) -> float:
    ordered = np.sort(np.asarray(values, dtype=np.float64))
    return float(np.quantile(ordered, probability, method="linear"))


def cpu_model() -> str:
    for line in Path("/proc/cpuinfo").read_text(encoding="utf-8").splitlines():
        if line.lower().startswith("model name"):
            return line.split(":", 1)[1].strip()
    return "unrecorded"


def summarize(samples: list[float]) -> dict:
    q25 = type7(samples, 0.25)
    med = type7(samples, 0.5)
    q75 = type7(samples, 0.75)
    return {
        "median_s": med,
        "iqr_s": q75 - q25,
        "q25_s": q25,
        "q75_s": q75,
        "samples_s": samples,
    }


def frame_to_atoms(frame) -> Atoms:
    """Same row the fair campaign writes into ASE's database."""
    pos = np.asarray(frame.coords_array(), dtype=float)
    symbols = []
    for atom in frame.atoms:
        if isinstance(atom, str):
            symbols.append(atom)
        elif isinstance(atom, dict):
            symbols.append(atom.get("symbol") or atom.get("element") or "X")
        else:
            symbols.append(getattr(atom, "symbol", str(atom)))
    atoms = Atoms(symbols=symbols, positions=pos, cell=list(frame.cell), pbc=True)
    if frame.energy is not None:
        atoms.info["energy"] = float(frame.energy)
    atoms.info["mass"] = float(atoms.get_masses().sum())
    volume = (
        float(abs(np.linalg.det(atoms.cell.array))) if atoms.cell.rank == 3 else 0.0
    )
    atoms.info["volume"] = volume
    return atoms


def ladder(work: Path) -> tuple[Path, list]:
    text = FIXTURE.read_text(encoding="utf-8")
    path = work / "n100.con"
    path.write_text(text * N_FRAMES, encoding="utf-8")
    frames = readcon.read_con(str(path))
    if len(frames) != N_FRAMES:
        raise SystemExit(f"expected {N_FRAMES} frames, read {len(frames)}")
    atoms = [frame_to_atoms(frame) for frame in frames]
    return path, atoms


def time_readcon(path: Path, work: Path) -> tuple[float, list[float]]:
    def once(tag: str) -> float:
        corpus = work / f"rdb-{tag}"
        if corpus.exists():
            shutil.rmtree(corpus)
        db = ConCorpus(str(corpus))
        started = time.perf_counter()
        n = db.append_trajectory(1, str(path))
        elapsed = time.perf_counter() - started
        if int(n) != N_FRAMES:
            raise SystemExit(f"readcon-db stored {n} frames")
        return elapsed

    warmup = 0.0
    for _ in range(WARMUP):
        warmup = once("warmup")
    samples = [once(f"s{i}") for i in range(SAMPLES)]
    return warmup, samples


def time_ase(atoms: list, work: Path) -> tuple[float, list[float]]:
    def once(tag: str) -> float:
        db_path = work / f"ase-{tag}.db"
        if db_path.exists():
            db_path.unlink()
        db = connect(str(db_path))
        started = time.perf_counter()
        with db:
            for index, row in enumerate(atoms):
                kw = {
                    "traj_id": 1,
                    "frame_idx": index,
                    "mass": row.info["mass"],
                    "volume": row.info["volume"],
                }
                if "energy" in row.info:
                    kw["energy"] = row.info["energy"]
                db.write(row, **kw)
        elapsed = time.perf_counter() - started
        if db.count() != N_FRAMES:
            raise SystemExit(f"ASE stored {db.count()} rows")
        return elapsed

    warmup = 0.0
    for _ in range(WARMUP):
        warmup = once("warmup")
    samples = [once(f"s{i}") for i in range(SAMPLES)]
    return warmup, samples


def main() -> None:
    RESULTS.mkdir(parents=True, exist_ok=True)
    with tempfile.TemporaryDirectory(prefix="readcon-db-append-") as tmp:
        work = Path(tmp)
        path, atoms = ladder(work)
        rdb_warm, rdb_samples = time_readcon(path, work)
        ase_warm, ase_samples = time_ase(atoms, work)
    payload = {
        "protocol": "serial, one warmup, n=6, type-7 median and IQR",
        "fixture": "resources/test/tiny_cuh2.con",
        "n_frames": N_FRAMES,
        "cpu": cpu_model(),
        "readcon": readcon.__version__,
        "readcon_db": {
            "warmup_s": rdb_warm,
            **summarize(rdb_samples),
        },
        "ase_db": {
            "warmup_s": ase_warm,
            **summarize(ase_samples),
        },
    }
    out = RESULTS / "append_vs_ase.json"
    log = RESULTS / "append_vs_ase.log"
    out.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
    lines = [
        f"protocol={payload['protocol']}",
        f"fixture={payload['fixture']} n_frames={N_FRAMES} cpu={payload['cpu']}",
        f"readcon={payload['readcon']}",
        f"readcon-db warmup_s={rdb_warm:.9e}",
    ]
    lines.extend(
        f"readcon-db sample={i} seconds={sample:.9e}"
        for i, sample in enumerate(rdb_samples)
    )
    lines.append(
        "readcon-db "
        f"median_s={payload['readcon_db']['median_s']:.9e} "
        f"iqr_s={payload['readcon_db']['iqr_s']:.9e}"
    )
    lines.append(f"ase.db warmup_s={ase_warm:.9e}")
    lines.extend(
        f"ase.db sample={i} seconds={sample:.9e}"
        for i, sample in enumerate(ase_samples)
    )
    lines.append(
        "ase.db "
        f"median_s={payload['ase_db']['median_s']:.9e} "
        f"iqr_s={payload['ase_db']['iqr_s']:.9e}"
    )
    log.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(log.read_text(), end="")
    print(f"wrote {out}")
    print(f"wrote {log}")


if __name__ == "__main__":
    main()
