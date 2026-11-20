#!/usr/bin/env python3
"""crate_layers.py: the library's layer check (ARCHITECTURE.md "Layers").

Every library module is assigned to a layer (scripts/checks/crate-layers.txt: a module path prefix and its
crate, longest prefix wins; the crates listed bottom to top). A reference from a module to a module of a
higher layer is an upward reference: it would be a dependency cycle once the layers are separate crates.
The references the tree still has are listed in scripts/checks/crate-layers-allow.txt (file and target
path, no line numbers); any reference not on that list fails, so the list only shrinks.

The scan reads `crate::` / `super::` / `self::` paths and `use` trees in every library source file
(src/ and crates/<name>/src/ of the crates that hold library modules, test code included: a test module
moves with its file) and the programs' (src/bin/**) `nfsu2::` paths, which must not reach a layer above
the program's own (crate-layers.txt `bin:<name>` lines; unlisted programs belong to the top layer).

Usage: crate_layers.py [repo] [--list]   --list prints every upward reference in allow-list form.
Exit 0 and "CRATE LAYERS: ok (...)" when clean, 1 with the offending references otherwise.
"""
import os
import re
import sys

ARGS = [a for a in sys.argv[1:] if not a.startswith("--")]
REPO = ARGS[0] if ARGS else os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "../.."))
LIST = "--list" in sys.argv
HERE = os.path.join(REPO, "scripts/checks")

# ---- the layer map ----------------------------------------------------------------------------------------
ASSIGN, BINS, ORDER, ROOTS = {}, {}, [], {}
for line in open(os.path.join(HERE, "crate-layers.txt"), encoding="utf-8"):
    line = line.split("#")[0].strip()
    if not line:
        continue
    words = line.split()
    if words[0] == "crate":
        # crate NAME SRC-ROOT: a layer, bottom to top, and the folder its modules are read from.
        ORDER.append(words[1])
        ROOTS[words[1]] = words[2]
    elif words[0].startswith("bin:"):
        BINS[words[0][4:]] = words[1]
    else:
        ASSIGN[words[0]] = words[1]
RANK = {c: i for i, c in enumerate(ORDER)}
TOP = ORDER[-1]


def layer_of(mod):
    """The layer of a module path (a list); the root module's items are `<root>`."""
    for n in range(len(mod), 0, -1):
        key = "::".join(mod[:n])
        if key in ASSIGN:
            return ASSIGN[key]
    return ASSIGN.get("<root>", TOP) if not mod else TOP


# ---- the files --------------------------------------------------------------------------------------------
def modpath(rel):
    p = rel[:-3].split("/")
    if p[-1] in ("mod", "lib"):
        p = p[:-1]
    return p


# A crate root file (lib.rs) belongs to the highest crate read from its folder (the facade's lib.rs to the
# top layer, crates/<name>/src/lib.rs to that crate).
ROOT_LAYER = {}
for crate, root in ROOTS.items():
    ROOT_LAYER[os.path.normpath(os.path.join(root, "lib.rs"))] = crate
FILES = {}  # repo-relative path -> module path
for root in sorted(set(ROOTS.values())):
    base = os.path.join(REPO, root)
    for d, ds, fs in os.walk(base):
        ds.sort()
        for f in sorted(fs):
            if not f.endswith(".rs"):
                continue
            full = os.path.join(d, f)
            rel = os.path.relpath(full, base)
            if rel.startswith("bin/") or rel == "main.rs":
                continue
            FILES[os.path.relpath(full, REPO)] = modpath(rel)
MODS = {tuple(m) for m in FILES.values()}
BINFILES = []
bindir = os.path.join(REPO, "src/bin")
for d, ds, fs in os.walk(bindir):
    for f in fs:
        if f.endswith(".rs"):
            BINFILES.append(os.path.relpath(os.path.join(d, f), REPO))


def strip(text):
    """Comments and string literals blanked (offsets kept), so neither is read as a path."""
    out = list(text)
    i, n = 0, len(text)
    while i < n:
        c = text[i]
        if text.startswith("//", i):
            j = text.find("\n", i)
            j = n if j < 0 else j
            for k in range(i, j):
                out[k] = " "
            i = j
        elif text.startswith("/*", i):
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            for k in range(i, j):
                if out[k] != "\n":
                    out[k] = " "
            i = j
        elif c == "r" and re.match(r'r#*"', text[i:i + 10]) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            m = re.match(r'r(#*)"', text[i:])
            end = text.find('"' + m.group(1), i + len(m.group(0)))
            end = n if end < 0 else end + 1 + len(m.group(1))
            for k in range(i, end):
                if out[k] != "\n":
                    out[k] = " "
            i = end
        elif c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            for k in range(i, min(j + 1, n)):
                if out[k] != "\n":
                    out[k] = " "
            i = j + 1
        elif c == "'" and re.match(r"'(\\.|[^\\'])'", text[i:i + 4]) or (c == "'" and re.match(r"'\\(x..|u\{[0-9a-fA-F]+\})'", text[i:i + 12])):
            m = re.match(r"'(\\x..|\\u\{[0-9a-fA-F]+\}|\\.|[^\\'])'", text[i:])
            for k in range(i, i + len(m.group(0))):
                out[k] = " "
            i += len(m.group(0))
        else:
            i += 1
    return "".join(out)


