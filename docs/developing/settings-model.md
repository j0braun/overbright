# Settings model and the conflict & redundancy audit (2026-11-06)

The settings rework, first half ("Conflict & redundancy audit"). The settings page (docs/developing/settings-page.md) is built on the
data delivered here. Nothing here is native: every row audited is a clone enhancement (or a native Customize
Display row an enhancement depends on); Original stays byte-identical (gate below).

* Code: `src/frontend/settings_model.rs` (`MODEL`, `RULES`, `matrix_configs`), `qol::Enhancements::effective`
  (the engine rules), `options::graphics_rows` (the page shows the rules), `graphics_table::GraphicsRow::is_offered`
  (blocked tiers are not offered), `scripts/checks/matrix.sh` (the matrix; in `checks.sh full`).
* Evidence: the maintainer's private captures (`pairs.txt` stills at the look direction spots, `pairs-bench.txt` driving frames,
  `costs.txt` GPU ms, `matrix.txt`, crops, the scripts).

## Method

* **Image difference**, base vs base + the stacked row (internal evidence,
  `pairmetrics.py`): the look direction spots at the original's cameras (the maintainer's tool `cshot.sh`, 1280×960, native 8x MSAA
  as World Detail 5 plays): street, harbour neon (casino), Coal Harbor docks, Jackson Heights, Quick Race start;
  rain street / rain harbour for the weather rows. Per frame: % of pixels changed (any channel > 2 levels, HUD
  masked), max delta, SSIM (luma, Gaussian σ 1.5), mean **FLIP** (NVIDIA ꟻLIP 1.7, `flip-evaluator`, BSD-3, LDR;
  a measuring tool in the work dir, not vendored), the look direction deltas (mean / p5 / p95 / RMS / glow) and the mean ΔY
  binned by the base's luma (0–16, 16–48, 48–128, 128–255: where a darkening lands — the "double darkening"
  check).
* The still shots run neither TAA, Upscaling nor FSR (they act on the driving frame only: TAA alone measured
  0 pixels against native in a shot). Those pairs use the racing bench's frames 240 and 420 (at speed, 1920×1080,
  deterministic run to run; `pairs_bench.sh`, `NFSU2_BENCH_DUMP_FRAMES`).
* **Cost**: `perf.sh bench`, racing / race / rain at 3840×2160, 3 runs × 300 frames, `ORDER=alternate`, GPU busy
  median of the run medians (`costs.sh`; reference desktop GPU, load average 20–60 from other jobs; GPU noise ≤ 2 %).
