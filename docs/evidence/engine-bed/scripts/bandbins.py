#!/usr/bin/env python3
"""bandbins.py NATIVE_RT_OUT CLONE_OUT [--t0 A --t1 B] [--win 0.1]
Both sides binned by the SAME quantity: the player's CAR event p1 (abk block +0x08 = HM +0xF8, the audio rpm)
and p3 (+0x10 = throttle*10.24). Native: native.wav placed by native.sync.tsv's sim clock (t=0 = green, drive.tsv),
params from native-events.tsv (sim clock). Clone: clone.f32 (48 kHz, t = frame/60), params from clone-events.tsv.
Prints per (rpm bin, throttle class) total and band dB, original | clone, and the GIN bytes."""
import os, sys, wave, math, bisect, argparse
import numpy as np
ap = argparse.ArgumentParser(); ap.add_argument('nat'); ap.add_argument('cl')
ap.add_argument('--t0', type=float, default=0); ap.add_argument('--t1', type=float, default=20)
ap.add_argument('--win', type=float, default=0.1); ap.add_argument('--rpmbin', type=int, default=500)
ap.add_argument('--nogreen', action='store_true', help='native t=0 at first sync clock (free roam)')
ap.add_argument('--series', action='store_true')
a = ap.parse_args()
BANDS = [(0, 125), (125, 250), (250, 500), (500, 1000), (1000, 2000), (2000, 4000), (4000, 8000)]
def bandpow(x, rate):
    m = x.mean(axis=1) if x.ndim == 2 else x
    tot = float((x ** 2).mean())
    if len(m) < 64: return tot, [0.0] * len(BANDS)
    w = np.hanning(len(m)); X = np.fft.rfft(m * w); f = np.fft.rfftfreq(len(m), 1 / rate)
    p = np.abs(X) ** 2; p = p / (w ** 2).sum() / len(m) * 2   # Parseval-ish: sums to mean square of mono
    return tot, [float(p[(f >= lo) & (f < hi)].sum()) for lo, hi in BANDS]
def db(x): return 10 * math.log10(x) if x > 1e-20 else -200.0
# ---- native
nat = a.nat
w = wave.open(os.path.join(nat, 'native.wav')); nrate = w.getframerate()
na = np.frombuffer(w.readframes(w.getnframes()), dtype='<i2').astype(np.float64).reshape(-1, 2) / 32768.0
sync = [l.rstrip('\n').split('\t') for l in open(os.path.join(nat, 'native.sync.tsv')) if l[0].isdigit()]
pts = [(int(r[3]), float(r[1])) for r in sync if len(r) > 3 and r[3].strip()]
start = 0
for k in range(1, len(pts)):
    if pts[k][0] < pts[k - 1][0]: start = k
xs, ys = [], []
for c, fr in pts[start:]:
    if not xs or c > xs[-1]: xs.append(c); ys.append(fr)
if a.nogreen or not os.path.exists(os.path.join(nat, 'drive.tsv')): green = xs[0]
else: green = int(open(os.path.join(nat, 'drive.tsv')).readlines()[1].split('\t')[0])
def nat_frame(t):
    c = green + t * 4000; i = min(max(bisect.bisect_left(xs, c), 1), len(xs) - 1)
    return ys[i - 1] + (c - xs[i - 1]) * (ys[i] - ys[i - 1]) / ((xs[i] - xs[i - 1]) or 1)
# native params: player CAR p12 == 1
nt, npar = [], []
for l in open(os.path.join(nat, 'native-events.tsv')):
    f = l.rstrip('\n').split('\t')
    if not f[0].isdigit(): continue
    for e in f[2:]:
        if e.startswith('CAR:'):
            p = [int(v) for v in e.split(':')[2].split(',')]
            if len(p) > 12 and p[12] == 1: nt.append((int(f[0]) - green) / 4000); npar.append(p); break
