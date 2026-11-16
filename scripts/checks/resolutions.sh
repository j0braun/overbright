#!/bin/bash
# docs/resolutions.md: the resolution matrix. Renders a fixed set of screens headless at every
# matrix size and every display option (Aspect Ratio Stretched = the original's behaviour, Wide = the
# clone's default, Wide + HUD Layout Full Width on the HUD screens) and checks each image with
# resolutions.py: the run (rc, panic / validation), the UI canvas rectangle and reaches against the
# option's rule, UI quads cut off or drawn outside their canvas, uniform scale (text too), the mouse's
# hit test at each widget's drawn centre, the 3D view's FOV rule, black bars, and the downscale compare
# against the same aspect's smallest size. Test tooling only (not native). A failing step of checks.sh
# full (RES_SET=family); after the check, `resolutions.py selftest` breaks passing cells one thing at a time and
# fails the run when a check does not catch its breakage.
#
#   scripts/checks/resolutions.sh OUTDIR [BIN]
#
# Env: RES_SET=all (default, the whole matrix) | family (one size per aspect family; checks.sh full),
#      RES_SIZES=WxH,... RES_SCREENS=a,b (subsets), RES_MOUSE=1 (also the mouse_* shots at each size),
#      JOBS (shots side by side, default 4), RES_SHEETS=0 (no contact sheets), SKIP_SHOTS=1 (re-check only).
# Output: OUTDIR/results.txt (one line per (size, option, screen): PASS|FAIL and why), results.tsv,
#      table.md, causes.txt, sheet_<aspect>_<option>.jpg, cells/<cell>/{shot.png,shot.log,geom.json,rc}.
#      RES_REPORT_ONLY=1: exit 0 whatever the verdicts (the first matrix run's report-only mode).
# Last lines: RESOLUTIONS SELFTEST: n/m checks fail on broken input; RESOLUTIONS: N cells, P pass (K known), F fail
# Exit status: 0 all cells pass (XFAIL = a listed known case) and the self-test caught every breakage; 1 a cell
#      failed or the self-test missed one; 2 a setup error.
set -u
set -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
. "$HERE/../lib/common.sh"
ck_local_env
ck_need_root
ck_need_tools python3 timeout
command -v magick > /dev/null || ck_need_tools convert montage identify
O=${1:?usage: resolutions.sh OUTDIR [BIN]}
BIN=${2:-${CARGO_TARGET_DIR:-$CK_REPO/target}/release/game}
[ -x "$BIN" ] || ck_die "no binary $BIN"
"$BIN" --dev-harness 2>/dev/null | grep -q 'dev-harness: on' || ck_die "$BIN has no dev harness (build with --features dev-harness)"
mkdir -p "$O/cells"
O=$(cd "$O" && pwd)
export TMPDIR="$O/tmp"; mkdir -p "$TMPDIR"
# The game's asset cache: the caller's NFSU2_CACHE_DIR, else a folder of this run (never the player's cache).
export NFSU2_CACHE_DIR="${NFSU2_CACHE_DIR:-$O/cache}"
ARGS=(--set "${RES_SET:-all}")
[ -n "${RES_SIZES:-}" ] && ARGS+=(--sizes "$RES_SIZES")
[ -n "${RES_SCREENS:-}" ] && ARGS+=(--screens "$RES_SCREENS")
[ "${RES_MOUSE:-0}" = 1 ] && ARGS+=(--mouse)
python3 "$HERE/resolutions.py" plan "${ARGS[@]}" > "$O/jobs.tsv" || ck_die "plan failed"
N=$(wc -l < "$O/jobs.tsv")
[ "$N" -gt 0 ] || ck_die "empty matrix"
T0=$(date +%s)

# one_cell LINE: one shot with its option's environment, the geometry dump next to it.
one_cell() {
  local cell size opt screen target envs d
  IFS=$'\t' read -r cell size opt screen target envs <<< "$1"
  d="$O/cells/$cell"
  [ -f "$d/rc" ] && [ "${RES_RERUN:-0}" != 1 ] && return 0   # resume
  rm -rf "$d"; mkdir -p "$d"
  local e=() kv
  for kv in $envs; do e+=("${kv//%20/ }"); done
  env "${e[@]}" XDG_DATA_HOME="$TMPDIR/xdg-$cell" NFSU2_SHOT_CLOCK=0 RADV_DEBUG=nodcc NFSU2_UI_GEOM="$d/geom.json" \
    timeout 600 "$BIN" --shot "$target" "$d/shot.png" > "$d/shot.log" 2>&1
  echo $? > "$d/rc"
  rm -rf "$TMPDIR/xdg-$cell"
}
export -f one_cell; export O BIN TMPDIR
if [ "${SKIP_SHOTS:-0}" != 1 ]; then
  tr '\n' '\0' < "$O/jobs.tsv" | xargs -0 -P "${JOBS:-4}" -I{} bash -c 'one_cell "$1"' _ {}
fi
T1=$(date +%s)
python3 "$HERE/resolutions.py" check "$O" > "$O/summary.txt"
rc=$?
python3 "$HERE/resolutions.py" selftest "$O" > "$O/selftest.txt"
src=$?
cat "$O/summary.txt" "$O/selftest.txt"
rm -rf "$TMPDIR"
echo "resolutions: $N jobs, shots $((T1 - T0)) s, check $(( $(date +%s) - T1 )) s; $O/results.txt, $O/table.md" >> "$O/summary.txt"
tail -1 "$O/summary.txt"
grep '^RESOLUTIONS SELFTEST:' "$O/selftest.txt" | tail -1
grep '^RESOLUTIONS:' "$O/summary.txt" | tail -1
[ $rc -le 1 ] && [ $src -le 1 ] || exit 2
[ "${RES_REPORT_ONLY:-0}" = 1 ] && exit 0
[ $rc = 0 ] && [ $src = 0 ] || exit 1
exit 0
