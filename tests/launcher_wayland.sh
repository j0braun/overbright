#!/bin/bash
# The launcher's folder browser by the mouse on Wayland at fractional and whole scales: a private headless
# mutter (its own D-Bus without any activatable service, its own XDG_RUNTIME_DIR and socket name, never the
# desktop's), the launcher as a Wayland client of it, the pointer driven through mutter's RemoteDesktop API
# (tests/launcher_wayland.py). Per scale: a click on Browse… opens the browser; a wheel notch over the list
# scrolls it 3 rows; 100 logical px of touchpad scrolling scroll it by whole rows (nothing lost); a double
# click on a row opens it. Positions are checked in winit's terms: the cursor in physical pixels, the
# scale factor as the monitor's.
#
# Usage: tests/launcher_wayland.sh <overbright> <work dir> [scale…]   (default scales: 1 1.25 1.5 2)
# Needs mutter, dbus-run-session and a python3 with gi (PyGObject; /usr/bin/python3 is tried first).
# Last line: "WAYLAND: PASS", "WAYLAND: FAIL (<scales>)" or "WAYLAND: SKIP (<why>)" (exit 3).
set -u -o pipefail
OB=$(cd "$(dirname "${1:?overbright}")" && pwd)/$(basename "$1") W=${2:?work dir}
shift 2
SCALES=${*:-1 1.25 1.5 2}
HERE=$(cd "$(dirname "$0")" && pwd)
PY=/usr/bin/python3; [ -x "$PY" ] || PY=$(command -v python3)
for t in mutter dbus-run-session; do command -v $t > /dev/null || { echo "WAYLAND: SKIP (no $t)"; exit 3; }; done
"$PY" -I -c 'from gi.repository import Gio' 2> /dev/null || { echo "WAYLAND: SKIP (no python gi)"; exit 3; }
mkdir -p "$W"; W=$(cd "$W" && pwd)

if [ "${1:-}" != --inner ] && [ -z "${OB_WAYLAND_INNER:-}" ]; then
  FAILED=()
  for sc in $SCALES; do
    O=$W/scale-$sc
    rm -rf "$O"; mkdir -p "$O/xdg" "$O/user-home" "$O/data" "$O/config" "$O/cache"; chmod 700 "$O/xdg"
    for i in $(seq -w 1 30); do mkdir -p "$O/user-home/Folder $i/Inside"; done
    # A session bus of its own: no service directories, so nothing is activated on it.
    cat > "$O/bus.conf" << EOF
<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
<busconfig>
  <type>session</type>
  <listen>unix:dir=$O/xdg</listen>
  <auth>EXTERNAL</auth>
  <policy context="default"><allow send_destination="*" eavesdrop="true"/><allow eavesdrop="true"/><allow own="*"/></policy>
</busconfig>
EOF
    env -i PATH="$PATH" HOME="$O/user-home" XDG_RUNTIME_DIR="$O/xdg" XDG_CONFIG_HOME="$O/config" XDG_DATA_HOME="$O/data" \
      XDG_CACHE_HOME="$O/cache" LANG=C.UTF-8 OB_WAYLAND_INNER=1 \
      timeout 120 dbus-run-session --config-file="$O/bus.conf" -- bash "$0" "$OB" "$O" "$sc" > "$O/run.log" 2>&1
    echo "== scale $sc"; grep -vE '^(libmutter|$)' "$O/run.log" | grep -E '^(monitor|scale|a |100|RESULT)' | sed 's/^/  /'
    grep -q '^RESULT PASS' "$O/run.log" || FAILED+=("$sc")
  done
  [ ${#FAILED[@]} = 0 ] && { echo "WAYLAND: PASS"; exit 0; }
  echo "WAYLAND: FAIL (${FAILED[*]})"; exit 1
fi

# ---- inside the session ---------------------------------------------------------------------------------
O=$W SC=$1
SOCK=ob-launcher-wayland
mutter --headless --wayland --no-x11 --virtual-monitor 1920x1080 --wayland-display=$SOCK > "$O/mutter.log" 2>&1 &
MPID=$!
for _ in $(seq 50); do [ -S "$XDG_RUNTIME_DIR/$SOCK" ] && break; sleep 0.2; done
[ -S "$XDG_RUNTIME_DIR/$SOCK" ] || { echo "RESULT FAIL mutter did not start"; kill $MPID; exit 1; }
sleep 1
"$PY" -I "$HERE/launcher_wayland.py" scale "$SC"
L=$O/launcher.out
(cd "$HOME" && exec env WAYLAND_DISPLAY=$SOCK NFSU2_NO_PADS=1 OVERBRIGHT_OWN_PICKER=1 NFSU2_CACHE_DIR="$O/cache" \
  RUST_LOG="info,launcher::screen=debug,launcher::input=debug" "$OB" > "$L" 2>&1) &
LPID=$!
sleep 2
"$PY" -I "$HERE/launcher_wayland.py" drive "$L" "$SC"
kill $LPID 2> /dev/null; wait $LPID 2> /dev/null
kill $MPID 2> /dev/null; wait $MPID 2> /dev/null
