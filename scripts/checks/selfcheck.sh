#!/bin/bash
# The gates' own self-test (docs/developing/checks.md "Self-test of the gates"): every gate is fed a deliberately
# broken input and must FAIL, and a known-good input and must PASS. The game is replaced by
# selfcheck_stub.py (PNGs, dump sets, bench JSON, WAVs, state hashes, the adapter line; faults from a
# `faults` file next to it), so the whole run takes about a minute and needs no GPU or game data. The
# real pixdiff is used (PIXDIFF, default $CARGO_TARGET_DIR/<checks profile>/examples/pixdiff, docs/developing/build-times.md).
#
#   selfcheck.sh OUTDIR [CASE_SUBSTRING]
#
# Prints one line per case ("ok   <case>: expected fail, got rc 1" / "BAD  ..."), the last line
# "SELFCHECK: PASS (N cases)" or "SELFCHECK: FAIL (k of N cases: ...)". Exit status 0 only on PASS.
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
O=${1:?usage: selfcheck.sh OUTDIR [CASE_SUBSTRING]}; ONLY=${2:-}
rm -rf "$O"; mkdir -p "$O"; O=$(cd "$O" && pwd)
. "$REPO/scripts/lib/common.sh"
PIXDIFF=${PIXDIFF:-${CARGO_TARGET_DIR:-$REPO/target}/$CK_PDIR/examples/pixdiff}
[ -x "$PIXDIFF" ] || { echo "selfcheck: no pixdiff at $PIXDIFF (cargo build --profile $CK_PROFILE --example pixdiff)"; exit 2; }
command -v python3 > /dev/null || { echo "selfcheck: no python3"; exit 2; }
PERF="$REPO/scripts/perf/perf.sh"

# The stubs' environment: nothing machine-local, nothing from the caller's game setup.
export NFSU2_LOCAL_ENV="$O/local.env"
mkdir -p "$O/root" "$O/tmp"
cat > "$NFSU2_LOCAL_ENV" << EOF
export NFSU2_ROOT="$O/root"
export PRESET_user="enh.stub 1"
export PERF_BASELINES="$O/baselines"
EOF
export TMPDIR="$O/tmp" NFSU2_HEADLESS_ONLY=1
unset CK_LOCAL_ENV SHOTS GATES FLOWS ENH_PRESETS RATE RATES RATE_FLOWS CONFIGS SET REPS FRAMES IGPU 2> /dev/null
# The caller's switches for its own run (a full check started with FRESH=1 WINE_SMOKE=1 RELEASE=1 PREV=...)
# must not reach the cases: each case names the switches it needs.
unset FRESH WINE_SMOKE WINE_SMOKE_SH RELEASE PREV NPREV LAVAPIPE SOAK FUZZ DEVPROBES SELFCHECK DET TESTS \
  SKIP_BUILD SKIP_TESTS FAST_SHOTS_FILE GATE_EXCLUDE GATE_ENH_PRESETS KNOWN NEWNONDET RES_SET CHECKS \
  EDITOR_SHOTS EDITOR_SET EDITOR_SIZES EDITOR_SCREENS EDITOR_SCALES EDITOR_MIN_STD 2> /dev/null

# stub DIR [FAULT…]: a fake game binary in DIR (game, faults, pixdiff next to it).
stub() {
  local d=$1; shift
  mkdir -p "$d"; cp "$HERE/selfcheck_stub.py" "$d/game"; chmod +x "$d/game"; ln -sf "$PIXDIFF" "$d/pixdiff"
  # The map editor's stand-in: the same script under the editor's name (it answers as the editor).
  cp "$d/game" "$d/overbright-editor"
  printf '%s\n' "$@" > "$d/faults"
}

N=0; BAD=()
# expect pass|fail CASE CMD…: run CMD (log in OUT/CASE.log), check its exit status.
expect() {
  local want=$1 name=$2; shift 2
  [ -z "$ONLY" ] || [[ $name == *"$ONLY"* ]] || return 0
  N=$((N + 1))
  "$@" > "$O/$name.log" 2>&1; local rc=$?
  local got=pass; [ $rc = 0 ] || got=fail
  if [ "$got" = "$want" ]; then echo "ok   $name: expected $want, got rc $rc"
  else BAD+=("$name"); echo "BAD  $name: expected $want, got rc $rc (log $O/$name.log):"; tail -5 "$O/$name.log" | sed 's/^/       /'; fi
}

