#!/bin/bash
# wasm32.sh [repo] [layers-file]: the packages on crate-layers.txt's `wasm` line build for wasm32-unknown-unknown
# (`cargo check`). Skipped, with a line, where that target is not installed (WASM32_REQUIRED=1 makes that a failure).
# Prints "CRATE LAYERS: wasm32 check ok|skipped|failed (...)"; exit 1 on a failure.
set -u -o pipefail
REPO=${1:-$(cd "$(dirname "$0")/../.." && pwd)}
LAYERS=${2:-$REPO/scripts/checks/crate-layers.txt}
cd "$REPO" || exit 2
wasm=$(grep -E '^wasm ' "$LAYERS" | sed 's/#.*//; s/^wasm //')
[ -n "$wasm" ] || exit 0
if rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
  args=(); for p in $wasm; do args+=(-p "$p"); done
  if log=$(cargo check --locked "${args[@]}" --target wasm32-unknown-unknown 2>&1); then
    echo "CRATE LAYERS: wasm32 check ok ($wasm)"
  else
    echo "CRATE LAYERS: wasm32 check failed ($wasm):"; grep -E '^error' --after-context=5 <<< "$log" | head -20; exit 1
  fi
elif [ "${WASM32_REQUIRED:-}" = 1 ]; then
  echo "CRATE LAYERS: wasm32 check failed (rustup target wasm32-unknown-unknown not installed, WASM32_REQUIRED=1)"; exit 1
else
  echo "CRATE LAYERS: wasm32 check skipped (rustup target wasm32-unknown-unknown not installed)"
fi
