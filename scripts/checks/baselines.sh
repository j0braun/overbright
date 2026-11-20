#!/bin/bash
# Output and load baselines of the engine (docs/developing/checks.md "Output baselines"): what today's engine draws,
# plays and simulates, stored as hashes, so a change of plumbing (the asset layer reading a base set instead of the
# game folder) can be proven to reproduce it bit for bit.
#
#   baselines.sh record  OUT BIN [NOTE]            run the output set with BIN, store it as a new baseline (with BIN,
#                                                  its load numbers and the frames' hashes); `current` -> it
#   baselines.sh check   OUT BIN [BASELINE_DIR]    run the same set with BIN, compare with the stored baseline
#                                                  (default `current`); every changed item named; exit 1 on any
#   baselines.sh rebless OUT BIN REASONS [NOTE]    an intended change: check, every changed item must match a line
#                                                  of REASONS ("<glob><TAB><why>"), then record; the mapping is
#                                                  written to the new baseline's changes.tsv and OUT/changes.tsv
#
# ASSETS=original (default) | base: where BIN reads the game from (NFSU2_ASSETS; base needs NFSU2_BASE_SET, and every
# run's log must say `assets: base`, so a binary without the asset layer cannot pass as one reading the base set).
# A baseline is recorded from the original files; `check` with ASSETS=base is the asset layer's proof.
# OUTPUTS (default "images shots audio sim"):
#   images  Original --bench explore/race/racing/drive/rain, every 100th of 600 frames, at IMG_SIZES (default
#           1280x960 3840x2160): the decoded pictures' hashes (pixdiff --hash)
#   shots   the gate's shots (scripts/checks/fast-shots.txt minus nondeterministic.txt; clock pinned at 0)
#   audio   the explore and race shots' WAVs (offline mixer; nothing is played)
#   sim     NFSU2_STATE_HASH over --flow firstrace, firstdrag, postrace, --playthrough and the parity scenario
#           (the last needs the private set): the hash files are kept, so a change names its first step and part
# LOAD=1 (default): record also benches BIN's loads (scripts/checks/loadgate.sh bench: warm, cold, 4 cores) and
# keeps the numbers beside the hashes; the load-speed gate itself runs A/B against the stored binary
# (loadgate.sh run OUT base=<baseline>/game new=BIN@SRC).
# Store: $ASSET_BASELINES (machine-local; scripts/local.env), else $PERF_BASELINES/asset-layer. Never in the repo:
# the pictures' hashes depend on the GPU and its driver (meta.txt names both).
# The parity runs' environment: RADV_DEBUG=nodcc as perf.sh parity (PARITY_RADV_DEBUG overrides).
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
. "$HERE/../lib/common.sh"
ck_local_env
die() { echo "baselines: $*" >&2; exit 2; }
STORE=${ASSET_BASELINES:-${PERF_BASELINES:+$PERF_BASELINES/asset-layer}}
ASSETS=${ASSETS:-original}
OUTPUTS=${OUTPUTS:-images shots audio sim}
export RADV_DEBUG=${PARITY_RADV_DEBUG-nodcc}

