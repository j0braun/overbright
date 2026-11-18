#!/bin/bash
# The launcher's first run on a fresh machine (docs/launcher.md, "Tests"), end to end with the
# shipping `game` and `overbright` side by side, on a private X display (never the desktop's):
# an empty HOME, no NFSU2_ROOT, no saved game folder, nothing but PATH in the environment.
#   a. `game --probe` exits 0 with the adapter, the modes and the first_run lines (no game folder needed);
#   b. the launcher opens on the first-run screen;
#   c. a wrong folder is refused in plain words ("Use this folder" off);
#   e. SPEED2.EXE is not needed: a folder whose SPEED2.EXE is a copy-protected one (a synthetic
#      header with SafeDisc's sections) is accepted with an information line; a folder without
#      SPEED2.EXE is accepted and used (the probe runs again with that folder);
#   p. the prepare step (docs/prepare.md) runs by itself once the folder is used, behind the main screen
#      (no screen of its own, the line beside Play says what it does) and is not started again;
#      "Prepare again" opens its screen and runs to its end (Done, Continue), the stamp is in the cache
#      folder and `game --probe` says prepared=1;
#   d. a folder whose files are all in another case (symlinks to the real game, names case-swapped)
#      is accepted and used: the probe runs again with it;
#   f. the Display / Resolution / Graphics rows are live with the probe's first-run values, the
#      same as `game --probe` prints for that folder;
#   g. nothing is written before Play (no options.txt); then Display changed and Play: options.txt
#      holds the first-run values + the choice;
#   h. Play starts the game; its log file appears in the logs folder and reports its start
#      (`game start: done`); the game window is closed as a player would (WM_DELETE_WINDOW); the
#      game ends with 0 and the launcher logs "a normal end" and exits 0;
#   i. a second start opens straight on the main screen with the saved folder.
# The screens are read from the launcher's own log (RUST_LOG launcher::screen=debug: each screen's
# texts, its controls' ids, "(off)" and boxes) and kept as screenshots in <work dir>.
#
# Usage: tests/launcher_fresh.sh <dir holding game and overbright> <X display | auto> <work dir>
# NFSU2_ROOT must name the player's game folder (the trees are symlinks to it; nothing is copied:
# one tree has no SPEED2.EXE, one a small synthetic stand-in made in <work dir>). "auto" starts a private Xvfb
# (-displayfd) and stops it by PID. RADV on Xvfb has no DRI3: the game presents through Mesa's
# software path (MESA_VK_WSI_DEBUG=sw), which only this test sets.
# Last line: "FRESH: PASS" or "FRESH: FAIL (<step>)". Exit 0 only on PASS.
set -u -o pipefail
BIN=$(cd "${1:?bin dir}" && pwd) DISP=${2:?display} W=${3:?work dir}
for t in xdotool python3; do command -v $t > /dev/null || { echo "FRESH: FAIL (no $t)"; exit 1; }; done
[ -x "$BIN/game" ] && [ -x "$BIN/overbright" ] || { echo "FRESH: FAIL (no game / overbright in $BIN)"; exit 1; }
[ -d "${NFSU2_ROOT:-/nonexistent}" ] || { echo "FRESH: FAIL (NFSU2_ROOT)"; exit 1; }
mkdir -p "$W"; W=$(cd "$W" && pwd)
rm -rf "$W/home" "$W/trees" "$W"/*.out "$W"/*.png "$W"/*.txt
mkdir -p "$W/home" "$W/trees"
H=$W/home
DATA=$H/.local/share/overbright
XPID="" LPID="" GPID=""
stop() { for p in $GPID $LPID $XPID; do kill "$p" 2> /dev/null && wait "$p" 2> /dev/null; done; }
fail() { echo "FAIL: $*"; stop; echo "FRESH: FAIL ($STEP)"; exit 1; }
ok() { echo "ok    $STEP: $*"; }

# ---- the display ------------------------------------------------------------------------------
if [ "$DISP" = auto ]; then
  command -v Xvfb > /dev/null || { echo "FRESH: FAIL (no Xvfb)"; exit 1; }
  exec 9> "$W/xvfb.display"
  Xvfb -displayfd 9 -screen 0 1920x1080x24 -nolisten tcp > "$W/xvfb.log" 2>&1 &
  XPID=$!; exec 9>&-
  for _ in $(seq 100); do [ -s "$W/xvfb.display" ] && break; sleep 0.1; done
  DISP=:$(tr -dc '0-9' < "$W/xvfb.display")
fi
X() { env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE DISPLAY="$DISP" "$@"; }
[ -z "$(X xdotool search --onlyvisible --name . 2> /dev/null)" ] || { echo "FRESH: FAIL (display $DISP already has windows)"; stop; exit 1; }
shot() { command -v import > /dev/null && X import -window root "$W/$1.png" 2> /dev/null; }
# The fresh machine: nothing but these.
run() {
  (cd "$H" && exec env -i HOME="$H" PATH=/usr/bin:/bin DISPLAY="$DISP" LANG=C.UTF-8 NFSU2_NO_PADS=1 NFSU2_AUDIO_HOST=none \
    MESA_VK_WSI_DEBUG=sw RUST_LOG="info,launcher::screen=debug" "$@")
}
# The launcher in the background, its output in $L; LPID is its own PID (stopped by it).
launch() {
  (cd "$H" && exec env -i HOME="$H" PATH=/usr/bin:/bin DISPLAY="$DISP" LANG=C.UTF-8 NFSU2_NO_PADS=1 NFSU2_AUDIO_HOST=none \
    MESA_VK_WSI_DEBUG=sw RUST_LOG="info,launcher::screen=debug" "$BIN/overbright" > "$L" 2>&1) &
  LPID=$!
}

# ---- the folders ------------------------------------------------------------------------------
# a: the game with every name case-swapped (symlinks); b: the same without SPEED2.EXE; c: the same,
# SPEED2.EXE a synthetic copy-protected header (SafeDisc's section names, nothing of the game).
python3 -I - "$NFSU2_ROOT" "$W/trees" << 'EOF' || fail "trees"
import os, sys
src, top = sys.argv[1], sys.argv[2]
for t in "abc":
    dst = os.path.join(top, "game-" + t)
    for d, dirs, files in os.walk(src):
        rel = os.path.relpath(d, src)
        out = dst if rel == "." else os.path.join(dst, *[p.swapcase() for p in rel.split(os.sep)])
        os.makedirs(out, exist_ok=True)
        for f in files:
            os.symlink(os.path.join(d, f), os.path.join(out, f.swapcase()))
for t in "bc":
    for f in os.listdir(os.path.join(top, "game-" + t)):
        if f.upper() == "SPEED2.EXE":
            os.remove(os.path.join(top, "game-" + t, f))
h = bytearray(0x400)
h[:2] = b"MZ"
h[0x3C:0x40] = (0x80).to_bytes(4, "little")
h[0x80:0x84] = b"PE\0\0"
h[0x86:0x88] = (2).to_bytes(2, "little")
h[0x94:0x96] = (0xE0).to_bytes(2, "little")
s = 0x80 + 24 + 0xE0
h[s:s + 7] = b"stxt774"
h[s + 40:s + 47] = b"stxt371"
open(os.path.join(top, "game-c", "speed2.exe"), "wb").write(h)
EOF
A=$W/trees/game-a B=$W/trees/game-b C=$W/trees/game-c

# ---- a. the probe -----------------------------------------------------------------------------
STEP=a
run "$BIN/game" --probe > "$W/probe.out" 2> "$W/probe.err"; rc=$?
[ $rc = 0 ] || fail "game --probe rc $rc: $(tail -3 "$W/probe.err")"
grep -q '^adapter.name=.' "$W/probe.out" && grep -q '^modes=[0-9]' "$W/probe.out" && grep -q '^monitor.exclusive=' "$W/probe.out" \
  || fail "probe output without adapter / modes / monitor: $(cat "$W/probe.out")"
grep -q '^first_run.resolution=' "$W/probe.out" || fail "no first_run lines: $(cat "$W/probe.out")"
grep -q '^ok=1' "$W/probe.out" || fail "probe not ok"
[ -e "$DATA/options.txt" ] && fail "the probe wrote options.txt"
ok "rc 0, $(grep '^adapter.name=' "$W/probe.out" | cut -d= -f2-), first_run lines"
# What the probe says once the folder is known (step f compares the launcher's rows with it).
run env NFSU2_ROOT="$A" "$BIN/game" --probe > "$W/probe-a.out" 2>&1
FR_RES=$(sed -n 's/^first_run.resolution=//p' "$W/probe-a.out")
FR_DETAIL=$(sed -n 's/^first_run.detail=//p' "$W/probe-a.out")
FR_CUSTOM=$(sed -n 's/^first_run.custom=//p' "$W/probe-a.out")
[ -n "$FR_RES" ] && [ -n "$FR_DETAIL" ] || fail "game --probe with NFSU2_ROOT=<folder a> gives no first_run lines"

# ---- the launcher -----------------------------------------------------------------------------
L=$W/launcher.out
launch
# wait_for <extended regex> <count> <seconds>: the launcher's output has the pattern count times.
wait_for() {
  local i=0
  while [ "$(grep -cE "$1" "$L" 2> /dev/null || true)" -lt "$2" ]; do
    i=$((i + 1)); [ $i -gt $(($3 * 10)) ] && return 1
    sleep 0.1
  done
}
# screen <name> <n>: the n-th time the log shows that screen; its block in $W/screen.txt.
screen() {
  wait_for "screen $1\$" "$2" 30 || return 1
  sleep 0.5
  awk -v want="screen $1" -v n="$2" '
    /launcher::screen\]/ { inb = 0 }
    /launcher::screen\] screen / { k += ($NF == substr(want, 8)); if (k == n && $NF == substr(want, 8)) inb = 1; next }
    inb && /^ +\[/ { sub(/^ +/, "  "); print }' "$L" > "$W/screen.txt"
  [ -s "$W/screen.txt" ]
}
has() { grep -qF -- "$1" "$W/screen.txt"; }
# The control's box (x y w h) from the screen's trace.
box() { grep -E "^  \[[a-z]+ $1( \(off\))? @" "$W/screen.txt" | head -1 | sed -E 's/.*@([0-9-]+),([0-9-]+) ([0-9]+)x([0-9]+)\].*/\1 \2 \3 \4/'; }
click() { # click <id> [right]: the control's middle, or its right arrow (a cycle)
  local b side=${2:-}; b=$(box "$1"); [ -n "$b" ] || return 1
  set -- $b
  local x=$(($1 + $3 / 2))
  [ "$side" = right ] && x=$(($1 + $3 - $4 / 2))
  X xdotool mousemove --window "$WIN" $x $(($2 + $4 / 2)) click 1
}
typepath() { # the first-run screen's path field: emptied, the path typed, Enter (Check)
  X xdotool key End
  X xdotool key --repeat 300 --delay 1 BackSpace
  X xdotool type --delay 2 "$1"
  X xdotool key Return
}

