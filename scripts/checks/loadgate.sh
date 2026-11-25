#!/bin/bash
# Load-speed gate (docs/developing/checks.md "Load-speed gate"): is a change's loading measurably slower, and where.
#
#   loadgate.sh run     OUT BASE=BIN[@SRC] NEW=BIN[@SRC]   bench both arms interleaved, then the gate; exit 1 when a
#                                                         load or a load stage is slower beyond its noise limit
#   loadgate.sh bench   OUT LABEL=BIN[@SRC] …            the runs only (resumable: a finished run is not repeated)
#   loadgate.sh stats   OUT --base A --new B             the gate on runs already made (scripts/perf/loadstat.py)
#   loadgate.sh summary OUT [LABEL]                      medians [min..max] and noise per mode, scene and stage
#   loadgate.sh selftest                                 the statistics on generated data
#
# BIN: a binary with the developer harness, best built with `perf.sh build` (the shipped release profile).
# @SRC: where that arm reads the game from: `original` (the game folder, NFSU2_ROOT; the default) or `base` (the
# imported base set, NFSU2_BASE_SET). Both arms may be one binary: `orig=BIN@original base=BIN@base`. Every run
# gets the asset layer's switch (NFSU2_BASE=off, or NFSU2_BASE=<the set's folder>); a `base` run counts only when its
# log has the layer's exit line (`asset layer: ... from base set`), an `original` run only without it, so a binary
# without the asset layer can never pass as one reading the base set.
# Modes (MODES, default "warm cold cores"):
#   warm   every file in the OS's file cache (the run before warmed it)
#   cold   before each run the arm's files are dropped from the OS's file cache without root
#          (scripts/perf/coldcache.py: fdatasync + posix_fadvise DONTNEED, residency checked with mincore): the
#          game data, the arm's asset cache and driver shader cache, the arm's binary. COLD_ROOT=copy (default)
#          makes a private copy of the game data in OUT first (fresh inodes nobody else reads or maps, so the drop
#          leaves nothing behind; ~1.7 GB, removed at the end); COLD_ROOT=inplace drops NFSU2_ROOT itself (pages
#          other programs map stay resident: each run's log says how much)
#   cores  warm, pinned to LOW_CPUS (default 0-3: four cores of a low-end machine; the game's pools follow it)
# Scenes (SCENES, default "boot explore"): --bench boot (process start to the first frame and the title) and
# --bench explore FRAMES (the first free-roam load and its first frame; FRAMES default 20); any other bench scene
# (race, racing, drive) the same way, its scenario's load and first frame; "transitions" adds
# --bench transitions. Every run writes NFSU2_TRACE: the load's stages are its spans.
# REPS (default 10, at least 8): runs per arm, mode and scene; the arms run adjacent, their order rotated each
# repetition. IGPU=1: the integrated GPU (IGPU_DEVICE). LOADGATE_LOCK=FILE: each repetition holds flock FILE
# (a machine's measuring lock). LOADGATE_MAXLOAD=N: each repetition waits (at most LOADGATE_MAXWAIT s, default 1800)
# for a 1-minute load average under N.
# NOISE_K (default 2): the limit's noise factor (loadstat.py).
# OUT: on the big disk; the arms' caches and the copy are removed at the end unless LOADGATE_KEEP=1.
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
. "$HERE/../lib/common.sh"
ck_local_env
STAT="$HERE/../perf/loadstat.py"; COLD="$HERE/../perf/coldcache.py"
die() { echo "loadgate: $*" >&2; exit 2; }
REPS=${REPS:-10}; FRAMES=${FRAMES:-20}; MODES=${MODES:-warm cold cores}; SCENES=${SCENES:-boot explore}
LOW_CPUS=${LOW_CPUS:-0-3}; COLD_ROOT=${COLD_ROOT:-copy}

