#!/usr/bin/env python3
"""fmetrics.py VIEW IMG... : the fountain box's numbers (luma Rec.709 on sRGB codes; areas in 1080p pixels).
mean / p95 of the box; sat = % >= 250; big = largest connected >= 250 region; flat = % of the box that is
bright (> 140) and smooth (5x5 std < 4: tubes / ropes, no streaks); drops = small bright specks (1..40 px
regions standing 35+ over their 9x9 mean) per 10k px of the box.
VIEW boxes in 1920x1080 coordinates; HUD boxes masked."""
import sys, subprocess, re, numpy as np

BOX = {
    'casino10': (0, 0, 1920, 560), 'casino30': (260, 330, 1460, 630), 'casino80': (420, 380, 1300, 560),
    'casinolow': (0, 0, 1920, 700), 'playerview': (0, 0, 1920, 560), 'wine': (0, 150, 1920, 560), 'C': (300, 30, 1350, 640), 'B': (450, 180, 1100, 520),
    'N': (0, 0, 1920, 640), 'P': (0, 0, 1450, 640),
}
HUD = [(40, 160, 220, 300), (1180, 70, 1580, 150), (0, 560, 420, 1080), (1150, 560, 1920, 1080)]

def load(p):
    w, h = map(int, subprocess.run(['magick', 'identify', '-format', '%w %h', p], capture_output=True, text=True).stdout.split()[:2])
    raw = subprocess.run(['magick', p, '-alpha', 'off', '-depth', '8', 'rgb:-'], capture_output=True).stdout
    return np.frombuffer(raw, np.uint8).reshape(h, w, 3).astype(np.float32)

def boxmean(a, k):
    p = k // 2
    q = np.pad(a, p, mode='edge').cumsum(0).cumsum(1)
    q = np.pad(q, ((1, 0), (1, 0)))
    return (q[k:, k:] - q[:-k, k:] - q[k:, :-k] + q[:-k, :-k]) / (k * k)

def components(mask):
    """areas of 8-connected regions (ImageMagick)."""
    h, w = mask.shape
    if not mask.any():
        return []
    pgm = b'P%d %d %d 255\n' % (5, w, h) + (mask.astype(np.uint8) * 255).tobytes()
    out = subprocess.run(['magick', 'pgm:-', '-define', 'connected-components:verbose=true', '-define',
                          'connected-components:area-threshold=0', '-connected-components', '8', 'null:'],
                         input=pgm, capture_output=True).stdout.decode()
    areas = []
    for line in out.splitlines():
        m = re.match(r'\s*\d+: \S+ \S+ (\d+) (gray\(255\)|\S*\(255,255,255\)|#FFFFFF)', line)
        if m:
            areas.append(int(m.group(1)))
    return areas

def metrics(view, p):
    im = load(p); h, w, _ = im.shape; s = w / 1920.0
    Y = 0.2126 * im[..., 0] + 0.7152 * im[..., 1] + 0.0722 * im[..., 2]
    m = np.zeros((h, w), bool)
    x0, y0, x1, y1 = BOX[view]
    m[int(y0 * s):int(y1 * s), int(x0 * s):int(x1 * s)] = True
    for a, b, c, d in HUD:
        m[int(b * s):int(d * s), int(a * s):int(c * s)] = False
    y = Y[m]
    sat = (Y >= 250) & m
    comps = components(sat)
    big = max(comps) / (s * s) if comps else 0
    k = max(3, int(round(5 * s)) | 1)
    mu = boxmean(Y, k); sd = np.sqrt(np.maximum(boxmean(Y * Y, k) - mu * mu, 0))
    flat = ((Y > 140) & (sd < 4) & m).sum() / m.sum() * 100
    k9 = max(5, int(round(9 * s)) | 1)
    spk = (Y - boxmean(Y, k9) > 35) & (Y > 90) & m
    sc = components(spk)
    drops = sum(1 for a in sc if a <= 40 * s * s) / (m.sum() / s / s) * 1e4
    return dict(mean=y.mean(), p95=np.percentile(y, 95), sat=sat.sum() / m.sum() * 100, big=big, flat=flat, drops=drops)

if __name__ == '__main__':
    v = sys.argv[1]
    for p in sys.argv[2:]:
        r = metrics(v, p)
        print(f"{p.split('/')[-1]:44s} mean {r['mean']:6.1f} p95 {r['p95']:6.1f} sat {r['sat']:5.2f}% big {r['big']:6.0f} flat {r['flat']:5.2f}% drops {r['drops']:6.2f}")
