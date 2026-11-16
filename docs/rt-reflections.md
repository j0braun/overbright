# Ray-traced reflections (experimental, opt-in)

A **clone deviation**, nothing native and nothing traced in SPEED2.EXE: the
original has only the planar road capture (view 4) and the car's cube
(docs/formats.md "Road reflections", docs/wet-roads-ssr.md). Defaults and the
Original preset are unchanged and byte-identical (below).

| row | new value | options.txt |
|---|---|---|
| Screen-Space Reflections (Reflections group) | **Ray Traced (experimental)**, after Ultra | `enh.ssr 3` |

`qol::ssr()` gives 3 only while the device was made with ray queries,
else 2 (SSR Ultra); `qol::rt_reflections()`. The tier is greyed out
(`offered`) unless the adapter can trace (`rt::adapter_can`: Vulkan,
discrete GPU, `EXPERIMENTAL_RAY_QUERY` offered; `NFSU2_RT=1` any Vulkan
adapter, `NFSU2_RT=0` never). No new label id (a tier of the SSR row).

## The rework in one table

The ray-tracing rework (the sections "Textured, glowing hits", "Light glows in the reflections",
"Quarter rate" below, the NIS characters' rays in the world's traced-shadow pass, docs/perf-budget.md,
and the hit textures built on the GPU) against the tree before it, the reference player's 4K settings
(SSAA 2x, Modern + RT, Ray Traced reflections), GPU busy ms per frame:

| scene | before | after |
|---|---|---|
| racing | 13.92 | 12.27 |
| night rain | 16.83 | 15.22 |
| race start | 16.60 | 14.13 |
| free roam | 10.95 | 9.86 |

The pictures gain the textured walls, windows and signs, the neon and the lamps' and cars' glows in wet
reflections (night rain: luma p95 +1 … +3 levels, mean within +0.6; 3–7 % of the pixels > 3 % at the wet
spots); the quarter-rate history steadies them (frame-to-frame change −0.4 % of the pixels in rain) and
softens the wet road's finest ripples a little at 200 %. VRAM: +350–425 MB at 4K (the hit texture layers
about 150 MB, the history pairs and the half trace about 215 MB).

Since then: the rays decide wet roads' and cars' reflections ("Primary on wet roads and cars"
below). Still open: the hits are not relit by the lamp clusters or the headlights.

## API status on this machine (verified 2026-11-05)
wgpu 30.0 (`wgpu-types` 30.0.1): `Features::EXPERIMENTAL_RAY_QUERY`
("may have major bugs … subject to breaking changes", Vulkan only,
native only), needs `DeviceDescriptor::experimental_features =
unsafe { ExperimentalFeatures::enabled() }`. WGSL: `enable wgpu_ray_query;`,
`acceleration_structure`, `ray_query`, `rayQueryInitialize / Proceed /
GetCommittedIntersection`, `RayDesc { flags, cull_mask, tmin, tmax,
origin, dir }`; API `create_blas / create_tlas /
build_acceleration_structures`, `TlasInstance` (3 × 4 row-major, 24-bit
custom data, 8-bit mask). Also offered: `EXPERIMENTAL_RAY_HIT_VERTEX_RETURN`,
`EXPERIMENTAL_RAY_TRACING_PIPELINES` (not used).

the maintainer's probe `rt_probe` (headless, no game data; exit 0 = usable), reference desktop GPU,
RADV NAVI32, Mesa 26.2.5 (internal evidence):
all three features offered; max BLAS primitives 536 870 911, TLAS instances
16 777 215; a 1 M-triangle BLAS + 2 k car BLAS + 65-instance TLAS built in
3.6 ms GPU; one coherent ray per pixel: 0.25 ms at 1080p, 1.0 ms at 4K;
TLAS rebuild (65 instances) 0.15 ms; every hit correct (grid / car / no
miss / no wrong t). **Verdict: usable here.** Not checked: other vendors,
Windows/DX12 (wgpu: Vulkan only), iGPUs (excluded by design).

