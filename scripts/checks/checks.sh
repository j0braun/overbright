#!/bin/bash
# Check harness (docs/developing/checks.md). Repo-relative; everything goes to OUTDIR (never /tmp).
#
#   checks.sh fast    OUTDIR   merge set: build, cargo test, selftest, the prepare check, the fast-shots.txt shots, the four flows (< ~8 min)
#   checks.sh full    OUTDIR   nightly set: + every --shot target (shot_targets.py), each twice (determinism),
#                              intro GPS, rain flow, showcase, 1080p shot, drive bench, playthrough, combination matrix, resolution matrix,
#                              the map editor's shot matrix (EDITOR_SHOTS=1 runs it in fast too)
#   checks.sh shots   OUTDIR T…  just those shot targets (twice with DET=1)
#   (fast and full also run the provenance-tag check, the evidence-tag check (evidence.sh), layering.sh and the importer unit tests; full, or LAVAPIPE=1, the lavapipe
#   check (SKIP without the driver is recorded as skipped, LAVAPIPE=require fails instead); full also selfcheck.sh,
#   the gates' own self-test, and release_check.sh, the shipping binary without the dev harness; RELEASE=1 runs it
#   in fast too; full, or FRESH=1, tests/launcher_fresh.sh (the launcher's first run on a fresh machine), tests/launcher_mouse.sh (the launcher by the mouse alone, scale 1 and 2) and tests/launcher_wayland.sh (the same browser on a private headless Wayland session at scale 1 to 2); full, or WINE_SMOKE=1, the Windows build under Wine, report-only: wine_smoke.sh, WINE_SMOKE=off skips;
#   full also the rate gate v2, counting: RATE=strict (the default) any difference across rates, RATE=1 what
#   rate-columns.txt marks "fail", RATE=0 leaves it out)
#
# Environment: REPO (default this script's repo), CARGO_TARGET_DIR (default REPO/target), NFSU2_ROOT (required),
# CHECKS_PROFILE (the Cargo profile of the build and the tests: gate, the default, or release; binaries in
# CARGO_TARGET_DIR/<profile>/, release's in release/; docs/developing/build-times.md),
# scripts/local.env (machine settings, e.g. the build env; scripts/local.env.example), FAST_SHOTS_FILE (default fast-shots.txt),
# JOBS (shots run side by side; default 4), SHOT_CLOCK (default 0 s; empty: wall clock), DET (fast: 0, full: 1; run each shot twice and pixdiff),
# PREV (an earlier OUTDIR: every deterministic shot pixdiffed against it), SKIP_BUILD=1, SKIP_TESTS=1,
# HISTORY (a file the one-line JSON summary is appended to; default OUTDIR/../checks-history.jsonl).
# Exit status: 0 only when every step passed. Last lines of OUTDIR/checks.txt: the summary;
# OUTDIR/summary.json and OUTDIR/summary.md hold it as one line each.
# Every process started here is waited for by PID; nothing is killed by name.
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=${REPO:-$(cd "$HERE/../.." && pwd)}
SET=${1:-}; O=${2:-}
case $SET in fast|full|shots) ;; *) sed -n '2,20p' "$0"; exit 2 ;; esac
[ -n "$O" ] || { echo "checks.sh: OUTDIR missing" >&2; exit 2; }
shift 2
[ "$SET" != shots ] || [ $# -gt 0 ] || { echo "checks.sh: shots needs at least one target" >&2; exit 2; }
. "$HERE/../lib/common.sh"
ck_local_env
ck_need_root
ck_need_tools cargo timeout python3 xargs md5sum awk
mkdir -p "$O"; O=$(cd "$O" && pwd)
# The game's asset cache for every run started here: a folder of this run (or the caller's NFSU2_CACHE_DIR), never
# the player's own cache folder: a check must not write into it, and builds of different trees must not rewrite
# each other's entries there.
export NFSU2_CACHE_DIR="${NFSU2_CACHE_DIR:-$O/cache}"
mkdir -p "$NFSU2_CACHE_DIR"
# The player's own cache and data folders are read-only for a check: their file lists are taken now and again at the
# end, and any change fails the run ("player folders"; ck_player_snapshot, PLAYER_DIRS= leaves the step out).
ck_player_snapshot > "$O/player-before.txt"
[ -z "$PLAYER_DIRS" ] || ck_player_watch "$O/player-games.txt"
cd "$REPO" || exit 2
export CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS:-6} NFSU2_HEADLESS_ONLY=1
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-$REPO/target}
# The shots', flows' and selftest's own data folders (std::env::temp_dir) under OUTDIR, removed at the end.
export TMPDIR="$O/tmp"; mkdir -p "$TMPDIR"
# RADV's delta colour compression changes a pixel of ~2 % of frames run to run (docs/developing/perf-gates.md).
export RADV_DEBUG=${RADV_DEBUG-nodcc}
JOBS=${JOBS:-4}
# The shots' game clock pinned at 0 (NFSU2_SHOT_CLOCK): the screens that animate on the wall clock
# (Game::now from `started`) come out the same every run. 0, not the 10 the main-thread work used: the shots open their screens at
# front-end time 0 and draw them at NFSU2_SHOT_MS (5 s), so a later clock catches openings and
# cross-fades half way (docs/developing/checks.md). SHOT_CLOCK= (empty) keeps the wall clock.
SHOT_CLOCK=${SHOT_CLOCK-0}
DET=${DET:-$([ "$SET" = fast ] && echo 0 || echo 1)}
BIN=$CARGO_TARGET_DIR/$CK_PDIR/game
PIXDIFF=$CARGO_TARGET_DIR/$CK_PDIR/examples/pixdiff
export CHECKS_PROFILE=$CK_PROFILE

