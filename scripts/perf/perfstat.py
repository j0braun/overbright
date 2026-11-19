#!/usr/bin/env python3
"""Noise-aware statistics for the performance harness (docs/developing/perf-gates.md).

usage:
  perfstat.py summary DIR [--label L] [--md FILE] [--json FILE]
      per label and config: every metric's run values, median, MAD, noise
      and the smallest change the run count can detect (MDE); --json writes
      a baseline file (baseline.json).
  perfstat.py compare DIR --base A --new B [--md FILE] [--json FILE]
      A/B of two labels run interleaved by perf.sh (the gate); exit 1 on a
      regression or a config that only one arm ran.
  perfstat.py compare DIR --baseline FILE --new B [...]
      B against a stored baseline (indicative only: another time, another
      machine load).
  perfstat.py gate DIR --base A --new B [--md FILE] [--json FILE] [--tol 0.03]
      the regression gate (scripts/checks/perfgate.sh): A/B as compare,
      on the gate's metrics only, the FAIL metrics' tolerances at least --tol
      (3 %); a compact table per config; exit 1 when a FAIL metric is a
      REGRESSION, or when the gate is incomplete: a config the bench set
      out to run (DIR/bench.log "expect" line) with fewer than its reps in
      either arm, a FAIL metric without values, or an unreadable run file.
      With --baseline FILE instead of --base: against a stored summary.json.
  perfstat.py top DIR [--label L] [--n 8]
      the largest GPU passes and CPU parts per config (median of runs).
  perfstat.py selftest

Inputs: DIR/runs/<label>__<config>__r<rep>.json (game --bench with
NFSU2_BENCH_JSON) and <label>__transitions__r<rep>.txt (--bench transitions).

The unit of replication is the RUN (frames within a run are autocorrelated
and share the machine state), so every test is on per-run values: the
metric is a per-run statistic (median GPU busy, CPU ms per frame, ...),
and runs of the two arms are compared by an exact Mann-Whitney U test
(two-sided). A change is flagged only when it is significant (p < ALPHA)
AND larger than both the metric's tolerance and 2x the measured run-to-run
noise (1.4826 MAD / median, pooled over both arms). Exit status 1 when a
REGRESSION is flagged.
"""
import json, math, os, re, statistics as st, sys, glob, itertools
from collections import defaultdict

ALPHA = 0.05

# metric: (description, relative tolerance, absolute floor, lower is better)
METRICS = {
    'gpu_med':      ('GPU busy median ms (timestamps)',            0.02, 0.01, True),
    'gpu_p95':      ('GPU busy p95 ms',                            0.03, 0.02, True),
    'cpu_proc':     ('process CPU ms/frame (all threads)',         0.02, 0.02, True),
    'cpu_main':     ('main thread CPU ms/frame',                   0.02, 0.01, True),
    'cpu_render':   ('render workers CPU ms/frame (summed)',       0.02, 0.02, True),
    'update_cpu':   ('update (simulation) CPU ms/frame',           0.03, 0.005, True),
    'draw_cpu':     ('draw CPU ms/frame (main thread)',            0.02, 0.01, True),
    'draw_cpu_med': ('draw CPU median ms (per frame)',             0.02, 0.01, True),
    'draw_cpu_p10': ('draw CPU p10 ms (least contended frames)',   0.02, 0.01, True),
    'draw_cpu_p99': ('draw CPU p99 ms',                            0.05, 0.05, True),
    'frame_p50':    ('frame wall median ms (load-sensitive)',      0.03, 0.05, True),
    'frame_p99':    ('frame wall p99 ms (load-sensitive)',         0.10, 0.5, True),
    'load_ms':      ('load ms (wall)',                             0.10, 10.0, True),
    'first_ms':     ('first frame ms (wall)',                      0.10, 5.0, True),
    'vram_mb':      ('own VRAM peak MB (amdgpu fdinfo)',           0.01, 8.0, True),
    'ram_mb':       ('RAM peak MB (VmHWM)',                        0.02, 16.0, True),
}
# Transition loads (--bench transitions): tolerance 10 %, floor 10 ms.
LOAD_TOL = (0.10, 10.0)


