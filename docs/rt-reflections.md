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
* Flat colour per triangle (no textures, no UV lookup): hits look
  faceted/blurry; only acceptable after the SSR filter. No lamp clusters,
  no glows, no emissive signs (additive textures are left out), sky misses
  keep the planar/cube image.
* Coarse LODs: the world's `low_solid` and car LOD C differ from what is
  drawn; self-hits are avoided only by tmin/offsets and the 1.5 m car
  rule. Smackables, the GPS arrow and moving props are not in the BLAS.
* The world build is a one-time 0.2 s CPU hitch on the first RT frame of a
  region (could move to a loader thread); BLAS compaction not used.
* The tier needs a restart to take effect (device feature).
* No temporal accumulation: one ray per trace pixel, noise handled by the
  SSR filter only.
