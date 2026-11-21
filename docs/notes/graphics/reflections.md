# Reflections: how it works

The technical part of [Reflections](../../playing/settings/reflections.md): how it works, what it costs, how it was checked and what is open. The player part (the rows and what they do) is on that page.

## Enhanced reflections and the HD HUD pack

Options → Display → Enhancements → Graphics → Enhanced Reflections switches both car and road enhancements. Native defaults remain off. The private HD-reflections package enables this for its copied settings; changing the row back restores native shading.

Car reflection direction, harmonic specular highlight and material Fresnel are evaluated per pixel using normalized interpolated surface normals. The original diffuse/material appearance remains. Enhanced car environment captures now use the main view’s mesh LOD and Detail Distance threshold, including scenery with no native low-detail mesh. Both single-face and multiview captures snapshot this policy before recording; native mode retains its low-detail capture. Resolution, near/far planes and the selected ordinary-frame face budget remain unchanged. Enhanced dynamic cubes now use fixed world XYZ axes, with matching world-space car lookup, so camera rotation cannot mix differently oriented old and new faces. The first enhanced capture, a resolution change in enhanced mode, or a change between native/enhanced bases refreshes all six faces once. Static frontend cubes retain their authored camera-relative convention. Extra captured geometry can increase GPU work. Wet roads use a horizontal planar scenery capture at the road height below the player, with main-view reflected mesh detail, a bounded nine-tap spatial filter and angle-dependent reflection strength. Authored material/vertex-alpha masks, headlight pools and already-mirrored lamp glows remain. The angle weight is artistic and preserves the original lighting style. This is a local planar capture, not SSR or ray tracing. It aligns scenery on the selected horizontal road plane; other elevations and sloping receivers remain approximate. Frontend scenes and missing/invalid road-height data retain the authored capture geometry.

Car Reflection Res, Road Reflection Res and Reflection Update Rate remain independent controls. These interact with SSAA and can add substantial GPU work. No FPS cap or physics change was introduced.

The package includes a private conversion of a third-party HD HUD pack:595 unique replacement textures from724 standard DDS sources. All595 match textures loaded by the clone, including names truncated in native metadata (resolved by full-name hashes).16 optional Unlimiter textures were excluded. Full decoded pixels/alpha match the source; mipmaps use alpha-weighted filtering. Original mod documentation and conversion manifest are included in the pack. See texture-packs.md for importing other packs.

The launcher enables the included pack unless NFSU2_TEXTURE_PACK is already set. To use original assets, launch with NFSU2_TEXTURE_PACK='' ./run.sh. Assets must be changed before launch; restart after changing the pack. Original game files and original project remain unchanged.

Validation:560 project tests plus one subprocess test passed;12 ignored tests include explicit GPU tests run separately as appropriate. Car GPU toggle changed743 pixels in its320×240 fixture and restored native pixels exactly. Road GPU tests cover native formula/restoration, masks, rotated surfaces, grazing response, filtering and shader variants.120 game runtime checks passed, including4K AA combinations. Native free-roam/race/drift/drag captures exactly match the previous AA build. Main-menu comparison differs only in its animated selection highlight. Menu/HUD captures visually inspected.

Car capture detail follow-up (2026-11-03): 453 non-ignored library tests passed with verified game assets; seven explicit GPU tests were ignored by that run. The dedicated environment-capture fixture passed separately with multiview enabled and disabled: 78 native versus 150 enhanced lit pixels, exact native restoration, and identical images between both paths. It covers a main-only mesh, a native-visible mesh, plan-time option snapshots, LOD B, 300% Detail Distance, and world-disabled face clearing. The existing road GPU regression also passed. These are synthetic capture images, not game-scene visual or performance measurements; arbitrary bridge/road-plane alignment remains outside that earlier change. See the coordinate follow-up below.

Car coordinate follow-up (2026-11-03): dynamic gameplay and rear-view cars explicitly bind the world cube convention; static frontend cubes remain camera-relative. Fixed axes remove camera-rotation seams, but partial updates still contain imagery from different capture positions/times as the car or scenery moves. This change does not provide temporal reconstruction, parallax correction, or independent captures per car.

