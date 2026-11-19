# Lighting II: Headlight Beams, Light Probes, Enhanced Glass: how it works

The technical part of [Lighting II: Headlight Beams, Light Probes, Enhanced Glass](../../playing/settings/lighting2.md): how it works, what it costs, how it was checked and what is open. The player part (the rows and what they do) is on that page.

## Overview

Three Graphics rows, all **Off by default** (native, byte-identical: parity gates
below). Nothing here exists in SPEED2.EXE except what is marked traced.

| row (group) | values | options.txt | code |
|---|---|---|---|
| **Headlight Beams** (Lighting) | Off / On | `enh.headlight_beams 0/1` | `render/lamps.rs` `beams`, world.wgsl `beam_light` (LAMPS blocks) |
| **Light Probes** (Lighting) | Off / On | `enh.light_probes 0/1` | `render/probes.rs`, `lighting.rs` `CarLighting::in_world` |
| **Enhanced Glass** (Cars) | Off / On | `enh.enhanced_glass 0/1` | `render/car_glass.rs`, `car_glass.wgsl` (own module) |

`qol::headlight_beams`, `qol::light_probes`, `qol::enhanced_glass`; label ids 0xC1_04E0_B0..B2 (the weather package took 0xA0..0xA4).
The Next-Gen car-lights work (docs/playing/settings/car-lights.md) added a third Headlight Beams value, **Next-Gen** (`enh.headlight_beams 2`): the beams plus the car lights in the air, on the road and on the cars, and lens streaks.

## When the original turns car lights on (traced, verified under Wine)
* Lights-on flag: car +0x93D (the same byte 0x615640 tests for the headlight glows, 0x6154A0 for
  the headlight pools, and 0x60F5AF for the projector's HeadlightColor, which is 0 when it is clear).
* Writers (`speed2.dis`, every reference to +0x925/+0x93C/+0x93D): the constructor 0x6045B0 sets
  +0x93D = 1 and +0x93C/+0x925/+0x926/+0x927/+0x928 = 0; **0x5F61A0** (`Car_UpdateLightsOn`, called
  each car update at 0x6049D2): owner kind ([[car+0x14]+4]) 1 returns at once (the player's car keeps
  the constructor's 1); otherwise, with +0x925 clear, +0x93D = 1 (0x5F62B1). Only with +0x925 set does
  it run the flicker timing (0x5EC170 `Car_SetLightFlicker` parameters, +0x92C stamp, +0x930..+0x938
  on/off times, +0x93C as the off state). +0x925 is only ever **written 0** (0x6045C3, 0x5F61E1,
  0x5F62AA), so the flicker never runs; 0x40D5C0 (AI honk/flash at a human) stores flicker
  parameters through 0x5EC170 that nothing reads. 0x5FB6B1.. only registers the bytes with a
  watch/serialiser (0x571650).
* So **every car's lights are on all the time in game**: no night/day, tunnel or time-of-day switch
  (NFSU2 is always night). Under Wine (`fastgpu.sh`, own copy, :229, free roam at the airport, 45 s
  incl. 35 s driving, `[0x890080]` car list walked once a second, `scripts/lights_on.py`): 541
  car-samples (27 player = kind 1, 82 kind 2, 432 traffic kind 3), **all +0x93D = 1, +0x925 = 0**
  (internal evidence, `freeroam_drive.webp`). A race was not logged
  (**open**, but no code path differs).
* The clone has no +0x93D: beams are on for every car that has a headlight pool (the player, race
  rivals, explore traffic, the shop companion) — the cars whose pools the native draws. A beam
  fades in/out only with distance (110–150 m from the eye), not by a light state, because the
  native state never changes.

## Headlight Beams
The native headlight projector (view 9, worldheadlight.efx; formats.md "The headlight pass") is the
player's only and lights only the instances view 9's size cull took. With the row on, **every car**
gets view 9's projector, unchanged: eye = car + 0.75 m up, forward − 0.1 z, 2.5 rad across × 0.6
down, `HEADLIGHTS` (128×64, the soft cookie with its bright centre) × HeadlightClipTex (the near
cut-off at ~3 m, gone by ~48 m) × HeadlightColor (render info +0x380 / 128, clamped to 1 as c0).
Changes against the native projector:
* applied per pixel to **every fogged surface** in the frustum (road and walls), not only the culled
  instances; added to the light term as worldheadlight adds it (`tex × (beam + v0) × 2`);
* a facing term: for other cars' beams `clamp(0.85 + 0.6 n·l) × smoothstep(−0.15, 0.05, n·l)`; for the
  player's (the native projector) `smoothstep(−0.1, 0, n·l)` (the second graphics-audit round, audit fix 8: the road grazes the
  beam, n·l 0.02–0.1, where the first form took 10–15 % of the native pool — tunnel road box −6.6 %, dusk
  −7.4 %; now 0.00 % with the row alone): every face toward the car takes exactly the native energy, faces
  turned away none (the native projector lit through walls);
* the player's beam is the first; with Lamp Shadows its lamp-shadow headlight layer (now drawn from the
  beam's own frustum, whatever Customize Display's Car Headlight says) and the other cars as soft
  ellipsoids shade it; the native projector term is then skipped (no double light);
