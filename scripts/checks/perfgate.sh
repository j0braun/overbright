#!/bin/bash
# Performance regression gate (docs/perf-budget.md "The regression gate").
#
#   perfgate.sh run    OUT NEW_BIN [BASELINE_DIR]   A/B of NEW_BIN against the accepted baseline binary,
#                                                   interleaved; exit 1 on a significant > 3 % regression
#   perfgate.sh accept OUT [NOTE]                   make OUT's NEW binary (and its runs) the accepted baseline
#   perfgate.sh seed   OUT NEW_BIN [NOTE]           first baseline: an A/A run of NEW_BIN (checks the gate's
#                                                   false-positive rate), then accept
#
# NEW_BIN: a binary built with perf.sh build (full release profile; gate.sh leaves one in OUTDIR/b/game).
# The set (8 configs, the gate set in perf.sh): racing / explore / race start Original 1080p, racing Original 4K,
# racing Ultra 4K, explore Extreme 1080p, rain "max" 4K, race start "max" 1080p ("max" = the machine-local play
# setup PRESET_user plus the newest rows; scripts/local.env). Without PRESET_user (a public checkout) the two "max"
# configs are left out: "perfgate: SKIP ..." lines, and the PERFGATE verdict names them ("skipped 2: ...").
# REPS 5 (Mann-Whitney 5+5: smallest p 0.008), FRAMES 300.
# Baselines: $PERF_BASELINES (required, e.g. in scripts/local.env: measured on one machine, so never in the
# repo), one dir per accepted binary (game, meta.txt, summary.json, compare.md), "current" -> the accepted one.
# A bench that did not complete (a run with rc != 0 or no JSON) fails the gate (exit 1), in both modes.
# A baseline dir without a game binary (or PERFGATE_JSON=1): NEW alone against its summary.json
# (indicative only: another time and load; never a hard fail unless PERFGATE_JSON_FAIL=1).
# OUT holds the runs and logs (~2 MB after the run; the ~550 MB of asset caches are removed unless
# PERFGATE_KEEP=1): on the big disk, never /tmp. Bench ~320 s (8 configs x 2 arms x 5 reps + warm-up).
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/../lib/common.sh"
ck_local_env
PERF="$HERE/../perf/perf.sh"; STAT="$HERE/../perf/perfstat.py"
die() { echo "perfgate: $*" >&2; exit 2; }
STORE=${PERF_BASELINES:-}
need_store() { [ -n "$STORE" ] || die "PERF_BASELINES is not set (the accepted baselines' folder; scripts/local.env)"; }
export SET=gate REPS=${REPS:-5} FRAMES=${FRAMES:-300}

bench_ab() {  # OUT BASE_BIN NEW_BIN
  local out=$1; mkdir -p "$out"
  local t0=$(date +%s)
  echo "perfgate: bench set gate, reps $REPS, frames $FRAMES; loadavg $(cut -d' ' -f1-3 /proc/loadavg)" | tee "$out/perfgate.log"
  "$PERF" bench "$out" base="$2" new="$3" >> "$out/perfgate.log" 2>&1; local rc=$?
  grep '^bench: SKIP ' "$out/perfgate.log" | sed 's/^bench: /perfgate: /' | sort -u
  echo "perfgate: bench took $(( $(date +%s) - t0 )) s, rc $rc; loadavg $(cut -d' ' -f1-3 /proc/loadavg)" | tee -a "$out/perfgate.log"
  # The per-label asset caches and data dirs are ~550 MB; the runs (JSON, logs) stay. PERFGATE_KEEP=1 keeps them.
  [ -n "${PERFGATE_KEEP:-}" ] || rm -rf "$out"/cache-* "$out"/xdg-*
  [ $rc = 0 ] || { echo "perfgate: BENCH FAILED (rc $rc): $(grep -E '^(bench|perf.sh): |rc=[1-9]' "$out/perfgate.log" | tail -3 | tr '\n' ' ')" | tee -a "$out/perfgate.log"; return 1; }
}

