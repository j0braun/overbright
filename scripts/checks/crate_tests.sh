#!/bin/bash
# crate_tests.sh [repo]: every workspace crate's own unit tests, one crate at a time (`cargo test -p <crate> --lib`),
# so a test that only works because another crate's `dev-harness` feature (or a sibling's test helper) is unified into
# the workspace build fails here. The crates are the `package` lines of crate-layers.txt (the test kit and the developer
# probes excluded). The library (`nfsu2`) is the one package that needs its own `dev-harness` feature for its tests.
# Each run is under the player-folder mask (home_mask.sh); the folders are verified at the end.
# Env: CT_PROFILE (cargo profile, default gate), CT_ONLY ("ob-core ob-sim": just these), CT_OUT (log folder).
# Prints one "CRATE TESTS: <crate> ok|FAILED (<n> passed)" line per crate and a final "CRATE TESTS: ok|failed" line.
set -u -o pipefail
REPO=${1:-$(cd "$(dirname "$0")/../.." && pwd)}
HERE=$REPO/scripts/checks
cd "$REPO" || exit 2
PROFILE=${CT_PROFILE:-gate}
OUT=${CT_OUT:-${TMPDIR:-/tmp}/crate-tests.$$}
mkdir -p "$OUT"
crates=${CT_ONLY:-$(sed 's/#.*//' "$HERE/crate-layers.txt" | awk '$1=="package" && $2!="ob-testkit" && $2!="devprobes" {print $2}')}
fail=0
for c in $crates; do
  feat=(); [ "$c" = nfsu2 ] && feat=(--features dev-harness)
  if "$HERE/home_mask.sh" run "$OUT/mask-$c" cargo test --profile "$PROFILE" --locked -p "$c" --lib "${feat[@]}" > "$OUT/$c.log" 2>&1; then
    echo "CRATE TESTS: $c ok ($(grep -E '^test result' "$OUT/$c.log" | sed -E 's/.* ([0-9]+) passed.*/\1/' | paste -sd+ | bc) passed)"
  else
    echo "CRATE TESTS: $c FAILED (log $OUT/$c.log)"; grep -E '^(error|failures:|test .* FAILED)' "$OUT/$c.log" | head -10; fail=1
  fi
  "$HERE/home_mask.sh" verify "$OUT/mask-$c" > "$OUT/verify-$c.log" 2>&1 || { echo "CRATE TESTS: $c wrote to the player's folders"; fail=1; }
done
[ $fail = 0 ] && echo "CRATE TESTS: ok ($(wc -w <<< "$crates") crates, each on its own)" || echo "CRATE TESTS: failed"
exit $fail
