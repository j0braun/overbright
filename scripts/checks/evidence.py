#!/usr/bin/env python3
"""Evidence register check (docs/developing/evidence.md).

Every structured behaviour claim in the audit docs carries one tag word:
traced, observed, inferred, choice or open. Scanned:
  - docs/original/coverage/*.md: each table row whose first cell is an address (0x...), column "Evidence";
  - docs/playing/native-deviations.md section 2: each table row, column "Basis";
  - prose in docs/playing, docs/modding, docs/developing and docs/original: a paragraph that says something behaves
    "like / as in / matches the original" or names a native value needs a tag word in the same paragraph.
A row without a tag fails, unless scripts/checks/evidence-allow.txt lists it
(`file<TAB>first 40 characters of the first cell<TAB>reason`). An allow line that no longer matches an untagged row
also fails, so the list only shrinks.

    evidence.py [repo]
"""
import os, re, sys

TAGS = re.compile(r"\b(traced|observed|inferred|choice|open)\b", re.I)
repo = os.path.abspath(sys.argv[1] if len(sys.argv) > 1 else os.path.join(os.path.dirname(__file__), "..", ".."))
errors, rows = [], 0

allow = {}
ap = os.path.join(repo, "scripts", "checks", "evidence-allow.txt")
if os.path.exists(ap):
    for n, line in enumerate(open(ap, encoding="utf-8"), 1):
        line = line.rstrip("\n")
        if not line or line.startswith("#"):
            continue
        c = line.split("\t")
        if len(c) != 3 or not c[2].strip():
            errors.append(f"evidence-allow.txt:{n}: want file<TAB>first cell<TAB>reason")
        else:
            allow[(c[0], c[1])] = False


def cells(line):
    return [x.strip() for x in line.strip().strip("|").split("|")]


def scan(rel, column, want_row, section=None):
    global rows
    path = os.path.join(repo, rel)
    if not os.path.exists(path):
        errors.append(f"{rel}: missing")
        return
    hdr, inside = None, section is None
    for n, line in enumerate(open(path, encoding="utf-8"), 1):
        if section is not None and line.startswith("## "):
            inside = line.startswith(section)
        if not line.startswith("|"):
            hdr = None  # a table ends at the first line that is not a row
        if not inside or not line.startswith("|"):
            continue
        c = cells(line)
        if column in c:
            hdr = c
            continue
        if hdr is None or set("".join(c)) <= set("-: ") or not want_row(c):
            continue
        rows += 1
        if len(c) != len(hdr):
            errors.append(f"{rel}:{n}: {len(c)} cells, the header has {len(hdr)}")
            continue
        if TAGS.search(c[hdr.index(column)]):
            continue
        key = (rel, c[0][:40])
        if key in allow:
            allow[key] = True
            continue
        errors.append(f"{rel}:{n}: no evidence tag (traced, observed, inferred, choice, open) in column {column}: {c[0][:60]}")


cov = os.path.join(repo, "docs", "original", "coverage")
for f in sorted(os.listdir(cov)):
    if f.endswith(".md") and f != "README.md":
        scan(f"docs/original/coverage/{f}", "Evidence", lambda c: c[0].startswith("0x"))
scan("docs/playing/native-deviations.md", "Basis", lambda c: True, "## 2.")
# Prose: a paragraph that says the game behaves like the original needs a tag word in the same paragraph.
CLAIM = re.compile(r"(like|matches|match|same as|as in|identical to|unlike|as) (the )?(original|native|SPEED2)|native (behaviou?r|value|rate)", re.I)
PROSE_DIRS = ["docs/playing", "docs/modding", "docs/developing", "docs/original"]
prose = 0
for d in PROSE_DIRS:
    for root, _, names in os.walk(os.path.join(repo, d)):
        for nm in sorted(names):
            if not nm.endswith(".md"):
                continue
            path = os.path.join(root, nm)
            rel = os.path.relpath(path, repo)
            if rel in ("docs/developing/evidence.md", "docs/original/coverage/README.md"):
                continue
            line_no = 1
            for para in re.split(r"(\n\s*\n)", open(path, encoding="utf-8").read()):
                m = CLAIM.search(para)
                if m and not para.lstrip().startswith("#"):
                    prose += 1
                    if not TAGS.search(para):
                        key = (rel, para.strip()[:40])
                        if key in allow:
                            allow[key] = True
                        else:
                            errors.append(f"{rel}:{line_no + para[:m.start()].count(chr(10))}: native-behaviour claim without a tag (traced, observed, inferred, choice, open): ...{para[max(0, m.start() - 30):m.end() + 30].strip()!r}")
                line_no += para.count("\n")
for (f, k), used in allow.items():
    if not used:
        errors.append(f"evidence-allow.txt: {f} {k[:40]!r}: no longer an untagged row, remove the line")
for e in errors:
    print("evidence: " + e)
print(f"evidence: {rows} claim rows, {prose} prose claims, {sum(allow.values())} allowed, {len(errors)} errors")
sys.exit(1 if errors else 0)
