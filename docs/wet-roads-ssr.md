# Wet Roads and Screen-Space Reflections (clone deviations, opt-in)

Screen-space reflections and wet roads. Two Graphics page rows, both off by default (native):

| row | values | options.txt |
|---|---|---|
| **Wet Roads** | Off / On | `enh.wet_roads 0/1` |
| **Screen-Space Reflections** | Off / Half / Full | `enh.ssr 0/1/2` |

Written only when not native; `qol::wet_roads()`, `qol::ssr()`. Meant to
map onto the graphics settings audit's presets (Wet Roads in Modern, SSR in Ultra). Nothing here
is native or traced; it builds on the traced road reflection
(docs/formats.md "Road reflections": worldreflect.efx, view 4 at
320 × 240, no cars in it) and, when on, Enhanced Reflections' planar
capture (docs/enhanced-reflections.md). The planar image stays the base on
every setting and every GPU.

## Wet Roads (`render/world.wgsl` `wet_surface`, the main view only)
A uniform branch at the top of worldreflect's fragment (`frame.wet.x`, a
new `Frame.wet` vec4 written only by the in-game main view: on, the
rain's amount weather +0x1F0, the road's wetness from the rain, the world
clock; docs/formats.md "Rain"). Front-end scenes (garage, showcase) never
get a weather and stay native. Per pixel, in world space:
* **Only where the rain wetted the road** (2026-11-04). The native
  wetness +0x509C is **1 at every track load** (0x61E100 writes 1.0 to
  views 1 and 2's weather) and dries by 0.0015 of the gap a step
  (0x61F2BD, 0x7A67CC) whatever the weather; its only reader is 0x490920
  (the reflection flares' stretch 0x7F39A0 = 1.2 + 8.8·wet; the ten
  references to +0x509C in the disassembly are the two writes at load,
  the constructor's 0, the debug forces 0x61F267 / 0x61F281 (0x7FAA50 = 1
  in the data, 0x8A1D38 BSS: never taken) and the step). Nothing of the
  road surface reads it or the rain amount: worldreflect draws at the
  texture's authored strength rain or not. Under Wine (a dry Quick Race,
  internal evidence, peek of
  [0x832EB4]+0x509C): 0.185 after ~19 s, 0.0478 → 0.0393 in 2.0 s (≈ 130
  steps of 0.9985), amount and target 0, the road dry. So the old level
  (wetness × …) made a dry start look wet for the first minute (5–6 %
  of pixels in the night and start scenes). Now `Weather::road_wet` (clone,
  weather.rs) follows +0x509C's rule from **0 at load**, rising only while
  the rain amount is non-zero (`weather::wet_step`); with it ≤ 0.001 the
  CPU leaves `wet.x` 0 (`render::wet_uniform`) and the native path draws:
  dry scenes are byte-identical to Wet Roads Off (explore, qrstart,
  motion: same md5). The native +0x509C still drives the flares.
* **Wetness level** = road_wet × (0.35 + 0.65 × rain); the wet look is
  faded in by k = smoothstep(0, 0.15, level) (reflection, slope, roughness
  and weight go to the native values as k → 0), so a drying road ends
  continuous with the native path.
* **Puddle mask**: two octaves of value noise (7 m and 2.3 m), thresholded
  wider the wetter, × the level, only where the texture's authored
  reflection alpha (tex.a·v0.a, the native strength) is > ~0.04.
* **Normal**: fine distortion (1.7 m⁻¹ noise gradient, dry parts only) +
  **rain ripples** (three ring-cell layers, 0.43–0.67 m, rings widening
  and fading over their life, amplitude × rain amount, stronger in
  puddles; time = the world clock 0x7FB718, `wet.w`: shots and flows show
  the rings at the scene's clock, e.g. 170 s under `NFSU2_RAIN_CLOCK`,
  not at time 0).
* The planar lookup moves with the slope (across: camera right; along the
  view: up/down) and is **blurred by roughness** (0.28 dry → 0.12 wet →
  0.015 puddle) with a **vertical stretch** (9 taps along the screen's
  vertical, ±len_x across; length grows toward grazing angles: the long
  wet-road streaks).
* Weight: the authored strength × (1 + 0.6 level) outside puddles, ≥ 0.9
  in puddles (mirror); the diffuse under a puddle × 0.75 at most. Nothing
  is lifted; the reflection is never dulled (no Fresnel unless Enhanced
  Reflections is on, whose own angle weight then applies as before).
