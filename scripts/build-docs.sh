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

if ! command -v cargo >/dev/null 2>&1; then
  echo "cargo is required to generate rustdoc" >&2
  exit 1
fi
echo "rustdoc"
# The docs and Pages jobs have no cargo registry cache. `--offline` then
# fails with "no matching package named heed". Fetch the lockfile first.
(
  cd "$ROOT"
  cargo fetch --locked
  RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps
)
# rustdoc 1.99 writes help.html and settings.html with the logo and the
# "Rustdoc" heading pointed at ./index.html. cargo doc does not emit that
# root page (it needs the unstable --enable-index-page flag), so those two
# links are missing after the tree is copied to _site/rust and _site/docs/rust.
python3 - "$ROOT/target/doc" <<'PY'
import html, json, pathlib, re, sys

doc = pathlib.Path(sys.argv[1])
index = doc / "index.html"
if index.is_file():
    raise SystemExit(0)
text = (doc / "crates.js").read_text()
match = re.search(r"window\.ALL_CRATES\s*=\s*(\[.*?\])\s*;", text)
if not match:
    sys.exit("target/doc/crates.js has no ALL_CRATES list")
crates = json.loads(match.group(1))
if len(crates) != 1:
    sys.exit(f"expected one rustdoc crate, found {crates}")
name = crates[0]
if not (doc / name / "index.html").is_file():
    sys.exit(f"cargo doc did not write {name}/index.html")
href = html.escape(f"{name}/index.html", quote=True)
label = html.escape(name)
index.write_text(
    "<!DOCTYPE html>\n"
    '<html lang="en">\n'
    "<head>\n"
    '<meta charset="utf-8">\n'
    f"<title>{label}</title>\n"
    f'<meta http-equiv="refresh" content="0; url={href}">\n'
    f'<link rel="canonical" href="{href}">\n'
    "</head>\n"
    "<body>\n"
    f'<p><a href="{href}">{label}</a></p>\n'
    "</body>\n"
    "</html>\n"
)
PY
# The generated Python page imports readcon_db. The docs environment does not
# ship that extension, so build it into the interpreter Sphinx is using.
if ! python3 -c "import readcon_db" >/dev/null 2>&1; then
  echo "install readcon_db for autodoc"
  python3 -m pip --version >/dev/null 2>&1 || python3 -m ensurepip --upgrade
  python3 -m pip install maturin
  # Pixi sets both VIRTUAL_ENV and CONDA_PREFIX. Maturin refuses that pair.
  # CONDA_PREFIX is the docs environment.
  (
    cd "$ROOT/python"
    env -u VIRTUAL_ENV maturin develop --release
  )
fi
sphinx-build -W -b html "$OUT" "$ROOT/docs/_build/html"
mkdir -p "$ROOT/docs/_build/html/rust"
cp -a "$ROOT/target/doc/." "$ROOT/docs/_build/html/rust/"
