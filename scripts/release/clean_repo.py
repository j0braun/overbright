#!/usr/bin/env python3
"""The clean repositories (docs/releasing.md, "The clean repository"); run through make_clean_repo.sh.

  clean_repo.py make SOURCE PUBLIC PRIVATE --name NAME --email EMAIL [--url URL] [--date DATE] [--message-file FILE] [--rev REV]
                [--work DIR] [--target-dir DIR] [--no-build]
  clean_repo.py verify SOURCE PUBLIC PRIVATE [--rev REV] [--work DIR] [--target-dir DIR] [--no-build]
  clean_repo.py reauthor NAME EMAIL [URL] REPO [REPO ...] [--date DATE]

make: PUBLIC gets the files of commit REV (default HEAD) of the repository SOURCE minus private.list
(public_tree.py, byte for byte, with the root Cargo.toml's private workspace members and their Cargo.lock
blocks left out), PRIVATE the files
on private.list plus PRIVATE-README.md (how to overlay it). Each is a new repository, branch main, with one
commit by NAME <EMAIL> (author and committer) dated DATE (default: REV's commit date), so the same input gives
the same commit ids. No remote is added. Then verify runs. SOURCE is only read; nothing is ever deleted.

verify: PASS / FAIL lines (also written to WORK/verify.log): one commit and no remote in each repository; no
private.list file in the public one; public + private = REV file for file (SHA-256); a plain `cargo metadata`
(no --locked) leaves the public work tree unchanged; the release scanner at 0 on
the public repository; no private pattern string in any object of its .git (packed and loose) nor in its other
.git files; the public tree builds without and with the developer harness and its tests pass with no game files
and no private set, each ignored test giving its reason; SOURCE's .git unchanged since make. Exit 0 only when
every line is PASS.

reauthor: redoes the single commit of each REPO (not yet published: one commit, no remote, no other ref, a clean
work tree, else refused) with NAME <EMAIL>, the same tree and date (or DATE); with URL the public message's
Source line is set. The old commit is then dropped from .git (reflog expired, gc --prune=now).

Python 3 standard library and git only (cargo for the build checks).
"""
import argparse
import hashlib
import os
import re
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
import public_tree  # noqa: E402
import scan  # noqa: E402

PRIVATE_README = "PRIVATE-README.md"
T0 = time.time()
SUBJECT_PUBLIC = "Overbright: first public source"
SUBJECT_PRIVATE = "Overbright: private companion set"
SOURCE_LINE = "Source: "
# Commit-message lines the .git object check accepts although a private pattern matches them: the
# co-author credit of the public commit (a pattern family names the assistant). Exact lines only.
COMMIT_CREDIT_LINES = {"Co-Authored-By: Claude <noreply@anthropic.com>"}


def public_message(url):
    body = ("Overbright is a reimplementation of Need for Speed Underground 2 (2004) in Rust and wgpu. It runs\n"
            "on the player's own copy of the game and ships no game data.\n\n"
            "This is the first commit of the public repository; earlier development history is not part of it.\n"
            "Licence: GPL-3.0-or-later (LICENSE); third-party notices: THIRD-PARTY-NOTICES.md.\n")
    return f"{SUBJECT_PUBLIC}\n\n{body}" + (f"\n{SOURCE_LINE}{url}\n" if url else "")


def private_message():
    return (f"{SUBJECT_PRIVATE}\n\n"
            "The files of scripts/release/private.list at their paths in the development tree: recorded\n"
            "captures of the original, private saves, developer notes and tools. Not for publication.\n"
            f"How to use it with the public repository: {PRIVATE_README}.\n")