# ---- evidence.sh: a claim row without a tag fails ----------------------------------------------------------------
ev_tree() { # DIR: a copy of the docs and the allow list the check reads
  mkdir -p "$1/docs/original/coverage" "$1/docs/playing" "$1/scripts/checks"
  cp "$REPO"/docs/original/coverage/*.md "$1/docs/original/coverage/"
  cp "$REPO/docs/playing/native-deviations.md" "$1/docs/playing/"
  cp "$REPO/scripts/checks/evidence-allow.txt" "$1/scripts/checks/"
}
ev_tree "$O/ev-good"
expect pass evidence-good "$HERE/evidence.sh" "$O/ev-good"
ev_tree "$O/ev-cov"
printf '\n| Address | Name | What it does | Ours | Status | Evidence | Risk |\n|---|---|---|---|---|---|---|\n| 0x999999 | — | a planted claim | — | **PORTED** | looks right | low |\n' >> "$O/ev-cov/docs/original/coverage/racer-ai.md"
expect fail evidence-untagged-coverage-row "$HERE/evidence.sh" "$O/ev-cov"
ev_tree "$O/ev-dev"
sed -i '0,/| choice |$/s//| |/' "$O/ev-dev/docs/playing/native-deviations.md"
expect fail evidence-untagged-deviation-row "$HERE/evidence.sh" "$O/ev-dev"
ev_tree "$O/ev-prose"
printf '\nThe planted rule works exactly like the original does.\n' >> "$O/ev-prose/docs/playing/native-deviations.md"
expect fail evidence-untagged-prose-claim "$HERE/evidence.sh" "$O/ev-prose"

# ---- perf.sh parity ------------------------------------------------------------
# parity CASE BASE_FAULTS NEW_FAULTS [ENV…]: the parity gates on two stubs.
parity() {
  local c=$1 bf=$2 nf=$3; shift 3
  stub "$O/$c/a" $bf; stub "$O/$c/b" $nf
  env GATES="images window shots audio sim" IMG_SIZES=64x64 IMG_SCENES="explore race" SHOTS="explore race" \
    FLOWS="firstrace playthrough" PARITY_FRAMES=3 DUMP_EVERY=1 "$@" "$PERF" parity "$O/$c/out" "$O/$c/a/game" "$O/$c/b/game"
}
expect pass parity-good parity parity-good "" ""
expect fail parity-missing-set parity parity-missing-set "" "nodump:img-race-64x64"
expect fail parity-zero-sets parity parity-zero-sets "nodump:all" "nodump:all"
expect fail parity-extra-set parity parity-extra-set "" "extradump"
expect fail parity-image-differs parity parity-image-differs "" "imgdiff:img-explore-64x64"
expect fail parity-window-differs parity parity-window-differs "" "imgdiff:win-racing"
expect fail parity-side-run-fails parity parity-side-run-fails "" "rc:img-explore-64x64"
expect fail parity-base-run-fails parity parity-base-run-fails "rc:win-explore" ""
expect fail parity-shot-panic parity parity-shot-panic "" "panic:race"
expect fail parity-shot-missing parity parity-shot-missing "" "noshot:race"
expect fail parity-shot-differs parity parity-shot-differs "" "shotdiff:race"
expect fail parity-shot-rc parity parity-shot-rc "" "rc:explore"
expect fail parity-shots-empty parity parity-shots-empty "" "" GATES=shots SHOTS=
# A target renamed since the base (scripts/checks/shot-renames.txt): the base draws it under its old name only.
expect pass parity-shot-renamed parity parity-shot-renamed "unknown:new_name" "unknown:old_name" GATES=shots \
  SHOTS="explore new_name" SHOT_BASE_MAP="new_name=old_name"
expect fail parity-shot-renamed-no-map parity parity-shot-renamed-no-map "unknown:new_name" "unknown:old_name" GATES=shots \
  SHOTS="explore new_name"
expect fail parity-shot-renamed-differs parity parity-shot-renamed-differs "unknown:new_name" "unknown:old_name shotdiff:new_name" \
  GATES=shots SHOTS="explore new_name" SHOT_BASE_MAP="new_name=old_name"
# gate.sh's choice of parity shots: a base target, a renamed one (exact pair and prefix rule), one the base lacks.
renames_pick() {
  printf 'old_name new_name\nold_* fresh_*\n' > "$O/renames.txt"
  local got; got=$(printf 'explore\nold_name\nold_x\n' | SHOT_RENAMES="$O/renames.txt" python3 "$HERE/shot_renames.py" pick - \
    explore new_name fresh_x brand_new | tr '\n' ';')
  echo "$got"; [ "$got" = "explore explore;new_name old_name;fresh_x old_x;" ]
}
expect pass shot-renames-pick renames_pick
# gate.sh's private set for a base that knows a fixture under its old name (scripts/checks/fixture-renames.txt).
fixture_alias() {
  local s="$O/fixture-set"; rm -rf "$s"; mkdir -p "$s/saves"; printf 'abc' > "$s/saves/new.sav"
  printf 'saves/old.sav saves/new.sav\nsaves/gone.sav saves/missing.sav\n' > "$O/fixture-renames.txt"
  local a b; a=$(FIXTURE_RENAMES="$O/fixture-renames.txt" python3 "$HERE/fixture_renames.py" alias "$s")
  b=$(FIXTURE_RENAMES="$O/fixture-renames.txt" python3 "$HERE/fixture_renames.py" alias "$s")
  echo "$a / $b"; [ "$a" = "aliased 1" ] && [ "$b" = "aliased 0" ] && [ "$(cat "$s/saves/old.sav")" = abc ] && [ ! -e "$s/saves/gone.sav" ]
}
expect pass fixture-renames-alias fixture_alias
expect fail parity-audio-differs parity parity-audio-differs "" "wavdiff"
expect fail parity-sim-differs parity parity-sim-differs "" "hashdiff:firstrace"
expect fail parity-sim-no-hash parity parity-sim-no-hash "" "nohash:playthrough"
expect fail parity-sim-flow-fails parity parity-sim-flow-fails "" "rc:firstrace"
expect fail parity-flow-panic parity parity-flow-panic "" "panic:playthrough"
expect fail parity-unknown-gate parity parity-unknown-gate "" "" GATES=imagez

# ---- rate gate -----------------------------------------------------------------
rate() { local c=$1; shift; stub "$O/$c/b" "$@"; RATE_FLOWS="firstrace racing" RATE_SECS=1 RATE_SELFTEST=20 "$PERF" rate "$O/$c/out" "$O/$c/b/game"; }
# rate_rc CASE WANT FAULT…: perf.sh rate's exit status must be WANT (0 identical, 1 hard, 3 report-only differences).
rate_rc() { local c=$1 want=$2; shift 2; rate "$c" "$@"; local rc=$?; [ $rc = "$want" ] || { echo "rc $rc, wanted $want"; return 1; }; }
expect pass rate-good rate rate-good
expect fail rate-differs rate rate-differs ratedep
# Report-only: the policy with every "fail" of the rate field turned to "report" (columns and image classes); with the
# real one (every column and image class counts) the same dependence is counted (exit 1).
awk '/^#/ || NF < 3 { print; next } { $2 = ($2 == "fail" ? "report" : $2); print }' "$HERE/rate-columns.txt" > "$O/rate-columns-report.txt"
rate_rc_report() { RATE_POLICY="$O/rate-columns-report.txt" rate_rc "$@"; }
expect pass rate-differs-report-only rate_rc_report rate-differs-report-only 3 ratedep
expect pass rate-differs-counted rate_rc rate-differs-counted 1 ratedep
# The image classes count ("images@CLASS fail"); with them reported a difference is exit 3.
expect pass rate-images-counted rate_rc rate-images-counted 1 rateimg
expect pass rate-images-report-only rate_rc_report rate-images-report-only 3 rateimg
# The image check's evidence: a planted 1-pixel difference is printed (class, rate, tick, count, max diff, box, pixel
# values) and the reference, the frame and a diff PNG are kept in the folder RATE_EVIDENCE_DIR names (not the real one).
rate_evidence() {
  local d="$O/rate-evidence-case"; rm -rf "$d"; mkdir -p "$d/ref" "$d/120" "$d/evid"
  python3 -I - "$HERE/../perf" "$d" << 'PY' || return 1
import sys
sys.path.insert(0, sys.argv[1])
import rateimg
d = sys.argv[2]
rows = [bytes([10, 20, 30] * 8) for _ in range(8)]
rateimg.write_png(d + '/ref/tick-000006.png', 8, 8, rows)
rows[3] = bytes([10, 20, 30] * 5 + [11, 20, 30] + [10, 20, 30] * 2)
rateimg.write_png(d + '/120/tick-000006.png', 8, 8, rows)
PY
  RATE_EVIDENCE_DIR="$d/evid" python3 -I "$HERE/../perf/rateimg.py" "$PIXDIFF" step "$d/ref" "120=$d/120" > "$d/out" 2> "$d/err"
  [ $? = 1 ] || { echo "rc is not 1"; return 1; }
  local f; f=$(ls -d "$d"/evid/*/ 2> /dev/null | head -1); [ -n "$f" ] || { echo "no evidence folder"; return 1; }
  grep -q "evidence ${f%/}" "$d/out" || { echo "the line does not name the folder"; return 1; }
  grep -q "class step, 120 Hz, tick-000006: 1 px differ, max channel diff 1, bbox x 5..5 y 3..3" "$d/err" || { echo "detail line missing"; cat "$d/err"; return 1; }
  grep -q '(5,3) ref (10, 20, 30) got (11, 20, 30)' "$d/err" || { echo "pixel values missing"; return 1; }
  for n in step-120hz-tick-000006-ref.png step-120hz-tick-000006.png step-120hz-tick-000006-diff.png; do
    [ -s "$f$n" ] || { echo "missing $n"; return 1; }
  done
}
expect pass rate-image-evidence rate_evidence
# A jittered run that ends short of the others counts (the runs end on a tick count at every schedule).
expect pass rate-jitter-short-counted rate_rc rate-jitter-short-counted 1 jittershort
expect fail rate-run-fails rate rate-run-fails rc:firstrace
# A flow that needs the private set (exit 3, a SKIP line): without the set (OB_PRIVATE names a folder without its saves)
# a named skip on the RATE line; with the set there the same run fails the gate.
mkdir -p "$O/no-private-set" "$O/full-private-set/tests/fixtures/native_saves"
rate_privskip() {
  local c=$1 set=$2; stub "$O/$c/b" privskip:menus
  OB_PRIVATE="$O/$set" RATE_FLOWS="firstrace menus" RATE_SECS=1 RATE_SELFTEST=20 "$PERF" rate "$O/$c/out" "$O/$c/b/game" || return 1
  grep -q "^RATE: PASS; .*skipped (needs the maintainer's private set (OB_PRIVATE)): menus;" "$O/$c/out/rate.txt" || { echo "the skip is not named on the RATE line"; return 1; }
}
expect pass rate-private-flow-skipped rate_privskip rate-private-flow-skipped no-private-set
expect fail rate-private-flow-with-set rate_privskip rate-private-flow-with-set full-private-set
expect fail rate-selftest-missed rate rate-selftest-missed noselftest

