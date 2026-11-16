# Shadow softness and stability under Modern Lighting (2026-11-05; clone deviation, opt-in)

Player reports 2026-11-05, settings: Modern + RT at 75 %, Shadows Ultra, Lamp Shadows High, Contact
Shadows, SSAO 100, Dynamic Lamps, Surface Detail, 4K (options copied from
the player's options file, read-only):
1. the casino canopy (Neon district, under the light balls): a dark blotchy pattern on the road that
   flickers (screenshots 13-45-37/39; the two files are pixel-identical frames, the car standing);
2. the flag girl at the first race start: "harsh lines";
3. the 350Z's shadows at the airport: "a bit harsh".

Nothing here is native: SPEED2.EXE has the baked vertex light, the car's shadow texture and its
harmonic only (docs/modern-lighting.md). With the Lighting row at Original nothing changes (Parity).

## Root causes (toggle matrix, `work/` scripts below; 1920×1080, 350Z, the player's options)
Each row: one option changed from the player's set. Still = car and camera held 60 frames
(`NFSU2_BENCH_AT` + `NFSU2_BENCH_FREEZE=1`), per-pixel temporal SD on the road near the car
(0,600–620,680 / 1100,600–1500,720 / the car 650,600–1100,900), rain (clock 162).

| change | road L | road R | car | look |
|---|---|---|---|---|
| (user's set) | 1.07 | 0.94 | 1.37 | hard, black, faceted car shadow from one canopy light |
| Modern Lighting off | 1.36 | 0.98 | 1.31 | no lamp/neon shadows |
| Modern (no RT) | 1.07 | 0.94 | 1.37 | soft ellipsoid shadow |
| Modern Low | 1.05 | 0.93 | 1.37 | |
| Wet Road Interaction off | 1.00 | 0.90 | 1.35 | the ripples are the road's main motion |
| SSR off | 1.52 | 1.25 | 1.47 | (the planar reflection shows more of the canopy's animation) |
| Enhanced Reflections off | 1.23 | 1.20 | 1.73 | |
| Surface Detail / Lamp Shadows / Contact / SSAO / Dynamic Lamps / Rain Detail / Shadows / Wet Roads / Probes / Beams off | ±0.05 | | | |

* **The mottled road is the native road texture in rain** (Original shows it too, `casinorain_full`
  before), and the dark spots on the body are Wet Car's droplets (rain at that time): left as they are.
  With the camera still, nothing of Modern Lighting moves: its variance is at or below Lighting =
  Original's (the canopy's animated native textures and their reflections, the rain ripples).
* **Flicker = the cluster lists' cut (fixed)**. Modern Lighting's light list (2233 lamps + 5716 neon
  emitters + the cars' lights) needed 100–117 k cluster indices at the casino; the index list held
  65 536 and was filled in cluster order, so every list after the cut (slices 10–11 on: ≈ 19 m from
  the eye) was **empty**: 1 828–1 883 of 3 456 clusters, the far road, the facades and the canopy got
  no Modern light (`casino_cluster_cut_debug.webp`, the debug view, top). The cut moved with every
  frame's light count (the traffic's lights, the view): logged while driving under the canopy, the
  cut slice flipped 10 ↔ 11 and the empty count changed every frame — tiles ahead of the car flipping
  between lit and the bake × ambient (dark). Now `lamps::MAX_INDICES` = 262 144 and, past that, every
  list is cut to one cap (`lamps::list_cap`, the longest lists lose their farthest lights) instead of
  emptying the last clusters.
* **RT shadows flipping (fixed)**: the 4 rays went to the first 4 lights of the cluster's list (by
  eye distance), so which lights were traced changed with the view; a light's shadow switched between
  its ray and its fallback (ellipsoid / none). Now the 4 strongest at the pixel are traced:
  independent of the eye and of the list order.
  (Found by reading the shader; not isolated by a separate measurement.)
* **Harsh = point-light rays + full-depth umbra + stacking**: one ray to each light's centre gives a
  binary, aliased edge (the car's low LOD's facets) at 85 % depth of a light the Bold strength already
  boosts over a halved ambient; a canopy light ball is metres wide, yet threw a razor shadow. Contact
  Shadows multiplied on top under the car.
* **The flag girl's lines = the relight on the face normal** (Modern on, RT or not; gone with Modern
  off, unchanged by every other row: `flaggirl_*`): the world pixel has only the triangle's normal
  (`cross(dpdx, dpdy)`); characters are drawn through the world pipelines with the clone's
  half-Lambert vertex colours (`character::shade`), which the relight read as a lamp bake: every
  triangle took a different share out and put a different pool back.

