#!/usr/bin/env python3
"""List every `--shot` target of the game binary (docs/developing/checks.md).

A tree with the shot table (src/bin/game/dev/shot/targets/, the
binary answers `game --shot-list` with `name<TAB>group` lines) is asked:
the script runs `<game> --shot-list` and prints the names. The binary is
the second argument, else $SHOT_LIST_BIN, else $CARGO_TARGET_DIR/<profile>/game,
else <repo>/target/<profile>/game (<profile>: $CHECKS_PROFILE, default gate); a tree with the table and no such binary
is an error (its source has no match left to parse). How it decides: the
tree has the table when one of its game sources holds `"--shot-list"`.

Older trees (bases in the gates) are parsed as before (the fallback below):
the targets are the string patterns of the arms of `game.mode = match screen {`
in `fn shot`, plus the Graphics group pages (`options_graphics_<group>`), the settings
page's group pages (`settings_<group>`) and the
prefix / suffix variants named in VARIANTS (finished_career_*, *_dday, *_kit, crib_parts), under the
names such a tree knows (shot-renames.txt applied backwards, shot_renames.py).
`fn shot` is looked up in every .rs file of the game binary (src/bin/game.rs, then
src/bin/game/** in path order), so the parse survives code moves.
Usage: shot_targets.py [repo [game-binary]]  -> one target per line, sorted, unique.
"""
import os, re, subprocess, sys, pathlib

# Fallback parser only (trees without `--shot-list`); the table's own lists are
# VARIANTS / UNLISTED in src/bin/game/dev/shot/targets/mod.rs.
# Variants that are not arms of their own but are used by the checks.
VARIANTS = ["main_dday", "explore_kit", "crib_parts", "finished_career_shop_checks", "finished_career_specials_hydro3",
            "finished_career_specials_purge", "finished_career_dynorun"]

# Arms that are not targets on their own (left out of the full set), with why.
EXCLUDE = {
    "shop_checks": "needs the finished career: run as finished_career_shop_checks",
    "specials_hydro3": "needs the finished career: run as finished_career_specials_hydro3",
    "specials_purge": "needs the finished career: run as finished_career_specials_purge",
    "vinyl_ask": "fails on a fresh profile ('the vinyl question' not reached); open",
}

# A floor for the parsed arms (283 targets on 2026-11-11): far fewer means the parser lost its place.
MIN_ARMS = 150


def game_sources(repo):
    """The game binary's .rs files: src/bin/game.rs first, then src/bin/game/** sorted by path."""
    root = pathlib.Path(repo) / "src/bin"
    files = [root / "game.rs"] if (root / "game.rs").is_file() else []
    if (root / "game").is_dir():
        files += sorted(p for p in (root / "game").rglob("*.rs") if p.is_file())
    return [p.read_text().splitlines() for p in files]


def has_shot_list(repo):
    """The tree's binary answers `--shot-list` (the shot table)."""
    return any('"--shot-list"' in "\n".join(lines) for lines in game_sources(repo))


def binary_targets(repo, binary):
    """`<binary> --shot-list`: the table's targets."""
    cands = [binary, os.environ.get("SHOT_LIST_BIN")]
    # The checks' profile folder (CHECKS_PROFILE, default gate; docs/developing/build-times.md).
    pdir = os.environ.get("CHECKS_PROFILE") or "gate"
    pdir = "debug" if pdir == "dev" else pdir
    if os.environ.get("CARGO_TARGET_DIR"):
        cands.append(os.path.join(os.environ["CARGO_TARGET_DIR"], pdir, "game"))
    cands.append(str(pathlib.Path(repo) / "target" / pdir / "game"))
    game = next((c for c in cands if c and os.access(c, os.X_OK)), None)
    if game is None:
        sys.exit(f"shot_targets.py: {repo} has the shot table (`--shot-list`) but no game binary was found (tried {[c for c in cands if c]})")
    r = subprocess.run([game, "--shot-list"], capture_output=True, text=True)
    if r.returncode != 0:
        sys.exit(f"shot_targets.py: {game} --shot-list failed (rc {r.returncode}): {r.stderr.strip()[-300:]}")
    out = {l.split("\t")[0] for l in r.stdout.splitlines() if l.strip()}
    if len(out) < MIN_ARMS:
        sys.exit(f"shot_targets.py: {game} --shot-list gave only {len(out)} targets (expected at least {MIN_ARMS})")
    return sorted(out)