# ---- b. the first-run screen ------------------------------------------------------------------
STEP=b
screen first-run 1 || fail "no first-run screen: $(tail -5 "$L")"
WIN=$(X xdotool search --onlyvisible --name '^Overbright$' | head -1)
[ -n "$WIN" ] || fail "no launcher window on $DISP"
X xdotool windowfocus "$WIN" 2> /dev/null; sleep 0.3
has "No game folder was found" || fail "first-run screen without the no-folder line: $(cat "$W/screen.txt")"
grep -qE '^  \[field path ' "$W/screen.txt" || fail "no path field"
shot b-first-run
# The window's icon (the exe icon's drawing, platform::icon: 64 x 64).
ICON="no xprop"
if command -v xprop > /dev/null; then
  icon=$(X xprop -id "$WIN" _NET_WM_ICON 2> /dev/null)
  echo "${icon:0:300}" | grep -qE 'Icon \(64 x 64\)|= 64, 64,' || fail "the launcher window has no icon"
  ICON="icon 64x64"
fi
ok "window $WIN on $DISP ($ICON), first-run screen, no folder found"

# ---- c. a wrong folder ------------------------------------------------------------------------
STEP=c
typepath "$H"
screen check 1 || fail "no check screen"
grep -qE '^  \[button use \(off\)' "$W/screen.txt" || fail "a wrong folder can be used: $(cat "$W/screen.txt")"
has "this is not a complete game folder" && has "Overbright cannot use this folder" || fail "no plain-words refusal: $(cat "$W/screen.txt")"
shot c-wrong-folder
ok "refused: $(grep -F '[error]' "$W/screen.txt" | head -1 | sed 's/^  \[error\] //' | cut -c1-90)"
X xdotool key Escape

