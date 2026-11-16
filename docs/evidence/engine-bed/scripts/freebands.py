#!/usr/bin/env python3
"""freebands.py NATIVE_DIR CLONE_DIR [--ranges a-b,c-d] [--win 0.5] [--clonewav clone.wav]
Native: native.wav placed by native.sync.tsv's sim clock, t = 0 at drive.tsv's first row's clock minus its t.
Clone: clone.wav (t = sample / rate, the event log's time) or clone.f32 (48 kHz). Total + band dB per window, and
per range the mean power's dB, original | clone (diff)."""
import os, sys, wave, math, bisect, argparse
import numpy as np
ap = argparse.ArgumentParser(); ap.add_argument('nat'); ap.add_argument('cl')
ap.add_argument('--ranges', default='1-5'); ap.add_argument('--win', type=float, default=0.5)
ap.add_argument('--clonewav', default='clone.wav'); ap.add_argument('--series', action='store_true')
ap.add_argument('--cshift', type=float, default=0.0, help='clone time = native time + cshift')
a = ap.parse_args()
BANDS = [(0, 125), (125, 250), (250, 500), (500, 1000), (1000, 2000), (2000, 4000), (4000, 8000)]
def bandpow(x, rate):
    m = x.mean(axis=1)
    w = np.hanning(len(m)); X = np.fft.rfft(m * w); f = np.fft.rfftfreq(len(m), 1 / rate)
    p = np.abs(X) ** 2 / (w ** 2).sum() / len(m) * 2
    return float((x ** 2).mean()), [float(p[(f >= lo) & (f < hi)].sum()) for lo, hi in BANDS]
def db(x): return 10 * math.log10(x) if x > 1e-20 else -200.0
def readwav(p):
    w = wave.open(p); r = w.getframerate()
    return np.frombuffer(w.readframes(w.getnframes()), dtype='<i2').astype(np.float64).reshape(-1, 2) / 32768.0, r
na, nrate = readwav(os.path.join(a.nat, 'native.wav'))
sync = [l.rstrip('\n').split('\t') for l in open(os.path.join(a.nat, 'native.sync.tsv')) if l[0].isdigit()]
pts = [(int(r[3]), float(r[1])) for r in sync if len(r) > 3 and r[3].strip()]
start = 0
for k in range(1, len(pts)):
    if pts[k][0] < pts[k - 1][0]: start = k
xs, ys = [], []
for c, fr in pts[start:]:
    if not xs or c > xs[-1]: xs.append(c); ys.append(fr)
d = open(os.path.join(a.nat, 'drive.tsv')).readlines()[1].split('\t')
c0 = int(d[0]) - float(d[1]) * 4000
def nat_frame(t):
    c = c0 + t * 4000; i = min(max(bisect.bisect_left(xs, c), 1), len(xs) - 1)
    return ys[i - 1] + (c - xs[i - 1]) * (ys[i] - ys[i - 1]) / ((xs[i] - xs[i - 1]) or 1)
cp = os.path.join(a.cl, a.clonewav)
if cp.endswith('.f32'): ca, crate = np.fromfile(cp, dtype='<f4').astype(np.float64).reshape(-1, 2), 48000
else: ca, crate = readwav(cp)
def seg(t0, t1):
    i0, i1 = int(nat_frame(t0)), int(nat_frame(t1))
    j0, j1 = int((t0 + a.cshift) * crate), int((t1 + a.cshift) * crate)
    return bandpow(na[i0:i1], nrate), bandpow(ca[j0:j1], crate)
head = 'range        total o | c (d)        ' + ' '.join('%9s' % ('%d-%d' % b) for b in BANDS)
print(head)
for rg in a.ranges.split(','):
    r0, r1 = (float(x) for x in rg.split('-'))
    acc = [[0, [0] * 7], [0, [0] * 7]]; n = 0; t = r0
    while t + a.win <= r1 + 1e-9:
        (to, bo), (tc, bc) = seg(t, t + a.win)
        if a.series:
            print('  %5.1f  %6.1f | %6.1f (%+5.1f)  ' % (t, db(to), db(tc), db(tc) - db(to)) + ' '.join('%+9.1f' % (db(y) - db(x)) for x, y in zip(bo, bc)))
        acc[0][0] += to; acc[1][0] += tc
        for k in range(7): acc[0][1][k] += bo[k]; acc[1][1][k] += bc[k]
        n += 1; t += a.win
    (to, bo), (tc, bc) = acc
    print('%-11s %6.1f | %6.1f (%+5.1f)  ' % (rg, db(to / n), db(tc / n), db(tc) - db(to)) + ' '.join('%9s' % ('%.1f|%.1f' % (db(x / n), db(y / n))) for x, y in zip(bo, bc)))
    print('%-11s %22s  ' % ('', 'diff') + ' '.join('%+9.1f' % (db(y) - db(x)) for x, y in zip(bo, bc)))
