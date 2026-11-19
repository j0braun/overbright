# Main-thread CPU

Less CPU per frame on the main thread, the same frames, sound and
simulation. No native behaviour changed and nothing here was traced in
SPEED2.EXE: every change is a faster way to compute the same values (each
one's argument below), proved by the perf harness's gates (`docs/developing/perf-gates.md`) step by
step. Results, gates and profiles: internal notes.

## Method

* Profiles: `perf.sh profile` (samply, 900 Hz, 3000 frames) of racing /
  explore / race start Original and racing Ultra, before and after; the
  callee trees from `scripts/perf/calltree.py` (the inclusive share of
  each function under `Game::draw` on the main thread).
* Six steps, each a source tree = the base + the steps before it + its own
  hunks (cut from the final diff, `git apply`; the last equals this copy).
  Each step's full-release binary against the step before:
  `perf.sh parity` (Original images at 1280×960 and 4K, the 144 Hz window
  path, Modern/Ultra, 15 shots, the explore/race WAVs, the state hash over
  firstrace, firstdrag, postrace, the playthrough and the circuit_s1
  scenario) plus the five front-end shots that run on the wall clock,
  pinned with `NFSU2_SHOT_CLOCK=10` (new, below); and `perf.sh bench`
  (`SET=quick`, `REPS=10`, arms interleaved) with `perf.sh stats`.
* Step 0 (`NFSU2_SHOT_CLOCK`, a measuring aid): base vs base+aid, shots and
  WAVs identical without the variable; with it main, career, options,
  worldmap and ending repeat byte for byte (two runs of one binary).

## The changes

### 1. A first-frame race in the base: the post passes' resources (`render/postfx.rs`)
Found by the gates, not the profile. In about 1 run in 40 the base binary
hung before its first frame, and in a few per cent drew the first frame
wrong — without its scene (only the glows and the UI) or uniformly dimmed
(max ±24) — the later frames carrying it on through the feedback passes
(±1). Evidence in internal notes.
* **Deadlock** (stacks of a hung run, a debug build `eu-stack` may attach
  to): the main thread in `PostFx::apply` → `Queue::write_texture` holds the
  gain map's init-status lock and waits for the device's trackers; a render
  thread's `Queue::submit` of the scene holds the trackers and waits for
  that init-status lock (wgpu-core 30 takes them in opposite orders). The
  scene's command buffers hold the gain map through the post bind groups
  (the motion blur pass).
* **Wrong frame**: `submit` zero-fills the not-yet-initialised ranges of the
  buffers and textures its command buffers use, and takes the pending
  writes only after that, so a `write_buffer` / `write_texture` from the
  main thread landing in between runs first and is wiped. The post passes'
  uniform buffer (bound by the motion blur in the scene's submission,
  written slot by slot by `apply` on the main thread) and the gain map were
  both first written on the first drawn frame.
* Fix: both made initialised at creation — the uniform and bounds buffers
  mapped at creation (zeros), the gain map written once (`apply` writes the
  frame's map over it before the bleach pass reads it). The values the
  passes read are the same as in the base's good runs.
* Proof, 4K Original / 1080p Modern / 1080p Ultra explore, first frame:
  the step's binary 0 of 200 / 0 of 200 / 0 of 200 wrong and no hang,
  where the base gave 1 hang in 40 and wrong frames in ~2–3 % (the gain map
  alone fixed the deadlock but left 5 of 200 / 2 of 100 dimmed; clearing
  every post target at creation did not help, the uniform buffer did).

A second run-to-run variation the gates hit (one pixel of the rain scene,
±3, in ~2 % of frames, base too) turned out to be the driver's: Mesa RADV's
delta colour compression on the multisampled scene (never with
`RADV_DEBUG=nodcc`, nor without MSAA, whatever the game's threads or
uploads do). The gates now run with DCC off (`docs/developing/perf-gates.md`).

### 2. HUD and UI (`frontend`, `render/ui.rs`)
* `fxhash.rs`: the Fx hash (rustc-hash 1.x, in-crate, no new dependency)
  for maps keyed by the game's own ids: `ScreenState`'s maps (looked up per
  object per frame by `layout`/`place`/`attributes`), `Assets::textures`,
  `in_game`, `packages`, the HUD's element and group maps, the font's
  glyphs, the UI's bind groups. Order: std's `RandomState` already visits a
  map in another order every run, so output that is identical run to run
  (the gates) cannot depend on it; none of these maps is iterated on an
  output path (checked: `hud.rs`'s `values_mut` only shifts start times;
  `Assets::fonts`, which *is* searched in order by the F3 overlay, stays
  std).
* `fe_hash` / `upper_hash`: the name hashes without the upper-case copy
  (same bytes folded; `upper_hash` falls back to Unicode upper case for a
  non-ASCII name, as `to_uppercase` did). Unit test against the copying forms.
* `layout`: one quad buffer with a range per queued object instead of a
  `Vec` per object (same sort, same order out); a hidden object's place
  (and everything under it) without its matrices when the mouse is not
  recording (only `hidden` and `root` of such an object are ever read);
  the package's guid index cached with its tree index; track values
  sampled into a stack buffer (`feng::sample_into`).
* `run_package`: per-object histories inline for the first three entries
  (`History`, read as a slice; was one allocation per object per frame).
* `Hud::quads`: the mask-driven hidden set (elements × features, the
  constructor's hide pass) cached per (low mask, high mask, drift race)
  for the package; minimap / popup script hashes out of the inner loops.
* `UiRenderer::record`: a quad's texture looked up once (was up to five
  hash lookups); the UI's encoder finished (wgpu's `finish`, the encoding)
  on a render thread like the post passes (`FrameEnd::start_ui`: the same
  command buffers in the same submission order).

### 3. Flare pools and the drawn sections (`game.rs`, `world.rs`)
* The drawn-section list (`World::drawn_sections_at`, with Keep All Loaded
  the look-ahead ring) was computed four times a frame at the same eye
  (flare pools, reflection flares, the views, the glow sprites): once now.
* `fill_flare_pools`: the light placements of the drawn sections from a
  per-section index (`World::light_placements_in`, ascending as the full
  list), instead of every light placement of the world tested with a
  linear `contains` on the list. Asset test: the same list for every
  native visible list of Bayview, a doubled one and a 400-section one.

### 4. Cars and props (`render/lighting.rs`, `render/mod.rs`, `game.rs`)
* `CarLighting::world_lights`: the camera section (`section_at`, a polygon
  scan) and its light packs once per frame, then `WorldLights::car` per
  car (was all of it per car).
* `WorldRenderer::move_placements`: a prop moved to the bit-identical
  transform it was last written with is skipped (its slots, cull values,
  spheres and moved marks already hold exactly that); `update_placement`
  and `reset_placements` drop the record.
* The shadow/neon texture hashes continued from the car name's hash
  (`bin_hash_from`) instead of a `format!` per car per frame.
* Debug switches read on frame paths (`NFSU2_FOG_DEBUG`,
  `NFSU2_HDR_PATTERN`, `NFSU2_LAMPS_LOG`, `NFSU2_SSR_*`,
  `NFSU2_HYDRAULICS`, `NFSU2_SIGHT_DEBUG`) read
  once (`env_set!` / `env_parsed!`): a `getenv` takes the environment lock
  and scans it.

### 5. Ultra planning (`render/lamps.rs`, `render/mod.rs`)
* `lamps::build` (Dynamic Lamps): the clusters' index lists by counting
  (sizes, offsets with the same cut at `MAX_INDICES`, then filled in lamp
  order) instead of 3456 growing `Vec`s a frame. Test against the old
  binning kept as the reference, up to a full index list.
* `update_fades` (LOD Fade): `trunc` without the libm call (baseline
  x86-64 has no rounding instruction): `x as i32 as f32` below 2^23, else
  `x`; equal for every finite value and NaN (test over 200 000 random
  bit patterns).

### 6. Lazy attributes in `layout`
An object's attributes (its script history replayed) are worked out when
its place is, so hidden objects and those under them are never replayed;
the same function (`object_attributes`, which `attributes` still maps).

## Results

reference desktop GPU, full release binaries, `perf.sh bench` (600 frames, arms
interleaved), machine load 15–190 from other jobs. Metric: draw CPU
p10 (ms of main-thread CPU in `Game::draw`, the least contended tenth of
the frames), median of 10 runs per arm; `perfstat` verdicts
(Mann-Whitney, p < 0.05 and beyond 2 × noise).

**Each step against the one before** (`SET=quick`; `stepN/compare.md`,
`stepN/parts.txt`):

| step | explore Orig | racing Orig | race start Orig | racing Orig 4K | racing Ultra | what moved (bench CPU parts, median ms/frame) |
|---|---|---|---|---|---|---|
| 1 race fix | same | same (a p99 flag; re-run 80/80 same) | same | same | same | — (one-time) |
| 2 HUD / UI | 0.473→0.388 (−18 %) ✔ | 0.605→0.514 (−15 %) ✔ | same | 0.739→0.590 (−20 %) ✔ | −6 % n.s. | HUD 0.111→0.061, UI 0.059→0.019 (racing) |
| 3 flare pools | 0.375→0.359 (−4 %) ✔ | same | same | same (a first-frame flag; re-run 80/80 same) | 1.151→1.075 (−7 %) ✔ | flare pools 0.030→0.010 (racing), 0.025→0.006 (explore) |
| 4 cars / props | same | same | same | same | same | car lighting 0.025→0.014 (racing), 0.038→0.022 (Ultra) |
| 5 Ultra planning | same | same | same | same | same | Dynamic Lamps 0.133→0.062 (Ultra) |
| 6 lazy attributes | same | same | same | same | same | HUD −5…−31 % (noisy) |

**Total, base → final** (`SET=standard`, 19 configs: 304 comparisons,
14 improvements, **0 regressions**; `total/compare.md`):

| scene (1080p unless noted) | Original | Modern | Ultra |
|---|---|---|---|
| explore | 0.484→0.363 (−25 %) ✔; 4K 0.555→0.427 (−23 %) ✔ | 0.738→0.617 (−16 %) | 0.870→0.693 (−20 %); 4K 0.943→0.733 (−22 %) ✔ |
| racing | 0.644→0.542 (−16 %, p 0.000); 4K 0.726→0.577 (−20 %) ✔ | 1.225→1.027 (−16 %) | 1.622→1.151 (−29 %, p 0.011); 4K 1.492→1.215 (−19 %, p 0.000) |
| drive | 0.664→0.541 (−19 %) ✔ | 0.971→0.893 (−8 %) | 1.232→1.042 (−15 %) ✔ |
| rain | 0.658→0.529 (−20 %) ✔ | 0.972→0.831 (−14 %) ✔ | 1.367→1.032 (−24 %) ✔ |
| race start | 0.670→0.694 (same) | 0.846→0.947 (same) | 1.241→1.122 (same) |

✔ = flagged improvement; the others shown without it are within the
noise rule (several at p ≤ 0.01). The race start is dominated by the
crowd's skinning. Main-thread CPU per frame (all of it, means) moved
by the same amounts within its larger noise (explore Original
0.779→0.502 ms, −36 %, flagged).

## Tried and dropped: the device poll on a thread of its own

wgpu-core 30 runs a non-blocking `maintain` inside every `queue.submit`;
the main thread's `device.poll(Poll)` in `WindowState::settle` pays the
rest (the finished frames' encoder teardown, map callbacks, deferred
destruction; 4–9 % of the main thread in the performance research's profiles). A "gpu maintain"
thread doing that poll, asked from `settle`, failed the enhanced gate with
a first frame missing its scene — the symptom of the base's race (change
1), found only afterwards. The step was dropped before that was known and
is not part of this work; with change 1 in place it can be tried again
(open).

## Open

* `queue.write_buffer` makes a staging buffer per call (wgpu 30); the
  world's per-frame writes (instances, frames, lamp clusters, fades) could
  go through the staging belt or wgpu 31's coalescing (playbook §4.2.4).
* Car Shadow Ultra (`Shadow::refine`, ~169 ground queries per car) and the
  pool reflections' mirror probes (a fresh ground query per pooled flare
  per frame) are native-port ground queries; caching them needs an
  invalidation on barrier switches (the ground reads them) — not done.
