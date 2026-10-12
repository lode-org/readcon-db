#!/usr/bin/env python3
"""Saddle-search CON corpora for the readcon-db evaluation.

eOn is not used: it is not on PATH in this environment. The searches are
ASE EMT calculations: an HCN/HNC NEB, a Cu adatom NEB, a Pt-H NEB, and a
Pt adatom dimer (min-mode) search. An aKMC trace then repeats the relaxed
states and the saddles. Scale directories extend that ensemble with further
EMT single points once the search tape runs out.

ASE's own note applies: H, C, and N EMT parameters are not for production
barriers. Pt and Cu use the standard EMT metal parameters. The store sees
the frames either way.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
from dataclasses import dataclass
from pathlib import Path

import numpy as np
from ase import Atoms
from ase.build import add_adsorbate, fcc100
from ase.calculators.emt import EMT
from ase.calculators.singlepoint import SinglePointCalculator
from ase.mep import DimerControl, MinModeAtoms, MinModeTranslate, NEB
from ase.optimize import BFGS, FIRE

import readcon

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "eval" / "data"
KT = 8.617333262145e-5 * 600.0


@dataclass
class Record:
    symbols: list
    positions: np.ndarray
    cell: np.ndarray
    energy: float
    forces: np.ndarray
    kind: str
    system: str


class Tape(EMT):
    def __init__(self, records: list, kind: str, system: str):
        super().__init__()
        self.records = records
        self.kind = kind
        self.system = system

    def calculate(self, atoms, properties, system_changes):
        super().calculate(atoms, properties, system_changes)
        if "energy" not in self.results or "forces" not in self.results:
            return
        self.records.append(
            Record(
                symbols=list(atoms.get_chemical_symbols()),
                positions=np.array(atoms.positions, dtype=float, copy=True),
                cell=np.array(atoms.cell.array, dtype=float, copy=True),
                energy=float(self.results["energy"]),
                forces=np.array(self.results["forces"], dtype=float, copy=True),
                kind=self.kind,
                system=self.system,
            )
        )


def plain(value):
    if isinstance(value, dict):
        return {str(k): plain(v) for k, v in value.items()}
    if isinstance(value, (list, tuple)):
        return [plain(v) for v in value]
    if isinstance(value, (np.bool_, bool)):
        return bool(value)
    if isinstance(value, (np.integer, int)) and not isinstance(value, bool):
        return int(value)
    if isinstance(value, (np.floating, float)):
        return float(value)
    return value


def stamp(frame, energy, frame_index, kind, neb_bead=None, neb_band=None):
    frame.set_energy(float(energy))
    frame.set_frame_index(int(frame_index))
    frame.set_string_metadata("kind", kind)
    if neb_bead is not None:
        frame.set_neb_bead(int(neb_bead))
    if neb_band is not None:
        frame.set_neb_band(int(neb_band))
    meta = plain(dict(frame.metadata))
    meta["pbc"] = [True, True, True]
    # Workaround, not a fix. from_ase leaves the user line empty, which is
    # legal CON. readcon.iter_con drops that leading newline
    # (readcon-core PyConFrameIterator::__next__). See
    # eval/bugs/empty-user-line.md. A non-empty user line keeps the bytes
    # on the path that the streaming reader accepts.
    return readcon.ConFrame(
        list(frame.cell),
        list(frame.angles),
        list(frame.atoms),
        prebox_header=["cpc-eval", ""],
        postbox_header=["0 0", "0 0 0"],
        metadata=meta,
    )


def record_to_frame(rec: Record, frame_index: int, neb_bead=None, neb_band=None):
    atoms = Atoms(symbols=rec.symbols, positions=rec.positions, cell=rec.cell, pbc=True)
    atoms.calc = SinglePointCalculator(atoms, energy=rec.energy, forces=rec.forces)
    frame = readcon.ConFrame.from_ase(atoms)
    return stamp(frame, rec.energy, frame_index, rec.kind, neb_bead, neb_band)


def freeze_atoms(atoms, kind: str) -> dict:
    atoms = atoms.copy()
    atoms.calc = EMT()
    energy = float(atoms.get_potential_energy())
    forces = np.array(atoms.get_forces(), dtype=float, copy=True)
    rec = Record(
        symbols=list(atoms.get_chemical_symbols()),
        positions=np.array(atoms.positions, dtype=float, copy=True),
        cell=np.array(atoms.cell.array, dtype=float, copy=True),
        energy=energy,
        forces=forces,
        kind=kind,
        system=kind,
    )
    frame = record_to_frame(rec, 0)
    text = readcon.write_con_string([frame], precision=12, canonical=True)
    parsed = readcon.read_con_string(text)[0]
    return {"kind": kind, "energy": float(parsed.energy), "text": text, "natoms": len(atoms)}


def relax(atoms, records, system, steps, fmax):
    atoms = atoms.copy()
    atoms.calc = Tape(records, "relax", system)
    BFGS(atoms, logfile=None).run(fmax=fmax, steps=steps)
    return atoms


def run_neb(start, end, records, system, n_images, steps, fmax):
    images = [start.copy()]
    for _ in range(n_images):
        images.append(start.copy())
    images.append(end.copy())
    for image in images:
        image.calc = Tape(records, "neb", system)
    neb = NEB(images, climb=True, method="improvedtangent")
    neb.interpolate()
    for image in images:
        # One evaluation of the interpolated band, then the optimizer steps.
        image.get_potential_energy()
        image.get_forces()
    FIRE(neb, logfile=None).run(fmax=fmax, steps=steps)
    return images


def run_dimer(atoms, records, steps, fmax):
    atoms = atoms.copy()
    atoms.calc = Tape(records, "dimer", "pt-adatom")
    mask = [False] * (len(atoms) - 1) + [True]
    with DimerControl(
        initial_eigenmode_method="displacement",
        displacement_method="vector",
        logfile=None,
        mask=[int(v) for v in mask],
    ) as control:
        dimer = MinModeAtoms(atoms, control)
        displacement = [[0.0, 0.0, 0.0] for _ in range(len(atoms))]
        displacement[-1][1] = -0.1
        dimer.displace(displacement_vector=displacement)
        with MinModeTranslate(dimer, logfile=None) as opt:
            opt.run(fmax=fmax, steps=steps)
    return atoms


def molecule(order: str) -> Atoms:
    # NEB interpolates by atom index, so both ends use the symbol order H, C, N.
    # HNC places H at the nitrogen end of the same three atoms.
    if order == "HCN":
        positions = [[0.0, 0.0, 0.0], [1.06, 0.0, 0.0], [2.20, 0.0, 0.0]]
    else:
        positions = [[2.20, 0.0, 0.0], [0.0, 0.0, 0.0], [1.15, 0.0, 0.0]]
    return Atoms("HCN", positions=positions, cell=[14.0, 14.0, 14.0], pbc=True)


def cu_endpoints():
    start = fcc100("Cu", size=(2, 2, 2), vacuum=6.0)
    add_adsorbate(start, "Cu", 1.7, "hollow")
    end = start.copy()
    end.positions[-1, 0] += float(start.cell[0, 0]) / 2.0
    return start, end


def pt_h_endpoints():
    start = fcc100("Pt", size=(2, 2, 2), vacuum=6.0)
    add_adsorbate(start, "H", 1.1, "hollow")
    end = start.copy()
    end.positions[-1, 0] += 1.3
    return start, end


def pt_adatom():
    atoms = fcc100("Pt", size=(2, 2, 1), vacuum=8.0)
    add_adsorbate(atoms, "Pt", 1.6, "hollow")
    return atoms


def write_records(path: Path, records: list[Record]) -> int:
    frames = [record_to_frame(rec, i) for i, rec in enumerate(records)]
    if not frames:
        path.write_text("")
        return 0
    path.write_text(readcon.write_con_string(frames, precision=12, canonical=True))
    return len(frames)


def kmc_trace(state_a, state_b, saddle, steps: int, rng: np.random.Generator) -> tuple[list[str], int]:
    """One state and one saddle per step.

    The hop probability is exp(-barrier/kT) / (exp(-barrier/kT) + 1). The
    stay rate is 1 in the same arbitrary prefactor as the hop. A large EMT
    barrier therefore keeps the walker in one basin, and the stored text of
    that basin is repeated.
    """
    states = [state_a, state_b]
    current = 0
    hops = 0
    out = []
    for _ in range(steps):
        out.append(states[current]["text"])
        out.append(saddle["text"])
        barrier = max(saddle["energy"] - states[current]["energy"], 0.0)
        # exp underflows to 0 well before a 50 eV barrier at this temperature.
        rate = math.exp(-barrier / KT) if barrier / KT < 700.0 else 0.0
        if rate > 0.0 and rng.random() < rate / (rate + 1.0):
            current = 1 - current
            hops += 1
    return out, hops


def highest_record(records, system):
    chosen = [rec for rec in records if rec.system == system and rec.kind == "neb"]
    if not chosen:
        return None
    return max(chosen, key=lambda rec: rec.energy)


class ScaleWriter:
    def __init__(self, directory: Path, chunk: int):
        self.directory = directory
        self.chunks = directory / "files"
        self.chunks.mkdir(parents=True, exist_ok=True)
        self.all_path = directory / "all.con"
        self.all_fh = self.all_path.open("w")
        self.buf = []
        self.chunk = chunk
        self.file_i = 0
        self.n = 0

    def add_frame(self, frame):
        self.buf.append(frame)
        self.n += 1
        if len(self.buf) >= self.chunk:
            self.flush()

    def add_record(self, rec: Record):
        self.add_frame(record_to_frame(rec, self.n))

    def flush(self):
        if not self.buf:
            return
        text = readcon.write_con_string(self.buf, precision=12, canonical=True)
        name = f"f{self.file_i:06d}.con"
        (self.chunks / name).write_text(text)
        self.all_fh.write(text)
        self.file_i += 1
        self.buf.clear()

    def close(self) -> dict:
        self.flush()
        self.all_fh.close()
        digest = hashlib.sha256()
        with self.all_path.open("rb") as handle:
            for block in iter(lambda: handle.read(1 << 20), b""):
                digest.update(block)
        return {
            "n_frames": self.n,
            "n_files": self.file_i,
            "raw_bytes": self.all_path.stat().st_size,
            "sha256": digest.hexdigest(),
        }


def displace(atoms: Atoms, rng: np.random.Generator, sigma: float) -> Record:
    trial = atoms.copy()
    trial.positions = trial.positions + rng.normal(0.0, sigma, size=trial.positions.shape)
    trial.calc = EMT()
    energy = float(trial.get_potential_energy())
    forces = np.array(trial.get_forces(), dtype=float, copy=True)
    return Record(
        symbols=list(trial.get_chemical_symbols()),
        positions=np.array(trial.positions, dtype=float, copy=True),
        cell=np.array(trial.cell.array, dtype=float, copy=True),
        energy=energy,
        forces=forces,
        kind="displace",
        system="displace",
    )


def fill_scale(directory: Path, pool: list[Record], bases: list[Atoms], n: int, chunk: int, rng):
    writer = ScaleWriter(directory, chunk)
    if pool:
        order = rng.permutation(len(pool))
        take = min(n, len(pool))
        for k in range(take):
            writer.add_record(pool[int(order[k])])
    sigmas = (0.02, 0.05, 0.10)
    base_i = 0
    while writer.n < n:
        base = bases[base_i % len(bases)]
        sigma = sigmas[(writer.n // len(bases)) % len(sigmas)]
        writer.add_record(displace(base, rng, sigma))
        base_i += 1
        if writer.n % 20000 == 0:
            print(f"  scale {n}: {writer.n} frames", flush=True)
    info = writer.close()
    if info["n_frames"] != n:
        raise SystemExit(f"scale {directory.name} wrote {info['n_frames']} frames, wanted {n}")
    # Spot-check the periodicity key and that forces survived the writer.
    first = next(readcon.iter_con(str(directory / "files" / "f000000.con")))
    meta = dict(first.metadata)
    if list(meta.get("pbc", [])) != [True, True, True]:
        raise SystemExit(f"scale frame lost explicit pbc: {meta.get('pbc')}")
    if first.energy is None or not first.has_forces:
        raise SystemExit("scale frame missing energy or forces")
    return info


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for block in iter(lambda: handle.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def safe_call(label, fn):
    try:
        return fn()
    except Exception as exc:
        print(f"search failed ({label}): {type(exc).__name__}: {exc}", flush=True)
        return None


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--sizes", default="1000,10000,100000,1000000")
    parser.add_argument("--kmc-steps", type=int, default=4000)
    parser.add_argument("--search-steps", type=int, default=20)
    parser.add_argument("--neb-images", type=int, default=7)
    parser.add_argument("--chunk", type=int, default=64)
    parser.add_argument("--seed", type=int, default=20261011)
    parser.add_argument("--skip-scale", action="store_true")
    args = parser.parse_args()
    sizes = [int(s) for s in args.sizes.split(",") if s]
    rng = np.random.default_rng(args.seed)
    campaign = DATA / "campaign"
    campaign.mkdir(parents=True, exist_ok=True)

    records: list[Record] = []
    print("HCN/HNC NEB", flush=True)
    hcn = safe_call("relax HCN", lambda: relax(molecule("HCN"), records, "hcn-hnc", args.search_steps, 0.05))
    hnc = safe_call("relax HNC", lambda: relax(molecule("HNC"), records, "hcn-hnc", args.search_steps, 0.05))
    if hcn is not None and hnc is not None:
        safe_call(
            "neb HCN",
            lambda: run_neb(hcn, hnc, records, "hcn-hnc", args.neb_images, args.search_steps, 0.1),
        )
    print("Cu adatom NEB", flush=True)
    cu_s, cu_e = cu_endpoints()
    cu_s = safe_call("relax Cu A", lambda: relax(cu_s, records, "cu-adatom", args.search_steps, 0.05))
    cu_e = safe_call("relax Cu B", lambda: relax(cu_e, records, "cu-adatom", args.search_steps, 0.05))
    if cu_s is not None and cu_e is not None:
        safe_call(
            "neb Cu",
            lambda: run_neb(cu_s, cu_e, records, "cu-adatom", args.neb_images, args.search_steps, 0.1),
        )
    print("Pt-H NEB", flush=True)
    pt_s, pt_e = pt_h_endpoints()
    pt_s = safe_call("relax PtH A", lambda: relax(pt_s, records, "pt-h", args.search_steps, 0.05))
    pt_e = safe_call("relax PtH B", lambda: relax(pt_e, records, "pt-h", args.search_steps, 0.05))
    if pt_s is not None and pt_e is not None:
        safe_call(
            "neb PtH",
            lambda: run_neb(pt_s, pt_e, records, "pt-h", args.neb_images, args.search_steps, 0.1),
        )
    print("Pt adatom dimer", flush=True)
    safe_call("dimer Pt", lambda: run_dimer(pt_adatom(), records, args.search_steps, 0.05))
    print(f"search tape: {len(records)} evaluations", flush=True)

    groups = {
        "hcn_hnc_neb": [rec for rec in records if rec.system == "hcn-hnc"],
        "cu_adatom_neb": [rec for rec in records if rec.system == "cu-adatom"],
        "pt_h_neb": [rec for rec in records if rec.system == "pt-h"],
        "pt_adatom_dimer": [rec for rec in records if rec.system == "pt-adatom"],
    }
    catalog = []
    for name, group in groups.items():
        path = campaign / f"{name}.con"
        n = write_records(path, group)
        catalog.append({"file": f"{name}.con", "n_frames": n, "sha256": sha256_file(path) if n else None})
        print(f"  {name}: {n} frames", flush=True)

    # Frozen states for the aKMC trace. Fall back to a single-point geometry
    # when a relax did not return.
    bases = []
    if hcn is not None:
        bases.append(hcn)
    if cu_s is not None:
        bases.append(cu_s)
    if pt_s is not None:
        bases.append(pt_s)
    bases.append(pt_adatom())
    if not bases:
        raise SystemExit("no geometries available to extend the corpora")

    trace_parts = []
    state_notes = []
    pairs = [
        ("hcn-hnc", hcn, hnc),
        ("cu-adatom", cu_s, cu_e),
        ("pt-h", pt_s, pt_e),
    ]
    for system, left, right in pairs:
        if left is None or right is None:
            continue
        saddle_rec = highest_record(records, system)
        if saddle_rec is None:
            continue
        state_a = freeze_atoms(left, f"{system}-a")
        state_b = freeze_atoms(right, f"{system}-b")
        saddle = freeze_atoms(
            Atoms(
                symbols=saddle_rec.symbols,
                positions=saddle_rec.positions,
                cell=saddle_rec.cell,
                pbc=True,
            ),
            f"{system}-saddle",
        )
        # freeze_atoms re-evaluates EMT, so the saddle text matches that geometry's EMT energy.
        piece, hops = kmc_trace(state_a, state_b, saddle, args.kmc_steps, rng)
        trace_parts.extend(piece)
        state_notes.append(
            {
                "system": system,
                "steps": args.kmc_steps,
                "frames": len(piece),
                "hops": hops,
                "state_a_eV": state_a["energy"],
                "state_b_eV": state_b["energy"],
                "saddle_eV": saddle["energy"],
                "state_bytes_equal": state_a["text"] == state_b["text"],
            }
        )
    trace_path = campaign / "akmc_trace.con"
    trace_path.write_text("".join(trace_parts))
    trace_frames = readcon.count_frames(str(trace_path)) if trace_parts else 0
    catalog.append(
        {
            "file": "akmc_trace.con",
            "n_frames": int(trace_frames),
            "sha256": sha256_file(trace_path) if trace_parts else None,
            "kmc": state_notes,
        }
    )
    print(f"  akmc trace: {trace_frames} frames", flush=True)
    (campaign / "catalog.json").write_text(json.dumps({"files": catalog}, indent=2) + "\n")

    manifest_path = DATA / "manifest.json"
    if args.skip_scale and manifest_path.is_file():
        manifest = json.loads(manifest_path.read_text())
        manifest["kmc_steps"] = args.kmc_steps
        manifest["search_steps"] = args.search_steps
        manifest["campaign"] = catalog
        manifest_path.write_text(json.dumps(manifest, indent=2) + "\n")
        print("updated campaign only", flush=True)
        return

    manifest = {"seed": args.seed, "kmc_steps": args.kmc_steps, "search_steps": args.search_steps, "campaign": catalog, "scale": []}
    for n in sizes:
        print(f"scale {n}", flush=True)
        info = fill_scale(DATA / "scale" / f"n{n}", records, bases, n, args.chunk, rng)
        info["name"] = f"n{n}"
        manifest["scale"].append(info)
        print(f"  n{n}: {info['raw_bytes']} bytes, {info['n_files']} files", flush=True)
    (DATA / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
    print("done", flush=True)


if __name__ == "__main__":
    main()
