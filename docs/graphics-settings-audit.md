# Graphics settings audit and the preset page (2026-11-04)

Goal: every graphics setting must make the image better or at least different; an effect that
only costs and adds nothing goes. SSAA is kept whatever the audit says. This file
holds the audit, what was removed or merged and why, and the new Graphics
page the presets are built on. All of it is a **clone deviation** (nothing
here exists in SPEED2.EXE); "Original" is the native image and stays the
default, byte-identical (no render code changed in this work).

## Method
* Build: <work dir> (the base + High Precision Post + Enhanced Car
  Paint). reference desktop GPU, RADV (Mesa 26.2.5), Vulkan. Machine shared with other
  agents (load average 10–20 during the runs).
* Scenes (`--shot`, deterministic run to run, 0 px difference between two
  base runs except the menu): **night** free roam (`explore`), **rain**
  (`NFSU2_RAIN_CLOCK=170 explore`, wet road + drops), **start** (Quick Race
  grid, `qrstart`), **garage** (`qrcars`), **menu** (`main`, the turntable;
  not deterministic: two base runs differ by 0.6–1.7 % of pixels, max 8–16
  levels, so menu values at that level are noise and marked `~`),
  **motion** (Quick Race circuit after 20 s at full throttle, 43 mph,
  `NFSU2_QR_MODE=circuit NFSU2_FRAMES=600 NFSU2_THROTTLE=1 qrstart`).
  Each at 1920×1080 and 3840×2160; the draw-distance rows also at 640×480.
* Each row value against the Original image of the same scene:
  changed pixels (any channel > 2 levels of 255), max delta, mean delta,
  SSIM (luma, 7×7). Crops: the 1/6-height block with the largest change,
  as Off | On | difference ×8.
* Shots render the native scene without MSAA (the shot harness; the game
  at World Detail 5 plays with 8x MSAA, Customize Display's Full Screen
  Anti-Aliasing 3 → `display::msaa_samples`), so the AA rows were run a
  second time against an 8x MSAA base (`NFSU2_MSAA=8`), which is what a
  player at Detail 5 sees.
* Cost: `--bench explore 300`, GPU busy ms (timestamps, median, min of 2–3
  runs), `NFSU2_BENCH_ENH`, at 1080p and 4K; the bench plays with native
  8x MSAA, so costs are against what the player has. Draw-distance rows:
  `--bench drive` (600 frames, 136 m, `NFSU2_POP_STATS=1`).
  The **4K run was clean** (base 2.08 ms at start and end). At 1080p the
  whole frame is ~0.6 ms of GPU and the card's clock state moves the
  numbers by ±0.3 ms under the shared load: the 1080p column from Road
  Reflection on is not reliable (it was repeated, still drifting); use the
  4K numbers (per-pixel effects cost about a quarter at 1080p).
* iGPU estimate (**inferred**, not measured: no iGPU here): a recent 12-CU integrated GPU
  / Iris Xe class part has ~1/7–1/9 of this card's memory bandwidth and
  ~1/5–1/10 of its shader rate; full-screen passes are bandwidth-bound, so
  multiply the 1080p cost by ~10 (≈ 2.5× the 4K number). A cost class
  "Medium" here (0.3–1 ms) is 3–10 ms there.
* Data: internal evidence (the `*.tsv` tables), scripts in
  `.../scripts/` (`run.sh`, `run_msaa8.sh`, `bench.sh`, `pops.sh`,
  `imgdiff.py`, `table.py`); raw shots in `<work dir>/settings-work`.

## Results (changed % / max delta; "4K worst" adds SSIM)