## How it works (`render/rt.rs`, `render/rt.wgsl`)
* **Device**: the feature is asked for only when the tier is chosen as the
  device is made (options.txt `enh.ssr 3`, or `NFSU2_BENCH_ENH` /
  `NFSU2_SHOT_ENH` naming it, or `NFSU2_RT=1`); a refusal falls back to the
  old device. So choosing the tier in the menu **applies after a restart**
  (help text says so); until then it runs as SSR Ultra. With the tier not
  chosen the device is made exactly as before.
* **World BLAS** (once per region, CPU 217 ms for Bayview, then one GPU
  build): every placement's `low_solid` (the LOD the native road reflection
  and cube faces draw; none = nothing, as there) in world space, unindexed;
  left out: SKYDOME, the GPS aid and dyno rollers (hidden markers), additive
  textures, alpha-tested / blended textures with mean alpha < 0.5.
  Bayview: **650 816 triangles**, 1163 textures. Placements moved at run
  time (smackables) stay where they were built.
* **Car BLAS** (cached per model: name, parts, remaps; ≤ 4 new a frame):
  in-game parts but wheels at LOD C (A when the car has no C parts), car
  space. Rebuilt TLAS each frame from a ring of three (frames in flight):
  the world + up to 96 cars (player, companion, rivals, encounters, traffic;
  nearest first) at `CarState::transform()`.
* **Colour per triangle** (`rt_colors`, RGBA8, × 2 in the shader): world =
  mean baked vertex colour × texture mean (world.efx `tex × v0 × 2`); car =
  texture mean × the light material's mid diffuse (min + range / 2).
  **Lamp clusters are not applied** to hits: the baked vertex colours
  already hold every street lamp's pool (docs/lamps-shadows.md "Pools too bright"), so the
  bake alone is the lit colour (an inference from the bright-lamps fix; no material system).
* **Trace**: the SSR trace pass in a variant (`rt::trace_source`: ssr.wgsl
  + rt.wgsl, its five anchors rewritten; the SSR modules are untouched).
  Where the screen march is unsure (confidence < 0.98: misses, off-screen
  and behind-camera rays, rays toward the camera, near-plane early outs)
  one ray query (300 m) fills the rest: `rgb = (ssr·c + rt·(1−c)·conf) / a`.
  Rays from a car skip a car hit within 1.5 m (its own coarser LOD) and go
  on against the world. The SSR filter and composite then run unchanged.
* **Never stronger than native on a dry road**: The chrome-flicker fix's luminance hold (the
  traced colour held to the planar term's luminance on a dry road) applies
  to RT world hits as to SSR hits; the composite weight is the native one.
  Car origins get the fog of the reflected path (as SSR's).

## Textured, glowing hits (prototype)
The first step of the ray-tracing rework: a hit no longer takes its triangle's flat colour.
* **Textured hits** (world triangles): per triangle a 48-byte surface record (`rt/geometry.rs`
  `Surface`: the three UVs, the three baked vertex colours, the texture's layer and the LOD's
  triangle term) and one texture array of every traced world texture, box-filtered to
  128 × 128 with its mips (`hit_layer`; Bayview's layers are made at the region's load). The hit
  is coloured as world.efx draws it, texture × interpolated baked colour × 2, at the mip the ray
  cone's width at the hit asks for (Akenine-Möller et al., "Texture Level of Detail Strategies
  for Real-Time Ray Tracing", Ray Tracing Gems ch. 20: the per-triangle term plus log2 of one
  trace pixel's angle × (the eye's distance to the surface + the reflected path)).
* **Neon and lit signs in the reflection**: the additive SGN_* / *NEON* / *_SIGN textures of every
  placement's main LOD (`emissive_geometry`, the names of Modern Lighting's emitters) are a second,
  non-opaque instance (mask 4) that no other ray sees (the opaque casts and the Modern + RT shadow
  rays now use mask 3). After the opaque hit, one more query from the same start to the hit visits
  every glowing triangle on the way without committing any and sums texture × alpha × vertex colour × 2
  (additive blending, so the order does not matter); the list's compute pass runs it on its one
  query (`glow_on_one_query`: a second query halved RADV's workgroup-memory traversal stack,
  measured +0.4 ms). A miss keeps the march's result (no glow over the sky).
