#!/usr/bin/env python3
"""pairs.py DIR A B [pass-substring]: per config, per rep: GPU-busy median and the pass's ms (A, B, B-A); then medians."""
import sys, json, glob, os, statistics as st, collections
d, a, b = sys.argv[1:4]; key = sys.argv[4] if len(sys.argv) > 4 else 'temporal resolve'
R = collections.defaultdict(dict)
for f in glob.glob(os.path.join(d, 'runs', '*.json')):
    lab, cfg, rep = os.path.basename(f)[:-5].rsplit('__', 2)
    if rep == 'r0': continue
    try: j = json.load(open(f))
    except Exception: continue
    g = st.median(j.get('gpu_busy_ms') or [0])
    p = sum(x['ms_per_frame'] for x in j.get('gpu_parts', []) if key in x['name'])
    R[(cfg, rep)][lab] = (g, p)
for cfg in sorted({c for c, _ in R}):
    rows = [(rep, R[(cfg, rep)]) for c, rep in sorted(R) if c == cfg and a in R[(cfg, rep)] and b in R[(cfg, rep)]]
    dg = [r[b][0] - r[a][0] for _, r in rows]; dp = [r[b][1] - r[a][1] for _, r in rows]
    print(f'{cfg}  {a} -> {b}  ({key})')
    for rep, r in rows:
        print(f'  {rep}: frame {r[a][0]:6.2f} -> {r[b][0]:6.2f} ({r[b][0]-r[a][0]:+5.2f})   pass {r[a][1]:6.3f} -> {r[b][1]:6.3f} ({r[b][1]-r[a][1]:+6.3f})')
    if rows:
        print(f'  median: frame {st.median([r[a][0] for _,r in rows]):6.2f} -> {st.median([r[b][0] for _,r in rows]):6.2f} ({st.median(dg):+5.2f}, {min(dg):+.2f}..{max(dg):+.2f})   pass {st.median([r[a][1] for _,r in rows]):6.3f} -> {st.median([r[b][1] for _,r in rows]):6.3f} ({st.median(dp):+6.3f}, {min(dp):+.3f}..{max(dp):+.3f})')