PRIVATE_README_TEXT = """# Overbright: the private companion set

This repository holds the files listed in `scripts/release/private.list`, at the paths they have in the
development tree: recorded captures of the original game, private saves, developer notes and tools. It is not
published. The public repository holds everything else. Together they make the full development tree, file
for file (the one difference: the public `Cargo.toml` leaves the private workspace member `devprobes` out of
`members`, and the public `Cargo.lock` leaves out its package block).

## Overlay it on a public checkout (the full development tree)

```
git clone <public repository> overbright
git clone <this repository> overbright-private
rsync -a --exclude /.git --exclude /PRIVATE-README.md overbright-private/ overbright/
git -C overbright-private ls-files | grep -vx PRIVATE-README.md >> overbright/.git/info/exclude
```

The last line keeps the overlaid files out of the public repository's `git status`. After the overlay, the
checkout holds the private set itself (`tests/fixtures/native_saves/` is there; build.rs sets `cfg(ob_private)`),
so the private tests run, the developer harness finds the saves, and `scripts/checks/gate.sh` finds the shot
and fixture rename lists. To build the developer probes, add `"devprobes"` to `members` in the checkout's
`Cargo.toml`; cargo then adds the member's block to `Cargo.lock` (commit neither change).

## Or keep the checkout public and point at the set

```
export OB_PRIVATE=$PWD/overbright-private
```

The tests (`testkit::private(path)`), the developer harness (`src/bin/game/dev/private.rs`) and
`scripts/checks/gate.sh` read the private files from `$OB_PRIVATE/<path>` (docs/testing.md). The private tools
(`tools/`, `devprobes/`, `scripts/tests/`, the private scripts) work only from an overlay.
"""


# ------------------------------------------------------------------ helpers

class Fail(SystemExit):
    pass


def git(repo, *args, env=None, input=None, check=True):
    e = dict(os.environ, GIT_OPTIONAL_LOCKS="0", GIT_CONFIG_NOSYSTEM="1", GIT_CONFIG_GLOBAL=os.devnull,
             GIT_TERMINAL_PROMPT="0", LC_ALL="C")
    e.update(env or {})
    r = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, env=e, input=input)
    if check and r.returncode != 0:
        raise Fail(f"git {' '.join(args)} in {repo}: {r.stderr.decode(errors='replace').strip()}")
    return r.stdout


def identity_env(name, email, date):
    for v, what in ((name, "name"), (email, "email")):
        if not v or re.search(r"[<>\n]", v):
            raise Fail(f"author {what} {v!r}: empty or containing < > or a line break")
    return {"GIT_AUTHOR_NAME": name, "GIT_AUTHOR_EMAIL": email, "GIT_AUTHOR_DATE": date,
            "GIT_COMMITTER_NAME": name, "GIT_COMMITTER_EMAIL": email, "GIT_COMMITTER_DATE": date}


def inside(a, b):
    """Is path a equal to or inside b?"""
    a, b = Path(a).resolve(), Path(b).resolve()
    return a == b or b in a.parents


def source_fingerprint(source):
    """SHA-256 over every file of SOURCE's .git folder (path, size, mtime) and its HEAD: changes when anything
    in it is written."""
    gd = Path(git(source, "rev-parse", "--absolute-git-dir").decode().strip())
    h = hashlib.sha256()
    for d, dirs, files in os.walk(gd):
        dirs.sort()
        for f in sorted(files):
            p = Path(d) / f
            try:
                st = p.lstat()
            except FileNotFoundError:
                continue
            h.update(f"{p.relative_to(gd)}\0{st.st_size}\0{st.st_mtime_ns}\n".encode("utf-8", "surrogateescape"))
    h.update(git(source, "rev-parse", "HEAD"))
    return h.hexdigest()


def commit_tree(repo, entries, message, ident):
    """Index exactly `entries` [(mode, path)] (files already in the work tree, hashed without filters, so the
    blobs are the bytes on disk), one root commit on refs/heads/main. -> commit id."""
    paths = [p for _m, p in entries]
    links = [p for m, p in entries if m == "120000"]
    if links:
        raise Fail(f"symbolic links are not supported: {links[:3]}")
    shas = git(repo, "hash-object", "-w", "--no-filters", "--stdin-paths",
               input="".join(p + "\n" for p in paths).encode("utf-8", "surrogateescape")).decode().split()
    if len(shas) != len(paths):
        raise Fail(f"git hash-object: {len(shas)} ids for {len(paths)} files")
    info = b"".join(f"{m} {s}\t".encode() + p.encode("utf-8", "surrogateescape") + b"\0"
                    for (m, p), s in zip(entries, shas))
    git(repo, "update-index", "--add", "-z", "--index-info", input=info)
    tree = git(repo, "write-tree").decode().strip()
    commit = git(repo, "commit-tree", tree, "-F", "-", env=ident, input=message.encode()).decode().strip()
    git(repo, "-c", "core.logAllRefUpdates=false", "update-ref", "refs/heads/main", commit, env=ident)
    git(repo, "update-index", "-q", "--refresh", check=False)
    return commit


