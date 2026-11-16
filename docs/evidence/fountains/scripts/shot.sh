#!/bin/bash
# shot.sh BIN OUT.png MODE VIEW CLOCK [VAR=value ...]
# MODE: orig | player | extreme | max | w1 | w2 | w3 (Original + Animated Water tier) | "k v;..." raw
# VIEW: casino10 casino30 casino80 casinolow | C B N P W (free cameras of the water docs) | at:x,y,z,h | look:e..,t..
set -u
W=${WORK:?the work dir}
SAVE=${REPO:?the source tree}/tests/fixtures/native_saves/career_complete.sav
G=$1; out=$2; mode=$3; view=$4; clock=$5; shift 5
: "${NFSU2_ROOT:?the game folder}" "${NFSU2_CACHE_DIR:?a cache folder of your own}"; export
export NFSU2_HEADLESS_ONLY=1 NFS_NO_UI=${NOUI:-0}
export XDG_DATA_HOME=$W/data XDG_CACHE_HOME=${NFSU2_CACHE_DIR:?a cache folder of your own}
export NFSU2_SHOT_SIZE=${SIZE:-1920x1080} NFSU2_RAIN_CLOCK=$clock NFSU2_SHOT_SAVE=$SAVE NFSU2_SHOT_HOLD=1
O=$(cat $W/orig.enh)
case $mode in
  orig) E="$O" ;;
  player) E="$(cat $W/player.enh)" ;;
  extreme) E="$O;$(cat $W/extreme.enh)" ;;
  max) E="$O;$(cat $W/extreme.enh);enh.water 3" ;;
  w1|w2|w3) E="$O;enh.water ${mode#w}" ;;
  *) E="$mode" ;;
esac
export NFSU2_SHOT_ENH="$E"
unset NFSU2_SHOT_LOOK
declare -A V=( [C]="-785.1,-284.5,26:-805,-296,31" [R]="-326,-547,21:-350,-575,28" [N]="-827,714,36:-830,690,44" [P]="-114,1494,40:-114,1470,46" [W]="-570,-150,20:-585,-170,27" [B]="-153.8,-615.1,18:-140,-600,24" )
case $view in
  casino10) export NFSU2_SHOT_AT=-306,-547,20,180 ;;
  casino30) export NFSU2_SHOT_AT=-286,-547,20,180 ;;
  casino80) export NFSU2_SHOT_AT=-236,-547,20,180 ;;
  casinolow) export NFSU2_SHOT_AT=-296,-535,20,180 NFSU2_SHOT_LOOK=-296,-535,20.5,-326,-552,22 ;;
  playerview) export NFSU2_SHOT_AT=-300,-547,20,180 ;;
  wine) export NFSU2_SHOT_AT=-351.5,-542.1,20,-10.9 ;;
  at:*) export NFSU2_SHOT_AT=${view#at:} ;;
  look:*) l=${view#look:}; export NFSU2_SHOT_LOOK=$l NFSU2_SHOT_AT="${l%%,*},$(echo $l | cut -d, -f2),$(( $(echo $l | cut -d, -f3 | cut -d. -f1) - 6 )),0" ;;
  *) f=${V[$view]}; t=${f%%:*}; e=${f#*:}; IFS=, read ex ey ez <<<"$e"
     export NFSU2_SHOT_LOOK="$e,$t" NFSU2_SHOT_AT="$ex,$ey,$((${ez%.*}-6)),0" ;;
esac
for kv in "$@"; do export "$kv"; done
timeout 1200 "$G" --shot explore "$out" > "${out%.png}.log" 2>&1 || echo "FAIL $out"
grep -qiE 'panicked|validation error' "${out%.png}.log" && echo "PANIC/VALIDATION $out"
true