# Listed nondeterministic targets: "always", and "wallclock" ones when the clock is not pinned.
NONDET=$(awk -v wall="$([ -z "$SHOT_CLOCK" ] && echo 1)" '!/^#/ && NF && ($2 == "always" || ($2 == "wallclock" && wall == 1)) {print $1}' "$HERE/nondeterministic.txt" | tr '\n' ' ')
T0=$(date +%s)
FAIL=()      # failed steps, for the summary
SKIPPED=()   # steps that could not run here (recorded, not passed)
declare -A STEP_S
OUT="$O/checks.txt"; : > "$OUT"
say() { echo "$*" | tee -a "$OUT"; }
fail() { FAIL+=("$1"); say "FAIL: $1"; }
# A tree that holds the private set (ck_private_here) accepts no skip for lack of it: such a skip fails.
PRIV_HERE=0; ck_private_here "$REPO" && PRIV_HERE=1
skip_private() {
  if [ "$PRIV_HERE" = 1 ]; then fail "$1 (skipped for lack of the private set, but this tree holds it: set OB_PRIVATE)"
  else SKIPPED+=("$1"); fi
}
is_nondet() { case " $NONDET " in *" $1 "*) return 0 ;; esac; return 1; }

# run NAME TIMEOUT CMD…: a step, waited for by PID, its log OUTDIR/NAME.log; sets RC.
run() {
  local name=$1 to=$2; shift 2
  local t=$(date +%s)
  timeout "$to" "$@" > "$O/$name.log" 2>&1 &
  local pid=$!; echo "$pid $name" >> "$O/pids.log"; wait $pid; RC=$?
  STEP_S[$name]=$(( $(date +%s) - t ))
}

say "checks $SET $(date -Is) repo $REPO target $CARGO_TARGET_DIR profile $CK_PROFILE jobs $JOBS det $DET local env $CK_LOCAL_ENV"
# bad_log NAME: fail the step when its log has a panic / validation-error line (render workers' panics do
# not end the process, so rc 0 alone is not enough).
bad_log() { local n; n=$(ck_bad "$O/$1.log"); [ "$n" = 0 ] || { say "  $1: $n panic/validation lines"; fail "$1 (panic/validation)"; }; }

# ---- build -------------------------------------------------------------------
if [ -z "${SKIP_BUILD:-}" ]; then
  run build 3600 cargo build --profile "$CK_PROFILE" --bins --examples --features dev-harness
  say "build rc=$RC ${STEP_S[build]} s"
  if [ $RC != 0 ]; then grep -E '^error' -A8 "$O/build.log" | head -40 | tee -a "$OUT"; fail build; say "BUILD FAILED, stopping"; fi