cmd_run() {
  need_store
  local out=${1:?OUT} new=${2:?NEW_BIN} bl=${3:-$STORE/current}
  [ -x "$new" ] || die "no binary $new"
  bl=$(readlink -f "$bl") || die "no baseline (seed one: perfgate.sh seed)"
  [ -d "$bl" ] || die "no baseline dir $bl (seed one: perfgate.sh seed OUT BIN)"
  echo "perfgate: baseline $bl ($(head -1 "$bl/meta.txt" 2>/dev/null))"
  if [ -x "$bl/game" ] && [ -z "${PERFGATE_JSON:-}" ]; then
    [ "$(md5sum < "$bl/game")" = "$(md5sum < "$new")" ] && echo "perfgate: note: NEW is the baseline binary itself (an A/A run)"
    local brc=0; bench_ab "$out" "$bl/game" "$new" || brc=1
    python3 "$STAT" gate "$out" --base base --new new --md "$out/perfgate.md" --json "$out/perfgate.json" || return 1
    return $brc
  else
    mkdir -p "$out"; echo "perfgate: stored-JSON mode (indicative)" | tee "$out/perfgate.log"
    # The comparison is indicative; a bench that did not run is not.
    "$PERF" bench "$out" new="$new" >> "$out/perfgate.log" 2>&1 || { echo "perfgate: BENCH FAILED (see $out/perfgate.log)"; return 1; }
    grep '^bench: SKIP ' "$out/perfgate.log" | sed 's/^bench: /perfgate: /' | sort -u
    python3 "$STAT" gate "$out" --baseline "$bl/summary.json" --new new --md "$out/perfgate.md" --json "$out/perfgate.json"
    local rc=$?; [ -n "${PERFGATE_JSON_FAIL:-}" ] && return $rc; return 0
  fi
}

cmd_accept() {
  need_store
  local out=${1:?OUT} note=${2:-accepted}
  local bin; bin=$(awk '$1 == "new" {print $2; exit}' "$out/bench.log") || die "no $out/bench.log"
  [ -x "$bin" ] || die "the run's new binary $bin is gone"
  grep -q '^bench: .* 0 failed' "$out/bench.log" || die "$out/bench.log does not end in a complete bench (bench: N runs, 0 failed): not accepted"
  local id="$(date +%Y%m%d-%H%M)-$(sha256sum < "$bin" | cut -c1-12)" dst
  dst="$STORE/$id"; mkdir -p "$dst" || die "cannot create $dst"
  # Everything first; "current" moves last and only when the baseline is whole.
  cp "$bin" "$dst/game" && [ -x "$dst/game" ] || die "copy of $bin to $dst/game failed"
  python3 "$STAT" summary "$out" --label new --md "$dst/summary.md" --json "$dst/summary.json" > /dev/null && [ -s "$dst/summary.json" ] || die "summary of $out failed"
  if [ -f "$out/perfgate.md" ]; then cp "$out/perfgate.md" "$dst/compare.md" || die "copy of perfgate.md failed"; fi
  { echo "$note"; echo "accepted $(date -Is) from $out"; echo "binary $bin sha256 $(sha256sum < "$bin" | cut -c1-64)"; grep '^set ' "$out/bench.log" | tail -1; } > "$dst/meta.txt" || die "meta.txt failed"
  ln -sfn "$id" "$STORE/current" || die "could not move $STORE/current"
  echo "perfgate: accepted $dst (current)"
}

cmd_seed() {
  need_store
  local out=${1:?OUT} new=${2:?NEW_BIN} note=${3:-seed}
  [ -x "$new" ] || die "no binary $new"
  bench_ab "$out" "$new" "$new" || die "seed: the A/A bench failed; nothing accepted"
  python3 "$STAT" gate "$out" --base base --new new --md "$out/perfgate.md" --json "$out/perfgate.json"
  echo "perfgate: (A/A: any FAIL above is a false positive of the rule)"
  cmd_accept "$out" "$note"
}

cmd=${1:-}; shift || true
case $cmd in
  run) cmd_run "$@" ;;
  accept) cmd_accept "$@" ;;
  seed) cmd_seed "$@" ;;
  *) sed -n '2,20p' "$0"; exit 2 ;;
esac
