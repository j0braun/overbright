#!/usr/bin/env python3
"""The list of Rust crates built into the player's programs, with their licences and licence texts
(scripts/release/package.sh, docs/developing/building-releases.md).

  crate_licenses.py --metadata M.json [--map /cargo-home=DIR] --target TRIPLE --out FILE [--root nfsu2 --root ob-editor]

M.json is `cargo metadata --format-version 1 --locked --offline --filter-platform TRIPLE` of the released
tree with the default features (the shipping build: no dev-harness). The crates are the normal-dependency
closure of the root packages (the workspace's packages that hold the player's programs: `nfsu2` with the game
and the launcher, `ob-editor` with the map editor; dev- and build-dependencies left out, the roots themselves
not listed, nor the workspace's library crates under them); proc-macro crates and what only they
use are listed as "build time" (they generate code at compile time and are not linked). The licence texts
are the crates' own files (LICENSE*, LICENCE*, COPYING*, NOTICE*, COPYRIGHT*, UNLICENSE*, the manifest's
license-file, LICENSES/*) from the crate sources cargo already downloaded; identical texts are written once
with the crates that use them. A crate without a licence file gets the standard text of its licence
(MIT: the MIT text with the manifest's authors; others: the same licence's text from another crate).

Every licence expression must be satisfiable with GPL-3.0-or-later-compatible licences (ALLOWED). Exit 0
when it is and every crate has a text; 1 otherwise (the reason is printed). Python 3 standard library only.
"""
import argparse
import hashlib
import json
import os
import re
import sys
from pathlib import Path

# Licences a GPL-3.0-or-later program may include (FSF's list of GPL-compatible free software licences).
ALLOWED = {
    "MIT", "MIT-0", "Apache-2.0", "Apache-2.0 WITH LLVM-exception", "BSD-2-Clause", "BSD-3-Clause", "0BSD",
    "ISC", "Zlib", "MPL-2.0", "CC0-1.0", "Unlicense", "BSL-1.0", "Unicode-3.0", "Unicode-DFS-2016",
    "LGPL-2.1-or-later", "LGPL-3.0", "LGPL-3.0-only", "LGPL-3.0-or-later", "GPL-3.0", "GPL-3.0-only",
    "GPL-3.0-or-later", "GPL-2.0-or-later", "NCSA", "X11", "Zlib-acknowledgement", "libpng-2.0", "IJG",
}
TEXT_FILE = re.compile(r"^(LICEN[CS]E|COPYING|NOTICE|COPYRIGHT|UNLICENSE)([-_.].*)?$", re.I)
SKIP_EXT = (".rs", ".toml", ".json", ".py", ".sh", ".yml", ".yaml", ".html")

MIT_BODY = """Permission is hereby granted, free of charge, to any person obtaining a copy of this software and
associated documentation files (the "Software"), to deal in the Software without restriction, including
without limitation the rights to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is furnished to do so, subject to the
following conditions:

The above copyright notice and this permission notice shall be included in all copies or substantial
portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT
LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO
EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER
IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
USE OR OTHER DEALINGS IN THE SOFTWARE.
"""


def tokenize(expr):
    expr = expr.replace("/", " OR ")
    return re.findall(r"\(|\)|[A-Za-z0-9.+\-]+", expr)


def satisfiable(expr):
    """-> (ok, the licences chosen). SPDX: OR = either side, AND = both, WITH = an exception (kept with its id)."""
    toks = tokenize(expr)
    pos = 0

    def atom():
        nonlocal pos
        if toks[pos] == "(":
            pos += 1
            r = disj()
            pos += 1  # ')'
            return r
        lic = toks[pos]
        pos += 1
        if pos < len(toks) and toks[pos] == "WITH":
            lic = f"{lic} WITH {toks[pos + 1]}"
            pos += 2
        ok = lic in ALLOWED or lic.rstrip("+") in ALLOWED
        return ok, [lic]

    def conj():
        nonlocal pos
        ok, used = atom()
        while pos < len(toks) and toks[pos] == "AND":
            pos += 1
            o2, u2 = atom()
            ok, used = ok and o2, used + u2
        return ok, used

    def disj():
        nonlocal pos
        best = conj()
        while pos < len(toks) and toks[pos] == "OR":
            pos += 1
            alt = conj()
            if alt[0] and not best[0]:
                best = alt
        return best

    try:
        return disj()
    except IndexError:
        return False, [expr]


