"""Exits from mode 3: to which mode (or 'placed' by jump) and the 2D player distance at the exit."""
import sys, math, collections
native = sys.argv[1] == 'native'
for p in sys.argv[2:]:
    st = {}; C = collections.Counter(); runs = 0
    for line in open(p):
        c = line.rstrip('\n').split('\t')
        if c[0] == 'wall' or c[0].startswith('#') or len(c) < 15: continue
        x, y, z = map(float, c[6].split(','))
        t = int(c[1]) / 4000 if native else int(c[1]) / 60
        m = int(c[5]); car = c[4]
        try:
            if native: px, py = map(float, c[26].split()[0].split(':')[1].split(','))
            else: px, py = map(float, c[17].split(','))
        except Exception: continue
        s = st.get(car)
        if s and s[1] == 3 and m != 3:
            jump = math.hypot(x - s[2], y - s[3]) > 40
            d = math.hypot(s[2] - s[4], s[3] - s[5])
            C[('placed' if jump else f'->{m}', min(int(d // 50) * 50, 500))] += 1
        st[car] = (t, m, x, y, px, py)
    tot = collections.Counter()
    for (k, d), v in C.items(): tot[k] += v
    print(p, dict(tot)); print('  ', sorted(C.items()))