* other cars' beams (at most 15, nearest first) have **no shadows** and are weighed by the bake's
  headroom (× 0.8 × (1 − smoothstep(0.2, 0.5, lum v0))): on a road the vertex colours already light
  brightly (the race start under the street lamps) four beams washed the texture out (first try:
  road mean +15 levels at the City Core start; now +7).
  **The look direction (2026-11-05, docs/notes/graphics/look-direction.md rule E2):** × 0.3 × (1 − smoothstep(0.1, 0.25, lum v0)) —
  the other cars' beams are new light held to the dark-gap budget (the drift start's grid: +10.7 mean,
  p95 +58 at 0.8; now +1.5 / +2.4).
Shader: world.wgsl LAMPS blocks only (`world_source(false)` strips them); beams sit after the
clustered lamps in the lamp list (`Frame.cluster_near.z` count, `.w` first). The lamp module is
chosen while the row is on (`SceneMode::lamps`).

## Light Probes
* Placement: along the world's road and race splines — `TRACKS/ROUTES<region>/RoutesFreeRoam.bin`
  and every `Routes<track>F.bin` of the region (the AI route records' control points), one every
  24 m, 1 m above the road, deduplicated within 12 m: 6 172 probes in Bayview (L4RA), 428 in the
  drag region L4RB. The region comes from the world's cache key (`TrackCache::region`).
* Bake: the native world rendering of the car reflection cube (the environment pipelines, LOD and
  cull of the cube; same textures, baked vertex colours, emissive signs and neon), 16² per face,
  world-axis faces, read back and averaged per face into an **ambient cube**. Probes within 180 m
  of the player's car (the camera before: a camera flying away from the car — replays,
  photo mode, the fountain orbit — kept baking), nearest first, one per frame (the read-back a
  frame later); arriving where probes are within 40 m but none of them is baked (a load, a reset)
  bakes the nearest 48 unbaked within 70 m at once (a hitch of the order of a section load).
  Kept for the world's life; a baked probe is never baked again unless a drive's set-up switches scenery it
  sees (`WorldRenderer::new_drive_probes`, docs/notes/graphics/ambient-detail.md "Kept-world caches": the probes whose
  ±`ENV_FAR` box holds a shown, hidden or moved placement are given up, those within `BURST_RADIUS` baked
  again in the drive's first frame; GPU cost below) (`ProbeField::plan`; the frame-budget fixes fixed
  the arrival test, which also held with *no* probe within 40 m and then re-baked up to 48
  baked probes every frame: 818 bakes of 118 probes in the 600-frame fountain orbit, now 100).