* `NFSU2_RT_FLAT=1` keeps the flat colours (frames identical to before, checked); cars keep the
  flat colour for now.
* Measured (the reference player's 4K settings, 4 interleaved pairs, load 12-20): racing frame
  13.99 → 14.21 ms, the rays 0.99 → 1.17; night rain 16.95 → 17.14, rays 1.11 → 1.32; VRAM
  +150 MB. The layers are made on the GPU (`hit_texture_gpu`, `rt_hit_shrink.wgsl`: each world texture
  uploaded as the game stores it, DXT with its mips, and all eight levels of its layer written by one
  compute dispatch, each from the source level of that size; a texture with one level larger than twice
  the layer is box-filtered on the CPU, `hit_layer`; `NFSU2_RT_CPU_LAYERS=1` filters all on the CPU):
  Bayview's 1040 layers in 41-47 ms instead of 213-224 ms, the region's traced scene in 165-175 ms
  (geometry 105-110 of it) instead of 337-351; pictures against the CPU's layers: 0.03 % of the pixels
  > 3 % (night rain). Pictures: luma mean / p95 unchanged (±0.3), 1-4 % of the
  pixels > 3 % at the wet Modern Lighting spots (reflected walls, windows and signs now have their
  texture), 0.1-0.2 % in a dry race.

## Light glows in the reflections (prototype)
The lamps' and cars' light glow sprites (docs/formats.md "Light glow sprites") are camera-facing
quads, not geometry, so no ray met them: a wet road reflected an off-screen or occluded lamp as
nothing. Now:
* **Street lamps**: every flare of the region's flare packs is a `Glow` (`rt/geometry.rs`: the type's
  first layer, colour × doubled alpha, size, depth clamp, facing exponent with flag 4) whose bounding
  octahedron (8 triangles, the record's mark `GLOW_MARK`) sits in the glowing instance (mask 4). The
  glow query's candidate loop takes each glow once (the last four seen) and adds the sprite's soft disc
  (`(1 − r²/R²)²`) at the ray's nearest point to its centre, its radius as the sprite's at the
  reflected path's length (eye to surface + t), × the streak buffer's steady state where the glows go
  through it (as the march sees an on-screen glow).
* **Cars' head and brake lights**: the 16 glows of the cars nearest the eye (headlights type 0, brake
  lights type 1 at the native 0.85: the traced scene does not know the brake) in the trace's uniform,
  tested one by one (scalar loads; a per-frame BLAS of their octahedra cost 0.75 ms: rays near the
  player's car start inside its lights' 5 m octahedra). A ray leaving a car skips glows within 1.5 m.
* **Lamps against the sky**: a ray that misses now still looks for glows up to 120 m and returns the
  reflection's own term (planar / cube) plus the glow.
* **Wet road streak**: on a road the glow is stretched along the vertical by the native mirror sprite's
  factor (1.2 … 10 by the wetness, `streak_factor`) and kept at 0.6 of the sprite's light (at 1 the
  night-rain p95 luma rose 4 levels).
* **Dry roads** cast no glow ray (their reflection is held to the native term's luminance, which the
  glow could not pass): a dry race costs nothing more.