Coordinate validation: three explicit GPU tests passed serially: RX8 with an asymmetric six-color cube (dynamic/static binding and native restoration), synthetic capture with camera yaw/pitch and capture translation (partial update matches the corresponding full-capture face), and the existing road shading regression. The capture test also passed with multiview disabled. Mode transitions and enhanced resolution reallocation refresh six faces once; later calls respect the requested one-face budget. World-disabled clearing remains covered. `cargo check --bin game` passed. Fixtures and exact commands are in the maintainer's private capture; these are correctness fixtures, not gameplay performance measurements.

Wet-road plane follow-up (2026-11-03): gameplay and deterministic world screenshots supply road height below the player only while the option is enabled. Both transformed props and baked world geometry use the same complete-world mirror. CPU culling uses the reflected camera and original bounds; native mode retains its original local-matrix flip and mirrored bounds. Scenery on/below the receiver is clipped with a 5 mm tolerance to keep the road surface from occluding the reflection. Headlight pools and lamp glows then draw with a separate copy of the authored capture frame, preserving their original placement and clipping behavior. No additional reflection target or draw pass is added. Plane clipping uses dedicated planar fragment entry points; the native world fragment path retains its original early-depth behavior. Performance has not been claimed equivalent from pixel restoration alone.

The plane is local and horizontal, follows the player's queried ground height, and does not identify every receiving road surface. Slopes, remote bridge decks at another elevation, sudden height changes, off-screen visibility exclusions, and the existing absence of world cars from road captures remain limitations. Frontend car/floor reflection placement is unchanged.

Road validation: 496 library tests passed (11 explicit tests ignored); all four reflection GPU tests passed separately. The local-plane fixture covers transformed/baked geometry equivalence, geometric placement, reflected-camera culling, below-plane clipping, missing/invalid ground fallback, native restoration, and independent authored pool/glow draws. At 1920×1080, native race and explore PNGs exactly match the car-only commit; enabled-vs-enabled comparisons against that exact commit isolate road changes to 115,440 race pixels and 401,084 explore pixels, confined to the road region. The race native PNG also matches the retained pre-enhancement build. See the maintainer's private capture for commands, images and limitations; no FPS or GPU-time improvement is claimed.

## Wet Roads and Screen-Space Reflections

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
capture ("Enhanced reflections and the HD HUD pack" (above)). The planar image stays the base on
every setting and every GPU.

### Wet Roads (`render/world.wgsl` `wet_surface`, the main view only)
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

### One wetness
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

