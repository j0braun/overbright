#!/usr/bin/env python3
"""The tail of a long bench run from its frame log (docs/perf-budget.md "CPU frame time and hitches").

  frametail.py LOG.tsv [TOP]    p50 / p95 / p99 / max of each column, then the slow frames, worst first

LOG.tsv is what `NFSU2_BENCH_FRAMELOG=LOG.tsv game --bench <scene> <frames>` writes (one line per timed frame,
src/bin/game/dev/bench/framelog.rs). wall = update + draw + the bench's wait for the GPU (ms); main busy = update
+ draw (ms; on a display schedule, NFSU2_BENCH_HZ, the draw holds the wait for the frame before); main CPU = the
main thread's CPU clock over the frame (ms). A slow frame is one over twice the median wall (and over 4 ms) or
twice the median main busy (and over 2 ms); each is listed with what happened in it: pipelines and shader modules
built, a change of section, traffic and encounter cars coming or going, and the draw's and the tick's parts over
0.25 ms of CPU.
"""
import statistics
import sys


def main():
    if len(sys.argv) < 2:
        sys.exit(__doc__)
    top = int(sys.argv[2]) if len(sys.argv) > 2 else 25
    with open(sys.argv[1]) as f:
        head = f.readline().rstrip('\n').split('\t')
        rows = []
        for line in f:
            v = line.rstrip('\n').split('\t')
            if len(v) >= len(head) - 1:
                rows.append(dict(zip(head, v + [''] * (len(head) - len(v)))))
    if not rows:
        sys.exit(f'frametail.py: no frames in {sys.argv[1]}')
    num = lambda r, k: float(r[k])
    wall = [num(r, 'update_ms') + num(r, 'draw_ms') + num(r, 'throttle_ms') for r in rows]
    busy = [num(r, 'update_ms') + num(r, 'draw_ms') for r in rows]
    cols = [('wall', wall), ('main busy', busy)]
    if 'main_cpu' in head:
        cols.append(('main CPU', [num(r, 'main_cpu') for r in rows]))
    cols += [('render CPU', [num(r, 'render_cpu') for r in rows]), ('other CPU', [num(r, 'other_cpu') for r in rows])]

    def q(v, p):
        s = sorted(v)
        return s[min(len(s) - 1, int(p * len(s)))]

    print(f'{len(rows)} frames')
    for name, v in cols:
        print(f'{name:11} p50 {q(v, .5):7.3f}  p95 {q(v, .95):7.3f}  p99 {q(v, .99):7.3f}  max {max(v):8.3f}  '
              f'mean {statistics.fmean(v):7.3f}')
    mw, mb = q(wall, .5), q(busy, .5)
    slow, prev = [], None
    for i, r in enumerate(rows):
        what = []
        if int(r['pipelines']) or int(r['modules']):
            what.append(f"built {r['pipelines']} pipelines / {r['modules']} modules")
        if prev is not None:
            for k in ('section', 'traffic', 'encounters'):
                if r[k] != prev[k]:
                    what.append(f'{k} {prev[k]} -> {r[k]}')
        if (wall[i] > 2 * mw and wall[i] > 4) or (busy[i] > 2 * mb and busy[i] > 2):
            slow.append((max(wall[i] / mw, busy[i] / mb), i, what))
        prev = r
    print(f'{len(slow)} slow frames (median wall {mw:.3f}, main busy {mb:.3f})')
    for x, i, what in sorted(slow, reverse=True)[:top]:
        r = rows[i]
        cpu = f" CPU {r['main_cpu']}" if 'main_cpu' in head else ''
        print(f"  frame {r['frame']:>6} x{x:5.1f}  wall {wall[i]:7.2f}  update {num(r, 'update_ms'):6.2f}  "
              f"draw {num(r, 'draw_ms'):6.2f}{cpu}  render CPU {r['render_cpu']} | {'; '.join(what)} | "
              f"{r.get('parts', '')[:200]}")


if __name__ == '__main__':
    main()
