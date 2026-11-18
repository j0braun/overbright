# First use: programs made before the frame that needs them

The Enhanced passes used to make their shader modules and pipelines in
their own first draw: the first frame after a drive's load with the
Graphics rows on, and the first frame after a row was switched on, took
the whole build (on the reference desktop: about 100 pipelines and 28
modules, ~45 ms of building, ~390 ms first frame at the Extreme preset;
several times that on a small machine). They are now made before that
frame: at the load, behind its loading screen, and at the moment a row
changes. Nothing drawn, heard or simulated changes: the objects are the
ones the frame would make, from the same descriptors, only earlier (the
gate is byte-identical). Nothing here was traced in SPEED2.EXE; it is how
the clone schedules its own work.

## The device's cache (`render::core::device_cache`)

Every pass outside the world's and the cars' shared sets (which keep
theirs per device already) asks for its programs through the cache the
`Gpu` holds:

* shader modules by their WGSL text (a hash and the text itself),
* bind group layouts by their entries,
* pipeline layouts by their groups (the cached layouts' identities) and
  immediates size,
* render and compute pipelines by the whole descriptor: layout, modules
  and entry points, overridable constants (their values' bits), vertex
  buffers, primitive, depth-stencil, multisample, colour targets,
  multiview. The label and the driver's pipeline cache are not part of
  the key.

So a pass made a second time (switched off and on, the upscaling stage's
second post chain, the warm-ups' copies below) builds nothing, and two
threads asking for one key build it once (the second waits for it). The
cache lives as long as its device and is reached through the `Gpu`, not
looked up by the device: wgpu compares devices by their id within an
instance, and a process can hold several instances (tests side by side).
The Animated Water's GPU halves (`render::water`, `water_hq`, `water_ng`) make
theirs through it too.

## What is made where

* **A drive's load** (`drive_in_loaded` → `loads::warm::warm_passes`), for
  the settings in force (one `RenderOptions` copy taken on the main thread)
  and the scene's sample count, on scoped threads side by side while the
  main thread ticks the load's PC_Loading gate:
  * the world renderer: its scene pipelines (as before), Light Probes (as
    before), and now the SSR mask, FSR 2's animation mask, the lamps'
    shadow layers (also under Modern Lighting), Wet Road Interaction's
    trail map, the rain's and spray's passes (whatever the weather), the
    Next-Gen sky, Modern + RT's traced shadows;
  * Ambient Detail's props, a job of their own (`AmbientWarm::run`): the
    props the menus made for the world as loaded, or those a world kept
    for the next drive carries from the last one, kept when the world, its
    barriers set up for the drive, still gives the same ones (every city
    drive), else made again for it (some drag, drift and URL-circuit races),
    and their pipelines (docs/ambient-detail.md "When the props are placed");
  * the car renderer: its sets (as before), the cars in the SSR mask,
    Enhanced Glass (both wet states with Wet Car) and its scene copy;
  * the post chain (`PostFx::warm_frame`): High Precision Post's set
    (with SSAA's compute resolve and its last pass, docs/ssaa-screen-passes.md) and
    its scaled-glow prefilter (the native glow's too, for a scene drawn at
    another size: Render Scale, FSR 1), the multisampled scene's lerp and streak
    copy, the fused passes once the self-check chose them, the horizon fog
    in the scene's mode; the chain itself is made at the load if the
    menus have not made it (as the first frame would);
  * the Enhanced screen passes (`render::enh::warm::frame_passes`, a copy
    of each pass the settings run, made and dropped: its programs stay in
    the cache): SSAO (with its multisampled apply), SSR, Volumetric Fog or
    its Next-Gen medium, Next-Gen car lights, Motion Blur / Depth Of Field,
    the temporal pass (its resolve, TAA's compute resolve where the
    device runs it, after its self-check, the copy to the scene and the
    two-target resolve for the scene's format; with FSR 2, its passes and
    its output into the scene, `Temporal::warm_fsr2`), the graphics passes
    after the chain (FXAA, SMAA, Sharpen, the resample, FSR 1's EASU and
    RCAS into the window's format, `Fsr::warm`), Sparks & Fire, the volume
    effects' programs (Ambient Detail Next-Gen, the spray's volume);
  * Photo Mode with its row on: its grade and its depth of field;
  * Animated Water at its tier (`Water::build`: the world's water mesh,
    the GPU half, Realistic's and Next-Gen's halves with their charts,
    fields and compute pipelines; `Water::warm_tier`: the surfaces',
    sheets', spray's, wet stone's, scene copy's, Next-Gen basins', lips',
    sheets', drops' and mist's pipelines for the scene's targets and,
    after the temporal pass, its single-sampled ones);
  * the smoke's GPU half for the Particles row (`Detail::prebuild`,
    `Cinematic::prebuild`: the object the first frame made) and its
    pipelines for the scene's targets.
  * Wet Road Interaction's spray volume (`WorldRenderer::spray_volume_ahead`,
    a job of its own, offered to the renderer once the jobs are done; also
    beside a world loaded ahead): the object the first frame with a spray
    cloud made (~12 ms of that frame); one that has drawn nothing holds
    no state. About 17 / 38 MB of GPU memory at the row's two tiers, held
    whether it rains or not (docs/perf-budget.md "First-use hitches built
    ahead");
  * in the post chain's warm-up, the lens drops' pass (0x5C7AC0, made by
    the first frame with drops before; it draws nothing until RAINDROPPC
    is set) and the rain tint's two pipelines (0x5C7870) for the scene's
    and the intermediate format.
* **A Graphics row changed with a drive in memory** (the options behind
  the pause menu): `Game::warm_for_settings`, run as the next frame starts
  (`draw_at`), makes the same set for the new settings before that frame
  draws, side by side on scoped threads; the frame then makes nothing. It
  runs only when the settings (the frame-dependent fields left out), the
  sample count, the mirror's or Photo Mode's row differ from what was last
  made.
* **The 3-D noise volume** (Particles Cinematic, the volume effects; ~0.25 s
  of one core, a table made once a run) is made on a thread of its own as
  soon as a row that reads it is on in the menus (`prefetch_tables`), so
  neither the load nor the first frame waits for it.
* **The garage and the shops**: the platform's renderer and the FE car's
  renderer are made in the visit's first tick (the platform's beside the
  car's load) and the front end's light rigs read beside them, rather than
  in the first draw.

**The scene's sample count a drive's programs are made for** is the one a
drive's frames draw with (`display::drive_msaa_samples`): with TAA or a
temporal Upscaling tier chosen, a drive's frames run the temporal pass with
no samples, while the frame drawing when a load or the menus' work ahead
starts (a menu, the loading screen, a garage) keeps the native Full Screen
Anti-Aliasing. The warm-ups took that frame's count, so with the native
FSAA on and a temporal row chosen a drive's first frames made the world's,
the cars', the horizon fog's, the blur's and the rain's programs for the
single-sampled scene (141 programs at Low, measured with the test below).
FSR 2's passes made their bind group layouts outside the device's cache, so
the pipelines keyed on them were never shared between two `Fsr2`s: they now
come from the cache (the same layouts, so the same pipelines), and a copy
made ahead leaves its programs for the frame's. Check:
`tests::first_use::the_temporal_rows_are_ready_before_their_first_frame`
(TAA, Upscaling 1–11 at Low, Medium, High and the Extreme preset with the
native FSAA on: nothing made by a free roam's first ten frames, loaded
straight away or after the menus' programs made ahead, whose load makes
none either).

## Ahead of the load

A drive's programs depend on the settings in force, the window's format
and the scene's sample count, not on the track or the car
(`render::enh::warm::ProgramSet`). So they are made before any load asks:

* **`enh::warm::drive_programs(gpu, set)`**, with no world and no car at
  hand: the world's scene sets for every pipeline key at the drive's mode
  (`world::record::warm_scene_programs`: `scene_pipelines_for`, which a
  renderer's own `set_mode` calls with its glow keys; the lamps' shadow
  layers, Wet Road Interaction's trail map, the traced shadows, the
  weather passes), the cars' (`car::warm_programs`: native, the scene's dry
  and wet, LOD Fade's, the SSR mask's, Enhanced Glass, the mirror's
  reflection set, which the menus' garage scene draws the FE car's floor
  reflection with), the Enhanced screen passes' (`frame_passes`), the post
  chain's additions on a copy of the chain, Photo Mode's. They land in the
  device's shared sets and cache; a renderer made later asks for the same
  descriptors and takes them.
* **The game** (`loads::warm::ProgramsAhead`, `Game::programs_frame`) starts
  that on a thread of its own with the first frame the window draws (the
  boot screens, the title screen, every menu) and again when the set in
  force changes (a Graphics row in the menus, the camera's mirror): only
  the difference is built. `NFSU2_NO_PROGRAMS_AHEAD=1` turns it off.
* **What needs a world** is made beside the world the menus load ahead
  (`Preload::world`, on its thread): the preloaded renderer's own warm-up
  (`WorldRenderer::warm_drive`: its glow keys, the masks, Light Probes'
  field and baker, Ambient Detail's props, the Next-Gen sky) and
  `enh::warm::world_programs`: Animated Water built and warmed at its
  tier and the smoke's GPU half, both handed to the drive that takes the
  preloaded world (`WorldAhead`: the objects its load would build on fresh
  ones), and the traced scene's world half (below).
* **The world's collision** (`World::collision_ahead`) is made beside the
  world the menus load ahead and on a load's own world thread (the drive's
  first placing made it in the region's first frame, 16–20 ms; only from
  the game's collision packs), and **the main menu's garage scene** is
  made while the title screen is up (`garage::ahead`): docs/start-and-menus.md.
* **A load's warm-up is unchanged** (`loads::warm::warm_passes`): it asks
  for everything as before, takes what is made, waits for a key another
  thread is making and builds what is missing (another set, a load that
  comes before the work ahead is done, a region nobody loaded ahead).
* **Quick Race**: its options and car-select screens load the chosen
  track's region ahead when it is not free roam's (`preload_drive_loads`),
  as the other menus load free roam's and the Engage dialog a career
  race's; back in the other menus free roam's is loaded again.

**The traced reflections' trace** (Ray Traced reflections): its pipelines
(the ray list's fragment and compute passes where the device takes the
list, else the fragment form's, docs/rt-reflections.md "The ray list") were
made by the first traced frame; `Ssr::warm_rt` makes them in the drive's
warm-up, the settings warm-up and `game --prepare` (`enh::warm::frame_passes`),
and the RT trace's bind group layouts now come from the device cache, so
the warm-up's objects are the frame's. Check:
`tests::first_use::the_ray_traced_rows_are_ready_before_their_first_frame`.

**The traced scene** (Modern Lighting with RT, the traced reflections):
its world half — the geometry gathered (about 0.1 s on one thread), the
colours, the BLAS and its build's submission — was made by the region's
first frame on the main thread (`rt::frame`). `rt::prepare` makes it at
the load's warm-up and beside a world loaded ahead; the first frame finds
it made. The same objects from the same world, submitted earlier.

Checks: `tests::first_use::a_load_after_the_menus_makes_no_program` (the
game files): with the main menu up until the work ahead is done, a
free-roam load and its frames make no pipeline and no module at every
preset, and neither after a preset is switched to in the menus. `tests::first_use::the_menus_garage_scene_after_the_boot_screens_makes_no_program`:
with the boot screens up until the work ahead is done, the main menu's
garage scene (its platform's renderer, the FE car's and its floor
reflection) makes none in its frames, at every preset (before: the
reflection set, one pipeline).

**The garage scenes' weather passes.** A garage scene's world draws the
rain's and spray's passes (`WeatherFx`) into its own target at the front
end's sample count (the native MSAA: the temporal pass is off there,
`display::front_msaa_samples`). Its renderer is made in the visit's first
tick, and with rain it built the haze's two pipelines in the scene's first
frames (seen at High). The weather passes' pipelines now come from the
device's cache (`device_cache::Programs`), and
`world::record::warm_front_scene_programs` makes them for that target and
count: in the menus' work ahead (`drive_programs`, the main menu's garage)
and in a drive's load warm-up (`loads::warm::warm_passes`, the shops'
garage scenes over it). A drive's own (`warm_scene_programs`,
`WorldRenderer::warm_drive_passes`) are now shared the same way. The
pictures are the gate's (identical). Numbers: docs/perf-budget.md "Load
times".

## Kept across a car change

A drive dropped for a car change on the same world (the shop exit with a
part fitted) leaves its world and the world's renderer for the next drive
(`loads.resident`, `RenderState.resident`), and now its Animated Water
(`RenderState.resident_water`, `Water::for_new_drive`: the mesh and the
GPU half, which nothing in a drive changes; the per-frame draw list, the
Next-Gen tick pick and the Realistic and Next-Gen halves start new). The
load takes it where its warm-up would build it again. Check:
`tests::first_use::a_water_kept_across_a_car_change_is_a_new_one`.

**Made ready in the menus** (`loads::kept`, `Game::kept_ahead_frame`). The
kept world's own load still put it back to its state as loaded
(`World::reset_to_loaded`, ~15 ms of one core), rewrote every placement's
instance (`WorldRenderer::reset_instances`, ~8 ms) and made the smoke's
GPU half and the water's Realistic and Next-Gen halves again. A car change
is always followed by menus (the part's question and the browser again,
the car lot's screens): from the first menu frame after the drop that work
runs on a worker, with the same calls on the same objects, and the load
takes the result (it waits for the worker if it is still running):

* the world's reset (the barrier switches as loaded: a world taken from
  `loads.resident` is never one whose drive keeps them);
* the renderer's instances (`reset_instances`: `reset_placements` without
  the loaded emitters, a process-wide table the frames read; the worker
  works the emitters out and the load sets them, `set_loaded_emitters`,
  where `reset_placements` did);
