#!/bin/bash
# cost.sh OUT REPS: base vs after interleaved, 4K, --bench explore 600 on orbit tracks R (casino), N (Beacon Hill)
# and X (no water); modes w3 (Original + Next-Gen water) and player. GPU busy medians and the water passes.
W=${WORK:?the work dir}; out=$1; reps=${2:-2}; mkdir -p $out
O=${REPO:?the source tree}/docs/evidence/water/scripts/orbit.py
python3 $O -326 -547 21 > $out/track_R.txt; python3 $O -827 714 36 > $out/track_N.txt
: "${NFSU2_ROOT:?the game folder}" "${NFSU2_CACHE_DIR:?a cache folder of your own}"; export NFSU2_HEADLESS_ONLY=1
for t in R N X; do for r in $(seq $reps); do for m in w3 player; do for g in base after; do
  case $m in w3) e="$(cat $W/orig.enh);enh.water 3" ;; player) e="$(cat $W/player.enh)" ;; esac
  tr=(); [ $t != X ] && tr=(NFSU2_BENCH_CAMTRACK="$out/track_$t.txt")
  f=$out/${t}_${m}_${g}_$r
  [ -s $f.json ] || env "${tr[@]}" XDG_DATA_HOME="$W/data" NFSU2_BENCH_SIZE=3840x2160 NFSU2_BENCH_JSON="$f.json" NFSU2_BENCH_ENH="$e" timeout 1800 $W/game-$g --bench explore 600 > $f.log 2>&1
done; done; done; done
python3 - "$out" <<'PY'
import json, glob, statistics as st, sys, os
out = sys.argv[1]; rows = {}; parts = {}
for f in sorted(glob.glob(f"{out}/*_*_*_*.json")):
    t, m, g, r = os.path.basename(f)[:-5].split('_')
    d = json.load(open(f)); gb = d.get('gpu_busy_ms') or [0]
    w = sum(p['ms_per_frame'] for p in d.get('gpu_parts', []) if 'water' in p['name'])
    for p in d.get('gpu_parts', []):
        if 'water' in p['name']: parts.setdefault((t, m, g, p['name']), []).append(p['ms_per_frame'])
    rows.setdefault((t, m, g), []).append((st.median(gb), w))
for (t, m, g), v in sorted(rows.items()):
    print(f"{t} {m:5s} {g:5s}: gpu busy median {st.median(x[0] for x in v):.3f} ms; water passes {st.median(x[1] for x in v):.3f} ms/frame (runs {len(v)})")
for k, v in sorted(parts.items()):
    print("  ", *k, f"{st.median(v):.3f}")
PY
