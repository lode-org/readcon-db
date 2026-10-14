"""Campaign-storage prototypes for a mixed saddle-search corpus.

Scratch only: this does not import or modify readcon-db. It generates one
Lennard-Jones MD trajectory and one vacancy-hop NEB band (different cell,
species, and atom count), writes every floating field with full binary64
digits, and measures two lossless layouts against an uncompressed LMDB
baseline:

  A. Per-frame zstd of the exact CON bytes (plain and with a trained
     dictionary). One 12-byte key, one get, then decompress.
  B. Byte-shuffled f64 SoA (optional XOR residual against the previous
     frame in the chunk) compressed with zstd. Random access decompresses
     the chunk that owns the frame.

Sizes are LMDB used bytes, (last_pgno + 1) * page_size, after a fresh
environment per codec. Read latency is a warm-cache random permutation
inside one read transaction, copying each value out the way
ConCorpus::get_frame_text copies a heed value.
"""

from __future__ import annotations

import hashlib
import json
import os
import platform
import shutil
import time
from pathlib import Path

import lmdb
import numpy as np
import zstandard as zstd

OUT = Path("/tmp/storage-a")
RESULTS = Path("/workspace/scratch/storage-a/results.json")

# CODATA 2018. a [Å/fs^2] = (F [eV/Å] / m [amu]) * FORCE_TO_ACCEL.
_Q = 1.602176634e-19
_U = 1.66053906660e-27
FORCE_TO_ACCEL = (_Q / 1e-10) / _U * 1e-20
# KE [eV] = 0.5 * m [amu] * v [Å/fs]^2 * KE_TO_EV.
KE_TO_EV = _U * 1e10 / _Q
KB_EV = 8.617333262145e-5

MD_FRAMES = 600
MD_CELLS = 4  # 4^3 * 4 = 256 atoms
MD_DT_FS = 5.0
MD_TEMP_K = 400.0
MD_A_CU = 3.615
NEB_CELLS = 3  # 3^3 * 4 - 1 = 107 atoms
NEB_IMAGES = 19
NEB_ITERS = 60
NEB_A = 4.05
RNG_SEED = 20260314

ZSTD_LEVEL = 3
DICT_SIZE = 32 * 1024
DICT_MD_TRAIN = 32
DICT_NEB_TRAIN = 7


def fmt_f64(x: float) -> str:
    """17 significant digits, correctly rounded. Enough for a unique binary64."""
    s = format(float(x), ".16e")
    if float(s) != float(x):
        s = repr(float(x))
        if float(s) != float(x):
            raise RuntimeError(f"binary64 did not round-trip: {x!r}")
    return s


def dump_full(obj) -> str:
    if isinstance(obj, dict):
        body = ",".join(
            f"{json.dumps(k)}:{dump_full(v)}" for k, v in sorted(obj.items())
        )
        return "{" + body + "}"
    if isinstance(obj, list):
        return "[" + ",".join(dump_full(v) for v in obj) + "]"
    if isinstance(obj, float):
        return fmt_f64(obj)
    if isinstance(obj, bool):
        return "true" if obj else "false"
    if isinstance(obj, int):
        return str(obj)
    if isinstance(obj, str):
        return json.dumps(obj)
    raise TypeError(type(obj))


def fcc_sites(n_cells: int, a: float) -> np.ndarray:
    basis = np.array(
        [[0.0, 0.0, 0.0], [0.5, 0.5, 0.0], [0.5, 0.0, 0.5], [0.0, 0.5, 0.5]],
        dtype=np.float64,
    )
    cells = np.stack(
        np.meshgrid(
            np.arange(n_cells), np.arange(n_cells), np.arange(n_cells), indexing="ij"
        ),
        axis=-1,
    ).reshape(-1, 3)
    return ((cells[:, None, :] + basis[None]) * a).reshape(-1, 3)


def mic(d: np.ndarray, box: np.ndarray) -> np.ndarray:
    return d - box * np.round(d / box)


def wrap(x: np.ndarray, box: np.ndarray) -> np.ndarray:
    return x - box * np.floor(x / box)