| Row / value | night | rain | start | garage | menu | motion | 4K worst | GPU ms 1080p | GPU ms 4K | Verdict |
|---|---|---|---|---|---|---|---|---|---|---|
| Render Scale 50 % | 21.1/243 | 17.1/207 | 29.0/255 | 15.4/237 | 15.4/237 | 32.4/255 | 20.6/255, 0.970 | −0.18 | −1.32 | visible (worse), saves time → Performance |
| Render Scale 75 % | 15.2/251 | 11.3/208 | 19.5/242 | 10.1/238 | 10.1/238 | 20.2/249 | 12.9/239, 0.982 | −0.11 | −0.74 | as above |
| Render Scale 150 % | 10.2/217 | 6.6/158 | 14.8/209 | 7.0/223 | 7.6/223 | 14.6/227 | 9.9/240, 0.992 | +0.63 | +0.01* | **merged into SSAA** |
| Render Scale 200 % | 9.1/219 | 5.2/156 | 15.4/222 | 6.9/220 | 7.1/220 | 15.1/221 | 9.9/240, 0.992 | +1.50 | +0.00* | **merged into SSAA** (= SSAA 4x image) |
| Texture Filtering 2x | 3.9/233 | 3.3/160 | 2.1/247 | 0.7/84 | 0.6/84 | 2.9/243 | 2.5/231, 0.998 | 0 | 0 | **worse than native** |
| Texture Filtering 4x | 1.9/234 | 1.4/162 | 0.9/241 | 0.2/57 | ~ | 1.1/231 | 0.8/147, 1.000 | 0 | 0 | **worse than native** |
| Texture Filtering 16x | 0 | 0 | 0 | 0 | ~ | 0 | 0 | 0 | 0 | **no effect: removed** |
| AA FXAA | 4.0/141 | 3.2/158 | 4.8/177 | 1.7/167 | 2.0/167 | 4.5/182 | 2.6/183, 0.996 | +0.04 | +0.11 | visible, cheap |
| AA SMAA 1x | 0.8/117 | 0.7/103 | 2.0/120 | 0.7/112 | 0.9/112 | 1.8/119 | 1.0/120, 0.998 | +0.14 | +0.36 | visible, cheap |
| AA MSAA 2x / 4x | 1.3–3.4 | 1.3–1.7 | 2.7–3.4 | 2.5–2.7 | 2.6 | 1.9–2.3 | 1.5–1.9 | ≈0 | −0.97 / −0.74 | vs native 8x: fewer samples + alpha-to-coverage; cheaper |
| AA MSAA 8x | 1.8/186 | 1.9/167 | 3.7/230 | 2.8/152 | 2.6/152 | 2.4/165 | 2.0/192, 0.995 | +0.06 | +0.01 | vs native 8x: **0.3–1.2 %** (alpha-to-coverage on foliage, fences), free |
| AA SSAA 2x | 10.6/188 | 7.1/173 | 14.8/228 | 7.2/202 | 8.0/202 | 14.5/215 | 9.9/240, 0.992 | +0.05 | −0.17 | visible; drops native MSAA, so ≈ free |
| AA SSAA 4x | 9.1/219 | 5.2/156 | 15.4/222 | 6.9/220 | 6.9/220 | 15.1/221 | 10.9/207, 0.991 | +0.35 | +1.57 | visible, costly at 4K (kept) |
| AA SSAA 8x / 9x / 16x | 10.0–10.7 | 5.9–6.5 | 17.2–17.9 | 7.1–7.3 | 7.0–8.2 | 16.7–17.4 | 12.2–12.7 | +1.35 / +1.51 / +3.61 | +5.87 / +6.60 / +14.65 | visible over 4x (4x vs 16x: 6–10 % px, max ~110), very costly (kept) |
| AA: MSAA Override 4x / 16x | = MSAA 4x / 8x | | | | | | | | | same image as the AA row's MSAA; 16x falls back to 8x on this GPU; kept for combinations |
| AA: MSAA Override Off (with MSAA 4x) | 0 vs no-MSAA | | | | | | 1.5 vs native 8x | −0.03 | −1.16 | turns native MSAA off: a performance choice |
| AA: Post Filter FXAA / SMAA | = AA FXAA / SMAA | | | | | | | +0.12 / +0.16 | +0.12 / +0.35 | same image as the AA row's; kept for combinations |
| AA: Post Filter SMAA+FXAA | 4.3/166 | 3.4/181 | 5.5/188 | 2.0/193 | 2.6/193 | 5.2/211 | 3.0/213, 0.994 | +0.22 | +0.46 | visible |
| Sharpening 25 % | 13.1/43 | 11.8/43 | 14.5/51 | 6.2/49 | 6.3/49 | 17.1/55 | 9.1/54, 0.993 | +0.15 | +0.14 | visible, cheap |
| Sharpening 100 % | 35.8/81 | 30.7/77 | 33.9/93 | 24.8/87 | 25.2/87 | 40.8/91 | 29.7/95, 0.969 | +0.15 | +0.14 | visible (strong), cheap |
| Road Reflection Half | 1.0/29 | 0.6/21 | 1.2/58 | 0.7/74 | ~ | 2.0/75 | 2.8/86, 0.997 | (noisy) | +0.00 | visible on wet road, free |
| Road Reflection Full | 1.7/43 | 1.1/23 | 1.6/59 | 1.0/81 | ~ | 3.1/91 | 3.2/95, 0.995 | (noisy) | +0.04 | visible (half vs full 0.4–1.4 %), free |
| Car Reflection 256 / 1024 | 0.2/195 | 0.1/157 | 1.0/142 – 1.3/180 | 0 | ~ | 0.2/150 | 1.3/176, 0.997 | (noisy) | 0.00 / +0.03 | visible on car bodies (sharper cube); 256 vs 1024 0.1–0.9 %; free |
| Detail Distance 150 / 300 % | 0 | 0 | 0 | 0 | ~ | 0 | 0 | (noisy) | 0 | no effect in stills (also 640×480); **in motion: scenery pops 18.6 % → 5.8 % of frames, popped size 1904 → 162** (drive bench); GPU +0.04 |
| Car Detail Distance 150 / 300 % | 0 | 0 | 0 | 0 | ~ | 0.01/197 | 0.01/240 | (noisy) | 0 | one distant car's LOD; in motion car LOD pops 10 → 8; marginal, free |
| LOD Fade | 0 | 0 | 0 | 0 | ~ | 0 | 0 | (noisy) | 0 | no effect in stills; **in motion: 112 scenery pops → 0, car LOD switches all cross-faded**; CPU ~+0.06 |
| Keep All Loaded | 0 | 0 | 0.03/101 | 0 | ~ | 0 | 0.03/119 | (noisy) | 0 | far scenery present at the start; in motion sections appear at 103 m min / 373 m median instead of 38 / 190 m; CPU ~+0.1 |
| Reversed-Z Depth | 0.01/198 | 0.02/104 | 0.01/131 | 0 | ~ | 0.00/144 | 0.02/141, 1.000 | (noisy) | 0 | **no visible effect** (a few edge pixels): removed |
| Shadow Quality High / Ultra | 0.03/6 | 0.03/6 | 0.5/84 – 0.6/84 | 0 | ~ | 0.00/70 | 0.6/170, 1.000 | (noisy) | 0 (CPU +0.07 / +0.15) | small: smoother car-shadow edge in close views only (High vs Ultra 0.13 %, max 53); kept |
| Reflection Update Rate Every Frame | 0 | 0 | 0 | 0 | ~ | 0 | 0 | (noisy) | +0.01 | **no effect: native at Detail 5 already renders all six faces every frame** (rate 3, 0x7FEEA0 row 12); removed |
| Ambient Occlusion 25 % | 1.0/15 | 0.5/13 | 4.8/23 | 0 | ~ | 2.1/24 | 3.6/31, 0.999 | +0.16 | +0.55 | visible (subtle), not in the front end |
| Ambient Occlusion 100 % | 4.6/62 | 3.7/44 | 12.5/91 | 0 | ~ | 7.4/81 | 10.0/117, 0.995 | +0.14 | +0.57 | visible |
| Enhanced Reflections | 11.3/239 | 9.2/202 | 10.9/189 | 4.3/134 | 5.3/134 | 11.2/242 | 11.3/239, 0.993 | (noisy) | +0.01 | very visible, free |
| High Precision Post Half / Full | 1.7/8 – 2.0/12 | 0.6/36 – 0.8/36 | 2.9/38 – 3.5/38 | 3.0/51 – 3.6/51 | 3.7–4.3 | 3.3/15 – 4.3/27 | 3.4/20, 0.997 | +0.28 / +0.44 | +1.89 / +2.50 | visible (smoother glow, no banding), the costliest non-AA row |
| Enhanced Car Paint | 0.2/205 | 0.1/186 | 0.8/150 | 0.5/102 | 1.4/102 | 0.2/242 | 0.8/161, 0.999 | +0.03 | +0.01 | visible on cars, free |

