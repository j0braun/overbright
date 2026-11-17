# Modern Lighting (phase 1 + phase 2 "bold"; clone deviation, opt-in)

Phase 2 (the bold option): see **Phase 2** at the end — the Graphics →
Lighting page's **Lighting** row (Original / Modern Low / Modern / Modern + RT) and **Lighting
Strength** (Classic … Bold), the whole city, lamps + neon/signs + car lights, shadows, cars lit alike.

Strictly opt-in: the original baked lighting stays
the default and the classic look stays one toggle away. Phase 1 = this design and a working
prototype for one district of Bayview behind a **hidden env switch** (no menu row yet).
Nothing here is native: SPEED2.EXE lights the world with baked vertex colours only (world.efx
`tex × v0 × 2`) and the cars with the light packs' harmonic (0x48E3D0 → 0x4893A0). Everything
below about the bake is **measured from the game's data** (the maintainer's probe `modern_light_fit`); how
EA produced the bake is **not traced** (offline tools, not in SPEED2.EXE) and stays **inferred**.

## What the bake holds (measured, `cargo run --release -p devprobes --bin modern_light_fit`)
Bayview (L4RA): 2233 light-pack lamps (all point lights, outer 30 m / inner 10 m, grey
(150,150,150) or bluish (110,110,125); docs/lamps-shadows.md), 4.13 M vertices of the main-LOD
placements (sky section and additive textures left out), 187 238 of them on the reflective road
meshes (flag 0x40).
* **Roads: the bake's pools are the light-pack lamps.** Road vertices lit by one lamp only, by
  horizontal distance from it: mean bake luminance 0.69 at 0–4 m, 0.56 at 6–8, 0.41 at 10–12,
  0.30 at 14–16, 0.24 at 18–20, 0.20 at 24–26, 0.18 at 28–30 (vertex-colour units, 0.5 = the
  texture at 1×); the floor between lamps is 0.19.
* **The pools follow a physical falloff better than the native weight**: least squares of the
  road bake on the summed lamp term, r² 0.17 with the native weight (0x4893A0: linear in d² to
  the outer radius), 0.10 with its square, **0.37 with inverse square × cosine** softened by
  150 m² and windowed to the authored outer radius ((1 − (d/r)⁴)²; softening searched over
  0–400 m², best 150; `modern_light::pool`).
* **The pools are warm, the packs grey**: per-channel fit of the road bake on pool × authored
  colour / 255: gains **0.749 / 0.655 / 0.445** (r² 0.37 / 0.35 / 0.29). Pool vertices (summed
  weight > 0.6) average chroma (1.06, 1.00, 0.85), the gaps (1.01, 1.00, 1.00): the sodium tint
  is in the bake, not in the packs.
