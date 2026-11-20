#!/usr/bin/env python3
"""Load-speed gate statistics (docs/developing/checks.md "Load-speed gate"; scripts/checks/loadgate.sh).

    loadstat.py gate OUT --base A --new B [--md F] [--json F]   A/B verdict per mode; exit 1 when a load is slower
    loadstat.py summary OUT [--label L] [--md F] [--json F]     medians [min..max] and noise per mode and measure
    loadstat.py selftest                                        the statistics on generated data

OUT/runs/<label>__<mode>__<scene>__r<rep>.txt (the run's log), .json (NFSU2_BENCH_JSON) and .trace.json
(NFSU2_TRACE). One value per run and measure, in ms of wall time:
  totals  boot: "start -> first frame", "start -> title" (--bench boot's last line);
          explore: "load", "first frame", "load + first frame" (the bench's JSON);
          transitions: each "<what>: load .., with its first frame N ms" line's N;
  stages  "stage <span>": the wall time of every NFSU2_TRACE span of that name summed over the run (per-frame
          spans "frame: …" left out). A span missing from a run counts 0 there, so a stage that only one arm has
          (new work, or work that went away) is compared too.

The rule, per mode (warm, cold, cores) and measure; n runs per arm (at least 8; 10 by default):
  shift  = Hodges-Lehmann estimate of new - base (the median of all n*n pairwise differences)
  noise  = 1.4826 * the pooled median absolute deviation of both arms around their own medians (ms)
  limit  = max(floor, pct * the base median, NOISE_K * noise)   floors: FLOORS below; NOISE_K 2 (--noise-k)
  p      = one-sided Mann-Whitney p of "new is slower" (exact without ties, else normal with tie correction)
  SLOWER (fails the gate): shift > limit and p < 0.01 / M (Bonferroni over the M measures of the mode)
  slower, not proven (reported, passes): shift > limit, p too large: repeat with more runs
  faster: the same rule the other way (reported)
The 95 % Hodges-Lehmann interval of the shift is printed beside each line.
"""
import argparse
import glob
import json
import math
import os
import random
import re
import sys
from collections import defaultdict

# (absolute ms, fraction of the base median) per mode and kind.
FLOORS = {
    ("warm", "total"): (5.0, 0.05), ("warm", "stage"): (2.0, 0.05),
    ("cores", "total"): (5.0, 0.05), ("cores", "stage"): (2.0, 0.05),
    ("cold", "total"): (20.0, 0.05), ("cold", "stage"): (10.0, 0.05),
}
ALPHA = 0.01  # family-wise per mode (Bonferroni): about 1 false alarm in 100 gates on noise alone
MIN_RUNS = 8
RUN_RE = re.compile(r"^(?P<label>[A-Za-z0-9.-]+)__(?P<mode>[a-z0-9]+)__(?P<scene>[a-z0-9]+)__r(?P<rep>\d+)\.txt$")


# ---- reading runs --------------------------------------------------------------------------------------------

def read_trace(path):
    """Span name -> summed wall ms (complete events; per-frame spans left out)."""
    try:
        text = open(path, encoding="utf-8").read().strip()
    except OSError:
        return None
    text = text.rstrip(",")
    if not text.endswith("]"):
        text = text.rstrip(",\n") + "]"
    try:
        events = json.loads(text)
    except ValueError:
        return None
    out = defaultdict(float)
    for e in events:
        if e.get("ph") == "X" and not e.get("name", "").startswith("frame:"):
            out[e["name"]] += float(e.get("dur", 0.0)) / 1000.0
    return out


