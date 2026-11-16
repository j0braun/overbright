# Next-gen atmosphere: Volumetric Fog "Next-Gen" and Sky "Next-Gen" (clone deviations, opt-in)

The
goal: one atmosphere layer at the level of FH5 / NFS Heat / Unbound at night, in NFSU2's palette and grade.

**Nothing here is in SPEED2.EXE and nothing new was traced.** The native atmosphere is world.efx's per-vertex
fog (FogValue / FogColor), the horizon fog grid (0x5C7480) and the painted SKYDOME (0x60F890, `sky.rs`); the
clone keeps all of them unchanged and the two tiers below are layered on top. Every constant is **inferred**
(tuned by eye against the native frames). Original stays the default and byte-identical (see "Parity").

| row (Graphics → Lighting) | values | options.txt | label |
|---|---|---|---|
| **Volumetric Fog** | Off / Low / High / **Next-Gen** | `enh.volumetric_fog 0..3` | 0xC104E056 (existing) |
| **Sky** (new) | Original / **Next-Gen** | `enh.sky 0/1` | **0xC104E0B6** (new; unused in `the main checkout` and every wf3xx copy on 2026-11-05) |

No preset changes: Ultra keeps Volumetric Fog High, Extreme keeps its rows; Next-Gen and the Sky row are
manual. `qol::volumetric_fog()` = 3 and `qol::sky()` = 1 switch them.

## Code

