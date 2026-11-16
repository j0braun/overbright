import sys, math, collections
native = sys.argv[1] == 'native'
for p in sys.argv[2:]:
    st = {}; C = collections.Counter(); placed = {}; belt = young = 0.0
    for line in open(p):
        c = line.rstrip('\n').split('\t')
        if c[0] == 'wall' or c[0].startswith('#') or len(c) < 15: continue
        x, y, z = map(float, c[6].split(','))
        t = int(c[1]) / 4000 if native else int(c[1]) / 60
        m = int(c[5]); car = c[4]
        s = st.get(car)
        if s:
            jump = math.hypot(x - s[2], y - s[3]) > 40 or t < s[0]
            if jump: placed[car] = t
            elif (s[1], m) in ((2, 3), (3, 4), (2, 4), (4, 2)): C[(s[1], m)] += 1
            if m == 4 and not jump and t - s[0] < 1:
                belt += t - s[0]
                if car in placed and t - placed[car] < 10: young += t - s[0]
        st[car] = (t, m, x, y)
    print(p, dict(C), f'young belt {100*young/max(belt,1e-9):.0f} %')
