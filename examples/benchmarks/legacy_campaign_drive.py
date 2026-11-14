"""Rerun one historical fair campaign at the current sample count.

db1 through db7 are the full ladder. db8 and db9 are the subsets named in
their logs. db10 through db14 are the 100-frame corpus. The text sizes are
the ``bytes=`` values from each campaign log. Threads and the sample count
are the current fair protocol: twelve samples, readcon-db at one thread and
again with the pool unset, ASE at one thread, and one BLAS thread.
"""

from __future__ import annotations

import argparse
import os
import socket
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
REMOTE_TIP = "origin/fix/ingest-species-count"

LADDER_BYTES = {
    "paper40x8": 12480,
    "atoms-1000": 42552,
    "atoms-10000": 461328,
    "atoms-100000": 5009079,
    "atoms-1000000": 54086580,
    "frames-1": 725,
    "frames-100": 73625,
    "frames-10000": 7674467,
}
DB8_BYTES = {
    "atoms-100000": 5009079,
    "atoms-1000000": 54086580,
    "frames-100": 73625,
    "frames-10000": 7674467,
}
DB9_BYTES = {
    "atoms-1000000": 54086580,
    "frames-10000": 7674467,
    "frames-100": 73625,
}
FRAMES_BYTES = {"frames-100": 73625}

# db13 has the same frames-100 workload as db10. The harness mode is db10.
CAMPAIGNS = {
    "db1": {"mode": "all", "bytes": LADDER_BYTES},
    "db2": {"mode": "all", "bytes": LADDER_BYTES},
    "db3": {"mode": "all", "bytes": LADDER_BYTES},
    "db4": {"mode": "all", "bytes": LADDER_BYTES},
    "db5": {"mode": "all", "bytes": LADDER_BYTES},
    "db6": {"mode": "all", "bytes": LADDER_BYTES},
    "db7": {"mode": "all", "bytes": LADDER_BYTES},
    "db8": {"mode": "db8", "bytes": DB8_BYTES},
    "db9": {"mode": "db9", "bytes": DB9_BYTES},
    "db10": {"mode": "db10", "bytes": FRAMES_BYTES},
    "db11": {"mode": "db11", "bytes": FRAMES_BYTES},
    "db12": {"mode": "db12", "bytes": FRAMES_BYTES},
    "db13": {"mode": "db10", "bytes": FRAMES_BYTES},
    "db14": {"mode": "db14", "bytes": FRAMES_BYTES},
}


def measure_root() -> Path:
    root = Path(os.environ.get("READCON_MEASURE_ROOT", ""))
    if root.is_dir():
        return root
    return REPO


def require_tip() -> str:
    """Refuse unless HEAD is the remote branch tip and the tree is clean."""
    root = measure_root()
    subprocess_fetch(root)
    tip = _git(root, "rev-parse", REMOTE_TIP)
    head = _git(root, "rev-parse", "HEAD")
    if head != tip:
        raise SystemExit(
            f"refusing to measure: HEAD {head} is not the remote branch tip {tip}"
        )
    dirty = _git(root, "status", "--porcelain")
    if dirty.strip():
        raise SystemExit("refusing to measure: working tree is not clean")
    return head


def subprocess_fetch(root: Path) -> None:
    import subprocess

    subprocess.check_call(
        ["git", "fetch", "origin", "fix/ingest-species-count"], cwd=root
    )


def _git(root: Path, *args: str) -> str:
    import subprocess

    return subprocess.check_output(["git", *args], cwd=root, text=True).strip()


def log_bytes(log_path: Path) -> dict[str, int]:
    found = {}
    for line in log_path.read_text().splitlines():
        if not line.startswith("workload ") or "bytes=" not in line:
            continue
        name = line.split()[1]
        for part in line.split():
            if part.startswith("bytes="):
                found[name] = int(part.split("=", 1)[1])
    return found


def check_log(name: str, out: Path, tip: str) -> None:
    log_path = out / "campaign.log"
    lines = log_path.read_text().splitlines()
    if not any(line == "samples 12" for line in lines):
        raise SystemExit(f"{name} did not keep 12 samples")
    if not any(line.startswith("rev_parse " + tip) for line in lines):
        raise SystemExit(f"{name} was not measured on {tip}")
    if lines[2] != "host host":
        raise SystemExit(f"{name} host line is {lines[2]!r}")
    got = log_bytes(log_path)
    expect = CAMPAIGNS[name]["bytes"]
    if got != expect:
        raise SystemExit(f"{name} text sizes {got} != recorded {expect}")


def run(name: str, out: Path) -> None:
    root = measure_root()
    bench = root / "examples" / "benchmarks"
    sys.path.insert(0, str(bench))
    import fair_ase_protocol as fair

    tip = require_tip()
    socket.gethostname = lambda: "host"
    spec = CAMPAIGNS[name]
    sys.argv = [fair.__file__, "--out", str(out), "--mode", spec["mode"]]
    fair.main()
    check_log(name, out, tip)


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--campaign", required=True, choices=tuple(CAMPAIGNS))
    ap.add_argument("--out", type=Path, required=True)
    args = ap.parse_args()
    run(args.campaign, args.out)


if __name__ == "__main__":
    main()
