#!/bin/bash
# The in-game World Map's GPS through REAL window events (docs/formats.md
# "In-game World Map", "No free destination"): the game runs windowed on a
# private Xvfb display, sandboxed (bwrap: own PID namespace, no /dev/snd,
# no $XDG_RUNTIME_DIR, no Wayland; its own XDG_DATA_HOME and NFSU2_CACHE_DIR, a copy of a
# native save as the only profile), and xdotool presses the keys and
# clicks there. Nothing reaches the desktop or the player's saves.
#
#   scripts/map_xtest.sh [SAVE:NAME ...]
#
# Default saves (private fixtures; without the private set it exits 3 with a SKIP line):
# tests/fixtures/native_saves/stage1_win_streetx.sav:NAME
# (a stage with events on the map) and the finished-career save (stage 6,
# a finished career: shops, car lot and garage only). Env: BIN (default
# $CARGO_TARGET_DIR/release/game or target/release/game), NFSU2_ROOT, OUT
# (logs and screenshots; default a new temp dir). Needs Xvfb, xdotool,
# bwrap and ImageMagick's import. NFSU2_INPUT_LOG's tag carries the map's
# selection (icon, target hash), the GPS and the focused legend row, and in
# free roam the GPS target and state; NFSU2_INPUT_PROBE "map" gives each
# marker's window point.
#
# Checked per save, as the original does under Wine: Career →
# Return to Explore Mode; M opens the map with the GPS off; Right / Left
# step the selection; Down focuses a legend row (its group's first marker
# selected); Enter hides that group, Enter again shows it; C asks "engage the GPS system?" with No focused, so Enter
# answers No and the map stays; C, Left, Enter engages on the selected
# marker and the map closes (free roam with the GPS on that target); M
# again: the selection on the target, Right does nothing with the GPS on;
# C: "Deactivating GPS", Enter clears it and the map stays; Right steps
# again; a click on another marker asks, a click on Yes engages on it; the
# help bar's (De)activate GPS asks too; Esc closes the map.
set -u
cd "$(dirname "$0")/.."
BIN=${BIN:-${CARGO_TARGET_DIR:-target}/release/game}
. scripts/lib/common.sh
ck_local_env
ck_need_root
OUT=${OUT:-$(mktemp -d "${TMPDIR:-/tmp}/map_xtest.XXXXXX")}
mkdir -p "$OUT"
SAVES=("$@")
F=tests/fixtures/native_saves
if [ ${#SAVES[@]} -eq 0 ]; then
  # The default saves are private fixtures ($OB_PRIVATE, else this checkout while it holds the set).
  if SA=$(ck_private_file "$F/stage1_win_streetx.sav") && SB=$(ck_private_file "$F/career_complete.sav"); then
    SAVES=("$SA:NAME" "$SB:FINISHED")
  else
    echo "map_xtest: SKIP (the default saves need the maintainer's private set (OB_PRIVATE); or name saves of your own: SAVE:NAME ...)"
    exit 3
  fi
fi
for t in Xvfb xdotool bwrap import; do command -v $t >/dev/null || { echo "missing $t"; exit 2; }; done
[ -x "$BIN" ] || { echo "no binary $BIN"; exit 2; }

FAILS=0
XV= ; GAME=
cleanup() {
    [ -n "$GAME" ] && kill -TERM -- -"$GAME" 2>/dev/null
    sleep 0.5
    [ -n "$GAME" ] && kill -KILL -- -"$GAME" 2>/dev/null
    [ -n "$XV" ] && kill "$XV" 2>/dev/null
    GAME= ; XV=
}
trap cleanup EXIT

free_display() {
    for n in $(seq 150 250); do
        [ -e /tmp/.X11-unix/X$n ] || [ -e /tmp/.X$n-lock ] || { echo $n; return; }
    done
}

X() { DISPLAY=:$D xdotool "$@" 2>/dev/null; }
last() { tail -n 1 "$LOG" 2>/dev/null; }
tag() { last | sed -n 's/.* tag=\([^ ]*\).*/\1/p'; }
wait_tag() { # regex timeout_s
    local i
    for i in $(seq 1 $(( $2 * 4 ))); do
        tag | grep -Eq "$1" && return 0
        sleep 0.25
    done
    return 1
}
key() { X key "$1"; sleep "${2:-0.5}"; }
shot() { DISPLAY=:$D import -window root "$DIR/$1.png" 2>/dev/null; }
step() { STEP=$((STEP + 1)); shot "$(printf %02d $STEP)-$1"; echo "  [$STEP] $1: $(tag)"; }
check() { # name condition...
    local name=$1; shift
    if "$@"; then echo "  ok   $name"; else echo "  FAIL $name (tag $(tag))"; FAILS=$((FAILS + 1)); shot "fail-$name"; fi
}
# The map tag's fields: InGameMap:sel=<i>:<hash>:gps=<0|1>:focus=<r>.
sel() { tag | sed -n 's/^InGameMap:sel=\([^:]*\):.*/\1/p'; }
target() { tag | sed -n 's/^InGameMap:sel=[^:]*:\([0-9A-F]*\):.*/\1/p'; }
gps_on_map() { tag | sed -n 's/.*:gps=\([01]\):.*/\1/p'; }
# Free roam: Explore:gps=<hash>:<state> or Explore:gps=-.
explore_gps() { tag | sed -n 's/^Explore:gps=\([^:]*\).*/\1/p'; }
# A fresh log line with the probes (a 1-pixel move and back).
fresh() { X mousemove_relative -- 1 0; sleep 0.25; X mousemove_relative -- -1 0; sleep 0.25; }
probe() { last | tr ' ' '\n' | sed -n "s/^$1@\(.*\)/\1/p" | tr ',' ' '; }
click() { X mousemove "$1" "$2"; sleep 0.2; X mousedown 1; sleep 0.08; X mouseup 1; sleep 0.6; }
away() { X mousemove 2 2; sleep 0.3; }

run_save() {
    local save=${1%:*} name=${1##*:}
    D=${XTEST_DISPLAY:-$(free_display)}
    DIR=$OUT/$name
    STEP=0
    rm -rf "$DIR"; mkdir -p "$DIR/data/overbright" "$DIR/data/NFS Underground 2/$name"
    cp "$save" "$DIR/data/NFS Underground 2/$name/$name"
    LOG=$DIR/input.log
    echo "== $name ($(basename "$save"), display :$D) =="
    Xvfb :$D -screen 0 1344x1024x24 -nolisten tcp >"$DIR/xvfb.log" 2>&1 &
    XV=$!
    for _ in $(seq 1 40); do [ -e /tmp/.X11-unix/X$D ] && break; sleep 0.1; done
    printf 'resolution 1280x960\nfullscreen 0\n' >"$DIR/data/overbright/options.txt"
    local probe="map,UI_PC_Help_Bar:icon_1,UI_PC_Help_Bar:icon_2,GenericDialog_SMALL:#B8A7C6CC,GenericDialog_SMALL:#B8A7C6CD"
    env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE -u NFSU2_HEADLESS_ONLY DISPLAY=:$D XDG_DATA_HOME="$DIR/data" NFSU2_CACHE_DIR="${NFSU2_CACHE_DIR:-$OUT/cache}" MESA_VK_WSI_DEBUG=sw \
        NFSU2_INPUT_LOG="$LOG" NFSU2_INPUT_PROBE="$probe" \
        setsid bwrap --unshare-pid --dev-bind / / --tmpfs /dev/snd --tmpfs "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" --unsetenv WAYLAND_DISPLAY \
        "$BIN" >"$DIR/game.log" 2>&1 &
    GAME=$!
    local win=
    for _ in $(seq 1 120); do
        win=$(X search --onlyvisible --name "Need for Speed" | head -n 1)
        [ -n "$win" ] && break
        sleep 0.5
    done
    if [ -z "$win" ]; then echo "  FAIL no window"; FAILS=$((FAILS + 1)); cleanup; return; fi
    X windowmove "$win" 0 0; X windowfocus "$win"
    away

    # Boot (Enter skips the movies) → the main menu with the profile.
    for _ in $(seq 1 20); do tag | grep -q '^Icons:Main' && break; key Return 0.8; done
    check main wait_tag '^Icons:Main' 5
    # Career (left of Quick Race) → the garage → Return to Explore Mode.
    key Left 1; key Return 2
    check career wait_tag '^Icons:Career' 5
    key Return 2
    check crib wait_tag '^Icons:Crib' 20
    sleep 1.5; key Return 1.5; key Left; key Return 1
    if ! wait_tag '^Explore' 40; then
        echo "  FAIL explore (tag $(tag))"; FAILS=$((FAILS + 1)); shot fail-explore; cleanup; return
    fi
    sleep 3
    step free-roam

    # M: the map, GPS off.
    key m 1.5
    check map-open wait_tag '^InGameMap:.*:gps=0:' 3
    step map
    local s0; s0=$(sel)
    [ -n "$s0" ] || { echo "  FAIL no selection"; FAILS=$((FAILS + 1)); cleanup; return; }
    key Right; step right
    # Only the focused row's group steps (0x4B0850); a group of one marker
    # keeps its selection (the stage-2 save's Car Lot).
    if [ "$(sel)" = "$s0" ]; then
        echo "  ok   right-stays (one marker in the focused group)"
    else
        echo "  ok   right-steps"
    fi
    key Left; step left
    check left-steps-back test "$(sel)" = "$s0"
    key Down; key Down; key Down; step down3
    check down-focus wait_tag ':focus=3$' 2
    local t; t=$(target)
    # Every captured value must be non-empty: `test "" = ""` would pass a check that read nothing.
    check target-read test -n "$t"
    # Enter (Accept, 0x4B0EF0) hides the focused row's group (the
    # selection in it dropped), Enter again shows it (its first marker).
    key Return; step enter-hide
    check enter-hides test "$(sel)" = "-"
    key Return; step enter-show
    check enter-shows test "$(target)" = "$t"
    # C: the question with No focused; Enter answers No.
    key c 1; step question
    check question-no-focused wait_tag '^Dialog:GenericDialog_[A-Z]*:1$' 2
    key Return 1; step enter-no
    check enter-is-no wait_tag "^InGameMap:sel=[^:]*:$t:gps=0:" 2
    # C, Left (Yes), Enter: the GPS on the selected marker, the map closed.
    key c 1; key Left; key Return 1.5; step engaged
    check engage-closes wait_tag "^Explore:gps=$t:" 3
    sleep 3; step route
    local route; route=$(tag | sed -n 's/^Explore:gps=[^:]*:\([0-9]\)/\1/p')
    check gps-route-read test -n "$route"
    check gps-on test "$route" != 4
    # M with the GPS on: the selection on the target; Right does nothing.
    key m 1.5; step map-gps-on
    check map-gps-on wait_tag "^InGameMap:sel=[^:]*:$t:gps=1:" 2
    key Right; step right-gps-on
    check right-ignored-gps-on test "$(target)" = "$t"
    # C: "Deactivating GPS"; Enter (OK) clears it and the map stays.
    key c 1; step deactivating
    check deactivating wait_tag '^Dialog:' 2
    key Return 1; step deactivated
    check deactivated wait_tag '^InGameMap:.*:gps=0:' 2
    local s1; s1=$(sel)
    check selection-read test -n "$s1"
    key Right; step right-again
    check right-steps-again test "$(sel)" != "$s1"
    # The mouse: a click on another marker asks; Yes engages on it.
    fresh
    local m mi mt mp
    m=$(last | tr ' ' '\n' | grep -E '^m[0-9]+=' | grep -v "=$t@" | grep -v "^m$(sel)=" | tail -n 1)
    mi=${m%%=*}; mi=${mi#m}; mt=${m#*=}; mt=${mt%@*}; mp=$(echo "${m#*@}" | tr ',' ' ')
    if [ -n "$m" ]; then
        click $mp; step click-marker
        check click-asks wait_tag '^Dialog:' 2
        sleep 0.5; fresh
        local yes; yes=$(probe 'GenericDialog_SMALL:#B8A7C6CC')
        if [ -n "$yes" ]; then click $yes; else key Left; key Return; fi
        step click-yes
        check click-engages wait_tag "^Explore:gps=$mt:" 3
        away
        # The help bar's Deactivate GPS (button2) asks; OK; Activate asks;
        # Esc answers No.
        key m 1.5; fresh
        click $(probe UI_PC_Help_Bar:icon_2); step help-deactivate
        check help-deactivate wait_tag '^Dialog:' 2
        away; key Return 1
        check help-deactivated wait_tag '^InGameMap:.*:gps=0:' 2
        fresh
        click $(probe UI_PC_Help_Bar:icon_2); step help-activate
        check help-activate wait_tag '^Dialog:' 2
        away; key Escape 1
        check help-no wait_tag '^InGameMap:.*:gps=0:' 2
    else
        check markers-for-mouse false
    fi
    key Escape 1
    check back wait_tag '^Explore' 3
    if [ "$(ck_bad "$DIR/game.log")" != 0 ]; then echo "  FAIL panic/validation"; FAILS=$((FAILS + 1)); fi
    cleanup
}

for s in "${SAVES[@]}"; do run_save "$s"; done
echo "logs: $OUT"
if [ $FAILS -eq 0 ]; then echo "map_xtest: all passed"; else echo "map_xtest: $FAILS failed"; exit 1; fi
