#!/usr/bin/env python3
"""studio_checks.py [repo] [budget|lint|registry]: the Studio preview's three source checks
(docs/developing/checks.md "Studio checks"; settings in scripts/checks/studio-checks.txt).

  budget    a Studio package's dependency tree (`cargo tree -e normal,build`, distinct crates) stays within the
            count its `budget PACKAGE N` line gives, so a new dependency is a decision, not a drift. A budget more
            than 25 % above the real count fails too (the line only grows with a reason).
  lint      Studio's production code (crates/ob-studio*/src, test code left out) has no `dbg!`, `todo!` or
            `unimplemented!`, and no `println!`/`eprintln!` outside the files a `print-ok FILE REASON` line names
            (the programs' own output). A print-ok line for a file with no print left fails.
  registry  every command of the registry (crates/ob-studio/src/registry.rs COMMANDS) is documented in
            docs/modding/studio-cli.md and named by a test (crates/ob-studio*/tests, the `*tests*.rs` files).

Prints one `STUDIO CHECKS: <part> ok (...)` line per part; exit 0 when clean, 1 with the offences.
"""
import glob
import os
import re
import subprocess
import sys

args = [a for a in sys.argv[1:]]
REPO = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "../.."))
if args and os.path.isdir(args[0]):
    REPO = os.path.abspath(args.pop(0))
PARTS = args or ["budget", "lint", "registry"]
os.chdir(REPO)
SETTINGS = "scripts/checks/studio-checks.txt"
bad = []


def settings(key):
    out = []
    if os.path.exists(SETTINGS):
        for line in open(SETTINGS, encoding="utf-8"):
            w = line.split("#")[0].split()
            if w and w[0] == key:
                out.append(w[1:])
    return out


def read(p):
    return open(p, encoding="utf-8", errors="replace").read()


def budget():
    for w in settings("budget"):
        pkg, cap = w[0], int(w[1])
        r = subprocess.run(["cargo", "tree", "-p", pkg, "-e", "normal,build", "--prefix", "none", "--locked", "--offline"],
                           capture_output=True, text=True)
        if r.returncode:
            bad.append(f"budget: cargo tree -p {pkg} failed: {r.stderr.strip()[:200]}")
            continue
        n = len({l.split(" (")[0] for l in r.stdout.splitlines() if l.strip()})
        if n > cap:
            bad.append(f"budget: {pkg} depends on {n} crates, the budget is {cap} (a new dependency: raise the line with the reason)")
        elif cap > n * 1.25:
            bad.append(f"budget: {pkg} has {n} crates but the budget is {cap}: lower the line to at most {int(n * 1.25)}")
        else:
            print(f"STUDIO CHECKS: budget ok ({pkg} {n} of {cap})")


BAN = re.compile(r"\b(dbg|todo|unimplemented)!\s*[(\[{]")
PRINT = re.compile(r"\be?println!\s*[(\[{]")


def is_test_file(p):
    b = os.path.basename(p)
    return "tests" in p.split("/") or b == "tests.rs" or b.endswith("_tests.rs") or "tests" in b


def lint():
    allowed = {w[0]: " ".join(w[1:]) for w in settings("print-ok")}
    for k, why in allowed.items():
        if not why:
            bad.append(f"lint: print-ok {k} has no reason")
    printed = set()
    files = 0
    for p in sorted(glob.glob("crates/ob-studio*/src/**/*.rs", recursive=True)):
        if is_test_file(p):
            continue
        files += 1
        for i, line in enumerate(read(p).splitlines(), 1):
            if line.lstrip().startswith("#[cfg(test)]"):
                break
            code = re.sub(r"//.*$", "", line)
            code = re.sub(r'"(?:[^"\\]|\\.)*"', '""', code)
            if BAN.search(code):
                bad.append(f"lint: {p}:{i}: {BAN.search(code).group(0)} in production code")
            if PRINT.search(code):
                printed.add(p)
                if p not in allowed:
                    bad.append(f"lint: {p}:{i}: print in production code (a program's own output goes on a print-ok line)")
    for k in allowed:
        if k not in printed:
            bad.append(f"lint: print-ok {k}: no print left in it (remove the line)")
    if not any(b.startswith("lint") for b in bad):
        print(f"STUDIO CHECKS: lint ok ({files} files, {len(printed)} with their own output)")


def registry():
    src = read("crates/ob-studio/src/registry.rs")
    i = src.index("pub const COMMANDS")
    body = src[i:]
    names = re.findall(r'Command\s*\{\s*name:\s*"([a-z.]+)"', body)
    if len(names) < 2:
        bad.append(f"registry: read only {len(names)} commands from registry.rs (the parser lost its place)")
        return
    doc = read("docs/modding/studio-cli.md")
    tests = [p for p in glob.glob("crates/ob-studio*/**/*", recursive=True)
             if os.path.isfile(p) and p.endswith((".rs", ".json", ".txt")) and ("/tests/" in p or is_test_file(p))]
    text = "\n".join(read(p) for p in tests)
    for c in names:
        w = c.split(".")
        if "`" + " ".join(w) + "`" not in doc:
            bad.append(f"registry: `{' '.join(w)}` is not in docs/modding/studio-cli.md")
        pats = [r'"\s*' + r'"\s*,\s*"'.join(w) + r'\s*"', re.escape(c), r"\b" + r"\s+".join(w) + r"\b"]
        if not any(re.search(p, text) for p in pats):
            bad.append(f"registry: command `{c}` is run by no test")
    if not any(b.startswith("registry") for b in bad):
        print(f"STUDIO CHECKS: registry ok ({len(names)} commands documented and tested)")


for part in PARTS:
    {"budget": budget, "lint": lint, "registry": registry}[part]()
for b in bad:
    print("STUDIO CHECKS: " + b)
sys.exit(1 if bad else 0)
