#!/usr/bin/env python3
"""The public tree: a copy of the tracked files minus every path on private.list (docs/release-scan.md).

  public_tree.py DEST [--root ROOT] [--list]
  public_tree.py DEST --rev REV [--root ROOT] [--private-dest PDEST] [--list]

DEST must not exist yet (or be empty).  It is what the public repository will hold, so building and testing it
there proves that nothing in it needs the private set: `cargo build` and `cargo test` must work, with the
private tests reported as ignored ("needs OB_PRIVATE ...").  --list prints the copied paths instead of
copying.

Without --rev the files are the work tree's tracked files (`git ls-files`) and private.list is this script's own.
With --rev they are the blobs of that commit of ROOT's repository, byte for byte (no line-ending or other
conversion), with their modes, and private.list is the commit's own; --private-dest also writes the private
files (the complement) there.  ROOT's repository is only read.  Python 3 standard library only.
In both, the root Cargo.toml leaves the private workspace members out of `members` and the root Cargo.lock
leaves out their [[package]] blocks (nothing else in either file changes).
"""
import argparse
import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import scan  # noqa: E402

PRIVATE_LIST = "scripts/release/private.list"


def public_files(root, data=scan.HERE):
    private = scan.load_private(Path(data) / "private.list")
    return [f for f in scan.tracked_files(root) if not scan.under(f, private)]


def private_patterns(text):
    """private.list text -> its patterns (as scan.load_private)."""
    return [l.split("#")[0].strip() for l in text.splitlines() if l.split("#")[0].strip()]


def git(root, *args, **kw):
    env = dict(os.environ, GIT_OPTIONAL_LOCKS="0")
    return subprocess.run(["git", "-C", str(root), *args], capture_output=True, check=True, env=env, **kw).stdout


def rev_entries(root, rev):
    """-> [(mode, sha, path)] of every file of commit `rev` (blobs and symlinks; a submodule is refused)."""
    out = []
    for rec in git(root, "ls-tree", "-r", "-z", "--full-tree", rev).split(b"\0"):
        if not rec:
            continue
        meta, path = rec.split(b"\t", 1)
        mode, kind, sha = meta.decode().split()
        if kind != "blob":
            raise SystemExit(f"{path.decode()}: a {kind} entry (submodule) is not supported")
        out.append((mode, sha, path.decode("utf-8", "surrogateescape")))
    return out


def blobs(root, shas):
    """-> {sha: bytes} for the given blob ids, read through one `git cat-file --batch`."""
    want = sorted(set(shas))
    proc = subprocess.run(["git", "-C", str(root), "cat-file", "--batch"], input="".join(s + "\n" for s in want).encode(),
                          capture_output=True, check=True, env=dict(os.environ, GIT_OPTIONAL_LOCKS="0"))
    data, at, out = proc.stdout, 0, {}
    for s in want:
        nl = data.index(b"\n", at)
        head = data[at:nl].decode().split()
        if len(head) != 3 or head[0] != s:
            raise SystemExit(f"git cat-file: unexpected answer for {s}: {data[at:nl][:80]!r}")
        size = int(head[2])
        out[s] = data[nl + 1:nl + 1 + size]
        at = nl + 1 + size + 1
    return out


def split_rev(root, rev):
    """-> (public entries, private entries, private patterns) of commit `rev`, by the commit's own private.list."""
    entries = rev_entries(root, rev)
    private = private_patterns(git(root, "show", f"{rev}:{PRIVATE_LIST}").decode())
    pub = [e for e in entries if not scan.under(e[2], private)]
    priv = [e for e in entries if scan.under(e[2], private)]
    return pub, priv, private


def write_entries(dest, entries, content):
    for mode, sha, path in entries:
        out = Path(dest) / path
        out.parent.mkdir(parents=True, exist_ok=True)
        if mode == "120000":
            os.symlink(content[sha].decode("utf-8", "surrogateescape"), out)
            continue
        out.write_bytes(content[sha])
        os.chmod(out, 0o755 if mode == "100755" else 0o644)


def not_empty(d):
    return d.exists() and (not d.is_dir() or any(d.iterdir()))


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("dest")
    ap.add_argument("--root", default=str(scan.DEFAULT_ROOT))
    ap.add_argument("--rev", help="take the files of this commit (byte for byte) and its private.list")
    ap.add_argument("--private-dest", help="with --rev: also write the private files here")
    ap.add_argument("--list", action="store_true")
    a = ap.parse_args(argv)
    root, dest = Path(a.root), Path(a.dest)
    if a.private_dest and not a.rev:
        ap.error("--private-dest needs --rev")
    if a.rev:
        pub, priv, private = split_rev(root, a.rev)
        if a.list:
            print("\n".join(e[2] for e in pub))
            return 0
        for d in [dest] + ([Path(a.private_dest)] if a.private_dest else []):
            if not_empty(d):
                print(f"{d}: not empty", file=sys.stderr)
                return 2
        content = blobs(root, [e[1] for e in pub + (priv if a.private_dest else [])])
        write_entries(dest, pub, content)
        if a.private_dest:
            write_entries(a.private_dest, priv, content)
        dropped, removed = drop_private_members(dest, private, rev_reader(root, a.rev))
        print(f"{len(pub)} files -> {dest}" + (f", {len(priv)} private files -> {a.private_dest}" if a.private_dest else "")
              + describe(dropped, removed))
        return 0
    files = public_files(root)
    if a.list:
        print("\n".join(files))
        return 0
    if not_empty(dest):
        print(f"{dest}: not empty", file=sys.stderr)
        return 2
    for f in files:
        out = dest / f
        out.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(root / f, out)
    dropped, removed = drop_private_members(dest, scan.load_private(scan.HERE / "private.list"), tree_reader(root))
    print(f"{len(files)} files -> {dest}" + describe(dropped, removed))
    return 0