def lj_force_energy(
    x: np.ndarray, sigma: np.ndarray, eps: np.ndarray, box: np.ndarray, rc: float
) -> tuple[np.ndarray, float]:
    d = mic(x[:, None, :] - x[None, :, :], box)
    r2 = np.einsum("ijk,ijk->ij", d, d)
    sig = 0.5 * (sigma[:, None] + sigma[None, :])
    ep = np.sqrt(eps[:, None] * eps[None, :])
    mask = (r2 > 1e-18) & (r2 < rc * rc)
    r2s = np.where(mask, r2, 1.0)
    inv_r2 = (sig * sig) / r2s
    sr6 = inv_r2**3
    sr12 = sr6 * sr6
    rc2 = rc * rc
    sr6_c = ((sig * sig) / rc2) ** 3
    sr12_c = sr6_c * sr6_c
    pair_e = np.where(mask, 4.0 * ep * ((sr12 - sr6) - (sr12_c - sr6_c)), 0.0)
    energy = 0.5 * float(pair_e.sum())
    fmag = np.where(mask, 24.0 * ep * (2.0 * sr12 - sr6) / r2s, 0.0)
    force = np.einsum("ij,ijk->ik", fmag, d)
    return force, energy


def kinetic_energy(v: np.ndarray, mass: np.ndarray) -> float:
    return float(0.5 * KE_TO_EV * np.sum(mass * np.sum(v * v, axis=1)))


def fmax(force: np.ndarray) -> float:
    return float(np.sqrt(np.einsum("ij,ij->i", force, force).max()))


def sigma_of_lattice(a: float) -> float:
    """sigma whose LJ minimum sits on the FCC nearest neighbour."""
    nn = a / np.sqrt(2.0)
    return float(nn / 2.0 ** (1.0 / 6.0))


def write_con(
    title: str,
    meta: dict,
    box: np.ndarray,
    symbols: list[str],
    counts: list[int],
    masses: list[float],
    x: np.ndarray,
    force: np.ndarray | None,
    vel: np.ndarray | None,
) -> str:
    lines = [
        title,
        dump_full(meta),
        f"{fmt_f64(box[0])}\t{fmt_f64(box[1])}\t{fmt_f64(box[2])}",
        f"{fmt_f64(90.0)}\t{fmt_f64(90.0)}\t{fmt_f64(90.0)}",
        "0 0",
        "218 0 1",
        str(len(symbols)),
        " ".join(str(c) for c in counts),
        " ".join(fmt_f64(m) for m in masses),
    ]
    cursor = 0
    blocks = [("Coordinates", x)]
    if vel is not None:
        blocks.append(("Velocities", vel))
    if force is not None:
        blocks.append(("Forces", force))
    # Species labels repeat before every section, matching the CON files
    # this crate already stores (component blocks, not a single header).
    species_blocks = []
    for name, arr in blocks:
        cursor = 0
        for comp, (sym, count) in enumerate(zip(symbols, counts), start=1):
            species_blocks.append(sym)
            species_blocks.append(f"{name} of Component {comp}")
            sl = arr[cursor : cursor + count]
            for local, row in enumerate(sl):
                atom_id = cursor + local
                species_blocks.append(
                    f"{fmt_f64(row[0])} {fmt_f64(row[1])} {fmt_f64(row[2])} 0 {atom_id}"
                )
            cursor += count
    lines.extend(species_blocks)
    return "\n".join(lines) + "\n"


def _relax(x, sigma, eps, box, rc, steps: int) -> tuple[np.ndarray, np.ndarray, float]:
    for _ in range(steps):
        force, energy = lj_force_energy(x, sigma, eps, box, rc)
        step = 0.04 * force
        nrm = np.linalg.norm(step, axis=1)
        cap = np.minimum(1.0, 0.04 / np.maximum(nrm, 1e-12))
        x = wrap(x + step * cap[:, None], box)
    force, energy = lj_force_energy(x, sigma, eps, box, rc)
    return x, force, energy