def pct(v, p):
    v = sorted(v)
    if not v:
        return float('nan')
    k = (len(v) - 1) * p
    lo, hi = math.floor(k), math.ceil(k)
    return v[lo] + (v[hi] - v[lo]) * (k - lo)


def mad(v):
    if len(v) < 2:
        return 0.0
    m = st.median(v)
    return st.median([abs(x - m) for x in v])


def run_metrics(d):
    th = {t['name']: t['ms_per_frame'] for t in d.get('threads', [])}
    g = d.get('gpu_busy_ms') or []
    m = {
        'gpu_med': st.median(g) if g else None,
        'gpu_p95': pct(g, 0.95) if g else None,
        'cpu_proc': d.get('process_cpu_ms_per_frame'),
        'cpu_main': th.get('main (game)'),
        'cpu_render': sum(v for k, v in th.items() if k.startswith('render')) or None,
        'update_cpu': d.get('update_cpu_ms_per_frame'),
        'draw_cpu': d.get('draw_cpu_ms_per_frame'),
        'draw_cpu_med': pct(d['draw_cpu_ms'], 0.5) if d.get('draw_cpu_ms') else None,
        'draw_cpu_p10': pct(d['draw_cpu_ms'], 0.10) if d.get('draw_cpu_ms') else None,
        'draw_cpu_p99': pct(d['draw_cpu_ms'], 0.99) if d.get('draw_cpu_ms') else None,
        'frame_p50': pct(d['frame_ms'], 0.5) if d.get('frame_ms') else None,
        'frame_p99': pct(d['frame_ms'], 0.99) if d.get('frame_ms') else None,
        'load_ms': d.get('load_ms'),
        'first_ms': d.get('first_frame_ms'),
        'vram_mb': d['vram_peak_kib'] / 1024 if d.get('vram_peak_kib') else None,
        'ram_mb': (d.get('vm_hwm_end_kib') or d.get('vm_hwm_kib') or 0) / 1024 or None,
    }
    return m


def text_metrics(t):
    """The metrics a --bench text log carries (a run without
    NFSU2_BENCH_JSON, e.g. a binary older than the JSON output): no per-thread CPU,
    no VRAM; percentiles as the bench prints them (index, not
    interpolated), so compare text runs with text runs only."""
    def g(rx, k=1):
        mm = re.search(rx, t, re.M)
        return float(mm[k]) if mm else None
    stat = r'mean\s+([\d.]+)\s+median\s+([\d.]+)\s+p95\s+([\d.]+)\s+p99\s+([\d.]+)\s+max\s+([\d.]+)'
    return {
        'gpu_med': g(r'gpu busy ms/frame: ' + stat, 2),
        'gpu_p95': g(r'gpu busy ms/frame: ' + stat, 3),
        'cpu_proc': g(r'process cpu \(all threads\) ms/frame: ([\d.]+)'),
        'update_cpu': g(r'update   ms/frame: .*\(cpu ([\d.]+)\)'),
        'draw_cpu': g(r'draw     ms/frame: .*\(cpu ([\d.]+)\)'),
        'draw_cpu_p99': g(r'draw     cpu ms:\s+' + stat, 4),
        'frame_p50': g(r'frame    ms/frame: ' + stat, 2),
        'frame_p99': g(r'frame    ms/frame: ' + stat, 4),
        'load_ms': g(r'load (\d+) ms, first frame'),
        'first_ms': g(r'first frame ([\d.]+) ms'),
        'ram_mb': g(r'VmHWM\): (\d+) MB'),
    }


def parse_transitions(text):
    out = {}
    for line in text.splitlines():
        mm = re.match(r'^(\S.*?): load (\d+) ms, with its first frame (\d+) ms', line)
        if mm:
            out['load: ' + mm[1]] = float(mm[3])  # the load with its first frame
    return out


# Run files load_dir could not use (unparsable name or JSON): the gate fails on any.
LOAD_ERRORS = []


