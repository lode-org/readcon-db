#!/usr/bin/env bash
# Assemble GitHub Pages tree: ox-html landing at / plus Sphinx under /docs/.
# Landing source is website/index.org (scripts/export-landing.sh).
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEST="${1:-$ROOT/_site}"
HTML="$ROOT/docs/_build/html"

if [[ ! -f "$HTML/index.html" ]]; then
  echo "missing $HTML/index.html; run: pixi r -e docs docbld" >&2
  exit 1
fi
if [[ ! -f "$ROOT/website/index.html" ]]; then
  echo "missing $ROOT/website/index.html" >&2
  exit 1
fi

mkdir -p "$DEST/docs"
cp -a "$ROOT/website/." "$DEST/"
cp -a "$HTML/." "$DEST/docs/"
cp -a "$ROOT/assets" "$DEST/assets"
cp "$HTML/objects.inv" "$DEST/objects.inv"

if ! grep -Fq 'href="docs/"' "$DEST/index.html"; then
  echo "landing Docs CTA must be href=\"docs/\" (published Sphinx tree)" >&2
  exit 1
fi
if [[ ! -f "$DEST/docs/index.html" ]]; then
  echo "missing $DEST/docs/index.html after assemble" >&2
  exit 1
fi

echo "assembled $DEST"