def generate_neb() -> dict:
    a = NEB_A
    sites = fcc_sites(NEB_CELLS, a)
    box = np.array([NEB_CELLS * a] * 3, dtype=np.float64)
    # Vacancy at the origin. The hopping atom starts on a nearest neighbour
    # and ends in the vacancy. Spectator order is identical on both ends.
    vacancy = 0
    neigh = int(np.linalg.norm(sites - np.array([0.5 * a, 0.5 * a, 0.0]), axis=1).argmin())
    if neigh == vacancy:
        raise RuntimeError("neighbour search failed")
    spectators = [i for i in range(sites.shape[0]) if i not in (vacancy, neigh)]
    reactant = np.vstack([sites[neigh], sites[spectators]])
    product = np.vstack([sites[vacancy], sites[spectators]])
    n = reactant.shape[0]
    sigma = np.full(n, sigma_of_lattice(a))
    eps = np.full(n, 0.20)
    rc = 5.5
    if rc >= 0.5 * float(box.min()):
        raise RuntimeError("NEB cutoff is past half the box")
    reactant, _, _ = _relax(reactant, sigma, eps, box, rc, 80)
    product, _, _ = _relax(product, sigma, eps, box, rc, 80)
    images = np.linspace(0.0, 1.0, NEB_IMAGES)[:, None, None] * mic(
        product - reactant, box
    )[None] + reactant
    images = wrap(images, box)
    images[0] = reactant
    images[-1] = product
    k_spring = 0.8
    energies = np.zeros(NEB_IMAGES)
    forces = np.zeros_like(images)
    for _ in range(NEB_ITERS):
        for i in range(NEB_IMAGES):
            forces[i], energies[i] = lj_force_energy(images[i], sigma, eps, box, rc)
        new = images.copy()
        for i in range(1, NEB_IMAGES - 1):
            tp = mic(images[i + 1] - images[i], box).ravel()
            tm = mic(images[i] - images[i - 1], box).ravel()
            dep = energies[i + 1] - energies[i]
            dem = energies[i] - energies[i - 1]
            if dep * dem > 0.0:
                tangent = tp if dep > 0.0 else tm
            else:
                tangent = tp if abs(dep) >= abs(dem) else tm
            norm = np.linalg.norm(tangent)
            if norm < 1e-15:
                continue
            tangent = tangent / norm
            f_true = forces[i].ravel()
            f_perp = f_true - np.dot(f_true, tangent) * tangent
            f_spring = k_spring * (np.dot(tp, tangent) - np.dot(tm, tangent)) * tangent
            step = 0.05 * (f_perp + f_spring).reshape(n, 3)
            nrm = np.linalg.norm(step, axis=1)
            cap = np.minimum(1.0, 0.05 / np.maximum(nrm, 1e-12))
            new[i] = wrap(images[i] + step * cap[:, None], box)
        images = new
    for i in range(NEB_IMAGES):
        forces[i], energies[i] = lj_force_energy(images[i], sigma, eps, box, rc)
    barrier = float(energies.max() - max(energies[0], energies[-1]))
    if barrier < 0.02:
        raise RuntimeError(f"NEB barrier collapsed: {energies.tolist()}")
    texts = []
    for i in range(NEB_IMAGES):
        meta = {
            "con_spec_version": 3,
            "energy": float(energies[i]),
            "frame_index": i,
            "neb_band": 0,
            "neb_bead": i,
            "pbc": [True, True, True],
            "potential": {"params": {"cutoff": rc, "epsilon": 0.20}, "type": "lj"},
            "sections": ["forces"],
            "units": {"energy": "eV", "length": "angstrom", "mass": "amu", "time": "fs"},
        }
        texts.append(
            write_con(
                "LJ NEB image",
                meta,
                box,
                ["Al"],
                [n],
                [26.9815385],
                images[i],
                forces[i],
                None,
            )
        )
    # Positions of every image, kept so the SoA tier is the same f64 the text
    # was printed from (full digits), not a re-parse.
    return {
        "traj_id": 2,
        "kind": "neb",
        "symbols": ["Al"],
        "n_atoms": n,
        "n_frames": NEB_IMAGES,
        "box": box,
        "texts": texts,
        "positions": images,
        "forces": forces,
        "velocities": None,
        "energies": [float(e) for e in energies],
        "barrier_eV": barrier,
        "endpoint_fmax": [fmax(forces[0]), fmax(forces[-1])],
        "saddle_index": int(energies.argmax()),
    }


