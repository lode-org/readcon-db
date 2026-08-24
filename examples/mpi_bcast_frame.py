#!/usr/bin/env python3
"""Rank 0 packs one RCSO frame; every rank receives it on the caller comm.

mpi4py (or LAMMPS) already started MPI. This script never calls Init.
Pass the communicator you own: LAMMPS ``lmp.world`` via mpi4py, or a
``Split`` / ``Dup``. The helper never names the process-wide world handle.

    mpirun -n 4 python examples/mpi_bcast_frame.py <corpus_dir> [traj] [frame]
"""

from __future__ import annotations

import sys

from mpi4py import MPI

from readcon_db import ConCorpus, bcast_packed_frame, bcast_packed_frames


def main(argv: list[str]) -> int:
    # Host-owned comm. Standalone we Dup the process-wide handle so the
    # helper never sees that handle unless the caller passed it. A LAMMPS
    # Python fix passes lmp.world (or a split) and skips this Dup.
    comm = MPI.COMM_WORLD.Dup()
    try:
        rank = comm.Get_rank()
        if len(argv) < 2:
            if rank == 0:
                print(
                    f"usage: {argv[0]} <corpus_dir> [traj] [frame]",
                    file=sys.stderr,
                )
            return 1
        corpus = argv[1]
        traj = int(argv[2]) if len(argv) > 2 else 1
        frame = int(argv[3]) if len(argv) > 3 else 0
        blob = bcast_packed_frame(comm, corpus, traj, frame, root=0)
        batch = bcast_packed_frames(comm, corpus, [(traj, frame)], root=0)
        xyz = ConCorpus.unpack_positions(blob)
        _ = ConCorpus.unpack_batch(batch)
        if rank == 0:
            x0, y0, z0 = xyz[0]
            print(
                f"bcast {len(blob)} bytes, natoms={len(xyz)} "
                f"xyz0=({x0:.4f},{y0:.4f},{z0:.4f})"
            )
        return 0
    finally:
        comm.Free()


if __name__ == "__main__":
    sys.exit(main(sys.argv))
