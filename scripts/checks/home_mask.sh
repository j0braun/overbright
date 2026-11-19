#!/bin/bash
# home_mask.sh: run a command with the player's own folders hidden, and prove afterwards that it wrote nothing there.
#
#   home_mask.sh run MASKDIR CMD...   runs CMD in a user + mount namespace (unshare -rm, no root needed) in which each
#                                     of the player's folders that exists is replaced by an empty folder under MASKDIR
#                                     (a bind mount, visible to CMD and its children only: the real folders are never
#                                     opened). CMD gets the same HOME and environment as without the mask.
#   home_mask.sh verify MASKDIR       after the run: prints one line per folder and exits 1 when anything was written:
#                                     an entry in a mask folder, a changed folder time (an entry made and removed
#                                     again), or a folder that was missing before and exists now.
#
# The player's folders, as crates/ob-core/src/platform/dirs.rs resolves them on Linux without NFSU2_DATA_DIR:
#   <data home>/overbright, <data home>/NFS Underground 2 (the profiles), <cache home>/overbright,
#   <cache home>/overbright-dev; <data home> = $XDG_DATA_HOME (set, not empty) else ~/.local/share, <cache home> the
#   same with XDG_CACHE_HOME and ~/.cache.
# Without user namespaces (some containers, kernels with them switched off) `run` starts CMD unmasked and `verify`
# prints "home mask: unavailable" (exit 0): the folders are then only checked for appearing.
set -u
mode=${1:-}; M=${2:-}
[ -n "$M" ] && { [ "$mode" = run ] || [ "$mode" = verify ]; } || { sed -n '2,17p' "$0"; exit 2; }
shift 2

if [ "$mode" = run ]; then
  [ $# -gt 0 ] || { echo "home_mask.sh: run needs a command" >&2; exit 2; }
  mkdir -p "$M"; M=$(cd "$M" && pwd)
  data=${XDG_DATA_HOME:-}; [ -n "$data" ] || data=$HOME/.local/share
  cache=${XDG_CACHE_HOME:-}; [ -n "$cache" ] || cache=$HOME/.cache
  : > "$M/folders.tsv"
  n=0
  for f in "$data/overbright" "$data/NFS Underground 2" "$cache/overbright" "$cache/overbright-dev"; do
    n=$((n + 1))
    if [ -d "$f" ]; then
      mkdir -p "$M/$n"
      printf '%s\t%s\tmasked\t%s\n' "$n" "$f" "$(stat -c %.9Y "$M/$n")" >> "$M/folders.tsv"
    else
      printf '%s\t%s\tabsent\t-\n' "$n" "$f" >> "$M/folders.tsv"
    fi
  done
  if ! unshare -rm --propagation private true 2>/dev/null; then
    echo unavailable > "$M/status"
    exec "$@"
  fi
  echo masked > "$M/status"
  # Inside: bind each mask folder over its real folder, check that every one took, then become CMD (exec: a timeout's
  # signal reaches CMD itself).
  exec unshare -rm --propagation private bash -c '
    M=$1; shift
    while IFS=$(printf "\t") read -r n f how _; do
      [ "$how" = masked ] || continue
      mount --bind "$M/$n" "$f" || { echo "home mask: could not mask $f" >&2; exit 97; }
      [ "$(stat -c %d:%i "$f")" = "$(stat -c %d:%i "$M/$n")" ] || { echo "home mask: $f is not masked" >&2; exit 97; }
    done < "$M/folders.tsv"
    exec "$@"' home_mask "$M" "$@"
fi

# verify
M=$(cd "$M" 2>/dev/null && pwd) || { echo "home mask: no mask folder $2" >&2; exit 2; }
[ -f "$M/folders.tsv" ] || { echo "home mask: $M/folders.tsv missing (was the command run through home_mask.sh run?)" >&2; exit 2; }
status=$(cat "$M/status" 2>/dev/null || echo unknown)
bad=0
while IFS=$'\t' read -r n f how t0; do
  if [ "$how" = masked ]; then
    [ "$status" = masked ] || continue
    entries=$(cd "$M/$n" && find . -mindepth 1 -maxdepth 3 | sed 's|^\./||' | head -10 | tr '\n' ' ')
    t1=$(stat -c %.9Y "$M/$n")
    if [ -n "$entries" ]; then echo "home mask: WRITTEN $f: $entries"; bad=1
    elif [ "$t1" != "$t0" ]; then echo "home mask: WRITTEN $f: an entry was made and removed again"; bad=1
    else echo "home mask: untouched $f"; fi
  else
    if [ -e "$f" ]; then echo "home mask: WRITTEN $f: missing before the run, exists now"; bad=1
    else echo "home mask: untouched $f (absent)"; fi
  fi
done < "$M/folders.tsv"
[ "$status" = masked ] || echo "home mask: unavailable (no user namespaces): the existing folders were not masked"
exit $bad
