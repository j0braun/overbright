#!/bin/bash
# The Enhanced quality levels' cost (levels.txt): GPU busy median ms, 4 scenes, Original and the three levels.
#   levels.sh GAME OUT [igpu]    NFSU2_ROOT and NFSU2_CACHE_DIR set by the caller; IGPU_DEVICE=vendor:device for igpu.
set -u
G=$1 OUT=$2 GPU=${3:-}
LOW="enh.enhanced_paint 1;enh.enhanced_reflections 1;enh.road_reflection 1;enh.shadows 2;enh.lod_fade 1;enh.detail_distance 150;enh.wet_car 1;enh.rain_detail 1"
MED="$LOW;enh.hdr_post 1;enh.car_reflection 256;enh.wet_roads 1;enh.ssao 50;enh.dynamic_lamps 1;enh.lamp_shadows 1;enh.contact_shadows 1;enh.spray 1;enh.headlight_beams 1;enh.light_probes 1;enh.volumetric_fog 1"
HIGH="$MED;enh.hdr_post 2;enh.road_reflection 2;enh.car_reflection 1024;enh.detail_distance 200;enh.car_detail 200;enh.keep_loaded 1;enh.ssr 1;enh.lamp_shadows 2;enh.volumetric_fog 2;enh.particles 2;enh.water 1;enh.rain_detail 2;enh.wet_interaction 1;enh.enhanced_glass 1"
declare -A CFG=([orig]="" [low]="$LOW" [med]="$MED" [high]="$HIGH")
mkdir -p "$OUT"
for rep in 1 2; do for size in 1920x1080 3840x2160; do for sc in explore racing race rain; do for cfg in orig low med high; do
  [ -n "$GPU" ] && [ $size != 1920x1080 ] && continue
  s=$sc; extra=(); [ $sc = rain ] && { s=drive; extra=(NFSU2_RAIN_CLOCK=162); }
  [ -n "$GPU" ] && extra+=(MESA_VK_DEVICE_SELECT=$IGPU_DEVICE MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE=1)
  env "${extra[@]}" XDG_DATA_HOME="$OUT/xdg" NFSU2_BENCH_SIZE=$size NFSU2_BENCH_ENH="${CFG[$cfg]}" \
    NFSU2_BENCH_JSON="$OUT/${size}_${sc}_${cfg}_$rep.json" "$G" --bench $s 300 > "$OUT/${size}_${sc}_${cfg}_$rep.log" 2>&1
done; done; done; done
