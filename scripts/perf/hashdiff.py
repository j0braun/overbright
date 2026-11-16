#!/usr/bin/env python3
"""First divergence of two NFSU2_STATE_HASH files (base A, new B): the step and the columns that differ.
usage: hashdiff.py A.tsv B.tsv [POLICY]

Columns are matched by name; a column only one side prints (an older base binary prints the eight legacy columns,
a newer one the v2 columns too) is left out and the line says so. POLICY (default scripts/checks/rate-columns.txt):
its "parity" field per column: fail = a difference fails (exit 1), report = shown only, skip = not compared.
Columns the policy does not list count as fail."""
import os
import sys


def policy(path, field=2):
    out = {}
    try:
        for line in open(path):
            p = line.split('#')[0].split()
            if len(p) >= 3:
                out[p[0]] = p[field]
    except OSError:
        pass
    return out


def main(argv):
    if len(argv) < 2:
        print(__doc__)
        return 2
    pol = policy(argv[2] if len(argv) > 2 else os.path.join(os.path.dirname(__file__), '..', 'checks', 'rate-columns.txt'))
    a = open(argv[0]).read().splitlines()
    b = open(argv[1]).read().splitlines()
    ha, hb = a[0].split('\t'), b[0].split('\t')
    common = [c for c in ha if c in hb and c != 'step' and pol.get(c, 'fail') != 'skip']
    only = [c for c in ha if c not in hb] + [c for c in hb if c not in ha]
    ia = {c: ha.index(c) for c in common}
    ib = {c: hb.index(c) for c in common}
    first = {}
    for i, (x, y) in enumerate(zip(a[1:], b[1:])):
        if x == y:
            continue
        fx, fy = x.split('\t'), y.split('\t')
        for c in common:
            if c not in first and (fx[ia[c]] if ia[c] < len(fx) else None) != (fy[ib[c]] if ib[c] < len(fy) else None):
                first[c] = i
        if len(first) == len(common):
            break
    note = f' (columns compared: {len(common)}; only one side prints {", ".join(only)})' if only else ''
    failing = sorted((s, c) for c, s in first.items() if pol.get(c, 'fail') == 'fail' and c != 'all')
    reported = sorted((s, c) for c, s in first.items() if pol.get(c, 'fail') != 'fail')
    rep = f'; report-only columns differ: {", ".join(f"{c}@{s}" for s, c in reported)}' if reported else ''
    if failing:
        s0 = failing[0][0]
        print(f'step {s0}: {", ".join(c for s, c in failing if s == s0)} differ{note}{rep}')
        return 1
    if len(a) != len(b):
        print(f'same for {min(len(a), len(b)) - 1} steps, then one run stops ({len(a) - 1} vs {len(b) - 1} steps){note}{rep}')
        return 1
    print(f'identical, {len(a) - 1} steps{" in the counted columns" if reported else ""}{note}{rep}')
    return 0


if __name__ == '__main__':
    sys.exit(main(sys.argv[1:]))
