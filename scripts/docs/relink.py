#!/usr/bin/env python3
"""Rewrite references to moved docs pages from a move map (docs/notes/MOVED.tsv by default).

The map has one row per move: `old path<TAB>new path`; a row whose old path ends in `/` moves a whole folder.
A third column names the section of the new page that a merged page became: a link without an anchor then
gets that section's anchor, and a plain mention not already followed by a section name or anchor gets
` "Section"` after it.
Run it after the `git mv`s. It rewrites, in every tracked text file:

* relative links in Markdown files (`[text](path#anchor)`, `[id]: path`, `src="…"` / `href="…"`), recomputed
  for the linking file's new place and the target's new place; a link that already resolves is left alone;
* plain mentions of an old path (`docs/old.md`, `../../docs/old.md`, `docs/old-folder/…`) in comments, strings,
  scripts and lists, e.g. `include_str!` / `concat!(env!(…))` paths and the release lists;
* bare file names of pages that were renamed (`old-name.md` → its new `docs/…` path).

The map file itself and the stub pages left at old paths (`--keep PATH`, repeatable) are not rewritten.
It is idempotent: running it again changes nothing.

Usage: scripts/docs/relink.py [--map docs/notes/MOVED.tsv] [--keep docs/old.md ...] [--dry-run]
"""
import argparse
import os
import posixpath
import re
import subprocess
import sys

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
LINK_INLINE = re.compile(r"(\]\()(<?)([^)\s>]+)(>?(?:\s+\"[^\"]*\")?\))")
LINK_DEF = re.compile(r"^(\s*\[[^\]]+\]:\s*)(\S+)", re.M)
LINK_HTML = re.compile(r"((?:src|href)=\")([^\"]+)(\")")
SKIP_SCHEMES = re.compile(r"^(?:[a-z][a-z0-9+.-]*:|#|/)", re.I)


SECTIONS = {}