* Pools and flares stay: they are part of the planar image.

## One wetness
The audit (`audit/gfx/AUDIT.md` §2 #33, §5 #6) found the road reading **dry in the rain unless SSR was
on**, and six wet effects each deriving their own wetness. Causes (measured, `rainharb` / `rain` at the
Modern preset, SSR off):
* `wet_surface` weighted every wet reflection by the texture's **authored** reflection alpha only
  (`alpha × (1 + 0.6 level)`): a road authored with little reflection (Coal Harbor's plaza road) got next
  to none, whatever the rain;
* with Enhanced Reflections on (every preset from Modern), its angle weight `0.35 + 0.65 (1 − n·v)⁵`
  multiplied the wet reflection's **colour** too, puddles included: Wet Roads alone showed more sheen than
  the Modern preset (rainharb right road box +4.7 vs +2.2);
* three puddle rules (`level` for Wet Roads, `√level` for Wet Road Interaction / Rain Next-Gen / the CPU
  splashes), two film curves (Wet Roads' `0.35 + 0.65 rain`, never 1 in Bayview's 0.7 showers; the
  spray's own `water_depth`), the raw `road_wet` for the spray rates, the car's spray film and the rain haze.

Now one source, `materials::world::wetness::Wetness` (CPU) and `shaders/enh/weather/wetness.wgsl` (its WGSL
twin, included by the world and the weather modules): **film** = road_wet × (0.35 + 0.65 · min(rain / 0.6, 1))
(full in a shower; **inferred**, the two old curves made one), **puddles** = √film, one puddle field
(`wetness_puddle`, the old noise and threshold), and the water film's Fresnel (`wetness_film_fresnel`,
Schlick, f0 0.02).

| effect | reads |
|---|---|
| Wet Roads (`wet_surface`) and the SSR mask (its weight) | film, puddles, the film's Fresnel |
| Wet Road Interaction (trail map, splashes, emitters) | film, `puddle_at` (√film) |
| Spray (rates), Wet Car (spray film) | film |
| Rain Next-Gen rings | film → `wetness_puddle` |
| Rain haze (wet-air weight) | film |

In `wet_surface` the reflection weight is now max(authored × Enhanced Reflections' angle, the film's
Fresnel (≤ `FILM_MAX` 0.6) on up-facing surfaces) outside puddles, the puddles' mirror as before; the
angle weight moved from the colour to the authored and puddle weights (the film keeps water's Fresnel);
the film's added reflection takes `FILM_DIFFUSE` (0.35) of itself from the diffuse (no lift; a full
energy split turned the lit kerbs dark: p95 −8..−13). SSR still replaces the planar term where it hits,
on top of a road that is already wet. Dry scenes take the native path as before (`wet.x` 0): the dry
enhanced shots are byte-identical (street Modern / Ultra / Wet Roads, evidence `docs/evidence/gfx-b3/`).
Steps unchanged (the spray, trail stamps and films step where and when they did; only what they read).

Acceptance (1280 × 960, HUD masked, against the Original of the same frame, whose road is the dry road:
the native road draws the same rain or not). Sheen = mean max(0, ΔY) in road boxes away from the car,
cov = % of the box lifted > 4 levels, hf = the box's high-frequency energy against the Original's
(speckle; < 1 = smoother):

| shot (SSR off unless named) | Δmean before → after | sheen L / R before → after | cov L / R | hf after |
|---|---|---|---|---|
| rain Modern | +1.10 → +1.22 | 2.7 / 1.6 → 21.5 / 1.7 | 18 / 10 → 66 / 12 % | 0.85 / 1.08 |
| rainharb Modern | +0.08 → −0.05 | 0.6 / 2.2 → 2.1 / 10.9 | 3 / 17 → 11 / 45 % | 0.99 / 0.98 |
| rain Ultra (SSR Half) | +3.29 → +2.59 | 5.5 / 5.8 → 16.8 / 3.6 | | 0.73 / 0.86 |
| rainharb Ultra | +2.46 → +1.52 | 2.7 / 5.5 → 5.6 / 11.2 | | 0.88 / 0.77 |

## Screen-Space Reflections (`render/ssr.rs`, `ssr.wgsl`)
1. **Mask** (`WorldRenderer::plan_wet_mask`, `draw_wet_mask`): right
   after the main view, its worldreflect draws once more (same vertex
   shader and order, `fs_wet_mask[_alpha_test]`) into two Rgba16Float
   targets and an Rgba8 one of the scene's size with a single-sample depth
   of their own: (the reflection term as the main pass added it,
   unweighted; weight × fog), (normal x, y; kind + roughness; view depth)
   and (the weight per channel / its largest). Independent of MSAA. The
   road part only when the road reflection is drawn (Road Reflection
   Detail on). **Cars**: after the car pass the cars draw once more
   into the same mask (`car::record_ssr_masks` for the batched pass,
   `render_sampled` for the shots; armed per frame by `plan_views` through
   `CarShared::arm_ssr_mask`; front-end lighting never): `car_ssr.wgsl`
   appended to car.wgsl (its own module; the cars' pipelines are
   untouched) writes car.efx's env term (the cube's colour, unweighted)
   and its weight 4·(EnvmapMin + fresnel·EnvmapRange)·r1.a (× the source
   alpha for the blended glass; additive and destination-alpha draws are
   left out), the world normal, view depth and roughness: 0.04, or with
   Enhanced Car Paint its type's (mix(0.10, 0.02, clearcoat) + 0.04·flake,
   chrome 0.015), widened by the normal's spread over the pixel. Kind in
   surface.z: road r, car 1 + r (normal z ≥ 0) or 2 + r.
