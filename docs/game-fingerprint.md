# Game-version fingerprint (2026-11-05)

Not native: clone tooling (first-run QoL family). Code: `src/fingerprint.rs`,
hooks in `src/bin/game.rs` (`--check-game`, the window start) and
`src/qol.rs` (`crash::report`).

The clone is traced on one build of SPEED2.EXE (docs/formats.md's addresses
are that build's) and reads GLOBAL/GlobalB.lzc's car, upgrade and career
rows. A different EXE or a modded GlobalB.lzc can look and play
differently, so the build is now recorded where reports are made:

* `game --check-game [dir]`: after the existing size / XXH3 lines, a
  `fingerprint (SHA-256):` block — size and SHA-256 of SPEED2.EXE,
  GLOBAL/GlobalB.lzc, GLOBAL/GLOBALB.BUN, GLOBAL/InGameCommon.bun,
  FRONTEND/FrontB.lzc, LANGUAGES/English.bin, CARS/TEXTURES.BIN,
  CARS/WHEELS/TEXTURES.BIN, TRACKS/L4RA.BUN (`fingerprint::FILES`), each
  "known" or "MODIFIED or unknown version"; then `edition: …` and, for an
  unknown or modified one, a WARNING line ("the clone still runs"); loader
  / mod files next to the game (dinput8.dll, d3d9.dll, SCRIPTS) are listed.
* The window start: the same fingerprint on a background thread
  (`fingerprint::start`, about 27 MB read); the summary is logged — `info`
  for the known edition, `warn` otherwise — and kept. The game never stops
  for it. Headless check modes (shots, bench, flows) do not run it.
* Crash reports: a `game files:` line (verdict, SPEED2.EXE and GlobalB.lzc
  size + SHA-256, mod files), or "(not fingerprinted yet)".
* Mark-this-spot reports (docs/mark-spot.md): the same line.

Lookup is case-insensitive (`viv::find_ci`), as for the game's own files.

## Known editions

| edition | SPEED2.EXE | GlobalB.lzc |
|---|---|---|
| NFSU2 PC 1.2, the traced build (the player's install, `EDITIONS[0]`; all nine files' hashes recorded) | 4,788,224 B `8f6a9ae2…dfe2a297` | 5,145,778 B `ad015fdb…4a2d6e90` |
| another SPEED2.EXE build (`OTHER_EXE_BUILDS`, **inferred**: only size and the hash's ends known, from the OpenUG2 study $OB_PRIVATE/research/openug2-study-2026-11-05.md) | 4,800,512 B `f9dd86c0…24168a7` | — |

The player's install also holds dinput8.dll + SCRIPTS/NFSUnderground2.WidescreenFix.asi
(the WidescreenFix mod): reported as "mod files present (ignored by the clone)".

## Tests

`fingerprint::tests`: SHA-256 vectors; the edition table's shape; an empty
folder (all missing → unknown); a modified EXE and a lower-case
`global/globalb.lzc` (flagged, found) with dinput8.dll listed; the other
build's partial match; with NFSU2_ROOT: the reference install is the known
edition, and a copy whose GlobalB.lzc has one byte (+0x3E0) flipped is
"UNKNOWN OR MODIFIED: GLOBAL/GlobalB.lzc" (the OpenUG2 case).

## Open

Other retail editions (EU / other languages, 1.0 / 1.1 patches, Steam-less
re-releases) have no recorded hashes: they show as unknown until a user
sends a `--check-game` output. Whether the clone runs correctly on them is
not known.
