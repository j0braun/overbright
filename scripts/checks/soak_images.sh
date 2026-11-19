#!/bin/bash
# Image soak (docs/developing/checks.md "Image soak"): the parity gate's Original image dumps (perf.sh parity, gate
# "images") run RUNS times with ONE binary, every run compared with run 1 frame by frame. Two runs of the
# same binary must give byte-identical frames; a difference is nondeterminism in the binary (or the driver),
# not a change between base and new.
#
#   soak_images.sh BIN OUTDIR [RUNS] [SCENES] [SIZES]
#
# BIN: a game binary built with --features dev-harness. RUNS: default 10. SCENES: default "drive race"
# (any of explore race racing drive rain, as the gate: rain = drive with NFSU2_RAIN_CLOCK=162). SIZES:
# default "1280x960 3840x2160". The gate's settings: --bench SCENE $FRAMES (600), a dump every $EVERY (100)
# frames, NFSU2_BENCH_GPU=0, RADV_DEBUG=nodcc (PARITY_RADV_DEBUG= keeps DCC, as perf.sh).
# Environment:
#   PAR=n      n runs at once (default 1)
#   LOAD=n     n `nice -n 19` busy loops of the script's own for the whole soak (CPU load), stopped by PID
#   IOLOAD=n   n `nice -n 19` write+fsync loops (1 GiB files under OUTDIR/ioload), stopped by PID
#   CACHE=shared|fresh   shared (default): one asset cache for every run, built by a short warm-up run per
#              scene first (as a gate side after its first run); fresh: every run starts with an empty cache
#              (each run builds the prepared cache itself, the gate side's first run)
#   KEEP=1     keep every run's dumps (default: a run identical to run 1 is deleted; differing frames are
#              always kept under OUTDIR/diffs)
#   PIXDIFF    the pixdiff binary (default: next to BIN)
# Output: OUTDIR/soak.txt (also on stdout): per scene/size the runs that differ from run 1, their frames and
# pixel counts, and per frame the number of distinct images; OUTDIR/diffs/<set>/rNN/: run 1's and run NN's
# frame, a difference image (ImageMagick `compare`, red = differing pixels) and the first differing pixels.
# Exit status: 0 every run identical, 1 a run differs, 2 a run failed or a usage error.
# Every process started here is waited for or stopped by its own PID; nothing is killed by name.
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/../lib/common.sh"
ck_local_env
BIN=$(readlink -f "${1:?usage: soak_images.sh BIN OUTDIR [RUNS] [SCENES] [SIZES]}")
OUT=${2:?usage: soak_images.sh BIN OUTDIR [RUNS] [SCENES] [SIZES]}
RUNS=${3:-10}
SCENES=${4:-drive race}
SIZES=${5:-1280x960 3840x2160}
[ -x "$BIN" ] || ck_die "no binary $BIN"
case $RUNS in '' | *[!0-9]*) ck_die "RUNS must be a number" ;; esac
[ "$RUNS" -ge 2 ] || ck_die "RUNS must be at least 2"
for sc in $SCENES; do case $sc in explore | race | racing | drive | rain) ;; *) ck_die "unknown scene $sc" ;; esac; done
ck_need_root
ck_need_tools md5sum
PIXDIFF=${PIXDIFF:-$(dirname "$BIN")/pixdiff}
[ -x "$PIXDIFF" ] || ck_die "no pixdiff at $PIXDIFF (cargo build --release --example pixdiff, or PIXDIFF=)"
HAVE_IM=0; command -v compare > /dev/null 2>&1 && command -v magick > /dev/null 2>&1 && HAVE_IM=1
PAR=${PAR:-1} LOAD=${LOAD:-0} IOLOAD=${IOLOAD:-0} CACHE=${CACHE:-shared}
FRAMES=${FRAMES:-600} EVERY=${EVERY:-100}
case $CACHE in shared | fresh) ;; *) ck_die "CACHE must be shared or fresh" ;; esac
export RADV_DEBUG=${PARITY_RADV_DEBUG-nodcc}
mkdir -p "$OUT" || exit 2
OUT=$(cd "$OUT" && pwd)
rm -rf "$OUT/runs" "$OUT/diffs" "$OUT/logs" "$OUT/xdg" "$OUT/cache" "$OUT/tmp" "$OUT/ioload"
mkdir -p "$OUT/runs" "$OUT/diffs" "$OUT/logs" "$OUT/xdg" "$OUT/tmp"
: > "$OUT/soak.txt"
say() { echo "$*" | tee -a "$OUT/soak.txt"; }

