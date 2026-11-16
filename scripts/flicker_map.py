#!/usr/bin/env python3
"""flicker_map.py RECT OUT.png FRAME.png...: the per-pixel mean |second temporal difference| of luma (x8, clipped) as a grey image."""
import subprocess, sys
import numpy as np
sys.path.insert(0, __file__.rsplit('/', 1)[0])
from importlib.machinery import SourceFileLoader
fl = SourceFileLoader('flicker', __file__.rsplit('/', 1)[0] + '/flicker.py')
rect, out, files = sys.argv[1], sys.argv[2], sys.argv[3:]
def load(path):
    w, rest = rect.split('x'); h, x, y = rest.split('+')
    raw = subprocess.run(['magick', path, '-crop', rect, '+repage', '-depth', '8', 'rgb:-'], check=True, capture_output=True).stdout
    a = np.frombuffer(raw, np.uint8).reshape(int(h), int(w), 3).astype(np.float64)
    return a[..., 0] * 0.299 + a[..., 1] * 0.587 + a[..., 2] * 0.114
s = np.stack([load(f) for f in files])
m = np.clip(np.abs(s[2:] - 2 * s[1:-1] + s[:-2]).mean(axis=0) * 8, 0, 255).astype(np.uint8)
h, w = m.shape
subprocess.run(['magick', '-size', f'{w}x{h}', '-depth', '8', 'gray:-', out], input=m.tobytes(), check=True)
