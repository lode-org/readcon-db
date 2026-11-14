"""One fair sample. Warm selected walks i*17 mod 64 once."""

import gc
import json
import os
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import fair_ase_protocol as fair

import numpy as np

DEST = Path(sys.argv[1])
KIND = sys.argv[2]
ARM = os.environ.get("READCON_BENCH_ARM", "lattice")
N_FRAMES = int(os.environ.get("READCON_BENCH_NFRAMES", "64"))
N_ATOMS = int(os.environ.get("READCON_BENCH_NATOMS", str(fair.arm_natoms(ARM))))
# 17 is coprime to 64, so this is one permutation of every frame.
FRAME_SEQUENCE = tuple((i * 17) % N_FRAMES for i in range(N_FRAMES))
HOLDER = {}
PAYLOAD = {}


def check(op: str, got, frame: int | None = None) -> None:
    if op == "metadata":
        lo, hi = fair.metadata_bounds(N_FRAMES)
        fair.assert_frame_ids(got, list(range(lo, hi + 1)))
        return
    pos = np.asarray(got["positions"])
    assert pos.shape[1:] == (N_ATOMS, 3), pos.shape
    if ARM == "lattice":
        assert pos.reshape(-1, N_ATOMS, 3)[0, 0, 1].tobytes() == np.float64(np.pi).tobytes()
        energy = np.asarray(got["energy"], dtype=np.float64)
        if op == "selected":
            mid = N_FRAMES // 2 if frame is None else frame
            assert pos.shape[0] == 1
            assert (
                energy.reshape(-1)[0].tobytes()
                == (np.float64(-42.5) + np.float64(mid)).tobytes()
            )
            return
        if op == "full_read":
            assert pos.shape[0] == N_FRAMES
            return
        lo, hi = fair.range_bounds(N_FRAMES)
        assert pos.shape[0] == hi - lo + 1
        return
    if op == "selected":
        mid = N_FRAMES // 2 if frame is None else frame
        assert pos.shape[0] == 1
        expect = fair.arm_positions(ARM, mid, N_ATOMS)
    elif op == "full_read":
        assert pos.shape[0] == N_FRAMES
        expect = fair.arm_positions(ARM, 0, N_ATOMS)
    else:
        lo, hi = fair.range_bounds(N_FRAMES)
        assert pos.shape[0] == hi - lo + 1
        expect = fair.arm_positions(ARM, lo, N_ATOMS)
    if np.asarray(pos[0]).tobytes() != np.asarray(expect, dtype=np.float64).tobytes():
        raise AssertionError(f"{ARM} {op} coordinates")


def cold_sample(op: str) -> dict:
    mid = N_FRAMES // 2
    state = {}

    def setup(state=state) -> None:
        fair.prepare_cold_files(KIND, DEST)
        if KIND == "readcon-db":
            state["db"] = fair.open_readcon(DEST / "corpus", warm=False)
        else:
            state["db"] = fair.open_ase_reader(DEST / "atoms.db")

    def teardown(state=state) -> None:
        check(op, state["got"])
        fair.close_reader(KIND, state["db"])
        state["db"] = None
        state["got"] = None

    def run_timed(state=state) -> None:
        state["got"] = fair.fetch_materialized(KIND, state["db"], op, mid, N_FRAMES)

    row = fair.sample_operation(run_timed, setup=setup, teardown=teardown)
    return {"seconds": row["seconds"], "inner_n": row["inner_n"]}


def ensure_warm() -> None:
    if "db" in HOLDER:
        return
    if KIND == "readcon-db":
        HOLDER["db"] = fair.open_readcon(DEST / "corpus", warm=True)
    else:
        HOLDER["db"] = fair.open_ase_reader(DEST / "atoms.db")
    got = fair.fetch_materialized(
        KIND, HOLDER["db"], "full_read", N_FRAMES // 2, N_FRAMES
    )
    check("full_read", got)


def warm_sample(op: str) -> dict:
    ensure_warm()
    if op == "selected":
        return warm_selected_distinct()
    mid = N_FRAMES // 2
    state = {}

    def run_timed(state=state) -> None:
        state["got"] = fair.fetch_materialized(KIND, HOLDER["db"], op, mid, N_FRAMES)

    def teardown(state=state) -> None:
        check(op, state["got"])
        state["got"] = None

    row = fair.sample_operation(run_timed, teardown=teardown)
    return {"seconds": row["seconds"], "inner_n": row["inner_n"]}


def warm_selected_distinct() -> dict:
    """One warm pass over every frame. The clock is that pass once."""
    ensure_warm()
    state = {}

    def run() -> None:
        energies = []
        last = None
        for idx in FRAME_SEQUENCE:
            got = fair.fetch_materialized(KIND, HOLDER["db"], "selected", idx, N_FRAMES)
            energies.append(np.asarray(got["energy"], dtype=np.float64).reshape(-1)[0])
            last = got
        state["energies"] = energies
        state["last"] = last

    def teardown() -> None:
        got = state["energies"]
        if len(set(FRAME_SEQUENCE)) != len(FRAME_SEQUENCE):
            raise AssertionError("sequence repeats a frame")
        for idx, energy in zip(FRAME_SEQUENCE, got):
            expect = np.float64(-42.5) + np.float64(idx)
            if energy.tobytes() != expect.tobytes():
                raise AssertionError((idx, float(energy), float(expect)))
        check("selected", state["last"], FRAME_SEQUENCE[-1])
        state.clear()

    collect = gc.isenabled()
    try:
        if collect:
            gc.disable()
        t0 = time.perf_counter()
        run()
        dt = time.perf_counter() - t0
    finally:
        if collect:
            gc.enable()
        teardown()
    return {"seconds": dt / N_FRAMES, "inner_n": N_FRAMES, "pass_s": dt}


def payload() -> dict:
    if "ready" in PAYLOAD:
        return PAYLOAD["ready"]
    if KIND == "readcon-db":
        if ARM == "lattice":
            text, records = fair.build_text_records(N_FRAMES, N_ATOMS)
        else:
            text, records = fair.build_arm(ARM, N_FRAMES, N_ATOMS)
        built = {
            "text": text,
            "records": records,
            "n_frames": N_FRAMES,
            "geometry_only": False,
        }
    else:
        atoms, records = fair.build_atom_records(N_FRAMES, N_ATOMS)
        built = {
            "atoms": atoms,
            "records": records,
            "n_frames": N_FRAMES,
            "geometry_only": False,
        }
    PAYLOAD["ready"] = built
    return built


def ingest_sample() -> dict:
    work = DEST.parent / f"ingest-{KIND}"
    stat = fair.timed_insert(KIND, "traj-100k", payload(), work)
    return {
        "seconds": stat["seconds"],
        "inner_n": stat["inner_n"],
        "disk_bytes": stat["disk_bytes"],
    }


def main() -> None:
    sys.stdout.write("READY\n")
    sys.stdout.flush()
    for line in sys.stdin:
        parts = line.strip().split()
        if not parts:
            continue
        if parts[0] == "quit":
            break
        if parts[0] == "ingest":
            row = ingest_sample()
        else:
            cache, op = parts
            row = cold_sample(op) if cache == "cold" else warm_sample(op)
        sys.stdout.write(json.dumps(row) + "\n")
        sys.stdout.flush()


if __name__ == "__main__":
    main()