# side NAME ENV… -- ARGS…: one run of $BIN, its log in $O/logs/NAME.log, waited for by PID; status = the run's.
side() {
  local name=$1; shift
  local -a env=()
  while [ $# -gt 0 ] && [ "$1" != -- ]; do env+=("$1"); shift; done
  shift
  mkdir -p "$O/logs"
  local -a src=(NFSU2_ASSETS="$ASSETS"); [ "$ASSETS" = base ] && src+=(NFSU2_BASE_SET="$NFSU2_BASE_SET")
  env -u DISPLAY -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE XDG_DATA_HOME="$O/xdg-$name" NFSU2_CACHE_DIR="$O/cache" NFSU2_AUDIO_HOST=none \
    TMPDIR="$O/tmp" "${src[@]}" "${env[@]}" timeout 1800 "$BIN" "$@" > "$O/logs/$name.log" 2>&1 &
  local pid=$!; echo "$pid $name" >> "$O/pids.log"; wait $pid; local rc=$?
  if [ $rc = 0 ] && [ "$ASSETS" = base ] && ! grep -q '^assets: base' "$O/logs/$name.log"; then
    rc=98; echo "baselines: no 'assets: base' line: this binary does not read the base set" >> "$O/logs/$name.log"
  fi
  [ $rc != 0 ] || [ "$(ck_bad "$O/logs/$name.log")" = 0 ] || rc=97
  echo "$name rc=$rc" >> "$O/rc.txt"
  return $rc
}

# row KIND NAME HASH DETAIL: one line of the output manifest.
row() { printf '%s\t%s\t%s\t%s\n' "$1" "$2" "$3" "$4" >> "$O/outputs.tsv"; }
fail_row() { row "$1" "$2" "RUN-FAILED" "$3"; NFAIL=$((NFAIL + 1)); }

shots_list() {
  local nondet; nondet=" $(awk '!/^#/ && NF && $2 == "always" {print $1}' "$HERE/nondeterministic.txt" | tr '\n' ' ') "
  local s; for s in $(grep -v '^#' "$HERE/fast-shots.txt"); do case $nondet in *" $s "*) ;; *) echo "$s" ;; esac; done
}

circuit_save() {
  local f=tests/fixtures/native_saves/stage1_win_streetx.sav d
  for d in "${OB_PRIVATE:-}" "$REPO"; do [ -n "$d" ] && [ -f "$d/$f" ] && { echo "$d/$f"; return; }; done
}