def init_repo(path):
    path.mkdir(parents=True, exist_ok=True)
    # No template (no sample hooks, no description file); branch main.
    git(path, "init", "--quiet", "--template=", "--initial-branch=main")


def mode_of(p):
    if p.is_symlink():
        return "120000"
    return "100755" if os.access(p, os.X_OK) else "100644"


# ------------------------------------------------------------------ make

def check_targets(source, public, private, work):
    if subprocess.run(["git", "-C", str(source), "rev-parse", "--git-dir"], capture_output=True).returncode != 0:
        raise Fail(f"{source}: not a git repository")
    for d, what in ((public, "public"), (private, "private")):
        if public_tree.not_empty(Path(d)):
            raise Fail(f"{d} ({what} target): exists and is not empty; this script writes only into new folders and deletes nothing")
    for a, b in ((public, private), (private, public)):
        if inside(a, b):
            raise Fail(f"{a} and {b}: one inside the other")
    for d in (public, private, work):
        if inside(d, source) or inside(source, d):
            raise Fail(f"{d}: inside the source repository {source} (or the other way round)")
    if inside(work, public) or inside(work, private):
        raise Fail(f"{work}: the work folder must not be inside a target repository")


def cmd_make(a):
    source, public, private = Path(a.source).resolve(), Path(a.public).resolve(), Path(a.private).resolve()
    work = Path(a.work).resolve() if a.work else public.parent / "clean-repo-work"
    check_targets(source, public, private, work)
    rev = git(source, "rev-parse", "--verify", f"{a.rev}^{{commit}}").decode().strip()
    date = a.date or git(source, "log", "-1", "--format=%cI", rev).decode().strip()
    ident = identity_env(a.name, a.email, date)
    work.mkdir(parents=True, exist_ok=True)
    fp = source_fingerprint(source)
    (work / "source.fingerprint").write_text(f"{source}\n{fp}\n")
    t0 = time.time()
    print(f"source {source} at {rev} ({date}); public -> {public}, private -> {private}", flush=True)
    for d in (public, private):
        init_repo(d)
    pub, priv, private_pats = public_tree.split_rev(source, rev)
    content = public_tree.blobs(source, [e[1] for e in pub + priv])
    public_tree.write_entries(public, pub, content)
    public_tree.write_entries(private, priv, content)
    dropped, removed = public_tree.drop_private_members(public, private_pats, public_tree.rev_reader(source, rev))
    del content
    (private / PRIVATE_README).write_text(PRIVATE_README_TEXT)
    msg_pub = public_message(a.url)
    if a.message_file:
        msg_pub = Path(a.message_file).read_text().rstrip("\n") + "\n"
    c_pub = commit_tree(public, [(mode_of(public / p), p) for _m, _s, p in pub], msg_pub, ident)
    c_priv = commit_tree(private, [(mode_of(private / p), p) for _m, _s, p in priv] + [("100644", PRIVATE_README)],
                         private_message(), ident)
    print(f"public:  {len(pub)} files, commit {c_pub}" + public_tree.describe(dropped, removed))
    print(f"private: {len(priv)} files + {PRIVATE_README}, commit {c_priv}")
    print(f"made in {time.time() - t0:.1f} s; author and committer {a.name} <{a.email}>, date {date}", flush=True)
    a.rev = rev
    a.work = str(work)
    return cmd_verify(a)


# ------------------------------------------------------------------ verify

class Report:
    def __init__(self, log):
        self.lines, self.log = [], log

    def line(self, ok, text):
        s = f"{'PASS' if ok else 'FAIL'}  {text}"
        self.lines.append(s)
        print(s, flush=True)
        return ok

    def skip(self, text):
        s = f"SKIP  {text}"
        self.lines.append(s)
        print(s, flush=True)

    def done(self):
        bad = [l for l in self.lines if l.startswith("FAIL")]
        last = (f"CLEAN REPO: {'PASS' if not bad else 'FAIL'} ({len(self.lines) - len(bad)} of {len(self.lines)} lines "
                f"PASS), {time.time() - T0:.0f} s in all")
        print(last)
        self.log.write_text("\n".join(self.lines + [last]) + "\n")
        return 0 if not bad else 1