# ---- artificial load (own PIDs only) ----------------------------------------
LOAD_PIDS=()
stop_load() {
  local p
  for p in "${LOAD_PIDS[@]}"; do kill "$p" 2> /dev/null; done
  for p in "${LOAD_PIDS[@]}"; do wait "$p" 2> /dev/null; done
  LOAD_PIDS=()
  rm -rf "$OUT/ioload"
}
trap stop_load EXIT
trap 'exit 130' INT TERM
for ((k = 0; k < LOAD; k++)); do
  nice -n 19 bash -c 'while :; do :; done' &
  LOAD_PIDS+=($!)
done
if [ "$IOLOAD" -gt 0 ]; then
  mkdir -p "$OUT/ioload"
  for ((k = 0; k < IOLOAD; k++)); do
    # The loop's dd ends by itself (it is not killed); the loop stops at the next turn.
    nice -n 19 bash -c 'while :; do dd if=/dev/zero of="$1" bs=1M count=1024 conv=fsync status=none; done' _ "$OUT/ioload/f$k" &
    LOAD_PIDS+=($!)
  done
fi

say "soak $(date -Is): $BIN ($(md5sum < "$BIN" | cut -c1-16)) runs $RUNS scenes [$SCENES] sizes [$SIZES] frames $FRAMES every $EVERY; PAR=$PAR LOAD=$LOAD IOLOAD=$IOLOAD CACHE=$CACHE RADV_DEBUG=$RADV_DEBUG; loadavg $(cut -d' ' -f1-3 /proc/loadavg)"

# run NAME SCENE SIZE FRAMES DUMPDIR CACHEDIR: one bench run, its rc in OUT/logs/NAME.rc.
run() {
  local name=$1 sc=$2 size=$3 frames=$4 dump=$5 cache=$6 s=$2 e=()
  [ "$sc" = rain ] && { s=drive; e=(NFSU2_RAIN_CLOCK=162); }
  mkdir -p "$cache"
  env -u DISPLAY -u WAYLAND_DISPLAY XDG_DATA_HOME="$OUT/xdg/$name" NFSU2_CACHE_DIR="$cache" TMPDIR="$OUT/tmp" "${e[@]}" \
    NFSU2_BENCH_SIZE="$size" ${dump:+NFSU2_BENCH_DUMP="$dump"} NFSU2_BENCH_DUMP_EVERY="$EVERY" NFSU2_BENCH_GPU=0 \
    timeout 1800 "$BIN" --bench "$s" "$frames" > "$OUT/logs/$name.log" 2>&1
  local rc=$?
  [ "$(ck_bad "$OUT/logs/$name.log")" = 0 ] || [ $rc != 0 ] || rc=99
  echo $rc > "$OUT/logs/$name.rc"
  rm -rf "$OUT/xdg/$name"
}

if [ "$CACHE" = shared ]; then
  for sc in $SCENES; do
    run "warm-$sc" "$sc" 1280x960 2 "" "$OUT/cache"
    [ "$(cat "$OUT/logs/warm-$sc.rc")" = 0 ] || { say "warm-up run for $sc failed (rc $(cat "$OUT/logs/warm-$sc.rc"), $OUT/logs/warm-$sc.log)"; exit 2; }
  done
fi