fi
[ -x "$BIN" ] || { fail "no binary $BIN"; }
if [ ${#FAIL[@]} -gt 0 ]; then
  echo "{\"set\":\"$SET\",\"date\":\"$(date -Is)\",\"ok\":false,\"failed\":[\"build\"]}" | tee "$O/summary.json" >> "${HISTORY:-$O/../checks-history.jsonl}"
  exit 1
fi
say "binary md5 $(md5sum < "$BIN" | cut -c1-32)"
# The binary under test has the developer harness (`dev-harness`, off by default): a shipping build would
# answer every check's flag with the usage error, and a missed `--features` must fail here, not later.
HARNESS=$("$BIN" --dev-harness 2>&1); HRC=$?
if [ $HRC != 0 ] || [ "$HARNESS" != "dev-harness: on" ]; then
  say "harness: $BIN is not a dev-harness build (rc $HRC: $(echo "$HARNESS" | head -2 | tr '\n' ' '))"
  fail "harness (build with --features dev-harness)"
  echo "{\"set\":\"$SET\",\"date\":\"$(date -Is)\",\"ok\":false,\"failed\":[\"harness\"]}" | tee "$O/summary.json" >> "${HISTORY:-$O/../checks-history.jsonl}"
  exit 1
fi

# ---- cargo test --------------------------------------------------------------
# --no-fail-fast: every test binary runs. The total counts every "test result" line; a binary that
# started ("Running" / "Doc-tests") without a result line crashed (signal, abort) and fails the step.
if [ -z "${SKIP_TESTS:-}" ] && [ "$SET" != shots ]; then
  # The tests run with the player's own folders hidden behind empty ones (home_mask.sh: a user + mount namespace),
  # so a test that resolves them anyway writes nothing real, and the step fails when anything landed there.
  run tests 3600 "$HERE/home_mask.sh" run "$O/home-mask" cargo test --profile "$CK_PROFILE" --no-fail-fast --features dev-harness
  # A test that runs the game's own test binary as a child prints the child's "test result" lines too:
  # each binary's own result is the last one before the next header.
  read -r P F I B R < <(awk '
    function flush() { if (inb) { if (have) { p += lp; f += lf; g += lg; r++ } } }
    /^ *Running |^ *Doc-tests / { flush(); b++; inb = 1; have = 0; next }
    /^test result:/ { have = 1; lp = lf = lg = 0
      for (i = 1; i <= NF; i++) { if ($(i) ~ /^passed;?$/) lp = $(i-1); if ($(i) ~ /^failed;?$/) lf = $(i-1); if ($(i) ~ /^ignored;?$/) lg = $(i-1) } }
    END { flush(); print p+0, f+0, g+0, b+0, r+0 }' "$O/tests.log")
  TESTS="$P passed, $F failed, $I ignored; $B test binaries, $R results"
  say "tests: $TESTS; rc=$RC ${STEP_S[tests]} s"
  grep -E 'panicked|^error|signal:|SIGSEGV|SIGABRT|^failures:|^    [a-z_:0-9]+$' "$O/tests.log" | head -40 | tee -a "$OUT"
  if [ "$RC" != 0 ] || [ "$F" != 0 ] || [ "$B" != "$R" ] || [ "$P" = 0 ]; then fail "tests ($TESTS, rc=$RC)"; fi
  "$HERE/home_mask.sh" verify "$O/home-mask" > "$O/home-mask.txt" 2>&1; HM=$?
  tee -a "$OUT" < "$O/home-mask.txt"
  [ $HM = 0 ] || fail "tests (wrote to the player's folders: $O/home-mask.txt)"
  # Data tests (docs/developing/testing.md): counted by reason (game files, private set, both); a skip fails only
  # when this run could have avoided it (ck_data_tests: NFSU2_ROOT is set, or the private set is here).
  DT=$(ck_data_tests "$O/tests.log" "$REPO"); DRC=$?
  echo "$DT" | grep -v '^FAIL: ' | tee -a "$OUT"
  if [ $DRC != 0 ]; then while IFS= read -r l; do fail "${l#FAIL: }"; done < <(echo "$DT" | grep '^FAIL: '); fi
fi

# ---- selftest ----------------------------------------------------------------
if [ "$SET" != shots ]; then
  run selftest 1200 "$BIN" --selftest
  say "selftest rc=$RC ${STEP_S[selftest]} s: $(tail -1 "$O/selftest.log" | cut -c1-160)"
  [ $RC = 0 ] || fail selftest
  bad_log selftest
  # Steps that need the maintainer's private set (src/bin/game/dev/private.rs) print "SKIP: <step>: needs
  # ...": recorded as skipped, never passed.
  while IFS= read -r l; do say "  $l"; skip_private "selftest: ${l#SKIP: }"; done < <(grep -E '^SKIP: ' "$O/selftest.log")
fi

# ---- prepare (docs/developing/prepare.md): game --prepare on empty folders, twice, a first load, a killed run ----
if [ "$SET" != shots ]; then
  run prepare 900 "$HERE/prepare.sh" "$O/prepare" "$BIN"
  say "prepare rc=$RC ${STEP_S[prepare]} s: $(grep -E '^PREPARE' "$O/prepare.log" | tail -1 | cut -c1-220)"
  [ $RC = 0 ] || fail prepare
fi

# ---- shots -------------------------------------------------------------------
# shot TARGET K: one run (K = 1, 2), PNG + log under OUTDIR/shots[K]; a line in OUTDIR/shots.tsv.
shot() {
  local s=$1 k=$2 d="$O/shots"; [ "$k" = 2 ] && d="$O/shots2"
  local t=$(date +%s%N)
  XDG_DATA_HOME="$TMPDIR/xdg-$s-$k" NFSU2_SHOT_CLOCK=$SHOT_CLOCK timeout 900 "$BIN" --shot "$s" "$d/$s.png" > "$d/$s.log" 2>&1
  local rc=$? ms=$(( ($(date +%s%N) - t) / 1000000 ))
  local bad; bad=$(ck_bad "$d/$s.log")
  printf '%s\t%s\t%s\t%s\t%s\n' "$s" "$k" "$rc" "$bad" "$ms" >> "$d/shots.tsv"
}
export -f shot ck_bad; export O BIN TMPDIR SHOT_CLOCK CK_PANIC_RE
SHOTS=""
case $SET in
  fast) F=${FAST_SHOTS_FILE:-$HERE/fast-shots.txt}
        if [ -f "$F" ]; then SHOTS=$(grep -v '^#' "$F" | tr '\n' ' ') || SHOTS=""; else fail "no shot list $F"; fi ;;
  full) SHOTS=$(python3 "$HERE/shot_targets.py" "$REPO" "$BIN" | tr '\n' ' ') || { fail "shot_targets.py failed"; SHOTS=""; } ;;
  shots) SHOTS="$*" ;;
esac
NSHOTS=$(echo $SHOTS | wc -w)
# An empty list is not a pass: the shot and determinism gates would compare nothing.
[ "$NSHOTS" -gt 0 ] || fail "no shot targets ($SET)"

mkdir -p "$O/shots" "$O/shots2"; : > "$O/shots/shots.tsv"; : > "$O/shots2/shots.tsv"
t=$(date +%s)
# K=1 for all, then K=2 (the determinism run) for all, JOBS at a time.
{ for s in $SHOTS; do echo "$s 1"; done; [ "$DET" = 1 ] && for s in $SHOTS; do echo "$s 2"; done; } \
  | xargs -P "$JOBS" -n 2 bash -c 'shot "$0" "$1"' &
XPID=$!; echo "$XPID shots" >> "$O/pids.log"; wait $XPID
STEP_S[shots]=$(( $(date +%s) - t ))
# rc 3: the target needs the maintainer's private set and is not there (src/bin/game/dev/private.rs): skipped.
SHOT_SKIP=$(awk -F'\t' '$3 == 3 && $4 == 0 {print $1}' "$O/shots/shots.tsv" "$O/shots2/shots.tsv" | sort -u | tr '\n' ' ')
for s in $SHOT_SKIP; do skip_private "shot $s (needs the maintainer's private set (OB_PRIVATE))"; done
SHOT_BAD=$(awk -F'\t' '($3 != 0 && $3 != 3) || $4 != 0 {print $1 " rc=" $3 " panic/validation=" $4}' "$O/shots/shots.tsv" "$O/shots2/shots.tsv" | sort -u)
NBAD=$(echo -n "$SHOT_BAD" | grep -c .)
# Every shot must have written its row (K=1, and K=2 with DET=1): a dead xargs or worker is a failure.
for k in 1 2; do
  [ $k = 1 ] || [ "$DET" = 1 ] || continue
  d=shots; [ $k = 2 ] && d=shots2
  rows=$(cut -f1 "$O/$d/shots.tsv" | sort -u | wc -l)
  if [ "$rows" != "$NSHOTS" ]; then
    miss=$(comm -23 <(echo $SHOTS | tr ' ' '\n' | sort -u) <(cut -f1 "$O/$d/shots.tsv" | sort -u) | tr '\n' ' ')
    say "shots run $k: $rows of $NSHOTS targets wrote a result; missing: ${miss:-?}"; fail "shots run $k incomplete ($rows of $NSHOTS)"
  fi