# ---- e. SPEED2.EXE not needed: a copy-protected one (information only), none at all (used) -------
STEP=e
screen first-run 2 || fail "Back did not return to the first-run screen"
typepath "$C"
screen check 2 || fail "no check screen for the copy-protected executable"
has "SPEED2.EXE is the copy-protected retail executable; Overbright does not need it" || fail "no information line: $(cat "$W/screen.txt")"
grep -qE '^  \[button use @' "$W/screen.txt" || fail "a folder with the copy-protected SPEED2.EXE cannot be used: $(cat "$W/screen.txt")"
grep -qF '[error]' "$W/screen.txt" && fail "an error line for the copy-protected SPEED2.EXE: $(cat "$W/screen.txt")"
shot e-protected-exe
ok "copy-protected SPEED2.EXE: information line, usable"
X xdotool key Escape
screen first-run 3 || fail "Back did not return to the first-run screen"
typepath "$B"
screen check 3 || fail "no check screen for the folder without SPEED2.EXE"
grep -qE '^  \[button use @' "$W/screen.txt" || fail "the folder without SPEED2.EXE cannot be used: $(cat "$W/screen.txt")"
grep -qF "SPEED2" "$W/screen.txt" && fail "SPEED2.EXE named for a folder without it: $(cat "$W/screen.txt")"
shot e-no-exe
X xdotool key Return   # Use this folder (focused)
screen main 1 || fail "Use did not open the main screen"
wait_for "probe: the game folder changed" 1 10 || fail "no new probe for the chosen folder"
wait_for "first-run values [0-9]" 1 30 || fail "the new probe gave no first-run values: $(grep 'probe:' "$L")"
ok "no SPEED2.EXE: used; probe run again with it ($(grep -oE 'first-run values [^;]*$' "$L" | tail -1))"

