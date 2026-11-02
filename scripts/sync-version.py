#!/usr/bin/env python3
"""Rewrite every copy of the version from the one in Cargo.toml.

pixi.toml, fortran/ReadConDb/fpm.toml and CITATION.cff carry cog blocks.
Cargo.lock keeps no comments, so an anchored pattern replaces any version
there. CI runs this and then `git diff --exit-code`.

Needs cogapp: python -m pip install cogapp
"""

import re
import sys
from pathlib import Path

from cogapp import Cog
from crate_version import crate_version

ROOT = Path(__file__).resolve().parent.parent
V = r"\d+\.\d+\.\d+(?:-(?:alpha|beta|rc|dev|pre)\.?\d*)?"

COG_FILES = ["pixi.toml", "fortran/ReadConDb/fpm.toml", "CITATION.cff"]
LOCK_ENTRY = rf'^(name = "readcon-db"\nversion = ")(?:{V})"'


def main():
    version = crate_version()
    status = Cog().main(
        ["cog", "-r", "-D", f"VERSION={version}", *(str(ROOT / f) for f in COG_FILES)]
    )
    if status:
        return status
    lock = ROOT / "Cargo.lock"
    text = lock.read_text(encoding="utf-8")
    new = re.sub(LOCK_ENTRY, rf'\g<1>{version}"', text, flags=re.MULTILINE)
    if new != text:
        lock.write_text(new, encoding="utf-8")
    return 0


if __name__ == "__main__":
    sys.exit(main())