### Screen-Space Reflections (`render/ssr.rs`, `ssr.wgsl`)
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
   is no hit (grazing rays); from a car its own body (within 2 m of the
   start's depth) is none only while the ray grazes the surface it starts
   on, and blocks it otherwise (see "A car's own body in its reflection");
   the road and other cars are hits. The hit's colour: the
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
**Overlays** ("The GPS arrow kept out of reflections" (below)): the GPS arrow, which the game
draws in the player's view only (0x631767), is flagged in the mask
(`fs_overlay_mask`, tint.a 1) and is no hit for any ray (`overlay_hit`).
`NFSU2_SSR_DEBUG=1`: the composite shows R = mask weight, G = confidence,
B = depth test passed (0.5) + car (0.5) (for tuning); `NFSU2_SSR_NOGLOW=1`
leaves the glow sprites out of the hits (a check aid).

**2026-11-05, exact; docs/notes/performance/gpu.md "SSR, motion blur tiles, submissions"**: the passes run
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

#### The mask from the main view
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

### Native byte-identity
Both rows Off: same pipelines, passes and targets; `Frame` grew by one
vec4 (zero) and worldreflect has one uniform branch. md5 against <base>
(`--shot`, 1280 × 960): see docs/evidence/wet-ssr/README.md. The reflections pass (1920 ×
1080 against <base>): Off identical in explore, rain, qrstart, motion,
intro, qrcars, race, drift, drag (main / career differ run to run in the
base too); Wet Roads in the dry explore / qrstart / motion identical to
Off.

### Cost (gpu ms/frame, `--bench explore 600`, details in the evidence README)
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

### Tests
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

### Open
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

### A car's own body in its reflection
A ray from a car pixel used to skip **every** texel of its own car within 2 m of the start's depth. Meant for
the grazing self-hits of interpolated normals, it also let a trunk-lid ray pass through the wing, the rear
window and the roof and take the lamps and sky behind the car on screen: a bright blob between the wing's
legs that changed shape and place with every subpixel of camera motion (Ultra and up; High has no SSR).

Now the own body is skipped only while the ray grazes the surface it starts on: the ray has not yet been in
front of the depth pyramid (`escaped`) and is less than 0.1 m behind the body (`SELF_GRAZE`). Once the ray has
left the surface, or meets the body further behind (up to 2 m, `SELF_BEHIND`, where a ray behind something
thin would otherwise go on), the body occludes it: a miss, confidence 0, the car cube stays there, as at High.
Other cars and the road are hits as before.

Measured with `--bench explore 160` (near chase camera, every frame dumped), the RX-7 with the big wing from
the community body-shop save, `scripts/flicker.py` acc>16 over the wing and lid (280×80+500+620): Ultra 2.10 → 1.70 % (SSR off 1.45),
Extreme 2.02 → 1.68 % (1.47); held still 0.49 → 0.30 % (SSR off 0.29). The blob is gone; what is left above the
SSR-off level is the car's wanted SSR (quarter panels, bumper, taillight glow on the body), which stays. Stock 240SX
unchanged; the stock Sentra's wing had a smaller patch of the same (Ultra 3.30 → 2.73 %). Evidence:
`docs/evidence/ssr-own-body/`. Original, Balanced and High
frames are byte-identical (no SSR); SSR trace time unchanged (0.12 ms Ultra, 0.32–0.33 ms Extreme at 1280×960).

### Changes (2026-11-04, docs/notes/graphics/car-look.md "Chrome trunk flicker and dry-road reflection strength")
- **Dry road at native strength.** On a dry road (Wet Roads' uniform not on), a road ray's traced
  colour is held to the planar term's luminance. Cars hit and darker detail stay as traced.
- Measured as a share of the native reflection: SSR alone went from 1.24–1.48× to 0.87–1.15×, and the
  the max setup from 0.66–1.17× to 0.46–0.66× (Enhanced Reflections' weight).
- **Glow sprites.** They are no longer added at hits on a car's mask texel or within a 2–4 m path. A
  road ray under a car took its neon underglow's sprites, drawn as magenta streaks.
- `ssr::Mask::wet` carries the wet flag in `fog_color.w`.

### Ray Traced tier

Described in full under "Ray-traced reflections (experimental)" below.

`enh.ssr 3` (Ray Traced (experimental), Vulkan discrete GPUs, after a restart): the trace pass's
ray-query variant completes the screen march's misses (off screen, behind the camera); filter and
composite as above, the chrome-flicker fix's dry-road hold included.

### The road's dry-road pipelines (registers)
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
`docs/notes/performance/budget.md` "Modern + RT's traced shadows: the colour shader's registers".

## Ray-traced reflections (experimental)

A **clone deviation**, nothing native and nothing traced in SPEED2.EXE: the
original has only the planar road capture (view 4) and the car's cube
(docs/formats.md "Road reflections", "Wet Roads and Screen-Space Reflections" (above)). Defaults and the
Original preset are unchanged and byte-identical (below).

| row | new value | options.txt |
|---|---|---|
| Screen-Space Reflections (Reflections group) | **Ray Traced (experimental)**, after Ultra | `enh.ssr 3` |

`qol::ssr()` gives 3 only while the device was made with ray queries,
else 2 (SSR Ultra); `qol::rt_reflections()`. The tier is greyed out
(`offered`) unless the adapter can trace (`rt::adapter_can`: Vulkan,
discrete GPU, `EXPERIMENTAL_RAY_QUERY` offered; `NFSU2_RT=1` any Vulkan
adapter, `NFSU2_RT=0` never). No new label id (a tier of the SSR row).

### The rework in one table

The ray-tracing rework (the sections "Textured, glowing hits", "Light glows in the reflections",
"Quarter rate" below, the NIS characters' rays in the world's traced-shadow pass, docs/notes/performance/budget.md,
and the hit textures built on the GPU) against the tree before it, the play setup at 4K
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

### API status on the reference desktop (verified 2026-11-05)
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

### How it works (`render/rt.rs`, `render/rt.wgsl`)
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
  already hold every street lamp's pool (docs/notes/graphics/lamps-shadows.md "Pools too bright"), so the
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

### Textured, glowing hits (prototype)
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
* Measured (the play setup at 4K, 4 interleaved pairs, load 12-20): racing frame
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

### Light glows in the reflections (prototype)
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
Measured (the play setup at 4K, 3 interleaved triples, load 11-16): racing (dry)
13.99 / 14.14 / 14.28 ms (today / textured / + glows), rays 0.99 / 1.16 / 1.12; night rain
16.98 / 17.33 / 17.54, rays 1.10 / 1.34 / 1.67. Pictures, night-rain race start frames 100-111 vs
today: luma mean +0.3 … +0.6, p5 equal, p95 +1.1 … +3.0; 3-7 % of the pixels > 3 % at the wet spots;
frame-to-frame change +0.26 % of the pixels (no added flicker; no history is kept).

### Quarter rate (prototype)
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
Measured (the play setup at 4K, 3 interleaved triples, load 8): racing 13.88 today →
14.14 (glows) → **12.72** ms; night rain 16.73 → 17.51 → **15.50**: the trace 1.85 → 0.71 ms, its rays
1.52 → 0.46, the resolve 0.33. Pictures against the full rate (frames 100-111): rain 0.93 % of the
pixels > 3 %, racing 0.12 %, luma mean / p5 / p95 within 0.1; frame-to-frame change −0.4 % of the
pixels in rain (the history steadies the reflections); at 200 % the wet road's finest ripples are a
little softer.

### Primary on wet roads and cars (prototype)
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

Measured (the play setup at 4K, Modern + RT, 4 interleaved pairs against the tree before,
load 4-9, GPU busy ms): with the traced shadows' changes of docs/notes/graphics/shadow-softness.md "Every light"
(which pay for the dry frames' rays) racing 12.32 → 12.31, night rain 15.03 → 14.74, race start
14.13 → 13.94, free roam 9.12 → 9.31. The reflections alone: night rain −0.6 ms (the march gone: trace
0.72 → 0.02, rays 0.50 → 1.12 with the march where the screen is short), dry frames +0.1 ms of rays
(the cars). Pictures at the four wet spots against the march-first form (Modern without RT, so only
the reflections differ): 1.6-5.9 % of the pixels > 3 %, luma mean −0.3 … +0.2, p5 equal, p95 −0.5 …
+1.1. Night-rain race start frames 100-109: 2.3 % > 3 %, mean −0.15, p95 −1.1, frame-to-frame change
30.61 % vs 30.68 % of the pixels (no added flicker). The quarter rate's difference to the full rate on
the wet road (the lower half): mean 0.99 vs 1.01 levels, about equal on this bench (its road is rough
enough for the surface's reprojection to dominate). Crops of before and after are kept with the maintainer's private evidence.

### The ray list (`rt/trace.rs`: `list_trace_source`, `list_rays_source`, `ssr_trace_list`)
A ray query in a fragment shader keeps its traversal stack in scratch
memory on RADV (76 entries per lane, `radv_nir_lower_ray_queries.c`; in a
compute shader a 16-entry stack in workgroup memory), which every wave of
the trace pays whether its pixels trace or not; RADV also compiles any
shader holding a ray query in the ray-query wave size (32), and this
pass's march is faster in the pixel shaders' 64 (measured, docs/notes/performance/budget.md
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
  (docs/notes/performance/budget.md "Ray-traced reflections: the rays from a list",
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
  the first traced frame (docs/developing/first-use-builds.md). A trace too large for
  the list on a device that otherwise takes it (`list_fits`: over 32767 a
  side or the buffer limits) would make the fragment form's in its frame.
* DX12: the tier is Vulkan-only (`rt::adapter_can`); both forms compile
  for Windows, neither can run there.

### Cost (gpu busy ms/frame, median, `--bench … 300`, 2 runs each, Wet Roads on; internal evidence)
| scene | 1080p SSR Ultra → RT | Δ | 4K SSR Ultra → RT | Δ |
|---|---|---|---|---|
| explore | 1.31 → 1.56 | +0.25 | 4.40 → 5.19 | +0.79 |
| racing (cars) | 1.52 → 2.03 | +0.51 | 5.35 → 6.89 | +1.54 |
| rain (clock 170) | 1.37 → 1.78 | +0.40 | 4.93 → 6.04 | +1.11 |
The trace pass: 0.30–0.39 → 0.64–0.90 ms (1080p), 1.09–1.39 → 1.97–3.0 ms
(4K). VRAM peak about +350 MB at 4K (noisy metric). Draw-thread CPU
+0.1–0.15 ms in racing (car keys hashed per frame). Tier cost: 3.3 ms at 4K
(over Off). Shared machine (load avg ~50–110), reference desktop GPU.
With the ray list (above; the play setup at 4K (SSAA 2x) with Modern + RT, 7 runs per
arm): the trace 3.17 → 2.55 ms racing, 3.31 → 2.99 race start, 3.62 → 2.91 rain, 2.79 → 2.45 free
roam; the frame 19.02 → 18.55, 20.27 → 19.95, 23.45 → 22.87, 13.06 → 12.74 ms; 27–45 % of the trace's
pixels need a ray (docs/notes/performance/budget.md "Ray-traced reflections: the rays from a list").

### Images (`evidence/rt-reflections/`, 1280 × 960 `--shot`, left SSR Ultra, right RT, Wet Roads on)
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

### Native byte-identity
With every row native, `--shot explore / intro / qrcars / qrstart / race /
drift / drag` are md5-identical with and without a ray-query-capable
device run (`NFSU2_RT=0`); SSR Ultra (`enh.ssr 2`) identical in intro and
qrstart likewise. The Original bench (racing 1080p, 600 frames, 3 runs
each) shows no GPU difference (0.75–0.76 ms both). Not run: `perf.sh
parity` against a base build (the default path does not reach any new
code: the feature is not requested, `ssr ≤ 2` takes the old pipelines).

### Tests
`render::rt::tests`: `trace_source_hooks_every_anchor_once` (each anchor in
ssr.wgsl once; the RT module parses and validates in naga),
`instance_transform_is_row_major_3x4`, `pack_halves_and_clamps`,
`bayview_world_geometry_fits` (game data; skipped without NFSU2_ROOT: >
100 k triangles, within the 24-bit custom index with the cars' room);
`qol::tests::ray_traced_tier_falls_back_to_ultra_without_ray_queries`;
`enh.ssr` clamps at 3. GPU check: `cargo run --release -p devprobes --bin rt_probe` (maintainer only (private developer probes, not in the public repository)).
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

### Risks / open
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

## The GPS arrow kept out of reflections

Issue: the GPS arrow showed in the reflections. The play setup
(the play setup's options file) has Screen-Space Reflections Full, Wet Roads and
Enhanced Reflections on.

### Native (traced, SPEED2.EXE)
- MARKER_GPS_AID is drawn by 0x5F3CC0 → 0x5EACA0 → 0x5C5940. Its only caller is World_DrawCars
  0x6316E0, at 0x63177A, and only for the view whose number (view +4) equals the player's
  ([player +0x18], compared at 0x631767). 0x5F3AF0 (called just before it) and the dyno rollers
  0x56E900 are behind the same test.
- The frame function 0x5CAAE0 draws view 4 (the road reflection) with 0x5D6AE0, 0x566410, 0x631260,
  0x630DF0, the pools 0x6154A0, 0x5C7E50, then the glows 0x491860 / 0x61AEA0 / 0x61ACF0 / 0x494270
  (block 0x5CAB03–0x5CACB5). The cube faces get 0x60F890 and 0x5D6AE0 (0x5CAD01–0x5CADE9). 0x6316E0
  is in neither, so **the arrow is never in the road reflection or in the car cube**.
- The particle draw Particles_Render 0x48BDF0 (marker icons and rings, the outrun challenge marker,
  smoke) is in neither block either: called at 0x5CAEC8 / 0x5CAED3 (the 0x6316E0 view-3 block) and at
  0x5CB541 / 0x5CB54C (the main view).
- Under Wine (fastgpu, DXVK; a new career's airport free roam with the GPS on the car lot): the arrow
  is above the screen centre and the rear glass and the road show no blue
  (`rear_window_before_after.jpg`, top left).

### The clone
- The planar road capture (native or Enhanced), the cube and the rear-view mirror already culled the
  arrow. Its placement goes in with view flags 0, and the 0x40 of those views culls it
  (`WorldView::culls`). Test: `gps_aid_is_drawn_in_the_main_view_only`.
- **Leak: SSR.** The trace reads the main view's colour and depth, where the arrow is. Car rays (the
  rear glass) and wet-road rays hit it. With the play setup the 350Z's rear window showed the
  arrow (`rear_window_before_after.jpg`, middle left). It was not the glow sprites: still there with
  NFSU2_SSR_NOGLOW=1. With SSR off it is gone.
- **Fix (SSR only):**
  - `WorldCore::overlay_draws` holds the arrow's draws (the placement's groups, `World::gps_aid`).
  - `draw_wet_mask` draws them first into the SSR mask with `fs_overlay_mask` (world.wgsl). That
    writes no reflection term, the view depth, and tint.a = 1, and writes the mask's depth, so the
    road and cars behind the arrow keep the flag. It runs whenever the mask is drawn, also with Road
    Reflection Detail off (car SSR).
  - In `ssr.wgsl` `fs_trace`, a hit is no hit when an overlay is at or in front of the hit's depth,
    on the hit texel or one trace texel away (`overlay_hit`: bilinear lookup and resolved edges). The
    ray goes on past the arrow as past something thin. Misses keep the planar or cube image, which
    never holds the arrow.
- Native and SSR-off paths are unchanged: the overlay pass exists only in the SSR mask pass. The
  native `--shot intro` was byte-identical before and after (max diff 0). With the play setup,
  the only pixels that changed were the rear window (bbox 549,558–669,623 of 1280×960).

### Other markers (checked)
- The event, shop and challenge markers, tips and banks are particle effects (`Game::marker_effects`,
  0x6054C0). The clone draws them in the main view only, as the original does. They write no depth,
  so SSR cannot hit them. A ray that hits the surface behind one could take its colour.
  - `--shot marker`, `prompt` (car in the trigger) and `shopmarker`, dry and in rain
    (NFSU2_RAIN_CLOCK=170), SSR on vs off: the diffs show only the reflected buildings and cars, with
    no marker shape on the road or the car.
  - **Open:** not excluded. Flagging additive particles would need their own mask draw. The scenes
    above show no leak.
- Route arrows and barriers are world scenery (native view flags), and the original reflects them as
  well.

### Evidence
the maintainer's private captures:
- `rear_window_before_after.jpg`: the original; the clone native; the play setup before / after;
  the same in rain.
- `intro_user_full_before_after.jpg`.

### Tests
- `render::native::world::reflection_tests::gps_aid_is_drawn_in_the_main_view_only`
- `render::native::world::reflection_tests::overlay_draws_are_the_placements_group_draws`
- `render::ssr::tests::overlay_is_left_out_of_traced_reflections` (GPU, ignored: a wet road reflects
  a wall; flagged as an overlay it does not)
- `traced_wall_reflects_on_the_mirror_road` now runs on a wet road. It had failed since the chrome-flicker fix's dry-road
  hold.
