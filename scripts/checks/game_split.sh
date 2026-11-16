#!/bin/bash
# game_split.sh [repo]: the refactor's size report (the maintainer's private notes).
# Prints the line count of src/bin/game.rs and src/render/mod.rs, every .rs
# file under src/ longer than 1,500 lines, and every fn longer than 150 lines
# (file:line, its length). A brace-depth scanner that skips comments, strings,
# raw strings and char literals; a fn without a body (`fn f();`) is not counted.
REPO=${1:-$(cd "$(dirname "$0")/../.." && pwd)}
exec python3 - "$REPO" <<'EOF'
import os, re, sys

repo = sys.argv[1]
FILE_MAX, FN_MAX = 1500, 150


def rs_files(root):
    for d, dirs, files in os.walk(os.path.join(repo, root)):
        dirs.sort()
        for f in sorted(files):
            if f.endswith(".rs"):
                yield os.path.relpath(os.path.join(d, f), repo)


def lines_of(path):
    with open(os.path.join(repo, path), encoding="utf-8", errors="replace") as f:
        return f.read().count("\n")


IDENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*")
CHAR = re.compile(r"'(?:\\(?:x[0-9a-fA-F]{2}|u\{[0-9a-fA-F]+\}|.)|[^\\'\n])'")


def long_fns(path):
    """-> [(start line, length, name)] of fns longer than FN_MAX lines."""
    s = open(os.path.join(repo, path), encoding="utf-8", errors="replace").read()
    out, stack, pending = [], [], None  # stack: (name, start line, depth at its `{`)
    depth, nest, line, i, n = 0, 0, 1, 0, len(s)
    while i < n:
        c = s[i]
        if c == "\n":
            line += 1
            i += 1
        elif s.startswith("//", i):
            j = s.find("\n", i)
            i = n if j < 0 else j
        elif s.startswith("/*", i):
            lvl, i = 1, i + 2
            while i < n and lvl:
                if s.startswith("/*", i):
                    lvl, i = lvl + 1, i + 2
                elif s.startswith("*/", i):
                    lvl, i = lvl - 1, i + 2
                else:
                    line += s[i] == "\n"
                    i += 1
        elif c == "r" and (m := re.match(r'r(#*)"', s[i:i + 300])) and not (i and (s[i - 1].isalnum() or s[i - 1] == "_")):
            end = '"' + m.group(1)
            j = s.find(end, i + len(m.group(0)))
            j = n if j < 0 else j + len(end)
            line += s.count("\n", i, j)
            i = j
        elif c == '"' or (c == "b" and s.startswith('b"', i)):
            i += 1 if c == '"' else 2
            while i < n and s[i] != '"':
                if s[i] == "\\":
                    line += s[i + 1:i + 2] == "\n"
                    i += 2
                    continue
                line += s[i] == "\n"
                i += 1
            i += 1
        elif c == "'" and (m := CHAR.match(s, i)):
            i = m.end()
        elif c.isalpha() or c == "_":
            m = IDENT.match(s, i)
            word = m.group(0)
            i = m.end()
            if word == "fn":
                nm = IDENT.match(s, i + len(s[i:]) - len(s[i:].lstrip()))
                if nm:
                    pending = (nm.group(0), line, depth, nest)
        elif c == "{":
            depth += 1
            if pending:
                stack.append((pending[0], pending[1], depth))
                pending = None
            i += 1
        elif c == "}":
            if stack and stack[-1][2] == depth:
                name, start, _ = stack.pop()
                if line - start + 1 > FN_MAX:
                    out.append((start, line - start + 1, name))
            depth -= 1
            i += 1
        elif c in "([":
            nest += 1
            i += 1
        elif c in ")]":
            nest -= 1
            i += 1
        elif c == ";":
            # A fn without a body (`fn f();`) - but not the `;` of an array type in the signature (`[f32; 4]`).
            if pending and depth == pending[2] and nest == pending[3]:
                pending = None
            i += 1
        else:
            i += 1
    return out


for p in ("src/bin/game.rs", "src/render/mod.rs"):
    print(f"lines {p}: {lines_of(p) if os.path.exists(os.path.join(repo, p)) else 'missing'}")
files = list(rs_files("src"))
big = [(lines_of(f), f) for f in files]
big = [(n, f) for n, f in big if n > FILE_MAX]
print(f"files over {FILE_MAX} lines: {len(big)}")
for n, f in sorted(big, reverse=True):
    print(f"  {n:6} {f}")
fns = [(ln, f, start, name) for f in files for start, ln, name in long_fns(f)]
print(f"fns over {FN_MAX} lines: {len(fns)}")
for ln, f, start, name in sorted(fns, key=lambda r: (-r[0], r[1], r[2])):
    print(f"  {ln:6} {f}:{start} {name}")
EOF