def split_name(f, ext):
    """label, config, rep of DIR/runs/<label>__<config>__r<rep>.<ext>, or None (recorded)."""
    parts = os.path.basename(f)[:-len(ext)].rsplit('__', 2)
    if len(parts) != 3 or not re.fullmatch(r'r\d+', parts[2]):
        LOAD_ERRORS.append(f'{f}: name is not label__config__rN')
        return None
    return parts


def expected(d):
    """The last bench block's plan from DIR/bench.log: (reps, [labels], [configs]) or None."""
    try:
        lines = [l for l in open(os.path.join(d, 'bench.log'), errors='replace') if l.startswith('expect ')]
    except OSError:
        return None
    if not lines:
        return None
    mm = re.match(r'^expect reps (\d+) labels (.*?) configs (.*)$', lines[-1].strip())
    if not mm:
        return None
    return int(mm[1]), mm[2].split(), mm[3].split()


def skipped(d):
    """The configs the last bench block left out and why (DIR/bench.log "skip CONFIG: REASON" lines after its "expect")."""
    try:
        lines = open(os.path.join(d, 'bench.log'), errors='replace').read().splitlines()
    except OSError:
        return []
    last = max((i for i, l in enumerate(lines) if l.startswith('expect ')), default=-1)
    return [l[5:].strip() for l in lines[last + 1:] if l.startswith('skip ')]


def load_dir(d):
    """{label: {config: [run dicts]}} with 'metrics', 'gpu_parts', 'cpu_parts', 'loadavg'."""
    data = defaultdict(lambda: defaultdict(list))
    for f in sorted(glob.glob(os.path.join(d, 'runs', '*__*__r*.json'))):
        sp = split_name(f, '.json')
        if not sp:
            continue
        label, cfg, rep = sp
        try:
            j = json.load(open(f))
        except Exception as e:
            print(f'skip {f}: {e}', file=sys.stderr)
            LOAD_ERRORS.append(f'{f}: {e}')
            continue
        txt = f[:-5] + '.txt'
        la = None
        if os.path.exists(txt):
            mm = re.search(r'^loadavg: ([\d.]+)', open(txt, errors='replace').read(), re.M)
            la = float(mm[1]) if mm else None
        data[label][cfg].append(dict(rep=int(rep[1:]), metrics=run_metrics(j),
                                     gpu_parts={p['name']: p['ms_per_frame'] for p in j.get('gpu_parts', [])},
                                     cpu_parts={p['name']: p['ms_per_frame'] for p in j.get('cpu_parts', [])},
                                     loadavg=la, frames=j.get('frames')))
    # Runs without JSON: their text logs.
    for f in sorted(glob.glob(os.path.join(d, 'runs', '*__*__r*.txt'))):
        sp = split_name(f, '.txt')
        if not sp:
            continue
        label, cfg, rep = sp
        if cfg == 'transitions' or os.path.exists(f[:-4] + '.json'):
            continue
        t = open(f, errors='replace').read()
        if 'gpu busy ms/frame' not in t and 'process cpu' not in t:
            continue
        mm = re.search(r'^loadavg: ([\d.]+)', t, re.M)
        data[label][cfg].append(dict(rep=int(rep[1:]), metrics={k: v for k, v in text_metrics(t).items() if v is not None},
                                     gpu_parts={}, cpu_parts={}, loadavg=float(mm[1]) if mm else None, frames=None))
    for f in sorted(glob.glob(os.path.join(d, 'runs', '*__transitions__r*.txt'))):
        sp = split_name(f, '.txt')
        if not sp:
            continue
        label, cfg, rep = sp
        t = open(f, errors='replace').read()
        loads = parse_transitions(t)
        if not loads:
            continue
        mm = re.search(r'^loadavg: ([\d.]+)', t, re.M)
        data[label][cfg].append(dict(rep=int(rep[1:]), metrics=loads, gpu_parts={}, cpu_parts={},
                                     loadavg=float(mm[1]) if mm else None, frames=None))
    return data


def metric_spec(name):
    if name in METRICS:
        return METRICS[name]
    return (name, LOAD_TOL[0], LOAD_TOL[1], True)


# ---- exact Mann-Whitney U ------------------------------------------------