2. In the post passes, after the resolve and SSAO (`PostFx::set_ssr`):
   scene **copy**; **depth pyramid** (R32Float linear view depth, nearest
   per cell, base = trace size (the scene texels under each trace texel,
   any ratio, ≤ 8 a side), ≤ 8 levels, sky = far away);
   **trace** at half or full size of the **screen** (under SSAA /
   Render Scale the scene is larger than the screen; tracing its own size
   doubled the cost inside Ultra, +2.1 ms vs +1.15 alone at 4K): reflected view ray in view space,
   clipped to the near plane and 150 m, walked on screen by a stackless
   hi-Z march (48 / 64 steps), hit within max(0.35 m, 4 % z) behind a
   surface; confidence = screen-edge fade × reach fade × fade for rays
   turning to the camera. From the road the road itself at its own depth
   is no hit (grazing rays); from a car its own body within 2 m of the
   start is none, the road and other cars are. The hit's colour: the
   scene's with **its fog taken off** ((c − fog·(1 − f)) / max(f,
   0.35), f = world.efx's factor at the hit's view depth; the planar
   capture it replaces is worldnofog and the composite's weight already
   holds the road's fog, so the fog was applied twice before), cars hit
   as drawn (unfogged); plus **the glow sprites** there (they go to the
   streak buffer after these passes, so `PostFx::apply` draws them once
   more, alone, into a scene-size target through the multisampled streak
   copy, × the streak buffer's steady state 255/(255 − decay) per channel,
   0x5CBB6A); for a car origin then fogged over the reflected path's
   length (its cube's views are fogged from the car). **filter**: 12-tap
   golden-spiral ellipse (3× taller than wide), radius from roughness,
   depth-, kind- and confidence-weighted (no history: no TAA needed);
   **composite**: where the mask's surface is still the front one (road:
   |linear depth − mask depth| ≤ 2 % + 0.15 m; car: the mask not behind
   the scene, as glass writes no depth), `scene += weight × tint ×
   (mix(planar, ssr, conf) − planar)` (planar: the road's capture or the
   car's cube), keeping max(…, planar) where the planar term is bright
   (flares, headlight pools). With Enhanced Reflections **off** the road's
   confidence is steepened (smoothstep 0.3–0.7): the native capture flips
   each prop about its own origin, so it and the true-mirror trace do not
   line up and a long crossfade showed both; the doubled band is now short.
   Alpha (bleach mask) untouched. Misses / off-screen rays keep the planar
   image.
**Overlays** (docs/gps-reflection.md): the GPS arrow, which the game
draws in the player's view only (0x631767), is flagged in the mask
(`fs_overlay_mask`, tint.a 1) and is no hit for any ray (`overlay_hit`).
`NFSU2_SSR_DEBUG=1`: the composite shows R = mask weight, G = confidence,
B = depth test passed (0.5) + car (0.5) (for tuning); `NFSU2_SSR_NOGLOW=1`
leaves the glow sprites out of the hits (a check aid).

**2026-11-05, exact; docs/perf-gpu.md "SSR, motion blur tiles, submissions"**: the passes run
only where the mask needs them, with the same shaders per pixel.
* The scene-sized **copy** and **composite** are depth-tested against the
  world's mask depth (`Mask::depth`, read only, compare NotEqual with
  `vs_mask`'s clear value): pixels the mask never drew are not shaded; the
  composite loads the scene (it returned `base` = the scene there). The
  trace now samples the scene itself (unchanged until the composite), so
  the copy only feeds the composite's `base`.
* `fs_linear` (the pyramid's base) also writes a trace-sized Depth16 mask
  (0: trace and filter, 0.5: filter only, 1: neither; the same tests as
  their early outs on the same mask texel) and the mask's kind and depth
  per trace pixel (Rg16Float, the f16 values as they are). The trace
  (`vs_trace`, z 0.25) and the filter (`vs_filter`, z 0.75) are tested
  against it (Greater): skipped pixels keep the cleared 0 they returned.
  The filter's 12 taps read the trace-sized kinds instead of the
  scene-sized surface (`kinds_tex`, `mask_texel(q + 0.5)` is the texel
  `fs_linear` read for pixel q). The scene depth is read four texels at a
  time (`textureGather`; the multisampled variant keeps its loads).
* The march: the level's size from the uniform (no `textureDimensions`
  per step), the ray depth at t carried from the step before, and the
  overlay test (up to 10 loads) only when it decides the hit.
* `NFSU2_SSR_FULL=1` (A/B aid): the copy and composite over every pixel
  as before; the debug views always do. A mask depth of another size or
  format falls back to that too.
* **A half-float scene (High Precision Post) takes the composite by the
  blend** (`fs_composite_add`, `ADD_CHANGE`): the composite's change
  (`composite_change`, the same code the copy's form adds) is returned
  alone and added onto the scene by ONE / ONE with the alpha kept, so the
  scene is not copied first. Not exact: the sum is rounded once by the
  blend rather than by the target's write: at most two 8-bit steps on
  0.03–0.28 % of the pixels of a 3840x2160 frame (one on nearly all of
  them), 1 − SSIM ≤ 2.3e-6, means unchanged; under FSR 1 the upscale's
  sharpening takes a few of them further (at most 12 steps on 3 pixels of
  29 million) (docs/evidence/perf-budget/ssr-composite.md).
  An 8-bit scene keeps the copy (its target would clamp a negative change).

### The mask from the main view
The mask pass drew the road a second time to work out `wet_surface` again, the term the main view had just
added: in rain the whole wet branch per road fragment (ripples, puddle noise, the trail map's tracks and wakes,
the planar capture's blur taps), and for the road the main view hides too (under the cars, behind buildings),
which its own depth does not know. Measured with the branch's parts left out one at a time (night rain,
`--bench drive` at clock 162; integrated GPU at level High, 1080p, 4 interleaved runs each; the player's
settings at 4K): mask 5.64 / 0.74 ms, of it 0.83 / 0.23 the road's raster and the three targets' writes alone;
without the ripples 4.14 / 0.65, without the puddle noise 4.50 / 0.68, without the trail map's terms 4.57 / 0.65,
without the eight planar blur taps 5.33 / 0.65.

Now, with the lamps' depth pre-pass at one sample (no MSAA), the main view's opaque worldreflect draws — one
run of its draw order, right before the first blended draw — are drawn in a render pass of their own between
the draws before and after them (the same order, the same EQUAL pipelines' depth test), whose second and third
targets are the mask's term and surface: the `_mrt` entries (`world_masks.wgsl`) give the colour as
`reflect_colour` does and those two from the same `wet_surface` (`WorldShared::mrt_pipelines`,
`WorldRenderer::mask_mrt_for`, `WorldCore::draw_road_with_mask`). The mask pass then loads them and draws the
road's depth and kind only (`fs_wet_mask_kind`; the overlays and the cars' mask as before), and a last pass
draws the road the main view hides (under the cars, behind a wall): tested against the main view's depth
(GREATER, LESS reversed: only what lies behind it is shaded), the road's frontmost fragment kept (the mask's own
depth, read: `fs_wet_mask_hidden`), its term and surface as the mask pass worked them out, so the SSR filter's
taps beside an occluder read what they read before. Only where it was measured to pay: a wet road (the dry
road's term is a few taps) on a discrete GPU (`WorldRenderer::mask_from_main_device`; on the integrated GPU see
below). Otherwise (an integrated GPU, a dry road, MSAA, no pre-pass, Road Reflection off, an order whose road is
not one opaque run) the mask pass works the term out itself, as before. Both sets of pipelines
are made by the drive's load (`warm_drive`; nothing depends on the weather),
`tests::first_use::the_mask_from_the_main_view_is_ready_before_its_first_frame`.

Pixels: the same bytes. The play setup in 4K night rain and level High at 1080p, 19 frames each (0, 60,
120, 200–215): identical to the mask pass's frames; 480 × 270, 120 frames
(`tests::fog_rain_paths::ssr_mask_from_the_main_view_keeps_the_frames`). Without the hidden road's pass the SSR
and RT filters' taps beside occluders lost its term: strips at the foot of walls and kerbs, up to 93 levels on
0.4 % of the pixels at 4K (measured, not kept).

Cost, night rain (`--bench drive` at clock 162), interleaved runs: the play setup at 4K, 7 runs: frame
22.72 [22.65..22.78] → 22.41 [22.34..22.50] ms (mask 0.738 → 0.220, of it the hidden road 0.17; main view
6.87 → 7.18). The reference integrated GPU at level High, 1080p, 7 runs (load 11–18): frame 68.78
[66.09..70.37] → 66.62 [63.76..68.72] (mask 5.51 → 0.92; main view 15.80 → 20.14): the passes it touches −0.25
ms only, and a loss on the dry road. The main view's extra cost there is the `_mrt` program, not how its values
leave it: the split pass with the old programs 15.9, the term alone (one target, half the bytes) 20.27, both
20.30, storage writes instead of targets 20.40 (`@early_depth_test(force)`), constant values in both 19.07; the
mask pass writes the same targets for 0.8. So the integrated GPU keeps the mask pass.

## Native byte-identity
Both rows Off: same pipelines, passes and targets; `Frame` grew by one
vec4 (zero) and worldreflect has one uniform branch. md5 against <base>
(`--shot`, 1280 × 960): see docs/evidence/wet-ssr/README.md. The reflections pass (1920 ×
1080 against <base>): Off identical in explore, rain, qrstart, motion,
intro, qrcars, race, drift, drag (main / career differ run to run in the
base too); Wet Roads in the dry explore / qrstart / motion identical to
Off.

## Cost (gpu ms/frame, `--bench explore 600`, details in the evidence README)
| | 1080p | 4K | iGPU 2-CU 1080p |
|---|---|---|---|
| Off | 1.03 | 2.36 | 12.63 |
| Wet Roads | +0.04 | ~0 | +1.7 |
| SSR Half | +0.52 | +1.20 | +14.5 |
| SSR Full | +0.54 | +1.98 | +20.4 |
Cost classes: Wet Roads low; SSR high (Ultra).

The reflections pass (gpu busy ms/frame, `--bench … 300`, reference desktop GPU, shared machine, one
run each, the base → the reflections pass; raw: docs/evidence/s2-reflections/README.md):

| scene, size | SSR Half alone | SSR Full alone | SSR Half inside Ultra |
|---|---|---|---|
| explore 1080p | +0.39 → +0.46 | +0.54 → +0.58 | +0.60 → +0.54 |
| explore 4K | +1.23 → +1.41 | +1.91 → +2.10 | +2.19 → +1.85 |
| race 1080p | +0.45 → +0.58 | +0.70 → +0.86 | +0.75 → +0.78 |
| race 4K | +1.60 → +1.98 | +2.75 → +3.21 | +2.93 → +2.79 |
SSR alone costs more (+0.04–0.46 ms; the most in the race at 4K, where
the cars fill the screen: cars traced, their mask pass, glow sprites drawn
again, kind-aware filter); inside Ultra (SSAA 2x) the trace at the
screen's size more than pays for that at 4K (−0.34 explore, −0.14 race).
Wet Roads: unchanged (the bench scenes are dry: the native path).

## Tests
Test: `weather::tests::road_wetness_follows_the_rain_only` (0 at load, the
native rule, the decay rate seen under Wine),
`render::native::world::pipelines::wet_roads_tests::wet_uniform_is_native_on_a_dry_road`,
`render::car::ssr_mask_tests::mask_module_appends_to_the_native_car_shader`,
`ssr::tests::pyramid_levels_and_trace_sizes` (screen-size traces under
SSAA / scale). `qol` `wet_roads_and_ssr_round_trip_and_native_default`, ranges in
`enhancements_round_trip` / `graphics_values_held_to_their_rows`; options
row order and stepping (`graphics_page_steps_and_restores`);
`ssr::tests::pyramid_levels_and_trace_sizes`; GPU (ignored, run
explicitly): `ssr::tests::traced_wall_reflects_on_the_mirror_road` (a red
wall over a mirror road at half/full size, 1× and 4× depth: the road turns
red, the wall and the alpha are unchanged, no validation error).

## Open
* Judged against the clone's native shots only (a deviation: nothing in
  the original to capture).