# ---- checks.sh ---------------------------------------------------------------
# checks CASE FAULTS SET [ARGS…] [-- ENV…]: checks.sh on a stub (no build, no cargo test).
checks() {
  local c=$1 f=$2 set=$3; shift 3
  local args=() envs=()
  while [ $# -gt 0 ] && [ "$1" != -- ]; do args+=("$1"); shift; done
  [ $# -gt 0 ] && { shift; envs=("$@"); }
  stub "$O/$c/t/$CK_PDIR" $f; mkdir -p "$O/$c/t/$CK_PDIR/examples"; ln -sf "$PIXDIFF" "$O/$c/t/$CK_PDIR/examples/pixdiff"
  env CARGO_TARGET_DIR="$O/$c/t" SKIP_BUILD=1 SKIP_TESTS=1 HISTORY="$O/$c/history.jsonl" JOBS=2 "${envs[@]}" \
    "$HERE/checks.sh" "$set" "$O/$c/out" "${args[@]}"
}
: > "$O/empty-shots.txt"; printf 'explore\nrace\n' > "$O/two-shots.txt"
expect pass checks-shots-good checks checks-shots-good "" shots explore race -- DET=1
expect fail checks-shots-none checks checks-shots-none "" shots
expect fail checks-no-harness checks checks-no-harness "noharness" shots explore race
expect fail checks-shots-panic checks checks-shots-panic "panic:race" shots explore race
expect fail checks-shots-rc checks checks-shots-rc "rc:race" shots explore race
expect fail checks-shots-nondet checks checks-shots-nondet "nondet:race" shots explore race -- DET=1
expect fail checks-shots-nopng-det checks checks-shots-nopng-det "noshot:race" shots explore race -- DET=1
expect pass checks-fast-good checks checks-fast-good "" fast -- FAST_SHOTS_FILE="$O/two-shots.txt"
expect fail checks-fast-empty-list checks checks-fast-empty-list "" fast -- FAST_SHOTS_FILE="$O/empty-shots.txt"
expect fail checks-fast-no-list checks checks-fast-no-list "" fast -- FAST_SHOTS_FILE="$O/nonexistent.txt"
expect fail checks-fast-flow-panic checks checks-fast-flow-panic "panic:firstdrag" fast -- FAST_SHOTS_FILE="$O/two-shots.txt"
expect fail checks-fast-flow-rc checks checks-fast-flow-rc "rc:postrace" fast -- FAST_SHOTS_FILE="$O/two-shots.txt"
# The Wine smoke is report-only, but a Windows build that does not compile fails the set.
fakewine() { printf '#!/bin/bash\nmkdir -p "$1"; echo "WINE_SMOKE: FAIL (%s) (1 s)" > "$1/wine_smoke.txt"; exit 1\n' "$2" > "$1"; chmod +x "$1"; }
fakewine "$O/fake-smoke-build.sh" build; fakewine "$O/fake-smoke-run.sh" vk_main
expect fail checks-fast-windows-build-fails checks checks-fast-windows-build-fails "" fast -- FAST_SHOTS_FILE="$O/two-shots.txt" WINE_SMOKE=1 WINE_SMOKE_SH="$O/fake-smoke-build.sh"
expect pass checks-fast-smoke-run-fails-report-only checks checks-fast-smoke-run-fails-report-only "" fast -- FAST_SHOTS_FILE="$O/two-shots.txt" WINE_SMOKE=1 WINE_SMOKE_SH="$O/fake-smoke-run.sh"
expect fail checks-fast-prepare-fails checks checks-fast-prepare-fails "prepfail" fast -- FAST_SHOTS_FILE="$O/two-shots.txt"
expect fail checks-fast-prepare-no-stamp checks checks-fast-prepare-no-stamp "nostamp" fast -- FAST_SHOTS_FILE="$O/two-shots.txt"
expect fail checks-fast-prepare-not-idempotent checks checks-fast-prepare-not-idempotent "prepgrow" fast -- FAST_SHOTS_FILE="$O/two-shots.txt"
expect fail checks-fast-prepare-load-adds checks checks-fast-prepare-load-adds "loadgrow" fast -- FAST_SHOTS_FILE="$O/two-shots.txt"
expect fail checks-fast-prepare-level-stale checks checks-fast-prepare-level-stale "levelstale" fast -- FAST_SHOTS_FILE="$O/two-shots.txt"
: > "$O/empty.env"
expect fail checks-no-root env NFSU2_LOCAL_ENV="$O/empty.env" NFSU2_ROOT= "$HERE/checks.sh" shots "$O/no-root" explore

# The editor shot matrix in the fast set (EDITOR_SHOTS=1; full runs it always): a broken editor shot fails the set.
expect pass checks-fast-editor-good checks checks-fast-editor-good "" fast -- FAST_SHOTS_FILE="$O/two-shots.txt" \
  EDITOR_SHOTS=1 EDITOR_SIZES=1280x720
expect fail checks-fast-editor-broken checks checks-fast-editor-broken "edrc:props_3d" fast -- FAST_SHOTS_FILE="$O/two-shots.txt" \
  EDITOR_SHOTS=1 EDITOR_SIZES=1280x720

# ---- editor_shots.sh: the map editor's shot matrix on the stand-in ---------------------------------------------
# edshots CASE FAULTS [ENV…]: two sizes (one at display scale 1.5) x the three screens.
edshots() {
  local c=$1 f=$2; shift 2
  stub "$O/$c/b" $f
  env EDITOR_SIZES=1280x720,1920x1080@1.5 JOBS=2 "$@" "$HERE/editor_shots.sh" "$O/$c/out" "$O/$c/b/overbright-editor"
}
expect pass editor-shots-good edshots editor-shots-good ""
expect fail editor-shots-rc edshots editor-shots-rc "edrc:races_map"
expect fail editor-shots-panic edshots editor-shots-panic "edpanic:start"
expect fail editor-shots-no-png edshots editor-shots-no-png "ednoshot:props_3d"
expect fail editor-shots-wrong-size edshots editor-shots-wrong-size "edsize:start"
expect fail editor-shots-flat edshots editor-shots-flat "edflat:start"
expect fail editor-shots-empty-view edshots editor-shots-empty-view "edworld:props_3d"
expect fail editor-shots-toolbar-clipped edshots editor-shots-toolbar-clipped "edclip:races_map"
expect fail editor-shots-not-repeatable edshots editor-shots-not-repeatable "ednondet:props_3d"
expect fail editor-shots-unknown-screen edshots editor-shots-unknown-screen "" EDITOR_SCREENS=start,nonesuch
expect fail editor-shots-no-binary env "$HERE/editor_shots.sh" "$O/editor-shots-no-binary/out" "$O/nonexistent/overbright-editor"
# Headless: the runs start with no display variable (the stand-in refuses one). A copy of the script that keeps
# DISPLAY must fail with the desktop's display set (each copy must differ from the script).
edshots_display() {
  local c=$1 d="$O/$1/s"
  mkdir -p "$d/checks"; ln -sfn "$REPO/scripts/lib" "$d/lib"; cp "$HERE/resolutions.py" "$d/checks/"
  sed 's/env -u DISPLAY -u WAYLAND_DISPLAY/env -u WAYLAND_DISPLAY/' "$HERE/editor_shots.sh" > "$d/checks/editor_shots.sh"
  cmp -s "$HERE/editor_shots.sh" "$d/checks/editor_shots.sh" && { echo "the edit changed nothing"; return 0; }
  stub "$O/$c/b"
  DISPLAY=:0 EDITOR_SIZES=1280x720 bash "$d/checks/editor_shots.sh" "$O/$c/out" "$O/$c/b/overbright-editor"
}
expect fail editor-shots-display-kept edshots_display editor-shots-display-kept
expect pass editor-shots-display-unset edshots editor-shots-display-unset "" DISPLAY=:0 WAYLAND_DISPLAY=wayland-0

# ---- the data-test rule (ck_data_tests): a skip fails only when the run could have avoided it ----------------------
# datatests CASE GAME PRIV BOTH [ENV…]: a cargo test log with that many tests ignored for the game files, the private
# set and both, judged in a tree without the private set (OB_PRIVATE unset) unless ENV says otherwise.
datatests() {
  local c=$1 g=$2 p=$3 b=$4 i; shift 4
  local log="$O/$c.tests.log" tree="$O/$c.tree"; mkdir -p "$tree"
  { echo "test ok_one ... ok"
    for ((i = 0; i < g; i++)); do echo "test game_$i ... ignored, needs NFSU2_ROOT (the player's own game files)"; done
    for ((i = 0; i < p; i++)); do echo "test priv_$i ... ignored, needs OB_PRIVATE (private captures and saves)"; done
    for ((i = 0; i < b; i++)); do echo "test both_$i ... ignored, needs NFSU2_ROOT (the player's own game files) and OB_PRIVATE (private captures and saves)"; done
  } > "$log"
  ( unset OB_PRIVATE OB_REQUIRE; export NFSU2_ROOT="$O/root"; [ $# = 0 ] || export "$@"; . "$HERE/../lib/common.sh"; ck_data_tests "$log" "$tree" )
}
# A public tree with the game files: the private set's tests (alone and with the game files) are a correct skip.
expect pass datatests-public-tree datatests datatests-public-tree 0 42 63
expect fail datatests-game-skip-with-root datatests datatests-game-skip-with-root 1 42 63
expect pass datatests-game-skip-without-root datatests datatests-game-skip-without-root 5 0 0 NFSU2_ROOT=
mkdir -p "$O/private-set"
expect fail datatests-private-here-both datatests datatests-private-here-both 0 0 1 OB_PRIVATE="$O/private-set"
expect fail datatests-private-here-alone datatests datatests-private-here-alone 0 1 0 OB_PRIVATE="$O/private-set"
expect pass datatests-private-here-all-ran datatests datatests-private-here-all-ran 0 0 0 OB_PRIVATE="$O/private-set"
expect fail datatests-require-private datatests datatests-require-private 0 0 2 OB_REQUIRE=private

# ---- the tests step's mask of the player's folders (home_mask.sh) --------------------------------------------------
# homemask CASE SCRIPT: SCRIPT runs masked under a stand-in home (only ~/.local/share/overbright and ~/.cache/overbright
# exist there); the case passes only when the verify passes and the stand-in folders are as before.
homemask() {
  local c=$1 script=$2 h="$O/$1.home"
  mkdir -p "$h/.local/share/overbright" "$h/.cache/overbright"
  ( unset XDG_DATA_HOME XDG_CACHE_HOME; export HOME="$h"
    "$HERE/home_mask.sh" run "$O/$c.mask" bash -c "$script" || exit 3
    "$HERE/home_mask.sh" verify "$O/$c.mask" ) || return 1
  # The stand-in folders themselves stay as they were: the writes went into the mask.
  [ -z "$(find "$h/.local/share/overbright" "$h/.cache/overbright" -mindepth 1)" ] && [ ! -e "$h/.local/share/NFS Underground 2" ]
}
expect pass homemask-untouched homemask homemask-untouched 'ls ~/.local/share/overbright > /dev/null'
expect fail homemask-written homemask homemask-written 'echo x > ~/.local/share/overbright/options.txt'
expect fail homemask-made-and-removed homemask homemask-made-and-removed 'f=~/.local/share/overbright/start-marker.txt; echo 1 > $f; rm $f'
expect fail homemask-cache-written homemask homemask-cache-written 'mkdir ~/.cache/overbright/driver'
expect fail homemask-absent-made homemask homemask-absent-made 'mkdir ~/".local/share/NFS Underground 2"'

# ---- bench, perfstat, perfgate ---------------------------------------------------
bench() { local c=$1; shift; stub "$O/$c/b" "$@"; CONFIGS="racing-orig-64x64 explore-orig-64x64" REPS=2 FRAMES=3 "$PERF" bench "$O/$c/out" new="$O/$c/b/game"; }
expect pass bench-good bench bench-good
expect fail bench-run-fails bench bench-run-fails benchfail:explore
# The A/B gate statistics on a complete and on an incomplete run set.
abgate() {
  local c=$1 drop=$2
  stub "$O/$c/b"
  CONFIGS="racing-orig-64x64 explore-orig-64x64" REPS=2 FRAMES=3 "$PERF" bench "$O/$c/out" base="$O/$c/b/game" new="$O/$c/b/game" || return 2
  [ -z "$drop" ] || rm -f "$O/$c/out/runs/"$drop
  python3 "$REPO/scripts/perf/perfstat.py" gate "$O/$c/out" --base base --new new
}
expect pass perfstat-gate-good abgate perfstat-gate-good ""
expect fail perfstat-gate-missing-config abgate perfstat-gate-missing-config "new__explore-orig-64x64__r*"
expect fail perfstat-gate-missing-rep abgate perfstat-gate-missing-rep "base__racing-orig-64x64__r2.json"
# A truncated run JSON must fail the gate, not be skipped silently.
badjson() { abgate perfstat-gate-bad-json2 "" > /dev/null 2>&1; echo '{"frames": 3,' > "$O/perfstat-gate-bad-json2/out/runs/new__racing-orig-64x64__r1.json"; python3 "$REPO/scripts/perf/perfstat.py" gate "$O/perfstat-gate-bad-json2/out" --base base --new new; }
expect fail perfstat-gate-truncated-json badjson
perfgate() {
  local c=$1; shift
  stub "$O/$c/base"; stub "$O/$c/b" "$@"
  mkdir -p "$O/baselines/sc-$c"; cp "$O/$c/base/game" "$O/baselines/sc-$c/game"; echo "selfcheck baseline" > "$O/baselines/sc-$c/meta.txt"
  REPS=2 FRAMES=3 "$HERE/perfgate.sh" run "$O/$c/out" "$O/$c/b/game" "$O/baselines/sc-$c"
}
expect pass perfgate-good perfgate perfgate-good
expect fail perfgate-bench-fails perfgate perfgate-bench-fails benchfail:race
# The gate set's two "max" configs need PRESET_user (machine-local; a public checkout has none). With it they run: a
# bench failing only in them fails the gate. Without it they are skipped, and the verdict and the log name them.
perfgate_nopreset() {
  grep -v PRESET_user "$NFSU2_LOCAL_ENV" > "$O/local-nopreset.env"
  NFSU2_LOCAL_ENV="$O/local-nopreset.env" PRESET_user= perfgate "$@" || return 1
  grep -q '^PERFGATE: pass .*(skipped 2: rain-max-3840x2160: needs PRESET_user' "$O/$1/out/perfgate.md" \
    && grep -q '^perfgate: SKIP race-max-1920x1080: needs PRESET_user' "$O/$1.log" 2>/dev/null || { echo "the skip is not named"; return 1; }
}
expect fail perfgate-max-runs-with-preset perfgate perfgate-max-runs-with-preset benchfailenh:enh.stub
expect pass perfgate-max-skipped-without-preset perfgate_nopreset perfgate-max-skipped-without-preset benchfailenh:enh.stub

# ---- loadgate.sh: the asset layer's arms --------------------------------------------------------
# One binary, both sources: the base arm must have read the set (the layer's exit line), the original arm must not.
loadg() {
  local c=$1; shift
  stub "$O/$c/b" "$@"; mkdir -p "$O/$c/set" "$O/$c/out/bin"
  # loadgate runs private copies of the binaries (OUT/bin/LABEL): the stub's faults beside them.
  cp "$O/$c/b/faults" "$O/$c/out/bin/faults"
  NFSU2_BASE_SET="$O/$c/set" REPS=8 MODES=warm SCENES="explore race" FRAMES=3 "$HERE/loadgate.sh" bench "$O/$c/out" \
    orig="$O/$c/b/game@original" base="$O/$c/b/game@base" || return 1
  python3 -I "$REPO/scripts/perf/loadstat.py" gate "$O/$c/out" --base orig --new base
}
expect pass loadgate-layer-good loadg loadgate-layer-good
expect fail loadgate-layer-not-read loadg loadgate-layer-not-read nolayer
expect fail loadgate-layer-in-original loadg loadgate-layer-in-original layeralways

# ---- lavapipe.sh ---------------------------------------------------------------------
: > "$O/fake_icd.json"
lvp() { local c=$1; shift; stub "$O/$c/b" "$@"; LVP_ICD="$O/fake_icd.json" LVP_SHOTS="main" "$HERE/lavapipe.sh" "$O/$c/out" "$O/$c/b/game"; }
expect pass lavapipe-good lvp lavapipe-good "adapter:llvmpipe (LLVM 20.1.8, 256 bits)"
expect fail lavapipe-hardware-gpu lvp lavapipe-hardware-gpu
expect fail lavapipe-no-adapter-line lvp lavapipe-no-adapter-line noadapter
expect fail lavapipe-panic lvp lavapipe-panic "adapter:llvmpipe" "panic:main"
skipcase() { local r; LVP_ICD="$O/nonexistent_icd.json" "$HERE/lavapipe.sh" "$O/lavapipe-skip/out" "$O/lavapipe-good/b/game"; r=$?; [ $r = 3 ]; }
expect pass lavapipe-skip-is-rc3 skipcase

# ---- matrix.sh -----------------------------------------------------------------------
mat() { local c=$1; shift; stub "$O/$c/b" "$@"; SCENES=explore PAIR_SCENES= "$HERE/matrix.sh" "$O/$c/b/game" "$O/$c/out"; }
expect pass matrix-good mat matrix-good
expect fail matrix-panic mat matrix-panic matrixpanic

# ---- wine_smoke.sh: the runs' environment (--env-check, a stand-in for wine) ----------------------------------
# Good: the windowed steps drop the inherited NFSU2_HEADLESS_ONLY (exported above, as checks.sh does), every run
# stays on the script's own display, the headless runs set the guard themselves, the windowed runs name no sound
# host (a check is never heard). Broken: copies of the script with
# one of those taken out (each copy must differ from the script, or the case proves nothing).
wsenv() {
  local c=$1 sedexpr=$2 d="$O/$1/s"
  mkdir -p "$d/checks"; ln -sfn "$REPO/scripts/lib" "$d/lib"
  if [ -z "$sedexpr" ]; then cp "$HERE/wine_smoke.sh" "$d/checks/"
  else sed "$sedexpr" "$HERE/wine_smoke.sh" > "$d/checks/wine_smoke.sh"
    cmp -s "$HERE/wine_smoke.sh" "$d/checks/wine_smoke.sh" && { echo "the edit '$sedexpr' changed nothing"; return 0; }
  fi
  bash "$d/checks/wine_smoke.sh" --env-check "$O/$c/out"
}
expect pass wineenv-good wsenv wineenv-good ""
expect fail wineenv-window-inherits-guard wsenv wineenv-window-inherits-guard '/^  env -u WAYLAND_DISPLAY/s/ -u NFSU2_HEADLESS_ONLY//'
expect fail wineenv-window-desktop-display wsenv wineenv-window-desktop-display '/^  env -u WAYLAND_DISPLAY/s/ DISPLAY=":$DNUM"//'
expect fail wineenv-window-wayland wsenv wineenv-window-wayland '/^  env -u WAYLAND_DISPLAY/s/-u WAYLAND_DISPLAY //'
expect fail wineenv-headless-unguarded wsenv wineenv-headless-unguarded '/^  W "\${WENV\[@\]}"/s/ NFSU2_HEADLESS_ONLY=1//'
expect fail wineenv-window-heard wsenv wineenv-window-heard '/^  env -u WAYLAND_DISPLAY/s/ NFSU2_AUDIO_HOST=none//'

# ---- a check whose default input is a private fixture (halo.sh: the finished-career save) ----------------------
# In a tree without the private set (a copy of the script, lib/ linked) and no OB_PRIVATE: exit 3 with the SKIP line
# naming the set. With the save in $OB_PRIVATE the check runs: a bench that fails then fails it (exit 2).
halo_tree() {
  local d="$O/$1/tree"; mkdir -p "$d/scripts/checks"; ln -sfn "$REPO/scripts/lib" "$d/scripts/lib"
  cp "$HERE/halo.sh" "$HERE/halo.py" "$d/scripts/checks/"; stub "$O/$1/b" "${@:2}"
}
halo_public() {
  halo_tree "$1"; ( unset OB_PRIVATE HALO_SAVE; bash "$O/$1/tree/scripts/checks/halo.sh" "$O/$1/out" "$O/$1/b/game" ); local rc=$?
  [ $rc = 3 ] || { echo "rc $rc, wanted 3 (skipped)"; return 1; }
  grep -q "^halo: SKIP (.*needs the maintainer's private set (OB_PRIVATE)" "$O/$1.log" || { echo "no SKIP line naming the private set"; return 1; }
}
halo_private() {
  halo_tree "$1" benchfail:explore; mkdir -p "$O/$1/set/tests/fixtures/native_saves"; : > "$O/$1/set/tests/fixtures/native_saves/career_complete.sav"
  ( unset HALO_SAVE; OB_PRIVATE="$O/$1/set" bash "$O/$1/tree/scripts/checks/halo.sh" "$O/$1/out" "$O/$1/b/game" )
}
expect pass halo-skipped-without-private-set halo_public halo-skipped-without-private-set
expect fail halo-runs-with-private-set halo_private halo-runs-with-private-set

# ---- layering.sh (the render and harness layering lint) -------------------------------------------------------
# A small tree with the lint and its two lists: one listed hit per rule passes, test code passes; a new hit of
# each rule (the harness outside its feature, an ungated test module, a settings read, the enhancement layer, a
# QoL read, global state) fails.
layering() {
  local c=$1 what=${2:-} t="$O/$1"
  mkdir -p "$t/scripts/checks" "$t/src/render/native" "$t/src/bin/game/hud"
  cp "$HERE/layering.sh" "$t/scripts/checks/"
  printf '# list\nsrc/render/native/a.rs\tlet x = crate::qol::hdr_active();\nsrc/render/native/a.rs\tfn t() { crate::qol::Enhancements::default().apply(); }\n' > "$t/scripts/checks/layering-settings-allow.txt"
  printf '# list\nqol\tsrc/render/native/a.rs\tlet x = crate::qol::hdr_active();\twhy\nstate\tsrc/render/native/a.rs\tstatic N: AtomicU8 = AtomicU8::new(0);\twhy\n' > "$t/scripts/checks/layering-allow.txt"
  printf 'fn f() {\n    let x = crate::qol::hdr_active();\n}\nstatic N: AtomicU8 = AtomicU8::new(0);\n#[cfg(test)]\nmod tests {\n    fn t() { crate::qol::Enhancements::default().apply(); }\n}\n' > "$t/src/render/native/a.rs"
  printf '#[cfg(all(test, feature = "dev-harness"))]\nmod tests;\n#[cfg(feature = "dev-harness")]\nuse crate::dev::x;\n' > "$t/src/bin/game/hud/mod.rs"
  printf 'fn t() { crate::dev::state_hash::v(); }\n' > "$t/src/bin/game/hud/tests.rs"
  case $what in
    harness) printf 'fn g() { dev::shot::list(); }\n' >> "$t/src/bin/game/hud/mod.rs" ;;
    test-mod) printf 'mod tests;\n' > "$t/src/bin/game/hud/mod.rs" ;;
    settings) printf 'fn h() { crate::render::RenderOptions::in_force(); }\n' >> "$t/src/render/native/a.rs" ;;
    enh) printf 'use crate::render::enh::x;\n' >> "$t/src/render/native/a.rs" ;;
    qol) printf 'fn h() { let y = crate::qol::hdr_active(); }\n' >> "$t/src/render/native/a.rs" ;;
    state) printf 'static M: Mutex<u8> = Mutex::new(0);\n' >> "$t/src/render/native/a.rs" ;;
  esac
  bash "$t/scripts/checks/layering.sh" "$t"
}
expect pass layering-good layering layering-good
for w in harness test-mod settings enh qol state; do expect fail "layering-new-$w" layering "layering-new-$w" "$w"; done

