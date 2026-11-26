#!/usr/bin/env python3
"""stray_opens.py [repo] [--list]: the stray-open lint (issue "Game reads everything through the asset layer").

A game file is read through `game_fs` (read, open_file, open_reader, find, path...), which the asset layer answers.
A direct `std::fs` open of a path (`fs::read(`, `fs::read_to_string(`, `File::open(`, `fs::read_dir(`, `fs::metadata(`,
`OpenOptions`) in production code reads the game folder behind the layer's back unless the path is the program's own
(settings, saves, caches, the player's packs, the import). scripts/checks/stray-opens.txt lists, per file, how many
such calls it may hold and why; the lint fails when a file holds more than listed (or is not listed), and when a
listed count is higher than the file's now (shrink the list: it only goes down). Test code (from the first
`#[cfg(test)]` at the start of a line on, `tests/`, `tests.rs`, `*_tests.rs`), the developer harness (`dev/`),
the tools and comment lines are not counted. A new read of a game file goes through `game_fs`; an own-data read gets a
line with its reason. The sealed playthrough (sealed_copy.sh) is the proof that no game read is left on the
playthrough's way; this lint keeps a new one from appearing.

  --list   print the current counts in the list's format (for writing a line)
"""
import os, re, sys

PAT = re.compile(r'(?<![A-Za-z0-9_])(?:std::)?(?:fs::(?:read|read_to_string|read_dir|metadata)|File::open|OpenOptions::new)\s*\(?')
SKIP_DIR = ('/tests/', '/dev/', '/fuzz/', '/devprobes/', '/tools/', '/bin/overbright-', '/ob-testkit/', '/ob-studio')


def counts(repo):
    out = {}
    for top in ('src', 'crates'):
        for d, _, fs in os.walk(os.path.join(repo, top)):
            for f in fs:
                if not f.endswith('.rs') or f == 'tests.rs' or f.endswith('_tests.rs'):
                    continue
                p = os.path.join(d, f)
                rel = os.path.relpath(p, repo)
                if any(s in '/' + rel for s in SKIP_DIR):
                    continue
                n = 0
                for line in open(p, encoding='utf-8', errors='replace'):
                    if line.startswith('#[cfg(test)]') or line.startswith('#[cfg(all(test'):
                        break
                    t = line.strip()
                    if t.startswith('//'):
                        continue
                    n += len(PAT.findall(line))
                if n:
                    out[rel] = n
    return out


def main():
    args = [a for a in sys.argv[1:] if not a.startswith('--')]
    repo = args[0] if args else os.path.join(os.path.dirname(os.path.abspath(__file__)), '..', '..')
    now = counts(repo)
    if '--list' in sys.argv:
        for k in sorted(now):
            print(f'{k}\t{now[k]}\t')
        return 0
    allow = {}
    lp = os.path.join(repo, 'scripts', 'checks', 'stray-opens.txt')
    for line in open(lp, encoding='utf-8'):
        if line.startswith('#') or not line.strip():
            continue
        f = line.rstrip('\n').split('\t')
        if len(f) < 3 or not f[1].isdigit() or not f[2].strip():
            print(f'stray-opens: bad line (file, count, reason): {line.strip()}')
            return 1
        allow[f[0]] = int(f[1])
    bad = 0
    for k in sorted(now):
        if now[k] > allow.get(k, 0):
            bad += 1
            print(f'stray-opens: {k}: {now[k]} direct std::fs open(s), {allow.get(k, 0)} listed. A game file is read through '
                  f'nfsu2::game_fs (the asset layer answers it); an own-data read is listed in scripts/checks/stray-opens.txt with its reason')
    for k in sorted(allow):
        if now.get(k, 0) < allow[k]:
            bad += 1
            print(f'stray-opens: {k}: lists {allow[k]} but holds {now.get(k, 0)}: lower the list (it only goes down)')
    if bad:
        return 1
    print(f'STRAY OPENS: ok ({sum(now.values())} direct opens in {len(now)} files, all listed)')
    return 0


sys.exit(main())