* **Everything else is not lamp light**: over all 4.13 M vertices the lamps explain r² 0.02–0.03
  (walls, buildings, props: the artists' paint, sky, neon spill, bounce). Neon and signs are not
  in the light packs at all (docs/lamps-shadows.md).

So a relight that rebuilds the world's light from the light packs would throw away most of the
image; the packs explain the road pools and nothing else.

## Options compared (wgpu 30; reference desktop GPU ≈ 1.1–1.8 ms/frame now; iGPU class = the reference 2-CU RDNA2 iGPU, ≈ 12.6 ms at 1080p)
| | (a) Rebuilt lightmaps / vertex lighting offline from the packs | (b) Real-time GI (probe grid / DDGI-like on ray query, SSGI fallback) | (c) Hybrid: bake-anchored relight (picked) |
|---|---|---|---|
| What | Re-bake each vertex (or a new lightmap UV set) from the packs' lamps + sky/moon, bake vertex colours as albedo reference | Dynamic diffuse GI from probes updated by rays (or screen-space), lamps as the light sources | Keep the bake; model only the lamps' share of it per pixel (fitted to the bake), take it out and put it back relit (shadows, per-pixel normal, specular); the residual (paint, sky, neon, bounce) stays the bake's |
| Look risk vs the art (sodium/neon, wet night streets, heavy glow) | **High**: the packs explain 2–3 % of the bake's variance off the roads; walls, neon spill and the painted darkness would be replaced by what the packs say. Albedo recovery from tex × v0 is ill-posed (one product, two unknowns) | **High**: GI lifts the dark gaps between pools (the GTA DE / HL2 RTX failure, research/looks-perf-architecture-2026-11-04.md); neon is emissive textures, not lights, so GI would need invented proxy lights | **Low**: unshadowed and on the face normal the result is the bake exactly (`relight` test); the share taken is capped at 85 % of the bake; nothing is lifted |
| Cost reference desktop GPU | ≈ 0 at run time (a second colour stream) + an offline bake (minutes, per region) | probe update + sampling ≈ 0.5–2 ms; SSGI ≈ 1 ms at 1080p (estimate, class figures) | measured: +0.3 ms at 1080p, +0.7 ms at 4K in a race; ≈ 0 over Ultra |
| Cost iGPU | ≈ 0 | 5–20 ms (estimate): not viable; ray query not on the target iGPUs | measured +21.6 ms on the 2-CU iGPU as prototyped: a Low tier / the vertex stream needed |
| Effort | Large: UV unwrap or per-instance vertex streams (world solids are instanced), a baker, cache format | Very large: probe placement exists (the Lighting II work's 6172 road probes), but update, leak control, ray-query fallback, temporal stability | Small for the core (this prototype: one new module + LAMPS-block shader), medium for phase 2 |
| Wet roads / glow | needs the planar reflection and glow left as they are | GI on mirror roads is invisible; spec needs its own path anyway | specular from the same fitted lamps on top of the planar / SSR reflections; glow untouched |

**Pick: (c)**, because (1) the data shows the bake already *is* a good physical lamp solution on the
roads (r² 0.37 with inverse square) and pure art everywhere else, so the only defensible thing to
"modernise" is the lamps' share, and only where a modern renderer knows more than a vertex bake:
occlusion (cars, poles, trees, awnings in the pools), per-pixel normals (Surface Detail), specular;
(2) it is energy-anchored to the bake, so the darkness between lamps and the sodium hue are kept
by construction; (3) it reuses the lamp clusters and shadow layers (no new passes) and stays cheap
on a dGPU; (4) (a) and (b) can still feed it later (a per-vertex share stream from (a) is phase 2,
below; a probe GI term would only be added for cars).

## The prototype (phase 1)
Switch: `NFSU2_MODERN_LIGHTING=1` (or `citycore`): **City Core**, the box of the in-game World
Map's City Core zoom (record 1 of 0x7F42C8: texels 186, 175, size 307 of the 512 map, with L4RA's
map frame left −3425.43, top 3287.26, size 5633.16 → x −1379 … 1999, y −2016 … 1362);
`all` = every lamp; `NFSU2_MODERN_LIGHTING_BOX=x0,y0,x1,y1` any box. `NFSU2_MODERN_LIGHTING_DEBUG=1`
shows red = the modelled share, green = relit share, blue = shadowed share × 4 (× the texture).
Unset or `0`: off (read once at start).

Data: the maintainer's probe `modern_light_fit` (the fit; FIT_ROAD=1 roads only, FIT_LIST=1 bright pool
spots, FIT_DOORS=1 shop doors, FIT_NEAR=x,y,z lamps near a point; output in
internal evidence).
Code: `src/render/modern_light.rs` (switch, district, fitted constants, CPU mirror, tests),
world.wgsl `// BEGIN MODERN` … `// END MODERN` blocks (all inside LAMPS blocks), hooks in
`lamps.rs::plan_lamps` (clusters and lamp layers at High while on, the headlight's layer only
with the Lamp Shadows row; lamps.x = 2 + the near-car count; each uploaded
lamp's ML colour in `Lamp.shadow.yzw`), `SceneMode::current` (the lamps' module while on),
`mod.rs::world_source` (MODERN lines stripped unless on), `game.rs::car_occluders` (the cars'
boxes also while on).