| file | what |
|---|---|
| `src/render/atmosphere.rs`, `atmosphere.wgsl` | the froxel medium (new) |
| `src/render/sky_ng.rs`, `sky_ng.wgsl` | the Next-Gen sky (ported from the earlier `night_sky` prototype) with the panorama |
| `src/render/volumetric.rs` | `Frame::next` (the Next-Gen inputs; with them postfx runs the atmosphere instead of the cones) |
| `src/render/postfx.rs` | the dispatch (same place in the chain as the temporal tier's fog: after SSAO / SSR, before the native passes) |
| `src/render/lamps.rs` | `WorldRenderer::lamp_shadow_layers` (read-only: the Lamp Shadows maps, views and holders) |
| `src/render/volume_fx.rs`, `smoke_volume.rs`, `smoke_cinematic.rs` | additive accessors `medium()` (the smoke's density volume and placement) |
| `src/render/mod.rs` | `WorldCore::sky_rows / sky_ng`, the sky's prepare (plan_views), panorama update (record_views), draws (draw_pass_faded); `set_weather` → `sky_ng::set_rain` |
| `src/bin/game.rs` | `volumetric_frame` fills `Frame::next` for tier 3; `atmosphere_drive` adds the clock and the smoke medium |
| `src/qol.rs`, `src/frontend/{options,graphics_table}.rs` | the tier, the row, label, help, costs |

## Volumetric Fog "Next-Gen" (`render::atmosphere`)

A froxel volume: 160 × 90 screen tiles × 64 exponential slices of view depth from 0.5 to 320 m
(Wronski 2014; Hillaire 2015). Three passes, ≈ 0.4 ms at 4K (constant in resolution but for the apply):

1. **Inject** (`cs_inject`, one thread a cell): the haze's extinction — a height fog `0.005 /m ·
   e^(−h / 28 m)` above the ground under the eye, modulated by two octaves of drifting 3-D value noise
   (a 32³ tiling texture, wind 1.7 m/s), × (1 + 1.3 · rain) and a height × (1 + 1.2 · rain) in rain — and the
   light it scatters toward the eye, with a Henyey–Greenstein phase (g 0.55: halos looking into a light):
   * **street lamps** (the light packs, `lamps::lamps_of`; the 56 nearest ahead within 260 m): the native
     weight (0x4893A0) squared as a window (nothing beyond the authored radius, as Dynamic Lamps), inside it an
     inverse-square falloff from a 3 m source, a downward lobe under the head plus a small halo all round;
     **light shafts**: each lamp holding a Lamp Shadows layer is shadowed per cell by its map (one bilinear
     compare) — trees, signs and bridges cut the lit air under it. Needs Lamp Shadows Low/High (the player's
     setting is High); without it the cones are unshadowed.
   * **neon and lit signs**: Modern Lighting's emitters (`modern_light_bold::loaded_emitters`, made for every
     world at load whatever the Lighting row): the 40 nearest within 140 m, their colour, the half-space in
     front of the sign plus a glow all round — coloured haze round the neon.
   * **spot lights**: with the Next-Gen car-lights work's Headlight Beams **Next-Gen** on, every drawn car's cone from the Next-Gen car-lights work's list
     (`car_lights::air_lights`, converted by `atmosphere::spots_from_car_lights`; the nearest takes the
     headlight's shadow layer) at `NX6_SPOT_SCALE` 0.3: The Next-Gen car-lights work's own pass draws the sharp cookie shafts, the medium
     adds only the soft light they spread into the haze (no double beam). With it off, the player's headlight
     (view 9's projector, High's beam) at full gain. **The Next-Gen car-lights work owns the cones' look**; the medium is this work's.
   * **the city's ambient**: the native FogColor × 0.8 (the far haze converges to the native fog's own colour:
     the palette is kept) plus a warm glow low over the lit streets (the lamps' mean colour × 0.18 over 14 m).
   * a soft knee (5) on the summed light (overlapping halos saturate instead of blowing out), and in rain the
     lights' gain ÷ (1 + 1.3 rain) (the denser air shows in the ambient haze and the extinction, not as a
     white-out under every lamp).
   * **temporal**: the cell's sample is jittered along its slice (a golden-ratio sequence) and blended 12 %
     with the last frame's value at the same world point (reprojected through the last view; a cut > 30 m
     drops the history).
2. **Integrate** (`cs_integrate`, a thread per column): front to back, energy-conserving
   (S · (1 − e^(−σΔs)) / σ). **Unified with volume_fx**: the volumetric smoke's density volume (Particles
   Cinematic, `volume_fx`'s world-aligned 0.4 m grid) adds to the extinction of the light scattered *behind*
   it, not to the transmittance applied to the scene (the smoke is already composited there) — the haze and
   lamp halos behind a burnout are hidden by it (the smoke integration's open item "fog cones show through the smoke").
3. **Apply** (`fs_apply`, full size): the volume at the pixel's view depth (trilinear), `scene × T + S`,
   ±0.5/255 dither; alpha (the bleach mask) kept. The sky (the dome, past 3 km, or the far plane) takes the
   haze of 200 m at half its light (the native fog already veils the dome).

It runs before the post chain, so exposure, grade, bleach and HDR output treat it like the scene.
Tuning aids: `NFSU2_ATMO_LOOK=haze=…,height=…,rain=…,rain_height=…,ambient=…,glow=…,glow_height=…,noise=…,g=…,blend=…,lamp=…,emitter=…,spot=…,knee=…`,
`NFSU2_ATMO_DIMS=x,y,z`, `NFSU2_ATMO_DEBUG=1` (the scattered light alone × 4), `NFSU2_ATMO_NO_SHAFTS=1`.

### The narrow interface for the Next-Gen rain work (rain) and the Next-Gen car-lights work (car lights)
* `atmosphere::medium(rain) -> (extinction at the ground 1/m, height m)`: the shared medium; the Next-Gen rain work's rain haze
  and the Next-Gen car-lights work's cones read it, nothing else.
* `NextGen::spots: Vec<Spot>` + `spot_scale` (projector cones: eye, axes, x/z and y/z scales, reach, colour,
  shadow layer): filled from the Next-Gen car-lights work's cones when its tier is on (`game.rs` `volumetric_frame`). The Next-Gen car-lights work's own pass still
  uses its own density (`car_lights::Look::density`); taking `atmosphere::medium` there is left to the Next-Gen car-lights work.
* `NextGen::smoke: Option<Smoke>`: any volume_fx medium (spray, mist) can be passed the same way.

### One medium and its energy bound
The audit found five volume paths with their own air (fog Next-Gen ×(1 + 1.3 rain), Low/High's own
×(1 + 0.3 rain), the car lights' air ×(1 + 1.5 rain), Rain Next-Gen's haze, Rain Detail's wet-air lamp
halos), fog Next-Gen lifting the blacks (Δp5 +0.63 average, neon +1.99) and lamp halos on top of the
native glow sprites (the maintainer's private notes, issue 5). The narrow interface above was only
half true: nothing but this pass read `medium()`.
* **(a) One medium** — `render::enh::volumes::medium` (`Medium { extinction, height, thickness }` from
  `atmosphere::medium`): the froxels take its extinction and height; the car lights' air its thickness
  (`air_density`, `Look::rain_density` gone); Rain Next-Gen's haze the rain's share of it
  (`rain_share`, same values as before) and **none while this pass runs** (it holds the rain's air: two
  hazes of one rain before); Rain Detail's wet-air halo billboards are not drawn (`counts().halos` 0).
  One energy rule: a lamp's or a sign's light scattered in the air is its dry amount in any rain (this
  pass's gains ÷ thickness as before; Low/High's cones now too, High's own +30 % in rain gone).
* **(b) The energy bound and the double halo** — no halo all round a lamp head (`0.35 e^(−0.6 d²)`) or a
  sign (`0.5 e^(−0.15 d²)`): the native glow sprite and overbright are the halo. **Veil floor**: the light
  every pixel of the frame shares is the native fog's (world.efx fades to FogColor already), so the froxel
  light is taken off by the frame's floor: `cs_integrate` puts each column's light over its depth into a
  256-bin histogram (¼ code a bin), `cs_floor` (one thread, after the integrate in the same compute pass)
  takes the `Look::floor` (0.1) quantile's lower edge into a 1 × 1 R32Float and clears the bins, and
  `fs_apply` adds max(L − floor, 0) (hue kept). Per-pixel histograms were tried and fail: the near
  geometry gets next to no light, so any pixel quantile is 0. Tuning aid `NFSU2_ATMO_LOOK=floor=…`.

| fog Next-Gen alone | Δmean before → after | Δp5 before → after (limit +0.3) | lamp box (street A / rain / rainharb) |
|---|---|---|---|
| airport | +1.37 → +0.63 | +0.64 → +0.42 | |
| harbour | +1.50 → +0.62 | +0.36 → +0.07 | |
| neon | +3.84 → +1.42 | +1.99 → +0.85 | |
| rain | +2.63 → +0.81 | +0.72 → 0.00 | +5.14 → +1.77 |
| rainharb | +2.93 → +0.63 | +1.00 → 0.00 | +4.07 → +1.11 |
| street | +1.39 → +0.52 | +0.72 → +0.21 | +2.15 → +0.33 |
| tunnel | +1.06 → +0.56 | +0.14 → +0.07 | |

Neon and airport stay over +0.3: their dark pixels sit under the arcade's signs and the airport lamps,
where the local light (not the shared veil) lifts them; a higher quantile removes the haze everywhere
(0.4: airport and street 0.00 mean). Saturation of the bright pixels unchanged (Δ ≤ 0.005).
* **(c) One volume**: see "The one volume: headlights in rain" below; Low and High are tiers of the same
  medium since "The tiers: one medium at every fog tier".

## The one volume: headlights in rain
With Volumetric Fog Next-Gen the froxel medium is the only air: every light feeds it once.

**What stacked (measured, rain, the oncoming car 30 m off, box around it, the player's options).** Over
Original the box was +30.4 luma (pixels ≥ 240 in it: 448 → 2947 at 4K, the "≈ 7×"). By elimination: the
medium's copy of the Next-Gen car lights' cones −17.6 (it took every cone again at 0.3, without the march's
near cut-off and aureole and with its gain not bound to the rain rule: the light right in front of each car
and the forward-scattered glare of a beam the eye stands in); the car lights' own march −3.0, their halos −1.0,
lens streaks −1.4; the Next-Gen sky drawn into the road reflections −11 (the overcast lit by the city, mirrored
over the whole wet road; in a tunnel also where the reflection does not draw the ceiling: box +26); the rest
wet roads / enhanced reflections (≈ −3 each, the lamps' own reflections). Beams Next-Gen alone +6.1, Fog
Next-Gen alone +3.0, both +24.4: the same beams lit the same air twice.

**The design.**
* Every Next-Gen car's headlight cone is a spot of the medium, whole (`NX6_SPOT_SCALE` 1), shaped as the car
  lights' march had it: the projector's axes and scales, the cookie's lit centre (`COOKIE_X/Y`), HeadlightClipTex
  (no light in the first ~5 m, none past ~48 m), 1/(1 + z²/100), the car lights' phase, and the aureole: an eye
  in a beam looking back at the lamps gets nothing from it (their glare is the native sprite's and the lens
  core's). Gain `spot` 4 (the dry look of the march it replaces, calibrated at the highway, street and the
  oncoming view).
* The cars' tail / brake lamps are the medium's car lamps (`car_lights::medium_lamps` → `atmosphere::car_lamps`,
  ≤ 16, kind 2: a 0.6 m source, the tail's lobe, the halos' summed bound `halo_veils` kept, tails at half); the
  headlights' own halos stay out.
* The car lights' march (cones and halos) does not run while the medium does (`PostFx::pre_passes`,
  `CarLightsPass::run(…, air = false)`): only its lens streaks. Rain Next-Gen's haze adds no light of the lamps
  while any Volumetric Fog tier runs, none of the cars' cones with Car Lights Next-Gen (the drops and streaks
  keep both: lit rain, not air).
* One rain rule (`Medium::scatter_scale`): the cells' extinction grows with the thickness (×2.3 in full rain),
  every light's scattered light with √thickness (×1.52): denser air shows every beam and halo more, never as a
  white-out. Before, the lamps and signs were rain-constant, the spots grew ×2.3 and the march ×2.3.
* Energy: single scattering with the medium's transmittance bounds a light's air light by its own light; what
  the air takes out of a beam is taken off its road light (Headlight Beams × `Medium::beam_transmittance`,
  e^(−σ · 12 m): 0.97 dry, 0.93 in full rain).
* The blacks stay black: besides the frame's floor, each of 16 × 9 screen tiles' least column light is a veil
  taken off its pixels (`tile_floor`, bilinear between tile centres; `Look::tile` 1): light a whole stretch of
  the view shares (the neon street's haze over the dark gaps between the tubes) goes, the beams, shafts and
  halos above it stay.
* Sky Next-Gen no longer draws into the road reflections (they keep their native clear black; the cars' cube
  faces still show it), see "Sky Next-Gen" below.

**Measured** (rain, clock 167, the player's options = every Next-Gen tier with Modern Lighting ray traced; "max"
the same within ±1; Δ against Original at the same frame, luma 0–255; the box ahead of each headlight's air:
around the oncoming car, ahead of the player's car):

| scene (4K) | Δmean before → after | Δp5 | Δp95 | box Δ | box pixels ≥ 240 ÷ Original's |
|---|---|---|---|---|---|
| oncoming car 30 m | +3.93 → −3.80 | +2.07 → −0.18 | +5.56 → −11.43 | +30.0 → +2.1 | 6.6 → 2.3 |
| oncoming car 80 m | +5.57 → +0.59 | +0.50 → −0.07 | +27.1 → +2.45 | +57.2 → +1.2 | (86 → 47 px; Original 2) |
| following traffic | +3.09 → −2.14 | +0.06 → −0.44 | +9.32 → −3.06 | +11.6 → −4.6 | 0.7 → 0.4 |
| own beams, highway | +4.58 → −1.56 | +0.93 → +0.72 | +17.2 → −11.8 | +10.7 → −6.4 | 3.2 → 1.3 |
| tunnel exit | +3.27 → −0.02 | −0.93 → −1.07 | +4.64 → −5.00 | +27.7 → −5.3 | 112 → 1.7 |
| neon arcade | −3.00 → −4.90 | +3.49 → +1.55 | +3.71 → +1.44 | +2.0 → −3.9 | 3.5 → 2.1 |

What is left above Original there is not the air: with Volumetric Fog Off the same frames have Δp5 neon +2.2,
highway +1.0 (the rest of the stack, Rain Next-Gen's haze back on); the oncoming car's pixels ≥ 240 are mostly
the wet road's reflection of its lamps (Wet Roads off: 2.3 → 1.7×). Volumetric Fog Next-Gen alone over Original at the audit's
spots (1280 × 960, dry clock 97 unless named): Δp5 airport +0.21 → 0.00, harbour +0.14 → 0.00, neon arcade
+0.71 → +0.13 (dry +1.01 → +0.09), street 0.00, tunnel 0.00, rain harbour 0.00, own beams +0.21 → +0.14; Δmean
≤ +0.39 (was ≤ +1.22). The original game itself (Wine, DXVK, the same camera at the traffic light (1169.9, −304),
the shower held at 0.7): no light in the air at all, the beam shows only as the road pool. Its brightness
matches the clone's Original once both capture conditions are right (see formats.md "Rain", *Reference
captures*): at 60 frames a second with every graphics option at its maximum the box ahead of the car reads
78.3 (the clone's Original 77.3) and the frame without HUD and car 34.1 (32.9). A first capture read 90.6
because that session ran with the performance-level-3 defaults (Bleach Bypass and Rain off) at the native
120 frames a second.

Target (proposed): in rain the box ahead of a headlight within +4 of Original's (the rain spots' brightness
budget) and its pixels ≥ 240 within 2.5× Original's (the dry budget's 2×, the wet road's own lamp reflections
added), the blacks Δp5 ≤ +0.3 for the volume itself. The original game draws no lit air; the one volume's beam
is the enhancement, held to what the original's road pool already shows.

Cost (GPU busy, median of frames 30+, `--bench drive 300` in rain, two runs each, the reference desktop GPU):
1080p the player's options 5.65 → 5.52 ms, max 6.89 → 6.76; 4K 18.39 → 18.04, 22.10 → 21.33 (the car lights'
march no longer runs; the medium's extra lights cost less than it). The presets ±0.03 (noise).

At this point Volumetric Fog Low / High (Ultra, Extreme) still ran their own cone pass (Δp5 +0.57..+0.85 at the
audit's airport / harbour / neon spots): folded in below.

## The tiers: one medium at every fog tier
Volumetric Fog Low and High (the tiers Ultra and Extreme set) were a separate analytic cone pass
(`volumetric.wgsl`, a half-size march of lamp cones with no extinction, its own air): a second participating
medium beside the froxels, lit again by whatever else put light in the air (Car Lights Next-Gen's march drew the
beams a second time under Low / High; Rain Next-Gen's haze added the rain's air on top), and lifting the blacks
(Δp5 up to +1.5 at the airport). Now **every tier is this froxel medium** (`atmosphere::tier`, `Tier`): the same
lights, the same energy rule (`Medium::scatter_scale`, `beam_transmittance`), the same veil floors; the cone pass
and its shader are gone, `volumetric.rs` keeps a frame's inputs and the nearest-lamps pick other effects share.

| tier | grid (x × y × slices) | range | street lamps / signs / car lamps / cones | shafts | history |
|---|---|---|---|---|---|
| Low | 64 × 36 × 32 | 160 m | 12 / 8 / 4 / 2 | no | no |
| High (Ultra, Extreme) | 96 × 54 × 48 | 240 m | 32 / 24 / 8 / 4 | yes | no |
| Next-Gen (manual) | 160 × 90 × 64 | 320 m | 56 / 40 / 16 / 8 | yes | yes |

Chosen by measurement (1080p, GPU timestamps of the pass, reference desktop GPU): High's 128 × 72 × 48 cost
0.13–0.16 ms against the cone pass's 0.14 ms in the night race; 96 × 54 × 48 is 0.11–0.13. Low 80 × 45 × 32 was
level with the old Low (0.09); 64 × 36 × 32 is 0.08.
* **No history on Low / High.** The temporal blend accumulates drawn frames, so a frame at a tick would differ
  between display rates (the rate gate compares Extreme's frames at 30 / 120 / 200 Hz with 60 Hz bit for bit,
  docs/checks.md). Low / High sample each slice at its centre, from the frame's own view only. The lights the air
  shows are smooth (a lamp's window, a 3 m source, the cone's cookie), so the coarser grid holds them without the
  history's smoothing; motion check below.
* **Everything else is shared**: with Car Lights Next-Gen their cones and tail / brake lamps are the medium's at
  every tier (their own march does not run while any tier does, `PostFx::pre_passes`); Rain Next-Gen's haze adds
  no air of its own under any tier (`rain_fx`), the beams' road light pays what the air takes
  (`beam_transmittance`) under any tier.
* **Two fixes for every tier.** (1) The signs' glow took no notice of which way a sign faces (the picker tested
  the light kind 3, Modern Lighting's emitters are kind 2): every sign glowed all round, over the dark facade it
  hangs on; it now lights the half-space in front. Its glow in the air keeps to half the sign's light reach
  (`Look::emitter_reach` 0.5: a rainy overpass's lit office block lifted the frame's p5 by +0.57 at 1.0, +0.14 at
  0.5). (2) `cs_floor`, the veil quantile, was one thread walking 256 bins (~0.07 ms of latency a frame at any
  tier); it is now one workgroup scanning them in parallel, the same floor.

**Look** (1920 × 1080, Original + the row against Original, the audit's spots and the rain-light scenes, 13
scenes; `<work dir>` metrics, the sheets in the report): Δp5 before → after, max over the scenes (mean): Low
+0.50 → **+0.21** (+0.18 → +0.05), High +1.50 → **+0.21** (+0.44 → +0.05), Next-Gen +0.56 → **+0.21** (+0.09 →
+0.04; the overpass's sign above); Δmean max Low +0.56 → +0.27, High +1.00 → +0.27, Next-Gen +0.40 → +0.30. In
rain at 4K the oncoming car's box pixels ≥ 240 against Original: the row alone 1.0–1.5× at 30 / 80 m (before up
to 1.6×); the presets Ultra 2.2 → 2.1× (30 m), Extreme 1.9 → 1.9×, Ultra + Beams Next-Gen + Rain Next-Gen 2.8 →
2.4×; at 80 m Original has 2 such pixels (1080p scale) and the presets 16–20 with or without the fog. Ultra /
Extreme over Original at the 13 scenes: Δp5 lower or equal everywhere (airport +2.21 → +1.07, harbour +1.72 →
+1.07, arcade +1.71 → +1.14, rain arcade +1.43 → +0.99), Δmean 0.4–0.9 lower: the High cones' extra light is
gone; what the presets keep above Original there is the rest of the stack (the same frames with the row Off).

**Motion** (`--bench drive` in rain, frames 200–223, the fog's own light = the frame less the same frame with the
row Off, box 750 × 400 ahead of the car): frame-to-frame change of its mean in the box Ultra 0.0146 → 0.0146 luma,
Low 0.012 → 0.020, Ultra + Beams Next-Gen 0.005 → 0.024; mean per-pixel change 0.43 → 0.51 (Ultra), 0.18 → 0.36
(Low): under one code. Its 99.9th percentile 5 → 12 luma: the edges of buildings against the sky (the medium
now hazes the sky at 200 m, the cones did not), which move with the camera; no change in flat areas.

**Cost** (GPU ms per frame, reference desktop GPU, `--bench` 300 frames, 2 runs each, before → after; the pass =
its GPU timestamp span, at 4K including the work the span takes in before it; "night" = free roam frozen at the
casino's lamps (-400, -524), "rain" = `drive` at clock 162, "race" = `racing`):

| | 1080p pass: night / rain / race | 4K frame over Original: night / rain / race | integrated GPU, 1080p, pass: night / rain |
|---|---|---|---|
| Low | 0.095 → 0.084 / 0.097 → 0.089 / 0.103 → 0.098 | 0.31 → 0.24 / 0.32 → 0.26 / 0.33 → 0.28 | 3.05 → 2.09 / 2.91 → 1.98 |
| High | 0.147 → 0.106 / 0.142 → 0.104 / 0.144 → 0.119 | 0.47 → 0.26 / 0.47 → 0.26 / 0.51 → 0.31 | 5.59 → 2.66 / 5.25 → 2.73 |
| Next-Gen | 0.349 → 0.272 / 0.284 → 0.221 / 0.343 → 0.280 | 0.47 → 0.39 / 0.42 → 0.36 / 0.48 → 0.43 | 8.07 → 6.68 / 5.26 → 5.23 |
| Ultra (frame) | 2.695 → 2.580 / 3.766 → 3.677 / 2.717 → 2.617 | 9.58 → 9.10 / 12.67 → 12.29 / 9.50 → 9.14 (frame) | |
| Extreme (frame) | 3.762 → 3.664 / 5.007 → 4.938 / 3.683 → 3.596 | 12.62 → 12.17 / 16.14 → 15.76 / 12.21 → 11.88 (frame) | |

Every tier is cheaper than it was, Low ≤ High < Next-Gen everywhere; at 4K Low and High are within 0.05 ms of each
other (the full-size apply, the same for both, is most of it). Integrated GPU: `MESA_VK_DEVICE_SELECT` to the
reference integrated GPU, `taskset -c 0-3` (docs/perf-integrated-gpu.md); the presets still leave the row Off
on that class.

## Sky "Next-Gen" (`render::sky_ng`)

The earlier night-sky prototype (hidden `NFSU2_NIGHT_SKY`) brought to the row, with the
changes its recommendation asked for:
* **The sky panorama**: the cloud layer (domain-warped fBm on a plane at 1500 m, 650 m in rain; cover
  0.5 + 0.45 rain; two light taps toward the moon for silver edges) is rendered by `cs_panorama` into a
  1024 × 256 Rgba16Float lat-long of the upper hemisphere (u azimuth, v = √(elevation / 90°): the rows
  bunch at the horizon), **a quarter of the rows a frame** (all of them the first frame). The effects-tiers pass: only that quarter is
  dispatched (before, every row was and three quarters of the lanes idled): 0.08 → 0.025 ms at 4K, 2.5 → 0.68 ms on
  the reference integrated GPU, the same texels. The views sample it;
  stars and the moon stay analytic at full resolution.
* **City glow**: the dome's lit horizon colour (its 90th-percentile row), warmed (× 1.3, 1.08, 0.82), × 0.9:
  added to the clear sky low over the horizon (two exponentials, 5° and 18°) and to the clouds' undersides
  (dense cores and low / far clouds most; × 1.6 in rain) — light pollution in the dome's own palette.
* **Reflections**: the cars' cube faces draw the sky where they drew nothing (natively their clear black; 0x60F890 gives them no SKYDOME; the road reflections, mirrored and planar, keep their native black: what they leave undrawn is not only sky — a tunnel's or bridge's underside is not in them — and on a wet road the mirrored overcast lit the whole road from below, see "The one volume"): a far-plane
  hemisphere with the view's own frame (group 0, its matrices and fog), before their pools and flares.
* Everything else as the prototype: the dome's palette statistics (made at every load, < 5 ms, so the row can be
  switched in game), the dome's native shading (texel × v0 × 2, fogged), drawn after the opaque world over the
  dome's pixels (depth-tested), the skymorph fade, the moon (atan2 angle, not acos) and hashed stars.
* **A night sky (the second graphics-audit round, audit fix 4, decision 2026-11-12)**: at dusk/dawn the Next-Gen sky had replaced
  the painted sunset clouds with one smooth pink blob (audit `dusk__o+sky1` Δsky +8.8, Next-Gen preset +9.4).
  Now its weight follows the game's own sky state (`sky_ng::night_weight`, from `crate::sky`'s `SkyFrame`):
  1 on the table's night pairs (index 2 / 6, `SKY_NIGHT_A`), 0 on sunrise / sunset (0 / 4), and through the
  20 s dome morph (0x5CF687, SKYMORPHALPHA = t / 20) the same linear fade. The shader spends the weight in two
  halves one after the other: first its clear sky veils the dome (blend SrcAlpha / OneMinusSrcAlpha, dst alpha
  kept), then its clouds, stars and moon come in — the painted clouds and these are never on screen together.
  At 0 nothing is drawn or dispatched (native dome, byte-identical sky: dusk Δsky 0.00, 0 % pixels); the
  reflection views fade from their native clear black. At night (1) the sky is as before but for the next point.
* **Cut-outs at speed (the second graphics-audit round)**: the racing camera looks at the layer 2–10° over the horizon, 9–40 km away,
  where the octave count fell to 2 and the cloud rim kept the near band (0.32): smooth blobs with sharp rims,
  lit flat by the city glow, squashed by the perspective into bright slabs. Stable from frame to frame (not
  the panorama's quarter-row update nor a reprojection: the eye moves ~0.4 m a frame against a layer 1.5 km
  up). Now ≥ 3 octaves, the rim widens with distance (+0.6 by 20 km), the city glow on far undersides × 0.55
  and the fade into the dome's mean at 10–30 km (was 18–42). Evidence `docs/evidence/gfx-b2/`.
* Tuning aids kept: `NFSU2_NIGHT_SKY_COVER`, `_RAIN`, `_TIME`, `_DEBUG`, and `NFSU2_SKY_GLOW`.

## Cost (GPU busy ms/frame, reference desktop GPU (RADV), 3840 × 2160, `--bench explore 600` frozen at two night spots,
perf.sh's Ultra preset + the rows, 2 interleaved runs; internal evidence, raw lines
in `costs-4k.txt`)

| config (mean of 2 runs) | spot E (street, lamps) | spot G (park, skyline) |
|---|---|---|
| Ultra, Volumetric Fog **Off** | 8.27 | 8.62 |
| Ultra (Volumetric Fog High) | 9.30 (+1.03) | 9.52 (+0.90) |
| Ultra + Fog **Next-Gen** | 8.84 (**+0.57**) | 9.20 (**+0.58**) |
| Ultra + Sky **Next-Gen** (fog High) | 9.86 (+0.56 over Ultra) | 10.19 (+0.67 over Ultra) |
| Ultra + both Next-Gen | 9.37 (**+1.10** over fog Off; +0.07 over Ultra) | 9.85 (**+1.23**; +0.33 over Ultra) |

Within the budget (≤ +1.5 ms at 4K). Next-Gen fog is cheaper than High here (High's cone march at half size
is 0.9–1.0 ms in these lamp-dense views). Passes (GPU timestamps): `graphics: atmosphere` 0.36–0.41 ms (inject
+ integrate + apply), `world: sky panorama` 0.08 ms; the rest of the Sky row is the hemisphere in the world
pass and the cube's faces drawn one by one (the multiview pass given up). The row hints: Next-Gen fog 0.58,
Sky 0.62. Other jobs' processes shared the GPU during the runs (interleaved to spread the noise; ±0.1 ms).
No performance shortcut changes the look (the froxel grid is the method itself, not a reduced version).

## The lights a workgroup's cells reach (the injection's cull)
`cs_inject` takes every light the tier holds (Low 24, High 64, Next-Gen 112) in every cell, though a light reaches
only the cells inside its outer radius. A workgroup's 8 × 8 cells share one slice: they lie in the quad its four
corner rays cut from the plane of that slice's view depth (the cells' positions are an affine map of their screen
position on that plane). Four threads put the corners into workgroup memory, every thread then tests a light at
a time against the corners' box (with a margin over the cells' own `d² ≥ r²` test and their rounding) and sets
its bit; the cells walk the set bits in order, so each cell takes the same lights in the same order as the plain
loop, and lights left out are lights each of the cells would have skipped: the same cells, byte for byte
(`tests::fog_rain_paths::fog_light_cull_draws_the_plain_frames`; `set_plain_inject` puts every light in every
workgroup for it). This is Hillaire's (2015) volume tiles reusing a culled light list, done inside the pass. A
ray clamped at the 0.05 cosine (outside any field of view the game takes) puts every light in.

| tier, GPU (7 interleaved runs each) | `cs_inject` before → after (ms) |
|---|---|
| Next-Gen, the reference desktop GPU, 4K, the player's rows: rain / racing | 0.114 → 0.061 / 0.172 → 0.055 |
| High, the reference integrated GPU, 1080p, World Detail 3, level High: rain / racing | 0.668 → 0.548 / 0.772 → 0.455 |

## The rain's tint folded into the apply
In rain the native tint (0x5C7870, `PostFx::rain_tint`) multiplies the whole frame by its colour right after the
medium is added, a full-screen pass of its own at the scene's size. Where nothing touches the scene between the two
(no lens drops: the native Rain Splatter off, as World Detail 3 has it; no Next-Gen car lights' lens streaks; the
image still the scene, i.e. no streak copy before the tint) and in High Precision Post's half-float scene with every
tint factor at most 1, `PostFx::fold_tint` hands the tint to the apply: `fs_apply_tint` multiplies the light it adds
by it, and its blend multiplies the scene by the same factors (the blend constant), (scene + light) × tint in one
pass, and the tint pass is left out. The pixels are the two passes' up to the half float's rounding of one product
instead of two (Medium in rain, 1080p, 19 frames at rest and in motion: at most 2 levels, 0 % of the pixels off by
more than 1 %, 1 − SSIM 6.6e−5, the change's own frame-to-frame motion 0.034 levels;
`tests::fog_rain_paths::rain_tint_folded_into_the_fog_keeps_the_frames`, `PostFx::set_plain_tint` keeps the pass for
it). The reference integrated GPU at level Medium in rain: the tint 0.412 ms → none, the apply 1.158 → 1.123. With
the lens drops (World Detail 4–5, the player's settings) the drops sit between the two and the tint runs as before.

## Evidence (the maintainer's private captures, 1280 × 720 at 66 %, the game's chase camera, frame 60 of a frozen
`--bench explore`; Original | Ultra | Next-Gen = Ultra + Volumetric Fog Next-Gen + Sky Next-Gen)

* Six night spots: `atmo_core_*` (City Core plaza), `atmo_neon_*` (the shops' neon street), `atmo_skyline_*`
  (the harbour skyline from the park), `atmo_hills_*` (the hills tunnel), `atmo_highway_*` (under the elevated
  highway), `atmo_rain_*` (rain, clock 170 s).
* Crops: `atmo_skyline_crop100_ultra_nextgen.webp`, `atmo_highway_crop200_ultra_nextgen.webp`,
  `sky_clouds_crop200.webp`, `sky_moon_crop200.webp`.
* Motion: `atmo_motion_skyline_nextgen.webp` (30 frames of a moving camera, every 2nd frame), and a static
  check: consecutive frames differ by ≤ 0.36 % RMSE with Next-Gen (Ultra ≤ 0.09 %; the searchlights animate
  in both) — the residue is ≤ 2/255 of temporal noise in the lit haze near lamps, not visible at 100 %.
* Reflections: `sky_reflections_rain_ultra_nextgen_crop100.webp` (City Core in rain: the wet road's reflection
  and the car's glass / paint (its cube) show the sky) and `sky_reflections_rain_diff.webp` (red: the pixels
  that differ between Ultra and Ultra + Sky Next-Gen: the sky, the road mirror, the car's cube reflections).
* Sky: `sky_moon_orig_ultra_nextgen.webp`, `sky_skyline_ultra_nextgen.webp`, `sky_rain_ultra_nextgen.webp`,
  `sky_clouds_timelapse.webp` (8 stills 30 s of drift apart).
* Capture: `scripts/evidence.sh`, `scripts/bshot.sh` (bench placement `NFSU2_BENCH_AT` + `NFSU2_BENCH_FREEZE`),
  `scripts/shot.sh` (free-camera sky shots), `scripts/spots.py`.

## Self-assessment (honest, against FH5 / NFS Heat / Unbound at night)

Better than High: the night air now has depth — distance haze in the native fog colour that thickens low and
in rain, soft halos and lit air under every street lamp, coloured glows round neon and signs, the skyline
receding into haze, a moonlit sky with drifting clouds lit from below by the city, the sky in the road and
car reflections, and the haze hidden behind volumetric smoke. It keeps NFSU2's palette and grade (the far
haze converges to the native fog colour). Against the references it is **not at their level**:
(1) the effect is deliberately subtle in dry weather — the native fog already covers the distance and the
user's grade is dark; Heat / Unbound push much denser, bloomier fog; (2) light shafts are physically shadowed
only by the Lamp Shadows layers (≤ 12 lamps within 70 m, 140° down; cars as no occluder) and the shafts read
only where geometry hangs under a lamp — open streets show cones and halos, not crisp god rays; neon and signs
glow but cast no shafts (no shadow maps for them); (3) froxel resolution (≈ 24 px tiles at 4K) gives soft
halos with slight light leaking at silhouettes; (4) the clouds are one 2.5-D layer, not raymarched volumes, and
in the player's chase-camera view the sky is a small part of the frame; (5) with the Sky row on, the cars' cube
gives up the multiview pass (six faces in one) for the per-face path (CPU cost, see "Cost").

## Parity

Original: tier 0 and the Sky row 0 run nothing new (`Frame::next` is None below tier 3; `sky_ng::enabled()`
false: the sky is never made, `sky_rows` is computed at load only). The palette rows cost < 5 ms at a load.
On <base> (2026-11-05): base and new binaries PASS. Ported onto main, `gate.sh … <base> extreme`: base and new binaries (differ); checks PASS (1172 tests, selftest, 28 shots, first race / drag, post race, playthrough), 29 shots 0 differ, window path, enhanced Modern / Ultra / Extreme, audio and five simulation hashes identical: **PARITY: all gates passed**.

## Tests

`render::atmosphere::tests`: `module_validates` (both depth variants, entry points, uniform size),
`slices_round_trip_and_span_the_range`, `rain_thickens_and_raises_the_haze`, `jitter_covers_the_slice`,
`look_overrides`, `lights_pick_nearest_ahead_with_layers`; `render::sky_ng::tests`: `modules_validate` (main /
reflection modules), `off_by_default`, `stats_order_and_alpha`, `cap_rings_weighted`, `bayview_palette` (game
data; skips without NFSU2_ROOT); `render::atmosphere::tests::nx6_cones_become_spots`; `qol::tests::enhancements_round_trip` (the tier and the row, clamped).
The one volume: `render::atmosphere::tests::car_lamps_join_the_medium`, `render::enh::volumes::medium::tests::
rain_scatters_more_but_bounded` (√thickness, the beam's transmittance), `render::car_lights::tests::
medium_takes_tails_not_heads` (tails / brakes only, bounded by the lamp's own light).

## Credits (algorithms; no third-party code vendored, so no THIRD-PARTY-NOTICES entry)

* B. Wronski, "Volumetric Fog: Unified Compute Shader-Based Solution to Atmospheric Scattering", SIGGRAPH 2014.
* S. Hillaire, "Towards Unified and Physically-Based Volumetric Lighting in Frostbite", SIGGRAPH 2015
  (energy-conserving integration).
* L. Henyey, J. Greenstein, 1941 (phase); J. Jimenez, SIGGRAPH 2014 (interleaved gradient noise).
* SplitMix64 (Steele, Lea & Flood, OOPSLA 2014) for the noise texture.
* The sky: as the prototype (Schneider 2015, Hillaire 2016, pcg2d by Jarzynski & Olano 2020).

## Open

* The cars' tail / brake lamps (the Next-Gen car-lights work's points) do not light the haze's froxels (the Next-Gen car-lights work's pass draws their halos).
* Neon shafts (shadowed), the cars as occluders of the lamps' shafts.
* A multiview sky pipeline (the cube's six faces in one pass with the sky).
* The rain streaks are not lit by the haze's light volume; a shared light volume (the integrated cells)
  could be sampled by them and by volume_fx's march.

## The look budget (docs/look-direction.md rules E1 / E4)
Measured against Original at the look direction spots, Volumetric Fog Next-Gen lifted the mean luma +14 (+63 at the
casino neon, the sky +22) and the blacks (p5) +7.7: `ambient` put the native FogColor back on top of
world.efx's own per-vertex fog (double fog), the haze veiled the scene a second time, and the sign / lamp
halos doubled the native glow sprites and overbright. `atmosphere::LOOK` now: haze 0.0025, ambient
**0.05**, glow 0.03, lamp **40**, emitter **12**, knee **1.5**, and the new **veil 0.2** (the share of the
haze's extinction applied to the scene behind it; the medium's own light still integrates the full
extinction; uniform `prev_forward.x`, applied in `fs_apply`). Result: mean +1.4, p5 +1.0, p95 +1.1 over
the six audit spots (the casino neon +5.1); the halos stay, the wash is gone. All tunable with `NFSU2_ATMO_LOOK` (`veil=` added).

## No veil (2026-11-06)
The look direction's `veil` 0.2 still darkened the distance in the rain while the medium's light comes from the lamps near the
eye only (player: "a transparent black texture disappearing when coming close"): veil 0, the native fog alone
attenuates the scene. Numbers and the drive-towards sequence: docs/wet-road-car.md "The glow shell, the washed road and the distance veil at max settings".
