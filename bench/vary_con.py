"""bench/vary_con.py SRC.con OUT.con FRAMES

Repeat one real CON file as a trajectory whose coordinates drift.
Frame 0 is the source text. Later frames add i*1e-4 angstrom to each x.
"""

import re
import sys

COORD = re.compile(
    r"^(\s*)(-?\d+\.\d+)(\s+-?\d+\.\d+\s+-?\d+\.\d+\s+\d+\s+\d+\s*)$",
    re.M,
)


def main() -> None:
    src, out, frames = sys.argv[1], sys.argv[2], int(sys.argv[3])
    text = open(src, encoding="utf-8").read()
    if not text.endswith("\n"):
        text += "\n"
    with open(out, "w", encoding="utf-8") as fh:
        for i in range(frames):
            if i == 0:
                fh.write(text)
                continue

            def shift(match: re.Match[str], step: int = i) -> str:
                x = float(match.group(2)) + step * 1e-4
                return f"{match.group(1)}{x:.6f}{match.group(3)}"

            fh.write(COORD.sub(shift, text))


if __name__ == "__main__":
    main()
