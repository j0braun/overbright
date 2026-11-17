#!/usr/bin/env python3
"""Release scanner (docs/release-scan.md): finds what must not go into the public tree.

  scan.py text    [--list] [--family F] [--scope tree|public|all]   private paths, names, remarks, jargon, unit codes
  scan.py binary  [--list]                                          embedded / non-text files by class
  scan.py tables  [--list] [--fail]                                 exe-copied const tables (>= 24 entries)
  scan.py summary                                                   totals for all three (+ the release plan's numbers)
  scan.py archive DIR [--list] [--game-ref DIR_OR_FILE ...]          an unpacked release archive (package.sh)
  scan.py leak-terms                                                the private strings the binary leak checks search for
  scan.py allow-key PATH LINE                                       the allow.txt key of that line (=HASH of its trimmed text)
  scan.py unused-allow [--archive DIR ...]                          allow-list entries that match nothing any more (exit 1 if any)

Python 3 standard library only.  Exit status: text and summary exit 3 when families.private.txt is missing (unless --no-private); text and binary exit 1 on any hit that is neither allowed
(allow.txt / allow-binary.txt) nor (binary) under a path in private.list; tables exits 1 only with --fail.
"""
import argparse
import fnmatch
import hashlib
import os
import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

HERE = Path(__file__).resolve().parent
DEFAULT_ROOT = HERE.parents[1]

# The part of the tree that stays public whatever else is curated (release plan C-a.3).
PUBLIC_ROOTS = ["src/", "tests/", "scripts/checks/", "scripts/perf/", "build.rs", "Cargo.toml", "README.md"]
# The scanner's own files are scanned like any other (no exemption); the private pattern file is on private.list.
SELF_EXCLUDE = []
PRIVATE_FILE = "families.private.txt"
FAMILIES = ["private-path", "personal-name", "owner-remark", "owner-wording", "agent-wording", "hardware", "third-party-mod",
            "unit-code", "hyphen-code", "commit-ref"]
# Families counted in the plan's "private patterns" number (agent wording and unit codes are separate numbers).
PRIVATE_UNION = ["private-path", "personal-name", "owner-remark", "owner-wording", "hardware", "third-party-mod"]
# The plan's pattern P is data: the [plan-p] section of families.private.txt (summary comparison only).
PLAN_UNIT = re.compile(r"\b(G|D|P|NX|UX|DX|MF|LK|GS|BF|CK|CT|DR|TP|WR|E|S|Q|U|R|M|L|A)[0-9]{1,3}[a-h]?\b")

UNIT_RE = re.compile(r"\b[A-Z]{1,3}[0-9]{1,2}[a-z]?\b")
# Hyphenated work codes: groups of capitals joined by hyphens and ending in a hyphen and a number (with an
# optional lower-case letter); one to three capitals, a hyphen, a lower-case letter and a number; the
# native-fix series (a capital N, a hyphen and capitals, or capitals with a hyphen and one of the series words
# below) and the AI series name; and review finding ids (a lower-case area name, a hyphen, one digit and an
# optional letter, a hyphen, three digits). Real tokens of this shape (licence ids, encodings, CRC
# widths, car names) are listed in the [hyphen-allow] section of families.txt; a file name under
# docs/native-fixes/ is a name, not a remark (FIX_FILE_RE).
HYPHEN_RE = re.compile(r"\b(?:[A-Z]{1,6}(?:-[A-Z]{2,6})*-[0-9]{1,2}[a-z]?|[A-Z]{1,3}-[a-z][0-9]{1,2}"
                       r"|N-[A-Z]{2,5}[0-9]?|[A-Z]{2,5}-(?:FIX|SNAP|USER|PLAY|BUG)|A[I]-T"
                       r"|[a-z]{2,10}-[0-9][a-z]?-[0-9]{3})\b")
# A reference to a file under docs/native-fixes/ by its name: blanked before the code families look.
FIX_FILE_RE = re.compile(r"native-fixes/[A-Za-z0-9_-]+\.tsv")
COMMIT_RE = re.compile(r"\b(?:commit|main|base|merge|HEAD)\b(?P<gap>[^\n]{0,24}?)(?<![0-9A-Za-z_])(?P<h>[0-9a-f]{7,40})\b",
                       re.I)
COMMIT_KW = re.compile(r"commit|main|base|merge|HEAD", re.I)
BINARY_EXT = {"png", "webp", "jpg", "jpeg", "gif", "gz", "zlib", "bin", "sav", "zip", "ico", "ttf", "otf", "woff", "woff2",
              "wav", "ogg", "mp3", "pdf", "so", "exe", "dll"}


# ------------------------------------------------------------------ data files

# Sections that are not text-scan families: allow tokens, the plan's pattern, and the fixed strings the
# shipped-binary leak checks search for (package.sh, scripts/checks/release_check.sh; `leak-terms` mode).
NOT_PATTERNS = ("unit-allow", "hyphen-allow", "plan-p", "leak-strings")

def parse_families(text):
    """families text -> {section: [regex lines]} (comments and blank lines dropped)."""
    sections = defaultdict(list)
    cur = None
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        m = re.fullmatch(r"\[([a-z-]+)\]", line)
        if m:
            cur = m.group(1)
            continue
        sections[cur].append(line)
    return sections


def compile_families(sections):
    """-> (family -> [one compiled alternation], set of allowed unit tokens)."""
    def one(p):
        return "(?i:%s)" % p[4:] if p.startswith("(?i)") else "(?:%s)" % p
    fam = {k: [re.compile("|".join(one(p) for p in v))] for k, v in sections.items() if k not in NOT_PATTERNS}
    allow = {t for sec in ("unit-allow", "hyphen-allow") for line in sections.get(sec, [])
             for t in re.split(r"[\s,]+", line) if t}
    return fam, allow


def private_file(data=HERE):
    """families.private.txt: next to this script (the full tree, or a public checkout with the private set laid
    over it), else, for the scanner's own data folder, in the private set $OB_PRIVATE points at (docs/testing.md:
    a public checkout that keeps the set beside it)."""
    pf = Path(data) / PRIVATE_FILE
    if not pf.exists() and Path(data) == HERE and os.environ.get("OB_PRIVATE"):
        alt = Path(os.environ["OB_PRIVATE"]) / "scripts" / "release" / PRIVATE_FILE
        if alt.exists():
            return alt
    return pf


def load_families_ex(data=HERE, use_private=True):
    """-> (fam, unit_allow, plan_p regex or None, n_private_patterns or None when not loaded)."""
    sections = parse_families((Path(data) / "families.txt").read_text())
    n_private = None
    pf = private_file(data)
    if use_private and pf.exists():
        priv = parse_families(pf.read_text())
        n_private = sum(len(v) for k, v in priv.items() if k not in NOT_PATTERNS)
        for k, v in priv.items():
            sections[k] = sections.get(k, []) + v
    plan = sections.get("plan-p")
    plan_p = re.compile("|".join(plan), re.I) if plan else None
    fam, allow = compile_families(sections)
    return fam, allow, plan_p, n_private