def load_map(path):
    files, dirs = {}, {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            if not line.strip() or line.startswith("#"):
                continue
            cols = line.rstrip("\n").split("\t")
            old, new = cols[:2]
            (dirs if old.endswith("/") else files)[old] = new
            if len(cols) > 2 and cols[2]:
                SECTIONS[old] = cols[2]
    return files, dirs


class Mover:
    def __init__(self, files, dirs):
        self.files, self.dirs = files, dirs
        self.back = {v: k for k, v in files.items()}
        self.back_dirs = {v: k for k, v in dirs.items()}

    def forward(self, p):
        """New path of an old repo-relative path, or None when the map does not move it."""
        if p in self.files:
            return self.files[p]
        for old, new in self.dirs.items():
            if p == old.rstrip("/"):
                return new.rstrip("/")
            if p.startswith(old):
                return new + p[len(old):]
        return None

    def backward(self, p):
        """Where a file now at p lived before the moves (p itself when it did not move)."""
        if p in self.back:
            return self.back[p]
        for new, old in self.back_dirs.items():
            if p.startswith(new):
                return old + p[len(new):]
        return p


def exists(rel):
    return os.path.exists(os.path.join(ROOT, rel))


def fix_link(mover, here, before, link):
    """The link from file `here` (which lived at `before`) rewritten for the moves, or None to leave it."""
    if SKIP_SCHEMES.match(link):
        return None
    path, sep, frag = link.partition("#")
    if not path:
        return None
    bases = [posixpath.dirname(here)] + ([posixpath.dirname(before)] if before != here else [])
    for i, base in enumerate(bases):
        target = posixpath.normpath(posixpath.join(base, path))
        if target.startswith(".."):
            continue
        moved = mover.forward(target)
        if moved is not None and not frag and target in SECTIONS:
            sep, frag = "#", slug(SECTIONS[target])
        if moved is not None and exists(moved):
            new = posixpath.relpath(moved, posixpath.dirname(here))
        elif moved is None and exists(target):
            if i == 0:
                return None  # resolves already
            new = posixpath.relpath(target, posixpath.dirname(here))
        else:
            continue
        if path.endswith("/") and not new.endswith("/"):
            new += "/"
        return new + sep + frag
    return None


def slug(title):
    """GitHub's heading anchor."""
    import unicodedata
    out = []
    for ch in title.strip().lower():
        if ch in " -":
            out.append("-")
        elif ch == "_" or unicodedata.category(ch)[0] in "LN":
            out.append(ch)
    return "".join(out)


def plain_patterns(files, dirs):
    """One regex for every old path (folders without their slash) and one for renamed pages' bare names."""
    paths = dict(files)
    paths.update({o.rstrip("/"): n.rstrip("/") for o, n in dirs.items()})
    names = {posixpath.basename(o): n for o, n in files.items() if posixpath.basename(o) != posixpath.basename(n)}
    for o in files:
        if o in SECTIONS:
            paths[o] = (paths[o], SECTIONS[o])
            b = posixpath.basename(o)
            if b in names:
                names[b] = (names[b], SECTIONS[o])
    alt = lambda keys: "|".join(re.escape(k) for k in sorted(keys, key=len, reverse=True))
    full = re.compile(r"(?<![\w-])(" + alt(paths) + r")(?![\w-])")
    bare = re.compile(r"(?<![\w/.-])(" + alt(names) + r")(?![\w-])") if names else None
    return [(full, paths), (bare, names)]


def replacement(m, value, listing=False, table=False):
    """A plain mention's new text: the new path, plus the section a merged page became unless one follows."""
    if isinstance(value, str):
        return value
    path, section = value
    if listing:
        return path
    if table and not rest.startswith(("#", ' "', " §", '"')):
        return f"{path}#{slug(section)}"  # a list's doc column takes path#anchor
    rest = m.string[m.end():m.end() + 3]
    if rest.startswith(("#", ' "', " §", '"')) or re.match(r":[*=0-9]", rest):
        return path
    return f'{path} "{section}"'


def rewrite_markdown(mover, rel, text):
    before = mover.backward(rel)

    def sub(m, g):
        new = fix_link(mover, rel, before, m.group(g))
        if new is None:
            return m.group(0)
        return m.group(0)[: m.start(g) - m.start(0)] + new + m.group(0)[m.end(g) - m.start(0):]

    out, fence = [], False
    for line in text.splitlines(keepends=True):
        if line.lstrip().startswith(("```", "~~~")):
            fence = not fence
        if not fence:
            line = LINK_INLINE.sub(lambda m: sub(m, 3), line)
            line = LINK_HTML.sub(lambda m: sub(m, 2), line)
            line = LINK_DEF.sub(lambda m: sub(m, 2), line)
        out.append(line)
    return "".join(out)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--map", default="docs/notes/MOVED.tsv")
    ap.add_argument("--keep", action="append", default=[], help="a stub page at an old path (not rewritten)")
    ap.add_argument("--dry-run", action="store_true")
    a = ap.parse_args()
    files, dirs = load_map(os.path.join(ROOT, a.map))
    mover = Mover(files, dirs)
    pats = plain_patterns(files, dirs)
    skip = set([a.map] + a.keep)
    tracked = subprocess.run(["git", "ls-files", "-z"], cwd=ROOT, capture_output=True, check=True).stdout
    changed = 0
    for rel in tracked.decode().split("\0"):
        if not rel or rel in skip or not os.path.isfile(os.path.join(ROOT, rel)):
            continue
        raw = open(os.path.join(ROOT, rel), "rb").read()
        if b"\0" in raw[:65536]:
            continue
        try:
            text = raw.decode("utf-8")
        except UnicodeDecodeError:
            continue
        new = text
        if rel.endswith(".md"):
            new = rewrite_markdown(mover, rel, new)
        listing = posixpath.basename(rel).startswith("SHA256SUMS")  # a checksum list names files only
        is_tsv = rel.endswith(".tsv")
        for pat, table in pats:
            if pat is not None:
                new = pat.sub(lambda m: replacement(m, table[m.group(1)], listing, is_tsv), new)
        if new != text:
            changed += 1
            if a.dry_run:
                print(rel)
            else:
                with open(os.path.join(ROOT, rel), "w", encoding="utf-8", newline="") as f:
                    f.write(new)
    print(f"relink: {changed} files {'would change' if a.dry_run else 'changed'}", file=sys.stderr)


if __name__ == "__main__":
    main()
