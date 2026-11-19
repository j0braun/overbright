#!/bin/bash
# Merge gate (docs/developing/checks.md): the checks plus the parity of a base against this tree.
#
#   gate.sh OUTDIR BASE [extreme]
#
# BASE: a git commit of this repo (git archive) or a source directory (another copy of the repo).
# Runs checks.sh (CHECKS=fast|full, default fast) on this tree into OUTDIR/chk, builds BASE and this tree
# with perf.sh's full release profile (one target dir per tree), prints both md5s, and runs perf.sh parity
# with the deterministic shot targets (fast-shots.txt minus nondeterministic.txt, minus targets the base
# does not have; a target renamed since the base, scripts/checks/shot-renames.txt, runs under its old name on the
# base side: SHOT_BASE_MAP; a private fixture renamed since the base, scripts/checks/fixture-renames.txt, is linked
# under its old name in the parity's private set). "extreme": also the Extreme presets in the enhanced gate. The rate gate v2 on the new binary runs
# by default (in checks.sh full, else in the parity) and counts: RATE=strict (the default) fails on any difference
# across rates, RATE=1 on what scripts/checks/rate-columns.txt marks "fail" (since the final policy the same set),
# RATE=0 leaves it out; docs/developing/checks.md "Rate gate".
# Identical binaries: fine when the game's sources (src/, examples/, build.rs, Cargo.*) are identical too
# (a docs/scripts-only change; reported as such); a FAILURE when they differ (a stale or wrong build: the
# parity would compare a binary with itself).
# GATE_EXCLUDE: extra rsync --exclude patterns (space separated) when copying the trees (scripts/local.env).
# Everything goes to OUTDIR (big disk, never /tmp); the source copies and perf-work are removed at the end.
set -u -o pipefail
HERE=$(cd "$(dirname "$0")" && pwd)
REPO=$(cd "$HERE/../.." && pwd)
. "$HERE/../lib/common.sh"
ck_local_env
G=${1:?usage: gate.sh OUTDIR BASE [extreme]}; B=${2:?usage: gate.sh OUTDIR BASE [extreme]}
ck_need_root
ck_need_tools rsync git tar md5sum python3 cargo
rm -rf "$G"; mkdir -p "$G"; G=$(cd "$G" && pwd)
cd "$REPO" || exit 2
echo "gate: $(date -Is) repo $REPO base $B local env $CK_LOCAL_ENV"
export RATE=${RATE:-strict}
# The checks step builds with the gate profile (CHECKS_PROFILE, docs/developing/build-times.md); the parity's base and
# candidate are both built by perf.sh build with the shipped release profile, so the parity compares like with like.
export CHECKS_PROFILE=$CK_PROFILE
echo "gate: checks profile $CK_PROFILE; parity builds: release (both sides)"
# The player's folders before the gate (its builds, checks and parity); compared at the end (ck_player_snapshot).
ck_player_snapshot > "$G/player-before.txt"
[ -z "$PLAYER_DIRS" ] || ck_player_watch "$G/player-games.txt"
"$HERE/checks.sh" "${CHECKS:-fast}" "$G/chk" > "$G/checks.log" 2>&1; CRC=$?
if [ -f "$G/chk/checks.txt" ]; then
  grep -E '^(build|tests|data tests|selftest|prepare|shots|determinism|firstrace|firstdrag|postrace|playthrough|matrix|resolutions|lavapipe|wine_smoke|devprobes|fuzz|release|fresh|mouse[12]|wayland|selfcheck|soak|rate|player folders|FAIL|CHECKS)' "$G/chk/checks.txt"
else
  echo "gate: checks.sh wrote no checks.txt (rc $CRC): $(tail -3 "$G/checks.log" | tr '\n' ' ')"; [ $CRC != 0 ] || CRC=1
fi
mkdir -p "$G/src-a" "$G/src-b" "$G/a" "$G/b"
EXC=(--exclude /target --exclude '/target-*' --exclude .git --exclude /scripts/local.env)
for x in ${GATE_EXCLUDE:-}; do EXC+=(--exclude "$x"); done
if [ -d "$B" ]; then rsync -a "${EXC[@]}" "$B/" "$G/src-a/" || { echo "gate: copying BASE $B failed"; exit 2; }
else git archive "$B" | tar -x -C "$G/src-a" || { echo "gate: BASE $B is neither a directory nor a commit"; exit 2; }; fi
rsync -a "${EXC[@]}" ./ "$G/src-b/" || { echo "gate: copying this tree failed"; exit 2; }
PW="$G/perf-work"
# The base binary is the same for every unit gated against one base: with GATE_BASE_CACHE=<dir> (machine-local, e.g. in
# scripts/local.env) it is built once per base source tree and build script, then reused.
BKEY=""
if [ -n "${GATE_BASE_CACHE:-}" ]; then
  BKEY=$( { (cd "$G/src-a" && find src crates build_support examples build.rs Cargo.toml Cargo.lock rustfmt.toml -type f 2>/dev/null | LC_ALL=C sort | xargs -r md5sum); md5sum < scripts/perf/perf.sh; md5sum < scripts/lib/common.sh; rustc -V; } | md5sum | cut -d' ' -f1)