def closure(meta, root_names):
    pkgs = {p["id"]: p for p in meta["packages"]}
    nodes = {n["id"]: n for n in meta["resolve"]["nodes"]}
    roots = [next(i for i, p in pkgs.items() if p["name"] == r and p["source"] is None) for r in root_names]

    def normal_deps(nid):
        for d in nodes[nid]["deps"]:
            if any(k["kind"] is None for k in d["dep_kinds"]):
                yield d["pkg"]

    def walk(skip_proc_macro):
        seen, todo = set(roots), list(roots)
        while todo:
            n = todo.pop()
            for d in normal_deps(n):
                if d in seen:
                    continue
                if skip_proc_macro and any("proc-macro" in t["kind"] for t in pkgs[d]["targets"]):
                    continue
                seen.add(d)
                todo.append(d)
        return seen - set(roots)

    # The workspace's own crates (ob-core and the other library layers) are Overbright's code, under its
    # licence: not listed.
    own = set(meta.get("workspace_members", []))
    every, linked = walk(False) - own, walk(True) - own
    return pkgs, roots, every, linked


def licence_files(crate_dir, license_file):
    files = []
    if license_file:
        p = crate_dir / license_file
        if p.is_file():
            files.append(p)
    for d in [crate_dir, crate_dir / "LICENSES", crate_dir / "licenses"]:
        if not d.is_dir():
            continue
        for p in sorted(d.iterdir()):
            if p.is_file() and (d != crate_dir or TEXT_FILE.match(p.name)) and not p.name.endswith(SKIP_EXT) \
                    and p not in files:
                files.append(p)
    return files


def html_text(t):
    """A plain-text reading of a simple HTML page (the Rust standard library's COPYRIGHT-library.html)."""
    import html
    t = re.sub(r"(?is)<(script|style|head).*?</\1>", "", t)
    t = re.sub(r"(?i)<br\s*/?>|</(p|h[1-6]|li|pre|div|tr)>", "\n", t)
    t = re.sub(r"(?i)<li[^>]*>", "- ", t)
    t = html.unescape(re.sub(r"<[^>]+>", "", t))
    return re.sub(r"\n{3,}", "\n\n", "\n".join(l.rstrip() for l in t.split("\n"))).strip() + "\n"


