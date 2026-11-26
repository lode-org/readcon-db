"""Cu107-with-forces trajectory from the repository copper fixture.

paper/reframe-v2 (52268690c7b671053ce997b10a26cf5a30edefdb) and
eval/cpc-paper (6146c3be9cde2d35f78cc540dedd978d995976b8) contain no
Cu107 trajectory. Frames are built from the two copper atoms in
resources/test/tiny_cuh2_forces.con: their coordinates, forces, mass,
header lines, and metadata JSON. Those two sites are tiled on a 5x5x5
lattice and the first 107 sites are kept. Frame k adds k * 1e-4 angstrom
to x and multiplies forces by (1 + k * 1e-6).
"""

from __future__ import annotations

import json
import math
from pathlib import Path

N_ATOMS = 107
GRID = 5
SHIFT_PER_FRAME = 1e-4
FORCE_SCALE_PER_FRAME = 1e-6

ROOT = Path(__file__).resolve().parents[2]
FIXTURE = ROOT / "resources" / "test" / "tiny_cuh2_forces.con"


def canon(value: float) -> float:
    """Decimal spelling that reads back as the same binary64 in Python."""
    return float(repr(float(value)))


def _rows_after(lines: list[str], label: str) -> list[tuple[float, float, float, int]]:
    for index, line in enumerate(lines):
        if line.strip() != label:
            continue
        if index == 0 or lines[index - 1].strip() != "Cu":
            continue
        rows: list[tuple[float, float, float, int]] = []
        for raw in lines[index + 1 :]:
            parts = raw.split()
            if len(parts) < 4:
                break
            try:
                rows.append(
                    (float(parts[0]), float(parts[1]), float(parts[2]), int(parts[3]))
                )
            except ValueError:
                break
        if not rows:
            break
        return rows
    raise RuntimeError(f"fixture has no Cu block for {label}")


def load_seed(path: Path = FIXTURE) -> dict:
    lines = path.read_text(encoding="utf-8").splitlines()
    if len(lines) < 9:
        raise RuntimeError("fixture header is short")
    meta = json.loads(lines[1])
    if "forces" not in meta.get("sections", []):
        raise RuntimeError("fixture metadata does not list forces")
    masses = lines[8].split()
    if not masses:
        raise RuntimeError("fixture has no mass")
    coords = _rows_after(lines, "Coordinates of Component 1")
    forces = _rows_after(lines, "Forces of Component 1")
    if len(forces) < len(coords):
        raise RuntimeError("fixture copper forces are shorter than coordinates")
    return {
        "line1": lines[0],
        "meta_line": lines[1],
        "angles_line": lines[3],
        "line5": lines[4],
        "line6": lines[5],
        "mass_token": masses[0],
        "coords": coords,
        "forces": forces[: len(coords)],
    }


def _spacing(seed: dict) -> float:
    coords = seed["coords"]
    if len(coords) >= 2:
        gap = abs(coords[1][0] - coords[0][0])
        if gap > 0.0:
            return gap
    return 2.5


def box_side(seed: dict, n_frames: int) -> float:
    origin = seed["coords"][0]
    spacing = _spacing(seed)
    shift = (n_frames - 1) * SHIFT_PER_FRAME
    peak = 0.0
    for axis in range(3):
        extra = shift if axis == 0 else 0.0
        peak = max(peak, origin[axis] + (GRID - 1) * spacing + extra)
    return float(max(40, math.ceil(peak + 5.0)))


def frame_geometry(
    seed: dict, frame_index: int
) -> tuple[list[tuple[float, float, float]], list[tuple[float, float, float]]]:
    origin = seed["coords"][0]
    spacing = _spacing(seed)
    shift = frame_index * SHIFT_PER_FRAME
    scale = 1.0 + frame_index * FORCE_SCALE_PER_FRAME
    positions: list[tuple[float, float, float]] = []
    forces: list[tuple[float, float, float]] = []
    for atom in range(N_ATOMS):
        ix = atom % GRID
        iy = (atom // GRID) % GRID
        iz = atom // (GRID * GRID)
        src = atom % len(seed["coords"])
        force = seed["forces"][src]
        positions.append(
            (
                canon(origin[0] + ix * spacing + shift),
                canon(origin[1] + iy * spacing),
                canon(origin[2] + iz * spacing),
            )
        )
        forces.append(
            (
                canon(force[0] * scale),
                canon(force[1] * scale),
                canon(force[2] * scale),
            )
        )
    return positions, forces


def write_con(path: Path, n_frames: int, seed: dict | None = None) -> dict:
    seed = load_seed() if seed is None else seed
    side = box_side(seed, n_frames)
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="\n") as handle:
        for frame_index in range(n_frames):
            positions, forces = frame_geometry(seed, frame_index)
            flags = [
                seed["coords"][atom % len(seed["coords"])][3] for atom in range(N_ATOMS)
            ]
            lines = [
                seed["line1"],
                seed["meta_line"],
                f"{side:.6f} {side:.6f} {side:.6f}",
                seed["angles_line"],
                seed["line5"],
                seed["line6"],
                "1",
                str(N_ATOMS),
                seed["mass_token"],
                "Cu",
                "Coordinates of Component 1",
            ]
            for atom, (pos, flag) in enumerate(zip(positions, flags)):
                lines.append(
                    f"   {pos[0]!r} {pos[1]!r} {pos[2]!r} {flag} {atom}"
                )
            lines.append("")
            lines.append("Cu")
            lines.append("Forces of Component 1")
            for atom, (force, flag) in enumerate(zip(forces, flags)):
                lines.append(
                    f"   {force[0]!r} {force[1]!r} {force[2]!r} {flag} {atom}"
                )
            handle.write("\n".join(lines))
            handle.write("\n")
    return seed