done
say "shots: $NSHOTS targets, $NBAD failed (rc or panic/validation), ${STEP_S[shots]} s at $JOBS jobs${SHOT_SKIP:+; skipped (needs OB_PRIVATE): $SHOT_SKIP}"
[ -n "$SHOT_BAD" ] && echo "$SHOT_BAD" | sed 's/^/  /' | tee -a "$OUT"
[ "$NBAD" = 0 ] || fail "shots ($NBAD)"
sort -t$'\t' -k5 -n -r "$O/shots/shots.tsv" | head -5 | awk -F'\t' '{printf "%s %.1f s; ", $1, $5/1000}' | sed 's/^/  slowest: /' | tee -a "$OUT"; echo | tee -a "$OUT"
# Determinism: run 1 vs run 2 of each shot (pixdiff AE 0). Targets in nondeterministic.txt are reported, not failed.
NDIFF=0; NEWNONDET=""; KNOWN=""
if [ "$DET" = 1 ]; then
  for s in $SHOTS; do
    case " $SHOT_SKIP " in *" $s "*) continue ;; esac
    # A shot without its PNG has already failed above (rc); count it here too so it cannot pass determinism.
    if ! [ -s "$O/shots/$s.png" ] || ! [ -s "$O/shots2/$s.png" ]; then NEWNONDET="$NEWNONDET $s(no png)"; NDIFF=$((NDIFF + 1)); continue; fi
    if ! "$PIXDIFF" "$O/shots/$s.png" "$O/shots2/$s.png" > "$O/shots2/$s.pixdiff" 2>&1; then
      if is_nondet "$s"; then KNOWN="$KNOWN $s"; else NEWNONDET="$NEWNONDET $s"; NDIFF=$((NDIFF + 1)); fi
    fi
  done
  say "determinism (run 1 vs 2, AE 0): $NDIFF unlisted targets differ:${NEWNONDET:- none}; listed nondeterministic that differed:${KNOWN:- none}"
  [ "$NDIFF" = 0 ] || fail "determinism ($NDIFF targets)"
fi
# Against an earlier run (PREV=OUTDIR): every deterministic target.
NPREV=""
if [ -n "${PREV:-}" ] && [ -d "$PREV/shots" ]; then
  mkdir -p "$O/prev-a" "$O/prev-b"
  # Like with like: an earlier run built with another profile is named (its header's "profile" word; runs
  # from before the gate profile have none and were built with release).
  PPROF=$(sed -n 's/^checks .* profile \([^ ]*\) .*/\1/p' "$PREV/checks.txt" 2>/dev/null | head -1); PPROF=${PPROF:-release}
  [ "$PPROF" = "$CK_PROFILE" ] || say "note: PREV was built with profile $PPROF, this run with $CK_PROFILE"
  for s in $SHOTS; do is_nondet "$s" && continue; [ -s "$PREV/shots/$s.png" ] && [ -s "$O/shots/$s.png" ] || continue
    ln -sf "$PREV/shots/$s.png" "$O/prev-a/$s.png"; ln -sf "$O/shots/$s.png" "$O/prev-b/$s.png"; done
  "$PIXDIFF" "$O/prev-a" "$O/prev-b" > "$O/prev.pixdiff" 2>&1
  NPREV=$(grep -oE '^[a-z0-9_]+\.png: [0-9]+ of' "$O/prev.pixdiff" | grep -v ': 0 of' | cut -d. -f1 | tr '\n' ' ')
  say "vs $PREV: $(tail -1 "$O/prev.pixdiff"); changed: ${NPREV:-none}"
fi

# ---- flows -------------------------------------------------------------------
# flow_start NAME TIMEOUT CMD…: a flow in the background with its own data folder (PID recorded);
# flow_end NAME: waited for, its rc and the log's last line.
declare -A FPID FT
flow_start() {
  local name=$1 to=$2; shift 2
  mkdir -p "$TMPDIR/xdg-$name"; FT[$name]=$(date +%s)
  XDG_DATA_HOME="$TMPDIR/xdg-$name" NFSU2_FLOW_DATA="$TMPDIR/xdg-$name" timeout "$to" "$@" > "$O/$name.log" 2>&1 &
  FPID[$name]=$!; echo "${FPID[$name]} $name" >> "$O/pids.log"
}
flow_end() {
  local name=$1; wait "${FPID[$name]}"; RC=$?; STEP_S[$name]=$(( $(date +%s) - FT[$name] ))
  say "$name rc=$RC ${STEP_S[$name]} s: $(tail -1 "$O/$name.log" | cut -c1-140)"
  if [ $RC = 3 ] && grep -qE '^SKIP: ' "$O/$name.log"; then skip_private "$name (needs the maintainer's private set (OB_PRIVATE))"; return; fi
  [ $RC = 0 ] || fail "$name"
  bad_log "$name"
}
flow() { flow_start "$@"; flow_end "$1"; }
if [ "$SET" != shots ]; then
  # The four flows (merge and nightly sets), side by side (each has its own data folder).
  flow_start firstrace 1200 "$BIN" --flow firstrace
  flow_start firstdrag 1500 "$BIN" --flow firstdrag
  flow_start postrace 900 "$BIN" --flow postrace
  flow_start playthrough 900 "$BIN" --playthrough
  for f in firstrace firstdrag postrace playthrough; do flow_end $f; done
