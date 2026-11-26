# Game-version fingerprint

Not native: clone tooling (first-run QoL family). Code: `crates/ob-core/src/fingerprint.rs`,
hooks in `src/bin/game.rs` (`--check-game`, the window start) and
`src/qol/` (`crash::report`).

The clone reads GLOBAL/GlobalB.lzc's car, upgrade and career rows and the
other data files; a modded GlobalB.lzc can look and play differently, so
the data's edition is recorded where reports are made. `SPEED2.EXE` is
fingerprinted for the reports only: the game does not read it
(docs/original/exe-tables.md; the asset check below reads its movie-name suffix for
the report), so it is never part of the verdict, and a folder
without it, or with the copy-protected retail executable, is fine.

* `game --check-game [dir]`: after the existing size / XXH3 lines, a
  `fingerprint (SHA-256):` block — size and SHA-256 of SPEED2.EXE,
  GLOBAL/GlobalB.lzc, GLOBAL/GLOBALB.BUN, GLOBAL/InGameCommon.bun,
  FRONTEND/FrontB.lzc, LANGUAGES/English.bin, CARS/TEXTURES.BIN,
  CARS/WHEELS/TEXTURES.BIN, TRACKS/L4RA.BUN and TRACKS/LOC4DYNTEX.BIN (the
  sky textures HD mods replace; optional: missing is no change,
  `fingerprint::OPTIONAL`) (`fingerprint::FILES`), each
  "known" or "MODIFIED or unknown version" (SPEED2.EXE: also "copy-protected
  retail executable", recognised by SafeDisc's sections `stxt774` /
  `stxt371`, `fingerprint::safedisc`); then `edition: …` over the data files
  and, for an unknown or modified one, a WARNING line ("Overbright still
  runs"); a note that SPEED2.EXE is not needed (and what it is, when it is
  not the reference build: `Fingerprint::exe_note`, also an Info line on the
  launcher's check screen); loader / mod files next to the game (dinput8.dll,
  d3d9.dll, SCRIPTS) are listed.
* The window start: the same fingerprint on a background thread
  (`fingerprint::start`, about 27 MB read); the summary is logged — `info`
  for the known edition, `warn` otherwise — and kept. The game never stops
  for it. Headless check modes (shots, bench, flows) do not run it.
* Crash reports: a `game files:` line (verdict, SPEED2.EXE and GlobalB.lzc
  size + SHA-256, mod files), or "(not fingerprinted yet)".
* Mark-this-spot reports (docs/playing/mark-spot.md): the same line.

Lookup is case-insensitive (`viv::find_ci`), as for the game's own files.

## Asset check (movies and menu files)

Code: `src/game_assets.rs`. Report only: nothing here changes what the
game opens. The game and the check use the same resolver
(`game_assets::resolve_movie`, which `movie::path` calls), so the report
says exactly which file plays.

* Movie resolution, any case: the name the executable builds
  (`<name>_fr.vp6` for most names), `<name>.vp6` (the US 1.2 English
  edition has no suffix at all, plus a `movies.tag`), the text language's
  `<name>_<xx>.vp6`, `<name>_en.vp6`, then any one `<name>_<xx>.vp6`
  (docs/playing/native-deviations.md, "Movie files"). A movie with none of these
  is skipped, as in the original (traced: docs/formats/movies-replays.md, IG_PlayMovie and the movie lookup); the game logs `movie <name>: no file
  (tried …)`.
* Edition from the files: the MOVIES layout (US: no suffix; European
  multi-language: several suffixes; single-language: one; mixed; no
  MOVIES folder), the LANGUAGES files, the text file the game loads
  (`LANGUAGES/English.bin`), and the movie suffix the player's SPEED2.EXE
  builds (the string after `%sMOVIES\%s%s`; read for the report only).
* Inventory: the boot movies, the story movies, the tutorials and the
  neighbourhood flybys the game asks for (`game_assets::wanted_movies`),
  and the menu and text files (`frontend::assets::asset_files`, for the active text language), each with
  the file found and how (the built name, no suffix, a language's file) or
  missing with the names tried.
* Where it shows: summary lines in `setup::check_lines` (the launcher's
  check screen, `--check-game`, the support file); the per-group list
  as counts in the check screen's summary rows and Details (`launcher::check`); the full per-file list in
  `--check-game` and the support file ("game files the game looks for:");
  the summary and group lines once in the start log (`game assets: …`,
  from `fingerprint::start`'s thread).

## Known editions

| edition | SPEED2.EXE | GlobalB.lzc |
|---|---|---|
| European PC 1.2 release, `SPEED2.EXE` without copy protection: the traced build (`EDITIONS[0]`; all nine files' hashes recorded; region byte 0x7FA73C = 6, the European value) | 4,788,224 B `8f6a9ae2…dfe2a297` | 5,145,778 B `ad015fdb…4a2d6e90` |
| European PC 1.2 release, the copy-protected retail `SPEED2.EXE` (SafeDisc; `OTHER_EXE_BUILDS`; hash measured on a copy; named for information, works like any other) | 5,987,978 B `4d51643c…818f` | — |
| another SPEED2.EXE build (`OTHER_EXE_BUILDS`, **inferred**: only size and the hash's ends known, from a study of the OpenUG2 project, kept with the maintainer's private evidence) | 4,800,512 B `f9dd86c0…24168a7` | — |

The player's install also holds dinput8.dll + SCRIPTS/NFSUnderground2.WidescreenFix.asi
(the WidescreenFix mod): reported as "mod files present (ignored by the clone)".

## Tests

`fingerprint::tests`: SHA-256 vectors; the edition table's shape; an empty
folder (all missing → unknown); a modified EXE and a lower-case
`global/globalb.lzc` (flagged, found) with dinput8.dll listed; the other
builds' partial matches (the copy-protected one named as such); the
executable never decides the edition (`the_executable_does_not_decide_the_edition`:
missing, another build, SafeDisc sections); with NFSU2_ROOT: the reference install is the known
edition, and a copy whose GlobalB.lzc has one byte (+0x3E0) flipped is
"UNKNOWN OR MODIFIED: GLOBAL/GlobalB.lzc" (the OpenUG2 case).

Every executable works, and so does none: docs/original/exe-tables.md, "Which
executables work".

## Open

Other retail editions (other languages, 1.0 / 1.1, re-releases) have no
recorded hashes: they show as unknown until a user sends a `--check-game`
output. Overbright starts with all of them.

A text language other than English has no recorded size or checksum either. The check requires the chosen
language's file in place of `English.bin`, lists it as "not verified" and leaves a missing `English.bin` out of
the edition verdict (`Fingerprint::verdict_skipping`, `qol::setup::check_for`).
