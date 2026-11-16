# CircleWatch on a native-walk.tsv: per car, unwrapped heading over 10 s windows; report >300 deg
import sys, collections, math
rows = [l.rstrip('\n').split('\t') for l in open(sys.argv[1]) if not l.startswith(('#', 'wall'))]
by = collections.defaultdict(list)
for r in rows:
    if len(r) < 13: continue
    p = tuple(float(x) for x in r[6].split(','))
    by[r[4]].append((int(r[1]) / 4000.0, float(r[7]), float(r[8]), p, int(r[5]), r[9]))
found = 0; flips = collections.Counter()
for car, h in by.items():
    hist = collections.deque(); last = -1e9; un = None; prev = None
    for t, hd, sp, p, mode, act in h:
        if prev and math.dist(prev[3][:2], p[:2]) > 10: hist.clear(); un = None
        d = 0 if un is None else ((hd - prev[1] + 180) % 360 - 180)
        if un is not None and abs(d) > 150: flips[mode] += 1
        un = hd if un is None else un + d
        prev = (t, hd, sp, p)
        hist.append((t, un, sp, p, mode))
        while hist and t - hist[0][0] > 10: hist.popleft()
        if t - last < 10 or len(hist) < 2: continue
        lo = min(x[1] for x in hist); hi = max(x[1] for x in hist)
        if hi - lo > 300:
            mean = sum(x[2] for x in hist) / len(hist)
            back = math.dist(hist[0][3][:2], p[:2])
            if mean < 1 and back > 25: continue
            last = t; found += 1
            print(f'{car} t {t:.1f} turned {hi-lo:.0f} mean {mean:.1f} back {back:.0f} at {p[0]:.0f},{p[1]:.0f} mode {mode} act {act}')
print('circling', found, 'single-sample flips >150 deg by mode', dict(flips))
