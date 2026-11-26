#!/bin/bash
# Performance harness and parity gates (docs/developing/perf-gates.md).
#
#   perf.sh build  SRC OUT_BIN                 full release profile (thin LTO, cgu 1; never the fast-build env)
#   perf.sh bench  OUT LABEL=BIN[:nojson] …    the benchmark set, interleaved, NFSU2_BENCH_JSON per run
#   perf.sh stats  OUT --base A --new B        A/B statistics (exit 1 on a regression) -> OUT/compare.md
#   perf.sh stats  OUT --baseline FILE --new B against a stored baseline (indicative)
#   perf.sh summary OUT [LABEL]                per-config medians, noise, MDE -> OUT/summary.md, OUT/summary.json
#   perf.sh parity OUT BASE_BIN NEW_BIN        image, window-path, enhanced, shot, audio and simulation gates
#                                              (+ GATES=... rate: the cross-rate gate on NEW_BIN, see below)
#   perf.sh rate   OUT BIN                     the rate gate v2 alone: scripted flows and benches on display
#                                              schedules (RATES, default 30 60 120 200 Hz, + jitter), state hash
#                                              columns, α = 0 images and WAVs compared ACROSS rates (project rule:
#                                              gameplay and look identical at any rate); exit 0 / 1 / 3, see rate_gates
#   perf.sh profile OUT SRC SCENE [PRESET] [FRAMES]   samply CPU profile (line-tables build) + top functions
#
# Environment: SET=quick|standard|full|igpu|loads|d3|d4|d4ab|d6|d8|d11|gate (bench; default standard), CONFIGS (a list
# instead of SET), IGPU=1 (a second, integrated GPU: IGPU_DEVICE=vendor:device, e.g. in scripts/local.env), ORDER=alternate,
# PRESET_<name> (enh lines of a preset; PRESET_user = a play setup, machine-local: scripts/local.env),
# REPS (default 5), FRAMES (default 600), GATES (parity; default "images window enhanced shots audio sim"), SHOTS and
# SHOT_BASE_MAP (parity shots; "new=old …": the base draws a renamed target under its old name, set by gate.sh),
# PIXDIFF (the pixdiff binary; default: built next to the game binary), NFSU2_ROOT (required), PERF_WORK
# (target dirs; default REPO/target/perf-work), SAMPLY (default `samply` on PATH).
# Machine settings come from scripts/local.env (scripts/local.env.example). Exit status: non-zero when a step
# did not run or a gate failed (bench: any run with rc != 0 or without its JSON).
# Every process started here is waited for by PID; nothing is killed by name.
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
. "$HERE/../lib/common.sh"
ck_local_env
PERF_WORK=${PERF_WORK:-$REPO/target/perf-work}
REPS=${REPS:-5}
FRAMES=${FRAMES:-600}
MODERN="enh.hdr_post 1;enh.post_aa 5;enh.enhanced_paint 1;enh.enhanced_reflections 1;enh.road_reflection 1;enh.car_reflection 256;enh.ssao 50;enh.shadows 2;enh.lod_fade 1;enh.detail_distance 150;enh.wet_roads 1;enh.dynamic_lamps 1;enh.lamp_shadows 1;enh.contact_shadows 1"
ULTRA="$MODERN;enh.post_aa 6;enh.hdr_post 2;enh.road_reflection 2;enh.car_reflection 1024;enh.detail_distance 200;enh.car_detail 200;enh.keep_loaded 1;enh.ssr 1;enh.lamp_shadows 2;enh.volumetric_fog 2"
# The enhancements added after the first presets (not in the bench sets): Ultra plus particles, water, wet cars,
# rain detail/spray/lens drops, motion blur/DOF tiers, beams, probes, glass, Surface Detail, smooth cars, far draw;
# EXTREME_FSR the same with FSR 1 instead of TAA. Gate them with ENH_PRESETS="extreme extreme_fsr" GATES=enhanced.
EXTREME="$ULTRA;enh.particles 2;enh.water 1;enh.wet_car 1;enh.rain_detail 2;enh.spray 1;enh.lens_drops 1;enh.wet_interaction 2;enh.motion_blur 2;enh.depth_of_field 1;enh.headlight_beams 1;enh.light_probes 1;enh.enhanced_glass 1;enh.surface_detail 2;enh.smooth_cars 1;enh.far_draw 1"
EXTREME_FSR="$EXTREME;enh.post_aa 5;enh.upscale 5"
# docs/notes/performance/budget.md: ULTRA26 = Preset::Ultra (qol.rs) as of the first budget table (the older ULTRA above is kept
# for the stored numbers); USER = a play setup (enh.* graphics lines and World Detail custom values), machine-local:
# $PRESET_user (scripts/local.env; the presets user and max need it); MAX = USER plus every newer row at its top
# tier (MAX_ROWS: Modern Lighting + RT, Cinematic smoke, Realistic water, Wet Road High, Rain Detail High, the
# weather, Per-Object motion blur, Cinematic DOF, beams, probes, glass, Surface Detail Materials + Normals,
# smooth cars, World Detail Extreme, Dynamic Lamps). Any other preset name P: the enh lines in $PRESET_P
# (@MAX@ in them = MAX's lines; scripts/perf/budget.sh's leave-one-out presets).
ULTRA26="$MODERN;enh.wet_car 1;enh.rain_detail 1;enh.spray 1;enh.headlight_beams 1;enh.light_probes 1;enh.post_aa 6;enh.hdr_post 2;enh.road_reflection 2;enh.car_reflection 1024;enh.detail_distance 200;enh.car_detail 200;enh.keep_loaded 1;enh.ssr 1;enh.lamp_shadows 2;enh.volumetric_fog 2;enh.particles 2;enh.water 1;enh.rain_detail 2;enh.wet_interaction 1;enh.enhanced_glass 1"
PLAY=${PRESET_user:-}
MAX_ROWS="enh.modern_lighting 3;enh.particles 3;enh.water 2;enh.wet_interaction 2;enh.rain_detail 2;enh.wet_car 1;enh.spray 1;enh.lens_drops 1;enh.motion_blur 2;enh.depth_of_field 1;enh.headlight_beams 1;enh.light_probes 1;enh.enhanced_glass 1;enh.surface_detail 2;enh.smooth_cars 1;enh.far_draw 1;enh.dynamic_lamps 1"
MAX=""; [ -n "$PLAY" ] && MAX="$PLAY;$MAX_ROWS"

die() { echo "perf.sh: $*" >&2; exit 2; }

# preset_enh NAME: the enh lines of a preset (empty for orig); dies on an unknown name or a missing
# machine-local preset. One table for bench, profile and the enhanced gate.
preset_enh() {
  case $1 in
    orig) echo "" ;; modern) echo "$MODERN" ;; ultra) echo "$ULTRA" ;; extreme) echo "$EXTREME" ;; extreme_fsr) echo "$EXTREME_FSR" ;;
    ultra26) echo "$ULTRA26" ;;
    user|max) [ -n "$PLAY" ] || die "preset $1 needs PRESET_user (the play setup's enh lines; scripts/local.env)"
      [ "$1" = user ] && echo "$PLAY" || echo "$MAX" ;;
    *) local pv="PRESET_$1"; [ -n "${!pv:-}" ] || die "unknown preset $1 (no \$PRESET_$1)"; local pl=${!pv}
       case $pl in *@MAX@*) [ -n "$MAX" ] || die "preset $1 uses @MAX@, which needs PRESET_user (scripts/local.env)" ;; esac
       echo "${pl//@MAX@/$MAX}" ;;
  esac
}

