#!/usr/bin/env bash
# Structural gate: CPC appendix table matches the frozen fair campaign JSON.
# Does not run the bench. Run from the repository root.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
exec python3 paper/cpc/scripts/gen_appendix.py --check
