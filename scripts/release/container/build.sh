#!/bin/bash
# The release build in a container (podman): the Linux binaries (glibc 2.35 baseline) and the Windows
# ones (x86_64-pc-windows-gnu, mingw-w64) from one commit or one source tree. See README.md here.
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
# IMAGE (default overbright-build:<hash of the Containerfile>; built when missing).
# Out dir: src/ (the copied tree), cargo-home/ (the crate cache, reused), target-linux/, target-windows/
# (cargo's target directories, reused), dist/<target>/ (the binaries), build.log.
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

# The image: tagged by the Containerfile's hash, so an edited Containerfile gives a new image.
IMAGE=${IMAGE:-overbright-build:$(sha256sum "$HERE/Containerfile" | cut -c1-12)}
if ! "$PODMAN" image exists "$IMAGE"; then
    echo "building image $IMAGE"
    "$PODMAN" build -t "$IMAGE" -f "$HERE/Containerfile" "$HERE"
fi

run() { # run <network: default|none> <env...>: inside.sh in the image
    local net=()
    [ "$1" = default ] || net=(--network="$1")
    shift
    "$PODMAN" run --rm "${net[@]}" --cpus="$JOBS" \
        -v "$OUT/src:/src:ro" -v "$HERE/inside.sh:/inside.sh:ro" -v "$OUT/cargo-home:/cargo-home" \
        -v "$OUT/target-linux:/target-linux" -v "$OUT/target-windows:/target-windows" -v "$OUT/dist:/dist" \
        -e CARGO_BUILD_JOBS="$JOBS" -e OB_VERSION="$OB_VERSION" -e OB_COMMIT="$OB_COMMIT" \
        -e SOURCE_DATE_EPOCH="$SOURCE_DATE_EPOCH" -e OB_SOURCE_URL="${OB_SOURCE_URL:-}" "$@" "$IMAGE" bash /inside.sh
}
{
    # The crates once with the network (a no-op when the cache has them), then the build offline.
    run default -e MODE=fetch
    rm -rf "$OUT/dist"/*
    run none -e MODE=build -e TARGETS="$TARGETS" -e DEV_HARNESS=$DEV -e TESTS=$TESTS -e FAST=$FAST
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