# The benchmark set: configs "scene-preset-WxH" (scene rain = drive at clock 162 s, rain 0.70).
configs() {
  # CONFIGS="cfg cfg …" overrides the set (any scene-preset-WxH, see run_one).
  [ -n "${CONFIGS:-}" ] && { echo $CONFIGS; return; }
  case "${SET:-standard}" in
    # The regression gate (scripts/checks/perfgate.sh): 8 configs, the Original path and the heavy ones.
    # Without PRESET_user (a machine-local play setup, scripts/local.env; a public checkout has none) the two "max"
    # configs cannot run: they are left out and named on a "skip" line (gate_skipped), never passed silently.
    gate) if [ -n "$PLAY" ]; then echo racing-orig-1920x1080 explore-orig-1920x1080 race-orig-1920x1080 racing-orig-3840x2160 racing-ultra-3840x2160 explore-extreme-1920x1080 rain-max-3840x2160 race-max-1920x1080
          else echo racing-orig-1920x1080 explore-orig-1920x1080 race-orig-1920x1080 racing-orig-3840x2160 racing-ultra-3840x2160 explore-extreme-1920x1080; fi ;;
    quick) echo racing-orig-1920x1080 racing-ultra-1920x1080 explore-orig-1920x1080 race-orig-1920x1080 racing-orig-3840x2160 ;;
    standard)
      for s in explore racing race drive rain; do for p in orig modern ultra; do echo $s-$p-1920x1080; done; done
      for s in explore racing; do for p in orig ultra; do echo $s-$p-3840x2160; done; done ;;
    full) for r in 1920x1080 3840x2160; do for s in explore racing race drive rain; do for p in orig modern ultra; do echo $s-$p-$r; done; done; done ;;
    igpu) for s in explore racing race drive; do echo $s-orig-1920x1080; done; echo racing-modern-1920x1080 ;;
    # The CPU unit's set, with the Extreme preset (every enhancement) and the race start scene.
    d3) for s in race racing explore; do for p in orig extreme; do echo $s-$p-1920x1080; done; done ;;
    # The GPU unit's profile set (explore, racing, rain, race start × Original, Ultra, Extreme × 1080p, 4K).
    d4) for r in 1920x1080 3840x2160; do for s in explore racing rain race; do for p in orig ultra extreme; do echo $s-$p-$r; done; done; done ;;
    # The GPU-encoding work's A/B set: the encoding / main-thread steps (1080p) and the GPU steps (4K).
    d4ab) for s in racing explore race; do for p in orig extreme; do echo $s-$p-1920x1080; done; done; echo racing-ultra-3840x2160 racing-extreme-3840x2160 ;;
    # The second audit round's A/B set: the smoke (explore Extreme), the fog (Ultra / Extreme 4K), the cars (race start, racing Original).
    d6) echo explore-extreme-1920x1080 explore-extreme-3840x2160 racing-ultra-3840x2160 racing-extreme-3840x2160 rain-ultra-3840x2160 race-extreme-3840x2160 race-orig-1920x1080 race-orig-3840x2160 racing-orig-1920x1080 ;;
    # The depth pre-pass work's A/B set: the main view's depth pre-pass and the lamps' specialisation (4K Ultra / Extreme every scene,
    # Ultra 1080p, Modern 4K (MSAA 8x), Original 1080p as the control).
    d8) for s in racing explore rain race; do for p in ultra extreme; do echo $s-$p-3840x2160; done; done; echo racing-ultra-1920x1080 racing-modern-3840x2160 racing-orig-1920x1080 ;;
    # The SSR / motion-blur work's A/B set: SSR / motion blur / submissions (4K Ultra / Extreme / the play setup every scene,
    # racing user / Ultra 1080p, Original 1080p as the control).
    d11) for s in racing explore rain race; do for p in ultra extreme user; do echo $s-$p-3840x2160; done; done; echo racing-user-1920x1080 racing-ultra-1920x1080 racing-orig-1920x1080 ;;
    loads) echo transitions ;;
    *) die "unknown SET ${SET}" ;;
  esac
}

# gate_skipped: the configs of the set left out here and why, one "CONFIG: REASON" per line (empty: none).
gate_skipped() {
  [ -z "${CONFIGS:-}" ] && [ "${SET:-standard}" = gate ] && [ -z "$PLAY" ] || return 0
  local c; for c in rain-max-3840x2160 race-max-1920x1080; do echo "$c: needs PRESET_user (a play setup, scripts/local.env)"; done
}

# run_one OUT LABEL BIN CONFIG REP [FRAMES]: one bench run, waited for by PID.
run_one() {
  local out=$1 label=$2 bin=$3 cfg=$4 rep=$5 frames=${6:-$FRAMES}
  local base="$out/runs/${label}__${cfg}__r$rep"
  local -a env=(XDG_DATA_HOME="$out/xdg-$label" NFSU2_CACHE_DIR="$out/cache-$label")
  local json=1; [ "${bin##*:}" = nojson ] && { json=0; bin=${bin%:nojson}; }
  if [ "${SET:-}" = igpu ] || [ -n "${IGPU:-}" ]; then
    [ -n "${IGPU_DEVICE:-}" ] || die "IGPU needs IGPU_DEVICE=vendor:device (MESA_VK_DEVICE_SELECT; scripts/local.env)"
    env+=(MESA_VK_DEVICE_SELECT="$IGPU_DEVICE" MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE=1)
  fi
  local scene=$cfg
  if [ "$cfg" != transitions ]; then
    IFS=- read -r scene preset size <<< "$cfg"
    env+=(NFSU2_BENCH_SIZE="$size"); [ $json = 1 ] && env+=(NFSU2_BENCH_JSON="$base.json")
    [ -n "$size" ] || die "config $cfg is not scene-preset-WxH"
    local enh; enh=$(preset_enh "$preset") || exit 2
    [ "$preset" = orig ] || env+=(NFSU2_BENCH_ENH="$enh")
    [ "$scene" = rain ] && { scene=drive; env+=(NFSU2_RAIN_CLOCK=162); }
    # Scene water = explore with the camera orbiting the Beacon Hill fountain steps (docs/notes/graphics/water.md's
    # worst case N, the maintainer's private capture).
    if [ "$scene" = water ]; then
      scene=explore
      [ -s "$out/track_water.txt" ] || python3 "$REPO/docs/evidence/water/scripts/orbit.py" -827 714 36 20 6 > "$out/track_water.txt"
      env+=(NFSU2_BENCH_CAMTRACK="$out/track_water.txt")
    fi
  else
    env+=(NFSU2_BENCH_SIZE=1920x1080)
  fi
  { echo "bin: $bin"; echo "config: $cfg rep $rep"; echo "loadavg: $(cat /proc/loadavg)"; } > "$base.txt"
  env "${env[@]}" timeout 900 "$bin" --bench "$scene" "$frames" >> "$base.txt" 2>&1 &
  local pid=$!
  echo "$pid $label $cfg r$rep" >> "$out/pids.log"
  wait $pid; local rc=$?
  echo "rc=$rc loadavg-end: $(cat /proc/loadavg)" >> "$base.txt"
  # A run counts only with rc 0 and (JSON arms) a JSON that parses.
  if [ $rc = 0 ] && [ $json = 1 ] && ! json_ok "$base.json"; then rc=99; echo "rc=$rc: no valid JSON $base.json" >> "$base.txt"; fi
  [ $rc = 0 ] || echo "  run $label $cfg r$rep: rc=$rc (see $base.txt)" >&2
  return $rc
}

json_ok() { [ -s "$1" ] && python3 -I -c 'import json, sys; json.load(open(sys.argv[1]))' "$1" 2> /dev/null; }

# harness_features SRC: `--features dev-harness` when SRC's Cargo.toml has the feature (the developer
# harness, off by default; every perf.sh run drives the game through it), else nothing.
harness_features() { grep -q '^dev-harness *=' "$1/Cargo.toml" 2> /dev/null && echo "--features dev-harness"; }

