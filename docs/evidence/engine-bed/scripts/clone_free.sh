#!/bin/bash
# clone_free.sh OUT KMH GEAR FRAMES [SOLO] [HOLD]: the clone on the highway straight (AT, default 352.6 -1445 36.6,
# heading 107), rolling at KMH in GEAR (1 = first) with the throttle held (HOLD=1: standing, no gas), from the save
# SAVE (the original's run's save: same options, manual gearbox); offline mix OUT/clone.wav, event log OUT/events.tsv,
# stems OUT/stems.tsv (split names), optional NFSU2_AUDIO_SOLO list. GAME = the clone built with the developer harness.
OUT=$1; V=$2; G=$3; F=$4; SOLO=$5; HOLD=$6
mkdir -p "$OUT"; rm -f "$OUT/stems.tsv" "$OUT/events.tsv"
env ${SAVE:+NFSU2_SHOT_SAVE=$SAVE} NFSU2_AUDIO_WAV="$OUT/clone.wav" NFSU2_EVENT_LOG="$OUT/events.tsv" \
  NFSU2_AUDIO_STEMS="$OUT/stems.tsv" NFSU2_AUDIO_STEMS_SPLIT=1 ${SOLO:+NFSU2_AUDIO_SOLO=$SOLO} ${HOLD:+NFSU2_SHOT_HOLD=1} \
  NFSU2_PROP_LOG=1 NFSU2_SHOT_AT=${AT:-352.6,-1445.0,36.6,107} ${V:+NFSU2_FX_SPEED=$V} ${G:+NFSU2_FX_GEAR=$G} NFSU2_FX_FRAMES=$F \
  timeout 900 "$GAME" --shot explore "$OUT/shot.png" > "$OUT/clone.log" 2>&1
