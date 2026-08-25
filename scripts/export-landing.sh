#!/usr/bin/env bash
# Export website/index.org -> website/index.html via ox-html.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
if ! command -v emacs >/dev/null 2>&1; then
  echo "emacs required to export website/index.org" >&2
  exit 1
fi
emacs --batch -l website/export.el
if ! grep -Fq 'href="docs/"' website/index.html; then
  echo "exported landing Docs CTA must be href=\"docs/\"" >&2
  exit 1
fi
echo "export-landing: website/index.html"