# produce OUTDIR: run the output set with $BIN into OUTDIR (outputs.tsv, sim/*.tsv, frames/ when KEEP_FRAMES=1).
produce() {
  O=$1; NFAIL=0
  mkdir -p "$O/tmp" "$O/sim"; : > "$O/outputs.tsv"
  echo "baselines: $(date -Is) bin $BIN ($(sha256sum < "$BIN" | cut -c1-16)) assets $ASSETS outputs $OUTPUTS; loadavg $(cut -d' ' -f1-3 /proc/loadavg)" | tee "$O/produce.log"
  local g s size
  for g in $OUTPUTS; do case $g in
  images)
    for size in ${IMG_SIZES:-1280x960 3840x2160}; do for s in ${IMG_SCENES:-explore race racing drive rain}; do
      local sc=$s e=(); [ $s = rain ] && { sc=drive; e=(NFSU2_RAIN_CLOCK=162); }
      local n="img-$s-$size" d="$O/frames/img-$s-$size"; mkdir -p "$d"
      if side "$n" "${e[@]}" NFSU2_BENCH_SIZE=$size NFSU2_BENCH_DUMP="$d" NFSU2_BENCH_DUMP_EVERY=100 NFSU2_BENCH_GPU=0 -- --bench $sc 600 \
         && [ "$(find "$d" -name '*.png' | wc -l)" -gt 0 ]; then
        "$PIXDIFF" --hash "$d" | while read -r h dim f; do row image "$n/$(basename "$f")" "$h" "$dim"; done
      else fail_row image "$n" "see logs/$n.log"; fi
    done; done ;;
  shots)
    mkdir -p "$O/frames/shots"
    for s in $(shots_list); do
      local f="$O/frames/shots/$s.png"
      side "shot-$s" NFSU2_SHOT_CLOCK=0 -- --shot "$s" "$f"; local rc=$?
      if [ $rc = 3 ] && grep -q '^SKIP: ' "$O/logs/shot-$s.log" && ! ck_private_here "$REPO"; then row shot "$s" SKIP "needs the private set"
      elif [ $rc = 0 ] && [ -s "$f" ]; then local h dim; read -r h dim _ <<< "$("$PIXDIFF" --hash "$f")"; row shot "$s" "$h" "$dim"
      else fail_row shot "$s" "rc $rc, see logs/shot-$s.log"; fi
    done ;;
  audio)
    for s in explore race; do
      if side "wav-$s" NFSU2_AUDIO_WAV="$O/tmp/$s.wav" -- --shot $s "$O/tmp/wav-$s.png" && [ -s "$O/tmp/$s.wav" ]; then
        row audio "$s" "$(sha256sum < "$O/tmp/$s.wav" | cut -c1-32)" "$(stat -c %s "$O/tmp/$s.wav") bytes"
      else fail_row audio "$s" "see logs/wav-$s.log"; fi
    done ;;
  sim)
    local save f; save=$(circuit_save)
    for f in ${FLOWS-firstrace firstdrag postrace playthrough parity}; do
      local a=(--flow "$f") e=()
      [ "$f" = playthrough ] && a=(--playthrough)
      [ "$f" = postrace ] && { mkdir -p "$O/flowdata"; e=(NFSU2_FLOW_DATA="$O/flowdata"); }
      if [ "$f" = parity ]; then
        [ -n "$save" ] || { row sim parity SKIP "needs the private set"; continue; }
        sed -e "s#^save .*#save $save#" -e "s#^out .*#out $O/tmp/parity-circuit_s1.tsv#" "$REPO/scripts/perf/plans/circuit_s1.plan" > "$O/tmp/circuit_s1.plan"
        a=(--flow parity "$O/tmp/circuit_s1.plan")
      fi
      if side "flow-$f" NFSU2_STATE_HASH="$O/sim/$f.tsv" "${e[@]}" -- "${a[@]}" && [ -s "$O/sim/$f.tsv" ]; then
        row sim "$f" "$(sha256sum < "$O/sim/$f.tsv" | cut -c1-32)" "$(($(wc -l < "$O/sim/$f.tsv") - 1)) steps"
      else fail_row sim "$f" "see logs/flow-$f.log"; fi
    done ;;
  *) die "unknown output $g" ;;
  esac; done
  [ -n "${KEEP_FRAMES:-}" ] || rm -rf "$O/frames"
  rm -rf "$O/tmp" "$O"/xdg-* "$O/cache" "$O/flowdata"
  echo "baselines: $(wc -l < "$O/outputs.tsv") items, $NFAIL runs failed" | tee -a "$O/produce.log"
  [ $NFAIL = 0 ]
}