def one_commit(repo):
    n = git(repo, "rev-list", "--all", "--count", check=False).decode().strip()
    remotes = git(repo, "remote").decode().split()
    refs = git(repo, "for-each-ref", "--format=%(refname)").decode().split()
    head = git(repo, "rev-parse", "HEAD", check=False).decode().strip()
    return n == "1" and not remotes and refs == ["refs/heads/main"], n, remotes, refs, head


def tree_files(repo):
    """-> {path: (mode, blob id)} of HEAD."""
    out = {}
    for mode, sha, path in public_tree.rev_entries(repo, "HEAD"):
        out[path] = (mode, sha)
    return out


def sha256s(repo, ids):
    return {s: hashlib.sha256(b).hexdigest() for s, b in public_tree.blobs(repo, ids).items()}


def overlay_check(r, source, rev, public, private):
    src = {p: (m, s) for m, s, p in public_tree.rev_entries(source, rev)}
    pub, priv = tree_files(public), tree_files(private)
    priv.pop(PRIVATE_README, None)
    h_src = sha256s(source, [s for _m, s in src.values()])
    h_pub = sha256s(public, [s for _m, s in pub.values()])
    h_priv = sha256s(private, [s for _m, s in priv.values()])
    private_pats = public_tree.private_patterns(git(source, "show", f"{rev}:{public_tree.PRIVATE_LIST}").decode())
    # The allowed differences: the public Cargo.toml without the private members, the public Cargo.lock
    # without their package blocks (each derived here from the source's file, so source minus those = public).
    expect, removed = {}, []
    if "Cargo.toml" in src:
        read = lambda f: public_tree.blobs(source, [src[f][1]])[src[f][1]].decode() if f in src else None
        toml, lock, _dropped, removed = public_tree.public_workspace_files(
            read("Cargo.toml"), read("Cargo.lock"), private_pats, public_tree.rev_reader(source, rev))
        expect["Cargo.toml"] = hashlib.sha256(toml.encode()).hexdigest()
        if lock is not None:
            expect["Cargo.lock"] = hashlib.sha256(lock.encode()).hexdigest()
    bad, both = [], sorted(set(pub) & set(priv))
    bad += [f"{p}: in both repositories" for p in both]
    bad += [f"{p}: not in the source" for p in sorted((set(pub) | set(priv)) - set(src))]
    lines_src, lines_ovl = [], []
    for p, (m, s) in sorted(src.items()):
        lines_src.append(f"{m} {h_src[s]}  {p}")
        side = pub.get(p) or priv.get(p)
        if side is None:
            bad.append(f"{p}: in neither repository")
            continue
        got = h_pub[side[1]] if p in pub else h_priv[side[1]]
        want = expect[p] if (p in expect and p in pub) else h_src[s]
        if side[0] != m or got != want:
            bad.append(f"{p}: mode {side[0]} sha256 {got[:12]} vs the source's {m} {want[:12]}")
        lines_ovl.append(f"{side[0]} {h_src[s] if got == want else 'differs:' + got}  {p}")
        # the work tree too (the files a developer gets on checkout)
        f = (public if p in pub else private) / p
        if not f.is_symlink() and f.is_file() and hashlib.sha256(f.read_bytes()).hexdigest() != got:
            bad.append(f"{p}: work-tree file differs from its commit")
    d_src = hashlib.sha256("\n".join(lines_src).encode()).hexdigest()
    d_ovl = hashlib.sha256("\n".join(lines_ovl).encode()).hexdigest()
    for b in bad[:20]:
        print(f"      {b}")
    r.line(not bad and d_src == d_ovl,
           f"overlay: public {len(pub)} + private {len(priv)} files = source {rev[:12]} {len(src)} files, mode and "
           f"SHA-256 each (Cargo.toml: members without the private ones; Cargo.lock: without their packages "
           f"{', '.join(removed) or '(none)'}); digest source {d_src[:16]} overlay "
           f"{d_ovl[:16]}{'; ' + str(len(bad)) + ' differences' if bad else ''}")