# run_one OUT LABEL MODE SCENE REP: one run, waited for by PID; status 0 when it completed.
run_one() {
  local out=$1 label=$2 mode=$3 scene=$4 rep=$5
  local base="$out/runs/${label}__${mode}__${scene}__r$rep"
  local src=${SRCS[$label]} root=$ROOT
  local -a env=(XDG_DATA_HOME="$out/xdg-$label" NFSU2_CACHE_DIR="$out/cache-$label" MESA_SHADER_CACHE_DIR="$out/mesa-$label"
    NFSU2_AUDIO_HOST=none NFSU2_ROOT="$root" NFSU2_TRACE="$base.trace.json" NFSU2_BENCH_SIZE=1920x1080)
  # The asset layer's switch (src/asset_layer.rs): the base set by its folder, or off (never a set the caller's
  # environment or the arm's data folder would choose).
  if [ "$src" = base ]; then env+=(NFSU2_BASE="$BASESET"); else env+=(NFSU2_BASE=off); fi
  if [ -n "${IGPU:-}" ]; then
    [ -n "${IGPU_DEVICE:-}" ] || die "IGPU needs IGPU_DEVICE=vendor:device (scripts/local.env)"
    env+=(MESA_VK_DEVICE_SELECT="$IGPU_DEVICE" MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE=1)
  fi
  local -a args=(--bench "$scene") pre=()
  # Every scene but boot and transitions loads a scenario and writes its JSON (load_ms, first_frame_ms).
  case $scene in boot|transitions) ;; *) args+=("$FRAMES"); env+=(NFSU2_BENCH_JSON="$base.json") ;; esac
  [ "$mode" = cores ] && pre=(taskset -c "$LOW_CPUS")
  rm -f "$base.json" "$base.trace.json"
  { echo "bin: ${BINS[$label]} assets $src"; echo "config: $mode $scene rep $rep"; echo "loadavg: $(cat /proc/loadavg)"; } > "$base.txt"
  if [ "$mode" = cold ]; then
    local -a drop=("$root" "$out/cache-$label" "$out/mesa-$label" "${BINS[$label]}")
    [ "$src" = base ] && drop+=("$BASESET")
    echo "cold: $(python3 -I "$COLD" drop "${drop[@]}")" >> "$base.txt"
  fi
  env -u DISPLAY -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE "${env[@]}" "${pre[@]}" timeout 600 "${BINS[$label]}" "${args[@]}" >> "$base.txt" 2>&1 &
  local pid=$!; echo "$pid $label $mode $scene r$rep" >> "$out/pids.log"
  wait $pid; local rc=$?
  # The exit line of a run that read through the layer: a `base` run must have it, an `original` run must not.
  local layer=0; grep -q '^asset layer: .* from base set' "$base.txt" && layer=1
  if [ $rc = 0 ] && [ "$src" = base ] && [ $layer = 0 ]; then
    rc=98; echo "loadgate: no 'asset layer: ... from base set' line: this run did not read the base set" >> "$base.txt"
  elif [ $rc = 0 ] && [ "$src" = original ] && [ $layer = 1 ]; then
    rc=98; echo "loadgate: an 'asset layer: ... from base set' line in a run of the game folder" >> "$base.txt"
  fi
  if [ $rc = 0 ] && [ "$(ck_bad "$base.txt")" != 0 ]; then rc=97; fi
  echo "rc=$rc loadavg-end: $(cat /proc/loadavg)" >> "$base.txt"
  [ $rc = 0 ] || echo "  run $label $mode $scene r$rep: rc=$rc (see $base.txt)" >&2
  return $rc
}

