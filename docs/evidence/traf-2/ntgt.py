"""Belt (mode 4) avoiders: tracked target's distance, mode, lateral; asked speed. native or clone dump."""
import sys, math, collections
native = sys.argv[2] == 'native'
rows = collections.defaultdict(dict)
for line in open(sys.argv[1]):
    c = line.rstrip('\n').split('\t')
    if c[0] == 'wall' or c[0].startswith('#'): continue
    try:
        x, y, z = map(float, c[6].split(','))
    except ValueError: continue
    if native:
        if len(c) < 21: continue
        key = c[4]; tgt = c[18]; asked = float(c[17])
    else:
        key = c[3]; tgt = c[10]; asked = float(c[11])
    rows[c[1]][key] = (int(c[5]), x, y, z, float(c[7]), float(c[8]), int(c[9]), tgt, asked)
d = collections.Counter(); m = collections.Counter(); ask = collections.Counter(); n = 0; slow = 0; nt = 0
lat = collections.Counter()
for f, cars in rows.items():
    for k, (mo, x, y, z, h, s, a, t, asked) in cars.items():
        if mo != 4 or a != 2: continue
        n += 1; slow += s < 3
        ask['0' if asked == 0 else '6' if abs(asked-6) < .01 else 'cap'] += 1
        o = cars.get(t)
        if o is None: nt += 1; continue
        dx, dy = o[1]-x, o[2]-y; dist = math.hypot(dx, dy)
        hr = math.radians(h); l = -dx*math.sin(hr)+dy*math.cos(hr)
        d[min(int(dist//15)*15, 150)] += 1; m[o[0]] += 1; lat[min(int(abs(l)), 10)] += 1
print(f'belt avoid samples {n}, under 3 m/s {slow}, target not a listed traffic car {nt}')
print('asked', dict(ask)); print('target dist (15 m bins)', sorted(d.items())); print('target mode', sorted(m.items())); print('|lateral|', sorted(lat.items()))
