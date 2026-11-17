# Look direction: polish over quantity (2026-11-05)

Problem: many effects, not enough quality and polish; the game often too bright and losing its
character without reaching a "next-gen" look. This page sets the look
target and the energy rules for every enhancement, records what made the enhanced presets too bright or
too flat, and gives a verdict per row. **Every later effect change follows the rules below**
(the look rule).

Nothing here is native code. The reference is the original, SPEED2.EXE, run under Wine (DXVK on the
reference desktop GPU). The clone's Original path was measured against it at the same spots (below) and is used as
the per-frame reference, because it is byte-identical run to run and matches the original's luminance.
Statuses: **measured** = from frames by the maintainer's tools; **inferred** = a tuning choice judged by eye.

## The target

NFSU2 at its best, rendered with modern quality, and not "generic modern":
* **dark, contrasty nights**: deep blacks between the lamps; the darkness is part of the art (the painted
  bake), not a lack of light;
* **sodium and neon colour**: orange pools and saturated pink, blue and green signs that keep their hue
  (no white-out);
* **heavy glow and streaks where the original has them**: the native glow sprites, the overbright bloom
  and the light streaks are the bloom; nothing else adds a second one;
* **the bleach-bypass grade and tint**: the native screen-filter chain is the only grade;
* the native **distance fog** (world.efx FogValue / FogColor and the horizon fog) is the atmosphere;
  enhancements may add local light in the air, never a second fog.

Reference frames: the original at 16 spots of Bayview (dusk, night dry, the rain spots' clock, city core,
casino neon, Coal Harbor, Beacon Hill, airport, highway), plus the main menu and the garage
(internal evidence: clock, car pose, the original's camera eye / look-at /
angle; the grids in `sbs/` hold the frames).

## How it is measured (the maintainer's tools)

* `wine_ref.py OUT --attach --keep`: in a running fastgpu session (the player's career save
  `the finished-career save`, free roam), puts the player car at each spot of `spots.py` (the maintainer's tool `place.py`),
  waits, reads back the car pose and **view 1's camera** (`[[0x8900B4]+0x238]+0x1C`: +0x40 eye, +0x60
  look-at, +0xC4 binary angle) and the world clock (0x865188 / 4000), and takes the frame (1280×960).
  Dusk spots are taken in the first minute after the load (the sunrise/dusk sky, clock < 60); night spots
  at clock 85–135 (the clock is written back to 90 s when it passes 135, so no sky step and no shower is
  crossed). The automatic navigation of the maintainer's tool `nav.py` pressed Enter into a Quick Race once; boot
  by hand (`peek.py key`) and use `--attach`.
* `cshot.sh OUT SPOT PRESET` renders the clone at the same spot with `POSES=` the original's camera
  (`NFSU2_SHOT_AT` the car pose, `NFSU2_SHOT_LOOK` eye, look-at and game FOV, `NFSU2_RAIN_CLOCK` the
  clock − 3 s) and the preset (`orig`, `user` = the player's 2026-11-04 settings, `ultra` = Preset::Ultra,
  `max` = perf.sh MAX + Volumetric Fog Next-Gen + Sky Next-Gen; `o+KEY=V` Original plus one row; `m-KEY=V`
  max with one row changed).
* `lstats.py` / `audit.py`: Rec.709 luma of the sRGB codes with the HUD masked: mean, median, p5, p95,
  % crushed (Y ≤ 8), RMS contrast, glow energy (mean of max(Y − 200, 0)), mean saturation, sky (top 18 %).
  `audit.py DIR BASE "spots" "rows"` gives each row's mean change against BASE over the spots.
* `sbs.sh` makes the grids: Original (Wine) | user before | user after | max after.

**Renderer of the reference: DXVK.** A first run under wined3d (Vulkan) had no headlight projector on the
road and read 9–21 levels darker than both DXVK and the clone (`poses-wined3d.tsv`); wined3d frames are not
used for brightness.

## Findings (measured)

**The clone's Original path is not the problem.** Over 11 dry spots at the original's cameras:

| | mean | median | p5 | p95 | RMS | glow | crushed % |
|---|---|---|---|---|---|---|---|
| original (Wine, DXVK) | 69.6 | 45.8 | 11.0 | 208.2 | 62.5 | 1.89 | 1.67 |
| clone Original | 70.5 | 46.0 | 11.0 | 204.4 | 61.7 | 1.73 | 1.68 |
| play setup, before → after | 71.6 → **69.6** | 47.4 → 45.8 | 11.8 → 11.5 | 205.2 → 203.1 | 61.5 → 60.8 | 1.74 → 1.65 | 1.34 → 1.47 |
| Ultra, before → after | 78.1 → **71.6** | 55.0 → 48.4 | 14.1 → 12.3 | 210.5 → 202.7 | 62.8 → 60.5 | 1.96 → 1.62 | 0.77 → 1.18 |
| max, before → after | 69.7 → 69.0 | 53.2 → 48.1 | **19.4 → 12.3** | **182.1 → 199.7** | **51.9 → 58.8** | 1.44 → 1.52 | 0.35 → 0.98 |

(internal evidence: the Wine table; the coal docks and Jackson Heights frames of the original lost the player car in the
teleport and are left out of this table.) Ultra was **brighter** (+8.5 mean, the median +9, the blacks
lifted, crushed pixels halved); max was **flat** (blacks +8.4, highlights −26, contrast −10): a milky,
lifted image with dimmed highlights. Over all 19 spots against clone Original (`audit-final.txt`): Ultra
+9.1 → **+1.4** mean, max p5 +6.2 → **+0.9**, p95 −21.2 → **−6.7**, RMS −8.5 → −3.3.

**Energy audit, ranked** (Original + one row, six spots: street, casino neon, Coal Harbor docks, Jackson
Heights, rain street, Quick Race start; `audit-rows-before.txt` / `audit-rows-after.txt`):

