#!/usr/bin/env python3
"""Halo check (docs/wet-road-car.md): halo.py WITH.png WITHOUT.png
WITH = the reported max rain chase (the finished-career save's Skyline, yellow underglow, 217 km/h) frame; WITHOUT = the same
frame with Wet Road Interaction and Next-Gen car lights off. In the box over the car's upper body (the
rear window, the spoiler, the flanks above the bumper: x 33-66 %, y 58-80 % of a 16:9 chase frame) the
rows may change the body only a little: no glow shell around the silhouette (mean |diff|), and nothing
they add there may be blue (the underglow is yellow: the B,G,R neon byte order bug lit it cyan)."""
import subprocess, sys, numpy as np
def load(p):
    w, h = map(int, subprocess.run(['magick', 'identify', '-format', '%w %h', p], capture_output=True, text=True).stdout.split()[:2])
    raw = subprocess.run(['magick', p, '-alpha', 'off', '-depth', '8', 'rgb:-'], capture_output=True).stdout
    return np.frombuffer(raw, np.uint8).reshape(h, w, 3).astype(np.float32)
a, b = load(sys.argv[1]), load(sys.argv[2])
h, w, _ = a.shape
box = (slice(int(h * 0.58), int(h * 0.80)), slice(int(w * 0.33), int(w * 0.66)))
d = a[box] - b[box]
halo = np.abs(d).mean()
added = np.clip(d, 0, None)
blue = (added[..., 2] - 0.5 * (added[..., 0] + added[..., 1])).mean()
HALO_MAX, BLUE_MAX = float(sys.argv[3]) if len(sys.argv) > 3 else 3.5, 1.0
ok = halo <= HALO_MAX and blue <= BLUE_MAX
print(f"halo: body box mean |diff| {halo:.2f} (max {HALO_MAX}), blue excess of the added light {blue:.2f} (max {BLUE_MAX}): {'PASS' if ok else 'FAIL'}")
sys.exit(0 if ok else 1)
