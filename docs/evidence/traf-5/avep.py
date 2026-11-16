"""Avoid Cars episodes per mover, both sides. Per mode: samples, avoid share, new episodes
per minute of that mode's time, median episode length (s), tracked object's kind at the start
(traffic in mode m / player / racer), and episodes carried in from another mode."""
import sys, math, collections, statistics
native = sys.argv[1] == 'native'
for p in sys.argv[2:]:
    st = {}; T = collections.Counter(); S = collections.Counter(); AV = collections.Counter()
    starts = collections.Counter(); kinds = collections.Counter(); lens = collections.defaultdict(list); carried = collections.Counter()
    ep = {}
    frame_modes = {}; rows = []
    for line in open(p):
        c = line.rstrip('\n').split('\t')
        if c[0] == 'wall' or c[0].startswith('#') or len(c) < 15: continue
        rows.append(c)
    # mode of every car by sample clock
    for c in rows:
        frame_modes[(c[1], c[4])] = int(c[5])
    for c in rows:
        x, y, z = map(float, c[6].split(','))
        t = int(c[1]) / 4000 if native else int(c[1]) / 60
        m = int(c[5]); car = c[4]; a = int(c[9])
        s = st.get(car)
        jump = (not s) or math.hypot(x - s[2], y - s[3]) > 40 or t < s[0]
        if s and not jump and t - s[0] < 1:
            T[s[1]] += t - s[0]
        S[m] += 1; AV[m] += a == 2
        if a == 2 and (jump or s[4] != 2):
            starts[m] += 1
            if native:
                tc = c[18]; tm = frame_modes.get((c[1], tc))
                k = f'traffic m{tm}' if tm is not None else ('none' if tc == '0x0' else 'player/racer')
            else:
                tg = int(c[10]); tm = frame_modes.get((c[1], str(tg))) if tg >= 0 else None
                k = f'traffic m{tm}' if tg >= 0 else ('player' if tg == -1 else 'racer')
            kinds[(m, k)] += 1
            ep[car] = (t, m)
        if a == 2 and s and s[4] == 2 and not jump and s[1] != m:
            carried[(s[1], m)] += 1
        if a != 2 and car in ep:
            t0, m0 = ep.pop(car); lens[m0].append(t - t0)
        st[car] = (t, m, x, y, a)
    print(p)
    for m in sorted(S):
        if m == 0: continue
        mins = T[m] / 60
        print(f'  mode {m}: {T[m]:.0f} s, Avoid {100*AV[m]/max(S[m],1):.1f} %, new episodes {starts[m]} ({starts[m]/max(mins,1e-9):.1f}/min), median len {statistics.median(lens[m]) if lens[m] else 0:.1f} s (n {len(lens[m])})')
    print('  tracked at start:', dict(sorted(kinds.items())))
    print('  avoid carried across mover change:', dict(carried))
