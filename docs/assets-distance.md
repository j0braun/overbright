# Assets and distance (2026-11-05)

Three opt-in clone deviations. Off — the default — every one
of them leaves the game byte-identical to before (parity gates below).
Nothing here is native: there is no SPEED2.EXE counterpart to trace; the
native values they build on are cited with their addresses where they are
traced (docs/formats.md, docs/world-systems-parity.md).

## 1. Smooth Car Shapes (Graphics → Cars, `enh.smooth_cars`)

`src/car_smooth.rs`, called once per car load in `Car::load_tuned` after
Fix Original Model Errors, on the CPU (geometry load path; no shader
change). Only the fitted parts' top-LOD (`_A`) lit car meshes are touched
(body, kits, bumpers, skirts, hoods, scoops, spoilers, mirrors, rims and
tyres, brakes; traffic cars too). Method: curved PN triangles (Vlachos et
al. 2001), conservative:

* An edge curves only when two triangles share it with a consistent
  winding and the turn along it is inside a band of **3°–50°**: on a
  smooth-shaded edge (same normals both sides) the angle between its ends'
  normals, on a crease (normals split) the angle between the crease
  line's directions at its ends (cross product of the two sides'
  normals; a crease end that turns more than 30° off the edge is a corner
  and keeps the edge straight). Flat panels (equal normals), hard corners,
  open borders (part seams, cut-outs) and edges of three or more triangles
  stay straight, so parts still meet exactly and flat areas stay flat.
* A curved edge's largest offset from its chord (sampled at 1/16 steps) is
  held to **≤ 2 cm** and **≤ 10 %** of its length; under **2 mm** the edge
  stays straight (nothing visible, triangles saved). Interior points are
  held to 2 cm as well. **Outward only**: an edge whose curve would sink
  into the car, or a patch point below its source triangle, stays on the
  triangle (car parts are layered millimetres apart; a sunken panel let
  the layer under it through in the first review). Measured maximum
  vertex displacement from the source triangle's plane: 20.0 mm on the
  cars that reach the cap (internal evidence: assets-distance/car_smooth_stats.tsv).