def scanner_check(r, public, work):
    if not (HERE / scan.PRIVATE_FILE).exists():
        r.line(False, f"release scanner: {scan.PRIVATE_FILE} is missing next to this script (run it from the full tree)")
        return
    for mode, scopes in (("text", ["tree", "public", "all"]), ("binary", [None])):
        for scope in scopes:
            args = [sys.executable, "-I", str(HERE / "scan.py"), mode, "--root", str(public)] + (["--scope", scope] if scope else [])
            p = subprocess.run(args, capture_output=True, text=True)
            name = f"{mode}{' --scope ' + scope if scope else ''}"
            (work / f"scan-{mode}-{scope or 'all'}.txt").write_text(p.stdout + p.stderr)
            if mode == "text":
                m = re.search(r"^ANY family\s+(\d+)\s+(\d+)", p.stdout, re.M)
                al = re.search(r"allowed by allow.txt: (\d+)", p.stdout)
                r.line(p.returncode == 0 and m and m.group(2) == "0",
                       f"scanner {name} on the public repository: {m.group(1) + ' files / ' + m.group(2) if m else '?'} "
                       f"lines not allowed ({al.group(1) if al else 0} allowed hits), exit {p.returncode}")
            else:
                m = re.search(r"tracked non-text files: (\d+); failing: (\d+); failing references: (\d+)", p.stdout)
                r.line(p.returncode == 0 and m and m.group(2) == "0" and m.group(3) == "0",
                       f"scanner {name} on the public repository: "
                       f"{m.group(1) + ' non-text files, ' + m.group(2) + ' failing, ' + m.group(3) + ' failing references' if m else '?'}"
                       f", exit {p.returncode}")


def private_regexes():
    """-> (the private pattern families of families.private.txt, the same plus its fixed leak strings). The leak
    strings are the shipped-binary check's (short save-name pieces and the like); they apply to binary blobs,
    while text is judged by the families as the text scan does."""
    secs = scan.parse_families((HERE / scan.PRIVATE_FILE).read_text())
    fam = {k: [re.compile("|".join(("(?i:%s)" % p[4:]) if p.startswith("(?i)") else "(?:%s)" % p for p in v))]
           for k, v in secs.items() if k not in scan.NOT_PATTERNS and v}
    leaks = [t for t in secs.get("leak-strings", []) if t]
    with_leaks = dict(fam)
    if leaks:
        with_leaks["leak-strings"] = [re.compile("|".join(re.escape(t) for t in leaks))]
    return fam, with_leaks


