# Fix Original Model Errors (2026-11-04)

An opt-in Enhancements row, **Fix Original Model Errors** (Off / On, `enh.fix_models`,
default Off). Off leaves every model exactly as shipped: each entry point in
`src/model_fixes.rs` returns at once. On applies small corrections, defined in code
written here from evidence in the player's own files, when a car is loaded (cars loaded
before the switch keep their geometry until reloaded). No mod file was downloaded,
read or copied; the public description of a community "small model fix" (
2721) was used only as a list of symptoms to check.

Method: the maintainer's probe `model_scan` scans every model file (81: each car's GEOMETRY.BIN
with all kits, bumpers, skirts, spoilers, hoods, scoops, mirrors, exhausts, the shared
part folders and every `CARS\WHEELS\GEOMETRY_*.BIN`); the maintainer's probe `model_fix_shot`
renders clone close-ups (native / `--fix`, `--paint r,g,b`); the original was checked
in SPEED2.EXE's main-menu Customize under Wine (`fastgpu.sh`, DXVK, copy
`$OB_PRIVATE/wine/runs/<run>`, display :200, profile `career_end_won.sav` so every
part is unlocked, WidescreenFix off). Raw captures: `<work dir>/models-work/{wine,shots}`.

## Fixes (confirmed in the original and in the clone)