def load_families(path):
    """Single-file loader (tests): -> (fam, unit_allow)."""
    return compile_families(parse_families(Path(path).read_text()))


def line_key(text):
    """The allow.txt key of one line: `=` and the first 12 hex digits of the SHA-256 of the line without its
    leading and trailing white space. It follows the line when lines above it are added or removed, and stops
    matching when the line itself is reworded (the hit is then reviewed again)."""
    return "=" + hashlib.sha256(text.strip().encode("utf-8", "surrogateescape")).hexdigest()[:12]


def load_allow(path):
    """allow.txt lines: path:key:family:reason. path is an fnmatch glob; key is `*` (every line of the file) or
    `=HASH` (the lines whose trimmed text has that hash, line_key); family may be *. Line numbers are refused:
    they break as soon as a line is inserted above the entry."""
    out = []
    p = Path(path)
    if not p.exists():
        return out
    for raw in p.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(":", 3)
        if len(parts) < 4:
            raise SystemExit(f"{path}: bad allow line (want path:key:family:reason): {raw}")
        if parts[1] != "*" and not re.fullmatch(r"=[0-9a-f]{12}", parts[1]):
            raise SystemExit(f"{path}: allow key must be * or =HASH (scan.py allow-key PATH LINE prints it), "
                             f"not a line number: {raw}")
        out.append((parts[0], parts[1], parts[2], parts[3]))
    return out


def load_globs(path):
    """allow-binary.txt lines: pattern:class:reason.  private.list lines: a path prefix/glob ('#' comments)."""
    out = []
    p = Path(path)
    if not p.exists():
        return out
    for raw in p.read_text().splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split(":", 2)
        out.append(tuple(x.strip() for x in parts) + ("",) * (3 - len(parts)))
    return out


def load_private(path):
    p = Path(path)
    if not p.exists():
        return []
    return [l.split("#")[0].strip() for l in p.read_text().splitlines() if l.split("#")[0].strip()]


def under(path, pats):
    for pat in pats:
        if pat.endswith("/"):
            if path.startswith(pat) or path == pat[:-1]:  # a file under the folder, or the folder itself
                return True
        elif path == pat or fnmatch.fnmatchcase(path, pat) or path.startswith(pat + "/"):
            return True
    return False


# ------------------------------------------------------------------ file access

def tracked_files(root):
    root = Path(root)
    try:
        out = subprocess.run(["git", "-C", str(root), "ls-files", "-z"], capture_output=True, check=True).stdout
        files = [f for f in out.decode("utf-8", "surrogateescape").split("\0") if f]
        return [f for f in files if (root / f).is_file()]
    except (subprocess.CalledProcessError, FileNotFoundError):
        pass
    files = []
    for d, dirs, names in os.walk(root):
        # Build output and analysis folders at the top only: a source folder such as dev/shot/targets/ is scanned.
        top = Path(d) == Path(root)
        dirs[:] = [x for x in dirs if x != ".git" and not (top and (x.startswith("target") or x == "analysis"))]
        for n in names:
            files.append(str((Path(d) / n).relative_to(root)))
    return sorted(files)


def is_text(root, rel):
    ext = rel.rsplit(".", 1)[-1].lower() if "." in rel.rsplit("/", 1)[-1] else ""
    if ext in BINARY_EXT:
        return False
    try:
        with open(Path(root) / rel, "rb") as f:
            return b"\0" not in f.read(8192)
    except OSError:
        return False


def read_text(root, rel):
    return (Path(root) / rel).read_text(encoding="utf-8", errors="replace")


# ------------------------------------------------------------------ text mode

def mask_code_rust(line):
    """Keep only comment text and string-literal contents of a Rust line (a constant's name is not a remark)."""
    line = re.sub(r"'(?:\\.|[^'\\])'", "' '", line)       # char literals
    line = re.sub(r"\\u\{[0-9A-Fa-f]+\}|\\x[0-9A-Fa-f]{2}", " ", line)
    out, i, n, in_str = [], 0, len(line), False
    while i < n:
        c = line[i]
        if in_str:
            if c == "\\":
                out.append("  ")
                i += 2
                continue
            if c == '"':
                in_str = False
                out.append(" ")
            else:
                out.append(c)
        elif c == '"':
            in_str = True
            out.append(" ")
        elif line.startswith("//", i):
            out.append(line[i:])
            break
        else:
            out.append(" ")
        i += 1
    return "".join(out)


def scan_text(path, text, fam, unit_allow):
    """-> list of (line_no, family, matched_text) for one file's text."""
    hits = []
    is_rs = path.endswith(".rs")
    for no, line in enumerate(text.split("\n"), 1):
        for name, regs in fam.items():
            for rg in regs:
                for m in rg.finditer(line):
                    hits.append((no, name, m.group(0)))
        unit_line = mask_code_rust(line) if is_rs else line
        unit_line = FIX_FILE_RE.sub(lambda m: " " * len(m.group(0)), unit_line)
        for m in UNIT_RE.finditer(unit_line):
            if m.group(0) not in unit_allow:
                hits.append((no, "unit-code", m.group(0)))
        for m in HYPHEN_RE.finditer(unit_line):
            if m.group(0) not in unit_allow:
                hits.append((no, "hyphen-code", m.group(0)))
        for m in (COMMIT_RE.finditer(line) if COMMIT_KW.search(line) else ()):
            h = m.group("h")
            if re.search(r"[0-9]", h) and re.search(r"[a-f]", h, re.I) and not line[max(0, m.start("h") - 2):m.start("h")] == "0x":
                hits.append((no, "commit-ref", m.group(0)))
    return sorted(set(hits))


def allowed(allow, path, text, family):
    """Is a hit of `family` in the line `text` of `path` allowed by an allow.txt entry?"""
    key = None
    for gp, gk, gf, _ in allow:
        if (gf == "*" or gf == family) and fnmatch.fnmatchcase(path, gp):
            if gk == "*":
                return True
            key = key or line_key(text)
            if gk == key:
                return True
    return False


def text_scope(root, files, private, scope):
    out = []
    for f in files:
        if under(f, SELF_EXCLUDE):
            continue
        if scope != "all" and under(f, private):
            continue
        if scope == "public" and not under(f, PUBLIC_ROOTS):
            continue
        out.append(f)
    return out


def run_text_scan(root, scope="tree", data=HERE, use_private=True, families=None):
    files = tracked_files(root)
    fam, unit_allow = families if families else load_families_ex(data, use_private)[:2]
    private = load_private(data / "private.list")
    allow = load_allow(data / "allow.txt")
    hits = []  # (path, line, family, match, allowed)
    for f in text_scope(root, files, private, scope):
        if not is_text(root, f):
            continue
        text = read_text(root, f)
        lines = text.split("\n")
        for no, family, match in scan_text(f, text, fam, unit_allow):
            hits.append((f, no, family, match, allowed(allow, f, lines[no - 1], family)))
    return hits


def family_counts(hits):
    files, lines = defaultdict(set), defaultdict(set)
    for p, no, fa, _m, ok in hits:
        if ok:
            continue
        files[fa].add(p)
        lines[fa].add((p, no))
    return files, lines


