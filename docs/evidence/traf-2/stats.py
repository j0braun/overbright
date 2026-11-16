"""Belt (mode 4) statistics: share of belt time in Avoid Cars (action 2), share of that under 3 m/s,
asked-0 share. native: trafwalk tsv(s); clone: TURN_DUMP tsv split into N equal streams."""
import sys, collections
def rows(p, native):
    for line in open(p):
        c = line.rstrip('\n').split('\t')
        if c[0] == 'wall' or c[0].startswith('#') or len(c) < 11: continue
        yield int(c[1]), int(c[5]), float(c[8]), int(c[9]), (float(c[17]) if native and len(c) > 17 else float(c[11]) if not native else float('nan'))
def summ(it):
    n = a = s = z = 0
    for _, m, sp, act, asked in it:
        if m != 4: continue
        n += 1
        if act == 2:
            a += 1; s += sp < 3; z += asked == 0
    return n, a, s, z
def show(name, n, a, s, z):
    print(f'{name}: belt samples {n}, Avoid Cars {a} ({100*a/max(n,1):.1f} %), under 3 m/s {s} ({100*s/max(a,1):.1f} % of it), asked 0 {z} ({100*z/max(a,1):.1f} %)')
mode = sys.argv[1]
if mode == 'native':
    T = [0, 0, 0, 0]
    for p in sys.argv[2:]:
        r = summ(rows(p, True)); show(p.split('/')[-2], *r); T = [x + y for x, y in zip(T, r)]
    show('native total', *T)
else:
    p, k = sys.argv[2], int(sys.argv[3])
    data = list(rows(p, False)); hi = max(d[0] for d in data) + 1
    T = [0, 0, 0, 0]
    for i in range(k):
        r = summ(d for d in data if i * hi // k <= d[0] < (i + 1) * hi // k); show(f'stream {i}', *r); T = [x + y for x, y in zip(T, r)]
    show('clone total', *T)
