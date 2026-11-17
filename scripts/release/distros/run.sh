#!/bin/bash
# The Linux player archive on the common distributions, each in a container (podman, rootless): does it
# start, find its libraries, check the game, draw frames, show the launcher? docs/building-releases.md, "Linux:
# what a player needs".
#
#   run.sh [--keep-images] [--novk | --novk-only] <overbright-…-linux-x86_64.tar.gz | unpacked folder>
#          <out dir> [name…]
#
# For each distribution of images.txt (or the names given):
#   <name>-bare  the image as pulled: ldd and --version (what a minimal install lacks);
#   <name>-full  plus what a desktop has (the Vulkan loader, Mesa's software driver lavapipe, Xvfb, the
#                X11, xkbcommon, ALSA and udev libraries): --version, --check-game against $NFSU2_ROOT
#                (mounted read-only), --probe on lavapipe, the harness game's headless shots ($SHOTS, real
#                frames), the launcher on an X server inside the container and one screenshot.
# --novk adds what a player without Vulkan sees, on Ubuntu 24.04: no Vulkan loader at all (novk: libvulkan1,
# which Xvfb's GL packages pull in, is removed again) and the loader without a driver (nodrv): the probe's
# line, the game's window start, the launcher's screen.
# --novk-only runs just those two.
#
# Nothing is heard or shown: the runs have no network, no devices (no /dev/snd, no /dev/dri), none of the
# host's display or runtime variables, and their X server is the container's own (Xvfb).
#
# Environment: NFSU2_ROOT (the game; required), DEV_GAME (a harness game of the same build, e.g.
# container/build.sh --dev-harness → dist/linux-dev/game; without it no shots), SHOTS (default
# "main explore"), PAR (containers at a time, default 3), CPUS (each, default 4), PODMAN.
# Out dir: one folder per run (results.txt, the shots, launcher.png), summary.txt (one line per
# distribution). The images it builds (localhost/overbright-distro-<name>[-novk]) are removed at the end
# unless --keep-images; the pulled base images are listed, not removed.
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
PODMAN=${PODMAN:-podman}
KEEP=0 NOVK=0 MATRIX=1
while [ $# -gt 0 ]; do
    case $1 in
        --keep-images) KEEP=1 ;;
        --novk) NOVK=1 ;;
        --novk-only) NOVK=1 MATRIX=0 ;;
        -h|--help) sed -n '2,32p' "$0"; exit 0 ;;
        -*) echo "run.sh: unknown option $1" >&2; exit 2 ;;
        *) break ;;
    esac
    shift
