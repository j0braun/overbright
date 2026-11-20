# Architecture

A Cargo workspace. The main package, `nfsu2` (a library and several binaries), sits on the library's lower
layers, `crates/ob-core` and `crates/ob-formats` (see Layers), and on `crates/ob-testkit` (the test kit, a dev-dependency; docs/developing/testing.md). The map editor lives in Studio's window crate, a leaf on the library, `crates/ob-studio-gui`
(its `src/editor/` and the `overbright-editor` binary; docs/modding/map-editor.md), so the game's dependency tree has
no egui; what the game and the editor share (the map data and the simulation stream, `tools::map::…`) is in `crates/ob-map`, below the library. In the maintainer's full tree only there is also the `devprobes`
workspace member (developer probes, not built by default; not in the public repository), and `fuzz/` is a separate crate.
Package dependencies point one way only, from the programs down to the library and from there to the kit
(`cargo metadata` shows no edge back up). The library holds everything that can be tested
without a window; the game binary (`src/bin/game.rs` + `src/bin/game/`) holds the application: the window,
the frame loop and the glue that turns the library's parts into the running game.

## The library (`src/`)

| area | modules | what |
|---|---|---|
| file formats | `formats/` | Pure parsers for the original's files: bundle chunks, string hashes, solids, scenery, textures, tracks and visibility, compressed packs (JDLZ, Huffman), VIV archives, VP6 video, routes, roads, collision, native saves, texture-mod packages (TexMod `.tpf`, TexWizard packs, ZIP). They take bytes and return data; every parser is a fuzz target (docs/developing/fuzzing.md). |
| the player's game files | `game_fs/`, `game_assets.rs`, `exe_tables/`, `fingerprint.rs`, `cache/` | Case-insensitive access to the game folder; which file the game opens for a wanted name (movies, per edition) and the asset check; the engine's tables in code (docs/original/exe-tables.md); the version fingerprint; the cache of converted world and car data. |
| world and cars | `world/`, `car/`, `ground.rs`, `collision.rs`, `smackable/`, `fog.rs`, `sky.rs`, `weather.rs` | A track assembled from the parsers, cars from their parts, the ground and wall queries, knock-over props, fog, sky, the weather cycle. |
| simulation | `drive/`, `handling/`, `contact.rs`, `race.rs`, `drift.rs`, `style.rs`, `nitrous.rs`, `tuning/`, `frame_clock.rs`, `interpolate.rs`, `rng_sites.rs` | Driving (sub-steps, rigid body, suspension, tyres, brakes, aero, steering), engine and drivetrain, contacts, race modes and scoring, the tick clock and interpolation, the original's random-number draw sites. |
| AI | `ai/`, `outrun.rs`, `opponents.rs`, `ai_looks.rs` | Race AI, routes and the road network, traffic, the outrun encounters. |
| cameras and scenes | `chase/`, `event_camera/`, `track_camera/`, `shop_scene/`, `replay_cams.rs`, `character/` | The chase camera, event and track cameras, the in-engine scenes. |
| front end and career | `frontend/` | The menus from the game's FEng packages, fonts and strings; the career, profiles and saves, the HUD, the options and the settings model of the Enhancements page. |
| audio | `audio/` | The original's audio objects and mixer: banks, codecs, the engine chain, car and world sounds, speech, EA Trax and the interactive score, reverb. |
| rendering | `render/` | The wgpu renderer, in three layers (below). |
| input and platform | `pad/`, `platform/`, `allocator.rs`, `latency.rs` | Gamepads (gilrs), folders and paths per OS (docs/playing/platform-dirs.md), the allocator choice, frame pacing. |
| extras and tools | `qol/`, `diag/`, `overlay/`, `mark_spot.rs`, `photo.rs`, `upscale*.rs`, `texture_mods.rs`, `tpf_import.rs`, `texwizard_import.rs`, `hud_mod.rs`, `hd_ui_pack.rs`, `model_fixes.rs` | The opt-in quality-of-life features, the F3 overlay, the support file and event log (docs/troubleshooting.md), report and photo tools, texture-pack generation, the texture mods' and the HUD mod's import. |
| launcher | `launcher/` (binary `src/bin/overbright.rs`), `launch/` | The launcher: game-folder search and check, settings, the start choices, the Mods page (texture mods, the HUD mod), start. CPU-drawn (no wgpu), so it opens without a Vulkan driver (docs/launcher.md). `launch/` is the contract the game and the map editor share with it (`game --probe`'s report, `game --prepare`'s lines, the `error.kind=` line, the look's colour tokens from `crates/ob-look`); the game binary, the map editor and the rest of the library name no `launcher::` path (a few library tests still use the launcher's model). |

### Rendering layers

- `render/core/`: the GPU device and backend choice, shader composition and validation, pipeline cache,
  render-target pool, uploads, worker jobs.
