#!/bin/bash
# Frame budget (docs/notes/performance/budget.md): every effect's cost as "everything max" (perf.sh's MAX: a maintainer's
# play setup + the newest rows) with that one row turned down (leave-one-out), in the effect's heavy scene,
# at 1920x1080 and 3840x2160, plus the preset totals over every scene.
#
#   budget.sh OUT BIN            the desktop-GPU matrix (REPS default 4, FRAMES 600, ORDER=alternate)
#   IGPU=1 budget.sh OUT BIN     the iGPU class (IGPU_DEVICE, scripts/local.env): presets at 1080p only
# The presets user and max need PRESET_user (scripts/local.env). Exit status non-zero when the bench or a
# table step failed.
#   budget.py OUT                the tables (OUT/budget.md)
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/../lib/common.sh"
ck_local_env
OUT=${1:?OUT}; BIN=${2:?BIN}
[ -x "$BIN" ] || ck_die "no binary $BIN"
[ -n "${PRESET_user:-}" ] || ck_die "PRESET_user is not set (the play setup's enh lines; scripts/local.env): the play-setup and max presets need it"
export REPS=${REPS:-4} FRAMES=${FRAMES:-600} ORDER=alternate
# Leave-one-out presets: MAX then the row's lower value (later lines win). name|lines|scene (see budget.py).
PRESETS_SH=$(python3 "$HERE/budget.py" --presets) && [ -n "$PRESETS_SH" ] || ck_die "budget.py --presets failed"
eval "$PRESETS_SH"
cfgs=""
if [ -n "${IGPU:-}" ]; then
  export FRAMES=${IGPU_FRAMES:-300}
  for s in explore racing race rain; do for p in orig modern ultra26 user max; do cfgs="$cfgs $s-$p-1920x1080"; done; done
else
  for r in 1920x1080 3840x2160; do
    for s in explore racing race rain water; do for p in orig modern ultra26 extreme user max; do cfgs="$cfgs $s-$p-$r"; done; done
    x=$(python3 "$HERE/budget.py" --configs $r) && [ -n "$x" ] || ck_die "budget.py --configs $r failed"
    cfgs="$cfgs $x"
  done
fi
export CONFIGS="$cfgs"
echo "budget: $(echo $cfgs | wc -w) configs, reps $REPS, frames $FRAMES, start $(date -Is), loadavg $(cut -d' ' -f1-3 /proc/loadavg)"
"$HERE/perf.sh" bench "$OUT" main="$BIN"; RC=$?
echo "budget: done $(date -Is), rc $RC, loadavg $(cut -d' ' -f1-3 /proc/loadavg)"
python3 "$HERE/budget.py" "$OUT" || RC=1
[ $RC = 0 ] || { echo "budget: FAILED (bench or tables incomplete; see $OUT/bench.log)" >&2; exit 1; }