## Fixes (world.wgsl MODERN / MLRT blocks, `lamps.rs`, `characters.rs`)
* **Area lights** (`ml_ray`): a street lamp is a 0.35 m disc, a sign / emitter cell
  clamp((reach − 7) / 6, 0.3, 2.5) m (its reach is 7 + 3 × the side of its area). Rays go to a fixed
  Vogel disc set; the pixels of each 2×2 quad take interleaved quarters of it (each light its own turn
  of the set) and the quad's mean of the shadowed light (`ml_quad_mean`, fine derivatives) resolves
  them: the strongest light 2 rays per pixel (8 points per quad), the others 1 (4 points). Smooth
  penumbrae, no dither, no noise, nothing depends on time — a still view is bit-stable, no
  accumulation or reprojection needed. Only the lost light is averaged (2×2 at render resolution),
  not the texture or the unshadowed light.
* **Strongest-4 ray selection**, ranked inside the light loop (every light takes its fallback shadow
  there; the 4 strongest then trace and swap their fallback for the rays'; a separate ranking pass
  cost ~10 ms at 4K under the canopy). A light is traced only if it holds ≥ 25 % of the pixel's
  relit light (`ML_RAY_SHARE`: a faint sign among dozens throws no visible shadow), never on ceilings
  and undersides (ng.z < −0.3; their fallback stays).
* **Umbra floor** `ML_UMBRA_FLOOR` 0.4: a pixel keeps at least 40 % of its unshadowed relit light
  (the bounce and the painted fill), so stacked shadows and the strength's darkening never go black.
* **Contact Shadows under Modern Lighting**: the darker of the contact shadow and the lights' shadow
  ratio at the pixel, not their product (as the glow-darkening pass for the shadow texture).
