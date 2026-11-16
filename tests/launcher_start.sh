#!/bin/sh
# The launcher's start test (docs/launcher.md, "Tests"): on a private X
# display, never the desktop's. A stub game prints a log and exits 101:
#   1. with "Start the game directly" set, the launcher starts it at once,
#      sees the failed start and opens its window on the failure screen
#      (the tail shown; checked in the launcher's log and a screenshot);
#   2. a second launcher meanwhile refuses to start;
#   3. Escape (Settings) and Enter (Play) start the stub again from the
#      window: the failure screen again.
# Usage: tests/launcher_start.sh <overbright binary> <X display, e.g. :153> <work dir> [stub]
# (Wine: [stub] = a Windows game.exe, run with --crash-test, instead of cmd.exe and a batch file)
# NFSU2_ROOT must name the game folder (the skip rule needs a usable one).
# Windows / Wine: OB_WINE=1 runs `wine <overbright.exe>`, and the stub is
# the Windows command interpreter running a batch file.
set -u
OB=$1 DISP=$2 W=$3
STUB_ARG=${4:-}
mkdir -p "$W"
W=$(cd "$W" && pwd)
rm -rf "$W/data" "$W/launcher.out" "$W"/*.png
DATA=$W/data
# Overbright's folder under the data home: its options.txt with the skip row.
APPDIR="$DATA/overbright"
mkdir -p "$APPDIR"
printf 'launcher.skip 1\n' > "$APPDIR/options.txt"
fail() { echo "FAIL: $*"; [ -n "${PID:-}" ] && kill "$PID" 2>/dev/null; exit 1; }

if [ "${OB_WINE:-}" = 1 ]; then
  BAT=$W/stub-game.bat
  printf '@echo off\r\necho stub: starting with %%*\r\necho GPU adapter: stub 1>&2\r\necho thread '"'"'main'"'"' panicked at src/stub.rs:1:1: 1>&2\r\necho no GPU adapter 1>&2\r\nexit /b 101\r\n' > "$BAT"
  if [ -n "$STUB_ARG" ]; then
    # A Windows game.exe as the stub: its deliberate panic (exit 101).
    STUB=$(winepath -w "$STUB_ARG" 2>/dev/null)
    ARGS="-- --crash-test"
  else
    STUB='C:\windows\system32\cmd.exe'
    ARGS="-- /c $(winepath -w "$BAT" 2>/dev/null)"
  fi
  RUN="wine $OB"
  # Overbright's folders: inside the prefix (Windows' rules) but isolated by XDG_DATA_HOME.
  # Overbright's folders: Wine hides XDG_* from Windows programs (renamed
  # WINE_HOST_XDG_*), so the portable override puts everything in $DATA.
  DATAW=$(winepath -w "$DATA" 2>/dev/null)
  export NFSU2_DATA_DIR="$DATAW"
  cp "$APPDIR/options.txt" "$DATA/options.txt"
  export NFSU2_ROOT="$(winepath -w "$NFSU2_ROOT" 2>/dev/null)"
else
  STUB=${STUB_ARG:-$W/stub-game}
  cat > "$W/stub-game" <<'EOF'
#!/bin/sh
echo "stub: starting with $*"
echo "GPU adapter: stub" >&2
echo "thread 'main' panicked at src/stub.rs:1:1:" >&2
echo "no GPU adapter" >&2
exit 101
EOF
  chmod +x "$W/stub-game"
  RUN=$OB
  ARGS="-- --windowed"
  export XDG_DATA_HOME="$DATA"
  # The cache home too: nothing the launcher starts touches the real one.
  export XDG_CACHE_HOME="$W/cache"
fi

# Never the desktop: no Wayland, our own X display.
env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE DISPLAY="$DISP" NFSU2_NO_PADS=1 OB_GAME_EXE="$STUB" \
  $RUN $ARGS > "$W/launcher.out" 2>&1 &
PID=$!
echo "launcher pid $PID"
wait_for() { # pattern count seconds
  i=0
  while [ "$(cat "$W/launcher.out" 2>/dev/null | grep -c "$1")" -lt "$2" ]; do
    i=$((i + 1)); [ $i -gt $(($3 * 10)) ] && return 1
    kill -0 "$PID" 2>/dev/null || return 1
    sleep 0.1
  done
}
wait_for "showing the failure screen" 1 60 || fail "no failure screen (output: $(cat "$W/launcher.out"))"
sleep 2
WIN=$(env -u WAYLAND_DISPLAY DISPLAY="$DISP" xdotool search --name '^Overbright$' | head -1)
[ -n "$WIN" ] || fail "no launcher window on $DISP"
echo "window $WIN on $DISP"
# Wine on Xvfb maps the window off screen after the game has run (it keeps
# its own idea of the place): move it into view for the screenshots.
[ "${OB_WINE:-}" = 1 ] && env -u WAYLAND_DISPLAY DISPLAY="$DISP" xdotool windowmove "$WIN" 0 0 && sleep 1
env -u WAYLAND_DISPLAY DISPLAY="$DISP" import -window root "$W/failure.png" || fail "no screenshot"
LOG=$(find "$DATA" -name 'launcher-*.log' | head -1)
[ -n "$LOG" ] || fail "no launcher log"
grep -q "exit code 101" "$LOG" || fail "log without the exit code: $(cat "$LOG")"
grep -q -E "no GPU adapter|crash test" "$LOG" || fail "log without the tail: $(cat "$LOG")"
grep -q "a failed start" "$LOG" || fail "not judged a failed start"
echo "log $LOG: ok"
# A second launcher: refused.
env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE DISPLAY="$DISP" NFSU2_NO_PADS=1 OB_GAME_EXE="$STUB" \
  $RUN > "$W/second.out" 2>&1
grep -q "already running" "$W/second.out" || fail "a second launcher started: $(cat "$W/second.out")"
echo "second launcher refused: ok"
# Settings (Escape), then Play (Enter, the main screen's default): again.
env -u WAYLAND_DISPLAY DISPLAY="$DISP" xdotool mousemove --window "$WIN" 5 5
env -u WAYLAND_DISPLAY DISPLAY="$DISP" xdotool windowfocus "$WIN" 2>/dev/null
sleep 0.5
env -u WAYLAND_DISPLAY DISPLAY="$DISP" xdotool key Escape
sleep 0.5
env -u WAYLAND_DISPLAY DISPLAY="$DISP" import -window root "$W/main.png"
env -u WAYLAND_DISPLAY DISPLAY="$DISP" xdotool key Return
wait_for "showing the failure screen" 2 60 || fail "Play from the window did not show the failure again"
echo "play from the window: failure screen again: ok"
sleep 1
[ "${OB_WINE:-}" = 1 ] && env -u WAYLAND_DISPLAY DISPLAY="$DISP" xdotool windowmove "$WIN" 0 0 && sleep 1
env -u WAYLAND_DISPLAY DISPLAY="$DISP" import -window root "$W/failure2.png"
kill "$PID" 2>/dev/null
wait "$PID" 2>/dev/null
# Wine: stop this prefix's own server (WINEPREFIX is the caller's private one).
[ "${OB_WINE:-}" = 1 ] && wineserver -k 2>/dev/null
echo "PASS"
