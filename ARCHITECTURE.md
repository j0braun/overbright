# Architecture

One Cargo package, `nfsu2` (a library and several binaries), plus, in the maintainer's full tree only, the `devprobes`
workspace member (developer probes, not built by default; not in the public repository) and a separate `fuzz/` crate. The library holds everything that can be tested
without a window; the game binary (`src/bin/game.rs` + `src/bin/game/`) holds the application: the window,
the frame loop and the glue that turns the library's parts into the running game.

## The library (`src/`)

| area | modules | what |
|---|---|---|
| file formats | `formats/` | Pure parsers for the original's files: bundle chunks, string hashes, solids, scenery, textures, tracks and visibility, compressed packs (JDLZ, Huffman), VIV archives, VP6 video, routes, roads, collision, native saves, texture-mod packages (TexMod `.tpf`, TexWizard packs, ZIP). They take bytes and return data; every parser is a fuzz target (docs/fuzzing.md). |
| the player's game files | `game_fs/`, `game_assets.rs`, `exe_tables/`, `fingerprint.rs`, `cache/` | Case-insensitive access to the game folder; which file the game opens for a wanted name (movies, per edition) and the asset check; the engine's tables in code (docs/exe-tables.md); the version fingerprint; the cache of converted world and car data. |
| world and cars | `world/`, `car/`, `ground.rs`, `collision.rs`, `smackable/`, `fog.rs`, `sky.rs`, `weather.rs` | A track assembled from the parsers, cars from their parts, the ground and wall queries, knock-over props, fog, sky, the weather cycle. |
| simulation | `drive/`, `handling/`, `contact.rs`, `race.rs`, `drift.rs`, `style.rs`, `nitrous.rs`, `tuning/`, `frame_clock.rs`, `interpolate.rs`, `rng_sites.rs` | Driving (sub-steps, rigid body, suspension, tyres, brakes, aero, steering), engine and drivetrain, contacts, race modes and scoring, the tick clock and interpolation, the original's random-number draw sites. |
| AI | `ai/`, `outrun.rs`, `opponents.rs`, `ai_looks.rs` | Race AI, routes and the road network, traffic, the outrun encounters. |
| cameras and scenes | `chase/`, `event_camera/`, `track_camera/`, `shop_scene/`, `replay_cams.rs`, `character/` | The chase camera, event and track cameras, the in-engine scenes. |
| front end and career | `frontend/` | The menus from the game's FEng packages, fonts and strings; the career, profiles and saves, the HUD, the options and the settings model of the Enhancements page. |
| audio | `audio/` | The original's audio objects and mixer: banks, codecs, the engine chain, car and world sounds, speech, EA Trax and the interactive score, reverb. |
| rendering | `render/` | The wgpu renderer, in three layers (below). |
| input and platform | `pad/`, `platform/`, `allocator.rs`, `latency.rs` | Gamepads (gilrs), folders and paths per OS (docs/platform-dirs.md), the allocator choice, frame pacing. |
| extras and tools | `qol/`, `diag/`, `overlay/`, `mark_spot.rs`, `photo.rs`, `upscale*.rs`, `texture_mods.rs`, `tpf_import.rs`, `texwizard_import.rs`, `hud_mod.rs`, `hd_ui_pack.rs`, `model_fixes.rs` | The opt-in quality-of-life features, the F3 overlay, the support file and event log (docs/troubleshooting.md), report and photo tools, texture-pack generation, the texture mods' and the HUD mod's import. |
| launcher | `launcher/` (binary `src/bin/overbright.rs`) | The launcher: game-folder search and check, settings, the start choices, the Mods page (texture mods, the HUD mod), start. CPU-drawn (no wgpu), so it opens without a Vulkan driver (docs/launcher.md). |

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

Other binaries in `src/bin/`: `overbright` (the launcher), `overbright-upscale` (texture-pack generator) and
developer tools (`inspect`, `world`, `car_check`, `car_inspect`, `car_textures`, `dump_textures`,
`bundle_dump`, `frontend`). `examples/` holds public check programs; `devprobes/` (private set) the developer probes.

## The tick model

The original steps its world once per main-loop frame by that frame's own length (at most 0.05 s), so it plays
differently at different frame rates. Overbright takes the original at 60 fps as the reference
(docs/uncapped-fps.md):

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
10. The rate gate (docs/checks.md) checks this by hashing the simulation state per tick at those rates.

## Data flow at start

`game` finds the game folder (`NFSU2_ROOT`, the kept folder, `--set-root`, the current folder), checks it,
then loads the front end from the game's
packages. Worlds and cars are converted once into the cache (`cache/`) and loaded from it afterwards;
loads run on worker threads behind the original's loading screens.

## The checks

- `cargo test --release --features dev-harness`: pure tests, data tests on the player's files (ignored with a
  reason without them), private tests (docs/testing.md).
- `scripts/checks/checks.sh fast|full`: build, tests, the self-test, headless shots twice for determinism, the
  flows, the settings matrix, the resolution matrix, the fuzz smoke, the shipping-build check
  (docs/checks.md).
- `scripts/checks/gate.sh`: the checks plus parity against a base build: frames, audio and simulation hashes
  (docs/perf-gates.md); `RATE=1` adds the rate gate.
- `scripts/checks/provenance.py`, `layering.sh` (provenance tags, render layering).