def expand_use(body):
    """`a::{b, c::d}` -> [[a, b], [a, c, d]]."""
    res = []

    def rec(prefix, s):
        s = s.strip()
        if not s:
            return
        i = s.find("{")
        if i == -1:
            res.append(prefix + [x for x in s.split(" as ")[0].strip().split("::") if x])
            return
        head = [x for x in s[:i].rstrip(":").strip().split("::") if x]
        inner = s[i + 1:s.rfind("}")]
        depth, cur, parts = 0, "", []
        for ch in inner:
            if ch == "{":
                depth += 1
            if ch == "}":
                depth -= 1
            if ch == "," and depth == 0:
                parts.append(cur)
                cur = ""
            else:
                cur += ch
        parts.append(cur)
        for p in parts:
            rec(prefix + head, p)

    rec([], body)
    return res


INLINE_MOD = re.compile(r'\bmod\s+([A-Za-z_][A-Za-z0-9_]*)\s*\{')
USE = re.compile(r'\buse\s+([^;]+);', re.S)
PATH = re.compile(r'(?<![A-Za-z0-9_$:])(crate|nfsu2|self|super)((?:::[A-Za-z_][A-Za-z0-9_]*)+)')


def inline_spans(t):
    """(start, end, name) of every inline `mod name { ... }` block."""
    spans = []
    for m in INLINE_MOD.finditer(t):
        depth, j = 0, m.end() - 1
        while j < len(t):
            if t[j] == "{":
                depth += 1
            elif t[j] == "}":
                depth -= 1
                if depth == 0:
                    break
            j += 1
        spans.append((m.end(), j, m.group(1)))
    return spans


def scope_at(cur, spans, pos):
    s = list(cur)
    for a, b, name in spans:
        if a <= pos < b:
            s.append(name)
    return s


def resolve(cur, path, lib):
    h = path[0]
    if h == "crate" or (h == "nfsu2" and lib):
        return path[1:]
    if h == "self":
        return cur + path[1:]
    if h == "super":
        c, i = list(cur), 0
        while i < len(path) and path[i] == "super":
            c, i = c[:-1], i + 1
        return c + path[i:]
    return None


def target_module(r):
    """The longest known module prefix of a resolved path; [] (the root) when none."""
    for n in range(len(r), 0, -1):
        if tuple(r[:n]) in MODS:
            return r[:n]
    return []


def refs_of(rel, cur, lib):
    t = strip(open(os.path.join(REPO, rel), encoding="utf-8", errors="replace").read())
    spans = inline_spans(t) if lib else []
    found = []
    for m in USE.finditer(t):
        for p in expand_use(m.group(1)):
            if p and p[0] in ("crate", "self", "super", "nfsu2"):
                found.append((m.start(), p))
    for m in PATH.finditer(t):
        found.append((m.start(), [x for x in (m.group(1) + m.group(2)).split("::") if x]))
    out = set()
    for pos, p in found:
        if not lib:
            if p[0] != "nfsu2":
                continue
            r = p[1:]
        else:
            r = resolve(scope_at(cur, spans, pos), p, True)
        if r is None:
            continue
        r = [x for x in r if x != "self"]
        tgt = target_module(r)
        # A root item (`crate::game_root`, an `env_*!` macro) is named by its first segment.
        shown = "::".join(tgt) if tgt else (r[0] if r else "")
        out.add((tuple(tgt), shown, "::".join(r[:len(tgt) + 1]) if r else ""))
    return out


ups = []
for rel, cur in sorted(FILES.items()):
    me = ROOT_LAYER.get(rel, layer_of(cur)) if not cur else layer_of(cur)
    for tgt, shown, full in refs_of(rel, cur, True):
        other = layer_of(list(tgt)) if tgt else ASSIGN.get("<root>", TOP)
        if RANK.get(other, len(ORDER)) > RANK.get(me, len(ORDER)):
            ups.append((rel, full or shown, me, other))
for rel in sorted(BINFILES):
    parts = rel.split("/")
    name = parts[2][:-3] if len(parts) == 3 else parts[2]
    me = BINS.get(name, TOP)
    for tgt, shown, full in refs_of(rel, [], False):
        other = layer_of(list(tgt)) if tgt else ASSIGN.get("<root>", TOP)
        if RANK.get(other, len(ORDER)) > RANK.get(me, len(ORDER)):
            ups.append((rel, full or shown, me, other))

