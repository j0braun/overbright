#!/bin/bash
# The release build in containers (podman): the Linux binaries (AlmaLinux 8 image, Containerfile.linux:
# glibc 2.28 baseline) and the Windows ones (Ubuntu 22.04 image, Containerfile: x86_64-pc-windows-gnu,
# mingw-w64) from one commit or one source tree. See README.md here.
#
#   build.sh [options] <commit | source dir> <out dir>
#     --linux | --windows   only that target (default: both)
#     --dev-harness         also the game with the developer harness (<out>/dist/<target>-dev/; never shipped)
#     --tests               also `cargo test --no-run --features dev-harness` for each target
#     --fast                no LTO, 16 codegen units: a check build, not a release
#     --no-check            skip the checks on the result (check_linux.sh, check_pe.py)
#   <commit>: built from `git archive` of the repository (REPO, default: the tree holding this script).
#   <source dir>: a working tree, copied without its git-ignored files.
#
# Environment: JOBS (default 4: cargo jobs and the container's CPU limit), PODMAN (default podman),
# OB_SOURCE_URL (where the source is published; build.rs passes it on, docs/releasing.md),
# IMAGE (the Windows image, default overbright-build:<hash of Containerfile>), IMAGE_LINUX (default
# overbright-build-linux:<hash of Containerfile.linux>); each is built when missing.
# Out dir: src/ (the copied tree), cargo-home/ (the crate cache, reused), target-linux/, target-windows/
# (cargo's target directories, reused while their image stays the same), dist/<target>/ (the binaries),
# build.log.
set -euo pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(cd "$HERE/../../.." && pwd)}
PODMAN=${PODMAN:-podman}
JOBS=${JOBS:-4}
TARGETS="" DEV=0 TESTS=0 FAST=0 CHECK=1
while [ $# -gt 0 ]; do
    case $1 in
        --linux) TARGETS="$TARGETS linux" ;;
        --windows) TARGETS="$TARGETS windows" ;;
        --dev-harness) DEV=1 ;;
        --tests) TESTS=1 ;;
        --fast) FAST=1 ;;
        --no-check) CHECK=0 ;;
        -h|--help) sed -n '2,18p' "$0"; exit 0 ;;
        -*) echo "build.sh: unknown option $1" >&2; exit 2 ;;
        *) break ;;
    esac
    shift