fi
if [ -n "$BKEY" ] && [ -x "$GATE_BASE_CACHE/$BKEY/game" ] && [ -x "$GATE_BASE_CACHE/$BKEY/pixdiff" ]; then
  cp "$GATE_BASE_CACHE/$BKEY/game" "$GATE_BASE_CACHE/$BKEY/pixdiff" "$G/a/" && echo "gate: base binary from the cache ($BKEY)" | tee "$G/build-a.log"
else
  PERF_WORK=$PW scripts/perf/perf.sh build "$G/src-a" "$G/a/game" > "$G/build-a.log" 2>&1 || { echo "gate: base build failed (see $G/build-a.log)"; exit 1; }
  if [ -n "$BKEY" ]; then
    mkdir -p "$GATE_BASE_CACHE/$BKEY.tmp$$" && cp "$G/a/game" "$G/a/pixdiff" "$GATE_BASE_CACHE/$BKEY.tmp$$/" \
      && { mv -T "$GATE_BASE_CACHE/$BKEY.tmp$$" "$GATE_BASE_CACHE/$BKEY" 2>/dev/null || rm -rf "$GATE_BASE_CACHE/$BKEY.tmp$$"; }
  fi
fi
PERF_WORK=$PW scripts/perf/perf.sh build "$G/src-b" "$G/b/game" > "$G/build-b.log" 2>&1 || { echo "gate: new build failed (see $G/build-b.log)"; exit 1; }
md5sum "$G/a/game" "$G/b/game" | tee "$G/md5.txt"
SAME=0; SRC_SAME=1; SRC_DIFF=""
if [ "$(md5sum < "$G/a/game")" = "$(md5sum < "$G/b/game")" ]; then
  SAME=1
  for p in src examples build.rs Cargo.toml Cargo.lock; do
    if [ -e "$G/src-a/$p" ] || [ -e "$G/src-b/$p" ]; then
      diff -rq "$G/src-a/$p" "$G/src-b/$p" > /dev/null 2>&1 || { SRC_SAME=0; SRC_DIFF="$SRC_DIFF $p"; }
    fi
  done
  if [ $SRC_SAME = 1 ]; then echo "gate: note: base and new binaries are identical and so are the game's sources (a change outside the game)"
  else echo "gate: FAIL: base and new binaries are identical although the game's sources differ ($SRC_DIFF ): a stale or wrong build; the parity would compare a binary with itself"; fi
fi
[ -n "${3:-}" ] && export ENH_PRESETS="${GATE_ENH_PRESETS:-extreme extreme_fsr}"
SC=${SHOT_CLOCK-0}
NONDET=" $(awk -v wall="$([ -z "$SC" ] && echo 1)" '!/^#/ && NF && ($2 == "always" || ($2 == "wallclock" && wall == 1)) {print $1}' "$HERE/nondeterministic.txt" | tr '\n' ' ') " || { echo "gate: cannot read nondeterministic.txt"; exit 2; }
# Only targets the base knows too (a new target has no base image), minus the nondeterministic ones.
KNOWN=" $(python3 "$HERE/shot_targets.py" "$G/src-a" "$G/a/game" | tr '\n' ' ') " || { echo "gate: shot_targets.py failed on the base"; exit 2; }
FAST=$(grep -v '^#' "$HERE/fast-shots.txt") || { echo "gate: no fast-shots.txt"; exit 2; }
CAND=""; for s in $FAST; do case $NONDET in *" $s "*) continue ;; esac; CAND="$CAND $s"; done
# A target the base knows under an old name (shot-renames.txt) is compared too: base OLD against new NEW.
PICK=$(printf '%s\n' $KNOWN | python3 "$HERE/shot_renames.py" pick - $CAND) || { echo "gate: shot_renames.py failed"; exit 2; }
SHOTS=$(awk '{printf " %s", $1}' <<< "$PICK"); SHOT_BASE_MAP=$(awk '$1 != $2 {printf "%s=%s ", $1, $2}' <<< "$PICK")
[ -n "$SHOTS" ] || { echo "gate: no parity shots (fast-shots.txt minus nondeterministic minus unknown to the base is empty)"; exit 2; }
echo "gate: parity shots:$SHOTS"
[ -z "$SHOT_BASE_MAP" ] || echo "gate: renamed shots (new=base): $SHOT_BASE_MAP"
# The shots with the clock pinned at 0, as checks.sh (all of fast-shots.txt repeat run to run).
export SHOTS SHOT_BASE_MAP NFSU2_SHOT_CLOCK=$SC
GATES_RUN=${GATES:-images window enhanced shots audio sim}
# The rate gate (RATE=strict by default, 1, or 0 = off): checks.sh full runs it (inherited RATE); with CHECKS=fast
# the parity's rate gate does.
export RATE=${RATE:-strict}
case $RATE in 1|strict) [ "${CHECKS:-fast}" = full ] || GATES_RUN="$GATES_RUN rate" ;; esac
# The game binaries' own data folders (std::env::temp_dir) under OUTDIR, not /tmp.
mkdir -p "$G/tmp"
# The harness reads the maintainer's private set from $OB_PRIVATE, else from the source folder its binary was built
# in (src/bin/game/dev/private.rs). A base binary taken from the cache was built in another gate's folder, which may
# be gone: both binaries read this gate's copy of the new tree unless the caller names a set.
# A private fixture renamed since the base (scripts/checks/fixture-renames.txt): the base binary reads it under its old
# name, so the parity's set has both names (a link OLD -> NEW). The links go into this gate's own copy: $G/src-b, or a
# copy of the caller's set made of links (cp -as), never into the caller's set itself.
PRIV="$G/src-b"
if [ -n "${OB_PRIVATE:-}" ]; then
  PRIV="$G/private-set"; mkdir -p "$PRIV" && cp -as "$(cd "$OB_PRIVATE" && pwd)/." "$PRIV/" || { echo "gate: cannot link the private set $OB_PRIVATE"; exit 2; }
