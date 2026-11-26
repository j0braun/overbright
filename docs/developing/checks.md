# Check harness (2026-11-05)

The checks every merge and every night run. Scripts: `scripts/checks/`
(repo-relative, everything into an output directory, never `/tmp`).
Not native: test tooling only; the game's behaviour is unchanged (the shot
targets below are `--shot` arms, the parity gate passes against the base).

## Commands

```
scripts/checks/checks.sh fast  <outdir>          # merge set   (276 s measured; build up to date)
scripts/checks/checks.sh full  <outdir>          # 274 targets x2 + flows etc. (257-362 s measured)
scripts/checks/checks.sh shots <outdir> T1 T2 …  # just those shot targets (twice, pixdiffed)
CHECKS=full scripts/checks/gate.sh <outdir> <base commit|base dir> [extreme]   # nightly: full + parity (1008 s measured)
scripts/checks/shot_targets.py [repo [game]]     # every --shot target: `game --shot-list` (the shot table, src/bin/game/dev/shot/targets/); trees without it: parsed from fn shot
scripts/checks/halo.sh <outdir> [game]   # the glow shell / blue tint around the car at max in the rain must not return (docs/notes/graphics/wet-road-car.md "The glow shell, the washed road and the distance veil at max settings")
scripts/checks/perfgate.sh run <outdir> <outdir of gate.sh>/b/game   # perf regression gate vs the accepted baseline (docs/notes/performance/budget.md)
scripts/checks/baselines.sh record|check|rebless <outdir> <game> …   # the stored output baselines: frames, sounds, simulation hashes ("Output baselines")
scripts/checks/loadgate.sh run <outdir> base=<game>[@original|@base] new=<game>[@original|@base]   # the load-speed gate: warm, cold, 4 cores ("Load-speed gate")
scripts/checks/selfcheck.sh <outdir>             # the gates' self-test: broken inputs must fail, good ones pass (~20 s)
scripts/checks/devprobes.sh                      # the developer probes (devprobes/) still compile: cargo check -p devprobes
scripts/checks/crate_layers.sh [repo]            # the crate layers: no new upward reference, package edges only down, no dev-harness in the shipped tree, Studio's crates through `nfsu2::studio_api` only (ARCHITECTURE.md "Layers"; in fast and full)
scripts/checks/layering.sh [repo]               # the render layers (core/native: no enhancement layer, QoL read or global state beyond layering-allow.txt; render code: no settings read beyond layering-settings-allow.txt), the game names the harness only behind its feature; in fast and full
scripts/checks/soak_images.sh <game> <outdir> [runs]   # the gate's image dumps N times with one binary (run-to-run flakes; "Image soak")
scripts/checks/fuzz.sh                           # the fuzz smoke: every parser target on mutated inputs (docs/developing/fuzzing.md; FUZZ_SECS, FUZZ_ITERS, FUZZ_ONLY)
scripts/checks/release_check.sh [outdir]          # the shipping binary (no dev-harness): no harness strings, harness flags give the usage error
scripts/checks/resolutions.sh <outdir> [game]    # the resolution matrix, every size x aspect option x 15 screens (docs/notes/graphics/resolutions.md; RES_SET=family|all, RES_MOUSE=1, RES_REPORT_ONLY=1)
scripts/checks/editor_shots.sh <outdir> [editor]  # the map editor's shot matrix, headless (EDITOR_SET=all|family, EDITOR_SIZES; "Editor shots")
scripts/checks/wine_smoke.sh <outdir> [game]     # the Windows dev-harness game.exe under Wine (report-only in full; "Wine smoke")
scripts/checks/traffic_harness.sh <outdir> [bin]  # the traffic harness, this game only: free roam standing and with the scripted player, statistics (report-only, in no set; docs/traffic-harness.md)
scripts/checks/wine_smoke.sh --selfcheck <outdir> [game]   # every Wine smoke check on a broken input: each must fail
scripts/checks/wine_smoke.sh --env-check <outdir>  # the Wine runs' environment on a stand-in for wine (no Wine needed)
ASSET_LAYER=1 scripts/checks/gate.sh …           # the parity with the new binary reading through an import of NFSU2_ROOT (docs/modding/asset-format/import.md §13)
CHECKS=full scripts/checks/gate.sh …            # with the rate gate v2 (counted by default: RATE=strict; RATE=0 leaves it out)
scripts/perf/perf.sh rate <outdir> <game>        # the rate gate v2 alone: hash columns, α=0 images, WAVs across 30/60/120/200 Hz + jitter
```