* **Characters** (`characters::CHARACTER_MARK`, instance m0.w 0.25, which the native shaders read
  as 0): no share taken out of their vertex colours, the lights' pools without the face cosine
  (× 0.5, the hemisphere's mean), their shadows (rays / ellipsoids / layers without the face test),
  no specular. Smooth bodies; the lights and shadows still reach them.
* **The settings rework conflict rule** (Graphics page): Dynamic Lamps greyed at its value while Modern Lighting is on
  (replaced); Lamp Shadows offers Off / Low only (High adds only lamp maps, which Modern owns; the row
  keeps the headlights' shadow); Contact Shadows' help states the darker-of.
* Debug: `NFSU2_MODERN_LIGHTING_DEBUG=2` shows the light the shadows take (red lamps, green others)
  and the traced lights per pixel / 4 (blue).
* Found on the way: a `ray_query` declared inside a loop crashed the RADV pipeline compile
  (segfault); each ray now runs in its own function (`ml_ray_one`).

## Evidence (`docs/evidence/shadow-softness/`, before = main + the bench aids only)
`<spot>_full.webp` before | after; `<spot>_crops.webp` 100 % (top) and 200 % at the shadow edge
(bottom). Spots: airport (1700, −884), casino canopy dry and rain (−400, −524, heading 180), City Core
street (−237.3, 79.8), neon district (−394, −531.1), garage approach (−388.6, 40.3); the flag girl
(`--shot race`): `flaggirl_before_after_ml0.webp` (before | after | Modern off), `flaggirl_200.webp`.

Still 60 frames at the casino (frames 0–59, temporal SD, whole frame / road ahead / lower half):

| | dry before | dry after | rain before | rain after | Lighting Original |
|---|---|---|---|---|---|
| whole | 6.38 | 6.75 | 5.85 | 6.22 | 7.34 / 6.73 |
| road ahead (500,440–1400,560) | 7.02 | 7.28 | 6.93 | 7.00 | 7.50 / 7.24 |
| lower half | 1.92 | 2.05 | 2.19 | 2.28 | 2.42 / 2.44 |
After stays below Lighting = Original everywhere (the variance is the native animated textures,
the traffic and the rain); the small rise over before is the far region now lit (it reflected the
animation dark before). Driving: the cluster cut no longer exists below 262 144 indices (the casino
needs ~101 k), so nothing flips per frame.

## Cost (reference desktop GPU, the player's options, GPU busy median ms, 300-frame benches, 2 interleaved reps, fast build; GPU shared with other jobs)
| scene | Modern before | Modern after | Modern + RT before | Modern + RT after |
|---|---|---|---|---|
| casino canopy still 1080p | 2.82 | 4.98 | 4.20 | 8.03 |
| casino canopy still 4K | 9.17 | 15.37 | 14.15 | 24.62 |
| racing 1080p | 3.83 | 4.06 | 5.09 | 6.63 |
| racing 4K | 9.95 | 11.12 | 13.30 | 17.78 |
Modern's rise is the cluster fix: before, half the canopy's clusters had no lights (dark, and
cheaper); that is correctness, not the new shadows. RT's own share: casino 4K +4.98 → +9.26 ms, racing
4K +3.35 → +6.66 (5 rays to the strongest lights where the old code traced fewer, in fewer
clusters). Steps measured on the way (casino 4K, RT): 2 rays × 4 lights with a ranking pass 43.5;
ranking fused into the loop 35.1; 1 ray each 26.4; 2 + 1 + 1 + 1 with a 15 % share 26.8; 25 % share
and no ceilings 24.4.

## Open
* Characters have no vertex normals in the world format: their Modern light is normal-independent
  (a flat wrap); a skinned-normal stream would give them proper N·L.
* The ray budget stays 4 lights × 2 rays; the 5th and later lights use ellipsoids / layers.
* The traced world is the low LOD (unchanged): curved facades can still shade a little where the
  drawn face is lit (faded on grazing faces).
* Native: nothing to trace (no counterpart in SPEED2.EXE); all constants are judgement calls.

## Checks
Gate (`scripts/checks/gate.sh … <base> extreme`, base and new md5): CHECKS PASS (fast:
1051 tests, selftest, 28 shots, flows, playthrough); PARITY all gates passed (Original AE 0 at 1280×960
and 4K, 144 Hz path, Modern/Ultra/Extreme presets AE 0, 28 shots AE 0, audio and sim hashes identical).
Modern Lighting phase 2 matrix (`modern-lighting-2/scripts/matrix.sh`, the player's live options / Ultra / Extreme × tiers
1–3 × explore, race, racing, rain): all runs clean.
Measuring aids: `--bench explore` with `NFSU2_BENCH_AT=x,y,z,deg` (the car placed, standing),
`NFSU2_BENCH_AT_DRIVE=n`, `NFSU2_BENCH_FREEZE_MOVE=dx,dy,dz` (a frozen view crawling);
`scripts/` here (`seq.sh` still/crawl dumps, `tvar.py` temporal SD, `cost.sh`).

## 2026-11-05: the rays at half resolution, outside the colour shader
Modern + RT's cost (racing 4K, the player's settings + Modern + RT: +7.5 ms over Modern) was the
rays in the world's colour shader: 2 + 1 + 1 + 1 per pixel, and the ray-query code's registers held
every world pixel's shader (+1.5 ms with no ray traced at all). Now (`render/rt_shadow.rs`,
`rt_shadow.wgsl`; numbers in `docs/perf-budget.md` "The new effects' costs"):
* **A compute pass between the depth pre-pass and the colour pass** traces, per 2 × 2 pixel
  block, the two strongest lights of the block's point (the pre-pass depth, its face normal from the
  neighbours' depth; the same ranking, share floor and no-ceiling rule as `ml_relight`) with this work's
  area-light rays (the strongest 2 rays, the second 1; the block's quarter of the Vogel set). It writes
  (light + 1, visibility) × 2, the block's view depth and octahedral normal (Rgba32Uint).
* **The colour pass gathers** the 3 × 3 blocks around each pixel with per-axis weights
  (1 − f) / 2, 1 / 2, f / 2 (the same weight on odd and even blocks wherever the pixel lies: every
  pixel takes all four quarters of the disc set, as this work's quad mean; the weights slide per pixel, no
  2-pixel steps), each block counted only on the pixel's plane (view depth extrapolated with its
  screen slopes, 0.4 % + 2 cm) and facing as its normal. The traced lights' visibility replaces their
  fallback inside the light loop (no ranking, no second loop, no ray code: `D9_ML_CHAR` false folds it
  out). This page's quad mean is gone for these pixels (the gather resolves the set).
