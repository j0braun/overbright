#!/usr/bin/env python3
"""Summarize a samply (gecko) profile + .syms.json: per-thread CPU, self and inclusive time per function.
usage: prof.py profile.json.gz [topN] [skip_s]   (skip_s: seconds after the first sample left out, the load)"""
import json, gzip, sys, re, bisect, collections, os
if len(sys.argv) < 2 or len(sys.argv) > 4:
    sys.exit(__doc__)
try:
    path = sys.argv[1]; top = int(sys.argv[2]) if len(sys.argv) > 2 else 40
    skip = float(sys.argv[3]) if len(sys.argv) > 3 else 0.0  # seconds after the first sample to skip (load)
except ValueError as e:
    sys.exit(f'prof.py: {e}\n{__doc__}')
p = json.load(gzip.open(path)); s = json.load(open(path.replace('.json.gz', '.json.syms.json')))
strings = s['string_table']
libsyms = {}
for d in s['data']:
    tab = sorted(d['symbol_table'], key=lambda e: e['rva'])
    libsyms[d['debug_name']] = ([e['rva'] for e in tab], tab)
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
    rvas, tab = libsyms[lib]
    k = bisect.bisect_right(rvas, addr) - 1
    if k >= 0 and addr < tab[k]['rva'] + max(tab[k]['size'], 1) + 4096:
        return demangle(strings[tab[k]['symbol']])
    return f'{lib}@{addr:x}'
libs = p['libs']
t0 = min((t['samples']['time'][0] for t in p['threads'] if t['samples']['length']), default=0)
res = []
for t in p['threads']:
    n = t['samples']['length']
    if n == 0: continue
    ft, fu, rt, st_ = t['frameTable'], t['funcTable'], t['resourceTable'], t['stackTable']
    def fname(f):
        r = fu['resource'][ft['func'][f]]
        lib = libs[rt['lib'][r]]['debugName'] if r >= 0 else '?'
        return sym(lib, ft['address'][f]) , lib
    cache = {}
    def stack_frames(sidx):
        out = []
        while sidx is not None and sidx >= 0:
            out.append(st_['frame'][sidx]); sidx = st_['prefix'][sidx]
        return out
    cpu = t['samples'].get('threadCPUDelta') or [1]*n
    selfc, incl = collections.Counter(), collections.Counter()
    total = 0
    for i in range(n):
        if t['samples']['time'][i] < t0 + skip*1000: continue
        sidx = t['samples']['stack'][i]
        w = cpu[i] or 0
        total += w
        if sidx is None: continue
        if sidx not in cache:
            names = [fname(f)[0] for f in stack_frames(sidx)]
            cache[sidx] = names
        names = cache[sidx]
        if names: selfc[names[0]] += w
        for nm in set(names): incl[nm] += w
    res.append((total, t['tid'], t['name'], selfc, incl, n))
res.sort(key=lambda x: -x[0])
grand = sum(r[0] for r in res)
if grand == 0:
    sys.exit(f'prof.py: no CPU samples after the first {skip} s (profile too short? lower skip_s)')
print(f'# {os.path.basename(path)}: total thread CPU {grand/1000:.0f} ms over {len(res)} threads (threadCPUDelta in us)')
for total, tid, name, selfc, incl, n in res[:12]:
    if total == 0: continue
    print(f'\n## thread {name} {tid}: {total/1000:.0f} ms CPU ({100*total/grand:.1f}%), {n} samples')
    print('self:', '; '.join(f'{k[:90]} {100*v/total:.1f}%' for k, v in selfc.most_common(top//2)))
    print('incl:', '; '.join(f'{k[:90]} {100*v/total:.1f}%' for k, v in incl.most_common(top) if not k.startswith(('std::','core::','__libc','start_thread','clone','_start','main','__clone'))))