* Use: `CarLighting::in_world` blends the nearest four baked probes within 40 m (inverse distance)
  into the car's harmonic before it is turned into the car's axes, as quadratic terms (per axis
  d²(P+N)/2 + 0.75·d(P−N)/2, the least-squares fit of a cube); **rgb only** — the specular channel
  (the highlight's strength) is left as it is. **the first graphics-audit round (2026-11-12, audit fix 2)**: the probe
  *replaces* the ambient it models instead of adding to it (`probes::ambient_terms`, `face_ambient`):
  per world face the car harmonic's own colour there is replaced by the probe face's hue at the car's
  own luminance, each channel within ±`HUE_CAP` (0.2) of it, lifted by at most `LIFT_CAP` (5 %) where the
  probe is brighter than a lit street (luminance 0.2..0.6), blended in by how lit the probe face is
  (0.04–0.15, smoothstep); a dark face changes nothing (**the dark gaps are not lifted**). Before, the
  probe's chroma × 2.5 and its brightness above 0.2 were added on top without a bound: the arcade car
  went R +31.5 % / mean +13.6 % (glowing red paint) while the street car stayed. Now (car box vs
  Original): neon R +5.3 %, mean +1.4 %; harbour R +0.5 %; street mean +0.2 %; hood −1.0 %
  (`audit/gfx/b1/sheets/neon-probes.jpg`). The env cube is left as it is (it is captured at the
  player's car every frame at World Detail 5); the probe colours reach the reflections through the
  harmonic's highlight colour.
* Bayview at night is mostly sodium-orange: the most coloured probes on the drive bench route
  (chroma ≤ 0.25) are warm; cars pick up that warmth (+2.7/+2.2/−3.6 levels, R/G/B mean of the 206 at the City
  Core start, 1280×720, `start_probes_zoom.webp`), a blue car turns slightly teal in the tunnel's yellow light
  (`tunnel_probes_zoom.webp`).

## Enhanced Glass
Glass = the alpha-blended draws whose light material is WINDSHIELD, WINDOWMASK, WINDSHIELD_TINT_*
(the Window Tint part's, 0x638DC5) or CLEARPLASTIC / HEADLIGHTGLASS (lamp covers) — the glass_alpha
materials of `car.rs` without MOLDINGS. With the row on (in game; front-end scenes keep native glass):
the car pass leaves them out; after it the scene is copied (resolved when multisampled) and the
glass is drawn over the copy by `car_glass.wgsl` (car.wgsl + appended stage, its own module):
`glass.rgb·a + behind(here + n_view.xy·bend)·(1 − a)·(1 − F/2) + env(per-pixel reflection)·F·0.5` —
the native colour and alpha (the tint) stay the base, `behind` is the copy read through a screen-space
refraction (bend 1.8 % of the screen for windows, 3.5 % for lamp covers, less face-on), F = Schlick
(F0 0.04) on the per-pixel n·v, env = the car's cube along the per-pixel reflection (camera axes, or
world axes for Enhanced Reflections' cube). Written opaque, each car's glass far to near (a nearer
pane over a farther one wins; the farther pane's tint behind it is lost: **inferred** minor). SSR's
car mask still takes the glass in its own pass, so with SSR on the glass also gets screen-space
reflections as before.

**What car.efx draws over the glass stays over it; the copy only where the glass reads.**
The composite writes the pane opaque, so a draw blended *after* the glass in the native order was
lost under it: the Lancer's `DECAL_REAR_WINDOW` part (the NFSPEED sticker) comes after the body's
rear WINDSHIELD_TINT, and the composite drew the pane over it. Now (`car_glass::mark_after_glass`,
`Draw.glass_after`) every blended draw after a car's first glass draw — decals, the lamp sprites —
and a glass draw laid on an earlier one of the same part (a second layer of one pane, centres
within 5 cm: the rear WINDOWMASK over the tint) leave the car pass too and are drawn after the
glass composite, in their native order, with the pipelines the car pass used (`glass_with`: native,
the scene mode's or LOD Fade's). Draws car.efx draws *before* the glass (the body's own decal under
the pane) stay in the car pass and are seen through the pane as before. And the scene copy
(single-sampled scenes: SSAA / no MSAA) is drawn only over the cars' glass rect: each car's glass
box (model space, + 0.3 m) projected, their union grown by the refraction's reach (5 % of the view)
— the only texels the composite reads (`copy_rect`; the whole view when a box reaches behind the eye;
no copy and no glass when all of it is off screen, the draws after it still go). Exact: Ultra shots
with and without the rect AE 0 (street, race start, drift, rain street, docks). Multisampled scenes
keep the full resolve (a resolve cannot be scissored). Also fixed: with TAA / TAAU / FSR 2 the glass
pipelines failed validation (`MIP_BIAS` not in the glass module; the game panicked as soon as a car's
glass was drawn): the glass module is now biased like the car's (`temporal::biased_source`).

**With Wet Car (docs/notes/graphics/weather.md): film first, glass second.** The glass pass builds its module
from the car's own module for that frame — car.wgsl, Enhanced Paint's or Wet Car's
(`car_source(paint, wet)`) — with car_glass.wgsl appended, picked per car by `wet_now`. So a wet
pane's `car_colour` already holds the water film (drops, rim darkening, streaks; car_wet.wgsl keeps
the water's light at full strength whatever the pane's alpha), and the glass composite then reads what
is behind through it and adds the pane's Fresnel reflection on top. The order is a choice
(**inferred** as the plausible one: the water sits on the outer face, so what is seen through the
pane is bent by the glass and the drops are seen on it unbent; refraction by the drops themselves is
not modelled). A dry car (film ≤ WET_MIN) uses the dry glass module, as its body uses the dry car
module.

## Native byte-identity
`scripts/perf/perf.sh parity`, base (this copy before this work, full release) vs this work (full release),
all three rows off: **PARITY: all gates passed** — Original dumps of explore/race/racing/drive/rain at
1280×960 and 3840×2160 AE 0 (10 sets), the 144 Hz window path AE 0, Modern/Ultra (perf.sh's preset
strings, without this work's rows) AE 0, 15 standard shots AE 0, explore/race WAVs identical, state
hashes identical over firstrace, firstdrag, postrace, the playthrough (82 695 steps) and the parity
scenario (internal evidence). How it is kept: the beams are only in
world.wgsl's LAMPS blocks (the native module strips them), the glass is a module of its own
(car.wgsl + car_glass.wgsl) and its car-pass hook is a branch on a CPU flag (`glass_split`, false
when off), the probes only add to the CPU harmonic when on.

## Cost
`scripts/bench.sh` (`--bench racing 600`: a night race start and the field driving, native 8x
MSAA; 3 runs per setting interleaved, GPU timestamps of the pass each adds to; the machine was
shared, whole-frame medians at 1080p moved by ±0.3 ms and are not used; raw: `costs.txt`):

| row | 1080p reference desktop GPU | 4K reference desktop GPU | iGPU 1080p (the reference 2-CU RDNA2 iGPU, 1 run) | class |
|---|---|---|---|---|
| Headlight Beams | ≈0 (world pass 0.40 → 0.40) | +0.21 (world 1.08 → 1.29) | +4.6 (world 8.0 → 12.6) | Low (iGPU High) |
| Light Probes | GPU ≈0; CPU ~0.1 ms/frame on the main thread while probes are baked (the 600-frame drive, the arrival burst included) | GPU ≈0 (≤ noise) | ≈0 | Free GPU / CPU 0.1 |
| Enhanced Glass | +0.11 (car pass 0.15 → 0.26) | +0.33 (0.30 → 0.63: the 8x resolve and the copy at 4K) | +0.7 (cars 4.4 → 5.1) | Medium |
| all three | +0.23 | +0.56 | +5.3 | |

Presets by cost: **Modern** gains Headlight Beams and Light Probes (next to the weather package's Wet Car, Rain Detail, Spray), **Ultra** also Enhanced Glass (next to Rain Detail High; left Ultra in the effects-tiers pass for the hidden window decals, back in the final effects pass: +0.11 / +0.35 ms at 4K racing / race start under Ultra)
(`qol::Preset::apply`); Original / Restore Native off. The rows' detail lines carry these costs
(`graphics_table.rs` BEAMS_COST_4K, PROBES_CPU_MS, GLASS_COST_4K).

## Evidence (the maintainer's private captures, 1920×1080 `--shot`, full release build)
`scripts/scenes.sh GAME OUT` renders (with `RAIN=1`: heavy rain, clock 170 s, Wet Roads + Wet Car
+ Rain Detail + Spray on in every setting; those crops are `rain_{night,tunnel,start,crowd}_off_all.webp`, `rain_user_start_before_after.webp`,
`rain_night_glass_zoom.webp`): night (explore, clock 93), tunnel (explore at
(−1701.3, 2178), inside the Observatory tunnel), neon (explore at (295, −554): the warm-lit city
street with the most coloured probes found), start (Quick Race grid, City Core signs), crowd
(S2_STREET_4 start from above, rivals' beams), garage (qrcars, front end). Each `<scene>_off_all.webp`
= Original | all three rows; `user_<scene>_before_after.webp` = the player's real options
(the play setup's options file) | the same + the three rows; zooms `start_{beams,probes,
glass}_zoom.webp`, `night_glass_zoom.webp`, `tunnel_probes_zoom.webp`. Changed pixels (AE, any
channel) vs Original: crowd all 2.9 % (beams), start 0.79 %, neon 0.17 %, tunnel 0.13 %, night
0.07 %; garage 0 (front end: no beams, probes or glass by design).

## Tests
`render::lamps::tests::beams_are_the_native_projector_on_every_car` (packing = `headlight_uniform`,
player first, nearest others, fade, cap, beams only in the lamps' module);
`render::probes::tests` (dark faces add nothing, hue at constant luminance; the ambient cube's
harmonic terms; nearest-probe blend and reach; face means and slots; `bayview_probes_cover_the_roads`
asset test, skips without NFSU2_ROOT); `render::car::glass::tests` (glass kinds by material; the
module only appends to car.wgsl and keeps the native colour/alpha as base); `qol`
`enhancements_round_trip` (45 deviations), options row order test.

## Open
* Cars are not lit by other cars' beams (world surfaces only); traffic beams cast no shadow.
* Probes: the bake sees the camera's visible section list, the fog and the time of the frame it is
  baked in (a probe baked while it rains keeps the rain's fog); probes between roads (plazas off the
  splines) are absent. A drive's set-up switch bakes again only the probes that see it; a probe's
  freshness is not kept per probe for these other inputs.
* Re-bake cost at a set-up switch (GPU timestamps, `tests::kept_world::light_probes_rebake_cost`,
  320×240 offscreen, Light Probes on; "world: light probes" in the drive's first frame / the next nine;
  the same set-up again gives up none and costs nothing extra; three runs each):
  | switch | probes given up | discrete GPU first / next 9 | integrated GPU first / next 9 (load ≈ 36–40) |
  |---|---|---|---|
  | URL 4701 → 4703 | 26–30 | 0.96–0.97 / 0.30 ms | 1.7–2.7 / 0.36–0.65 ms |
  | URL 4701 → 4701 | 0 | 0 / 0.28 ms | 0 / 0.49–0.88 ms |
  | drag 4222 → 4223 | 13 | 0.32–0.33 / 0.20 ms | 0.43–0.92 / 0.29–0.67 ms |
  | drag 4222 → 4222 | 0 | 0 / 0 | 0 / 0 |
  The drive start's check itself: 0.08–0.17 ms CPU (6 687–12 375 placements) behind the loading screen. With
  Light Probes off nothing is recorded or checked (a field left from before is dropped).
* Glass in the front end (garage, showroom) stays native (its render paths have no glass pass); the
  rear-view mirror keeps native glass.
* The look of all three is a judgement call (no native counterpart); constants are named in the code.
* `NFSU2_PROBE_LOG=1` logs the bake (placed/baked, the probes near the eye, each probe's cube) and
  the cars' probe terms (a check aid).
* The arrival burst (every probe within `BURST_RADIUS` when the car arrives where none is baked) draws each
  probe into a read-back buffer of its own and waits for the GPU once for all of them;
  `NFSU2_PROBE_BURST_SERIAL=1` waits after each probe instead (a check and measuring aid; the baked field is
  the same, game test `the_arrival_burst_bakes_the_same_field_with_one_wait`; docs/notes/performance/budget.md "Load times").
