#!/usr/bin/env bash
# MPI ingest and export benchmark for readcon-db.
#
#   bench/mpi_bench.sh FRAMES ATOMS "1 2 4 8" [REPEATS]
#
# Needs one MPI implementation (set MPICC and LAUNCH, for example
# MPICC=mpicc.mpich LAUNCH="mpiexec.mpich -n"). Builds the release
# examples, writes a synthetic trajectory, and for every rank count runs
# single-writer and sharded ingest plus the read checks. Prints one line
# per run with the wall time and the sha256 match against serial.
# Heavy sizes belong on a cloud runner, not a laptop.
set -euo pipefail
FRAMES=${1:-10000}
ATOMS=${2:-1000}
RANKS=${3:-"1 2 4"}
REPS=${4:-3}
LAUNCH=${LAUNCH:-"mpiexec -n"}
ROOT=$(cd "$(dirname "$0")/.." && pwd)
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
cargo build --release --features mpi --example mpi_ingest_check --example mpi_read_check \
  --manifest-path "$ROOT/Cargo.toml" >&2
BIN=${CARGO_TARGET_DIR:-$ROOT/target}/release/examples
python3 "$ROOT/bench/gen_con.py" "$WORK/traj.con" "$FRAMES" "$ATOMS"
echo "# file_bytes=$(stat -c %s "$WORK/traj.con") frames=$FRAMES atoms=$ATOMS"
FILES="$WORK/traj.con $WORK/traj.con $WORK/traj.con $WORK/traj.con"
for p in $RANKS; do
  for mode in single sharded; do
    for r in $(seq "$REPS"); do
      $LAUNCH "$p" "$BIN/mpi_ingest_check" "$mode" "$WORK/i-$p-$mode-$r" 4096 4 $FILES | tail -1
      rm -rf "$WORK/i-$p-$mode-$r"
    done
  done
  mkdir -p "$WORK/r-$p"
  $LAUNCH "$p" "$BIN/mpi_read_check" "$WORK/r-$p" "$WORK/traj.con" | tail -1
  rm -rf "$WORK/r-$p"
done