# ---- p. the prepare step: by itself behind the main screen once the folder is used, then "Prepare again" to its end ----
STEP=p
wait_for "prepare: started" 1 30 || fail "the prepare step did not start by itself after the folder was used: $(tail -5 "$L")"
grep -q "screen prepare$" "$L" && fail "the prepare step opened its screen by itself"
# The main screen's line beside Play while it runs (it may already be done on a fast machine).
seen=""
for _ in $(seq 40); do
  n=$(grep -c "screen main$" "$L"); screen main "$n" || fail "no main screen while preparing"
  has "in the background" && { seen=1; break; }
  grep -qE "prepare: (Done|Failed)" "$L" && break
  sleep 0.1
done
shot p-prepare-background
wait_for "prepare: (Done|Failed)" 1 300 || fail "the prepare run did not end: $(grep 'prepare:' "$L" | tail -3)"
grep -q "prepare: Done" "$L" || fail "the prepare run failed: $(grep 'prepare:' "$L" | tail -3)"
sleep 1
n=$(grep -c "screen main$" "$L"); screen main "$n" || fail "no main screen after the run"
has "in the background" && fail "the main screen still says it is preparing: $(cat "$W/screen.txt")"
[ "$(grep -c 'prepare: started' "$L")" = 1 ] || fail "the prepare step was started a second time"
click prepare || fail "no Prepare again button: $(cat "$W/screen.txt")"
wait_for "prepare: started" 2 10 || fail "Prepare again did not start a run"
wait_for "prepare: Done" 2 300 || fail "the prepare run did not finish: $(grep 'prepare:' "$L" | tail -3)"
# The launcher draws the screen again a frame after it logs the Done line (later on a busy machine): read the
# newest prepare screen until it shows the end, for up to about 10 s.
done_ok=""
for _ in $(seq 15); do
  n=$(grep -c "screen prepare$" "$L"); screen prepare "$n" || fail "no prepare screen after the run"
  has "Done." && grep -qE '^  \[button continue @' "$W/screen.txt" && { done_ok=1; break; }
  sleep 0.2