Per pixel (main view, fogged world draws incl. the road's worldreflect and Wet Roads), for each
clustered lamp in the district: `share += C · pool(lamp − p, face normal)`,
`lit += C · pool(lamp − p, shading normal) · vis`, C = authored colour / 255 × the gains,
vis = the lamp's depth layer (4 bilinear compares 1.5 texels off; depth 0.85, faded out where the 140° downward
layer is unreliable: faces grazing the light, rays > ~55° off the vertical) × the cars as soft
ellipsoids (depth 0.75; the cars within 45 m of the eye, at most 4 near the pixel); then `light = bake − share·k + lit·k`, k = min(1, 0.85 · bake / share)
per channel, plus a GGX specular of the same lamps on a wet road (Wet Roads, roughness 0.22) or
with Surface Detail's material (× 0.5; a dry road without a material shows next to none and skips it). Dynamic Lamps' add is replaced while on;
Lamp Shadows' headlight shading, Headlight Beams, Contact Shadows and the cars keep their own rows.

What changes on screen (A/B below): the cars throw soft shadows into the sodium pools (every car,
from every district lamp that reaches it), awnings / canopies / poles / foliage shadow the lamps'
part of the bake, the lamps get a specular sheen (visible on wet roads and with Surface Detail);
outside the lamps' reach and in neon-lit places nothing changes, by design.

## A/B evidence (the maintainer's private captures, 1920×1080 `--shot`, `scripts/scenes.sh GAME OUT`)
Each `<scene>_orig_ml_ultra_ultraml.webp` = Original | Original + switch (top), Ultra (perf.sh's
preset) | Ultra + switch (bottom). Zooms: `street_car_shadow_zoom.webp` (the car's shadow from the
lamp behind it), `garage_canopy_shadow_zoom.webp` (the shop door's canopy shadowing the lamp
above it), `rain_ultra_zoom.webp`; `debug_street_garage.webp` (the debug view).

| scene | where | changed pixels vs Original (switch) | vs Ultra (Ultra + switch) |
|---|---|---|---|
| street (lamp-lit street) | (−237.3, 79.8), clock 93 | 0.68 % | 1.09 % |
| neon | (295.1, −554.5) | 0.82 % | 1.47 % |
| tunnel | Observatory tunnel (−1701.3, 2130), outside City Core: switch `all` | 0.20 % | 0.44 % |
| open road / plaza | (−307.8, −345.7) | 0.04 % | 0.14 % |
| garage approach | shop door at (−381.1, 53.3), car at (−388.6, 40.3) | 1.26 % | 2.07 % |
| rain | street, clock 170 (heavy rain) | 0.73 % | 1.09 % |
| start | Quick Race grid (under an overpass, few pack lamps) | 0.17 % | 0.28 % |
The race-start crowd (S2_STREET_4 scene camera) has no light-pack lamp in reach: 0 changed.
This is a deliberately small change: the player should judge whether phase 2 should go bolder
(see Open).

