import sys, os
# cmpdrive.py NATIVE_DIR CLONE_DIR: player CAR p1 (audio rpm), p3, and speed over time on both sides
nat, cl = sys.argv[1:3]
d = [l.split('\t') for l in open(os.path.join(nat, 'drive.tsv')).read().splitlines()[1:]]
c0 = int(d[0][0]) - int(float(d[0][1]) * 4000)
nev = []
for l in open(os.path.join(nat, 'native-events.tsv')):
    f = l.rstrip('\n').split('\t')
    if not f[0].isdigit(): continue
    for e in f[2:]:
        if e.startswith('CAR:'):
            p = e.split(':')[2].split(',')
            if len(p) > 12 and p[12] == '1': nev.append(((int(f[0]) - c0) / 4000, p)); break
cev = []
for l in open(os.path.join(cl, 'events.tsv')):
    f = l.rstrip('\n').split('\t')
    if len(f) > 4 and f[1] == 's' and f[2] == 'CAR':
        p = f[4].split()
        if p[12] == '1': cev.append((float(f[0]), p))
spd = []
for l in open(os.path.join(cl, 'clone.log')):
    if l.startswith('PROPTICK'):
        c = l.split(); spd.append((int(c[1][1:]) / 60, float(c[-1]) * 3.6))
def at(xs, t):
    best = None
    for x in xs:
        if x[0] <= t: best = x
        else: break
    return best
print('t   | native: rpm p1 p3 p4 speed | clone: p1 p3 p4 speed')
for k in range(0, int(float(sys.argv[3]) if len(sys.argv) > 3 else 10) * 2):
    t = k / 2
    dn = at([(float(r[1]), r) for r in d], t); en = at(nev, t); ec = at(cev, t); sc = at(spd, t)
    print('%4.1f | %5s %5s %5s %6s %6s | %5s %5s %6s %6.1f' % (t, dn[1][3] if dn else '', en[1][1] if en else '', en[1][3] if en else '', en[1][4] if en else '', dn[1][4] if dn else '',
          ec[1][1] if ec else '', ec[1][3] if ec else '', ec[1][4] if ec else '', sc[1] if sc else 0))