done
[ -n "$done_ok" ] || fail "no Done line and Continue: $(cat "$W/screen.txt")"
shot p-prepare-done
[ -s "$H/.cache/overbright/prepared.txt" ] || fail "no prepared.txt in the cache folder"
m0=$(grep -c "screen main$" "$L")
X xdotool key Return   # Continue
screen main $((m0 + 1)) || fail "Continue did not return to the main screen"
PROBE2=$(cd "$H" && env -i HOME="$H" PATH=/usr/bin:/bin NFSU2_ROOT="$B" "$BIN/game" --probe 2> /dev/null | grep '^prepared=')
[ "$PROBE2" = "prepared=1" ] || fail "game --probe after the prepare run says ${PROBE2:-nothing}"
ok "ran by itself behind the main screen (${seen:+its line shown, }not started again), Prepare again ran to its end ($(grep -c 'prepare: Running' "$L") steps logged), stamp written, probe: $PROBE2"

# ---- d. every name in another case ------------------------------------------------------------
STEP=d
click change || fail "no Change button"
screen first-run 4 || fail "Change did not open the first-run screen"
typepath "$A"
screen check 4 || fail "no check screen for the case-swapped folder"
has "recognised" && has "All 9 files Overbright needs are present" || fail "case-swapped folder not recognised: $(cat "$W/screen.txt")"
grep -qE '^  \[button use @' "$W/screen.txt" || fail "case-swapped folder cannot be used"
grep -qiF "dinput8.dll" "$W/screen.txt" || fail "the mod files in another case not found: $(cat "$W/screen.txt")"
shot d-case-swapped
VLINE=$(grep -F 'recognised' "$W/screen.txt" | sed 's/^  \[ok\] //')
m0=$(grep -c "screen main$" "$L")
X xdotool key Return
screen main $((m0 + 1)) || fail "Use did not open the main screen"
wait_for "probe: the game folder changed" 2 10 || fail "no new probe for the second folder"
ok "accepted and used: $VLINE"

# ---- f. the display rows, live with the first-run values --------------------------------------
STEP=f
wait_for "first-run values [0-9]" 2 30 || fail "the second probe gave no first-run values"
grep 'first-run values' "$L" | tail -1 | grep -qF "first-run values ${FR_RES} detail ${FR_DETAIL}" \
  || fail "the launcher's first-run values differ from game --probe's ($FR_RES detail $FR_DETAIL): $(grep 'first-run values' "$L" | tail -1)"