\* Render Scale above 100 % at 4K is capped to what SSAA 2x gives (scene
budget 2 × 4K pixels), and with native 8x MSAA (the sample budget) to
100 %: at 4K on Detail 5 it did nothing at all.

Enhancements page (not graphics, audited for completeness): Field Of View
+10 changes 87 % of pixels, HUD Scale 130 % 20 % (both visible by design);
Frame Rate Cap, Durable Saves, Save Backups, Crash Reports change no pixel
(they are not image settings and stay); HD UI + Maps needs the private
pack (not present in the shot environment: 0 px). Photo Mode in Pause Menu
(docs/photo-mode.md) changes no frame by itself (it only lists the
pause menu item).

## Classification
* **Visible and worth it (cheap)**: Enhanced Reflections, Enhanced Car
  Paint, Road / Car Reflection resolution, MSAA 8x (alpha-to-coverage),
  FXAA, SMAA, Sharpening, SSAA 2x (≈ free, it replaces native MSAA),
  Shadow Quality (small, close views).
* **Visible but costly**: High Precision Post (4K +1.9/+2.5 ms), SSAA 4x
  (+1.6 ms 4K), SSAA 8x/9x/16x (+5.9…+14.7 ms 4K), Ambient Occlusion
  (+0.55 ms 4K).
* **No effect in these scenes, needs motion** (verified with the drive
  bench): LOD Fade, Detail Distance, Keep All Loaded, Car Detail Distance.
* **No visible effect: removed** — Texture Filtering (native Customize
  Display Texture Filtering at its top, as Detail 5 sets it, is already 16x
  anisotropic: `display::texture_sampler`; 2x/4x/8x only lower it),
  Reflection Update Rate (native rate 3 = every face every frame),
  Reversed-Z Depth (a handful of edge pixels, no z-fighting in any scene).
* **Merged**: Render Scale above 100 % (the same image as SSAA of that
  pixel count: 200 % = SSAA 4x pixel for pixel, and capped at 4K) → the
  Render Scale row now offers 50–100 % only and sits under Performance.
* **Duplicates kept on purpose**: AA: MSAA Override and AA: Post Filter
  render exactly the AA row's MSAA / FXAA / SMAA images; they exist for the
  combinations (SSAA + MSAA, MSAA + SMAA, SMAA + FXAA, MSAA Off). Values within a row that look alike (Shadow
  High vs Ultra, Car Reflection 256 vs 1024, SSAA 8x vs 9x) were kept:
  each still differs measurably and costs little.
* Resolution notes: SSAA's gain over 4x is larger at 1080p (4x vs 16x
  6–10 % of pixels) than at 4K (4–7 %), its cost 4× larger at 4K. High
  Precision Post costs ~7× more at 4K than at 1080p (its glow passes).

Removed rows stay readable: options.txt keeps `enh.anisotropy`,
`enh.reflection_rate`, `enh.reversed_z` (parsed and applied as before); a
group page lists such a row only while a saved file has it on, and any
change switches it off ("legacy" rows, `options::LEGACY_ROWS`). A saved
`enh.render_scale` above 100 still applies and steps back into 50–100.

## The Graphics page (Options → Display → Enhancements → Graphics)
Rows: **Preset** (Original / Clean / Modern / Ultra; "Custom" when the
values match none), six group pages, **Restore Native**. The preset is not
stored: it is recognised from the `enh.*` values (`Enhancements::preset`),
so options.txt is unchanged and old files load as before (they read
"Custom" unless they match). Choosing a preset sets every graphics value
and leaves FOV, HUD scale, frame cap, saves, crash reports and HD UI alone.

