import sys, math
# cmp.py ORIG_TRAIL CLONE_TRAIL [ROUTE]: clone rivals vs the original's rival trails (nearest point of any
# original rival line), by seconds since each car first moved; original trail format "t idx car x y z v",
# clone "t idx x y z v".
def load(p, orig):
    by = {}
    for l in open(p):
        if l.startswith('#'): continue
        f = l.split()
        if orig: t, k, x, y, v = float(f[0]), f[2], float(f[3]), float(f[4]), float(f[6])
        else: t, k, x, y, v = float(f[0]), f[1], float(f[2]), float(f[3]), float(f[5])
        by.setdefault(k, []).append((t, x, y, v))
    return by
o = load(sys.argv[1], True); c = load(sys.argv[2], False)
# original rivals: those that move
ol = [v for v in o.values() if max(s[3] for s in v) > 5]
opts = [(x, y) for v in ol for (t, x, y, s) in v if s > 0.5]
def near(p): return min(math.hypot(p[0]-a, p[1]-b) for a, b in opts)
def t0(v): return next((t for t, x, y, s in v if s > 0.5), v[0][0])
print('orig grid', sorted({(round(v[0][1],1), round(v[0][2],1)) for v in ol}))
print('clone grid', [(k, v[0][1], v[0][2]) for k, v in c.items()])
for k, v in c.items():
    s0 = t0(v); row = []
    for want in (0, 2, 4, 6, 8, 10, 12, 15, 20, 25, 30):
        pt = min(v, key=lambda r: abs(r[0] - s0 - want))
        row.append(f'{want}s:{near((pt[1], pt[2])):.0f}')
    ds = [near((x, y)) for t, x, y, s in v if s0 <= t <= s0 + 15]
    ds.sort()
    print(f'clone rival {k}: moves at {s0:.2f}; dist to orig lines', ' '.join(row), f'| first 15 s median {ds[len(ds)//2]:.1f} p90 {ds[int(len(ds)*.9)]:.1f} max {ds[-1]:.1f}')
