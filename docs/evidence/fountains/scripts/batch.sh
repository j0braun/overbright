#!/bin/bash
# batch.sh BIN OUTDIR [SRC_DIR]: the fountain set. 1080p: every view x mode x (night 97, rain 162);
# 4K: casino10/30/80/low night + casino30 rain for orig w2 w3 max player. Skips existing PNGs.
W=${WORK:?the work dir}
G=$1; O=$2; mkdir -p $O
X=(); [ -n "${3:-}" ] && X=(NFSU2_WATER_SRC_DIR=$3)
one() { # size view mode clock
  local n=$O/${2}_${3}_c${4}_${1}.png
  [ -s $n ] && return
  SIZE=$1 $W/shot.sh $G $n $3 $2 $4 "${X[@]}"
}
for c in 97 162; do for v in casino10 casino30 casino80 casinolow C B N P; do for m in orig w1 w2 w3 extreme max player; do
  one 1920x1080 $v $m $c
done; done; done
for v in casino10 casino30 casino80 casinolow; do for m in orig w2 w3 max player; do one 3840x2160 $v $m 97; done; done
for m in orig w2 w3 max player; do one 3840x2160 casino30 $m 162; done
echo done
