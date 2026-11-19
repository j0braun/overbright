#!/usr/bin/env python3
"""Provenance tags (docs/developing/provenance.md): every code site resting on an
inferred or open value carries a machine-checked tag

    // prov: inferred <ID>      (also open, choice)

and every <ID> is a row of docs/provenance.tsv (id, kind, doc, summary).
Fails (exit 1) on: a tag whose id is not registered, a registered id with no
tag in the code, a tag whose kind differs from its row, duplicate rows, a
malformed tag or row, a row's doc that does not exist.

    scripts/checks/provenance.py [repo]            check, print the counts
    scripts/checks/provenance.py [repo] --list     also every tag with its site
    scripts/checks/provenance.py [repo] --untagged the **inferred** / **open**
                                                   comment lines without a tag
"""
import os
import re
import sys

KINDS = ("inferred", "open", "choice")
TAG = re.compile(r"//[/!]?\s*prov:\s*(\S+)\s+(\S+)\s*$")
LOOSE = re.compile(r"//[/!]?\s*prov:")
ID = re.compile(r"^[a-z0-9_]+(\.[A-Za-z0-9_]+)+(-[0-9]+)?$")
MARK = re.compile(r"\*\*(inferred|open)\*\*", re.I)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    repo = os.path.abspath(args[0] if args else os.path.join(os.path.dirname(__file__), "..", ".."))
    errors = []
    reg = {}
    path = os.path.join(repo, "docs", "provenance.tsv")
    with open(path, encoding="utf-8") as f:
        for n, line in enumerate(f, 1):
            line = line.rstrip("\n")
            if not line or line.startswith("#"):
                continue
            cols = line.split("\t")
            if len(cols) != 4:
                errors.append(f"provenance.tsv:{n}: {len(cols)} columns, want 4 (id, kind, doc, summary)")
                continue
            pid, kind, doc, summary = cols
            if not ID.match(pid):
                errors.append(f"provenance.tsv:{n}: bad id {pid!r}")
            if kind not in KINDS:
                errors.append(f"provenance.tsv:{n}: {pid}: kind {kind!r} not one of {KINDS}")
            if pid in reg:
                errors.append(f"provenance.tsv:{n}: {pid} registered twice")
            for d in doc.split(","):
                if not os.path.isfile(os.path.join(repo, d.split("#")[0])):
                    errors.append(f"provenance.tsv:{n}: {pid}: doc {d} not found")
            if not summary.strip():
                errors.append(f"provenance.tsv:{n}: {pid}: empty summary")
            reg[pid] = kind
    tags = {}
    untagged = []
    # The library's source: src/ and the workspace crates' src/ (crates/<name>/src).
    roots = [os.path.join(repo, "src")]
    crates = os.path.join(repo, "crates")
    if os.path.isdir(crates):
        roots += [os.path.join(crates, c, "src") for c in sorted(os.listdir(crates)) if os.path.isdir(os.path.join(crates, c, "src"))]
    for root, dirs, files in (w for r in roots for w in os.walk(r)):
        dirs.sort()
        for name in sorted(files):
            if not name.endswith(".rs"):
                continue
            p = os.path.join(root, name)
            rel = os.path.relpath(p, repo)
            with open(p, encoding="utf-8", errors="replace") as f:
                lines = f.read().splitlines()
            for i, line in enumerate(lines):
                if LOOSE.search(line):
                    m = TAG.search(line)
                    if not m or m.group(1) not in KINDS or not ID.match(m.group(2)):
                        errors.append(f"{rel}:{i + 1}: malformed tag: {line.strip()}")
                        continue
                    kind, pid = m.group(1), m.group(2)
                    tags.setdefault(pid, []).append((rel, i + 1, kind))
                elif "//" in line and MARK.search(line) and not (i > 0 and LOOSE.search(lines[i - 1])):
                    untagged.append(f"{rel}:{i + 1}: {line.strip()[:110]}")
    for pid, sites in sorted(tags.items()):
        if pid not in reg:
            errors.append(f"{sites[0][0]}:{sites[0][1]}: tag {pid} is not in docs/provenance.tsv")
            continue
        for rel, n, kind in sites:
            if kind != reg[pid]:
                errors.append(f"{rel}:{n}: tag {pid} says {kind}, docs/provenance.tsv says {reg[pid]}")
    for pid in sorted(reg):
        if pid not in tags:
            errors.append(f"docs/provenance.tsv: {pid} has no tag in the source (resolved? remove the row, or tag its site)")
    counts = {k: sum(1 for v in reg.values() if v == k) for k in KINDS}
    nsites = sum(len(v) for v in tags.values())
    if "--list" in sys.argv:
        for pid, sites in sorted(tags.items()):
            for rel, n, kind in sites:
                print(f"{kind}\t{pid}\t{rel}:{n}")
    if "--untagged" in sys.argv:
        print("\n".join(untagged))
    for e in errors:
        print("provenance:", e)
    print(f"provenance: {len(reg)} ids ({counts['inferred']} inferred, {counts['open']} open, {counts['choice']} choice), "
          f"{nsites} tagged sites, {len(untagged)} untagged **inferred**/**open** comment lines; {len(errors)} errors")
    print("PROVENANCE: PASS" if not errors else "PROVENANCE: FAIL")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
