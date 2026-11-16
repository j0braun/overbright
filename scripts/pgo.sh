#!/usr/bin/env bash
# Optional profile-guided build of the game (README, "PGO"). The normal
# `cargo build --release` is untouched: this builds into its own target
# folders and leaves the result at $OUT/game.
#
#   scripts/pgo.sh [frames]      # NFSU2_ROOT must name the game folder
#
# Needs `rustup component add llvm-tools` (llvm-profdata matching rustc's
# LLVM; a system llvm-profdata of another LLVM version cannot read rustc's
# raw profiles). Works wherever rustc's -Cprofile-generate does (Linux,
# Windows under Git Bash/MSYS, macOS); the training run is the headless
# benches, so it needs the game's data and a GPU.
set -euo pipefail
cd "$(dirname "$0")/.."
FRAMES=${1:-600}
BASE=${CARGO_TARGET_DIR:-target}
GEN=$BASE/pgo-gen
OUT=$BASE/pgo
PROF=$BASE/pgo-profiles
: "${NFSU2_ROOT:?set NFSU2_ROOT to the game folder}"

SYSROOT=$(rustc --print sysroot)
HOST=$(rustc -vV | sed -n 's/^host: //p')
PROFDATA=$SYSROOT/lib/rustlib/$HOST/bin/llvm-profdata
[ -x "$PROFDATA" ] || [ -x "$PROFDATA.exe" ] || { echo "llvm-profdata not found: rustup component add llvm-tools" >&2; exit 1; }

rm -rf "$PROF"; mkdir -p "$PROF"
PROF_ABS=$(cd "$PROF" && pwd)

echo "== instrumented build"
CARGO_TARGET_DIR=$GEN RUSTFLAGS="${RUSTFLAGS:-} -Cprofile-generate=$PROF_ABS" cargo build --release --bin game --features dev-harness

echo "== training: the benches ($FRAMES frames each)"
for b in explore race racing drive; do
  "$GEN/release/game" --bench "$b" "$FRAMES" > "$PROF/train-$b.log" 2>&1 || { echo "bench $b failed (see $PROF/train-$b.log)" >&2; exit 1; }
done
"$GEN/release/game" --bench transitions > "$PROF/train-transitions.log" 2>&1 || { echo "bench transitions failed" >&2; exit 1; }

"$PROFDATA" merge -o "$PROF/merged.profdata" "$PROF_ABS"/*.profraw

echo "== optimised build"
CARGO_TARGET_DIR=$OUT RUSTFLAGS="${RUSTFLAGS:-} -Cprofile-use=$PROF_ABS/merged.profdata" cargo build --release --bin game --features dev-harness
mkdir -p "$OUT"
cp "$OUT/release/game" "$OUT/game" 2>/dev/null || cp "$OUT/release/game.exe" "$OUT/game.exe"
echo "PGO build: $OUT/game"
