"""Write ``bench/db26/high_byte.json``.

The probe is ``cargo run --example db26_highbyte``. It refuses unless
``require_pushed_commit`` passes, and this entry point calls that same
function before launching it. The JSON has the forces-only column set and
the full-shuffle decode time, both on the same lanes as the other side.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

import db24_drive as fair

REPO = fair.REPO
OUT = REPO / "bench" / "db26" / "high_byte.json"


def main() -> dict:
    fair.require_pushed_commit()
    env = os.environ.copy()
    env["PATH"] = str(Path.home() / ".local" / "bin") + os.pathsep + env.get("PATH", "")
    env["RUSTUP_TOOLCHAIN"] = "1.88.0"
    env["TMPDIR"] = str(REPO / "build" / "tmp")
    env.setdefault("DB26_COMMIT", fair.git_out("rev-parse", "HEAD"))
    (REPO / "build" / "tmp").mkdir(parents=True, exist_ok=True)
    proc = subprocess.run(
        ["cargo", "run", "--release", "--offline", "--example", "db26_highbyte"],
        cwd=REPO,
        check=True,
        capture_output=True,
        text=True,
        env=env,
    )
    line = proc.stdout.strip().splitlines()[-1]
    payload = json.loads(line)
    # The example also writes the file. Pretty-print the same object.
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, indent=2) + "\n")
    return payload


if __name__ == "__main__":
    json.dump(main(), sys.stdout, indent=2)
    sys.stdout.write("\n")