* A triangle with a curved edge becomes 3×3 triangles on its cubic patch;
  others keep their vertices and indices. Shared edges are evaluated once
  in one direction (bit-identical points both sides: no cracks); a straight
  edge next to an unsplit triangle gets no points of its own (they fold
  onto its corners: no T-junctions). UVs, colours and normals are
  interpolated linearly across the source triangle: textures and shading
  stay where they were. The solid's box stays the file's (the car
  shadow's box reads it).
* A solid that would pass 65 535 vertices drops to 2×2, else is left alone
  (none did).

Higher-detail wheels come from the same pass: the tread is a smooth
cylinder whose ring edges are creases against the sidewall, so the tyre's
outline and its rim lip become round (crops below); no separate wheel
geometry was invented.

Excluded parts (`car_smooth::EXCLUDED`, from the review): lamps
(`BRAKELIGHT`, `HEADLIGHT`), `DECAL`, `BADG`, `PLATE`, the trunk audio,
`HOOD_UNDER`, `TRUNK_UNDER`, `ENGINE`, the AEROSTYLE scoops
(`ROOF_STYLE07*`) and the exhaust tips (`EXHAUST`).

Tests: `car_smooth::tests::{flat_panels_stay, fold_curves_within_tolerance,
creases_stay, off_by_default, player_cars_stay_within_tolerance}`.
Statistics per car file: `devprobes/src/bin/car_smooth_stats.rs` (`PER_SOLID=1`
per solid). Sheets: `model_sheets OUT --smooth` (new flag).

### Cost

* Load: 3.5–6 ms of CPU per player car (its fitted parts), under 1 ms
  per traffic car, on the loading threads (`NFSU2_TIMING`, span "Car:
  smooth shapes"); a fitted 240SX goes from ~6 300 to ~26 000 LOD-A
  triangles. Whole car files (every kit): 3.5× the triangles on average
  (`car_smooth_stats.tsv`).
* Frame (`perf.sh bench`, SET=quick, 5 + 5 runs, same binary with and
  without the row, internal notes):
  no significant change in any GPU or CPU metric (racing 4K GPU 2.65 →
  2.62 ms; race 1080p +0.07 ms and racing 1080p +0.13 ms "not
  significant").
* Integrated GPU (reference integrated GPU, SET=igpu, 3 + 3 runs: indicative only,
  docs/perf-gates.md): race 20.2 → 22.7 ms, racing 15.7 → 16.3 ms,
  explore / drive / racing Modern within noise
  (internal notes).

### Review (every player car, before / after)

`model_sheets` full sets, native (`bin/model_sheets-base`, the copy before
this work) and `--smooth`, in `<work dir>/assets-work/sheets/full/{base,smooth}`:
29 player cars × 11 classes (fb, rb, skirt, wide, hood, spoiler, roof,
mirror, exhaust, overview, rims) + the 15 traffic cars, 788 sheet pairs,
22 452 tiles (with `--smooth` each tile is aimed at the unsmoothed parts,
so the framing is the native sheets'). Every tile pair was scored by
script (changed pixels, car pixels turned background, background turned
car, and large colour changes inside both silhouettes away from edges —
what a buried lamp, a hole or a spike gives); the 150 highest
(internal evidence: assets-distance/sheet_review_top150.tsv) and every car's
overview (front 3/4 + wheel close-up, LOD A, all 29) were looked at by
eye, side by side. Found and fixed during the review:

| Car / part | Seen | Fix |
|---|---|---|
| CELICA trunk lamps (rear bumper sheets) | black boxes beside the plate | lamps / decals excluded (`celica_trunk_lamps_before_exclusion.jpg` → `_after_`) |
| SUPRA roof; G35 trunk | black spots / a black patch: an inner layer showing through a sunken panel | outward-only rule (`g35_supra_sunken_panels_before_outward_rule.jpg` → `supra_g35_after_outward_rule.jpg`) |
| AEROSTYLE roof scoops (all cars) | the slats' edges rounded away | `ROOF_STYLE07` excluded (`supra_aerostyle_excluded_hummer_exhaust.jpg`) |
| HUMMER SONIC exhaust | the tip's lower half closed | `EXHAUST` excluded; the tip still reads a little darker (the bumper's cut-out), **open**, minor |

What it gains: the tyres' and rims' outlines are round instead of
polygonal, wheel arches, mirror housings and roof lines lose their
facets in close-ups (`240sx_wheels_native_vs_smooth.jpg`,
`240sx_lexani_rim_native_vs_smooth.jpg`, `overview_*.jpg`). In the
chase view at 1080p the change is a few pixels on silhouettes. The
traffic sheets' part tiles that put the camera inside the body (traffic
cars take no parts) differ, but show nothing a game camera sees.

## 2. World Detail Extreme (Graphics → Performance, `enh.far_draw`)

A seventh tier of World Detail above Ultra (200 % + the look-ahead ring):
Ultra **plus** the sections the native visible lists would add from the
drivable outlines within `qol::FAR_RING` = **400 m** of the eye section's
outline centre (`World::far_sections_at`, built on Ultra's
`World::lookahead_sections` and `lookahead_admits`, so no detail section
is drawn over its region's panorama or another region's sections). Those
extra sections are drawn in the main view with their **lowest LOD** (the
placements' `low_solid`, the reflection LOD 0x5D672A), by the main view's
pipelines, size cull and fog: the fog curves and the horizon fog are
untouched (same `FrameUniform` fog values). The road reflection, rear
view, car cube and lamp shadows keep the native/Ultra lists. The set is
one per section (cached by the look-ahead), computed when a section is
entered.

LOD Fade: the far sections take part in the per-instance fades
(`WorldRenderer::update_fades`): a section handed from the far set to the
main list cross-fades its low-LOD groups out while its full groups fade
in; a far section dropped fades out (`FadeState::far_sections /
far_lingering`); new far instances fade in from nothing.

Measured (`devprobes/src/bin/far_draw_probe`, internal evidence: assets-distance/far_draw_probe.txt):
over the 255 drivable outlines of L4RA, Extreme adds 96 sections on
average to Ultra's set (max 221); a cold look-ahead costs 0.7–1.7 ms
once per section entered (Ultra's 0.03–0.09 ms).

What it shows: **little**. Bayview's native lists are generous and the
city occludes, so the extra sections land behind buildings or in the
fog. Frame dumps, Ultra vs Extreme, 1280×960: `--bench racing` 40–150
differing pixels per dumped frame, `--bench drive` 20–450, `--bench
explore` 0; `--shot explore` at the five outlines that gain the most
(probe poses): 0, 0, 0, 228 pixels, and 19 541 at one pose under an
elevated road (the extra low-LOD sections fill the distance there). It
is offered as the top tier for elevated / open views; it is not part of
any preset.

**No lent panoramas (2026-11-12).** The look-ahead
(`World::lookahead_sections`, both Keep All Loaded's 60 m ring and this
tier's 400 m ring) lends no region panorama (sections x90–x99,
`world::is_panorama`) from another outline's list: the set holds exactly
the panoramas the eye section's own native list names. A panorama is a
stand-in (a few flat faces with a low-resolution texture) built for the
outlines whose lists name it; drawn from elsewhere it stands in front of
the scene as a flat face whose texture reads as streaks. Reported
from a 4K test build:
* by the Marquis cinema (section 1404, car at about (−265, −845) heading
  −40°) the 400 m ring lent ten panoramas (198, 398, 399, 595, 596, 598,
  599, 899, 1098, 1499); region 5's 596 (high PAN_ENTERTAINMENT, low
  PAN_STADIUM_Z_E96; TRN_FREEWAYLOD_A_DM 256², TRN_TREES_FENCES_LOD_A_,
  TRN_TREELINEA_DM) drew as a wide dark band with horizontal streaks
  across the trees behind the street lamp — only `enh.far_draw 0` removed it;
* downtown (section 1608, car at (455, −836) heading 180°, the Carmencita
  street) the 60 m ring lent seven (198, 595, 596, 598, 599, 899, 1098)
  beside the list's own 799, and a flat streaked slab stood over the
  buildings — only `enh.keep_loaded 0` removed it.
Found by switching the reported options off one at a time and with
`devprobes/src/bin/far_pick` (the sets and the far placements a fan of
chase-camera rays hits). Original never showed either. Test
`world::tests::look_ahead_lends_no_panorama`.

Pops (`NFSU2_POP_STATS=1`, 600 frames): without LOD Fade the far
sections' low-LOD scenery adds size-cull pops (drive bench: 15 → 133
instances at once, racing 88 → 169); **with LOD Fade 0 pops** in both
benches, as with Ultra (logs `assets-work/far/pop-*.log`).

Cost (`perf.sh bench`, SET=quick, 5 + 5 runs, arms Ultra world detail vs
Extreme on the same binary, internal notes):
GPU median racing Ultra 1080p 3.97 → 4.12 ms (+3.9 %, the one flagged
change), racing Original 4K 2.68 → 2.78 ms (+3.7 %, p 0.095), explore and
race unchanged; main-thread CPU within noise (the set is cached per
section). Integrated GPU (3 + 3, indicative): racing 15.6 → 16.1 ms,
drive 15.4 → 16.3 ms, explore / race / racing Modern within noise
(`igpu-compare-x0-x1.md`).

Tests: `world::tests::far_sections_extend_the_ultra_set_per_section`,
`options::tests::restore_native_gives_back_raised_custom_values` (the
row's Extreme tier). Probe: `devprobes/src/bin/far_draw_probe.rs`.

## 3. Traffic Density (Enhancements, `enh.traffic_density`)

Native / More / Dense: free roam's traffic pool (0x4FE9F0: track record
+0xEC + 2·level + reversed, 16 at level 3, capped by 0x4FEA51) × 1, × 1.5,
× 2, at most 32 (`ai::traffic::dense_pool`). Everything else is the native
traffic code the clone ports: the placer (0x42F910 → 0x42F800 →
0x42E9F0, one car per 1 s pass, 250–500 m, despawn at 250 m), the jam
rule (0x4112A0), the model pass 0x412BE0 (its caps make the extra cars
repeat models), the TrafficMover, the lane driver. **Races keep the
native pool**: their traffic is per event (career event +0x36, Quick
Race's MOTrafficLevel 0–3, docs/formats.md "Traffic level"), the original
has no density scaling for them, and a bigger pool would change how hard
a race is; `Traffic::load` is called with density 0 for races. A graphics
preset keeps the value (gameplay). Applies when free roam loads.

Measured (`--bench explore 3600`, 60 s standing at the garage exit,
`NFSU2_BENCH_ENH="enh.traffic_density N"`, logs in
`<work dir>/assets-work/traffic/`): traffic cars on the road at
the end 10 / 15 / 19 (Native / More / Dense); simulation thread CPU 0.07 /
0.22 / 0.20 ms a frame, process CPU 1.84 / 2.93 / 2.84 ms a frame (one run
each; the update CPU is quantised and noisy at n = 1, docs/perf-gates.md:
read as "about +0.15 ms"). Races: `--bench racing 600` with density 0 and 2
gave the **identical** `NFSU2_STATE_HASH` file (1 194 steps) and 10 traffic
cars both: races stay as fair as native.

Frame cost in the A/B set (arm `enh.traffic_density 2`, SET=quick, 5 + 5,
`compare-new-t2.md`): no change flagged (the explore bench stands at the
garage exit; the racing configs use the race pool, unchanged).

Tests: `ai::traffic::tests::dense_pool_scales_free_roam_only_by_choice`,
`options::tests::traffic_density_row_steps_and_restores`.

## Parity (defaults native)

`perf.sh parity` with the copy before this work (`bin/game-base`) and
this work's binary (internal notes):
Original dumps of the five scenes at 1280×960 and 3840×2160 AE 0,
window path AE 0, Modern explore/racing and Ultra explore AE 0, the 15
shots AE 0 with no panic/validation, audio WAVs identical, state hashes
identical over firstrace (4 810 steps), firstdrag (16 417), postrace
(29 601), playthrough (82 300) and the parity scenario (1 681). One
frame of Ultra racing differed in one pixel by 1; rerun three times per
binary (`assets-work/rerun/`), base vs base, new vs new and base vs new
were all identical: not reproducible, so not this work's (the opt-ins are
off in that config).

## Open
* Smooth Car Shapes: HUMMER SONIC exhaust tip a little darker (above).
  LOD B and C are left as shipped (seen from afar). The traffic sheets
  were scored but only their overviews looked at by eye.
* World Detail Extreme: in the measured views it shows little (above);
  a view that gains clearly (an elevated road at speed) was not found
  among the five probed poses beyond the under-road one. The far set is
  per eye section, so it can lag the eye by one outline.
* Traffic Density: not compared with the original (it has no such
  setting); the placer's pace (one car a second) is native, so a Dense
  pool fills over ~30 s after a load. Pedestrian density (this work
  mention): not looked at in this work (**open**).