def union_counts(hits, fams):
    fs, ls = set(), set()
    for p, no, fa, _m, ok in hits:
        if fa in fams and not ok:
            fs.add(p)
            ls.add((p, no))
    return len(fs), len(ls)


def print_family_table(hits, out=sys.stdout):
    files, lines = family_counts(hits)
    print(f"{'family':<18}{'files':>7}{'lines':>8}", file=out)
    for fa in FAMILIES:
        print(f"{fa:<18}{len(files[fa]):>7}{len(lines[fa]):>8}", file=out)
    allfiles = {p for p, _n, _f, _m, ok in hits if not ok}
    alllines = {(p, n) for p, n, _f, _m, ok in hits if not ok}
    print(f"{'ANY family':<18}{len(allfiles):>7}{len(alllines):>8}", file=out)
    nallow = sum(1 for h in hits if h[4])
    if nallow:
        print(f"(allowed by allow.txt: {nallow} hits)", file=out)


def private_status(a):
    """-> (use_private, n_loaded or None, status line). Missing private data without --no-private means exit 3."""
    use = not a.no_private
    n = load_families_ex(HERE, use)[3]
    if not use:
        return use, n, "private patterns: DISABLED (--no-private); results are NOT a clean-tree claim"
    if n is None:
        return use, n, (f"private patterns: NOT LOADED ({PRIVATE_FILE} missing); the scan below cannot show the tree is clean "
                        f"(exit 3 unless --no-private)")
    return use, n, f"private patterns: loaded {n}"


def cmd_text(a):
    use, n, status = private_status(a)
    print(status, file=sys.stderr)
    hits = run_text_scan(a.root, a.scope, use_private=use)
    if a.family:
        hits = [h for h in hits if h[2] == a.family]
    if a.list:
        for p, no, fa, m, ok in hits:
            print(f"{p}:{no}:{fa}:{m}{'  [allowed]' if ok else ''}")
    print_family_table(hits, sys.stderr if a.list else sys.stdout)
    bad = [h for h in hits if not h[4]]
    if use and n is None:
        return 3
    return 1 if bad else 0


# ------------------------------------------------------------------ binary mode

CLASS_RULES = [  # (regex on the repo-relative path, class); first match wins
    (r"\.sav$", "personal-save"),
    (r"(^|/)native_save_fresh\.bin$|(^|/)(handling|ground-faces|routes|burnout-records)\.bin$|(^|/)fe-.*-car-efx\.txt$",
     "game-data"),
    (r"^tests/fixtures/overlay(/|$)", "machine-snapshot"),
    (r"^tests/fixtures/native_saves(/|$)", "personal-save"),
    (r"^tests/fixtures/(native_blocks|outrun-native|visual_rating)(/|$)", "capture"),
    (r"^docs/evidence/.*\.(elf|bin|gz|zlib|vram|plan|pose|cars|csv|tsv|txt|dis|asm|s|S|c|log|jsonl|frames|json)$", "capture"),
    (r"^(docs/evidence|docs/benchmarks|review|tools/parity/refs)/.*\.(png|webp|jpg|jpeg|gif)$|^docs/[^/]*\.(png|jpg|webp)$", "screenshot"),
    (r"\.(wgsl|rs|toml|md|json|txt)$", "asset"),
]


def classify(path):
    for rg, cls in CLASS_RULES:
        if re.search(rg, path):
            return cls
    return "unclassified"


INCLUDE_RE = re.compile(r'include_(bytes|str)!\s*\(\s*(?:concat!\s*\(\s*env!\s*\(\s*"CARGO_MANIFEST_DIR"\s*\)\s*,\s*)?"([^"]+)"')
MANIFEST_RE = re.compile(r'CARGO_MANIFEST_DIR')
STR_RE = re.compile(r'"([^"\n]{1,200})"')


# `#[cfg(test)]` or `#[cfg(all(test, ...))]` at any indentation: the item after it (a module, an impl's fn, a
# use) exists only in test builds.  `cfg(not(test))` and `cfg(any(test, ...))` do not count.
TEST_CFG_RE = re.compile(r"^\s*#\[cfg\((?:test|all\(\s*test\s*,[^\]]*)\)\]")
OUT_OF_LINE_MOD_RE = re.compile(r"^\s*(?:pub(?:\([^)]*\))?\s+)?mod\s+([A-Za-z_][A-Za-z_0-9]*)\s*;")
PATH_ATTR_RE = re.compile(r'^\s*#\[path\s*=\s*"([^"]+)"\s*\]')


def code_only(line):
    """A Rust line with string/char literal contents and // comments blanked (braces in them do not count)."""
    line = re.sub(r"'(?:\\.|[^'\\])'", "' '", line)
    out, i, n, in_str = [], 0, len(line), False
    while i < n:
        c = line[i]
        if in_str:
            if c == "\\":
                out.append("  ")
                i += 2
                continue
            in_str = c != '"'
            out.append(" ")
        elif c == '"':
            in_str = True
            out.append(" ")
        elif line.startswith("//", i):
            break
        else:
            out.append(c)
        i += 1
    return "".join(out)


def test_spans(lines):
    """-> [(first, last)] 0-based line spans of the items under a test cfg (attribute line to the item's end:
    its `;` or the brace closing its block)."""
    spans = []
    code = None
    for j, l in enumerate(lines):
        if not TEST_CFG_RE.match(l):
            continue
        if code is None:
            code = [code_only(x) for x in lines]
        k = j + 1
        while k < len(lines) and (code[k].strip().startswith("#[") or not code[k].strip()):
            k += 1
        depth, end = 0, None
        for m in range(k, len(lines)):
            for ch in code[m]:
                if ch == "{":
                    depth += 1
                elif ch == "}":
                    depth -= 1
                    if depth == 0:
                        end = m
                        break
                elif ch == ";" and depth == 0:
                    end = m
                    break
            if end is not None:
                break
        spans.append((j, end if end is not None else len(lines) - 1))
    return spans


def in_test_mod(lines, idx, spans=None):
    """Is line idx (0-based) inside an item under #[cfg(test)] (any indentation: a test module, or a test-only
    fn inside an impl)?"""
    spans = test_spans(lines) if spans is None else spans
    return any(a <= idx <= b for a, b in spans)


CRATE_ROOT_RE = re.compile(r"^(src/(lib|main)\.rs|src/bin/[^/]+\.rs|(tests|examples|benches)/[^/]+\.rs|build\.rs"
                           r"|[^/]+/src/(lib|main)\.rs|[^/]+/src/bin/[^/]+\.rs)$")


