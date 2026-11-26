# Shared helpers of the check and perf scripts (docs/developing/checks.md "Machine setup"). Source it after `set -u`:
#
#   . "$(dirname "$0")/../lib/common.sh"
#
# ck_local_env   sources the machine's own, git-ignored settings: $NFSU2_LOCAL_ENV, else scripts/local.env
#                (documented in scripts/local.env.example: NFSU2_ROOT, build env, perf presets, baselines ...).
#                Sets CK_LOCAL_ENV to the file used ("none" without one) so the scripts can log it.
# ck_need_root   NFSU2_ROOT must name the game's data folder (no default: it differs on every machine).
# ck_need_tools  T…: every tool on PATH, else one clear message naming the missing ones.
# ck_bad FILE    the number of panic / validation-error lines of a log; a missing log counts as 1.
# ck_die MSG     message on stderr, exit 2.
# ck_private_file REL    a private-set file's path ($OB_PRIVATE, else this checkout), or nothing and status 1.
# ck_private_here REPO   true when that tree's build holds the private set: the rule of build.rs's `ob_private`
#                ($OB_PRIVATE names a folder, or REPO/tests/fixtures/native_saves exists). A step that skips
#                for lack of the set (src/bin/game/dev/private.rs, exit status 3) is then a failure, not a skip.
# ck_data_tests LOG REPO  the data-test rule on a cargo test log (below).
# CK_PROFILE, CK_PDIR  the Cargo profile of the checks' own builds and its output folder under the target dir:
#                $CHECKS_PROFILE, default `gate` (Cargo.toml [profile.gate], docs/developing/build-times.md); `release` for the
#                shipped profile. Releases, perf.sh, perfgate, pgo.sh and release_check.sh always use --release.
CK_LIB=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
CK_REPO=$(cd "$CK_LIB/../.." && pwd)
# The one panic / validation pattern (case-insensitive, ERE) every gate greps for.
CK_PANIC_RE='panicked|validation error'
ck_profile() { CK_PROFILE=${CHECKS_PROFILE:-gate}; case $CK_PROFILE in dev) CK_PDIR=debug ;; *) CK_PDIR=$CK_PROFILE ;; esac; }
ck_profile

ck_die() { echo "$(basename "$0"): $*" >&2; exit 2; }

ck_local_env() {
  [ -n "${CK_LOCAL_ENV:-}" ] && return 0   # once per process tree (exported)
  local f=${NFSU2_LOCAL_ENV:-$CK_REPO/scripts/local.env}
  if [ -f "$f" ]; then
    # shellcheck disable=SC1090
    . "$f" || ck_die "error while sourcing $f"
    CK_LOCAL_ENV=$f
  else
    [ -n "${NFSU2_LOCAL_ENV:-}" ] && ck_die "NFSU2_LOCAL_ENV=$NFSU2_LOCAL_ENV does not exist"
    CK_LOCAL_ENV=none
  fi
  export CK_LOCAL_ENV
  ck_profile   # the machine's settings may name CHECKS_PROFILE
}

ck_need_root() {
  [ -n "${NFSU2_ROOT:-}" ] || ck_die "NFSU2_ROOT is not set: export it (or set it in scripts/local.env) to the folder holding the game's data (GLOBAL/, TRACKS/, ...)"
  [ -d "$NFSU2_ROOT" ] || ck_die "NFSU2_ROOT=$NFSU2_ROOT is not a directory"
  export NFSU2_ROOT
}

