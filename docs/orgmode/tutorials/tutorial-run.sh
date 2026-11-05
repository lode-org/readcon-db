#!/usr/bin/env bash
# Run the commands of tutorial.org in a scratch directory, rewrite
# tutorial-run.log, then fail if a console block in tutorial.org is not a
# verbatim slice of that log.
# Usage: docs/orgmode/tutorials/tutorial-run.sh, after a release build of the crate
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
HERE="$ROOT/docs/orgmode/tutorials"
BIN="${READCON_DB:-$ROOT/target/release/readcon-db}"
[[ -x "$BIN" ]] || { echo "missing $BIN; run cargo build --release" >&2; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
mkdir "$WORK/bin"
ln -s "$BIN" "$WORK/bin/readcon-db"
ln -s "$ROOT/resources" "$WORK/resources"
export PATH="$WORK/bin:$PATH"

(
  cd "$WORK"
  while IFS= read -r cmd; do
    printf '$ %s\n' "$cmd"
    eval "$cmd" 2>&1
  done <<'EOF'
readcon-db ingest tutorial.db resources/test/tiny_multi_cuh2.con resources/test/sulfolene.con resources/test/tiny_cuh2_forces.con
ls tutorial.db
readcon-db select tutorial.db --symbol Cu
readcon-db select tutorial.db --symbol Cu --require-forces
readcon-db select tutorial.db --natoms-min 10
readcon-db select tutorial.db --energy-max 0
readcon-db positions tutorial.db --traj 1 --frame 0
readcon-db ingest tutorial.db --start-id 10 resources/test/tiny_multi_cuh2.con
readcon-db select tutorial.db --symbol Cu
readcon-db dedup-export tutorial.db --symbol Cu -o copper.xyz
rm -r tutorial.db copper.xyz
EOF
) > "$HERE/tutorial-run.log"

python3 - "$HERE/tutorial.org" "$HERE/tutorial-run.log" <<'PY'
import re, sys
page, log = (open(p).read() for p in sys.argv[1:])
blocks = re.findall(r"^#\+begin_src console\n(.*?)^#\+end_src", page, re.M | re.S)
bad = [b for b in blocks if b not in log]
for b in bad:
    print("not in tutorial-run.log:\n" + b, file=sys.stderr)
print(f"{len(blocks)} console blocks, {len(bad)} mismatched")
sys.exit(1 if bad or not blocks else 0)
PY