def run_values(base, scene):
    """Measures of one run (dict name -> (kind, ms)), or None when the run did not complete."""
    txt = open(base + ".txt", encoding="utf-8", errors="replace").read()
    if not re.search(r"^rc=0\b", txt, re.M):
        return None
    v = {}
    if scene == "boot":
        m = re.search(r"^bench boot: first frame (\d+) ms, title (\d+) ms after the process's start \(entry (\d+) ms", txt, re.M)
        if not m:
            return None
        v["start -> entry"] = ("total", float(m.group(3)))
        v["start -> first frame"] = ("total", float(m.group(1)))
        v["start -> title"] = ("total", float(m.group(2)))
    elif scene == "transitions":
        for m in re.finditer(r"^(.+?): load [0-9.]+ ms, with its first frame ([0-9.]+) ms", txt, re.M):
            v[m.group(1)] = ("total", float(m.group(2)))
        if not v:
            return None
    else:
        try:
            j = json.load(open(base + ".json", encoding="utf-8"))
        except (OSError, ValueError):
            return None
        v["load"] = ("total", float(j["load_ms"]))
        v["first frame"] = ("total", float(j["first_frame_ms"]))
        v["load + first frame"] = ("total", float(j["load_ms"]) + float(j["first_frame_ms"]))
    spans = read_trace(base + ".trace.json")
    if spans is None:
        return None
    for name, ms in spans.items():
        v["stage " + name] = ("stage", ms)
    return v


def load_runs(out):
    """{(label, mode, scene): [values per run]}, and the failed runs' names."""
    runs, failed = defaultdict(list), []
    for f in sorted(glob.glob(os.path.join(out, "runs", "*.txt"))):
        m = RUN_RE.match(os.path.basename(f))
        if not m or m.group("rep") == "0":
            continue
        v = run_values(f[:-4], m.group("scene"))
        if v is None:
            failed.append(os.path.basename(f)[:-4])
            continue
        runs[(m.group("label"), m.group("mode"), m.group("scene"))].append(v)
    return runs, failed


# ---- statistics ----------------------------------------------------------------------------------------------

