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

versions="$(python3 - "$ROOT" <<'PY'
import pathlib, sys, tomllib
root = pathlib.Path(sys.argv[1])
cargo = tomllib.loads((root / "Cargo.toml").read_text())
py = tomllib.loads((root / "python" / "pyproject.toml").read_text())
version = cargo["package"]["version"]
project = py["project"]
static = project.get("version")
dynamic = project.get("dynamic") or []
if static is None:
    if "version" not in dynamic:
        sys.exit(
            "python/pyproject.toml has no version and does not mark version as dynamic"
        )
elif static != version:
    sys.exit(f"Cargo.toml has {version}, python/pyproject.toml has {static}")
print(version, cargo["dependencies"]["readcon-core"])
PY
)"
read -r RCDB_VERSION CORE_REQ <<<"$versions"

find "$OUT" -name '*.rst' -delete
while IFS= read -r -d '' src; do
  rel="${src#"$ORG"/}"
  base="$(basename "$rel")"
  case "$base" in
    readme.org) continue ;;
  esac
  dest="$OUT/${rel%.org}.rst"
  mkdir -p "$(dirname "$dest")"
  sed -e "s/{{{readcon-db-version}}}/$RCDB_VERSION/g" \
      -e "s/{{{readcon-core-req}}}/$CORE_REQ/g" "$src" |
    pandoc -f org -t rst --wrap=none -o "$dest"
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

if grep -rl '{{{readcon-' "$OUT" --include='*.rst'; then
  echo "unknown {{{readcon-...}}} macro in the files above" >&2
  exit 1
fi

sphinx-build -b html "$OUT" "$ROOT/docs/_build/html"
