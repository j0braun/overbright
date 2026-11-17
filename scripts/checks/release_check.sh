#!/bin/bash
# The shipping binary leaves the developer harness out (docs/checks.md "Release check").
#
#   release_check.sh [OUTDIR]
#
# Builds `game` with the default features (no `dev-harness`) into its own target dir
# ($CARGO_TARGET_DIR/ship: the checks' own binary in $CARGO_TARGET_DIR/release stays the harness build),
# then asserts:
#   1. none of the harness's strings is in it: the private test saves' file and profile names (in the
#      maintainer's checkout, from the privacy scan's leak terms), the harness's flags (--selftest, --playthrough, --flow, --photo; not --shot / --bench: Mark Spot's
#      report names `game --shot race` and `game --bench explore 600` for the developers) and variables (NFSU2_STATE_HASH,
#      NFSU2_SHOT_DATA, NFSU2_SHOT_MS, NFSU2_SHOT_REAL_SAVE, NFSU2_DEV_SHOP, NFSU2_AUDIO_WAV);
#   2. every NFSU2_SHOT_* string left is one the game itself writes or reads: Mark Spot's repro lines
#      (src/mark_spot.rs: SAVE, CAR, AT, HOLD, LOOK, SIZE, ENH, EVENT) and the enhancement override
#      the renderer reads (src/render/debug_env.rs: ENH);
#   3. a harness flag (`--selftest`, `--dev-harness`) ends in the usage error: exit code 2, "unknown option".
# Without the privacy scan (a public checkout) or its private terms the save names are not searched: the check
# says so ("privacy scan: maintainer tool, not in this checkout" / "private terms: NOT LOADED"), runs the rest, and ends "RELEASE: ok, private terms NOT LOADED (...)" with exit status 3, never a plain ok.
# Last line: "RELEASE: ok (size N bytes)" or "RELEASE: FAIL (...)". Exit status 0 only on ok.
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(cd "$HERE/../.." && pwd)}
. "$HERE/../lib/common.sh"
ck_local_env
ck_need_tools cargo strings
cd "$REPO" || exit 2
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-6} CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$REPO/target}
SHIP_TARGET=${SHIP_TARGET:-$CARGO_TARGET_DIR/ship}
O=${1:-$SHIP_TARGET}; mkdir -p "$O"
# The game's asset cache: the caller's NFSU2_CACHE_DIR, else a folder of this run (never the player's cache).
export NFSU2_CACHE_DIR="${NFSU2_CACHE_DIR:-$O/cache}"
cargo build --release --bin game --target-dir "$SHIP_TARGET" || { echo "RELEASE: FAIL (build)"; exit 1; }
BIN=$SHIP_TARGET/release/game
F=()
strings -n 4 "$BIN" > "$O/strings.txt"
# The private save names (the maintainer's privacy scan; never published).
PRIV_LOADED=1 SCAN=$REPO/scripts/release/scan.py
if [ ! -f "$SCAN" ]; then
  PRIV_LOADED=0; PRIV_TERMS=""
  echo "release: privacy scan: maintainer tool, not in this checkout"
elif PRIV_TERMS=$(python3 -I "$SCAN" leak-terms 2> "$O/leak-terms.txt"); then
  echo "release: private terms: loaded $(echo -n "$PRIV_TERMS" | grep -c .)"
else
  PRIV_LOADED=0; PRIV_TERMS=""
  echo "release: private terms: NOT LOADED (needs the maintainer's private set (OB_PRIVATE)); the private save names were not searched"
fi
for s in $PRIV_TERMS --selftest --playthrough --flow --photo NFSU2_STATE_HASH NFSU2_SHOT_DATA NFSU2_SHOT_MS \
  NFSU2_SHOT_REAL_SAVE NFSU2_DEV_SHOP NFSU2_AUDIO_WAV; do
  n=$(grep -c -F -- "$s" "$O/strings.txt")
  [ "$n" = 0 ] || { echo "release: '$s' found $n times"; F+=("$s"); }
done
# The linker packs string constants back to back ("NFSU2_SHOT_ENHNFSU2_SKY_GLOW"): a name counts as the
# allowed one when the rest is empty or the next variable's start.
ALLOWED="NFSU2_SHOT_SAVE NFSU2_SHOT_CAR NFSU2_SHOT_AT NFSU2_SHOT_HOLD NFSU2_SHOT_LOOK NFSU2_SHOT_SIZE NFSU2_SHOT_ENH NFSU2_SHOT_EVENT"
for s in $(grep -o 'NFSU2_SHOT_[A-Z_]*' "$O/strings.txt" | sort -u); do
  ok=0
  for a in $ALLOWED; do case $s in "$a" | "$a"NFSU*) ok=1 ;; esac; done
  [ $ok = 1 ] || { echo "release: $s found (not one of the game's own)"; F+=("$s"); }
done
for flag in --selftest --dev-harness; do
  out=$(NFSU2_HEADLESS_ONLY=1 "$BIN" "$flag" 2>&1); rc=$?
  if [ $rc = 2 ] && echo "$out" | grep -q "unknown option $flag"; then echo "release: $flag -> rc 2, usage error"
  else echo "release: $flag -> rc $rc: $(echo "$out" | head -2 | tr '\n' ' ')"; F+=("$flag"); fi
done
SIZE=$(stat -c %s "$BIN")
if [ ${#F[@]} = 0 ] && [ $PRIV_LOADED = 0 ]; then echo "RELEASE: ok, private terms NOT LOADED (size $SIZE bytes)"; exit 3; fi
if [ ${#F[@]} = 0 ]; then echo "RELEASE: ok (size $SIZE bytes)"; exit 0; fi
echo "RELEASE: FAIL (${F[*]}; size $SIZE bytes)"; exit 1
