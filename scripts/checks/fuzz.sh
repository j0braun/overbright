#!/bin/bash
# The fuzz smoke (docs/fuzzing.md): every parser target of fuzz/targets.rs gets mutated inputs over seeds
# from the player's game files; no panic, no hang, no allocation above Formats' Limits.
#
#   fuzz.sh
#
# Part of `checks.sh full`. Needs NFSU2_ROOT (the seeds are the player's own files); without it the built-in
# seeds run and the step says so. Environment: FUZZ_SECS (seconds per target, default 2), FUZZ_ITERS,
# FUZZ_SEED, FUZZ_ONLY, REPO, CARGO_TARGET_DIR, CARGO_BUILD_JOBS.
# Exit status: cargo's. Last line: "fuzz: N targets, ok|FAILED (game seeds|built-in seeds only)".
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(cd "$HERE/../.." && pwd)}
. "$HERE/../lib/common.sh"
ck_local_env
ck_need_tools cargo
cd "$REPO" || exit 2
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-6} CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$REPO/target}
export FUZZ_SECS=${FUZZ_SECS:-2}
N=$(grep -c '^    t!(' fuzz/targets.rs)
# With the harness feature, like the checks' own build: a plain `cargo test` here relinks
# $CARGO_TARGET_DIR/release/game as the shipping binary, and every later step that runs "$BIN"
# with a harness flag then gets the usage error (the rate step did, 2026-11-12).
cargo test --release --features dev-harness --test fuzz_smoke -- --nocapture
RC=$?
# A data test that was ignored (no game) did not fuzz the real seeds.
SEEDS="game seeds"; [ -n "${NFSU2_ROOT:-}" ] || SEEDS="built-in seeds only"
echo "fuzz: $N targets, $([ $RC = 0 ] && echo ok || echo FAILED) ($SEEDS)"
exit $RC