fi
AL=$(python3 "$HERE/fixture_renames.py" alias "$PRIV") || { echo "gate: fixture_renames.py failed"; exit 2; }
[ "$AL" = "aliased 0" ] || echo "gate: renamed private fixtures: $AL under their old names for the base"
TMPDIR="$G/tmp" OB_PRIVATE="$PRIV" PERF_WORK=$PW GATES=$GATES_RUN scripts/perf/perf.sh parity "$G/parity" "$G/a/game" "$G/b/game" > "$G/parity.log" 2>&1; PRC=$?
grep -E '^(FAIL: |images|window|enhanced|shots|audio|sim|rate|RATE|PARITY)' "$G/parity.log" | cut -c1-240
grep -q '^PARITY: ' "$G/parity.log" || { echo "gate: perf.sh parity ended without a verdict (rc $PRC): $(tail -3 "$G/parity.log" | tr '\n' ' ')"; [ $PRC != 0 ] || PRC=1; }
rm -rf "$G/src-a" "$G/src-b" "$G/private-set" "$PW" "$G/tmp"
BRC=0; [ $SAME = 1 ] && [ $SRC_SAME = 0 ] && BRC=1
if [ -n "$PLAYER_DIRS" ]; then
  ck_player_snapshot > "$G/player-after.txt"
  if D=$(ck_player_compare "$G/player-before.txt" "$G/player-after.txt"); then echo "gate: player folders unchanged"
  elif [ -s "$G/player-games.txt" ]; then
    echo "gate: the player's folders changed during the gate; not judged, a game of the player's own used them: $(tr '\n' ' ' < "$G/player-games.txt")"; echo "$D"
  else echo "gate: FAIL: the player's folders changed during the gate:"; echo "$D"; BRC=1; fi
  ck_player_watch_stop
fi
# What did not run here (each named on its own line above: the CHECKS line's "skipped:", the parity's "SKIP" lines)
# is counted on the verdict line too: a gate with skips is never a plain PASS.
SKC=$(python3 -I -c 'import json, sys; print(len(json.load(open(sys.argv[1]))["skipped"]))' "$G/chk/summary.json" 2> /dev/null || echo "?")
SKP=$(grep -cE '^[a-z][a-z ]*: SKIP' "$G/parity.log" 2> /dev/null); SKP=${SKP:-0}
SKN=""; [ "$SKC" = 0 ] && [ "$SKP" = 0 ] || SKN="; skipped: checks $SKC, parity $SKP (named above)"
echo "GATE: $([ $CRC = 0 ] && [ $PRC = 0 ] && [ $BRC = 0 ] && echo PASS || echo FAIL): checks rc=$CRC parity rc=$PRC binaries $([ $SAME = 1 ] && { [ $SRC_SAME = 1 ] && echo "identical (sources identical)" || echo "IDENTICAL WITH DIFFERENT SOURCES"; } || echo differ)$SKN"
[ $CRC = 0 ] && [ $PRC = 0 ] && [ $BRC = 0 ]
