#!/bin/bash
# The Windows build under Wine, headless (docs/developing/checks.md "Wine smoke", docs/playing/platform-windows.md).
#
#   wine_smoke.sh OUTDIR [LINUX_BIN]            the smoke run (report-only in checks.sh full)
#   wine_smoke.sh --selfcheck OUTDIR [LINUX_BIN] every check against a broken input: each must FAIL
#   wine_smoke.sh --env-check OUTDIR            the runs' environment on a stand-in for wine (no Wine needed;
#                                               scripts/checks/selfcheck.sh runs it on broken copies too)
#
# The Windows game with the developer harness (`game.exe`, x86_64-pc-windows-gnu): $WINE_EXE, else built here
# with scripts/release/container/build.sh --windows --dev-harness --fast into $WINE_BUILD (default
# $CARGO_TARGET_DIR/wine-smoke-build; the crate cache and target dir are kept there between runs).
# LINUX_BIN (default $CARGO_TARGET_DIR/release/game, a dev-harness build): the reference shots, made here
# with the same settings unless $WINE_REF holds <target>.png already (checks.sh passes its own shots).
# Steps, each a line in OUTDIR/wine_smoke.txt:
#   harness     `game.exe --dev-harness` says "dev-harness: on"
#   version     `--version` exits 0 and names windows-x86_64
#   check_game  `--check-game <the game, as a Z:\ path>` exits 0 and finds every file
#   selftest    `--selftest` on Vulkan exits 0
#   vk_<t>      `--shot <t>` on Vulkan (winevulkan -> the host's driver) for each t in $WINE_SHOTS
#               (default "main explore race qrcars career"): exits 0, the log names a Vulkan adapter,
#               and the image matches the Linux one within the tolerance
#   vk_fallback `--shot explore` on Vulkan without the features DX12 lacks (NFSU2_GPU_DROP=multiview,
#               pipeline_cache): the fallbacks run, the log says so, the image matches Linux's with the same drop
#   dx12_main   `--shot main` on DX12 (Wine's vkd3d): PASS within the tolerance; XFAIL when Wine's own FXC
#               (d3dcompiler_47, vkd3d-shader) rejects wgpu's shader model 5.1 sampler heap and no DXC is
#               given ($WINE_DXC_DIR: a folder with dxcompiler.dll [dxil.dll], copied next to the exe); else FAIL
#   prepare     `--prepare` on Vulkan exits 0 with ok=1; `--probe` then says prepared=1 (docs/developing/prepare.md)
#   dx12_prepare the same on DX12 (no pipeline cache on disk there); XFAIL as dx12_main without a DXC
# The launcher (overbright.exe beside the build's game: $WINE_LAUNCHER, else dist/windows/overbright.exe; skipped
# when there is none; LINUX_LAUNCHER, default $CARGO_TARGET_DIR/release/overbright, draws the references):
#   l_render_<s>  `overbright.exe --render <s>` (the Mods page and its HUD states, the browser for the HUD mod,
#               the first-run screen) matches the Linux launcher's render within the tolerance
#   l_list      `--list-folder <a folder with accents, as a drive-letter path over 300 characters> hud` lists
#               the ZIP, the 7-Zip file and the extracted folder there with their notes
#   l_hud       `--add-hud-mod` with the HUD mod stand-in (scripts/checks/hud_mod_standin.py) in that folder:
#               "ok: HUD mod set up", the pack in the data folder (NFSU2_DATA_DIR, inside OUTDIR)
#   l_tpf       `--add-packages` with a package made for another game: its result line says so
#   l_start     tests/launcher_start.sh under Wine (the game.exe's --crash-test as the game; launcher.log; a test panic: exit 101): the failed start
#               opens the failure screen, a second launcher is refused, Play from the window fails again
#   l_mouse     tests/launcher_mouse.sh under Wine: the folder browser by the mouse alone (wheel, double clicks,
#               scroll bar, Open, the back button, places, path parts, This PC and the drives), the game folder
#               chosen and a package added by clicks
#   w_window    the game in its window (no flags, as a player starts it, without NFSU2_HEADLESS_ONLY; Vulkan through Mesa's software
#               present, MESA_VK_WSI_DEBUG=sw: Xvfb has no DRI3): reads the monitor and its modes (a display
#               mode without its fields must not stop it, docs/notes/graphics/resolutions.md), starts and reaches the title
#               screen ("boot: the title screen"; Enter skips the boot movies), no panic line
#   l_play      the launcher with "Start the game directly" set starts game.exe beside it: the game's own log
#               (in the launcher's data folder) reaches the title screen, no panic line in either log
# The map editor (overbright-editor.exe beside the build's game: $WINE_EDITOR, else dist/windows/overbright-editor.exe;
# when the build is made here it must be there, with WINE_EXE given and none found it is skipped; LINUX_EDITOR,
# default $CARGO_TARGET_DIR/<profile>/overbright-editor, draws the references with the same arguments):
#   e_start     `overbright-editor.exe --shot` (no map mod open, 1280x720) on Vulkan: exits 0, a PNG of that size,
#               the image matches the Linux editor's within the tolerance
#   e_props     the same with a map mod made here open on the city (Props mode, the 3D view: the world loaded and drawn
#               by the game's renderer under Wine), 1280x720; the view is not the same run to run even on Linux, so
#               the comparison is reported and the view must not be one flat colour
#   e_races     the same mod in Races mode on the 2D road-network map at 1600x960 and display scale 1.5
# Every log is grepped for panic / validation-error lines (any one fails its step).
# Tolerance (pixdiff): at most $WINE_TOL_PIXELS pixels differ (default 64) and no channel by more than
# $WINE_TOL_DIFF (default 4). Measured 2026-11-12: Wine+winevulkan on RADV gives byte-identical images
# (0 pixels) for every default target; the margin covers the Linux gate's known 1-4 px quirk (channel diff 1).
# The prefix (with the Windows asset cache) and tmp/ are removed at the end unless WINE_KEEP_PREFIX=1.
# Exit: 0 every step passed (XFAIL counts as passed), 1 a step failed, 3 skipped (no wine / Xvfb / podman
# without WINE_EXE; "WINE_SMOKE: SKIP (...)"). Last line: "WINE_SMOKE: PASS|FAIL (steps)|SKIP (why) (N s)".
# A private Xvfb (-displayfd: a free display) and a private WINEPREFIX under OUTDIR; every process is waited
# for by PID; at the end only this prefix's wineserver (wineserver -k with its WINEPREFIX) and our Xvfb (its
# PID) are stopped. Nothing is killed by name; no window can reach the player's desktop: every Wine process gets
# DISPLAY = our Xvfb, no WAYLAND_DISPLAY and XDG_RUNTIME_DIR = OUTDIR/xdg, the prefix is pinned to x11. The
# windowed runs (w_window, l_play) drop NFSU2_HEADLESS_ONLY (checks.sh exports it); every other run sets it.
# Not proved here: docs/developing/checks.md "Wine smoke" (real drivers, DX12 on real hardware, NTFS, audio, pads,
# pacing, fullscreen, DPI).
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/../lib/common.sh"
ck_local_env
REPO=$(cd "$HERE/../.." && pwd)
SELFCHECK=0 ENVCHECK=0
[ "${1:-}" = --selfcheck ] && { SELFCHECK=1; shift; }
[ "${1:-}" = --env-check ] && { ENVCHECK=1; shift; }
O=${1:?usage: wine_smoke.sh [--selfcheck|--env-check] OUTDIR [LINUX_BIN]}
mkdir -p "$O"; O=$(cd "$O" && pwd)
# The game's asset cache: the caller's NFSU2_CACHE_DIR, else a folder of this run (never the player's cache).
export NFSU2_CACHE_DIR="${NFSU2_CACHE_DIR:-$O/cache}"
TGT=${CARGO_TARGET_DIR:-$REPO/target}
LBIN=${2:-$TGT/$CK_PDIR/game}
PIXDIFF=${PIXDIFF:-$TGT/$CK_PDIR/examples/pixdiff}
SHOTS=${WINE_SHOTS:-main explore race qrcars career}
TOL_PX=${WINE_TOL_PIXELS:-64}
TOL_D=${WINE_TOL_DIFF:-4}
T0=$(date +%s)
REP="$O/wine_smoke.txt"; : > "$REP"
say() { echo "$*" | tee -a "$REP"; }
skip() { say "WINE_SMOKE: SKIP ($*) ($(( $(date +%s) - T0 )) s)"; exit 3; }

