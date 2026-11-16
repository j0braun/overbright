#!/usr/bin/env python3
"""hitbed.py NATIVE_DIR CLONE_DIR: the barrel hit in both (native: the first speed drop over 3 km/h between
drive.tsv rows; clone: the first PROPCAR), bed = RMS of the 0.5 s before the hit, hit = the loudest 50 ms RMS in
the second after it (dBFS)."""
import os, sys, wave, math, bisect
import numpy as np
nat, cl = sys.argv[1:3]
def db(x): return 10 * math.log10(x) if x > 1e-20 else -200.0
def readwav(p):
    w = wave.open(p); r = w.getframerate()
    return np.frombuffer(w.readframes(w.getnframes()), dtype='<i2').astype(np.float64).reshape(-1, 2) / 32768.0, r
na, nr = readwav(os.path.join(nat, 'native.wav'))
sync = [l.rstrip('\n').split('\t') for l in open(os.path.join(nat, 'native.sync.tsv')) if l[0].isdigit()]
pts = [(int(r[3]), float(r[1])) for r in sync if len(r) > 3 and r[3].strip()]
start = 0
for k in range(1, len(pts)):
    if pts[k][0] < pts[k - 1][0]: start = k
xs, ys = [], []
for c, fr in pts[start:]:
    if not xs or c > xs[-1]: xs.append(c); ys.append(fr)
def nf(c):
    i = min(max(bisect.bisect_left(xs, c), 1), len(xs) - 1)
    return int(ys[i - 1] + (c - xs[i - 1]) * (ys[i] - ys[i - 1]) / ((xs[i] - xs[i - 1]) or 1))
rows = [l.split('\t') for l in open(os.path.join(nat, 'drive.tsv')).read().splitlines()[1:]]
hit = None
for a, b in zip(rows, rows[1:]):
    if float(a[1]) > 0.3 and float(a[4]) - float(b[4]) > 3.0: hit = int(a[0]); break
def stats(x, rate, i):
    bed = db((x[max(0, i - rate // 2):i] ** 2).mean())
    w = rate // 20; seg = x[i:i + rate]
    loud = max(db((seg[k:k + w] ** 2).mean()) for k in range(0, len(seg) - w, w // 4))
    return bed, loud
if hit:
    # the native's game time runs slower than the wav (fixed cadence): 0.5 s before = 2000 ticks before
    i = nf(hit); j0 = nf(hit - 2000); w = nr // 20
    bed = db((na[j0:i] ** 2).mean()); seg = na[i:nf(hit + 4000)]
    loud = max(db((seg[k:k + w] ** 2).mean()) for k in range(0, len(seg) - w, w // 4))
    print('original: hit at clock %d  bed %.1f  hit %.1f  hit-over-bed %+.1f' % (hit, bed, loud, loud - bed))
else: print('original: no hit')
ca, cr = readwav(os.path.join(cl, 'clone.wav'))
h = None
for l in open(os.path.join(cl, 'clone.log')):
    if l.startswith('PROPCAR'): h = int(l.split()[1][1:]); break
if h:
    bed, loud = stats(ca, cr, int(h / 60 * cr))
    print('clone   : hit at step %d  bed %.1f  hit %.1f  hit-over-bed %+.1f' % (h, bed, loud, loud - bed))
else: print('clone: no hit')