* `NFSU2_RT_DEBUG=glow` shows the glow term alone (× 20), a check aid.
Measured (the reference player's 4K settings, 3 interleaved triples, load 11-16): racing (dry)
13.99 / 14.14 / 14.28 ms (today / textured / + glows), rays 0.99 / 1.16 / 1.12; night rain
16.98 / 17.33 / 17.54, rays 1.10 / 1.34 / 1.67. Pictures, night-rain race start frames 100-111 vs
today: luma mean +0.3 … +0.6, p5 equal, p95 +1.1 … +3.0; 3-7 % of the pixels > 3 % at the wet spots;
frame-to-frame change +0.26 % of the pixels (no added flicker; no history is kept).

## Quarter rate (prototype)
With the ray list (the path a discrete RADV GPU takes), the Ray Traced tier no longer traces every
trace pixel every frame:
* **Half-size trace, jittered**: the trace's fragment pass (the march and the ray list) runs on a
  half-size target; each of its pixels stands for one pixel of its 2 × 2 of the full trace
  (`trace_frag`: `floor(frag) × 2 + 0.5 + jitter`), a different one each frame (`JITTER`: (0,0),
  (1,1), (1,0), (0,1)), so the march, the rays and the result are exactly the full trace's at that
  pixel. No mask depth there (sizes differ): the trace's own early outs skip the pixels without a
  mask. The list holds the half-size pixel; its compute pass writes the half target.
* **Resolve** (`ssr_resolve.wgsl`, a fragment pass at the full trace size into one of two history
  pairs: colour, and the surface's view depth negative on a car): per pixel the half trace's 3 × 3
  samples on the same surface (kind, depth within 5 %), weighted by distance and confidence (the exact
  one where this frame traced the pixel); the last frame's result where the surface was (the road:
  the point reprojected by the camera's motion; a car: the same pixel, as the chase camera holds it),
  dropped where the depth or kind there differs, off screen, on the first frame or after a cut (eye
  20 m away), else held to the current samples' range (+10 %) and blended (the current at 0.5 where
  traced this frame, 0.2 elsewhere), all in confidence-weighted colour. The filter and composite then
  run as before on the resolved trace.
* `NFSU2_RT_QUARTER=0` traces every pixel every frame (the previous form, a check aid); the debug views
  and the fragment form (no list) always do.
* The history follows frames, not world ticks: at a low frame rate it covers more time (the clamp keeps
  it from trailing). VRAM: two full-size history pairs and the half trace, about +215 MB at 4K.
Measured (the reference player's 4K settings, 3 interleaved triples, load 8): racing 13.88 today →
14.14 (glows) → **12.72** ms; night rain 16.73 → 17.51 → **15.50**: the trace 1.85 → 0.71 ms, its rays
1.52 → 0.46, the resolve 0.33. Pictures against the full rate (frames 100-111): rain 0.93 % of the
pixels > 3 %, racing 0.12 %, luma mean / p5 / p95 within 0.1; frame-to-frame change −0.4 % of the
pixels in rain (the history steadies the reflections); at 200 % the wet road's finest ripples are a
little softer.

## Primary on wet roads and cars (prototype)
A wet road's and a car's paint and glass reflection is decided by the ray, not by the screen march:
* **Every pixel casts** (`rt_is_primary`: the tier, not a debug view, a car's surface or a wet frame's
  road). Their trace pixels are not marched first. On a wet frame every mask pixel is primary: the
  trace's fragment pass and the list are skipped and a dense compute pass (`cs_rt_primary`, 8 × 8
  tiles over the half trace) traces them; a dry frame's cars go through the same pass after the
  fragment pass has marched the roads (listing them cost more: the tiles' rays are more coherent than
  the list's).
* **The screen where it holds the hit** (`rt_screen`, after FidelityFX Hybrid Reflections' reuse of
  the screen): where the ray's hit projects on screen and the pyramid's depth there lies within the
  traced low LOD's offset of it (0.6 m or 5 %), the hit's colour is the drawn scene's, taken as the
  march takes a hit's (its fog taken off, the glow sprites there, a car's path fog) and faded into the
  traced colour over the screen's edge; no glow ray then. Elsewhere the traced hit's own colour
  (texture, bake, neon and glows).
* **The march where the traced scene is short** (`rt_primary_at`, wet roads): where something on
  screen nearer than the hit hides it, or the ray meets nothing, the screen march (fs_trace as
  ssr.wgsl writes it, `ssr_march`) is asked, and a march hit well before the ray's (under 90 % of its
  path) takes its place by the march's confidence: alpha-tested railings and foliage, the NIS
  characters and moved props are not in the traced scene but are on screen. Cars keep the ray's.