def test_only_files(root, files):
    """Rust files that are compiled only into tests: the module files of `#[cfg(test)] mod NAME;` declarations
    (with `#[path]`, or NAME.rs / NAME/mod.rs beside the declaring file's module directory), and every file
    under such a module's directory."""
    rs = [f for f in files if f.endswith(".rs")]
    have = set(rs)
    mods = set()
    for f in rs:
        text = read_text(root, f)
        if "cfg(" not in text or "mod " not in text:
            continue
        lines = text.split("\n")
        d = os.path.dirname(f)
        own = d if CRATE_ROOT_RE.match(f) or os.path.basename(f) in ("mod.rs", "lib.rs", "main.rs") else f[:-3]
        for j, l in enumerate(lines):
            if not TEST_CFG_RE.match(l):
                continue
            path = None
            k = j + 1
            while k < len(lines) and lines[k].strip().startswith("#["):
                pm = PATH_ATTR_RE.match(lines[k])
                path = pm.group(1) if pm else path
                k += 1
            m = OUT_OF_LINE_MOD_RE.match(lines[k]) if k < len(lines) else None
            if not m:
                continue
            if path:
                cands = [os.path.normpath(os.path.join(d, path))]
            else:
                cands = [os.path.join(own, m.group(1) + ".rs"), os.path.join(own, m.group(1), "mod.rs")]
            for c in cands:
                if c in have:
                    mods.add(c)
    out = set(mods)
    for m in mods:
        sub = os.path.dirname(m) if os.path.basename(m) == "mod.rs" else m[:-3]
        out.update(f for f in rs if f.startswith(sub + "/"))
    return out


def find_refs(root, files):
    """-> list of dicts for include_bytes!/include_str!/CARGO_MANIFEST_DIR references in tracked Rust sources."""
    refs = []
    test_files = test_only_files(root, files)
    for f in files:
        if not f.endswith(".rs"):
            continue
        text = read_text(root, f)
        if "include_" not in text and "CARGO_MANIFEST_DIR" not in text:
            continue
        lines = text.split("\n")
        spans = test_spans(lines)
        whole = f in test_files
        starts = [0]
        for l in lines:
            starts.append(starts[-1] + len(l) + 1)

        def line_of(pos):
            lo, hi = 0, len(starts) - 1
            while lo < hi - 1:
                mid = (lo + hi) // 2
                if starts[mid] <= pos:
                    lo = mid
                else:
                    hi = mid
            return lo
        used = []
        for m in INCLUDE_RE.finditer(text):
            kind, lit = m.group(1), m.group(2)
            manifest = "CARGO_MANIFEST_DIR" in m.group(0)
            used.append(m.span())
            target = os.path.normpath(lit.lstrip("/") if manifest else os.path.join(os.path.dirname(f), lit))
            ln = line_of(m.start())
            refs.append(dict(src=f, line=ln + 1, kind="include_" + kind, target=target, test=whole or in_test_mod(lines, ln, spans)))
        for m in MANIFEST_RE.finditer(text):
            if any(a <= m.start() < b for a, b in used):
                continue
            ln = line_of(m.start())
            tail = text[m.end():m.end() + 400]
            if tail.startswith('"'):
                tail = tail[1:]
            lit = STR_RE.search(tail.split(";")[0])
            target = os.path.normpath(lit.group(1).lstrip("/")) if lit and "/" in lit.group(1) else "(dynamic)"
            if target.startswith(("CARGO", "..")):
                target = "(dynamic)"
            refs.append(dict(src=f, line=ln + 1, kind="manifest-dir", target=target, test=whole or in_test_mod(lines, ln, spans)))
    return refs


def size_of(root, rel):
    p = Path(root) / rel
    if p.is_file():
        return p.stat().st_size
    if p.is_dir():
        return sum(x.stat().st_size for x in p.rglob("*") if x.is_file())
    return None


def run_binary_scan(root, data=HERE):
    files = tracked_files(root)
    private = load_private(data / "private.list")
    allow = load_globs(data / "allow-binary.txt")
    allow_pats = [a[0] for a in allow]
    refs = find_refs(root, files)
    for r in refs:
        t = r["target"]
        r["cls"] = classify(t) if t != "(dynamic)" else "dynamic"
        r["size"] = size_of(root, t) if t != "(dynamic)" else None
        r["shipping"] = r["src"].startswith("src/") and not r["test"]
        if t == "(dynamic)":
            r["status"] = "ok" if under(r["src"], allow_pats) else "FAIL"
            r["why"] = "" if r["status"] == "ok" else "dynamic data path"
        elif under(t, allow_pats):
            r["status"], r["why"] = "ok", "allow-binary"
        elif under(t, private) and not r["shipping"]:
            r["status"], r["why"] = "ok", "private path, test/tool use"
        elif under(t, private):
            r["status"], r["why"] = "FAIL", "private data embedded in shipping src"
        else:
            r["status"], r["why"] = "FAIL", "not allowed"
    nontext = []
    for f in files:
        if under(f, SELF_EXCLUDE) or is_text(root, f):
            continue
        cls = classify(f)
        if under(f, allow_pats):
            st, why = "ok", "allow-binary"
        elif under(f, private):
            st, why = "ok", "private path"
        else:
            st, why = "FAIL", "not allowed"
        nontext.append(dict(path=f, cls=cls, size=size_of(root, f), status=st, why=why))
    return refs, nontext


def binary_numbers(refs, nontext):
    inc = [r for r in refs if r["kind"].startswith("include_")]
    distinct = {r["target"] for r in inc}
    ev = {r["target"] for r in inc if re.search(r"evidence|fixtures|native_save_fresh", r["target"])}
    ev_lit = {(os.path.dirname(r["src"]), r["target"]) for r in inc if re.search(r"evidence|fixtures|native_save_fresh", r["target"])}
    manifest = [r for r in refs if r["kind"] == "manifest-dir" or "CARGO_MANIFEST_DIR" in r["kind"]]
    return dict(includes=len(inc), distinct=len(distinct), evidence_distinct=len(ev), manifest=len(manifest),
                shipping=len({r["target"] for r in inc if r["shipping"] and r["target"] in ev}),
                nontext=len(nontext), nontext_fail=sum(1 for n in nontext if n["status"] == "FAIL"),
                refs_fail=sum(1 for r in refs if r["status"] == "FAIL"))


def cmd_binary(a):
    refs, nontext = run_binary_scan(a.root)
    if a.list:
        for r in sorted(refs, key=lambda r: (r["src"], r["line"])):
            sz = "?" if r["size"] is None else r["size"]
            print(f"REF {r['src']}:{r['line']}: {r['kind']} -> {r['target']} [{sz} B] class={r['cls']} "
                  f"{'shipping' if r['shipping'] else 'test-only'} {r['status']} ({r['why']})")
        for n in nontext:
            print(f"FILE {n['path']} [{n['size']} B] class={n['cls']} {n['status']} ({n['why']})")
    print_binary_table(refs, nontext)
    return 1 if any(r["status"] == "FAIL" for r in refs) or any(n["status"] == "FAIL" for n in nontext) else 0


