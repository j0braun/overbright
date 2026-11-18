# Next-Gen car lights (clone deviation, opt-in)

The **Headlight Beams** row (Graphics → Lighting)
gains a third value: **Off / On / Next-Gen** (`enh.headlight_beams 0/1/2`; `qol::headlight_beams()` is
true for 1 and 2, `qol::car_lights_next_gen()` for 2). No new label id (tier names are strings; the row
keeps 0xC1_04E0_B0). Off and On are unchanged (Original byte-identical; the gate's Extreme preset has the
row at On and stays identical too). Next-Gen is in no preset (manual, like Particles Cinematic).

Nothing here exists in SPEED2.EXE and nothing new was traced. What the original decides is read, not
changed: which cars have lights (every car, always on: car +0x93D, docs/lighting2.md), where the lamps are
(the car models' light markers, `glow::CarFlare`, as 0x615640 places its glow sprites), the headlight
colour (render info +0x380, clamped as HeadlightColor 0x5BDAB0) and the brake state (car +0x944 via
`CarState::brake_lights`, held 0.5 s for racers as `glow_quads` reads it). Every look constant is
**inferred** (by eye) and named in `render::car_lights::LOOK` (`NFSU2_CARLIGHTS=k=v,…` overrides by name).

## What Next-Gen adds

| part | where | what |
|---|---|---|
| frame data | `game.rs` `publish_car_lights` → `car_lights::publish` | every drawn car (player, companion, rivals, encounter cars, traffic): lamps in world space (head L/R, tail L/R, brake L/R/centre while braking), box, headlight colour, underglow colour when a lit `%s_NEON` is drawn |
| headlight cones in the air | `car_lights.wgsl` `fs_march` (`CarLightsPass`, after Volumetric Fog in `PostFx::pre_passes`) | the 8 nearest cars' cones: the view ray clipped analytically to the projector's pyramid (view 9's axes and scales from `headlight_uniform`, the cookie's lit part), 12 steps with an interleaved-gradient offset; per step **both lamps'** HEADLIGHTS cookie (the native texture, squared: its bright centre is the shaft) × HeadlightClipTex (the native beam's near cut-off and reach) × 1/(1 + z²/100) × Henyey–Greenstein (g 0.3, 60 %) × a drifting two-octave density noise × the native fog × 60 m extinction |
| lamp halos in the air | same pass | every lamp (≤ 64) as a point light: closed-form in-scattering ∫ds/(h²+s²) along the ray (softened core 0.18 m, window 6 m), the lamp's lobe (headlights forward, tails backwards), the same phase; brake lights 3× the tails |
| density | `air_density` | 0.035 /m dry × (1 + 1.5 × rain) (the rain's amount `Weather::amount`); soft knee x/(1 + 0.8 max) so a camera inside several cones in rain stays under white |
| resolution | `divisor` | at most 720 rows (1080p ½, 4K ⅓, 4K with SSAA 2x ⅙); depth-aware 4-tap upsample using the marched distance stored in alpha |
| lens streaks | `vs_streak` / `fs_streak` | per visible lamp an anamorphic horizontal streak (≤ 16 % of the screen height, headlights cooler/bluer, tails red) and a soft, whiter core; occlusion from 5 scene-depth taps (a lamp ≤ 1 m behind the depth still counts: the markers sit inside the body); scaled by facing, brightness, distance and the native fog; × 1.6 in full rain (wet lens). Added on top of the native glow sprites, which stay as they are |
| road light | `lamps.rs` `plan_lamps` + world.wgsl `car_glow_light` (in `beam_light`'s loop, LAMPS blocks) | after the beams in the lamp list (≤ 24 within 70 m and not behind the eye, within the buffer before Modern Lighting's params record): per car a tail/brake point light (reach 4.5 m, 7.5 m braking; full behind, a quarter ahead) and the underglow as a soft rectangular **area light** on the ground under the car (light within 1.5 m of the car's footprint, toward the nearest point of the rectangle; the max-rain halo fix gain 0.15, was 0.45 / 1.8 m: docs/wet-road-car.md "The glow shell, the washed road and the distance veil at max settings"). With Modern Lighting's own car tail lights in the clusters only the braking part is added |
| car bodies | `lighting.rs` `WorldLights::car` → `car_lights::car_lamps` | car.wgsl's four Dynamic Lamps slots (native-weight form) take the strongest of the street lamps and: other cars' headlights the car stands in (cone 0.55..0.92 cos, 24 m), their tail/brake lights behind them, their underglows close by, and the car's **own underglow bounced off the road onto its sills** (two lights beside the car at road height, 1.7 m) |
| smoke | `game.rs` `particle_quads` | Particles Cinematic's volume gets every car's underglow (was the player's only) and the brake lights as point lights (`car_lights::smoke_points`); its own head and tail lights per car are the volumetric smoke's |

Wet roads: the road light is added to the light term as the beams' is, so the wet road shading and SSR
show it; the glow sprites' native wet streaks (`glow::reflection_sprites`) are unchanged.

## Energy budget (the first graphics-audit round, 2026-11-12, audit fix 3)
The audit's race start (`--shot race`, the grid girl in front of the grid) was **+8.6 mean / +15.6 p95**
over Original with Next-Gen alone: the night set floodlit. By elimination (`NFSU2_CARLIGHTS`): the lamps'
halos in the air (`point`) +7.0 mean / +8.8 p95 — 22 lamps, several within the halos' 6 m reach of the
camera, so each veils the whole view (∫ds/(h²+s²) with h ≈ the lamp's distance); the tail / brake light
on the road (`tail_road`) +0.5 / +3.4 (four braking cars, summed 1.75); the cones, streaks, body light
≈ 0; Beams On under it +0.97 / +3.3 (the beams replacing the native projector, not this work).
Two summed bounds, both on the CPU (`car_lights::LOOK`, tunable by name):
* `veil` 0.0015: `halo_veils` estimates each lamp's halo averaged over the screen (the shader's
  `point_halo` on a 12 × 9 grid of view rays, unoccluded, without the fog: high, never low) and caps
  every lamp at one level so the caps sum to the budget: the lamps at the camera are dimmed (race start
  to 1–30 %), a lamp whose halo is a small glow keeps it (QR start ~0.7, traffic 1). Scales `colour.w`
  only, so the lens streaks keep their strength.
* `road_budget` 0.15: `bound_glows` scales the road glows (tail / brake points, underglow areas) so
  their summed luminance stays within it: one car's tail lights (≈ 0.1) are unchanged, a braking car
  (≈ 0.36) or a grid of them light the road like ≈ 0.15.
Race start, Next-Gen alone vs Original: +8.6 / +15.6 → **+1.23 / +3.93**; Quick Race start +1.81 / +4.73 →
+1.34 / +3.13; traffic +0.89 / +3.13 → +0.76 / +2.33. The cones are untouched (beams still read as beams,
`audit/gfx/b1/sheets/race-beams2.jpg`). Next-Gen max (every top tier) at the race start: +14.3 → +10.7;
Car Lights Next-Gen's share is now ≈ 0 (Beams On instead of Next-Gen in that set: +11.1); the rest is
Sky Next-Gen (+5.7, audit fix 4) and the Extreme stack with the cars' restored highlight (the first graphics-audit round fix 1).
Cost: GPU unchanged (racing 1080p 0.99/0.99 → 0.995/0.988 ms); CPU 108 rays × ≤ 64 lamps per frame.

## Headlight Beams Off slower than On (the frame-budget gate's finding)
Not fixed here: The frame-budget fixes (`render::lamps::bin_sphere`) bins a Modern Lighting car
headlight (kind 3, `SUB_HEAD`, what `modern_light_bold::car_lights` adds per other car when Beams are Off)
by its cone instead of its whole 28 m sphere, which is the cause (inferred there; Beams On replaces those
cluster lights with the beams). Not duplicated (plan).

## Parity
Off / On: no change. The new code runs only with `car_lights_next_gen()`: `publish` stores nothing,
`cars()` returns nothing, `car_air_frame` returns None, `plan_lamps`' glows, `car_lamps` and the smoke
hooks are behind the same flag; world.wgsl's `car_glow_light` is inside the LAMPS block (the native module
strips it) and is reached only through a lamp-list entry with position.w ≥ 2, which only Next-Gen writes.
Gate: see the unit report (`scripts/checks/gate.sh … extreme`).

## Cost
GPU busy median per frame (ms), reference desktop GPU RADV, `--bench <scene> 600` at 3840×2160 with perf.sh's
**Extreme** preset (SSAA 2x: the scene is 7680×4320) and the row at On vs Next-Gen, 2 runs each, frames
after the first 20 % (internal evidence, `summ.py`; raw: `costs-4k-extreme.txt`):

| scene | On | Next-Gen | Δ | its own pass (`graphics: car lights`) |
|---|---|---|---|---|
| racing (night race, the field ahead) | 12.47 | 13.31 | **+0.85** | 0.58 |
| drive (free roam with traffic) | 14.41 | 15.39 | **+0.98** | 0.62 |
| rain (heavy rain) | 8.78 | 9.25 | **+0.47** | 0.40 |

Within the plan's ≤ +1 ms at 4K with traffic (just). The rest of Δ over the pass is the road glows in the
world pass' lamp loop and the cars' lamps. Steps that got there (racing, same preset): first version
+1.29 (pass 1.04: 12 cones × 14 steps at ⅓ of the 7680×4320 scene) → the march capped at 720 rows (⅙ with
SSAA 2x), 8 cones × 12 steps: +0.88 (0.58) → road glows ≤ 24, within 70 m and not behind the eye: +0.85.
The reduced march is not visible as such: the light it carries is smooth (noise ≥ 4.5 m features, a 6 m
halo window) and the upsample is depth-aware (edges of cars and walls stay sharp; `rain_front_ng_200.webp`
at 200 %). Row detail cost: `graphics_table.rs` CAR_LIGHTS_COST_4K (1.15 over Off).

Docs/perf-budget.md "The Next-Gen tiers — costs, tiers, GPU classes": the halos' air density is made once per lamp on the CPU
(`car_lights::air_at`, the shader's formula; `GpuPoint.colour.w`) instead of per pixel and lamp: the pass
0.62 → 0.52 ms (4K racing, the max setup), 14.0 → 10.3 ms on the reference integrated GPU (1080p); images within
4 / 255 at < 1 pixel in a million. Row hint 1.15 → 0.7 (over On, under max).

Docs/perf-budget.md "Final performance / look pass on the effects": (1) the cones' drifting density noise reads a periodic lattice
texture (`car_lights::noise_texels`, 32³ R8, repeat; the same hash per lattice point, quantised to
8 bits) with one filtered sample per octave at the smooth weights instead of eight hashes — the same
kind of field (a different seed past the 32-cell period, ~145 m), the points' CPU `air_at` reads the
same lattice; (2) the point lights' halos are binned per 16 × 16 tile of the reduced march on the CPU
(`point_tiles`: a lamp leaves a tile only where the shader's own rejects drop it for every pixel, test
`point_tiles_keep_every_lamp_a_pixel_can_see`); the loop keeps a uniform index (the lamps' loads stay
scalar) and skips the lamps the tile lacks — exact (Next-Gen shots AE 0 with (2) alone). Pass 0.52 →
0.45 ms (4K racing max), 10.3 → 9.2–9.4 ms (reference integrated GPU, 1080p); images SSIM ≥ 0.9994 (drift start:
the red haze's noise moved, 7 % of pixels by > 2 / 255, mean +0.2), motion statistics of a 40-frame
rain drive within 0.05 %. Tried and dropped: iterating only the tile's set bits (a per-pixel index made
the lamps' loads vector loads: 10.6 → 12.1 ms on the iGPU). Row hint 0.7 → 0.43.

## Evidence (the maintainer's private captures, 1920×1080, `scripts/scenes.sh GAME OUT`)
Original | Ultra (perf.sh's ULTRA + Beams On) | Next-Gen (the same, Beams Next-Gen):
`dry_front_*` (the S2_STREET_4 grid from ahead, night, dry), `rain_front_*` (the same in heavy rain,
world clock 170 s, Wet Roads / Wet Car / Rain Detail / Spray), `rain_chase_*` (the race shot's camera at
the grid in rain), `start_*` (Quick Race start, from behind), `neon_*` (`--shot neon_car`, Car Shadow
Ultra). 100 % crops `*_cars_ultra_ng_100.webp`, `neon_ultra_ng_100.webp`; 200 % `rain_front_ng_200.webp`.
Smoke: `smoke_burnout_ultra_ng.webp` (`--bench explore` frame 300, Particles Cinematic: the brake lights
glow red in the volumetric smoke). Motion: `motion_rain_race_taa_ng_f300_305_310.webp` (`--bench racing`
in rain with TAA, frames 300 / 305 / 310: cones, halos and streaks follow the cars without popping).

## Self-assessment (against FH5 / NFS Heat / Unbound at night)
* Closest to the bar: lens streaks and halos on oncoming lamps in rain, brake light in smoke, the underglow
  as a light on the road and the sills, tail / brake light on wet roads.
* Short of it: the cones carry no shadows in the air (no shafts cut by cars or posts); from the chase
  camera dry air shows the beams only faintly (by design: back-scattering); halos are analytic (no
  multiple scattering); streaks are per lamp (street lamps none); no temporal accumulation of the march
  (a fixed interleaved-gradient offset, smoothed by the upsample / TAA).

## Open
* The cones are not shadowed in the air (a car or a post in a beam does not cut a shaft; the road's beam
  keeps its Lamp Shadows layer).
* With Modern Lighting on, cars keep Modern Lighting's own car lamps (its form), so other cars' headlights
  do not light the paint there while Beams are On (Modern Lighting's car lights carry the tails).
* Lens streaks are per lamp, not from every bright pixel (street lamps get none).
* The look is a judgement call (no native counterpart).

## Under Volumetric Fog: the one volume
With the froxel medium running (any Volumetric Fog tier: Low, High and Next-Gen are tiers of it, docs/atmosphere.md
"The tiers"), the cones and the tail / brake halos are its lights (docs/atmosphere.md "The one volume: headlights in
rain"): this pass then draws only the lens streaks (`run(…, air = false)`); its march is the air with Volumetric Fog
Off only. The road and body light are unchanged.

## Oncoming headlights: no white disc
A player saw oncoming cars' fronts swallowed by white blobs (Car Lights Next-Gen, HDR post, Modern Lighting RT).
Reproduced headless with the reported options (`TRAFFIC_VIEW=oncoming`, `TRAFFIC_DIST` 4 / 24 / 74 ≈ 10 / 30 / 80 m
from the camera, new `TRAFFIC_LANE`), dry (clock 97) and rain (167), 1080p and 4K; by elimination
(`NFSU2_CARLIGHTS=spot=0 / point=0 / streak=0`) at 30 m dry: the cone air made 2200 of the 3733 pixels ≥ 240 in the
van's box, the lens streak and core 360, the halos 90. Causes (measured):
* **The cones' aureole** (`fs_march`): with the eye inside another car's beam looking back at the lamps, every ray
  toward the car's front runs along the beam (its first ~10 m held whole by `1 / (1 + z² / 100)`), forward
  scattering ×2 by the phase: a flat white disc wider than the car. That forward-scattered light around a lamp the
  eye looks into is the lamp's glare, which the native glow sprite and the lens core already draw. Now the march
  fades it out between cos 0.6 and 0.9 of the angle between the light's way and the way to the eye
  (`Look::aureole`); across the beam (the shafts) and from behind (the player's own beams) nothing changes.
* **The lens glare's distance law** (`vs_streak`): `1 / (1 + d² / 30²)` kept 10 m's streak and core out to 30 m,
  covering the front between the lamps. Now full within 10 m, then `200 / (100 + d²)` (the light reaching the lens).
Tried and dropped: an inverse-square near field per lamp and a per-car saturation cap (both left more white).
Numbers (the maintainer's private capture, area ≥ 240, 4K dry, Original / On / before / after):
10 m 6188 / 6117 / 7594 / 7616, 30 m 1100 / 1126 / 4158 / 1586, 80 m 55 / 66 / 112 / 86 (target: within 2× of
Original). Rain, 30 m: 448 / 1594 / 7297 / 3149; the rain's remainder is mostly not this effect ("On" is 3.6× the
Original there: wet road, rain lighting) — open. The player's own lights (chase camera, explore / rain street) and the
race start (`race`, `qrstart`, beams 2 alone and the reported options) are unchanged to ±0.1 mean / p95.
