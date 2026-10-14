"""Extra cuts on the same trajectory: zstd level, per-system ratio,
uncompressed SoA latency, bitshuffle size, and a tiny-frame floor.

Imports the generators from measure.py. Does not replace the LMDB table
in results.json.
"""

from __future__ import annotations

import json
import time
from pathlib import Path

import numpy as np
import zstandard as zstd

import measure as m

OUT = Path("/workspace/scratch/storage-a/sweep.json")


def bitshuffle(raw: bytes) -> bytes:
    """Bit-transpose every f64 in `raw` (bit i of every value is contiguous)."""
    x = np.frombuffer(raw, dtype=np.uint8)
    n_elem = x.size // 8
    pad = (-n_elem) % 8
    if pad:
        x = np.concatenate([np.frombuffer(raw, dtype=np.uint8), np.zeros(pad * 8, dtype=np.uint8)])
        n_elem += pad
    bits = np.unpackbits(x.reshape(n_elem, 8), axis=1, bitorder="little")
    packed = np.packbits(np.ascontiguousarray(bits.T), axis=1, bitorder="little")
    return packed.tobytes(), pad


def unbitshuffle(raw: bytes, n_bytes: int, pad_elems: int) -> bytes:
    n_elem = (n_bytes // 8) + pad_elems
    packed = np.frombuffer(raw, dtype=np.uint8)
    bits = np.unpackbits(packed.reshape(64, -1), axis=1, bitorder="little")
    back = np.packbits(np.ascontiguousarray(bits.T), axis=1, bitorder="little").reshape(-1)
    return back[:n_bytes].tobytes()


def main() -> None:
    rng = np.random.default_rng(m.RNG_SEED)
    md = m._simulate_md(rng)
    neb = m.generate_neb()
    blobs = {
        "md": [t.encode() for t in md["texts"]],
        "neb": [t.encode() for t in neb["texts"]],
    }
    all_blobs = blobs["md"] + blobs["neb"]
    levels = {}
    for level in (1, 3, 9):
        cctx = zstd.ZstdCompressor(level=level)
        dctx = zstd.ZstdDecompressor()
        t0 = time.perf_counter()
        comp = [cctx.compress(b) for b in all_blobs]
        compress_s = time.perf_counter() - t0
        t0 = time.perf_counter()
        for src, dst in zip(all_blobs, comp):
            if dctx.decompress(dst) != src:
                raise RuntimeError(f"level {level} round trip failed")
        decompress_s = time.perf_counter() - t0
        per = {}
        offset = 0
        for name, group in blobs.items():
            part = comp[offset : offset + len(group)]
            offset += len(group)
            raw = sum(len(b) for b in group)
            got = sum(len(b) for b in part)
            per[name] = {"raw_bytes": raw, "zstd_bytes": got, "ratio_raw_over_zstd": raw / got}
        levels[str(level)] = {
            "payload_bytes": sum(len(b) for b in comp),
            "compress_seconds": compress_s,
            "decompress_seconds": decompress_s,
            "decompress_us_per_frame": decompress_s / len(all_blobs) * 1e6,
            "compress_MiB_per_s": (sum(len(b) for b in all_blobs) / 1e6) / compress_s,
            "per_traj": per,
        }

    # Held-out plain zstd versus the dictionary already measured.
    dictionary = zstd.train_dictionary(
        m.DICT_SIZE,
        [blobs["md"][i] for i in m.train_ids(md["n_frames"], m.DICT_MD_TRAIN)]
        + [blobs["neb"][i] for i in m.train_ids(neb["n_frames"], m.DICT_NEB_TRAIN)],
        k=128,
        d=8,
    )
    c_plain = zstd.ZstdCompressor(level=3)
    c_dict = zstd.ZstdCompressor(level=3, dict_data=dictionary)
    md_held = [i for i in range(md["n_frames"]) if i not in set(m.train_ids(md["n_frames"], m.DICT_MD_TRAIN))]
    neb_held = [i for i in range(neb["n_frames"]) if i not in set(m.train_ids(neb["n_frames"], m.DICT_NEB_TRAIN))]

    def pack(idxs, group, ctx):
        raw = sum(len(group[i]) for i in idxs)
        got = sum(len(ctx.compress(group[i])) for i in idxs)
        return {"frames": len(idxs), "raw_bytes": raw, "zstd_bytes": got}

    held = {
        "md_plain": pack(md_held, blobs["md"], c_plain),
        "md_dict": pack(md_held, blobs["md"], c_dict),
        "neb_plain": pack(neb_held, blobs["neb"], c_plain),
        "neb_dict": pack(neb_held, blobs["neb"], c_dict),
    }

    # Uncompressed SoA in its own LMDB, one value per frame, 12-byte key.
    rows = []
    raw_soa = 0
    for traj in (md, neb):
        vel = traj["velocities"]
        for i in range(traj["n_frames"]):
            blob = m.soa_frame_bytes(
                traj["positions"][i],
                traj["forces"][i],
                None if vel is None else vel[i],
            )
            raw_soa += len(blob)
            rows.append((m.frame_key(traj["traj_id"], i), blob))
    env = m.open_env(m.OUT / "soa-raw")
    m.put_all(env, rows)
    env.sync()
    size = m.used_bytes(env)
    timing = m.bench_gets(env, [k for k, _ in rows], lambda v: bytes(v))
    env.close()

    # Bitshuffle + zstd size, in memory. Block is the whole chunk.
    def soa_list(traj):
        vel = traj["velocities"]
        return [
            m.soa_frame_bytes(
                traj["positions"][i],
                traj["forces"][i],
                None if vel is None else vel[i],
            )
            for i in range(traj["n_frames"])
        ]

    cctx = zstd.ZstdCompressor(level=3)
    bit_rows = {}
    for chunk in (1, 32):
        total_raw = 0
        total_sh = 0
        total_bit = 0
        for traj in (md, neb):
            frames = soa_list(traj)
            for c0 in range(0, len(frames), chunk):
                part = b"".join(frames[c0 : c0 + chunk])
                total_raw += len(part)
                shuffled = m.byte_shuffle(part)
                total_sh += len(cctx.compress(shuffled))
                bsh, pad = bitshuffle(part)
                back = unbitshuffle(bsh, len(part), pad)
                if back != part:
                    raise RuntimeError("bitshuffle round trip failed")
                total_bit += len(cctx.compress(bsh))
        bit_rows[f"c{chunk}"] = {
            "raw_soa_bytes": total_raw,
            "byte_shuffle_zstd_bytes": total_sh,
            "bitshuffle_zstd_bytes": total_bit,
        }

    # Tiny CON: four atoms, forces and velocities, full digits. The floor
    # under which a codec tag should stay raw.
    tiny_x = np.array(
        [[0.1, 0.2, 0.3], [1.5, 0.2, 0.3], [2.2, 2.3, 2.4], [2.2, 3.1, 2.4]],
        dtype=np.float64,
    )
    tiny_f = tiny_x * 0.01 + 1e-12
    tiny_v = tiny_x * 1e-4
    tiny = m.write_con(
        "tiny",
        {
            "con_spec_version": 3,
            "energy": -1.25,
            "sections": ["velocities", "forces"],
            "units": {"energy": "eV", "length": "angstrom", "mass": "amu", "time": "fs"},
        },
        np.array([12.0, 12.0, 12.0]),
        ["Cu", "H"],
        [2, 2],
        [63.546, 1.00784],
        tiny_x,
        tiny_f,
        tiny_v,
    ).encode()
    tiny_z = cctx.compress(tiny)

    # Level 1 inside LMDB, same tag and same get loop as measure.py's level 3.
    c1 = zstd.ZstdCompressor(level=1)
    d1 = zstd.ZstdDecompressor()
    l1_rows = [(m.frame_key(tid, i), b"\x01" + c1.compress(b)) for tid, i, b in (
        [(1, i, blobs["md"][i]) for i in range(len(blobs["md"]))]
        + [(2, i, blobs["neb"][i]) for i in range(len(blobs["neb"]))]
    )]
    env_l1 = m.open_env(m.OUT / "zstd-l1")
    m.put_all(env_l1, l1_rows)
    env_l1.sync()
    l1_size = m.used_bytes(env_l1)
    l1_timing = m.bench_gets(env_l1, [k for k, _ in l1_rows], lambda v: d1.decompress(bytes(v)[1:]))
    env_l1.close()

    OUT.write_text(
        json.dumps(
            {
                "levels": levels,
                "heldout_no_tag": held,
                "soa_raw_lmdb": {"lmdb": size, "value_bytes": raw_soa, "latency": timing},
                "bitshuffle": bit_rows,
                "tiny_con": {
                    "raw_bytes": len(tiny),
                    "zstd_l3_bytes": len(tiny_z),
                    "expands": len(tiny_z) >= len(tiny),
                },
                "zstd_l1_lmdb": {
                    "value_bytes": sum(len(v) for _, v in l1_rows),
                    "lmdb": l1_size,
                    "latency": l1_timing,
                },
            },
            indent=2,
        )
        + "\n"
    )
    print(json.dumps(json.loads(OUT.read_text()), indent=2))


if __name__ == "__main__":
    main()
