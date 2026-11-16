import sys, os, statistics as st
# layercmp.py NATIVE_DIR CLONE_DIR FROM TO: GIN bytes and CAR p4/p14/p25 means, both sides
nat, cl, t0, t1 = sys.argv[1], sys.argv[2], float(sys.argv[3]), float(sys.argv[4])
d = open(os.path.join(nat, 'drive.tsv')).readlines()[1].split('\t'); c0 = int(d[0]) - float(d[1]) * 4000
L = [l.split('\t') for l in open(os.path.join(nat, 'native-layers.tsv')).read().splitlines()[1:]]
rows = [r for r in L if r[0].isdigit() and t0 <= (int(r[0]) - c0) / 4000 <= t1]
g = lambda k: st.mean(float(r[k]) for r in rows)
print('native: n=%d gin108 %.1f gin10C %.1f gainE0 %.4f gainFC %.4f p4 %.0f p14 %.0f p25 %.0f' % (len(rows), g(1), g(2), g(3), g(4), g(7), g(8), g(9)))
pl = [[0.0] * 8 for _ in rows]
for i, r in enumerate(rows):
    for k in range(8):
        s, v, gn = r[10 + k].split(':'); pl[i][k] = float(gn) if s == '1' else 0.0
print('native abk player gains:', ' '.join('%.4f' % st.mean(p[k] for p in pl) for k in range(8)))
ev = [l.rstrip('\n').split('\t') for l in open(os.path.join(cl, 'events.tsv'))]
gin = [list(map(int, r[4].split())) for r in ev if len(r) > 4 and r[2] == 'GIN' and t0 <= float(r[0]) <= t1]
car = [list(map(int, r[4].split())) for r in ev if len(r) > 4 and r[1] == 's' and r[2] == 'CAR' and t0 <= float(r[0]) <= t1 and r[4].split()[12] == '1']
print('clone : gin bytes %.1f %.1f  HM+80 %.0f HM+84 %.0f level0 %.0f | p4 %.0f p14 %.0f p25 %.0f' % (st.mean(x[0] for x in gin), st.mean(x[1] for x in gin), st.mean(x[2] for x in gin), st.mean(x[3] for x in gin), st.mean(x[4] for x in gin), st.mean(x[4] for x in car), st.mean(x[14] for x in car), st.mean(x[25] for x in car)))