def _u_counts(n, m):
    """Number of arrangements giving each U for sample sizes n, m (no ties)."""
    # f[i][j][u]: classic recursion f(i,j,u) = f(i-1,j,u-j) + f(i,j-1,u)
    from functools import lru_cache

    @lru_cache(maxsize=None)
    def f(i, j):
        if i == 0 or j == 0:
            return (1,)
        a = f(i - 1, j)  # shifted by j
        b = f(i, j - 1)
        size = i * j + 1
        out = [0] * size
        for u, c in enumerate(a):
            if u + j < size:
                out[u + j] += c
        for u, c in enumerate(b):
            out[u] += c
        return tuple(out)
    return f(n, m)


def mannwhitney(x, y):
    """Two-sided p of H0 'same distribution', and U of y over x. Exact
    without ties (small n), normal approximation with tie correction else."""
    n, m = len(x), len(y)
    if n == 0 or m == 0:
        return float('nan'), 0
    allv = sorted([(v, 0) for v in x] + [(v, 1) for v in y])
    ranks = {}
    i = 0
    ties = []
    vals = [v for v, _ in allv]
    while i < len(vals):
        j = i
        while j + 1 < len(vals) and vals[j + 1] == vals[i]:
            j += 1
        r = (i + j) / 2 + 1
        ties.append(j - i + 1)
        for k in range(i, j + 1):
            ranks.setdefault(k, r)
        i = j + 1
    ry = sum(ranks[k] for k, (_, g) in enumerate(allv) if g == 1)
    u = ry - m * (m + 1) / 2
    has_ties = any(t > 1 for t in ties)
    if not has_ties and n + m <= 40:
        counts = _u_counts(m, n)
        total = sum(counts)
        mu = n * m / 2
        dev = abs(u - mu)
        p = sum(c for k, c in enumerate(counts) if abs(k - mu) >= dev - 1e-9) / total
        return min(1.0, p), u
    mu = n * m / 2
    N = n + m
    tc = sum(t ** 3 - t for t in ties) / (N * (N - 1)) if N > 1 else 0
    sd = math.sqrt(n * m / 12 * ((N + 1) - tc)) or 1e-12
    z = (abs(u - mu) - 0.5) / sd
    p = math.erfc(max(z, 0) / math.sqrt(2))
    return min(1.0, p), u


def noise(vs):
    """Pooled relative run-to-run noise (1.4826 MAD / median) of arms."""
    devs, meds = [], []
    for v in vs:
        if len(v) >= 2:
            m = st.median(v)
            meds.append(abs(m))
            devs += [x - m for x in v]
    if not devs or not meds or st.median(meds) == 0:
        return float('nan')
    return 1.4826 * st.median([abs(d) for d in devs]) / st.median(meds)


def mde(rel_noise, n):
    """Smallest relative change detectable with n runs per arm (two-sided
    alpha 0.05, power 0.8; normal approximation x 1.15 for the U test's
    efficiency on small samples)."""
    if not rel_noise == rel_noise or n < 2:
        return float('nan')
    return 1.15 * (1.96 + 0.84) * rel_noise * math.sqrt(2 / n)


def verdict(name, x, y):
    desc, tol, floor, lower_better = metric_spec(name)
    mx, my = st.median(x), st.median(y)
    delta = my - mx
    rel = delta / mx if mx else float('nan')
    p, _ = mannwhitney(x, y)
    nz = noise([x, y])
    need = max(tol, 2 * (nz if nz == nz else 0))
    big = abs(rel) > need and abs(delta) > floor
    sig = p < ALPHA
    if big and sig:
        worse = (delta > 0) == lower_better
        v = 'REGRESSION' if worse else 'improvement'
    elif big:
        v = 'not significant'
    else:
        v = 'same'
    return dict(metric=name, base=mx, new=my, delta=delta, rel=rel, p=p, noise=nz,
                need=need, verdict=v, n=(len(x), len(y)),
                base_range=(min(x), max(x)), new_range=(min(y), max(y)))


def values(runs, metric):
    return [r['metrics'][metric] for r in runs if r['metrics'].get(metric) is not None]


def metrics_of(runs):
    names = []
    for r in runs:
        for k in r['metrics']:
            if k not in names:
                names.append(k)
    return names