fi
if [ "$SET" = full ]; then
  NFSU2_INTRO_WAIT=90 NFSU2_AUDIO_STALL=1 NFSU2_AUDIO_WAV="$O/intro_gps.wav" run intro_gps 900 "$BIN" --shot intro_gps "$O/intro_gps.png"
  say "intro_gps rc=$RC: $(grep -E 'intro: GPS|Error' "$O/intro_gps.log" | tail -2 | tr '\n' ' ' | cut -c1-160)"; [ $RC = 0 ] || fail intro_gps; bad_log intro_gps
  flow flow_rain 600 "$BIN" --flow rain
  flow selftest_showcase 600 "$BIN" --selftest-showcase
  NFSU2_SHOT_SIZE=1920x1080 run explore_1080 600 "$BIN" --shot explore "$O/explore_1080.png"
  say "explore 1080p rc=$RC panic/validation $(ck_bad "$O/explore_1080.log")"; [ $RC = 0 ] || fail explore_1080; bad_log explore_1080
  NFSU2_BENCH_CAR=COROLLA run bench_drive 600 "$BIN" --bench drive
  say "bench drive rc=$RC: $(grep -E '^bench drive|stopped' "$O/bench_drive.log" | head -2 | tr '\n' ' ' | cut -c1-160)"; [ $RC = 0 ] || fail bench_drive; bad_log bench_drive
  # The combination matrix (every tier x every other enhancement x four scenes, docs/developing/settings-model.md).
  run matrix 3600 "$HERE/matrix.sh" "$BIN" "$O/matrix"
  say "matrix rc=$RC: $(grep -E '^matrix |^MATRIX' "$O/matrix.log" | tr '\n' ';' | cut -c1-400)"; [ $RC = 0 ] || fail matrix
  # The resolution matrix (docs/notes/graphics/resolutions.md) on the family set; a failing cell, or a check of it
  # that does not fail on broken input (its self-test), fails the set.
  if [ "${RES_SET:-family}" = off ]; then
    say "resolutions: SKIP (RES_SET=off)"
  else
    RES_SET=${RES_SET:-family} run resolutions 7200 "$HERE/resolutions.sh" "$O/resolutions" "$BIN"
    say "resolutions rc=$RC: $(grep -E '^RESOLUTIONS( SELFTEST)?:' "$O/resolutions.log" | tail -2 | tr '\n' ' ' | cut -c1-400)"
    [ $RC = 0 ] || { grep -E '^FAIL ' "$O/resolutions/results.txt" 2> /dev/null | head -10 | cut -c1-200 | tee -a "$OUT"; fail resolutions; }
  fi
fi

# The map editor's shot matrix (editor_shots.sh, docs/developing/checks.md "Editor shots"): `overbright-editor --shot` headless on
# three screens at one size per aspect family and two display scales (EDITOR_SET=all: every size of the resolution
# matrix); full, or EDITOR_SHOTS=1 in fast; EDITOR_SHOTS=off skips it. A failing cell fails the set.
if [ "$SET" = full ] || [ -n "${EDITOR_SHOTS:-}" ]; then
  if [ "${EDITOR_SHOTS:-}" = off ]; then
    say "editor_shots: SKIP (EDITOR_SHOTS=off)"; SKIPPED+=(editor_shots)
  else
    EDITOR_SET=${EDITOR_SET:-family} run editor_shots 3600 "$HERE/editor_shots.sh" "$O/editor_shots" "$CARGO_TARGET_DIR/$CK_PDIR/overbright-editor"
    say "editor_shots rc=$RC ${STEP_S[editor_shots]} s: $(grep -E '^EDITOR_SHOTS:' "$O/editor_shots.log" | tail -1 | cut -c1-200)"
    [ $RC = 0 ] || { grep -E '^FAIL ' "$O/editor_shots/results.txt" 2> /dev/null | head -10 | cut -c1-200 | tee -a "$OUT"; fail editor_shots; }
  fi
fi