# compare OLD NEW OUT: every item that differs, is missing or is new -> OUT/diff.tsv (item, old, new, detail).
compare() {
  local old=$1 new=$2 out=$3
  python3 -I - "$old/outputs.tsv" "$new/outputs.tsv" "$out/diff.tsv" <<'PY'
import sys
def load(p):
    d = {}
    for line in open(p):
        k, n, h, det = line.rstrip("\n").split("\t", 3)
        d[f"{k}/{n}"] = (h, det)
    return d
a, b = load(sys.argv[1]), load(sys.argv[2])
rows = []
for item in sorted(set(a) | set(b)):
    ha, hb = a.get(item, ("-", "")), b.get(item, ("-", ""))
    if ha[0] != hb[0] or "RUN-FAILED" in (ha[0], hb[0]):
        rows.append((item, ha[0], hb[0], hb[1] or ha[1]))
with open(sys.argv[3], "w") as f:
    for r in rows:
        f.write("\t".join(r) + "\n")
print(f"{len(a)} stored, {len(b)} now, {len(rows)} differ")
PY
  # The simulation's changes named by step and part (hashdiff.py).
  local item
  while IFS=$'\t' read -r item _ _ _; do
    case $item in sim/*) local f=${item#sim/}
      [ -s "$old/sim/$f.tsv" ] && [ -s "$new/sim/$f.tsv" ] && echo "  $item: $(python3 -I "$REPO/scripts/perf/hashdiff.py" "$old/sim/$f.tsv" "$new/sim/$f.tsv" "$HERE/rate-columns.txt" | head -1)" ;;
    esac
  done < "$out/diff.tsv"
}

store_new() {  # RUN_DIR NOTE: the run's manifest, hashes and BIN as a new baseline; prints its dir
  local run=$1 note=$2
  [ -n "$STORE" ] || die "no store: set ASSET_BASELINES (or PERF_BASELINES) in scripts/local.env"
  local id; id="$(date +%Y%m%d-%H%M%S)-$(sha256sum < "$BIN" | cut -c1-12)"
  local dst="$STORE/$id"; mkdir -p "$dst" || die "cannot create $dst"
  cp "$run/outputs.tsv" "$dst/" && cp -r "$run/sim" "$dst/" && cp "$BIN" "$dst/game" && cp "$PIXDIFF" "$dst/pixdiff" || die "copy into $dst failed"
  [ -d "$run/load" ] && { cp "$run/load/summary.md" "$run/load/summary.json" "$dst/" 2> /dev/null || true; }
  local adapter; adapter=$(grep -hm1 -E '^ *(adapter|GPU)' "$run"/logs/img-*.log 2> /dev/null | head -1 | sed 's/^ *//')
  { echo "$note"; echo "recorded $(date -Is) assets $ASSETS"; echo "binary sha256 $(sha256sum < "$BIN" | cut -c1-64)"
    echo "source $(git -C "$REPO" rev-parse --short HEAD 2> /dev/null || echo unknown)$(git -C "$REPO" diff --quiet 2> /dev/null || echo ' (with local changes)')"
    echo "gpu ${adapter:-unknown}"; echo "outputs $OUTPUTS"; echo "flows ${FLOWS-firstrace firstdrag postrace playthrough parity}"
    echo "img_sizes ${IMG_SIZES:-1280x960 3840x2160}"; echo "img_scenes ${IMG_SCENES:-explore race racing drive rain}"; } > "$dst/meta.txt"
  ln -sfn "$id" "$STORE/current" || die "could not move $STORE/current"
  echo "$dst"
}

setup() {  # OUT BIN
  ck_need_root
  BIN=$(readlink -f "${2:?BIN}"); [ -x "$BIN" ] || die "no binary $2"
  PIXDIFF=${PIXDIFF:-$(dirname "$BIN")/pixdiff}; [ -x "$PIXDIFF" ] || die "no pixdiff at $PIXDIFF (perf.sh build puts one beside the game)"
  "$PIXDIFF" --hash /dev/null > /dev/null 2>&1; [ $? = 1 ] || die "$PIXDIFF has no --hash (an older build)"
  case $ASSETS in original) ;; base) [ -n "${NFSU2_BASE_SET:-}" ] && [ -d "$NFSU2_BASE_SET" ] || die "ASSETS=base needs NFSU2_BASE_SET" ;; *) die "ASSETS is original or base" ;; esac
  rm -rf "$1"; mkdir -p "$1"; OUTD=$(cd "$1" && pwd)
}

