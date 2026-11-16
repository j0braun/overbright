#!/usr/bin/env python3
"""Renamed private fixtures (scripts/checks/fixture-renames.txt, docs/checks.md "Renamed private fixtures").

A base binary built before a fixture of the private set was renamed reads it under its old name. gate.sh gives
the parity's private set both names:

  fixture_renames.py alias SET     for each `OLD NEW` pair whose NEW is in SET and OLD is not: OLD made a symbolic
                                   link to NEW (relative, same folder or not); prints `aliased N`
  fixture_renames.py old PATH...   each PATH as an older tree knows it (the map applied backwards)

The map file is $FIXTURE_RENAMES, else fixture-renames.txt next to this script; a missing file is an empty map
(the public tree has none). SET must be a copy the caller owns: the links are made inside it.
"""
import os
import sys
from pathlib import Path


def load(path=None):
    """-> [(old, new)] relative paths."""
    p = Path(path or os.environ.get("FIXTURE_RENAMES") or Path(__file__).resolve().parent / "fixture-renames.txt")
    out = []
    if not p.is_file():
        return out
    for raw in p.read_text().splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = line.split()
        if len(parts) != 2 or any(Path(x).is_absolute() or ".." in Path(x).parts for x in parts):
            sys.exit(f"fixture_renames.py: {p}: bad line (want OLD NEW, relative paths): {raw}")
        out.append((parts[0], parts[1]))
    return out


def alias(root, renames):
    """Links OLD -> NEW under root where NEW exists and OLD does not; -> the number made."""
    root = Path(root)
    n = 0
    for old, new in renames:
        o, t = root / old, root / new
        if t.exists() and not o.exists() and not o.is_symlink():
            o.parent.mkdir(parents=True, exist_ok=True)
            o.symlink_to(os.path.relpath(t, o.parent))
            n += 1
    return n


def main(argv):
    if len(argv) == 2 and argv[0] == "alias":
        if not Path(argv[1]).is_dir():
            print(f"fixture_renames.py: {argv[1]} is not a folder", file=sys.stderr)
            return 2
        print(f"aliased {alias(argv[1], load())}")
        return 0
    if argv and argv[0] == "old":
        renames = load()
        for p in argv[1:]:
            print(next((o for o, n in renames if n == p), p))
        return 0
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
