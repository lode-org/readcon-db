#!/bin/sh
# Run one command with readcon-core patched to the campaign revision.
# Cargo.toml and Cargo.lock are restored even if the command fails.
# The published pin stays `=0.21.0`. See bench/readcon-core-0.23.toml.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname "$0")/.." && pwd)
cd "$ROOT"
CFG=$ROOT/bench/readcon-core-0.23.toml
cp Cargo.toml Cargo.toml.campaign-bak
cp Cargo.lock Cargo.lock.campaign-bak
restore() {
    mv Cargo.toml.campaign-bak Cargo.toml
    mv Cargo.lock.campaign-bak Cargo.lock
}
trap restore EXIT
# =0.21.0 cannot select the patched 0.23.0 crate. The trap puts the pin back.
sed -i 's/readcon-core = "=0.21.0"/readcon-core = "=0.23.0"/' Cargo.toml
cargo update -p readcon-core --config "$CFG" --manifest-path "$ROOT/Cargo.toml"
# 0.23 AtomDatum coordinates are methods. The published build leaves this unset.
# Cargo and maturin both read .cargo/config.toml, so the patch stays visible.
export READCON_DB_CORE_0_23=1
mkdir -p .cargo
if [ -f .cargo/config.toml ]; then
    cp .cargo/config.toml .cargo/config.toml.campaign-bak
fi
cp "$CFG" .cargo/config.toml
restore_cfg() {
    if [ -f .cargo/config.toml.campaign-bak ]; then
        mv .cargo/config.toml.campaign-bak .cargo/config.toml
    else
        rm -f .cargo/config.toml
    fi
}
trap 'restore; restore_cfg' EXIT
if [ "$1" = cargo ]; then
    shift
    cargo "$@"
elif [ "$1" = maturin ]; then
    shift
    maturin "$@"
else
    echo "usage: $0 cargo|maturin ..." >&2
    exit 2
fi