def print_binary_table(refs, nontext, out=sys.stdout):
    cls_refs, cls_files = defaultdict(set), defaultdict(lambda: [0, 0])
    for r in refs:
        cls_refs[r["cls"]].add(r["target"])
    for n in nontext:
        cls_files[n["cls"]][0] += 1
        cls_files[n["cls"]][1] += n["size"] or 0
    nums = binary_numbers(refs, nontext)
    print(f"references: {len(refs)} ({nums['includes']} include_*!, {nums['manifest']} CARGO_MANIFEST_DIR reads), "
          f"{nums['distinct']} distinct include targets, of which {nums['evidence_distinct']} under evidence|fixtures|native_save_fresh "
          f"({nums['shipping']} of those compiled into shipping src outside #[cfg(test)])", file=out)
    print(f"{'class':<18}{'ref targets':>12}{'tracked non-text files':>24}{'bytes':>12}", file=out)
    for c in sorted(set(cls_refs) | set(cls_files)):
        print(f"{c:<18}{len(cls_refs[c]):>12}{cls_files[c][0]:>24}{cls_files[c][1]:>12}", file=out)
    print(f"tracked non-text files: {nums['nontext']}; failing: {nums['nontext_fail']}; failing references: {nums['refs_fail']}", file=out)


# ------------------------------------------------------------------ tables mode

DECL_RE = re.compile(r"^[ \t]*(?:pub(?:\([^)]*\))?\s+)?(?:const|static)\s+([A-Za-z_][A-Za-z_0-9]*)\s*:\s*([^=]*?)=\s*&?\s*(?:\w+\s*)?\[", re.M)
ADDR_RE = re.compile(r"0x[0-9A-Fa-f]{5,8}\b|\bSPEED2")
MIN_ENTRIES = 24


def count_entries(text, i):
    """text[i] == '[': number of top-level elements of the array expression (handles `[x; N]`)."""
    depth = n = 0
    has = False
    j, L = i, len(text)
    while j < L:
        c = text[j]
        if c == '"':
            j += 1
            while j < L and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            has = True
        elif c == "'" and re.match(r"'(?:\\.|[^'\\])'", text[j:j + 6]):
            j += len(re.match(r"'(?:\\.|[^'\\])'", text[j:j + 6]).group(0))
            has = True
            continue
        elif text.startswith("//", j):
            j = text.find("\n", j)
            if j < 0:
                break
            continue
        elif c in "[({":
            depth += 1
            if depth > 1:
                has = True
        elif c in "])}":
            depth -= 1
            if depth == 0:
                break
        elif c == "," and depth == 1:
            n += 1
            has = False
        elif c == ";" and depth == 1:
            m = re.match(r";\s*([0-9_]+)\s*\]", text[j:])
            if m:
                return int(m.group(1).replace("_", ""))
        elif not c.isspace():
            has = True
        j += 1
    return n + (1 if has else 0)


def comment_block_above(lines, idx):
    """Contiguous // or /// or #[..] lines directly above 0-based line idx."""
    out, k = [], idx - 1
    while k >= 0 and (lines[k].strip().startswith(("//", "#["))):
        out.append(lines[k])
        k -= 1
    return "\n".join(out)


def scan_tables(path, text):
    """-> [(line, name, entries, doc_source)] for const/static arrays >= 24 entries whose doc names an address."""
    res = []
    lines = text.split("\n")
    for m in DECL_RE.finditer(text):
        ln = text.count("\n", 0, m.start())
        doc = comment_block_above(lines, ln)
        src = "doc"
        if not ADDR_RE.search(doc) and lines[ln].startswith((" ", "\t")):
            # function-local table: the enclosing fn's doc counts
            k = ln - 1
            while k >= 0 and not re.match(r"\s*(pub(\([^)]*\))?\s+)?(const\s+)?fn\s", lines[k]):
                k -= 1
            if k >= 0:
                doc, src = comment_block_above(lines, k), "fn-doc"
        if not ADDR_RE.search(doc):
            continue
        n = count_entries(text, m.end() - 1)
        if n >= MIN_ENTRIES:
            res.append((ln + 1, m.group(1), n, src))
    return res


def run_tables(root):
    out = []
    for f in tracked_files(root):
        if f.endswith(".rs") and not under(f, SELF_EXCLUDE):
            for ln, name, n, src in scan_tables(f, read_text(root, f)):
                out.append((f, ln, name, n, src))
    return out


def cmd_tables(a):
    tabs = run_tables(a.root)
    if a.list:
        for f, ln, name, n, src in tabs:
            print(f"{f}:{ln}: {name} {n} entries ({src})")
    print(f"tables: {len(tabs)} const arrays of >= {MIN_ENTRIES} entries naming an executable address, "
          f"{sum(t[3] for t in tabs)} entries in total, {len({t[0] for t in tabs})} files")
    return 1 if (a.fail and tabs) else 0


# ------------------------------------------------------------------ summary

def plan_numbers(root, files, private, plan_p):
    pub = [f for f in files if under(f, PUBLIC_ROOTS) and not under(f, SELF_EXCLUDE) and is_text(root, f)]
    pf = pl = 0
    uf = ul = 0
    for f in pub:
        t = read_text(root, f)
        n = sum(1 for l in t.split("\n") if plan_p.search(l))
        pf += n > 0
        pl += n
        if f.startswith(("src/", "tests/")):
            u = sum(1 for l in t.split("\n") if PLAN_UNIT.search(l))
            uf += u > 0
            ul += u
    return pf, pl, uf, ul


def cmd_summary(a):
    root = a.root
    files = tracked_files(root)
    private = load_private(HERE / "private.list")
    use, n, status = private_status(a)
    print(status)
    plan_p = load_families_ex(HERE, use)[2]
    print(f"tracked files: {len(files)}; text scope (tree minus private.list): "
          f"{len(text_scope(root, files, private, 'tree'))}; public roots: {', '.join(PUBLIC_ROOTS)}")
    print("\n== text: tracked tree minus private.list (--scope tree)")
    tree = run_text_scan(root, "tree", use_private=use)
    print_family_table(tree)
    print("\n== text: public roots only (--scope public)")
    pub = run_text_scan(root, "public", use_private=use)
    print_family_table(pub)
    print("\n== text: everything tracked incl. private.list paths (--scope all)")
    allh = run_text_scan(root, "all", use_private=use)
    print_family_table(allh)
    pub_all = [h for h in allh if under(h[0], PUBLIC_ROOTS)]
    pf, pl = union_counts(pub_all, PRIVATE_UNION)
    uf, ul = family_counts(pub_all)[0]["unit-code"], family_counts(pub_all)[1]["unit-code"]
    ppf, ppl, puf, pul = plan_numbers(root, files, private, plan_p) if plan_p else (-1, -1, -1, -1)
    print("\n== release plan numbers vs this scan")
    print(f"private patterns, public roots (paths in private.list counted too): plan 79 files / 933 lines; plan pattern P re-run on this tree: {ppf} / {ppl}; "
          f"scanner families {'+'.join(PRIVATE_UNION)}: {pf} / {pl}")
    print(f"unit codes in src+tests: plan <= 190 files / <= 1828 lines (its upper-bound regex, re-run: {puf} / {pul}); "
          f"scanner unit-code family (allow list + code identifiers masked), public roots incl. paths that will move: {len(uf)} / {len(ul)}")
    refs, nontext = run_binary_scan(root)
    print("\n== binary")
    print_binary_table(refs, nontext)
    tabs = run_tables(root)
    print("\n== tables")
    print(f"tables: {len(tabs)} const arrays of >= {MIN_ENTRIES} entries naming an executable address "
          f"({sum(1 for t in tabs if t[4] == 'fn-doc')} of them function-local, doc taken from the enclosing fn); plan: 43")
    print("\n" + status)
    return 3 if (use and n is None) else 0



