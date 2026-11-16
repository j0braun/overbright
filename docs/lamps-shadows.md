# Dynamic Lamps, Lamp Shadows, Contact Shadows (clone deviations, opt-in)

Wave G the lamps-and-shadows work (roadmap `research/roadmap-2026-11-04.md`, research report
item 4). Three Graphics rows in the Lighting group (`frontend/graphics_table.rs`,
with their tiers' help and costs), all off by default (native,
byte-identical). Presets: Modern = Dynamic Lamps On, Lamp Shadows Low,
Contact Shadows On; Ultra = Lamp Shadows High; Original / Restore Native
off.

| row | values | options.txt |
|---|---|---|
| **Dynamic Lamps** | Off / On | `enh.dynamic_lamps 0/1` |
| **Lamp Shadows** | Off / Low / High | `enh.lamp_shadows 0/1/2` |
| **Contact Shadows** | Off / On | `enh.contact_shadows 0/1` |

`qol::dynamic_lamps()`, `qol::lamp_shadows()`, `qol::contact_shadows()`.
Nothing here is native or traced: SPEED2.EXE lights the world with baked
vertex colours only (world.efx `tex × v0 × 2`), the cars with the light
packs' harmonic (0x48E3D0 → 0x4893A0, `render/lighting.rs`), and has no
dynamic shadow but the car's shadow texture (SHADOWIG/SHADOWFE, 0x6162F0).

## What the light packs hold (verified by reading the data, the maintainer's probe `lamp_census`)
Bayview (L4RA) has 2234 light-pack lights (0x135003, `formats/lights.rs`):
2009 of (150, 150, 150), 207 of (110, 110, 125), 17 warm (125, 110, 95)
with radius 40 / inner 20, all **point lights** (kind 2, sub 2, on,
intensity 1, outer 30 m, inner 10 m, direction straight down), plus A21's
"moon" (40, 40, 55, radius 9999.99). 2094 of them sit within 3 m of a
type-9 (street lamp) flare of the flare packs (2121 flares, all type 9).
So the packs are the **street lamps**; there are **no neon or shop-sign
lights** in them (neon and signs are emissive textures, the glow sprites
and the baked vertex colours, which this work leaves alone). The moon is
left out (radius > 200 m: the vertex colours already hold it).

## Dynamic Lamps (`render/lamps.rs`, world.wgsl LAMPS blocks, car.wgsl `car_lamps`)
* **Clusters** (CPU, each frame): the lamps of the drawn sections (the
  native visible list, as the world draw), within 450 m and inside the
  view (sphere vs frustum), nearest 1024; 16 × 9 screen tiles × 24
  exponential depth slices (1–600 m); each cluster lists the lamps whose
  sphere's box reaches it. Uploaded as storage buffers (group 0 bindings
  3–4 of the world views).
* **World** (main view only): per pixel, the cluster's lamps with the
  **native weight** (0x4893A0: 1 inside the inner radius, falling
  linearly in d² to 0 at the authored outer radius), squared (exactly 0
  at the outer radius, a pool rather than a flat lift where the 30 m radii
  overlap along a street), × N·L (face normal), × the authored colour ×
  0.45. It is applied as `light = v0 · (1 + 2 · min(lamps, 0.35) · headroom)`,
  headroom = 1 − lum(v0) / 0.5 (below): the lamp
  brightens and tints what the baked vertex colours already light, so a
  surface the art left dark stays dark and the baked hue is kept (a plain
  add of white light washed the grade out at the race start; tried and
  dropped). Fogged draws only (world.efx / worldreflect); the additive
  worldnofog textures, sky and glows take no light. Wet Roads keeps its
  own reflection term on top.
* **Cars**: `CarLighting::in_world` picks the four lamps with the largest
  native weight at the car; car.wgsl adds them per pixel (weight², N·L,
  through the material's diffuse scale) to `r1` — on top of the native
  harmonic, which already holds the same lamps averaged.
* High precision path: the term is not clamped, so with High Precision
  Post the scene keeps the overbright part; NFSU2's own grade (the post
  passes) runs after it as before. Glow sprites and streaks unchanged.

