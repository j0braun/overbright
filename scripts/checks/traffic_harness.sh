#!/bin/bash
# The traffic harness, this game's side only (docs/traffic-harness.md): report-only, not in the default sets.
#
#   traffic_harness.sh <outdir> [game test binary]
#
# Runs the harness's headless sessions (tests::traffic_harness, ignored; the dev-harness test binary of the checks'
# profile, built with `cargo test --profile gate --features dev-harness --bin game --no-run`) under the player-folder
# mask: free roam standing and free roam with the scripted player, TRAFH_SEEDS (default 0-2) seeds of TRAFH_SECONDS
# (default 120) each, then the statistics (tools/parity/trafh.py stats; the lane columns need the developer probe
# traffic_walk_lanes, built when devprobes/ is here). About 30 s of wall time for the defaults.
# Writes <outdir>/stand/, <outdir>/bot/ (tables, logs, report.txt) and prints one line per scenario:
#   TRAFFIC_HARNESS: <scen> placed … jam_share … headon_stops … lane_changes/car-min … (report-only)
# Exit status 0 whatever the numbers; 1 when a session or the statistics fail (the harness itself is broken).
# Environment: REPO, CARGO_TARGET_DIR, NFSU2_ROOT (scripts/local.env), TRAFH_SEEDS, TRAFH_SECONDS, TRAFH_SCENS
# (default "stand bot").
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(cd "$HERE/../.." && pwd)}
. "$HERE/../lib/common.sh"
ck_local_env
ck_need_root
OUT=${1:?usage: traffic_harness.sh <outdir> [game test binary]}
mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd)
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$REPO/target}
# The test binary: the newest deps/game-* that holds the harness test (the plain game binary lives there too,
# and must never be started by mistake).
BIN=${2:-}
if [ -z "$BIN" ]; then
  for f in $(ls -t "$CARGO_TARGET_DIR/$CK_PDIR"/deps/game-* 2>/dev/null | grep -v '\.d$'); do
    [ -x "$f" ] && grep -qa 'tests::traffic_harness' "$f" && { BIN=$f; break; }
  done
fi
[ -x "$BIN" ] || ck_die "no game test binary (cargo test --profile $CK_PROFILE --features dev-harness --bin game --no-run)"
if [ -d "$REPO/devprobes/src/bin" ] && [ -z "${TRAFH_LANES:-}" ]; then
  (cd "$REPO" && cargo build --profile "$CK_PROFILE" -p devprobes --bin traffic_walk_lanes) > "$OUT/lanes-build.log" 2>&1 \
    && export TRAFH_LANES="$CARGO_TARGET_DIR/$CK_PDIR/traffic_walk_lanes"
fi
RC=0
for scen in ${TRAFH_SCENS:-stand bot}; do
  d="$OUT/${scen//:/-}"
  if python3 "$REPO/tools/parity/trafh.py" run --out "$d" --scen "$scen" --seeds "${TRAFH_SEEDS:-0-2}" \
       --seconds "${TRAFH_SECONDS:-120}" --bin "$BIN" > "$d.log" 2>&1; then
    python3 - "$d/report.txt" "$scen" <<'EOF'
import sys
want = ('placed', 'spawn_per_min', 'sp_p50', 'jam_share', 'jam_permanent', 'headon_stops', 'lane_changes_per_car_min', 'act_avoid', 'bot_overtakes')
got = {}
for line in open(sys.argv[1]):
    f = line.split()
    if f and f[0] in want: got[f[0]] = f[1]
print(f'TRAFFIC_HARNESS: {sys.argv[2]} ' + ' '.join(f'{k} {got[k]}' for k in want if k in got) + ' (report-only)')
EOF
  else
    echo "TRAFFIC_HARNESS: $scen FAIL (the harness broke; $d.log)"; RC=1
  fi
done
exit $RC
