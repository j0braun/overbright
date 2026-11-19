#!/bin/bash
# Software-GPU startup check (docs/developing/software-gpu.md): `game --selftest` and a few shots on lavapipe
# (Mesa's llvmpipe Vulkan driver), as a machine without a GPU (or a CI runner) would start the game.
#
#   lavapipe.sh OUTDIR [BIN]      BIN default $CARGO_TARGET_DIR/release/game (REPO/target)
#
# The ICD: $LVP_ICD, else the system's /usr/share/vulkan/icd.d/lvp_icd*.json. Without one the check is
# skipped (exit 3, "LAVAPIPE: SKIP"; checks.sh records it as skipped, never as a pass). Fails on a non-zero
# exit, a panic / validation error, an adapter that is not llvmpipe, or a shot whose log names no adapter
# at all (the renderer's `GPU adapter: ` info line, RUST_LOG=info, or the bench's `adapter: ` line: without
# it nothing proves lavapipe was used). The selftest opens no GPU and prints no adapter.
# Each shot is also compared with the same shot on the default GPU (ImageMagick
# `compare -fuzz 4%`, reported only: the light streaks differ on lavapipe, docs/developing/software-gpu.md).
# Every process is waited for by PID; nothing is killed by name.
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/../lib/common.sh"
ck_local_env
REPO=$(cd "$HERE/../.." && pwd)
O=${1:?usage: lavapipe.sh OUTDIR [BIN]}
mkdir -p "$O"; O=$(cd "$O" && pwd)
BIN=${2:-${CARGO_TARGET_DIR:-$REPO/target}/$CK_PDIR/game}
ICD=${LVP_ICD:-$(ls /usr/share/vulkan/icd.d/lvp_icd*.json 2>/dev/null | head -1)}
if [ -z "$ICD" ] || [ ! -f "$ICD" ]; then
  echo "LAVAPIPE: SKIP (no lavapipe ICD; install Mesa's vulkan-swrast or set LVP_ICD)" | tee "$O/lavapipe.txt"
  exit 3
fi
[ -x "$BIN" ] || ck_die "no binary $BIN"
ck_need_root
export NFSU2_HEADLESS_ONLY=1 NFSU2_SHOT_CLOCK=${SHOT_CLOCK-0}
export TMPDIR="$O/tmp"; mkdir -p "$TMPDIR"
SHOTS=${LVP_SHOTS:-main explore race}
FAIL=0
: > "$O/lavapipe.txt"
say() { echo "$*" | tee -a "$O/lavapipe.txt"; }
step() { # step NAME NEED_ADAPTER CMD…: on lavapipe, waited for by PID
  local name=$1 need=$2; shift 2
  local t=$(date +%s)
  VK_DRIVER_FILES="$ICD" VK_ICD_FILENAMES="$ICD" RUST_LOG="${RUST_LOG:-info},nfsu2::render=info" timeout 1800 "$@" > "$O/$name.log" 2>&1 &
  local pid=$!; wait $pid; local rc=$?
  local bad; bad=$(ck_bad "$O/$name.log")
  local adapter; adapter=$(grep -m1 -oE '(GPU adapter|^adapter): .*' "$O/$name.log" | sed -E 's/^(GPU adapter|adapter): //')
  local ok=ok
  if [ $rc != 0 ] || [ "$bad" != 0 ]; then ok=FAIL; fi
  if [ -n "$adapter" ]; then
    echo "$adapter" | grep -qi llvmpipe || ok="FAIL (not lavapipe)"
  elif [ "$need" = 1 ]; then ok="FAIL (no adapter line)"; fi
  [ "$ok" = ok ] || FAIL=1
  say "$name: $ok rc=$rc panic/validation=$bad $(( $(date +%s) - t )) s ${adapter:+adapter $adapter}"
}
step selftest 0 "$BIN" --selftest
for s in $SHOTS; do
  XDG_DATA_HOME="$TMPDIR/xdg-$s" step "shot_$s" 1 "$BIN" --shot "$s" "$O/$s.png"
  if command -v compare > /dev/null && [ -s "$O/$s.png" ]; then
    XDG_DATA_HOME="$TMPDIR/xdg-gpu-$s" RADV_DEBUG=${RADV_DEBUG-nodcc} timeout 900 "$BIN" --shot "$s" "$O/$s-gpu.png" > "$O/$s-gpu.log" 2>&1 &
    wait $!
    [ -s "$O/$s-gpu.png" ] && say "  $s vs the default GPU: $(compare -metric AE -fuzz 4% "$O/$s.png" "$O/$s-gpu.png" null: 2>&1) pixels differ by > 4 %"
  fi
done
rm -rf "$TMPDIR"
say "$([ $FAIL = 0 ] && echo 'LAVAPIPE: PASS' || echo 'LAVAPIPE: FAIL')"
[ $FAIL = 0 ]
