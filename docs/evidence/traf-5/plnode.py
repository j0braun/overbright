"""Placements at speed: human node -> placed car's first walk point (rec:cp) and ahead/behind."""
import sys, math, collections, bisect, os
native = sys.argv[1] == 'native'; p = sys.argv[2]
pl = {}
if native:
    hdr = None
    for line in open(os.path.join(os.path.dirname(p), 'native.tsv')):
        c = line.rstrip('\n').split('\t')
        if hdr is None: hdr = {k: i for i, k in enumerate(c)}; continue
        pl[int(c[hdr['clock']])] = (float(c[hdr['c0.pos.x']]), float(c[hdr['c0.pos.y']]), float(c[hdr['c0.fwd.x']]), float(c[hdr['c0.fwd.y']]), float(c[hdr['c0.speed']]))
    ks = sorted(pl)
prev = {}; n = 0
for line in open(p):
    c = line.rstrip('\n').split('\t')
    if c[0] == 'wall' or c[0].startswith('#') or len(c) < 15: continue
    x, y, z = map(float, c[6].split(',')); car = c[4]; m = int(c[5])
    if native:
        clk = int(c[1]); i = bisect.bisect_left(ks, clk)
        if i >= len(ks): continue
        px, py, fx, fy, sp = pl[ks[i]]; node = c[27] if len(c) > 27 else '-'; front = ':'.join(c[12].split(' ')[0].split(':')[:2])
    else:
        px, py = map(float, c[17].split(',')); fx, fy = map(float, c[18].split(',')); sp = float(c[19]); node = c[24] if len(c) > 24 else '-'; front = c[22]
    q = prev.get(car)
    if q and math.hypot(x - q[0], y - q[1]) > 40 and m != 0 and q[2] >= 24.5865:
        along = (x - px) * fx + (y - py) * fy
        print(f'{"A" if along > 0 else "B"} node {q[3]:>16} -> {front:>12} mode {m} along {along:7.1f} d {math.hypot(x-px,y-py):6.1f}')
        n += 1
        if n > int(sys.argv[3]) if len(sys.argv) > 3 else 60: break
    prev[car] = (x, y, sp, node)