# ------------------------------------------------------------------ archive mode (scripts/release/package.sh)

# The privacy families; unit codes and commit ids are development jargon, not private, and licence texts are
# full of tokens like them.
ARCHIVE_FAMILIES = ["private-path", "personal-name", "owner-remark", "owner-wording", "agent-wording", "hardware", "third-party-mod"]
# Extra patterns for binaries: Windows home folders and the build's own scratch places.
ARCHIVE_BINARY_EXTRA = [re.compile(r"[A-Za-z]:\\Users\\[^\\\x00]+", re.I), re.compile(r"/root/|/Users/[A-Za-z]")]
GAME_WINDOW = 32        # bytes compared per fingerprint
GAME_MIN_RUN = 64       # a run of at least this many bytes equal to a reference file fails
GAME_MIN_DISTINCT = 12  # a fingerprint window needs this many distinct byte values (no padding, no fills)


def load_archive_allow(path):
    """allow-archive.txt: glob:family:regex:reason. A hit in a file matching glob, of that family (or *), in a
    line (text) or string (binary) that contains regex (re.search) is allowed: the regex names the context, so
    the same words elsewhere still fail."""
    out = []
    p = Path(path)
    if p.exists():
        for raw in p.read_text().splitlines():
            line = raw.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split(":", 3)
            if len(parts) < 4:
                raise SystemExit(f"{path}: bad line (want glob:family:regex:reason): {raw}")
            out.append((parts[0], parts[1], re.compile(parts[2]), parts[3]))
    return out


def binary_strings(data, n=6):
    """-> [(offset, text)]: printable ASCII runs and UTF-16LE runs (Windows resources and wide strings)."""
    out = [(m.start(), m.group().decode("ascii")) for m in re.finditer(rb"[\x20-\x7e]{%d,}" % n, data)]
    out += [(m.start(), m.group().decode("utf-16-le")) for m in re.finditer(rb"(?:[\x20-\x7e]\x00){%d,}" % n, data)]
    return out


def archive_files(root):
    root = Path(root)
    return sorted(str(p.relative_to(root)) for p in root.rglob("*") if p.is_file() or p.is_symlink())


def run_archive_text(root, data=HERE, use_private=True, families=None):
    """-> [(path, where, family, match, allowed)]: text files line by line, other files through their strings."""
    root = Path(root)
    fam = families if families else load_families_ex(data, use_private)[0]
    fam = {k: v for k, v in fam.items() if k in ARCHIVE_FAMILIES}
    allow = load_archive_allow(Path(data) / "allow-archive.txt")
    hits = []
    for rel in archive_files(root):
        raw = (root / rel).read_bytes()
        if is_text(root, rel):
            items = [(f"line {no}", line) for no, line in enumerate(raw.decode("utf-8", "replace").split("\n"), 1)]
            extra = []
        else:
            items = [(f"0x{off:x}", t) for off, t in binary_strings(raw)]
            extra = ARCHIVE_BINARY_EXTRA
        for where, line in items:
            found = [(name, m.group(0)) for name, regs in fam.items() for rg in regs for m in rg.finditer(line)]
            found += [("private-path", m.group(0)) for rg in extra for m in rg.finditer(line)]
            for family, match in found:
                ok = any((gf == "*" or gf == family) and fnmatch.fnmatchcase(rel, gp) and rx.search(line)
                         for gp, gf, rx, _ in allow)
                hits.append((rel, where, family, match, ok))
    return hits


def reference_files(refs):
    out = []
    for r in refs:
        r = Path(r)
        if r.is_file():
            out.append(r)
        elif r.is_dir():
            out += sorted(p for p in r.rglob("*") if p.is_file())
    return out