* **The mirror's history by the reflected point** (`ssr_resolve.wgsl`): the list's pass writes each
  primary ray's path to its hit (`g_rt_dist`, a half-size R32Float target); a wet road's resolve takes
  its history where the reflected point was (the eye's ray through the surface point, extended by the
  ray's path: the image a flat mirror shows), on the same stretch of road (its depth within 25 %),
  blended with the surface's own reprojection by roughness (the reflected point alone up to 0.15,
  the surface's from 0.45).
* `NFSU2_RT_PRIMARY=0` keeps the march first (the previous form, a check aid); `NFSU2_RT_DEBUG=primary`
  shows red where the screen's colour was taken, green the traced hit's, blue a miss.

Measured (the reference player's 4K settings, Modern + RT, 4 interleaved pairs against the tree before,
load 4-9, GPU busy ms): with the traced shadows' changes of docs/shadow-softness.md "Every light"
(which pay for the dry frames' rays) racing 12.32 → 12.31, night rain 15.03 → 14.74, race start
14.13 → 13.94, free roam 9.12 → 9.31. The reflections alone: night rain −0.6 ms (the march gone: trace
0.72 → 0.02, rays 0.50 → 1.12 with the march where the screen is short), dry frames +0.1 ms of rays
(the cars). Pictures at the four wet spots against the march-first form (Modern without RT, so only
the reflections differ): 1.6-5.9 % of the pixels > 3 %, luma mean −0.3 … +0.2, p5 equal, p95 −0.5 …
+1.1. Night-rain race start frames 100-109: 2.3 % > 3 %, mean −0.15, p95 −1.1, frame-to-frame change
30.61 % vs 30.68 % of the pixels (no added flicker). The quarter rate's difference to the full rate on
the wet road (the lower half): mean 0.99 vs 1.01 levels, about equal on this bench (its road is rough
enough for the surface's reprojection to dominate). Crops (left before, right after, then |diff| × 8 where three): `evidence/rt-next/primary_*`.

## The ray list (`rt/trace.rs`: `list_trace_source`, `list_rays_source`, `ssr_trace_list`)
A ray query in a fragment shader keeps its traversal stack in scratch
memory on RADV (76 entries per lane, `radv_nir_lower_ray_queries.c`; in a
compute shader a 16-entry stack in workgroup memory), which every wave of
the trace pays whether its pixels trace or not; RADV also compiles any
shader holding a ray query in the ray-query wave size (32), and this
pass's march is faster in the pixel shaders' 64 (measured, docs/perf-budget.md
"Wave sizes"). So, where the device allows it, the rays leave the fragment
pass (27–45 % of the trace's pixels need one in the scenes measured):
* **The trace's fragment pass** is the RT form with each `rt_fill(c, …)`
  call replaced by `rt_keep(c, …)` (the same five places): where `rt_fill`
  would trace (its own test, `!(c.a >= 0.98)`) the pixel goes into a list
  (`x | y << 15`, bit 31 for the four places whose result is 0) and, for
  the march's own result, its f32 `c` beside it; the pass writes `c` as
  before. No ray query in it. An `atomicAdd` per listed pixel gives its
  slot; every 64th also raises the compute pass's group count
  (`atomicMax`, the indirect dispatch's x; y and z stay 1).
* **The compute pass** (`cs_rt_rays`, 64 lanes, indirect) gives each listed
  pixel a lane: `rt_ray` is fs_trace's head as written in ssr.wgsl (the
  text from its first line to the reflected ray, cut out by its anchors),
  then `rt_fill` on the march's result as it was (0 for the bit-31 ones),
  with rt.wgsl's two casts on one ray query (`ONE_QUERY`: the same rays,
  so RADV gives the one query the whole stack); the result goes into the
  trace target (`textureStore`, the target a storage texture as well).
  The filter and composite then run unchanged.
* **Exact**: the same expressions on the same inputs, so the same f32
  results; the target's conversion to f16 is the same in both forms on
  RADV (measured: a fragment export to Rgba16Float rounds toward zero, and
  so does a compute shader's rgba16float `textureStore`: the same bits for
  every value of a 4.2 M value sweep, every f16, its neighbours and the
  rounding midpoints, but its 5 NaNs, whose payloads differ). Proven on the
  reference desktop GPU by the pictures and bench frames compared
  (docs/perf-budget.md "Ray-traced reflections: the rays from a list",
  `evidence/perf-budget/rt-ray-list.txt`) and the test (below); other
  drivers' conversions are not measured.
* **Chosen once** when the trace's targets are made (`rt::list_path`,
  `rt::list_fits`): ray queries, writable storage in fragment shaders, the
  trace's format writable as a storage texture, the list within the
  device's buffer limits (20 bytes per trace pixel: every pixel may need a
  ray) and the trace under 32768 a side; else the fragment form stays.
  `NFSU2_RT_TRACE=fragment` keeps the fragment form (a check aid);
  `NFSU2_RT_LIST_STATS=n` prints the list's count every n-th trace (a
  measuring aid). The game's log says which form a trace took (`ssr:
  ray-traced trace …`).
* **Programs**: the pipelines of the form the device takes (and the RT
  layouts, now through the device cache) are made by the drive's warm-up,
  the settings warm-up and `game --prepare` (`Ssr::warm_rt`,
  `enh::warm::frame_passes`); before, the RT trace's pipeline was made in
  the first traced frame (docs/first-use-builds.md). A trace too large for
  the list on a device that otherwise takes it (`list_fits`: over 32767 a
  side or the buffer limits) would make the fragment form's in its frame.
* DX12: the tier is Vulkan-only (`rt::adapter_can`); both forms compile
  for Windows, neither can run there.

## Cost (gpu busy ms/frame, median, `--bench … 300`, 2 runs each, Wet Roads on; internal evidence)
| scene | 1080p SSR Ultra → RT | Δ | 4K SSR Ultra → RT | Δ |
|---|---|---|---|---|
| explore | 1.31 → 1.56 | +0.25 | 4.40 → 5.19 | +0.79 |
| racing (cars) | 1.52 → 2.03 | +0.51 | 5.35 → 6.89 | +1.54 |
| rain (clock 170) | 1.37 → 1.78 | +0.40 | 4.93 → 6.04 | +1.11 |
The trace pass: 0.30–0.39 → 0.64–0.90 ms (1080p), 1.09–1.39 → 1.97–3.0 ms
(4K). VRAM peak about +350 MB at 4K (noisy metric). Draw-thread CPU
+0.1–0.15 ms in racing (car keys hashed per frame). Tier cost: 3.3 ms at 4K
(over Off). Shared machine (load avg ~50–110), reference desktop GPU.
With the ray list (above; the reference player's 4K SSAA 2x settings with Modern + RT, 7 runs per
arm): the trace 3.17 → 2.55 ms racing, 3.31 → 2.99 race start, 3.62 → 2.91 rain, 2.79 → 2.45 free
roam; the frame 19.02 → 18.55, 20.27 → 19.95, 23.45 → 22.87, 13.06 → 12.74 ms; 27–45 % of the trace's
pixels need a ray (docs/perf-budget.md "Ray-traced reflections: the rays from a list").

## Images (`evidence/rt-reflections/`, 1280 × 960 `--shot`, left SSR Ultra, right RT, Wet Roads on)
* `race_car_paint_ssr_ultra_vs_rt.jpg`: the race start's front car: the
  hood and headlight reflect what is behind the camera (the crowd, the red
  car beside) where SSR kept the cube.
* `qrstart_car_paint_ssr_ultra_vs_rt.jpg`, `intro_player_car_ssr_ultra_vs_rt.jpg`
  (night): side and roof panels take the overpass/buildings behind the
  camera.
* `rain_wet_road_ssr_ultra_vs_rt.jpg` (night shower): the wet road's
  off-screen rays now reflect the scenery behind; subtle (0.06 % of pixels
  over 2 %).
Changed pixels (> 2 %): intro 0.07 %, rain 0.06 %, qrstart 0.22 %, race 0.29 %.
Judged against the clone's own SSR (nothing to compare in the original).

## Native byte-identity
With every row native, `--shot explore / intro / qrcars / qrstart / race /
drift / drag` are md5-identical with and without a ray-query-capable
device run (`NFSU2_RT=0`); SSR Ultra (`enh.ssr 2`) identical in intro and
qrstart likewise. The Original bench (racing 1080p, 600 frames, 3 runs
each) shows no GPU difference (0.75–0.76 ms both). Not run: `perf.sh
parity` against a base build (the default path does not reach any new
code: the feature is not requested, `ssr ≤ 2` takes the old pipelines).

## Tests
`render::rt::tests`: `trace_source_hooks_every_anchor_once` (each anchor in
ssr.wgsl once; the RT module parses and validates in naga),
`instance_transform_is_row_major_3x4`, `pack_halves_and_clamps`,
`bayview_world_geometry_fits` (game data; skipped without NFSU2_ROOT: >
100 k triangles, within the 24-bit custom index with the cars' room);
`qol::tests::ray_traced_tier_falls_back_to_ultra_without_ray_queries`;
`enh.ssr` clamps at 3. GPU check: `cargo run --release -p devprobes --bin rt_probe`.
The ray list: `render::rt::tests::list_sources_follow_the_rt_form` (both
modules parse and validate, both depth forms; the fragment pass is the RT
form with its five `rt_fill` calls sent to `rt_keep` and no ray query; the
compute pass's head is fs_trace's text; one query),
`list_pixels_pack_in_15_bits`;
`render::ssr::tests::ray_list_traces_what_the_fragment_form_traces` (GPU
with ray queries, run explicitly with `--ignored`): a synthetic road with
rough and mirror parts, a car's patch facing the eye, the march's wall on
screen and a traced world around it (walls, ceiling, a car's box the
car's rays leave through), wet and dry, half and full size, single and
multisampled depth: the trace target equals the fragment form's bit for
bit and so does the picture; the list holds each pixel whose march result
has a confidence below 0.98 once, and no other, each with that result
(1520 of 4096 trace pixels listed, 128 of them with a result of 0);
altering the car's second cast in the list form alone fails it.
`tests::first_use::the_ray_traced_rows_are_ready_before_their_first_frame`
(game files, a GPU that traces): no program made in a drive's frames with
ray tracing on (free roam, a race, switched on in free roam, the native
FSAA's multisampled depth); without `Ssr::warm_rt` it fails (the list's
module and pipelines made in the first frame).

## Risks / open
* wgpu's API is experimental: breaking changes between wgpu versions are
  expected; driver bugs can hang or crash the GPU (UB per wgpu's docs).
  Only RADV/reference desktop GPU tested; NVIDIA/Intel, Windows untested.
* World hits are textured and see the neon / signs and the light glows (above); cars' hits keep a
  flat colour. Hits are not relit by the lamp clusters (the bake holds the lamps' pools) nor by the
  cars' headlights.
* Coarse LODs: the world's `low_solid` and car LOD C differ from what is
  drawn; self-hits are avoided only by tmin/offsets and the 1.5 m car
  rule. Smackables, the GPS arrow and moving props are not in the BLAS.
* The world build is a one-time 0.2 s CPU hitch on the first RT frame of a
  region (could move to a loader thread); BLAS compaction not used.
* The tier needs a restart to take effect (device feature).
* The primary reflection's screen test and march use the half trace's pyramid: a hit hidden by
  something within 0.6 m of it, or a thin occluder the pyramid's nearest depth hides, can take the
  wrong one. The fragment form (devices without the list) traces primary pixels in the fragment pass.
* The quarter rate's history follows frames, not world ticks, and is dropped at cuts and where the
  surface changes, and held to the current samples' range (+10 %) so it does not trail; at a low frame
  rate it covers more time. Without the ray list (no RADV-style list path) every pixel is traced every
  frame, as before.
