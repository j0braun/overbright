# Next-Gen rain: how it works

The technical part of [Next-Gen rain](../../playing/settings/rain-nextgen.md): how it works, what it costs, how it was checked and what is open. The player part (the rows and what they do) is on that page.

## Overview

Every effect at the "next-gen" level of the Next-Gen water. A fourth value on the
existing **Rain Detail** row (Graphics page, Weather group): Off / On / High / **Next-Gen**
(`enh.rain_detail 3`, `qol::RAIN_NEXT_GEN`, `qol::rain_next_gen()`). No new label id: the row keeps
0xC104E0A2 (`options::RAIN_DETAIL`); the tier name is a string of the row. Not in any preset (manual,
like Water's Realistic); perf.sh's EXTREME keeps `rain_detail 2`.

**Nothing here exists in SPEED2.EXE; nothing was traced or observed under Wine for this work.** It
follows only native values that were already traced (docs/notes/graphics/weather.md "Native inputs"): the rain
amount (weather +0x1F0), the cover zones (+0x1E4, route triggers of kinds 5/6, 0x61E710), the road
wetness rule (+0x509C, via `Weather::road_wet`), the world clock 0x7FB718, the light packs' lamps
and the cars' drawn poses. Every constant below is **inferred / by eye**. On, High and Original are
untouched (md5 below).

## What Next-Gen draws (`render/weather_fx.rs`, `weather_fx.wgsl` "this work" section)
In place of High's streaks, splashes and lamp halos (the spray pool, Wet Road Interaction and Lens
Drops rows keep their own meaning):
- **Rain volume** (`vs_rain_ng`): 26 000 × rain drops in three nested world-anchored layers (near
  0.35–7 m, mid 7–18 m, far 18–42 m; the far ones stand for bundles; boxes of the outer two ahead of the
  eye), cross-faded. Terminal velocity 6.5–9 m/s by drop size; a **gusting wind** (`wind_ng`: breeze
  1.1/0.5 m/s ± 1.4 m/s gusts turning slowly, its displacement integrated in closed form so drops never
  jump) carried more by small drops, plus a per-drop sway.
  - **Motion blur**: each streak is the drop's path *relative to the camera* over a 1/45 s exposure
    (≤ 1.6 m), so speed blurs the rain along the motion; the light spreads over the longer streak
    (√ energy term, ≥ 0.4).
  - **Width = the drop's own projected size** (2.2 mm), never under 1 px; a thinner drop is fainter
    instead of thinner, so it looks the same at 1080p and 4K and does not alias.
  - **Lighting** (`light_ng`): the 32 nearest lamps (light packs' (1 − d²/r²)² falloff) and the 8
    nearest cars' headlight cones and tail lights, with water's forward scattering (Henyey–Greenstein
    g 0.75 + a weak back lobe): bright when looking into a light or a beam, dim elsewhere; ambient = fog
    colour × 0.8 (what a drop refracts of the overcast sky).
  - **Occlusion**: none under a native cover zone (as the native drops); elsewhere a screen-space test
    (`covered_ng`): a surface seen in front of the points 2.5 / 6 / 12 m above the drop that lies over it
    (bridges, canopies, overhangs, trees in view). Depth-soft against the scene.
- **Splashes** (`spot_ng`, `vs_splash_ng`): 7000 × rain world-anchored spots in a 44 m square ahead
  of the eye, 0.32 s each. The surface is found by iterating the depth buffer from the ground under the
  eye (3 steps); a spot is kept only when it lands back on itself (seen, not hidden), faces up
  (|n.z| ≥ 0.7, 2-px plane fit, no depth edge) and is not covered. So it lands on roads, pavements,
  **car roofs and bonnets**, rails and kerbs, never on walls, and stays put while the camera moves
  (the weather package's screen-picked splashes slid). Sizes by surface: film 2.5–4 cm crown, puddle 5–8 cm with a jet,
  raised surfaces 2–3 cm; a flash, a faint crown sheet, 8 droplets on arcs.
- **Rain rings** (`vs_ring_ng`): on the puddles only (Wet Roads' field, `puddle_ng` = world.wgsl
  `wet_surface`'s field with the Wet Road Interaction work's √level, the CPU twin is `wet_road::puddle_at`), within 14 m, with
  Wet Roads on: a crest catching the light and a darker trough. Wet Roads' own normal ripples are
  unchanged. Puddle formation and drying stay the wet-roads work's / the Wet Road Interaction work's (owns that mask).
- **Rain haze** (`haze_scatter`, `fs_haze_half`, `fs_haze_up`): the lamps' and headlights' light
  in-scattered by the rain along each view ray, integrated in closed form over the chord inside each
  light's reach (the falloff is a polynomial along the ray), in two halves each with its HG phase: halos
  around lamps, glowing beams. Worked out at 1/4 resolution (a glow) and filtered up; the distance mist
  (extinction 0.0022/m × rain, 400 m on the sky) per pixel. Replaces High's halo sprites. Under a cover
  the haze stays (the air beyond is wet).
  **The final effects pass (look-direction E1 / E4, `weather_fx::HAZE_LOOK`, tuning aid `NFSU2_RAIN_HAZE=scatter=…,
  extinction=…,veil=…,ambient=…,sky=…,reach=…`):** this work's mist put the native fog colour back over the
  scene at 0.9 of its extinction (a second fog: the darks lifted to grey) and the lamps scattered over
  their whole reach (wide white halos over the native pink glow sprites): Original + Rain Next-Gen read
  +7.6 mean luma at the rain street and +13.4 at the rain harbour (R1 limit ±4). Now the mist veils at
  0.2 of its extinction with 0.1 of the fog colour as its own light and not over the sky (the native fog
  veils the dome), and the lamps scatter over half their reach (the same coefficient: tighter halos with
  the dark gaps kept, the sprites' pink kept). +0.45 / +1.54 mean, p5 +0.00 / +0.35, p95 +0.25 / +1.94,
  sky −0.95 / +0.76 (docs/notes/performance/budget.md "Final performance / look pass on the effects"). The wet-driving pass's spray and splashes are unchanged.
- **Rain on the car** (`car_wet.wgsl` `rain_on_car`, with Wet Car on; `Draw.wet[2].z` = 1): capillary
  impact rings in 5 cm cells on upward surfaces (roof, bonnet, boot, windshield) at a rate by the rain,
  filtered away when sub-pixel; on glass, sheets of water flowing with the air and gravity that bend the
  reflection. Below Next-Gen `wet[2].z` = 0 and the branch is not entered.
- **Lens drops** (with Lens Drops on; `LensSim::next_gen`): drops of many sizes; above the grip size
  (0.2) they run down at 3·(s − 0.2)² screens/s, leaving a trail of tiny drops, and swallow drops they
  touch (area kept); speed still blows them off. Drawn by the native lens pass (0x5C7AC0's shader) as
  before; the native 20 drops are unchanged.
- Debug: `NFSU2_RAIN_NG_SKIP=haze_up,haze_half,vs_rain_ng,vs_splash_ng,vs_ring_ng` leaves draws out.

## Native byte-identity
- Fast build, 1280 × 960, `--shot explore` at clock 165 (rain 0.75): Original, Rain Detail On and
  High (with Wet Car, Spray, Lens Drops) are md5-identical to the copy's base binary.
- Gate `scripts/checks/gate.sh … <base> extreme`: see the unit report / internal evidence.

## Cost (`--bench drive 600`, clock 162, rain 0.70, 4 cars; reference desktop GPU, shared machine, 1 run each)
GPU busy median ms/frame; configs carry Wet Car + Spray + Lens Drops (internal evidence).

| | 1080p frame | 1080p weather pass | 4K frame | 4K weather pass |
|---|---|---|---|---|
| Original | – | – | 2.55 | – |
| Rain Detail High | 0.81 | 0.09 | 2.86 | 0.39 |
| **Next-Gen** | 1.00 | 0.29 | 3.37–3.48 | 0.94 |

Next-Gen over Original at 4K: about +0.85 ms (budget +1.5). Of the 0.94 ms pass, the haze is ~0.65
(mostly its full-screen blend at 4K: a half-resolution in-scatter cost the same as quarter, the
per-pixel loop moved out did not change it), streaks ~0.1, splashes ~0.05. History: full-resolution
haze 1.30 ms, half-res bilateral 1.19, quarter-res filtered 0.94. At the play setup (SSAA) the
pass scales with the supersampled pixels (1080p: 0.68 ms before the haze optimisation). Row cost in
graphics_table.rs: 0.9 (4K).

### The haze left out under Volumetric Fog
With any Volumetric Fog tier the froxel medium holds the rain's air (docs/notes/graphics/atmosphere.md "One medium"): the haze's
in-scatter and its extinction are 0 (`rain_share` 0), so `fs_haze_half` writes zeros and `fs_haze_up` discards
every pixel (alpha 1 − e^0 = 0, light 0). `weather_fx::haze_draws` says so from the frame's air values, and the
draw leaves both passes out (`counts().haze` 0): the same frames, byte for byte
(`tests::fog_rain_paths::rain_haze_left_out_under_the_fog_draws_the_plain_frames`). 4K, SSAA 2x, the play setup's rows,
rain at clock 162 (7 interleaved runs each): the quarter-resolution pass 0.074 [0.073..0.074] ms and the full-screen
one 0.090 [0.089..0.090] → none. Without Volumetric Fog the haze runs as before.

## Tests
`render::weather_fx::tests`: `next_gen_counts_follow_the_rain` (On/High unchanged), `wind_drift_is_continuous`,
`next_gen_lens_drops_run_and_merge`, `frame_uniform_layout`; `qol::tests::weather_rows_round_trip_and_native_default`
(3 parses and writes, 9 clamps to 3).

## Evidence (the maintainer's private captures)
- `spot-*-orig-ultra-ng.webp`: Original | Ultra-like (rain High) | Next-Gen, `--shot explore` 1080p at clock 165:
  City Core, casino canopy (under a cover zone: no rain, the haze and halos beyond), garage approach,
  the fountain plaza ("highway" spot −897.9, 726.1), neon district. `highway-crop100-ultra-ng.webp` at 100 %.
- `drive-user-00100/00400-high-ng.webp`: the play setup + High | + Next-Gen, `--bench drive`
  (11 / 50 mph); `drive-user-00400-road-ng-crop100.webp`; `sequence-drive-ng-200-500.webp` (motion).
- `car-4k-ultra-ng-crop100.webp`: 4K rear of the car, High | Next-Gen.

## Self-assessment (honest)
- Better than High: depth (three layers), wind and gusts, speed blur, lights' forward scattering,
  halos and lit beams in rain haze, distance mist, splashes that stay put and reach car roofs, lens drops
  that run and merge. Resolution-independent streaks.
- Not at FH5 / NFS Unbound level yet: streaks are still lines (no refraction of the scene behind them,
  no bokeh of near drops); splashes are sprites, not simulated particles; car impacts are subtle at
  chase-camera distance; no drips from overhangs; the cover test is screen-space only (an off-screen
  roof does not stop the rain outside the native cover zones); the haze ignores the lamps' shadowing
  by geometry; rain rings on the Animated Water work / the water rework water bodies are not added; the volume_fx grid (64 m) was not used
  for the haze (a 64 m grid cannot carry distance mist; the analytic integral is cheaper) — open for
  the effects-tiers pass together with near-ground mist through volume_fx.
- TAA/SSAA: drawn in the late path (after TAA), so no ghosting; streak aliasing is handled by the width
  rule. Not motion-checked frame-by-frame for shimmer beyond the drive sequence.
