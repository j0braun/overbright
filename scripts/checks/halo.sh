#!/bin/bash
# Regression check for the glow-shell fix (docs/wet-road-car.md, max rain halo): the glow shell around the player's car
# and its blue tint in the rain at speed must not return.
#
#   halo.sh OUTDIR [GAME]
#
# Two 1920x1080 bench runs (frame 120) of the reported screenshot scene: the finished-career save's Skyline (yellow underglow,
# $HALO_SAVE from scripts/local.env, else the finished-career fixture under tests/fixtures/native_saves/)
# placed on the plaza street at 217 km/h in 4th gear, rain (clock 167), the overlay's max settings
# (every Next-Gen tier, SSAA 2x); once as is and once with Wet Road Interaction off and the car lights at
# Headlight Beams On. halo.py compares the car's upper body between them (before the fix: 6.5, after: 1.6).
# Exit status: halo.py's; 2 a run failed; 3 skipped: no HALO_SAVE and no private set (the default save is private).
set -u
HERE=$(cd "$(dirname "$0")" && pwd); REPO=$(cd "$HERE/../.." && pwd)
OUT=${1:?usage: halo.sh OUTDIR [GAME]}; G=${2:-$REPO/target/release/game}
mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd)
. "$REPO/scripts/lib/common.sh"; ck_local_env; ck_need_root
SAVE=${HALO_SAVE:-}
if [ -n "$SAVE" ]; then [ -f "$SAVE" ] || ck_die "HALO_SAVE=$SAVE is not a file"
elif ! SAVE=$(ck_private_file tests/fixtures/native_saves/career_complete.sav); then
  # The default scene's save is a private fixture: without the set the check cannot run here (exit 3, never a pass).
  echo "halo: SKIP (the finished-career save needs the maintainer's private set (OB_PRIVATE); or set HALO_SAVE to a profile of your own in scripts/local.env)"
  exit 3
fi
CUST="custom.0 3;custom.1 1;custom.2 2;custom.3 1;custom.4 1;custom.5 1;custom.6 2;custom.7 3;custom.8 1;custom.9 1;custom.10 1;custom.11 1;custom.12 1;custom.13 1;custom.14 3;custom.15 1;custom.16 1;custom.17 1;custom.18 1;custom.19 1;custom.20 2;custom.21 0"
MAX="$CUST;enh.post_aa 6;enh.road_reflection 2;enh.car_reflection 1024;enh.detail_distance 200;enh.car_detail 200;enh.hd_ui 1;enh.lod_fade 1;enh.keep_loaded 1;enh.shadows 2;enh.enhanced_reflections 1;enh.ssao 100;enh.hdr_post 2;enh.enhanced_paint 1;enh.wet_roads 1;enh.fix_models 1;enh.ssr 3;enh.dynamic_lamps 1;enh.lamp_shadows 1;enh.contact_shadows 1;enh.volumetric_fog 3;enh.sky 1;enh.particles 2;enh.sparks_fire 1;enh.wet_car 1;enh.rain_detail 3;enh.spray 1;enh.lens_drops 1;enh.wet_interaction 2;enh.water 3;enh.ambient_detail 2;enh.headlight_beams 2;enh.light_probes 1;enh.enhanced_glass 1;enh.surface_detail 2;enh.smooth_cars 1;enh.far_draw 1"
export NFSU2_HEADLESS_ONLY=1
export NFSU2_BENCH_SIZE=1920x1080 NFSU2_SHOT_SAVE=$SAVE NFSU2_BENCH_AT=-307.8,-345.7,23.0,90 NFSU2_FX_SPEED=217 NFSU2_FX_GEAR=4
export NFSU2_BENCH_AT_DRIVE=20 NFSU2_RAIN_CLOCK=167 NFSU2_BENCH_DUMP_EVERY=120 NFSU2_BENCH_GPU=0 TMPDIR=$OUT
for v in with without; do
  E="$MAX"; [ $v = without ] && E="$MAX;enh.wet_interaction 0;enh.headlight_beams 1"
  rm -rf "$OUT/$v"; NFSU2_BENCH_DUMP=$OUT/$v NFSU2_BENCH_ENH="$E" "$G" --bench explore 120 > "$OUT/$v.log" 2>&1 || { echo "halo: bench failed ($OUT/$v.log)"; exit 2; }
  grep -qi 'panic\|validation error' "$OUT/$v.log" && { echo "halo: panic/validation ($OUT/$v.log)"; exit 2; }
done
python3 "$HERE/halo.py" "$OUT/with/explore-00120.png" "$OUT/without/explore-00120.png"
