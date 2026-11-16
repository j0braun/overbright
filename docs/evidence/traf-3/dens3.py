"""Traffic placement and density: one method on both sides. native: dens3.py native RUNDIR...; clone: dens3.py clone DUMP.tsv
Per run: samples (one per sim sample), cars listed / non-parked, belt (mode 4) share with another
non-parked car within 60 m, belt share in Avoid Cars, placements (a car moving > 40 m between samples)
with removal/placement distance to the player (2D), along the player's facing, direction."""
import sys, math, collections, bisect
def runs_native(dirs):
    for r in dirs:
        pl = {}; hdr = None
        for line in open(f'{r}/native.tsv'):
            c = line.rstrip('\n').split('\t')
            if hdr is None: hdr = {k: i for i, k in enumerate(c)}; continue
            pl[int(c[hdr['clock']])] = (float(c[hdr['c0.pos.x']]), float(c[hdr['c0.pos.y']]), float(c[hdr['c0.fwd.x']]), float(c[hdr['c0.fwd.y']]), float(c[hdr['c0.speed']]))
        ks = sorted(pl); t0 = ks[0]
        frames = collections.defaultdict(list)
        for line in open(f'{r}/native-walk.tsv'):
            c = line.rstrip('\n').split('\t')
            if c[0] == 'wall' or c[0].startswith('#') or len(c) < 15: continue
            clk = int(c[1])
            if clk < t0: continue
            x, y, z = map(float, c[6].split(','))
            frames[clk].append((c[4], int(c[5]), x, y, float(c[7]), int(c[9]), c[14] == '1', float(c[8])))
        out = []
        for clk in sorted(frames):
            i = bisect.bisect_left(ks, clk)
            if i >= len(ks): continue
            out.append(((clk - t0) / 4000, pl[ks[i]], frames[clk]))
        yield r.rstrip('/').split('/')[-1], out
def runs_clone(p):
    cur = []; name = 0; last = None; frames = collections.OrderedDict()
    for line in open(p):
        c = line.rstrip('\n').split('\t')
        if len(c) < 24: continue
        n = int(c[1]); x, y, z = map(float, c[6].split(','))
        px, py = map(float, c[17].split(',')); fx, fy = map(float, c[18].split(','))
        frames.setdefault(n, [(px, py, fx, fy, float(c[19])), []])[1].append((c[4], int(c[5]), x, y, float(c[7]), int(c[9]), c[16] == '1', float(c[8])))
    run = []; prevp = None; k = 0
    for n, (p, rows) in frames.items():
        if prevp and math.hypot(p[0] - prevp[0], p[1] - prevp[1]) > 100:
            yield f'stream{k}', run; run = []; k += 1
        if not run: n0 = n
        run.append(((n - n0) / 60, p, rows)); prevp = p
    yield f'stream{k}', run
def bins(v, w, top):
    return min(int(v // w) * w, top)
def analyse(name, run):
    nb = near = avoid = 0; listed = active = 0; inact = 0
    place = []; parks = []; prev = {}; bd = collections.Counter(); sp = collections.Counter(); spd_of = {}; pg = collections.Counter()
    for t, p, rows in run:
        px, py, fx, fy, ps = p
        listed += len(rows); active += sum(m != 0 for _, m, *_ in rows)
        for car, m, x, y, h, a, act, spd in rows:
            q = prev.get(car)
            if q and math.hypot(x - q[1], y - q[2]) > 40:
                d0 = math.hypot(q[1] - q[4][0], q[2] - q[4][1]); d1 = math.hypot(x - px, y - py)
                along = (x - px) * fx + (y - py) * fy
                same = math.cos(math.radians(h)) * fx + math.sin(math.radians(h)) * fy > 0
                place.append((t, q[3], d0, m, d1, along, same))
                g = min((math.hypot(x2 - x, y2 - y) for o, m2, x2, y2, *_ in rows if o != car and m2 != 0), default=1e9)
                pg[bins(g, 30, 240)] += 1
            if q and q[3] != 0 and m == 0: parks.append(math.hypot(x - px, y - py))
            prev[car] = (t, x, y, m, p)
            spd_of[car] = spd
            if m != 4: continue
            nb += 1; inact += not act
            if a != 2: sp[bins(spd_of[car], 2, 30)] += 1
            d = min((math.hypot(x2 - x, y2 - y) for o, m2, x2, y2, *_ in rows if o != car and m2 != 0), default=1e9)
            near += d < 60; avoid += a == 2
            bd[bins(math.hypot(x - px, y - py), 100, 600)] += 1
    T = run[-1][0] - run[0][0] if run else 1
    n = len(run)
    print(f'{name}: {T:.0f} s, cars listed {listed/max(n,1):.1f}, non-parked {active/max(n,1):.1f}; belt samples {nb}: <60 m {100*near/max(nb,1):.1f} %, Avoid {100*avoid/max(nb,1):.1f} %, inactive {100*inact/max(nb,1):.1f} %')
    print(f'   belt car player distance %: {[(k, round(100*v/max(nb,1))) for k, v in sorted(bd.items())]}')
    fm = collections.Counter(q for _, q, *_ in place)
    d0 = collections.Counter(bins(x[2], 50, 400) for x in place); d1 = collections.Counter(bins(x[4], 50, 400) for x in place)
    al = collections.Counter('ahead' if x[5] > 0 else 'behind' for x in place); sm = collections.Counter('same' if x[6] else 'oncoming' for x in place)
    print(f'   placements {len(place)} ({60*len(place)/max(T,1):.1f}/min); from mode {dict(fm)}; {dict(al)}; {dict(sm)}')
    print(f'   parked {len(parks)} ({60*len(parks)/max(T,1):.1f}/min) at (50 m bins) {sorted(collections.Counter(bins(d, 50, 400) for d in parks).items())}')
    print(f'   removed at (50 m bins) {sorted(d0.items())}')
    print(f'   placed at  (50 m bins) {sorted(d1.items())}')
    print(f'   gap at placement to nearest non-parked car (30 m bins) {sorted(pg.items())}')
    print(f'   belt not-avoiding speed (2 m/s bins) {sorted(sp.items())}')
    return nb, near, avoid, inact, len(place), T, len(parks)
mode = sys.argv[1]
it = runs_native(sys.argv[2:]) if mode == 'native' else runs_clone(sys.argv[2])
tot = [0] * 7
for name, run in it:
    if len(run) < 100: continue
    r = analyse(name, run); tot = [a + b for a, b in zip(tot, r)]
nb, near, avoid, inact, npl, T, npk = tot
print(f'TOTAL belt {nb}: <60 m {100*near/max(nb,1):.1f} %, Avoid {100*avoid/max(nb,1):.1f} %, inactive {100*inact/max(nb,1):.1f} %, placements {60*npl/max(T,1):.1f}/min, parks {60*npk/max(T,1):.1f}/min')