def members_without_private(text, private):
    """-> (the Cargo.toml text with the private workspace members left out, the members left out)."""
    lines = text.split("\n")
    dropped = []
    for i, l in enumerate(lines):
        m = re.match(r'^(members\s*=\s*\[)([^\]]*)(\].*)$', l)
        if not m:
            continue
        items = [x.strip() for x in m.group(2).split(",") if x.strip()]
        keep = [x for x in items if not scan.under(x.strip('"') + "/", private)]
        dropped += [x.strip('"') for x in items if x not in keep]
        lines[i] = m.group(1) + ", ".join(keep) + m.group(3)
    return "\n".join(lines), dropped


def package_name(cargo_toml_text):
    """-> the `name` of a Cargo.toml's [package] table (None without one)."""
    m = re.search(r'(?ms)^\[package\]\s*$(.*?)(?=^\[|\Z)', cargo_toml_text)
    n = m and re.search(r'(?m)^name\s*=\s*"([^"]+)"', m.group(1))
    return n.group(1) if n else None


def member_packages(members, read):
    """-> the package names of the workspace member folders `members`; read(path) gives a file's text or None."""
    names = []
    for d in members:
        t = read(f"{d.rstrip('/')}/Cargo.toml")
        n = t and package_name(t)
        if n:
            names.append(n)
    return names


def lock_without_packages(text, names):
    """-> (the Cargo.lock text without the [[package]] blocks of the local (path, no `source`) packages `names`,
    the names removed). Every other byte stays as it is: the blocks are separated by one empty line."""
    parts, removed = text.split("\n\n"), []
    keep = []
    for part in parts:
        m = re.match(r'\[\[package\]\]\nname = "([^"]+)"\n', part)
        if m and m.group(1) in names and not re.search(r'(?m)^source = ', part):
            removed.append(m.group(1))
            continue
        keep.append(part)
    return "\n\n".join(keep), removed


def public_workspace_files(cargo_toml_text, cargo_lock_text, private, read):
    """The public Cargo.toml and Cargo.lock: the private workspace members left out of `members` and their
    package blocks out of the lock (cargo would drop them on the first build that is not --locked, leaving every
    outside developer with a changed Cargo.lock). -> (toml text, lock text or None, members, packages removed)."""
    toml, dropped = members_without_private(cargo_toml_text, private)
    lock, removed = None, []
    if cargo_lock_text is not None:
        lock, removed = lock_without_packages(cargo_lock_text, member_packages(dropped, read))
    return toml, lock, dropped, removed


def drop_private_members(dest, private, read):
    """DEST's root Cargo.toml without the private workspace members (devprobes/ until it moves to the private
    companion repo: cargo refuses a workspace whose member folder is missing) and its Cargo.lock without their
    package blocks. read(path) gives a source file's text (the members' own Cargo.toml are not in DEST).
    -> (members left out, lock packages removed)."""
    toml_p, lock_p = Path(dest) / "Cargo.toml", Path(dest) / "Cargo.lock"
    if not toml_p.exists():
        return [], []
    toml, lock, dropped, removed = public_workspace_files(
        toml_p.read_bytes().decode(), lock_p.read_bytes().decode() if lock_p.exists() else None, private, read)
    toml_p.write_bytes(toml.encode())
    if lock is not None:
        lock_p.write_bytes(lock.encode())
    return dropped, removed


def rev_reader(root, rev):
    """read(path) for drop_private_members: the file's text in commit `rev` of ROOT's repository, or None."""
    def read(path):
        r = subprocess.run(["git", "-C", str(root), "show", f"{rev}:{path}"], capture_output=True,
                           env=dict(os.environ, GIT_OPTIONAL_LOCKS="0"))
        return r.stdout.decode() if r.returncode == 0 else None
    return read


def tree_reader(root):
    def read(path):
        p = Path(root) / path
        return p.read_bytes().decode() if p.is_file() else None
    return read


def describe(dropped, removed):
    return ((f"; workspace members left out: {', '.join(dropped)}" if dropped else "")
            + (f"; Cargo.lock packages left out: {', '.join(removed)}" if removed else ""))


if __name__ == "__main__":
    sys.exit(main())
