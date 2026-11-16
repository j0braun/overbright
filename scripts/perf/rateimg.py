#!/usr/bin/env python3
"""Rate gate v2 images (docs/checks.md "Rate gate"): the α = 0 frames a rate bench dumps (NFSU2_RATE_DUMP:
tick-<n>.png at ticks ≡ 0 mod 6 from the schedule's start) compared against the reference rate's, by tick.

usage: rateimg.py PIXDIFF CLASS REF_DIR HZ=DIR ...

CLASS: orig (Original preset), step (Extreme with Anti-Aliasing 5 and no volumetric fog), hist (Extreme: its volumetric
fog's history and every other per-tick history) — each AE 0 — or taa (Extreme with TAA: the temporal pass accumulates
drawn frames by nature, so the frames differ between rates; bounded: mean |Δ| <= TAA_MEAN/255 per frame, default
2.5, the worst measured 1.8 at 30 Hz; p99.9 is not measured, pixdiff gives no histogram, so the line reports the max
channel diff too). Prints one line:
per rate the frames matched, identical, over the limit, the worst pixel count, max channel diff and mean |Δ|, and
the first tick that differs; ticks one side lacks are counted ("missing": a rate with no α = 0 frame there).
Exit 1 when a rate is over its limit or has no frame in common with the reference."""
import os
import re
import subprocess
import sys

LINE = re.compile(r'^(tick-\d+)\.png: (\d+) of (\d+) pixels differ .*max channel diff (\d+), mean abs diff ([0-9.]+)')
SAME = re.compile(r'^(tick-\d+)\.png: (identical|0 of)')


def ticks(d):
    try:
        return {f[:-4] for f in os.listdir(d) if f.startswith('tick-') and f.endswith('.png')}
    except OSError:
        return set()


def main(argv):
    if len(argv) < 4:
        print(__doc__)
        return 2
    pixdiff, cls, ref = argv[0], argv[1], argv[2]
    rt = ticks(ref)
    parts = []
    bad = 0
    for a in argv[3:]:
        hz, _, d = a.partition('=')
        ht = ticks(d)
        both = sorted(rt & ht)
        if not both:
            parts.append(f'{hz} Hz: no α=0 frame in common ({len(ht)} dumped, {len(rt)} at the reference)')
            bad += 1
            continue
        tmp = os.path.join(d, '.common')
        os.makedirs(tmp, exist_ok=True)
        tmpr = os.path.join(d, '.ref')
        os.makedirs(tmpr, exist_ok=True)
        for t in both:
            for src, dst in ((os.path.join(d, t + '.png'), os.path.join(tmp, t + '.png')),
                             (os.path.join(ref, t + '.png'), os.path.join(tmpr, t + '.png'))):
                if not os.path.exists(dst):
                    os.symlink(os.path.abspath(src), dst)
        out = subprocess.run([pixdiff, tmpr, tmp], capture_output=True, text=True).stdout.splitlines()
        rows = [m.groups() for m in map(LINE.match, out) if m]
        differ = [(t, int(p), int(mx), float(mean)) for t, p, _, mx, mean in rows if int(p) > 0]
        if cls == 'taa':
            limit = float(os.environ.get('TAA_MEAN', '2.5'))
            over = [r for r in differ if r[3] > limit]
        else:
            over = differ
        worst = max((r[1] for r in differ), default=0)
        maxd = max((r[2] for r in differ), default=0)
        mean = max((r[3] for r in differ), default=0.0)
        first = min((int(r[0][5:]) for r in differ), default=None)
        missing = len(rt ^ ht)
        bad += 1 if over else 0
        parts.append(f'{hz} Hz: {len(both)} frames, {len(both) - len(differ)} identical, {len(over)} over'
                     f' (worst {worst} px, max diff {maxd}, max mean {mean:.4f}'
                     + (f', first at tick {first}' if first is not None else '') + ')'
                     + (f', {missing} ticks on one side only' if missing else ''))
    print(f'{cls} vs {os.path.basename(os.path.normpath(ref))}: ' + '; '.join(parts))
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
