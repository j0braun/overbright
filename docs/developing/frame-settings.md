# Settings per frame and per tick

Structure only: the program reads the same values as before; what changed is
where and how often it reads them. The one deliberate exception is the frame
of a settings change: the draw of tick-stepped state uses the value that
state was stepped with (see "A tick's settings and a frame's").

## The two values

* **`render::FrameSettings`** (`crates/ob-render/src/render/native/frame_settings.rs`): what a
  drawn frame reads. `display: Display` and `switches: Switches` (from the
  Customize Display values, each computed once; the window's frame computed
  each of them up to three times before), and `opts: RenderOptions`, the
  Enhancements rows in force the draw reads (anti-aliasing / scale / temporal
  pass, Reversed-Z, HDR, world detail, reflections, shadows, lights and
  materials, weather and effects, the Graphics page's filters, the section
  distances; 72 fields, a `Copy` value: `RenderOptions` 88 bytes,
  `FrameSettings` with `Display` and `Switches` 104 bytes; 72 and 88 before
  the post chain's and the Enhanced effects' rows).
  `RenderOptions::in_force()` (`crates/ob-settings/src/qol/frame.rs`) copies them from the
  settings store with the store's own getters, so each field is exactly the
  value its getter returns (a ray-traced tier only while the device traces,
  HDR only where supported, ...). Derived values are methods
  (`msaa_override`, `road_reflection_size`, `scene_size`, `output_size_or`,
  `rt_reflections`, `supported_msaa`).
