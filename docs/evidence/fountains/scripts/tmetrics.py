#!/usr/bin/env python3
"""tmetrics.py VIEW F0 F1 ... : frame-to-frame change in the fountain box (consecutive 1/60 s ticks):
mean |dY| and p99 |dY|, and the share of box pixels flipping by more than 40 levels (popping / flicker)."""
import sys, numpy as np
sys.path.insert(0, __file__.rsplit('/', 1)[0])
from fmetrics import load, BOX, HUD
v = sys.argv[1]; fs = sys.argv[2:]
prev = None; md, p99, flip = [], [], []
for p in fs:
    im = load(p); h, w, _ = im.shape; s = w / 1920.0
    Y = 0.2126 * im[..., 0] + 0.7152 * im[..., 1] + 0.0722 * im[..., 2]
    m = np.zeros((h, w), bool); x0, y0, x1, y1 = BOX[v]
    m[int(y0 * s):int(y1 * s), int(x0 * s):int(x1 * s)] = True
    for a, b, c, d in HUD:
        m[int(b * s):int(d * s), int(a * s):int(c * s)] = False
    if prev is not None:
        d = np.abs(Y - prev)[m]
        md.append(d.mean()); p99.append(np.percentile(d, 99)); flip.append((d > 40).mean() * 100)
    prev = Y
print(f"{fs[0].rsplit('/',1)[-1][:-7]:30s} mean|dY| {np.mean(md):5.2f}  p99|dY| {np.mean(p99):6.1f}  flip>40 {np.mean(flip):5.2f}%  (n={len(md)})")
