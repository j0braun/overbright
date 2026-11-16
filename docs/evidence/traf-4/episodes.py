import sys, math, collections
native = sys.argv[1] == 'native'
E = collections.Counter(); D = collections.defaultdict(list); n = 0
for p in sys.argv[2:]:
    st = {}
    for line in open(p):
        c = line.rstrip('\n').split('\t')
        if c[0] == 'wall' or c[0].startswith('#') or len(c) < 15: continue
        x, y, z = map(float, c[6].split(','))
        t = int(c[1]) / 4000 if native else int(c[1]) / 60
        m = int(c[5]); act = (c[14] if native else c[16]) == '1'; car = c[4]
        s = st.get(car)
        if s is None: st[car] = [t, m, x, y, None]; continue
        jump = math.hypot(x - s[2], y - s[3]) > 40 or t < s[0]
        ep = s[4]
        if ep is not None and (jump or act):
            key = '>'.join(ep[0]); E[(key, 'placed' if jump else 'activated')] += 1; D[key].append(t - ep[1]); s[4] = None; ep = None
        elif ep is None and s[1] == 0 and m != 0 and not act and not jump:
            s[4] = ep = [['0', str(m)], t]
        elif ep is not None and str(m) != ep[0][-1]:
            ep[0].append(str(m))
        s[0], s[1], s[2], s[3] = t, m, x, y
for k, v in E.most_common(12): print(k, v, f'median {sorted(D[k[0]])[len(D[k[0]])//2]:.1f} s')
