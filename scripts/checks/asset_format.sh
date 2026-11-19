#!/bin/bash
# asset_format.sh [repo]: the asset format specification (docs/asset-format/) against the code
# (docs/asset-format/versioning.md §6):
#   1. the generated parts (rule tables, the manifest's field tables, the schemas made from
#      crates/ob-asset-format) are in step with the code (the example asset_format_docs --check);
#   2. every schema is valid and every example validates (scripts/docs/asset_format_check.py; skipped, with a line,
#      when the Python package jsonschema is missing).
# Prints "ASSET FORMAT DOCS: ..." and "ASSET FORMAT: ..."; exit 1 on any failure.
set -u -o pipefail
REPO=${1:-$(cd "$(dirname "$0")/../.." && pwd)}
cd "$REPO" || exit 2
fail=0
cargo run -q --release --locked -p ob-asset-format --example asset_format_docs -- --check || fail=1
python3 scripts/docs/asset_format_check.py; rc=$?
[ $rc = 1 ] && fail=1
exit $fail