# ---- the steps (defined first: --env-check runs some of them on a stand-in for wine) ---------------------
DNUM="" XPID=""
cleanup() {
  WINEPREFIX="$O/prefix" W wineserver -k > /dev/null 2>&1
  [ -n "$XPID" ] && kill "$XPID" 2> /dev/null && wait "$XPID" 2> /dev/null
  # The prefix holds the Windows asset cache (~1 GB): removed unless WINE_KEEP_PREFIX=1.
  [ "${WINE_KEEP_PREFIX:-0}" = 1 ] || rm -rf "$O/prefix" "$O/tmp"
}
# Every Wine process: our Xvfb only, with a runtime folder of our own (winex11 would take Wayland first if it
# were set, and without an X display Wine looks for the desktop's Wayland socket in XDG_RUNTIME_DIR).
W() { env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE XDG_RUNTIME_DIR="$O/xdg" DISPLAY=":$DNUM" "$@"; }
# The windowed runs (the game and the launcher as a player starts them): the same, and without the check set's
# NFSU2_HEADLESS_ONLY (checks.sh exports it for everything it starts; the game refuses to open a window with it,
# and Wine hands the Unix environment to the Windows program). Only ever on our own display, and never heard:
# NFSU2_AUDIO_HOST=none (the game opens no sound device) on top of the prefix without a sound driver.
WWIN() {
  [ -n "$DNUM" ] || ck_die "a windowed run without our own display"
  env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE -u NFSU2_HEADLESS_ONLY XDG_RUNTIME_DIR="$O/xdg" DISPLAY=":$DNUM" NFSU2_AUDIO_HOST=none "$@"
}
# wp PATH: PATH as Wine's programs see it. winepath -w cuts its answer at MAX_PATH (Wine 11: a 282-character
# folder came back as its first 257 characters, a folder that does not exist); a path outside the prefix's
# drive_c is the Z: drive (the root) and the path with backslashes, so a cut answer is replaced by that.
wp() {
  local w z="Z:${1//\//\\}"
  w=$(WINEPREFIX="$O/prefix" W winepath -w "$1" 2> /dev/null)
  case $z in "$w"?*) w=$z ;; esac
  printf '%s\n' "$w"
}
FAIL=() XFAIL=()
# wrun NAME TIMEOUT EXE ARGS…: one Wine run (log OUTDIR/NAME.log) with the variables in WENV, waited for
# by PID; sets RC, BAD, SECS.
WENV=()
wrun() {
  local name=$1 to=$2 exe=$3; shift 3
  mkdir -p "$O/tmp/$name"
  local tw; tw=$(wp "$O/tmp/$name")
  local t=$(date +%s)
  W "${WENV[@]}" NFSU2_ROOT="$ROOT_W" OB_PRIVATE="$PRIV_W" XDG_DATA_HOME="$tw" TMP="$tw" TEMP="$tw" NFSU2_HEADLESS_ONLY=1 NFSU2_SHOT_CLOCK=${SHOT_CLOCK-0} \
    RUST_LOG="${RUST_LOG:-info},naga=warn,wgpu_hal=warn,wgpu_core=warn" timeout "$to" wine "$exe" "$@" > "$O/$name.log" 2>&1 &
  wait $!; RC=$?
  SECS=$(( $(date +%s) - t ))
  BAD=$(ck_bad "$O/$name.log")
}
# result NAME OK DETAIL: a step's line; OK = ok | XFAIL | anything else (a failure).
result() {
  case $2 in
    ok) say "$1: ok $3" ;;
    XFAIL) XFAIL+=("$1"); say "$1: XFAIL $3" ;;
    *) FAIL+=("$1"); say "$1: FAIL $3" ;;
  esac
}

