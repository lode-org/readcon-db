#!/usr/bin/env bash
# Org sources to rst (pandoc), then Shibuya HTML. Generated rst is not committed.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ORG="$ROOT/docs/orgmode"
OUT="$ROOT/docs/source"

if ! command -v pandoc >/dev/null 2>&1; then
  echo "pandoc is required to publish docs/orgmode" >&2
  exit 1
fi

find "$OUT" -name '*.rst' -delete
while IFS= read -r -d '' src; do
  rel="${src#"$ORG"/}"
  base="$(basename "$rel")"
  case "$base" in
    readme.org|cooked-soa.org) continue ;;
  esac
  dest="$OUT/${rel%.org}.rst"
  mkdir -p "$(dirname "$dest")"
  pandoc -f org -t rst --wrap=none "$src" -o "$dest"
done < <(find "$ORG" -name '*.org' -print0)

python3 - "$OUT" <<'PY'
import pathlib, sys
root = pathlib.Path(sys.argv[1])
for path in root.rglob("*.rst"):
    lines = path.read_text().splitlines()
    out = []
    for line in lines:
        if (
            out
            and out[-1].startswith("   ")
            and line
            and not line.startswith((" ", "\t"))
        ):
            out.append("")
        out.append(line)
    path.write_text("\n".join(out) + "\n")
PY

sphinx-build -b html "$OUT" "$ROOT/docs/_build/html"