## Cost (measured)
`perf.sh bench` (SET=quick, REPS=3, 600 frames, full release profile, reference desktop GPU; arms: off, the
switch `1` = City Core, `all`; GPU timestamps' median ms per frame; raw internal evidence):

| config | off | City Core | all | added |
|---|---|---|---|---|
| explore, Original, 1080p | 0.637 | 0.689 | 0.689 | +0.05 |
| race, Original, 1080p | 0.890 | 1.170 | 1.171 | +0.28 |
| racing, Original, 1080p | 0.768 | 1.068 | 1.074 | +0.30 |
| racing, Original, 4K | 2.583 | 3.301 | 3.299 | +0.72 |
| racing, Ultra, 1080p | 4.134 | 4.052 | 4.082 | ≈ 0 (Ultra already runs the lamps' clusters and High layers; Dynamic Lamps' add is replaced) |

Draw CPU +0.03–0.07 ms (clusters for every frame, the lamp tagging). The cost is per pixel in the
world pass (main view 1.0 → ~1.8 ms at 4K): the cluster loop with a 4-tap shadow per shadowed
lamp and the near cars (at most 4 per pixel, cars within 45 m of the eye). Found on the way: a
3×3 PCF, all 32 cars per lamp and a dry specular cost another ~0.5 ms at 4K and were cut.
**iGPU** (the reference 2-CU RDNA2 iGPU, racing 1080p, one run): 15.4 → 37.1 ms (world pass 6.3 → 27.0):
**not viable there as it is**; phase 2 needs a Low tier (4 layers, 1 tap, no cars, nearest 4
lamps per pixel) or the per-vertex share stream (most of the work moves to the vertex / load).

## Native byte-identity
With the switch unset the native module is untouched (MODERN blocks sit inside LAMPS blocks) and
the lamps' module is the exact text it was before this work (`modern_light::source` drops the MODERN
lines; checked against <base>'s world.wgsl and by
`shader_matches_the_constants_and_strips_to_the_old_text`). `perf.sh parity` <base> (perf.sh
build, md5 6bfe8305…) vs this copy (md5 7d396b77…), ENH_PRESETS="modern ultra extreme
extreme_fsr": **PARITY: all gates passed** — Original dumps of explore/race/racing/drive/rain at
1280×960 and 3840×2160 AE 0, the 144 Hz window path AE 0, Modern/Ultra/Extreme/Extreme-FSR AE 0,
15 shots AE 0, audio identical, state hashes identical over firstrace, firstdrag, postrace, the
playthrough (82 412 steps) and the parity scenario (internal evidence).
(A first parity run of an earlier build, under load average ~50, had 1 pixel off by 1 in one
explore 4K frame; four reruns base/base, base/new, new/new were AE 0, and the final run passed.)

## Tests
`render::modern_light::tests`: the pool (1 below the lamp at 9 m, 0 at the authored radius,
monotone, cosine), the switch and the City Core box (from the zoom record and the map frame), the
ML colour (gains inside, 0 outside), the relight (bake kept unshadowed, floor 15 %, cap), the
shader's constants and the MODERN stripping / nesting.

## Open / phase 2
* **Pool shape**: the share is modelled per pixel, the bake is per vertex; the relight keeps the
  bake's (vertex-interpolated) pool shape. A per-instance vertex stream of the modelled share
  (offline at load; world solids are instanced, so per placement) would let the per-pixel pool
  replace the vertex one (rounder pools on coarse road triangles).
* **Cars**: still the native harmonic (+ Dynamic Lamps / Probes rows); phase 2: the same fitted
  lamps per pixel on the cars, and the cars receiving the lamp layers' shadows.
* **Shadow coverage**: the 12 nearest district lamps within 70 m hold layers (the lamps-and-shadows work's layer cache, at
  most 2 new a frame); farther pools keep only the cars' shadows. Lamp layers keep static scenery
  as drawn.
* **Walls**: the fit is for roads; off the roads the lamps explain little of the bake, so walls
  near lamps are relit only within the 85 % cap (an awning's shadow on a wall shows, a wall's
  painted light is left).
* Neon / signs are not lamps (no proxy lights invented); the moon / sky stays baked.
* Wider than City Core, a menu row, an iGPU tier (see Cost), and the interaction
  with Dynamic Lamps (replaced everywhere while on, also outside the district) are phase 2.
* The look is a judgement call: no native counterpart; constants are named in world.wgsl and
  `modern_light.rs`.