* **Overlays** the pre-pass does not hold (blended road decals: the start line's checker) lie a
  little over the surface the blocks traced: there the blocks of the surface point under the pixel are
  taken (`ml_vis_find`, ≤ 25 cm; before this the car's shadow on the start line moved ~10 px).
* **NIS characters** (not in the pre-pass) keep this work's in-shader rays: their own pipeline set
  (`ScenePipelines::characters`, `LF_ML_CHAR` → `D9_ML_CHAR`).
* Pixels whose blocks lie on another surface (silhouettes, alpha-tested foliage under MSAA's
  alpha-to-coverage) keep the light's fallback shadow (ellipsoid / lamp layer), as before for lights
  past the strongest four. Without the depth pre-pass (`NFSU2_DEPTH_PREPASS=0`) every pixel does.
* Debug view 2 (`NFSU2_MODERN_LIGHTING_DEBUG=2`): blue counts the lights the blocks shadow.
Image metrics vs before (4K, user's settings + Modern + RT; `docs/evidence/perf-budget/dx1/`):
casino canopy still SSIM 0.9993 (0.6 % of pixels > 2 levels, at shadow edges), rain drive 0.9994–0.9998,
race start 0.99998; crawl / drive sequences: temporal SD and second differences equal to before
(±0.2 %). Not native (no counterpart in SPEED2.EXE); the other tiers are byte-identical.

## The gather first (the colour shader's registers)
The colour pass's 3 × 3 gather of the blocks (`ml_vis_find` → `ml_vis_taps`) ran inside `ml_relight`:
its four slot ids and eight sums were live across the whole light loop, and the wet road's two
`lamp_shade` calls inlined it twice in the reflect function. On the reference desktop GPU (gfx11,
these shaders in wave32: every module with a ray query is compiled at the driver's ray-tracing wave
size, the characters' rays included) registers are allocated in steps of 24 per lane: the world
function needed 126 (allocated 144, 10 waves per SIMD), the reflect function 161 (168, 9 waves);
without the gather 112 (120, 12 waves) and 143 (144, 10 waves). With the gather left out the main
view is ~1.1 ms faster at 4K and SSAA 2x, while exact short cuts that skip most of its executed work
saved ~0.1: the gather's cost is the occupancy it takes from the whole colour shader.

Now `ml_vis_early` (mlrt.wgsl) gathers at the top of `world_colour` and `reflect_colour`, before
Surface Detail and the lighting, from the same inputs (the face normal, the view depth, its screen
slopes) with the same expressions, and keeps only what the loop reads: the slots' light ids, two to
a register, and each slot's traced visibility (`ml_vis`, made once); the loop picks it by id
(`ml_vis_slot`). Frames are identical (racing, race start, night rain at 4K). The world function
now needs 120 (120, 12 waves) in 22 of its 33 pipelines (the LOD fade's need 122 and stay at 144);
the reflect function 151–153 (still 168, 9 waves). Main view at the player's 4K settings, racing:
−0.15 ms (paired median of 6 interleaved runs; `docs/perf-budget.md`). What keeps the reflect
function at 9 waves: the wet surface's terms (`wet_surface`) are live across the lighting, and
`wet_surface` writes the roughness the lamps' specular reads (`g_surface`), so the two cannot simply
swap (tried: the wet road's highlights change).

With the registers freed, two exact short cuts in `ml_vis_taps` pay off (alone, before, they saved
~0.1 ms; the gather's work was hidden behind the occupancy): where all 3 × 3 blocks hold the centre
block's lights (two different ones, or one and none) with the same visibility, each fully lit or fully
shadowed, a light's sum is its weight sum or 0, so only the nine weights are summed (in the same order:
the same bits); where neither the centre nor the side block holds a light, nothing reads the sums. In
racing two thirds of the colour shader's waves see no traced light and most of the rest are uniform
(a counting shader over the blocks: 84–91 % of 8 × 8-pixel tiles take one of the two). A GPU test
(`gather_short_cuts_are_exact`, `--ignored`) runs today's gather and the full one over blocks holding
every case and compares their ids and sums bit for bit. Main view: −0.23 ms against before both steps.


## The blocks follow the screen under supersampling
With SSAA (or Render Scale above 100 %) the scene is larger than the screen: at 4K and SSAA 2x
5431 × 3055 scene pixels, 4.15 M blocks traced. The soft shadows are a smooth signal (area lights,
the 3 × 3 tent), and the scene's extra pixels added rays without adding shadow detail the screen can
show. Now (`rt_shadow::grid`) a supersampled scene takes one block per 2 × 2 *screen* pixels
(1920 × 1080 at 4K: 2.07 M), each sampling the scene pixel under its first screen pixel
(`rt_shadow.wgsl`, the pass's uniform `misc.yz` = the scene pixels per grid pixel); the colour pass
reads that scale (`ml_vis_grid`, the same uniform at group 0 binding 16) and still gathers its own
blocks on its own plane per scene pixel, so silhouettes and the depth / normal weights keep the
scene's resolution. Without supersampling the scale is exactly 1 and the grid, the sample pixels and
the gather's arithmetic are the ones before (frames identical). The output's size reaches the world
renderer through `WorldRenderer::set_screen_size` (`FrameInputs::out_size`). Cost and the picture
report: `docs/perf-budget.md`, `docs/evidence/perf-budget/rt-shadow-gather.md`. Not native (no
counterpart in SPEED2.EXE).
