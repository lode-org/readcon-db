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
# First version key only: the package's own, not a dependency's.
sed -i -E "0,/^version = \"[^\"]*\"/s//version = \"$version\"/" Cargo.toml
python3 scripts/sync-version.py
python3 scripts/check_version_lockstep.py
