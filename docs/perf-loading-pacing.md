# Loading waits, first-use pipelines and pacing (2026-11-05)

Wave D this work (roadmap §4 wave D, rules (1)–(5)): shorter transitions and
no first-use pipeline builds mid-play, the same frames, sound and
simulation; an opt-in power-saving frame cap. Nothing here was traced in
SPEED2.EXE: every change computes the same values sooner or with less work
(the arguments below), except Power Saving, a clone option that is Off by
default. Results, gates and logs: internal notes.

## Method

* `--bench transitions` (extended in this work, a measuring aid): every
  transition prints its first frame's parts — the update, the draw (with
  the draw's CPU parts over 2 ms, `prof::acc`) and the GPU wait — and two
  Quick Race starts from the main menu (circuit, drag) were added after the
  existing ones. Boot to the main menu: the whole `--shot main` process.
* `NFSU2_TRACE` (the perf harness's Chrome trace) for each load's critical path across
  threads, temporary spans to bisect the long ones (not kept), and samply
  (`perf.sh`'s user-only build, 1 kHz) over a whole transitions run.
* `render::pipelines_built()` (new, a measuring aid): a relaxed counter
  bumped wherever a pipeline is made (`Gpu::pipeline_cache`, which every
  pipeline site asks once, and the world's, cars' and weather passes' own
  sets). `--bench` prints the pipelines built by the load, in the first
  frame, and each later frame that built one (a first-use hitch).
* Gates: `perf.sh parity` base (main) against this copy, with
  Extreme / Extreme FSR and the pinned front-end shots; A/B numbers with a
  base copy that has only the measuring aids (`work/base-aid`, so both arms
  print the same lines).

## 1. Loading waits

What the loads waited on (warm asset cache, 1080p, reference desktop GPU):

| transition | before | found | after |
|---|---|---|---|
| free roam -> city race | load 52 ms, **first frame +206 ms** | the race's first update spent 205 ms CPU in `handling::launch_times` (0x5B06B0's standing-start loop), run per rival for the belt's quarter-mile speed: a car whose type index is past the 0-60 / 0-100 factor tables (31 entries) has factor 0, so `t · 0` never becomes non-zero and the loop runs to its 999,999-step limit (≈50 ms a car) | see the table in the README |
| any race start with rivals (and free roam's encounter racers) | `line_up_rivals` 44–68 ms wall for 2–3 ms main CPU | one rival's `Car::load_tuned` 41–58 ms wall for ≈10 ms thread CPU: `vinyl_layers` parsed every texture pack of the car's `VINYLS.BIN` (16 MB, JDLZ blocks decompressed one by one on the rayon pool) to keep the 2–8 textures its vinyl layers name | |
| free roam (first load, menu -> free roam, race -> free roam) | `Encounters: cars joined` 27–37 ms wall, < 1 ms CPU | the join waits for the encounter job: its car loads (above), then `AiRoutes::racing_speeds` (28 ms, 23 % SipHash on `LineNode` keys) for the class they decide | |

**The maintainer's e2e regression (race scene first frame, aa898d3 → 45f487e:
Original 43 → 278 ms, Ultra 115 → 338 ms)** is this first item. Bisected
with `--bench race` (fast builds of `git archive` trees, 3 runs each):
`5b57b2e` 47 / 42 / 171 ms, `843261e` (the traffic-mover parity, "BeltMover modes 2/4 for
traffic") 307 / 311 / 244 ms. The traffic-mover parity builds a belt for every traffic car
(`build_belt_state`, 3 new call sites), whose first build asks
`launch_times` for the quarter-mile speed; traffic types are past the
factor tables, so each ran the full 999,999 steps. With the fix below:
main 225 / 234 ms, this copy 61 / 17 ms (a second build
17 / 28 ms) — internal notes.

Changes:

* **`handling::launch_times`** (`src/handling.rs`): the loop also stops
  once every time that can still change is set — a time whose type factor
  is 0 stays 0 for the rest of the loop (`t · 0 = +0`, `t > 0`), so the
  game's remaining steps change nothing returned. Identical results:
  `launch_times_early_stop_is_exact` (tests/frontend.rs) runs the game's
  full loop as a reference for every car type at its own index and at 31
  and 45, all four figures bit for bit. Also speeds `DynoRun::of` for such
  types. The native loop is unchanged (0x5B06B0 runs the 999,999 steps).
* **`texture::parse_texture_pack_where`** (`src/formats/texture.rs`):
  the pack parse with a hash filter; a compressed block whose record hash
  (+0x00) is not wanted is not decompressed. `vinyl_layers` uses it.
  Identical textures in the same order: `vinyl_packs_filtered_parse_matches_full`
  compares it with the full parse filtered, for every pack of every car's
  `VINYLS.BIN` and `CARS/TEXTURES.BIN`, three filters each, and checks no
  full parse fails (a pack damaged in a block the filter skips would make
  the full parse fail as a whole while this one keeps the wanted textures;
  no shipped pack is).
* **`AiRoutes::racing_speeds`** (`src/ai/route.rs`): its two maps keyed by
  `LineNode` (`samples`, `near`) use the in-crate Fx hash (the main-thread pass'
  `fxhash.rs`): only looked up by key, never iterated, so the order (std's
  is random per run already) reaches no output.

Not changed (measured, reasons):

* `WorldRenderer::new: texture arrays` (15–50 ms wall, < 1 ms main CPU):
  the main thread waits for the pool's 1 MB copies into the staging
  buffers (the cache mapping's pages faulted in by the copy). Work on the
  pool, not an idle wait; `MAP_POPULATE` would only move the faults.
* `Encounters::new` 17 ms main CPU on a first free-roam load and the
  encounter racing lines (28 ms per class, memoised per stage / class):
  the class needs the loaded cars; a speculative start for a predicted
  class saves at most the car loads' 8–10 ms. Not done.
* The garage / shop first frame (36–67 ms; `Platform::load`'s JDLZ, the FE
  car load, `static_envmap`): under the 200 ms target; a preload is
  an open idea.

## 2. Frame pacing and power

* **Present mode**: unchanged. Vsync On is `Fifo` on every platform
  (Wayland included; Mesa ≥ 24.3 with fifo-v1 presents it reliably), Off is
  `Mailbox` (agreed deviation) or `Immediate`. Fifo by default with Vsync
  Off would change what the player feels (a wait per frame), so it stays
  the Vsync row's choice. Present timestamps: wgpu 30 exposes none (open,
  wgpu 31).
* **Power Saving** (new Enhancements row, opt-in, **Off by default** =
  today's behaviour; `src/extras.rs` `power_cap`, `Game::power_cap`):
  * Background: the window unfocused or occluded (winit `Focused` /
    `Occluded`) is held to 30 FPS;
  * Background + Menus: also any menu (front end, pause, dialogs) held to
    the monitor's refresh rate (30–240 Hz, 60 when unknown);
  * + Menus, Battery: also the whole game on battery (Linux
    `/sys/class/power_supply`: a Mains supply listed and none online),
    held to the refresh rate.
  The longest applicable cap wins and joins the Frame Rate Cap and the
  scene cap in `redraw` (waited out before input and the simulation, as
  they are). The world keeps its 60 fps cadence through the interpolating
  frame clock, so the simulation does not depend on the drawn rate. Off
  reads nothing (one atomic load per frame). Label id `0xC1_04E0_E8`
  (free in main and every copy, 2026-11-05).
* **Latency with Low Latency off / on** (`--bench latency`, the modern-extras work model
  display with real frames): see the README.

## 3. Smoke particle spikes

No exact cheaper form or cache found; nothing changed. The detailed
smoke's fragment noise (`gpu_particles.wgsl` `fs_main`: two value-noise
octaves, 16 `hash3` a fragment, 55 % of the pass per the GPU encoding pass) takes `q =
(uv · 2.6 + seed · 0.37, t · 1.4 + time · 0.15)`: x and y vary per pixel,
z per puff. Options weighed:
* a noise texture (2-D per puff or 3-D) — the hardware filter's weights
  are not the shader's smoothstep / mix in f32: not byte-identical;
* a per-frame table of the lattice hashes from a compute pass, read in the
  fragment shader — the same expressions, but the compiler may contract
  them differently in a compute and a fragment shader (no `NoContraction`
  in naga's SPIR-V), so bit-identity is not guaranteed and would have to
  be proven per driver; and 16 dependent loads instead of ≈12 ALU each is
  not clearly cheaper on RDNA 3;
* sharing the 8 corner hashes across a 2×2 quad (subgroup quad ops; most
  quads share a lattice cell) — exact (same instruction, same inputs), but
  the noise runs after two `discard`s, so killed lanes leave the quad
  incomplete; computing it before the discards adds the noise to every
  discarded fragment. Needs a measured experiment with `Features::SUBGROUP`;
  open.

## 4. Pipelines built on first use

Counted with `pipelines_built` over 600-frame benches (warm on-disk Vulkan
pipeline cache, so each build was 1–20 ms of draw). Before, built after
the first frame (see the README for the table): Original racing / drive 1
(the motion blur's lerp over the multisampled scene, the first time the
car went fast), Extreme 1–3 (Motion Blur / DOF's passes on frame 1, the
detailed smoke's draw when the first puff appeared, Animated Water's
surface / sheet pipelines when water first came into view), Extreme rain 1
(a weather pass kind starting later). Now built when their passes are
first possible, in the load's first frame:

* `PostFx::multisampled` builds the motion blur's multisampled lerp with
  the multisampled targets (`src/render/postfx.rs`);
* `BlurDof::warm` (`src/render/blur_dof.rs`), called by `blur_and_dof`
  the first frame the passes are wanted: every motion / DOF pipeline for
  the scene's format and sample count;
* `Detail::warm` (`src/render/gpu_particles.rs`, the pipeline build split
  out as `GpuSmoke::pipeline`) and `Water::warm` (`src/render/water.rs`),
  called in `draw_frame` before the passes with the same arguments as
  their draws;
* `WeatherFx::draw` builds all four of its pipelines the first frame it
  draws any.

The same pipelines (same descriptors), only built earlier: frames AE 0.
After: 0 pipelines built after the first frame in every measured scene
and preset.

## 5. The first frame's builds moved to the load (2026-11-12)

Items 2, 7, 13 and 15 of the performance audit. Same objects, built earlier: no
frame, sound or simulation changes (gate byte-identical).

* **Why the first frame built pipelines**: the world's and cars' renderers are made at the load
  with `SceneMode::NATIVE`; the first frame's `WorldRenderer::set_samples` / `CarRenderer::sync_scene`
  then built the scene's set for the frame's mode. With the options' default Full Screen
  Anti-Aliasing (8 samples) even the Original look rebuilt 42 world + 10 car pipelines there.
* **Library entry points**: `SceneMode::driving(samples)` (the frame's mode as a drive will set
  it: the temporal pass by `qol::temporal()`, which the menus' frames leave off),
  `WorldRenderer::warm(gpu, samples)` / `warm_drive(gpu, world, samples)` (the main view's
  pipelines; Light Probes' field and baker, `ensure_probe_baker`; Ambient Detail's props, kept by
  the load's warm-up and the first `ensure_ambient` only if the world still gives the same surfaces,
  `AmbientDetail::made_for`, docs/ambient-detail.md "When the props are placed")
  and `CarRenderer::warm(gpu, samples)` (scene, wet and LOD Fade sets into the device's shared
  cache). The game calls them at the end of `drive_in_loaded` (game-side list:
  `<work dir>/perf2-game-side.md`, not in this tree while `src/bin/game` is frozen).
* **Car and horizon fog modes**: `CarMode` and the horizon fog's mode carry `lamp_features: 0` (their pipelines never
  read it; each new lamp-row combination rebuilt the same car set, ~7 ms).
* **Audio (the audio-banks pass part)**: banks are shared process-wide while held (`audio/banks/cache.rs`
  `shared_bank`, weak entries); InGameCommon.bun's stitch lists parsed once a run
  (`shared_stitches`). The audio's start read MB_Stich_Collision.abk and InGameCommon.bun five
  times, MB_Stitch_Whoosh.abk four; now once. `prewarm_drive_banks` reads the world's banks on the
  car's load worker.
* **Profiles**: `all_slots`, `car_slots` and `to_native` read the current slot
  through `Profile::stashed_slot` instead of cloning the whole profile.
* **Aids**: `NFSU2_PIPELINE_LOG=1` prints each pipeline's call site (and a marker at each
  `save_pipeline_cache`, the load's end); `NFSU2_BANK_LOG=1` each bank read. Test:
  `render::native::car::record::warm_tests` (GPU, ignored by default: after the warm-up the frame's
  calls build 0 pipelines, six presets × 1 and 8 samples; fails with the car warm-up removed).

* **Loading screen over the warm-up (2026-11-12)**: `drive_in_loaded` keeps the load's
  `LoadingGate` (its clock started with the load) until after the warm-up, which runs on a scoped
  worker (`warm_drive`, `CarRenderer::warm`; `Arc<Gpu>`) while the main thread ticks the gate every
  1 ms (at least once), so PC_Loading goes up once the whole load passes 200 ms and animates over
  the warm-up; `loads_shown` and `loads.loading` are taken from the gate after it. A load kept from
  the same drive (`reuse`) has no gate, as before. `--bench transitions` (busy machine): Ambient +
  Probes "free roam, first load" 608 ms / PC_Loading shown 0 (base) → 629 ms / shown 1; every
  Ambient load over 200 ms shown, none under; Original loads under 200 ms none shown (a first load
  that the busy machine pushed over 200 ms is shown, as the rule says; base showed none even at
  207 ms).
* **HD UI first HUD frame (triage row 14)**: with HD UI Art in force (`hd_ui_pack::active`),
  `Game::hud_textures(race, track)` lists the HUD's textures at the load: the track map
  (TRACKMAP<n>, loaded as `map_view` would), the image resources of `hud::package_for(race)`, and
  the quads of a throwaway `hud::Hud` at its start for the drive's car (custom tachometer by rpm and
  HUDINDEX, the text's fonts). `prefetch_ui_textures` binds them through `UiRenderer::render` into a
  1×1 target (the same bind the first HUD frame does: pack art decoded and uploaded), one texture
  between two gate ticks during the warm-up. Test: `tests::hud_texture_list::the_hud_texture_list_covers_the_first_hud_frame`
  (every texture of the first `Game::hud` frame is listed). Bench `explore` with an HD UI pack
  (options.txt `enh.hd_ui 1` via `NFSU2_FLOW_DATA`, `NFSU2_TEXTURE_PACK`): first frame 212–404 ms →
  12–13 ms (4 runs each, load average ~65), load 208–224 → 427–437 ms (TRACKMAP4000's art alone
  150–210 ms), so such a load now shows PC_Loading. Off without the pack (no extra work).

Pipelines built in the bench's first frame (`--bench`, 1920×1080; machine-load independent):
Original 62 → 19, a max-settings profile 234 → 78, Ambient + Probes 64 → 21. What is left: the
`PostFx` chain (17, a bench artefact — the game makes `PostFx` in the menus) and the enhanced passes
that make their pipelines in their own first draw (SSAO, SSR, blur/DOF, volumetric fog, enhance,
water, smoke, glass, lamp shadows; not done here). Timings: internal notes.

## 6. Stalls on a small machine: songs, the garage's first tick, the mirror

Same samples, frames and ticks as before (gate byte-identical); measured
with `--bench transitions` / `--bench explore` and a decode probe, three
interleaved runs per arm on the whole machine and pinned to two cores
(`taskset -c 0,1`, a stand-in for a 4-core laptop).

* **Song decode** (`audio/trax/decode.rs`, `music::Pace`): a song or score
  piece used to be decoded whole on its thread as fast as it could, about
  0.9 s of a full core (song 1, 207 s) in the first second after it
  starts; on a small machine that burst competes with the frames. Live
  (a device mixer) the decoder now decodes `Pace::LIVE.lead` (6 s) at
  once, then at most `speed` (4) seconds of the stream per second of wall
  time, so it stays ahead of the mixer (one second per second) by its head start
  and more. The blocks appended and their samples are the same; an
  offline mix (the captures) decodes unpaced (it renders faster than real
  time). Decoder CPU in the first second: 872–902 ms → 45–46 ms (whole
  machine and two cores alike); after that ~18 ms per second until the
  song is decoded. Tests: `audio::trax::tests::pace_waits_only_past_its_lead`,
  `paced_decode_gives_the_same_samples` (bit-equal to the unpaced decode;
  live, only about its head start decoded a moment in; a dropped song's decoder
  ends).
* **The garage's first tick** (`garage_scene_tick`): a visit's first tick
  loaded the platform, then the FE car (both on the main thread, ~8 ms
  each). The car is now loaded on a scoped thread beside the platform and
  handed to the same `FailedLoads::load`; the later ticks are unchanged.
  "Into the garage" update 20.0 → 14.2 ms (whole machine), 21.4 → 15.9 ms
  (two cores); its first frame 42 → 36 / 43 → 36 ms.
* **The rear-view mirror at a drive's start**: when the first frame shows
  the mirror (Rearview Mirror on, bumper or hood camera), the load's
  warm-up also makes its target and frame passes
  (`WorldRenderer::warm_rear_view`: two pipelines, the `RVM` textures) and
  the cars' reflection pipelines (`CarRenderer::prepare_reflection`, shared
  by the device's cars). Pipelines in the first frame 22 → 19; first frame
  15.1 → 12.3 ms (whole machine; two cores within the noise).
* **Menus**: `icon_menu` decodes the saved profile only for the Career
  and Profile menus that read it; the tuning screens' gear count (asked
  every frame through the master list) is kept per car and parts
  (`DynoTuning::gears`; test `tests::dyno_audio::tuning_gears_follow_the_car_and_its_parts`).

What is left, with numbers from the same runs: the first frame with the
Enhanced passes on (Extreme preset: 240–280 ms, 100 pipelines, 83 of them
the enhanced post passes and effects made in their own first draw — SSR,
blur/DOF, SSAO, volumetric fog, enhance, water, smoke, glass, lamp
shadows); the garage's first draw ("post passes" 16–18 ms, inside the post chain,
not broken down); the shop's first frame (~20 ms: the FE car's
load and renderer, ~8 + 7 ms, the platform memoised since the first
visit).

## 7. The first Enhanced frame

The passes' programs are now made before the frame that needs them (at
the load, and when a Graphics row changes), through a device-wide cache;
the garage's and shops' renderers in the visit's first tick. Method,
numbers and what is left: docs/first-use-builds.md.
