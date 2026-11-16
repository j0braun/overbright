#!/bin/bash
# win144.sh GAME OUTDIR: the game WINDOWED on the private Xvfb :173 (sandboxed,
# own XDG_DATA_HOME with a finished-career save: SAVE=file PROFILE=its profile name), into free roam, the gas held 20 s,
# NFSU2_FRAME_CAP=144, NFSU2_FRAME_TRACE per drawn frame. Stops only its PIDs.
game=$1; out=$(realpath -m "$2"); n=173
mkdir -p "$out/xdg/NFS Underground 2/$PROFILE"
cp "$SAVE" "$out/xdg/NFS Underground 2/$PROFILE/$PROFILE"
. scripts/local.env 2>/dev/null
Xvfb :$n -screen 0 1920x1080x24 -nolisten tcp > "$out/xvfb.log" 2>&1 & xvfb=$!
echo "xvfb pid $xvfb" > "$out/pids"
for _ in $(seq 50); do [ -e /tmp/.X11-unix/X$n ] && break; sleep 0.1; done
env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE -u NFSU2_HEADLESS_ONLY -u PULSE_SERVER -u DBUS_SESSION_BUS_ADDRESS \
    DISPLAY=:$n XDG_DATA_HOME="$out/xdg" MESA_VK_WSI_DEBUG=sw NFSU2_FRAME_CAP=144 NFSU2_FRAME_TRACE="$out/trace.txt" \
    NFSU2_PREVIEW_LOG=1 RUST_LOG=info RUST_BACKTRACE=1 \
    bwrap --die-with-parent --dev-bind / / --tmpfs /dev/snd --tmpfs "${XDG_RUNTIME_DIR:-/run/user/$UID}" \
      --tmpfs /tmp/.X11-unix --bind /tmp/.X11-unix/X$n /tmp/.X11-unix/X$n \
      "$game" --no-boot > "$out/game.log" 2>&1 & gpid=$!
echo "game pid $gpid" >> "$out/pids"
trap 'kill $gpid 2>/dev/null; sleep 1; kill $xvfb 2>/dev/null' EXIT
export DISPLAY=:$n
win() { xdotool search --onlyvisible --name . 2>/dev/null | head -1; }
for _ in $(seq 600); do [ -n "$(win)" ] && break; sleep 0.1; done
echo "window: $(win) $(xdotool getwindowname "$(win)" 2>/dev/null)" | tee -a "$out/pids"
xdotool mousemove 1910 1070; xdotool windowfocus "$(win)" 2>/dev/null
key() { for k in "$@"; do xdotool keydown "$k"; sleep 0.08; xdotool keyup "$k"; sleep 0.4; done; }
screen() { grep -a "screen:" "$out/game.log" | tail -1 | sed 's/.*screen: //'; }
until_screen() { for _ in $(seq ${3:-20}); do screen | grep -qE "$2" && return 0; key $1; sleep 2; done; screen | grep -qE "$2"; }
sleep 5
until_screen Return '^UI_Main$' 40 || { echo "no main menu (at $(screen))"; exit 1; }
key Left Return; sleep 3
until_screen Return '^UI_CareerCrib$' 15 || { echo "no crib (at $(screen))"; exit 1; }
sleep 2; key Return; sleep 2
screen | grep -q "Explore Mode" || { echo "no explore question (at $(screen))"; exit 1; }
key Left Return
for _ in $(seq 120); do screen | grep -q '^explore$' && break; sleep 0.5; done
echo "at: $(screen)"; sleep 4
: > "$out/trace.mark"; wc -l < "$out/trace.txt" > "$out/trace.start"
xdotool keydown Down; sleep 8; xdotool keyup Down; xdotool keydown Up; xdotool keydown Left; sleep 2; xdotool keyup Left; sleep 12; xdotool keyup Up
wc -l < "$out/trace.txt" > "$out/trace.end"
import -window root "$out/end.png" 2>/dev/null
echo "panics: $(grep -c panicked "$out/game.log")"
