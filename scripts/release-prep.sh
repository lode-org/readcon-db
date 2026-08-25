#!/usr/bin/env bash
# Prepare a release commit.
# Usage: scripts/release-prep.sh X.Y.Z
# Requires: cog, prek, lychee, pixi (docs env). cargo test unless
# READCON_RELEASE_PREP_SKIP_TESTS=1 (run tests on the remote builder).
# Then open a PR, merge, and: git tag -s vX.Y.Z -m "vX.Y.Z" && git push origin vX.Y.Z
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
VER="${1:?usage: $0 X.Y.Z}"

if ! [[ "$VER" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-].*)?$ ]]; then
  echo "version must look like X.Y.Z" >&2
  exit 1
fi

for cmd in cog prek lychee; do
  if ! command -v "$cmd" >/dev/null 2>&1; then
    echo "$cmd required on PATH" >&2
    exit 1
  fi
done

if [[ "${READCON_RELEASE_PREP_SKIP_TESTS:-}" == 1 ]]; then
  echo "==> tests skipped (READCON_RELEASE_PREP_SKIP_TESTS=1)"
else
  echo "==> tests (default features)"
  cargo test --locked
fi

echo "==> version bump -> $VER"
sed -i "0,/^version = /{s/^version = \".*\"/version = \"${VER}\"/}" Cargo.toml
sed -i "s/^    version: '.*'/    version: '${VER}'/" meson.build
sed -i "0,/^version = /{s/^version = \".*\"/version = \"${VER}\"/}" python/pyproject.toml
sed -i "s/^release = \".*\"/release = \"${VER}\"/" docs/source/conf.py
sed -i "s/^version = \".*\"/version = \"${VER}\"/" docs/source/conf.py
sed -i "0,/^version = /{s/^version = \".*\"/version = \"${VER}\"/}" pixi.toml
sed -i "s/^version = \".*\"/version = \"${VER}\"/" fortran/ReadConDb/fpm.toml
sed -i "s/^version: .*/version: ${VER}/" CITATION.cff

echo "==> Cargo.lock refresh"
if [[ "${READCON_RELEASE_PREP_SKIP_TESTS:-}" == 1 ]]; then
  echo "skipping cargo test --locked; refresh Cargo.lock on the builder"
else
  cargo test --locked -q
fi

echo "==> CHANGELOG via cog"
prev="$(git describe --tags --abbrev=0)"
{
  sed -n '1,3p' CHANGELOG.md
  cog changelog "${prev}.." \
    | sed "s/^## Unreleased.*/## v${VER} - $(date +%F)/"
  echo
  awk '/^## v/{found=1} found' CHANGELOG.md
} > /tmp/CHANGELOG.md
mv /tmp/CHANGELOG.md CHANGELOG.md
if grep -q '^## Unreleased' CHANGELOG.md; then
  echo "CHANGELOG.md still has Unreleased; shipped tags must not dump it" >&2
  exit 1
fi

echo "==> version lockstep"
scripts/check_version_lockstep.sh

echo "==> prek"
prek run -a

echo "==> docs (sphinx), assemble site, and lychee"
if command -v pixi >/dev/null 2>&1 && [[ -f pixi.lock ]]; then
  pixi r -e docs docbld
elif command -v sphinx-build >/dev/null 2>&1; then
  sphinx-build -b html docs/source docs/_build/html
else
  python3 -m sphinx -b html docs/source docs/_build/html
fi
scripts/assemble-site.sh
lychee --config lychee.toml '_site/**/*.html'

echo "==> CPC fair-campaign freeze"
scripts/check-cpc-freeze.sh

echo "==> C/C++ distribution gate (no cbindgen required)"
scripts/check-cxx-dist.sh

echo "==> prebuilt C ABI gate (fixture, no cargo)"
scripts/check-clib-dist.sh --self-test

echo "==> stage release files"
git add Cargo.toml Cargo.lock meson.build python/pyproject.toml \
  docs/source/conf.py CHANGELOG.md \
  include/readcon-db.h cmake/ fortran/ReadConDb/fpm.toml CITATION.cff 2>/dev/null || true

echo "Ready. Review, then:"
echo "  git commit -m \"maint: bump to v${VER}\""
echo "  # open PR so CI (prek, cog, docs+lychee, cxx) runs"
echo "  # after merge:"
echo "  git tag -s v${VER} -m \"v${VER}\""
echo "  git push origin v${VER}"
echo "  # crates_publish.yml + python-wheels.yml + cxx_tarball.yml + c_lib_tarball.yml"
echo "  # After the tag: scripts/package-cxx.sh dist/ --vendor"
echo "  # Attach readcon-db-cxx-${VER}.tar.gz to the GitHub Release (cxx_tarball.yml)"
echo "  # Attach readcon-db-clib-${VER}-\$target.tar.gz (c_lib_tarball.yml, or dispatch tag=v${VER})"