# ---- the checks (each takes its input, so --selfcheck can hand it a broken one) --------------------------------
check_harness() { # EXE
  WENV=(); wrun harness 120 "$1" --dev-harness
  local ok=ok; grep -q '^dev-harness: on' "$O/harness.log" && [ $RC = 0 ] || ok=no
  result harness $ok "rc=$RC $(grep -v MESA "$O/harness.log" | head -1 | cut -c1-80)"
}
check_version() { # EXE
  WENV=(); wrun version 120 "$1" --version
  local v; v=$(grep -m1 '^Overbright ' "$O/version.log")
  local ok=ok; [ $RC = 0 ] && echo "$v" | grep -q 'windows-x86_64' || ok=no
  result version $ok "rc=$RC ${v:-no version line}"
}
check_game() { # EXE ROOT(Windows path)
  WENV=(); wrun check_game 300 "$1" --check-game "$2"
  local ok=ok; [ $RC = 0 ] && grep -qE 'ok: All [0-9]+ files' "$O/check_game.log" && [ "$BAD" = 0 ] || ok=no
  result check_game $ok "rc=$RC $(grep -m1 -E 'edition:|ok: All|missing|not' "$O/check_game.log" | cut -c1-100)"
}
check_selftest() { # EXE
  WENV=(WGPU_BACKEND=vulkan); wrun selftest 1800 "$1" --selftest
  local ok=ok; [ $RC = 0 ] && [ "$BAD" = 0 ] || ok=no
  result selftest $ok "rc=$RC panic/validation=$BAD $SECS s: $(grep -v MESA "$O/selftest.log" | tail -1 | cut -c1-100)"
}
check_prepare() { # EXE
  # `--prepare` on Vulkan: ends with ok=1 and its stamp, which `--probe` then reports (docs/developing/prepare.md).
  WENV=(WGPU_BACKEND=vulkan); wrun prepare 900 "$1" --prepare
  local ok=ok; [ $RC = 0 ] && grep -q '^ok=1' "$O/prepare.log" && [ "$BAD" = 0 ] || ok=no
  local what="rc=$RC panic/validation=$BAD $SECS s: $(grep -c '^step=' "$O/prepare.log") steps, $(grep -m1 -o '^pipeline_cache=[01]' "$O/prepare.log"), $(grep -m1 -o '^pipelines=[0-9]*' "$O/prepare.log")"
  WENV=(WGPU_BACKEND=vulkan); wrun prepare_probe 120 "$1" --probe
  grep -q '^prepared=1' "$O/prepare_probe.log" || { ok=no; what="$what; the probe does not say prepared=1"; }
  result prepare $ok "$what"
}
check_prepare_dx12() { # EXE
  # The same on DX12 (no pipeline cache there: pipeline_cache=0). XFAIL as dx12_main when Wine's own FXC cannot
  # compile wgpu's shaders and no DXC is given.
  WENV=(WGPU_BACKEND=dx12); wrun dx12_prepare 900 "$1" --prepare
  local comp; comp=$(grep -m1 -oE 'DX12 shader compiler: [A-Z]+' "$O/dx12_prepare.log" | cut -c23-)
  local what="rc=$RC panic/validation=$BAD $SECS s [${comp:-?}]: $(grep -c '^step=' "$O/dx12_prepare.log") steps, $(grep -m1 -o '^pipeline_cache=[01]' "$O/dx12_prepare.log"), $(grep -m1 -o '^pipelines=[0-9]*' "$O/dx12_prepare.log")"
  if [ $RC = 0 ] && grep -q '^ok=1' "$O/dx12_prepare.log" && [ "$BAD" = 0 ]; then
    result dx12_prepare ok "$what"
  elif grep -q 'E5017: Aborting due to not yet implemented feature: Shader model 5.1+ resource array' "$O/dx12_prepare.log"; then
    result dx12_prepare XFAIL "$what: Wine's d3dcompiler_47 (vkd3d-shader) has no SM 5.1 resource arrays; give WINE_DXC_DIR"
  else
    result dx12_prepare no "$what: $(grep -v MESA "$O/dx12_prepare.log" | tail -1 | cut -c1-120)"
  fi
}
# until_title PID LOGGLOB: waits (up to the process's own timeout) for "boot: the title screen" in the newest
# file matching LOGGLOB, sending Enter to the game's window meanwhile (it skips the boot movies); then stops
# this prefix's processes (wineserver -k) and waits for PID. Sets TITLE (1 reached, 0 not).
until_title() {
  local pid=$1 glob=$2 f
  TITLE=0
  while kill -0 "$pid" 2> /dev/null; do
    f=$(ls -t $glob 2> /dev/null | head -1)
    if [ -n "$f" ] && grep -q 'boot: the title screen' "$f"; then TITLE=1; break; fi
    DISPLAY=":$DNUM" xdotool search --onlyvisible --name 'Need for Speed' key Return > /dev/null 2>&1
    sleep 1
  done
  WINEPREFIX="$O/prefix" wineserver -k > /dev/null 2>&1
  wait "$pid" 2> /dev/null
  f=$(ls -t $glob 2> /dev/null | head -1)
  [ -n "$f" ] && grep -q 'boot: the title screen' "$f" && TITLE=1
}
# sound_lines LOG…: the lines that say a sound device was opened or written to (the game's own "audio: … output
# at", ALSA's messages). A check run must have none: nothing a check starts may be heard.
sound_lines() {
  local n=0 f c
  for f in "$@"; do [ -f "$f" ] || continue; c=$(grep -caE 'audio: [A-Za-z0-9 ]+ output at|ALSA lib' "$f"); n=$(( n + ${c:-0} )); done
  echo $n
}
check_window() { # EXE
  mkdir -p "$O/tmp/w_window"
  local tw; tw=$(wp "$O/tmp/w_window")
  local t=$(date +%s)
  WWIN NFSU2_ROOT="$ROOT_W" NFSU2_DATA_DIR="$tw" TMP="$tw" TEMP="$tw" NFSU2_NO_PADS=1 MESA_VK_WSI_DEBUG=sw WGPU_BACKEND=vulkan \
    RUST_LOG="${RUST_LOG:-info},naga=warn,wgpu_hal=warn,wgpu_core=warn" timeout 180 wine "$1" > "$O/w_window.log" 2>&1 &
  until_title $! "$O/w_window.log"
  BAD=$(ck_bad "$O/w_window.log")
  local snd; snd=$(sound_lines "$O/w_window.log" "$O"/tmp/w_window/logs/game-*.log)
  local ok=ok; [ "$TITLE" = 1 ] && [ "$BAD" = 0 ] && [ "$snd" = 0 ] || ok=no
  result w_window $ok "$(( $(date +%s) - t )) s: title screen $([ "$TITLE" = 1 ] && echo reached || echo NOT reached), panic=$BAD, sound device lines=$snd; $(grep -m1 -oE 'monitor Some.*Resolution list rebuilt' "$O/w_window.log" | cut -c1-90)"
}
# check_play LAUNCHER_EXE: the launcher with "Start the game directly" set starts the game beside it, as a
# player's double-click does: the game's own log (in the launcher's data folder) reaches the title screen.
check_play() {
  local pdata="$O/tmp/play-data"; rm -rf "$pdata"; mkdir -p "$pdata"
  printf 'launcher.skip 1\n' > "$pdata/options.txt"
  local t=$(date +%s)
  WWIN NFSU2_DATA_DIR="$(wp "$pdata")" NFSU2_ROOT="$ROOT_W" NFSU2_NO_PADS=1 MESA_VK_WSI_DEBUG=sw WGPU_BACKEND=vulkan \
    RUST_LOG="${RUST_LOG:-info},naga=warn,wgpu_hal=warn,wgpu_core=warn" timeout 180 wine "$1" > "$O/l_play.log" 2>&1 &
  until_title $! "$pdata/logs/game-*.log"
  local glog; glog=$(ls "$pdata"/logs/game-*.log 2> /dev/null | head -1)
  BAD=$(( $(ck_bad "${glog:-/nonexistent}") + $(ck_bad "$O/l_play.log") ))
  local snd; snd=$(sound_lines "$O/l_play.log" "$pdata"/logs/game-*.log)
  local ok=ok; [ "$TITLE" = 1 ] && [ "$BAD" = 0 ] && [ "$snd" = 0 ] || ok=no
  result l_play $ok "$(( $(date +%s) - t )) s: title screen $([ "$TITLE" = 1 ] && echo reached || echo NOT reached), panic=$BAD, sound device lines=$snd; $(grep -h -m1 -oE 'monitor Some.*Resolution list rebuilt' "${glog:-/dev/null}" | cut -c1-90)"
}
# compare NAME A B: pixdiff within the tolerance; sets CMP (the pixdiff summary) and returns its status.
compare() {
  [ -s "$2" ] && [ -s "$3" ] || { CMP="missing image ($( [ -s "$2" ] || echo "$2" ) $( [ -s "$3" ] || echo "$3" ))"; return 1; }
  "$PIXDIFF" "$2" "$3" --max-pixels "$TOL_PX" --max-diff "$TOL_D" > "$O/$1.pixdiff" 2>&1; local r=$?
  CMP=$(tail -1 "$O/$1.pixdiff" | sed 's/^pixdiff: 1 files, //')
  return $r
}
# reference TARGET NAME [ENV…]: the Linux shot (WINE_REF/<target>.png when given and no ENV), else made now.
reference() {
  local s=$1 name=$2; shift 2
  if [ $# = 0 ] && [ -n "${WINE_REF:-}" ] && [ -s "$WINE_REF/$s.png" ]; then echo "$WINE_REF/$s.png"; return; fi
  mkdir -p "$O/linux" "$O/tmp/linux-$name"
  env "$@" XDG_DATA_HOME="$O/tmp/linux-$name" NFSU2_HEADLESS_ONLY=1 NFSU2_SHOT_CLOCK=${SHOT_CLOCK-0} \
    RADV_DEBUG=${RADV_DEBUG-nodcc} timeout 900 "$LBIN" --shot "$s" "$O/linux/$name.png" > "$O/linux/$name.log" 2>&1 &
  wait $!
  echo "$O/linux/$name.png"
}
check_shot() { # NAME EXE TARGET REF [ENV…] (on Vulkan)
  local name=$1 exe=$2 s=$3 ref=$4; shift 4
  local pw; pw=$(wp "$O/$name.png")
  WENV=(WGPU_BACKEND=vulkan "$@"); wrun "$name" 1200 "$exe" --shot "$s" "$pw"
  local adapter; adapter=$(grep -m1 -oE 'GPU adapter: .*' "$O/$name.log" | cut -c14-)
  local ok=ok why=""
  [ $RC = 0 ] || { ok=no; why="rc=$RC "; }
  [ "$BAD" = 0 ] || { ok=no; why="${why}panic/validation=$BAD "; }
  echo "$adapter" | grep -q 'Vulkan)' || { ok=no; why="${why}no Vulkan adapter line "; }
  compare "$name" "$O/$name.png" "$ref" || { ok=no; why="${why}image over the tolerance "; }
  result "$name" $ok "${why}$SECS s ${adapter:+[$adapter] }vs Linux: $CMP"
}
check_dx12() { # EXE REF
  local pw; pw=$(wp "$O/dx12_main.png")
  WENV=(WGPU_BACKEND=dx12); wrun dx12_main 1200 "$1" --shot main "$pw"
  local adapter; adapter=$(grep -m1 -oE 'GPU adapter: .*' "$O/dx12_main.log" | cut -c14-)
  local comp; comp=$(grep -m1 -oE 'DX12 shader compiler: [A-Z]+' "$O/dx12_main.log" | cut -c23-)
  if [ $RC = 0 ] && [ "$BAD" = 0 ] && echo "$adapter" | grep -q 'Dx12)' && compare dx12_main "$O/dx12_main.png" "$2"; then
    result dx12_main ok "$SECS s [$adapter, ${comp:-?}] vs Linux: $CMP"
  elif [ "$comp" = FXC ] && grep -q 'E5017: Aborting due to not yet implemented feature: Shader model 5.1+ resource array' "$O/dx12_main.log"; then
    result dx12_main XFAIL "rc=$RC [$adapter, FXC]: Wine's d3dcompiler_47 (vkd3d-shader) has no SM 5.1 resource arrays (wgpu's sampler heap); give WINE_DXC_DIR"
  else
    compare dx12_main "$O/dx12_main.png" "$2"
    result dx12_main no "rc=$RC panic/validation=$BAD [${adapter:-no adapter}, ${comp:-?}] vs Linux: $CMP"
  fi
}

# The map editor. Each side (Linux, Wine) runs in a folder of its own holding `maps/` with a map mod for the shots,
# and names it relatively (`--maps maps`): the opening screen prints the maps folder as given, so both draw the same
# text (the editor also keeps its recent list there).
editor_side() { # DIR: the side's folder, made once
  [ -d "$1/maps/smoke" ] && return 0
  mkdir -p "$1/maps/smoke"; printf 'overbright-map 1\nname = Wine smoke\n' > "$1/maps/smoke/map.obmap"
}
# editor_args NAME: the shot's arguments after --shot/--maps.
editor_args() {
  case $1 in
    e_start) echo "--size 1280x720" ;;
    e_props) echo "--size 1280x720 --mod smoke --mode props --view 3d --tab browser" ;;
    e_races) echo "--size 1600x960 --scale 1.5 --mod smoke --mode races --view map --tab validation" ;;
  esac
}
# editor_reference NAME: the Linux editor's shot (headless: no display variable at all), its path on stdout.
editor_reference() {
  local name=$1; mkdir -p "$O/linux" "$O/tmp/linux-$name/data" "$O/tmp/linux-$name/cache"
  editor_side "$O/editor-linux"
  # shellcheck disable=SC2046
  ( cd "$O/editor-linux" && env -u DISPLAY -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE XDG_DATA_HOME="$O/tmp/linux-$name/data" \
    XDG_CACHE_HOME="$O/tmp/linux-$name/cache" NFSU2_HEADLESS_ONLY=1 RADV_DEBUG=${RADV_DEBUG-nodcc} \
    timeout 900 "$LED" --shot "$O/linux/$name.png" --maps maps $(editor_args "$name") > "$O/linux/$name.log" 2>&1 )
  echo "$O/linux/$name.png"
}
# check_editor NAME EXE REF: the Windows editor's shot on Vulkan. Exit 0, no panic / validation line, a PNG of the
# asked size; e_start and e_races must match the Linux shot within the tolerance. The 3D view (e_props) is not the
# same run to run even on Linux (the world's streaming; hundreds to a few thousand pixels): there the comparison is
# reported, and the view in the middle of the window must not be one flat colour (ImageMagick, when installed).
check_editor() {
  local name=$1 exe=$2 ref=$3
  editor_side "$O/editor-win"
  local want; want=$(editor_args "$name" | grep -oE '[0-9]+x[0-9]+')
  cd "$O/editor-win" || return
  # shellcheck disable=SC2046
  WENV=(WGPU_BACKEND=vulkan); wrun "$name" 1200 "$exe" --shot "$(wp "$O/$name.png")" --maps maps $(editor_args "$name")
  cd - > /dev/null || return
  local ok=ok why="" got cstd
  [ $RC = 0 ] || { ok=no; why="rc=$RC "; }
  [ "$BAD" = 0 ] || { ok=no; why="${why}panic/validation=$BAD "; }
  got=$(python3 -I -c 'import struct,sys; d=open(sys.argv[1],"rb").read(24); print("%dx%d" % struct.unpack(">II", d[16:24]) if d[:8] == b"\x89PNG\r\n\x1a\n" else "none")' "$O/$name.png" 2> /dev/null)
  [ "$got" = "$want" ] || { ok=no; why="${why}image ${got:-none}, not $want "; }
  # The status bar's right end shows the time the last frame's UI took (wall clock, differs run to run): it is
  # blanked in both images before the comparison (220x28 points; ImageMagick, when installed).
  local a="$O/$name.png" b=$ref
  if [ "$got" = "$want" ] && [ -s "$ref" ] && command -v magick > /dev/null; then
    local sc w h; sc=$(editor_args "$name" | grep -oE -- '--scale [0-9.]+' | cut -d' ' -f2); sc=${sc:-1}
    w=${want%x*} h=${want#*x}
    local box; box=$(awk -v w="$w" -v h="$h" -v s="$sc" 'BEGIN { printf "rectangle %d,%d %d,%d", w - 220 * s, h - 28 * s, w, h }')
    mkdir -p "$O/masked"
    magick "$O/$name.png" -fill black -draw "$box" "$O/masked/$name-wine.png" && magick "$ref" -fill black -draw "$box" "$O/masked/$name-linux.png" \
      && a="$O/masked/$name-wine.png" b="$O/masked/$name-linux.png"
  fi
  if [ "$name" = e_props ]; then
    compare "$name" "$a" "$b"; CMP="$CMP (reported only: the 3D view differs run to run)"
    if [ "$got" = "$want" ] && command -v magick > /dev/null; then
      cstd=$(magick "$O/$name.png" -alpha off -colorspace gray -gravity center -crop 30%x30%+0+0 +repage \
        -format '%[fx:standard_deviation]' info: 2> /dev/null)
      awk -v s="${cstd:-0}" 'BEGIN { exit !(s >= 0.02) }' || { ok=no; why="${why}nothing in the view (centre std ${cstd:-?}) "; }
    fi
  else
    compare "$name" "$a" "$b" || { ok=no; why="${why}image over the tolerance "; }
  fi
  result "$name" $ok "${why}$SECS s vs Linux: $CMP"
}

# ---- --env-check: the environment of the runs, on a stand-in for wine (no Wine, no Xvfb, no game) ------------
# As checks.sh and the desktop session leave it (the guard exported, the desktop's X and Wayland displays), the
# windowed steps (w_window, l_play) run through their own code here with a stand-in `wine` that writes the
# environment the Windows program would get and answers as the game does (no window under NFSU2_HEADLESS_ONLY),
# and one headless run (wrun) starts without the guard inherited. Each must hold: the windowed runs reach the
# stand-in's title screen (the guard not inherited), every run is on our display only (DISPLAY = ours, no
# WAYLAND_DISPLAY, our XDG_RUNTIME_DIR), the headless run carries NFSU2_HEADLESS_ONLY itself.
if [ "$ENVCHECK" = 1 ]; then
  export NFSU2_HEADLESS_ONLY=1 WAYLAND_DISPLAY=wayland-0 XDG_SESSION_TYPE=wayland DISPLAY=:0
  SB="$O/stand-in"; rm -rf "$SB" "$O/tmp"; mkdir -p "$SB" "$O/exe" "$O/tmp" "$O/xdg"
  cat > "$SB/wine" << 'EOF'
#!/bin/sh
# wine EXE [ARGS]: the environment the program gets; with no arguments the game's window start (refused under
# NFSU2_HEADLESS_ONLY, as the game does, else its title-screen line). The launcher (overbright.exe) starts the
# game beside it, whose log goes into the launcher's data folder (NFSU2_DATA_DIR, a Z: path).
exe=$1; shift
out=/dev/stdout
case $exe in
  *overbright.exe) d=$(printf '%s' "${NFSU2_DATA_DIR#Z:}" | tr '\\' /); mkdir -p "$d/logs"; out="$d/logs/game-stand-in.log" ;;