def norm(text):
    return re.sub(r"\s+", " ", text).strip()


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--metadata", required=True)
    ap.add_argument("--map", action="append", default=[], help="PREFIX=DIR: read a path under PREFIX from DIR")
    ap.add_argument("--target", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--root", action="append", help="a root package (repeatable; default: nfsu2 and ob-editor)")
    ap.add_argument("--section", nargs=2, action="append", default=[], metavar=("TITLE", "FILE"),
                    help="append FILE (text; .html is turned into text) as a section: the Rust standard library's "
                         "notices, the toolchain's runtime libraries")
    a = ap.parse_args()
    meta = json.load(open(a.metadata))
    maps = [m.split("=", 1) for m in a.map]

    def local(path):
        for pre, d in maps:
            if path.startswith(pre):
                return Path(d + path[len(pre):])
        return Path(path)

    pkgs, roots, every, linked = closure(meta, a.root or ["nfsu2", "ob-editor"])
    crates = sorted(every, key=lambda i: (pkgs[i]["name"], pkgs[i]["version"]))
    problems = []
    texts = {}  # hash -> {"text", "users": [..], "file"}
    per_crate = {}
    by_licence = {}  # licence id -> text (from a crate's file named after it), for crates without a file
    for cid in crates:
        p = pkgs[cid]
        name = f"{p['name']} {p['version']}"
        lic = p.get("license") or ""
        if not lic:
            problems.append(f"{name}: no licence expression (license-file {p.get('license_file')})")
        else:
            ok, _ = satisfiable(lic)
            if not ok:
                problems.append(f"{name}: licence '{lic}' is not GPL-3.0-or-later compatible")
        if p["source"] is None or not p["source"].startswith("registry+"):
            problems.append(f"{name}: not from crates.io ({p['source']})")
        crate_dir = local(p["manifest_path"]).parent
        if not crate_dir.is_dir():
            problems.append(f"{name}: sources not found at {crate_dir}")
            per_crate[cid] = []
            continue
        hs = []
        for f in licence_files(crate_dir, p.get("license_file")):
            t = f.read_text(encoding="utf-8", errors="replace").replace("\r\n", "\n").strip("\n") + "\n"
            h = hashlib.sha256(norm(t).encode()).hexdigest()
            texts.setdefault(h, {"text": t, "users": [], "file": f.name})["users"].append(name)
            hs.append(h)
            up = f.name.upper()
            for key, ids in [("APACHE", ["Apache-2.0"]), ("MIT", ["MIT"]), ("ZLIB", ["Zlib"]),
                             ("BSD", ["BSD-3-Clause"]), ("UNLICENSE", ["Unlicense"]), ("MPL", ["MPL-2.0"]),
                             ("BOOST", ["BSL-1.0"]), ("BSL", ["BSL-1.0"]), ("UNICODE", ["Unicode-3.0"])]:
                if key in up and key != "MIT":
                    for i in ids:
                        by_licence.setdefault(i, h)
        per_crate[cid] = hs
    # Crates without a file of their own: the standard text.
    generated = []
    for cid in crates:
        if per_crate[cid]:
            continue
        p = pkgs[cid]
        name = f"{p['name']} {p['version']}"
        _, used = satisfiable(p.get("license") or "")
        lic = next((u for u in used if u in ALLOWED), None)
        if lic in ("MIT", "MIT-0"):
            authors = ", ".join(re.sub(r"\s*<[^>]*>", "", x) for x in p.get("authors", [])) or f"the {p['name']} authors"
            t = f"MIT License\n\nCopyright (c) {authors}\n\n{MIT_BODY}"
            h = hashlib.sha256(norm(t).encode()).hexdigest()
            texts.setdefault(h, {"text": t, "users": [], "file": "(standard MIT text, the crate's authors)"})["users"].append(name)
            per_crate[cid] = [h]
            generated.append(name)
        elif lic in by_licence:
            h = by_licence[lic]
            texts[h]["users"].append(name + f" (no file of its own; the standard {lic} text)")
            per_crate[cid] = [h]
            generated.append(name)
        else:
            problems.append(f"{name}: no licence text (licence '{p.get('license')}')")

    order = sorted(texts.items(), key=lambda kv: (kv[1]["users"][0].lower(), kv[0]))
    num = {h: i + 1 for i, (h, _) in enumerate(order)}
    out = []
    out.append(f"Third-party licences of the Overbright programs ({a.target})\n")
    out.append("The player's programs (overbright, game, overbright-editor) are built from Overbright's own source")
    out.append("code (GPL-3.0-or-later, see LICENSE) and the Rust crates below, each under its own licence.")
    out.append("\"linked\" crates are compiled into the programs; \"build time\" crates (procedural macros and what")
    out.append("only they use) generate code while compiling and are not linked. Other third-party material (fonts,")
    out.append("shaders, algorithms) is listed in THIRD-PARTY-NOTICES.md. All crates come from")
    out.append("https://crates.io/crates/<name>/<version>.")
    out.append("After the crates' licence texts: the Rust standard library's notices and, for Windows, the")
    out.append("runtime libraries of the compiler toolchain that are linked into the programs.\n")
    w = max(len(f"{pkgs[c]['name']} {pkgs[c]['version']}") for c in crates)
    out.append(f"{'crate':<{w}}  {'use':<10}  licence (texts below)")
    for cid in crates:
        p = pkgs[cid]
        use = "linked" if cid in linked else "build time"
        refs = ",".join(f"#{num[h]}" for h in per_crate[cid])
        out.append(f"{p['name'] + ' ' + p['version']:<{w}}  {use:<10}  {p.get('license') or '?'}  [{refs}]")
    out.append(f"\n{len(crates)} crates ({len(linked)} linked, {len(crates) - len(linked)} build time), "
               f"{len(texts)} distinct licence texts.\n")
    for h, t in order:
        out.append("=" * 100)
        out.append(f"#{num[h]}  {t['file']}  used by: {', '.join(t['users'])}")
        out.append("=" * 100)
        out.append(t["text"])
    for title, f in a.section:
        t = Path(f).read_text(encoding="utf-8", errors="replace")
        if f.endswith(".html"):
            t = html_text(t)
        out.append("=" * 100)
        out.append(title)
        out.append("=" * 100)
        out.append(t.strip("\n") + "\n")
    Path(a.out).write_text("\n".join(out) + "\n", encoding="utf-8")
    print(f"crate_licenses: {a.target}: {len(crates)} crates ({len(linked)} linked), {len(texts)} texts, "
          f"{len(generated)} with a standard text, {len(problems)} problems -> {a.out}")
    for pr in problems:
        print(f"  problem: {pr}")
    return 1 if problems else 0


if __name__ == "__main__":
    sys.exit(main())
