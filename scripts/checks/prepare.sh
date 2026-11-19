#!/bin/bash
# The prepare mode's check (docs/developing/prepare.md):  prepare.sh OUTDIR GAME_BIN
#
# GAME_BIN: a game built with the developer harness (the check drives a first load with --bench).
# On cache folders of its own under OUTDIR (the game's and the driver's), with the display variables cleared:
#   1. `game --prepare` on empty caches ends with ok=1, writes its stamp and (Vulkan) the pipeline cache;
#      `game --probe` then says prepared=1, and prepared=0 with another Graphics level in options.txt (the
#      stamp carries the settings' program key);
#   2. a second run changes no cache file (idempotent);
#   3. a first load after it (`--bench explore`, at the options the prepare run read) leaves the pipeline
#      cache file as it is: every pipeline the load made was in it;
#   4. a run killed half way leaves no stamp, the next run ends with ok=1, and `--verify-cache` passes.
# PREPARE_OPTIONS=file: that options.txt for every run (default: none, the native defaults).
# Prints "PREPARE: PASS (...)" or "PREPARE: FAIL (...)"; exit 0 / 1. NFSU2_ROOT from scripts/local.env or the caller.
set -u
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/../lib/common.sh"
ck_local_env
ck_need_root
O=${1:?usage: prepare.sh OUTDIR GAME_BIN}; BIN=$(readlink -f "${2:?usage: prepare.sh OUTDIR GAME_BIN}")
rm -rf "$O"; mkdir -p "$O"; O=$(cd "$O" && pwd)
FAILS=()
fail() { FAILS+=("$1"); echo "  FAIL: $1"; }
# run NAME CACHE ARGS…: one headless run in the folders of cache folder CACHE.
run() {
  local name=$1 c=$2; shift 2
  mkdir -p "$O/$c/cache" "$O/$c/data/overbright" "$O/$c/config" "$O/$c/xdgcache"
  [ -z "${PREPARE_OPTIONS:-}" ] || cp "$PREPARE_OPTIONS" "$O/$c/data/overbright/options.txt"
  env -u DISPLAY -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE NFSU2_HEADLESS_ONLY=1 NFSU2_CACHE_DIR="$O/$c/cache" \
    XDG_DATA_HOME="$O/$c/data" XDG_CONFIG_HOME="$O/$c/config" XDG_CACHE_HOME="$O/$c/xdgcache" \
    timeout 600 "$BIN" "$@" > "$O/$name.log" 2>&1
}
listing() { (cd "$O/$1/cache" && find . -type f ! -name '*.tmp' -printf '%p %s\n' | LC_ALL=C sort); }
pcache() { find "$O/$1/cache" -maxdepth 1 -name 'wgpu_pipeline_cache*' -printf '%s\n' | head -1; }