esac
{
  echo "env: DISPLAY=${DISPLAY-<unset>}"
  echo "env: WAYLAND_DISPLAY=${WAYLAND_DISPLAY-<unset>}"
  echo "env: XDG_RUNTIME_DIR=${XDG_RUNTIME_DIR-<unset>}"
  echo "env: NFSU2_HEADLESS_ONLY=${NFSU2_HEADLESS_ONLY-<unset>}"
  echo "env: NFSU2_AUDIO_HOST=${NFSU2_AUDIO_HOST-<unset>}"
  if [ $# = 0 ]; then
    if [ -n "${NFSU2_HEADLESS_ONLY+set}" ]; then echo "Error: NFSU2_HEADLESS_ONLY is set: not opening a window (args [])"; exit 1; fi
    # As the game does: a sound device is opened unless the run names no sound host.
    [ "${NFSU2_AUDIO_HOST-}" = none ] || echo "[INFO  nfsu2::audio::mixer::device::cpal] audio: WASAPI output at 48000 Hz"
    echo "boot: the title screen"
  fi
} >> "$out"
EOF
  printf '#!/bin/sh\nprintf "Z:%%s\\n" "$(printf %%s "$2" | tr / "\\\\\\\\")"\n' > "$SB/winepath"
  printf '#!/bin/sh\n' > "$SB/wineserver"
  chmod +x "$SB/wine" "$SB/winepath" "$SB/wineserver"
  PATH="$SB:$PATH"
  # A display name no X server has: nothing (xdotool's Enter included) can reach a real one.
  DNUM=none
  : > "$O/exe/game.exe"; : > "$O/exe/overbright.exe"
  ROOT_W='Z:\game' PRIV_W='Z:\private'
  trap cleanup EXIT
  check_window "$O/exe/game.exe"
  check_play "$O/exe/overbright.exe"
  ( unset NFSU2_HEADLESS_ONLY; WENV=(); wrun env_headless 60 "$O/exe/game.exe" --version )
  glog=$(ls "$O"/tmp/play-data/logs/game-*.log 2> /dev/null | head -1)
  for f in "$O/w_window.log" "${glog:-$O/no-game-log}" "$O/env_headless.log"; do
    n=$(basename "$f" .log)
    ok=ok why=""
    grep -qx "env: DISPLAY=:$DNUM" "$f" 2> /dev/null || { ok=no; why="$why DISPLAY is not ours ($(grep -m1 -o 'DISPLAY=.*' "$f" 2> /dev/null));"; }
    grep -qx 'env: WAYLAND_DISPLAY=<unset>' "$f" 2> /dev/null || { ok=no; why="$why WAYLAND_DISPLAY set;"; }
    grep -qx "env: XDG_RUNTIME_DIR=$O/xdg" "$f" 2> /dev/null || { ok=no; why="$why XDG_RUNTIME_DIR not ours;"; }
    result "env_display_$n" $ok "${why:-our display only}"
  done
  # The windowed runs are never heard: no sound host named for the game.
  for f in "$O/w_window.log" "${glog:-$O/no-game-log}"; do
    ok=ok; grep -qx 'env: NFSU2_AUDIO_HOST=none' "$f" 2> /dev/null || ok=no
    result "env_sound_$(basename "$f" .log)" $ok "$(grep -m1 -o 'NFSU2_AUDIO_HOST=.*' "$f" 2> /dev/null)"
  done
  ok=ok; grep -qx 'env: NFSU2_HEADLESS_ONLY=1' "$O/env_headless.log" || ok=no
  result env_headless $ok "$(grep -m1 -o 'NFSU2_HEADLESS_ONLY=.*' "$O/env_headless.log")"
  if [ ${#FAIL[@]} = 0 ]; then say "WINE_SMOKE_ENVCHECK: PASS ($(( $(date +%s) - T0 )) s)"; exit 0; fi
  say "WINE_SMOKE_ENVCHECK: FAIL (${FAIL[*]}) ($(( $(date +%s) - T0 )) s)"; exit 1
fi

command -v wine > /dev/null || skip "no wine"
command -v Xvfb > /dev/null || skip "no Xvfb"
command -v winepath > /dev/null || skip "no winepath"
ck_need_root
[ -x "$PIXDIFF" ] || ck_die "no pixdiff at $PIXDIFF (cargo build --profile $CK_PROFILE --examples)"
# The Linux game shoots the references (all of them for vk_fallback and without WINE_REF): it needs the harness.
[ "$("$LBIN" --dev-harness 2>&1)" = "dev-harness: on" ] || ck_die "$LBIN is not a dev-harness build (the Linux references)"

# ---- the exe ------------------------------------------------------------------------------------------------
EXE=${WINE_EXE:-}
if [ -z "$EXE" ]; then
  command -v podman > /dev/null || skip "no podman to build game.exe (or set WINE_EXE)"
  B=${WINE_BUILD:-$TGT/wine-smoke-build}
  t=$(date +%s)
  # build.sh copies the tree with its mtimes: when WINE_BUILD last built a different tree with newer files,
  # cargo would call the game crate fresh and hand back that tree's exe. Its fingerprints go, so the game
  # crate (only) is always rebuilt (~1-2 min; the dependencies stay cached).
  rm -rf "$B"/target-windows/x86_64-pc-windows-gnu/release/.fingerprint/nfsu2-*
  JOBS=${JOBS:-4} "$REPO/scripts/release/container/build.sh" --windows --dev-harness --fast --no-check "$REPO" "$B" \
    > "$O/build.log" 2>&1 &
  wait $!; rc=$?
  say "build rc=$rc $(( $(date +%s) - t )) s ($B/build.log)"
  [ $rc = 0 ] || { say "WINE_SMOKE: FAIL (build) ($(( $(date +%s) - T0 )) s)"; exit 1; }
  EXE=$B/dist/windows-dev/game.exe
fi
[ -f "$EXE" ] || ck_die "no exe $EXE"
EEXE=${WINE_EDITOR:-$(dirname "$EXE")/../windows/overbright-editor.exe}
LED=${LINUX_EDITOR:-$TGT/$CK_PDIR/overbright-editor}
# The exe in a folder of its own (the DXC goes next to it); the asset cache, profiles and temp under OUTDIR.
mkdir -p "$O/exe" "$O/tmp"
cp -f "$EXE" "$O/exe/game.exe"
if [ -n "${WINE_DXC_DIR:-}" ]; then
  for f in dxcompiler.dll dxil.dll; do [ -f "$WINE_DXC_DIR/$f" ] && cp -f "$WINE_DXC_DIR/$f" "$O/exe/"; done
fi
say "exe $EXE md5 $(md5sum < "$EXE" | cut -c1-12)${WINE_DXC_DIR:+ dxc from $WINE_DXC_DIR}"

# ---- Xvfb and the prefix -------------------------------------------------------------------------------------
export WINEPREFIX="$O/prefix" WINEDEBUG=${WINEDEBUG:--all} WINEDLLOVERRIDES="mscoree=;mshtml=;winealsa.drv=;winepulse.drv=;wineoss.drv=;winecoreaudio.drv="
if [ -n "${WINE_DISPLAY:-}" ]; then   # a display of your own (else Xvfb picks a free one)
  echo "${WINE_DISPLAY#:}" > "$O/xvfb.display"
  Xvfb ":${WINE_DISPLAY#:}" -screen 0 1280x960x24 -nolisten tcp > "$O/xvfb.log" 2>&1 &
  XPID=$!; sleep 1
else
  exec 9> "$O/xvfb.display"
  Xvfb -displayfd 9 -screen 0 1280x960x24 -nolisten tcp > "$O/xvfb.log" 2>&1 &
  XPID=$!
  exec 9>&-
fi
for _ in $(seq 100); do [ -s "$O/xvfb.display" ] && break; kill -0 $XPID 2> /dev/null || break; sleep 0.1; done
DNUM=$(tr -dc '0-9' < "$O/xvfb.display")
{ [ -n "$DNUM" ] && kill -0 $XPID 2> /dev/null; } || { kill $XPID 2> /dev/null; skip "Xvfb did not start ($O/xvfb.log)"; }
trap cleanup EXIT
mkdir -p "$O/xdg"; chmod 700 "$O/xdg"
t=$(date +%s)
W wineboot -i > "$O/wineboot.log" 2>&1 &
wait $!; rc=$?
# Pinned to X11 right after the first boot (never Wine's Wayland driver).
W wine reg add 'HKCU\Software\Wine\Drivers' /v Graphics /d x11 /f >> "$O/wineboot.log" 2>&1 &
wait $!; rx=$?
# No sound driver in this prefix: a check must never be heard. Wine's ALSA driver opens the machine's sound cards
# directly, past the private runtime folder, so the windowed runs would play the title music on the speakers.
W wine reg add 'HKCU\Software\Wine\Drivers' /v Audio /d '' /f >> "$O/wineboot.log" 2>&1 &
wait $!; ra=$?
[ $ra = 0 ] || skip "the prefix's sound driver could not be switched off (rc=$ra, $O/wineboot.log): no Wine run without it"
say "prefix $WINEPREFIX on Xvfb :$DNUM: wineboot rc=$rc, pinned to x11 rc=$rx, sound driver off rc=$ra $(( $(date +%s) - t )) s"
ROOT_W=$(wp "$NFSU2_ROOT")
# The private set (docs/developing/testing.md "Private data") as Wine sees it: a harness built with it (in the container,
# from /src) looks for it there unless OB_PRIVATE names it; the repository holds it until it moves out.
PRIV=${OB_PRIVATE:-$REPO}
PRIV_W=$(wp "$PRIV")
say "private set $PRIV as $PRIV_W"

if [ "$SELFCHECK" = 0 ]; then
  check_window "$O/exe/game.exe"
  check_harness "$O/exe/game.exe"
  check_version "$O/exe/game.exe"
  check_game "$O/exe/game.exe" "$ROOT_W"
  check_selftest "$O/exe/game.exe"
  for s in $SHOTS; do check_shot "vk_$s" "$O/exe/game.exe" "$s" "$(reference "$s" "$s")"; done
  check_shot vk_fallback "$O/exe/game.exe" explore "$(reference explore explore-drop NFSU2_GPU_DROP=multiview,pipeline_cache)" \
    NFSU2_GPU_DROP=multiview,pipeline_cache
  grep -q 'runs without optional features, using fallbacks: multiview' "$O/vk_fallback.log" \
    || result vk_fallback_log no "(the log does not name the multiview fallback)"
  [ "${WINE_DX12:-1}" = 0 ] || check_dx12 "$O/exe/game.exe" "$(reference main main)"
  check_prepare "$O/exe/game.exe"
  [ "${WINE_DX12:-1}" = 0 ] || check_prepare_dx12 "$O/exe/game.exe"
else
  # Each check on a broken input: the exe cut to its first 64 KiB, a folder that is not the game, a shot
  # compared with another target's image, a log with a panic line. Every one must fail.
  head -c 65536 "$O/exe/game.exe" > "$O/exe/broken.exe"
  mkdir -p "$O/not-the-game"
  EMPTY_W=$(wp "$O/not-the-game")
  check_harness "$O/exe/broken.exe"
  check_version "$O/exe/broken.exe"
  check_game "$O/exe/game.exe" "$EMPTY_W"
  check_selftest "$O/exe/broken.exe"
  check_prepare "$O/exe/broken.exe"
  check_shot vk_broken_exe "$O/exe/broken.exe" main "$(reference main main)"
  check_shot vk_wrong_ref "$O/exe/game.exe" main "$(reference race race)"
  # The panic grep: a good run's log with one panic line appended must count as bad.
  cp "$O/vk_wrong_ref.log" "$O/panic_probe.log"; echo "thread 'main' panicked at src/x.rs:1:1:" >> "$O/panic_probe.log"
  [ "$(ck_bad "$O/panic_probe.log")" != 0 ] && result panic_grep no "a planted panic line is found" || result panic_grep ok "planted panic line missed"
  check_dx12 "$O/exe/broken.exe" "$(reference main main)"
  # The editor: a cut exe, and a good one compared with another screen's Linux image.
  if [ -f "$EEXE" ] && [ -x "$LED" ]; then
    cp -f "$EEXE" "$O/exe/overbright-editor.exe"
    head -c 65536 "$EEXE" > "$O/exe/broken-editor.exe"
    check_editor e_start "$O/exe/broken-editor.exe" "$(editor_reference e_start)"
    check_editor e_start "$O/exe/overbright-editor.exe" "$(editor_reference e_races)"
  else
    result e_missing no "(no editor exe $EEXE or no Linux editor $LED: its checks cannot be tested)"
  fi
  # Expected: every step failed.
  N=$(grep -cE '^[a-z_0-9]+: ' "$REP"); NF=${#FAIL[@]}
  say "selfcheck: $NF of $(( NF + $(grep -cE '^[a-z_0-9]+: (ok|XFAIL) ' "$REP") )) checks failed on broken input (all must)"
  if [ "$(grep -cE '^[a-z_0-9]+: (ok|XFAIL) ' "$REP")" = 0 ]; then
    say "WINE_SMOKE_SELFCHECK: PASS ($(( $(date +%s) - T0 )) s)"; exit 0
  fi
  say "WINE_SMOKE_SELFCHECK: FAIL (a check passed on broken input: $(grep -E '^[a-z_0-9]+: (ok|XFAIL) ' "$REP" | cut -d: -f1 | tr '\n' ' ')) ($(( $(date +%s) - T0 )) s)"
  exit 1
fi

# ---- the map editor ---------------------------------------------------------------------------------------------
if [ "$SELFCHECK" = 0 ]; then
  if [ -f "$EEXE" ] && [ -x "$LED" ]; then
    cp -f "$EEXE" "$O/exe/overbright-editor.exe"
    say "editor $EEXE md5 $(md5sum < "$EEXE" | cut -c1-12)"
    for e in e_start e_props e_races; do check_editor "$e" "$O/exe/overbright-editor.exe" "$(editor_reference "$e")"; done
  elif [ -z "${WINE_EXE:-}" ]; then
    result editor no "(the build made here has no $EEXE, or no Linux editor $LED)"
  else
    say "editor: SKIP (no $EEXE or no Linux editor $LED)"
  fi
fi

# ---- the launcher ---------------------------------------------------------------------------------------------
LEXE=${WINE_LAUNCHER:-$(dirname "$EXE")/../windows/overbright.exe}
LOB=${LINUX_LAUNCHER:-$TGT/$CK_PDIR/overbright}
if [ "$SELFCHECK" = 0 ] && [ -f "$LEXE" ] && [ -x "$LOB" ]; then
  cp -f "$LEXE" "$O/exe/overbright.exe"
  say "launcher $LEXE md5 $(md5sum < "$LEXE" | cut -c1-12)"
  LDATA="$O/tmp/launcher-data"; mkdir -p "$LDATA"
  LENV=(NFSU2_DATA_DIR="$(wp "$LDATA")" NFSU2_NO_PADS=1)
  for st in mods state:mods-hud-set-up state:mods-hud-waiting state:browse-hud first-run; do
    s=${st#state:}
    WENV=("${LENV[@]}"); wrun "l_render_$s" 300 "$O/exe/overbright.exe" --render "$s" --size 1280x720 "$(wp "$O/l_render_$s.png")"
    mkdir -p "$O/linux"
    env -u DISPLAY -u WAYLAND_DISPLAY "$LOB" --render "$s" --size 1280x720 "$O/linux/l_render_$s.png" > /dev/null 2>&1
    ok=ok; [ $RC = 0 ] && [ "$BAD" = 0 ] || ok=no
    compare "l_render_$s" "$O/l_render_$s.png" "$O/linux/l_render_$s.png" || ok=no
    result "l_render_$s" $ok "rc=$RC vs Linux: $CMP"
  done
  # A long folder with accents as a drive-letter path, past MAX_PATH (260) whatever the length of OUTDIR: Rust's
  # std hands such paths to Windows in the \\?\ form, which has no MAX_PATH limit (the manifest's longPathAware
  # covers the rest). Folders are added until the Windows path is over 300 characters.
  LONG="$O/tmp/Données été" i=0
  while [ $(( ${#LONG} + 7 )) -le 300 ]; do i=$((i + 1)); LONG="$LONG/a-rather-long-folder-name-for-the-path-test-$i"; done
  LONG="$LONG/mods"
  mkdir -p "$LONG"
  python3 -I "$REPO/scripts/checks/hud_mod_standin.py" "$LONG" > /dev/null
  printf '7z\xbc\xaf\x27\x1c' > "$LONG/hud-part-example.7z"
  LONG_W=$(wp "$LONG")
  WENV=("${LENV[@]}"); wrun l_list 120 "$O/exe/overbright.exe" --list-folder "$LONG_W" hud
  ok=ok; [ $RC = 0 ] && grep -q $'^map-part-example.zip\tZIP archive' "$O/l_list.log" \
    && grep -q $'^hud-part-example.7z\t7-Zip archive' "$O/l_list.log" && grep -q $'^hud-part-example\t' "$O/l_list.log" || ok=no
  result l_list $ok "rc=$RC ${#LONG_W}-character path: $(grep -c . "$O/l_list.log") rows"
  WENV=("${LENV[@]}"); wrun l_hud 600 "$O/exe/overbright.exe" --add-hud-mod "$LONG_W\\hud-part-example.7z" "$LONG_W\\map-part-example.zip"
  ok=ok; [ $RC = 0 ] && [ "$BAD" = 0 ] && grep -q '^ok: HUD mod set up: 595 HUD images and 115 maps' "$O/l_hud.log" \
    && [ "$(find "$LDATA" -path '*hud-mod/*.png' | wc -l)" = 710 ] || ok=no
  result l_hud $ok "rc=$RC $SECS s: $(grep -m1 -E '^(ok|error|warning):' "$O/l_hud.log" | cut -c1-90)"
  python3 -I - "$LONG/other-game.tpf" << 'PY'
import sys, zipfile
with zipfile.ZipFile(sys.argv[1], 'w') as z:
    z.writestr('GTA-SA.EXE_0x12345678.dds', b'DDS ' + bytes(124) + bytes(8))
PY
  WENV=("${LENV[@]}"); wrun l_tpf 600 "$O/exe/overbright.exe" --add-packages "$LONG_W\\other-game.tpf"
  ok=ok; [ "$BAD" = 0 ] && grep -q 'made for another game (GTA-SA.EXE)' "$O/l_tpf.log" || ok=no
  result l_tpf $ok "rc=$RC $(grep -m1 -E '^(ok|error):' "$O/l_tpf.log" | cut -c1-100)"
  mkdir -p "$O/l_start"
  W env WINEPREFIX="$WINEPREFIX" OB_WINE=1 NFSU2_ROOT="$NFSU2_ROOT" sh "$REPO/tests/launcher_start.sh" "$O/exe/overbright.exe" ":$DNUM" \
    "$O/l_start" "$O/exe/game.exe" > "$O/l_start.log" 2>&1 &
  wait $!; RC=$?
  ok=ok; [ $RC = 0 ] && tail -1 "$O/l_start.log" | grep -q '^PASS' || ok=no
  result l_start $ok "rc=$RC $(grep -E '^(FAIL|PASS)' "$O/l_start.log" | tail -1 | cut -c1-100)"
  # The launcher by the mouse alone (tests/launcher_mouse.sh under Wine): the browser, the drives, the game folder
  # chosen, a package added; the home folder is this prefix's profile folder.
  mkdir -p "$O/l_mouse"
  W env WINEPREFIX="$WINEPREFIX" OB_WINE=1 NFSU2_ROOT="$NFSU2_ROOT" bash "$REPO/tests/launcher_mouse.sh" "$O/exe/overbright.exe" ":$DNUM" \
    "$O/l_mouse" > "$O/l_mouse.log" 2>&1 &
  wait $!; RC=$?
  ok=ok; [ $RC = 0 ] && tail -1 "$O/l_mouse.log" | grep -q '^MOUSE: PASS' || ok=no
  result l_mouse $ok "rc=$RC $(grep -E '^(FAIL|MOUSE)' "$O/l_mouse.log" | head -2 | tr '\n' ' ' | cut -c1-140)"
  check_play "$O/exe/overbright.exe"
else
  [ "$SELFCHECK" = 0 ] && say "launcher: SKIP (no $LEXE or no Linux launcher $LOB)"
fi

if command -v xdotool > /dev/null; then
  say "windows on :$DNUM at the end: $(DISPLAY=":$DNUM" xdotool search --onlyvisible --name . 2> /dev/null | wc -l) (the shots are headless)"
fi
SECS=$(( $(date +%s) - T0 ))
X=""; [ ${#XFAIL[@]} -gt 0 ] && X=" xfail: ${XFAIL[*]};"
if [ ${#FAIL[@]} = 0 ]; then say "WINE_SMOKE: PASS (${X# } $SECS s)"; exit 0; fi
say "WINE_SMOKE: FAIL (${FAIL[*]};$X $SECS s)"
exit 1
