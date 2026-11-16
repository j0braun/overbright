#!/usr/bin/env python3
"""Rate gate v2 summary (docs/checks.md "Rate gate"): the "number to move".
usage: ratetable.py FLOW.json ...   (ratediff.py --json files)

Prints one table: a row per flow and rate, a column per state-hash column; a cell is "=" (identical to the reference
rate over the whole run), the first differing tick, "end@N" (identical until one run stops), "ndN" (the column also
differs between two runs at the reference rate: not deterministic) or "-" (not printed). Then the totals: identical
cells of all compared cells, per rate and overall."""
import json
import sys


def main(paths):
    runs = []
    for p in paths:
        try:
            runs.append(json.load(open(p)))
        except (OSError, ValueError):
            print(f'{p}: unreadable')
    cols = []
    for r in runs:
        for c in r.get('columns', []):
            if c not in cols:
                cols.append(c)
    w = {c: max(len(c), 5) for c in cols}
    for r in runs:
        for cells in r['rates'].values():
            for c, v in cells.items():
                w[c] = max(w[c], len(v))
    print('flow        rate     ' + ' '.join(c.ljust(w[c]) for c in cols) + '  same')
    tot = {}
    for r in runs:
        for hz, cells in r['rates'].items():
            same = sum(1 for v in cells.values() if v == '=')
            t = tot.setdefault(hz, [0, 0])
            t[0] += same
            t[1] += len(cells)
            print(f'{(r.get("flow") or "?"):<11} {hz:<8} ' + ' '.join(cells.get(c, '-').ljust(w[c]) for c in cols)
                  + f'  {same}/{len(cells)}')
        if r.get('control'):
            print(f'{(r.get("flow") or "?"):<11} {"ctl":<8} ' + ' '.join(r['control'].get(c, '=').ljust(w[c]) for c in cols)
                  + '  (reference run twice)')
    a = sum(t[0] for t in tot.values())
    b = sum(t[1] for t in tot.values())
    print('identical cells: ' + '; '.join(f'{hz} Hz {t[0]}/{t[1]}' for hz, t in tot.items()) + f'; total {a}/{b}')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
