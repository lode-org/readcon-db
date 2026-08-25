#!/usr/bin/env bash
# Assemble GitHub Pages: Shibuya Sphinx is the site (same as readcon-core).
# Copy the tree to / and to /docs/ so both URLs work.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${1:-$ROOT/_site}"
HTML="$ROOT/docs/_build/html"

if [[ ! -f "$HTML/index.html" ]]; then
  echo "missing $HTML/index.html; run: pixi r -e docs docbld" >&2
  exit 1
fi

mkdir -p "$DEST/docs"
cp -a "$HTML/." "$DEST/"
cp -a "$HTML/." "$DEST/docs/"
if [[ -d "$ROOT/assets" ]]; then
  cp -a "$ROOT/assets" "$DEST/assets"
fi

if ! grep -Fq 'rc-hero' "$DEST/index.html"; then
  echo "Shibuya index must include the CON-frame hero (rc-hero)" >&2
  exit 1
fi
if [[ ! -f "$DEST/getting-started.html" || ! -f "$DEST/docs/getting-started.html" ]]; then
  echo "missing getting-started.html at / or /docs/" >&2
  exit 1
fi

echo "assembled $DEST"