def objects_check(r, repo, label):
    """Every object of repo's .git (`cat-file --batch-all-objects`: packed and loose) and every other file in
    .git, against the private families. A blob of a HEAD path counts as that file: allow.txt applies to its text
    lines as in the text scan; binary blobs, trees, commits and the other .git files have no exceptions."""
    if not (HERE / scan.PRIVATE_FILE).exists():
        r.line(False, f"{label}: .git objects not checked ({scan.PRIVATE_FILE} missing)")
        return
    fam, fam_bin = private_regexes()
    allow = scan.load_allow(HERE / "allow.txt")
    paths = {}
    for p, (_m, s) in tree_files(repo).items():
        paths.setdefault(s, []).append(p)
    proc = subprocess.run(["git", "-C", str(repo), "cat-file", "--batch-all-objects", "--batch", "--unordered"],
                          capture_output=True, check=True, env=dict(os.environ, GIT_OPTIONAL_LOCKS="0"))
    data, at, n, hits, allowed = proc.stdout, 0, 0, [], 0
    while at < len(data):
        nl = data.index(b"\n", at)
        sha, kind, size = data[at:nl].decode().split()
        body = data[nl + 1:nl + 1 + int(size)]
        at = nl + 1 + int(size) + 1
        n += 1
        if kind == "blob" and b"\0" not in body[:8192] and sha in paths:
            for no, line in enumerate(body.decode("utf-8", "replace").split("\n"), 1):
                for name, regs in fam.items():
                    for rg in regs:
                        for m in rg.finditer(line):
                            if all(scan.allowed(allow, p, line, name) for p in paths[sha]):
                                allowed += 1
                            else:
                                hits.append(f"{kind} {sha[:12]} ({paths[sha][0]}:{no}) {name}: {m.group(0)}")
            continue
        binary = kind == "blob" and b"\0" in body[:8192]
        texts = [t for _o, t in scan.binary_strings(body)] if binary else [body.decode("utf-8", "replace")]
        if kind == "commit":
            texts = ["\n".join(ln for ln in texts[0].split("\n") if ln not in COMMIT_CREDIT_LINES)]
        for t in texts:
            for name, regs in (fam_bin if binary else fam).items():
                for rg in regs:
                    for m in rg.finditer(t):
                        hits.append(f"{kind} {sha[:12]}{' (' + paths[sha][0] + ')' if sha in paths else ''} {name}: {m.group(0)}")
    gd = Path(git(repo, "rev-parse", "--absolute-git-dir").decode().strip())
    others = 0
    for d, dirs, files in os.walk(gd):
        if Path(d) == gd / "objects":
            dirs[:] = ["info"]  # packs and loose objects are read above (decompressed)
        for f in files:
            p = Path(d) / f
            if p.parent.parent == gd / "objects" and re.fullmatch(r"[0-9a-f]{2}", p.parent.name):
                continue
            others += 1
            t = p.read_bytes().decode("utf-8", "replace")
            for name, regs in fam.items():
                for rg in regs:
                    for m in rg.finditer(t):
                        hits.append(f".git/{p.relative_to(gd)} {name}: {m.group(0)}")
    for h in hits[:20]:
        print(f"      {h}")
    r.line(not hits, f"{label}: no private pattern string in its {n} .git objects (packed and loose, via cat-file "
                     f"--batch-all-objects) nor its {others} other .git files ({len(hits)} hits; {allowed} allowed "
                     f"by allow.txt in text files)")


def build_env(work, target):
    env = {k: v for k, v in os.environ.items()
           if k not in ("NFSU2_ROOT", "OB_PRIVATE", "DISPLAY", "WAYLAND_DISPLAY", "XDG_SESSION_TYPE", "OB_REQUIRE")}
    x = work / "xdg"
    for sub in ("data", "config", "cache", "state"):
        (x / sub).mkdir(parents=True, exist_ok=True)
    env.update(CARGO_TARGET_DIR=str(target), NFSU2_CACHE_DIR=str(work / "cache"), XDG_DATA_HOME=str(x / "data"),
               XDG_CONFIG_HOME=str(x / "config"), XDG_CACHE_HOME=str(x / "cache"), XDG_STATE_HOME=str(x / "state"))
    return env


def lock_check(r, public, work, target):
    """`cargo metadata` WITHOUT --locked, as an outside developer's first plain `cargo build` resolves: cargo
    rewrites Cargo.lock when it does not match the public workspace (a package block of a left-out member, a
    package only that member needed), so the work tree must still be unchanged afterwards."""
    if not (public / "Cargo.toml").exists():
        r.skip("Cargo.lock as cargo resolves it: no Cargo.toml in the public repository")
        return
    if subprocess.run(["sh", "-c", "command -v cargo"], capture_output=True).returncode != 0:
        r.skip("Cargo.lock as cargo resolves it: no cargo on PATH")
        return
    log = work / "metadata.log"
    lock = public / "Cargo.lock"
    before = lock.read_bytes() if lock.exists() else None
    with open(log, "wb") as f:
        rc = subprocess.run(["cargo", "metadata", "--format-version", "1"], cwd=public, env=build_env(work, target),
                            stdout=subprocess.DEVNULL, stderr=f).returncode
    st = git(public, "status", "--porcelain", "--untracked-files=all").decode()
    # verify only reads: a lock cargo rewrote is put back (the line above has recorded the change).
    if before is not None and lock.read_bytes() != before:
        (work / "Cargo.lock.resolved").write_bytes(lock.read_bytes())
        lock.write_bytes(before)
    r.line(rc == 0 and not st, f"cargo metadata without --locked (the resolve of a plain `cargo build`): exit {rc}, "
                               f"the public work tree {'changed: ' + str(st.splitlines()[:5]) if st else 'unchanged'}"
                               f"{' (log ' + str(log) + ')' if rc else ''}")


