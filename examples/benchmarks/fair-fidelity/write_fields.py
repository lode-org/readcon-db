"""Regenerate ``FIELDS.md`` from a fresh two-store round trip."""

from __future__ import annotations

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import fair_fidelity


def main() -> None:
    fair_fidelity.write_fields()
    print(fair_fidelity.FIELDS_PATH)


if __name__ == "__main__":
    main()
