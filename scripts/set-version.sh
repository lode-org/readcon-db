#!/usr/bin/env bash
# Keep the release tag and every language package on one version.
set -euo pipefail
version=${1:?usage: set-version.sh VERSION}
if ! [[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+([.-].*)?$ ]]; then
    echo "version must look like X.Y.Z" >&2
    exit 1
fi
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
for file in Cargo.toml python/pyproject.toml pixi.toml fortran/ReadConDb/fpm.toml; do
    sed -i -E "0,/^version = \"[^\"]*\"/s//version = \"$version\"/" "$file"
done
sed -i -E "/^name = \"readcon-db\"$/{n;s/^version = \"[^\"]*\"/version = \"$version\"/;}" Cargo.lock
sed -i -E "s/^    version: '[^']*'/    version: '$version'/" meson.build
sed -i -E "s/^release = \"[^\"]*\"/release = \"$version\"/; s/^version = \"[^\"]*\"/version = \"$version\"/" docs/source/conf.py
sed -i -E "s/^version: .*/version: $version/" CITATION.cff
bash scripts/check_version_lockstep.sh
