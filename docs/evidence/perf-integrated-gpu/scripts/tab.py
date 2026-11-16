#!/usr/bin/env python3
"""tab.py DIR [--passes] [--ab A B]: per config medians (and scatter = min..max over runs).
Columns: GPU busy p50, frame p50/p99, main CPU, render CPU (ms/frame). --passes: pass x ms x share per config.
--ab A B: A vs B deltas per config (GPU, frame p50, main, render) and per pass."""
import sys, json, glob, os, statistics as st, collections
d = sys.argv[1]
runs = collections.defaultdict(list)
for f in sorted(glob.glob(os.path.join(d, 'runs', '*.json'))):
    lab, cfg, rep = os.path.basename(f)[:-5].rsplit('__', 2)
    try:
        j = json.load(open(f))
    except Exception:
        continue
    th = {t['name']: t['ms_per_frame'] for t in j.get('threads', [])}
    fr = sorted(j['frame_ms'])
    g = sorted(j.get('gpu_busy_ms') or [0])
    runs[(lab, cfg)].append({
        'gpu': st.median(g), 'f50': st.median(fr), 'f99': fr[int(0.99 * (len(fr) - 1))],
        'main': th.get('main (game)', 0), 'rend': sum(v for k, v in th.items() if k.startswith('render')),
        'nrend': sum(1 for k in th if k.startswith('render')),
        'parts': {p['name']: p['ms_per_frame'] for p in j.get('gpu_parts', [])},
    })

def med(rs, k):
    v = [r[k] for r in rs]
    return st.median(v), min(v), max(v)

def fmt(t):
    return f'{t[0]:6.2f} ({t[1]:.2f}-{t[2]:.2f})'

def cfgkey(c):
    s, p, r = c.split('-')
    return (['free', 'start', 'racing', 'rain', 'lamps'].index(s) if s in ['free', 'start', 'racing', 'rain', 'lamps'] else 9, ['orig', 'bal', 'high'].index(p) if p in ['orig', 'bal', 'high'] else 9, r)

labels = sorted({l for l, _ in runs})
cfgs = sorted({c for _, c in runs}, key=cfgkey)
if '--ab' in sys.argv:
    a, b = sys.argv[sys.argv.index('--ab') + 1:][:2]
    print(f'{"config":24s} {"GPU " + a:>9s} {"GPU " + b:>9s} {"d%":>6s} | {"f50 " + a:>9s} {"f50 " + b:>9s} | main {a}/{b} | render {a}/{b}')
    for c in cfgs:
        ra, rb = runs.get((a, c)), runs.get((b, c))
        if not ra or not rb:
            continue
        ga, gb = med(ra, 'gpu')[0], med(rb, 'gpu')[0]
        print(f'{c:24s} {ga:9.3f} {gb:9.3f} {100 * (gb - ga) / ga:+6.1f} | {med(ra, "f50")[0]:9.2f} {med(rb, "f50")[0]:9.2f} | p99 {med(ra, "f99")[0]:.2f}/{med(rb, "f99")[0]:.2f} | {med(ra, "main")[0]:.2f}/{med(rb, "main")[0]:.2f} | {med(ra, "rend")[0]:.2f}/{med(rb, "rend")[0]:.2f}  n {len(ra)}/{len(rb)}  gpu scatter {fmt(med(ra, "gpu"))} / {fmt(med(rb, "gpu"))}')
        if '--passes' in sys.argv:
            names = sorted(set().union(*[r['parts'] for r in ra + rb]))
            for n in names:
                pa = st.median([r['parts'].get(n, 0) for r in ra]); pb = st.median([r['parts'].get(n, 0) for r in rb])
                if abs(pb - pa) >= 0.01:
                    print(f'      {n:40s} {pa:7.3f} -> {pb:7.3f} ({pb - pa:+.3f})')
    sys.exit()
for lab in labels:
    print(f'## {lab}')
    print(f'{"config":24s} {"GPU p50 (scatter)":>20s} {"frame p50":>20s} {"frame p99":>20s} {"main CPU":>20s} {"render CPU":>20s}')
    for c in cfgs:
        rs = runs.get((lab, c))
        if not rs:
            continue
        print(f'{c:24s} {fmt(med(rs, "gpu")):>20s} {fmt(med(rs, "f50")):>20s} {fmt(med(rs, "f99")):>20s} {fmt(med(rs, "main")):>20s} {fmt(med(rs, "rend")):>20s}  n={len(rs)} workers={rs[0]["nrend"]}')
        if '--passes' in sys.argv:
            names = set().union(*[r['parts'] for r in rs])
            m = {n: st.median([r['parts'].get(n, 0) for r in rs]) for n in names if n != '(between submissions)'}
            tot = sum(m.values())
            print('    ' + '; '.join(f'{n} {v:.2f} ({100 * v / tot:.0f}%)' for n, v in sorted(m.items(), key=lambda kv: -kv[1]) if v >= 0.05))
