#!/bin/bash
# passes.sh OUT "label=bin ..." "cfg ..."  — interleaved fixed-length benches for the pass table.
# cfg = scene-preset-WxH; scenes explore racing race rain; presets orig low med high and med+<row> variants (below).
# env GPU=igpu|dgpu (igpu: IGPU_DEVICE), REPS, FRAMES, CACHE, NFSU2_ROOT; an arm "label=bin@VAR=v" adds env VAR=v to that arm
set -u
OUT=$1; ARMS=$2; CFGS=$3
REPS=${REPS:-2}; FRAMES=${FRAMES:-300}; GPU=${GPU:-igpu}
mkdir -p "$OUT/runs" "$OUT/xdg/run" "$OUT/xdg/config" "$OUT/xdg/state"; chmod 700 "$OUT/xdg/run"
. "$(dirname "$0")/levels.sh"
run() { # label bin cfg rep
  local label=$1 bin=$2 cfg=$3 rep=$4 armenv=""
  case $bin in *@*) armenv=${bin#*@}; bin=${bin%%@*} ;; esac
  IFS=- read -r scene preset size <<< "$cfg"
  # a preset's '-' is written '_' in the config (racing-med_ssao-1920x1080)
  preset=${preset//_/-}
  local enh; enh=$(enh_of "$preset"); [ "$enh" = BAD ] && { echo "bad preset $preset" | tee -a "$OUT/fail.log"; return; }
  local base="$OUT/runs/${label}__${cfg}__r$rep"
  local -a e=(NFSU2_ROOT="$NFSU2_ROOT" XDG_DATA_HOME="$OUT/xdg/data-$label" XDG_CONFIG_HOME="$OUT/xdg/config" XDG_STATE_HOME="$OUT/xdg/state"
    XDG_RUNTIME_DIR="$OUT/xdg/run" NFSU2_CACHE_DIR="${CACHE:?CACHE=<a cache folder of your own>}/$label" NFSU2_BENCH_SIZE="$size" NFSU2_BENCH_JSON="$base.json")
  [ "$GPU" = igpu ] && e+=(MESA_VK_DEVICE_SELECT=${IGPU_DEVICE:?} MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE=1)
  [ -n "$enh" ] && e+=(NFSU2_BENCH_ENH="$enh")
  [ -n "${XENV:-}" ] && for x in $XENV; do e+=("$x"); done
  [ -n "$armenv" ] && for x in ${armenv//@/ }; do e+=("$x"); done
  local s=$scene
  [ $scene = rain ] && { s=drive; e+=(NFSU2_RAIN_CLOCK=162); }
  { echo "bin $bin cfg $cfg rep $rep gpu $GPU"; echo "loadavg $(cat /proc/loadavg)"; echo "dgpu-busy $(cat /sys/class/drm/card0/device/gpu_busy_percent 2>/dev/null)"; } > "$base.txt"
  env -u WAYLAND_DISPLAY -u DISPLAY -u XDG_SESSION_TYPE "${e[@]}" timeout 600 "$bin" --bench "$s" "$FRAMES" >> "$base.txt" 2>&1 &
  local pid=$!; echo $pid > "$OUT/cur.pid"; wait $pid; local rc=$?
  echo "rc=$rc loadavg-end $(cat /proc/loadavg) dgpu-busy $(cat /sys/class/drm/card0/device/gpu_busy_percent 2>/dev/null)" >> "$base.txt"
  [ $rc = 0 ] || echo "FAIL $label $cfg r$rep rc=$rc" | tee -a "$OUT/fail.log"
}
read -r -a A <<< "$ARMS"; n=${#A[@]}
if [ -z "${NOWARM:-}" ]; then for a in "${A[@]}"; do FR=$FRAMES; FRAMES=40; run "${a%%=*}" "${a#*=}" "${CFGS%% *}" 0; FRAMES=$FR; rm -f "$OUT"/runs/*__r0.json; done; fi
echo "start $(date -Is) gpu $GPU reps $REPS frames $FRAMES arms $ARMS uptime $(uptime)" >> "$OUT/log"
for rep in $(seq 1 "$REPS"); do
  order=$CFGS; [ $((rep % 2)) = 0 ] && order=$(echo $CFGS | tr ' ' '\n' | tac | tr '\n' ' ')
  for cfg in $order; do
    for k in $(seq 0 $((n - 1))); do
      i=$(( (k + rep) % n )); a=${A[$i]}
      run "${a%%=*}" "${a#*=}" "$cfg" "$rep"
    done
  done
done
echo "end $(date -Is)" >> "$OUT/log"