cmd_build() {
  local src=${1:?SRC} dst=${2:?OUT_BIN}
  # One target dir per source tree: cargo's freshness check compares mtimes against the dep-info of the
  # last build in the dir, so a second tree with older mtimes (rsync -a, git archive) would silently get
  # the first tree's binary.
  local tgt="$PERF_WORK/target-full-$(realpath "$src" | sha256sum | cut -c1-12)"
  # The measured profile: Cargo.toml's [profile.release] untouched.
  ( cd "$src" && env -u CARGO_PROFILE_RELEASE_LTO -u CARGO_PROFILE_RELEASE_CODEGEN_UNITS -u CARGO_PROFILE_RELEASE_INCREMENTAL \
      -u CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS -u RUSTC_WRAPPER CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-6} \
      CARGO_TARGET_DIR="$tgt" cargo build --release --bin game --example pixdiff $(harness_features "$src") ) || die "build failed"
  mkdir -p "$(dirname "$dst")"
  cp "$tgt/release/game" "$dst" || die "copy of the game binary to $dst failed"
  cp "$tgt/release/examples/pixdiff" "$(dirname "$dst")/pixdiff" || die "copy of pixdiff failed"
  sha256sum "$dst"
}

cmd_bench() {
  local out=${1:?OUT}; shift
  ck_need_root
  [ $# -ge 1 ] || die "bench OUT LABEL=BIN …"
  mkdir -p "$out/runs"
  local labels=() bins=()
  # LABEL=BIN[:nojson]: ":nojson" runs that arm without NFSU2_BENCH_JSON (its text log is parsed;
  # for binaries from before the JSON output existed, or to measure the JSON's own cost).
  for a in "$@"; do local b=${a#*=} suf=""; [ "${b##*:}" = nojson ] && { suf=":nojson"; b=${b%:nojson}; }
    [ -x "$b" ] || die "bench: no binary $b ($a)"
    labels+=("${a%%=*}"); bins+=("$(readlink -f "$b")$suf"); done
  local cfgs; cfgs=$(configs) || exit 2
  [ -n "$cfgs" ] || die "bench: no configs"
  # Resume only with the same binary per label (an earlier binary's runs would be reported as this one's).
  local i
  for i in "${!labels[@]}"; do
    local sha; sha=$(sha256sum < "${bins[$i]%:nojson}" | cut -c1-64)
    local mark="$out/runs/.bin-${labels[$i]}"
    if [ -s "$mark" ] && [ "$(cat "$mark")" != "$sha" ]; then die "bench: $out already holds runs of label ${labels[$i]} from another binary ($(cut -c1-16 "$mark")); use a fresh OUT"; fi
    echo "$sha" > "$mark"
  done
  { echo "set $([ -n "${CONFIGS:-}" ] && echo "CONFIGS ($(echo $CONFIGS | wc -w) configs)" || echo "${SET:-standard}") reps $REPS frames $FRAMES started $(date -Is)"; for i in "${!labels[@]}"; do echo "${labels[$i]} ${bins[$i]} $(sha256sum < "${bins[$i]%:nojson}" | cut -c1-16)"; done; } >> "$out/bench.log"
  echo "expect reps $REPS labels ${labels[*]} configs $(echo $cfgs)" >> "$out/bench.log"
  # Configs of the set that cannot run here: "skip CONFIG: REASON" lines (perfstat.py gate names them in its verdict).
  local sk; sk=$(gate_skipped)
  [ -z "$sk" ] || { sed 's/^/skip /' <<< "$sk" >> "$out/bench.log"; sed 's/^/bench: SKIP /' <<< "$sk" >&2; }
  local nfail=0 nrun=0
  # Warm each binary's own asset cache (not recorded).
  for i in "${!labels[@]}"; do
    for s in explore racing race drive; do run_one "$out" "${labels[$i]}" "${bins[$i]}" "$s-orig-1280x960" 0 30 || nfail=$((nfail + 1)); done
    rm -f "$out"/runs/"${labels[$i]}"__*__r0.*
  done
  local n=${#labels[@]}
  for rep in $(seq 1 "$REPS"); do
    # ORDER=alternate runs the configs backwards on even repetitions (a slow drift of the machine
    # then hits a config's neighbours alike; for sets compared config against config, docs/notes/performance/budget.md).
    local order=$cfgs; [ "${ORDER:-}" = alternate ] && [ $((rep % 2)) = 0 ] && order=$(echo $cfgs | tr ' ' '\n' | tac)
    for cfg in $order; do
      # Arms adjacent per config, their order rotated each repetition.
      for k in $(seq 0 $((n - 1))); do
        local i=$(( (k + rep) % n ))
        local f="$out/runs/${labels[$i]}__${cfg}__r$rep"
        if [ "${bins[$i]##*:}" = nojson ]; then grep -q '^rc=0' "$f.txt" 2> /dev/null && continue
        else json_ok "$f.json" && grep -q '^rc=0' "$f.txt" 2> /dev/null && continue; fi
        nrun=$((nrun + 1))
        run_one "$out" "${labels[$i]}" "${bins[$i]}" "$cfg" "$rep" || nfail=$((nfail + 1))
      done
    done
    echo "rep $rep done $(date -Is) loadavg $(cut -d' ' -f1-3 /proc/loadavg)" >> "$out/bench.log"
  done
  echo "bench: $nrun runs, $nfail failed $(date -Is)" | tee -a "$out/bench.log" >&2
  [ $nfail = 0 ]
}

cmd_stats() { local out=${1:?OUT}; shift; python3 "$HERE/perfstat.py" compare "$out" "$@" --md "$out/compare.md" --json "$out/compare.json"; }
cmd_summary() { local out=${1:?OUT}; python3 "$HERE/perfstat.py" summary "$out" ${2:+--label $2} --md "$out/summary.md" --json "$out/summary.json"; python3 "$HERE/perfstat.py" top "$out" ${2:+--label $2} > "$out/top.md"; tail -3 "$out/summary.md"; }

# ---- parity gates ------------------------------------------------------------
# Every gate's verdict is explicit: a run that failed (rc != 0), a missing output (no dump set, no PNG,
# no WAV, no hash table), a panic / validation line or a difference makes its gate line say FAIL and
# counts in GFAIL; "nothing compared" is a failure too. PARITY passes only with GFAIL = 0.

declare -A SIDE_RC     # "label:name" -> rc of that run
RUN_NAMES=()           # every name run with `both`, in order

# side OUT LABEL BIN NAME ENV… -- ARGS…: run BIN with ENV and ARGS, log in OUT/LABEL/NAME.log, waited for by PID.
side() {
  local out=$1 label=$2 bin=$3 name=$4; shift 4
  local -a env=()
  # @SIDE@ in a value: the side's label (each side dumps into its own dir).
  while [ $# -gt 0 ] && [ "$1" != -- ]; do env+=("${1//@SIDE@/$label}"); shift; done
  [ $# -gt 0 ] || die "side $name: no -- before the arguments"
  shift
  mkdir -p "$out/$label"
  env XDG_DATA_HOME="$out/$label/xdg-$name" NFSU2_CACHE_DIR="${SIDE_CACHE:-$out/cache-$label}" "${env[@]}" timeout 1800 "$bin" "$@" > "$out/$label/$name.log" 2>&1 &
  local pid=$!; echo "$pid $label $name" >> "$out/pids.log"; wait $pid; local rc=$?
  echo "$name rc=$rc" >> "$out/$label/rc.txt"
  SIDE_RC["$label:$name"]=$rc
  return $rc
}

# both NAME ENV… -- ARGS…: the same run with BASE and NEW, one after the other.
both() { RUN_NAMES+=("$1"); side "$OUT" base "$BASE" "$@"; side "$OUT" new "$NEW" "$@"; return 0; }

# private_skip NAME: true when both sides of NAME exited 3 with a "SKIP: ... private set" line (src/bin/game/dev/private.rs)
# and the private set is really not here (no saves folder in $OB_PRIVATE, else in REPO: a public checkout). With the
# set here such a skip stays a failed run.
private_skip() {
  [ -d "${OB_PRIVATE:-$REPO}/tests/fixtures/native_saves" ] && return 1
  local l; for l in base new; do
    [ "${SIDE_RC["$l:$1"]:-}" = 3 ] && grep -q '^SKIP: ' "$OUT/$l/$1.log" || return 1
  done
}
# The circuit plan's start save (private): REPO's, else the private set's; empty when neither has it.
circuit_save() {
  local f=tests/fixtures/native_saves/stage1_win_streetx.sav d
  for d in "$REPO" "${OB_PRIVATE:-}"; do [ -n "$d" ] && [ -f "$d/$f" ] && { echo "$d/$f"; return; }; done
}

# rcs NAME: "" when both sides ran clean (rc 0, no panic/validation line), else what went wrong.
rcs() {
  local n=$1 r="" l
  for l in base new; do
    local rc=${SIDE_RC["$l:$n"]:-not run}
    [ "$rc" = 0 ] || r="$r $l rc=$rc"
    local b; b=$(ck_bad "$OUT/$l/$n.log"); [ "$b" = 0 ] || r="$r $l panic/validation=$b"
  done
  echo "${r# }"
}

GFAIL=0; FAILED_GATES=()
gate() { echo "$*" | tee -a "$OUT/gates.txt"; }
# verdict NAME OK LINE: the gate's line; OK != 0 counts it as failed (the line gets "FAIL: " in front).
verdict() {
  if [ "$2" = 0 ]; then gate "$3"; else GFAIL=$((GFAIL + 1)); FAILED_GATES+=("$1"); gate "FAIL: $3"; fi
}

# base_shot NAME: the name the base binary draws NAME under (SHOT_BASE_MAP "new=old …", set by gate.sh for targets
# renamed since the base, scripts/checks/shot-renames.txt); NAME itself otherwise. The images keep NAME on both sides.
base_shot() { local m; for m in ${SHOT_BASE_MAP:-}; do [ "${m%%=*}" = "$1" ] && { echo "${m#*=}"; return; }; done; echo "$1"; }
pix() { "$PIXDIFF" "$@" > "$OUT/pixdiff.tmp" 2>&1; local rc=$?; cat "$OUT/pixdiff.tmp" >> "$OUT/pixdiff.log"; tail -1 "$OUT/pixdiff.tmp"; return $rc; }

npng() { [ -d "$1" ] && find "$1" -maxdepth 1 -name '*.png' | wc -l || echo 0; }

# pix_dirs PREFIX [limits]: every dump set PREFIX-* that was run (RUN_NAMES) or exists on either side.
# A set is bad when either side's run failed, either side has no PNG in it, or pixdiff fails (a frame
# missing on one side, a size change, over the limits). Exit status 1 when any set is bad or none was compared.
pix_dirs() {
  local pre=$1; shift
  local total=0 bad=0 n names
  names=$( { for n in "${RUN_NAMES[@]}"; do case $n in "$pre"-*) echo "$n" ;; esac; done
             for n in "$OUT"/base/dumps/"$pre"-* "$OUT"/new/dumps/"$pre"-*; do [ -d "$n" ] && basename "$n"; done; } | sort -u)
  for n in $names; do
    total=$((total + 1))
    local b="$OUT/base/dumps/$n" w="$OUT/new/dumps/$n" why; why=$(rcs "$n")
    local nb nw; nb=$(npng "$b"); nw=$(npng "$w")
    if [ "$nb" = 0 ] || [ "$nw" = 0 ]; then
      bad=$((bad + 1)); echo "  $n: MISSING (base $nb frames, new $nw frames)${why:+; $why}" >> "$OUT/gates.txt"; continue
    fi
    local line; line=$(pix "$b" "$w" "$@"); local rc=$?
    if [ $rc != 0 ] || [ -n "$why" ]; then bad=$((bad + 1)); fi
    echo "  $n: $line${why:+; RUN FAILED: $why}" >> "$OUT/gates.txt"
  done
  echo "$total dump sets, $bad failed (missing, run failed or over the limits)"
  [ $total -gt 0 ] && [ $bad = 0 ]
}

cmd_parity() {
  OUT=${1:?OUT}; BASE=$(readlink -f "${2:?BASE_BIN}"); NEW=$(readlink -f "${3:?NEW_BIN}")
  [ -x "$BASE" ] || die "no base binary $2"; [ -x "$NEW" ] || die "no new binary $3"
  ck_need_root
  # RADV's delta colour compression on the multisampled scene changes one pixel of a frame by up to 3
  # in ~2 % of frames, base vs base (never with DCC off, which gives the majority image every time;
  # docs/developing/perf-gates.md). The gates compare with it off; the benches keep the driver's default.
  # PARITY_RADV_DEBUG= (empty) keeps DCC. Only Mesa's RADV reads the variable.
  export RADV_DEBUG=${PARITY_RADV_DEBUG-nodcc}
  PIXDIFF=${PIXDIFF:-$(dirname "$NEW")/pixdiff}
  [ -x "$PIXDIFF" ] || die "no pixdiff at $PIXDIFF (cargo build --release --example pixdiff)"
  mkdir -p "$OUT"; : > "$OUT/gates.txt"
  local gates=${GATES:-images window enhanced shots audio sim}
  local every=${DUMP_EVERY:-100} frames=${PARITY_FRAMES:-600} line ok
  local g; for g in $gates; do case $g in images|window|enhanced|shots|audio|sim|rate) ;; *) die "unknown gate $g" ;; esac; done
  gate "parity $(date -Is): base $BASE ($(sha256sum < "$BASE" | cut -c1-16)) new $NEW ($(sha256sum < "$NEW" | cut -c1-16)); loadavg $(cut -d' ' -f1-3 /proc/loadavg); RADV_DEBUG=${RADV_DEBUG:-}; gates $gates"
  local rate_rc=0
  for g in $gates; do case $g in
  images)
    # Original, byte-identical: the bench scenes' frames every $every at two sizes.
    for size in ${IMG_SIZES:-1280x960 3840x2160}; do for sc in ${IMG_SCENES:-explore race racing drive rain}; do
      local s=$sc e=(); [ $sc = rain ] && { s=drive; e=(NFSU2_RAIN_CLOCK=162); }
      both "img-$sc-$size" "${e[@]}" NFSU2_BENCH_SIZE=$size NFSU2_BENCH_DUMP="$OUT/@SIDE@/dumps/img-$sc-$size" NFSU2_BENCH_DUMP_EVERY=$every NFSU2_BENCH_GPU=0 -- --bench $s $frames
    done; done
    line=$(pix_dirs img); ok=$?
    verdict images $ok "images (Original, AE 0): $line" ;;
  window)
    # The window path's frames: the world at its 60 fps cadence, drawn at 144 Hz, interpolated (draw_between).
    both win-racing NFSU2_BENCH_HZ=144 NFSU2_BENCH_SIZE=1920x1080 NFSU2_BENCH_DUMP="$OUT/@SIDE@/dumps/win-racing" NFSU2_BENCH_DUMP_EVERY=37 NFSU2_BENCH_GPU=0 -- --bench racing 720
    both win-explore NFSU2_BENCH_HZ=144 NFSU2_BENCH_SIZE=1920x1080 NFSU2_BENCH_DUMP="$OUT/@SIDE@/dumps/win-explore" NFSU2_BENCH_DUMP_EVERY=37 NFSU2_BENCH_GPU=0 -- --bench explore 720
    line=$(pix_dirs win); ok=$?
    verdict window $ok "window path (144 Hz interpolated, AE 0): $line" ;;
  enhanced)
    # Modern and Ultra: AE 0 where a unit claims exact; otherwise the report (max diff, pixels) for approval.
    local p
    for p in ${ENH_PRESETS:-modern ultra}; do local enh; enh=$(preset_enh "$p") || exit 2
      for sc in ${ENH_SCENES:-racing explore}; do
        both "enh-$sc-$p" NFSU2_BENCH_ENH="$enh" NFSU2_BENCH_SIZE=1920x1080 NFSU2_BENCH_DUMP="$OUT/@SIDE@/dumps/enh-$sc-$p" NFSU2_BENCH_DUMP_EVERY=$every NFSU2_BENCH_GPU=0 -- --bench $sc $frames
      done; done
    line=$(pix_dirs enh --max-pixels ${ENH_MAX_PIXELS:-0} --max-diff ${ENH_MAX_DIFF:-0}); ok=$?
    verdict enhanced $ok "enhanced (${ENH_PRESETS:-modern ultra}; AE 0 unless approved, limits ENH_MAX_PIXELS=${ENH_MAX_PIXELS:-0} ENH_MAX_DIFF=${ENH_MAX_DIFF:-0}): $line" ;;
  shots)
    # main, career, options, worldmap and ending are left out by default: they animate on the wall
    # clock (Game::now without a fixed clock) and differ run to run with ONE binary (2026-11-04).
    local shots=${SHOTS-explore intro qrcars race qrstart drift drag garage_neon options_graphics options_graphics_image options_enhancements customize_display hood sms ui2d}
    local l; for l in base new; do mkdir -p "$OUT/$l/shots"; done
    local s runbad="" skipped=""
    for s in $shots; do
      side "$OUT" base "$BASE" "shot-$s" -- --shot "$(base_shot "$s")" "$OUT/base/shots/$s.png"
      side "$OUT" new "$NEW" "shot-$s" -- --shot "$s" "$OUT/new/shots/$s.png"
      local why; why=$(rcs "shot-$s")
      if [ -n "$why" ] && private_skip "shot-$s"; then
        skipped="$skipped $s"; rm -f "$OUT/base/shots/$s.png" "$OUT/new/shots/$s.png"; continue
      fi
      [ -z "$why" ] || runbad="$runbad $s($why)"
    done
    [ -z "$skipped" ] || gate "shots: SKIP (needs the maintainer's private set (OB_PRIVATE)):$skipped"
    local nshots; nshots=$(( $(echo $shots | wc -w) - $(echo $skipped | wc -w) ))
    if [ "$nshots" = 0 ]; then verdict shots 1 "shots: NO SHOTS (empty SHOTS): nothing compared"; continue; fi
    line=$(pix "$OUT/base/shots" "$OUT/new/shots")
    # Missing or unreadable PNGs (pixdiff's MISSING / ERROR lines) always fail.
    local missing; missing=$(grep -E ': (MISSING|ERROR)' "$OUT/pixdiff.tmp" | cut -d: -f1 | tr '\n' ' ')
    # A shot that differs is run again with BASE: if base differs from
    # itself the shot is not deterministic (wall-clock animation) and is
    # reported, not counted; else it is a real difference.
    local real=0 noisy=""
    mkdir -p "$OUT/base/shots2"
    for s in $(grep -oE '^[a-z0-9_]+\.png: [0-9]+ of' "$OUT/pixdiff.tmp" | cut -d. -f1); do
      local same=1 k
      for k in 1 2; do
        side "$OUT" base "$BASE" "shot2-$s-$k" -- --shot "$(base_shot "$s")" "$OUT/base/shots2/$s-$k.png" || same=0
        "$PIXDIFF" "$OUT/base/shots/$s.png" "$OUT/base/shots2/$s-$k.png" > /dev/null || same=0
      done
      if [ $same = 1 ]; then real=$((real + 1)); gate "  shot $s: DIFFERENT (base repeats itself twice)"; else noisy="$noisy $s"; fi
    done
    ok=0; { [ -z "$runbad" ] && [ -z "$missing" ] && [ $real = 0 ]; } || ok=1
    verdict shots $ok "shots (AE 0; $nshots targets): $line; failed runs:${runbad:- none}; missing:${missing:- none}; $real real differences; not deterministic run to run (base vs base differs, not counted):${noisy:- none}" ;;
  audio)
    # The game's mix on the offline mixer: the explore and race shots' WAVs byte-identical.
    for s in explore race; do
      side "$OUT" base "$BASE" "wav-$s" NFSU2_AUDIO_WAV="$OUT/base/$s.wav" -- --shot $s "$OUT/base/wav-$s.png"
      side "$OUT" new "$NEW" "wav-$s" NFSU2_AUDIO_WAV="$OUT/new/$s.wav" -- --shot $s "$OUT/new/wav-$s.png"
      local why; why=$(rcs "wav-$s")
      if [ -n "$why" ]; then verdict "audio-$s" 1 "audio $s: RUN FAILED ($why)"
      elif ! [ -s "$OUT/base/$s.wav" ] || ! [ -s "$OUT/new/$s.wav" ]; then verdict "audio-$s" 1 "audio $s: MISSING WAV (base $(stat -c %s "$OUT/base/$s.wav" 2> /dev/null || echo none), new $(stat -c %s "$OUT/new/$s.wav" 2> /dev/null || echo none) bytes)"
      elif cmp -s "$OUT/base/$s.wav" "$OUT/new/$s.wav"; then verdict "audio-$s" 0 "audio $s: identical ($(stat -c %s "$OUT/new/$s.wav") bytes)"
      else verdict "audio-$s" 1 "audio $s: DIFFERENT"; fi
    done ;;
  sim)
    sim_gates ;;
  rate)
    # Not base vs new: NEW against itself across display rates (cmd_rate). Its own verdict (RATE: ...);
    # it fails the parity on a counted difference (exit 1) and, with RATE=strict (the default here), on any (exit 3).
    rate_gates "$OUT/rate" "$NEW"; rate_rc=$? ;;
  esac; done
  gate "parity done $(date -Is)"
  if [ $GFAIL -gt 0 ]; then echo "PARITY: FAILURES in $GFAIL gates (${FAILED_GATES[*]}; see $OUT/gates.txt)"; return 1; fi
  if [ $rate_rc = 1 ] || { [ $rate_rc != 0 ] && [ "${RATE:-strict}" = strict ]; }; then echo "PARITY: rate gate failed (rc $rate_rc: 1 = a counted column, a run or the self-test; RATE=strict counts every difference)"; return 1; fi
  echo "PARITY: all gates passed"
}

