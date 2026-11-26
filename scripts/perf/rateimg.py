#!/usr/bin/env python3
"""Rate gate v2 images (docs/developing/checks.md "Rate gate"): the α = 0 frames a rate bench dumps (NFSU2_RATE_DUMP:
tick-<n>.png at ticks ≡ 0 mod 6 from the schedule's start) compared against the reference rate's, by tick.

usage: rateimg.py PIXDIFF CLASS REF_DIR HZ=DIR ...

CLASS: orig (Original preset), step (Extreme with Anti-Aliasing 5 and no volumetric fog), hist (Extreme: its volumetric
fog's history and every other per-tick history) — each AE 0 — or taa (Extreme with TAA: the temporal pass accumulates
drawn frames by nature, so the frames differ between rates; bounded: mean |Δ| <= TAA_MEAN/255 per frame, default
2.5, the worst measured 1.8 at 30 Hz; p99.9 is not measured, pixdiff gives no histogram, so the line reports the max
channel diff too). Prints one line:
per rate the frames matched, identical, over the limit, the worst pixel count, max channel diff and mean |Δ|, and
the first tick that differs; ticks one side lacks are counted ("missing": a rate with no α = 0 frame there).
Exit 1 when a rate is over its limit or has no frame in common with the reference.

Evidence (does not change the verdict): for the first EVID_TICKS (3) over-limit ticks of a rate, a detail block goes to
stderr (class, rate, tick, differing pixels, max channel diff, bounding box, up to 8 pixel coordinates with both
values), and the reference frame, the rate's frame and a diff PNG (differing pixels magenta, a yellow frame around
the box) are copied to $RATE_EVIDENCE_DIR/<date-time>-<short rev>/ (default `overbright-rate-evidence` in the system's temp folder; the 20
newest folders are kept). The folder is named in the printed line ("evidence <folder>")."""
import datetime
import os
import re
import shutil
import struct
import subprocess
import sys
import tempfile
import zlib

EVID_TICKS = 3
EVID_KEEP = 20

LINE = re.compile(r'^(tick-\d+)\.png: (\d+) of (\d+) pixels differ .*max channel diff (\d+), mean abs diff ([0-9.]+)')
SAME = re.compile(r'^(tick-\d+)\.png: (identical|0 of)')


def ticks(d):
    try:
        return {f[:-4] for f in os.listdir(d) if f.startswith('tick-') and f.endswith('.png')}
    except OSError:
        return set()


def read_png(path):
    """(width, height, channels, rows of bytes) of an 8-bit non-interlaced grey/RGB/RGBA PNG, or None."""
    with open(path, 'rb') as f:
        data = f.read()
    if data[:8] != b'\x89PNG\r\n\x1a\n':
        return None
    pos, idat, hdr = 8, [], None
    while pos + 8 <= len(data):
        n, typ = struct.unpack('>I4s', data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + n]
        pos += 12 + n
        if typ == b'IHDR':
            hdr = struct.unpack('>IIBBBBB', body)
        elif typ == b'IDAT':
            idat.append(body)
    if not hdr:
        return None
    w, h, depth, ctype, _, _, inter = hdr
    ch = {0: 1, 2: 3, 4: 2, 6: 4}.get(ctype)
    if depth != 8 or inter or ch is None:
        return None
    raw = zlib.decompress(b''.join(idat))
    stride = w * ch
    rows, prev = [], bytearray(stride)
    for y in range(h):
        ft = raw[y * (stride + 1)]
        cur = bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        if ft == 1:
            for i in range(ch, stride):
                cur[i] = (cur[i] + cur[i - ch]) & 255
        elif ft == 2:
            for i in range(stride):
                cur[i] = (cur[i] + prev[i]) & 255
        elif ft == 3:
            for i in range(stride):
                cur[i] = (cur[i] + (((cur[i - ch] if i >= ch else 0) + prev[i]) >> 1)) & 255
        elif ft == 4:
            for i in range(stride):
                a = cur[i - ch] if i >= ch else 0
                b = prev[i]
                c = prev[i - ch] if i >= ch else 0
                pa, pb, pc = abs(b - c), abs(a - c), abs(a + b - 2 * c)
                cur[i] = (cur[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        elif ft != 0:
            return None
        rows.append(cur)
        prev = cur
    return w, h, ch, rows


def write_png(path, w, h, rows):
    """RGB 8-bit PNG from rows of w*3 bytes."""
    def chunk(t, b):
        return struct.pack('>I', len(b)) + t + b + struct.pack('>I', zlib.crc32(t + b) & 0xffffffff)
    raw = b''.join(b'\0' + bytes(r) for r in rows)
    with open(path, 'wb') as f:
        f.write(b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0))
                + chunk(b'IDAT', zlib.compress(raw, 6)) + chunk(b'IEND', b''))


