#!/bin/bash
# The developer probes still compile (docs/checks.md, docs/refactor/examples-triage.tsv).
#
#   devprobes.sh
#
# devprobes/ is a workspace member but not a default one, so the build step's
# `cargo build --release --bins --examples` does not build its ~160 binaries. This step type-checks
# them (`cargo check --release -p devprobes --bins`: no codegen, no linking) so a library change
# that breaks a probe is caught; the fix belongs in the same change. Part of `checks.sh full` only.
# Environment: REPO (default this script's repo), CARGO_TARGET_DIR (default REPO/target), CARGO_BUILD_JOBS.
# Exit status: cargo's (0 = every probe compiles); 3 when the tree has no devprobes/ (a public checkout: the probes
# are part of the maintainer's private set, scripts/release/private.list). Last line: "devprobes: N binaries
# checked, ok|FAILED" or "devprobes: SKIP ...".
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(cd "$HERE/../.." && pwd)}
. "$HERE/../lib/common.sh"
ck_local_env
ck_need_tools cargo
cd "$REPO" || exit 2
if [ ! -d devprobes/src/bin ] || ! grep -qE '^members *=.*"devprobes"' Cargo.toml; then
  echo "devprobes: SKIP (no devprobes/ workspace member here: the probes are in the maintainer's private set, not in a public checkout)"
  exit 3
fi
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-6} CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$REPO/target}
N=$(find devprobes/src/bin -maxdepth 1 -name '*.rs' | wc -l)
cargo check --release -p devprobes --bins
RC=$?
echo "devprobes: $N binaries checked, $([ $RC = 0 ] && echo ok || echo FAILED)"
exit $RC