* SSR reflects what is on screen; the glow sprites are traced from this
  frame's sprites × the streak buffer's steady state, not from the buffer
  itself (its trails at speed are not reflected).
* The fog taken off a hit assumes world.efx's fog; additive (worldnofog)
  surfaces far in the fog are lifted at most 1 / 0.35.
* Car SSR: one layer per pixel (a blended decal or window replaces the
  paint's term under it); LOD Fade's dither is not in the mask (the new
  LOD's draws only); traffic's and rivals' cars reflect each other only
  when on screen. The rear-view mirror and the front end keep the cube.
* Enhanced Reflections off: the native per-prop flip still disagrees with
  the trace; the crossfade is only shortened (a true fix would need the
  true-mirror capture, i.e. Enhanced Reflections).

## Changes (2026-11-04, docs/chrome-flicker.md)
- **Dry road at native strength.** On a dry road (Wet Roads' uniform not on), a road ray's traced
  colour is held to the planar term's luminance. Cars hit and darker detail stay as traced.
- Measured as a share of the native reflection: SSR alone went from 1.24–1.48× to 0.87–1.15×, and the
  the max setup from 0.66–1.17× to 0.46–0.66× (Enhanced Reflections' weight).
- **Glow sprites.** They are no longer added at hits on a car's mask texel or within a 2–4 m path. A
  road ray under a car took its neon underglow's sprites, drawn as magenta streaks.
- `ssr::Mask::wet` carries the wet flag in `fog_color.w`.

## Ray Traced tier (docs/rt-reflections.md)
`enh.ssr 3` (Ray Traced (experimental), Vulkan discrete GPUs, after a restart): the trace pass's
ray-query variant completes the screen march's misses (off screen, behind the camera); filter and
composite as above, the chrome-flicker fix's dry-road hold included.

## The road's dry-road pipelines (registers)
The Wet Roads branch of `reflect_colour` (`if frame.wet.x > 0.5 { wet_surface … }`) runs only once rain
has wetted the road, but its terms set the road function's register allocation in every frame: with the
lamps' module the road's pixel shader needed 143 registers per lane on the reference desktop GPU
(Modern: 144 allocated, 10 waves per SIMD; Modern + RT: 153–161, 168, 9 waves) and 79–80 on the
integrated GPU at Medium (80, 12 waves); without the branch 115–116 (120, 12 waves), 123–125
(144, 10 waves) and 60–62 (64, 16 waves). So the main view's road draws that the depth pre-pass
covers (`DepthPrepass::equal`) also have a dry-road set (`DepthPrepass::equal_dry`, `LF_DRY`): the
same pipelines with the override `D9_WET` false, which compiles the branch out. A frame takes it when
its own wet.x is 0 (`ViewsPlan::dry_road`, decided where the uniform is written), so the branch it
leaves out is one that frame would not have taken: frames identical, dry and wet. Both sets are made
with the scene's pipelines (the load's warm-up, the menus' programs made ahead, `--prepare`): rain
wetting the road mid-drive switches sets without making anything (`first_use.rs`,
`rain_starting_mid_drive_makes_no_programs`). Six more pipelines per scene mode (two road keys,
three LOD Fade forms; cold, `--prepare` with the play setup's options made 359 instead of 353, +0.45 s);
the lamps' modules only (the native module's text is unchanged). Numbers:
`docs/perf-budget.md` "Modern + RT's traced shadows: the colour shader's registers".
