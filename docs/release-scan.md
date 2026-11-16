# Release scan

`scripts/release/scan.py` (Python 3, standard library only) finds what must not reach the public tree.
It changes nothing; it only reports. Later release units fix the hits family by family.

```
scripts/release/scan.py summary                    # totals for all modes (about 40 s)
scripts/release/scan.py text   [--list] [--family F] [--scope tree|public|all]
scripts/release/scan.py binary [--list]
scripts/release/scan.py leak-terms                 # the private strings the shipped-binary leak checks search for
scripts/release/scan.py allow-key PATH LINE        # the allow.txt key (=HASH) of that line
scripts/release/scan.py unused-allow [--archive DIR ...]   # allow-list entries that match nothing any more
scripts/release/scan.py tables [--list] [--fail]
python3 -m unittest discover -s scripts/release    # the scanner's own tests
scripts/release/public_tree.py DEST                # the public tree: tracked files minus private.list, copied to DEST
scripts/release/public_tree.py DEST --rev REV [--private-dest P]   # the same from a commit, byte for byte; P gets the private files
```

Exit status: `text` and `binary` exit 1 on any hit that is not allowed; `tables` exits 1 only with `--fail`; `summary` exits 0.
`--list` prints `path:line:family:match` (text) or one line per reference/file (binary).
`unused-allow` lists the entries of the three allow lists that no longer match anything, as `UNUSED <file>:<line>: <entry>`, and exits 1 if there are any. For allow.txt, the text scan runs over every tracked file. An entry counts as used by every hit it matches, so an entry shadowed by an earlier one still counts. Entries whose hits all lie in private.list files are printed as `private-only`: the default scope skips those files, but `--scope all` still uses the entries. For allow-binary.txt it checks the include and manifest-dir targets, the sources of dynamic reads and the tracked non-text files. allow-archive.txt needs an unpacked release archive (`--archive DIR`, repeatable, e.g. package.sh's `<out>/stage/overbright-*`). Without one, that list is reported as not checked.

## Files (all in scripts/release/)
| file | role |
|---|---|
| `families.txt` | generic pattern data: one regex per line in `[private-path]`, `[owner-remark]`, `[agent-wording]`, `[hardware]`; `[unit-allow]` and `[hyphen-allow]` = tokens the unit-code and hyphen-code regexes must not report. Extend here. |
| `families.private.txt` | maintainer/machine/game-copy specific patterns (same format, plus `[plan-p]`, and `[leak-strings]`: fixed strings, one per line, that must not be in a shipped binary, read by `leak-terms`); listed in `private.list`, never published. |
| `allow.txt` | `path:key:family:reason`; path is an fnmatch glob, family may be `*`; key is `*` (every line of the file) or `=HASH`, the first 12 hex digits of the SHA-256 of the allowed line without leading and trailing white space (`scan.py allow-key PATH LINE` prints it). A keyed entry follows its line when lines are added or removed above it and stops matching when that line is reworded; another line with the same token is not allowed by it. Line numbers are refused. Matching text hits are reported as allowed and do not fail. |
| `allow-binary.txt` | `pattern:class:reason` (fnmatch glob or path prefix): embedded/non-text files that may stay public. |
| `private.list` | path prefixes (`dir/`), exact paths or globs that move to the private companion repo. The text scan skips them (`--scope all` includes them); the binary scan accepts files under them. |
| `test_scan.py` | unit tests with a fixture string per family. |
| `public_tree.py` | copies the public tree (tracked files minus `private.list`) to a new folder, with the root Cargo.toml's private workspace members (devprobes/) left out, so that it can be built and tested on its own: `cargo build`, `cargo test --features dev-harness` (private tests ignored with the reason), `--selftest` and the shots (steps that need the maintainer's saves print `SKIP: ... needs the maintainer's private set (OB_PRIVATE)`; a shot or flow that has nothing else to run exits 3, recorded as skipped by checks.sh; docs/testing.md). |