def fmt(v, d=3):
    return '–' if v is None or v != v else f'{v:.{d}f}'


def cmd_summary(d, label=None, md=None, js=None):
    data = load_dir(d)
    out, lines = {}, []
    for lab in sorted(data):
        if label and lab != label:
            continue
        lines.append(f'## {lab}\n')
        lines.append('| config | metric | n | median | range | noise % | MDE % (n) | MDE % (n=5) |')
        lines.append('|---|---|---|---|---|---|---|---|')
        out[lab] = {}
        for cfg in sorted(data[lab]):
            runs = data[lab][cfg]
            out[lab][cfg] = {'runs': len(runs), 'loadavg': [r['loadavg'] for r in runs], 'metrics': {}}
            for m in metrics_of(runs):
                v = values(runs, m)
                if not v:
                    continue
                nz = noise([v])
                out[lab][cfg]['metrics'][m] = dict(values=v, median=st.median(v), mad=mad(v), noise=nz)
                lines.append(f'| {cfg} | {m} | {len(v)} | {fmt(st.median(v))} | {fmt(min(v))}–{fmt(max(v))} | {fmt(100 * nz, 1)} | {fmt(100 * mde(nz, len(v)), 1)} | {fmt(100 * mde(nz, 5), 1)} |')
            # Median GPU parts / CPU parts (for the top lists).
            for key in ('gpu_parts', 'cpu_parts'):
                names = set().union(*[r[key].keys() for r in runs]) if runs else set()
                out[lab][cfg][key] = {n: st.median([r[key].get(n, 0.0) for r in runs]) for n in names}
        lines.append('')
    text = '\n'.join(lines)
    if md:
        open(md, 'w').write(text + '\n')
    else:
        print(text)
    if js:
        json.dump(out, open(js, 'w'), indent=1)
    return 0


def cmd_compare(d, base=None, new=None, baseline=None, md=None, js=None):
    data = load_dir(d)
    if new not in data:
        sys.exit(f'no runs of label {new} in {d}')
    if baseline:
        b = json.load(open(baseline))
        bl = b.get(base) if base else next(iter(b.values()))
        arm_a = {cfg: {m: v['values'] for m, v in c['metrics'].items()} for cfg, c in bl.items()}
        title = f'{new} vs the stored baseline {baseline} (indicative: other time and machine load)'
    else:
        if base not in data:
            sys.exit(f'no runs of label {base} in {d}')
        arm_a = {cfg: {m: values(runs, m) for m in metrics_of(runs)} for cfg, runs in data[base].items()}
        title = f'{new} vs {base} (interleaved A/B)'
    res, lines, regress = [], [f'# {title}\n', f'Flag rule: exact Mann-Whitney U on per-run values, p < {ALPHA}, and |change| > max(tolerance, 2 x noise) and > floor.\n',
                               '| config | metric | base median [range] | new median [range] | change | p | noise % | needed % | verdict |', '|---|---|---|---|---|---|---|---|---|'], 0
    only = sorted(set(data[new]) ^ set(arm_a))
    for cfg in sorted(data[new]):
        if cfg not in arm_a:
            continue
        runs = data[new][cfg]
        for m in metrics_of(runs):
            x, y = arm_a[cfg].get(m, []), values(runs, m)
            if len(x) < 2 or len(y) < 2:
                continue
            r = verdict(m, x, y)
            r['config'] = cfg
            res.append(r)
            regress += r['verdict'] == 'REGRESSION'
            mark = '**' if r['verdict'] in ('REGRESSION', 'improvement') else ''
            lines.append(f"| {cfg} | {m} | {fmt(r['base'])} [{fmt(r['base_range'][0])}–{fmt(r['base_range'][1])}] | {fmt(r['new'])} [{fmt(r['new_range'][0])}–{fmt(r['new_range'][1])}] | {100 * r['rel']:+.2f} % | {r['p']:.3f} | {fmt(100 * r['noise'], 1)} | {100 * r['need']:.1f} | {mark}{r['verdict']}{mark} |")
    counts = defaultdict(int)
    for r in res:
        counts[r['verdict']] += 1
    lines.append('')
    if only:
        lines.append(f"configs run by one arm only (not compared): {', '.join(only)}")
    lines.append(f"perfstat: {len(res)} comparisons: " + ', '.join(f'{k} {v}' for k, v in sorted(counts.items()))
                 + (f'; INCOMPLETE: {len(only)} configs in one arm only' if only else ''))
    text = '\n'.join(lines)
    if md:
        open(md, 'w').write(text + '\n')
    print(text if not md else lines[-1])
    if js:
        json.dump(res, open(js, 'w'), indent=1)
    return 1 if regress or (only and not baseline) else 0


