"""python bench/gen_con.py OUT.con FRAMES ATOMS [SEED]

Write a synthetic CON trajectory of distinct frames (jittered fcc copper
in a periodic box) for the MPI ingest benchmark.
"""

import sys

import numpy as np


def main():
    out, frames, atoms = sys.argv[1], int(sys.argv[2]), int(sys.argv[3])
    rng = np.random.default_rng(int(sys.argv[4]) if len(sys.argv) > 4 else 0)
    side = int(np.ceil((atoms / 4) ** (1 / 3)))
    a = 3.61
    basis = np.array([[0, 0, 0], [0.5, 0.5, 0], [0.5, 0, 0.5], [0, 0.5, 0.5]])
    cells = np.array(
        [[i, j, k] for i in range(side) for j in range(side) for k in range(side)]
    )
    base = ((cells[:, None, :] + basis[None]) * a).reshape(-1, 3)[:atoms]
    box = side * a
    with open(out, "w") as f:
        for t in range(frames):
            r = base + rng.normal(scale=0.05, size=base.shape)
            f.write(f"Synthetic frame {t}\n\n")
            f.write(
                f"{box:.6f} {box:.6f} {box:.6f}\n90.000000 90.000000 90.000000\n\n\n"
            )
            f.write(f"1\n{atoms}\n63.546000\nCu\nCoordinates of Component 1\n")
            for i, p in enumerate(r):
                f.write(f"{p[0]:.6f} {p[1]:.6f} {p[2]:.6f} 0 {i}\n")


if __name__ == "__main__":
    main()