# 1. empty caches
t0=$(date +%s.%N)
run prepare1 a --prepare; rc=$?
t1=$(date +%s.%N)
{ [ $rc = 0 ] && grep -qx 'ok=1' "$O/prepare1.log"; } || fail "the first run: rc $rc, $(tail -1 "$O/prepare1.log")"
[ -s "$O/a/cache/prepared.txt" ] || fail "no stamp after the first run"
steps=$(grep -c '^step=' "$O/prepare1.log")
[ "$steps" -ge 3 ] || fail "only $steps step lines"
has_pc=$(grep -o '^pipeline_cache=[01]' "$O/prepare1.log" | cut -d= -f2)
if [ "$has_pc" = 1 ]; then [ -n "$(pcache a)" ] || fail "no pipeline cache file"; fi
run probe1 a --probe
grep -qx 'prepared=1' "$O/probe1.log" || fail "the probe does not say prepared=1 ($(grep '^prepared' "$O/probe1.log"))"
# The same stamp with another Graphics level in options.txt (SSAO on): not prepared for it.
mkdir -p "$O/lv/cache" "$O/lv/data/overbright"
cp "$O/a/cache/prepared.txt" "$O/lv/cache/"
{ [ -z "${PREPARE_OPTIONS:-}" ] || grep -v '^enh\.ssao ' "$PREPARE_OPTIONS"; echo 'enh.ssao 50'; } > "$O/lv-options.txt"
PREPARE_OPTIONS="$O/lv-options.txt" run probe-level lv --probe
grep -qx 'prepared=0' "$O/probe-level.log" || fail "the probe says prepared after the level changed ($(grep '^prepared' "$O/probe-level.log"))"
# 2. idempotent
listing a > "$O/list1.txt"
t2=$(date +%s.%N)
run prepare2 a --prepare; rc=$?
t3=$(date +%s.%N)
{ [ $rc = 0 ] && grep -qx 'ok=1' "$O/prepare2.log"; } || fail "the second run: rc $rc"
listing a > "$O/list2.txt"
cmp -s "$O/list1.txt" "$O/list2.txt" || fail "the second run changed cache files: $(diff "$O/list1.txt" "$O/list2.txt" | grep -c '^[<>]') lines"
# 3. a first load finds every pipeline in the cache
if "$BIN" --dev-harness > /dev/null 2>&1 < /dev/null && [ "$has_pc" = 1 ]; then
  before=$(pcache a)
  # (The bench works in a data folder of its own unless it is given one: the prepare run's, for its options.)
  NFSU2_FLOW_DATA="$O/a/data" run load a --bench explore 2; rc=$?
  [ $rc = 0 ] || fail "the first load after it: rc $rc"
  after=$(pcache a)
  [ "$before" = "$after" ] || fail "the first load added pipelines to the cache ($before -> $after bytes)"
  load=$(grep -o 'load [0-9]* ms, first frame [0-9.]* ms' "$O/load.log")
else
  load="first load not driven (no developer harness or no pipeline cache)"
fi
# 4. killed half way
mkdir -p "$O/b/cache" "$O/b/data/overbright" "$O/b/config" "$O/b/xdgcache"
[ -z "${PREPARE_OPTIONS:-}" ] || cp "$PREPARE_OPTIONS" "$O/b/data/overbright/options.txt"
env -u DISPLAY -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE NFSU2_HEADLESS_ONLY=1 NFSU2_CACHE_DIR="$O/b/cache" \
  XDG_DATA_HOME="$O/b/data" XDG_CONFIG_HOME="$O/b/config" XDG_CACHE_HOME="$O/b/xdgcache" \
  "$BIN" --prepare > "$O/prepare-killed.log" 2>&1 &
pid=$!
for _ in $(seq 1 200); do grep -q '^step=2/' "$O/prepare-killed.log" 2> /dev/null && break; sleep 0.02; done
sleep 0.15
kill -9 $pid 2> /dev/null; wait $pid 2> /dev/null
grep -qx 'ok=1' "$O/prepare-killed.log" && echo "  note: the run ended before it could be killed"
[ -e "$O/b/cache/prepared.txt" ] && ! grep -qx 'ok=1' "$O/prepare-killed.log" && fail "a stamp after a killed run"
run prepare3 b --prepare; rc=$?
{ [ $rc = 0 ] && grep -qx 'ok=1' "$O/prepare3.log"; } || fail "the run after a killed one: rc $rc, $(tail -1 "$O/prepare3.log")"
run verify b --verify-cache; rc=$?
[ $rc = 0 ] || fail "--verify-cache after a killed run and a full one: rc $rc, $(tail -1 "$O/verify.log")"
first=$(echo "$t1 - $t0" | bc); second=$(echo "$t3 - $t2" | bc)
what="first run ${first} s, second ${second} s, $(grep -o '^pipelines=[0-9]*' "$O/prepare1.log"), pipeline cache ${has_pc:-?}, $(du -sm "$O/a/cache" | cut -f1) MB; $load"
rm -rf "$O/a/cache" "$O/b/cache" "$O/a/xdgcache" "$O/b/xdgcache"
if [ ${#FAILS[@]} = 0 ]; then echo "PREPARE: PASS ($what)"; else echo "PREPARE: FAIL (${FAILS[*]}; $what)"; exit 1; fi