normlog() { sed -E 's/[0-9]+(\.[0-9]+)? ?(s|ms)\b/T \2/g; s#/(base|new)/#/SIDE/#g; s/ +/ /g' "$1"; }

# Simulation: per-step state hashes over the flows, the playthrough and a parity scenario.
sim_gates() {
  local plan="$HERE/plans/circuit_s1.plan" l f save
  save=$(circuit_save)
  for l in base new; do
    sed -e "s#^save .*#save $save#" -e "s#^out .*#out $OUT/$l/parity-circuit_s1.tsv#" "$plan" > "$OUT/$l.circuit_s1.plan"
  done
  local flows=${FLOWS-firstrace firstdrag postrace playthrough parity}
  # The parity scenario starts from a private save: without the private set (a public checkout) it is left out.
  if [ -z "$save" ] && [[ " $flows " == *" parity "* ]]; then
    flows=$(echo " $flows " | sed 's/ parity / /; s/^ *//; s/ *$//')
    gate "sim parity: SKIP (its start save needs the maintainer's private set (OB_PRIVATE))"
  fi
  [ -n "$flows" ] || { verdict sim 1 "sim: NO FLOWS (empty FLOWS): nothing compared"; return; }
  for f in $flows; do
    local args=(--flow "$f") e=()
    [ "$f" = playthrough ] && args=(--playthrough)
    [ "$f" = postrace ] && e=(NFSU2_FLOW_DATA="$OUT/flowdata-LABEL")
    for l in base new; do
      local bin=$BASE; [ $l = new ] && bin=$NEW
      local a=("${args[@]}"); [ "$f" = parity ] && a=(--flow parity "$OUT/$l.circuit_s1.plan")
      local ee=() x; for x in "${e[@]}"; do ee+=("${x//LABEL/$l}"); done
      [ ${#ee[@]} -gt 0 ] && mkdir -p "$OUT/flowdata-$l"
      side "$OUT" $l "$bin" "flow-$f" NFSU2_STATE_HASH="$OUT/$l/hash-$f.tsv" "${ee[@]}" -- "${a[@]}"
    done
    local why; why=$(rcs "flow-$f")
    local v ok=0
    if [ -s "$OUT/base/hash-$f.tsv" ] && [ -s "$OUT/new/hash-$f.tsv" ]; then
      if cmp -s "$OUT/base/hash-$f.tsv" "$OUT/new/hash-$f.tsv"; then
        v="state hash identical over $(($(wc -l < "$OUT/new/hash-$f.tsv") - 1)) steps"
      else
        # Columns by name (an older base prints fewer), each counted as scripts/checks/rate-columns.txt's
        # "parity" field says (the v2 columns are report-only).
        local hd; hd=$(python3 "$HERE/hashdiff.py" "$OUT/base/hash-$f.tsv" "$OUT/new/hash-$f.tsv" "$HERE/../checks/rate-columns.txt")
        if [ $? = 0 ]; then v="state hash $hd"; else ok=1; v="state hash DIFFERENT: first $hd"; fi
      fi
    elif ! [ -s "$OUT/new/hash-$f.tsv" ]; then
      ok=1; v="MISSING: no state hashes from NEW ($OUT/new/hash-$f.tsv)"
    else
      # A base binary without NFSU2_STATE_HASH (an older build): its log and the parity rows instead.
      # Logs compared whole, wall times ("12.3 s", "4 ms"), the side's dir and padding normalised.
      local nd; nd=$(diff <(normlog "$OUT/base/flow-$f.log") <(normlog "$OUT/new/flow-$f.log") | grep -c '^[<>]')
      [ "$nd" = 0 ] || ok=1
      v="no base hashes (a base older than the hashes): flow logs (wall times normalised) $([ "$nd" = 0 ] && echo "identical, $(wc -l < "$OUT/new/flow-$f.log") lines" || echo "DIFFERENT in $nd lines")"
      if [ "$f" = parity ]; then
        if cmp -s "$OUT/base/parity-circuit_s1.tsv" "$OUT/new/parity-circuit_s1.tsv" && [ -s "$OUT/new/parity-circuit_s1.tsv" ]; then v="$v; clone.tsv rows identical"
        else ok=1; v="$v; clone.tsv rows DIFFERENT or MISSING"; fi
      fi
    fi
    [ -z "$why" ] || { ok=1; v="$v; RUN FAILED: $why"; }
    verdict "sim-$f" $ok "sim $f: $v"
  done
}

# ---- rate gate ---------------------------------------------------------------
# rate_gates OUT BIN (rate gate v2; docs/developing/checks.md "Rate gate"): the project rule "gameplay and look identical
# at 30, 60, 120, 200 fps". NEW against itself across display schedules, never base vs new. Each flow of
# RATE_FLOWS runs once per schedule of RATES (default "30 60 120 200") plus RATE_JITTER (default "jitter:7"; empty:
# none) plus a second run at RATE_REF (the control "ctl": columns that differ run to run are marked "nd"; RATE_CONTROL=0
# skips it), with NFSU2_RATE=<schedule> (exact integer-unit frames; inputs keyed on world-update ticks; the simulated
# wall clock; the draw-side state updates of each frame), NFSU2_STATE_HASH (v2 columns) and NFSU2_AUDIO_WAV.
# Flows: firstrace firstdrag postrace menus garage scenes outrun (--flow X) and racing explore replay (--bench X for
# RATE_SECS (default 10) s of ticks, NFSU2_RATE_TICKS, at every schedule; replay after NFSU2_REPLAY_AFTER=600 race frames). ratediff.py compares each
# schedule's table with RATE_REF's column by column (first differing tick per column), ratetable.py prints the
# flows × rates × columns table (the "number to move"). Images (RATE_IMG, default "orig step hist taa"; the racing
# bench at the numeric rates, NFSU2_RATE_DUMP: α = 0 frames at ticks ≡ 0 mod 6): rateimg.py against RATE_REF (orig =
# the racing runs above; step = Extreme with Anti-Aliasing 5 and no volumetric fog; hist = Extreme; taa = Extreme
# with TAA, bounded, not exact: rateimg.py). WAVs: ratewav.py against RATE_REF.
# Self-test (RATE_SELFTEST, default 300; empty: off): firstrace at RATE_REF and 120 Hz with NFSU2_RATE_SELFTEST=<tick>
# (one RNG draw per drawn frame from that tick): the comparison must report a difference at or after that tick and
# none before it, else "RATE SELF-TEST: MISSED".
# Exit: 0 every compared column identical; 1 a column scripts/checks/rate-columns.txt marks "fail" differs, an image
# class it marks "fail" differs (taa: over its bound), a run failed, or the self-test missed; 3 only report-only
# columns, images or WAVs differ. Callers: RATE=1 fails on 1, RATE=strict (the gates' default) on anything but 0.
rate_gates() {
  local out=$1 bin=$2 f s
  [ -x "$bin" ] || die "rate: no binary $bin"
  mkdir -p "$out"; : > "$out/rate.txt"
  local SIDE_CACHE="$out/cache"   # one asset cache for every rate (same binary)
  local rates=${RATES:-30 60 120 200} ref=${RATE_REF:-60} secs=${RATE_SECS:-10} hard=0 soft=0 nflows=0
  local jitter=${RATE_JITTER-jitter:7} pix=${PIXDIFF:-$(dirname "$bin")/pixdiff}
  local scheds="$rates $jitter"; [ "${RATE_CONTROL:-1}" = 0 ] || scheds="$scheds ctl"
  case " $rates " in *" $ref "*) ;; *) die "rate: RATE_REF $ref is not in RATES ($rates)" ;; esac
  local t0; t0=$(date +%s)
  say_rate() { echo "$*" | tee -a "$out/rate.txt"; }
  # rate_run LABEL SCHED FLOW EXTRA_ENV…: one run; sets RRUN_WHY on a failure.
  rate_run() {
    local label=$1 sch=$2 f=$3; shift 3
    local hz=$sch; [ "$sch" = ctl ] && hz=$ref
    # Benches end on a tick count (NFSU2_RATE_TICKS = RATE_SECS of 60 Hz ticks), every schedule alike: a jittered
    # run's stalls drop time past the tick cap, so a frame count or a time span would end it a few ticks short.
    # The frame count is only an upper bound there (frames are at least 4 ms).
    case $hz in jitter:*) local n=$((secs * 250)) h=60 ;; *) local n=$((secs * ${hz%.*})) h=$hz ;; esac
    local e=(NFSU2_RATE="$hz" NFSU2_STATE_HASH="$out/$f-$label.tsv" NFSU2_AUDIO_WAV="$out/$f-$label.wav") a=()
    case $f in
      firstrace) e+=(NFSU2_FIRSTRACE_HZ=$h); a=(--flow firstrace) ;;
      firstdrag) e+=(NFSU2_FIRSTDRAG_HZ=$h); a=(--flow firstdrag) ;;
      postrace) mkdir -p "$out/flowdata-$f-$label"; e+=(NFSU2_POSTRACE_HZ=$h NFSU2_FLOW_DATA="$out/flowdata-$f-$label"); a=(--flow postrace) ;;
      menus|garage|scenes|outrun) a=(--flow "$f") ;;
      racing|explore|race|drive|replay) e+=(NFSU2_BENCH_SIZE=${RATE_SIZE:-1280x720} NFSU2_BENCH_GPU=0 NFSU2_BENCH_AUDIO=1 NFSU2_REPLAY_AFTER=${RATE_REPLAY_AFTER:-600} NFSU2_RATE_TICKS=$((secs * 60))); a=(--bench "$f" $n) ;;
      *) die "rate: unknown flow $f" ;;
    esac
    side "$out" "$label" "$bin" "$f" "${e[@]}" "$@" -- "${a[@]}"
    local rc=$? b; b=$(ck_bad "$out/$label/$f.log")
    # A flow that needs the maintainer's private set exits 3 with a "SKIP: " line (src/bin/game/dev/private.rs):
    # without the set (no saves folder in $OB_PRIVATE, else in REPO) a skip, with it a failed run.
    # Only the flows' runs may skip (RRUN_ALLOW_SKIP); the image and self-test runs never do.
    if [ -n "${RRUN_ALLOW_SKIP:-}" ] && [ $rc = 3 ] && [ "$b" = 0 ] && grep -q '^SKIP: ' "$out/$label/$f.log" && ! [ -d "${OB_PRIVATE:-$REPO}/tests/fixtures/native_saves" ]; then
      RRUN_SKIP="$RRUN_SKIP $label"; return
    fi
    { [ $rc = 0 ] && [ "$b" = 0 ]; } || RRUN_WHY="$RRUN_WHY $label:rc=$rc,panic/validation=$b [$({ grep -m1 -iE 'panic|validation|error|timed out' "$out/$label/$f.log" 2>/dev/null | cut -c1-160; echo "last: $(tail -n1 "$out/$label/$f.log" 2>/dev/null | cut -c1-120)"; } | tr '\n' ' ')]"
  }
  local jsons=() skipped="" RRUN_ALLOW_SKIP=1
  for f in ${RATE_FLOWS:-firstrace firstdrag postrace menus garage scenes outrun racing explore replay}; do
    nflows=$((nflows + 1))
    local tabs=() wavs=() RRUN_WHY="" RRUN_SKIP="" lab
    for s in $scheds; do
      lab=${s//:/-}; local de=()
      case $f:$s in racing:[0-9]*|racing:ctl) de=(NFSU2_RATE_DUMP="$out/img-orig-$lab") ;; esac
      rate_run "$lab" "$s" "$f" "${de[@]}"
      tabs+=("$s=$out/$f-$lab.tsv"); wavs+=("$s=$out/$f-$lab.wav")
    done
    # Skipped at every schedule (it needs the private set, which is not here): named and left out, never passed.
    # Skipped at some schedules only: a failure.
    if [ -n "$RRUN_SKIP" ]; then
      if [ -z "$RRUN_WHY" ] && [ "$(echo $RRUN_SKIP | wc -w)" = "$(echo $scheds | wc -w)" ]; then
        skipped="$skipped $f"
        say_rate "rate $f: SKIP (needs the maintainer's private set (OB_PRIVATE)): $(grep -m1 '^SKIP: ' "$out/${scheds%% *}/$f.log" | cut -c7-160)"
        continue
      fi
      RRUN_WHY="$RRUN_WHY skipped at some schedules only:$RRUN_SKIP"
    fi
    local line; line=$(python3 "$HERE/ratediff.py" ${RATE_POLICY:+--policy "$RATE_POLICY"} --flow "$f" --json "$out/$f.json" "$ref" "${tabs[@]}"); local rc=$?
    jsons+=("$out/$f.json")
    local wl; wl=$(python3 "$HERE/ratewav.py" "$ref" "${wavs[@]}"); local wrc=$?
    if [ -n "$RRUN_WHY" ] || [ $rc != 0 ]; then hard=$((hard + 1)); say_rate "FAIL: rate $f: $line${RRUN_WHY:+; RUN FAILED:$RRUN_WHY}"
    elif grep -q 'columns identical; first' <<< "$line"; then soft=$((soft + 1)); say_rate "rate $f: $line"
    else say_rate "rate $f: $line"; fi
    say_rate "  wav $f: $wl"; [ $wrc = 0 ] || soft=$((soft + 1))
  done
  RRUN_ALLOW_SKIP=
  # Images: the racing bench's α = 0 frames per preset class.
  case " ${RATE_FLOWS:-racing} " in *" racing "*)
    local cls
    for cls in ${RATE_IMG-orig step hist taa}; do
      local enh="" dirs=()
      case $cls in
        orig) ;;
        step) enh="$EXTREME;enh.post_aa 5;enh.volumetric_fog 0" ;;
        hist) enh="$EXTREME" ;;
        taa) enh="$EXTREME;enh.post_aa 11" ;;
        *) die "rate: unknown image class $cls" ;;
      esac
      for s in $rates; do
        if [ $cls != orig ]; then
          local RRUN_WHY=""
          rate_run "$cls-$s" "$s" racing NFSU2_BENCH_ENH="$enh" NFSU2_RATE_DUMP="$out/img-$cls-$s"
          [ -z "$RRUN_WHY" ] || { hard=$((hard + 1)); say_rate "FAIL: rate images $cls: RUN FAILED:$RRUN_WHY"; }
        fi
        [ "$s" = "$ref" ] || dirs+=("$s=$out/img-$cls-$s")
      done
      [ $cls = orig ] && [ -d "$out/img-orig-ctl" ] && dirs+=("ctl=$out/img-orig-ctl")
      local il; il=$(python3 "$HERE/rateimg.py" "$pix" "$cls" "$out/img-$cls-$ref" "${dirs[@]}"); local irc=$?
      # An image class the policy marks "images@CLASS fail" counts (a difference, or for taa one over its bound).
      local ipol; ipol=$(awk -v k="images@$cls" '$1 == k { print $2 }' "${RATE_POLICY:-$HERE/../checks/rate-columns.txt}" 2>/dev/null)
      if [ $irc != 0 ] && [ "$ipol" = fail ]; then hard=$((hard + 1)); say_rate "  FAIL: images $il"
      else say_rate "  images $il"; [ $irc = 0 ] || soft=$((soft + 1)); fi
    done ;;
  esac
  # Self-test: an injected harness-only rate dependence must be caught: firstrace at RATE_REF and 120 Hz with
  # NFSU2_RATE_SELFTEST=<tick>; caught = a column's first difference between the rates comes earlier with the
  # injection than without it, at or after the injected tick (and none earlier than that); columns firstrace's
  # control run found not deterministic run to run are left out.
  if [ -n "${RATE_SELFTEST-300}" ]; then
    local at=${RATE_SELFTEST-300} RRUN_WHY="" k
    for k in $ref 120; do
      [ -s "$out/firstrace-$k.tsv" ] || rate_run "$k" "$k" firstrace
      rate_run "selftest-$k" "$k" firstrace NFSU2_RATE_SELFTEST=$at
    done
    python3 "$HERE/ratediff.py" --flow plain --json "$out/selftest-plain.json" "$ref" "$ref=$out/firstrace-$ref.tsv" "120=$out/firstrace-120.tsv" > /dev/null
    python3 "$HERE/ratediff.py" --flow selftest --json "$out/selftest.json" "$ref" "$ref=$out/firstrace-selftest-$ref.tsv" "120=$out/firstrace-selftest-120.tsv" > /dev/null
    local sl; sl=$(python3 - "$out/selftest-plain.json" "$out/selftest.json" "$at" "$out/firstrace.json" << 'PY'
import json, os, sys
p, q, at = json.load(open(sys.argv[1]))['rates'].get('120', {}), json.load(open(sys.argv[2]))['rates'].get('120', {}), int(sys.argv[3])
# Columns firstrace's control (the reference run twice) found not deterministic are left out.
nd = set(json.load(open(sys.argv[4])).get('control', {})) if os.path.exists(sys.argv[4]) else set()
# Columns known to differ between two identical runs now and then, whatever the control pair showed this time
# (RATE_ND, default none): aud is deterministic since the audio clock, clk and fe since game time runs on ticks.
nd |= set(os.environ.get('RATE_ND', '').split())
q = {c: v for c, v in q.items() if c not in nd}
tick = lambda v: 10**12 if v == '=' else int(v.lstrip('nd').replace('end@', ''))
# A column the injection moved: its first difference earlier with it than without, at or after the injected tick.
hit = [f'{c}@{q[c]}' for c in q if tick(q[c]) < tick(p.get(c, '=')) and tick(q[c]) >= at]
early = [f'{c}@{q[c]}' for c in q if tick(q[c]) < tick(p.get(c, '=')) and tick(q[c]) < at]
print(('caught: ' if hit and not early else 'MISSED: ') + f'columns the injection made differ earlier (from tick {at} on): {", ".join(hit) or "none"}' + (f'; before the injection: {", ".join(early)}' if early else ''))
PY
)
    if [ -z "$RRUN_WHY" ] && [ "${sl%%:*}" = caught ]; then say_rate "rate self-test (NFSU2_RATE_SELFTEST=$at, firstrace $ref vs 120 Hz): $sl"
    else hard=$((hard + 1)); say_rate "FAIL: RATE SELF-TEST (NFSU2_RATE_SELFTEST=$at, firstrace $ref vs 120 Hz): $sl${RRUN_WHY:+; RUN FAILED:$RRUN_WHY}"; fi
  fi
  [ $nflows -gt 0 ] || { say_rate "FAIL: rate: no flows (empty RATE_FLOWS)"; hard=1; }
  python3 "$HERE/ratetable.py" "${jsons[@]}" | tee -a "$out/rate.txt" > "$out/table.txt"
  local cells; cells=$(grep '^identical cells' "$out/table.txt" | sed 's/.*total //')
  local skn=""; [ -z "$skipped" ] || skn=" skipped (needs the maintainer's private set (OB_PRIVATE)):$skipped;"
  local v="RATE: $([ $hard != 0 ] && echo "FAIL ($hard hard failures)" || { [ $soft != 0 ] && echo "DIFFERENCES ($soft parts differ across rates $rates${jitter:+ $jitter})" || echo PASS; }); identical cells $cells;$skn $(( $(date +%s) - t0 )) s"
  say_rate "$v"
  [ -n "${OUT:-}" ] && [ -f "$OUT/gates.txt" ] && sed 's/^/  /' "$out/rate.txt" >> "$OUT/gates.txt" && echo "rate (cross-rate, NEW only; counted, RATE=${RATE:-strict}): $v" >> "$OUT/gates.txt"
  [ $hard = 0 ] || return 1
  [ $soft = 0 ] || return 3
  return 0
}