sleep 0.5
n=$(grep -c "screen main$" "$L"); screen main "$n" || fail "no main screen after the probe"
for c in display resolution graphics; do
  grep -qE "^  \[cycle $c @" "$W/screen.txt" || fail "the $c row is not live: $(cat "$W/screen.txt")"
done
grep -qE "^  \[cycle resolution @.*\] ${FR_RES/x/ × }" "$W/screen.txt" || fail "Resolution is not the first-run $FR_RES: $(grep resolution "$W/screen.txt")"
has "$A" || fail "the Game row does not show the folder"
shot f-main
ok "Display, Resolution ($FR_RES), Graphics live; first-run detail $FR_DETAIL as game --probe prints"

# ---- g. nothing written before Play; then the first write -------------------------------------
STEP=g
[ -e "$DATA/options.txt" ] && fail "options.txt written before Play: $(head -3 "$DATA/options.txt")"
[ -e "$DATA/game-root.txt" ] || fail "the folder was not kept (game-root.txt)"
grep -qF "$A" "$DATA/game-root.txt" || fail "game-root.txt does not hold the used folder"
WANT_FS=$(sed -n 's/^first_run.fullscreen=//p' "$W/probe-a.out")
click display right || fail "no Display row"
sleep 0.5
n=$(grep -c "screen main$" "$L"); screen main "$n"
NOW_FS=$((1 - WANT_FS))
want_word=Windowed; [ $NOW_FS = 1 ] && want_word="Full screen"
grep -qE "^  \[cycle display @.*\] $want_word" "$W/screen.txt" || fail "Display did not change to $want_word: $(grep display "$W/screen.txt")"
[ -e "$DATA/options.txt" ] && fail "options.txt written on a change (before Play)"
ok "no options.txt before Play; Display changed (fullscreen $WANT_FS -> $NOW_FS)"

# ---- h. Play: the game starts, is closed, the launcher reports a normal end --------------------
STEP=h
click play || fail "no Play button"
wait_for "game started \(pid [0-9]+\)" 1 30 || fail "Play did not start the game: $(tail -5 "$L")"
GPID=$(grep -oE 'game started \(pid [0-9]+' "$L" | grep -oE '[0-9]+$' | tail -1)
[ -e "$DATA/options.txt" ] || fail "no options.txt after Play"
cp "$DATA/options.txt" "$W/options-first.txt"
# The first write: the first-run values + the choice (the trap test pins the whole text).
grep -qx "fullscreen $NOW_FS" "$W/options-first.txt" || fail "options.txt without the choice: $(grep fullscreen "$W/options-first.txt")"
grep -qx "detail $FR_DETAIL" "$W/options-first.txt" || fail "options.txt detail is not the first-run $FR_DETAIL: $(grep '^detail' "$W/options-first.txt")"
got=$(for i in $(seq 0 21); do sed -n "s/^custom.$i //p" "$W/options-first.txt"; done | paste -sd,)
[ "$got" = "$FR_CUSTOM" ] || fail "options.txt custom values $got, not the first-run $FR_CUSTOM"
ok "options.txt written at Play: first-run detail $FR_DETAIL and custom values, fullscreen $NOW_FS"
GLOG=""
for _ in $(seq 1800); do
  GLOG=$(ls "$DATA"/logs/game-*.log 2> /dev/null | head -1)
  [ -n "$GLOG" ] && grep -q "game start: done" "$GLOG" && break
  kill -0 "$GPID" 2> /dev/null || break
  sleep 0.1