## Phase 2 "bold" (2026-11-05)
Phase 1 made too little visible difference, hence the bold option. Still a clone
deviation, strictly opt-in; nothing native was traced for it (there is none: the bake and the
packs' harmonic are all SPEED2.EXE has). With the row at Original the frame is byte-identical to
before (see Parity).

### Menu (Graphics → Lighting, first two rows)
| row | label id | tiers |
|---|---|---|
| Lighting | 0xC1_04E0_9A | Original · Modern Low · Modern · Modern + RT (greyed without ray queries, `qol::rt_offered`; takes effect after a restart like SSR's RT tier: `rt::wanted` asks for the feature when `enh.modern_lighting 3`) |
| Lighting Strength | 0xC1_04E0_9B | Classic (0) · 25% · 50% · 75% · Bold (100, default); greyed while Lighting is Original |
Stored as `enh.modern_lighting 0..3`, `enh.modern_strength 0..100` (options.txt); presets leave the
row at Original. Ids checked unused in this copy and in the main checkout (2026-11-05).
The hidden switch of phase 1 still works (`NFSU2_MODERN_LIGHTING`, strength from
`NFSU2_MODERN_STRENGTH`, default 0 = phase 1's look).

### Design (`src/render/modern_light_bold.rs`, world.wgsl `// BEGIN MODERN` / `// BEGIN MLRT`, car_modern.wgsl)
Phase 1's bake-anchored relight is kept and opened up by the strength *s*:
`light = (bake − share·k)·A + lit·k'·B + min(other lights, 0.45)·headroom`
* **share / k** as phase 1 (the fitted lamp share, at most 85 % of the bake per channel);
  **A** = 1 → 0.5 (the rest of the bake: the artists' paint, sky, bounce — the "painted
  darkness" deepened); **B** = 1 → 1.6 (the relit lamp pools); the pools' softening 150 → 40 m²
  (rounder, tighter per-pixel pools); **k'** = k → 1 − 0.7·(1 − k) on the ground only (`ng.z`
  > 0.3…0.7: the road and pavement take the dynamic pool, walls and foliage keep phase 1's cap so a
  lamp beside a facade does not wash it out).
* **Neon and signs as lights** (`emitters`): the world's additive textures named SGN_* / *NEON* /
  *_SIGN (not flare / mist sprites, fake light beams, the city grid, headlight glows, windows,
  race-only barriers; inventory: the maintainer's probe `modern_light_inventory`), merged per 5 m cell and
  section: area-weighted centre, mean emitted colour (texture mean × alpha × vertex colour × 2),
  area-weighted normal; a point light at full hue, reach 8–24 m and strength by the area, lighting
  only the half-space in front of the sign, faded in from 1–3.5 m (a sign never lights its own
  facade), mostly the ground (walls × 0.15). Bayview: 5716 emitters, 6 ms at load.
* **Car lights** (`car_lights`): every car's tail lights (a red light behind its box, reach 5 m)
  and the other cars' headlights as 70° spots (28 m) unless Headlight Beams draws them; the
  player's road keeps the native projector. A car's own lights skip its own shadow ellipsoid.
* **Shadows**: 24 cached lamp layers at 1024² (the lamps-and-shadows work's layer cache; 4 new a frame; reach 110 m;
  Low: 4 at 512², one compare), the cars as soft ellipsoids (Low: the nearest car only), at most
  12 shadowed lights per pixel (Low 4). **Modern + RT**: one ray query (the ray-traced reflections work's traced scene: the
  world's low LOD and every car's LOD C, rebuilt a frame before the world draws,
  `prepare_modern_rt` in game.rs, shared with RT reflections) toward each of the 4 strongest
  lamps / signs per pixel; cars cast true shapes, every lamp in reach shadows (not only the 24
  layered); the rest falls back to layers / ellipsoids.
* **Specular**: GGX from every light on the ground also dry (roughness 0.55 → 0.38 by *s*),
  wet roughness 0.22, × 1 → 1.6.
* **Cars lit by the same lights** (`car_lamps` → car_modern.wgsl `car_modern`): the four strongest of
  the frame's lights at the car, per pixel with the world's falloff, a Blinn highlight, and the
  harmonic × 1 → 0.75 (the car's baked light darkened as the world's).
* Strength 0 = phase 1 exactly (no new light, A = B = 1, cap kept).

### A/B evidence (the maintainer's private captures, 1920×1080 `--shot`, `scripts/scenes.sh GAME OUT`)
Each `<scene>.webp` = Original | Ultra (perf.sh's preset) on top, Modern | Modern + RT below
(strength Bold, the default). `<scene>_low.webp` Original | Modern Low; `strength_<scene>.webp`
Classic (0) | 50 % | Bold | Ultra + Modern. Spots: This page's (street, neon, tunnel, open, garage,
rain, race start) and four more found by the maintainer's probe `modern_light_spots` on the free-roam routes
inside the World Map's zoom boxes (0x7F42C8): **neondistrict** (−394.0, −531.1: the most neon /
sign light of Bayview, the casino), **coal** (Coal Harbor East docks, 299.2, −700.5), **coalwest**
(Coal Harbor West, −523.2, −511.6), **jackson** (Jackson Heights hills, −897.9, 726.1).
What changes: the darkness between the lamps deepens (the bake's painted level × 0.5 at Bold),
the lamps' pools are per-pixel and brighter on the ground, the cars throw shadows into every pool
(soft ellipsoids; true shapes with RT), the tail lights put a red pool behind every car, the neon
tints the pavement under the signs, the dry road takes a specular from every light, and the cars
darken with the street and light up under the lamps. Walls, signs and the sky keep the art's
light (only darker); scenes stay recognisably NFSU2.

### Cost (measured, `scripts/costs.sh`: --bench 600 frames, 3 interleaved reps, median of the runs' median GPU busy ms; fast release build)
reference desktop GPU, strength 75 % (the cost does not depend on the strength once it is above 0):

| config | Original | Modern Low | Modern | Modern + RT | Ultra | Ultra + Modern |
|---|---|---|---|---|---|---|
| explore 1080p | 0.636 | 0.756 | 0.783 (+0.15) | 1.050 (+0.41) | 2.741 | 2.901 (+0.16) |
| race start 1080p | 0.961 | 1.751 | 1.870 (+0.91) | 3.169 (+2.21) | 4.618 | 5.864 (+1.25) |
| racing 1080p | 0.774 | 1.078 (+0.30) | 2.158 (+1.38) | 3.338 (+2.56) | 4.354¹ | 6.226¹ (+1.87) |
| rain 1080p | 0.831 | 1.421 | 1.557 (+0.73) | 2.815 (+1.98) | 4.883 | 5.403 (+0.52) |
| racing 4K | 2.569 | 3.391 (+0.82) | 5.347 (+2.78) | 8.148 (+5.58) | 14.812¹ | 18.761¹ (+3.95) |
The racing rows are the final build (2 reps, strength Bold; Low after its last cut); the others
and ¹ the build before Low's cut and the cars' packed indices (3 reps, 75 %; Modern / RT within
~0.2 ms of the final; the RT rows predate its grazing-face fade, a few ALU ops). Target "≤ +2 ms at 1080p Modern" met in every scene (worst: racing +1.38). Draw CPU +0.1–0.4 ms
(the merged light list and its clusters, the emitters' tagging, the car lights). Load: the
emitters take 6 ms per region load.

**iGPU class** (the reference 2-CU RDNA2 iGPU, `IGPU=1`, one run each, ms GPU):

| config | Original | Modern Low | Modern | for reference: Dynamic Lamps + Lamp Shadows Low (the Modern preset's) |
|---|---|---|---|---|
| explore 1080p | 12.2 | 24.2 | 25.7 | — |
| racing 1080p | 16.1 | 35.7 | 55.1 | 49.4 (separate run) |
Modern Low = the 4 nearest lights of the pixel's cluster, the fitted pool relit as it is (one
evaluation), 2 shadowed (one compare), the nearest car's shadow, no dry specular, 4 layers at
512² made at the lamps-and-shadows work's rate. It is cheaper than the lamps the Modern preset already turns on there, but
still ≈ +20 ms in a race (rerun 2026-11-05 10:25 outside another agent's GPU-starvation window, during the parity run's CPU load) on a 2-CU iGPU (an experiment without lights showed ≈ +8 ms is the lamps'
module and layers alone, the rest is per light × the world's overdraw): playable at 30 fps there,
not at 60.

### Files and hooks
New: `src/render/modern_light_bold.rs` (tiers, params, emitters, car lights, tagging, the cars'
lamps, the RT frame slot and group), the maintainer's probe `modern_light_inventory`,
the maintainer's probe `modern_light_spots`, the maintainer's private captures (+ `scripts/scenes.sh`,
`scripts/costs.sh`). Hooks: world.wgsl's MODERN block (rewritten: `MlParams`, `ml_relight` with
the strength, `ml_near_pool`, the MLRT ray query), `src/render/car_modern.wgsl` (`car_modern`, appended to a car module form
by `modern_light_bold::car_source`, selected per draw by a negative `lamps[1].w`; `car.rs` takes the
form as module bit 4 while the row is on — car.wgsl itself is base's text: a first build that put
the branch into car.wgsl moved a few car pixels by 2–4 at Original through the driver's codegen,
caught by the parity gate), `lamps.rs::plan_lamps` (the tier's layers,
the merged light list, the params record at index 1040, the RT group), `lighting.rs`
(`WorldLights::car` takes `car_lamps`), `mod.rs` (`SceneMode::modern`, the world module per tier
with the ray-query directive and binding 11, `frame_layout_with`, `load_emitters` at a region's
load), `rt.rs::wanted`, `game.rs` (`prepare_modern_rt` before the world draws; `attach_rt` reuses
its scene), `qol.rs` (`enh.modern_lighting`, `enh.modern_strength`), `options.rs` /
`graphics_table.rs` (the rows), `modern_light.rs` (the district follows the row;
`source_tier`).

### Tests
`render::modern_light_bold::tests`: the params from Classic to Bold and per tier, the layers per
tier, the emissive names, the emitter's light (area, hue, facing), the car lights (behind the box,
owner, the player's pool skipped, Headlight Beams), the light pool (pack = phase 1's, emitters'
near fade), the cars' lamps (off without params, own light skipped), and **every tier's world
module (plain, texture arrays, mip bias) parsing and validating in naga**, the params index, MLRT
nesting and stripping (this caught the ray-query directive landing after the mip-bias form's
declarations), and the cars' Modern module forms (paint, wet, surface) validating with car.wgsl
untouched (`car_modules`). Updated: the Graphics page's row order and defaults, the options round trip.

### What remains (phase 2), and why
* **iGPU**: Modern Low is ≈ +20 ms in a race on the 2-CU RDNA2 (≈ 30 fps there); the lamps'
  module and layers alone are ≈ +8 ms (the lamps-and-shadows work's rows cost more on it). A per-vertex light stream would
  be the next step for that class; not done (the world solids are instanced: a stream per
  placement and its upload are a unit of their own).
* **Cars do not receive the lamps' shadow layers / rays** (they are lit by the four strongest
  lights per pixel, unshadowed; their harmonic keeps the bake's level × the ambient). Binding the
  layers and the scene to the car pipelines changes their layout (a car-renderer unit).
* **RT on curved signs**: the traced world is the low LOD; on curved facades (the casino) a lamp's
  ray can still hit the proxy where the drawn face is lit (faded on grazing faces, wall rays
  start 0.6 m out). Rays: the 4 strongest lights per pixel; the rest use the layers / ellipsoids.
* **Pool shape on the road** still carries the bake's per-vertex pattern in its residual (the
  share taken out is per pixel, the bake per vertex); at Bold the ambient × 0.5 hides most of it.
* Neon is approximated per 5 m cell (no area lights proper, no textured light); the windows and
  the sky's light stay the bake's.

### Parity (row at Original)
`perf.sh parity` <base> (perf.sh build) vs this copy with both fixes below (md5
7ac11e1b604f51179e358ec2fe1624f8), ENH_PRESETS="modern ultra extreme extreme_fsr": Original dumps
of explore/race/racing/drive/rain at 1280×960 and 3840×2160 AE 0, the 144 Hz window path AE 0,
Modern/Ultra/Extreme/Extreme-FSR AE 0, 15 shots AE 0 (no panic/validation), audio identical, state
hashes identical over firstrace, firstdrag, postrace, the playthrough (82 412 steps) and the
parity scenario (internal evidence).

### Fixes after the first play check
* **Windowed start crashed** ("Surface::configure: Failed to wait for GPU to come idle") on a
  Wayland desktop. Not Modern Lighting phase 2's: <base> (main) built from its own tree crashes the same
  way there, 2 of 2. wgpu-core 30 waits for the queue to drain before it reconfigures a surface and
  reports this error when a submission lands during the wait; the preload thread builds the world
  renderer and submits its uploads while the window is first resized. `game.rs::configure_surface`
  (all five configure sites) retries that one error after the queue drains (logged; any other
  error, or 100 in a row, stays fatal); test `configure_surface_tests`. On the player's desktop the
  fixed build then started 3 of 3 (1–2 retries each).
* **Modern + RT froze in rain with SSR** (render workers: "binding 11 not found in expected bind
  group layout", pipeline `fs_wet_mask`): the main view's group with the traced scene was handed to
  every pipeline that binds the main group (the SSR mask, the cover shadow and the native ones
  have the plain layout). Now `lamp_main` stays the plain group and `lamp_main_rt` is bound only by
  the scene's pipelines while they are made with the RT layout (`WorldCore::scene_group`: the main
  draw, LOD fade, car shadows, particles, glows, characters). Matrix check
  internal evidence: every tier × the play setup / Ultra /
  Extreme × explore, race start, racing, rain, headless (NFSU2_RT=1), fails on any panic or
  validation error.

### 2026-11-05: soft, stable shadows
See `docs/shadow-softness.md`: the cluster index list no longer empties the far clusters (the casino
flicker), Modern + RT traces the 4 strongest lights per pixel as area lights (2 rays each, 2×2
interleaved and resolved over the quad), shadows keep 40 % of the unshadowed light, Contact Shadows
take the darker-of, NIS characters are lit without the face normal, Dynamic Lamps / Lamp Shadows High
greyed while Modern Lighting is on.

## Bake-anchored darkening (2026-11-05, docs/look-direction.md rule E3)
At Bold the strength halved the rest of the bake everywhere and boosted the relit pools ×1.6: over the
The look direction spots the mean luma fell −12.6, the highlights (p95) −34 and the native glow −0.44 (the garage's
lit turntable p95 −100). `ml_relight` now fades the strength's ambient, boost and emitter light out
where the bake itself is bright (`ML_KEEP_LO` 0.06 → `ML_KEEP_HI` 0.22 baked luminance): the darkness
between the lights still deepens, a brightly baked surface keeps the artists' level and the relit share
is redistributed, not added. Now (Modern + RT, Bold, 10 the look direction spots) mean −3.3, p95 −2.8, glow −0.1: darker between the lights, the art's lit surfaces kept. Strength 0 is unchanged (phase 1).

### 2026-11-05: Modern + RT's rays at half resolution
The rays left the world's colour shader: a compute pass after the depth pre-pass traces the two
strongest lights per 2 × 2 block, the colour pass gathers them (`render/rt_shadow.rs`; design in
`docs/shadow-softness.md` "2026-11-05: the rays at half resolution, outside the colour shader", numbers in `docs/perf-budget.md` "The new effects' costs": racing 4K with the player's
settings RT +7.4 → +2.7 ms over Modern). NIS characters take the blocks too: drawn into the depth first and
the blocks traced again over their box (docs/shadow-softness.md).
