import sys, collections
rows = [l.rstrip('\n').split('\t') for l in open(sys.argv[1]) if not l.startswith(('#', 'wall'))]
by = collections.defaultdict(list)
for r in rows:
    if len(r) < 13: continue
    by[r[4]].append(r)
changes = []; ends = []; gaps = []
for car, rs in by.items():
    prev = None; last_change = None
    for r in rs:
        walk = r[12].split(' ')
        key = (walk[-1].split(':')[:2], len(walk))
        clk = int(r[1])
        try: ls = float(walk[-1].split(':')[2])
        except: continue
        if prev is not None and walk[-1] != prev:
            if last_change is not None: gaps.append((clk - last_change) / 4000)
            last_change = clk
        prev = walk[-1]
        ends.append(ls)
gaps.sort(); ends.sort()
q = lambda v, p: v[int(p * (len(v) - 1))] if v else None
print('walk end changes: n', len(gaps), 'interval p10/p50/p90', q(gaps, .1), q(gaps, .5), q(gaps, .9))
print('last point s p10/p50/p90', q(ends, .1), q(ends, .5), q(ends, .9))
