#!/usr/bin/env python3
"""Per-config median ms/frame of CPU parts, base vs new arms. usage: parts.py BENCHDIR [part substrings...]"""
import sys, json, glob, os, statistics, collections
d = sys.argv[1]; want = sys.argv[2:]
vals = collections.defaultdict(list)
for f in glob.glob(os.path.join(d, 'runs', '*.json')):
    parts = os.path.basename(f)[:-5].rsplit('__', 2)
    if len(parts) != 3:
        print(f'skip {f}: name is not label__config__rN', file=sys.stderr); continue
    label, cfg, rep = parts
    try: j = json.load(open(f))
    except Exception as e:
        print(f'skip {f}: {e}', file=sys.stderr); continue
    for p in j.get('cpu_parts', []):
        if not want or any(w in p['name'] for w in want):
            vals[(cfg, p['name'], label)].append(p['ms_per_frame'])
    if j.get('draw_cpu_ms_per_frame') is not None:  # a missing metric is no value, not a zero
        vals[(cfg, 'DRAW CPU (mean)', label)].append(j['draw_cpu_ms_per_frame'])
keys = sorted({(c, n) for (c, n, l) in vals})
for c, n in keys:
    b, nw = vals.get((c, n, 'base'), []), vals.get((c, n, 'new'), [])
    if not b or not nw: continue
    mb, mn = statistics.median(b), statistics.median(nw)
    print(f'{c:28s} {n.strip():45s} base {mb:7.4f}  new {mn:7.4f}  {100*(mn-mb)/mb if mb else 0:+6.1f} %  (n {len(b)}/{len(nw)})')