# Regression gate (scripts/checks/perfgate.sh, docs/notes/performance/budget.md): the
# metrics that fail the gate (times the change can make worse) and the ones
# only reported (spikes, process totals, memory, loads).
GATE_FAIL = ['gpu_med', 'draw_cpu_p10', 'cpu_render', 'cpu_main']
GATE_INFO = ['gpu_p95', 'cpu_proc', 'vram_mb', 'ram_mb', 'load_ms']


def cmd_gate(d, base=None, new=None, baseline=None, md=None, js=None, tol=0.03):
    data = load_dir(d)
    if new not in data:
        sys.exit(f'no runs of label {new} in {d}')
    if baseline:
        b = json.load(open(baseline))
        bl = b.get(base) if base and base in b else next(iter(b.values()))
        arm_a = {cfg: {m: v['values'] for m, v in c['metrics'].items()} for cfg, c in bl.items()}
        title = f'{new} vs the stored baseline {baseline} (indicative only: another time and machine load)'
    else:
        if base not in data:
            sys.exit(f'no runs of label {base} in {d}')
        arm_a = {cfg: {m: values(runs, m) for m in metrics_of(runs)} for cfg, runs in data[base].items()}
        title = f'{new} vs {base} (interleaved A/B)'
    global METRICS
    saved = METRICS
    METRICS = {k: (v[0], max(v[1], tol) if k in GATE_FAIL else v[1], v[2], v[3]) for k, v in saved.items()}
    try:
        res, fails, warns = [], [], []
        cols = GATE_FAIL + GATE_INFO
        head = '| config | n | ' + ' | '.join(cols) + ' |'
        lines = [f'# perfgate: {title}\n',
                 f'Each cell: base -> new median, change, verdict (p, needed %). FAIL metrics ({", ".join(GATE_FAIL)}): '
                 f'REGRESSION when Mann-Whitney p < {ALPHA} and |change| > max({100 * tol:.0f} %, tolerance, 2 x noise) and > floor. '
                 f'The others ({", ".join(GATE_INFO)}) are reported, never fail.\n', head, '|' + '---|' * (len(cols) + 2)]
        short = {'same': 'ok', 'not significant': 'n.s.', 'improvement': 'better', 'REGRESSION': 'REGRESSION'}
        # Incomplete is a failure, not a pass: every config the bench set out to run, with its reps in
        # both arms (stored-baseline mode: the new arm's reps, and the config in the baseline).
        missing = [f'unreadable run file {e}' for e in LOAD_ERRORS]
        plan = expected(d)
        if plan is None:
            missing.append('no "expect" line in bench.log (the bench did not record what it set out to run)')
            cfgs = sorted(set(data[new]) | set(arm_a))
        else:
            reps, _, cfgs = plan
            for cfg in cfgs:
                nn = len(data[new].get(cfg, []))
                if nn < reps:
                    missing.append(f'{cfg}: {nn} of {reps} {new} runs')
                if baseline:
                    if cfg not in arm_a:
                        missing.append(f'{cfg}: not in the baseline')
                else:
                    nb = len(data[base].get(cfg, []))
                    if nb < reps:
                        missing.append(f'{cfg}: {nb} of {reps} {base} runs')
        for cfg in cfgs:
            if cfg not in arm_a or cfg not in data[new]:
                lines.append(f'| {cfg} | – | MISSING ({"no base runs" if cfg not in arm_a else "no new runs"}) |' + ' |' * (len(cols) - 1))
                if plan is None:
                    missing.append(f'{cfg}: run by one arm only')
                continue
            runs = data[new][cfg]
            cells, n = [], None
            for m in cols:
                x, y = arm_a[cfg].get(m, []), values(runs, m)
                if len(x) < 2 or len(y) < 2:
                    cells.append('MISSING' if m in GATE_FAIL else '–')
                    if m in GATE_FAIL:
                        missing.append(f'{cfg} {m}: {len(x)} base / {len(y)} new values')
                    continue
                r = verdict(m, x, y)
                r['config'] = cfg
                r['gate'] = 'fail' if m in GATE_FAIL else 'info'
                res.append(r)
                n = r['n']
                v = short[r['verdict']]
                if r['verdict'] == 'REGRESSION':
                    (fails if m in GATE_FAIL else warns).append(f"{cfg} {m} {100 * r['rel']:+.1f} % (p {r['p']:.3f})")
                    v = f'**{v}**' if m in GATE_FAIL else 'worse (info)'
                cells.append(f"{fmt(r['base'])}->{fmt(r['new'])} {100 * r['rel']:+.1f}% {v} ({r['p']:.3f}, {100 * r['need']:.0f})")
            lines.append(f'| {cfg} | {n[0] if n else "–"}+{n[1] if n else "–"} | ' + ' | '.join(cells) + ' |')
        lines.append('')
        if fails:
            verdict_line = f'PERFGATE: FAIL ({len(fails)} regressions: ' + '; '.join(fails) + ')'
        elif missing:
            verdict_line = f'PERFGATE: FAIL (incomplete, {len(missing)}: ' + '; '.join(missing[:12]) + ('; …' if len(missing) > 12 else '') + ')'
        else:
            verdict_line = 'PERFGATE: pass (no significant regression > 3 %)'
        if warns:
            lines.append('worse, reported only: ' + '; '.join(warns))
        # Configs the bench left out (perf.sh gate_skipped): named in the verdict, never counted as passed.
        sk = skipped(d)
        if sk:
            verdict_line += f' (skipped {len(sk)}: ' + '; '.join(sk) + ')'
        lines.append(verdict_line)
        text = '\n'.join(lines)
        if md:
            open(md, 'w').write(text + '\n')
        print(text)
        if js:
            json.dump(res, open(js, 'w'), indent=1)
    finally:
        METRICS = saved
    return 1 if fails or missing else 0


