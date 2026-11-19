#!/usr/bin/env python3
"""Rate gate v2 (docs/developing/checks.md "Rate gate"): NFSU2_STATE_HASH tables of ONE flow run at several display rates,
compared ACROSS rates against a reference rate, column by column.

usage: ratediff.py [--policy FILE] [--json OUT.json] [--flow NAME] [--strict] REF HZ=TABLE.tsv [HZ=TABLE.tsv ...]
                   [ctl=TABLE.tsv]

HZ is any label (30, 60, 120, 200, jitter:7 ...). "ctl" is a second run at the reference rate (the control): a column
that differs between the reference and the control is not deterministic run to run; its cells are marked "nd".
Columns are matched by name (tables from older binaries have fewer); the policy file (scripts/checks/
rate-columns.txt, field "rate") says per column: fail (a difference counts), report (shown only), skip (left out).
A line "COLUMN@FLOW" overrides the column's line for that flow (--flow); its rule "fail:R1,R2" counts a
difference only at those schedules (e.g. fail:120,200,jitter:7), report elsewhere. In a counted column a difference that is
only the length ("end@N": one run shorter, the common ticks identical) counts at every schedule, a jittered one included:
the runs end on a tick count (the benches' NFSU2_RATE_TICKS, the flows' own tick-keyed ends), so a jittered run whose
stalls drop real time beyond the tick clock's 3-tick cap still covers the same ticks.
--strict counts every compared column. Prints one line: per rate its step count and a digest, then per rate the
number of identical columns and the first differing tick of each column that differs ("col@tick", "col:end@N" when
the tables agree until one stops). --json writes the cells for ratetable.py. Exit 1 when a counted column differs, a
table is missing or empty, or the reference is missing.
"""
import hashlib
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))


def policy(path):
    out = {}
    try:
        for line in open(path):
            p = line.split('#')[0].split()
            if len(p) >= 3:
                out[p[0]] = p[1]
    except OSError:
        pass
    return out


def load(path):
    try:
        lines = open(path).read().splitlines()
    except OSError:
        return None, None
    if len(lines) < 2:
        return None, None
    return lines[0].split('\t'), [l.split('\t') for l in lines[1:]]


def digest(rows):
    h = hashlib.sha256()
    for r in rows:
        h.update(('\t'.join(r[1:]) + '\n').encode())
    return h.hexdigest()[:12]


def first_diffs(ha, ra, hb, rb, cols):
    """Per column: None (identical over every step and the same length), ('tick', i) or ('end', n)."""
    ia = {c: ha.index(c) for c in cols}
    ib = {c: hb.index(c) for c in cols}
    out = {c: None for c in cols}
    left = set(cols)
    for i, (x, y) in enumerate(zip(ra, rb)):
        if not left:
            break
        for c in list(left):
            if (x[ia[c]] if ia[c] < len(x) else None) != (y[ib[c]] if ib[c] < len(y) else None):
                out[c] = ('tick', i)
                left.discard(c)
    if len(ra) != len(rb):
        for c in left:
            out[c] = ('end', min(len(ra), len(rb)))
    return out


def cell(d, nd):
    if d is None:
        return '='
    s = f'{d[1]}' if d[0] == 'tick' else f'end@{d[1]}'
    return f'nd{s}' if nd else s


def main(argv):
    pol_path = os.path.join(HERE, '..', 'checks', 'rate-columns.txt')
    out_json = flow = None
    strict = False
    while argv and argv[0].startswith('--'):
        a = argv.pop(0)
        if a == '--policy':
            pol_path = argv.pop(0)
        elif a == '--json':
            out_json = argv.pop(0)
        elif a == '--flow':
            flow = argv.pop(0)
        elif a == '--strict':
            strict = True
        else:
            print(__doc__)
            return 2
    if len(argv) < 2:
        print(__doc__)
        return 2
    pol = policy(pol_path)
    ref = argv[0]
    tabs = {}
    for a in argv[1:]:
        hz, _, path = a.partition('=')
        tabs[hz] = load(path)
    bad = 0
    parts = []
    for hz, (head, rows) in tabs.items():
        if not rows:
            parts.append(f'{hz} Hz MISSING')
            bad += 1
        else:
            parts.append(f'{hz} Hz {len(rows)} steps #{digest(rows)}')
    rh, rr = tabs.get(ref, (None, None))
    res = {'flow': flow, 'ref': ref, 'rates': {}, 'columns': []}
    diffs = []
    if not rr:
        diffs.append(f'no reference table at {ref} Hz')
        bad += 1
    else:
        cols = [c for c in rh if c != 'step' and pol.get(c, 'report') != 'skip']
        res['columns'] = cols
        nd = {}
        ch, cr = tabs.get('ctl', (None, None))
        if cr:
            ccols = [c for c in cols if c in ch]
            nd = {c: d for c, d in first_diffs(rh, rr, ch, cr, ccols).items() if d is not None}
            res['control'] = {c: cell(d, False) for c, d in nd.items()}
            diffs.append(f'control (ref twice): {"identical" if not nd else "NOT deterministic: " + ", ".join(f"{c}@{cell(d, False)}" for c, d in nd.items())}')
        for hz, (head, rows) in tabs.items():
            if hz in (ref, 'ctl') or not rows:
                continue
            common = [c for c in cols if c in head]
            fd = first_diffs(rh, rr, head, rows, common)
            cells = {c: cell(fd[c], c in nd and fd[c] is not None and (nd[c][1] <= fd[c][1] or nd[c][0] == 'end'))
                     for c in common}
            res['rates'][hz] = cells
            same = sum(1 for c in common if fd[c] is None)
            diff = [c for c in common if fd[c] is not None]
            def rule(c):
                return pol.get(f'{c}@{flow}', pol.get(c, 'report'))
            def fails(c):
                r = rule(c)
                return r == 'fail' or (r.startswith('fail:') and hz in r[5:].split(','))
            counted = [c for c in diff if strict or fails(c)]
            bad += 1 if counted else 0
            order = sorted(diff, key=lambda c: (fd[c][1], common.index(c)))
            diffs.append(f'{hz} vs {ref} Hz: {same} of {len(common)} columns identical'
                         + (f'; first: {", ".join(f"{c}@{cells[c]}" for c in order)}' if diff else '')
                         + (f' [counted: {", ".join(counted)}]' if counted else ''))
    if out_json:
        with open(out_json, 'w') as f:
            json.dump(res, f)
    print('; '.join(parts) + ' | ' + '; '.join(diffs))
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
