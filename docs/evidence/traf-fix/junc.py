import sys, collections
rows = [l.rstrip('\n').split('\t') for l in open(sys.argv[1]) if not l.startswith(('#', 'wall'))]
seen = collections.defaultdict(lambda: [0, set(), None, None])  # j -> samples, cars, first clock, last
spans = collections.defaultdict(list)  # (car, j) -> list of clocks
for r in rows:
    if len(r) < 13: continue
    for w in r[12].split(' '):
        f = w.split(':')
        if len(f) == 5:
            j = int(f[4]); key = (r[4], j, f[0] + ':' + f[1])
            spans[key].append((int(r[1]), float(f[2]), float(f[3]), r[10], r[12].split(' ')[-1]))
out = collections.defaultdict(list)
for (car, j, pt), cl in spans.items():
    # split into episodes (gap > 1 s)
    ep = [[cl[0]]]
    for c in cl[1:]:
        if c[0] - ep[-1][-1][0] > 4000: ep.append([c])
        else: ep[-1].append(c)
    for e in ep:
        out[j].append(((e[-1][0] - e[0][0]) / 4000, pt, car, e[0][1:], e[-1][1:]))
for j in sorted(out):
    eps = out[j]
    print(j, 'episodes', len(eps), 'durations', sorted(round(x[0], 1) for x in eps)[:30], 'pt', eps[0][1])
    if len(sys.argv) > 2 and str(j) in sys.argv[2:]:
        for e in eps: print('   ', e)