def cmd_top(d, label=None, n=8):
    data = load_dir(d)
    for lab in sorted(data):
        if label and lab != label:
            continue
        for cfg in sorted(data[lab]):
            runs = data[lab][cfg]
            if not runs or not runs[0]['gpu_parts'] and not runs[0]['cpu_parts']:
                continue
            gm = st.median(values(runs, 'gpu_med')) if values(runs, 'gpu_med') else float('nan')
            print(f'\n### {lab} {cfg} (n={len(runs)}): GPU busy {fmt(gm)} ms, main CPU {fmt(st.median(values(runs, "cpu_main")) if values(runs, "cpu_main") else None)} ms, render workers {fmt(st.median(values(runs, "cpu_render")) if values(runs, "cpu_render") else None)} ms')
            for key, title in (('gpu_parts', 'GPU passes'), ('cpu_parts', 'CPU parts (main thread)')):
                names = set().union(*[r[key].keys() for r in runs])
                med = {k: st.median([r[key].get(k, 0.0) for r in runs]) for k in names if k != '(between submissions)'}
                tot = sum(med.values()) if key == 'gpu_parts' else None
                top = sorted(med.items(), key=lambda kv: -kv[1])[:n]
                share = (lambda v: f' ({100 * v / tot:.0f} %)') if tot else (lambda v: '')
                print(f'{title}: ' + '; '.join(f'{k} {v:.3f}{share(v)}' for k, v in top))
    return 0