# ---- quality: provenance tags, software GPU -----------------------------------------
if [ "$SET" != shots ]; then
  run provenance 120 python3 "$HERE/provenance.py" "$REPO"
  say "provenance rc=$RC: $(grep -E '^provenance: [0-9]+ ids' "$O/provenance.log" | cut -c13-160)"
  [ $RC = 0 ] || { grep -E '^provenance: ' "$O/provenance.log" | grep -v ' ids (' | head -20 | tee -a "$OUT"; fail provenance; }
  # The evidence register (docs/developing/evidence.md): every claim row in the coverage maps and the deviation table is tagged.
  run evidence 60 "$HERE/evidence.sh" "$REPO"
  say "evidence rc=$RC: $(tail -1 "$O/evidence.log" | cut -c11-160)"
  [ $RC = 0 ] || { grep -E '^evidence: ' "$O/evidence.log" | grep -v ' claim rows' | head -20 | tee -a "$OUT"; fail evidence; }
  # The workspace's layers (ARCHITECTURE.md "Layers"): no new upward reference, package edges only downward,
  # no dev-harness in the shipped packages' tree.
  run crate_layers 300 "$HERE/crate_layers.sh" "$REPO"
  say "crate_layers rc=$RC: $(grep -E '^CRATE LAYERS: ok' "$O/crate_layers.log" | cut -c15-160)"
  [ $RC = 0 ] || { grep -vE '^CRATE LAYERS: (ok|packages ok|dev-harness off|wasm32 check (ok|skipped)|[a-z-]+ has none)' "$O/crate_layers.log" | head -20 | tee -a "$OUT"; fail crate_layers; }
  # The render layers and the harness boundary (layering.sh: core/native take no enhancement, QoL or global state
  # beyond its lists, render code no new settings read, the game names the harness only behind its feature).
  run layering 120 "$HERE/layering.sh" "$REPO"
  say "layering rc=$RC: $(grep -E '^LAYERING: ok' "$O/layering.log" | cut -c11-160)"
  [ $RC = 0 ] || { grep -vE '^LAYERING: ok' "$O/layering.log" | head -20 | tee -a "$OUT"; fail layering; }
  # The stray-open lint (stray_opens.py, stray-opens.txt): a game file is read through game_fs (the asset layer), so
  # no file holds a direct std::fs open beyond its listed count.
  run stray_opens 60 "$HERE/stray_opens.sh" "$REPO"
  say "stray_opens rc=$RC: $(grep -E '^STRAY OPENS: ok' "$O/stray_opens.log" | cut -c14-160)"
  [ $RC = 0 ] || { grep -vE '^STRAY OPENS: ok' "$O/stray_opens.log" | head -20 | tee -a "$OUT"; fail stray_opens; }
  # The importers' unit tests (scripts/tests) need Pillow: without it they are recorded as skipped. They are part
  # of the private set (with the importers they test): a public checkout has no scripts/tests/.
  if [ ! -d "$REPO/scripts/tests" ]; then
    SKIPPED+=("pytests (needs the maintainer's private set laid over the checkout)")
    say "scripts/tests: SKIP (not in this tree: the importers' tests are in the maintainer's private set)"
  elif python3 -I -c 'import PIL' 2> /dev/null; then
    run pytests 300 python3 -I -m unittest discover -s "$REPO/scripts/tests"
    say "scripts/tests rc=$RC: $(grep -E '^(Ran|OK|FAILED)' "$O/pytests.log" | tr '\n' ' ')"; [ $RC = 0 ] || fail pytests
  else
    say "scripts/tests: SKIP (no Python Pillow)"; SKIPPED+=(pytests)
  fi
fi
# lavapipe (llvmpipe Vulkan): the full set, or LAVAPIPE=1; skipped without the driver (docs/developing/software-gpu.md).
if [ "$SET" = full ] || [ -n "${LAVAPIPE:-}" ]; then
  run lavapipe 3600 "$HERE/lavapipe.sh" "$O/lavapipe" "$BIN"
  say "lavapipe rc=$RC: $(tail -1 "$O/lavapipe/lavapipe.txt" 2>/dev/null)"
  # rc 3: no lavapipe driver here. Recorded as skipped (summary, last line), never as a pass;
  # LAVAPIPE=require turns it into a failure.
  if [ $RC = 3 ]; then
    if [ "${LAVAPIPE:-}" = require ]; then fail "lavapipe (no driver, LAVAPIPE=require)"; else SKIPPED+=(lavapipe); fi
  elif [ $RC != 0 ]; then fail lavapipe; fi
fi
# The Windows build under Wine (wine_smoke.sh, docs/developing/checks.md "Wine smoke"), report-only for now:
# its line and time in the summary, never a failure — except when the Windows build itself does not
# compile ("WINE_SMOKE: FAIL (build)"): that fails the set (wine_build), since no Windows game exists then.
# WINE_SMOKE=off skips it; without wine / Xvfb (or podman and no WINE_EXE) it is recorded as skipped. The
# Linux reference images are this run's shots. WINE_SMOKE_SH: another script in its place (the self-check).
if [ "$SET" = full ] || [ -n "${WINE_SMOKE:-}" ]; then
  if [ "${WINE_SMOKE:-}" = off ]; then
    say "wine_smoke: SKIP (WINE_SMOKE=off)"; SKIPPED+=(wine_smoke)
  else
    run wine_smoke 7200 env WINE_REF="$O/shots" PIXDIFF="$PIXDIFF" "${WINE_SMOKE_SH:-$HERE/wine_smoke.sh}" "$O/wine_smoke" "$BIN"
    say "wine_smoke rc=$RC ${STEP_S[wine_smoke]} s (report-only): $(grep -E '^WINE_SMOKE' "$O/wine_smoke/wine_smoke.txt" 2>/dev/null | tail -1 | cut -c1-200)"
    grep -E '^[a-z0-9_]+: (FAIL|XFAIL)' "$O/wine_smoke/wine_smoke.txt" 2>/dev/null | head -12 | cut -c1-200 | sed 's/^/  /' | tee -a "$OUT"
    [ $RC = 3 ] && SKIPPED+=(wine_smoke)
    grep -q '^WINE_SMOKE: FAIL (build)' "$O/wine_smoke/wine_smoke.txt" 2>/dev/null && fail wine_build
  fi
fi
# The developer probes (devprobes/, not built by the build step) still compile: a failure fails the
# full set, so a library change that breaks a probe is fixed in the same change instead of rotting.
if [ "$SET" = full ] || [ -n "${DEVPROBES:-}" ]; then
  run devprobes 1800 "$HERE/devprobes.sh"
  say "devprobes rc=$RC ${STEP_S[devprobes]} s: $(tail -1 "$O/devprobes.log" | cut -c1-160)"
  # rc 3: no devprobes/ member (a public checkout; the probes are private).
  # Never a failure: with OB_PRIVATE pointing at the set the checkout still has no probes (only an overlay has).
  if [ $RC = 3 ]; then SKIPPED+=("devprobes (needs the maintainer's private set laid over the checkout)"); RC=0; fi
  [ $RC = 0 ] || { grep -E '^error' -A6 "$O/devprobes.log" | head -30 | tee -a "$OUT"; fail devprobes; }