* LOD Fade's per-instance pass (`update_fades`) is O(instances in the drawn
  sections) per frame; an incremental form was not attempted.
* The HUD still evaluates its package every frame (~0.06–0.08 ms); a whole
  layout cache cannot hit in game (the needle, speed and timers change
  every frame) — the menus, which could, are not in the bench set.
* The device poll off the main thread (above): to try again on top of
  change 1, with the gates.
* wgpu-core 30's two hazards (above) remain for any buffer or texture whose
  first write comes from one thread while another submits command
  buffers that use it. The post passes' were the ones the gates hit; the
  enhancements' own lazily made buffers (SSAO, SSR, volumetric, lamps,
  temporal) were not audited one by one (the Modern / Ultra runs above
  showed no wrong frame). A general rule (create GPU resources
  initialised) or wgpu 31 would close it.
* The race-start crowd and the render workers' encoding are other
  units'.

## The port onto main (2026-11-05)

This page's six steps re-applied one by one on main (the temporal tier … the Animated Water work, the parity passes,
the test isolation since this work's base), each a 3-way merge of this work's step trees
(step tree k-1 → k) into the copy, the
conflicts resolved by hand, the measuring aid (`NFSU2_SHOT_CLOCK`) first
as step 0. Each step's full-release binary gated against the one before
(`perf.sh parity` with all gates + the five front-end shots on
`NFSU2_SHOT_CLOCK=10`), plus a gate the perf harness's presets do not reach (below).
Results: internal notes.

