#!/usr/bin/env python3
"""Callers of a self-time function on the busiest (or given) thread: for each sample
whose leaf frame contains FUNC_SUBSTR (or whose stack does, with --any), the chain of the DEPTH frames
above it, weighted by thread CPU, as a share of the thread.
usage: callers.py prof.json.gz FUNC_SUBSTR [skip_s] [thread_substr] [depth]"""
import sys, runpy
path, needle = sys.argv[1], sys.argv[2]
skip = float(sys.argv[3]) if len(sys.argv) > 3 else 4
want = sys.argv[4] if len(sys.argv) > 4 and sys.argv[4] else None
depth = int(sys.argv[5]) if len(sys.argv) > 5 else 4
# Reuse calltree.py's loader with a root that never matches (it only prints its header line).
sys.argv = [sys.argv[0].replace('callers.py', 'calltree.py'), path, '\x00never', '1', '100', str(skip)] + ([want] if want else [])
import io, contextlib
with contextlib.redirect_stdout(io.StringIO()):
    g = runpy.run_path(__file__.replace('callers.py', 'calltree.py'))
t, names, cpu, t0 = g['t'], g['names'], g['cpu'], g['t0']
total = 0; agg = {}
for i in range(t['samples']['length']):
    if t['samples']['time'][i] < t0 + skip * 1000: continue
    w = cpu[i] or 0; total += w
    sidx = t['samples']['stack'][i]
    if sidx is None: continue
    ns = names(sidx)
    k = max((j for j, n in enumerate(ns) if needle in n), default=None)
    if k is None: continue
    chain = ' <- '.join(n[:90] for n in reversed(ns[max(0, k - depth):k + 1]))
    agg[chain] = agg.get(chain, 0) + w
print(f'thread {t["name"]}: {total/1000:.0f} ms; samples in "{needle}": {sum(agg.values())*100/max(total,1):.1f}%')
for c, w in sorted(agg.items(), key=lambda x: -x[1])[:25]:
    print(f'{100*w/total:5.1f}% {c}')