- `render/native/`: what the original draws, as it draws it: world, cars, characters, effects, the post chain
  (bleach bypass, overbright glow, light streaks), the UI and the display path.
- `render/enh/`: the opt-in enhancements, grouped by kind (assets, geometry, lighting, materials, post,
  volumes, weather).

`scripts/checks/layering.sh` enforces the direction: `core` and `native` never reach `enh` or the QoL
settings (settings come in as values) and hold no global mutable state. Native ends where `render/enh/` and
the `qol` / Enhancements-page settings begin; with every enhancement off, the native layers produce the
original's frames byte for byte, which the gate checks.

## Layers

The library is being cut into crates that only depend downward: `ob-core` (`crates/ob-core`: the env macros,
`game_root`, the clocks, the game folder access, the asset cache, the platform folders, the fingerprint, the
logs, the game's random numbers, the camera's values), `ob-formats` (`crates/ob-formats`: the file-format
parsers and the engine's tables, on `ob-core` only), `ob-asset-format` (`crates/ob-asset-format`: the core of
Overbright's own asset format, docs/modding/asset-format/: exact floats, names, digests, bundles, the pack manifest,
layering, and the generator that keeps the specification's rule tables and schemas in step with the code; on
`ob-formats`; nothing in the game reads through it yet), `ob-map` (`crates/ob-map`: the map data — map mod
files and their store, the edit operations, custom races and the original's race tracks and routes they run on,
the simulation stream — on `ob-core` and `ob-formats`; the map editor's session, which owns a world, stays in the
library), then the rest of the library. Beside them, `ob-look` (`crates/ob-look`: the look the launcher, the map
editor and Studio share — colour tokens, lamps, the type, the button legends' data and the words on shared buttons and menu items; no dependencies) sits below the
library, which re-exports its tokens at `launch::theme`. Each layer's
modules are listed in `scripts/checks/crate-layers.txt`; `scripts/checks/crate_layers.sh` (in every checks set)
fails on a reference from a module to a higher layer that is not on `crate-layers-allow.txt` (the references
still to be moved; the list only shrinks), on a package dependency that points up (`cargo metadata`), on a
forbidden crate in a low package's tree (no wgpu, winit or cpal under `ob-core`, `ob-formats`,
`ob-asset-format`, `ob-map` and `ob-look`) and on
`dev-harness` switched on anywhere in the shipped packages' tree. Studio's crates sit on top of the library: `ob-studio`
(`crates/ob-studio`: Studio without a window, no egui) and `ob-studio-gui` (the window). They name the library only
through its one interface for Studio, `nfsu2::studio_api` (`src/studio_api.rs`: the library's items Studio may use,
one by one, under the library's module names), and reach the lower layers through their own crates; the layer check
refuses any other `nfsu2::` path in them (crate-layers.txt `studio` line) and any glob re-export of the library's
own modules in the interface. When the library moves a module, that file changes, not Studio. `ob-studio` declares every kind of content Studio edits (placements and races so
far), its fields and every command once (`ob_studio::registry`); the declarations give the schema export
(`crates/ob-studio/schema/studio.json`, kept in step by a test, `OB_STUDIO_BLESS=1` rewrites it), and a test keeps
each kind's fields, required fields and word lists equal to its file's schema in the asset format
(docs/modding/asset-format/schemas/). Every map edit has a command (an exhaustive match: a new edit does not build
without one). `ob_studio::keys` reads and writes the two kinds' keys (an original placement's `(region, scenery
section, index)`, a pack's own `<pack>:<id>`, `race:<pack>:<id>`) and checks a placement merge's guard against the
base set (`WLD.11`). `ob_studio::history` keeps a mod's history on disk: every step, with its author, is appended
to the mod's `.editor/history.journal` (records with a length and a hash, flushed before the step counts; a record
cut by a crash ends the journal and is cut off by the next writer), the mod's files are written at quiet points and
on save through a temporary file and a rename, and opening a mod whose journal is ahead of its files offers the
unsaved steps back. A change made against an older step is merged item by item (a map mod's details, each prop
edit, each race) and only an item changed both ways is a conflict; undo and redo take back the last step of any
author, `revert` any kept step, and named versions keep a state. One program writes a mod at a time: it holds the
operating system's lock on `.editor/studio.lock`, which goes with the process however it ends, so a crashed
writer's lock needs no clean-up; a second opener gets the mod read-only, told who holds it. Other programs send
changes through `.editor/inbox/` (request files; the reply is written only after the journal has the step, so an
`ok` is final, and a request answered before a crash is not made twice), and a write to the files behind Studio's
back becomes a step under "outside Studio". All file access goes through one trait, so
`crates/ob-studio/tests/history_crash.rs` stops a session at every one of its file operations (fault injection)
and `history_kill.rs` kills a writing child process; the mod always reads back whole, as before or after the
interrupted write. `ob_studio::validate` holds the map checks the editor's Validation tab shows, and `ob_studio::cli` is Studio's command-line tool (`overbright-studio-cli`, docs/modding/studio-cli.md): one verb per registered command, one flag per argument, JSON output and fixed exit codes; its edits are the editor's edits as JSON, made as steps of the mod's history, or sent to the inbox while Studio has the mod open. The library keeps every `nfsu2::…` path: a
module that moves into a crate below is re-exported at its old place. Where a module is split (the cache check, the support
file, the winit and wgpu parts of `platform` and `prof`), the library's `src/lib.rs` keeps the upper part
beside the re-exported lower one. Each library crate has its own build id for the asset cache
(docs/developing/asset-cache.md) and passes `cargo check --target wasm32-unknown-unknown`.