Scope of the text scan: every tracked file (`git ls-files`; without git the tree is walked minus `.git`, `target*`, `analysis`) that is text (no NUL byte, not a known binary extension), minus `private.list` (the scanner's own files are scanned too). `--scope public` keeps only src/, tests/, scripts/checks/, scripts/perf/, build.rs, Cargo.toml, README.md.

## Text families
Generic patterns live in `families.txt` (public). Everything that names this project's maintainer, machine or game copy lives in `families.private.txt`, which is listed in `private.list` and never published; this document therefore describes the families without spelling the private values.
- **private-path**: home-directory paths (`/home/<name>`), the maintainer's work and agent folders, the game-copy download folder, work-folder shapes (`wf`, `base`, `tgt` plus a number), copies of the original-game prefix named after a task, scratch paths of the build tooling, local environment files and the private task queue file.
- **personal-name**: the maintainer's login and save/profile name, community save names, private fixture folders (private file only).
- **owner-remark**: quoted remarks of the maintainer ("the" plus the word for the player, "report" wording, a speaker-label prefix, dated quotes in parentheses, "the" plus the word for the maintainer and its possessive).
- **agent-wording**: wording of the delegation workflow (the coordinating role, other/parallel/work assistants, delegated helpers, task descriptions) and the names of the assistant tools (private file). Plain "agent" alone is not matched, because the game has AI drivers.
- **hardware**: GPU/CPU model families and sensor names (generic part); the exact model strings and device ids of the build machine and its hostname (private file).
- **third-party-mod**: names of the private HD UI / map mods and their source site (private file).
- **unit-code**: `\b[A-Z]{1,3}[0-9]{1,2}[a-z]?\b` (a work-unit code is one to three capitals, one or two digits and an optional lower-case letter) minus the `[unit-allow]` tokens (pad buttons, function keys, texture formats, display servers, codecs, game data names, world section names; derived from the real hits). In `.rs` files only comments and string-literal contents are scanned, so a constant's name does not count. The allow list is a first cut that the scrub unit extends.
- **hyphen-code**: hyphenated work codes (`scan.py` `HYPHEN_RE`): capitals joined by hyphens ending in a number, a capital and a lower-case letter with a number after a hyphen, the native-fix series names, and review finding ids (an area name, a digit with an optional letter, three digits) minus the `[hyphen-allow]` tokens (licence ids, encodings, checksums, codec standards, car names, display connector names). A reference to a file under `docs/native-fixes/` by its name is blanked first: those file names stay (this also holds for the unit-code family). As for unit codes, `.rs` files are scanned in comments and string literals only.
- **commit-ref**: a hex token of 7-40 characters containing a digit and a letter within 24 characters after `commit|main|base|merge|HEAD`.

## Private pattern data
`scan.py` loads `families.txt` and, when it exists, `families.private.txt` (next to it, else
`$OB_PRIVATE/scripts/release/families.private.txt` for a public checkout that keeps the private set beside it; same format; its sections are merged into the public ones; a `[plan-p]` section holds the release plan's own pattern for the `summary` comparison). `summary` and `text` print `private patterns: loaded N` or `private patterns: NOT LOADED`. `leak-terms` prints the `[leak-strings]` section (one string per line) for scripts/checks/release_check.sh and scripts/release/package.sh; without the private file it prints `private leak terms: NOT LOADED` on stderr and exits 3, and those checks then say that the private terms were not loaded and do not report a plain pass (release_check.sh ends `RELEASE: ok, private terms NOT LOADED` with exit status 3, which checks.sh records as skipped, or fails in a tree that holds the private set; package.sh counts it as a failed check). Without the private file, `text` and `summary` exit 3 (a scan without it must not claim the tree is clean) unless `--no-private` is given explicitly. The scanner's own files are scanned like any other file (no exemption); the public generic patterns are written with a bracketed letter so they do not match themselves, and `test_scan.py` uses synthetic names injected as test families.

## Binary mode
Lists every `include_bytes!` / `include_str!` (path resolved against the source file, or `concat!(env!("CARGO_MANIFEST_DIR"), ...)`) and every `CARGO_MANIFEST_DIR` data read in tracked Rust sources with target path and size, plus every tracked non-text file. Classes: `game-data` (regenerable from the player's own game files), `capture` (memory captures, recorded traces, disassembly), `personal-save`, `machine-snapshot`, `screenshot`, `asset` (legitimate project files such as shaders), `dynamic`, `unclassified` (rules: `CLASS_RULES` in scan.py).
A target/file passes when it matches `allow-binary.txt` or lies under `private.list`. One deliberate stricter rule: a reference from `src/` that is not test code and points at a private path still fails ("private data embedded in shipping src"), since moving the file would break the shipping build; the same reference from tests/ or from test code passes. Test code is an item under `#[cfg(test)]` or `#[cfg(all(test, ...))]` at any indentation (a test module, or a test-only fn inside an impl, up to the brace closing it), and every file of a module declared `#[cfg(test)] mod x;` (with its `#[path]`, or x.rs / x/mod.rs, and the files under that module). A `private.list` folder `d/` also covers a reference to the folder itself (a test listing `d`). A test that passes here still must not `include_*!` a private path: the public tree would not compile; tests read private files at run time through the test kit's `private(path)` (docs/testing.md), the developer harness through `src/bin/game/dev/private.rs`.

## Tables mode
`const`/`static` arrays of at least 24 entries (counted from the initialiser; `[x; N]` uses N) whose doc comment (contiguous `//` block above; for a function-local table the enclosing fn's doc) names an executable address (`0x` plus 5-8 hex digits, or "SPEED2"). These are the tables to migrate to run-time reads from the player's own game executable.

## Current totals (merge base 1, 2026-11-11; full output internal evidence: release/scan-baseline.txt)
| measure | this scan | release plan |
|---|---|---|
| text, tree minus private.list: files / lines with any family | 368 / 5085 | n/a |
| private-path / personal-name / owner-remark | 139 / 869, 73 / 353, 130 / 442 files / lines | |
| agent-wording / hardware / third-party-mod | 63 / 133, 59 / 155, 8 / 12 | |
| unit-code / commit-ref | 324 / 3367, 26 / 41 | |
| private patterns in the public roots (private.list paths counted) | 86 files / 990 lines; the plan's own pattern P re-run: 75 / 936 | 79 / 933 |
| unit codes in src+tests, public roots | 204 / 1730 (family); the plan's upper-bound regex re-run: 195 / 1869 | <= 190 / <= 1828 |
| include_bytes!/include_str! of evidence/fixtures/native_save_fresh | 81 distinct resolved files (14 compiled into shipping src); 233 include references + 7 manifest-dir reads | 92 + 7 |
| tracked non-text files | 1193 (4 not allowed: docs/bayview-*.png, an internal screenshot) | |
| exe-address tables of >= 24 entries | 45 | 43 |

Differences: the tree has grown since the plan's count (P re-run 75 / 936 vs 79 / 933: the plan's grep was run on an older tree and without `-I`). The family union is broader than P (it also covers owner-remark and hardware wording), hence 86 / 990. The plan's 92 counts distinct include *spellings* (`../docs/x` and `../../docs/x` count twice); resolved to repo paths there are 81 files. Tables: the plan's script is not available; this scan finds 45, among them 4 function-local tables whose address sits in the fn doc, and it does not count microtalk `RC` (only a module doc precedes it), so 43 is within the counting method.