# ---- Studio's interface into the library --------------------------------------------------------------------
# crate-layers.txt `studio DIR…`: Studio's crates name the library only as `nfsu2::studio_api::…` (a deep path such
# as `nfsu2::world::World` would tie Studio to the library's module layout), and the interface (src/studio_api.rs)
# re-exports the library's items one by one: no glob re-export of a `crate::`/`self::`/`super::` path.
STUDIO = []
for line in open(os.path.join(HERE, "crate-layers.txt"), encoding="utf-8"):
    w = line.split("#")[0].split()
    if w and w[0] == "studio":
        STUDIO += w[1:]


def deep_paths(text):
    """Every `nfsu2` path in `text` (comments and strings blanked) that does not go through `studio_api`."""
    t = strip(text)
    bad = []
    for m in USE.finditer(t):
        for p in expand_use(m.group(1)):
            if p and p[0] == "nfsu2" and (len(p) < 2 or p[1] != "studio_api"):
                bad.append("::".join(p))
    for m in re.finditer(r'(?<![A-Za-z0-9_$:])nfsu2\b(?!\s*::\s*studio_api\b)((?:\s*::\s*[A-Za-z_][A-Za-z0-9_]*)*)', t):
        if not t[:m.start()].rstrip().endswith("use"):
            bad.append(re.sub(r"\s", "", m.group(0)))
    return sorted(set(bad))


def globbed(text):
    """The glob re-exports of the library's own paths in `text`."""
    t = strip(text)
    return sorted({"::".join(p) for m in USE.finditer(t) for p in expand_use(m.group(1))
                   if p and p[0] in ("crate", "self", "super") and p[-1] == "*"})


def studio_self_test():
    """The check fails on planted deep paths and globs, and passes the interface's own form."""
    assert deep_paths("use nfsu2::studio_api::{world, render::Gpu};\nlet w = nfsu2::studio_api::world::World::flat(1.0);") == []
    assert deep_paths("use nfsu2::world::World;") == ["nfsu2::world::World"]
    assert deep_paths("use nfsu2::{studio_api::fog, render};") == ["nfsu2::render"]
    assert deep_paths("fn f() { nfsu2::qol::setup::saved_root(); }") == ["nfsu2::qol::setup::saved_root"]
    assert deep_paths("// nfsu2::world in a comment\nlet s = \"nfsu2::world\";") == []
    assert globbed("pub use crate::world::*;") == ["crate::world::*"]
    assert globbed("pub use crate::world::{World, parts_of};\npub use ob_map::tools::map::format::*;") == []


studio_bad = []
if STUDIO:
    studio_self_test()
    for d in STUDIO:
        for dd, ds, fs in os.walk(os.path.join(REPO, d)):
            ds.sort()
            for f in sorted(fs):
                if f.endswith(".rs"):
                    full = os.path.join(dd, f)
                    for p in deep_paths(open(full, encoding="utf-8", errors="replace").read()):
                        studio_bad.append(f"{os.path.relpath(full, REPO)}: {p}")
    api = os.path.join(REPO, "src/studio_api.rs")
    for p in globbed(open(api, encoding="utf-8").read()) if os.path.exists(api) else ["src/studio_api.rs missing"]:
        studio_bad.append(f"src/studio_api.rs: glob re-export {p}")
    if studio_bad:
        print("CRATE LAYERS: Studio names the library outside its interface (nfsu2::studio_api, ARCHITECTURE.md):")
        for b in studio_bad:
            print("  " + b)

if LIST:
    for rel, path, a, b in sorted(set(ups)):
        print(f"{rel} {path}")
    sys.exit(0)

allow = set()
apath = os.path.join(HERE, "crate-layers-allow.txt")
if os.path.exists(apath):
    for line in open(apath, encoding="utf-8"):
        line = line.split("#")[0].strip()
        if line:
            f, p = line.split()
            allow.add((f, p))
new = sorted({(rel, path, a, b) for rel, path, a, b in ups if (rel, path) not in allow})
have = {(rel, path) for rel, path, _, _ in ups}
stale = sorted(allow - have)
if new:
    print("CRATE LAYERS: upward references (a lower layer naming a higher one; ARCHITECTURE.md, Layers):")
    for rel, path, a, b in new:
        print(f"  {rel}: {path} ({a} -> {b})")
if stale:
    print("CRATE LAYERS: allow-list lines no longer needed (take them off crate-layers-allow.txt):")
    for f, p in stale:
        print(f"  {f} {p}")
if new or stale or studio_bad:
    sys.exit(1)
print(f"CRATE LAYERS: ok ({len(FILES)} library files, {len(BINFILES)} program files, {len(allow)} allowed upward references, "
      f"Studio through nfsu2::studio_api only: {' '.join(STUDIO) or 'none'})")