* Animated Water built at its tier (`Water::build`, `warm_tier`) and the
  smoke's GPU half (`Detail` / `Cinematic::prebuild`) for the settings in
  force then; a load for other settings (a row changed meanwhile) drops
  those halves and makes its own, as before.

The kept renderer is not drawn while kept, and its instance writes are
queued after the frames that last drew it. **The player's car's renderer**
is built on the preloaded car's thread once the car is loaded
(`Game::spawn_car_load_drawn`, `Preload.car_renderer`; the shop's menus
load the new car ahead as the other menus do) and taken by the load when
built for what it would build it for (`cars_ahead::BuildKey`); else the
load builds it as before. Check:
`tests::first_use::a_car_change_made_ready_in_the_menus_is_the_one_made_at_the_exit`
(two games fit the same part at the fountains and stay the same frames in
the body shop's menus, one with this work switched off: the same world
dump, water state and first five pictures at the Modern, Ultra and Extreme
presets; a variant that skips the world's reset fails it). A drop followed
by a load with no menu frame between (the developer harness's "out of a
shop, a part fitted", the tests above) does the work in the load as
before. `NFSU2_NO_PRELOAD=1` turns it off.

## Car renderers ahead (`render::cars_ahead`)

A car's renderer (`CarRenderer::new`: its textures uploaded, its buffers,
its draws) is made by the first frame that draws the car
(`for_each_drawn_car`): 2–3.6 ms of that frame for a traffic car placed
mid-race or an encounter racer driving onto the road. Those cars exist
before they show: the drive's traffic pool is made with the drive (the
race's is paused during the start's countdown) and the encounter racers are
loaded and parked. Once a tick (`Game::cars_ahead_tick`) the ones that have
no renderer and are not drawn by the next frame are built on a worker, one
batch at a time, each with the key of what it was built for (`BuildKey`:
the device, the window's format, the car sampler's filter and anisotropy,
the texture packs' generation; nothing else of the settings reaches the
build). The first frame that draws the car takes it when it is ready and
its key is the frame's; otherwise the frame builds it in place as before
(and the one built ahead is dropped off the frame). A renderer that has
not drawn holds no state, so the frame draws the same either way. The
encounter racers' car models are shared with the worker (`Arc<Car>`, as
the traffic's already were). **The race's rivals**: their renderers were
made by the race's first frame (~7 ms of it with three rivals on the
integrated GPU's machine); each is now built on its car's load thread in
the race's set-up (`line_up_rivals`, beside the other rivals' loads; for a
car loaded ahead behind the Engage dialog, on a thread that joins that
load) and offered to the frame under the rival's id (`CarsAhead::offer`).
The renderer reads the car's parts, solids, textures and light materials;
what the set-up changes on the car after its load (`set_race`, the
difficulty, the drag AI's shift set-up) is its handling. **A lost race's
ending companion** (the AI winner riding along: a copy of the winner's car
with an id of its own, `SceneCar::from_rival`, made when the race-over hold
has run, about 3 s after the player's finish): once the player has
finished and the winner by the race end's finishing order is a rival that
has finished (`finish_order`, the same function), its car is loaded again
from the key the race's set-up loaded it with and its renderer built after
it on a thread (`loads::CompanionAhead`); the ending offers it under the
companion's id when it is done and was built for that rival, and the first
frame that draws the companion takes it when its build key is the frame's.
Not predicted: a drift race (placed by its points), a test run and a
replay; there, and when the build is not done, the frame builds it as
before. What it holds: one car renderer (the one the ending's frame would
build) from the player's finish to the ending, about 3 s earlier; the car
loaded for it is dropped once the renderer is built. Check:
`tests::first_use::a_lost_race_ending_takes_the_companion_built_ahead`.
`NFSU2_NO_PRELOAD=1` turns it off with the other work ahead.
The line-up behind the Engage dialog (`Game::predict_rivals`) is drawn on a
copy of the shared generator as the race's set-up draws it: the line-up,
then the looks, in entry order, with nothing drawn on the generator between
the dialog's Engage and the line-up (the world build's seed draw comes after
the looks, docs/rng.md "Race's" order). An event of several races (URL
events, tournaments) is predicted for its first race, where its line-up is
drawn. The race takes a car loaded ahead only when its key (car, parts,
performance packages) is the one it asks for, so a prediction that misses
costs the load, never the race.
A Quick Race's rivals load ahead the same way while its transmission
question is up (`Game::predict_quick_rivals`): the car is picked then and
either answer starts the race at once. The line-up is drawn after the
track's load, from the event slot `quick_event` set up (names and skill
rows drawn there), with the picked car's SUV pool and the player's car as
`drive_in` makes it (its type, and the performance rating its race set-up
gives it, `Car::set_race` on the tuned record); nothing draws on the shared
generator between the pick and the line-up, and a prediction is made again
when the generator moved (the menus' sounds draw on it) or another car was
picked. A drag race (no question) and a player who turned the question off
start at once: nothing is loaded ahead for them.
A race raced again (Restart, a tournament's Next Race) races the same
field (docs/formats.md "Race types: Restart and Next Race keep the field"),
so its set-up takes the rivals' cars of the race before for the same keys
(`Loads::raced_cars`: the paused race's, or the finished race's kept for its
replay or, without a recording, kept at its end) instead of loading them
again; each car's renderer is built again from it on the car's thread, as
for a load (the set-up makes the car's handling again from its stock
record, so the car is the load's).
Check: `tests::first_use::the_race_traffic_takes_renderers_built_ahead`,
`tests::first_use::the_race_rivals_take_renderers_built_with_their_loads`
(from the Engage dialog the race takes every rival loaded ahead) and
`tests::rivals_ahead::the_engaged_race_takes_every_rival_loaded_ahead`
(every event kind of stages 1–5, at a stage's start and half way through
it: all cars taken, the same rivals, generator and first 30 steps' state
hashes as the same start with nothing loaded ahead),
`tests::rivals_ahead::the_quick_race_takes_every_rival_loaded_ahead` (every
Quick Race mode with a racing mode, three cars, an SUV among them: the same
checks) and
`tests::race_again::a_restart_with_the_cars_kept_is_the_restart_that_loads_them`.

## Encounter racers ahead (`EncountersAhead`)

Career free roam's set-up draws its three encounter racers on the shared
generator (`Encounters::picks`: the names, each racer's car, paint, neon,
skill row and seed, then their performance matched to the player's and
the looks; 0x540790 → 0x53F010(3)) and loads their cars on threads that
the set-up joins before it ends. In the menus that go on to free roam
(`preload_encounters`) the same draws are made on a copy of the generator
as it stands (`Game::predict_encounters`) and the three cars loaded on
threads; the set-up takes a load only when its key (car, parts,
performance packages) is the one it asks for, and loads any other itself,
so a prediction that misses costs the load, never the racers.

* **The draws** are the set-up's own from the generator's state: nothing
  draws on the shared generator between `explore`'s start and the set-up
  (the drive's load draws nothing; the world build's seed, the traffic
  slots and the rain come after it, docs/rng.md "Free roam's order"). The
  prediction drew a traffic seed first whenever free roam's traffic was
  not loaded, a draw the traffic load no longer makes (its models draw on
  the shared generator after the world build): from the menus with no
  drive and after any race it predicted from a stream one draw ahead and
  missed all three cars, except when the stream fell back in step in the
  names' draws (`draw_names` draws again for a name already taken, so the
  two streams line up when the first name drawn comes up again among the
  next ones) and on a shop's exit, with free roam's traffic kept. Removed.
* **What else the draws read** (`EncounterInputs`): the player's car
  type, its rating and SUV pool, the career difficulty and the car key the
  racers' performance is matched to (`human_perf`). A prediction is made
  again on the first tick any of these or the generator moved (Resume
  Career's open-event picks, a post-race save point's, a car or part
  changed), once the last loads are in; the loads the new prediction no
  longer names are let go; at most 8 predictions a run of menus.
* **The screens**: the front end with no drive, the post-race screens, a
  race's pause, a shop's screens, the garage (Jump to Garage keeps the
  drive), and every menu screen after one of them until a drive or a race
  runs (the post-race save point's questions, a won race's movie). Not on
  a screen's first half second (its own set-up, as the garage scene's car,
  goes first); a question over a screen (the garage's Return to Explore
  Mode) goes on with the screen's time. The car the draws read is the
  drive's when free roam keeps it, else the menus' preloaded one once it is
  loaded.

What it holds: three car loads made ahead (the set-up's own cars, which it
takes), and for a prediction that missed, up to three more until the
set-up drops them. Check:
`tests::encounters_ahead::every_free_roam_entry_takes_its_encounter_cars_loaded_ahead`
(two games walked through the same screens in real time, one with nothing
loaded ahead at its loads; from a fresh career and stage-2 and stage-4
careers saved by the harness: Resume Career and the garage in a hurry,
a race's end with its results and post-race save, a shop's exit, Jump to
Garage, a Quick Race left for the career: all three cars taken in every
entry, the same racers, generator and first 60 ticks' state hashes).
`NFSU2_NO_PRELOAD=1` turns it off with the other work ahead.

## Parallel creation

wgpu 30 makes pipelines from any thread. On Vulkan `create_render_pipeline`
holds no device-wide lock in wgpu-core; wgpu-hal translates the module to
SPIR-V per pipeline and takes a short lock only for its render-pass cache.
On DX12 the shader compile (DXC or FXC) runs outside wgpu-hal's shader-cache
lock (taken only to look up and insert). Measured on the reference desktop
(Vulkan): switching Modern Lighting RT on made 65 pipelines and 3 modules
whose builds summed 150 ms in 28 ms of wall time.

## Measured (reference desktop, 1920×1080; median of 3 interleaved runs, whole machine / two cores)

The base arm is the base with the same measuring aids (the counters, the
device cache's stand-in that makes every object). The machine ran other
gates beside these runs (load average 30–500): single runs scatter, the
ranges are in the internal notes.

First frame after a load into free roam (`--bench explore`), base → now:

| preset | pipelines / modules | first frame ms (whole) | first frame ms (two cores) |
|---|---|---|---|
| Original | 19 / 1 → 0 / 0 | 9.8 → 5.7 | 10.2 → 7.5 |
| Balanced | 36 / 1 → 0 / 0 | 12.0 → 5.0 | 11.9 → 8.0 |
| High | 47 / 5 → 0 / 0 | 27.4 → 12.8 | 36.1 → 34.6 |
| Ultra | 88 / 18 → 3 / 0 | 112 → 70 | 125 → 114 |
| Extreme | 102 / 28 → 3 / 0 | 390 → 171 | 408 → 179 |
| Extreme, empty caches | 102 / 28 → 3 / 0 | 741 → 202 | 933 → 216 |

The 3 left are the Animated Water's. In a race (`--bench race`) Extreme
393 → 187 / 488 → 200 ms. After a row is switched on in free roam the
frame drawn with it makes nothing (base: 2–68 pipelines, up to 47 ms of
frame on two cores for Headlight Beams); the warm-up before it takes
2–50 ms (Ambient Detail: ~0.35 s then, its props' placement, which the frame
made before; since its slot decisions are made once per texture, about
0.1 s of CPU in the city, docs/perf-budget.md "Ambient Detail's props: made
in the menus, kept for the drive"). Photo Mode's first frame makes nothing
(base: 1).

The 3 were the Animated Water's; they are made at the load now too (below).
What the Extreme first frame still does: the light probes' bake, and, in the benches only, Surface
Detail's maps loaded before the first frame (`surface_maps::blocking`). The bake's arrival burst waits for
the GPU once for all its probes (it waited once per probe: 5–9 turns of a shared GPU's queue in the first
frame; docs/perf-budget.md "Load times").

Loads: a drive's load now also makes the passes' programs, on threads
beside the load's own work. From the menus (the world preloaded, the noise
table made) Extreme's load is ~90 ms on the whole machine either way and
~100 → ~140 ms on two cores; a load from the game's start makes the post
chain too (in the game the menus have made it).

Animated Water at the casino square's fountains (`--bench explore 60`,
1920×1080, the camera on an orbit of the fountains: `NFSU2_BENCH_CAMTRACK`
from the private script `orbit.py -326 -547 21 25 4 120`,
`NFSU2_BENCH_ENH="enh.water N"`, Extreme by `NFSU2_BENCH_PRESET=Extreme`;
median of 3 interleaved runs), base → now:

| setting | first-frame builds | first frame ms | load ms |
|---|---|---|---|
| Original + On | 3 pipelines → 0 | 45.5 → 5.0 | 142 → 179 |
| Original + Realistic | 3 pipelines (+1 in a later frame) → 0 | 71.0 → 5.0 | 144 → 206 |
| Original + Next-Gen | 18 pipelines, 10 modules (+ 2 later frames) → 0 | 283.9 → 5.8 | 142 → 425 |
| Extreme + Next-Gen | 7 pipelines (+1 later) → 0 | 246.9 → 91.3 | 312 → 334 |

The load takes over what the first frame did: the water mesh, the GPU
halves (Next-Gen's jets' charts and basins' fields, ~0.25 s of the 0.28 s
it adds alone) and the pipelines; beside Extreme's other warm-ups it adds
~20 ms. A tier switched in the options is made by the settings warm-up
before the frame that draws with it.

## Check

`tests::first_use` (the game files): after a free-roam load and a race at
every preset, no drawn frame makes a pipeline or a module; switched from
Original to each preset in free roam in turn, and with Photo Mode opened,
neither (`render::record_builds`: the places programs were made for one
`Gpu`, a measuring aid); and a free roam loaded at the casino square's
fountains with Animated Water at each tier, then the tiers switched to in
turn there and Extreme's rows with Next-Gen water, neither (the water's
builds were left out of the check before; with them counted the check
failed at the first tier: three pipelines in the first frame). Not reached:
rain starting mid-drive (its passes, the spray volume
and the lens drops' pass are made at the load whatever the weather). The
shops' garage scenes are checked by
`tests::first_use::the_shops_garage_scenes_after_a_load_make_no_program`
(each shop kind's front end over free roam, at every preset).

The world modules have two forms of the lamps' light-cluster loop, one per device (subgroup operations or
not, docs/lamps-shadows.md "The cluster loop"): the warm-ups and `game --prepare` make the world's programs
through the device's `WorldShared`, so they make the form that device draws with.
`tests::first_use::the_cluster_loop_forms_draw_the_same_and_are_ready` loads free roam and a city circuit at
High and Ultra on a device with subgroups and on one without (`NFSU2_GPU_DROP=subgroup`): no frame makes a
program in either, and the pictures are the same.

Aids: `--bench` prints the first frame's builds (`first frame builds:`),
its update / draw / GPU wait and the draw's parts; `NFSU2_BENCH_PRESET`
(a Graphics preset by name), `NFSU2_BENCH_PHOTO_AT=n` (Photo Mode at frame
n), `NFSU2_BENCH_FIRST_FRAMES=n` (the first n frames each),
`NFSU2_BENCH_ENH_SWITCH` now also times the settings warm-up;
`--bench transitions` prints each transition's builds and its slowest
frame in the second after with that frame's parts; `NFSU2_PIPELINE_LOG=1`
prints each build's place and time.