cmd=${1:-}; shift || true
case $cmd in
  record)
    setup "${1:?OUT}" "${2:?BIN}"; note=${3:-recorded}
    [ "$ASSETS" = original ] || die "a baseline is recorded from the original files (ASSETS=original)"
    produce "$OUTD/run" || die "runs failed: nothing stored (see $OUTD/run/outputs.tsv)"
    if [ "${LOAD:-1}" = 1 ]; then
      "$HERE/loadgate.sh" bench "$OUTD/load-runs" "today=$BIN@original" || die "the load bench failed: nothing stored"
      mkdir -p "$OUTD/run/load"
      python3 -I "$REPO/scripts/perf/loadstat.py" summary "$OUTD/load-runs" --md "$OUTD/run/load/summary.md" --json "$OUTD/run/load/summary.json" || die "load summary failed"
      rm -rf "$OUTD/load-runs/root" "$OUTD/load-runs/bin" "$OUTD"/load-runs/cache-* "$OUTD"/load-runs/mesa-* "$OUTD"/load-runs/xdg-*
    fi
    dst=$(store_new "$OUTD/run" "$note") || exit 2
    echo "BASELINES: recorded $dst ($(wc -l < "$dst/outputs.tsv") items)" ;;
  check|rebless)
    setup "${1:?OUT}" "${2:?BIN}"
    if [ "$cmd" = check ]; then bl=$(readlink -f "${3:-$STORE/current}"); else bl=$(readlink -f "$STORE/current"); reasons=${3:?REASONS}; note=${4:-rebless}; fi
    [ -f "$bl/outputs.tsv" ] || die "no stored baseline at ${3:-$STORE/current} (record one: baselines.sh record OUT BIN)"
    echo "baselines: against $bl ($(head -1 "$bl/meta.txt"))"
    # The set the baseline was recorded with.
    OUTPUTS=$(sed -n 's/^outputs //p' "$bl/meta.txt"); [ -n "$OUTPUTS" ] || OUTPUTS="images shots audio sim"
    grep -q '^flows ' "$bl/meta.txt" && FLOWS=$(sed -n 's/^flows //p' "$bl/meta.txt")
    grep -q '^img_sizes ' "$bl/meta.txt" && IMG_SIZES=$(sed -n 's/^img_sizes //p' "$bl/meta.txt")
    grep -q '^img_scenes ' "$bl/meta.txt" && IMG_SCENES=$(sed -n 's/^img_scenes //p' "$bl/meta.txt")
    produce "$OUTD/run"; prc=$?
    compare "$bl" "$OUTD/run" "$OUTD"
    nd=$(wc -l < "$OUTD/diff.tsv")
    if [ "$cmd" = check ]; then
      [ "$nd" = 0 ] || { echo "baselines: changed items (item, stored, now, detail):"; head -40 "$OUTD/diff.tsv" | sed 's/^/  /'; }
      echo "BASELINES: $([ "$nd" = 0 ] && [ $prc = 0 ] && echo "PASS: identical to the stored baseline" || echo "FAIL: $nd items differ$([ $prc = 0 ] || echo ", runs failed")") (assets $ASSETS; $OUTD/diff.tsv)"
      [ "$nd" = 0 ] && [ $prc = 0 ]
    else
      [ $prc = 0 ] || die "runs failed: nothing re-recorded"
      [ -f "$reasons" ] || die "no REASONS file $reasons"
      # Every changed item mapped to a reason (the first matching glob), else nothing is re-recorded.
      : > "$OUTD/changes.tsv"; unmapped=0
      while IFS=$'\t' read -r item old new det; do
        why=""
        while IFS=$'\t' read -r pat r; do case $pat in ''|'#'*) continue ;; esac
          # shellcheck disable=SC2053
          [[ $item == $pat ]] && { why=$r; break; }
        done < "$reasons"
        [ -n "$why" ] || { unmapped=$((unmapped + 1)); echo "  unmapped: $item"; }
        printf '%s\t%s\t%s\t%s\n' "$item" "$old" "$new" "${why:-UNMAPPED}" >> "$OUTD/changes.tsv"
      done < "$OUTD/diff.tsv"
      [ $unmapped = 0 ] || { echo "BASELINES: FAIL: $unmapped of $nd changed items have no reason in $reasons: nothing re-recorded"; exit 1; }
      dst=$(store_new "$OUTD/run" "$note (re-blessed from $(basename "$bl"))") || exit 2
      cp "$OUTD/changes.tsv" "$dst/changes.tsv"
      echo "BASELINES: re-recorded $dst: $nd changed items, each mapped ($OUTD/changes.tsv)"
    fi ;;
  *) sed -n '2,32p' "$0"; exit 2 ;;
esac