## The game binary (`src/bin/game.rs`, `src/bin/game/`)

| folder | what |
|---|---|
| `shell/` | Start-up, command line, window, the frame loop and the display cadence, first run, the probe for the launcher. |
| `sim/` | The running world: explore, races, traffic, encounters, AI, replay, event cameras, the game's random stream. |
| `update/`, `input/`, `loads/`, `flow/` | Per-tick updates, input handling, world and car loading, the game's flow (boot movies, race set-up and end, free roam, replay). |
| `menus/`, `ui/`, `hud/`, `career_glue/`, `shops/`, `garage/`, `dyno/`, `finish/` | Front-end screens, the HUD, the career's glue, shops, garage, dyno, race finish. |
| `render/`, `enh/`, `audio/`, `effects_glue.rs`, `car_build.rs`, `state.rs` | Drawing a frame, the enhancement glue, the audio inputs, effects, car assembly, the game state. |
| `dev/`, `dev_hooks.rs` | The developer harness (`--shot`, `--selftest`, `--flow`, `--bench`, `--playthrough`, parity, audio capture). Compiled only with the Cargo feature `dev-harness`; the game's paths call it only through `dev_hooks.rs`, which is a set of no-op stubs without the feature (layering rule 3). |
| `tests/` | The game binary's tests (built with `dev-harness`). |

The map editor's binary is `crates/ob-studio-gui/src/bin/overbright-editor.rs`. Other binaries in `src/bin/`:
`overbright` (the launcher), `overbright-upscale` (texture-pack generator) and
developer tools (`inspect`, `world`, `car_check`, `car_inspect`, `car_textures`, `dump_textures`,
`bundle_dump`, `frontend`). `examples/` holds public check programs; `devprobes/` (private set) the developer probes.

## The tick model

The original steps its world once per main-loop frame by that frame's own length (at most 0.05 s), so it plays
differently at different frame rates. Overbright takes the original at 60 fps as the reference
(docs/original/uncapped-fps.md):

1. Real time is counted in whole units of 1/36,000 s (`frame_clock::TickClock`), exact for the common rates.
2. The world advances in ticks. A race tick is 1/60 s, stepped as the original's 60 fps frame
   (`FrameClock`: its 1/1800 s tick pattern 30, 29, 31 and its 1/4000 s carry).
3. While an in-engine scene holds the cars (race start and end, career start, dyno), the tick is 1/30 s,
   as the original's limiter gives it.
4. Each tick's kind is chosen from the state before it, so the sequence of ticks depends on the game
   state only, never on the display.
5. At most 3 race ticks (2 scene ticks) per drawn frame; time beyond that is dropped, so below 20 fps the
   world slows down as the original's cap makes it.
6. Frames between ticks are drawn interpolated (`interpolate`, `Game::draw_between`).
7. Drawing is never capped; an optional frame cap only limits drawing.
8. The front end without a loaded world updates once per drawn frame; its timers are being moved onto ticks.
9. The world state at tick n is therefore the same at 30, 60, 120 or 200 Hz.
10. The rate gate (docs/developing/checks.md) checks this by hashing the simulation state per tick at those rates.

## Data flow at start

`game` finds the game folder (`NFSU2_ROOT`, the kept folder, `--set-root`, the current folder), checks it,
then loads the front end from the game's
packages. Worlds and cars are converted once into the cache (`cache/`) and loaded from it afterwards;
loads run on worker threads behind the original's loading screens.

## The checks

- `cargo test --release --features dev-harness`: pure tests, data tests on the player's files (ignored with a
  reason without them), private tests (docs/developing/testing.md).
- `scripts/checks/checks.sh fast|full`: build, tests, the self-test, headless shots twice for determinism, the
  flows, the settings matrix, the resolution matrix, the fuzz smoke, the shipping-build check
  (docs/developing/checks.md).
- `scripts/checks/gate.sh`: the checks plus parity against a base build: frames, audio and simulation hashes
  (docs/developing/perf-gates.md); `RATE=1` adds the rate gate.
- `scripts/checks/provenance.py`, `layering.sh`, `crate_layers.sh` (provenance tags, render layering, the crate
  layers).
