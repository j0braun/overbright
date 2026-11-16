#!/usr/bin/env python3
"""pollview.py RUN [STEP_S] [SECS]: carpoll.tsv by time since the green (native.log), AI cars; with the
AI FX_SKID p3 from native-events.tsv nearest in clock."""
import csv, sys, os, re, bisect
run = sys.argv[1]; step = float(sys.argv[2]) if len(sys.argv) > 2 else 0.1; secs = float(sys.argv[3]) if len(sys.argv) > 3 else 2.5
green = int(re.search(r'green at clock (\d+)', open(os.path.join(run, 'native.log')).read()).group(1))
rows = list(csv.DictReader(open(os.path.join(run, 'carpoll.tsv')), delimiter='\t'))
ev = []
for l in open(os.path.join(run, 'native-events.tsv')):
    f = l.rstrip('\n').split('\t')
    p3 = [x.split(':')[2].split(',')[3] for x in f[2:] if x.startswith('FX_SKID') and len(set(x.split(':')[2].split(',')[:3])) == 1]
    ev.append((int(f[0]), '/'.join(p3)))
ev.sort(); ek = [e[0] for e in ev]
cars = sorted({r['car'] for r in rows if r['kind'] == '3'})
for c in cars:
    print('== car', c)
    print('   t   gear  rpm   v      gas  clutch spin fl/fr/rr/rl         xslip fl/fr/rr/rl        W90   p3(AI, nearest event)')
    nxt = -1e9
    for r in rows:
        if r['car'] != c: continue
        t = (int(r['clock']) - green) / 4000
        if t < -0.1 or t > secs or t < nxt: continue
        nxt = t + step
        x = [float(r['x_' + w]) for w in ('fl', 'fr', 'rr', 'rl')]
        i = bisect.bisect_left(ek, int(r['clock'])); i = min(i, len(ev) - 1)
        print('%5.2f %3s %5s %6s %5s %5s  %s  %s %5.2f  %s' % (t, r['gear'], r['rpm'], r['speed'][:5], r['gas'], r['clutch'],
              ' '.join('%5.1f' % float(r['spin_' + w]) for w in ('fl', 'fr', 'rr', 'rl')), ' '.join('%5.2f' % v for v in x),
              x[1] + x[2], ev[i][1] if ev else ''))