def _simulate_md(rng: np.random.Generator) -> dict:
    """Same integration as a campaign MD file, retaining the f64 arrays."""
    a = MD_A_CU
    x = fcc_sites(MD_CELLS, a)
    n = x.shape[0]
    box = np.array([MD_CELLS * a] * 3, dtype=np.float64)
    n_ni = n // 5
    is_ni = np.zeros(n, dtype=bool)
    is_ni[rng.choice(n, n_ni, replace=False)] = True
    order = np.argsort(is_ni, kind="mergesort")
    x = x[order]
    is_ni = is_ni[order]
    sigma = np.where(is_ni, sigma_of_lattice(3.52), sigma_of_lattice(a)).astype(np.float64)
    eps = np.where(is_ni, 0.22, 0.17).astype(np.float64)
    mass = np.where(is_ni, 58.6934, 63.546).astype(np.float64)
    rc = 6.0
    if rc >= 0.5 * float(box.min()):
        raise RuntimeError("cutoff is past half the box")
    scale = np.sqrt(KB_EV * MD_TEMP_K / (mass * KE_TO_EV))
    v = rng.normal(size=x.shape) * scale[:, None]
    v -= np.average(v, axis=0, weights=mass)
    dt = MD_DT_FS
    force, pot = lj_force_energy(x, sigma, eps, box, rc)
    texts = []
    positions = np.empty((MD_FRAMES, n, 3), dtype=np.float64)
    forces = np.empty_like(positions)
    vels = np.empty_like(positions)
    energies = []
    rms_steps = []
    counts = [int((~is_ni).sum()), int(is_ni.sum())]
    for i in range(MD_FRAMES):
        positions[i] = x
        forces[i] = force
        vels[i] = v
        ke = kinetic_energy(v, mass)
        energies.append(pot + ke)
        meta = {
            "con_spec_version": 3,
            "energy": float(pot),
            "frame_index": i,
            "pbc": [True, True, True],
            "potential": {
                "params": {"cutoff": rc, "epsilon_Cu": 0.17, "epsilon_Ni": 0.22},
                "type": "lj",
            },
            "sections": ["velocities", "forces"],
            "time": i * dt,
            "timestep": dt,
            "units": {"energy": "eV", "length": "angstrom", "mass": "amu", "time": "fs"},
        }
        texts.append(
            write_con(
                "LJ MD frame",
                meta,
                box,
                ["Cu", "Ni"],
                counts,
                [63.546, 58.6934],
                x,
                force,
                v,
            )
        )
        accel = force / mass[:, None] * FORCE_TO_ACCEL
        v = v + 0.5 * dt * accel
        x_new = wrap(x + dt * v, box)
        rms_steps.append(float(np.sqrt(np.mean(np.sum(mic(x_new - x, box) ** 2, axis=1)))))
        x = x_new
        force, pot = lj_force_energy(x, sigma, eps, box, rc)
        accel = force / mass[:, None] * FORCE_TO_ACCEL
        v = v + 0.5 * dt * accel
    e = np.asarray(energies)
    drift = float((e.max() - e.min()) / abs(float(e[0])))
    if not np.isfinite(e).all() or drift > 0.05:
        raise RuntimeError(f"MD energy is not conserved (relative span {drift})")
    if max(rms_steps) > 0.5 or min(rms_steps) < 1e-6:
        raise RuntimeError(f"MD step RMS looks wrong: {min(rms_steps)}..{max(rms_steps)}")
    return {
        "traj_id": 1,
        "kind": "md",
        "symbols": ["Cu", "Ni"],
        "n_atoms": int(n),
        "n_frames": MD_FRAMES,
        "counts": counts,
        "texts": texts,
        "positions": positions,
        "forces": forces,
        "velocities": vels,
        "energy_first_eV": float(e[0]),
        "energy_last_eV": float(e[-1]),
        "energy_rel_span": drift,
        "rms_step_A_min": float(min(rms_steps)),
        "rms_step_A_max": float(max(rms_steps)),
    }


def frame_key(traj_id: int, frame_idx: int) -> bytes:
    """FrameKey::to_bytes: big-endian traj_id u64 || frame_idx u32 (12 bytes)."""
    return int(traj_id).to_bytes(8, "big") + int(frame_idx).to_bytes(4, "big")


def byte_shuffle(raw: bytes) -> bytes:
    a = np.frombuffer(raw, dtype=np.uint8)
    if a.size % 8 != 0:
        raise RuntimeError("shuffle input is not a whole number of f64")
    n = a.size // 8
    return np.ascontiguousarray(a.reshape(n, 8).T).tobytes()


def byte_unshuffle(raw: bytes) -> bytes:
    a = np.frombuffer(raw, dtype=np.uint8)
    n = a.size // 8
    return np.ascontiguousarray(a.reshape(8, n).T).tobytes()


def soa_frame_bytes(pos, force, vel) -> bytes:
    parts = [np.ascontiguousarray(pos, dtype="<f8").tobytes()]
    if force is not None:
        parts.append(np.ascontiguousarray(force, dtype="<f8").tobytes())
    if vel is not None:
        parts.append(np.ascontiguousarray(vel, dtype="<f8").tobytes())
    return b"".join(parts)


def chunk_payload(frames: list[bytes], xor: bool) -> bytes:
    width = len(frames[0])
    if any(len(f) != width for f in frames):
        raise RuntimeError("chunk mixes frame widths")
    bits = np.frombuffer(b"".join(frames), dtype=np.uint64).reshape(len(frames), -1).copy()
    if xor and len(frames) > 1:
        bits[1:] ^= bits[:-1]
    return byte_shuffle(bits.tobytes())


