#!/bin/bash
# Evidence register check (docs/developing/evidence.md): every claim row in the coverage maps and the
# deviation table carries a tag. Quick (< 1 s), docs only. Exit 0 = pass.
#   evidence.sh [REPO]
HERE=$(cd "$(dirname "$0")" && pwd)
exec python3 "$HERE/evidence.py" "${1:-$(cd "$HERE/../.." && pwd)}"