| Group | Rows (one per concept; † = shares the original's Customize Display row) |
|---|---|
| Image | Anti-Aliasing, MSAA Override, AA Post Filter, Sharpening, Texture Filtering † |
| Lighting | Ambient Occlusion, Car Shadow / Neon † (lamps, shadow atlas; Headlight Beams, Light Probes; Surface Detail, manual, docs/surface-maps.md) |
| Reflections | Enhanced Reflections, Road Reflection Detail †, Wet Roads, Screen-Space Reflections |
| Cars | Enhanced Car Paint, Enhanced Glass, Car Reflection Detail †, Car Reflection Update Rate †, Car Geometry Detail † |
| Effects | Post-Processing (was High Precision Post), LOD Fade (TAA, volumetric fog) |
| Performance | Render Scale (50–100 %), World Detail † (Keep All Loaded folded into its Ultra tier) |
| Display (docs/hdr.md) | HDR Output (greyed without an HDR signal), Peak Brightness, Paper White, SDR Gamma; the page is the calibration screen (test pattern); presets leave it alone |

Each non-native value showed its cost class in the value
(`SSAA 4x [High]`); the modern settings pass moved it to the detail line (below). Classes: Free < 0.05 ms, Low < 0.3, Med < 1, High < 3,
Extreme ≥ 3 ms of reference desktop GPU time (`options::cost`, from the 4K timings
scaled by pixels; an iGPU pays ~10×). The original Full Screen
Anti-Aliasing row still reads "Disabled - Enhancements" when an MSAA or
SSAA choice overrides it.

| Preset | Values (tier names) | 4K cost (reference desktop GPU) |
|---|---|---|
| Original | native (default) | 0 |
| Clean | Post-Processing High, MSAA 8x (+ alpha-to-coverage), Enhanced Car Paint | +1.86 ms (measured) |
| Modern | Clean + Enhanced Reflections, Road Reflection Very High, Car Reflection Very High, Ambient Occlusion Medium, Car Shadow Ultra, LOD Fade, World Detail Very High, **Wet Roads** (lamps/shadows, the temporal tier's TAA to add; the Lighting II work Headlight Beams, Light Probes) | +2.53 ms (measured) |
| Ultra | Modern + SSAA 2x, Post-Processing Ultra, Road Reflection Ultra, Car Reflection Ultra, World Detail Ultra (incl. the old Keep All Loaded), Car Geometry Detail Ultra, **Screen-Space Reflections High** (half; Ultra/full stays manual) (volumetrics to add) the Lighting II work: Enhanced Glass. | +5.36 ms (measured; iGPU estimate: SSR High alone ≈ +14.5 ms) |

Preset costs: measured as a whole in the small-settings pass (below); the earlier sums
(Clean 1.9, Modern 2.7, Ultra 4.4 ms) were inferred, Ultra's was 1 ms low. 16x anisotropic filtering, part of the planned "Clean"
preset, is native at Detail 5 (Customize Display's Texture
Filtering), so the presets do not touch it.

## Crops (the maintainer's private captures, Off | On | difference ×8)
Render Scale 75 % `1920x1080_motion_render_scale_75.jpg`; FXAA
`1920x1080_motion_aa_fxaa.jpg`; SMAA `1920x1080_start_aa_smaa.jpg`; MSAA 8x
vs native 8x (alpha-to-coverage on the palm) `1920x1080_msaa8_motion_aa_msaa8.jpg`;
SSAA 2x / 4x / 16x vs native 8x `1920x1080_msaa8_start_aa_ssaa{2,4,16}.jpg`,
4K `3840x2160_msaa8_start_aa_ssaa4.jpg`; SMAA+FXAA `1920x1080_msaa8_night_filter_smaa_fxaa.jpg`;
Sharpening 25 % `1920x1080_motion_sharpen_25.jpg`; Road Reflection Full
`1920x1080_motion_road_refl_full.jpg`; Car Reflection 1024
`1920x1080_start_car_refl_1024.jpg`; Car Detail 300 %
`1920x1080_motion_car_detail_300.jpg`; Ambient Occlusion 100 %
`1920x1080_start_ssao_100.jpg`; Shadow Ultra `1920x1080_start_shadows_ultra.jpg`;
Enhanced Reflections `1920x1080_night_enh_reflections.jpg`; High Precision
Post Full `1920x1080_garage_hdr_full.jpg`; Enhanced Car Paint
`1920x1080_start_paint.jpg`; Keep All Loaded `1920x1080_start_keep_loaded.jpg`;
removed rows: Reversed-Z `1920x1080_night_reversed_z.jpg` (only edge
pixels), Texture Filtering 4x `1920x1080_night_aniso_4.jpg` (blurrier than
native). The pages: `ui-graphics.png`, `ui-graphics-custom.png`,
`ui-image.png`, `ui-effects-4k.png`.

## Tests
`options::tests::graphics_page_steps_and_restores` (page and group rows,
every row in one group, removed rows absent, labels distinct, cost tag,
Render Scale ≤ 100 % and a saved 150 % stepping back, SSAA, Restore
Native), `graphics_presets_cycle_and_round_trip` (cycle both ways,
Custom, QoL kept, each preset round-trips options.txt and is recognised,
uses no removed row, presets grow), `removed_rows_show_while_on_and_switch_off`;
`qol::tests::enhancements_round_trip` now counts 26 deviations.
`tests/ssaa.rs` and `tests/spatial_aa.rs` read the AA row from the Image
group page. Native image byte-identical: `explore`, `qrstart`, `qrcars`
at 1080p, 0 differing pixels against the audit's base shots.
Shots `options_graphics`, `options_graphics_custom`,
`options_graphics_{image,lighting,reflections,cars,effects,performance,lod}`
(the old `options_graphics_end` scroll shot is gone with the flat page).

## Open
* Wet Roads, SSR: rows, costs (the wet-roads and SSR work's own 4K timings) and presets
  done in the modern settings pass; the audit's `run.sh` / `bench.sh` were not re-run for them.
* No iGPU was measured; the ×10 factor is an estimate.
* 1080p costs under the shared load are noisy below ~0.3 ms; the classes
  use the clean 4K run scaled by pixels.
* Reversed-Z may matter on scenes with distant coplanar geometry not in
  this set; it stays honoured from options.txt.

# Named tiers, one row per concept (2026-11-04)

Goal: every setting at a modern standard, and the car detail settings (update rate etc.)
streamlined so that no two settings do the same thing. All
clone deviations (no native counterpart); defaults, the Original preset and
Restore Native stay native.

## What changed
* Every Graphics row is one entry of a table,
  `src/frontend/graphics_table.rs` (`ROWS`): label, group, a one-line help,
  named tiers (each with its own help and measured cost), `get` / `set`
  onto the stored values. Values read as tier names (Off / Low / Medium /
  High / Very High / Ultra, or the original's own values); numbers stay
  only for Render Scale (`75% (1080p)` at 1440p: the rendered height, as the font has no arrow; `100% (Native)`), MSAA counts
  and the AA method names (`FXAA`, `SSAA 4x`), as modern games show them.
  The Enhancements page's FOV (`+10°`), HUD Scale (`110%`), Frame Rate Cap
  (`144 FPS`) were already shown that way and stay.
* A **detail line** under the list (the list's spare row slots 12–13 at
  half the row text's size, `options::detail_line`) shows the selected
  row's help, its tier's help and the tier's cost here
  (`Cost: Med (+0.55 ms here; integrated GPU ~10x)`; Render Scale below
  100 %: `Saves ~0.7 ms here`). Also on the Graphics page (each preset /
  group), the Enhancements page and HD UI. The original's pages have none.
* **One row per concept**: the clone rows that enhanced something the
  original already has a Customize Display row for are merged into that
  row. The merged row keeps the original's label (its game string), its
  values as its first tiers (same storage, `custom.N` in options.txt) and
  gets the clone's tiers above World Detail 5's value. The Customize
  Display page keeps the original's row (the original's screen); it is the
  same setting: a Low/High row reads the enhanced tier's name while one is
  on, and any change there first goes back to World Detail 5's value
  (`options::change`, `RowId::Custom`). An enhanced tier holds the
  original's value at least at World Detail 5's (`graphics_table::normalize`,
  also after the World Detail slider, Default Settings, a preset and
  loading).

## Native vs clone: which rows controlled the same thing

| Original (Customize Display, address) | Clone row before the modern settings pass | Same thing? | Now (one row) |
|---|---|---|---|
| Car Reflection Update Rate 0–3 (0x870CB8) | Reflection Update Rate (Every Frame; removed in the settings audit) | **yes** (= rate 3) | Car Reflection Update Rate †: Off / Low / Medium / High; old `enh.reflection_rate 1` loads as High (custom 0 = 3) |
| Car Reflection Detail Low/High (0x870CBC: world in the cube) | Car Reflection Res 128–1024 | same concept (reflection detail) | Car Reflection Detail †: Low / High / Very High (256) / Ultra (1024) |
| Car Shadow / Neon 0–2 (0x870CC0) | Shadow Quality Native/High/Ultra (shadow texture filter) | same concept | Car Shadow / Neon †: Off / Low / High / Ultra (filtered) |
| Car Geometry Detail Low/High (0x870CF8: start LOD) | Car Detail Distance 100–300 % | same concept (car LOD) | Car Geometry Detail †: Low / High / Ultra (200 %) |
| World Detail 0–3 (0x870CF4: scenery LOD B) | Detail Distance 100–300 % (scenery size cull) and Keep All Loaded (every section is resident in the clone anyway; it only widens the drawn set by `World::lookahead_sections` / `LOOKAHEAD_RING`) | same concept (scenery detail / draw distance) | World Detail †: Lowest / Low / High / High (max) / Very High (150 %) / Ultra (200 % + lookahead ring) / Extreme (the assets-and-distance work: Ultra + the sections a 400 m ring lends, at their lowest LOD; `enh.far_draw 1`, docs/assets-distance.md); old `enh.keep_loaded 1` loads as Ultra. Residency becomes an automatic memory-budget decision, not a setting |
| Road Reflection Detail 0–3 (0x870CD4) | Road Reflection Res Native/Half/Full | same concept | Road Reflection Detail †: Off / Low / Medium / High / Very High (half screen) / Ultra (full screen) |
| Texture Filtering 0–2 (0x870D18; 2 = 16x anisotropic) | Texture Filtering 2x–16x (removed in the settings audit) | **yes** (2x–8x only lowered native 16x) | Texture Filtering †: Point / Linear / 16x Anisotropic; old `enh.anisotropy N` loads as 16x Anisotropic (custom 20 = 2) |
| Full Screen Anti-Aliasing 0–3 (0x870D08) | Anti-Aliasing MSAA 2x/4x/8x, MSAA Override | overlap | **kept** as the player's 2026-11-02 combination design: the original row reads "Disabled - Enhancements" while an AA choice overrides it, so only one acts |
| Over Bright, Light Glow (0x870CFC, 0x870CE4) | Post-Processing (High Precision Post) | no: precision and glow size of those passes, not on/off | separate |
| Rain Splatter (0x870D10) | Wet Roads | no (screen drops vs road surface) | separate |
| Level Of Detail 1–5 (Display page, presets the values) | Graphics Preset | no: presets of different values (`custom.*` vs `enh.*`) | separate |
| Motion Blur (0x870CDC: the speed blur 0x5CC470) | Motion Blur Camera / Per-Object (docs/motionblur-dof.md) | same concept (blur from motion) | Motion Blur †: Off / On / Camera / Per-Object (`custom.11`, `enh.motion_blur` 1–2) |
| Depth Of Field (0x870CF0: its pass 0x5CCCE0 never runs behind the chase camera, not ported) | Depth Of Field Cinematic | same concept | Depth Of Field †: Off / On / Cinematic (`custom.13`, `enh.depth_of_field 1`) |
| Car Headlight, Crowds, Light Trails, Particle System, Fog, Tinting, Horizon Fog, Enhanced Contrast, Vsync | — | no clone counterpart | Customize Display only |
| — | Sharpening, Ambient Occlusion, Enhanced Reflections, Wet Roads, SSR, Enhanced Car Paint, Post-Processing, LOD Fade, Render Scale | no native counterpart | Graphics only |

## Tiers (old values → new tiers)

| Row | Old values | New tiers → stored value | Old value loads as |
|---|---|---|---|
| Anti-Aliasing | Native, FXAA, SMAA 1x, MSAA 2/4/8x, SSAA 2/4/8/9/16x | same names (`enh.post_aa` 0–10) | unchanged |
| MSAA Override (was "AA: MSAA Override") | Preset/Off/2x/4x/8x/16x | Auto / Off / 2x / 4x / 8x / 16x | unchanged |
| AA Post Filter (was "AA: Post Filter") | Preset/Off/FXAA/SMAA/SMAA + FXAA | Auto / Off / FXAA / SMAA / SMAA + FXAA | unchanged |
| Sharpening | Off, 25, 50, 75, 100 % | Off / Low 25 / Medium 75 / High 100 | 50 → 75 (tie up) |
| Texture Filtering | (native slider) + clone 2x–16x | Point / Linear / 16x Anisotropic (custom 20) | any `enh.anisotropy` → 16x |
| Ambient Occlusion | Off, 25, 50, 75, 100 % | Off / Low 25 / Medium 50 / High 100 | 75 → 100 |
| Car Shadow / Neon | (native 0–2) + Native/High/Ultra | Off / Low / High (native) / Ultra (`enh.shadows 2`) | High (1) → Ultra |
| Enhanced Reflections, Wet Roads, Enhanced Car Paint, LOD Fade | Off/On | Off / On | unchanged |
| Keep All Loaded | Off/On | no row: World Detail Ultra (`enh.keep_loaded 1` + `enh.detail_distance 200`) | On → World Detail Ultra |
| Road Reflection Detail | (native 0–3) + Native/Half/Full | Off / Low / Medium / High / Very High (`enh.road_reflection 1`) / Ultra (2) | unchanged |
| Screen-Space Reflections | Off/Half/Full | Off / High (half) / Ultra (full) | unchanged |
| Car Reflection Detail | (native Low/High) + 128/256/512/1024 | Low / High / Very High (256) / Ultra (1024) | 512 → 1024 (power-of-two tie up) |
| Car Reflection Update Rate | (native 0–3) + Every Frame | Off / Low / Medium / High | Every Frame → High |
| Car Geometry Detail | (native Low/High) + 100/150/200/300 % | Low / High / Ultra (200 %) | 150, 300 → 200 |
| Post-Processing (was High Precision Post) | Off/Half-Size Glow/Full-Size Glow | Native / High / Ultra | unchanged |
| Render Scale | 50/67/75/85/100 % | same, shown with the rendered height | >100 % still loads, steps back |
| World Detail | (native 0–3) + 100/150/200/300 % | Lowest / Low / High / High (max) / Very High (150 %) / Ultra (200 % + lookahead ring) / Extreme (Ultra + `enh.far_draw 1`, the assets-and-distance work) | 200, 300 → Ultra |
| Smooth Car Shapes (Cars) | — | Off / On (`enh.smooth_cars`) | — |
| Reversed-Z (removed in the settings audit) | Off/On | listed only while on | unchanged |

"Opt-in stays on": a saved enhanced value never loads as native
(`qol::nearest`). Tiers write their own values, which read back unchanged
(`qol::tests::old_values_map_to_the_nearest_tier`). The options.txt keys
are unchanged; the merged rows' native tiers are the existing `custom.N`
lines.

## Tier steps measured (adjacent tiers, 1920×1080, `scripts/tiers.sh`)
Changed % of pixels / max delta between a tier and the one below
(`data/tiers_1920x1080.tsv`, `data/pops_tiers.txt`):
* Sharpening (start / motion): 0→25 14.5 / 17.1 %, max 51–55; 25→50 only
  1.4 / 1.5 %, max 10 (too small: dropped); 25→75 8.5 %, max 25; 75→100
  10.4 %, max 25 → tiers 25 / 75 / 100.
* Ambient Occlusion (start / night): 0→25 4.8 / 1.0 %; 25→50 4.9 / 1.2 %;
  50→100 8.3 / 2.6 %, max 59 → 25 / 50 / 100 (75 dropped).
* Car reflection cube (start): 128→256 0.97 %, max 142; 256→512 0.71 %;
  512→1024 0.46 %, max 111 → 256 / 1024 (each step ≥ 0.97 %).
* World Detail distance (drive bench, scenery pops, 600 frames): native
  18.6 % of frames, size 1904; 150 % 6.3 % / 324; 200 % 4.0 % / 126; 300 %
 5.8 % / 162 — no better than 200 % → tiers 150 / 200. Keep All
  Loaded: sections first drawn at 103 m min / 373 m median instead of
  38 / 190 m → added to the 200 % tier (Ultra).
* Car Geometry distance (car LOD switches): native 10; 150 % 9; 200 % 6;
  300 % 8 → one tier, 200 %.
* Car Shadow High (filter 1) vs Ultra (2): 0.13 %, max 53 → one
  tier. Road Reflection half vs full 0.4–1.4 %, SSR half vs full,
  Post-Processing half vs full glow kept.
* World Detail 3 draws as 2 in the clone (0x5BE1D0 returns 2 for both);
  it stays selectable as the original's value ("High (max)").
Crop: `1920x1080_start_tier_car_refl_256_512.jpg`.

## Costs
Each tier carries its 4K GPU / CPU ms (`Tier::cost`) from the settings audit
(and the wet-roads work for Wet Roads / SSR); the detail line scales it to the output.
Render Scale 67 % / 85 % costs are **inferred** (interpolated between the
measured 50 % and 75 %). Ultra preset: +5.4 ms measured, not the 4.4 ms sum.

## Adding a row
1. A `RowId` variant and a label id with its text (`options::clone_label`,
   next free id 0xC1_04E0_4A+).
2. The stored value: a `qol::Enhancements` field with `parse` / `write` /
   `apply` / `deviations` (tier values as a `*_STEPS` const; `parse` maps
   old values with `nearest`).
3. One `GraphicsRow` in `graphics_table::ROWS` at its group's place: tiers
   (name, help, `tc(…, gpu_4k_ms, cpu_ms)` for a measured cost), `get`,
   `set`; `native: Some(i)` if it extends a Customize Display row.
4. Add the `RowId` to the Graphics arm of `options::change`; presets in
   `qol::Preset::apply`; the row order in
   `options::tests::graphics_page_steps_and_restores`.

## Tests
`options::tests::graphics_page_steps_and_restores` (row order, native tiers,
tier names distinct, every tier visited both ways, detail lines, Render
Scale text), `shared_rows_extend_the_original_values` (native tiers then
enhanced, the Customize Display row, World Detail slider, old duplicate
keys, normalize a no-op at native), `graphics_presets_cycle_and_round_trip`
(each preset's values are tiers), `removed_rows_show_while_on_and_switch_off`,
`qol::tests::old_values_map_to_the_nearest_tier`.
Pages: `ui-g6b-*.png`.

# Restore Native, HD UI + Maps, presets as a whole (2026-11-04)

Clone deviations only; Original stays byte-identical. Build <work dir>
(base + this work), reference desktop GPU, RADV. The machine was shared (load average
23–40, other builds and benches); all GPU timings are GPU busy
medians of `--bench explore 300` (timestamps), the values interleaved per
repetition (3 reps, base at start and end). The 4K and 1080p GPU numbers
were stable this time: spread between reps ≤ 0.05 ms 4K, ≤ 0.02 ms 1080p
(one 1080p HD UI "off" outlier, 0.80 ms, dropped). Data:
internal evidence, scripts `scripts/s1/`
(`all.sh` drives `run.sh`, `bench.sh`, `hdui.sh`, `bench_hdui.sh`).

## Restore Native gives back raised Customize Display values
An enhanced tier of a shared row (World Detail, Car Shadow, Road /
Car Reflection Detail, Car Reflection Update Rate, Car Geometry Detail,
Texture Filtering) holds the original's value at least at World Detail 5's;
on a machine set lower, Restore Native and the Original preset left it
raised. Now `graphics_table::custom_part` remembers the player's own value
before the first raise (`Settings::custom_raised`, options.txt
`enh.raised.N v`, written only while a value is remembered, so a native
file is unchanged), and
`graphics_table::normalize` gives it back once no enhanced tier holds
that row: Restore Native (`Settings::restore_native` now normalizes), the
Original preset, or a preset without that tier (Ultra → Clean). A native
value chosen on the row (Graphics or Customize Display), the World Detail
slider and Default Settings are the player's own choice and clear the memory
(the slider's new values are remembered if a tier re-raises them). Test:
`options::tests::restore_native_gives_back_raised_custom_values`.

## HD UI + Maps (one row; the private pack via `NFSU2_TEXTURE_PACK`)
Off (`NFSU2_TEXTURE_PACK=""`) vs On (`enh.hd_ui 1` in the shot's options.txt
+ the pack), changed % / max / SSIM:

| Scene | 1080p | 4K | What changes |
|---|---|---|---|
| motion (race HUD) | 4.55 / 234 / 0.989 | 4.69 / 235 / 0.985 | tachometer, speed, HUD art sharper (crop `s1_1920x1080_motion_hd_ui.jpg`) |
| explore (HUD, minimap) | 3.57 / 234 / 0.992 | 3.73 / 237 / 0.990 | HUD and minimap |
| ingamemap (full map) | 11.6 / 194 / 0.976 | 11.3 / 227 / 0.967 | HD map art: more road detail (`s1_1920x1080_ingamemap_hd_ui.jpg`) |
| worldmap | 2.09 / 194 / 0.995 | 1.92 / 224 / 0.991 | map art (`s1_1920x1080_worldmap_hd_ui.jpg`) |
| main menu | 3.86 / 213 | 4.47 / 236 | logo / menu art (menu noise alone 0.6–1.7 %) |
| qrstart (race-start scene, no HUD) | 0 | 0 | none |

Cost: GPU +0.03–0.05 ms at 1080p, +0.10–0.15 ms at 4K (larger textures;
Low class); CPU per frame unchanged. Shot wall times with the pack on were
0.0–0.7 s longer (texture decode at load; noisy under the load, not a
benchmark). Verdict: visible in every HUD / map scene, cheap: kept.
VRAM use was not measured (open).

## Wet Roads and Screen-Space Reflections in the audit's scenes
Changed % / max vs Original (1080p; 4K within ±0.3 %):

| Value | night | rain | start | garage | menu | motion | GPU 4K dry / rain | GPU 1080p |
|---|---|---|---|---|---|---|---|---|
| Wet Roads | 5.1 / 71 | 11.6 / 127 | 6.1 / 101 | 0 | ~ | 2.5 / 78 | +0.00 / −0.01 | +0.02 |
| SSR High | 8.2 / 83 | 6.8 / 60 | 5.6 / 75 | 0 | ~ | 5.2 / 88 | +1.15 / +1.25 | +0.35 / +0.37 |
| SSR Ultra | 8.1 / 83 | 6.7 / 60 | 5.5 / 94 | 0 | ~ | 5.2 / 95 | +1.80 / +2.03 | +0.48 / +0.53 |

SSR High vs Ultra directly: 0.2–1.4 % of pixels, max 21–47, SSIM ≥ 0.9994
(a small step for +0.65–0.8 ms at 4K; kept as the manual top tier as the wet-roads work
decided). The tier costs in `graphics_table` (SSR 1.2 / 1.98, Wet Roads
0.04 ms 4K) agree with these within the spread and were left unchanged.
Wet Roads also changes the night and start scenes (5–6 %); why it acts
outside the rain clock was not traced (the native wetness is 1 at every track load and only sizes the flares; Wet Roads now follows the rain only, dry scenes = Off; docs/wet-roads-ssr.md). Crop
`s1_1920x1080_rain_wet_roads.jpg`.

## Presets as a whole
Against Original with native 8x MSAA (`NFSU2_MSAA=8` shots), changed % /
SSIM (1080p): Clean 1.3–4.8 % / ≥ 0.993; Modern 7.0–19.1 % / ≥ 0.978;
Ultra 10.3–27.1 % / ≥ 0.968 (crop `s1_1920x1080_msaa8_start_ultra.jpg`).
GPU busy over Original (base 2.17 ms 4K, 0.60 ms 1080p):

| Preset | 4K | 1080p | Old sum (4K) |
|---|---|---|---|
| Clean | +1.86 (1.85–1.87) | +0.26 | 1.9 |
| Modern | +2.53 (2.49–2.54) | +0.44 | 2.7 |
| Ultra | +5.36 (5.33–5.38) | +1.29 | 4.4 |

Ultra split at 4K: Ultra without SSR +3.25; SSR High inside Ultra +2.1
(vs +1.15 alone; SSAA 2x + SSR alone +1.29 over Original, SSAA 2x alone
−0.19). SSR traced the 2x-supersampled image; the reflections pass traces at the screen's
size (SSR inside Ultra +1.85 ms at 4K, docs/wet-roads-ssr.md). The Graphics page's preset help
now shows the measured 4K numbers (+1.9 / +2.5 / +5.4 ms).

## IGPU numbers
Still not measured (no iGPU here). The detail line now reads
`integrated GPU est. ~10x` and the SSR High help `integrated GPU: est.
~15 ms`, so the help marks them as estimates.

# TAA, Upscaling, Volumetric Fog (2026-11-04)
Anti-Aliasing gains **TAA** (`enh.post_aa 11`, exclusive with MSAA / SSAA /
post filters); Performance gains **Upscaling** (Off / Quality / Balanced /
Performance, `enh.upscale`; Render Scale greyed under it); Lighting gains
**Volumetric Fog** (Off / Low / High, `enh.volumetric_fog`). Ultra adds
Volumetric Fog High; TAA and Upscaling stay manual. Measurements, costs
and crops: docs/temporal.md.