declare -A BINS SRCS
LABELS=()
parse_arms() {
  local a
  for a in "$@"; do
    case $a in *=*) ;; *) die "arm $a is not LABEL=BIN[@SRC]" ;; esac
    local label=${a%%=*} rest=${a#*=} src=original
    case $rest in *@*) src=${rest##*@}; rest=${rest%@*} ;; esac
    [[ $label =~ ^[A-Za-z0-9.-]+$ ]] || die "label $label: letters, digits, dot and dash only"
    case $src in original|base) ;; *) die "arm $a: SRC is original or base" ;; esac
    [ -x "$rest" ] || die "no binary $rest"
    [ "$src" = base ] && { [ -n "${NFSU2_BASE_SET:-}" ] && [ -d "$NFSU2_BASE_SET" ] || die "arm $a reads the base set: NFSU2_BASE_SET must name it"; }
    LABELS+=("$label"); BINS[$label]=$(readlink -f "$rest"); SRCS[$label]=$src
  done
  [ ${#LABELS[@]} -ge 1 ] || die "no arms"
}

cmd_bench() {
  local out=${1:?OUT}; shift
  ck_need_root
  parse_arms "$@"
  [ "$REPS" -ge 8 ] || die "REPS $REPS: at least 8 (with fewer the one-sided test cannot reach its threshold)"
  mkdir -p "$out/runs" "$out/bin"; out=$(cd "$out" && pwd)
  local l
  # Each arm's binary as a private copy (its pages are the arm's own, for the cold drop).
  for l in "${LABELS[@]}"; do
    local dst="$out/bin/$l"
    if [ -f "$dst" ] && ! cmp -s "$dst" "${BINS[$l]}"; then die "$out holds another binary for label $l; use a fresh OUT"; fi
    cp "${BINS[$l]}" "$dst" && BINS[$l]=$dst || die "copy of the binary for $l failed"
  done
  ROOT=$NFSU2_ROOT; BASESET=${NFSU2_BASE_SET:-}
  if [[ " $MODES " == *" cold "* ]] && [ "$COLD_ROOT" = copy ]; then
    if [ ! -d "$out/root" ]; then
      echo "loadgate: private copy of the game data for the cold runs: $(python3 -I "$COLD" copy "$NFSU2_ROOT" "$out/root.tmp")" | tee -a "$out/loadgate.log"
      mv "$out/root.tmp" "$out/root" || die "copy of the game data failed"
    fi
    ROOT="$out/root"
    if [ -n "$BASESET" ]; then
      [ -d "$out/base-set" ] || { python3 -I "$COLD" copy "$BASESET" "$out/base-set.tmp" > /dev/null && mv "$out/base-set.tmp" "$out/base-set"; } || die "copy of the base set failed"
      BASESET="$out/base-set"
    fi
  fi
  { echo "bench $(date -Is) reps $REPS modes $MODES scenes $SCENES frames $FRAMES low cpus $LOW_CPUS cold root $COLD_ROOT${IGPU:+ igpu}; loadavg $(cut -d' ' -f1-3 /proc/loadavg)"
    for l in "${LABELS[@]}"; do echo "arm $l ${BINS[$l]} $(sha256sum < "${BINS[$l]}" | cut -c1-16) assets ${SRCS[$l]}"; done; } | tee -a "$out/loadgate.log"
  local nfail=0 n=${#LABELS[@]} s
  # Warm-up (not counted): each arm's asset cache and shader cache filled, the data read once.
  for l in "${LABELS[@]}"; do for s in $SCENES; do run_one "$out" "$l" warm "$s" 0 || nfail=$((nfail + 1)); done; done
  local rep mode k fd
  for rep in $(seq 1 "$REPS"); do
    # LOADGATE_MAXLOAD=N: before each repetition wait (up to LOADGATE_MAXWAIT s, default 1800) until the 1-minute load
    # average is under N.
    if [ -n "${LOADGATE_MAXLOAD:-}" ]; then
      local waited=0
      while [ $waited -lt "${LOADGATE_MAXWAIT:-1800}" ] && awk -v m="$LOADGATE_MAXLOAD" '{exit !($1 >= m)}' /proc/loadavg; do sleep 10; waited=$((waited + 10)); done
      [ $waited = 0 ] || echo "rep $rep waited $waited s for the load average under $LOADGATE_MAXLOAD" >> "$out/loadgate.log"
    fi
    if [ -n "${LOADGATE_LOCK:-}" ]; then exec {fd}> "$LOADGATE_LOCK" && flock "$fd"; fi
    for mode in $MODES; do for s in $SCENES; do
      for k in $(seq 0 $((n - 1))); do
        l=${LABELS[$(( (k + rep) % n ))]}
        local f="$out/runs/${l}__${mode}__${s}__r$rep"
        grep -q '^rc=0' "$f.txt" 2> /dev/null && continue
        run_one "$out" "$l" "$mode" "$s" "$rep" || nfail=$((nfail + 1))
      done
    done; done
    [ -n "${LOADGATE_LOCK:-}" ] && { flock -u "$fd"; exec {fd}>&-; }
    echo "rep $rep done $(date -Is) loadavg $(cut -d' ' -f1-3 /proc/loadavg)" >> "$out/loadgate.log"
  done
  echo "loadgate: bench done, $nfail runs failed $(date -Is)" | tee -a "$out/loadgate.log"
  [ $nfail = 0 ]
}

cleanup() {
  local out=$1
  [ -n "${LOADGATE_KEEP:-}" ] && return 0
  local l; for l in "${LABELS[@]}"; do rm -rf "$out/cache-$l" "$out/mesa-$l" "$out/xdg-$l"; done
  rm -rf "$out/root" "$out/base-set" "$out/bin"
}

cmd=${1:-}; shift || true
case $cmd in
  run)
    out=${1:?OUT}; shift; [ $# = 2 ] || die "run OUT BASE=BIN[@SRC] NEW=BIN[@SRC]"
    a=${1%%=*}; b=${2%%=*}
    cmd_bench "$out" "$@"; brc=$?
    out=$(cd "$out" && pwd)
    python3 -I "$STAT" gate "$out" --base "$a" --new "$b" --md "$out/loadgate.md" --json "$out/loadgate.json" | tee -a "$out/loadgate.log"; grc=${PIPESTATUS[0]}
    cleanup "$out"
    [ $brc = 0 ] && [ $grc = 0 ] ;;
  bench) cmd_bench "$@" ;;
  stats) out=${1:?OUT}; shift; python3 -I "$STAT" gate "$out" "$@" --md "$out/loadgate.md" --json "$out/loadgate.json" ;;
  summary) python3 -I "$STAT" summary "${1:?OUT}" ${2:+--label "$2"} --md "$1/summary.md" --json "$1/summary.json" && cat "$1/summary.md" ;;
  selftest) python3 -I "$STAT" selftest ;;
  *) sed -n '2,35p' "$0"; exit 2 ;;
esac
