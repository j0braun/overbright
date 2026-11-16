#!/bin/bash
# A private sound server for the clone's device checks (docs/formats.md "No in-game sound"):
#   scripts/audio_sandbox.sh [--pulse] [--no-wireplumber] CMD...
# runs CMD in bwrap with its own PID namespace and /proc, no /dev/snd, an empty /run/user/<uid> and
# /run/dbus, no Wayland/X, and its own PipeWire + WirePlumber (device monitors off) with one null sink
# "nfs" (48 kHz stereo); --pulse adds pipewire-pulse (and a private cookie). The servers' PIDs are in
# SB_PW, SB_WP, SB_PP and the state folder in SB_S (under $NFSU2_SB_DIR, default $TMPDIR or /tmp).
# Inside, `pw-metadata -n settings 0 clock.force-quantum 4096` forces an xrun on every stream, and
# `pw-record --target nfs -P '{ stream.capture.sink = true }' out.wav` records the sink. Kill only by
# PID (SB_*), never by name.
set -u
if [ "${1:-}" != --inner ]; then
  exec bwrap --unshare-pid --dev-bind / / --proc /proc --tmpfs /dev/snd --tmpfs "/run/user/$(id -u)" --tmpfs /run/dbus \
    --unsetenv WAYLAND_DISPLAY --unsetenv DISPLAY --die-with-parent "$(realpath "$0")" --inner "$@"
fi
shift
PULSE= WP=1
while [ $# -gt 0 ]; do
  case "$1" in --pulse) PULSE=1; shift;; --no-wireplumber) WP=; shift;; *) break;; esac
done
S=$(mktemp -d "${NFSU2_SB_DIR:-${TMPDIR:-/tmp}}/nfsu2-audio-sb.XXXXXX")
mkdir -p "$S/run" "$S/home/.config/pulse" "$S/cfg/pipewire/pipewire.conf.d" "$S/cfg/wireplumber/wireplumber.conf.d" "$S/state" "$S/data"
chmod 700 "$S/run"
export XDG_RUNTIME_DIR=$S/run HOME=$S/home XDG_CONFIG_HOME=$S/cfg XDG_STATE_HOME=$S/state XDG_DATA_HOME=$S/data
unset DBUS_SESSION_BUS_ADDRESS PULSE_SERVER PIPEWIRE_REMOTE
head -c 256 /dev/urandom > "$S/home/.config/pulse/cookie"
cat > "$S/cfg/pipewire/pipewire.conf.d/null.conf" <<'C'
context.objects = [
  { factory = adapter args = { factory.name = support.null-audio-sink node.name = "nfs" node.description = "nfs" media.class = "Audio/Sink" audio.position = [ FL FR ] audio.rate = 48000 monitor.channel-volumes = true } }
]
C
cat > "$S/cfg/wireplumber/wireplumber.conf.d/nodev.conf" <<'C'
wireplumber.profiles = { main = { monitor.v4l2 = disabled monitor.libcamera = disabled monitor.bluez = disabled monitor.bluez-midi = disabled monitor.alsa = disabled monitor.alsa-midi = disabled support.dbus = disabled support.reserve-device = disabled support.portal-permissionstore = disabled } }
C
SB_PW= SB_WP= SB_PP=
pipewire > "$S/pw.log" 2>&1 & SB_PW=$!
sleep 0.5
[ -n "$WP" ] && { wireplumber > "$S/wp.log" 2>&1 & SB_WP=$!; }
[ -n "$PULSE" ] && { pipewire-pulse > "$S/pp.log" 2>&1 & SB_PP=$!; }
sleep 1.5
export SB_PW SB_WP SB_PP SB_S=$S
echo "audio sandbox: $S (pipewire $SB_PW, wireplumber ${SB_WP:-none}, pipewire-pulse ${SB_PP:-none})" >&2
"$@"; rc=$?
kill $SB_PP $SB_WP $SB_PW 2>/dev/null; wait 2>/dev/null
exit $rc