| set | what |
|---|---|
| fast | build (`--features dev-harness`; then `game --dev-harness` must answer `dev-harness: on`, else the set stops: a binary without the harness is never tested), `cargo test --profile gate --no-fail-fast --features dev-harness` (run through `home_mask.sh`: the player's data and cache folders are hidden behind empty ones in a user + mount namespace, and the step fails when the tests wrote anything there, an entry made and removed again included; `home-mask.txt`), `--selftest`, `prepare.sh` (`game --prepare` on empty folders of its own: ends with `ok=1` and its stamp, a second run changes no cache file, a first load after it finds every pipeline in the pipeline cache, a killed run leaves no stamp and the next one passes; docs/developing/prepare.md), the targets of `fast-shots.txt` (31 on 2026-11-11; the count is whatever the file lists, checks.txt prints it), the four flows side by side (`firstrace`, `firstdrag`, `postrace`, `--playthrough`) |
| full | fast + every target of `shot_targets.py` (274), each run twice and pixdiffed (determinism), intro GPS (offline audio), `--flow rain`, `--selftest-showcase`, explore at 1080p, `--bench drive`, the combination matrix (`scripts/checks/matrix.sh`, the settings model: every Graphics tier on Original and on max, every Customize Display value under max, every pair of rows, every settings-model rule, four scenes, live switches; ~70 s; docs/developing/settings-model.md), `devprobes.sh` (the ~160 developer probes in `devprobes/` type-check; a failure fails the set; `DEVPROBES=1` adds it to fast), `fuzz.sh` (the fuzz smoke, docs/developing/fuzzing.md: 38 targets, 2 s each on mutated inputs over seeds from the player's files; a panic, a call over 3 s or an allocation over 256 MiB fails the set; `FUZZ=1` adds it to fast), `release_check.sh` (the shipping build without `dev-harness`, in `$CARGO_TARGET_DIR/ship`: none of the maintainer's save names, harness flags or harness variables in `strings`, the only `NFSU2_SHOT_*` left those of Mark Spot's report and the renderer's enhancement override, `--selftest` and `--dev-harness` exit 2 with the usage error; prints its size; `RELEASE=1` adds it to fast) , `resolutions.sh` (the resolution-matrix work / the resolution fixes, **a failing step**: `RES_SET=family`, one size per aspect family plus the minimum 640×480 and the portrait 1080×1920, 8 sizes; a FAIL cell fails the set (XFAIL = a case listed in `KNOWN`, the option's own rule), and so does its self-test when a check does not fail on deliberately broken input; prints its `RESOLUTIONS:` and `RESOLUTIONS SELFTEST:` lines into checks.txt; `RES_SET=off` leaves it out; docs/notes/graphics/resolutions.md), `wine_smoke.sh` (the platform work, **report-only** except for the build: the Windows build under Wine, its `WINE_SMOKE:` line and seconds in checks.txt / `steps_s`; a Windows game that does not compile, `WINE_SMOKE: FAIL (build)`, fails the set as `wine_build`; `WINE_SMOKE=off` leaves it out, no wine/Xvfb/podman = skipped; "Wine smoke" below) |

Environment: `CARGO_TARGET_DIR` (default `<repo>/target`), `JOBS` (shots
side by side, default 4), `DET` (two runs per shot; fast 0, full 1),
`PREV=<earlier outdir>` (every deterministic shot pixdiffed against that
run: "changed: …"), `SHOT_CLOCK` (default 0, see below; empty = wall clock),
`SKIP_BUILD=1`, `SKIP_TESTS=1`, `HISTORY` (default `<outdir>/../checks-history.jsonl`).
`TMPDIR` is set to `<outdir>/tmp` (the shots' and flows' own data folders)
and removed at the end; `RADV_DEBUG=nodcc` as `perf.sh parity`.
Every process is waited for by PID; nothing is killed by name.

Output: `<outdir>/checks.txt` (one line per step), per-step logs, `shots/`
and `shots2/` (PNG, log, `shots.tsv`: target, run, rc, panic/validation
count, ms). Exit status 0 only when every step passed; last line
`CHECKS: PASS|FAIL (…)`.

## Build profile (`CHECKS_PROFILE`)

The checks build and test with the **gate** profile (`Cargo.toml` `[profile.gate]`: the release settings
without LTO, 16 codegen units, incremental; docs/developing/build-times.md has the measured times). It computes the
same results as the shipped profile (Rust does no fast-math); only build time and binary size differ.

- `CHECKS_PROFILE=gate` (the default) or `CHECKS_PROFILE=release` (the shipped profile, e.g. to compare
  with an older run), from the environment or `scripts/local.env`. `scripts/lib/common.sh` sets `CK_PROFILE`
  and `CK_PDIR` from it.
- Cargo keeps each profile in its own folder under the target dir: the checks' binaries are in
  `target/gate/` (`game`, `examples/pixdiff`, …), the shipped profile's in `target/release/`; gate and
  release artifacts never mix. The scripts that default to a binary (`resolutions.sh`, `lavapipe.sh`,
  `halo.sh`, `wine_smoke.sh`, `texture_mod_shot.sh`, `map_mod_looks_shot.sh`, `selfcheck.sh`'s pixdiff, `shot_targets.py`) read
  it from the profile's folder.
- checks.txt's first line names the profile (`profile gate`). With `PREV=<earlier outdir>` a run built
  with another profile is named (`note: PREV was built with profile …`; runs without the word were
  release builds).
- **Always the shipped profile (`--release`):** `gate.sh`'s parity (base and candidate are both built by
  `perf.sh build`, so the parity compares like with like), `perfgate.sh` and every perf number,
  `release_check.sh`, the fresh-start launcher build, `pgo.sh` and the release container. `gate.sh`
  prints `gate: checks profile gate; parity builds: release (both sides)`.

## Summary line (diffable)

Each run writes `summary.json` (one line) and `summary.md` (one table row)
and appends the JSON to the history file:
`{"set","date","ok","seconds","bin_md5","tests","shots","shots_failed","nondeterministic_new","changed_vs_prev","failed","steps_s"}`.

## Cargo test total

Before, successful `test result` lines were filtered out, so a test binary
that died (signal, abort) printed nothing and looked green. Now: `cargo test
--no-fail-fast` and the line `tests: N passed, M failed, K ignored; B test
binaries, R results`. Each `Running` / `Doc-tests` header must be followed
by its own result (the last `test result` before the next header: some lib
tests run the test binary as a child, whose result lines appear too); the
step fails on rc ≠ 0, any failure, B ≠ R or 0 passed.

Data tests (docs/developing/testing.md) are listed as `ignored, needs NFSU2_ROOT ...`
when the tests were built without the game, `ignored, needs OB_PRIVATE ...`
without the private set, and `needs NFSU2_ROOT ... and OB_PRIVATE ...` for
both. The line `data tests ignored: G need the game files (NFSU2_ROOT), P the
private set (OB_PRIVATE), B both; ...` counts the three reasons apart
(`ck_data_tests` in scripts/lib/common.sh). With NFSU2_ROOT set (always, here)
G must be 0; with the private set here (`ck_private_here`) or
`OB_REQUIRE=private`, P and B must be 0 too, else the step fails. A public
checkout without the set passes with P and B stated.

## Shot targets added

| target | what | deterministic |
|---|---|---|
| `lapko` | Quick Race circuit with Lap KO (`settings.qr[0].lap_ko`, 3 laps, 3 opponents; `race.knockout`, 0x89E7B0 = 3), full throttle to 3 s after the green | yes |
| `explore_rain` | free roam in the shower: the world clock at 162 s as `NFSU2_RAIN_CLOCK=162` (the bench's rain scene), through a shot-only static (`SHOT_RAIN_CLOCK`; the variable still wins) | yes |
| `photo` | photo mode as `--photo` opens it in free roam: the still with the help bar and Aperture f/2.8 (`photo_check(…, Some(out))` stops there) | yes |
| `options_texture_pack` | Image group, World Texture Pack selected, greyed ("Not Generated") | yes |
| `options_texture_pack_ready` | the same with an (empty) generated-pack manifest written into the shot's own data folder: the row enabled (refuses to run with `NFSU2_SHOT_REAL_SAVE` / `NFSU2_SHOT_DATA`) | yes |
| `crashcam`, `replay` | existed (crash camera, replay at its middle, paused); now in both sets | yes |

## Renamed shot targets

The parity compares shots by name: a base binary draws a target under the name it knows. A renamed target is
listed in `scripts/checks/shot-renames.txt` (`OLD NEW`; `OLD* NEW*` renames a prefix). `gate.sh` takes the parity
shots that the base knows by their own name or by an old one (`shot_renames.py pick`), prints
`gate: renamed shots (new=base): …` and passes the pairs to `perf.sh parity` as `SHOT_BASE_MAP`: the base side runs
`--shot OLD`, the new side `--shot NEW`, and both images are filed under NEW. `shot_targets.py`'s parser for trees
without the shot table lists the variants under their old names. The finished-career variants are named
`finished_career_<target>` (the screen with the finished-career save as the profile): `finished_career_shop_checks`,
`finished_career_specials_hydro3`, `finished_career_specials_purge`, `finished_career_dynorun`, and the prefix on any
other target. The map holds the old names of the maintainer's private set and is part of that set; without it
the map is empty.

## Renamed private fixtures

A base binary reads the private set's files by the names it was built with. A fixture renamed since then is listed in
`scripts/checks/fixture-renames.txt` (`OLD NEW`, paths under the set's root). Before the parity `gate.sh` links each
OLD to its NEW in the set the parity reads (`fixture_renames.py alias`: its own copy of the new tree, or, when the
caller names `OB_PRIVATE`, a copy of that set made of links; never the caller's folder itself) and prints
`gate: renamed private fixtures: aliased N under their old names for the base`. The new binary reads only the new
names. The map holds the old names and is part of the private set; without it the map is empty.

## Determinism (run to run, one binary)

Survey 2026-11-05 (274 targets × 2 runs, 4 at a time, `RADV_DEBUG=nodcc`):
on the wall clock 89 targets differ run to run (listed in
`nondeterministic.txt`, tag `wallclock`: main, career, options, worldmap, the
shops, car lot, crib, memory-card and dialog screens, …). Cause: `Game::now`
without a fixed clock reads `started.elapsed()`, so the pulse of the selected
icon (and similar loops) is at a different phase each run. Fix (harness, no
code change): the checks pin the shot clock with the main-thread pass's `NFSU2_SHOT_CLOCK` at
**0**. Then all 274 repeat byte for byte. Not 10 (the main-thread pass's value): the shots
open their screens at front-end time 0 and draw them at `NFSU2_SHOT_MS`
(5 s); a clock at 10 s catches openings and cross-fades half way (e.g.
`pause_quit` drew the pause menu fading into the question). Against the
wall-clock images, clock 0 changes 100 targets by small areas only (the
selected icon's pulse, 2.5 % of the pixels in `main`; worst 31 689 pixels).
`gate.sh` passes the same clock to `perf.sh parity`, so its shot gate covers
the front-end shots too. Tag `always` in `nondeterministic.txt` is for
targets that differ even with the clock pinned (none today).
`Game::now` runs on the game's ticks, not the wall
clock: the 89 targets repeat byte for byte with the clock unpinned too (each
shot twice with `NFSU2_SHOT_CLOCK` unset, all identical), and their
`wallclock` entries are gone. The pin stays the checks' default (it fixes the
front-end time of the still).

Not standalone (left out of the list: `UNLISTED` in src/bin/game/dev/shot/targets/mod.rs; `EXCLUDE` in `shot_targets.py`'s fallback parser): `shop_checks`,
`specials_hydro3`, `specials_purge` (need the finished career: run as
`finished_career_…`), `vinyl_ask` (fails on a fresh profile, "the vinyl question" not
reached; open, predates the check harness).

## Image soak (`soak_images.sh`, `SOAK=N`; 2026-11-11)

```
scripts/checks/soak_images.sh BIN OUTDIR [RUNS] [SCENES] [SIZES]   # default 10, "drive race", "1280x960 3840x2160"
PAR=3 LOAD=8 IOLOAD=1 scripts/checks/soak_images.sh target/release/game <outdir> 20 drive 1280x960
SOAK=20 scripts/checks/checks.sh full <outdir>    # the soak as an optional step (off by default)
```

The parity gate's Original image dumps (`perf.sh parity`, gate `images`:
`--bench SCENE 600`, a dump every 100 frames, `NFSU2_BENCH_GPU=0`,
`RADV_DEBUG=nodcc`; scene `rain` = drive with `NFSU2_RAIN_CLOCK=162`) run
RUNS times with **one** binary; every run is compared with run 1 frame by
frame (pixdiff). Two runs of one binary must be byte-identical, so any
difference is nondeterminism in the binary or the driver, never a change
between base and new: the tool for a gate that failed `images` by a few
pixels.

* `PAR=n` runs at once (several processes on the GPU, as concurrent gates);
  `LOAD=n` starts n `nice -n 19` busy loops of its own and `IOLOAD=n` n
  write+fsync loops (1 GiB files in OUTDIR), both stopped by PID at the end
  (also on Ctrl-C / timeout).
* `CACHE=shared` (default): one asset cache, filled by a short warm-up run
  per scene (as a gate side after its first run); `CACHE=fresh`: every run
  builds its own cache (the gate side's first run; exposes anything whose
  order changes run to run in the prepared cache).
* `FRAMES`, `EVERY`, `KEEP=1` (keep identical runs' dumps), `PIXDIFF`.
* Output `OUTDIR/soak.txt`: per scene/size how many runs differ from run 1,
  per run the frames with pixels and the largest channel difference, and per
  frame the number of distinct images; `OUTDIR/diffs/<set>/rNN/`: run 1's and
  run NN's frame, a difference image (red = differing pixels, ImageMagick
  `compare`) and the first differing pixels (`*.pixels.txt`). Last line
  `SOAK: IDENTICAL | DIFFER | FAIL`; exit 0 / 1 / 2.
* In `checks.sh` (any set but `shots`): `SOAK=N` adds the step `soak`
  (`SOAK_SCENES`, `SOAK_SIZES`, `SOAK_PAR`, `SOAK_LOAD`, `SOAK_IOLOAD`); a
  run that differs fails the step. Off by default (20 runs of the two
  default sets take ~5 min on an idle machine).

Measured 2026-11-11 (fast release build of the base, reference desktop GPU, machine
load average 20–130 during the runs, `PAR=3 LOAD=8 IOLOAD=1`):

| binary | cache | sets × runs | differ |
|---|---|---|---|
| base | shared | drive 1280×960 × 20, race 3840×2160 × 20 | 0 of 19, 0 of 19 |
| base | fresh | drive 1280×960 × 12, race 3840×2160 × 12 | 0 of 11, 0 of 11 |
| base | shared, `PAR=4 LOAD=12 IOLOAD=2` | all 5 scenes × 2 sizes × 8 | 0 of 70 |
| with the fix below | shared / fresh | as the first two rows | 0 of 19, 0 of 19 / 0 of 11, 0 of 11 |

The gates' three one-off differences of that day (1–4 pixels, ±1, in
consecutive dumps of one set: race 3840×2160 400–600, drive 1280×960
100–300) did not come back in these 182 runs (**open**). Their pattern, a
±1 carried over several dumps, matches a one-frame disturbance carried on
by the post passes' feedback (streak / overbright, as the main-thread pass's frame-0 race in
docs/notes/performance/main-thread.md); no ordering or timing dependence of the Original
bench path was found that could cause one (of the base: the render workers'
in-order queue, the main thread's writes after the scene's hand-off, the
bench's synchronous loads, the fixed clock, the post self-check verdict).

Fixed with it (of the base): `assign_textures`
(`src/render/native/world/build/textures.rs`) numbered the flip-books'
frames (the world's first texture slots) in `World::texture_animations`'
`HashMap` order, a new order every process, so two cache builds of one
binary gave different texture slots, texture-array layouts and track cache
files (`track-L4RA.nfc`, `w.tdmap`); now in hash order: two fresh cache
builds of one binary are byte-identical (the wgpu pipeline cache, a driver
blob, still differs). The images did not depend on the order (12 fresh-cache
runs each of drive 1280×960 and race 3840×2160 identical before the fix).

## Timings (2026-11-05, reference desktop GPU, 16 threads, build up to date)

| run | total | biggest steps |
|---|---|---|
| fast | 276 s | playthrough 178 s (the four flows side by side), cargo test 78 s, selftest 17 s, 28 shots 3 s |
| full | 257–362 s | 274 targets x 2 at 4 jobs 46–141 s, playthrough 118–122 s, cargo test 31–36 s |
| nightly (`CHECKS=full gate.sh`) | 1008 s | the full checks + two full-profile builds + `perf.sh parity` (images, window, enhanced, 24 shots, audio, sim) |

A first build from an empty target dir adds ~3.5 min (sccache warm).

## Machine setup (2026-11-11)

Not native: test tooling only. The scripts carry no private paths. Everything
machine-specific comes from `scripts/local.env` (git-ignored; template
`scripts/local.env.example`, or `NFSU2_LOCAL_ENV=<file>`), sourced once by
`scripts/lib/common.sh` and logged (`local env <file|none>` in checks.txt and
the gate's first line):

| variable | used by | without it |
|---|---|---|
| `NFSU2_ROOT` | every script that starts the game | stops with a clear message (no default) |
| `CHECKS_PROFILE` | checks.sh's `cargo build` / `cargo test`, fuzz.sh, the scripts' default binary paths | `gate` |
| build env (e.g. a fast-build file) | every `cargo` call (a fast-build file's `CARGO_PROFILE_RELEASE_*` values are the ones the gate profile sets itself) | the plain profiles |
| `PERF_WORK` | perf.sh build / profile | `<repo>/target/perf-work` |
| `PERF_BASELINES` | perfgate.sh | perfgate stops with a message |
| `ASSET_BASELINES` | baselines.sh (the stored output baselines) | `$PERF_BASELINES/asset-layer`; neither set: baselines.sh stops with a message |
| `PRESET_user` | perf.sh / budget.sh / perfgate presets `user` and `max` (`max` = it + `MAX_ROWS`) | those presets stop with a message; perfgate's set leaves its two `max` configs out (skipped, named) |
| `IGPU_DEVICE` | `IGPU=1` (MESA_VK_DEVICE_SELECT) | `IGPU=1` stops with a message |
| `SAMPLY` | perf.sh profile | `samply` on PATH |
| `GATE_EXCLUDE` | gate.sh's rsync of the trees | only `/target*`, `.git`, `scripts/local.env` excluded |

Tools are checked up front (`ck_need_tools`: cargo, timeout, python3, …).
`RADV_DEBUG=nodcc` stays the parity/check default: only Mesa's RADV reads it.

## How the gates decide

A step that did not run never counts as passed:

- **checks.sh**: an empty shot list (missing `fast-shots.txt` /
  `FAST_SHOTS_FILE`, a failing `shot_targets.py`, `shots` without targets)
  fails; every shot must write its TSV row (per run); a shot without its PNG
  fails determinism; every step's log (selftest, flows, intro GPS, 1080p,
  bench, showcase) is grepped for `panicked|validation error`
  (`scripts/lib/common.sh`, one pattern everywhere); lavapipe without a
  driver is recorded as **skipped** (`summary.json` `skipped`, last line
  `CHECKS: PASS (full, skipped: lavapipe; …)`), `LAVAPIPE=require` fails it;
  the importer unit tests (`scripts/tests`) run when Python has Pillow, else
  skipped likewise; so are the steps that need the maintainer's private set
  where it is missing (the public tree, docs/developing/testing.md "Private data"): a
  shot or flow that exits 3 with a `SKIP: ... needs the maintainer's private
  set (OB_PRIVATE)` line, each such `SKIP:` line of the selftest (the
  selftest itself still has to pass), and the release check's
  `RELEASE: ok, private terms NOT LOADED` (exit 3; in a public checkout after
  `release: privacy scan: maintainer tool, not in this checkout`). In a public checkout
  `scripts/tests/` and `devprobes/` are absent (both are private): the
  importer tests and the devprobes step are recorded as skipped, never failed.
  The gate's parity (perf.sh) does the same without the set (no
  `tests/fixtures/native_saves/` in `$OB_PRIVATE`, else in the tree): a shot
  that exits 3 with a `SKIP:` line on both sides is listed as
  `shots: SKIP (needs the maintainer's private set (OB_PRIVATE)): …` and left
  out of the comparison, and the `parity` scenario of the sim gate (it starts
  from a private save) is left out with `sim parity: SKIP (…)`. With the set
  there, such a skip stays a failed run. `halo.sh` and `scripts/map_xtest.sh`
  (their default saves are private) exit 3 with a `SKIP (… needs the
  maintainer's private set (OB_PRIVATE) …)` line unless a save of your own is
  named. `set -o pipefail` in every gate script.
- **lavapipe.sh**: matches the renderer's `GPU adapter: ` info line (the
  bench's `adapter: ` too); a shot whose log names no adapter fails (nothing
  would prove lavapipe was used); exit 3 = SKIP.
- **perf.sh parity**: each gate line is an explicit verdict (`FAIL: …` and
  counted). images / window / enhanced: the dump sets are the union of the
  sets that were run and the sets on either side; a set with no frames on a
  side (MISSING), a failed side run (rc ≠ 0 or a panic line on either side)
  or a pixdiff failure (a frame missing on one side, over the limits) fails;
  zero sets compared fails. shots: either side's rc / panic, a missing PNG
  or a real difference fails; an empty `SHOTS` fails. audio: rc, missing
  WAV, difference. sim: rc on either side, no hash table from NEW,
  difference. Unknown gate names stop the run.
- **perf.sh bench**: exit 1 when a run has rc ≠ 0 or (JSON arms) no JSON
  that parses (the warm-up runs too); `bench.log` gets an `expect reps N
  labels … configs …` line and a `bench: N runs, M failed` line. A resume
  checks the binary's hash per label (`runs/.bin-<label>`) and re-runs
  unparsable JSONs.
- **perfstat.py gate**: fails when incomplete: a config of the `expect` line
  with fewer than REPS runs in an arm, a FAIL metric without values
  (`MISSING` cell), an unreadable run file, or no `expect` line.
  `compare` fails on a config run by one arm only.
- **perfgate.sh**: fails when the bench fails (both modes); `accept` checks
  each copy and moves `current` last. Without `PRESET_user` (machine-local, so
  a public checkout has none) the gate set's two "max" configs are left out:
  `perfgate: SKIP <config>: needs PRESET_user …` lines, a `skip` line per
  config in `bench.log`, and the verdict names them
  (`PERFGATE: pass (…) (skipped 2: …)`).
- **gate.sh**: identical base and new binaries pass only when the game's
  sources (`src`, `crates`, `build_support`, `build.rs`, `Cargo.*`) are identical too (a
  docs, scripts or tools-only change such as `examples/`, reported); with different sources it is a FAIL
  (stale or wrong build). An empty parity shot list or a parity log without
  its verdict fails. Last line `GATE: PASS|FAIL: …`; with steps that did not
  run it ends `; skipped: checks N, parity M (named above)` (the CHECKS
  line's `skipped:` list and the parity's `SKIP` lines).

## Output baselines (`baselines.sh`)

What the engine draws, plays and simulates today, stored as hashes, so that a change of plumbing (the asset layer
reading an imported base set instead of the game folder, docs/modding/asset-format/parity.md) can be proven to
reproduce it bit for bit. The parity gate compares two binaries side by side; a baseline is the same comparison
against a stored record, and it can be run with either data source.

```
scripts/checks/baselines.sh record  <outdir> <game> [note]          # run the set, store it; `current` -> it
scripts/checks/baselines.sh check   <outdir> <game> [baseline dir]  # run it again, compare with the store
scripts/checks/baselines.sh rebless <outdir> <game> <reasons> [note]  # an intended change (below)
ASSETS=base NFSU2_BASE_SET=<dir> scripts/checks/baselines.sh check …   # the same set read from the base set
```

- **The set** (`OUTPUTS`, default all four): `images` (Original `--bench` explore, race, racing, drive and rain,
  every 100th of 600 frames at 1280×960 and 3840×2160), `shots` (the gate's shots: `fast-shots.txt` minus
  `nondeterministic.txt`, clock pinned at 0), `audio` (the explore and race shots' WAVs from the offline mixer),
  `sim` (`NFSU2_STATE_HASH` over `--flow firstrace`, `firstdrag`, `postrace`, `--playthrough` and the parity
  scenario; the last needs the private set and is stored as `SKIP` without it). Pictures are hashed decoded
  (`pixdiff --hash`: XXH3-128 of the size and the RGBA8 pixels), so a PNG encoder change does not count; the state
  hash files are kept whole, so a changed run names its first differing step and part (`hashdiff.py`).
  `RADV_DEBUG=nodcc` as the parity gate.
- **The store**: `$ASSET_BASELINES` (else `$PERF_BASELINES/asset-layer`), one folder per baseline with
  `outputs.tsv` (kind, item, hash, detail), `sim/*.tsv`, the binary that made it (`game`), `pixdiff`, `meta.txt`
  (note, source commit, GPU, the set) and the binary's load numbers (`summary.md`, `summary.json`: the load-speed
  gate's warm, cold and 4-core runs). Machine-local, never in the repository: the pictures' hashes depend on the GPU
  and its driver.
- **The source switch**: `ASSETS=original` (the default) or `ASSETS=base`. Every run gets the asset layer's switch:
  `NFSU2_BASE=off`, or for `base` `NFSU2_BASE` set to the folder `NFSU2_BASE_SET` names; a `base` run counts only
  when the game's log has the layer's exit line (`asset layer: ... from base set`) and an `original` run only
  without it, so a binary without the asset layer can never pass as one that read the base set. A baseline is
  always recorded from the original files.
- **Verdicts**: `BASELINES: PASS: identical to the stored baseline` or `BASELINES: FAIL: N items differ` with
  every changed item (`diff.tsv`: item, stored hash, new hash, detail). A run that fails, a missing picture or a
  panic line is a changed item (`RUN-FAILED`), never a pass.

### Re-bless rule

A change that is **meant** to move an output (an AI fix that changes the simulation's hashes, a native fix that
changes a frame) re-records the baselines **in the same change**, and every changed item is accounted for:

1. `baselines.sh check` against `current` lists what moved.
2. A reasons file names the cause for every changed item, one `<glob><TAB><why>` per line (the first matching glob
   counts), for example `sim/*<TAB>rivals brake for the corner as the original (choice: example text for a reason file) does` or
   `image/img-race-*<TAB>…`.
3. `baselines.sh rebless <outdir> <game> <reasons> "<change>"` runs the set again and refuses (`BASELINES: FAIL: N
   of M changed items have no reason`) while a single item is unmapped; with all mapped it records the new
   baseline, links it to the old one and writes `changes.tsv` (item, old hash, new hash, reason) into it.
4. The change's description carries the `changes.tsv` summary: which items moved and why. An item that changed
   without a reason is a bug of the change, not a re-bless.

A change of plumbing (the asset layer, a reader moved, a cache format) never re-blesses: it must pass `check`
unchanged.

## Load-speed gate (`loadgate.sh`)

Fails a change that makes loading measurably slower and names the stage that got slower. The asset layer's moves
(docs/modding/asset-format/performance.md §6) must pass it besides the equality gates.

```
scripts/checks/loadgate.sh run <outdir> base=<base game> new=<new game>        # two binaries
scripts/checks/loadgate.sh run <outdir> orig=<game>@original base=<game>@base  # one binary, both data sources
scripts/checks/loadgate.sh bench <outdir> today=<game>   # runs only;  … summary <outdir>   numbers per stage
scripts/checks/loadgate.sh selftest                      # the statistics on generated data
```

- **Runs.** Per arm, mode and scene `REPS` runs (default 10, at least 8), the arms adjacent and their order turned
  each repetition, after one warm-up run per arm (its asset cache and driver shader cache filled; each arm has its
  own). Scenes (`SCENES`): `--bench boot` (process start to the entry, the first frame and the title) and
  `--bench explore 20` (the first free-roam load of a process and its first frame); `transitions` can be added.
  Every run writes `NFSU2_TRACE`; each span's wall time summed over the run is a stage (`frame: …` spans left out).
- **Modes** (`MODES`): `warm` (every file in the OS's file cache), `cold` (below), `cores` (warm, pinned with
  `taskset -c` to `LOW_CPUS`, default `0-3`: four cores of a low-end machine; the game's thread pools follow the
  affinity). `IGPU=1` runs every mode on the integrated GPU.
- **Cold without root.** `drop_caches` needs root. Before every cold run, `scripts/perf/coldcache.py drop`
  flushes and drops the arm's files from the OS's file cache with `fdatasync` + `posix_fadvise(DONTNEED)`, which
  any user may do on files they can open: the game data, the arm's asset cache and shader cache and the arm's copy
  of the binary. DONTNEED leaves pages that another process maps or keeps reading, so by default the gate first
  makes a **private copy** of the game data in its folder (`COLD_ROOT=copy`; fresh inodes no other program on the
  machine touches, ~1.7 GB, removed at the end) and every mode reads that copy. After each drop the tool measures
  with `mincore` what is still resident and writes it into the run's log (`cold: dropped N MB, resident after M
  MB`); `COLD_ROOT=inplace` drops the game folder itself (other programs reading it then keep pages warm, the log
  shows how many). Reading through `O_DIRECT` was not used: it bypasses the cache instead of emptying it, and the
  game's own reads (mapped cache files, buffered reads) would still hit it. What stays warm: the system's and the
  driver's libraries and the kernel, as after a reboot of a running desktop.
- **Noise model.** Per mode and measure (each total and each stage): the shift = the Hodges–Lehmann estimate of
  new − base (median of all pairwise differences, with its 95 % interval); the noise = 1.4826 × the pooled median
  absolute deviation of both arms around their own medians; the **limit = max(floor, 5 % of the base median,
  2 × noise)** (`NOISE_K`), floors 5 ms for totals and 2 ms for stages (warm, 4 cores), 20 ms and 10 ms cold; and
  the one-sided Mann–Whitney p of "new is slower" (exact without ties). **SLOWER** (fails): shift > limit and
  p < 0.01 / M, M the measures of that mode (Bonferroni: about one false alarm in a hundred gates on noise alone,
  checked by the self-test on 500 generated A/A gates). Shift > limit without that p: "slower, not proven"
  (reported, passes: repeat with more runs). A stage only one arm has counts 0 in the other, so new work (an
  asset layer's mount) is judged too.
- **Verdict.** One line per mode (`loadgate: cold: SLOWER: totals: …; slower stage: explore World::load: from the
  cache +20.1 ms (+…%)`), then `LOADGATE: PASS|FAIL: …`; `loadgate.md` holds every measure's medians, shift,
  interval, noise, limit and p. A run that failed (rc ≠ 0, a panic line, no JSON or trace, a `base` arm without
  the asset layer's exit line, an `original` arm with it) or fewer than 8 runs in an arm fails the gate.
- **Machine.** `LOADGATE_LOCK=<file>`: each repetition holds `flock` on that file (a shared machine's measuring
  lock); `LOADGATE_MAXLOAD=N`: each repetition first waits for a 1-minute load average under N
  (`LOADGATE_MAXWAIT` s at most, default 1800). About 25 s per repetition of two arms, three modes and two scenes on
  the reference desktop, so about 5 minutes for a gate of 10 (plus the copy of the game data, ~10 s on a quiet disk).

**Proof** (reference desktop, other jobs keeping the load average at about 8–40; docs/evidence/load-gate/): the
same binary in both arms passes (`LOADGATE: PASS: no load slower beyond its noise limit`, no measure flagged); a
binary that sleeps 20 ms inside the track's cache read fails in every mode and names the stage
(`LOADGATE: FAIL: slower: warm: explore World::load: from the cache; cold: …; cores: …`, +18.5 / +20.5 / +20.6 ms,
p 5.4e-06). On such a shared machine the load totals' noise (30–170 ms) hides a 20 ms change; the stages, measured
on their own, do not. On a quieter machine (load 5–12) the A/A passes again and the planted run fails warm and on 4
cores with the warm load total flagged too (`explore load +20.3 ms`). Stages nest (`drive_in` holds `World::load`
holds its cache read), so the verdict names the innermost stage that carries the shift: among the slower stages with
at least 80 % of the largest shift, the one with the smallest base time. `loadgate.sh selftest`: the exact U distribution, 500 generated A/A gates of 60 measures
(about 1 % flag anything), a planted 25 ms on a quiet stage found, a new 30 ms stage found.

## Self-test of the gates (`selfcheck.sh`)

`scripts/checks/selfcheck.sh OUTDIR [case]` (part of `checks.sh full`, or
`SELFCHECK=1`): ~20 s, no GPU, no game data. The game is replaced by
`selfcheck_stub.py` (writes PNGs, dump sets, bench JSON, WAVs, state hashes
and the adapter line; faults from a `faults` file next to it); the real
pixdiff compares. Every case must give the expected verdict (the count is on the
`SELFCHECK:` line):

| gate | must pass | must fail |
|---|---|---|
| perf.sh parity | identical stubs; a renamed target with `SHOT_BASE_MAP` | dump set missing on new; zero dump sets; a set on new only; differing frame (images, window); new or base side run rc 1; shot panic / missing PNG / different / rc; `SHOTS=` empty; a renamed target without the map, or different; WAV differs; state hash differs / missing / flow rc / flow panic; unknown gate |
| shot_renames.py | the gate's pick: a base target, an exact and a prefix rename, a target the base lacks left out | |
| perf.sh rate | rate-independent hashes; rate-dependent hashes or images give exit 3 under a policy that reports them | hashes or images depend on the rate (counted, exit 1); a jittered run ends short; a run fails; the self-test injection is missed |
| checks.sh | `shots` (DET=1), `fast` with two shots | `shots` without targets; shot panic / rc / nondeterministic / no PNG; empty or missing fast list; flow panic / rc; no NFSU2_ROOT |
| perf.sh bench | complete | a failed run |
| perfstat.py gate | complete A/A | a config missing in one arm; one rep missing; a truncated JSON |
| perfgate.sh run | A/A on stubs; without `PRESET_user` the two "max" configs skipped and named in the verdict | a failed bench; with `PRESET_user` a bench failing only in the "max" configs |
| halo.sh | in a tree without the private set and no `OB_PRIVATE`: exit 3 with the SKIP line naming the set | with the save in `$OB_PRIVATE` a failing bench (the check ran) |
| lavapipe.sh | llvmpipe adapter | hardware adapter; no adapter line; panic; (no ICD → rc 3) |
| matrix.sh | clean | panic line |
| wine_smoke.sh `--env-check` | the script as it is | copies of it whose windowed wrapper keeps `NFSU2_HEADLESS_ONLY`, keeps the caller's `DISPLAY` or `WAYLAND_DISPLAY`, or whose headless runs do not set the guard |

## Rate gate v2 (on by default, `RATE=strict`)

Rule: the game is never locked to 30 or 60 fps, and gameplay and look
are identical at 30, 60, 120 and 200 fps (reference: the original at a steady
60 fps). The gate runs
ONE binary against itself across display schedules; it is not base vs new.

**Run it:** `scripts/perf/perf.sh rate <outdir> <game>` (alone);
`scripts/checks/checks.sh full …` and `scripts/checks/gate.sh …` run it by
default (with `CHECKS=fast` the gate's parity runs it as its `rate` gate).
`RATE=strict` (the default) fails on any difference across rates; `RATE=1`
on a run that failed, a missed self-test, or a column / image class
`scripts/checks/rate-columns.txt` marks `fail` — since the final policy every
state column at every schedule (jitter included) and every image class, so
the two agree; `RATE=0` leaves the gate out. A jittered run that ends short
of the others counts (the runs end on a tick count). perf.sh rate's exit: 0
all identical, 1 hard (counted column or image class, failed run, missed
self-test), 3 only report-only differences. About 15–25 min on the loaded
machine (see the evidence file's last line).

**What it guarantees** (docs/original/uncapped-fps.md "Rate independence: what the
gate guarantees"): all 600 state cells (15 columns × 10 flows × 30 / 120 /
200 Hz and jitter) identical to 60 Hz; every offline mix byte-identical;
the racing bench's α = 0 frames identical to 60 Hz in the `orig`, `step` and
`hist` classes; the `taa` class (Extreme with TAA, a history over drawn
frames by nature) within mean |Δ| ≤ 2.5/255 per frame.

**Schedules** (`NFSU2_RATE`, `src/bin/game/dev/rate.rs`): `30 60 120 200`:
frames of exactly 1/hz s in 1/36 000 s units; the f32 interval handed to
`Game::advance` is compensated so the sum of intervals never falls behind the
exact time (the cadence sees the ticks at the integer schedule's frames;
`naive:<hz>` keeps the old uncompensated 1/hz, which shows the old drift). `jitter:<seed>`
(default `jitter:7`, `RATE_JITTER`): 4–40 ms frames plus a 120 ms stall every
4–6 s; hashes only. In a rate run the game's clock is its own (`Game::now`/
`seconds` on ticks; the harness pins no clock), the harness
keys every flow input on the world-update (tick) index (`rate::keyed`; the flows drive `ticks::Frames`, one tick at a time inside the drawn
frames, and queue their keys for that tick through the input queue,
`Game::input_at`, so an input or a check happens at its tick, also
in a frame that holds two; this is the default without `NFSU2_RATE` too), and runs each frame's draw-side state updates headless
(`rate::drawn`: HUD and menu layout, the mouse map's frame, the garage's
request, `frame_drawn`; `NFSU2_RATE_DRAW=<mask>` picks them). Without
`NFSU2_RATE` the flows run their 1/60 s frames (or `NFSU2_*_HZ`) through
the same tick driver, the benches as before.

**Flows** (`RATE_FLOWS`): `firstrace firstdrag postrace` (as before),
`menus` (main → Career and back → Options: Audio, Display → Quick Race:
Circuit, track, the car lot stepped three times, into the race),
`garage` (body-shop parts list stepped, Hydraulics with a level-2 part
pumping, Customize categories), `scenes` (the career's start scene, the dyno's
DynoEnter, stage 0's race start), `outrun` (the explore outrun flow),
benches `racing explore replay` (`RATE_SECS`, default 10 s of ticks at
every schedule: `NFSU2_RATE_TICKS`, the frame that would pass the last tick
cut to end on it; replay after 600 race frames). Each runs at every schedule plus a second run at the reference
(`ctl`): a column that differs run to run is marked `nd`.

**Columns** (`NFSU2_STATE_HASH`, `src/bin/game/dev/state_hash.rs`): the eight
legacy columns unchanged (`step dt all global player race rivals traffic`;
`global` keeps the drawn-frame counter, so the base-vs-new sim gate compares
old and new binaries as before), then `rate` (R / S / F: the cadence the state
picks next), `gsim` (globals without the frame counter: mode and menu kind,
every RNG stream reachable — game, race field, autopilot, traffic, encounters
and their field, particles —, game and world clocks, the update counter, cash,
the scene clock, the replay's playback), `clk` (`now`, `seconds`, the stepped
world time), `enc` (zone, encounters and outrun), `hud` (`Hud` and `HudState`),
`fe` (the front end's remembered state and hovers, the icon menu's opening,
the garage FE), `fx` (particles by key, life and position; the rain: RNG,
drops, splats, wind), `vis` (overbright level, nitrous blur and camera
velocity, tint, fog, horizon fog, flare blink, screen flash), `envf` (the
env cube's per-tick schedule: the tick's schedule index, the faces due at
it and the eye they are rendered from; not the drawn-frame counter) and `aud` (what has been posted
to the mixer: voices started, each live voice's source, handle, gain, pitch,
pan and send). `hashdiff.py` (the sim gate) matches columns by name, compares
the common ones and says which only one side prints; the policy file's
`parity` field: legacy columns `fail`, v2 columns `report`. `clk` also holds
the GameClock (tick, package clock, real-time clock, sim clock, world
counter); it and `fe` do not read the wall clock.

**Images**: the racing bench at the numeric rates dumps its α = 0 frames at
ticks ≡ 0 (mod 6) from the schedule's start (`NFSU2_RATE_DUMP`); `rateimg.py`
compares them by tick against 60 Hz for `orig` (Original), `step`
(Extreme with Anti-Aliasing 5 and no volumetric fog) and `hist` (Extreme),
each AE 0, and `taa` (Extreme with TAA: mean |Δ| ≤ `TAA_MEAN`/255 per frame,
default 2.5; p99.9 is not measured, pixdiff gives no histogram, the max
channel diff is reported). `images@CLASS fail` in `rate-columns.txt` makes a
difference (for `taa`: one over its bound) a hard failure of the rate gate;
all four count. **Audio**: each run's offline mix (`NFSU2_AUDIO_WAV`,
rendered per tick after the tick's sounds: each tick its real-time
length, 800 frames a RACE tick and 1600 a SCENE tick at 48 kHz, flows and
benches alike, docs/original/uncapped-fps.md "Audio on ticks"; before: pumped per world update; the bench pumps 800 / 1600 samples per tick in a rate
run) compared byte for byte (`ratewav.py`). **Self-test**:
`NFSU2_RATE_SELFTEST=<tick>` (harness only) draws once from the game's RNG per
drawn frame from that tick; firstrace at 60 and 120 Hz with it must show a
column differing earlier than without it, at or after the tick, else `FAIL:
RATE SELF-TEST: MISSED`. `selfcheck.sh` has the stub cases (identical,
rate-dependent, report-only exit 3, run fails, self-test missed).

**Output**: `<outdir>/rate.txt`: per flow one line (steps and digest per
schedule, the control, per rate the identical columns and each differing
column's first tick), its WAV line, the image lines, the self-test line, the
table (`ratetable.py`: flow × rate × column, `=` identical, a number = first
differing tick, `end@N`, `nd…`) with the identical cells per rate and in
total — the number later units move — and `RATE: PASS|DIFFERENCES|FAIL`.

**Baseline (2026-11-12, `docs/evidence/checks/rate-2026-11-12.txt`; 334 s on
the loaded machine; exit 3 = report-only differences; self-test caught at
gsim@300):** identical cells **236 of 600** (30 Hz 14/150, 120 Hz 110/150,
200 Hz 101/150, jitter 11/150). In short:
- 30 Hz and jitter differ in the sim columns from the first frame ≥ 25 ms
  (tick 180 race scene, 593 racing, 1201 explore) and run fewer ticks.
- 120 / 200 Hz: `player race rivals traffic gsim fx enc` identical in every
  flow but `menus` (the front end without a world ticks per drawn frame,
  so `dt`/`clk` differ from tick 0 and the race it starts from tick 1004);
  `envf` differs everywhere (env-face counter per draw); `hud` in the
  races from the first HUD tick (the HUD is stepped in the draw); `vis` in
  the benches from their first drawn frame (tint/fog/overbright per
  draw); `clk` at 200 Hz (the frame that runs tick n starts later than n/60 s:
  wall-clock FE time); `fe` in `garage` at 1080 (F4–F7) and `menus` at 60.
- `aud` differs run to run at 60 Hz in firstrace, firstdrag, scenes, racing,
  replay (`nd`; control) — an open determinism finding — and by
  rate in menus/garage from tick 1–2 and outrun from 193.
  **With the audio clock** `aud` is identical run to run in every flow (five 60 Hz
  runs each) and is no longer excluded from the self-test (`RATE_ND`); the
  causes and the audio clock: docs/original/uncapped-fps.md "Audio on ticks". What
  still differs at 120/200 Hz are sounds posted from the draw: the garage
  request's turn and part sounds (`garage_request`). **With the HUD on the tick**
  the HUD updates on the tick and its sounds go out with the tick's audio:
  `hud` is identical to 60 Hz in every flow wherever the sim is, `aud` in
  explore and outrun at 120/200 Hz (rate-columns.txt counts them;
  docs/original/uncapped-fps.md "HUD on ticks"; evidence rate-2026-11-12-u4.txt).
- Images: no α = 0 frame at 30 Hz; 120/200 Hz every frame differs
  (Original and step: max diff 181–249); 60 vs 60 differs in some frames too
  (see the evidence line `ctl`).
- WAVs: identical only in some flows at 120/200 Hz; see the `wav` lines.

**Bisected:** firstdrag ≥ 120 Hz from step 1909 (2026-11-11
baseline, binary 2f1e2af): the harness, not the game. That flow's bot counted
its shift and steer waits per drawn frame (`shift_wait -= dt`,
`steer_hold -= dt` each frame), so at 120/200 Hz a steer key was released a
world tick off the 60 Hz run's; the first state to differ was the player's car
(`player`, then `race`). Fixed by the drag check (10d7a13: race-clock deadlines, one
decision per world update), already in this base: firstdrag at 60/120 Hz is
identical over all 4576 steps without `NFSU2_RATE` and in every sim column
with it. postrace from step 0: also the harness: its Enter that skips the
start scene was keyed on the drawn-frame count (`frame > 2`); the scene ticks
at 1/30 s, so at 60 Hz the 3rd frame boundary falls after tick 0, at 120/200 Hz
before any tick (25 / 15 ms < 33.3 ms) and at 30 Hz after tick 2 — the scene
was skipped at a different tick (step 0 at 120/200, step 1 at 30). With the
tick-keyed input of a rate run postrace is identical at 120/200 Hz in every
sim column. No production code is involved in either; a later harness change (to the
cadence) makes tick keying the default for the flows outside rate runs.

### Base binary cache

`gate.sh` builds the base tree and the new tree and compares their output. With `GATE_BASE_CACHE=<dir>` (set it in
`scripts/local.env`) the base binary is kept per base source tree (keyed by a hash of `src/`, `examples/`, the build
files, the build script and the compiler version) and reused by later gates against the same base.

## Editor shots (`editor_shots.sh`)

The map editor (`overbright-editor`, docs/modding/map-editor.md) draws its whole window off-screen with `--shot`; this
step runs it on three screens at the sizes of the resolution matrix (`resolutions.py`'s `MATRIX`; `EDITOR_SET=family`:
its `FAMILY_SET`) plus `2560x1440@1.5` and `3840x2160@2` (display scales; `EDITOR_SCALES`):
`start` (no map mod open), `props_3d` (a map mod made by the script, with no edits, open on the city with the
props' side in front and the 3D view, the prop browser below) and `races_map` (the races' side in front on the 2D
road-network map, the validation list). Every run starts with no display variable at all (`DISPLAY`, `WAYLAND_DISPLAY` unset), so an editor that
tried to open a window would fail; each cell has its own data, cache and maps folders under the output dir.
A cell passes when the run exits 0, its log has no panic / validation-error line, the PNG has exactly the asked
pixel size, the image is not one flat colour (grey standard deviation ≥ `EDITOR_MIN_STD`, 0.02) and, on the two
world screens, the middle 30 % of the window (the view) is not flat either and the log says `editor layout:
toolbar fits` (the editor checks its own toolbar after the last frame: no item cut off at the window's edge or on
top of another; otherwise it logs `editor layout: toolbar clipped: …` with the items, and the cell fails). A
`--shot` runs on a fixed clock (no frame time in the status bar, the view's animations at a fixed time), so the
first world cell is drawn a second time and must give the same picture pixel for pixel (`EDITOR_REPEAT=0` skips
this; a `repeat` line in results.txt). Sizes whose window in points (size ÷
scale) is below the editor's smallest window, 960×600, are listed as `n/a` and not run (640×480 and 800×600 of
the matrix). `checks.sh full` runs the family set (a failing cell fails the set); `EDITOR_SHOTS=1` adds it to
fast, `EDITOR_SHOTS=off` leaves it out. The whole matrix (75 cells) took about 2.5 minutes here at `JOBS=2`.
Last line `EDITOR_SHOTS: N cells, P pass, F fail; K sizes under the smallest window (S s)`. selfcheck.sh runs it
on the stand-in editor (`selfcheck_stub.py` under the editor's name): a good run passes; a failing exit, a panic
line, no PNG, a wrong size, a flat image, a flat view, a clipped toolbar, a picture that differs between two runs,
an unknown screen, a missing binary and a copy of the script that keeps `DISPLAY` must each fail.

## Wine smoke (`wine_smoke.sh`, 2026-11-12)

The Windows build (`x86_64-pc-windows-gnu`, the developer harness) under Wine on the reference desktop, headless:
a private Xvfb (`-displayfd`, or `WINE_DISPLAY=<n>`), a fresh private `WINEPREFIX` under the output dir (never
`~/.wine`, never the original-game prefix) pinned to the x11 driver right after `wineboot`; every Wine process
(`winepath` included) gets `DISPLAY` = that Xvfb, no `WAYLAND_DISPLAY` / `XDG_SESSION_TYPE` and
`XDG_RUNTIME_DIR=<outdir>/xdg`; at the end only that prefix's wineserver (`wineserver -k` with its `WINEPREFIX`)
and our Xvfb (by PID) are stopped.
- `NFSU2_HEADLESS_ONLY`: `checks.sh` exports it for everything it starts, and Wine hands the Unix environment to
  the Windows program, so the windowed steps (`w_window`, `l_play`) drop it (one wrapper, only with our own
  display set); every other Wine run sets it itself (stand-alone runs too), and the Linux reference shots do.
- Never heard: the prefix has no sound driver (`HKCU\Software\Wine\Drivers` `Audio` empty, set right after the
  first boot; Wine's ALSA driver opens the machine's sound cards directly, which a private `XDG_RUNTIME_DIR` does
  not stop), and the windowed steps start the game with `NFSU2_AUDIO_HOST=none` (no sound device is opened).
  They also set `NFSU2_KEEP_RUNNING=1`: nobody focuses their window, and In the background's native Pause
  would stop the game (docs/original/focus-loss.md). The other windowed scripts (`*_xtest.sh`,
  `dyno_xvfb.sh`) set it too.
  `w_window` and `l_play` fail when their logs say a device was opened (`audio: … output at`, an ALSA message);
  `--env-check` proves the wrapper carries the switch. The launcher's fresh-machine test starts its game the
  same way and fails on the same line.

- The exe: `WINE_EXE=<game.exe>`, else built by `scripts/release/container/build.sh --windows --dev-harness
  --fast` into `WINE_BUILD` (default `$CARGO_TARGET_DIR/<smoke build dir>`; a cold one compiles every crate for Windows, a warm one took ~3–4 min here).
  `WINE_DXC_DIR=<dir with dxcompiler.dll [dxil.dll]>` copies DXC next to it (not shipped in the tree,
  docs/playing/platform-windows.md).
- Steps (one line each in `<outdir>/wine_smoke.txt`): `harness` (`--dev-harness`), `version`, `check_game`
  (the game as a `Z:\` path), `selftest` (Vulkan), `vk_<t>` for `WINE_SHOTS` (default `main explore race qrcars
  career`: exit 0, a Vulkan `GPU adapter:` line, the image vs Linux's within the tolerance), `vk_fallback`
  (`explore` with `NFSU2_GPU_DROP=multiview,pipeline_cache`, the features DX12 lacks: the fallbacks' log line
  and the image vs Linux's with the same drop), `dx12_main` (`WGPU_BACKEND=dx12`, Wine's vkd3d; `WINE_DX12=0`
  skips it). Every log is grepped for panic / validation-error lines.
- The private set: `OB_PRIVATE` (else the repository) is handed to every Wine run as Wine sees it (`Z:\…`); a
  Windows harness built in the container with the set otherwise looks for it under the container's `/src`
  and fails (the harness builds the path from its parts, so the message has one kind of separator).
- The launcher (`overbright.exe` beside the build's programs, `WINE_LAUNCHER=` to name one; skipped without):
  `l_render_<s>` (the Mods page, its HUD states, the HUD mod browser, the first-run screen vs the Linux
  launcher's renders), `l_list` (`--list-folder` of a folder with accents as a drive-letter path over 300
  characters, whatever the output dir's length: past MAX_PATH), `l_hud` (`--add-hud-mod` with the stand-in of scripts/checks/hud_mod_standin.py: the 7-Zip file
  chosen, its extracted folder and the map ZIP beside it used, 710 PNGs in the data folder), `l_tpf`
  (`--add-packages` with a package made for another game: its result line), `l_start`
  (tests/launcher_start.sh under Wine: the failure screen after a crashing start, launcher.log, a panic in the window reported with exit code 101), `l_mouse`
  (tests/launcher_mouse.sh under Wine: the folder browser, This PC and the drives, a game folder and a package
  by the mouse alone, docs/launcher.md "The mouse"), `l_play` (the launcher with
  "Start the game directly" starts `game.exe` beside it: the game's log reaches the title screen).
- The map editor (`overbright-editor.exe` beside the build's programs, `WINE_EDITOR=` to name one; a build made
  here must have it, with `WINE_EXE` and none found it is skipped; `LINUX_EDITOR` draws the references):
  `e_start` (`--shot` with no map mod open, 1280×720), `e_props` (a map mod made here open on the city, the
  props' side in front, the 3D view: the world loaded and drawn under Wine, 1280×720) and `e_races` (the races'
  side in front on the 2D map, 1600×960 at display scale 1.5): each exits 0, writes a PNG of the asked size, and `e_start` and `e_races` match
  the Linux editor's shot with the same arguments within the tolerance (both sides name their maps folder
  relatively, so the opening screen prints the same text). The 3D view is not the same run to run even on Linux
  (the world's streaming: a few hundred to a few thousand pixels between two Linux runs), so `e_props` only reports
  its comparison and instead needs the view in the middle of the window not to be one flat colour. `--selfcheck` adds the editor cut to 64 KiB (`e_start`) and
  a good editor's `e_start` compared with `e_races`'s image.
- Studio (`overbright-studio.exe` beside the editor, `WINE_STUDIO=` to name one; skipped when there is none;
  `LINUX_STUDIO` draws the reference): `s_start` (`--shot` on Home with the same maps folder, 1280×720) exits 0,
  writes a PNG of that size and matches the Linux Studio's shot within the tolerance. `--selfcheck` adds Studio cut
  to 64 KiB.
- `w_window`: the game in its window on the Xvfb (no flags, Vulkan through Mesa's software present), reading the
  monitor and its modes and reaching the title screen (Enter skips the boot movies).
- Drive-letter paths: `winepath -w` cuts its answer at MAX_PATH (Wine 11: a 282-character folder came back as its
  first 257 characters, a folder that does not exist); a path outside the prefix's `drive_c` is built as `Z:` plus
  the path with backslashes instead whenever winepath's answer is a cut-off start of it.
- Reference images: `WINE_REF=<dir>` (checks.sh passes its own `shots/`, same clock and isolation), else the
  Linux game (second argument) shoots them.
- Tolerance (`pixdiff --max-pixels $WINE_TOL_PIXELS --max-diff $WINE_TOL_DIFF`, default 64 pixels, channel
  diff 4). Measured: winevulkan on the host's RADV gives **byte-identical** images to the Linux build (0 pixels
  in all 6 Vulkan shots, two separate runs, 1280×960); same naga SPIR-V, same driver. The margin only covers the
  known run-to-run quirk of the Linux gate (1–4 pixels, channel diff 1); anything larger is a real difference.
  DX12 has no measured tolerance (it does not render here).
- DX12 here: the device opens on vkd3d, but Wine's FXC (d3dcompiler_47 on vkd3d-shader) rejects every shader
  with a sampler (`E5017 … Shader model 5.1+ resource array`; wgpu 30 binds samplers through an SM 5.1 heap),
  and a compile error is fatal in wgpu. Without DXC that is recorded as **XFAIL** (counted as passed);
  any other DX12 outcome that is not a match is a failure.
- `--selfcheck`: the exe cut to 64 KiB (`harness`, `version`, `selftest`, `vk_broken_exe`, `dx12_main` must fail),
  a folder that is not the game (`check_game`), main compared with race's image (`vk_wrong_ref`), a log with a
  planted panic line (`panic_grep`): `WINE_SMOKE_SELFCHECK: PASS` only when every one failed.
- `--env-check` (no Wine, no Xvfb, no game; in `selfcheck.sh`): with `NFSU2_HEADLESS_ONLY`, `DISPLAY=:0` and
  `WAYLAND_DISPLAY` set as checks.sh and the desktop leave them, `w_window` and `l_play` run their own code with a
  stand-in `wine` (it writes the environment the program gets and refuses the window under the guard, as the game
  does) on a display name no X server has, and one headless run starts without the guard inherited: the windowed
  steps must reach the stand-in's title screen, every run must see only our display and runtime folder, the
  headless run must carry the guard. `WINE_SMOKE_ENVCHECK: PASS|FAIL`.
- Exit 0 pass, 1 fail, 3 skip; last line `WINE_SMOKE: PASS|FAIL (…)|SKIP (…) (N s)`.

What it does **not** prove: real Windows drivers (NVIDIA/AMD/Intel Vulkan ICDs, real D3D12 drivers) — under
Wine the host's Mesa driver renders; DX12 rendering at all, and real FXC/DXC behaviour; NTFS and Windows file
semantics (mapped-file replace, sharing violations, antivirus, OneDrive, MAX_PATH, non-ASCII user names);
audio devices (WASAPI, device switching); gamepads and wheels (WGI/XInput/DirectInput); frame pacing and timer
resolution (Wine's `Sleep`/`timeBeginPeriod` are not Windows'); exclusive full screen, Alt+Tab, DPI scaling and
multi-monitor (the shots are offscreen); the window icon and manifest as Explorer shows them; SmartScreen /
Defender on an unsigned exe; performance. Those stay on the tester checklist (real machines).