def targets(repo, binary=None):
    if has_shot_list(repo):
        return binary_targets(repo, binary)
    return parsed_targets(repo)


def parsed_targets(repo):
    """The fallback for trees without the shot table: parse `fn shot`'s match."""
    sources = game_sources(repo)
    # The file that holds `fn shot` (its match and its pre-dispatch lines are in that fn).
    src, start, rest = None, None, []
    for lines in sources:
        at = next((i for i, l in enumerate(lines) if re.match(r"^(pub(\([a-z]+\))? )?fn shot\(game: &mut Game, screen: &str, out: &str\)", l)), None) if src is None else None
        if at is not None:
            src, start = lines, at
        elif lines is not src:
            rest += lines
    if src is None:
        sys.exit("shot_targets.py: `fn shot(game: &mut Game, screen: &str, out: &str)` not found in src/bin/game.rs or src/bin/game/**")
    m = next((i for i in range(start, len(src)) if src[i].strip() == "game.mode = match screen {"), None)
    if m is None:
        sys.exit("shot_targets.py: `game.mode = match screen {` not found after fn shot")
    out, head, ended = set(), None, False
    for line in src[m + 1:]:
        if line.startswith("        other => anyhow::bail!(\"unknown screen"):
            ended = True
            break
        # An arm's head starts at 8 spaces with a string pattern and runs to its `=>`.
        if head is None and re.match(r'^ {8}"', line):
            head = ""
        if head is not None:
            head += line
            if "=>" in line:
                pat = head.split("=>")[0].split(" if ")[0]
                out.update(re.findall(r'"([a-z0-9_]+)"', pat))
                head = None
        # `g if g.starts_with("options_graphics_")`: its group names.
    # The parse follows game.rs's layout: a layout change must stop the checks, not shrink the target list.
    if not ended:
        sys.exit("shot_targets.py: the match's `other => anyhow::bail!(\"unknown screen` arm was not found (game.rs layout changed?)")
    if len(out) < MIN_ARMS:
        sys.exit(f"shot_targets.py: only {len(out)} arms parsed (expected at least {MIN_ARMS}; game.rs layout changed?)")
    # The group tables: after the match in fn shot's file, else in another file of the binary.
    body = "\n".join(src[m:] + rest)
    g = re.search(r'let group = match &g\["options_graphics_"\.len\(\)\.\.\] \{(.*?)other =>', body, re.S)
    if g:
        out.update("options_graphics_" + n for n in re.findall(r'"([a-z_]+)"', g.group(1)))
    # The settings page: `g if g.starts_with("settings_")`: the settings page's groups.
    g = re.search(r'let group = match &g\["settings_"\.len\(\)\.\.\] \{(.*?)other =>', body, re.S)
    if g:
        out.update("settings_" + n for n in re.findall(r'"([a-z_]+)"', g.group(1)))
    # The variants under the names a tree without the table knows (it predates every rename).
    sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
    import shot_renames
    renames = shot_renames.load()
    out.update((shot_renames.old_names(v, renames) or [v])[0] for v in VARIANTS)
    # Targets dispatched before the match (`if screen == "photo"`, `explore_rain`).
    body_text = "\n".join(src[start:m])
    out.update(re.findall(r'if screen == "([a-z0-9_]+)" \{', body_text) + re.findall(r'let screen = if screen == "([a-z0-9_]+)"', body_text))
    return sorted(out - set(EXCLUDE))

if __name__ == "__main__":
    repo = sys.argv[1] if len(sys.argv) > 1 else pathlib.Path(__file__).resolve().parents[2]
    for t in targets(repo, sys.argv[2] if len(sys.argv) > 2 else None):
        print(t)