done
[ $# -eq 2 ] || { sed -n '2,18p' "$0" >&2; exit 2; }
SRC_ARG=$1
OUT=$(mkdir -p "$2" && cd "$2" && pwd)
TARGETS=${TARGETS:-linux windows}

# The source tree and its version stamp.
rm -rf "$OUT/src"
mkdir -p "$OUT/src" "$OUT/cargo-home" "$OUT/target-linux" "$OUT/target-windows" "$OUT/dist"
if [ -d "$SRC_ARG" ]; then
    src=$(cd "$SRC_ARG" && pwd)
    rsync -a --exclude /.git --exclude '/target' --exclude '/target-*' --filter=':- .gitignore' "$src/" "$OUT/src/"
    if git -C "$src" rev-parse --git-dir >/dev/null 2>&1; then
        OB_COMMIT=$(git -C "$src" rev-parse HEAD)
        [ -z "$(git -C "$src" status --porcelain --untracked-files=no)" ] || OB_COMMIT="$OB_COMMIT-modified"
        OB_VERSION=$(git -C "$src" describe --tags --always --dirty)
        SOURCE_DATE_EPOCH=$(git -C "$src" log -1 --format=%ct)
    else
        OB_COMMIT=unknown OB_VERSION="" SOURCE_DATE_EPOCH=$(date +%s)
    fi
else
    OB_COMMIT=$(git -C "$REPO" rev-parse --verify "$SRC_ARG^{commit}")
    git -C "$REPO" archive --format=tar "$OB_COMMIT" | tar -x -C "$OUT/src"
    OB_VERSION=$(git -C "$REPO" describe --tags --always "$OB_COMMIT")
    SOURCE_DATE_EPOCH=$(git -C "$REPO" log -1 --format=%ct "$OB_COMMIT")
fi
[[ $OB_VERSION =~ ^v[0-9] ]] && OB_VERSION=${OB_VERSION#v}   # tag v0.1.0 -> version 0.1.0 (as package.sh)
echo "source: $SRC_ARG  version: ${OB_VERSION:-<Cargo.toml>}  commit: $OB_COMMIT"

# The images: tagged by their Containerfile's hash, so an edited Containerfile gives a new image. Windows
# (and the crate fetch) in the Ubuntu 22.04 image, Linux in the AlmaLinux 8 one: the oldest glibc a
# Linux binary needs is the one it was linked against.
IMAGE=${IMAGE:-overbright-build:$(sha256sum "$HERE/Containerfile" | cut -c1-12)}
IMAGE_LINUX=${IMAGE_LINUX:-overbright-build-linux:$(sha256sum "$HERE/Containerfile.linux" | cut -c1-12)}
image() { # image <tag> <Containerfile>
    if ! "$PODMAN" image exists "$1"; then
        echo "building image $1"
        "$PODMAN" build -t "$1" -f "$HERE/$2" "$HERE"
    fi
}
image "$IMAGE" Containerfile
case " $TARGETS " in *" linux "*) image "$IMAGE_LINUX" Containerfile.linux ;; esac
# A target directory filled by another image is emptied first: cargo does not rebuild C code (mimalloc)
# when only the system's compiler and headers change. One without a note was filled by the Windows image
# (before the Linux build had its own).
for t in $TARGETS; do
    img=$IMAGE; [ "$t" = linux ] && img=$IMAGE_LINUX
    if [ "$(cat "$OUT/target-$t.image" 2>/dev/null || echo "$IMAGE")" != "$img" ]; then
        rm -rf "$OUT/target-$t"
        mkdir -p "$OUT/target-$t"
    fi
    echo "$img" > "$OUT/target-$t.image"
done

run() { # run <image> <network: default|none> <env...>: inside.sh in the image
    local img=$1 net=()
    [ "$2" = default ] || net=(--network="$2")
    shift 2
    "$PODMAN" run --rm "${net[@]}" --cpus="$JOBS" \
        -v "$OUT/src:/src:ro" -v "$HERE/inside.sh:/inside.sh:ro" -v "$OUT/cargo-home:/cargo-home" \
        -v "$OUT/target-linux:/target-linux" -v "$OUT/target-windows:/target-windows" -v "$OUT/dist:/dist" \
        -e CARGO_BUILD_JOBS="$JOBS" -e OB_VERSION="$OB_VERSION" -e OB_COMMIT="$OB_COMMIT" \
        -e SOURCE_DATE_EPOCH="$SOURCE_DATE_EPOCH" -e OB_SOURCE_URL="${OB_SOURCE_URL:-}" "$@" "$img" bash /inside.sh
}
{
    # The crates once with the network (a no-op when the cache has them), then the build offline.
    run "$IMAGE" default -e MODE=fetch
    rm -rf "$OUT/dist"/*
    for t in $TARGETS; do
        img=$IMAGE; [ "$t" = linux ] && img=$IMAGE_LINUX
        run "$img" none -e MODE=build -e TARGETS="$t" -e DEV_HARNESS=$DEV -e TESTS=$TESTS -e FAST=$FAST || exit 1
    done
} 2>&1 | tee "$OUT/build.log"
[ "${PIPESTATUS[0]}" -eq 0 ] || { echo "build.sh: build failed (see $OUT/build.log)" >&2; exit 1; }

[ "$CHECK" = 1 ] || exit 0
status=0
for t in $TARGETS; do
    case $t in
        linux) for b in game overbright; do "$HERE/../check_linux.sh" "$OUT/dist/linux/$b" || status=1; done ;;
        windows) for b in game overbright; do python3 "$HERE/../check_pe.py" --gui "$OUT/dist/windows/$b.exe" || status=1; done ;;
    esac
done
exit $status