def median(xs):
    s = sorted(xs)
    n = len(s)
    if n == 0:
        return float("nan")
    return s[n // 2] if n % 2 else 0.5 * (s[n // 2 - 1] + s[n // 2])


def noise(a, b):
    ma, mb = median(a), median(b)
    return 1.4826 * median([abs(x - ma) for x in a] + [abs(x - mb) for x in b])


def u_counts(n, m):
    """Number of arrangements with each Mann-Whitney U value, 0..n*m (no ties)."""
    # f[i][j] = counts for i, j; built with the recursion f(i,j,u) = f(i-1,j,u-j) + f(i,j-1,u).
    prev = [[1] for _ in range(m + 1)]  # i = 0: U = 0 only
    for i in range(1, n + 1):
        cur = [[1]]  # j = 0
        for j in range(1, m + 1):
            a = prev[j]           # f(i-1, j, ·) shifted by j
            b = cur[j - 1]        # f(i, j-1, ·)
            size = i * j + 1
            row = [0] * size
            for u, c in enumerate(a):
                if u + j < size:
                    row[u + j] += c
            for u, c in enumerate(b):
                row[u] += c
            cur.append(row)
        prev = cur
    return prev[m]


_UC = {}


def u_dist(n, m):
    if (n, m) not in _UC:
        _UC[(n, m)] = u_counts(n, m)
    return _UC[(n, m)]


def p_greater(a, b):
    """One-sided p of 'b tends to be larger than a' (Mann-Whitney)."""
    n, m = len(a), len(b)
    u = sum(1.0 if y > x else 0.5 if y == x else 0.0 for x in a for y in b)  # U of b over a
    allv = a + b
    ties = len(allv) != len(set(allv))
    if not ties:
        c = u_dist(n, m)
        tot = sum(c)
        k = int(round(u))
        return sum(c[k:]) / tot
    # Normal approximation with tie correction and continuity correction.
    N = n + m
    counts = defaultdict(int)
    for x in allv:
        counts[x] += 1
    tie = sum(t ** 3 - t for t in counts.values())
    var = n * m / 12.0 * ((N + 1) - tie / (N * (N - 1)))
    if var <= 0:
        return 1.0
    z = (u - n * m / 2.0 - 0.5) / math.sqrt(var)
    return 0.5 * math.erfc(z / math.sqrt(2))


def hl(a, b):
    """Hodges-Lehmann shift (b - a) and its 95 % interval."""
    d = sorted(y - x for x in a for y in b)
    n, m = len(a), len(b)
    est = median(d)
    c = u_dist(n, m)
    tot = sum(c)
    # k: the largest count with P(U <= k - 1) <= 0.025; the interval is d[k-1] .. d[nm-k].
    cum, k = 0, 0
    for u, cnt in enumerate(c):
        if (cum + cnt) / tot > 0.025:
            k = u
            break
        cum += cnt
    k = max(k, 1)
    return est, d[k - 1], d[n * m - k]


def judge(mode, kind, a, b, M, noise_k):
    floor_abs, pct = FLOORS.get((mode, kind), (5.0, 0.05))
    base_med = median(a)
    nz = noise(a, b)
    limit = max(floor_abs, pct * abs(base_med), noise_k * nz)
    est, lo, hi = hl(a, b)
    p_slow, p_fast = p_greater(a, b), p_greater(b, a)
    thr = ALPHA / max(M, 1)
    if est > limit and p_slow < thr:
        verdict = "SLOWER"
    elif est > limit:
        verdict = "slower, not proven"
    elif -est > limit and p_fast < thr:
        verdict = "faster"
    else:
        verdict = "same"
    return {"verdict": verdict, "base_median": base_med, "new_median": median(b), "shift": est, "ci": [lo, hi],
            "noise": nz, "limit": limit, "p_slower": p_slow, "p_threshold": thr, "n": [len(a), len(b)]}


def measures(vals):
    names = {}
    for v in vals:
        for k, (kind, _) in v.items():
            names[k] = kind
    return names


def series(vals, name):
    return [v[name][1] if name in v else 0.0 for v in vals]


# ---- commands ------------------------------------------------------------------------------------------------

def cmd_gate(args):
    runs, failed = load_runs(args.out)
    modes = sorted({k[1] for k in runs if k[0] in (args.base, args.new)}, key=lambda m: ["warm", "cold", "cores"].index(m) if m in ("warm", "cold", "cores") else 9)
    report, lines, slower_all, unsure_all = {"modes": {}, "failed_runs": failed}, [], [], []
    bad = bool(failed)
    md = [f"# Load-speed gate: {args.base} (base) vs {args.new} (new)", ""]
    if failed:
        md.append(f"Failed runs (count as a failure): {', '.join(failed)}")
    for mode in modes:
        scenes = sorted({k[2] for k in runs if k[1] == mode})
        rows = []
        for scene in scenes:
            a, b = runs.get((args.base, mode, scene), []), runs.get((args.new, mode, scene), [])
            if len(a) < MIN_RUNS or len(b) < MIN_RUNS:
                bad = True
                lines.append(f"loadgate: {mode} {scene}: too few runs (base {len(a)}, new {len(b)}; at least {MIN_RUNS} each)")
                continue
            names = measures(a + b)
            for name, kind in names.items():
                rows.append((scene, name, kind, series(a, name), series(b, name)))
        M = len(rows)
        res = []
        for scene, name, kind, a, b in rows:
            r = judge(mode, kind, a, b, M, args.noise_k)
            r.update(scene=scene, measure=name, kind=kind)
            res.append(r)
        report["modes"][mode] = res
        md += [f"## {mode} ({M} measures; p threshold {ALPHA}/{M})", "",
               "| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |",
               "|---|---|---|---|---|---|---|---|---|"]
        for r in sorted(res, key=lambda r: (r["scene"], r["kind"] != "total", -r["shift"])):
            if r["kind"] == "stage" and r["verdict"] == "same" and max(r["base_median"], r["new_median"]) < 1.0:
                continue
            md.append(f"| {r['scene']} | {r['measure']} | {r['base_median']:.1f} | {r['new_median']:.1f} | "
                      f"{r['shift']:+.1f} [{r['ci'][0]:+.1f}..{r['ci'][1]:+.1f}] | {r['noise']:.1f} | {r['limit']:.1f} | "
                      f"{r['p_slower']:.2g} | {r['verdict']} |")
        md.append("")
        slow = sorted([r for r in res if r["verdict"] == "SLOWER"], key=lambda r: -r["shift"])
        unsure = [r for r in res if r["verdict"] == "slower, not proven"]
        fast = [r for r in res if r["verdict"] == "faster"]
        tot = [r for r in res if r["kind"] == "total"]
        desc = "; ".join(f"{r['scene']} {r['measure']} {r['base_median']:.0f} -> {r['new_median']:.0f} ms" for r in tot)
        if slow:
            bad = True
            # Stages nest (drive_in holds World::load holds its cache read): the stage named first is the innermost
            # one that carries the slowdown, i.e. among the stages with at least 80 % of the largest shift the one
            # with the smallest base time; then the rest by shift.
            st = [r for r in slow if r["kind"] == "stage"]
            if st:
                top = max(r["shift"] for r in st)
                inner = min((r for r in st if r["shift"] >= 0.8 * top), key=lambda r: r["base_median"])
                st = [inner] + [r for r in st if r is not inner]
            tt = [r for r in slow if r["kind"] == "total"]
            named = ", ".join(f"{r['scene']} {r['measure'][6:]} {r['shift']:+.1f} ms ({100 * r['shift'] / max(r['base_median'], 1e-9):+.0f} %)" for r in st[:4]) or "no single stage (spread over the load)"
            tl = ", ".join(f"{r['scene']} {r['measure']} {r['shift']:+.1f} ms" for r in tt) or "no total beyond its limit"
            lines.append(f"loadgate: {mode}: SLOWER: totals: {tl}; slower stage: {named}")
            slower_all.append(f"{mode}: " + (st[0]['scene'] + ' ' + st[0]['measure'][6:] if st else tt[0]['scene'] + ' ' + tt[0]['measure']))
        else:
            lines.append(f"loadgate: {mode}: same ({desc})")
        if unsure:
            unsure_all += [f"{mode} {r['scene']} {r['measure']}" for r in unsure]
            lines.append(f"loadgate: {mode}: slower, not proven (repeat with more runs): " + ", ".join(f"{r['scene']} {r['measure']} {r['shift']:+.1f} ms (p {r['p_slower']:.2g})" for r in unsure[:4]))
        if fast:
            lines.append(f"loadgate: {mode}: faster: " + ", ".join(f"{r['scene']} {r['measure']} {r['shift']:+.1f} ms" for r in sorted(fast, key=lambda r: r['shift'])[:4]))
    if not modes:
        bad = True
        lines.append(f"loadgate: no runs of {args.base} and {args.new} in {args.out}/runs")
    if failed:
        lines.append(f"loadgate: {len(failed)} runs failed: {' '.join(failed[:5])}")
    verdict = "FAIL" if bad else "PASS"
    why = ("slower: " + "; ".join(slower_all)) if slower_all else ("runs missing or failed" if bad else "no load slower beyond its noise limit")
    lines.append(f"LOADGATE: {verdict}: {why}" + (f" ({len(unsure_all)} not proven)" if unsure_all else ""))
    print("\n".join(lines))
    report["verdict"] = verdict
    if args.md:
        open(args.md, "w").write("\n".join(md + lines) + "\n")
    if args.json:
        json.dump(report, open(args.json, "w"), indent=1)
    return 1 if bad else 0


def cmd_summary(args):
    runs, failed = load_runs(args.out)
    out = {}
    md = ["# Load times (ms of wall time, median [min..max]; noise = 1.4826 MAD)", ""]
    for (label, mode, scene), vals in sorted(runs.items()):
        if args.label and label != args.label:
            continue
        names = measures(vals)
        md += [f"## {label} {mode} {scene} ({len(vals)} runs)", "", "| measure | median [min..max] | noise |", "|---|---|---|"]
        rows = {}
        for name in sorted(names, key=lambda n: (names[n] != "total", -median(series(vals, n)))):
            s = series(vals, name)
            med = median(s)
            if names[name] == "stage" and med < 1.0:
                continue
            nz = 1.4826 * median([abs(x - med) for x in s])
            rows[name] = {"median": med, "min": min(s), "max": max(s), "noise": nz, "kind": names[name]}
            md.append(f"| {name} | {med:.1f} [{min(s):.1f}..{max(s):.1f}] | {nz:.1f} |")
        md.append("")
        out[f"{label}/{mode}/{scene}"] = rows
    if failed:
        md.append(f"Failed runs: {' '.join(failed)}")
    text = "\n".join(md) + "\n"
    if args.md:
        open(args.md, "w").write(text)
    else:
        sys.stdout.write(text)
    if args.json:
        json.dump(out, open(args.json, "w"), indent=1)
    return 0


def cmd_selftest(_args):
    ok = True

    def check(cond, what):
        nonlocal ok
        print(("ok   " if cond else "FAIL ") + what)
        ok &= bool(cond)

    # Exact distribution: n = m = 5 has 252 arrangements, P(U >= 25) = 1/252; n = m = 10: 184756.
    check(sum(u_dist(5, 5)) == 252 and u_dist(5, 5)[25] == 1, "U distribution 5+5")
    check(sum(u_dist(10, 10)) == 184756, "U distribution 10+10")
    a = [float(x) for x in range(10)]
    b = [x + 100 for x in a]
    check(abs(p_greater(a, b) - 1 / 184756) < 1e-12, "smallest one-sided p 10+10")
    est, lo, hi = hl(a, b)
    check(est == 100 and lo <= 100 <= hi, "Hodges-Lehmann shift of a constant offset")
    # A/A: 500 gates of 60 measures on heavy-tailed noise; the share that flags anything must stay small.
    rng = random.Random(7)
    flagged = 0
    for _ in range(500):
        any_flag = False
        for _m in range(60):
            base = rng.uniform(1, 300)
            sd = base * rng.uniform(0.01, 0.3)
            xa = [base + rng.gauss(0, sd) * (3 if rng.random() < 0.1 else 1) for _ in range(10)]
            xb = [base + rng.gauss(0, sd) * (3 if rng.random() < 0.1 else 1) for _ in range(10)]
            if judge("warm", "stage", xa, xb, 60, 2.0)["verdict"] == "SLOWER":
                any_flag = True
        flagged += any_flag
    check(flagged <= 10, f"A/A false alarms {flagged} of 500 gates (60 measures each; the family-wise rate is 1 %)")
    # A planted 25 ms on a 70 ms stage with 2 ms noise is found; 25 ms on a 250 ms total with 40 ms noise is not
    # claimed as proven by the total alone.
    xa = [70 + rng.gauss(0, 2) for _ in range(10)]
    xb = [95 + rng.gauss(0, 2) for _ in range(10)]
    check(judge("warm", "stage", xa, xb, 60, 2.0)["verdict"] == "SLOWER", "planted 25 ms on a quiet stage: SLOWER")
    xa = [250 + rng.gauss(0, 40) for _ in range(10)]
    xb = [275 + rng.gauss(0, 40) for _ in range(10)]
    check(judge("warm", "total", xa, xb, 60, 2.0)["verdict"] != "SLOWER", "25 ms on a 40 ms-noise total: not claimed")
    # A stage only the new arm has (absent = 0 in the base) is compared as such.
    check(judge("warm", "stage", [0.0] * 10, [30 + rng.gauss(0, 1) for _ in range(10)], 60, 2.0)["verdict"] == "SLOWER", "a new 30 ms stage: SLOWER")
    print("SELFTEST:", "pass" if ok else "FAIL")
    return 0 if ok else 1


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sp = ap.add_subparsers(dest="cmd", required=True)
    g = sp.add_parser("gate")
    g.add_argument("out")
    g.add_argument("--base", required=True)
    g.add_argument("--new", required=True)
    g.add_argument("--md")
    g.add_argument("--json")
    g.add_argument("--noise-k", type=float, default=float(os.environ.get("NOISE_K", "2")))
    s = sp.add_parser("summary")
    s.add_argument("out")
    s.add_argument("--label")
    s.add_argument("--md")
    s.add_argument("--json")
    sp.add_parser("selftest")
    a = ap.parse_args()
    return {"gate": cmd_gate, "summary": cmd_summary, "selftest": cmd_selftest}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