ngt, ngin = [], []
lp = os.path.join(nat, 'native-layers.tsv')
if os.path.exists(lp):
    for l in open(lp).read().splitlines()[1:]:
        f = l.split('\t')
        try: ngt.append((int(f[0]) - green) / 4000); ngin.append((int(f[1]), int(f[2])))
        except Exception: pass
# ---- clone
cl = a.cl
ca = np.fromfile(os.path.join(cl, 'clone.f32'), dtype='<f4').astype(np.float64).reshape(-1, 2)
ct, cpar, cgt, cgin = [], [], [], []
player_id = None
for l in open(os.path.join(cl, 'clone-events.tsv')):
    f = l.rstrip('\n').split('\t')
    if len(f) < 5: continue
    if f[1] == 's' and f[2] == 'CAR':
        p = [int(v) for v in f[4].split()]
        if len(p) > 12 and p[12] == 1: ct.append(float(f[0])); cpar.append(p)
    elif f[1] == 's' and f[2] == 'GIN':
        p = [int(v) for v in f[4].split()]; cgt.append(float(f[0])); cgin.append((p[0], p[1]))
def at(ts, vs, t, d=None):
    if not ts: return d
    i = bisect.bisect_right(ts, t) - 1
    return vs[i] if i >= 0 else d
bins = {}; series = []
t = a.t0
while t + a.win <= a.t1:
    i0, i1 = int(nat_frame(t)), int(nat_frame(t + a.win))
    if i1 <= i0 + 64 or i1 > len(na): t += a.win; continue
    ntot, nb = bandpow(na[i0:i1], nrate)
    c0, c1 = int(t * 48000), int((t + a.win) * 48000)
    if c1 > len(ca): break
    ctot, cb = bandpow(ca[c0:c1], 48000)
    pn = at(nt, npar, t + a.win / 2); pc = at(ct, cpar, t + a.win / 2)
    gn = at(ngt, ngin, t + a.win / 2, (-1, -1)); gc = at(cgt, cgin, t + a.win / 2, (-1, -1))
    if pn is None or pc is None: t += a.win; continue
    def cls(p): return 'full' if p[3] >= 900 else ('off' if p[3] < 100 else 'part')
    series.append((t, pn[1], pc[1], pn[3], pc[3], gn, gc, db(ntot), db(ctot)))
    for side, p, tot, b, g in ((0, pn, ntot, nb, gn), (1, pc, ctot, cb, gc)):
        k = (cls(p), int(p[1] // a.rpmbin) * a.rpmbin)
        e = bins.setdefault(k, [[[], [], []], [[], [], []]])
        e[side][0].append(tot); e[side][1].append(b); e[side][2].append(g)
    t += a.win
if a.series:
    print('t rpm_n rpm_c thr_n thr_c gin_n gin_c dB_n dB_c')
    for r in series: print('%.1f %5d %5d %4d %4d %s %s %6.1f %6.1f' % r)
print('class  rpm   n_o/n_c  total o|c (d)   ' + '  '.join('%d-%d' % b for b in BANDS) + '   GIN o|c')
for k in sorted(bins):
    (to, bo, go), (tc, bc, gc) = bins[k]
    if len(to) < 3 or len(tc) < 3: continue
    mo, mc = np.mean(to), np.mean(tc)
    bo, bc = np.mean(bo, axis=0), np.mean(bc, axis=0)
    gs = '%.0f/%.0f|%.0f/%.0f' % (np.mean([g[0] for g in go]), np.mean([g[1] for g in go]), np.mean([g[0] for g in gc]), np.mean([g[1] for g in gc]))
    print('%-4s %5d  %3d/%3d  %6.1f|%6.1f (%+5.1f)  ' % (k[0], k[1], len(to), len(tc), db(mo), db(mc), db(mc) - db(mo)) +
          '  '.join('%+5.1f' % (db(y) - db(x)) for x, y in zip(bo, bc)) + '   ' + gs)
# overall band table
allo = [b for k in bins for b in bins[k][0][1]]; allc = [b for k in bins for b in bins[k][1][1]]