def build_checks(r, public, work, target):
    env = build_env(work, target)
    steps = [("build", ["cargo", "build", "--release", "--locked"], "cargo build --release (the shipping game, no developer harness)"),
             ("build-dev", ["cargo", "build", "--release", "--locked", "--features", "dev-harness", "--bins", "--examples"],
              "cargo build --release --features dev-harness --bins --examples"),
             ("test", ["cargo", "test", "--release", "--locked", "--features", "dev-harness", "--no-fail-fast"],
              "cargo test --release --features dev-harness, no NFSU2_ROOT, no OB_PRIVATE")]
    for name, cmd, what in steps:
        t0 = time.time()
        log = work / f"{name}.log"
        with open(log, "wb") as f:
            rc = subprocess.run(cmd, cwd=public, env=env, stdout=f, stderr=subprocess.STDOUT).returncode
        secs = time.time() - t0
        if name != "test":
            r.line(rc == 0, f"{what}: exit {rc}, {secs:.0f} s (log {log})")
            if rc != 0:
                return
            continue
        text = log.read_text(errors="replace")
        res = re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored", text)
        passed, failed, ignored = (sum(int(x[i]) for x in res) for i in (1, 2, 3))
        bare = re.findall(r"^test (\S+) \.\.\. ignored$", text, re.M)
        game = len(re.findall(r"^test .* \.\.\. ignored, .*NFSU2_ROOT", text, re.M))
        priv = len(re.findall(r"^test .* \.\.\. ignored, .*OB_PRIVATE", text, re.M))
        for b in bare[:10]:
            print(f"      ignored without a reason: {b}")
        r.line(rc == 0 and failed == 0 and res and not bare,
               f"{what}: exit {rc}, {len(res)} test binaries, {passed} passed, {failed} failed, {ignored} ignored "
               f"({game} need NFSU2_ROOT, {priv} OB_PRIVATE, {len(bare)} without a reason), {secs:.0f} s")
    st = git(public, "status", "--porcelain", "--untracked-files=all").decode()
    r.line(not st, "the public work tree is unchanged by the builds and tests" + (f": {st.splitlines()[:5]}" if st else ""))


def cmd_verify(a):
    source, public, private = Path(a.source).resolve(), Path(a.public).resolve(), Path(a.private).resolve()
    work = Path(a.work).resolve() if a.work else public.parent / "clean-repo-work"
    work.mkdir(parents=True, exist_ok=True)
    rev = git(source, "rev-parse", "--verify", f"{a.rev}^{{commit}}").decode().strip()
    r = Report(work / "verify.log")
    for repo, label in ((public, "public"), (private, "private")):
        ok, n, remotes, refs, head = one_commit(repo)
        r.line(ok, f"{label} repository: {n} commit ({head[:12]}), branch main only (refs: {' '.join(refs)}), "
                   f"remotes: {' '.join(remotes) or 'none'}")
    private_pats = public_tree.private_patterns(git(source, "show", f"{rev}:{public_tree.PRIVATE_LIST}").decode())
    files = list(tree_files(public))
    on_list = [f for f in files if scan.under(f, private_pats)]
    for f in on_list[:10]:
        print(f"      {f}")
    r.line(not on_list, f"public repository: none of its {len(files)} files is on private.list ({len(on_list)} are)")
    overlay_check(r, source, rev, public, private)
    scanner_check(r, public, work)
    objects_check(r, public, "public repository")
    target = Path(a.target_dir or os.environ.get("CARGO_TARGET_DIR") or work / "target")
    if inside(target, public):
        r.line(False, f"target dir {target} is inside the public repository")
    else:
        lock_check(r, public, work, target)
        if a.no_build:
            r.skip("builds and tests of the public tree (--no-build)")
        else:
            build_checks(r, public, work, target)
    fpf = work / "source.fingerprint"
    if fpf.exists():
        want_src, want = fpf.read_text().split("\n")[:2]
        now = source_fingerprint(source)
        r.line(want_src == str(source) and now == want,
               f"source repository untouched: its .git (every file's size and time) and HEAD as before make "
               f"({now[:16]})")
    else:
        r.skip(f"source repository untouched: no fingerprint from make in {work}")
    return r.done()


