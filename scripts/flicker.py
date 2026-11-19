#!/usr/bin/env python3
"""Temporal flicker of a region over a frame sequence (docs/notes/graphics/car-look.md "Chrome trunk flicker and dry-road reflection strength").

flicker.py RECT FRAME.png...   RECT = WxH+X+Y
Prints: mean luma, mean per-pixel temporal std, p99 std, mean |frame-to-frame
luma delta|, share of pixels whose consecutive delta exceeds 8 levels, mean |second
difference| and its share over 16 levels.
Luma = Rec.601 on 0-255 sRGB values. Needs ImageMagick and numpy."""
import subprocess, sys
import numpy as np

def load(path, rect):
    w, rest = rect.split('x'); h, x, y = rest.split('+')
    raw = subprocess.run(['magick', path, '-crop', rect, '+repage', '-depth', '8', 'rgb:-'], check=True, capture_output=True).stdout
    a = np.frombuffer(raw, np.uint8).reshape(int(h), int(w), 3).astype(np.float64)
    return a[..., 0] * 0.299 + a[..., 1] * 0.587 + a[..., 2] * 0.114

def main():
    rect, files = sys.argv[1], sys.argv[2:]
    s = np.stack([load(f, rect) for f in files])
    std = s.std(axis=0)
    d = np.abs(np.diff(s, axis=0))
    # Second difference: cancels steady motion, keeps frame-to-frame
    # alternation (flicker) when the car or camera moves.
    a = np.abs(s[2:] - 2 * s[1:-1] + s[:-2])
    print(f"frames {len(files)} mean {s.mean():.2f} std_mean {std.mean():.3f} std_p99 {np.percentile(std, 99):.2f} delta_mean {d.mean():.3f} delta>8 {100 * (d > 8).mean():.3f}% acc_mean {a.mean():.3f} acc>16 {100 * (a > 16).mean():.3f}%")

main()