* **`sim::TickSettings`** (`src/bin/game/sim/tick_settings.rs`): what a tick
  reads after the world update: the switches, Particles, Sparks & Fire,
  Motion Blur's tier, the weather rows, Wet Road Interaction, LOD Fade and the
  pop-in fixes, the rows the Enhanced effects' tick steps with (Rain Detail,
  Spray & Splashes, Lens Drops, Ambient Detail, Sky), Keep All Loaded and
  World Detail Extreme (the tick views' sections) and the chase camera's
  Field Of View offset (28 bytes, `Copy`; `TickInput` 112 bytes). Taken once
  per tick in `Game::update_world`, right after all of the tick's input is
  applied: the key queue (`apply_input_queue`), then the pads and the mouse
  (`tick_input`). An option changed by a key, a pad button or a mouse click
  is in force in that same tick, whichever input changed it
  (`tests::settings_moments::an_option_changed_by_any_input_is_in_force_in_its_tick`), kept in `Pacing::tick_settings`, handed to the simulation in
  `TickInput::settings` and captured in the tick's snapshot
  (`TickSnapshot::settings`).

## Where they are built

* The window's frame: `FrameDraw::frame_targets`, right after the temporal
  pass is decided (`qol::set_temporal_frame`), the point where the frame's
  settings are final: the display mode, the HDR switch (`sync_hdr`), the pack
  reload and the temporal decision are made. It travels in
  `FrameTargets::settings`, `FrameInputs::settings`, `FrameParams::opts` and
  the renderers' parameters. The renderers made on a frame before that point
  (a freshly loaded region's, `WorldRenderer::new` / `CarRenderer::new`) take
  their own `in_force()` copy at that moment, as they read the globals then.
* A shot (`dev/shot`): `render_offscreen` takes one copy at its start; its
  world's display and switches are the `NFS_DETAIL` presets, as before.
* Loads: `drive_in` takes one copy on the main thread before the warm-up
  worker starts and moves it in (see below); the other loads and renderer
  builds take a copy at the call.

## Readers converted

* Render core / native (the layering report's "QoL settings read"): the world
  renderer (plan, pipelines / `SceneMode::current` / `driving`, `set_mode`,
  cull sizes, environment cube, shadows' neon tier, the build's sampler and
  cube size), the car renderer (modes, wet state, uniforms, surface maps, SSR
  mask pipelines, LOD fade, lighting options), `display::msaa_samples`,
  `texture_sampler`, the horizon fog pass. Enhanced draw halves that called
  them: lod fade, lamp shadow layers, car glass, water's pick.
* The game's draw: `render/frame/*`, `render/native/*` (car LOD, flares,
  glows, particles, rear view, shadows, cover), `render/enh/frame_inputs`,
  `render/enh/post`, the garage's draw.
* Ticks: the effects' births (`Effects::step_in_order`), the Enhanced
  effects' tick (`enh_fx_tick`), the native visuals (`visuals_tick`).
* The post chain (`render/native/post`, `render/enh/post`): its entries take
  the frame's `RenderOptions` (`PostFx::scene_view`, `multisampled`,
  `set_upscale`, `pre_passes`, `apply`, `apply_ticks`, `commit_tick`,
  `temporal_pass`, `Temporal::next_jitter` / `run`, `temporal::mip_bias_q`,
  `enhance::Plan::of`, `blur_dof::MotionMode::of`, `dof_on`, `dof_wanted`):
  the targets' High Precision Post and particle tier, HDR Output, the
  released tiers, FSR 2, the filters. The streak buffer's probes
  (`render::streak`) are scene renders of their own: each runs
  `frame_targets`, so the probe's scene and its post chain up to the commit
  read the probe's one snapshot, taken where its 60 Hz frame would be drawn
  (between two ticks of the drawn frame, after the first tick's input); the
  drawn frame after it takes its own. Within one scene render nothing reads
  the store.
* The Enhanced effects' draw halves (`render/enh`): lamps and their shadow
  layers, Modern Lighting (`modern_light::district` / `enabled`,
  `modern_light_bold::tier` / `strength` / `wants_rt` / `scene_modern`), the
  Next-Gen car lights' feed (`car_lights::publish` / `cars`), light probes
  (`bake_probes`, `probes::cube_at`), Enhanced Glass, the wet cars' uniform,
  the water (`water::View::tier` / `modern`), the rain and spray
  (`WeatherFx::prepare`, `prepare_weather_fx`, the trail map's plan), Lens
  Drops' frame list, Ambient Detail's draw (`Frame::tier`, `draw_volume`),
  the Next-Gen sky (the frame's row carried to the render threads in
  `WorldCore::sky_on`), LOD Fade's draw gates, the pop meter's output width.
  Their tick halves take the tick's: `EnhTick::opts` (`EnhTickOptions`) for
  `WorldRenderer::enh_tick` (spray, trail map, lens drops, Ambient Detail's
  step, the sky's rows), the cars' lists (`wet_road::car_extra`,
  `ambient_detail::add_car`), and `TickView::opts` (`TickOptions`) for the
  steps the tick views drive (LOD Fade's per-instance fades, the lamps'
  section fades and shadow-layer hand-out).
* Section distances: `World::drawn_sections_at` / `far_sections_at` take
  Keep All Loaded / World Detail Extreme from the caller: the frame's for
  the frame's eye (`world_lights`, the far sections), the tick's for each
  published tick view (`TickRecord::opts`, recorded at the tick's end).
* The chase camera's Field Of View offset: `ChaseCamera::update` takes it
  (an appended parameter); the drive holds the tick's
  (`Drive::fov_offset`, set as `Session::step` begins from
  `TickInput::settings`; the store's at the drive's load). The draw never
  read it: it draws the tick snapshot's camera.

## A tick's settings and a frame's

A setting the tick steps state with, and the draw reads again, could differ
for one frame when it changes between a tick and the next drawn frame (a
menu's mouse or pad input changes the store after the tick took its
settings). The draw now shows tick-stepped state with the value it was
stepped with: the newest tick's (`FramePoses::stepped` from the snapshot,
`FrameInputs::stepped` / `render::frame::Stepped`; `WeatherSim::stepped`
for the world renderer's weather), the frame's own when no tick stepped it
(shots, a still). The new value is drawn from the next tick on.

| setting | tick reads | draw reads | the draw uses |
|---|---|---|---|
| Motion Blur switch + tier | `visuals_tick` (the native speed blur steps) | the speed blur pass (`scene.rs`) | the tick's |
| Light Glow switch | `visuals_tick` (the flare pools' blink), `enh_fx_tick` (the cars' lists) | the flare pools' fill | the tick's (the glow pass and post chain: the frame's, no state) |
| Horizon Fog switch | `visuals_tick` (its visibility steps) | the horizon fog pass | the tick's |
| Particles / Sparks & Fire | `Effects::step_in_order` (the births recorded per tier) | `particle_quads` (the GPU smoke and sparks prepared) | the tick's |
| LOD Fade / pop-in fixes | `enh_fx_tick` (traffic ring), the tick views' steps | the traffic ring of the flares and car LODs, the fade gates | the frame's (computed per frame from the drawn poses; no stepped state), the steps the tick's |
| Wet Road Interaction | `enh_fx_tick`, `enh_tick` (trail commits, spray) | the spray's neon light (`glow_quads`), the spray volume, the trail map's tier | the tick's |
| Spray & Splashes | `enh_tick` (the spray pool) | `prepare_weather_fx`, `WeatherFx::prepare` | the tick's |
| Weather rows (any) | `enh_fx_tick` / `enh_tick` (the scene) | `wet_car::publish_weather`, the weather view (`fx_view`) | the tick's |
| Lens Drops | `enh_tick` (the lens sim) | `lens_frame` (the post chain's drops) | the tick's (window), the frame's (shots) |

Test: `tests::settings_moments` (free roam, the rows changed between a tick
and a draw: the frame's draw values are the tick's, the next tick's the new
ones; the chase camera keeps the tick's angle until then) and
`render::enh::weather::host::tests` (`WeatherSim::stepped_or`).

## Thread-crossing reads found (the hazard removed)

Measured with a probe on every store load that logged the thread and the
call path (benches racing / explore / race at 144 Hz, Extreme and Extreme
FSR, flows garage and menus): no render thread (`render N`) read the store
in a frame. The reads off the main thread were in two load workers:

* `Game::drive_in_loaded`'s warm-up worker (`WorldRenderer::warm_drive`,
  `CarRenderer::warm`, rayon inside `set_mode`): Post AA, Upscaling, the lamp
  rows, Reversed-Z, HDR, Modern Lighting, the temporal frame, LOD Fade, Light
  Probes, Ambient Detail, Enhanced Paint, Wet Car, MSAA. It ran while the main
  thread ticked the load and drew its loading frames, which write the temporal
  frame flag; it now gets a copy taken on the main thread before it starts.
* `Game::load_world_and_car`'s car loader (Fix Original Model Errors, Smooth
  Car Shapes, in the car build): not a frame or tick read; left (a later
  step, `AssetOptions`).

## What is left (layering rule 5's list, `scripts/checks/layering-settings-allow.txt`)

The list holds only reads that are not a frame's or a tick's (the file's
comment section gives the reason per file):

* Start-up: `core/gpu.rs` writes the device's caps into the store;
  `rt::scene::wanted` (the device's features, made once).
* Pacing: `display/present.rs` `frame_cap` (the frame loop, not a frame read).
* Loads: the car model fixes and Smooth Car Shapes (the car build), the HD UI
  pack's folder, Reload Packs When Changed (the pack watcher).
* A test-only helper: `shader::world_source` (its tests' shader text).
* Tests: the store set up and read back by the tests themselves.

Outside `crates/ob-render/src/render`, the game's draw still reads `qol::temporal()` in
`frame_targets` (the temporal decision itself, before the snapshot) and in
the streak buffer's `streak_probe_due` and probe bookkeeping (a decision
before a frame is drawn); `qol::hdr_output_active()` in the window's
swap-chain switch (`sync_hdr`).

The store holds the values in force as one value (`qol::InForce`, crates/ob-settings/src/qol/store.rs): the
effective `Enhancements` (the stored values with the engine rules applied) and the device's and the session's
facts (caps, MSAA mask, ray-tracing and HDR support, the HDR signal and display, output size, Compare, the
temporal frame, the HDR pattern). No value lives in a global atomic: every getter reads through the store, every
setter changes it one change at a time, and `Enhancements::apply_now` replaces the values in force in one change
(a reader sees the old settings or the new ones, never half of an apply). `in_force()` and `TickSettings::take()`
copy from it, and the options screen, the launcher, the saves, start-up and the loads read it. Now read by nothing
else (only the two copies, the options screen, the launcher, the saves,
start-up, the loads, the developer harness and tests): Anti-Aliasing and its
filter and MSAA rows, Render Scale's FSR / FSR 2 tiers (Upscaling), Reversed-Z,
Texture Filtering's anisotropy, High Precision Post, LOD Fade, Keep All
Loaded (and the spot marker) and World Detail Extreme, Detail Distance, Car
Detail Distance, Road Reflection Res, Car Reflection, Enhanced Reflections,
Reflection Every Frame, Shadow Quality, Screen-Space Reflections (and the
device's ray-tracing decision), SSAO, Sharpen, Dynamic Lamps, Lamp Shadows,
Contact Shadows, Headlight Beams, Light Probes, Surface Detail,
Enhanced Paint, Enhanced Glass, Scene Car Specular, Modern Lighting (and the
device's decision) and its strength, Ambient Detail, Wet Roads, Wet Car, Wet
Road Interaction, Rain Detail, Spray & Splashes, Lens Drops, Particles,
Sparks & Fire, Animated Water, Volumetric Fog, Sky, Motion Blur, Depth Of
Field, Field Of View offset.
