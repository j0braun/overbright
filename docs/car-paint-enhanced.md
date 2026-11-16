# Enhanced Car Paint (opt-in clone deviation, 2026-11-04)

Graphics page row **Enhanced Car Paint** (`enh.enhanced_paint 1` in options.txt), **Off by default**.
Not native: the original has no such layer. It adds detail on top of car.efx (docs/car-shading-compare.md,
verified equal to the original in the car-paint comparison) without changing the grade: each paint keeps its hue and
brightness at normal viewing. Works with Enhanced Reflections on or off, MSAA/alpha-to-coverage, LOD Fade,
and does not touch the post chain (High Precision Post, is independent).

## Paint types (from the game's own paint data)
A paint part (slot 0x3F, brand attribute 0xEBB03E66 = GLOSS / METAL / PEARL, the finish UI_Paint
cycles, 0x55D0E0) names its light material in attribute 0x6BA02C05 (`car::Paint::from_part`). The draw's
light material (after CARSKIN → paint and the rim / brake swaps, 0x61797F / 0x62E05F) gives the type,
`render::car::PaintType::of` (verified against GLOBALB by the test below):

| Light material | Finish | Type | clearcoat x | flake y | pearl shift z | crisp env w |
|---|---|---|---|---|---|---|
| REGPAINT*, USER_RIMS_GLOSS, MAGSILVERGLOSS, USER_CALIPERS, TRAFFIC | GLOSS | Gloss | 1.0 | 0 | 0 | 1 |
| METPAINT*, MAGSILVER, MAGGUNMETAL, USER_RIMS_MET / _GUNMETAL / _MAGSILVER | METAL | Metallic | 0.8 | 1.0 | 0 | 1 |
| PEARL*, RIMPEARL* | PEARL | Pearl | 0.8 | 0.6 | 1.0 | 1 |
| CUSTOMPAINT_* (the PEARL finish's two-tone paints) | PEARL | Candy | 1.0 | 0.35 | 1.6 | 1 |
| CHROME, MAGCHROME, USER_RIMS_CHROME, USER_RIMS_SILVERGLOSS | – | Chrome | 0.35 | 0 | 0 | 1 |
| everything else (glass, plastics, rubber, lights, decals, MATTEPLASTIC…) | – | Native | 0 | 0 | 0 | 0 |

NFSU2 has no separate "candy" or "matte" body finish: matte materials (MATTEPLASTIC, DULLPLASTIC) stay
native; "candy" is the name used here for CUSTOMPAINT_* (inferred from their diffuse/specular colours
differing in hue, e.g. CUSTOMPAINT_7 purple diffuse / blue specular), not a game term.

## What the layer does (src/render/car_paint.wgsl)
Built from car.efx's own terms (the harmonic HarmonicCoeff lighting, Specular/Envmap Min/Range):
- **Crisp reflection**: the env cube is looked up along the per-pixel reflection direction (the native
  path interpolates the vertex shader's), same cube and convention, so the mean is unchanged.
- **Clearcoat highlight**: a sharp lobe (power 96) towards the harmonic's dominant light (its linear
  terms' specular channel, h[7..9].w), Fresnel-weighted (Schlick, F0 0.04), scaled by the paint's gloss
  (the skin alpha). The lobe is widened by the normal's spread in the pixel (Toksvig-style, `fwidth`) and
  dimmed in proportion (normalised), so distant cars do not shimmer or gain a sheen. No second env layer:
  car.efx's Fresnel env term already is the coat's reflection (a first version that added one brightened
  grazing panels by up to 12 levels and was dropped).
- **Metallic flake**: per cell (4 mm, car space, so it sticks to the body) the *filtered* glint of the
  sub-pixel flakes (the mean of a power-20 lobe over normals tilted ±0.35 per axis, 40 % of cells:
  0.072·(r·l)³) varied per cell by ±40 %, plus a ±1 % zero-mean grain. Mip-like: the cell grows with the
  pixel footprint in power-of-two octaves, two blended (no popping), never below ~1.5 px (no shimmer),
  weaker with each octave and gone by the sixth. (Until 2026-11-04 each cell glinted with one tilted
  normal, on/off at 40 %, with ±3 % grain: isolated bright cells that read as raindrops, much worse
  with Sharpen 100. See docs/car-raindrops.md.)
- **Pearl / candy shift**: at grazing angles ((1 − n·v)³) the colour moves towards the material's
  specular tint at a constant luminance (Rec.601 weights), so the brightness is unchanged.

## Native stays untouched
Two shader modules: the native pipelines use `car.wgsl` alone (only the struct's padding is named
differently); the paint pipelines use `car.wgsl` with its final combine (`r0 = 2*r0*r1.a + r1`) routed
through `paint_layer` + `car_paint.wgsl`, built on first use (`CarShared::module`). Pipelines are keyed by
(format, SceneMode, paint). The per-draw `paint` vec4 (bytes 368..384 of the 512-byte entry, formerly
padding) is zero when the option is off.

## Evidence (the maintainer's private captures; each image: Off | On)
Shots 1280x960 (`NFSU2_SHOT_ENH="enh.enhanced_paint 0|1"`): `garage.jpg` (`--shot main`, stock 206),
`dday350z.jpg` (main_dday, Rachel's 350Z on the turntable), `intro350z.jpg` (Rachel's 350Z in free roam
at night, `--shot intro`), `pearl.jpg` / `candy.jpg` / `metal.jpg` (`main` + `NFSU2_SHOT_FIT=0x3F:123`
PEARL3, `0x3F:136` CUSTOMPAINT_7, `0x3F:104` METPAINTSILVER), `metal-flake-3x.jpg` (hood, 3x),
`rims.jpg` (`NFSU2_SHOT_FIT=0x1D:10,0x1E:10,0x44:152`: aftermarket rims with the USER_RIMS_CHROME rim
paint; that the rim paint applies there is **inferred**, not checked per draw), `qrescalade.jpg`
(`NFSU2_SHOT_QRCAR=ESCALADE`, CHROME trim, white metallic). The main-menu shots are not deterministic run
to run (the turntable), so Off and On differ slightly in pose there; the qrcars and intro shots are.

**Grade held** (`region-means.txt`, the car-paint comparison's matched-frame rectangles, On − Off, 0–255): car crops
+0.1/+0.1/+0.1 (240SX, 206, Rachel's 350Z main_dday), +0.2/+0.1/+0.2 (stock 350Z), +0.1/+0.1/+0.5
(Rachel's 350Z intro); the largest local change is the intro trunk's highlight, +1.3/+0.9/+4.5 (the coat
highlight and flake sparkle under the street light). Same with Enhanced Reflections on (car crops ≤ 0.5).

**Native byte-identical**: with the option off, `qrcars` 240SX / 350Z / PEUGOT / ESCALADE and `intro`
are pixel-identical to a build of the base (0 differing pixels). A first version that put the
layer behind a runtime branch in one module changed one pixel by 2 levels on the 350Z, hence the two
modules.

**GPU cost** (reference desktop GPU, `--bench explore|race 300`, `NFSU2_BENCH_SIZE`, three runs each, GPU busy
median of the last run, the "cars" pass): 1080p explore 0.60 → 0.57 ms (cars 0.03 both), race 0.78 →
0.78 (cars 0.11–0.12 → 0.12–0.13); 4K explore 2.08 → 2.07 (cars 0.05–0.06 → 0.06), race 2.94 → 2.92
(cars 0.38–0.41 → 0.40–0.43). Within noise; at most ~0.02 ms on the car pass at 4K. Cars fill little of
those frames; the cost scales with car pixels (a few ALU ops and one cube fetch per paint pixel, two
hashes for flake), so a full-screen garage car costs more but still well under 0.1 ms on this GPU
(**inferred**, not timed: the front end has no bench). Laptop-class GPUs: not measured.

## Tests
- `qol::tests::enhanced_paint_round_trip_and_native_default` (options.txt round trip, default off),
  `enhancements_round_trip` (25 deviations), `frontend::options::tests::graphics_page_steps_and_restores`
  (the row, its label, step and Restore Native).
- `render::car::paint_tests::draw_uniform_packing_matches_car_wgsl` (512-byte entry, `paint` at 368, the
  WGSL struct's field order), `paint_module_only_replaces_the_final_combine`,
  `paint_types_by_light_material`, `every_paint_part_has_its_finish_type` (GLOBALB: every slot-0x3F paint
  of the 350Z by finish; skips without NFSU2_ROOT).

## Open
- The look is a judgement call for the player (strengths are the table above and three constants in
  car_paint.wgsl: COAT_POWER, FLAKE_CELL, FLAKE_POWER).
- The front end's floor reflection of the car (`render_reflection`) and `CarRenderer::render` (shop
  scenes) keep the native pipelines; only the main car passes (`queue`, `render_sampled`) get the layer.
- A stock car with MAGCHROME rims was not found among the 19 cars checked (`car_inspect`); the chrome
  shots use aftermarket rims with the USER_RIMS_CHROME rim paint and the Escalade's CHROME trim.
- Nothing here is traced in SPEED2.EXE (none exists there); the paint-type mapping is from the game's
  data (GLOBALB) only.
