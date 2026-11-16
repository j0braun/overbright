#!/bin/bash
# measure.sh <game binary> <tag> [extra env...]: start the shipping game on :180 in a sandbox HOME, WORK: the work folder.
# wait for "game start: done", wait $WAIT s, close the window, time until the process ends.
set -u
BIN=$1 TAG=$2; shift 2
W=$WORK/$TAG; rm -rf "$W"; mkdir -p "$W/home"
. scripts/local.env
H=$W/home DATA=$H/.local/share/overbright
(cd "$H" && exec env -i HOME="$H" PATH=/usr/bin:/bin DISPLAY=:180 LANG=C.UTF-8 NFSU2_NO_PADS=1 NFSU2_ROOT="$NFSU2_ROOT" \
  MESA_VK_WSI_DEBUG=sw RUST_LOG=info "$@" ${STRACE:+strace -f -tt -o $W/strace.txt} "$BIN" > "$W/out.txt" 2>&1) &
GPID=$!
echo "pid $GPID"
for _ in $(seq 1800); do
  GLOG=$(ls "$DATA"/logs/game-*.log 2> /dev/null | head -1)
  [ -n "$GLOG" ] && grep -q "game start: done" "$GLOG" && break
  kill -0 $GPID 2>/dev/null || break; sleep 0.1
done
sleep ${WAIT:-2}
GWIN=$(env -u WAYLAND_DISPLAY DISPLAY=:180 xdotool search --onlyvisible --name . 2>/dev/null | tail -1)
echo "win $GWIN"
t0=$(date +%s.%N)
env -u WAYLAND_DISPLAY DISPLAY=:180 python3 -I $WORK/close.py "$GWIN"
echo "closed at $(date +%H:%M:%S.%N)"
[ -n "${STACKS:-}" ] && (for t in 0.3 0.5 0.5 0.5 0.5 0.5 0.5 0.5 1 1 1 1 1 1; do sleep $t; for t in /proc/$GPID/task/*; do echo "$(cat $t/comm) $(cut -d" " -f3,14,15 $t/stat) $(cat $t/wchan)"; done > $W/stack-$(date +%s.%N).txt 2>&1; done) &
for _ in $(seq 600); do kill -0 $GPID 2>/dev/null || break; sleep 0.05; done
t1=$(date +%s.%N)
kill -0 $GPID 2>/dev/null && { echo "STILL RUNNING after 30 s"; kill $GPID; }
wait $GPID; echo "rc $? exit after $(echo "$t1 - $t0" | bc) s"
