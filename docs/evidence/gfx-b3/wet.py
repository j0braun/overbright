#!/usr/bin/env python3
"""wet.py DIR [ORIGDIR]: wetness/volume metrics per <scene>__<label>.png against <scene>__0orig.png
(Rec.709 luma on sRGB codes, HUD masked as lstats does). Road boxes away from the car (1280x960, x0,x1,y0,y1):
sheen = mean max(0, dY) in the boxes (light the wet road adds over the dry native road: the native road
draws the same rain or not, docs/wet-roads-ssr.md), cov = % box px with dY > 4, hf = box high-frequency
energy (mean |Y - box3x3(Y)|, speckle) as a ratio to Original's."""
import sys, os, glob
import numpy as np
# Maintainer only (private test set): lstats comes from the private look tools under $OB_PRIVATE.
sys.path.insert(0, os.path.expandvars('$OB_PRIVATE/tools/look'))
from lstats import load, stats
ROAD = {'rain': [(0, 400, 568, 602), (880, 1270, 568, 600)],
        'rainharb': [(20, 470, 560, 615), (820, 1270, 500, 590)],
        'street': [(0, 400, 568, 602), (880, 1270, 568, 600)]}
def Y(i): return 0.2126 * i[..., 0] + 0.7152 * i[..., 1] + 0.0722 * i[..., 2]
def hf(y):
    p = np.pad(y, 1, mode='edge'); b = sum(p[1 + dy:p.shape[0] - 1 + dy, 1 + dx:p.shape[1] - 1 + dx] for dy in (-1, 0, 1) for dx in (-1, 0, 1)) / 9
    return np.abs(y - b).mean()
d = sys.argv[1]; od = sys.argv[2] if len(sys.argv) > 2 else d
for f in sorted(glob.glob(f'{d}/*__*.png')):
    n = os.path.basename(f)[:-4]; sc, lab = n.split('__', 1)
    o = f'{od}/{sc}__0orig.png'
    if lab == '0orig' or not os.path.exists(o): continue
    im, oi = load(f), load(o); s, so = stats(im), stats(oi)
    line = f"{n:24s} dmean {s['mean']-so['mean']:+6.2f} dp5 {s['p5']-so['p5']:+5.2f} dp95 {s['p95']-so['p95']:+6.2f} dsat {s['satb']-so['satb']:+.3f}"
    if sc in ROAD:
        ya, yo = Y(im), Y(oi); sh = []; cv = []; h = []
        for x0, x1, y0, y1 in ROAD[sc]:
            dy = ya[y0:y1, x0:x1] - yo[y0:y1, x0:x1]
            sh.append(np.maximum(dy, 0).mean()); cv.append(100 * (dy > 4).mean()); h.append(hf(ya[y0:y1, x0:x1]) / max(hf(yo[y0:y1, x0:x1]), 1e-3))
        line += f"  road sheen L/R {sh[0]:5.2f}/{sh[1]:5.2f} cov {cv[0]:5.1f}/{cv[1]:5.1f}% hf x{h[0]:.2f}/{h[1]:.2f}"
    print(line)