* The Customize Display switches (`custom.N`) do not act in the shot path for Car Shadow / Neon (custom.2: 0 and
  2 give the same frame); rules on native rows are therefore from the code (traced in the clone, addresses in
  the rule's `evidence`), and the car-shadow-under-Modern-Lighting question stays open.

## Audit: pair → finding → decision

GPU ms at 4K; "px" = % of pixels changed; Δ = the look direction mean / p5 / p95 luma.

| stack (row under / over row) | finding (image) | cost 4K | decision |
|---|---|---|---|
| SSAA over TAA | one row (exclusive by construction) | — | kept |
| SSAA / any AA tier under **Upscaling (TAA)** | **0 px** both frames: silently nothing (the row read "TAA (Upscaling)" but stayed active) | — | **Overridden** (greyed, "Overridden by Upscaling (TAA)") |
| SSAA under **FSR 2** | 0.00–0.008 px (max 28) | — | Overridden (was greyed; now says why) |
| **SSAA under FSR 1** | 6.6–7.4 px, SSIM 0.991–0.992: SSAA's `msaa_for` = 1 **removed the native MSAA** under FSR 1 (FSR scales the scene itself, so no supersampling happened) — a real conflict | 1.42 → 1.03 (cheaper because worse) | **Engine + Blocks**: SSAA and TAA tiers not offered under FSR 1, a stored one is not drawn (`effective`: post_aa → native) |
| **TAA with FSR 1** | 31–35 px, SSIM 0.93–0.94: TAA turned FSR 1 into the temporal tier's temporal upscaler (0.5 % from TAA Upscaling Quality); FSR 1 itself was gone | — | **Engine + Blocks** (as SSAA) |
| MSAA 8x override under FSR 1 | 6.5–9.1 px: alpha-to-coverage, works | — | kept |
| MSAA override / AA filter under TAA | 0 px | — | Overridden (were greyed; now say why) |
| **MSAA 8x under SSAA 2x** (the reference play setup has MSAA Off) | 4.5–4.7 px, SSIM 0.995–0.996, FLIP 0.010–0.012: real, small edge gain | **+9.60** (8.70 → 18.30) | kept (a 2026-11-02 combination design); the cost hint was inferred 6.4 → now measured (1.2 ms × samples × SSAA pixels/2) |
| SMAA under SSAA 2x | 2.0–2.9 px, FLIP 0.006–0.008 | (0.35 hint) | kept |
| **Dynamic Lamps under Modern Lighting** | **0 px** at 4 spots + race start | +0.00 (3.632 → 3.630) | **Engine + Overridden** ("Overridden by Modern Lighting"; was greyed at its value: silently nothing) |
| Lamp Shadows Low under Modern Lighting | 0.03–4.4 px (your headlights' shadow; street −1.0 mean) | +0.20 | kept |
| Lamp Shadows High vs Low under Modern | 0.13–0.82 px, max 54 | +0.06 (n.s.) | **Blocks** High (was greyed; now says why) |
| Contact Shadows under Modern Lighting | 0.5–2.0 px, harbour −0.87 mean (alone 1.6–2.4 px): it still grounds the cars | +0.16 (race) | kept |
| **SSAO over the bake** (no visible difference reported) | no double darkening in the bake's darks: ΔY in 0–16 ≈ 0 (≤ 0.16 at 100 %); it darkens lit mid-tones (48–128: −0.2..−1.0 at 25 %, −1.5..−3.8 at 100 %), the cars' own bodies and the underglow pool (crop `ssao100_street_sbs.png`). At 25 % (the play setup): Δmean −0.13..−0.38, SSIM ≥ 0.9995, FLIP ≤ 0.013 — below visibility | 50 %: +0.72; under Modern +0.64; Low / Medium / High cost the same (0.91–0.96) | **kept, open**: the tier text says Low is barely visible; masking the car bodies / underglow in `ssao.wgsl` (its `glow_share` is too narrow) is a shader change left to a look unit (not this audit's scope) |
| Car Shadow Ultra over native High | 0–0.57 px (smoother edge, neon banding) | 0.17 | kept |
| Modern Lighting's shadows over the bake (strength 0, "Classic") | 7–21 px, Δmean −2.0 (street) / −5.2 (docks) / +0.55 (neon): shadows where the bake had none (cars, posts) | (ML) | kept (the look direction's rule-E3 tuning covers it) |
| Light Probes under Modern Lighting | 2.4–6.6 px, as alone (2.8–6.8): independent (cars pick up neon colour) | −0.07 (noise) | kept |
| **Spray under Wet Road Interaction** | **0 px** (rain street / harbour) | +0.00 | **Engine + Overridden** |
| **Wet Roads under Wet Road Interaction** | **0 px**; with Rain Detail Next-Gen 0.19–0.31 px (its puddle rings read Wet Roads alone) | −0.08 (noise) | **Engine + Overridden**: Wet Road Interaction "includes Wet Roads' wet surface" — now also for the rings |
| SSR without Road Reflection Detail | 0.16–0.63 px (car paint only) vs 6.9–10.2 px with it: the roads need the road reflection | 3–5 ms stays | **Partial**: the value reads "Ultra (road reflection off)" and the detail line says why |
| SSR under RT reflections | one row (Ray Traced is SSR's tier) | — | kept |
| **Sharpening over FSR 1 / FSR 2** | FSR's RCAS already sharpens; Medium CAS on top: 47–53 px (FSR 1) / 31–35 px (FSR 2), p95 +0.9..+5.0 (R3 ±4), p5 −1.5..−2.1: halos | +0.17 / +0.14 | **Engine + Overridden** ("Overridden by FSR's sharpening") |
| Sharpening over TAA | 22–26 px; the TAA tier's help asks for it | (0.14) | kept |
| Motion blur (Camera) under TAA / FSR 2 | at speed 3.6 / 5.1 px vs 4.9 alone: works, no extra smear in the frames | +0.59 | kept (motion check: frame 420 at speed) |
| **Volumetric Fog High's headlight cone under Headlight Beams Next-Gen** | the same beam's cone drawn twice (the Next-Gen car-lights work's pass and High's player cone): rain +1.10 / +1.13 mean, glow +0.04..+0.07, 20–21 px | +0.13 | **Engine**: High leaves the player's cone to the Next-Gen car-lights work (`volumetric_frame`); Next-Gen fog already took the Next-Gen car-lights work's cones as a soft fill |
| Particles / Sparks & Fire without Particle System | the native emitters are off (`particles_off = custom[10] == 0`): nothing to enhance | — | **Needs** ("Needs Particle System") |
| Lens Drops without Rain Splatter | the extra drops join the native lens pass, drawn only with `custom[19] && custom[10]` (postfx `Switches`) | — | **Needs** ("Needs Rain Splatter") |
| Modern Strength with Modern Lighting off | nothing to set | — | **Needs** (was greyed at its value) |
| Post-Processing Native under HDR Output | HDR forces the float chain (`hdr_post` ≥ 1) | — | **Blocks** Native |
| Render Scale under SSAA / Upscaling | set by them | — | Overridden (were greyed; now say why) |
| Native car shadow under Modern Lighting | not measurable in shots (custom.2 does not act there) | — | **open** |
| Rain Detail Next-Gen's haze vs Volumetric Fog | The final effects pass's unit (rain haze) | — | left to the final effects pass |

Engine rules (`Enhancements::effective`) change only stacks that drew nothing extra (0 px) or conflicted; the
stored value is kept (options.txt unchanged), so turning the overriding row off restores the row's own value
(test `switching_back_restores_the_value`).

The look direction of the engine changes against the stack before: Dynamic Lamps / Spray / Wet Roads 0 (identical
frames); SSAA or TAA under FSR 1 → FSR 1 with its MSAA (the native-MSAA FSR 1 frame: the SSAA stack was +0.11
mean / +0.2..+1.5 p95 brighter-edged, TAA −0.5 mean / −1..−4.7 p95); Sharpening under FSR → FSR's own (the
stack's p95 +0.9..+5.0 removed). The fog-cone fix (Volumetric Fog High + Headlight Beams Next-Gen against
Original, before → after; `fogcone.txt`, grid `fogcone_rain_street_before_after.webp`): street mean +2.91 → +2.21,
p95 +1.91 → +0.78, glow +0.053 → +0.014; rain street +2.83 → +1.76 (p5 +0.21 → 0); rain harbour +3.48 → +2.44
(p95 +1.91 → +0.27); race start +2.42 → +2.01 — every row now inside R1–R4. Sky Next-Gen and perf.sh's MAX (with
Modern Lighting + Dynamic Lamps, Wet Road Interaction + Spray + Wet Roads: the engine rules' stacks) are 0 pixels
from the base at the four spots.

`pairs.txt` also holds still-shot rows for the upscaler / sharpening pairs: those are not the driving frame (the
shot path runs no TAA / FSR) and are superseded by `pairs-bench.txt`.

## Bugs the matrix found (fixed)

1. **Enhanced Glass under TAA Upscaling / FSR 2** (any upscaling temporal tier below the screen's size): pipeline
   creation failed ("pipeline-overridable constant 'MIP_BIAS' not found", label `car glass`): the glass module is
   not the mip-biased one, but got the scene's `MIP_BIAS`. `car_glass.rs` drops that constant (no shader change).
2. **Switching Upscaling Off → TAA Upscaling / FSR 2 live** (the pause menu's Graphics page): every render
   worker panicked ("attachments have differing sample counts: depth 1, colour 8"); a start with Upscaling on was
   fine. The main post chain kept the unstaged frames' 8x multisampled scene when the stage took over;
   `PostFx::set_upscale` now drops it on entering the staged path (as a start with Upscaling has it).
   Reproduced with `NFSU2_BENCH_ENH_SWITCH="60:enh.upscale 1" --bench explore 90` (4 panics → 0).
3. **Post-Processing switched live under Headlight Beams Next-Gen**: the `car lights` pipelines stayed in the old
   scene format ("RenderPass uses Bgra8Unorm, pipeline Rgba16Float"); `CarLightsPass` keeps its format and is
   remade when it changes.

4. **High Precision Post switched live under Sky Next-Gen**: the `sky next-gen` pipeline had one slot, replaced
   for the new format while a pass in the old one still drew ("RenderPass Bgra8Unorm, pipeline Rgba16Float",
   intermittent); `NightSky` keeps one pipeline per target and each pass takes the one of its scene mode
   (images: 0 pixels from the base, above).

Note: a render worker's panic does not end the process (rc 0); the matrix and every check that greps logs for
`panicked|validation` catch it, an exit code alone does not.

## The matrix (`scripts/checks/matrix.sh`, in `checks.sh full`)

One bench process per scene (explore, race start, racing, rain at clock 162; 1280×720, `NFSU2_RT=1` for the
ray-traced tiers), `NFSU2_BENCH_MATRIX=orig,max,rules[,pairs]`: the settings of
`settings_model::matrix_configs`, each drawn 3 frames, switched live as the page does —

* `orig`: Original + every row at every tier (189 settings);
* `max`: every row at its top offered tier, then every row at every tier under it, and every Customize Display
  value under it (the native rows under every enhancement; 1 + 189 + 49);
* `rules`: each rule's row at every tier with the rule firing, on Original and on max;
* `pairs` (explore, racing): Original + every two rows at their top tiers (44 rows: 946 pairs).

It is generated from `graphics_table::ROWS`, so a new row or tier is in the matrix without editing it. Fails on
a non-zero exit, any panic / validation error / RecvError in the log, or fewer settings drawn than listed.
Results: internal evidence. `NFSU2_BENCH_ENH_SWITCH` also takes several switches now
(`"30:…|60:…"`, a leading `!` resets to Original first, `custom.N V` lines).

## The settings model (for the settings page)

`settings_model::MODEL`: one entry per clone row (every Graphics row and every Enhancements-page row; test
`model_covers_every_row`):

* **group** (the settings rework's seven): Display (HDR Output + calibration), Image Quality (AA, sharpening, filtering,
  post precision, LOD fade, reflections, car paint / glass / detail, smooth cars, fix models), Lighting & Effects
  (Modern Lighting + strength, SSAO, shadows, lamps, beams, probes, surface detail, fog, sky, particles, sparks,
  motion blur, DOF, ambient), Weather & Water, Performance (render scale, upscaling, world detail, frame cap, low
  latency, in the background, power saving, overlay), Gameplay & Accessibility (FOV, HUD, palette, flashing, lists, outrun variety,
  traffic, replay cameras, photo mode, mark spot, saves, crash reports), Packs (HD UI, reload, texture pack,
  generate);
* **origin**: `Shared(n)` (the original's Customize Display row n as its first tiers: texture filtering, road /
  car reflection, update rate, car geometry, car shadow, world detail, motion blur, DOF), `Enhanced`, `Utility`
  (no image change), `Native` (reserved for the original's own rows on the settings page's page);
* **applies**: Live, `NextLoad(what)` (smooth cars, fix models, traffic density, texture pack),
  `Restart` (HD UI without Reload Packs), `RestartFor(tiers)` (Modern + RT, Ray Traced SSR);
* **cost hint**: the tiers' `cost` in `graphics_table` (the frame-budget fixes' 4K numbers under max, the effects-tiers pass rows; MSAA under
  SSAA now measured);
* **dependencies / exclusions**: `settings_model::RULES` — row, by, effect (`Overridden`, `Blocks` tiers,
  `Needs`, `Partial`), when, the plain-words reason the detail line shows, `engine` (resolved by the renderer
  too) and the evidence. `shown()` gives the value text and enabled state; `note()` the reason. Tests:
  `rules_are_well_formed` (rows exist, a Blocks rule leaves a tier to step to, nothing but a Needs fires on
  Original), `engine_rules_match_effective`.

## Stored values

No options.txt key is renamed or merged: the rules act on the values in force, the stored values stay as they
are (`effective()` only changes what is drawn). Presets are unchanged (`Preset::apply`
values).

## Label ids

None added (the page shows the rules through the existing value text and detail line).

## Open

* SSAO darkens the cars' own bodies and the underglow pool (crop); a mask (car stencil / wider `glow_share`) is a
  shader change for a look unit. The Low tier is below visibility over the bake at 0.91 ms.
* Native car shadow under Modern Lighting: not measurable in shots (custom.2 does not act in the shot path).
* MSAA 8x under SSAA 2x: +9.6 ms at 4K for 4.5 % sharper edges — kept by the player's combination design; the settings page may
  want a warning badge.
* Rain Detail Next-Gen's haze vs Volumetric Fog.

## Gate

* `CHECKS=full scripts/checks/gate.sh … <base> extreme`: CHECKS PASS (full, 592 s: build, 1242 tests passed / 0
  failed, selftest, 275 shots × 2 with 0 failed and 0 new nondeterministic, flows, playthrough, showcase, the
  matrix 4 × clean, provenance); PARITY all gates passed (Original images AE 0, 144 Hz window path AE 0, enhanced
  incl. Extreme / Extreme FSR AE 0, 29 shots 0 differ, audio identical, five sim hashes identical); binaries
  differ (md5 of base and new).
* `perfgate.sh run` against `current`: **pass** (no significant regression > 3 %; load average 8–22).
