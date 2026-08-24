#!/usr/bin/env bash
# Structural gate: CPC appendix table matches paper/cpc/freeze/.
# Does not run the bench. Run from the repository root.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
exec python3 paper/cpc/scripts/gen_fair_table.py --check