def chunk_unpayload(raw: bytes, nframes: int, xor: bool) -> bytes:
    bits = np.frombuffer(byte_unshuffle(raw), dtype=np.uint64).reshape(nframes, -1).copy()
    if xor and nframes > 1:
        for i in range(1, nframes):
            bits[i] ^= bits[i - 1]
    return bits.tobytes()


def open_env(path: Path) -> lmdb.Environment:
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True)
    # 512 MiB is address space. The file grows by pages actually written.
    return lmdb.open(
        str(path),
        map_size=512 << 20,
        subdir=True,
        max_dbs=4,
        sync=True,
        metasync=True,
        writemap=False,
        lock=True,
    )


def used_bytes(env: lmdb.Environment) -> dict:
    info = env.info()
    stat = env.stat()
    page = int(stat["psize"])
    last = int(info["last_pgno"])
    data = Path(env.path()) / "data.mdb"
    return {
        "page_size": page,
        "last_pgno": last,
        "used_bytes": (last + 1) * page,
        "file_bytes": data.stat().st_size,
    }


def put_all(env: lmdb.Environment, rows: list[tuple[bytes, bytes]]) -> None:
    with env.begin(write=True) as txn:
        for key, val in rows:
            txn.put(key, val)


def bench_gets(env: lmdb.Environment, keys: list[bytes], decode) -> dict:
    """Warm the pages, then time random gets. decode(bytes) -> bytes must
    touch the restored payload so a lazy buffer cannot pretend to be a read.
    """
    with env.begin() as txn:
        for key in keys:
            decode(txn.get(key))
    order = list(range(len(keys)))
    rng = np.random.default_rng(99)
    passes = []
    checksum = 0
    for _ in range(5):
        rng.shuffle(order)
        with env.begin() as txn:
            t0 = time.perf_counter_ns()
            for i in order:
                out = decode(txn.get(keys[i]))
                checksum = (checksum + out[0] + out[-1] + (len(out) & 255)) & 0xFFFFFFFF
            dt = time.perf_counter_ns() - t0
        passes.append(dt / len(keys))
    passes.sort()
    # Cold-ish: ask the kernel to drop the file, then time 48 random gets
    # one at a time. This is best-effort; the VM may ignore the advice.
    fd = os.open(str(Path(env.path()) / "data.mdb"), os.O_RDONLY)
    try:
        os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
    finally:
        os.close(fd)
    cold_keys = [keys[i] for i in order[:48]]
    cold = []
    with env.begin() as txn:
        for key in cold_keys:
            t0 = time.perf_counter_ns()
            out = decode(txn.get(key))
            checksum = (checksum + out[0] + out[-1]) & 0xFFFFFFFF
            cold.append(time.perf_counter_ns() - t0)
    cold.sort()
    return {
        "warm_ns_per_frame_median_of_5": passes[len(passes) // 2],
        "warm_ns_per_frame_min": passes[0],
        "warm_us_per_frame": passes[len(passes) // 2] / 1e3,
        "cold_us_p50": cold[len(cold) // 2] / 1e3,
        "cold_us_p90": cold[int(len(cold) * 0.9)] / 1e3,
        "checksum": int(checksum),
        "n_frames": len(keys),
    }


def train_ids(n: int, k: int) -> list[int]:
    if k >= n:
        return list(range(n))
    return [int(round(i * (n - 1) / (k - 1))) for i in range(k)]


def main() -> None:
    t_gen = time.perf_counter()
    rng = np.random.default_rng(RNG_SEED)
    md = _simulate_md(rng)
    neb = generate_neb()
    trajs = [md, neb]
    gen_s = time.perf_counter() - t_gen

    texts = [(t["traj_id"], i, blob.encode("utf-8")) for t in trajs for i, blob in enumerate(t["texts"])]
    raw_con = sum(len(b) for _, _, b in texts)
    uniq = len({b for _, _, b in texts})
    if uniq != len(texts):
        raise RuntimeError(f"frames are not all distinct ({uniq} unique / {len(texts)})")
    # Every printed scientific token that claims 17 digits must parse back.
    sample = (texts[0][2] + texts[-1][2]).decode("utf-8")
    bad = 0
    for tok in sample.split():
        if "e" in tok and tok[0] in "+-0123456789":
            try:
                if float(format(float(tok), ".16e")) != float(tok):
                    bad += 1
            except ValueError:
                pass
    if bad:
        raise RuntimeError(f"{bad} tokens failed the digit check")

    md_ids = set(train_ids(md["n_frames"], DICT_MD_TRAIN))
    neb_ids = set(train_ids(neb["n_frames"], DICT_NEB_TRAIN))
    train_samples = []
    held_md = []
    held_neb = []
    for tid, i, blob in texts:
        if tid == 1 and i in md_ids:
            train_samples.append(blob)
        elif tid == 2 and i in neb_ids:
            train_samples.append(blob)
        elif tid == 1:
            held_md.append(blob)
        else:
            held_neb.append(blob)
    dictionary = zstd.train_dictionary(DICT_SIZE, train_samples, k=128, d=8)
    dict_bytes = dictionary.as_bytes()

    cctx = zstd.ZstdCompressor(level=ZSTD_LEVEL)
    cctx_d = zstd.ZstdCompressor(level=ZSTD_LEVEL, dict_data=dictionary)
    dctx = zstd.ZstdDecompressor()
    dctx_d = zstd.ZstdDecompressor(dict_data=dictionary)

    def zstd_pack(blob: bytes, ctx) -> bytes:
        return b"\x01" + ctx.compress(blob)

    t0 = time.perf_counter()
    plain_rows = [(frame_key(tid, i), blob) for tid, i, blob in texts]
    zstd_rows = [(frame_key(tid, i), zstd_pack(blob, cctx)) for tid, i, blob in texts]
    dict_rows = [(frame_key(tid, i), zstd_pack(blob, cctx_d)) for tid, i, blob in texts]
    pack_s = time.perf_counter() - t0

    def split_size(rows, pred) -> int:
        return sum(len(v) for (k, v), (tid, i, _) in zip(rows, texts) if pred(tid, i))

    held_md_raw = sum(len(b) for b in held_md)
    held_neb_raw = sum(len(b) for b in held_neb)
    held_md_c = sum(len(zstd_pack(b, cctx_d)) for b in held_md)
    held_neb_c = sum(len(zstd_pack(b, cctx_d)) for b in held_neb)
    train_raw = sum(len(b) for b in train_samples)
    train_c = sum(len(zstd_pack(b, cctx_d)) for b in train_samples)

    # Numeric payloads from the integrator arrays, one self-contained frame
    # and several chunk widths. Chunks never cross a trajectory: the two
    # systems have different atom counts and different section sets.
    def traj_frame_blobs(traj) -> list[bytes]:
        out = []
        vel = traj["velocities"]
        force = traj["forces"]
        pos = traj["positions"]
        for i in range(traj["n_frames"]):
            out.append(
                soa_frame_bytes(
                    pos[i],
                    force[i],
                    None if vel is None else vel[i],
                )
            )
        return out

    # Random access pays a second get when the frame is not the chunk key.
    def soa_rows(chunk: int, xor: bool):
        rows = []
        owners = []  # (frame_key, chunk_key, local, width) in campaign order
        payload = 0
        per_traj = {}
        for traj in trajs:
            blobs = traj_frame_blobs(traj)
            got = 0
            for c0 in range(0, len(blobs), chunk):
                part = blobs[c0 : c0 + chunk]
                use_xor = xor and len(part) > 1
                body = chunk_payload(part, use_xor)
                if chunk_unpayload(body, len(part), use_xor) != b"".join(part):
                    raise RuntimeError(f"SoA round trip failed chunk={chunk} xor={xor}")
                comp = cctx.compress(body)
                cidx = c0 // chunk
                ckey = frame_key(0x8000_0000_0000_0000 | traj["traj_id"], cidx)
                header = (
                    len(part).to_bytes(4, "little")
                    + len(part[0]).to_bytes(4, "little")
                    + bytes([1 if use_xor else 0])
                )
                val = header + comp
                rows.append((ckey, val))
                payload += len(val)
                got += len(val)
                for local in range(len(part)):
                    fkey = frame_key(traj["traj_id"], c0 + local)
                    rows.append((fkey, ckey + local.to_bytes(4, "little")))
                    owners.append((fkey, ckey, local, len(part[0])))
            per_traj[traj["kind"]] = {
                "raw_soa_bytes": sum(len(b) for b in blobs),
                "stored_chunk_bytes": got,
                "n_frames": len(blobs),
            }
        return rows, owners, payload, per_traj

    soa_cfgs = []
    for chunk in (1, 8, 32):
        for xor in (False, True):
            if chunk == 1 and xor:
                continue
            rows, owners, payload, per_traj = soa_rows(chunk, xor)
            soa_cfgs.append(
                {
                    "name": f"soa-c{chunk}{'-xor' if xor else ''}",
                    "chunk": chunk,
                    "xor": xor,
                    "rows": rows,
                    "owners": owners,
                    "payload_bytes": payload,
                    "per_traj": per_traj,
                }
            )

    stores = {
        "plain": plain_rows,
        "zstd-l3": zstd_rows,
        "zstd-dict-l3": dict_rows + [(b"\xff" * 12, dict_bytes)],
    }

    def decode_plain(val: bytes) -> bytes:
        return bytes(val)

    def decode_zstd(val: bytes) -> bytes:
        return dctx.decompress(bytes(val)[1:])

    def decode_dict(val: bytes) -> bytes:
        if len(val) > 1000 and val[:4] == dict_bytes[:4]:
            return bytes(val[:16])
        return dctx_d.decompress(bytes(val)[1:])

    measured = {}
    keys = [k for k, _ in plain_rows]
    # Verify text codecs restore the exact bytes the hash would see.
    for row_c, row_p in zip(zstd_rows, plain_rows):
        if decode_zstd(row_c[1]) != row_p[1]:
            raise RuntimeError("zstd did not restore CON bytes")
    for row_c, row_p in zip(dict_rows, plain_rows):
        if decode_dict(row_c[1]) != row_p[1]:
            raise RuntimeError("dictionary zstd did not restore CON bytes")

    for name, rows in stores.items():
        env = open_env(OUT / name)
        put_all(env, rows)
        env.sync()
        size = used_bytes(env)
        if name == "plain":
            dec = decode_plain
            read_keys = keys
        elif name == "zstd-l3":
            dec = decode_zstd
            read_keys = keys
        else:
            dec = decode_dict
            read_keys = keys
        timing = bench_gets(env, read_keys, dec)
        # Confirm one restored MD frame still matches.
        with env.begin() as txn:
            got = dec(txn.get(keys[0]))
            if name != "plain" and got != plain_rows[0][1]:
                raise RuntimeError(f"{name} LMDB round trip failed")
            if name == "plain" and got != plain_rows[0][1]:
                raise RuntimeError("plain LMDB round trip failed")
        measured[name] = {
            "lmdb": size,
            "value_bytes": sum(len(v) for _, v in rows),
            "latency": timing,
        }
        env.close()

    def bench_soa(env, owners) -> dict:
        # Warm every frame through the two-get path.
        cache_key = None
        cache_body = None
        cache_width = 0

        def restore(txn, fkey, ckey, local, width) -> bytes:
            nonlocal cache_key, cache_body, cache_width
            if cache_key != ckey:
                val = bytes(txn.get(ckey))
                nframes = int.from_bytes(val[0:4], "little")
                width_h = int.from_bytes(val[4:8], "little")
                use_xor = val[8] == 1
                body = chunk_unpayload(dctx.decompress(val[9:]), nframes, use_xor)
                if len(body) != nframes * width_h:
                    raise RuntimeError("chunk length mismatch")
                cache_key = ckey
                cache_body = body
                cache_width = width_h
            start = local * cache_width
            return cache_body[start : start + cache_width]

        with env.begin() as txn:
            for fkey, ckey, local, width in owners:
                restore(txn, fkey, ckey, local, width)
        order = list(range(len(owners)))
        rng_b = np.random.default_rng(99)
        passes = []
        checksum = 0
        for _ in range(5):
            rng_b.shuffle(order)
            cache_key = None
            with env.begin() as txn:
                t0 = time.perf_counter_ns()
                for i in order:
                    fkey, ckey, local, width = owners[i]
                    out = restore(txn, fkey, ckey, local, width)
                    checksum = (checksum + out[0] + out[-1] + (len(out) & 255)) & 0xFFFFFFFF
                passes.append((time.perf_counter_ns() - t0) / len(owners))
        passes.sort()
        fd = os.open(str(Path(env.path()) / "data.mdb"), os.O_RDONLY)
        try:
            os.posix_fadvise(fd, 0, 0, os.POSIX_FADV_DONTNEED)
        finally:
            os.close(fd)
        cache_key = None
        cold = []
        with env.begin() as txn:
            for i in order[:48]:
                fkey, ckey, local, width = owners[i]
                # Drop the cache so each of these pays for its chunk. A real
                # random stream does not reuse the previous chunk on purpose;
                # forcing a miss matches "random frame, chunk not hot".
                cache_key = None
                t0 = time.perf_counter_ns()
                out = restore(txn, fkey, ckey, local, width)
                checksum = (checksum + out[0] + out[-1]) & 0xFFFFFFFF
                cold.append(time.perf_counter_ns() - t0)
        cold.sort()
        return {
            "warm_ns_per_frame_median_of_5": passes[len(passes) // 2],
            "warm_ns_per_frame_min": passes[0],
            "warm_us_per_frame": passes[len(passes) // 2] / 1e3,
            "cold_us_p50": cold[len(cold) // 2] / 1e3,
            "cold_us_p90": cold[int(len(cold) * 0.9)] / 1e3,
            "checksum": int(checksum),
            "n_frames": len(owners),
            "cold_note": "chunk cache cleared before every cold get",
        }

    for cfg in soa_cfgs:
        env = open_env(OUT / cfg["name"])
        put_all(env, cfg["rows"])
        env.sync()
        size = used_bytes(env)
        timing = bench_soa(env, cfg["owners"])
        measured[cfg["name"]] = {
            "lmdb": size,
            "value_bytes": cfg["payload_bytes"],
            "locator_bytes": 16 * len(cfg["owners"]),
            "latency": timing,
            "per_traj": cfg["per_traj"],
            "chunk": cfg["chunk"],
            "xor": cfg["xor"],
        }
        env.close()

    per_traj_raw = {}
    for traj in trajs:
        blob = b"".join(s.encode("utf-8") for s in traj["texts"])
        per_traj_raw[traj["kind"]] = {
            "n_atoms": traj["n_atoms"],
            "n_frames": traj["n_frames"],
            "symbols": traj["symbols"],
            "raw_con_bytes": len(blob),
            "raw_con_per_frame": len(blob) // traj["n_frames"],
        }

    text_payload = {
        "plain": sum(len(v) for _, v in plain_rows),
        "zstd-l3": sum(len(v) for _, v in zstd_rows),
        "zstd-dict-l3": sum(len(v) for _, v in dict_rows),
        "dict_bytes": len(dict_bytes),
        "train_frames": len(train_samples),
        "train_raw_bytes": train_raw,
        "train_dict_compressed_bytes": train_c,
        "held_md_frames": len(held_md),
        "held_md_raw_bytes": held_md_raw,
        "held_md_dict_compressed_bytes": held_md_c,
        "held_neb_frames": len(held_neb),
        "held_neb_raw_bytes": held_neb_raw,
        "held_neb_dict_compressed_bytes": held_neb_c,
        "pack_seconds": pack_s,
    }

    cpu = platform.processor() or platform.machine()
    try:
        cpu = Path("/proc/cpuinfo").read_text().split("model name")[1].split("\n", 1)[0].strip(" :\t")
    except (OSError, IndexError):
        pass

    results = {
        "zstd_version": list(zstd.ZSTD_VERSION),
        "cpu": cpu,
        "python": platform.python_version(),
        "generate_seconds": gen_s,
        "force_to_accel": FORCE_TO_ACCEL,
        "ke_to_ev": KE_TO_EV,
        "md": {
            "n_atoms": md["n_atoms"],
            "n_frames": md["n_frames"],
            "counts": md["counts"],
            "dt_fs": MD_DT_FS,
            "temperature_K": MD_TEMP_K,
            "energy_first_eV": md["energy_first_eV"],
            "energy_last_eV": md["energy_last_eV"],
            "energy_rel_span": md["energy_rel_span"],
            "rms_step_A_min": md["rms_step_A_min"],
            "rms_step_A_max": md["rms_step_A_max"],
        },
        "neb": {
            "n_atoms": neb["n_atoms"],
            "n_frames": neb["n_frames"],
            "symbols": neb["symbols"],
            "barrier_eV": neb["barrier_eV"],
            "endpoint_fmax": neb["endpoint_fmax"],
            "saddle_index": neb["saddle_index"],
            "energies_eV": neb["energies"],
        },
        "n_frames_total": len(texts),
        "n_unique_frames": uniq,
        "raw_con_bytes": raw_con,
        "per_traj_raw": per_traj_raw,
        "text_payload": text_payload,
        "stores": {
            name: {
                **{k: v for k, v in body.items() if k != "per_traj"},
                **({"per_traj": body["per_traj"]} if "per_traj" in body else {}),
            }
            for name, body in measured.items()
        },
        "corpus_sha256_16": hashlib.sha256(b"".join(b for _, _, b in texts)).hexdigest()[:16],
    }
    RESULTS.write_text(json.dumps(results, indent=2) + "\n")
    print(json.dumps({k: results[k] for k in ("md", "neb", "raw_con_bytes", "text_payload")}, indent=2))
    for name, body in measured.items():
        print(
            f"{name:16} used={body['lmdb']['used_bytes']:10} "
            f"file={body['lmdb']['file_bytes']:10} "
            f"vals={body['value_bytes']:10} "
            f"warm_us={body['latency']['warm_us_per_frame']:.2f} "
            f"cold_p50_us={body['latency']['cold_us_p50']:.1f}"
        )


if __name__ == "__main__":
    main()
