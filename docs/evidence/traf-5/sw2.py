"""Per run: 2->3 switches with the action before/at the switch; new Avoid (type 2) episodes by mode."""
import sys, math, collections
native = sys.argv[1] == 'native'
def rows(p):
    for line in open(p):
        c = line.rstrip('\n').split('\t')
        if c[0] == 'wall' or c[0].startswith('#') or len(c) < 15: continue
        yield c
for p in sys.argv[2:]:
    st = {}; C = collections.Counter(); A = collections.Counter(); runs = 1
    for c in rows(p):
        x, y, z = map(float, c[6].split(','))
        t = int(c[1]) / 4000 if native else int(c[1]) / 60
        m = int(c[5]); car = c[4]; a = int(c[9])
        s = st.get(car)
        if s:
            if t < s[0] - 5: runs += 1
            jump = math.hypot(x - s[2], y - s[3]) > 40 or t < s[0]
            if not jump:
                if (s[1], m) == (2, 3): C[('2>3 act', s[4], a)] += 1
                if s[4] != 2 and a == 2: A[('newavoid in mode', s[1], m)] += 1
        st[car] = (t, m, x, y, a)
    print(p); [print('  ', k, v) for k, v in sorted(C.items(), key=lambda kv: -kv[1])]; [print('  ', k, v) for k, v in sorted(A.items(), key=lambda kv: -kv[1])]