| Car | Part | Symptom | Data (original) | Correction | Evidence |
|---|---|---|---|---|---|
| IS300 | FRENZY rear bumper (`IS300_KIT11_REAR_BUMPER_A`) | two exhaust tips drawn into each other | markers LEFT_EXHAUST ×2 (−2.220, −0.504, 0.035) / (−2.231, −0.503, 0.010), REAR_LICENSE_PLATE ×2 at one spot; the exhaust walk 0x639843 files every EXHAUST-type marker | keep each marker's first record (`model_fixes::markers` on the walk; `Solid::markers` too) | internal evidence, `is300_frenzy_clone_orig_vs_fixed.png` |
| GOLF | SNYPER front bumper (`GOLF_KIT01_FRONT_BUMPER_A/B/C`) | lower grille slats in the body colour | the slats are MOLDINGS on the skin `GOLF_SKIN1` at UV (0.043, 0.854), a painted texel (the scanner's `skinpaint` check: the composite skin changes with the paint there) | those corners → (0.795, 0.719), the black trim box the Golf's other skin-mapped MOLDINGS (KIT03 bumper) sample | `golf_snyper_wine.png` (light slats on the silver car), `golf_snyper_clone_orig_vs_fixed.png` (red paint) |
| MUSTANGGT | front wheels | tyre runs into the front of the arch, gap behind | car type (GLOBALB 0x34600 +0x120) front x 1.445; the arch circle fitted to the stock body / bumper / skirt has its centre at x 1.376 (dx −0.069); rear dx +0.021 | drawn wheels, brakes, spinners and their pivots −0.069 m in x (`Car::wheel_shift`); physics keeps the table | `mustanggt_front_wheel_wine.png`, `mus_fw_clone_orig_vs_fixed.png` |
| A3 | front wheels | same | front x 1.170, arch centre 1.128 (dx −0.042); rear dx +0.001 | −0.042 m | `a3_front_wheel_wine.png`, `a3_fw_clone_orig_vs_fixed.png` |
| PEUGOT106 | stock roof antenna with any roof scoop | the antenna runs through the scoop | three crossed DECAL quads (`PEUGOT106_BADGING`) in `PEUGOT106_BASE_A/B/C` at x −0.02…0.29, y ±0.01, z 1.16…1.33; every scoop sits at the ROOF_SCOOP marker (−0.130, 0, 1.175) | with a roof scoop (slot 7) above level 0 fitted, those six triangles per LOD are drawn degenerate (`model_fixes::hidden_triangles`, `CarPart::hidden_tris`) | the maintainer's private capture |

The arch fit (`model_scan --check wheel`, kind `arch`) is a measurement, not a traced
value: per 10° bin of the upper half the nearest body vertex beyond the tyre within the
outer 15 cm of the side, a circle fitted to them. Other cars with |dx| ≥ 0.035 at the
front were checked by eye in the visual model review (below): IMPREZAWRX, PEUGOT106, PEUGOT and NAVIGATOR
are fixed; the 3000GT is not.

## Checked, not fixed

| Car | Part | Symptom (list) | Result |
|---|---|---|---|
| RSX | Snyper radiator, Ace mapping, Vapor normals | — | **not applicable**: the player's install has no `CARS\RSX` (nor CIVIC / S2000 geometry); its part records point at hashes no file holds |
| SENTRA | stock spoiler mapping | — | not confirmed: every body triangle samples one skin texel (0.977, 0.998), as the aftermarket spoilers do; looks body-coloured in the clone |
| SENTRA | MANTIS (KITW02) missing grilles | upper opening renders black | the opening has no grille geometry; the original shows the same; a fix would add geometry (outside "small corrections") |
| SENTRA | APOCALYPSE (KITW04) normals | — | not confirmed: winding agrees on every triangle; 5.8 % of its area has a corner normal > 70° off the face, in the range of the other wide bodies (KITW02 5.6 %, KITW03 8.0 %) |
| SENTRA | exhaust markers in bumpers | — | not confirmed: one RIGHT_EXHAUST per rear bumper; no outlet buried in its part at LOD A (`exhaust` check); contact sheet of all 26 rear bumpers in the work dir |
| GOLF | badge gaps in hoods | — | not found in a contact sheet of all 29 hoods (front 3/4 view); **open** |
| WHEELS | MEISTER S2R (`WORK_STYLE02`) mapping | lettering on the lip | not an error: the lip's UVs (v from 0.246) reach the "MEISTER S2R" lettering drawn into the face area of `WORK_STYLE02_WHEEL` (rows 34–38 of 128); EQUIP 02 (`WORK_STYLE01`) uses the same layout. The original shows the same fleck on the lip: native |

## Scanner findings (top, internal evidence)
- `marker`: the only duplicated marker at LOD A is the IS300 FRENZY one (fixed); LOD B
  files repeat their marker lists, which no draw reads (the walk takes LOD A).
- `exhaust`: 6 buried outlets, all at LOD B (MIATA KIT12, FOCUS KIT16 / KIT25 / KIT09,
  IMPREZAWRX KIT14, GOLF KIT09); LOD B is seen only from afar; not fixed.
- `skinpaint`: 682 trim pieces on painted texels; most are systematic (every car's
  STYLE13 / STYLE20 hood vents, door sills) and read as intended body-coloured trim;
  the GOLF SNYPER slats rank first and are the one confirmed.
- `uv`: COROLLA STYLE06–14 BRAKELIGHT_B UVs span ±16 000 (LOD B only); not fixed.
- `winding` / `shade` / `normals`: no part inverted; spinner and rim hubs carry the
  most off-face normals (hard edges smoothed), as designed.

## Visual review

Contact sheets: the maintainer's probe `model_sheets` (`model_sheets OUTDIR [--cars A,B] [--classes
fb,rb,skirt,wide,hood,spoiler,roof,mirror,exhaust,overview,rims] [--traffic] [--rimref CAR]
[--fix] [--smooth] [--lod N]`; `--smooth`: Smooth Car Shapes, docs/assets-distance.md). It renders 1920×1080 sheets, one or more pages per car per part class.
Each part gets two tiles: its class view in the stock paint, and a second view in a dark paint,
using the part's carbon twin where it has one. Each tile is aimed at the fitted part's own LOD A
geometry. The overview shows the five fixed angles (front 3/4, rear 3/4, side, low front, wheel
close-ups) in stock and dark paint, at LOD A and LOD B. Rims: every style the car takes. A `.tsv`
next to each sheet lists its tiles. The full set covers 29 player cars and 15 traffic cars: 680
sheets with 17 963 tiles, a few minutes on the reference desktop GPU. It lives in
`<work dir>/models3-work/sheets`. Downscaled samples are in
the maintainer's private captures (every car's front-bumper and overview sheet, the G35 roofs,
and the first rim reference sheet).

How much was reviewed by eye (`evidence/model-review/review_log_lead.md`, `review_forks/`,
checklist in the work dir):
- 405 sheets in full by five review forks. They covered every sheet of 18 cars: 240SX, CORSA,
  ECLIPSE, ESCALADE, FOCUS, G35, GTO, HUMMER, IMPREZAWRX, IS300, LANCEREVO8, MUSTANGGT,
  NAVIGATOR, PEUGOT, PEUGOT106, SENTRA, SKYLINE and SUPRA (240SX partly).
- 88 sheets by the maintainer:
  - 3000GT and 350Z: everything, the classes past the bumpers only as montages;
  - A3, CELICA and COROLLA: bumpers and skirts;
  - GOLF, MIATA, RX7, RX8, TIBURON and TT: page 1 of the front and rear bumpers;
  - all 15 traffic cars.
- 187 sheets were rendered but not looked at. They are the remaining hood, spoiler, roof, mirror,
  exhaust, overview and rim sheets, and the bumper page 2 and skirt sheets, of A3, CELICA,
  COROLLA, GOLF, MIATA, RX7, RX8, TIBURON and TT, plus the 350Z rim and rim-reference sheets.
  The rim models are shared, and 18 cars' rim sheets were reviewed.

Wine checks were done in the original's main-menu Customize → Car Shop → Body Shop. Rims
(after Exhaust Tips) gives a side view. Hoods, Roof Scoops and Carbon Fiber were also used. The
copy is `$OB_PRIVATE/wine/runs/<run>` on display :208 with the `career_end_won` profile. Raw
shots are in `<work dir>/models3-work/wine`.

| Car | Part | Symptom | Original y/n | Fixed y/n | Crop |
|---|---|---|---|---|---|
| IMPREZAWRX | front wheels | tyre touches the arch's front edge, gap behind (arch fit dx −0.044) | y | y (Fix row, −0.044 m) | `imprezawrx_front_wheel_wine_vs_clone.jpg`, `wheel_fix_native_vs_fixed.jpg` |
| PEUGOT106 | front wheels | same (dx −0.038) | y | y (−0.038 m) | `peugot106_front_wheel_wine_vs_clone.jpg` |
| PEUGOT | front wheels | same (dx −0.036) | y | y (−0.036 m) | `peugot_front_wheel_wine_vs_clone.jpg` |
| NAVIGATOR | front wheels | same (dx −0.056); the rear is centred | y | y (−0.056 m) | `navigator_wheels_wine.jpg` |
| 3000GT | front wheels | the scanner lead (dx −0.046) comes from a poor circle fit (dz −0.165) | n: centred in the original | n | `3000gt_wheels_wine_vs_clone.jpg` |
| G35, 240SX | roof (slot 8 TOP) | slivers and cracks over the sunroof; the sunroof glass is hidden | n: **clone bug**; the original shows the sunroof, and with a scoop the closed roof | y (native path, see below) | `g35_roof_wine_noscoop_vs_scoop.jpg`, `g35_roof_clone_before_after.jpg` |
| all cars | CF parts (`_CF`, Carbon Fiber hoods, roof scoops, spoilers) | the carbon weave renders light grey, near the body's brightness | n: the original's carbon is dark | y (R8: carbon parts take the `CARBONFIBER` light material, docs/car-materials.md) | `g35_carbon_hood_wine_vs_clone.jpg`, `../car-materials/` |
| all cars | CHROME / carbon trim: FUSION hood (STYLE14) chevrons, RAGE (KIT14) intake blade, SLINGBLADE (KIT28) fins, SWEEPER (KIT29) / ROADRUNNER (KIT21) lips | flat white under the showroom envmap | n, and n in the clone: in the same Body Shop frame the clone matches (docs/car-materials.md); the white came from the sheet tool's env cube | n (nothing to fix) | `g35_fusion_hood_wine_vs_clone.jpg`, `../car-materials/g35_fusion_hood_orig_clone.jpg` |
| SKYLINE | SQUIRE 9 (KIT22) rear bumper | empty right exhaust cut-out | not compared (authoring) | n | `review_forks/SKYLINE.md` |
| GOLF | hoods | badge gaps (this work lead) | not found in 29 hood close-ups | n | `review_forks` closeups |
| WHEELS | MEISTER S2R lettering | red fleck of the lettering on the lip | y: the same fleck on the Sentra in the original's Rim Styles | n (native) | `mf3b/sentra_meister_s2r_wine.jpg` |
| SENTRA | MANTIS (KITW02) grille (re-check) | upper opening black, no mesh | y: the same in the original | n (missing geometry) | `mf3b/sentra_mantis_wine_vs_clone.jpg` |
| PEUGOT106 | stock antenna (re-check) | through every scoop | y (JETSTREAM) | y (Fix row, above) | `mf3b/peugot106_…jpg` |
| 3000GT | SQUIRE 9 plate (re-check) | plate over the bars | y: the same in the original | n (authoring) | `mf3b/3000gt_squire9_plate_wine.jpg` |
| GOLF | TWISTER (STYLE07) hood | dashed crack along a seam mid-hood | y: faint in the original too | n (T-junction; a fix needs new triangles) | `mf3b/golf_twister_wine_vs_clone.jpg` |
| MIATA, RX7 | VELOCITY (STYLE23) hood | the cowl's inner face light (SKIN1 + CLEARPLASTIC / MOLDINGS ranges; black DULLPLASTIC on other cars) | n/a: the split hoods (part kind 5: COPPERHEAD, COBRA, VELOCITY, GATO, TRAP) are never listed; the original's Hoods and Carbon Fiber Hoods lists end at FACTOR X, as the clone's `shop_parts` (0x5449D0) | n (unreachable) | `mf3b/miata_hood_list_ends_factorx_wine.jpg` |
| TIBURON | stock wheel | thin dark line at the front edge of the right wheels | n: not in the original's side view | n, **open**: the clone samples the inner cap quad (PLAINNOTHING, u 0.752…1.0 on the alpha disc of `TIBURON_TIRE`) with WRAP, so bilinear at u = 1.0 pulls in the opaque column 0 past the alpha test; a clamped car sampler (experiment) removes the line. car.efx sets no ADDRESSU/V (its sampler records hold only the filter states; world.efx sets 165/166 = WRAP), so the original keeps the stage's last address mode: not traced. **L2 (2026-11-05)**: traced in an apitrace of the original (Quick Race car select, the Hyundai Coupe): every car draw sets sampler 1 to WRAP (the wheel's alpha-tested texture too, ALPHAREF 95), so the clone's WRAP is native and the clamp is not a fix. **Leftover**: the original draws the line too (its pixels at the predicted spot of the car-select frame, on near-black ground; internal notes): native, nothing to fix | `mf3b/tiburon_wheel_line_wine_vs_clone_wrap_clamp.jpg` |

The roof is a clone bug, not a model error. The generic part draw's slot switch (0x623136) sends
slot 8 to 0x62315B. There, for the car types that 0x60C860 accepts (240SX, CIVIC, RSX, G35), the
draw keeps group 1 (`FULLROOF`) when the roof scoop (ride +0x5AC) is above level 0, and group 0
(`ROOF`) otherwise. Every other car never draws group 1. The clone drew both groups. The fix is
`car::top_group_drawn`, in the native path (not the Fix row), with the test
`car::shared_part_tests::top_draws_one_roof_group`.

### 2026-11-04: the rest of the sheets
Re-rendered from the tree with the carbon fix (internal sheets, 207 sheets for
A3, CELICA, COROLLA, GOLF, MIATA, RX7, RX8, TIBURON, TT plus `--rimref 350Z` and the 350Z rims). The
rim sheets now come in 3 pages and the rim reference in 6 (same tiles as the visual model review's 4 and 7). Reviewed by
eye, sheet by sheet, with a checklist (internal evidence, log `review_log.md`):
**180 sheets reviewed, 0 left** = the visual model review's 187 unreviewed sheets (177 under the new paging) + the 350Z's
3 rim sheets. Every car's whole sheet set has now been looked at. Findings are in the table above
(rows marked the model review sheets). Seen and judged as design or known: the CF scoops' light silver (CARBONFIBRE2 →
DEFAULT in the original too, docs/car-materials.md), the chrome family (FUSION, VALHALLA grille,
ARMAGEDDON lip), AIRSTREAM (ROOF_STYLE11) overhanging sloped roofs and WINDTUNNEL's nose on the
Miata hardtop (shared scoops), pop-up lamps raised on the Corolla, the stock rim drawn W23 next to
W26 aftermarket rims (the sheet's width choice).

Not fixed and not checked by eye in the visual model review:
- the LOD-B-only items (buried exhaust outlets, COROLLA BRAKELIGHT_B UVs; the overview sheets
  show LOD B, with no gross errors);
- the sheet tool's dark tiles on traffic cars (their skins take no body paint).