done
[ $# -ge 2 ] || { sed -n '2,32p' "$0" >&2; exit 2; }
[ -d "${NFSU2_ROOT:-/nonexistent}" ] || { echo "run.sh: NFSU2_ROOT is not a folder" >&2; exit 2; }
SRC=$1
OUT=$(mkdir -p "$2" && cd "$2" && pwd)
shift 2
if [ -f "$SRC" ]; then
    mkdir -p "$OUT/archive"
    tar -xzf "$SRC" -C "$OUT/archive" || exit 1
    A=$(dirname "$(find "$OUT/archive" -maxdepth 2 -name overbright -type f | head -1)")
else
    A=$(cd "$SRC" && pwd)
fi
[ -x "$A/game" ] && [ -x "$A/overbright" ] || { echo "run.sh: no game and overbright in $A" >&2; exit 2; }
DEV=$OUT/dev-game
mkdir -p "$DEV"
[ -n "${DEV_GAME:-}" ] && cp -p "$DEV_GAME" "$DEV/game"
NAMES=${*:-$(grep -v '^#' "$HERE/images.txt" | awk 'NF {print $1}')}

# The test image of a distribution: the packages a desktop install has (Vulkan: loader + lavapipe).
packages() { # packages <family> <vk 0|1>
    local x11
    case $1 in
        deb|deb64|deb-eol)
            # deb-eol: a release past its security support (its security archive is gone from the mirrors).
            local asound=libasound2 eol=""; [ $1 = deb64 ] && asound=libasound2t64
            [ $1 = deb-eol ] && eol="sed -i '/-security/d' /etc/apt/sources.list && "
            echo "${eol}apt-get update -qq && DEBIAN_FRONTEND=noninteractive apt-get install -y -qq --no-install-recommends \
$asound libudev1 libx11-6 libx11-xcb1 libxcursor1 libxi6 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 xvfb \
$([ $2 = 1 ] && echo libvulkan1 mesa-vulkan-drivers) && rm -rf /var/lib/apt/lists/*\
$([ $2 = 0 ] && echo " && (dpkg -r --force-depends libvulkan1 > /dev/null 2>&1 || true)")" ;;
        rpm)
            echo "dnf -y -q install --setopt=install_weak_deps=False alsa-lib systemd-libs libX11 libX11-xcb libXcursor \
libXi libxkbcommon libxkbcommon-x11 libwayland-client xorg-x11-server-Xvfb $([ $2 = 1 ] && echo vulkan-loader mesa-vulkan-drivers) \
&& dnf clean all" ;;
        arch)
            echo "pacman -Syu --noconfirm --needed alsa-lib systemd-libs libx11 libxcursor libxi libxkbcommon libxkbcommon-x11 \
wayland xorg-server-xvfb $([ $2 = 1 ] && echo vulkan-icd-loader vulkan-swrast) && pacman -Scc --noconfirm" ;;
        suse)
            echo "zypper -n --gpg-auto-import-keys in --no-recommends libasound2 libudev1 libX11-6 libX11-xcb1 libXcursor1 \
libXi6 libxkbcommon0 libxkbcommon-x11-0 libwayland-client0 xorg-x11-server-Xvfb $([ $2 = 1 ] && echo libvulkan1 libvulkan_lvp) \
&& zypper clean -a" ;;
    esac
}
image() { # image <name> <vk 0|1>: the test image's tag (built when missing; up to three tries: mirrors fail)
    local n=$1 vk=$2 base fam tag
    base=$(awk -v n="$n" '$1 == n {print $2}' "$HERE/images.txt")
    fam=$(awk -v n="$n" '$1 == n {print $3}' "$HERE/images.txt")
    tag=localhost/overbright-distro-$n$([ "$vk" = 0 ] && echo -novk || true)
    if ! "$PODMAN" image exists "$tag"; then
        local d=$OUT/images/$n-$vk
        mkdir -p "$d"
        printf 'FROM %s\nRUN %s\n' "$base" "$(packages "$fam" "$vk")" > "$d/Containerfile"
        for _ in 1 2 3; do
            "$PODMAN" build -q -t "$tag" -f "$d/Containerfile" "$d" > "$d/build.log" 2>&1 && break
        done
    fi
    "$PODMAN" image exists "$tag" && echo "$tag"
}
run1() { # run1 <name> <mode> <image>
    local o=$OUT/$1-$2
    mkdir -p "$o"
    "$PODMAN" run --rm --network=none --pull=never --cpus="${CPUS:-4}" \
        --mount type=bind,src="$A",dst=/a,ro --mount type=bind,src="$DEV",dst=/dev-game,ro \
        --mount "type=bind,src=$NFSU2_ROOT,dst=/game,ro" --mount type=bind,src="$o",dst=/out \
        --mount type=bind,src="$HERE/inside.sh",dst=/inside.sh,ro \
        -e MODE="$2" -e SHOTS="${SHOTS:-main explore}" -e LP_NUM_THREADS="${CPUS:-4}" \
        "$3" bash /inside.sh > "$o/console.log" 2>&1
    if [ -f "$o/launcher.xwd" ]; then
        { command -v magick > /dev/null && magick "xwd:$o/launcher.xwd" "$o/launcher.png"; } \
            || { command -v convert > /dev/null && convert "xwd:$o/launcher.xwd" "$o/launcher.png"; } \
            && rm -f "$o/launcher.xwd"
    fi
}
one() { # one <name>: bare and full
    local n=$1 base img
    base=$(awk -v n="$n" '$1 == n {print $2}' "$HERE/images.txt")
    [ -n "$base" ] || { echo "$n: not in images.txt" > "$OUT/$n.error"; return; }
    "$PODMAN" image exists "$base" || "$PODMAN" pull -q "$base" > /dev/null 2>&1 || "$PODMAN" pull -q "$base" > /dev/null 2>&1
    run1 "$n" bare "$base"
    if img=$(image "$n" 1); then run1 "$n" full "$img"; else echo "$n: the test image could not be built" > "$OUT/$n.error"; fi
}
export -f one run1 image packages
export OUT A DEV HERE PODMAN
[ $MATRIX = 1 ] || NAMES=""
[ -z "$NAMES" ] || printf '%s\n' $NAMES | xargs -P "${PAR:-3}" -I{} bash -c 'one {}'
if [ $NOVK = 1 ]; then
    if img=$(image ubuntu-24.04 0); then run1 ubuntu-24.04 novk "$img"; fi
    if img=$(image ubuntu-24.04 1); then run1 ubuntu-24.04 nodrv "$img"; fi
fi

# The summary: one line per distribution.
{
    [ -z "$NAMES" ] || python3 -I "$HERE/summary.py" "$OUT" $NAMES
    for m in novk nodrv; do
        [ -f "$OUT/ubuntu-24.04-$m/results.txt" ] && grep -E "^RESULT $m " "$OUT/ubuntu-24.04-$m/results.txt" | sed 's/^RESULT /ubuntu-24.04 /'
    done
} | tee "$OUT/summary.txt"
if [ $KEEP = 0 ]; then
    for n in $NAMES ubuntu-24.04; do
        for t in "localhost/overbright-distro-$n" "localhost/overbright-distro-$n-novk"; do
            "$PODMAN" image exists "$t" && "$PODMAN" rmi -f "$t" > /dev/null
        done
    done
fi
echo "base images used (not removed): $(for n in $NAMES; do awk -v n="$n" '$1 == n {printf "%s ", $2}' "$HERE/images.txt"; done)"
