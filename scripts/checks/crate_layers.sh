#!/bin/bash
# crate_layers.sh [repo]: the workspace's layer check (ARCHITECTURE.md "Layers"). Fails (exit 1) when
#   1. a library module names a module of a higher layer that is not on the allow-list
#      (crate_layers.py: scripts/checks/crate-layers.txt, crate-layers-allow.txt; the programs in src/bin/ too),
#      or an allow-list line is no longer needed, or one of Studio's crates (crate-layers.txt `studio` line) names
#      the library other than as `nfsu2::studio_api::…` (src/studio_api.rs, which re-exports no library module by glob);
#   2. a workspace package depends (normal or build edge) on a package listed above it in crate-layers.txt
#      (`cargo metadata`), so the package graph only points down;
#   3. a package's dependency tree holds a crate its `forbid` line names (`cargo tree`);
#   4. the shipped packages' dependency tree turns on `dev-harness` in any workspace crate (`cargo tree -e
#      features`: every workspace package but devprobes, default features), so the developer harness is never
#      switched on behind the shipping build's back;
#   5. a package on crate-layers.txt's `wasm` line does not build for wasm32-unknown-unknown (when that target is
#      installed; a line says when it is not).
# Prints "CRATE LAYERS: ok (...)" and one line per part when clean. Needs python3 and cargo (no build: the
# lock file is read; only the wasm32 check compiles).
set -u -o pipefail
REPO=${1:-$(cd "$(dirname "$0")/../.." && pwd)}
cd "$REPO" || exit 2
HERE=scripts/checks
fail=0
python3 "$HERE/crate_layers.py" "$REPO" || fail=1

meta=$(cargo metadata --format-version 1 --no-deps --locked 2>/dev/null) || { echo "CRATE LAYERS: cargo metadata failed"; exit 2; }
out=$(python3 -c '
import json, sys
meta = json.loads(sys.argv[1]); order = []; forbid = {}
for line in open(sys.argv[2], encoding="utf-8"):
    w = line.split("#")[0].split()
    if len(w) == 2 and w[0] == "package": order.append(w[1])
rank = {p: i for i, p in enumerate(order)}
names = {p["name"] for p in meta["packages"]}
bad = []
for p in meta["packages"]:
    me = p["name"]
    if me not in rank:
        bad.append(me + ": not listed in crate-layers.txt (package lines)"); continue
    for d in p["dependencies"]:
        dn, kind = d["name"], d.get("kind") or "normal"
        if dn not in names or kind == "dev": continue
        if rank.get(dn, -1) >= rank[me]:
            bad.append(f"{me} -> {dn} ({kind}): not below it")
for b in bad: print("  " + b)
print("PACKAGES " + " ".join(o for o in order if o in names))
' "$meta" "$HERE/crate-layers.txt") || { echo "CRATE LAYERS: package check failed"; exit 2; }
pk=$(echo "$out" | sed -n 's/^PACKAGES //p')
if echo "$out" | grep -q '^  '; then echo "CRATE LAYERS: package dependencies pointing up:"; echo "$out" | grep '^  '; fail=1
else echo "CRATE LAYERS: packages ok (bottom to top: $pk)"; fi

while read -r _ p crates; do
  case " $pk " in *" $p "*) ;; *) continue ;; esac
  tree=$(cargo tree -p "$p" -e normal,build --prefix none --locked 2>&1) || { echo "CRATE LAYERS: cargo tree -p $p failed"; fail=1; continue; }
  hit=""
  for c in $crates; do grep -q "^$c v" <<< "$tree" && hit="$hit $c"; done
  if [ -n "$hit" ]; then echo "CRATE LAYERS: $p's dependency tree has:$hit"; fail=1
  else echo "CRATE LAYERS: $p has none of: $crates"; fi
done < <(grep -E '^forbid ' "$HERE/crate-layers.txt" | sed 's/#.*//')

wasm=$(grep -E '^wasm ' "$HERE/crate-layers.txt" | sed 's/#.*//; s/^wasm //')
if [ -n "$wasm" ]; then
  if rustup target list --installed 2>/dev/null | grep -qx wasm32-unknown-unknown; then
    args=(); for p in $wasm; do args+=(-p "$p"); done
    if log=$(cargo check --locked "${args[@]}" --target wasm32-unknown-unknown 2>&1); then echo "CRATE LAYERS: wasm32 check ok ($wasm)"
    else echo "CRATE LAYERS: wasm32 check failed ($wasm):"; grep -E '^error' --after-context=5 <<< "$log" | head -20; fail=1; fi
  else
    echo "CRATE LAYERS: wasm32 check skipped (rustup target wasm32-unknown-unknown not installed)"
  fi
fi

harness=0
for p in $pk; do
  [ "$p" = devprobes ] && continue
  n=$(cargo tree --workspace --exclude devprobes -e normal,build,features -i "$p" --prefix none --locked 2>/dev/null | grep -c 'feature "dev-harness"')
  [ "$n" = 0 ] || { echo "CRATE LAYERS: the shipped packages turn on $p's dev-harness"; harness=1; fail=1; }
done
[ $harness = 0 ] && echo "CRATE LAYERS: dev-harness off in the shipped packages"
exit $fail
