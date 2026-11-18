#!/usr/bin/env python3
"""jets.py IMG... : fountain jet region (1280x960: x 0..1280, y 220..480, the arcs over the casino basins; the
HUD's map icon 60..210 x 180..340 left out) luma p95 / mean / p50 (Rec.709 on sRGB codes)."""
import sys, os
import numpy as np
# Maintainer only (private test set): lstats comes from the private look tools under $OB_PRIVATE.
sys.path.insert(0, os.path.expandvars('$OB_PRIVATE/tools/look'))
from lstats import load
for p in sys.argv[1:]:
    im = load(p); Y = 0.2126 * im[..., 0] + 0.7152 * im[..., 1] + 0.0722 * im[..., 2]
    m = np.zeros(Y.shape, bool); m[220:480, :] = True; m[180:340, 60:210] = False
    y = Y[m]
    print(f"{os.path.basename(p):28s} jets p95 {np.percentile(y,95):6.1f} p75 {np.percentile(y,75):6.1f} mean {y.mean():6.1f}")