def compare(ref_png, png):
    """(differing pixel list [(x, y, ref values, values)], max channel diff, (w, h, ch, ref rows)) or None."""
    a, b = read_png(ref_png), read_png(png)
    if not a or not b or a[:3] != b[:3]:
        return None
    w, h, ch, ra = a
    rb = b[3]
    diffs, mx = [], 0
    for y in range(h):
        if ra[y] == rb[y]:
            continue
        for x in range(w):
            p, q = tuple(ra[y][x * ch:(x + 1) * ch]), tuple(rb[y][x * ch:(x + 1) * ch])
            if p != q:
                diffs.append((x, y, p, q))
                mx = max(mx, max(abs(i - j) for i, j in zip(p, q)))
    return diffs, mx, a


def diff_png(path, a, diffs):
    w, h, ch, rows = a
    out = []
    for r in rows:
        o = bytearray()
        for x in range(w):
            g = (r[x * ch] if ch < 3 else (r[x * ch] + r[x * ch + 1] + r[x * ch + 2]) // 3) // 3
            o += bytes((g, g, g))
        out.append(o)
    x0, x1 = min(d[0] for d in diffs), max(d[0] for d in diffs)
    y0, y1 = min(d[1] for d in diffs), max(d[1] for d in diffs)
    if len(diffs) < 1000:
        l, r_, t, b = max(x0 - 4, 0), min(x1 + 4, w - 1), max(y0 - 4, 0), min(y1 + 4, h - 1)
        for x in range(l, r_ + 1):
            for y in (t, b):
                out[y][x * 3:x * 3 + 3] = b'\xff\xff\x00'
        for y in range(t, b + 1):
            for x in (l, r_):
                out[y][x * 3:x * 3 + 3] = b'\xff\xff\x00'
    for x, y, _, _ in diffs:
        out[y][x * 3:x * 3 + 3] = b'\xff\x00\xff'
    write_png(path, w, h, out)


def evidence_dir(state):
    """The run's evidence folder (made once, older ones beyond the newest EVID_KEEP removed)."""
    if 'dir' in state:
        return state['dir']
    root = os.environ.get('RATE_EVIDENCE_DIR') or os.path.join(tempfile.gettempdir(), 'overbright-rate-evidence')
    try:
        rev = subprocess.run(['git', '-C', os.path.dirname(os.path.abspath(__file__)), 'rev-parse', '--short', 'HEAD'],
                             capture_output=True, text=True, timeout=20).stdout.strip() or 'norev'
    except (OSError, subprocess.SubprocessError):
        rev = 'norev'
    d = os.path.join(root, datetime.datetime.now().strftime('%Y%m%d-%H%M%S') + '-' + rev)
    try:
        os.makedirs(d, exist_ok=True)
        old = sorted(n for n in os.listdir(root) if re.match(r'^\d{8}-\d{6}-', n) and os.path.isdir(os.path.join(root, n)))
        for n in old[:-EVID_KEEP]:
            shutil.rmtree(os.path.join(root, n), ignore_errors=True)
    except OSError as e:
        print(f'rateimg: evidence folder {d}: {e}', file=sys.stderr)
        d = None
    state['dir'] = d
    return d


def save_evidence(state, cls, hz, tick, ref_png, png):
    """Print the detail of one differing tick (stderr) and keep its images; never raises."""
    try:
        d = evidence_dir(state)
        res = None
        try:
            res = compare(ref_png, png)
        except (OSError, zlib.error, struct.error, ValueError) as e:
            print(f'rateimg: {cls} {hz} Hz {tick}: cannot decode the PNGs: {e}', file=sys.stderr)
        if res and res[0]:
            diffs, mx, a = res
            xs, ys = [p[0] for p in diffs], [p[1] for p in diffs]
            print(f'rateimg evidence: class {cls}, {hz} Hz, {tick}: {len(diffs)} px differ, max channel diff {mx}, '
                  f'bbox x {min(xs)}..{max(xs)} y {min(ys)}..{max(ys)}', file=sys.stderr)
            for x, y, p, q in diffs[:8]:
                print(f'  ({x},{y}) ref {p} got {q}', file=sys.stderr)
        elif res is not None:
            print(f'rateimg evidence: class {cls}, {hz} Hz, {tick}: the PNGs decode identical (pixdiff saw a difference)',
                  file=sys.stderr)
        if d:
            pre = f'{cls}-{hz}hz-{tick}'
            shutil.copy(ref_png, os.path.join(d, pre + '-ref.png'))
            shutil.copy(png, os.path.join(d, pre + '.png'))
            if res and res[0]:
                diff_png(os.path.join(d, pre + '-diff.png'), res[2], res[0])
    except Exception as e:  # evidence must never change the verdict
        print(f'rateimg: evidence for {cls} {hz} Hz {tick} failed: {e!r}', file=sys.stderr)


def main(argv):
    if len(argv) < 4:
        print(__doc__)
        return 2
    pixdiff, cls, ref = argv[0], argv[1], argv[2]
    rt = ticks(ref)
    parts = []
    state = {}
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
        for r in over[:EVID_TICKS]:
            save_evidence(state, cls, hz, r[0], os.path.join(ref, r[0] + '.png'), os.path.join(d, r[0] + '.png'))
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
    ev = f' [evidence {state["dir"]}]' if state.get('dir') else ''
    print(f'{cls} vs {os.path.basename(os.path.normpath(ref))}: ' + '; '.join(parts) + ev)
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