cmd_rate() {
  local out=${1:?OUT} bin; bin=$(readlink -f "${2:?BIN}")
  ck_need_root
  export RADV_DEBUG=${PARITY_RADV_DEBUG-nodcc}
  mkdir -p "$out"; OUT=""
  rate_gates "$out" "$bin"
}

cmd_profile() {
  local out=${1:?OUT} src=${2:?SRC} scene=${3:?SCENE} preset=${4:-orig} frames=${5:-3000}
  local samply=${SAMPLY:-$(command -v samply)}
  [ -n "$samply" ] && [ -x "$samply" ] || die "no samply (put samply 0.13.1 on PATH or set SAMPLY; docs/developing/perf-gates.md: the perf_group patch)"
  ck_need_root
  local enh; enh=$(preset_enh "$preset") || exit 2
  mkdir -p "$out"
  # Same codegen as the measured build, plus line tables for symbols. One target dir per source tree
  # (as cmd_build: cargo's mtime freshness would give a second tree the first one's binary).
  local tgt="$PERF_WORK/target-prof-$(realpath "$src" | sha256sum | cut -c1-12)"
  ( cd "$src" && env -u CARGO_PROFILE_RELEASE_LTO -u CARGO_PROFILE_RELEASE_CODEGEN_UNITS -u CARGO_PROFILE_RELEASE_INCREMENTAL \
      -u CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS -u RUSTC_WRAPPER CARGO_PROFILE_RELEASE_DEBUG=line-tables-only \
      CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-6} CARGO_TARGET_DIR="$tgt" cargo build --release --bin game $(harness_features "$src") ) || die "build failed"
  local bin="$out/game-prof"; cp "$tgt/release/game" "$bin" || die "copy of the profile binary failed"
  local s=$scene e=(); [ "$scene" = rain ] && { s=drive; e=(NFSU2_RAIN_CLOCK=162); }
  local name="$scene-$preset"
  env XDG_DATA_HOME="$out/xdg" NFSU2_CACHE_DIR="$out/cache" NFSU2_BENCH_SIZE=${SIZE:-1920x1080} ${enh:+NFSU2_BENCH_ENH="$enh"} "${e[@]}" \
    NFSU2_BENCH_JSON="$out/$name.bench.json" timeout 1200 "$samply" record -s -n --unstable-presymbolicate -r 900 -o "$out/$name.json.gz" "$bin" --bench "$s" "$frames" > "$out/$name.out" 2>&1 &
  local pid=$!; echo "$pid profile $name" >> "$out/pids.log"; wait $pid || die "profile run failed (rc $?; see $out/$name.out)"
  # Top functions per thread (load skipped: first 4 s) and the GPU passes of the same run.
  python3 "$HERE/prof.py" "$out/$name.json.gz" 40 ${SKIP:-4} > "$out/$name.summary.txt"
  python3 - "$out/$name.bench.json" >> "$out/$name.summary.txt" <<'EOF'
import json, sys
d = json.load(open(sys.argv[1]))
parts = sorted((p for p in d.get('gpu_parts', []) if p['name'] != '(between submissions)'), key=lambda p: -p['ms_per_frame'])
tot = sum(p['ms_per_frame'] for p in parts) or 1
print('\n## GPU passes (timestamps, this run): ' + '; '.join(f"{p['name']} {p['ms_per_frame']:.3f} ms ({100*p['ms_per_frame']/tot:.0f} %)" for p in parts[:12]))
print('## threads CPU ms/frame: ' + '; '.join(f"{t['name']} {t['ms_per_frame']:.3f}" for t in d.get('threads', [])[:8]))
EOF
  head -12 "$out/$name.summary.txt" | cut -c1-400
}

cmd=${1:-}; shift || true
case $cmd in
  build) cmd_build "$@" ;;
  bench) cmd_bench "$@" ;;
  stats) cmd_stats "$@" ;;
  summary) cmd_summary "$@" ;;
  parity) cmd_parity "$@" ;;
  rate) cmd_rate "$@" ;;
  profile) cmd_profile "$@" ;;
  *) sed -n '2,20p' "$0"; exit 2 ;;
esac
