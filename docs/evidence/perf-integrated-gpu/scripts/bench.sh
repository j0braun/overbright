#!/bin/bash
# pb.sh OUT "label=bin label=bin" "cfg cfg ..."  — interleaved benches, perfstat-compatible run names.
# cfg = scene-preset-WxH ; env GPU=igpu|dgpu, CPUS (taskset list, default 0-3), REPS (3), FRAMES (300), HZ (bench schedule)
# scenes: free (explore), start (race), racing, rain (drive @ rain clock 162), lamps (casino, frozen)
set -u
OUT=$1; ARMS=$2; CFGS=$3
REPS=${REPS:-3}; FRAMES=${FRAMES:-300}; CPUS=${CPUS:-0-3}; GPU=${GPU:-igpu}
mkdir -p "$OUT/runs"
BAL="enh.hdr_post 1;enh.post_aa 5;enh.enhanced_paint 1"
HIGH="$BAL;enh.enhanced_reflections 1;enh.road_reflection 1;enh.car_reflection 256;enh.ssao 50;enh.shadows 2;enh.lod_fade 1;enh.detail_distance 150;enh.wet_roads 1;enh.dynamic_lamps 1;enh.lamp_shadows 1;enh.contact_shadows 1;enh.wet_car 1;enh.rain_detail 1;enh.spray 1;enh.headlight_beams 1;enh.light_probes 1;enh.volumetric_fog 0"
NR=${NFSU2_ROOT:?}
run() { # label bin cfg rep
  local label=$1 bin=$2 cfg=$3 rep=$4
  IFS=- read -r scene preset size <<< "$cfg"
  local base="$OUT/runs/${label}__${cfg}__r$rep"
  local -a e=(NFSU2_ROOT="$NR" XDG_DATA_HOME="$OUT/xdg-$label" NFSU2_CACHE_DIR="$OUT/cache-$label" NFSU2_BENCH_SIZE="$size" NFSU2_BENCH_JSON="$base.json")
  [ "$GPU" = igpu ] && e+=(MESA_VK_DEVICE_SELECT=${IGPU_DEVICE:?} MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE=1)
  [ "$GPU" = dgpu ] && e+=(MESA_VK_DEVICE_SELECT=${DGPU_DEVICE:?} MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE=1)
  [ -n "${HZ:-}" ] && e+=(NFSU2_BENCH_HZ=$HZ)
  [ -n "${XENV:-}" ] && for x in $XENV; do e+=("$x"); done
  case $preset in orig) ;; bal) e+=(NFSU2_BENCH_ENH="$BAL") ;; high) e+=(NFSU2_BENCH_ENH="$HIGH") ;; wi1) e+=(NFSU2_BENCH_ENH="$HIGH;enh.wet_interaction 1") ;; wi2) e+=(NFSU2_BENCH_ENH="$HIGH;enh.wet_interaction 2") ;; *) echo "bad preset $preset"; exit 2;; esac
  local s=$scene
  case $scene in
    free) s=explore ;; start) s=race ;; racing) s=racing ;;
    rain) s=drive; e+=(NFSU2_RAIN_CLOCK=162) ;;
    lamps) s=explore; e+=(NFSU2_BENCH_AT=-400,-524,19.5,180 NFSU2_BENCH_FREEZE=1 NFSU2_RAIN_CLOCK=93) ;;
  esac
  { echo "bin $bin cfg $cfg rep $rep gpu $GPU cpus $CPUS"; echo "loadavg $(cat /proc/loadavg)"; } > "$base.txt"
  env -u WAYLAND_DISPLAY -u DISPLAY "${e[@]}" timeout 900 taskset -c "$CPUS" "$bin" --bench "$s" "$FRAMES" >> "$base.txt" 2>&1 &
  local pid=$!; wait $pid; local rc=$?
  echo "rc=$rc loadavg-end $(cat /proc/loadavg)" >> "$base.txt"
  [ $rc = 0 ] || echo "FAIL $label $cfg r$rep rc=$rc" | tee -a "$OUT/fail.log"
}
read -r -a A <<< "$ARMS"; n=${#A[@]}
# Warm-up per arm (asset cache, post self-check verdict), not recorded.
if [ -z "${NOWARM:-}" ]; then for a in "${A[@]}"; do FR=$FRAMES; FRAMES=60; run "${a%%=*}" "${a#*=}" "${CFGS%% *}" 0; FRAMES=$FR; rm -f "$OUT"/runs/*__r0.json; done; fi
echo "start $(date -Is) gpu $GPU cpus $CPUS reps $REPS frames $FRAMES hz ${HZ:-} arms $ARMS" >> "$OUT/log"
for rep in $(seq 1 "$REPS"); do
  for cfg in $CFGS; do
    for k in $(seq 0 $((n - 1))); do
      i=$(( (k + rep) % n )); a=${A[$i]}
      run "${a%%=*}" "${a#*=}" "$cfg" "$rep"
    done
  done
done
echo "end $(date -Is)" >> "$OUT/log"
