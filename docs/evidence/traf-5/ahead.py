"""Placements while the player is at >= 24.5865 m/s: ahead/behind along the player's facing; per human node (clone col 24 / native col 27)."""
import sys, math, collections
native = sys.argv[1] == 'native'
for p in sys.argv[2:]:
    prev = {}; A = collections.Counter(); byn = collections.defaultdict(collections.Counter); pl = {}
    if native:
        import os
        hdr = None
        for line in open(os.path.join(os.path.dirname(p), 'native.tsv')):
            c = line.rstrip('\n').split('\t')
            if hdr is None: hdr = {k: i for i, k in enumerate(c)}; continue
            pl[int(c[hdr['clock']])] = (float(c[hdr['c0.pos.x']]), float(c[hdr['c0.pos.y']]), float(c[hdr['c0.fwd.x']]), float(c[hdr['c0.fwd.y']]), float(c[hdr['c0.speed']]))
        ks = sorted(pl)
    import bisect
    for line in open(p):
        c = line.rstrip('\n').split('\t')
        if c[0] == 'wall' or c[0].startswith('#') or len(c) < 15: continue
        x, y, z = map(float, c[6].split(',')); car = c[4]; m = int(c[5])
        if native:
            clk = int(c[1]); i = bisect.bisect_left(ks, clk)
            if i >= len(ks): continue
            px, py, fx, fy, sp = pl[ks[i]]; node = c[27] if len(c) > 27 else '-'
        else:
            px, py = map(float, c[17].split(',')); fx, fy = map(float, c[18].split(',')); sp = float(c[19]); node = c[24] if len(c) > 24 else '-'
        q = prev.get(car)
        if q and math.hypot(x - q[0], y - q[1]) > 40 and m != 0 and q[2] >= 24.5865:
            along = (x - px) * fx + (y - py) * fy
            k = 'ahead' if along > 0 else 'behind'
            A[k] += 1; byn[q[3]][k] += 1
        prev[car] = (x, y, sp, node)
    tot = sum(A.values())
    print(p, dict(A), f'ahead {100*A["ahead"]/max(tot,1):.0f} %')
    for n, v in sorted(byn.items(), key=lambda kv: -sum(kv[1].values()))[:15]: print('   node', n, dict(v))