# The jobs, run-major (every set's run 1 first), at most PAR at once, waited for by PID.
JOBS=()
for ((r = 1; r <= RUNS; r++)); do for size in $SIZES; do for sc in $SCENES; do JOBS+=("$sc $size $r"); done; done; done
declare -A RUNNING=()
reap() {
  local done_pid
  wait -n -p done_pid "${!RUNNING[@]}" 2> /dev/null
  [ -n "${done_pid:-}" ] && unset "RUNNING[$done_pid]"
}
started=$(date +%s)
for j in "${JOBS[@]}"; do
  read -r sc size r <<< "$j"
  while [ ${#RUNNING[@]} -ge "$PAR" ]; do reap; done
  set_name="$sc-$size"; rn=$(printf r%02d "$r")
  cache="$OUT/cache"; [ "$CACHE" = fresh ] && cache="$OUT/cache/$set_name-$rn"
  run "$set_name-$rn" "$sc" "$size" "$FRAMES" "$OUT/runs/$set_name/$rn" "$cache" &
  RUNNING[$!]=1
done
while [ ${#RUNNING[@]} -gt 0 ]; do reap; done
stop_load
[ "$CACHE" = fresh ] && rm -rf "$OUT/cache"
say "runs done in $(($(date +%s) - started)) s; loadavg $(cut -d' ' -f1-3 /proc/loadavg)"

# ---- comparison ----------------------------------------------------------------
FAILED=0 DIFFER=0
for size in $SIZES; do for sc in $SCENES; do
  set_name="$sc-$size"; ref="$OUT/runs/$set_name/r01"
  nfail=0 ndiff=0 detail=""
  for ((r = 1; r <= RUNS; r++)); do
    rn=$(printf r%02d "$r"); rc=$(cat "$OUT/logs/$set_name-$rn.rc" 2> /dev/null || echo "not run")
    [ "$rc" = 0 ] || { nfail=$((nfail + 1)); detail="$detail $rn:rc=$rc"; }
  done
  if [ "$(cat "$OUT/logs/$set_name-r01.rc" 2> /dev/null)" != 0 ] || ! ls "$ref"/*.png > /dev/null 2>&1; then
    say "$set_name: run 1 failed or dumped nothing ($OUT/logs/$set_name-r01.log); not compared"; FAILED=1; continue
  fi
  for ((r = 2; r <= RUNS; r++)); do
    rn=$(printf r%02d "$r"); dir="$OUT/runs/$set_name/$rn"
    [ "$(cat "$OUT/logs/$set_name-$rn.rc" 2> /dev/null)" = 0 ] || continue
    out=$("$PIXDIFF" "$ref" "$dir" 2>&1); prc=$?
    if [ $prc = 0 ]; then
      [ "${KEEP:-0}" = 1 ] || rm -rf "$dir"
      continue
    fi
    ndiff=$((ndiff + 1))
    frames=""
    while IFS= read -r line; do
      case $line in pixdiff:*) continue ;; esac
      f=${line%%:*}; frames="$frames ${f%.png}($(echo "$line" | sed -nE 's/.*: ([0-9]+) of [0-9]+ pixels differ.*max channel diff ([0-9]+).*/\1 px, max \2/p; s/.*: (MISSING.*|ERROR.*)/\1/p'))"
      d="$OUT/diffs/$set_name/$rn"; mkdir -p "$d"
      [ -f "$ref/$f" ] && cp "$ref/$f" "$d/${f%.png}.r01.png"
      [ -f "$dir/$f" ] && cp "$dir/$f" "$d/${f%.png}.$rn.png"
      if [ $HAVE_IM = 1 ] && [ -f "$ref/$f" ] && [ -f "$dir/$f" ]; then
        compare -metric AE -fuzz 0 -highlight-color red -lowlight-color black "$ref/$f" "$dir/$f" "$d/${f%.png}.diff.png" 2> /dev/null
        magick "$ref/$f" "$dir/$f" -alpha off -compose difference -composite txt:- 2> /dev/null \
          | awk 'NR > 1 && $0 !~ /#000000/' | head -100 > "$d/${f%.png}.pixels.txt"
      fi
    done <<< "$out"
    detail="$detail $rn:$frames"
  done
  # Distinct images per frame over every run (kept dumps and the differing frames).
  variants=""
  for p in "$ref"/*.png; do
    f=$(basename "$p")
    n=$( { md5sum < "$p"; for d in "$OUT/diffs/$set_name"/r*; do [ -f "$d/${f%.png}.$(basename "$d").png" ] && md5sum < "$d/${f%.png}.$(basename "$d").png"; done; } | sort -u | wc -l)
    [ "$n" -gt 1 ] && variants="$variants ${f%.png}=$n"
  done
  [ $nfail -gt 0 ] && FAILED=1
  [ $ndiff -gt 0 ] && DIFFER=1
  say "$set_name: $((RUNS - 1)) runs compared with run 1: $ndiff differ, $nfail failed${variants:+; distinct images:$variants}${detail:+;$detail}"
done; done
[ -d "$OUT/runs" ] && [ "${KEEP:-0}" != 1 ] && find "$OUT/runs" -mindepth 2 -maxdepth 2 -type d ! -name r01 -empty -delete 2> /dev/null
rm -rf "$OUT/tmp" "$OUT/xdg"
say "SOAK: $([ $FAILED = 1 ] && echo "FAIL (a run failed)" || { [ $DIFFER = 1 ] && echo "DIFFER (runs of one binary differ)" || echo "IDENTICAL"; }) $(date -Is)"
[ $FAILED = 1 ] && exit 2
[ $DIFFER = 1 ] && exit 1
exit 0