ck_need_tools() {
  local t miss=()
  for t in "$@"; do command -v "$t" > /dev/null 2>&1 || miss+=("$t"); done
  [ ${#miss[@]} = 0 ] || ck_die "missing tools: ${miss[*]} (install them or put them on PATH)"
}

ck_bad() {
  [ -f "$1" ] || { echo 1; return 0; }
  local n; n=$(grep -ciE "$CK_PANIC_RE" "$1")
  echo "${n:-0}"
}
# ck_private_file REL: the path of a private-set file (REL relative to the set's root): under $OB_PRIVATE, else in
# this checkout while it holds the set; prints nothing and returns 1 when neither has it (a public checkout).
ck_private_file() {
  local d
  for d in "${OB_PRIVATE:-}" "$CK_REPO"; do [ -n "$d" ] && [ -f "$d/$1" ] && { echo "$d/$1"; return 0; }; done
  return 1
}
ck_private_here() {
  if [ -n "${OB_PRIVATE:-}" ]; then [ -d "$OB_PRIVATE" ]; return; fi
  [ -d "$1/tests/fixtures/native_saves" ]
}

# ck_data_tests LOG REPO: the data-test rule of the checks (docs/developing/testing.md) on a `cargo test` log. The ignored
# data tests are counted by reason: the game files alone ("needs NFSU2_ROOT ..."), the private set alone
# ("needs OB_PRIVATE ..."), both. A skip fails only when this run could have avoided it: NFSU2_ROOT is set and a
# test is ignored for the game files alone, or the private set is here (ck_private_here REPO, or
# OB_REQUIRE=private) and a test is ignored for it (alone or with the game files). Prints the counts line, then
# for a failure the first ignored tests and one line "FAIL: <reason>"; exit 1 on a failure.
ck_data_tests() {
  local log=$1 repo=$2 game priv both bad=0
  game=$(grep -E '^test .* \.\.\. ignored, needs NFSU2_ROOT' "$log" | grep -cv 'OB_PRIVATE')
  priv=$(grep -cE '^test .* \.\.\. ignored, needs OB_PRIVATE' "$log")
  both=$(grep -E '^test .* \.\.\. ignored, needs NFSU2_ROOT' "$log" | grep -c 'OB_PRIVATE')
  local have_priv=0 why_priv=""
  if ck_private_here "$repo"; then have_priv=1; why_priv="the private set is here"
  else case ",${OB_REQUIRE:-}," in *,private,*) have_priv=1; why_priv="OB_REQUIRE=private" ;; esac; fi
  echo "data tests ignored: $game need the game files (NFSU2_ROOT), $priv the private set (OB_PRIVATE), $both both;" \
    "NFSU2_ROOT $([ -n "${NFSU2_ROOT:-}" ] && echo set || echo unset), private set $([ $have_priv = 1 ] && echo "here ($why_priv)" || echo "not here (its tests are a correct skip)")"
  if [ -n "${NFSU2_ROOT:-}" ] && [ "$game" != 0 ]; then
    grep -E '^test .* \.\.\. ignored, needs NFSU2_ROOT' "$log" | grep -v 'OB_PRIVATE' | head -10
    echo "FAIL: data tests ($game ignored for the game files although NFSU2_ROOT is set: a stale build or a broken test kit)"
    bad=1
  fi
  if [ $have_priv = 1 ] && [ $((priv + both)) != 0 ]; then
    grep -E '^test .* \.\.\. ignored, .*OB_PRIVATE' "$log" | head -10
    echo "FAIL: data tests ($((priv + both)) ignored for the private set although $why_priv)"
    bad=1
  fi
  return $bad
}

# The player's own cache and data folders, read-only for every check: ck_player_snapshot prints their file lists
# (path, type, size, modification time; sorted) for a before/after comparison. Read only; a folder that is not there
# is not made. PLAYER_DIRS (space-separated) names them (default: the cache and data folders under $HOME);
# PLAYER_DIRS= (empty) prints nothing, e.g. while the player plays on this machine during a run.
PLAYER_DIRS=${PLAYER_DIRS-"${HOME:-/nonexistent}/.cache/overbright ${HOME:-/nonexistent}/.local/share/overbright"}
ck_player_snapshot() {
  local d
  for d in $PLAYER_DIRS; do [ -d "$d" ] && find "$d" -printf "$d/%P\t%y\t%s\t%T@\n" 2>/dev/null; done | LC_ALL=C sort
}
# ck_player_games: the programs running now that use the player's own folders: processes of this user whose program
# is named game, overbright, overbright-studio, overbright-editor or overbright-studio-cli (the player's own session:
# playing, or Studio open on a map mod) and whose environment names no folder of its own: no NFSU2_CACHE_DIR (every
# game a check or a gate starts has one), no NFSU2_DATA_DIR, no XDG_DATA_HOME other than ours, and a HOME that is
# ours (a test that runs a program under a fake home is not the player). One program path per line. Linux only
# (reads /proc); prints nothing elsewhere.
ck_player_games() {
  local d exe env
  for d in /proc/[0-9]*; do
    [ -O "$d" ] || continue
    exe=$(readlink "$d/exe" 2>/dev/null) || continue
    case ${exe##*/} in
      game|overbright|overbright-studio|overbright-editor|overbright-studio-cli) ;;
      "game (deleted)"|"overbright (deleted)"|"overbright-studio (deleted)"|"overbright-editor (deleted)") ;;
      *) continue ;;
    esac
    env=$(tr '\0' '\n' < "$d/environ" 2>/dev/null) || continue
    grep -q '^NFSU2_CACHE_DIR=\|^NFSU2_DATA_DIR=' <<< "$env" && continue
    grep -q '^HOME=' <<< "$env" && ! grep -qxF "HOME=${HOME:-}" <<< "$env" && continue
    grep -q '^XDG_DATA_HOME=.' <<< "$env" && ! grep -qxF "XDG_DATA_HOME=${XDG_DATA_HOME:-}" <<< "$env" && continue
    echo "$exe"
  done
}
# ck_player_watch FILE: until the calling script ends, notes every few seconds which such games run (FILE: one
# program path per line, each once). A change in the player's folders while one of them ran is the player's own
# game at work and is not judged (the caller says so and lists it as a skip); without one it fails.
ck_player_watch() {
  local f=$1 me=$$
  : > "$f"
  ( while kill -0 "$me" 2>/dev/null; do
      ck_player_games | while read -r exe; do grep -qxF "$exe" "$f" 2>/dev/null || echo "$exe" >> "$f"; done
      sleep 3
    done ) > /dev/null 2>&1 &
  CK_PLAYER_WATCH_PID=$!
}
ck_player_watch_stop() {
  [ -n "${CK_PLAYER_WATCH_PID:-}" ] && kill "$CK_PLAYER_WATCH_PID" 2>/dev/null
  CK_PLAYER_WATCH_PID=
}
# ck_player_compare BEFORE AFTER: 0 when equal; else the first differences on stdout and 1.
# A game log (logs/game-<start time>.log) that was there before and has only grown is the player's own running
# game writing on: a game a check starts gets a log of its own (a new entry, which is a difference). Such
# lines are left out of the comparison.
ck_player_compare() {
  cmp -s "$1" "$2" && return 0
  local d
  d=$(awk -F'\t' -v A="$1" '
    FILENAME == A { before[$1] = $0; bsize[$1] = $3; next }
    { after[$1] = $0; asize[$1] = $3 }
    END {
      for (k in before) {
        if (!(k in after)) { print "< " before[k]; continue }
        if (before[k] == after[k]) continue
        if (k ~ /\/logs\/game-[^\/]*\.log$/ && asize[k] + 0 >= bsize[k] + 0) continue
        print "< " before[k]; print "> " after[k]
      }
      for (k in after) if (!(k in before)) print "> " after[k]
    }' "$1" "$2" | LC_ALL=C sort -k2 | head -8 | cut -c1-200)
  [ -z "$d" ] && return 0
  echo "$d"
  return 1
}
