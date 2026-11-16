#!/bin/bash
# motion.sh BIN OUTDIR MODE VIEW [N] [SIZE]: N consecutive ticks (1/60 s) from clock 97.0 at VIEW.
W=${WORK:?the work dir}
G=$1; O=$2; m=$3; v=$4; n=${5:-12}; mkdir -p $O
for k in $(seq 0 $((n-1))); do
  c=$(python3 -c "print(f'{97 + $k/60:.6f}')")
  t=$m; case $m in *";"*) t=taa;; esac; f=$O/${v}_${t}_$(printf %02d $k).png
  [ -s $f ] || SIZE=${6:-1920x1080} $W/shot.sh $G $f "$m" $v $c
done