## The cluster loop (lamps.wgsl `lamp_light`, a speed change: the same pixels)
`lamp_light` finds the pixel's cluster and adds its lamps (`cluster_light`, in the cluster's order). Where
the device has subgroup operations (wgpu `Features::SUBGROUP`, asked for in `Gpu::open` where the adapter
offers it) the world modules take the loop's subgroup form (`#if SUBGROUPS`,
`registry::subgroup_clusters`): the first active lane's cluster index is broadcast
(`subgroupBroadcastFirst`), the lanes in that cluster add its lamps and leave, the others take the next
lane's cluster. The cluster's index is then the same in every lane that adds it, so its count and lamp list
are scalar loads (one per subgroup, not one per lane; the scalarised light loop of idTech 6 and of Drobot,
"Improved Culling for Tiled and Clustered Rendering", SIGGRAPH 2017). Every pixel still adds its own
cluster's lamps in the same order with the same operations, so the sums are the same bits.
* At most 128 rounds (wgpu's largest subgroup): each round ends at least the first lane's cluster, so
  every lane the broadcast includes is done by then. The bound only stops a lane the broadcast leaves out
  (a helper lane where wave operations skip helpers, DX12's default); a helper's light reaches no pixel
  and no derivative (the coverage path's `fwidth` reads the alpha, which the lamps leave alone).
* The form is a property of the device, chosen when its first world module is made, never per frame.
  A pipeline constant could not choose it: wgpu validates a module against the device's capabilities
  before any constant applies, and the subgroup builtins need the capability.
* Without subgroups the plain loop (each lane its own cluster): adapters that do not offer them, and DX12
  through FXC (shader model 5.1; wgpu offers `SUBGROUP` on DX12 only from shader model 6.0 with wave
  operations, i.e. through DXC). `NFSU2_GPU_DROP=subgroup` makes a device without them (A/B, checks). The
  bench's `device:` line says which form runs.
* With `SUBGROUP` on a Vulkan device wgpu also sets `ALLOW_VARYING_SUBGROUP_SIZE` on every pipeline stage.
  The driver's pipeline cache from a build without it therefore misses once; RADV picks the same wave sizes
  for shaders without subgroup operations.
* Proof: the composed modules (`validate_tests::the_cluster_loop_takes_subgroups_only_where_the_device_has_them`:
  the plain form validates without the capability, the subgroup form only with it, in the fragment stage,
  and the per-cluster sum is one text); free roam and a city circuit drawn the same in both forms at High
  and Ultra (`tests::first_use::the_cluster_loop_forms_draw_the_same_and_are_ready`, which also finds no
  program made in their frames); the bench's frame dumps at Medium and High (racing, race start, rain)
  byte-identical on both GPUs; the gate. Speed: docs/perf-integrated-gpu.md "The cluster loop".

## Lamp Shadows (`plan_lamps`, `record_lamp_shadows`)
* A Depth32Float **2D array**: Low 512², 4 lamp layers; High 1024², 12
  lamp layers; + 1 layer for the **player's headlight** projector
  (view 9's frustum from `headlight_uniform`, redrawn every frame).
* Lamp layers: the nearest downward lamps (all of Bayview's) within 70 m
  of the eye, from the lamp, looking down, 140° wide, near 0.6 m (past
  the lamp's head), far 1.5 × the outer radius. **Static scenery cached**:
  a lamp keeps its layer while it stays among the nearest; a layer is
  drawn only when it gets a new lamp (at most 2 a world tick, docs/uncapped-fps.md "Enhancement effects on ticks"; was 2 a drawn frame). Drawn with the
  main view's LOD, opaque and alpha-tested draws (holes kept), depth bias
  (2, slope 2.5), blended and sky draws left out.
* **A scenery switch redraws the layers it touches** (`update_placement` →
  `release_lamp_layers_seeing`): when a drive's set-up shows, hides or
  moves a placement the layers draw (a draw of its group neither blended
  nor the sky) and its bounds before or after lie in a held layer's view
  (`lamp_view_holds`: its frustum), that layer is given up; the next step
  of the layers hands it out again (the lamp still wanted) and draws it with
  the scenery as it is now, fading in, as a fresh load's layer. The other
  layers are kept as drawn. Only a set-up calls it: nothing per frame. In
  the city the barriers a race or free roam switches near the race grids
  are blended (barrier tape, the start line's graphics), so no layer is
  redrawn there; on the drag strips, URL circuits and drift tracks the
  set-ups switch road pieces, walls and markers the layers draw.
  `tests::kept_world::lamp_layers_follow_a_kept_drives_barriers`: free roam
  → a city circuit → free roam on the spot (every layer kept), L4RG's URL
  circuits 4701 → 4703 (3 of 4 layers drawn again) and L4RB's drag strips
  4222 → 4223 (1 of 2) from one grid on the world kept; each layer then
  holds the depth a layer drawn afresh holds (without the redraw, the URL
  circuits' three held their first race's), and the frame a second later
  equals the frame with the lamps started afresh.
* **Cars dynamic**, as soft ellipsoids (each car's `shadow_box`, × 1.15)
  between the pixel and the lamp / the headlight's eye, at most 32 cars,
  the nearest first; the player's own car is skipped for its headlight.
* Lookup: normal offset + an offset toward the light (grazing headlight
  angles), 1 bilinear compare (Low) or 3 × 3 (High), faded out at the
  frustum's edge. **Partial only**: a lamp layer removes at most 70 % of
  that lamp's added light, a car 60 %, the headlight layer 70 % of the
  headlight's light. The baked light is never shadowed.
* Without Dynamic Lamps only the headlight layer and the cars act (on the
  headlight projector's light).

## Contact Shadows
Per world pixel (main view, fogged draws, also characters drawn through
the main view's group), for each car within 4 m: a soft box occlusion in
the car's axes — full inside 85 % × 80 % of the footprint, fading out by
0.35 m + 0.15 × the half width; full at and under the box's bottom, gone
half the box's height above it and 0.9 m under it; at most 50 % darker.
It darkens the world under the cars *before* the native shadow texture
is blended over it (that pass is unchanged), so it never replaces it.
**docs/glow-darkening.md**: now full inside 75 % of the box and 0 at its edge; a car's
underglow takes it back by its light; the native shadow texture over it gives the darker of the
two (`fs_car_shadow`), not their product.
Overlap with the native **Car Shadow** switch (Customize Display,
g_CarShadowEnable): both ground the car, but they are separate effects
(the native texture vs an occlusion term); Contact Shadows works with the
native shadow on or off.

## Per-pixel work (the culling per cluster)
Every lit world pixel finds its cluster and walks the cluster's lamps; per
lamp (with Lamp Shadows) it multiplied a factor for **every** car (up to 32)
whose soft ellipsoid lies between the pixel and the lamp, the player's
headlight did the same once more, and Contact Shadows tested every car
again. Most of those cars cannot touch the pixel's cluster at all. The CPU
now works that out once per frame per cluster (`lamps::car_masks`,
`render/enh/lighting/lamps/car_masks.rs`) and the pixels test only the cars
left, in the same order, so every product is bit for bit the same:

* **The clusters' bounds** (`cluster_spheres`): each cluster's bounding
  sphere from the tiles' corner rays (the inverse view projection, so
  jitter and a rolled camera are included) and its slice's depths, the
  first slice from the eye, the last to 10 km; 2 % + 5 cm wider against the
  GPU's rounding of a pixel's tile and slice.
* **A lamp's cars** (a word beside each entry of a cluster's list, at the
  entry + `LAMP_CAR_MASKS`): the points a lamp lights in a cluster lie in
  the cluster's sphere (or the lamp's reach, whichever is smaller); every
  segment from the lamp to them lies in the capsule of that radius around
  the segment to the sphere's centre. A car is kept if its shading
  ellipsoid (`closest` below 1.3: a sphere of 1.3 × 1.15 × its largest half
  extent) can meet that capsule and its 12 m reach the cluster; past
  `closest` 1.3 `smoothstep` gives exactly 1 and the factor `mix(0.4, 1, 1)`
  is exactly 1.
* **Modern Lighting's cars**: its relight tests the cars gathered near the
  pixel (at most four) for every light; it takes the same entry's cars, so
  a gathered car that cannot shade the cluster from that light is skipped
  (its factor is exactly 1 past the same `closest`).
* **The headlight's cars** (`HEADLIGHT_CARS`, a word per cluster): the same
  test from the player's headlight eye to the cluster's sphere, never the
  player's own car (`headlight_visibility`, the player's beam).
* **Contact Shadows' cars** (`CONTACT_CARS`, a word per cluster): the cars
  whose 4 m reach touches the cluster. The Modern Lighting path took the
  contact shadow twice per pixel; it is taken once.
* **The lamp lists** (`build_in`): a lamp is listed only in the clusters of
  its screen box whose sphere its own sphere reaches (the box's corners
  and the slices beside the sphere held a third of the entries). A lamp
  cannot light a point outside its sphere, so a pixel's sum is the same.
  Not with Modern Lighting Low, whose pixels take the first four of a list,
  reached or not.
* **The lamp loop's early outs** (exact): a lamp outside its spot cone or
  behind the face adds nothing and takes no shadow lookup (unless Surface
  Detail's specular and light direction take it); the loop ends once every
  channel of the sum reached `LAMP_MAX` (`lamp_shade` takes at most that, the
  terms are never negative); no lamp loop where the bake has no headroom.
* **The beams' first test** (exact): `headlight_clip` is above 0 exactly
  where its texel position lies between 32 and 48 (the clip texture's lit
  run, texels 33–47), so the beam loop tests that position and the
  projector's uv first and reads the two clip texels only for the pixels
  a beam lights (`the_beams_clip_range_is_where_the_clip_is_lit` checks
  every f32 around both ends).

The cars per lamp, per pixel on the bench frames (desktop GPU, a counting
build; the counts include fragments the depth test hides later, that build
having late depth tests): racing 13.9 → 0.2 car tests, contact 16 → 0.24,
lamp entries 2.8 → 1.9; the cars that do shade (`closest` < 1.3) the same
before and after. CPU: the "world: lamps" span 0.06–0.08 → 0.21–0.35 ms
per frame (1080p Medium), 0.20–0.27 → 0.38–0.47 (every row up, 4K).

**Effect** (main view, GPU ms, median; the whole frame in brackets; every
bench frame byte-identical; `docs/evidence/perf-budget/lamp-culling.txt`):

| scene | integrated GPU, 1080p, World Detail 3, Medium | High | desktop GPU, 4K, every row up |
|---|---|---|---|
| racing | 11.2 → 7.5 (24.5 → 21.0) | 13.1 → 8.8 (47.2 → 42.9) | 6.1 → 4.5 (18.9 → 16.9) |
| rain at night | 12.5 → 8.5 (28.2 → 24.2) | – | 6.6 → 5.3 (23.3 → 21.4) |
| race start | 9.4 → 7.9 (25.4 → 24.0) | – | 5.5 → 4.8 (20.5 → 19.3) |
| casino canopy | 6.5 → 4.7 (17.3 → 15.0) | – | – |

Per step (integrated GPU, Medium racing): the cars per cluster 11.2 → 8.3,
the tight lists → 8.15, the early outs → 8.1 (within the runs' spread),
the beams' first test → 7.5 (desktop GPU, every row up: 6.2 → 5.0 → 4.7 →
4.7 → 4.5, and Modern Lighting's cars per entry → 4.4, frame 16.9 → 16.8; its
"world: lamps" span 0.52 → 0.69 ms with the relight's longer lists).
With subgroups (the cluster loop's subgroup form) the cars' bits are
walked for the subgroup together (`subgroupOr`, each lane taking only its
own, in order), so each car's record is read once per subgroup: the
integrated GPU's world shader keeps the subgroup loop's 72 VGPRs (14 waves
per SIMD); read per lane, as without subgroups, 4 of its 12 big pipelines
needed 96.

Tried and not kept: the beams per cluster (the loop over the beams and car
lights cut from 4 to 1.25 per pixel, the main view not faster on either GPU,
more code), the cars' bits walked with a uniform loop and a bit test (scalar
loads, but slower: 9.0 vs 8.6 ms on the integrated GPU), the lamp record read
in two parts (no change), the beams' cookie and shadow skipped where a beam
adds nothing (no change).

## Native byte-identity
The lamp code is in `// BEGIN LAMPS` … `// END LAMPS` blocks of world.wgsl;
`world_source(false)` drops them, so the native module is the old shader
plus three `Frame` fields and group 0 bindings 3–8 (placeholders for all
views but the main one with an option on). A first version with the
hooks inline changed 1–3 pixels by 1–3 levels in explore / intro / drift
(driver code generation); with the blocks stripped, all off is
md5-identical to the base (evidence README). `SceneMode::lamps` picks the
lamp module for the main view's pipelines only while an option is on.

## Cost (gpu busy ms added, `--bench explore 600` at night; details and raw numbers in docs/evidence/lamps-shadows/README.md)
| | 1080p | 4K | iGPU 1080p (measured, 2-CU RDNA2; World Detail 5, the bench default: docs/perf-integrated-gpu.md "World Detail") |
|---|---|---|---|
| Off (frame) | 0.69 | 2.44 | 12.6 |
| Dynamic Lamps | +0.03 | +0.03 | +3.6 |
| Lamp Shadows Low (over Lamps) | +0.13 | +0.25 | +5.0 |
| Lamp Shadows High (over Lamps) | +0.21 | +0.46 | +9.3 |
| Contact Shadows | +0.02 | +0.01 | +2.7 |
| All three (High) | +0.28 | +0.61 | +13.8 |
Cost classes (options `cost`): Lamps Free, Contact Free, Shadows Low/High
Low (1080p) to Medium (4K). On the iGPU Lamps and Contact are Medium,
Lamp Shadows High.

## The falling rain under the lamps (the native particles' pass)

The native particles' pass (`sprites.rs` `draw_native_particles`, its own GPU mark "graphics: native particles")
draws the falling rain with world.efx (`Effect::World`, fogged: docs/formats.md "Drop draw"), so with the lamps'
module every pixel of every drop ran the whole lamp shading (`lamp_shade`: the cluster loop, Lamp Shadows, Contact
Shadows, beams, Modern Lighting's relight and rays). About 260 drops (0.06 × 1.4 m quads, RAINDROP's alpha at most
68/255 times the template's 0x1C) cost 0.28–0.30 ms at the player's settings in 4K rain and 0.60–0.70 ms on the
integrated GPU (1080p, World Detail 3, Medium and High); with the drops drawn with the cheap `fs_nofog` program
(a measurement only) 0.022 / 0.17: the shading was nine tenths of the pass. The smoke and the other particles
(worldnodepth) take no lamp light and cost 0.006 ms.

Two changes, Enhanced only (the lamps' module; the native modules are not touched):

1. **Alpha 0 left before the shading (exact).** Alpha-blended keys without the alpha test take `fs_world_clear` /
   `fs_nofog_clear` (fragment.wgsl, `pipelines::clear_skip`): a fragment whose alpha (texel.a · v0.a, as `shade`
   makes it) is 0 blends src · 0 + dst · 1, the target as it was, no depth written; it is discarded before the
   lamps (a demote: the derivatives the neighbours take are unchanged). RAINDROP is 39 % transparent texels.
   Byte-identical (4K the player's settings and 1080p High rain, 17 frames each, md5 of the pixels).
2. **The drops lit at their corners** (`rain_lit.wgsl`, `DropPipelines`, `sprites::DropProbe`). The fogged colour
   is linear in the texel: rgb = texel.rgb · K + S, K and S depending on the position (the lamps, shadows, beams,
   Modern Lighting's light and specular), not on the texel. A probe pass draws each drop as a 2 × 2 block of a
   256 × 32 half-float target, one pixel per corner (the drop's vertices placed twice as far from its centre, so
   the pixel centres interpolate to the corners; the drop is a parallelogram, every input affine across it, the
   block's derivatives spanning the drop), K into the upper half and S into the lower (two pipelines, two
   viewports). The drop then takes K and S filtered between its corners: one texture read and one blend per pixel
   instead of the shading. Not byte-identical: a drop's light is interpolated along its 1.4 m instead of evaluated
   per pixel (a shadow edge or a light's falloff across one drop becomes a gradient).

Measured (GPU timer queries, median of interleaved runs [range]; probe + pass):

| | base | 1. alpha 0 | 2. corners |
|---|---|---|---|
| 4K the player's settings, rain (4 runs, load 24–31) | 0.295 [0.294..0.298] | 0.259 [0.256..0.262] | 0.032 + 0.029 |
| frame (GPU busy) | 18.95 [18.88..18.98] | 18.98 | 18.74 [18.65..18.81] |
| integrated GPU High rain (2 runs each, slow state) | 0.68–0.70 | 0.637 | 0.05 + 0.24 |
| integrated GPU Medium rain (2 runs, mixed state) | 0.602 [0.576..0.629] | 0.521 | 0.05 + 0.27 |

What is left on the integrated GPU is the drops' fill and blend (the plain program's 0.17 ms) and the probe's
fixed cost (its pass and two pipelines, 0.05). Pictures of step 2 against step 1 (17 frames each, 16 consecutive,
`docs/evidence/perf-budget/rain-drops-lit.txt`): 4K at most 38 levels on 50 pixels > 20 (a drop's specular
highlight over a light), 0.10 % of the pixels > 1 %, 1 − SSIM ≤ 1.3e−5, mean / p5 / p95 luma unchanged; 1080p High
at most 21, 0.05 % > 1 %, 1 − SSIM ≤ 7e−6; the difference's own frame-to-frame change 0.012 / 0.008 levels (no
flicker). Original and the levels without the lamps' module draw as before.

## Open / left out
* Neon and shop signs: not in the light packs (above); no proxy lights
  were invented for them.
* Lamp layers hold static scenery as of when they were drawn: a section
  that becomes visible later is not redrawn into a held layer until the
  lamp leaves and comes back (a scenery switch is, above). A knocked-over
  prop's layer keeps it at rest.
* Cars cast lamp / headlight shadows as ellipsoids, not as depth (no car
  depth pass); they receive no lamp shadow themselves.
* Rear-view mirror, road reflection and the environment cube take no
  lamps (native views).
* Nothing to compare in the original (a deviation): judged against the
  clone's native shots.

## Tests
`render::lamps::tests` (native weight curve, slices, clustering of a lamp
ahead / behind / in an invisible section, layer assignment and budget,
car lamp choice), `qol::tests::lamps_and_shadows_round_trip_and_native_default`,
ranges in `enhancements_round_trip` / `graphics_values_held_to_their_rows`,
row order and label ids in the options tests,
`render::world_source_strips_the_lamps` (native module free of lamp code).

## Pools too bright ("dynamic lamps make the spots too bright")
**Cause**: the baked vertex colours already hold every street lamp's pool;
The first version scaled the bake by (1 + 2 · lamps) everywhere inside a lamp's radius,
so a pool centre got its baked light ×1.5 on top (the race start's pool
clipped at 255; High Precision Post, wet roads and SSR then glow and
reflect that). **Fix** (world only; cars keep their per-pixel lamps, they
have no bake): the term is weighted by the bake's headroom below
`LAMP_PEAK` = 0.5 (vertex-colour units, the texture at 1×) and the summed
lamp term capped at `LAMP_MAX` = 0.35: a surface baked at or above 0.5 (a
pool) is left exactly as baked, nothing ends brighter than max(bake,
0.5), a dark gap gains at most ×1.7 of its (dark) bake.
`lamps::lamp_term` mirrors it; `pool_peak_is_pinned_to_the_bake` pins the
peak ratio to 1 and checks world.wgsl uses the same form and constants.
Reference = the clone's Original image (the native path), not a Wine
capture of the original (**open**).

Road luminance (Rec.709, 0–255) of the frame's lower road band (55–92 %
of the image height without the letterbox, 8 % side margins): peak = 99th
percentile, mean, gap = 25th percentile; 1920 × 1080 `--shot`. "1x" =
Dynamic Lamps only; "max" = Ultra + every row at its top (SSAA 2x, High
Precision Post full, road/car reflections Ultra, SSR full, SSAO 100 %,
wet roads, enhanced reflections / paint, world/car detail 200 %, keep
loaded) + Dynamic Lamps, Lamp Shadows High, Contact Shadows.

| scene | Original | before | after | max: Original | max: before | max: after |
|---|---|---|---|---|---|---|
| start (City Core grid) | 251.2 / 81.3 / 36.7 | **255.0 / 94.4** / 38.8 | 251.5 / 82.8 / 37.6 | 246.9 / 79.7 / 35.7 | **254.9 / 89.6** / 36.8 | 247.1 / 79.5 / 35.7 |
| night (overpass) | 183.9 / 65.8 / 49.9 | 187.5 / 68.8 / 52.0 | 185.0 / 67.0 / 50.8 | 183.0 / 65.1 / 49.0 | 187.0 / 67.4 / 50.1 | 184.1 / 65.6 / 49.1 |
| street (S4_URL_7) | 55.8 / 26.3 / 14.2 | 55.9 / 26.4 / 14.2 | 55.8 / 26.3 / 14.2 | 44.3 / 24.2 / 14.0 | 44.3 / 22.7 / 13.5 | 44.3 / 22.7 / 13.5 |
| crowd (S2_STREET_4) | 241.4 / 90.8 / 32.9 | = Original | = Original | 240.9 / 83.3 / 32.5 | 248.2 / 71.2 / 30.5 | 248.2 / 71.2 / 30.5 |

(peak / mean / gap.) street and crowd have no light-pack lamp near the
road band (Dynamic Lamps alone changes nothing there); their "max"
difference is Lamp Shadows (the cars shading the headlight beam) and
Contact Shadows, unchanged by the bright-lamps fix. Crops (Original | before | after, the
start's pool): internal evidence,
the max setup `r6_start_pool_max_orig_before_after.webp`. Script:
internal evidence.

## The Lighting II work (docs/lighting2.md)
With Headlight Beams on, every car's headlight is view 9's projector as a beam in the same LAMPS
blocks (after the lamps in the lamp list); the headlight shadow layer follows the player's beam
(drawn whatever Customize Display's Car Headlight says) and the native projector term is skipped.

## The gap budget (2026-11-05, docs/look-direction.md rule E2)
`LAMP_MAX` 0.35 → **0.12**: a dark gap now gains at most ×1.24 of its bake (was ×1.7). Over the look direction
spots Dynamic Lamps alone lifted the mean luma +2.6 (the coal docks +6.8), the darkness between the
lamps — NFSU2's look — the most; now +1.7 (measured, the maintainer's private captures). Pools and peaks
are unchanged (`pool_peak_is_pinned_to_the_bake`).