# ---- cases of the maintainer's own release tools (selfcheck.d/*.sh, sourced here; not in public checkouts) -------
rel_cases=0
for f in "$HERE"/selfcheck.d/*.sh; do [ -f "$f" ] || continue; . "$f"; rel_cases=1; done
[ $rel_cases = 1 ] || echo "privacy scan: maintainer tool, not in this checkout"

# ---- the player's folders (ck_player_compare) -------------------------------------
# player CASE AFTER-LINE…: the compare of a two-entry listing against the same listing with the game log's line replaced.
player() {
  local c=$1; shift
  printf '/p/cache/a.nfc\tf\t10\t1.0\n/p/data/logs/game-20260101-100000.log\tf\t500\t2.0\n' > "$O/$c.before"
  { printf '/p/cache/a.nfc\tf\t10\t1.0\n'; printf '%b\n' "$@"; } > "$O/$c.after"
  ( . "$HERE/../lib/common.sh" && ck_player_compare "$O/$c.before" "$O/$c.after" )
}
expect pass player-own-log-grew player player-own-log-grew '/p/data/logs/game-20260101-100000.log\tf\t900\t3.0'
expect fail player-log-shrank player player-log-shrank '/p/data/logs/game-20260101-100000.log\tf\t100\t3.0'
expect fail player-new-log player player-new-log '/p/data/logs/game-20260101-100000.log\tf\t500\t2.0' '/p/data/logs/game-20260101-110000.log\tf\t5\t3.0'
expect fail player-log-gone player player-log-gone
expect fail player-other-file-grew player player-other-file-grew '/p/data/logs/game-20260101-100000.log\tf\t500\t2.0' '/p/data/options.txt\tf\t5\t3.0'

# A game that uses the player's folders is seen (ck_player_games, ck_player_watch); one with its own cache folder is not.
# The stand-in is a copy of sleep named game.
player_game() {
  local d="$O/player-game-$1" pid rc=0
  rm -rf "$d"; mkdir -p "$d"; cp "$(command -v sleep)" "$d/game"
  if [ "$1" = own-cache ]; then NFSU2_CACHE_DIR="$d/cache" "$d/game" 20 & pid=$!
  else env -u NFSU2_CACHE_DIR "$d/game" 20 & pid=$!; fi
  ( . "$HERE/../lib/common.sh"
    ck_player_watch "$d/seen.txt"; sleep 1
    for _ in 1 2 3 4 5 6 7 8; do grep -qxF "$d/game" "$d/seen.txt" && break; sleep 0.5; done
    ck_player_watch_stop
    now=$(ck_player_games)
    grep -qxF "$d/game" <<< "$now" && grep -qxF "$d/game" "$d/seen.txt" ) || rc=1
  kill "$pid" 2>/dev/null; wait "$pid" 2>/dev/null
  return $rc
}
expect pass player-game-seen player_game players-folders
expect fail player-game-with-own-cache-not-seen player_game own-cache

echo
if [ ${#BAD[@]} = 0 ]; then echo "SELFCHECK: PASS ($N cases)"; exit 0; fi
echo "SELFCHECK: FAIL (${#BAD[@]} of $N cases: ${BAD[*]})"; exit 1