def cmd_selftest():
    # Exact U distribution: n=m=3 has C(6,3)=20 arrangements.
    assert sum(_u_counts(3, 3)) == 20
    p, u = mannwhitney([1, 2, 3], [4, 5, 6])
    assert abs(p - 0.1) < 1e-9 and u == 9, (p, u)
    p, _ = mannwhitney([1, 2, 3, 4, 5], [6, 7, 8, 9, 10])
    assert abs(p - 2 / 252) < 1e-9, p
    p, _ = mannwhitney([1, 3, 5, 7, 9], [2, 4, 6, 8, 10])
    assert p > 0.5, p
    # Ties fall back to the normal approximation and stay in [0, 1].
    p, _ = mannwhitney([1, 1, 2, 2], [2, 3, 3, 4])
    assert 0 < p < 1
    # Verdicts: identical arms are "same"; a clear 10 % shift with 5 runs
    # each is a regression; a 1 % shift under 2 % tolerance is "same".
    base = [1.00, 1.01, 0.99, 1.00, 1.02]
    assert verdict('gpu_med', base, base)['verdict'] == 'same'
    assert verdict('gpu_med', base, [x * 1.10 for x in base])['verdict'] == 'REGRESSION'
    assert verdict('gpu_med', base, [x * 0.90 for x in base])['verdict'] == 'improvement'
    assert verdict('gpu_med', base, [x * 1.01 for x in base])['verdict'] == 'same'
    # Noise gates the flag: a 4 % shift with ~6 % noise is not flagged.
    noisy = [1.0, 1.06, 0.94, 1.03, 0.97]
    assert verdict('gpu_med', noisy, [x * 1.04 for x in noisy])['verdict'] != 'REGRESSION'
    assert parse_transitions('free roam -> race: load 199 ms, with its first frame 244 ms; x\n') == {'load: free roam -> race': 244.0}
    assert abs(pct([1, 2, 3, 4], 0.5) - 2.5) < 1e-12
    t = ("bench racing: load 316 ms, first frame 57.3 ms, 120 frames\n  process cpu (all threads) ms/frame: 2.67\n"
         "  update   ms/frame: mean   0.10  median   0.09  p95   0.20  p99   0.30  max   0.40  (cpu 0.08)\n"
         "  gpu busy ms/frame: mean   0.71  median   0.70  p95   0.80  p99   0.90  max   1.00  (timestamps, 120 frames)\n")
    m = text_metrics(t)
    assert m['gpu_med'] == 0.70 and m['gpu_p95'] == 0.80 and m['update_cpu'] == 0.08 and m['load_ms'] == 316 and m['cpu_proc'] == 2.67, m
    # The gate's 3 % floor: a clean 2.5 % shift is not a gate failure.
    global METRICS
    saved = METRICS
    METRICS = {k: (v[0], max(v[1], 0.03) if k in GATE_FAIL else v[1], v[2], v[3]) for k, v in saved.items()}
    try:
        tight = [1.000, 1.001, 0.999, 1.000, 1.002]
        assert verdict('gpu_med', tight, [x * 1.025 for x in tight])['verdict'] == 'same'
        assert verdict('gpu_med', tight, [x * 1.04 for x in tight])['verdict'] == 'REGRESSION'
    finally:
        METRICS = saved
    print('perfstat selftest: ok')
    return 0


def main(a):
    if not a:
        print(__doc__)
        return 2
    cmd, rest = a[0], a[1:]
    opt = {}
    pos = []
    i = 0
    while i < len(rest):
        if rest[i].startswith('--'):
            opt[rest[i][2:]] = rest[i + 1]
            i += 2
        else:
            pos.append(rest[i])
            i += 1
    if cmd == 'selftest':
        return cmd_selftest()
    if cmd == 'summary':
        return cmd_summary(pos[0], opt.get('label'), opt.get('md'), opt.get('json'))
    if cmd == 'compare':
        return cmd_compare(pos[0], opt.get('base'), opt.get('new'), opt.get('baseline'), opt.get('md'), opt.get('json'))
    if cmd == 'gate':
        return cmd_gate(pos[0], opt.get('base'), opt.get('new'), opt.get('baseline'), opt.get('md'), opt.get('json'), float(opt.get('tol', 0.03)))
    if cmd == 'top':
        return cmd_top(pos[0], opt.get('label'), int(opt.get('n', 8)))
    print(__doc__)
    return 2


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