done
[ -n "$GLOG" ] || fail "no game log in $DATA/logs"
grep -q "game start: done" "$GLOG" || fail "the game did not report its start: $(tail -5 "$GLOG")"
# A check is never heard: the game was started without a sound host (NFSU2_AUDIO_HOST=none) and opened no device.
grep -aqE 'audio: [A-Za-z0-9 ]+ output at' "$GLOG" "$L" && fail "the game opened a sound device: $(grep -ahE 'audio: ' "$GLOG" "$L" | head -2)"
tr '\0' '\n' < "/proc/$GPID/environ" | grep -qx "NFSU2_ROOT=$A" || fail "the game was not given the chosen folder"
sleep 2
GWIN=$(X xdotool search --onlyvisible --pid "$GPID" 2> /dev/null | tail -1)
[ -n "$GWIN" ] || fail "no game window"
shot h-game
# The player closes the window (WM_DELETE_WINDOW, as a window manager's close button sends it).
X python3 -I - "$GWIN" << 'EOF' || fail "closing the game window"
import ctypes, ctypes.util, sys
x = ctypes.CDLL(ctypes.util.find_library("X11"))
x.XOpenDisplay.restype = ctypes.c_void_p
x.XOpenDisplay.argtypes = [ctypes.c_char_p]
x.XInternAtom.restype = ctypes.c_ulong
x.XInternAtom.argtypes = [ctypes.c_void_p, ctypes.c_char_p, ctypes.c_int]
x.XSendEvent.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_long, ctypes.c_void_p]
x.XFlush.argtypes = x.XCloseDisplay.argtypes = [ctypes.c_void_p]
class Msg(ctypes.Structure):
    _fields_ = [("type", ctypes.c_int), ("serial", ctypes.c_ulong), ("send_event", ctypes.c_int), ("display", ctypes.c_void_p),
                ("window", ctypes.c_ulong), ("message_type", ctypes.c_ulong), ("format", ctypes.c_int), ("data", ctypes.c_long * 5)]
class Ev(ctypes.Union):
    _fields_ = [("xclient", Msg), ("pad", ctypes.c_long * 24)]
d = x.XOpenDisplay(None)
w = int(sys.argv[1])
e = Ev()
e.xclient.type, e.xclient.window, e.xclient.format = 33, w, 32
e.xclient.message_type = x.XInternAtom(d, b"WM_PROTOCOLS", 0)
e.xclient.data[0] = x.XInternAtom(d, b"WM_DELETE_WINDOW", 0)
x.XSendEvent(d, w, 0, 0, ctypes.byref(e))
x.XFlush(d)
x.XCloseDisplay(d)
EOF
for _ in $(seq 300); do kill -0 "$LPID" 2> /dev/null || break; sleep 0.1; done
kill -0 "$LPID" 2> /dev/null && fail "the launcher did not end after the game was closed"
wait "$LPID"; rc=$?; LPID="" GPID=""
[ $rc = 0 ] || fail "launcher exit $rc: $(tail -5 "$L")"
LLOG=$(ls "$DATA"/logs/launcher-*.log 2> /dev/null | head -1)
[ -n "$LLOG" ] || fail "no launcher log"
grep -q "exit code 0 (a normal end)" "$LLOG" || fail "the launcher did not report a normal end: $(cat "$LLOG")"
ok "game started (log $(basename "$GLOG"): start done), closed; $(tail -1 "$LLOG" | cut -c1-70); launcher exit 0"

# ---- i. the second start: the main screen with the saved folder --------------------------------
STEP=i
: > "$L"
launch
screen main 1 || fail "the second start did not open on the main screen: $(head -20 "$L")"
has "$A" || fail "the saved folder is not shown: $(cat "$W/screen.txt")"
grep -q "screen first-run" "$L" && fail "the first-run screen was shown again"
WIN=$(X xdotool search --onlyvisible --name '^Overbright$' | head -1)
shot i-second-start
X xdotool windowfocus "$WIN" 2> /dev/null; sleep 0.3
X xdotool key Escape
for _ in $(seq 100); do kill -0 "$LPID" 2> /dev/null || break; sleep 0.1; done
wait "$LPID"; rc=$?; LPID=""
[ $rc = 0 ] || fail "launcher exit $rc after Escape"
ok "main screen, folder $(basename "$A"), Escape quits (exit 0)"
stop
echo "FRESH: PASS"
