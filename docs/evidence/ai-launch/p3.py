#!/usr/bin/env python3
"""p3.py RUN [MAXFRAME]: the AI racers' FX_SKID (one-event instances: p0 = p1 = p2) p3/p4/p5 by record frame."""
import csv, sys, os
run = sys.argv[1]; mx = int(sys.argv[2]) if len(sys.argv) > 2 else 200
rows = list(csv.DictReader(open(os.path.join(run, 'native.tsv')), delimiter='\t'))
clk = [int(float(r['clock'])) for r in rows]
for l in open(os.path.join(run, 'native-events.tsv')):
    f = l.rstrip('\n').split('\t'); c = int(f[0])
    if c < clk[0] - 300 or c > clk[-1]: continue
    fr = min(range(len(clk)), key=lambda i: abs(clk[i] - c))
    if fr > mx: continue
    out = []
    for x in f[2:]:
        if not x.startswith('FX_SKID'): continue
        n, inst, p = x.split(':'); p = p.split(',')
        if not (p[0] == p[1] == p[2]): continue
        out.append('%s:%s/%s/%s' % (inst[-3:], p[3], p[4], p[5]))
    print('%4d %.3f  %s' % (fr, (c - clk[0]) / 4000, '  '.join(out)))
