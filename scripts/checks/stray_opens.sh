#!/bin/bash
# stray_opens.sh [repo]: the stray-open lint (stray_opens.py, scripts/checks/stray-opens.txt): no new direct std::fs
# open of a path in production code outside the list; the game's files go through nfsu2::game_fs.
exec python3 -I "$(dirname "$0")/stray_opens.py" "$@"