fi
# The fuzz smoke (docs/developing/fuzzing.md): every parser target on mutated inputs over seeds from the player's game files.
if [ "$SET" = full ] || [ -n "${FUZZ:-}" ]; then
  run fuzz 3600 "$HERE/fuzz.sh"
  say "fuzz rc=$RC ${STEP_S[fuzz]} s: $(tail -1 "$O/fuzz.log" | cut -c1-160)"
  [ $RC = 0 ] || { grep -E '^(error|fuzz|[a-z_0-9]+: (input|seed))' "$O/fuzz.log" | head -30 | tee -a "$OUT"; fail fuzz; }
fi
# The shipping binary (no dev-harness): no harness strings, harness flags end in the usage error.
if [ "$SET" = full ] || [ -n "${RELEASE:-}" ]; then
  run release 3600 "$HERE/release_check.sh" "$O/release"
  say "release rc=$RC ${STEP_S[release]} s: $(tail -1 "$O/release.log" | cut -c1-200)"
  # rc 3: everything passed but the private save names were not searched (no privacy scan or no private terms).
  # Without the private set that is expected: the terms are the maintainer's own.
  if [ $RC = 3 ]; then skip_private "release: the private save names not searched (needs the maintainer's private set (OB_PRIVATE))"
  else [ $RC = 0 ] || { grep -E '^(release|error)' "$O/release.log" | head -20 | tee -a "$OUT"; fail release; }; fi
fi
# The launcher's first run on a fresh machine (tests/launcher_fresh.sh, docs/launcher.md): the shipping game and
# launcher side by side (built into the release check's target dir), an empty HOME, a private Xvfb; FRESH=1 in fast.
if [ "$SET" = full ] || [ -n "${FRESH:-}" ]; then
  if command -v Xvfb > /dev/null && command -v xdotool > /dev/null; then
    SHIP=${SHIP_TARGET:-$CARGO_TARGET_DIR/ship}
    run fresh 1800 bash -c 'cargo build --release --bin game --bin overbright --target-dir "$1" && "$2" "$1/release" auto "$3"' _ \
      "$SHIP" "$REPO/tests/launcher_fresh.sh" "$O/fresh"
    say "fresh rc=$RC ${STEP_S[fresh]} s: $(tail -1 "$O/fresh.log" | cut -c1-200)"
    [ $RC = 0 ] || { grep -E '^(FAIL|error)' "$O/fresh.log" | head -10 | tee -a "$OUT"; fail fresh; }
    # The launcher by the mouse alone (tests/launcher_mouse.sh): the folder browser by clicks, double clicks,
    # the wheel and the scroll bar, a game folder chosen, a package added; at display scale 1 and 2.
    for sc in 1 2; do
      run "mouse$sc" 900 "$REPO/tests/launcher_mouse.sh" "$SHIP/release/overbright" auto "$O/mouse$sc" "$sc"
      say "mouse$sc rc=$RC ${STEP_S[mouse$sc]} s: $(tail -1 "$O/mouse$sc.log" | cut -c1-200)"
      [ $RC = 0 ] || { grep -E '^(FAIL|error)' "$O/mouse$sc.log" | head -10 | tee -a "$OUT"; fail "mouse$sc"; }
    done
    # The same browser on Wayland at scale 1, 1.25, 1.5, 2 (tests/launcher_wayland.sh: a private headless mutter;
    # SKIP without mutter or python gi).
    run wayland 900 "$REPO/tests/launcher_wayland.sh" "$SHIP/release/overbright" "$O/wayland"
    say "wayland rc=$RC ${STEP_S[wayland]} s: $(tail -1 "$O/wayland.log" | cut -c1-200)"
    if [ $RC = 3 ]; then SKIPPED+=(wayland)
    else [ $RC = 0 ] || { grep -E 'RESULT FAIL|^WAYLAND' "$O/wayland.log" | head -10 | tee -a "$OUT"; fail wayland; }; fi
  else
    say "fresh: SKIP (no Xvfb / xdotool)"; SKIPPED+=(fresh)
  fi
fi
# The gates' own self-test (selfcheck.sh): every gate must fail on a broken input and pass a good one.
if [ "$SET" = full ] || [ -n "${SELFCHECK:-}" ]; then
  run selfcheck 1800 "$HERE/selfcheck.sh" "$O/selfcheck"
  say "selfcheck rc=$RC: $(tail -1 "$O/selfcheck.log" | cut -c1-200)"
  [ $RC = 0 ] || { grep -E '^(BAD|FAIL)' "$O/selfcheck.log" | head -20 | tee -a "$OUT"; fail selfcheck; }
