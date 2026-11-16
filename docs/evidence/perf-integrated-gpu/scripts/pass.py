#!/usr/bin/env python3
"""pass.py DIR [--top N] [--label L] [--ab A B]: per config GPU busy median (min-max over runs), frame p50/p99,
and the top N passes (median ms over runs). --ab A B: deltas per config and per pass >= 0.02 ms."""
import sys, json, glob, os, statistics as st, collections
d = sys.argv[1]
top = int(sys.argv[sys.argv.index('--top') + 1]) if '--top' in sys.argv else 6
runs = collections.defaultdict(list)
for f in sorted(glob.glob(os.path.join(d, 'runs', '*.json'))):
    lab, cfg, rep = os.path.basename(f)[:-5].rsplit('__', 2)
    try:
        j = json.load(open(f))
    except Exception:
        continue
    fr = sorted(j['frame_ms']); g = sorted(j.get('gpu_busy_ms') or [0])
    runs[(lab, cfg)].append({'gpu': st.median(g), 'g99': g[int(0.99 * (len(g) - 1))], 'f50': st.median(fr),
                             'f99': fr[int(0.99 * (len(fr) - 1))],
                             'parts': {p['name']: p['ms_per_frame'] for p in j.get('gpu_parts', [])}})
def m(rs, k): v = [r[k] for r in rs]; return st.median(v), min(v), max(v)
def parts(rs):
    names = set().union(*[r['parts'] for r in rs])
    return {n: st.median([r['parts'].get(n, 0) for r in rs]) for n in names}
labels = sorted({l for l, _ in runs}); cfgs = sorted({c for _, c in runs})
if '--ab' in sys.argv:
    a, b = sys.argv[sys.argv.index('--ab') + 1:][:2]
    for c in cfgs:
        ra, rb = runs.get((a, c)), runs.get((b, c))
        if not ra or not rb: continue
        ga, gb = m(ra, 'gpu'), m(rb, 'gpu')
        print(f'{c:28s} GPU {ga[0]:7.2f} ({ga[1]:.2f}-{ga[2]:.2f}) -> {gb[0]:7.2f} ({gb[1]:.2f}-{gb[2]:.2f}) {gb[0]-ga[0]:+6.2f} ms {100*(gb[0]-ga[0])/ga[0]:+5.1f}%  p99 {m(ra,"g99")[0]:.2f}->{m(rb,"g99")[0]:.2f} n {len(ra)}/{len(rb)}')
        pa, pb = parts(ra), parts(rb)
        for n in sorted(set(pa) | set(pb), key=lambda n: -(abs(pb.get(n, 0) - pa.get(n, 0)))):
            dd = pb.get(n, 0) - pa.get(n, 0)
            if abs(dd) >= 0.02: print(f'    {n:55s} {pa.get(n,0):7.3f} -> {pb.get(n,0):7.3f} {dd:+.3f}')
    sys.exit()
if '--loo' in sys.argv:
    ref = sys.argv[sys.argv.index('--loo') + 1]
    for l in labels:
        rr = runs.get((l, ref))
        if not rr: continue
        g0 = m(rr, 'gpu')[0]; p0 = parts(rr)
        print(f'{ref}: GPU {g0:.2f} ({m(rr,"gpu")[1]:.2f}-{m(rr,"gpu")[2]:.2f})')
        for c in cfgs:
            if c == ref or not runs.get((l, c)): continue
            rs = runs[(l, c)]; g = m(rs, 'gpu'); p1 = parts(rs)
            ds = sorted(((n, p0.get(n, 0) - p1.get(n, 0)) for n in set(p0) | set(p1)), key=lambda x: -abs(x[1]))
            print(f'  {c:30s} saves {g0-g[0]:+6.2f} ms (run {g[1]:.2f}-{g[2]:.2f})  ' + '; '.join(f'{n} {d:+.2f}' for n, d in ds[:5] if abs(d) >= 0.05))
    sys.exit()
want = sys.argv[sys.argv.index('--label') + 1] if '--label' in sys.argv else None
for l in labels:
    if want and l != want: continue
    for c in cfgs:
        rs = runs.get((l, c))
        if not rs: continue
        g = m(rs, 'gpu')
        p = parts(rs); tot = sum(p.values()) or 1
        ps = '; '.join(f'{n} {v:.2f}' for n, v in sorted(p.items(), key=lambda x: -x[1])[:top])
        print(f'{l} {c:28s} GPU {g[0]:7.2f} ({g[1]:.2f}-{g[2]:.2f}) p99 {m(rs,"g99")[0]:7.2f} frame {m(rs,"f50")[0]:.2f}/{m(rs,"f99")[0]:.2f} n={len(rs)}')
        print(f'    {ps}')
