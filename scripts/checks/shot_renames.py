#!/usr/bin/env python3
"""Renamed shot targets (scripts/checks/shot-renames.txt, docs/developing/checks.md "Renamed shot targets").

The parity compares shots by name between a base and a new binary. When a target is renamed, the base binary
knows only the old name: gate.sh asks this script which name to run on the base side.

  shot_renames.py pick KNOWN_FILE SHOT...   for each SHOT the base can draw: `SHOT BASE_NAME` (BASE_NAME = SHOT when
                                            the base knows SHOT itself, else the old name the map gives it);
                                            KNOWN_FILE: the base's targets, one per line (`-` = stdin)
  shot_renames.py old NAME...               each NAME as an older tree knows it (the map applied backwards)

The map file is $SHOT_RENAMES, else shot-renames.txt next to this script; a missing file is an empty map.
"""
import os
import sys
from pathlib import Path


def load(path=None):
    """-> [(old, new)]; a pair ending in `*` on both sides is a prefix rule."""
    p = Path(path or os.environ.get("SHOT_RENAMES") or Path(__file__).resolve().parent / "shot-renames.txt")
    out = []
    if not p.is_file():
        return out
    for raw in p.read_text().splitlines():
        line = raw.split("#", 1)[0].strip()
        if not line:
            continue
        parts = line.split()
        if len(parts) != 2 or parts[0].endswith("*") != parts[1].endswith("*"):
            sys.exit(f"shot_renames.py: {p}: bad line (want OLD NEW, or OLD* NEW*): {raw}")
        out.append((parts[0], parts[1]))
    return out


def old_names(new, renames):
    """The old names `new` had, exact pairs first, then prefix rules."""
    out = [o for o, n in renames if not n.endswith("*") and n == new]
    out += [o[:-1] + new[len(n) - 1:] for o, n in renames if n.endswith("*") and new.startswith(n[:-1])]
    return out


def pick(known, shots, renames):
    """-> [(shot, base_name)] for the shots the base can draw, in the given order."""
    out = []
    for s in shots:
        if s in known:
            out.append((s, s))
            continue
        old = next((o for o in old_names(s, renames) if o in known), None)
        if old is not None:
            out.append((s, old))
    return out


def main(argv):
    if len(argv) >= 2 and argv[0] == "pick":
        src = sys.stdin if argv[1] == "-" else open(argv[1])
        known = {l.strip() for l in src if l.strip()}
        for s, b in pick(known, argv[2:], load()):
            print(s, b)
        return 0
    if argv and argv[0] == "old":
        renames = load()
        for n in argv[1:]:
            print((old_names(n, renames) or [n])[0])
        return 0
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