fi
# Rate gate v2 (full set; docs/developing/checks.md "Rate gate"): this binary against itself at 30/60/120/200 Hz and a jitter
# schedule (perf.sh rate). RATE=strict (the default) fails on every difference; RATE=1 on a run that failed, the
# gate's self-test, or a column / image class scripts/checks/rate-columns.txt marks "fail" (since the final policy
# every one); RATE=0 leaves the step out.
RATE=${RATE:-strict}
if [ "$SET" = full ] && [ "$RATE" != 0 ]; then
  run rate 14400 env PIXDIFF="$PIXDIFF" "$REPO/scripts/perf/perf.sh" rate "$O/rate" "$BIN"
  say "rate rc=$RC ${STEP_S[rate]} s ($([ "$RATE" = strict ] && echo strict || echo policy)): $(grep '^RATE:' "$O/rate/rate.txt" 2>/dev/null | tail -1 | cut -c1-200)"
  grep -E '^(FAIL|rate self-test)' "$O/rate/rate.txt" 2>/dev/null | cut -c1-200 | tee -a "$OUT"
  case $RC in 0) ;; 3) [ "$RATE" != strict ] || fail "rate (RATE=strict)" ;; *) fail rate ;; esac
  # A flow that needs the private set (perf.sh rate: "rate FLOW: SKIP (...)") is a skip, as the shots' and flows' are.
  while read -r rf; do skip_private "rate $rf (needs the maintainer's private set (OB_PRIVATE))"; done \
    < <(sed -n 's/^rate \([a-z_0-9]*\): SKIP .*/\1/p' "$O/rate/rate.txt" 2>/dev/null)
fi
# Image soak (off by default; SOAK=N, N >= 2): the parity gate's Original image dumps N times with this
# binary, every run compared with the first (soak_images.sh, docs/developing/checks.md "Image soak"). SOAK_SCENES
# (default "drive race"), SOAK_SIZES (default "1280x960 3840x2160"), SOAK_PAR, SOAK_LOAD, SOAK_IOLOAD.
if [ -n "${SOAK:-}" ] && [ "${SOAK:-0}" != 0 ] && [ "$SET" != shots ]; then
  run soak 14400 env PAR="${SOAK_PAR:-1}" LOAD="${SOAK_LOAD:-0}" IOLOAD="${SOAK_IOLOAD:-0}" PIXDIFF="$PIXDIFF" \
    "$HERE/soak_images.sh" "$BIN" "$O/soak" "$SOAK" "${SOAK_SCENES:-drive race}" "${SOAK_SIZES:-1280x960 3840x2160}"
  say "soak rc=$RC ${STEP_S[soak]} s: $(grep -E '^[a-z]+-[0-9]+x[0-9]+: ' "$O/soak/soak.txt" 2>/dev/null | cut -d';' -f1 | tr '\n' ';' | cut -c1-300) $(grep '^SOAK' "$O/soak/soak.txt" 2>/dev/null | cut -d' ' -f1-3)"
  [ $RC = 0 ] || fail soak
fi

# ---- the player's folders untouched ------------------------------------------------
if [ -n "$PLAYER_DIRS" ]; then
  ck_player_snapshot > "$O/player-after.txt"
  if D=$(ck_player_compare "$O/player-before.txt" "$O/player-after.txt"); then
    say "player folders: unchanged ($(wc -l < "$O/player-after.txt") entries)"
  else
    say "player folders: CHANGED during the run ($O/player-before.txt, player-after.txt), first differences:"
    echo "$D" | tee -a "$OUT"
    if [ -s "$O/player-games.txt" ]; then
      say "player folders: not judged, a game of the player's own used them during the run: $(tr '\n' ' ' < "$O/player-games.txt")"
      SKIPPED+=("player folders (the player's own game ran)")
    else
      fail "player folders"
    fi
  fi
  ck_player_watch_stop
fi

# ---- summary -----------------------------------------------------------------
TOTAL=$(( $(date +%s) - T0 ))
OK=true; [ ${#FAIL[@]} -gt 0 ] && OK=false
md5=$(md5sum < "$BIN" | cut -c1-12)
python3 - "$O" "$SET" "$OK" "$TOTAL" "$md5" "${TESTS:-skipped}" "$NSHOTS" "$NBAD" "$NDIFF" "${NEWNONDET# }" "${NPREV% }" "$(printf '%s|' "${FAIL[@]}")" "$(printf '%s|' "${SKIPPED[@]}")" "$(for k in "${!STEP_S[@]}"; do echo -n "$k=${STEP_S[$k]} "; done)" <<'EOF' | tee -a "$OUT"
import json, sys, datetime
o, st, ok, total, md5, tests, n, bad, nd, newnd, prev, fails, skipped, steps = sys.argv[1:]
d = {"set": st, "date": datetime.datetime.now().astimezone().isoformat(timespec="seconds"), "ok": ok == "true",
     "seconds": int(total), "bin_md5": md5, "tests": tests, "shots": int(n), "shots_failed": int(bad),
     "nondeterministic_new": newnd.split(), "changed_vs_prev": prev.split(),
     "failed": [f for f in fails.split("|") if f], "skipped": [f for f in skipped.split("|") if f], "steps_s": {k: int(v) for k, v in (x.split("=") for x in steps.split())}}
line = json.dumps(d, separators=(",", ":"))
open(f"{o}/summary.json", "w").write(line + "\n")
md = (f"| {d['date']} | {st} | {'PASS' if d['ok'] else 'FAIL'} | {total} s | {md5} | tests {tests} | shots {n} ({bad} failed, "
      f"{len(d['nondeterministic_new'])} new nondet) | {', '.join(d['failed']) or '-'} | skipped {', '.join(d['skipped']) or '-'} |")
open(f"{o}/summary.md", "w").write(md + "\n")
print(md)
print(line)
EOF
cat "$O/summary.json" >> "${HISTORY:-$O/../checks-history.jsonl}"
rm -rf "$TMPDIR"
SK=""; [ ${#SKIPPED[@]} -gt 0 ] && SK=" skipped: ${SKIPPED[*]};"
say "$([ "$OK" = true ] && echo "CHECKS: PASS" || echo "CHECKS: FAIL (${FAIL[*]})") ($SET,$SK $TOTAL s)"
[ "$OK" = true ]
