#!/bin/bash
# Runs inside one distribution container, started by run.sh (no network, no devices, no host display).
# Mounts: /a the unpacked Linux archive (read-only), /dev-game a harness game of the same build (read-only,
# may be empty), /game the player's game folder (read-only), /out this run's results.
# MODE=bare: ldd and --version only (the image as pulled). MODE=full: also --check-game, --probe on
# lavapipe, the headless shots ($SHOTS), the launcher on this container's own Xvfb and one screenshot.
# MODE=novk / nodrv: what a player without Vulkan sees (no loader at all / the loader but no driver): the
# probe, the game's window start, the launcher's screen.
# Every result is one "RESULT ..." line in /out/results.txt.
set -u
MODE=${MODE:-full}
O=/out
# The caches inside the container (gone with it, hundreds of MB each); results, logs and data in /out.
C=/var/tmp/overbright
mkdir -p $O/home $O/data $O/fb $C/cache $C/cache-home $C/tmp
export HOME=$O/home XDG_DATA_HOME=$O/data XDG_CACHE_HOME=$C/cache-home NFSU2_CACHE_DIR=$C/cache TMPDIR=$C/tmp
# No sound device exists in the container; the game is told so as well.
export NFSU2_AUDIO_HOST=none LANG=C.UTF-8
unset DISPLAY WAYLAND_DISPLAY XDG_RUNTIME_DIR XDG_SESSION_TYPE
r() { echo "RESULT $*" | tee -a $O/results.txt; }
: > $O/results.txt
r "os $(. /etc/os-release; echo "$PRETTY_NAME")"
r "glibc $(getconf GNU_LIBC_VERSION 2> /dev/null || ldd --version 2>&1 | head -1)"
for b in game overbright; do
    if command -v ldd > /dev/null; then
        nf=$(ldd /a/$b 2>&1 | grep -E 'not found' | sed 's/^[[:space:]]*//' | tr '\n' ';')
        r "ldd $b: ${nf:-all found}"
    else
        r "ldd $b: no ldd in the image"
    fi
    out=$(timeout 30 /a/$b --version 2>&1)
    r "$b --version rc=$?: $(echo "$out" | head -3 | tr '\n' '|' | cut -c1-300)"
done
[ "$MODE" = bare ] && exit 0

out=$(timeout 300 /a/game --check-game /game 2>&1)
r "game --check-game rc=$?: $(echo "$out" | grep -c '^  ok') ok lines, $(echo "$out" | grep -ciE '^  (error|missing)') error lines"
echo "$out" > $O/check-game.txt

# The container's own X server; its screen is written to a file (Xvfb -fbdir: XWD format).
D=7
Xvfb :$D -screen 0 1280x800x24 -nolisten tcp -fbdir $O/fb > $O/xvfb.log 2>&1 &
XPID=$!
for _ in $(seq 100); do [ -e /tmp/.X11-unix/X$D ] && break; sleep 0.2; done
[ -e /tmp/.X11-unix/X$D ] && r "xvfb up" || r "xvfb FAILED: $(tail -2 $O/xvfb.log | tr '\n' '|')"
launcher_shot() { # launcher_shot <seconds>: the launcher on :$D, its screen after that long
    DISPLAY=:$D NFSU2_ROOT=/game LD_DEBUG=files LD_DEBUG_OUTPUT=$O/lddbg-launcher timeout $(($1 + 30)) /a/overbright > $O/launcher.log 2>&1 &
    local pid=$!
    sleep "$1"
    if kill -0 $pid 2> /dev/null; then r "launcher running after $1 s"
    else wait $pid; r "launcher EXITED rc=$?: $(tail -3 $O/launcher.log | tr '\n' '|' | cut -c1-300)"; fi
    cp $O/fb/Xvfb_screen0 $O/launcher.xwd 2> /dev/null && r "launcher screenshot taken" || r "launcher screenshot FAILED"
    kill $pid 2> /dev/null
    wait $pid 2> /dev/null
}

if [ "$MODE" = novk ] || [ "$MODE" = nodrv ]; then
    # nodrv: the loader finds no driver (no ICD file where it looks).
    [ "$MODE" = nodrv ] && export VK_ICD_FILENAMES=/nonexistent.json VK_DRIVER_FILES=/nonexistent.json
    out=$(DISPLAY=:$D timeout 60 /a/game --probe 2>&1)
    rc=$?
    echo "$out" > $O/probe.txt
    r "$MODE probe rc=$rc: $(echo "$out" | grep -E '^(error|ok)=' | tr '\n' '|')"
    out=$(DISPLAY=:$D NFSU2_ROOT=/game timeout 60 /a/game 2>&1)
    rc=$?
    echo "$out" > $O/game-window.txt
    r "$MODE game window rc=$rc: $(echo "$out" | grep -E 'could not|Vulkan|adapter|panicked' | tail -2 | tr '\n' '|' | cut -c1-500)"
    launcher_shot 20
    kill $XPID 2> /dev/null
    wait $XPID 2> /dev/null
    exit 0
fi

# The game's own hardware check, on lavapipe (Mesa's software Vulkan driver).
out=$(DISPLAY=:$D timeout 120 /a/game --probe 2>&1)
rc=$?
echo "$out" > $O/probe.txt
r "game --probe rc=$rc: $(echo "$out" | grep -E '^(adapter.name|adapter.driver|error|ok)=' | tr '\n' '|' | cut -c1-300)"

# Real frames: the harness game's headless shots (no window, no audio device).
if [ -x /dev-game/game ]; then
    for s in ${SHOTS:-main}; do
        t=$(date +%s)
        NFSU2_HEADLESS_ONLY=1 NFSU2_ROOT=/game timeout ${SHOT_TIMEOUT:-900} /dev-game/game --shot $s $O/shot-$s.png > $O/shot-$s.log 2>&1
        rc=$?
        pv=$(grep -ciE 'panicked|validation error' $O/shot-$s.log)
        r "shot $s rc=$rc $(stat -c %s $O/shot-$s.png 2> /dev/null || echo 0) bytes $(($(date +%s) - t)) s panics/validation=$pv"
    done
else
    r "shot: no harness game (DEV_GAME)"
fi

# The launcher in its window: one screenshot, and the libraries it and the game's probe loaded at run time.
launcher_shot ${LAUNCHER_WAIT:-20}
loaded=$(cat $O/lddbg-launcher.* 2> /dev/null | grep -oE 'file=[^ ]+ +\[0\];  dynamically loaded' | sed 's/file=//; s/ .*//; s|.*/||' | sort -u | tr '\n' ' ')
r "loaded at run time: $loaded"
rm -f $O/lddbg-launcher.*
kill $XPID 2> /dev/null
wait $XPID 2> /dev/null
exit 0