| # | row | before (mean / p5 / p95 / glow) | cause | after |
|---|---|---|---|---|
| 1 | Volumetric Fog **Next-Gen** | +14.2 / **+7.7** / +2.8 / +0.30; sky +21.9; casino neon +63 | the native FogColor put back on top of world.efx's fog (ambient 0.8: double fog), the haze veiling the scene again, sign and lamp halos on top of the native glow sprites and overbright (double glow) | +1.4 / +1.0 / +1.1 / +0.04 |
| 2 | **Modern Lighting** (Bold) | −12.6 / −0.7 / **−33.9** / −0.44; garage p95 −100 | the strength halved the bake everywhere (lit facades, signs, the garage's turntable) and boosted the pools ×1.6: dark *and* flat, highlights and glow gone | −3.3 / −0.3 / −3.2 / −0.03 |
| 3 | Volumetric Fog **High** (Ultra) | +8.0 / +1.3 / +11.0 / +0.39; rain +14.4 | purely additive cones (no extinction), 48 lamps, bloomed again by the native overbright | +1.3 / +0.3 / +1.7 / +0.06 |
| 4 | Headlight Beams, the other cars' | drift start +10.7 / p95 **+58** | every grid car's projector added at 0.8 onto roads the bake already lights | drift +1.5 |
| 5 | Volumetric Fog Low (play setup) | +2.8 / +0.4 / +2.6 | as 3 | +0.5 / +0.2 / +0.4 |
| 6 | Dynamic Lamps | +2.6 / +0.4 / +1.0; Coal Harbor +6.8 | the dark gaps' bake ×1.7 (the lamps are already in the bake) | +1.7 / +0.3 / +0.8 |
| 7 | small | Sky Next-Gen +0.5, Light Probes +0.3, Wet Roads +0.1, Surface Detail, Water, Wet Car, Enhanced Paint ≈ 0; darkeners SSAO −0.5, Lamp Shadows −0.4, Contact Shadows −0.4, Enhanced Reflections −0.4 (contrast, wanted); High Precision Post +0.1 (precision only) | — | unchanged |
| 8 | Depth of Field (manual) | p95 −1.8 (−9 at the race start), RMS −0.6 | blur flattens the highlights in a still | unchanged (stays out of the presets) |

So the brightness came from **air light and lamp light added on top of light the original already has**
(fog colour, glow, the bake's pools), and the flatness of max from the **Next-Gen haze** (lifted blacks)
plus **Modern Lighting's global darkening** (lost highlights). None of it came from the grade: the
enhanced path's grade is the native chain (High Precision Post measured +0.1).

## What changed (the pipeline: one energy anchor)

The grade stays one consistent chain for every path: the native screen filter (bleach bypass, tint,
overbright, glow, streaks), in float with High Precision Post, through the captured LUT with HDR Output.
No tonemap curve, no ACES, no auto-exposure. The fix is in the energy that feeds it: **every lighting row
is anchored to the native lighting data** (the bake, FogColor, the glow sprites) instead of adding to it.

* **E1, no double fog** — the next-gen atmosphere (`atmosphere::LOOK`, `atmosphere.wgsl fs_apply`): ambient
  0.8 → 0.05, ground glow 0.18 → 0.03, haze 0.005 → 0.0025 /m, new `veil` 0.2 (the share of the haze's
  extinction applied to the scene; the medium's own light still integrates the full extinction).
* **E2, the bake is the anchor for added light** — Dynamic Lamps `LAMP_MAX` 0.35 → 0.12 (a gap gains at
  most ×1.24 of its bake); the other cars' beams `BEAM_OTHERS` 0.8 → 0.3, gone from a bake of 0.25.
* **E3, darken only where the original is dark** — Modern Lighting (`ml_relight`): the strength's
  darkening, pool boost and emitter light fade out between baked luminance 0.06 and 0.22
  (`ML_KEEP_LO/HI`): the gaps get darker, the art's lit surfaces keep their level.
* **E4, the air-light budget** — Volumetric Fog Low/High `DENSITY` 0.2 → 0.04, `RAIN_DENSITY` 1.5 → 0.3;
  the next-gen atmosphere lamp halos 130 → 40, sign halos 110 → 12, knee 5 → 1.5.

Tests: `atmosphere::tests::look_keeps_the_native_fog_and_glow`, `modern_light_bold::tests::
darkening_is_anchored_to_the_bake`, `volumetric::tests::air_light_stays_in_the_look_budget`,
`lamps::tests` (gap bound ×1.24, `BEAM_OTHERS`). Original is byte-identical (all changes are in enhanced
shader blocks and constants; gate parity below).

## The rules for every effect (from now on)

A unit that adds or changes an enhancement reports, at this work's spots (the maintainer's tools, at least the six audit
spots plus its own), **Original + its row** and **the presets with it**, against Original:

| rule | budget (mean over the spots, 0–255 luma) |
|---|---|
| **R1 brightness** | Δmean within **±2.5** for one row (rain spots ±4); a preset's total within **+3 / −4** |
| **R2 blacks** | Δp5 ≤ **+1** (one row), ≤ +1.5 (preset); crushed % must not fall by more than half |
| **R3 highlights and glow** | Δp95 within **±4**, Δglow within **±0.2**; no row lowers the original's highlights more than that |
| **R4 contrast** | ΔRMS ≥ **−2** (one row), ≥ −4 (preset) |
| **R5 colour** | mean saturation within ±0.02; neon and signs keep their hue at the clip (no white-out; any highlight compression preserves saturation per hue — the Remix lesson) |
| **R6 sky** | Δsky within ±2 at night |

and follows the energy rules:
* **E1 no double fog**: the native fog owns attenuation and FogColor. An air effect adds the light of
  local sources only; it does not re-add FogColor (ambient ≤ 0.1 of it) or veil the scene again
  (veil ≤ 0.25). Fog adds to the scene, never replaces it.
* **E2 the bake is the anchor**: never rebuild the light from scratch. New per-pixel light on world
  surfaces shapes the bake (a dark gap gains ≤ ×1.25, nothing ends above max(bake, the pool peak)); a new
  light source the original does not have (other cars' beams, car glows) is held to the same gap budget.
* **E3 darken only where the original is dark**: a darkening term fades out where the bake is bright; the
  art's lit surfaces, signs and the garage keep their level.
* **E4 count the glow twice**: every light added before the native post chain is bloomed again by the
  overbright pass. Halos and air light must not duplicate the native glow sprites and streaks; budget
  them with the glow metric.
* **E5 one grade**: the native screen filter is the grade. No ACES by default, no auto-exposure at night;
  HDR output goes through the captured LUT.
* **E6 car lights at every tier**: the native light markers (0x615640) always drive the clone's car
  lights; no tier may lose head, tail or brake lights.
* **E7 motion**: every temporal or accumulating effect gets a motion check on the racing bench path
  (camera > 50 m/s): no ghosting, no lost sense of speed. Stills lie (judge in motion too).
* **E8 report**: the table above for the row, the grids (Original | before | after), cost, and a verdict
  (keep / tune / merge / remove from presets). A row that costs > 0.5 ms at 4K and moves no metric and no
  visible detail at the spots is a candidate to leave the presets.

## Per-row verdict

| row | verdict | note |
|---|---|---|
| High Precision Post | **keep** | precision only (+0.1) |
| Anti-aliasing rows, aniso, reversed-Z, detail distances, LOD fade, keep loaded | **keep** | no light |
| Enhanced Paint, Enhanced Reflections, Road / Car Reflection, SSR, Wet Roads, Wet Car | **keep** | within ±0.5 |
| SSAO, Car Shadow, Lamp Shadows, Contact Shadows | **keep** | −0.4..−0.5: contrast, in the original's direction |
| Dynamic Lamps | **tuned** | +2.6 → +1.7; still the largest lift left in Modern/Ultra; a later unit may make it energy-neutral (shape only) |
| Headlight Beams | **tuned** | the grid wash gone; the player's beam replaces the native projector (no double) |
| Light Probes | **keep** | +0.3, CPU only |
| Enhanced Glass | **back in Ultra** | The effects-tiers pass took it out (0.65 ms at 4K, the composite hid the NFSPEED rear-window decal); the final effects pass draws the decals after the pane and copies only the glass's rect: +0.11 / +0.35 ms at 4K, R1–R6 met (docs/perf-budget.md "Final performance / look pass on the effects") |
| Volumetric Fog Low / High | **tuned** | tiers of the one froxel medium (docs/atmosphere.md "The tiers"); High stays in Ultra |
| Volumetric Fog Next-Gen, Sky Next-Gen | **tuned**, manual | the wash and the lifted blacks gone; halos kept |
| Modern Lighting | **tuned**, manual | darker gaps, lit art kept; RT costs 9 ms at 4K for shadows in the pools |
| Surface Detail, Water, Rain Detail, Spray, Particles | **keep** | ≈ 0 light |
| Motion Blur, Depth of Field | **keep manual, out of the presets** | DOF flattens the highlights (p95 −9 at the race start); both need the E7 motion check |
| Ambient Detail | **manual (audit)** | R1–R6 met (Δmean −0.03, p95 −0.11) but ≤ 0.04 % of the frame changes at the chase camera (eleven spots): no preset gain; Next-Gen +7.8 ms on an iGPU |
| Next-Gen rows | **manual** | Rain Detail Next-Gen's haze retuned in the final effects pass to E1 / E4 (rain street +7.6 → +0.45, rain harbour +13.4 → +1.54: R1 met); Beams Next-Gen +1.33 / p95 +1.17 (met); fog / sky as above; no preset sets a Next-Gen tier on any GPU class |

No preset values changed: with the fixes Modern / Ultra / Extreme meet the preset budget (Ultra +1.4 mean,
p5 +1.2, p95 −1.3 over 19 spots); max (manual top tiers) is −2.1 / +0.9 / −6.7 — darker and somewhat
softer than the original at the race start (DOF) and the Coal Harbor docks (Modern Lighting's darker
gaps), which is the direction of the look; it no longer lifts the blacks.

## Evidence (the maintainer's private captures)
`sbs/<spot>.webp` (Original | user before | user after | max after, half size; all 21 spots in the work
dir), `ultra/<spot>_orig_ultra-before_ultra-after.webp`, `audit-rows-before.txt`, `audit-rows-after.txt`,
`audit-final.txt` (19 spots vs clone Original), internal evidence: the Wine table (11 spots vs the original),
`poses-dxvk.tsv` / `poses-wined3d.tsv` (the original's poses and cameras).

## Open
* **Rain** (closed): the original looked dry at the street spot (−247.6, 81.7) at clock 171 and 219
  only because those frames were taken at its own 120 frames a second, where the weather update is
  skipped (docs/formats.md, Rain). At 60 frames a second, with every option at its maximum read back
  from memory, it rains there (amount and target 0.7 at clocks 171.5 and 219.7, the shower from about
  150 to 220 at that place) and the frames match the clone's Original (mean / p5 / p50 / p95 without
  HUD and car: 45.7 / 8.5 / 28.1 / 169.0 against 46.0 / 9.1 / 28.3 / 169.5, and 46.1 / 8.5 / 28.7 /
  169.0 against 46.2 / 9.1 / 28.4 / 170.1; the dry controls at the traffic light 26.5 against 26.7 and
  26.9 against 26.6). The rain map agrees at that position.
* The coal docks and Jackson Heights frames of the original lost the player car after the teleport.
* Race start, drift and the front end were compared clone to clone (no Wine frame at the same camera);
  the main menu and the garage of the original are in the work dir only (other car).
* Motion checks of the tuned rows were not run here; the changes are constants of existing passes.
