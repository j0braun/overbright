#!/usr/bin/env python3
"""Check the relative links of every tracked Markdown file: the target file or folder exists and, for a link
to a Markdown heading (`page.md#anchor`, `#anchor`), the heading exists (GitHub's anchors, -1/-2 for repeats;
explicit `<a id="…">` / `name="…"` anchors count too).

Usage: scripts/docs/check_links.py [--anchors] [PATH ...]   (default: every tracked *.md; exit 1 on a broken link)
Without --anchors only the files are checked. --public also counts a link from a public file to a file on
scripts/release/private.list (it breaks in the public tree); private files' own links are not checked then.
"""
import argparse
import collections
import os
import posixpath
import re
import subprocess
import sys
import unicodedata
import fnmatch

ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "..", ".."))
LINK_INLINE = re.compile(r"\]\(<?([^)\s>]+)>?(?:\s+\"[^\"]*\")?\)")
LINK_DEF = re.compile(r"^\s*\[[^\]]+\]:\s*(\S+)")
LINK_HTML = re.compile(r"(?:src|href)=\"([^\"]+)\"")
SCHEME = re.compile(r"^[a-z][a-z0-9+.-]*:", re.I)
CODE_SPAN = re.compile(r"`[^`]*`")
FENCE = re.compile(r"^\s*(```|~~~)")


def slug(title):
    title = re.sub(r"<[^>]+>", "", title)
    title = re.sub(r"\[([^\]]*)\]\([^)]*\)", r"\1", title)
    out = []
    for ch in title.strip().lower():
        if ch in " -":
            out.append("-")
        elif ch == "_" or unicodedata.category(ch)[0] in "LN":
            out.append(ch)
    return "".join(out)


_anchor_cache = {}


def anchors(rel):
    if rel not in _anchor_cache:
        found, seen, fence = set(), collections.Counter(), False
        try:
            lines = open(os.path.join(ROOT, rel), encoding="utf-8").read().splitlines()
        except (OSError, UnicodeDecodeError):
            lines = []
        for line in lines:
            if FENCE.match(line):
                fence = not fence
                continue
            if fence:
                continue
            for m in re.finditer(r"(?:id|name)=\"([^\"]+)\"", line):
                found.add(m.group(1))
            m = re.match(r"^#{1,6} (.*?)#*\s*$", line)
            if m:
                base = slug(m.group(1))
                n = seen[base]
                seen[base] += 1
                found.add(base if n == 0 else f"{base}-{n}")
        _anchor_cache[rel] = found
    return _anchor_cache[rel]


def load_private():
    p = os.path.join(ROOT, "scripts", "release", "private.list")
    if not os.path.exists(p):
        return []
    return [l.split("#")[0].strip() for l in open(p, encoding="utf-8") if l.split("#")[0].strip()]


def under(path, pats):
    for pat in pats:
        if pat.endswith("/"):
            if path.startswith(pat) or path == pat[:-1]:
                return True
        elif path == pat or fnmatch.fnmatchcase(path, pat) or path.startswith(pat + "/"):
            return True
    return False


def links(text):
    fence = False
    for no, line in enumerate(text.splitlines(), 1):
        if FENCE.match(line):
            fence = not fence
            continue
        if fence:
            continue
        line = CODE_SPAN.sub("", line)
        for rx in (LINK_INLINE, LINK_HTML):
            for m in rx.finditer(line):
                yield no, m.group(1)
        m = LINK_DEF.match(line)
        if m:
            yield no, m.group(1)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--anchors", action="store_true", help="also check #anchors")
    ap.add_argument("--public", action="store_true", help="also flag public files' links to private files")
    ap.add_argument("paths", nargs="*")
    a = ap.parse_args()
    files = a.paths or [
        f for f in subprocess.run(["git", "ls-files", "*.md"], cwd=ROOT, capture_output=True, text=True,
                                  check=True).stdout.split("\n") if f
    ]
    private = load_private() if a.public else []
    broken = 0
    for rel in files:
        if private and under(rel, private):
            continue
        try:
            text = open(os.path.join(ROOT, rel), encoding="utf-8").read()
        except (OSError, UnicodeDecodeError):
            continue
        for no, link in links(text):
            if SCHEME.match(link) or link.startswith("/"):
                continue
            path, _, frag = link.partition("#")
            target = posixpath.normpath(posixpath.join(posixpath.dirname(rel), path)) if path else rel
            bad = None
            if target.startswith("..") or not os.path.exists(os.path.join(ROOT, target)):
                bad = "missing file"
            elif private and os.path.isfile(os.path.join(ROOT, target)) and under(target, private):
                bad = "private target"
            elif a.anchors and frag and target.endswith(".md") and frag not in anchors(target):
                bad = "missing anchor"
            if bad:
                broken += 1
                print(f"{rel}:{no}: {bad}: {link}")
    print(f"check_links: {len(files)} files, {broken} broken links", file=sys.stderr)
    sys.exit(1 if broken else 0)


if __name__ == "__main__":
    main()