# ------------------------------------------------------------------ reauthor

def cmd_reauthor(a):
    args = list(a.args)
    if len(args) < 3:
        raise Fail("--reauthor NAME EMAIL [SOURCE_URL] REPO [REPO ...]")
    name, email = args[0], args[1]
    url = args[2] if "://" in args[2] else None
    repos = [Path(p).resolve() for p in args[3 if url else 2:]]
    if not repos:
        raise Fail("--reauthor: no repository given")
    # Check every repository first: all or none are changed.
    plan = []
    for repo in repos:
        ok, n, remotes, refs, head = one_commit(repo)
        if not ok:
            raise Fail(f"{repo}: refused: {n} commits, remotes [{' '.join(remotes)}], refs [{' '.join(refs)}] "
                       f"(only an unpublished repository with one commit on main, no remote and no other ref)")
        st = git(repo, "status", "--porcelain", "--untracked-files=all").decode()
        if st:
            raise Fail(f"{repo}: refused: the work tree has changes ({st.splitlines()[0]} ...)")
        msg = git(repo, "log", "-1", "--format=%B", head).decode().rstrip("\n") + "\n"
        if url and msg.startswith(SUBJECT_PUBLIC + "\n"):
            msg = public_message(url)
        date = a.date or git(repo, "log", "-1", "--format=%aI", head).decode().strip()
        plan.append((repo, head, msg, identity_env(name, email, date), date))
    for repo, old, msg, ident, date in plan:
        tree = git(repo, "rev-parse", f"{old}^{{tree}}").decode().strip()
        new = git(repo, "commit-tree", tree, "-F", "-", env=ident, input=msg.encode()).decode().strip()
        git(repo, "-c", "core.logAllRefUpdates=false", "update-ref", "refs/heads/main", new, old, env=ident)
        git(repo, "reflog", "expire", "--expire=now", "--all")
        git(repo, "gc", "--quiet", "--prune=now")
        gone = subprocess.run(["git", "-C", str(repo), "cat-file", "-e", old], capture_output=True).returncode != 0
        print(f"{'ok  ' if gone else 'FAIL'}  {repo}: {old[:12]} -> {new[:12]} by {name} <{email}>, date {date}; "
              f"old commit {'gone from .git' if gone else 'STILL IN .git'}")
        if not gone:
            return 1
    print("run --verify again on the re-authored repositories (the commit ids changed; the files did not)")
    return 0


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    for c in ("make", "verify"):
        p = sub.add_parser(c)
        p.add_argument("source")
        p.add_argument("public")
        p.add_argument("private")
        p.add_argument("--rev", default="HEAD")
        p.add_argument("--work", help="logs, the source fingerprint, caches (default: clean-repo-work next to PUBLIC)")
        p.add_argument("--target-dir", help="cargo target dir of the build checks (default: $CARGO_TARGET_DIR, else WORK/target)")
        p.add_argument("--no-build", action="store_true", help="skip the build and test checks")
        if c == "make":
            p.add_argument("--name", default=os.environ.get("OB_AUTHOR_NAME"), required="OB_AUTHOR_NAME" not in os.environ)
            p.add_argument("--email", default=os.environ.get("OB_AUTHOR_EMAIL"), required="OB_AUTHOR_EMAIL" not in os.environ)
            p.add_argument("--url", default=os.environ.get("OB_SOURCE_URL"))
            p.add_argument("--date", default=os.environ.get("OB_COMMIT_DATE"), help="commit date (default: REV's commit date)")
            p.add_argument("--message-file", help="the public commit's message (default: the built-in first-commit message)")
    p = sub.add_parser("reauthor")
    p.add_argument("args", nargs="+", metavar="NAME EMAIL [SOURCE_URL] REPO")
    p.add_argument("--date", default=os.environ.get("OB_COMMIT_DATE"))
    a = ap.parse_args(argv)
    try:
        return {"make": cmd_make, "verify": cmd_verify, "reauthor": cmd_reauthor}[a.cmd](a)
    except Fail as e:
        print(f"make_clean_repo: {e.code}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main())