def _window_hash(a, offs=None):
    """A 64-bit hash of the GAME_WINDOW bytes of uint8 array `a` at each offset in `offs` (None: every offset)
    (numpy, wrapping arithmetic)."""
    import numpy as np
    n = len(a) - GAME_WINDOW + 1 if offs is None else len(offs)
    h = np.zeros(n, dtype=np.uint64)
    for w in range(GAME_WINDOW // 8):
        k = np.zeros(n, dtype=np.uint64)
        for j in range(8):
            c = 8 * w + j
            byte = a[c:c + n] if offs is None else a[offs + c]
            k |= byte.astype(np.uint64) << np.uint64(8 * j)
        h = (h ^ k) * np.uint64(0x9E3779B97F4A7C15) + np.uint64(w + 1)
    return h


def _crc32_tables():
    """The standard CRC-32 tables (polynomial 0x04C11DB7): reflected (zlib, PNG) and MSB-first, as little-endian
    32-bit values."""
    refl, msb = [], []
    for n in range(256):
        c = n
        for _ in range(8):
            c = (0xEDB88320 ^ (c >> 1)) if c & 1 else c >> 1
        refl.append(c)
        c = n << 24
        for _ in range(8):
            c = ((c << 1) ^ 0x04C11DB7) & 0xFFFFFFFF if c & 0x80000000 else (c << 1) & 0xFFFFFFFF
        msb.append(c)
    return [b"".join(x.to_bytes(4, "little") for x in t) for t in (refl, msb)]


CRC32_TABLES = _crc32_tables()
# Bytes every Windows executable carries: the MS-DOS stub and the version resource's structure names.
PE_COMMON = [b"\x0e\x1f\xba\x0e\x00\xb4\x09\xcd\x21\xb8\x01\x4c\xcd\x21This program cannot be run in DOS mode.\r\r\n$",
             "VarFileInfo".encode("utf-16-le"), "StringFileInfo".encode("utf-16-le"), "Translation".encode("utf-16-le")]


def _step_len(run, pos, width):
    """Bytes from `pos` that form a step table of `width`-byte values: non-decreasing, each step 0 or a power
    of two."""
    end, prev = pos, None
    while end + width <= len(run):
        v = int.from_bytes(run[end:end + width], "little")
        if prev is not None:
            d = v - prev
            if d < 0 or d & (d - 1):
                break
        prev, end = v, end + width
    return end - pos


def generic_run(run):
    """True for a byte run that any program may hold and is no game data: a piece of a standard CRC-32
    table, a run of fewer than 24 non-zero bytes, a short run around the MS-DOS stub or the version resource's
    structure names that every Windows executable has, or at most three step tables back to back (each a non-decreasing sequence of 8-, 16-
    or 32-bit values whose steps are 0 or a power of two: identity tables, deflate's length/distance code,
    base and extra-bits tables as zlib and miniz store them), allowing a few cut bytes at the ends."""
    core = run.strip(b"\0")
    if len(core) >= 16 and any(core in t for t in CRC32_TABLES):
        return True
    if sum(1 for x in run if x) < 24:   # almost all zero: padding around a few values
        return True
    if any(p in run for p in PE_COMMON) and len(run) <= 160:
        return True
    pos, segments = 0, 0
    while pos < len(core):
        if len(core) - pos <= 8:   # a cut value or a neighbour's bytes at the end
            return segments > 0
        best = max(_step_len(core, p, w) + (p - pos) for w in (1, 2, 4) for p in range(pos, min(pos + 4, len(core))))
        if best < 16:
            return False
        pos += best
        segments += 1
        if segments > 3:
            return False
    return segments > 0


def game_data_runs(binaries, refs, min_run=GAME_MIN_RUN, log=None, generic=None):
    """Byte runs of >= min_run in `binaries` (paths) equal to a part of a reference file (the player's game
    files, saves, captures). Fingerprints: hashes of GAME_WINDOW-byte windows of each reference file at a
    stride (16 in executables and files < 64 KiB, 32 in files < 4 MiB, 1024 above: a run of >= stride +
    GAME_WINDOW - 1 bytes always holds one), skipping fills and padding (< GAME_MIN_DISTINCT distinct
    bytes); every offset of every binary is hashed the same way (numpy), the hits are verified byte by byte
    and extended both ways. -> [(binary, offset, ref file, ref offset, length)]."""
    import numpy as np
    hashes, owner = [], []
    files = reference_files(refs)
    for fi, f in enumerate(files):
        size = f.stat().st_size
        if size < GAME_WINDOW:
            continue
        stride = 16 if (size < 65536 or f.suffix.lower() in (".exe", ".dll")) else 32 if size < (4 << 20) else 1024
        a = np.fromfile(f, dtype=np.uint8)
        offs = np.arange(0, size - GAME_WINDOW + 1, stride, dtype=np.int64)
        win = np.sort(a[offs[:, None] + np.arange(GAME_WINDOW)], axis=1)
        offs = offs[1 + (win[:, 1:] != win[:, :-1]).sum(axis=1) >= GAME_MIN_DISTINCT]
        if len(offs):
            hashes.append(_window_hash(a, offs))
            owner.append(np.stack([np.full(len(offs), fi, dtype=np.int64), offs], axis=1))
    if not hashes:
        return []
    hashes, owner = np.concatenate(hashes), np.concatenate(owner)
    order = np.argsort(hashes, kind="stable")
    hashes, owner = hashes[order], owner[order]
    if log:
        log(f"game data: {len(files)} reference files, {len(hashes)} fingerprints")
    cache = {}

    def ref_bytes(fi):
        if fi not in cache:
            cache[fi] = files[fi].read_bytes()
        return cache[fi]

    runs = []
    for b in binaries:
        data = Path(b).read_bytes()
        if len(data) < GAME_WINDOW:
            continue
        arr = np.frombuffer(data, dtype=np.uint8)
        bh = _window_hash(arr)
        cand = np.nonzero(np.isin(bh, hashes))[0]
        covered_until = -1
        for i in cand.tolist():
            if i < covered_until:
                continue
            lo, hi = np.searchsorted(hashes, bh[i], "left"), np.searchsorted(hashes, bh[i], "right")
            best = None
            for fi, off in owner[lo:hi].tolist():
                ref = ref_bytes(fi)
                if data[i:i + GAME_WINDOW] != ref[off:off + GAME_WINDOW]:
                    continue
                s_b, s_r = i, off   # extend backwards and forwards
                while s_b > 0 and s_r > 0 and data[s_b - 1] == ref[s_r - 1]:
                    s_b, s_r = s_b - 1, s_r - 1
                e_b, e_r = i + GAME_WINDOW, off + GAME_WINDOW
                while e_b < len(data) and e_r < len(ref) and data[e_b] == ref[e_r]:
                    e_b, e_r = e_b + 1, e_r + 1
                if best is None or e_b - s_b > best[4]:
                    best = (str(b), s_b, str(files[fi]), s_r, e_b - s_b)
            if best and best[4] >= min_run:
                covered_until = best[1] + best[4]
                if not generic_run(data[best[1]:best[1] + best[4]]):
                    runs.append(best)
                elif generic is not None:
                    generic.append(best)
    return runs


def cmd_archive(a):
    use, n, status = private_status(a)
    print(status, file=sys.stderr)
    if use and n is None:
        return 3
    root = Path(a.dir)
    hits = run_archive_text(root, use_private=use)
    bad = [h for h in hits if not h[4]]
    for p, where, fa, m, ok in (hits if a.list else bad):
        print(f"  {p}:{where}:{fa}:{m}{'  [allowed]' if ok else ''}")
    nfiles = len(archive_files(root))
    print(("ok    " if not bad else "FAIL  ") + f"archive text/strings: {nfiles} files, {len(hits)} hits, {len(bad)} not allowed")
    fail = bool(bad)
    if a.game_ref:
        bins = [root / f for f in archive_files(root) if not is_text(root, f)]
        gen = []
        try:
            runs = game_data_runs(bins, a.game_ref, log=lambda s: print(f"      {s}"), generic=gen)
        except ImportError:
            print("FAIL  embedded game data: numpy is needed for this check")
            return 1
        runs.sort(key=lambda r: -r[4])
        for b, off, ref, roff, ln in runs:
            print(f"  {Path(b).relative_to(root)}+0x{off:x}: {ln} bytes equal to {Path(ref).name}+0x{roff:x}")
        for b, off, ref, roff, ln in (gen if a.list else []):
            print(f"      generic (allowed): {Path(b).relative_to(root)}+0x{off:x} {ln} bytes = {Path(ref).name}+0x{roff:x}")
        by_ref = defaultdict(lambda: [0, 0])
        for r in runs:
            by_ref[Path(r[2]).name][0] += 1
            by_ref[Path(r[2]).name][1] += r[4]
        per = ", ".join(f"{k} {n} runs / {sz} bytes" for k, (n, sz) in sorted(by_ref.items(), key=lambda kv: -kv[1][1]))
        print(("ok    " if not runs else "FAIL  ") + f"embedded game data: {len(bins)} binaries, {len(runs)} runs of >= "
              f"{GAME_MIN_RUN} bytes equal to the reference files{': ' + per if per else ''} "
              f"({len(gen)} generic tables allowed: CRC-32, step tables, PE boilerplate)")
        fail |= bool(runs)
    return 1 if fail else 0


def cmd_leak_terms(a):
    """The [leak-strings] section of families.private.txt, one string per line, for the shipped-binary leak
    checks. Without the private file: a NOT LOADED line on stderr, nothing on stdout, exit 3."""
    pf = private_file()
    if not pf.exists():
        print(f"private leak terms: NOT LOADED ({PRIVATE_FILE} missing)", file=sys.stderr)
        return 3
    terms = parse_families(pf.read_text()).get("leak-strings", [])
    print(f"private leak terms: loaded {len(terms)}", file=sys.stderr)
    for t in terms:
        print(t)
    return 0


def cmd_allow_key(a):
    """`allow-key PATH LINE`: the key an allow.txt entry for that line uses, and the line."""
    if not a.dir or not a.line or not a.line.isdigit():
        print("usage: scan.py allow-key PATH LINE", file=sys.stderr)
        return 2
    lines = read_text(a.root, a.dir).split("\n")
    no = int(a.line)
    if not 0 < no <= len(lines):
        print(f"{a.dir} has no line {no}", file=sys.stderr)
        return 2
    print(f"{a.dir}:{line_key(lines[no - 1])}:FAMILY:REASON    # {lines[no - 1].strip()[:100]}")
    return 0


# ------------------------------------------------------------------ unused-allow mode

def _entry_lines(path):
    """-> [(line number, text)] of the entries of an allow file, in load order (comments and blanks skipped)."""
    p = Path(path)
    if not p.exists():
        return []
    return [(no, raw.strip()) for no, raw in enumerate(p.read_text().splitlines(), 1)
            if raw.strip() and not raw.strip().startswith("#")]


def unused_text_allow(root, data=HERE, use_private=True, families=None):
    """allow.txt entries that allow no hit: -> (entries, unused, private_only). The text scan runs over every
    tracked file (scope all) and an entry counts as used by every hit it matches (not only the first entry
    that allows it). private_only: entries whose hits all lie in private.list files, which the default scope
    skips (they still serve `text --scope all`)."""
    data = Path(data)
    files = tracked_files(root)
    fam, unit_allow = families if families else load_families_ex(data, use_private)[:2]
    private = load_private(data / "private.list")
    allow = load_allow(data / "allow.txt")
    used, used_public = set(), set()
    for f in text_scope(root, files, private, "all"):
        if not is_text(root, f):
            continue
        hits = scan_text(f, read_text(root, f), fam, unit_allow)
        if not hits:
            continue
        lines = read_text(root, f).split("\n")
        pub = not under(f, private)
        for no, family, _m in hits:
            key = None
            for i, (gp, gk, gf, _) in enumerate(allow):
                if (gf == "*" or gf == family) and fnmatch.fnmatchcase(f, gp):
                    if gk != "*":
                        key = key or line_key(lines[no - 1])
                        if gk != key:
                            continue
                    used.add(i)
                    if pub:
                        used_public.add(i)
    entries = _entry_lines(data / "allow.txt")
    unused = [entries[i] for i in range(len(allow)) if i not in used]
    private_only = [entries[i] for i in range(len(allow)) if i in used and i not in used_public]
    return entries, unused, private_only


def unused_binary_allow(root, data=HERE):
    """allow-binary.txt entries that no include/manifest-dir target, dynamic-read source or tracked non-text
    file matches: -> (entries, unused)."""
    data = Path(data)
    files = tracked_files(root)
    allow = load_globs(data / "allow-binary.txt")
    subjects = set()
    for r in find_refs(root, files):
        subjects.add(r["src"] if r["target"] == "(dynamic)" else r["target"])
    subjects |= {f for f in files if not under(f, SELF_EXCLUDE) and not is_text(root, f)}
    entries = _entry_lines(data / "allow-binary.txt")
    unused = [entries[i] for i, a in enumerate(allow) if not any(under(x, [a[0]]) for x in subjects)]
    return entries, unused


def unused_archive_allow(dirs, data=HERE, use_private=True, families=None):
    """allow-archive.txt entries that allow no hit in any of the unpacked archive folders `dirs`:
    -> (entries, unused)."""
    data = Path(data)
    fam = families if families else load_families_ex(data, use_private)[0]
    fam = {k: v for k, v in fam.items() if k in ARCHIVE_FAMILIES}
    allow = load_archive_allow(data / "allow-archive.txt")
    used = set()
    for d in dirs:
        root = Path(d)
        for rel in archive_files(root):
            raw = (root / rel).read_bytes()
            items = (raw.decode("utf-8", "replace").split("\n") if is_text(root, rel)
                     else [t for _o, t in binary_strings(raw)])
            for line in items:
                found = [name for name, regs in fam.items() for rg in regs if rg.search(line)]
                found += ["private-path" for rg in ARCHIVE_BINARY_EXTRA if rg.search(line)]
                for family in found:
                    for i, (gp, gf, rx, _) in enumerate(allow):
                        if (gf == "*" or gf == family) and fnmatch.fnmatchcase(rel, gp) and rx.search(line):
                            used.add(i)
    entries = _entry_lines(data / "allow-archive.txt")
    return entries, [entries[i] for i in range(len(allow)) if i not in used]


def cmd_unused_allow(a):
    """`unused-allow [--archive DIR ...]`: the allow-list entries that match nothing any more."""
    use, n, status = private_status(a)
    print(status, file=sys.stderr)
    if use and n is None:
        return 3  # without the private families, entries for their hits would look unused
    bad = 0
    entries, unused, private_only = unused_text_allow(a.root, use_private=use)
    for no, e in unused:
        print(f"UNUSED allow.txt:{no}: {e}")
    for no, e in private_only:
        print(f"private-only allow.txt:{no}: {e}")
    print(f"allow.txt: {len(entries)} entries, {len(unused)} unused, {len(private_only)} only for private.list files")
    bad += len(unused)
    entries, unused = unused_binary_allow(a.root)
    for no, e in unused:
        print(f"UNUSED allow-binary.txt:{no}: {e}")
    print(f"allow-binary.txt: {len(entries)} entries, {len(unused)} unused")
    bad += len(unused)
    if a.archive:
        entries, unused = unused_archive_allow(a.archive, use_private=use)
        for no, e in unused:
            print(f"UNUSED allow-archive.txt:{no}: {e}")
        print(f"allow-archive.txt: {len(entries)} entries, {len(unused)} unused (archives: {', '.join(a.archive)})")
        bad += len(unused)
    else:
        print("allow-archive.txt: not checked (--archive DIR: an unpacked release archive folder, repeatable)")
    return 1 if bad else 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("mode", choices=["text", "binary", "tables", "summary", "archive", "leak-terms", "allow-key", "unused-allow"])
    ap.add_argument("dir", nargs="?", help="archive: the unpacked archive's folder; allow-key: the file (repo-relative)")
    ap.add_argument("line", nargs="?", help="allow-key: the line number")
    ap.add_argument("--game-ref", action="append", default=[],
                    help="archive: a file or folder whose bytes must not be embedded (the game, saves); repeatable")
    ap.add_argument("--root", default=str(DEFAULT_ROOT))
    ap.add_argument("--list", action="store_true", help="print every hit")
    ap.add_argument("--family", choices=FAMILIES)
    ap.add_argument("--scope", choices=["tree", "public", "all"], default="tree",
                    help="tree: tracked minus private.list (default); public: only the always-public roots; all: also private.list paths")
    ap.add_argument("--no-private", action="store_true",
                    help="text/summary: do not load families.private.txt (without it, a missing file exits 3)")
    ap.add_argument("--fail", action="store_true", help="tables: exit 1 when any table is found")
    ap.add_argument("--archive", action="append", default=[],
                    help="unused-allow: an unpacked release archive folder for allow-archive.txt; repeatable")
    a = ap.parse_args(argv)
    if a.mode == "archive" and not a.dir:
        ap.error("archive needs DIR")
    return {"text": cmd_text, "binary": cmd_binary, "tables": cmd_tables, "summary": cmd_summary,
            "archive": cmd_archive, "leak-terms": cmd_leak_terms, "allow-key": cmd_allow_key,
            "unused-allow": cmd_unused_allow}[a.mode](a)


if __name__ == "__main__":
    sys.exit(main())
