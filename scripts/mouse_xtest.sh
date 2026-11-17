#!/bin/bash
# The front end's mouse through REAL window events (docs/formats.md
# "Front-end mouse"): the game runs windowed on a private Xvfb display,
# sandboxed (bwrap: no /dev/snd, no $XDG_RUNTIME_DIR, no Wayland; its own
# XDG_DATA_HOME and NFSU2_CACHE_DIR: a plain start never moves the real folders), and xdotool moves, clicks, drags and turns the wheel
# there. Nothing reaches the desktop or the player's saves.
#
#   scripts/mouse_xtest.sh [WxH[:aspect[:scale]] ...]
#
# aspect 0 Wide (default), 1 Stretched; scale a WINIT_X11_SCALE_FACTOR.
# Default configurations: 1280x960 1920x1080 2560x1080 2560x1080:1
# 1920x1080:0:1.5. Env: BIN (default target/release/game, or
# $CARGO_TARGET_DIR/release/game), NFSU2_ROOT, OUT (logs and screenshots;
# default a new temp dir). Needs Xvfb, xdotool, bwrap and ImageMagick's
# import. The game writes NFSU2_INPUT_LOG (a line per frame with mouse
# activity: the edges, whether its cursor is drawn, the screen and its
# selection) and NFSU2_INPUT_PROBE's object positions, which the steps
# below check.
#
# Checked per configuration: no cursor during the boot movies (0x8383AC),
# none on the splash either (agreed deviation); MC_Main's Yes / No: hover
# focuses, click answers; UI_Main: the strip's arrows, the handle's drag,
# a click on the selected icon, the help bar's Quit Game (the quit
# question; hover focuses its buttons, Cancel closes it); Options: a
# category opened by a click, row hover, a right-arrow click, the help
# bar's Back. The first configuration also starts a career: no cursor
# through its movie, scene and while driving; the World Map's and the
# pause menu's help bars, the 2 s rule there.
set -u
cd "$(dirname "$0")/.."
BIN=${BIN:-${CARGO_TARGET_DIR:-target}/release/game}
. scripts/lib/common.sh
ck_local_env
ck_need_root
OUT=${OUT:-$(mktemp -d "${TMPDIR:-/tmp}/mouse_xtest.XXXXXX")}
mkdir -p "$OUT"
CONFIGS=("$@")
[ ${#CONFIGS[@]} -eq 0 ] && CONFIGS=(1280x960 1920x1080 2560x1080 2560x1080:1 1920x1080:0:1.5)
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

# --- helpers on the current configuration ($D, $LOG, $DIR) ---
X() { DISPLAY=:$D xdotool "$@" 2>/dev/null; }
last() { tail -n 1 "$LOG" 2>/dev/null; }
tag() { last | sed -n 's/.* tag=\([^ ]*\).*/\1/p'; }
# A fresh log line (a 1-pixel move and back): the probes as now laid out.
fresh() { X mousemove_relative -- 1 0; sleep 0.25; X mousemove_relative -- -1 0; sleep 0.25; }
# An object's window point; a few fresh lines while it is not laid out yet
# (a slow machine draws the new screen later).
pos() {
    local p i
    for i in 1 2 3 4; do
        p=$(last | tr ' ' '\n' | sed -n "s/^$1@\(.*\)/\1/p" | tr ',' ' ')
        [ -n "$p" ] && break
        fresh
    done
    echo $p
}
wait_tag() { # regex timeout_s
    local i
    for i in $(seq 1 $(( $2 * 4 ))); do
        tag | grep -Eq "$1" && return 0
        sleep 0.25
    done
    return 1
}
wait_count() { # pattern count timeout_s: the log has more lines with pattern than count
    local i
    for i in $(seq 1 $(( $3 * 4 ))); do
        [ "$(grep -c -- "$1" "$LOG")" -gt "$2" ] && return 0
        sleep 0.25
    done
    return 1
}
move() { [ $# -ge 2 ] || { echo "  (no position)"; return 1; }; X mousemove "$1" "$2"; sleep 0.3; }
click() { [ $# -ge 2 ] || { echo "  (no position)"; return 1; }; X mousemove "$1" "$2"; sleep 0.15; X mousedown 1; sleep 0.08; X mouseup 1; sleep 0.4; }
quick_click() { [ $# -ge 2 ] || { echo "  (no position)"; return 1; }; X mousemove "$1" "$2"; sleep 0.15; X click 1; sleep 0.4; }
shot() { DISPLAY=:$D import -window root "$DIR/$1.png" 2>/dev/null; }
check() { # name condition...
    local name=$1; shift
    if "$@"; then echo "  ok   $name"; else echo "  FAIL $name (tag $(tag))"; FAILS=$((FAILS + 1)); shot "fail-$name"; fi
}
sel() { tag | awk -F: '{print $NF}'; }
click_obj() { # probe-name
    fresh
    local p; p=$(pos "$1")
    [ -n "$p" ] || { echo "  (no $1 on screen)"; return 1; }
    click $p
}
key() { X key "$1"; sleep 0.4; }

run_config() {
    local cfg=$1 size aspect scale
    IFS=: read -r size aspect scale <<<"$cfg"
    aspect=${aspect:-0}
    local W=${size%x*} H=${size#*x}
    D=${XTEST_DISPLAY:-$(free_display)}
    DIR=$OUT/$(echo "$cfg" | tr ':.' '__')
    rm -rf "$DIR"; mkdir -p "$DIR/data/overbright"
    LOG=$DIR/input.log
    echo "== $cfg (display :$D) =="
    Xvfb :$D -screen 0 $((W + 64))x$((H + 64))x24 -nolisten tcp >"$DIR/xvfb.log" 2>&1 &
    XV=$!
    for _ in $(seq 1 40); do [ -e /tmp/.X11-unix/X$D ] && break; sleep 0.1; done
    printf 'resolution %s\nfullscreen 0\naspect %s\n' "$size" "$aspect" >"$DIR/data/overbright/options.txt"
    local probe="MC_Main:#B1000CC6,MC_Main:#B1000CC7,MC_Main:#B1000CC8,UI_Main:ScrollBar_Arrow_1,UI_Main:ScrollBar_Arrow_2,UI_Main:ScrollBar_Handle,UI_Main:#1"
    probe+=",UI_OptionsMain:#1,UI_StartCareer:#1,UI_PC_Help_Bar:icon_1,UI_PC_Help_Bar:icon_2,UI_PC_Help_Bar:icon_3,rows"
    probe+=",GenericDialog_SMALL:#B8A7C6CC,GenericDialog_SMALL:#B8A7C6CD,GenericDialog_MED:#B8A7C6CC,GenericDialog_MED:#B8A7C6CD"
    # setsid: the sandbox leads its own process group (killed as a group).
    env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE -u NFSU2_HEADLESS_ONLY DISPLAY=:$D XDG_DATA_HOME="$DIR/data" NFSU2_CACHE_DIR="${NFSU2_CACHE_DIR:-$OUT/cache}" MESA_VK_WSI_DEBUG=sw \
        ${scale:+WINIT_X11_SCALE_FACTOR=$scale} NFSU2_INPUT_LOG="$LOG" NFSU2_INPUT_PROBE="$probe" \
        setsid bwrap --dev-bind / / --tmpfs /dev/snd --tmpfs "${XDG_RUNTIME_DIR:-/run/user/$(id -u)}" --unsetenv WAYLAND_DISPLAY \
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
    local geo; geo=$(X getwindowgeometry "$win" | sed -n 's/.*Geometry: //p')
    echo "  window $geo"

    # Boot movies: the mouse moves, no cursor (0x8383AC).
    wait_tag '^Movie$' 30
    for i in 1 2 3 4 5 6; do move $((100 + i * 40)) $((100 + i * 30)); done
    shot movie
    check movie-no-cursor bash -c "grep -q 'tag=Movie' '$LOG' && ! grep 'tag=Movie' '$LOG' | grep -q 'cursor=1'"
    # Skip to the splash (Enter skips the movies: agreed deviation).
    for _ in $(seq 1 20); do tag | grep -q '^Splash' && break; key Return; sleep 0.6; done
    move 300 300; move 320 310
    # Agreed deviation (docs/native-deviations.md; `cursor_shown`): no cursor before
    # the menus, the splash included (the original shows it there).
    check splash-no-cursor bash -c "grep -q 'tag=Splash' '$LOG' && ! grep 'tag=Splash' '$LOG' | grep -q 'cursor=1'"
    shot splash
    for _ in $(seq 1 10); do tag | grep -q '^McMain' && break; key Return; sleep 1; done
    wait_tag '^McMain:Ask' 20
    # MC_Main "Would you like to create a new profile?": hover Yes, click it.
    sleep 1; fresh
    local yes no; yes=$(pos 'MC_Main:#B1000CC6'); no=$(pos 'MC_Main:#B1000CC7')
    move $no; move $yes
    check mcmain-hover wait_tag '^McMain:Ask:0' 2
    shot mcmain
    click $yes
    check mcmain-click wait_tag '^McMain:Name' 3
    X type --delay 60 MOUSE; sleep 0.3; key Return
    wait_tag '^McMain:Confirm' 5
    sleep 1; fresh; yes=$(pos 'MC_Main:#B1000CC6')
    quick_click $yes   # a click shorter than a frame (press + release in one)
    check mcmain-confirm wait_tag '^McMessage' 5
    # "Save complete." [Continue] (the middle slot).
    sleep 1; fresh
    click $(pos 'MC_Main:#B1000CC8')
    check mcmessage-click wait_tag '^Icons:Main' 15

    # UI_Main: the strip's arrows.
    sleep 1.5
    local s0; s0=$(sel)
    click_obj UI_Main:ScrollBar_Arrow_2
    check main-arrow-right test "$(sel)" = $((s0 + 1))
    click_obj UI_Main:ScrollBar_Arrow_1
    check main-arrow-left test "$(sel)" = "$s0"
    shot main
    # The handle dragged right steps the strip on (0x52E4F0).
    fresh
    local h; h=$(pos UI_Main:ScrollBar_Handle)
    if [ -n "$h" ]; then
        set -- $h
        X mousemove "$1" "$2"; sleep 0.15; X mousedown 1; sleep 0.1
        for k in 1 2 3 4 5 6 7 8; do X mousemove $(($1 + k * W / 40)) "$2"; sleep 0.08; done
        X mouseup 1; sleep 0.4
        check main-drag test "$(sel)" -gt "$s0"
        # Back to Quick Race by the left arrow.
        for _ in 1 2 3 4 5 6; do [ "$(sel)" -gt "$s0" ] || break; click_obj UI_Main:ScrollBar_Arrow_1; done
    else
        check main-drag false
    fi
    # The help bar: Quit Game (UI_Main's icon_1, the bottom slot) asks.
    click_obj UI_PC_Help_Bar:icon_1
    check help-quit wait_tag '^Dialog:' 3
    shot quit
    # Hover the dialog's left button (Quit Game) then click Cancel.
    sleep 0.8; fresh
    local pkg; pkg=$(tag | cut -d: -f2)
    local left right; left=$(pos "$pkg:#B8A7C6CC"); right=$(pos "$pkg:#B8A7C6CD")
    move $left
    check dialog-hover wait_tag '^Dialog:.*:0$' 2
    click $right
    check dialog-cancel wait_tag '^Icons:Main' 3

    # Options: the strip's arrows to it (index 6), a click on the icon.
    for _ in $(seq 1 8); do [ "$(sel)" -lt 6 ] || break; click_obj UI_Main:ScrollBar_Arrow_2; done
    sleep 0.6
    click_obj 'UI_Main:#1'
    check options-open wait_tag '^Icons:Options' 4
    sleep 1.2
    click_obj 'UI_OptionsMain:#1'
    check options-category wait_tag '^Options' 4
    sleep 1; fresh
    local r1; r1=$(pos row1)
    if [ -n "$r1" ]; then
        move $r1
        check options-row-hover wait_tag '^Options:1$' 2
        local n0; n0=$(grep -c ' p=1 ' "$LOG")
        click $(pos rowR1)
        check options-arrow-press wait_count ' p=1 ' "$n0" 4
    else
        check options-rows false
    fi
    shot options
    # Back by the help bar (icon_2 over Quit Game).
    click_obj UI_PC_Help_Bar:icon_2
    check options-back wait_tag '^Icons:Options' 4
    # The career (first configuration, or XTEST_CAREER=1): no cursor
    # through the intro movie, the in-engine scene and while driving
    # (agreed deviation); the in-game World Map's help-bar Back; the
    # pause menu's cursor (2 s rule) and its help-bar Continue.
    if [ "${XTEST_CAREER:-$FIRST}" = 1 ]; then
        key Escape; sleep 1
        # The options save question and its "Save complete." by the mouse.
        for _ in 1 2 3 4; do
            tag | grep -q '^McMessage' || break
            sleep 0.8; fresh
            if [ -n "$(pos 'MC_Main:#B1000CC6')" ]; then click $(pos 'MC_Main:#B1000CC6'); else click $(pos 'MC_Main:#B1000CC8'); fi
            sleep 1.5
        done
        check options-save-answered wait_tag '^Icons:Main' 6
        for _ in $(seq 1 8); do [ "$(sel)" -gt 0 ] || break; click_obj UI_Main:ScrollBar_Arrow_1; done
        sleep 0.6; click_obj 'UI_Main:#1'
        check career-open wait_tag '^Icons:Career' 4
        sleep 1; click_obj 'UI_StartCareer:#1'
        wait_tag '^(Movie|Explore)' 10
        for _ in $(seq 1 25); do move $((300 + RANDOM % 600)) $((200 + RANDOM % 400)); key Return; sleep 1; done
        for i in 1 2 3 4; do move $((400 + i * 50)) 500; done
        check drive-no-cursor bash -c "grep -q 'tag=Explore' '$LOG' && ! grep -E 'tag=(Explore|Movie)' '$LOG' | grep -q 'cursor=1'"
        shot drive
        key m; wait_tag '^InGameMap' 4
        move 500 400; move 520 420
        check map-cursor bash -c "tail -n 1 '$LOG' | grep -q 'cursor=1'"
        shot map
        click_obj UI_PC_Help_Bar:icon_1
        check map-back wait_tag '^Explore' 4
        key Escape; wait_tag '^Icons:ExplorePause' 4
        move 500 400; move 520 420
        check pause-cursor bash -c "tail -n 1 '$LOG' | grep -q 'cursor=1'"
        sleep 2.6
        check pause-cursor-2s bash -c "tail -n 1 '$LOG' | grep -q 'cursor=0'"
        click_obj UI_PC_Help_Bar:icon_1
        check pause-continue wait_tag '^Explore' 4
    fi
    if [ "$(ck_bad "$DIR/game.log")" != 0 ]; then echo "  FAIL panic/validation"; FAILS=$((FAILS + 1)); fi
    cleanup
}

FIRST=1
for c in "${CONFIGS[@]}"; do run_config "$c"; FIRST=0; done
echo "logs: $OUT"
if [ $FAILS -eq 0 ]; then echo "mouse_xtest: all passed"; else echo "mouse_xtest: $FAILS failed"; exit 1; fi
