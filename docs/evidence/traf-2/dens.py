import sys, math, collections
by = collections.defaultdict(list)
for line in open(sys.argv[1]):
    c = line.rstrip('\n').split('\t')
    if c[0] == 'wall' or c[0].startswith('#'): continue
    try: x, y, z = map(float, c[6].split(','))
    except: continue
    m = int(c[5])
    if m == 0: continue   # parked / paused (inactive) in the original: not road objects
    by[c[1]].append((c[4], m, x, y, z, int(c[9])))
cnt = collections.Counter(); n = 0
for clk, rows in by.items():
    for (car, m, x, y, z, a) in rows:
        if m != 4: continue
        n += 1
        d = min((math.hypot(x2-x, y2-y) for (o, m2, x2, y2, z2, a2) in rows if o != car), default=1e9)
        b = '<30' if d < 30 else '<60' if d < 60 else '<125' if d < 125 else '>=125'
        cnt[b] += 1; cnt[(b, a == 2)] += 1
for b in ['<30', '<60', '<125', '>=125']:
    print(f'nearest active traffic car {b:>6}: {100*cnt[b]/n:5.1f} % of belt samples, avoiding in {100*cnt[(b, True)]/max(cnt[b],1):.1f} %')
