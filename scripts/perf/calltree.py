#!/usr/bin/env python3
"""Callee tree under ROOT on the busiest thread: the inclusive share of each
callee of the first frame matching ROOT_SUBSTR, of the thread's CPU.
usage: calltree.py prof.json.gz ROOT_SUBSTR [depth] [min_pct] [skip_s] [thread_substr]
(e.g. calltree.py racing-orig.json.gz "Game>::draw" 2 0.8)"""
import sys, runpy, collections, json, gzip
path, root = sys.argv[1], sys.argv[2]
depth = int(sys.argv[3]) if len(sys.argv) > 3 else 4
minp = float(sys.argv[4]) if len(sys.argv) > 4 else 0.5
skip = float(sys.argv[5]) if len(sys.argv) > 5 else 4
want = sys.argv[6] if len(sys.argv) > 6 else None
import re, bisect, os
p = json.load(gzip.open(path)); s = json.load(open(path.replace('.json.gz', '.json.syms.json')))
strings = s['string_table']; libsyms = {}
for d in s['data']:
    tab = sorted(d['symbol_table'], key=lambda e: e['rva']); libsyms[d['debug_name']] = ([e['rva'] for e in tab], tab)
def demangle(n):
    if n.startswith('_ZN'):
        out, i = [], 3
        while i < len(n) and n[i].isdigit():
            j = i
            while n[j].isdigit(): j += 1
            L = int(n[i:j]); seg = n[j:j+L]; i = j + L
            if re.fullmatch(r'h[0-9a-f]{16}', seg): break
            out.append(seg)
        n = '::'.join(out)
        for a, b in (('$LT$','<'),('$GT$','>'),('$RF$','&'),('$BP$','*'),('$C$',','),('$u20$',' '),('$u7b$','{'),('$u7d$','}'),('$u5b$','['),('$u5d$',']'),('$u27$',"'"),('..','::'),('$SP$','@')):
            n = n.replace(a, b)
    return n
def sym(lib, addr):
    if lib not in libsyms or addr < 0: return f'{lib}@?'
    rvas, tab = libsyms[lib]; k = bisect.bisect_right(rvas, addr) - 1
    if k >= 0 and addr < tab[k]['rva'] + max(tab[k]['size'], 1) + 4096: return demangle(strings[tab[k]['symbol']])
    return f'{lib}@{addr:x}'
libs = p['libs']
t0 = min((t['samples']['time'][0] for t in p['threads'] if t['samples']['length']), default=0)
best = None
for t in p['threads']:
    if t['samples']['length'] == 0: continue
    if want and want not in t['name'] and want not in str(t['tid']): continue
    cpu = t['samples'].get('threadCPUDelta') or [1]*t['samples']['length']
    tot = sum(c or 0 for c in cpu)
    if best is None or tot > best[0]: best = (tot, t)
if best is None:
    sys.exit(f'calltree.py: no thread with samples' + (f' matching {want!r}' if want else ''))
_, t = best
ft, fu, rt, st_ = t['frameTable'], t['funcTable'], t['resourceTable'], t['stackTable']
def fname(f):
    r = fu['resource'][ft['func'][f]]; lib = libs[rt['lib'][r]]['debugName'] if r >= 0 else '?'
    return sym(lib, ft['address'][f])
cache = {}
def names(sidx):
    if sidx in cache: return cache[sidx]
    out = []; x = sidx
    while x is not None and x >= 0: out.append(st_['frame'][x]); x = st_['prefix'][x]
    r = [fname(f) for f in reversed(out)]  # root first
    # collapse consecutive duplicates (inlined frames)
    c = [];
    for n in r:
        if not c or c[-1] != n: c.append(n)
    cache[sidx] = c; return c
cpu = t['samples'].get('threadCPUDelta') or [1]*t['samples']['length']
total = 0; tree = {}
def add(path, w):
    node = tree
    for n in path:
        e = node.setdefault(n, [0, {}]); e[0] += w; node = e[1]
for i in range(t['samples']['length']):
    if t['samples']['time'][i] < t0 + skip*1000: continue
    w = cpu[i] or 0; total += w
    sidx = t['samples']['stack'][i]
    if sidx is None: continue
    ns = names(sidx)
    k = next((j for j, n in enumerate(ns) if root in n), None)
    if k is None: continue
    add(['ROOT'] + ns[k+1:], w)
if total == 0:
    sys.exit(f'calltree.py: no samples after the first {skip} s (profile too short? lower skip_s)')
print(f'thread {t["name"]}: {total/1000:.0f} ms; under "{root}": {tree.get("ROOT",[0])[0]*100/total:.1f}%')
def show(node, d, ind):
    for n, (w, ch) in sorted(node.items(), key=lambda x: -x[1][0]):
        pct = 100*w/total
        if pct < minp: continue
        print(f'{ind}{pct:5.1f}% {n[:150]}')
        if d > 1: show(ch, d-1, ind + '  ')
if 'ROOT' in tree: show(tree['ROOT'][1], depth, '')