Conflicts and how they were kept:
* `render/ui.rs` (step 2): The modern-extras work's HUD palette — the quad loop keeps its
  index (`hud_palette_of(qi)`) and this work's single texture lookup; the
  palette pipelines are chosen from the looked-up blend
  (`blend + PALETTE_PIPELINES`), the sharp-sampling test still on the
  plain blend. Checked: racing with `enh.hud_palette 2` byte-identical
  across the steps, and different from palette 0.
* `world.rs`, `game.rs` (step 3): `drawn_sections_at` once per frame as
  in the main-thread pass; the assets-and-distance work's far sections (`far_sections_at`) now taken from that list.
* `render/lighting.rs`, `game.rs` (step 4): The Lighting II work's light probes moved into
  `WorldLights::car` (per car, as `in_world` did); the photo mode's
  hidden-car test kept.
* Steps 5 and 6 merged clean (the Lighting II work's beams are written after the clusters'
  lights, not binned).

Step 1 extended (the first-write hazard, change 1 above, for every
buffer added since and the enhancements' lazily made ones the main-thread pass left open):
* `render::zeroed_buffer` (mapped at creation, unmapped at once) for the
  uniform / storage buffers the main thread writes with `write_buffer`:
  SMAA / enhance passes, blur & DOF (uniforms, cars), FSR, GPU particles
  (smoke pool, births, ticks, descriptions, colours, frame), weather fx
  (frame, spray), photo grade, lens drops, SSR, SSAO, volumetric
  (uniforms, lamps), temporal (uniforms, cars), HDR output, water frame,
  light probe frames, lamps (lights, clusters, views, occluders, shadow
  frames), the post passes' horizon fog vertices, grade LUT / image and
  check uniforms. Read-back (`MAP_READ`) buffers and textures written whole
  at creation (water, smoke, rain drop, grade identity) needed nothing.
* Surface Detail's normal-map array (`render/surface.rs`): `pump` uploads
  its layers from the main thread over many frames while the world views
  that sample it are submitted from render threads — the same hazard on a
  texture. Zero-written whole at creation (in batches of 64 layers per
  mip), before any bind group holds it.
* Proof on main: 4K Original explore, first frame, 120 runs per binary
  (DCC off): main+step 0 1 wrong frame (99.9 % of pixels, the scene
  missing — this work's symptom), step 1 0 of 120; Extreme preset (below)
  explore and rain, frames 0–3, 60 runs each: identical for both.
* Two more frame-path `getenv`s cached (step 4's rule): `NFSU2_WATER_PLAIN`
  (water, per frame), `NFSU2_WEATHER_DEBUG` (weather fx, per frame).

Extra gate (`perf.sh`: `ENH_PRESETS="extreme extreme_fsr"`, `ENH_SCENES`):
The perf harness's Modern / Ultra presets predate most enhancements, so the gates did not
run them. `EXTREME` = Ultra + particles, water, wet car, rain detail,
spray, lens drops, per-object motion blur, DOF, beams, light probes,
enhanced glass, Surface Detail (materials + normals), smooth cars, World
Detail Extreme; `EXTREME_FSR` the same with FSR 1 (Balanced) instead of
TAA. Racing / explore / rain at 1080p, every step against the one before:
AE 0 (and one binary against itself: AE 0, so the gate is usable).

### Results

Gates: every step against the one before passed everything (Original
images 1280×960 + 4K, window path, Modern / Ultra, 15 shots, five
front-end shots on `NFSU2_SHOT_CLOCK=10`, WAVs, state hashes over
firstrace 4916 / firstdrag 15559 / postrace 30779 / playthrough 82412 /
parity 1753 steps) and the Extreme / Extreme FSR gate (racing, explore,
rain); internal notes.

Total, main → final (`perf.sh bench`, `SET=standard`, `REPS=10`, arms
interleaved, load 7–44; 304 comparisons: **35 improvements, 0
regressions**, 3 not significant on wall-clock load/first-frame
metrics; internal notes). Draw CPU p10, ms,
median of 10 runs (✔ = flagged improvement):

| scene (1080p unless noted) | Original | Modern | Ultra |
|---|---|---|---|
| explore | 0.487→0.392 (−20 %) ✔; 4K 0.529→0.410 (−22 %) ✔ | 0.682→0.540 (−21 %) ✔ | 0.846→0.683 (−19 %) ✔; 4K 0.905→0.738 (−18 %) ✔ |
| racing | 0.612→0.503 (−18 %) ✔; 4K 0.691→0.571 (−17 %) ✔ | 0.975→0.845 (−13 %) ✔ | 1.260→1.035 (−18 %) ✔; 4K 1.326→1.089 (−18 %) ✔ |
| drive | 0.612→0.489 (−20 %) ✔ | 0.923→0.772 (−16 %) ✔ | 1.176→0.972 (−17 %) ✔ |
| rain | 0.649→0.523 (−19 %) ✔ | 0.955→0.796 (−17 %) ✔ | 1.225→0.995 (−19 %) ✔ |
| race start | 0.608→0.580 (−5 %) ✔ | 0.814→0.761 (−6 %) ✔ | 1.093→0.990 (−9 %, p 0.09) |

Main-thread CPU (means): rain Original 0.852→0.719 (−16 %) ✔, rain
Modern 1.214→1.090 (−10 %) ✔, the rest within the means' noise.

The main-thread port open: the Extreme preset is not in the bench sets (only gated); other
frame-path `getenv`s in `game.rs` added since this work's base were not audited
one by one (the two found in the render modules are cached).

## CPU frame time (2026-11-05)

The CPU frame pass: less CPU per frame, the
same frames, sound and simulation. Nothing here was traced in SPEED2.EXE
and no native behaviour changed: each step computes the same values
faster, or elsewhere. Base: main . Results, gates, profiles:
internal notes.

### Profile first (step 1 of the plan)
`perf.sh profile` (samply, 900 Hz, 3000 frames, 1080p; full-release
codegen + line tables) of the race start, racing, explore and rain at
Original, race start / racing at Ultra, race start / racing / explore at
the Extreme preset (the main-thread port's: Ultra + every newer enhancement). Every scene's
main-thread and render-worker hotspots with their shares:
internal notes. Thread CPU per frame (bench JSON
of the same runs): main 0.81–0.91 ms (Original), 1.42–1.89 ms
(Ultra/Extreme); the four render workers together 1.33–1.90 / 1.87–3.14 ms
— 60–64 % of the process's CPU, ≈55–70 % of it wgpu-core's render-pass
encoding (`encode_render_pass`, binder `assign_group`,
`register_init_action`; the GPU encoding pass's).

Found, by share of the main thread (base):
| hotspot | share | step |
|---|---|---|
| `Game::preload_frame` → `player_car` → `with_derived_slots` / `ride_slot_parts` → `PartDb::find` (a part search per slot, every frame, in every mode) | 8.8–10.6 % (Original), 3.5–5.9 % (Ultra/Extreme) | 3 |
| the device's upkeep (`Device::maintain`: the finished frames' command encoders torn down — RADV's memset —, map callbacks) in the bench's throttle poll / `settle`'s poll | 7–10 % | tried, dropped (step 5) |
| Extreme: `WorldRenderer::update_fades` (LOD Fade, per drawn instance) | 11.7 % self, 15.7 % incl. | open |
| Extreme: `World::lookahead_sections` from `far_sections_at` (the outlines scanned every frame for a set fixed per section) and `drawn_sections_at` | 2.4 % + 2.0 % | 4 (the far one) |
| race start: the crowd's skinning (`CharacterRenderer::render`) | 0.08–0.10 ms/frame over the bench run, ≤ 0.84–1.18 ms in a start-scene frame; 4.2 % of the Extreme race profile | 1 |
| Ultra/Extreme: `Shadow::refine` (Car Shadow Ultra ground queries) | 3.0–6.2 % | open |
| per-frame `getenv`s (counted with an `LD_PRELOAD` shim, `scripts/perf/getenvcount.c`): ~20 a frame (`NFSU2_DEBUG_ACTIONS` ×6, `NFSU2_AUDIO_DEBUG` ×3, `NFSU2_DEV_SHOP` ×3, `NFSU2_PREVIEW_LOG` ×2–3, `NFSU2_NIS_PROBE`, `NFSU2_FLASH_PROBE`, `NFSU2_PRESENT`, `NFSU2_MSAA`, the bench's own six) | each a lock + an environment scan | 2 |

### The steps
1. **Crowd skinning on a render thread** (`render/characters.rs`,
   `render/mod.rs` `Pending<T>`, `game.rs`). The race start scene's
   characters are posed and skinned by a job on the renderer's own worker
   threads (not rayon), started right after the camera's jitter is set
   (their frame, place and the camera final), while the main thread
   records the world and the cars; `draw_characters!` waits for it,
   uploads and records as before. The job runs the very function the
   main thread ran (`skin_frame`, factored out unchanged) on the same
   inputs; `render` uses its result only when the inputs it was started on
   are bit-identical to the ones it is called with (view-projection, time,
   origin), else skins in place. The first frame of a scene (when the
   renderer is made, adding the world's texture slots) skins in place as
   before. `SceneCrowd::set` is an `Arc` shared with the job. Test:
   `skinning_on_a_render_thread_matches_in_place` (asset test: every
   IntroNis scene, 40 frames, culled and not; the job's corners and runs
   byte-identical to in-place skinning, scratch reused).
2. **Frame-path environment reads** (`lib.rs` `env_value!` /
   `env_value_os!`, `game.rs`, `audio/{fe,streams,roadwind}.rs`,
   `render/display.rs`): the switches above read once per call site
   (this work's rule; none is set at run time — `NFSU2_ENDING_NIS` and
   `NFSU2_CACHE_DIR`, which are, stay live). Also the ray-traced reflections work check
   (plan item 4): `rt::frame` runs on the main thread after the last
   frame's end and before this frame's scene submission, so its
   `write_buffer`s cannot meet a render thread's `submit` (this work's hazard);
   its colour and uniform buffers are made initialised anyway
   (`zeroed_buffer`, the main-thread port rule) since the cars' colours land in the
   colour buffer's tail later. The TLAS rebuild (one small encoder and a
   submit per frame, on the main thread) was not measurable here: the
   presets measured do not enable Ray Traced reflections.
3. **The player's car only where preloading needs it** (`game.rs`
   `preload_frame`): `player_car()` (the ride rebuilt from the record:
   `with_derived_slots`, a `PartDb::find` per slot) ran every frame for
   `kept`, which (with the car it picks) is read only on the screens that
   go on to free roam. Now worked out only there (`to_free_roam`); driving
   and racing skip it. The same values wherever they are read.
4. **World Detail Extreme's far set per section** (`world.rs`,
   `render/mod.rs`): `far_sections_at` measures from the eye section's
   outline centre at a fixed ring, so its look-ahead set depends on the
   section alone; it is kept for the last section
   (`LookaheadCache::far`) instead of scanning every drivable outline each
   frame (a section without an outline, measured from the eye, is not
   kept). Asset test `kept_far_sets_are_the_fresh_ones` (a walk over
   Bayview's outlines, leaving and re-entering sections). And the far
   sections' LOD-fade size uses `trunc_fast` like the main ones (this work's
   proof: equal for every finite value).
Tried and dropped (step 5, gated and measured, not kept): **the device's
upkeep on the render thread that ends the frame** — after the frame's
submit and present, the `FrameEnd` job did a non-blocking
`device.poll(Poll)`, so the finished frames' encoder teardown would run
there instead of in the main thread's poll. All gates passed (AE 0, the
Extreme gate, hashes identical), but it moved nothing: 0 of 96
comparisons flagged, main-thread CPU ±1 % in every config
(internal notes). With two frames in flight
the GPU has not finished any earlier frame at the moment the job polls;
the teardown is still found by the main thread's later poll (the bench's
throttle `poll(Wait)`, `settle`'s poll in the window path). Moving it off
the main thread needs a poll that waits for those frames, i.e. a thread
of its own blocking in `poll(Wait)`; wgpu-core 30 holds the device's
snatch lock (read) for that whole wait, so a main-thread destroy (a
write lock) would stall behind the GPU (read in wgpu-core 30.0.1's `Device::maintain`; inferred, not measured) — not done (open, the GPU encoding pass / wgpu 31).

### Gates
Each step's full-release binary (`perf.sh build`; md5 of every pair
differs, listed in internal notes) against the step
before: `perf.sh parity` with all gates (Original images 1280×960 + 4K,
144 Hz window path, Modern / Ultra, 15 shots, explore / race WAVs, state
hashes over firstrace 4916 / firstdrag 15559 / postrace 30779 /
playthrough 82412 / circuit_s1 1753 steps), the Extreme / Extreme FSR
gate on racing, explore, rain **and the race start** (the crowd), and the
five front-end shots on `NFSU2_SHOT_CLOCK=10`: every step "all gates
passed", AE 0 everywhere, hashes identical (step 5 too, before it was
dropped).

### Results
reference desktop GPU, full-release binaries, `perf.sh bench` (600 frames, 1080p
unless noted, arms interleaved), machine load 3–17.

**Steps** (`SET=d3`: race start / racing / explore × Original / Extreme;
the six binaries s0…s5 interleaved, 10 runs each; `stepN/compare.md`).
Main-thread CPU ms/frame (means, median of 10 runs):

| step | race start Orig | race start Extreme | racing Orig | racing Extreme | explore Orig | explore Extreme |
|---|---|---|---|---|---|---|
| 1 crowd on a render thread | 0.716→0.491 (−31 %) ✔ | 1.606→1.285 (−20 %) ✔ | 0.696→0.606 (−13 %) ✔ | 1.696→1.596 (−6 %) ✔ | 0.459→0.437 (−5 %) ✔ | same |
| 2 environment reads | same | same | same | same (draw p10 −2.6 % ✔) | same | same |
| 3 player's car where needed | 0.486→0.451 (−7 %) ✔ | same (draw p10 −3.3 % ✔) | 0.614→0.572 (−7 %) ✔ | same (update −30 % ✔) | 0.437→0.392 (−10 %) ✔ | same |
| 4 far set per section | same | same | same | draw p10 1.242→1.168 (−6 %) ✔ | same | same |
| 5 poll at the frame's end (dropped) | same | same | same | same | same | same |

The race benches (race, racing, drive, rain) all begin at the start line,
where the start scene's crowd plays: step 1 moves its skinning to the
render workers (their CPU +0.06…0.30 ms/frame, flagged as a "regression"
of `cpu_render`; process CPU unchanged, −1…+1 %). The explore gain of
step 1 (−5 %, no crowd there) is not explained by the change (a code
layout effect is the guess; inferred).

**Total, main → step 4** (`SET=standard`, 19 configs, 6 runs per arm,
`total-standard/compare.md`; 304 comparisons: 108 improvements, 8
"regressions" — all `cpu_render` in race-start-led scenes, the moved
skinning —, 10 not significant). Main-thread CPU ms/frame (means):

| scene | Original | Modern | Ultra |
|---|---|---|---|
| race start | 0.741→0.459 (−38 %) ✔ | 1.059→0.756 (−29 %) ✔ | 1.346→0.979 (−27 %) ✔ |
| racing | 0.718→0.587 (−18 %) ✔; 4K 0.863→0.725 (−16 %) ✔ | 1.135→1.013 (−11 %) ✔ | 1.445→1.275 (−12 %) ✔; 4K 1.518→1.377 (−9 %) ✔ |
| drive | 0.691→0.569 (−18 %) ✔ | 1.067→0.935 (−12 %) ✔ | 1.343→1.203 (−10 %) ✔ |
| rain | 0.715→0.591 (−17 %) ✔ | 1.103→1.003 (−9 %) ✔ | 1.381→1.237 (−10 %) ✔ |
| explore | 0.471→0.409 (−13 %) ✔; 4K 0.573→0.498 (−13 %) ✔ | 0.661→0.605 (−9 %) ✔ | 0.877→0.796 (−9 %) ✔; 4K 0.931→0.857 (−8 %) ✔ |

Draw CPU p10 (this work's metric): race start Original 0.586→0.357 (−39 %),
Modern 0.763→0.521 (−32 %), Ultra 1.020→0.730 (−28 %); racing Original
0.505→0.464 (−8 %); explore Original 0.366→0.347 (−5 %). Process CPU
never went up (−0…−5 %; flagged −3…−4 % in explore / racing Ultra and
racing Original 4K). GPU time unchanged.

### What is left (final profiles, internal notes)
Main thread, racing Original: HUD (`Hud::quads` → `frontend::layout`)
17.7 %, simulation (`step_race`) 27 %, the GPU timestamps' read-back
(`prof::gpu::end_frame`, a bench aid) 6.4 %, slot writes 3.4 %, cars'
uniforms 3.0 %, `CarBatch::upload`'s copy 2.9 %, reflection glows
2.8 %, car shadows 2.5 %, UI record 2.3 % — many items above 1 %.
Extreme: `update_fades` 16–17 % self, `lamps::build` 3–5 %,
`TrackGround::query` / `choose_pack_with` 5–6 % (Car Shadow Ultra's
`Shadow::refine` ground queries), memcpy 7 % (car uniforms, slot
writes). Render workers: wgpu-core's encoding as in the performance research (the GPU encoding pass).

### Open (with reasons)
* `update_fades` (LOD Fade, per drawn instance per frame; 16 % of the
  Extreme main thread): exact parallel or incremental forms need the
  per-slot fade state split by section and the changed list merged in
  slot order; not attempted.
* Car Shadow Ultra's ground queries (`Shadow::refine`, 3–6 %): a cache
  needs an invalidation on barrier switches (this work's open item, unchanged).
* The device's upkeep off the main thread (step 5 above): needs a
  blocking poll on a thread of its own (lock behaviour, the GPU encoding pass / wgpu 31).
* Per-car simulation in parallel (plan item 2): `step_race` is 10–27 %
  of the main thread but the cars meet within a step (contacts, traffic
  and rival AI read each other's state), so an order-preserving parallel
  form is not a local change; with no exact split found it was not done
  and no 1-vs-N-thread hash test was written for it (the crowd's
  1-vs-N test is `skinning_on_a_render_thread_matches_in_place`).
* Load-time items (plan item 2), measured with `NFSU2_TIMING=1 --bench
  transitions` (load 3–8): the first world's `WorldRenderer::new: groups
  and draws` 78 ms wall for 18 ms thread CPU, `texture arrays` 49 ms
  wall / 3.5 ms CPU (wait-bound, as the performance research found); `Encounters::new` 20.6 ms
  (18.5 ms CPU) the first time, 4–5 ms after; `Encounters: cars joined`
  53–74 ms wall for < 1 ms CPU (waiting for the car loads). Not CPU-frame
  work: left to the loading unit.
* Render bundles: not tried (the culled lists change most frames).
* Staging belt: present already (`render::upload`, `w.belt`); the
  remaining `queue.write_buffer`s (slot writes, fades: 2–3 %) are open.
* The ray-traced reflections work TLAS rebuild cost: Ray Traced reflections are in no measured
  preset; not measured.

## GPU frame time and render encoding (2026-11-05)

In `docs/notes/performance/gpu.md` (results internal notes): the per-pass
GPU profile of explore / racing / rain / the race start at Original,
Ultra and Extreme, 1080p and 4K; the world's texture arrays bound as one
binding array (the render workers' encoding: a bind-group switch on 81 %
of the world draws before); LOD Fade's walk over pre-listed groups and
slices (`update_fades`, the CPU frame pass's open item); the pipeline cache for the last
five pipelines without it. Every step AE 0 and state-hash identical.

## Audit round 2 (2026-11-05)

The fresh CPU profile (internal notes,
`samply-s0-racing-extreme.txt`) and the GPU steps are in
`docs/notes/performance/gpu.md` "Audit round 2". No main-thread step was made in this work; the
CPU side is not the frame's limit in any measured config (main thread
0.4–0.8 ms Original, 0.8–2.4 ms Extreme against GPU 0.6–21 ms), and the
items ≥ 1 % left have no exact form found in this run:

* HUD (8–12 % of the main thread at Original, `Hud::quads` →
  `frontend::layout`): the package is evaluated every frame because the
  needle, speed and timers change every frame; unchanged.
* Simulation (`step_race`, 10–27 %): the cars meet within a step
  (contacts, traffic and rival AI read each other's state); no
  order-preserving split found, so no per-car parallel form and no
  1-vs-N-thread hash test (as the CPU frame pass).
* Car Shadow Ultra / pooled reflection flares' ground queries
  (`TrackGround::choose_pack_with` 3.1 % + `query` 2.5 % at Extreme; the
  flares' share comes from a fresh `GroundQuery` per flare per frame,
  `game::reflection_glows`): a fresh query's answer is a function of the
  point and the global barrier switches only, so a per-frame cache keyed
  on the point (bits) and invalidated on any barrier switch would be
  exact; the shadow's persistent queries carry pack / recent-floor state
  (the GPU encoding pass's note) and need that state in the key. Not done (open; ≈ 0.02–
  0.04 ms a frame at Extreme).
* LOD Fade (`fade_slots` 9 % self at Extreme): The GPU encoding pass's form; an incremental
  form is still open.
