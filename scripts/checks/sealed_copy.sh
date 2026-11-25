#!/bin/bash
# sealed_copy.sh: the full playthrough reads no game file from the game folder once the player's own copy is made.
#
#   sealed_copy.sh GAME SET OUTDIR
#     GAME    a game binary built with dev-harness
#     SET     a finished base set's folder (overbright-import import --game "$NFSU2_ROOT" --base <dir>)
#     OUTDIR  work folder (made; a skeleton of the game folder, the two runs' logs and state hashes)
#
# Two runs of `GAME --playthrough` with per-step state hashes (NFSU2_STATE_HASH):
#   folder  NFSU2_BASE=off: the game folder, as without a copy;
#   sealed  NFSU2_BASE=SET in a user + mount namespace in which the game folder ($NFSU2_ROOT) is replaced by a
#           skeleton: the same folders and names, every file the same size and time but sparse, without read
#           permission, and the run without capabilities (setpriv): opening any file of the game folder fails, so
#           a read the copy does not answer cannot go unnoticed. The names and folders stay (the game still lists
#           the folder to find a file's spelling).
# PASS when both runs end with rc 0, their state hashes are identical, and the sealed run's exit line says
# "0 game files from the folder". Without user namespaces the sealed run is not possible: SKIP (exit 0).
# The game folder is only read (stat and listing for the skeleton). Run it under home_mask.sh (docs/testing.md).
set -u
GAME=${1:?game binary} SET=${2:?base set folder} OUT=${3:?out dir}
: "${NFSU2_ROOT:?NFSU2_ROOT (the game folder)}"
[ -f "$SET/import.toml" ] || { echo "SEALED: FAIL (no finished base set at $SET)"; exit 1; }
mkdir -p "$OUT"; OUT=$(cd "$OUT" && pwd); GAME=$(cd "$(dirname "$GAME")" && pwd)/$(basename "$GAME")
command -v unshare > /dev/null && command -v setpriv > /dev/null && unshare -rm true 2> /dev/null \
  || { echo "SEALED: SKIP (no user + mount namespaces here: unshare -rm / setpriv)"; exit 0; }

# The skeleton: folders and sparse, unreadable files of the same size and time.
SK=$OUT/skeleton
if [ ! -d "$SK" ]; then
  mkdir -p "$SK.tmp"
  (cd "$NFSU2_ROOT" && find . -type d -print0) | (cd "$SK.tmp" && xargs -0 mkdir -p)
  (cd "$NFSU2_ROOT" && find . -type f -print0) | while IFS= read -r -d '' f; do
    truncate -s "$(stat -c %s "$NFSU2_ROOT/$f")" "$SK.tmp/$f" && touch -r "$NFSU2_ROOT/$f" "$SK.tmp/$f" && chmod 000 "$SK.tmp/$f"
  done
  mv "$SK.tmp" "$SK"
fi

run() { # run NAME ENV...: the playthrough, its log and state hashes in OUT
  local n=$1; shift
  rm -rf "$OUT/$n-data" "$OUT/$n-cache"
  env XDG_DATA_HOME="$OUT/$n-data" XDG_CACHE_HOME="$OUT/$n-cache" NFSU2_CACHE_DIR="$OUT/$n-cache/game" \
    NFSU2_STATE_HASH="$OUT/$n.tsv" "$@" > "$OUT/$n.log" 2>&1
}
run folder NFSU2_BASE=off timeout 1800 "$GAME" --playthrough; FRC=$?
export SEAL_SKEL=$SK SEAL_ROOT=$NFSU2_ROOT
run sealed NFSU2_BASE="$SET" unshare -rm bash -c \
  'mount --bind "$SEAL_SKEL" "$SEAL_ROOT" && exec setpriv --inh-caps=-all --bounding-set=-all -- "$@"' _ \
  timeout 1800 "$GAME" --playthrough; SRC=$?

ok=0
echo "folder run rc $FRC, sealed run rc $SRC"; [ $FRC = 0 ] && [ $SRC = 0 ] || ok=1
EXIT=$(grep -m1 '^asset layer: .* from base set' "$OUT/sealed.log")
echo "sealed: ${EXIT:-no exit line of the asset layer (the set was not mounted)}"
[[ "$EXIT" == *", 0 game files from the folder" ]] || { ok=1; grep '^asset layer: not in base set: ' "$OUT/sealed.log" | head -20; }
if [ -s "$OUT/folder.tsv" ] && cmp -s "$OUT/folder.tsv" "$OUT/sealed.tsv"; then
  echo "state hash identical over $(($(wc -l < "$OUT/sealed.tsv") - 1)) steps"
else ok=1; echo "state hash DIFFERENT or MISSING ($OUT/folder.tsv, $OUT/sealed.tsv)"; fi
[ $ok = 0 ] && echo "SEALED: PASS" || echo "SEALED: FAIL"
exit $ok
