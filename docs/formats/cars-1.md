# Cars, handling, collision, cameras (1)

Part of [docs/formats.md](../formats.md), the traced behaviour of `SPEED2.EXE`; the tags
(**verified**, **inferred**, **open**) are explained there.

## Cars (in progress)

* `CARS/<CAR>/GEOMETRY.BIN`: one solid list. Car meshes have flag 0x80 (car
  effect) and mostly 0x100, with 36-byte vertices (pos, normal, colour,
  uv; normals checked to be unit length). The stride follows 0x80, not
  0x100 (**verified** over every `CARS\*\GEOMETRY.BIN`: all 18 727 meshes
  with 0x80 have 36-byte vertices, among them 346 without 0x100 — the
  HOOD_UNDER / TRUNK_UNDER models and `PLATES`' LICENSE_PLATE_STYLE01_A,
  flags 0x4080, which the clone used to read as 24-byte and so dropped). Each solid has light-material hashes
  (0x134013), indexed by material +0x20 (0xFF = default), and 0x50-byte
  markers (0x13401A: hash, pad, 4×4 matrix).
* The name field holds 27 characters. Longer names are recovered by matching
  the stored hash (`complete_truncated_name`).
* The car-type table is GLOBALB 0x34600 (stride 0x890, 46 cars). Wheel i is
  at +0x120 + 0x30·i: position x, y, z, then radius (0.314 for the 240SX).
  Engine data follows (idle/redline/max rpm 800/6500/7000).
* Light materials are GLOBALB 0x135200 records (0xA0 bytes, name at +0x14,
  floats at +0x30). 0x5BD4B0 turns them into car.efx constants: DiffuseMin =
  rgb(+0x34)·f(+0x30) with a = +0x50; DiffuseRange = rgb(+0x44)·f(+0x40) −
  min; SpecularMin/Range from +0x5C/+0x60 and +0x6C/+0x70; EnvmapMin/Range
  from +0x80/+0x84 and +0x90/+0x94, each ×0.5. The env-map terms are zeroed
  unless mesh flag 0x100 is set. SpecularPower = global·f(+0x58),
  EnvmapPower = f(+0x7C). The effect handle names were recovered from the
  registration function 0x5D0F90. (Shader path, verified at 0x5BD999…
  0x5BDAA6: the vectors go to handles +0x164 DiffuseMin, +0x18C
  DiffuseRange, +0x7CC SpecularMin, +0x81C SpecularRange, +0x2A4
  EnvmapMin, +0x27C EnvmapRange, floats +0x7F4 SpecularPower, +0x2CC
  EnvmapPower. The env terms are also zeroed when
  g_CarEnvironmentMapEnable 0x870CB8 is 0; the clone assumes it on.)
  **The flags are the draw's** (verified 2026-11-04): the mesh material
  record's +0x38 (0x5C5BB1 → 0x5CEB40 entry +0x14 → 0x5CF559), not the mesh
  header's +0x0C; e.g. the 240SX's BASE_A / TRUNK_A DULLPLASTIC ranges carry
  0x4080 in 0xC180 meshes and draw without the env term. `solid::Material::flags`;
  see docs/car-shading-compare.md (2026-11-04 re-comparison).
* **Light-material loader** (verified; 0x48E070, chunk 0x135200, record =
  chunk body): +0x80 and +0x90 are multiplied by 0x78D780 (1.0), and for
  records whose +0x10 is 3 (all of GLOBALB's) 0x5BB5C0 overwrites the
  diffuse alpha +0x50 / +0x54 (DiffuseMin.w / Min.w + Range.w, the draw's
  alpha) by the name hash at +0x0C: WINDOWMASK, WINDSHIELD, MOLDINGS,
  WINDSHIELD_TINT_L1_BLACK / _L1_GREEN → 0.3 / 1.0 (0x7FF060/70,
  0x7FF064/74); WINDSHIELD_TINT_L2_BLACK / BLUE / GREEN / RED → 0.4 / 1.0
  (0x7FF068/78); the twelve WINDSHIELD_TINT_L3_* → 0.5 / 1.0
  (0x7FF06C/7C); CLEARPLASTIC, HEADLIGHTGLASS → 0.4 / 0.8 (0x7FF080/84).
  Others keep the file's values (WINDSHIELD's file values, 0.697 / 0.082,
  would make the glass nearly clear face-on). The record with hash
  0x2C420D64 (DEFAULT) becomes the default material 0x7F399C.
  `parse_light_materials` applies this.
* car.efx shaders (verified by disassembling IDI_CAR_FX's vs_1_1 / ps_1_1
  blobs at resource offsets 0x3604 (vs) / 0x3448 (ps)): car.wgsl matches them
  instruction for instruction. The glass layers are WINDOWMASK and
  WINDSHIELD (or the tint's material) draws of the same WINDOW texture
  (RGB ≤ 50, alpha mostly 255), blended SRCALPHA / INVSRCALPHA; their
  brightness comes from the env term (2·env·EnvmapMin…Range doubled again
  by the alpha), i.e. from the cube. The front end's static cube is what
  the garage glass reflects, at the strength the clone gives it (checked
  by apitrace, below). Rachel's preset fits WINDSHIELD_TINT_L1_MBLACK
  (not on 0x5BB5C0's list: alpha 0.959 / 0.704; the trace shows that
  material's constants on the main-menu 350Z's glass).
* **Car shading checked against the running original** (verified,
  2026-10-29; apitrace 14 over DXVK of SPEED2.EXE under Wine, private
  setup $OB_PRIVATE/wine/runs/<run>, retraced with d3dretrace
  `--snapshot`; frames and side-by-sides in
  $OB_PRIVATE/wine/runs/<run>/compare). car.efx's technique "car" (the
  effect also holds "car_fixed_envmap" and "car_fixed", unused at max
  detail) is drawn with all 24 vertex constants in one
  SetVertexShaderConstantF and EnvmapMin/Range as ps c0/c1. On the Quick
  Race car select (Peugeot 206, stock paint) the clone's per-draw
  HarmonicCoeff, LocalEyePos, Diffuse/Specular/Envmap Min/Range and the
  two powers equal the trace's to 4 digits for every material (paint
  0.662/0.655/0.672/0.993, SpecularPower 5.72, EnvmapPower 0.262;
  MAGSILVER rims; WINDOWMASK / WINDSHIELD glass), and the FE shadow grid
  (fe.efx, 3 strips of 8 vertices, colour 0xFE808080, z 0.152) matches
  `shadow::car_shadow` to 2 cm. In game (airport start) the six faces of
  the dynamic cube 0x870858 (snapshotted as they are rendered, order
  +Z, +X, −Z, −X, +Y, −Y) match the clone's faces, −Y (up) black but for
  a flare in both. The image of the scene **before the screen passes**
  (main target 0xCB7B08 just before the first post-pass render target
  switch) matches the clone's car and turntable floor to a few levels
  (car side 25/32/39 vs 27/34/41). So the saturated blue paint, the
  violet-white glass and hood, the bright violet rims and the dark floor
  of the final original frame are made by the screen passes (screen
  filter / tint, overbright, glow), not by car.efx; the remaining car
  differences belong to `render/postfx`. Also seen: in the front end each
  car is drawn twice per frame, first into the floor-reflection target
  (render target 0x5D79FF0, WorldView row y = +up: the car mirrored),
  then into the main target.
* **Re-compared 2026-10-30** against the original's frames in
  $OB_PRIVATE/wine/compare/orig (main.png: Rachel's 350Z on the
  turntable; qrcars.png; intro.png): the current clone (`--shot
  main_dday`, `qrcars`, `intro`) now matches them — saturated paint, the
  violet-white glass and hood, the bright violet-white rim and tyre edges
  (the screen passes, see Screen filters), vinyl, carbon hood, wide body,
  spoiler, rims; car-crop means within 1 level (54.4/53.1/47.9 vs
  54.2/53.2/46.9). The "matte grey" in DIFFERENCES.md was the clone of
  2026-10-29 02:57, before the postfx rounds. The rear plate draws
  GLOBALB's LICENSE_PLATE ("UNDRGND 2") in `intro` and `explore` as in the
  original's intro frame (the empty plate came from the 24-byte reading
  of LICENSE_PLATE_STYLE01_A's 0x4080 meshes, fixed earlier). 0x631E30
  also hashes `<car>_LICENSE_PLATE` into its +0x28C (render info +0x2CC),
  which no draw reads (**verified** by the offset's readers). The
  drilled brake disc behind the 350Z's front rims was darker than the
  original's (**fixed** 2026-11-01, by an apitrace of the main
  menu, see "Wheel draw" → "The showroom"): the material, texture
  (BRAKEDISC 128² DXT1, identical texels) and normals matched; the
  showroom's brakes take the in-game placement and the camber turn, whose
  light context (HarmonicCoeff turned into the brake's axes) lit the
  disc's face 15 % more and from a lower eye. Its models are BRAKE_STYLE02's: texture BRAKE_GLOBAL
  (0x17F9F794) with CALIPERDECAL / BRAKEDISC, and DDA11455 with CALIPER
  (0xD6640DFF, no light-material record, so DEFAULT through 0x48F6A0's
  fallback 0x7F399C, as in the clone; remapped to render info +0x398
  only when slot 0x46 holds a paint, which the preset does not). The
  brake light contexts (0x62EB26, 0x62F287, 0x62FA4E, 0x6304A8) are not
  checked against a trace. Render info +0x1768… is a second texture remap
  table: BRAKE_GLOBAL → BRAKE_GLOBAL_LEFT (0x85E9C79E) and others
  (0x638672…0x6386DB; its use is not traced).
* **Paint and reflections compared frame by frame** (2026-11-01;
  the original at full speed on DXVK (fastgpu), the finished-career save's finished-career
  save in both games; images in $OB_PRIVATE/research/paint-compare,
  orig | clone | |diff|×4): the career garage (Evo, red paint, carbon
  trunk, aftermarket rims and brakes), Customize → Paint on the RX-7 with
  Gloss, Metallic and Pearlescent swatches 0 and 4 (the base paint has
  only these three categories, `]` cycles them; no matte), free roam at
  the garage exit at clock 23 s (sunrise dome) and 103 s (night dome),
  and the Quick Race start (Peugeot 206). The car crops agree to 0.3–2
  levels in the channel means with identical luminance percentiles; the
  diffs are edges (sub-pixel pose) and the floor, not the paint, the
  env-map reflections, the Fresnel/gloss weighting or the clamps. The one
  shading difference was the exhaust tip's paint (part paints, below).
  Seen but not car shading: the original's Paint screen keeps the cursor
  on the current paint. The clone's key path already did (Enter on a slot,
  0x55CC60 → 0x548640: finish from the slot's setup item or fitted part,
  then that part selected; finished-career: Metallic, swatch 12 of 60, the red); only
  the `paint` / `paint_base` / `paint_engine` shots forced Gloss and swatch
  0. They now enter the slot with Enter (`shot_paint_slot`), falling back
  to swatch 0 for a slot the car cannot paint. **Open**: U2_Paint_HighLight
  (0x9BA6D658) does not show in the clone's shots on either swatch; the
  original's look of the cursor (0x55CC60 runs the swatch's own
  "Highlight") was not compared.
* **Garage exit, free roam** (2026-11-01, `--shot crib_exit`: Yes on
  "Return to Explore Mode?", the drive-out, NFSU2_SHOT_MS of free roam with
  the particles stepped; with NFSU2_SHOT_SAVE the profile's garage).
  Against an apitrace of the original (the finished-career save's save, frame 10800): the
  headlight pool and the marker's ground smoke now match (see the headlight
  pass and **Ring** under Emitter draws); frames in
  $OB_PRIVATE/research/garexit-compare. The top-left HUD shows in both
  the garage's engage icon (car between two arrows, pulsing) over the SMS
  phone.
* **Per-part paint materials** (verified; render info setup 0x638DE8…
  0x6390A1): the car-config part at +0x590 + 4·slot names its light
  material by attribute 0x6BA02C05 (0x61F710, looked up by 0x48F6A0,
  default 0x7F399C when missing), kept only when a gating part is above
  level 0 (part byte +5 >> 5 ≥ 1): +0x390 = slot 0x49 gated by the front
  rim 0x1D; +0x394 = 0x47 by the exhaust 0x10; +0x398 = 0x46 by the brakes
  0x1B; +0x3A0 = 0x45 by the spoiler 0x0C; +0x3A4 = 0x4B by the roof scoop
  0x07; +0x3A8 = 0x4C by the mirrors 0x20; +0x39C = 0x48 when slot 0x22 is
  level ≥ 3, else +0x384. The rim draw (0x61792D…0x6179FE) replaces, in
  both wheel models (render info +0x1B48 / +0x1B68), light materials
  0x22719FA9 (MAGSILVER), 0xFD102A92 (MAGCHROME) and 0x12049251 with
  +0x390 when non-zero (0x48D860); the mirror draw (aftermarket mirrors,
  slot 0x20 above level 0: 0x6269D6 group 0, 0x626F4B group 1) replaces
  0x6BFA56DA (the aftermarket mirror models' paint reference, e.g.
  MIRROR_POST_LEFT_STYLE03_A's body mesh) with +0x3A8, or the body paint
  +0x384 when +0x3A8 is 0 (0x622F4B…0x622F5C; likewise +0x3A0 and +0x3A4
  fall back to +0x384, 0x622EF8…0x622F44); the stock mirrors' CARSKIN
  takes +0x384 (0x626FD3). **Verified** by apitrace (a community save's Corsa in
  the garage: the mirror's c18 = the body paint's 0.1952 0.1608 0.1636;
  the clone drew it black, 0x6BFA56DA having no light material record;
  `CarPart::mirror_paint`, `Car::mirror_material`); the brake draw (0x62E018…0x62E0BC)
  replaces 0xD6640DFF and 0xA3186E2B in the brake models (+0x1B08) with
  +0x398. The clone does both (`Car::part_light_material_for`; the paint
  part's attribute must exist — a fitted paint part without one is
  **inferred** to give no remap rather than DEFAULT). The other part paints
  (**verified**, 2026-11-01): the roof-scoop draw (slot 7, render
  info +0x1888, two groups, 0x6262C0 / 0x6263E0) remaps CARSKIN
  (0xD6D6080A) to the roof-scoop paint (stack copy of +0x3A4, or +0x384
  without one, 0x622F33); the spoiler draw (slot 0xC, +0x1928, 0x626355)
  remaps CARSKIN to +0x3A0 (or +0x384, 0x622EF8); the exhaust draw (slot
  0x10, +0x19A8 / +0x19B8 at every exhaust marker, 0x624C05…0x624C36)
  remaps EXHAUST_TIP (0x3B3A1F8D) to +0x394 only when it is non-zero (no
  body fallback). The clone did none of the three, so a painted exhaust
  tip drew chrome (the finished-career save's Evo, free roam and garage, against the
  original's red tip); now `CarPart::slot_paint`. +0x3A8 is the mirror
  paint above.
* car.efx registers (from CTAB): HarmonicCoeff c0–9, WorldViewProj c10–13,
  WorldView c14–16, LocalEyePos c17, DiffuseMin/Range c18/19,
  SpecularMin/Range c20/21, SpecularPower c22, EnvmapPower c23; PS
  EnvmapMin/Range c0/c1, s0 = env cube, s1 = diffuse.
* Texture remapping, from 0x6381A0 / 0x631E30 / 0x636850. Geometry uses
  generic names (`CARSKIN` light material; `%s_SKIN1`; `WINDOW_FRONT`;
  `HEADLIGHT_LEFT`; `BRAKELIGHT_*`; `CALIPER`; …). Light textures
  (**verified**, 0x631E30 + 0x60C950): the bases are the headlight (slot
  0xE, ride +0x5C8) and brakelight (0xF, +0x5CC) parts' attribute
  0x10C98090 (0x4951F0; else the bare `HEADLIGHT` / `BRAKELIGHT`);
  `HEADLIGHT_LEFT/RIGHT` → base + `_OFF`, `HEADLIGHT_GLASS_LEFT/RIGHT` →
  base + `_GLASS_OFF`, likewise for the brake lights, and
  `BRAKELIGHT_CENTRE` and `BRAKELIGHT_GLASS_CENTRE` → `%s_CENTRE_BRAKELIGHT`
  + `_OFF` (render info +0x320). Rachel's 350Z: VOLTAGE →
  `350Z_STYLE02_HEADLIGHT`, REAPER → `350Z_STYLE04_BRAKELIGHT` (the
  clone drew KIT00's for every car; `stock_texture_remap`, which keeps
  the KIT00 lookup only as a fallback where the named texture is
  missing). Light states (**verified**): 0x631E30 (render info + 0x40
  as its table) fills three headlight entries +0x2F0/+0x2F4/+0x2F8 with
  base + `_OFF` (0x7A89DC), +0x2FC/+0x300 with `_GLASS_OFF` (0x7A89D0),
  +0x304 `HEADLIGHT_GLASS_DAMAGE0`, the brake lights likewise at
  +0x308…+0x31C and the centre at +0x320…+0x328. The draw's 0x60C950
  (from 0x620AF0) always takes +0x2F4 / +0x300 for the headlights and
  +0x308 / +0x314 + 4·flag for the brake lights (flag = owner +0x944 or
  0x8A1BF4), +0x320 for the centre: every state names the same `_OFF`
  texture, and no `_ON` string exists in the EXE, so the light textures
  never change; lit and braking lights show only as glow sprites. Each car builds a
  from→to table: skin slots → the vinyl/paint composite (or `%s_SKIN1`),
  lights → `HEADLIGHT*`/`BRAKELIGHT*` on/off/glass variants, plus `%s_TIRE`,
  `%s_RIM`, `%s_BADGING`, `%s_SHADOWIG`, `%s_NEON`. `%s_SKIN1` exists in no
  file: it is composited at run time (paint + `%s_DOORLINE[_MASK]` +
  vinyls). The composite code sits next to 0x612B80 (callers 0x61D7A0 and
  0x634040).
* Skin compositor (0x61D7A0 → 0x611F20, **verified** logic):
  1. Fill `%s_SKIN1` with the paint colour (the paint part's RED/GREEN/BLUE/
     GLOSS attributes).
  2. Blend layers over it. A layer's texel is taken where its `_MASK` green
     channel ≥ 128, blended by m/128 below that, and ignored at 0. Vinyls
     (layers 1–4, colour-remapped by `REMAP`) come before the panel-line
     layer `%s_DOORLINE` (or `_KIT` / `_WIDEBODY`, chosen by 0x612B80).
  3. Seven marker texels (matched by magic float values) are rewritten with
     other parts' paint colours (see "Part-paint marker texels" below).

  The clone implements all three (`car.rs` `composite_skin`). Details
  (**verified**): the fill's alpha is the paint's GLOSS (0x61D7A0 packs
  GLOSS<<24|B<<16|G<<8|R; 0x611F20 swizzles it to ARGB), and a layer
  texel with m ≥ 128 replaces the whole texel, alpha included (m < 128:
  0x611B10 with alpha summed; a remapped vinyl's alpha is the largest
  weighted colour alpha). DUMMY_SKIN has no blending, so the skin alpha
  is only car.efx's gloss weight (r0 = 2·(env + spec)·tex.a + diffuse);
  before, the clone kept it 255, so every composited paint (GLOSS 0x80 or
  0x64) had about twice the envmap and specular. The panel-line layer
  (0x61D7A0 layer 5, never remapped) is 0x612B80's: car name +
  `_DOORLINE_WIDEBODY` when slot 6 (ride +0x5A8) holds a part above level
  0, else `_DOORLINE_KIT` when slot 1, 2 or 0xB does (the loop reads only
  the first three of its table {1, 2, 0xB, 5}), else `_DOORLINE`; the
  mask is that name + `_MASK` (350Z: `350Z_DOORLINE_WIDEBODY_MASK`
  0x7F8EEB2D, stored truncated as `350Z_DOORLINE_WIDEBODY_`). The clone
  used the stock `_DOORLINE` for every car (`Car::doorline`). The skin's
  size is DUMMY_SKIN's (512²).
* Light-material remaps (verified; 0x6174A4, through 0x48D860 per part
  before it is drawn): `CARSKIN` and 0xA7366AE6 take render info +0x384,
  the paint part's attribute 0x6BA02C05 (`METPAINT_*`, `REGPAINT*`,
  `PEARL*`, `CUSTOMPAINT_*`); `WINDSHIELD` takes +0x38C, set at 0x638DC5
  from the window-tint part (slot 0x99, car +0x7F4) by the same
  attribute, or `WINDSHIELD` itself without one. The stock tint
  (0x3D263938) names `WINDSHIELD`; the shop's tints name
  `WINDSHIELD_TINT_L1_*` … `_L3_*`.
* **Window texture** (verified): the render info's texture-remap table
  (+0x638, 0x10-byte entries {from, to, cached TextureInfo (−1 = look up
  again), –}) holds the six window slots at +0x708…+0x758: WINDOW_FRONT,
  WINDOW_REAR, WINDOW_LEFT_FRONT, WINDOW_LEFT_REAR, WINDOW_RIGHT_FRONT,
  WINDOW_RIGHT_REAR. 0x615290 sets window i's target by its damage state
  ((damage +0xE0 >> (2i + 16)) & 3, from car +0x958) through 0x802740:
  WINDOW, WINDOW_DAMAGE0, 1, 2 — GLOBALB textures (128² P8, alpha blend);
  the rear's entry +0x768 (the hatch glass's REAR_DEFROSTER, a GLOBALB
  DXT3 texture with alpha blending) keeps REAR_DEFROSTER while intact and
  takes WINDOW_DAMAGEn with the rear; entries +0x778 + 0x10·i take the
  second table 0x802750 (0xECBB8329, 0x89984A57/58/59; their keys are not
  traced). The clone remaps all six slots to WINDOW and loads
  REAR_DEFROSTER (no window damage). The 240SX's hatch glass is
  REAR_DEFROSTER: before it was loaded the clone drew it with its opaque
  white fallback, which made the rear glass bright.
* Wheel mesh (`KIT00_FRONT_WHEEL`) is a right-hand wheel. The rim face is at
  y = 0 and the tyre extends to y ≈ 0.21. Its light materials: RUBBER (the
  tyre), MAGSILVER / MAGCHROME / 0x12049251 (the rim, replaced by the rim
  paint +0x390), DULLPLASTIC, and PLAINNOTHING. **Verified** (geometry):
  the PLAINNOTHING meshes are not a blur disc but the wheel's dark inside —
  a quad closing the inner end at y ≈ 0.184 (radius ≈ 0.33) and, on most
  wheels, a ten-sided barrel behind the spokes (350Z: radius 0.22, y
  0.035…0.18; RACINGHART_STYLE07: radius 0.31, y 0.027…0.146). They use the
  wheel's own texture (`%s_TIRE`, or the rim's), and PLAINNOTHING's
  constants are all 0, so they draw black. No code names PLAINNOTHING
  (0x010CB64A has no reference in the EXE) and the wheel draw passes the
  whole model to 0x5C5940 with only the rim remaps (0x61797F, 0x627784),
  so they are always drawn (the clone did hide them, showing the floor
  through the rim). The rim's textures are resolved per car from the rim
  name: `%s_TIRE`, `%s_RIM` and `%s_RIM_BLUR`
  (the blur format string is at 0x7A88DC; resolved at 0x632A0A, hashes at
  render info +0x280/+0x284/+0x288). 0x638487 copies
  them into the car's texture-remap table at +0x630 (entries of 0x10 bytes).
  Those three are remap-table keys (+0x6A8 tire, +0x688 rim, +0x6B8
  blur; +0x68C is the rim with the region suffix 0x7FA74C, 0x6386F7), with
  targets from render info +0x2E0/+0x2E4 (0x636850). No `*_RIM_BLUR` or
  `<CAR>_RIM` texture exists in the cars' TEXTURES.BIN, CARS/TEXTURES.BIN
  or CARS/WHEELS/TEXTURES.BIN, and nothing rewrites the blur entry
  (verified: the only copies of the remap table, 0x615350: +0x638 →
  +0xAC8 and +0xF58, are body variants; 0x636850 sets +0x6AC ← +0x2E0,
  +0x6BC ← +0x2E4, +0x6CC ← +0x2E8 once, 0x6386E4; the constructor sets
  all three to `%s_CENTRE_BRAKELIGHT` + `_OFF`, 0x6323A3..0x6323C5).
* **Wheel draw** (front end 0x6162F0, matrix loop 0x6175B0…0x61774C; in
  game 0x620AF0, loop 0x627174…0x6274C4; **verified**). Per wheel i the
  model's matrix is S·[R]·F·T·car: S = scale y by width/0.184 (0x8026C4)
  and move y by −0.5 (0x8026C0) × width; F the wheel's frame
  ([ebp+0x18] + 0x40·i, front end; in game the same frame, the front ones
  first turned about x, below), its y translation zeroed; T the face shift
  y = −(p ∓ 0.5·width), p = render info +0x400 + 8i (= −record +0x1C).
  R is a 180° turn about z (0x5BAE70: 0x5BACA0's z rotation by 0x8000
  binary degrees = diag(−1, −1, 1)), built at 0x617863 / 0x62766E and
  multiplied in only for wheels 0 and 3 — the left ones (record y > 0) —
  at 0x617A1F / 0x619CBA (front end) and 0x627993 / 0x62C1E8 (in game).
  Multiply order (row vectors, esp tracked through the pushes;
  **verified** 2026-10-28, test `aftermarket_rim_is_a_r_f_b`): front end
  C = A·R at 0x617A1F (A at esp+0x120 + 0x40i, R at esp+0xC0) into
  esp+0x70, copied to esp+0x10 (0x4012B0(dst, src), cdecl), D = C·F at
  0x617E72 (F = [ebp+0x18] + 0x40i), copied again, E = D·B at 0x618107
  (two pushes: esp+0x230 there is B at esp+0x220). In game the same:
  A esp+0x200, R esp+0x100 (0x62766E), C = A·R 0x62798C, D = C·F
  0x627DDF (F the frame or its x-turned copy esp+0x990), E = D·B
  0x62807B with B = esp+0x3D0 + 0x40i. B is applied last, after A's
  scale, so the scale never moves the face: y = s·y_model − w/2 (A),
  negated on the left (R), then −(p ∓ w/2) (B) ⇒ face + s·y_model.
  Rachel's 350Z (DDAY_PLAYER_CAR, RACINGHART_STYLE07_20_26_A, model y
  −0.0100…0.1938; widths 0.225 / 0.235 → s 1.2228 / 1.2772, faces
  ±0.88176): rim y ±0.8940…±0.6448 front, ±0.8945…±0.6342 rear in the
  front end — the clone's matrices give the same. The wide body KITW02's
  arch lip sits at |y| 0.962 (front axle) / 0.941 (rear), the stock
  body's at 0.893 / 0.900. The front end draws the wheels at +0x1C on any
  body (4.7–6.8 cm inside the wide arch); **in game** a fitted wide body
  moves them to the record's +0x20 (0.946 on the 350Z; see the face slot
  k below, **verified**). 0x61FE20 reads only the car type record ([render info +8] + 0x120 + 0x30i:
  +0x14 width, +0x1C face, +0x20) and the rim parts' levels (ride +0x604
  / +0x608); the wide body's attributes are CV (0x61C330: the collision
  volume's name hash, default `<CAR>_CV`, from body +0x5A4 / +0x5A8 when
  ride flags +0x83D / +0x83E allow; used by the physics set-up 0x5AED15)
  and 0xF7933C86 = bin_hash("EXCLUDE_SUV") (a shop filter). Whether the
  CV collision volume moves the physics wheel frames' x/z: **open** (the
  draw zeroes only their y).
  * **In-game brakes** (**verified** 2026-10-28; esp tracked along the
    control flow, depth 0xEE4 in the body; `car.rs`, test
    `in_game_brakes_sit_at_the_marker`). The loop also builds, per
    wheel, T_brake = esp+0x4D0 + 0x40i (0x627416…0x627439): y = −(p + v)
    for p < 0, −(p − v) else, v = s·X when X ≠ 0 else w/2 (w the table
    width, 0x8026C0 = 0.5), X = render info +0x430 (wheels 0, 1) / +0x434
    (2, 3), unless 0x7A6694 / 0x7A6698 are non-zero (both 0, no writer);
    and it zeroes the y of all three frames [ebp+0x18], [ebp+0x1C],
    [ebp+0x20] (0x627472…0x627491). Each brake is M·F·T_brake (row
    vectors), F = [ebp+0x1C] + 0x40i, the front ones first turned by
    0x5BAD00 with their translation kept (wheel 0 by +angle at 0x62E141,
    wheel 1 by −angle at 0x62EBB7): wheel 0 M = diag(1, −1, 1)
    (0x7A66D4…DC into esp+0x310/0x324/0x338 at 0x62E0CA, product at
    0x62E43C), wheel 1 no M (F1·T at 0x62EBDD), wheel 2 no M (F2·T at
    0x62F2D4), wheel 3 M again (0x62FA9D, ·T at 0x62FDBC); then the car
    matrix (ebx). So in game a brake's origin sits v inside the face
    (stock 350Z: X 0.0837, s 1 → y ∓0.798 against the centre ∓0.755;
    Rachel's: X 0.1091, s 1.2228 front → ±0.813 with the wide body's
    face 0.946). The front end keeps them at the centre (0x6162F0, no
    T_brake). X is the y (+0x44) of the `MARKER_BRAKE` marker
    (0xEA8DFB05, string 0x7A8D1C) of the front / rear rim model, +0x1B48
    / +0x1B68 = slot 0x1D / 0x1E group 0 LOD A, found by 0x48D8F0 →
    0x488FA0, 0 when absent (0x6390A6…0x639112, after 0x61FE20).
    `devprobes/src/bin/brake_markers.rs`: every stock `<CAR>_KIT00_FRONT_WHEEL_A/B`
    has one (y 0.0511 A3 … 0.1373 HUMMER, x = z = 0), as do the shop rims
    except LEXANI_STYLE14, LIMITEDALLOY_STYLE01, SPINNER_STYLE12,
    STARK_STYLE01 and the `_SPIN` groups (those take w/2); LOD C models
    carry none. The calipers are part of the brake models: no separate
    caliper draw in 0x62DF85…0x6304D4.
  * The pose pivot: F (which carries the steer) comes before the face
    shift B / T_brake, so each part turns about its own origin after
    A·R: the rim and spinner about the tyre's middle (y = B's, T_spin's),
    the in-game brake about T_brake's y; the front-end brake about the
    centre (its frame keeps its translation). The clone's
    `CarPart::pivot_y` carries this.
  So the left wheels are the right-hand model turned round, not mirrored
  (a directional rim keeps its sense; the sidewall is not reversed); the
  face still lands at +0x1C. No term scales the wheel to the record's
  tyre radius (+0x10): the model is drawn at its own size. Each wheel is
  drawn only when render info +0x2D78/+0x2D98/+0x2DB8/+0x2DD8 (wheel
  0…3) is set and, with a car object, its byte +0x3F4…+0x3F7 is non-zero.
  * Record wheel +0x20 (±0.5 on every traffic type): 0x61FE20
    stores it negated at render info +0x404 + 8i (0x61FED0) beside the
    face +0x400 + 8i, and SPINNEROFFSET at +0x438 / +0x43C + 8i. No
    instruction reads +0x404 by a fixed offset (the 0x404 hits in
    0x627xxx/0x628xxx are stack slots). The in-game loop reads the face
    through a pointer, info + 0x438 + 4k + 8i − 0x38 (0x62714A, 0x62718E),
    k = the frame slot esp+0xD8. **Verified** (control-flow esp tracking:
    the body's depth is 0xEE4 on every path; the last stores before
    0x62714A are 0x622F8A / 0x622FD1, on every path): k = 1 when ride
    +0x5A8 (slot 6, WIDE_BODY) holds a part whose level (byte +5 >> 5) is
    above 0, else 0 (0x622F63…0x622FD1). So in game a wide body reads
    +0x404 = −record +0x20 as p (and +0x43C, the same SPINNEROFFSET copy,
    as o) for the rim, spinner and brake shifts; the front end always
    reads +0x400 (0x61759A). Record +0x20 on the player cars: 350Z 0.946,
    240SX 0.902, GTO 0.997 … (0.5 on traffic, which has no wide bodies).
    Ported (`Wheel::face_wide`; in game the rims, spinners and brakes use
    it with a wide body). Traffic is not drawn through 0x620AF0: 0x6316E0
    hands kind-3 cars to the front-end draw 0x6162F0 (verified, see
    Traffic → Traffic wheels). PARCELVAN (`devprobes/src/bin/van_wheels.rs`): wheels at y ±0.855,
    face +0x1C ±0.936, tyre model y −0.007..0.296 (so it spans
    0.64..0.94), body half width 1.266 with its wheel-arch vertices at
    |y| 0.976; PANELVAN face ±0.85/0.87, body 1.13. The clone draws them
    at the face like every car; the reported missing van wheels are not
    explained by anything traced here.
  * Each wheel gets **its own light context**: 0x48F660 is run with the
    wheel's full matrix into a 0x1C0-byte context per wheel (front end
    0x61876D, 0x619166, 0x619C21, 0x61AAD1; in game 0x6286E7 … 0x62DE8D,
    rim and spinner models), and each brake into its own (0x62EB26,
    0x62F287, 0x62FA4E, 0x6304A8, with the brake's matrix; one branch
    draws a brake with its wheel's context). HarmonicCoeff and LocalEyePos are therefore in the wheel's
    turned axes. Before the clone lit them in the car's axes, which with
    the turn left the left rims dark (their outward normals faced the
    car's right); it now lights wheels and brakes per part.
  * In 0x620AF0 (in game and in the showroom), the front wheels' frames
    (rim and spinner) are first turned about x (0x5BAD00, 0x62794C /
    0x6287D3 / 0x62964F / 0x62A138, on a copy of the frame) by 7°
    (0x7A675C) × v, v = 0x59A430 on [render info +4] + 0x10, i.e.
    0.5·(ride byte +0x364 ≠ 0) + 0.5·(ride byte +0x363 ≠ 0), used when
    0x7A6758 is 0 (always) and the LOD ≤ 1 (0x6270B2…0x627112, the angle
    trunc(7·v·65536)/360 binary degrees). **Verified** 2026-11-01:
    ride +0x10 is the performance object, whose upgrade state is ride
    +0x2F0 with the fitted part bytes at +0x50 (see **AI cars**), so
    +0x363 / +0x364 are performance parts 0x23 and 0x24 (0x24 = Adjustable
    Camber Plates, language string 0x7067AB57). Read from the running
    original's main menu (render info found by its face table +0x400):
    Rachel's 350Z has +0x363 = 0, +0x364 = 1, v = 0.5, 637 binary degrees
    (3.5°); her preset fits 0x24 but not 0x23. The turn is about the car's
    x axis after the steer, at the model's origin; wheel 0 by +angle,
    wheel 1 by −angle (apitrace: front-right rim axle (−0.5, 0.8644,
    −0.0529) with the steer 30°). The **brakes** are turned by the same
    angle at 0x62E141 / 0x62EBB7 — but in place, on the caller's frame
    [ebp+0x1C] itself — so each 0x620AF0 call on the same frames adds a
    turn: the showroom draws the car into the floor reflection first
    (1×, 3.5°) and then the view (2×, 7.0°), both read from the apitrace
    (main menu, 320×240 reflection pass then the main pass, frames 10300
    and 10500 alike). In game the frames are filled again before each car's
    draw (0x63182C → 0x5EC800 before 0x6316AE), so **inferred** 1× there
    (the chase-view trace drew no front brake to measure). The front end's
    0x6162F0 calls 0x59A430 too (0x617564) but discards the result. The
    matching z shift (0x7A6760 0.03 × 0x7A6758) is 0, 0x7A6758 being 0
    and never written. Clone: `Car::camber`, `camber_angle`,
    `WheelPose::tilt` / `brake_tilt`, the garage's per-draw count.
  * **The showroom** (verified 2026-11-01 by apitrace): the front end's
    car (0x631239) is drawn by 0x620AF0 like the racers, not by 0x6162F0
    (which draws only traffic, 0x6318F5). So its rims sit at the in-game
    face (Rachel's wide body: |y| 0.9307 = record +0x20 0.946 less the
    shift), its brakes at the MARKER_BRAKE shift (|y| 0.8125) with the
    camber turn above, rather than 0x6162F0's +0x1C face and centred
    brakes, which the clone drew before. **Open**: the spinners (in game
    only in the clone). The showroom wheels' z (**verified** 2026-11-01,
    <work dir>): the FE car's wheel frames (0x5F6830 with its bit in +0x5B0:
    record +0x120 + 0x30·i through [car +0x20]) and its placement 0x4A6670
    (lift = mean(radius − wheel z), the frames' z less min(corner, +0x178))
    read the FE car's own physics object, FE car +0x20 → 0x5B5360(1) →
    0x5B4FD0(1, 1, 1) → 0x5B4DD0: the upgraded record. Rachel's suspension
    (category 5, gain 0.66) adds 0.66 × 0.0225 (+0x604) to the front wheel
    z: 0.17072 → 0.18557, the trace's brake origin, and the body sits that
    much lower. The clone drew the FE car from the stock record
    (`Car::load_with`); it now loads it with the ride's performance parts
    (`GarageRequest::perf`, `Car::load_upgraded`; test
    `showroom_wheels_take_the_upgraded_record`). **Open**: which tuning
    settings the FE car's physics object holds (0x5B4790 runs when
    0x59B0F0 > 0); the clone applies none there.
  * **Per-wheel shifts** (verified, loop 0x627174…0x6274C4, wheel i,
    p = render info +0x400 + 8i, w the width +0x420 + 4i (0.184 when 0),
    o = +0x438 + 8i, B = +0x430 front / +0x434 rear): S (0x200 + 0x40i)
    scales y by w/0.184 and moves y −0.5w; T_rim (0x3D0…) y = −(p + 0.5w)
    for p < 0, −(p − 0.5w) else; T_spin (0x5D0…) y = T_rim.y + o for
    p < 0, − o else; T_brake (0x4D0…) y = −(p ± h), h = B·w/0.184, or
    0.5w when B is 0. B = the y (+0x44) of the `MARKER_BRAKE` marker on
    the front (+0x1B48) / rear (+0x1B68) rim model, 0 without (0x6390A6).
    o = the front rim part's (ride +0x604) `SPINNEROFFSET` attribute (a
    float, 0 without) for all four wheels (0x61FE20).
  * **Spinners** (verified; `car.rs`, `Scene::InGame`, `CarPart::spinner`):
    in game, when the rim part has a second model group (+0x1B58 front,
    +0x1B78 rear; e.g. DA BENJAMINS `_15_23_SPIN`), it is drawn per wheel
    as S·[R]·F_spin·T_spin·car (spinner chain 0x628813…0x6295AB for wheel
    0: stage 1 S·R, stage 2 the frame, stage 3 T_spin), lit with its own
    light context (0x62956E), with no rim paint (the remap loop at
    0x627784 pairs the spinner models with material +0x3AC, set to 0 at
    0x63908F). The front end never draws group 1 (0x6162F0 reads only
    +0x1B48 / +0x1B68 + LOD). F_spin is the caller's third frame buffer
    ([ebp+0x20], filled by 0x5EC800 from 0x63182C): turned about y by
    car object +0x4E8 + 4i (radians × 65536/2π, 0x78423C), the front ones
    then about z by the steer +0x40E, at the wheel's table position with
    z + (+0x380 + 4i). The angle runs in 0x5ECAD0 (from 0x6012F7): rate
    (+0x4F8 + 4i) += (ω − rate)·0.0035 (0x7A5660)/0.0225 (0x7A565C)·dt,
    angle += rate·dt, ω = +0x330 + 4i, the wheel's angular speed (v/r
    at 0x5F537C). The clone uses its wheel spin for ω
    (`WheelState::spinner_rate` / `spinner`). `world --car X` with
    NFS_SPINNERS=1 (and NFS_SPINNER_ANGLE) shows them; `game --shot
    <screen>_spin` fits them to Rachel's car.
  * **Open**: what fills the wheel frames ([ebp+0x18]; in the front end
    0x6316E0 passes a buffer filled by 0x5F6830) and the in-game ones
    (suspension, roll); the clone uses its own pose (lift, steer, roll).
* Car effect constants and shading are transcribed in `render/car.wgsl`
  (re-checked against IDI_CAR_FX's second vs_1_1 / ps_1_1 pair, 0x3604 /
  0x3448 in the resource; its technique also has car_fixed and
  car_fixed_envmap fallbacks). 0x5BD4B0 keeps the env-map terms only when
  mesh flag 0x100 is set and g_CarEnvironmentMapEnable (0x870CB8) is
  non-zero; then without 0x80 it zeroes diffuse and specular.
  SpecularPower = 1.1 (0x800B94, read only at 0x5BDA63) × f(+0x58).
* **Car lighting** (verified; `render/lighting.rs`). Per car each frame
  0x620AF0 builds a 0x1C0-byte light context: 0x48ED60 gathers lights,
  0x48F660 (0x48E6C0 + 0x48A6A0) turns them into the car's axes, 0x5BD150
  uploads HarmonicCoeff (context +0x120, ten vec4) and LocalEyePos
  (+0xC0).
  * The scene's three directional lights: in game the static setup
    0x7F3F00 (never written), records of 0x1C bytes from +0x20: flag, θ°,
    φ°, r, g, b, intensity — (1, 200, 90, white, 0.5), (1, 42, 33, white,
    0.35), (1, 219, 10, white, 0.1). Direction (0x48A310): A = θ + 180, B
    = 90 − φ as binary angles `(int)(deg·65536)/360`, d = (sinB cosA, sinB
    sinA, cosB); flagged lights are camera-fixed: (−d.y)·right + (−d.z)·
    down + d.x·forward, with the camera axes of the look-at 0x5BA9B0
    (forward, right = (−up)×forward, down = forward×right). Colour =
    intensity·rgb. The frontend's setup (verified) is render info
    +0x904, handed to 0x620AF0 by 0x63121C: 0x63B89B starts it at the
    default 0x7F3E10 ((1, 149, 100), (1, 303, 36), (1, 76, 22), white,
    0.2 each); 0x4A7180 sets it to the named setup of the platform scene
    it loads (`FRONTEND\PLATFORMS\%s.BIN`, names at 0x7F60E8: NONE,
    SHOWROOM, AUDIOSHOP01, CARLOT01, CRIB01, MEGALOSHOP01, PAINTSHOP01,
    PARTSSHOP01, PERFSHOP01) through 0x48A2D0, which searches the 0x74-byte
    records of chunk 0x3B700 (FrontB, ten setups; `parse_light_setups`).
    Only SHOWROOM (0x4AF464) and CRIB01 (0x4AFAD0) are ever passed there.
    The third caller of 0x48A2D0 is GarageMain.fng's constructor (0x4EA900,
    vtable 0x7F82D4 entry 0x4EB130), the garage behind the car lots, which
    sets +0x904 of both its cars (0x4EAE1E): SHOWROOM when the game mode
    0x850078 ≠ 1 (not career); in the career, when 0x863444 and the
    visited shop 0x86344C are set, the shop record's scene file (+0x40)
    up to its first '.' (UC_CAR_LOT: CARLOT01.BIN → CARLOT01; the others
    AUDIOSHOP01, PAINTSHOP01, PARTSSHOP01, PERFSHOP01, CRIB01), else
    CRIB01. The clone follows this for the platform scene and the light
    setup alike (`Game::frontend_scene`: the main menu, Quick Race and
    Customize SHOWROOM; in the career the visited shop's, else CRIB01).
    0x863444 (verified) is the career object 0x85AD40's +0x8704: the map
    trigger the shop was entered from. 0x532B20 stores the trigger the car
    drives into at +0x8700 and its shop at +0x8724; on entering the shop
    0x514FD0 copies +0x8700 → 0x863444 and +0x8724 → 0x86344C (as does
    0x500B00), and 0x541B22 / 0x500B40 zero both together. So 0x863444 is
    set exactly when a shop is, and the clone's shop test covers it.
  * Track lights (0x48E3D0): the light packs (below); per pack the leaf of
    its box tree holding the car (0x5747F0: the first child box, depth
    first); a light counts when on (+7 = 1) and closer than its outer
    radius, weight 1 inside the inner radius, else 1 − (d² − in²)/(out² −
    in²); only kinds 2 (point) and 3 (spot) light cars, at most ten.
    Colour = rgb·0.75·weight /255, spots also × the cone fade (f − t)/(1 −
    t), t = cos(cone/2) through 0x43C350, f = dot(norm(car − light),
    spot direction); direction = light − car.
  * Each light, unit direction d and colour c with w = 0.29r + 0.54g +
    0.17b, adds a second-order spherical-harmonic lobe (0x4895E0) plus a
    −0.05·c lobe at −d (0x489AF0), as the quadratic c0 + c1x² + c2y² +
    c3z² + c4xy + c5xz + c6yz + c7x + c8y + c9z: with e = 3z² − 1, g = x²
    − y²: c0 += c·0.282095·0.886227 − c·0.315392·e·0.247708; c1 += and
    c2 −= c·0.546274·g·0.429043; c3 += c·0.315392·e·0.743125; c4 +=
    c·1.092548·xy·0.858086; c5 += c·1.092548·xz·0.858086; c6 +=
    c·1.092548·yz·0.429043 (half of c4/c5's factor, as in the game);
    c7..c9 += c·0.488603·1.023328·(x, y, z).
  * 0x489ED0: the colour channels are left as they are (flag 0x82DAE4 =
    0); the w channel is scaled to length 0.8 over its ten terms
    (0x7F39BC = 1, 0x7F39C0 = 0.8).
  * 0x489BA0 rewrites the form in the car's axes: q'(n) = q(R n).
  * LocalEyePos (0x48A6A0): the eye in the car's axes, its xyz length
    (0x5BB1A0 clears w to zero) clamped to 2.78 in game (2.78–8 in the frontend, 0x8654A4 =
    3): a distance-clamped eye direction, one per car for the body's parts;
    the wheel draw builds one per wheel and brake with that model's
    matrix (see "Wheel draw"). Paired native main/reflected body and all
    four rim/brake contexts: `docs/evidence/fe-reflection-lighting`.
* **Light packs** (verified; `formats/lights.rs`): container 0x80135000
  in each streaming section (loader 0x48E0B0): 0x135001 header (16-byte
  aligned; +8 u16 version 4, +0xC section, +0x1C count), 0x135002 box
  tree (16-aligned; nodes from +0x10, 0x30 bytes: centre, i16 +0xE child
  count or −light count, half extents +0x10, i16 indices +0x1C),
  0x135003 lights of 0x60 bytes (kind +4, +5, on +7, colour +0xC r/g/b
  bytes, position +0x10, outer +0x1C, spot direction +0x20, intensity
  +0x2C, inner +0x30, cone +0x34). Bayview (L4RA) has 2234, nearly all
  point lights of (110, 110, 125), radius 30, inner 10.
  Only the **loaded** sections' packs light the cars (verified): the
  0x80135000 case of 0x48E0B0 links the pack header into the list
  0x830D14 (tail 0x830D18, 0x48E22F…0x48E248) and into the section record
  (0x5DB690 +0xC); 0x48E3D0 walks that list (leaf by 0x5747F0, each light
  through 0x4893A0: on, inside the outer radius, weight 1 inside the inner
  radius else linear to 0; kinds 2 and 3 collected, at most 24; kind 0
  with +5 = 2 summed into a weighted ambient, other kind 0 the nearest
  ambient). Section 121 (A21) holds a "moon" light (kind 2, (40, 40, 55),
  at (−4500, −2000, 3000), radius 9999.99) that reaches the whole map; at
  the airport start the original's HarmonicCoeff is the white rig alone
  (equal r, g, b), so A21 is not loaded there. The clone took every
  pack; it now takes the packs of the camera section's visible list
  (`CarLighting::in_world`; the visible list standing in for the loaded
  set is **inferred**, as for the world draw), which gives the trace's
  harmonic (c0 0.243 vs 0.242); a world without visibility lists takes
  none, never every pack (`World::light_pack_sections`).
  The ten-light test of 0x48E3D0 (`9 < count`, **verified**) guards the
  pack walk only; the scene groups of 0x835850 go on until 0x4893A0's 24
  (the clone had capped both at ten).
* **Environment map** (verified; `WorldRenderer::render_environment`).
  EnvMapTexture (0x5BCAE0) is, in game (0x8654A4 = 6), the render-target
  cube 0x870858 (128², 0x7FEE6C), else the static cube 0x87088C made from
  the frontend textures XPOS…ZNEG. The six faces are views 0x10–0x15
  (0x5BA220: +X 0x11, −X 0x13, +Y 0x14, −Y 0x15, +Z 0x10, −Z 0x12), set up
  by 0x5C3460 (checked by running it under emulation): 90° (0x871B90 =
  0x4000), near 0.5, far 64 (0x7FEE88/8C), at the player car + (0, 0,
  0.95) (0x61AB60), oriented by the main camera so they are the standard
  cube faces of its (right, down, forward) axes — car.efx samples with
  the reflection vector in that camera space (WorldView, 0x5BDDF0).
  0x5CAAE0 renders each enabled face: clear to black (0x5BA3D0), the sky
  model 0x8A1F10 without depth (0x60F890), then the world when
  g_CarEnvironmentMapUpdateData (0x870CBC) is set. Faces per frame follow
  g_CarEnvironmentMapEnable (0x870CB8, capped by the card at 3) through
  the table 0x7FEEA0: 1 face a frame at 1, …, all six at 3. The clone runs
  the top settings (all faces, world on). **Open:** the sky model the
  faces draw (0x8A1F10, scaled 0.004525²) — the clone's sky is a world
  placement, so the faces show black past 64 m.
* **Part database** (GLOBALB 0x80034602, loader 0x636CDB; `src/formats/parts.rs`).
  Its children are read from right after their 8-byte headers, not from
  the 16-byte aligned payload: 0x34603 header (a pointer slot and a count
  per table), 0x34606 string pool (dword indexed), 0x3460C attribute lists
  (`i16 count`, u16 indices), 0x34605 attributes (`u32 key, u32 value`),
  0x3460A models (0x24: +0 named, +2 the name's middle part, two groups ×
  four LOD names), 0x3460B group hashes, 0x34604 parts (0x0E: +0 hash, +4
  part type, +5 kind | level << 5, +6 flag, +7 group index, +8 name, +0xA
  attribute list, +0xC model). A named model's solid is
  `<CAR><middle><name>_<LOD letter>` (240SX + `_KIT00` + `_FRONT_BUMPER` +
  `_A`; `_BASE`, `_DECAL`, `_WIDE1` likewise); a hashed model holds the
  solid hash. The prefix is in fact a seed picked by part +0x06 (see
  "Shared part models" below): the car's name is its group hash. The slot table in the EXE (0x802960) gives each of the 0xAA
  slots its part type; GLOBALB 0x34607 gives each slot two groups (the
  car's own name hash where 0xFFFFFFFF; 0x6101F0), then per-car overrides.
  A part for a slot is the first of the slot's type in the first group
  that has one, skipping parts marked EXCLUDE_UG1 / EXCLUDE_UG2 /
  EXCLUDE_SUV on the cars listed at 0x610030 / 0x6100B0 and the SUV
  groups (0x61B680). The stock car (0x637050) takes the level-0 part per
  slot (0x61BC90), and for the paint (slot 0x3F) the part named by car
  type +0x84C; the paint's RED, GREEN, BLUE, GLOSS attributes give its
  colour and attribute 0x6BA02C05 its light material (0x61D7A0;
  METPAINTSILVER on the 240SX, REGPAINTBLUE on the Peugeot's gloss blue).
  Brakes: group 0 front, group 1 rear. Verified on every player car by
  `devprobes/src/bin/stock_check.rs` (all real parts' LOD A solids exist; the misses
  are optional second groups, placeholders and the plate). Visual points
  per part: table 0x34A1C (0x28 per category, levels 1–3 at +4/+0x10/+0x1C;
  0x501860 via 0x501D70), stock (level 0) parts score 0.
* **Shared part models** (verified unless marked; `car.rs`
  `shared_part_files`, `PartDb::model_seed`):
  * Files: the game keeps them loaded with the cars (0x57F040; the same
    list for each of its load / unload modes): the table 0x86658C (eleven
    paths copied from 0x7FA978 by the static initialiser 0x77D850) —
    `CARS\EXHAUST`, `BRAKES`, `MIRRORS_POST`, `MIRRORS_BODY`,
    `MIRRORS_SUV`, `MIRRORS_HUMMER`, `SPOILER`, `SPOILER_HATCH`,
    `SPOILER_SUV`, `ROOF`, `AUDIO` (each `\GEOMETRY.BIN`) — then
    `CARS\WHEELS\GEOMETRY_%s.BIN` (0x7A0AB4) for every part of type 0x43
    in group 0xA6E1B733 (0x61B680), `%s` the part's name (0x610010), when
    shorter than 0x1E3. `PLATES\GEOMETRY.BIN` and the texture packs
    `CARS\WHEELS\TEXTURES.BIN` (the rims' textures, e.g.
    `RACINGHART_STYLE07_WHEEL`), `CARS\TEXTURES.BIN`, `BRAKES` / `PLATES`
    `\TEXTURES.BIN` (empty) are in the same string table (0x7FA9A0…0x7FA9B4,
    read by the front-end loader at 0x57FBDE). `CARS\WHEELS\GEOMETRY.BIN`,
    `CARS\MIRRORS` and `CARS\SPINNER` are named nowhere in the EXE. A
    model is found by solid hash among everything loaded (0x4908E0), so the
    clone parses, from those files, only the solids the fitted parts name.
    The shared parts' other textures are GLOBALB's (the post mirrors'
    `CHROME`), which is always resident; the clone takes those a shared
    solid uses.
  * Names: the car model builder 0x634810 (and the wheel object 0x633660
    → 0x60FF90) continue the string hash (0x43DB80) from a seed chosen by
    part +0x06: 0 the hash start 0xFFFFFFFF, 1 the part's group hash (part
    +0x07 into 0x3460B: the car's name for its own parts, `SPOILER`,
    `ROOF`, … for the shared ones), otherwise the part's brand, attribute
    0xEBB03E66 (0x4FE110 → 0x61B510); then the middle string (model +2),
    the name and `_A`…`_D`. In GLOBALB, +0x06 = 0 parts have hashed models
    (624) or none, 1 parts named ones (8447), and the 1369 parts with 2 are
    all rims (type 0x1C): `RACINGHART` + `_STYLE07` + `_20_26` + `_A`
    (`devprobes/src/bin/part_flags.rs`, `devprobes/src/bin/shared_parts.rs`).
  * Mounts (the car draw 0x620AF0): 0x634F66 looks up four markers
    (0x488FA0) on the LOD A solid of the base part (slot 0, ride +0x590):
    SPOILER → render info +0x2C04, ROOF_SCOOP → +0x2C08, 0x9B971511 →
    +0x2C0C (left mirror), 0x0EE87EC4 → +0x2C10 (right mirror; names not
    in the EXE, both also handed to the damage object +0x108 / +0x10C). A
    marker (chunk 0x13401A, 0x50 bytes: hash, then a row-vector 4×4 at
    +0x10) places a part as marker × car matrix. Models per slot sit at
    render info +0x17A8 + slot·0x20 + group·0x10 + LOD·4 (slots 0–0x3E,
    two groups, 0x634810). The spoiler's two groups are drawn at SPOILER
    (0x625AAB), else in car space (0x626334); the roof scoop's at
    ROOF_SCOOP (0x626039), else 0x6263B9; mirrors whose part (ride +0x610)
    is above level 0 (0x62645D) draw group 0 at the left marker and group
    1 at the right one with its +0x14 (the y axis's y, −1 in the data)
    negated (0x626A4C), since the right models are built facing −y; stock
    mirrors in car space (0x626F7F). `devprobes/src/bin/part_markers.rs`,
    `devprobes/src/bin/fitted_parts.rs`.
  * Brakes (slots 0x1B / 0x1C, models +0x1B08 / +0x1B28; drawn from
    0x62DF85) use the wheel's own matrix (frame +0x1C), not the rim's
    face-shifted one (frame +0x18, whose y translation 0x6162F0 zeroes
    before adding −(face ± width/2)): they sit at the wheel centre
    (record +0x04), wheel 0's through diag(1, −1, 1) (0x7A66D4…DC,
    0x62E0CA), the front ones turned by the steer angle (0x5BAD00) with
    the translation taken out and put back (**verified** for the front
    end; in game the frame's y is zeroed and the brake gets the
    MARKER_BRAKE shift esp+0x4D0 — **verified** and ported, see "Wheel
    draw", "In-game brakes").
  * Rims (slots 0x1D / 0x1E) go through the wheel placement
    like the stock ones: the shared rims are built as the cars' own
    wheels (RACINGHART_STYLE07_20_26_A: y −0.01…0.19, radius 0.340;
    350Z_KIT00_FRONT_WHEEL_A: y −0.013…0.194, radius 0.348), and the wheel
    draw (models +0x1B48 / +0x1B68 at 0x61795B) scales them only across
    (see "Wheel draw"; verified), turned round on the left.
  * Exhausts (slot 0x10, `CARS\EXHAUST`; **verified**): 0x639843 (in the
    render info set-up, the loop 0x6395B0…0x639959) walks slots 0–0x3E
    whose ride +0x838 flag is set, takes each one's group 0 LOD A model
    (render info +0x17A8 + slot·0x20) and files its markers (0x48D8A0) by
    hash into lists in the render info; EXHAUST (0xA5B28001),
    EXHAUST_LEFT (0xA2799C2B), LEFT_EXHAUST (0xBCF8A18B), RIGHT_EXHAUST
    (0xBD7CF15E), 0xDF066A41 and EXHAUST_RIGHT (0xF21BE91E) go to the list
    at +0x598 (nodes: +0x10 the marker, +4…+0xC its translation). The
    car draw (0x6244F7…0x624CA5) draws the exhaust's two model groups
    (+0x19A8 / +0x19B8, EXHAUST_TIP light material from render info +0x394
    via 0x48D860) at every node, as R × marker × car, R = 0x5BADB0(I, 0x4000)
    — a quarter turn about y, rows (cos, 0, −sin), (0, 1, 0), (sin, 0, cos)
    (0x43C370 = sin(a + 0x4000), 0x43C2B0 = sin), i.e. (0, 0, −1), (0, 1, 0),
    (1, 0, 0): the model's +x runs along the marker's −z, forward under the
    car. The 350Z's markers are on the rear bumper (KIT00: y ±0.40 at
    x −2.10) and the wide bodies (KITW02: y ±0.393 at x −1.985); the right
    one's y axis is negated, so the right tip is mirrored. `car.rs`
    (`EXHAUST_MARKERS`); the clone searches only the car's own solids for
    the markers.
  * License plate (slot 0x21, LICENSE_PLATE in the slot-name table
    0x802EF0; **verified**): the same walk (0x639805) files
    REAR_LICENSE_PLATE (0xC805F7E0) markers in the list at render info
    +0x620; FRONT_LICENSE_PLATE (0xAA35C17F) and LICENSE_PLATE_FRONT
    (0x725758BF) go to +0x618 (0x639798), which no code reads as a list
    (no `[reg+0x618]` reader of the render info found; so no front plate).
    The car draw (0x624CAB…0x625A79; skipped past LOD 2, against the
    exhaust's LOD 1) draws the plate part's two model groups (+0x1BC8 /
    +0x1BD8) at every node through the exhaust's quarter turn R. The
    stock part LICENSE PLATE STYLE01 names `CARS\PLATES`'
    LICENSE_PLATE_STYLE01_A (0.32 × 0.16, facing the model's −x, so the
    marker's +z: back), texture LICENSE_PLATE (0x029A22F7) and light
    material LICENSEPLATE (0x4C95C6F8). LICENSE_PLATE is a fixed 128×64
    DXT1 texture in GLOBALB with "Need for Speed" and "UNDRGND 2" painted
    in: no text is rendered (`PLATES\TEXTURES.BIN` is empty; the
    remap table set up at 0x638487 has no entry for it). 0x631E30 also
    hashes `%s_LICENSE_PLATE` into texture info +0x28C (render info
    +0x2CC) without adding it to the car's texture list; no reader found
    (**open**: what it is for, and whether a custom-plate option exists —
    no plate string or front-end screen found). Cars with the marker: all
    player cars (e.g. 350Z KIT00 at x −2.07, z 0.54); the traffic models
    (4DR_SEDAN, TAXI, PICKUP, SUV, …) have no plate markers, so none is
    drawn on them. `car.rs` (`LICENSE_PLATE`, `REAR_LICENSE_PLATE_MARKER`).
  * Slot flags (ride +0x838, a byte per slot; **verified**, 0x61BCE0, run
    after a part change): all set, then cleared for each name a fitted
    part's attributes 0xE80A3B62 give — matched against the part type
    names 0x80333C and cleared at that index through the slot table
    0x802DE8 (entry i is slot i), type 0x45 excepted (so the TWISTER hood's
    DECAL_HOOD, type 0x25, clears slot 0x25). Slot 6 with a part above
    level 0 (a wide body) clears slots 1, 2, 5, 0xB, 0x17, 0x18 and
    0x37–0x3A and sets 6 and 0x3B–0x3E (and replaces the doors 0x11–0x16
    and the wide decal slots 0x3B–0x3E with `%s_KITW0%d_DOOR_…` /
    `%s_WIDE%d_…` parts); without one the reverse, the doors stock. The
    generic part draw (0x6230D5) skips slots 3, 4, 7, 0xC, 0xD, 0x10 and
    0x1B–0x2E (table 0x630D3C; they have their own draws) and draws the
    others only while their flag is set (0x62319F; slots 1–0x1A also go
    through a per-slot switch at 0x623143, not traced). Before this the
    clone drew Rachel's 350Z with the KIT00 bumpers, body and skirt inside
    its Mantis wide body. `car.rs` `shown_slots`. **Open**: the slot 9
    (HOODUNDER), 0xD (engine, mode 3) and 0x1D (0x839BC1 → slot 0x1D flag,
    which gates the wheel draw at 0x627514) cases, the switch at 0x623143,
    and whether the own-draw slots test the flags.
  * Part-paint marker texels (**verified**, 0x61D7A0 → 0x611F20): the
    composite target is `DUMMY_SKIN%d` (0x61C2A6; CARS/TEXTURES.BIN,
    hashes 0x530B82B0…B5, 512² A8R8G8B8, the cache picked by hash at
    0x61DD59). On its first composite 0x611F20 scans that texture, before
    filling it, for the ARGB values 0xBFFF0000, 0xBF00FF00, 0xBF0000FF,
    0xBFFF00FF, 0xBF00FFFF, 0xBFFFFF00, 0xBFFFFFFF (compared as floats)
    and keeps their positions (0x8A05E8, flag 0x8A1D18); all six
    DUMMY_SKINs have them as 4×4 blocks at y 508–511, x 508 (red) down to
    484 (white) (`devprobes/src/bin/skin_markers.rs`). After the layers, those
    texels take colours k = 0…6 from the paint slots 0x44 PAINT_ENGINE,
    0x45 PAINT_SPOILER, 0x46 PAINT_BRAKE, 0x47 PAINT_EXHAUST, 0x48
    PAINT_AUDIO, 0x4B PAINT_ROOF, 0x4C PAINT_MIRROR (names from 0x802DE8):
    the slot's part's RED, GREEN, BLUE, GLOSS (0x5BE050 → ARGB), or the
    body paint when the slot is empty, and for 0x48 also unless the audio
    part (slot 0x22, ride +0x618) is level 3 or more. The shared parts'
    `GLOBAL_SKIN1` maps to the skin, so aftermarket mirrors, roofs and
    spoilers take their paint from these texels. `car.rs`
    `composite_skin` (alpha left opaque).
  * **Trunk audio components** (verified; `car.rs` `AUDIO_COMP_*`):
    0x6350DC…0x6351EB looks up the twelve markers of table 0x8A4324
    (`MARKER_AUDIO_COMP_0` … `_11`, hashed at 0x77F0C0) on the audio
    part's (slot 0x22) group 0 LOD A model into render info +0x2184…
    (the 350Z's CUSTOM LAYOUT has 6, TUNED 4, STOCK none). 0x620AF0 draws
    them right after the trunk audio (0x630976…0x630D25), with the same
    trunk-scaled light context (harmonic × the trunk's value × 0.9, so 0
    here): for marker i, at marker × car, model group g = (audio part
    level > 1) (the `_PAINT` models), LOD A only, light material CARSKIN
    (0xD6D6080A) → render info +0x39C. In the front end (0x8654A4 = 3)
    or the in-game showcase (0x836352, set by the magazine photo screen
    0x4CE770, cleared by 0x499FA0) the model is component i's own part
    (slot 0x23 + i, ride +0x61C + 4i; render info +0x2274 + 0xC0·i +
    0x60·g, loaded 0x635440); otherwise the placeholder of its size,
    whatever is fitted (render info +0x2B74 + 0x18·(2·size + g),
    AUDIO_COMP_EMPTY_{SMALL,MEDIUM,LARGE}[_PAINT]_A, names 0x7A8B38…,
    loaded at 0x635729; sizes by i: S S M M L L S S S S M M, 0x630C42),
    and the draw flags get bit 0 (meaning not traced). +0x39C (0x63909B)
    is the light material (attribute 0x6BA02C05) of the slot 0x48 paint
    part when the audio part is level 3 or more, else the body paint
    +0x384 (`Car::audio_material`); the trunk audio itself maps
    0xD6D6080A → +0x384 and 0x5BE72F1C → +0x39C. The clone's
    showcase does not set 0x836352 (**open**), so it draws the parts in
    the front end and the placeholders in game; `game --shot <screen>_audio` fits the top
    layout and components to Rachel's car, NFSU2_AUDIO_ONLY=1 (or
    `comp`) draws only the trunk audio (or the components). The
    0x401E80(n) tests
    guarding the mount draws (2 for the spoiler, 1 for the roof and
    mirrors) and the matrices the stock mirrors use (stack +0x60 / +0x68,
    by the damage code) are not traced; nor is render info +0x394 (the
    exhaust tip's light material).
* **open / placeholders:**
  * Rachel's 350Z (DDAY_PLAYER_CAR) against the references: its vinyl ART
    FACTORY 6 has REMAP 0, so its purple/black dragon is drawn with the
    image's own colours (verified, 0x61D7A0); its decal slots 0x34–0x3E
    hold only the RECT MEDIUM placements and slots 0x61–0x98 no decals,
    (corrected: the preset fits slot 0x89 HKS USA and slot 0x91, the
    quarter-panel decals, drawn since the decal port) and its hood's
    `CARBONFIBRE` attribute makes it a carbon hood. **Open**: the
    windshield tint MEDIUM BLACK's look and the wide body's grey lip
    (no carbon part is fitted, so it is not the carbon remap);
  * **Carbon fibre** (**verified**, `car.rs` `carbon_for`,
    `Car::part_texture_for`): 0x636850 copies the remap table +0x638
    (0x49 entries; keys `%s_SKIN1`, `%s_SKIN1B`, `GLOBAL_SKIN1`,
    0xA7366AE6, 0x3C84D757 first, 0x638487) to +0xAC8 and +0xF58, sets
    entries 0–2 of both to the skin (+0x2D8), entry 3 (0xA7366AE6) of both
    and entry 4 (0x3C84D757) of +0xAC8 to `CARBONFIBRE` (0x7A8528, GLOBALB
    512² DXT1). In the 350Z's models 0x3C84D757 is the body panels' key
    (bumpers, hoods, body, doors) and 0xA7366AE6 the skirts' main panel
    and some hood trims. The part binding (0x634B0B…0x634E69, slot =
    [esp+0x20]) picks: bumpers 1 / 2 → +0xAC8 while KIT_CARBON (slot 0x5D,
    ride +0x704) is above level 0; wide body 6 → +0xAC8 with slot 6 and
    0x5D above level 0; hood 9 → +0xAC8 when the hood part's attribute
    0x721AFF7C (`CARBONFIBRE`) is non-zero; trunk 0xA → +0xAC8 with
    TRUNK_CARBON (0x60, +0x710) above level 0, else +0xF58 with 0x5D;
    doors 0x11 / 0x12 on a wide body → +0xAC8 with 0x5D; body 5, skirt
    0xB and the doors otherwise → +0xF58 with 0x5D; the rest the main
    table. Flags +0x3B0…+0x3C8 record the choice. They are read by the
    generic part draw (verified, docs/car-materials.md). Slots 1 / 2 / 9 /
    0xA / 6, 0x11, 0x12 with +0x3B0 / +0x3B4 / +0x3BC / +0x3C4 / +0x3C8 set
    remap CARSKIN to render info +0x388, the `CARBONFIBER` light material
    (0x638DA9, string 0x7A8D2C), at 0x6231B3. Every generic-draw model
    remaps 0xA7366AE6 to +0x388 while +0x3B8 (the kit) is set, else to the
    paint +0x384 (0x623451). The clone had kept the paint on carbon. The
    0x5E (hood) and 0x5F slots are not read here. With the kit on a wide
    body the whole body draws carbon.
  * **Decals** (**verified**, `car.rs` `decal_block`, `dummy_decal`):
    the decal location models (slots 0x34–0x3E, `%s_DECAL_HOOD_RECT_…`,
    `%s_WIDE2_DECAL_LEFT_DOOR_…`) are drawn by the generic part draw
    (slots 0–0x3E, 0x6244D5) with their ride +0x838 flags. Their meshes
    texture `DUMMY_DECAL1`…`8` (one material per decal position; the
    350Z's wide left door has six, its quarter one). 0x634AEC binds slot
    s's model to block s − 0x34 (− 4 for 0x3B–0x3E, so the wide slots
    share the plain ones' decals) of render info +0x13E8 (7 × 8 entries
    of 0x10). 0x61FF50 fills each block: keys `DUMMY_DECAL1…8`, targets
    `DEFAULTALPHA` (GLOBALB 32² DXT3, additive); then for placement slots
    0x34–0x3A (0x34 only with a hood part, ride +0x5B4) whose part has
    SIZE and SHAPE attributes, entry j takes the part in slot 0x61 +
    8·block + j: its NAME attribute (0x0019CBC0, a hash) continued with
    `_SQUARE`, `_RECT` or `_WIDE` by SHAPE (e.g. 5ZIGEN →
    `5ZIGEN_RECT`, CARS/TEXTURES.BIN, 256×64 DXT3 alpha). So decals are
    separate meshes, not composited into the skin. 0x631E30's last loop
    lists the same names for loading. The stock car (0x637050) fills no
    part into slots 0x40–0x43, 0x61–0x98 or 0x19 (the clone fell back to
    the level-0 decal of every slot). **Inferred**: a missing decal
    texture draws as `DEFAULTALPHA`. `game --shot decal_door` with
    `NFSU2_FIT=0x81,0x82` fits right-door decals (slots in hex);
  * the second model group: drawn when its solid exists; except slot 8
    (TOP: `ROOF` / `FULLROOF`), **verified** (2026-11-04): the
    generic draw's slot switch (0x623136, tables 0x630DA8 / 0x630D68)
    sends slot 8 to 0x62315B, which draws only group 1 (`FULLROOF`, the
    closed roof) when 0x60C860 accepts the car type (3, 5, 0x12, 0x17:
    240SX, CIVIC, RSX, G35) and the roof scoop (ride +0x5AC, slot 7) is
    above level 0, and only group 0 (`ROOF`, with the sunroof) otherwise
    (`car::top_group_drawn`; seen in the original's Body Shop on the G35).
    ENGINE / ENGINE_UNLIT is not traced;
  * prelit car meshes (TRUNK_UNDER) are not drawn: the car shader has no
    unlit path;
  * the frontend environment map in the garage scenes (the static cube
    from `FRONTEND\ENVMAPS\%s.BIN` faces XPOS…ZNEG, 0x5B9920, also run
    from 0x4A7316, 0x5800A5 and 0x5D1DCA; the clone loads it only for the
    magazine cover, and the car lot still uses its gradient cube);
  * the car's part animations (render info +0x2D44, 0x433CD0 state /
    0x434610 value per channel: 0 hood, 1 trunk, 2 / 3 left / right
    door) — **ported** (`formats::zam`, `part_anim`, `car::ANIMATED`),
    verified unless marked:
    * Data: `CARS\%s\PARTS_ANIMATIONS.BIN` (0x61C68B; the car's
      "PartsAnimations" object 0x438070 keeps its name hash at +0x24).
      Chunk 0xE34010 (0x4348F0) is an EAGL4 animation bank, an ELF32
      object whose `__AnimationBank:::bank` (+0x04 count, +0x10 / +0x14
      animation and name pointers) holds `<CAR>_ZAM_<NAME>_q` (rotation
      keys x, y, z, w) and `_t` (translation keys, mostly none) channels:
      +0x0A u16 frames, +0x0C the key block (+0x08 components, keys from
      +0x20, a key per frame). Chunk 0x37100 (0x4381CE): from +0x10, 21
      rest matrices in 0x433F40's order (hood REG, FRONT, the four
      SPLITs; left door REG / SCISSOR / SUICIDE; right ditto; trunk; left
      WIDE1–4; right WIDE1–4). 0x433AD0 finds an animation by
      bin_hash(`<CAR>` + `_ZAM_…`). On the 240SX HOOD_REG turns −46° about
      y at the hinge x 0.856, z 0.623 (its front up), TRUNK_REG +53°.
    * Playback (0x433E20 / 0x433D50 / 0x434190): a record per channel
      (+0x08 speed, +0x10 state 0 open, 1 shut, 2 opening, 3 closing,
      +0x14 variant, +0x18 start, the 4000 Hz tick × 0.00025); open on a
      shut channel and close on an open one start from now, a reversal
      mirrors the elapsed time; a channel without a record gets one,
      opening (0x433A80). The draw takes t = elapsed (opening), L −
      elapsed (closing), L or 0, L = length ÷ speed ÷ 30, and settles the
      state at L; frame = 30 · speed · t; value = frame ÷ length.
      Native length is the channel header's frame count (0x434AC4 →
      0x6E2110), including the terminal frame. Sampling clamps at the
      final key and uses normalized linear quaternion interpolation
      (0x6E7300 → 0x6E4450), copying exact keys without normalization.
      See [authored RX8 evidence](../evidence/fe-authored-animation/README.md).
    * Draw (0x620AF0's generic part draw, 0x62345B…0x6243E5): the
      slot·2 + group byte of GLOBALB chunk 0x34608 (0x636C69 → 0x8A1CDC)
      names the channel (hood: slots 9, 0x19, 0x34; trunk: 0xA, 0x1A,
      0xF group 1; doors: 0x11–0x14, 0x37 / 0x38, 0x3B / 0x3C); while the
      channel is not shut the model is posed T(a − p) · A · [R⁻¹] · T(p −
      r) in car space (row-vector products and 0x5BAEA0 translations
      there): A the animation's turn, R its rest matrix, a its translation
      or R's (0x434190), p the solid's +0x70 (0x488E80); R⁻¹ for the hood
      and the doors' SCISSOR / WIDE variants (0x623A7A, 0x434550). The
      engine (slot 0xD) draws only while the hood is not shut, its group 0
      lit by the harmonic × hood value × 0.6; the trunk audio's group 0 by
      × trunk value × 0.9 (below).
    * Animated mesh bias order: the native copy/inverse sequence yields
      B · P · B⁻¹ · M in column notation, where B is the original base,
      M its copied mesh with world z +0.01, and P the part pose above.
      Thus the copied displacement rotates with an opened part. See
      [the original-instruction fixture](../evidence/fe-open-part/README.md).
    * Who opens them: the garage camera for slot 0x44 (0x4C1D40 opens
      the hood, GarageMain +0x160 / +0x15C, and shuts it on the next
      view); UI_Paint on Engine Accents (0x55CC60 at 0x55CD02; 0x55CE85
      closes); the neon lists, hood for 0x9B and trunk for 0x9D
      (0x50F290 from 0x553580 / 0x5571A2, closed from 0x562B8C; the
      clone closes on leaving the list, **inferred**); the TRUNK AUDIO
      screens (0x5593F0 cases 2–5 open the trunk, case 0 closes the
      hood); all through 0x4C1860(channel, open, speed) (a close only
      when not shut, 0x4A5B20; the gate 0x4A5B10 → 0x6106E0 is not
      traced) → 0x61F830. `game --shot paint_engine` (the hood up, the
      engine lit) and `--shot audio` (the hatch up); NFSU2_SHOT_MS under
      a second shows them moving.
    * Ported 2026-11-04 ([shop-part-animations.md](../shop-part-animations.md)):
      the split hood and door variants (0x61F830; ANIMSTYLE, WIDE<kind>),
      their FE sounds (0x4C / 0x4F for ANIMSTYLE 1, else 0x50 / 0x4E), the
      spoiler's group 0 on the trunk's posed matrix and the mirrors on the
      doors' (0x6243F4, 0x625AE1, 0x626515 / 0x626A86 / 0x626FA6), slot
      0x36 on the trunk (0x60C900) and slot 0xE on the hood for HUMMER
      (type 8; group 1 with a VERTSPLIT hood), the engine / audio level-3
      glow on c0 (+0x364 × 0.6, +0x37C × 1.5), the components lit by the
      audio's group 0 context, the Doors / Split Hoods lists shutting at
      once on a step or Back (Wine).
    * **Open**: the split hood's left half is black in the original and
      painted in the clone (same silhouette); which screen opens the
      doors in game (0x521050 closes only; 0x523CC0, 0x563BC6, 0x56A5D1,
      0x56A7AC close) (the in-game showcase's Show Off, 0x4B5AD0, is
      ported: see **UI_ShowCase_Preview**); whether animated parts take a
      light context in their own axes (the clone keeps the car's);
  * what the bright yellow-white patch on the player car's trunk in
    `--shot explore` is lit by: it is the specular term (harmonic of the
    reflection vector), not a part drawn with the wrong context (the
    patch stays with diffuse and specular kept and the env map removed,
    and goes with only the diffuse); which light of the context makes it,
    and whether the real game's dimmer highlight is the same term, is
    not traced; (docs/car-scene-specular.md: the rig 0x7F3F00's
    camera-fixed key light, computed from the geometry; the enhanced
    car shading in the higher Enhanced settings and with ray tracing takes
    the specular channel from the scene's lights; Original and the lower
    settings keep it native);
  * which cars get the trunk audio in game: 0x620AF0 returns before it
    (and the rest of the draw) when the car object's (render info [0])
    +0x14 → +4 is not 1 (**not traced**: probably the human player); the
    clone draws it for every car.
* **Part light contexts** (verified, 0x620AF0 at 0x6304D4..0x630700):
  * Engine, slot 0xD (render info +0x1948 = +0x17A8 + 0xD·0x20, groups 0
    and 1): drawn only while animation channel 0 (the hood) is not idle
    (0x433CD0(+0x2D44 → +8 → +0xD0, 0) ≠ 1; +0x2D44 = 0 counts as idle).
    Group 0 takes a 0x1C0-byte copy of the light context with the ten
    HarmonicCoeff vectors (+0x120…) × v·0.6 (v = 0x434610(…, 0, 0), the
    channel's value) and, when the slot 0xD part is level 3 (part byte
    +5 & 0xE0 = 0x60), c0.xyz += (R, G, B)/256 · A/256 · v·0.6 from render
    info bytes +0x364..+0x367; group 1 the main context. Light material
    0xD6D6080A → render info +0x384 (0x48D860). The clone draws it so
    (`CarPart::hood_gate`, `harmonic_anim`), without the level-3 c0 term
    (**open**).
  * Trunk audio, slot 0x22 (ride +0x618; its two groups' models at render
    info +0x21B4, 4 LODs × 0x18 bytes each, filled at 0x635201): group 0
    takes the copy with the harmonic × v·0.9 (channel 1, the trunk) and,
    for a level-3 audio part, c0.xyz += bytes +0x37C..+0x37F (R, G, B,
    intensity) · v·1.5; group 1 the main context; light materials
    0xD6D6080A → +0x384 and 0x5BE72F1C → +0x39C (level 3) or +0x384. The
    clone scales group 0's harmonic by the trunk's value × 0.9
    (`CarPart::harmonic_anim`; 0 with the trunk shut, so only the env map
    lights it then), without the level-3 c0 term (**open**).
* **Car shadow** (verified; src/shadow.rs, `WorldRenderer::render_shadows`):
  * Texture: render info +0x33C = `%s_SHADOWFE` (0x7A8D44) when 0x8654A4
    = 3 (front end) or the reward-screen flag 0x838578 (0x510C30) is set,
    else `%s_SHADOWIG` (0x7A8D50), through 0x4901D0 (0x6387DE..0x638840).
    The constructor sets the texture's +0x54 = 0 and rendering order +0x51
    = 2 (texture 0xBADB4475 at +0x340 gets 3, `%s_NEON` (0x7A88F8) at
    +0x344 gets 4, 0x6388A4..0x6388EB; `NEONBLUR` (0x7A8D38) goes to
    +0x348). Traffic cars' are in CARS\TEXTURES.BIN.
  * Drawn by 0x6162F0 when g_CarShadowEnable (0x870CC0, registry option
    string 0x7A3874; quality presets 0x5BE880.. set it, 2 at the top
    tiers) is non-zero, after the car passed the size (≥ 4) and box tests.
  * Box: render info +0x10 / +0x20 (0x634810): the union of the solid
    header boxes (+0x20 / +0x30, 0x48D7E0) of the LOD A model of groups 0
    and 1 of each fitted slot 0–7, 9–0xC, 0x2F–0x33 (jump table 0x63681C;
    0x23–0x2E skipped), from an empty box (0x43D510), grown by 0.1 m
    (0x784358) each side (`Car::shadow_box`).
  * Height t: in game (0x8654A4 ≠ 3) the car's height above the ground at
    its position (0x5E7E80) over 0..3 m (0x7A6668 / 0x7A666C): h/3 for
    0 < h < 3, 1 above, else 0; 0 in the front end; a non-zero 0x8A1BE8
    replaces it (**not traced**).
  * Grid: 4×4 vertices, x from min to max of the box × s × 1.05 (game,
    0x7A66A0) / 1.2 (FE, 0x7A66AC), y × s × 1.05 (0x7A66A4) / 1.14
    (0x7A66A8), s = 1 + 0.75t (0x7A6670), steps of 1/3 (0x786348); world
    x, y = position + x·row0.xy + y·row1.xy (the z row × 0); UV = (column/3,
    row/3).
  * Ground (game, when render info [0] has its context at +0x5C4): the
    four wheels (car type table, 0x5B5360 + 0x10 + k·0x30) to world, each
    queried for height, normal and hit (0x5E72F0; a miss gives the point's
    z and (0, 0, −1)), the normal scaled by −1/n.z. Each vertex is queried
    at the car's z; a miss takes the plane of its corner wheel (table
    2,2,1,1, 2,2,1,1, 3,3,0,0, 3,3,0,0) if that wheel hit; the four inner
    vertices (((k>>1)^k)&5 = 5) are raised 0.08 (0x7A66B4; FE 0, 0x7A66B8);
    a vertex is kept if it hit, is within 4 m of the ground at the car
    (0x60CF80) and within 1.5 m of that ground's plane (0x60CFA0), else it
    takes the last kept height (starting at the car's z) and counts a miss
    for its row; then + 0.04 (0x7A66BC). Otherwise (FE) every vertex is at
    0.152 (0x7A66C4) plus the inner raise.
  * Colour 0x808080 | a<<24, a = trunc(2·(base + (top − base)·(1 − t)·|row2.z|))
    clamped 0..0xFE (0x75BED4), base/top 0/0x72 in game, 0x80/0x80 in FE.
    Nothing is drawn with a = 0 or no texture.
  * Draw: strip j (rows j, j+1; 8 vertices) unless rows j and j+1 missed 4
    or more times together; 0x5BE190 (texture, 8, matrix), 0x5BE140 /
    0x5BE0B0 / 0x5BE100 per vertex, 0x5CEC50 queues it (0x5CEA40) under
    effect 0x86EB5C = 13, worldnodepth, whose vertex shader writes no fog
    and whose pass writes colour without alpha (COLORWRITEENABLE 7 against
    world's 15, by the Wine state table order **inferred**). The flush
    draws strips (+0x0C = 1) as DrawPrimitiveUP triangle strips with
    ZWRITEENABLE off (0x5CF822 / 0x5CF89C), depth test as set. The matrix
    (+0x118) is T(−pos)·T(pos), the identity, so the vertices are world
    positions. The clone draws them after the cars with the texture's
    blend and the no-fog pipeline. In SHOWROOM the FE grid at 0.152 m is
    under the disc tops (0.159 m) and does not show, which the clone
    reproduces.
  * **Pulled toward the eye (verified, 2026-10-29, fx70;
    `Shadow::toward_eye`).** Car_Draw (0x620AF0) builds the shadow's
    matrix [esp+0x178] as 0x401340(out, 0x872E90 identity, [ebp+0x10]) and
    then 0x48AE30(out, s) with the view in ecx: a uniform scale by `s`
    about the view's eye (view +0x40 → camera +0x40), translation
    included. `s = (d − 0.104) / d` (0x62133C..0x621365: 1.0 0x784250,
    0.104 0x7A66CC), `d` the eye-to-car distance (0x43CE10 at 0x621277,
    stored at [esp+0x60]); 0.023 (0x7A66D0) replaces 0.104 when
    g_RaceCameraDirector (0x8900B4) is set and its +0x238 object exists
    with +0x0C ≠ 1. **Resolved (2026-10-29):** 0x8900B4 is the
    first entry of the player list (0x8900B4[i], count 0x8900BC, filled by
    the player constructor 0x5F1680; 0x8900AC[i] by index), not a camera
    director; player +0x238 is the player's camera mover, +0x0C the type
    Camera_Factory (0x5F1FE0) built (type 0 leaves +0x238 null). So the
    near pull applies while player 0's camera is not the chase camera
    (type 1): look-back / nearby-object views 0x0D / 0x0E (keys 0x7F /
    0x80, 0x602BE0, 0x6022F0), the jump cameras 0x0F / 0x10 (racer +0x58 =
    6 / 7, 0x604750), 0x11 (input mode 5, 0x605C80), track cameras 0x12,
    the target camera 0x13 (0x5FABD0), the showcase 0x14, the dyno 0x15
    (0x600600). In the front end (no player) and under an NIS (type 0, no
    mover) it is 0.104. 0x631738 sets 0x8A1C18 = (mover type == 1) from the
    same test. Clone: `shadow::shadow_pull`, `camera_mover` (showcase,
    replay track shot, else chase). The same matrix goes to the underglow pass and the hull
    block (0x621F0F). The scale moves nothing on screen; it is a depth
    bias that lifts the FE grid (0.152 m) over the disc tops (0.159 m).
    **Corrected:** the note above that the FE shadow "does not show" in
    SHOWROOM was wrong — apitrace t1 call 10645088 (Quick Race car
    select) has the shadow's WorldViewProj equal to the view's scaled by
    0.98903 = (9.48 − 0.104) / 9.48 in x, y, w and depth (the eye 9.48 m
    from disc 0), and the original's pre-post frame shows the dark grid
    under the car. The clone now draws it there.
  * The clone draws the shadows (and underglows) into the scene target
    before the end-of-frame passes; they went to the swap chain's view
    before, which `PostFx` then overwrote, so no shadow showed.
  * **open**: whether the ground context at render info [0] + 0x5C4 is
    the physics' (the clone queries a copy of the car's own); the draw
    order against blended car parts of rendering order ≥ 2 (the clone
    draws all shadows after all cars). Reward covers now use their native
    bucket split and RGB-only SHADOWFE pass (2026-11-03; see
    `docs/evidence/cover-shadow/README.md`).
  * **open** (not drawn): the block at 0x621958..0x622113, run when
    g_CarShadowEnable > 1 (and 0x8026B8, 0x802918 set, render info +0x340
    loaded, the car not in the front end, 0x60D4E0 false): it is not the
    neon. It projects the points of the model at render info [0] + 0x3C
    (vtable +0x7C(0), count at +0x14; 0x5AAD30 per point, z + 0.275,
    0x7A6664) onto the ground along 0x8A1EF0 / 0x8A1EF4 by height, takes
    their hull (0x60D010 at ground + 0.06, 0x60D1C0), and draws a fan
    from its centre plus a feathered rim (0x60D4A0; v 0 → 1, u 0.155,
    0x7A6658) with texture 0xBADB4475 (+0x340, no name found in the
    data) in 0x808080 | a << 24, a from the height t (55 − 55t,
    0x7A65B4 / 0x7A88C0; when the draw's [esp+0x6C] is 2 also ×
    ([esp+0x64] − 20) / (30 − 20), 0x7A66EC / 0x7A66E8), and 0x8026BC =
    1.4 − 0.3t (0x7A6660 / 0x7A665C). What 0x8A1EF0/F4,
    0x8026B8, 0x802918 and 0x60D4E0 are is not traced.
* **Car neon** (verified; `shadow::Neon`, `shadow::neon_color`,
  `Car::neon`, `car_shadow_passes` in game.rs):
  * Colours (0x635B7C → 0x61FC30 per part): render info +0x34C/+0x350
    from the underglow (slot 0x9A, ride +0x7F8), +0x35C/+0x360 engine
    (0x9B, +0x7FC), +0x368/+0x36C cabin (0x9C, +0x800), +0x374/+0x378
    trunk (0x9D, +0x804). The first dword is the part's RED, GREEN, BLUE,
    ALPHA attributes (low bytes, 0x60FF00), the second RED2 − RED …
    ALPHA2 − ALPHA as bytes (read back unsigned); zeros without a part.
    The part data: NO NEON level 0 all zero; colours level 1–2 ALPHA =
    ALPHA2 = 64 (steady); PULSE parts level 3 ALPHA 16 → ALPHA2 64, same
    RGB (`devprobes/src/bin/neon_parts.rs`). Rachel's D-Day preset has NO NEON in
    0x9A–0x9D; CALEB_GTO (RED PULSE), NIKKI_MUSTANGGT, TOM_G35, AL_RX8,
    the DEMO_PRESETs and others have neon.
  * Per frame (0x6227DE..0x622D35), only when a part in 0x9A, 0x9B or
    0x9D is above level 0 (part byte +5 >> 5): s = tick × 0.00025
    (0x784268; 0x865188 in game, 0x8651A8 when 0x8654A4 = 3; 1.0 on the
    reward screen, 0x510C30); a, b, c = 6.1, 7.2, 4.7 (0x7A6674..7C) × s,
    each minus trunc(x / 2π)·2π; sines by 0x43C350 (16-bit binary
    angle); p = trunc(1.2 · |sin a| · |sin b|) (0x7A6680); each channel
    k = clamp(trunc(delta_k · |sin c|) + base_k, 0, 0xFE), alpha + p.
    +0x358 = that colour; +0x354 = the same RGB with alpha
    min(trunc(A · clamp(up.z, 0, 1) · (1 − t)) · 4, 0xFF) (t the
    shadow's height fraction, 0x6226DE); the engine's (+0x364) and
    trunk's (+0x37C) the same way (alpha + p, no ×4 step) when those
    parts are fitted (**not ported**: they light the engine and trunk
    audio models, which need the part animations). Without the level
    gate +0x354 keeps its last value (0 from the constructor, 0x6387BB).
  * Underglow (0x622D3F..0x622F93): the shadow grid's loop runs twice,
    pass 0 the shadow (+0x33C, its colour), pass 1 the same 16 vertices,
    UVs and strip skips with `%s_NEON` (+0x344) in +0x354, skipped when
    the texture is missing or the alpha byte is 0; colours go through
    0x5BE0B0 (RED to the D3DCOLOR's red). In game and in the front end
    alike (inside g_CarShadowEnable ≠ 0). The cars' `%s_NEON` are
    Additive (350Z_NEON 128×64 DXT3: glow bars along the sides and arcs
    at the ends). `--shot neon_car` fits a slot 0x9A part to Rachel's car
    by name (NFSU2_NEON, default MAGENTA); `--shot neon` with NFSU2_NEON
    set does the same in the specialties garage.
  * **Neon tubes** (verified in the disassembly, ported 2026-10-31,
    <work dir>; `shadow::neon_tube_quads`, `shadow::neon_tube_color`,
    `Car::neon_tubes`, `neon_tubes` / `tube_view` in game.rs). The bright
    bars along the sills are not the shadow grid's underglow but these:
    * Markers (0x6357A0..0x635901, at the car's build): +0x2C14 = 0, then
      for k = 0..2 the model of slot (1, 0xB, 2)[k] (front bumper, skirts,
      rear bumper; all three slot 6 when the wide body, ride +0x5A8, is
      above level 0), render info +0x17A8 + slot·0x20 → +0x0C (LOD A
      group 0), and for i = 0..2 the markers `FRONT_NEON_` /
      `SIDE_NEON_` / `REAR_NEON_` (0x7A8C90 / 80 / 70) + i + `START` /
      `END` (0x7A14F4 / 0x7A51C4) by bin hash (0x43DB50, 0x488FA0); a
      pair is kept when both exist (+0x2C18, 8 bytes each, up to 9),
      +0x2C14 = 2 × pairs. The engine pairs ENGINE_NEON_START_%d /
      ENGINE_NEON_END_%d go to +0x2C64 / +0x2C60 (0x635B76).
    * Draw: the glow pass calls 0x61ACA0 last (0x5CB629; 0x5CBC93 into
      the streak buffer). With the glow off the front end still calls it
      (0x5CB633: 0x8654A4 ≠ 6), the game does not. Effect 13
      (worldnodepth), ZENABLE 1, ZWRITEENABLE 0, COLORWRITEENABLE 7 as the
      other glows. 0x61ACA0 walks the list 0x8A30F0 (node +0x08 the render
      info, skipped when 0 or node +0xB64 = 0) and calls 0x615A20(view,
      node +0x910, node +0x920). **Inferred**: those are the car's drawn
      position and rotation, and the list holds every drawn car (the
      clone: the player, rivals, encounter racers; the front end's car).
    * 0x615A20: with a part in slot 0x9A (ride +0x7F8, any level)
      0x60D800(view, pos, rot, +0x2C14, +0x2C18, +0x358, 0.045 (0x7A673C),
      1, 1.0). With a slot 0x9B part and the hood channel (0x433CD0(anim,
      0)) not in state 1, twice for the engine pairs: colour +0x364, width
      0.0175 (0x7A6740), 0, fade 0x434610(anim, 0, 0); then colour alpha
      ftol(A · 0.3) (0x7A6604), width 0.175 (×10, 0x7A6600) (**not
      ported**: the engine neon, the trunk (+0x800 onward) and cabin
      tubes).
    * 0x60D800(view, pos, rot, count, pairs, colour, width, shorten,
      fade): 0x40 bytes of the quad buffer (0x82DA5C / 0x82DA60) or
      nothing; texture render info +0x348 (NEONBLUR, GLOBALB 64×64 DXT3
      Additive). d = |pos − camera +0x40|, f = (13018 / camera +0xC4)²
      (0x7A6738; +0xC4 the field of view as a binary angle); d ≤ 10f
      (0x7A674C): k = L = 1; 10f < d < 75f (0x7A6750): r = (d − 10f) /
      65f, k = 20r + 1 − r (0x7A6754), L = 1 − r (the 1.0 left on the FPU
      stack); else k = 1, L = 0 (0x7D7D3C). w = k · width; a =
      ftol(colour.A · L · fade), nothing when 0, alpha min(2a, 0xFE). Per
      pair i: A, B = the START / END markers' translations (0x441000 on
      the zero point) under rot, plus pos; when `shorten` and i = 1 both
      ends move 6 % of B − A inward. S = the unit screen direction from
      A to B (0x5BC4B0 pixels, y down; 0x43CE50, (1, 0) for zero). With r
      = camera column 0 (right) · w and u = column 1 (down) · w: n = r·S.y
      − u·S.x, t = r·S.x + u·S.y; quads (0x5C7E90) [A, B, B − n, A − n]
      and [A, B, B + n, A + n], UVs (½, ½), (½, ½), (½, 0), (½, 0); caps
      [A − n, A − n − t, A + n − t, A + n] UVs (½, 0), (0, 0), (0, 1),
      (½, 1) and [B − n, B − n + t, B + n + t, B + n] UVs (½, 0), (1, 0),
      (1, 1), (½, 1); every vertex colour bytes (+0x358 bytes 0, 1, 2,
      alpha), which 0x5C5590 swaps so byte 0 (RED) is the red.
    * +0x358 is the pulsed colour before the underglow's ×4 alpha step
      (`neon_tube_color`); 0 while no neon part is lit, so nothing.
    * The clone draws them with the glow sprites in game (`glow_quads`)
      and, in the front end, into the scene after the shadow passes
      (**inferred**: the front end's glow buffer). The player's report "no
      car has an underglow in the garage" (the finished-career save: Corolla slot 0 WHITE
      PULSE, Skyline slot 4 YELLOW; `devprobes/src/bin/save_neon.rs`) was these
      tubes missing: the shadow-grid underglow was drawn.
    * `--shot garage_neon`: the crib's Car Select on the fixture
      the finished-career save (NFSU2_SHOT_SAVE=file for another save,
      NFSU2_SHOT_SLOT=i for another career slot, default 0).
  * Checked against apitraces (2026-11-01): the underglow pass is
    the shadow grid's 3 strips again (DrawPrimitiveUP, 8 vertices each,
    SRCALPHA / ONE, ZWRITE off) with `%s_NEON`; a community save's Corsa in the
    garage has vertex colour 0xFC800000 (the clone 0xFF800000: the
    original's fade is just under 1), DarkNeS' Skyline (a PULSE part,
    ALPHA 16 → 64) in free roam 0xB4E61ABD. With the alpha set to the
    original frame's 180 the clone's underglow matches it; its shot had
    caught the pulse at alpha 60, which is the faint line reported. The
    pink ground in the reported free-roam pair is the road texture at the
    original's spawn, not neon (the clone shows it at the same pose).
  * Clone deviation: Car Shadow / Neon Ultra uploads `%s_NEON` rebuilt (4×, deblocked, border faded
    to 0 against the native masks' lit border, the "cutoff line"; half float),
    docs/neon-quality.md; Original is unchanged.
  * **open**: where the +0x2C18 marker pairs' node fields +0x910 / +0x920
    are set; the cabin neon (0x9C/0x9E) draw; the clock the clone uses
    (its run time in game, the garage's time in the front end); the
    original's rims lit by the neon in the garage (not a traced effect).
* **Horizon fog**: see **Horizon fog** under Screen filters (ported,
  `nfsu2::horizon_fog`, `PostFx::horizon_fog`).

## Handling (verified except where noted)

Units are tonnes, metres and seconds, so forces are kN and torques kN·m. The
240SX's torque curve peaks at 0.216, the real KA24DE's 216 N·m; its crank
inertia, 0.00025 t·m², is 0.25 kg·m².

**Floating point (measured):** the game's main thread runs with
the x87 control word **0x007F** (gdb `$fctrl` at every game-code stop: 0x5A141D,
0x594728, 0x41347C, 0x5B5CB8, and in Wine code): precision control 24 bits
(single), round to nearest, every exception masked — Direct3D's default FPU
set-up (no FPU_PRESERVE). So every x87 operation rounds its mantissa to 24 bits
as an f32 operation does; the clone's f32 (SSE) arithmetic gives the same
results except where the x87 differs from IEEE single: the 80-bit register's
wider exponent range (no overflow / underflow / denormal at the f32 limits
inside a chain of operations) and values loaded exactly (`fild` of an int
above 2^24, `fld` of a double) before the first rounding. For comparisons
against the original: compare f32 bit patterns where the native chain is f32
operations (e.g. `standings::round_centi`: 60 000 ticks × 0.00025 is
15.000001, not 15.0); do not reproduce a computation in f64 and round once at
the end, which misses the rounding the x87 applies at each step.

**The race start's grid stance (measured, r3: the player's 206,
slot 0):** during the start scene the car drives onto the grid under the scene
(pos.x −51.66 → −35.004 by frame ~140) and settles with its normal motion (vel.z
9.8e-3 → ~1e-5 by frame 178). From the countdown's first frame the velocity is
exactly 0 and the compressions FL/FR/RR/RL = −0.000349462 / −0.000262946 /
0.000505954 / 0.000419319 stay bit-constant through the countdown (the rest
test, class 2, holds the car), changing in the 1e-7 range only after the
green: the stance is the scene's own settle, not a countdown function. The
clone does the same (**measured**, `--flow firstrace` / `firstdrag` at 60 Hz,
the player's car per world update): its car settles in the scene's drive-in,
and from the first update at or past the scene's cut (race status 3) its
velocity is exactly 0 and its four compressions bit-constant until the green,
with brake 1.0 — the first race and its Restart (cut 180), the first drag,
its 5 s start run and its Restart (cuts 180 / 200). Test
`first_race::tests::the_player_stands_still_through_the_countdown`.
**Against the original on the same event and slot (observed)**: on the pinned
STAGE_1_CIRCUIT_1 grid (`tools/parity/scenarios/circuit_s1_stance.toml`,
`circuit_s1_launch.toml`, per-wheel fields `comp_*`, `crate_*`, `sforce_*`,
`gnorm_*`, `rest`; table by `tools/parity/stance.py`) the clone's compressions,
body pose and rest class lie inside the original's own run-to-run spread
(about 3 µm: FL/FR/RR/RL −0.4292 / −0.2058 / +0.5910 / +0.3676 mm and
−0.4266 / −0.2064 / +0.5884 / +0.3681 mm in two runs, the clone −0.4247 /
−0.2064 / +0.5847 / +0.3665 mm). Where it comes from: through the start scene
the car is on the NIS mover (mover +0x60 counts the scene's placements); on the
countdown's first frame the physics mover measures the suspensions (0x5A87A0)
at the scene's last pose and the rest test (0x5992C0, class 2) holds it at
once — the "uneven" stance is the scene's last pose, not a settle (the
hand-over's code address is not traced). The earlier 1.6 mm reading of the
clone's first race was another grid slot. Putting the original's exact stance
on the clone (`PARITY_STANCE=1`) leaves the launch's 2 s / 3 s speeds
unchanged: the launch residual was the AI's look-ahead point measured from the
body origin instead of car +0x60 (0x403AF0 / 0x403BA0 / 0x415630 / 0x4054D0;
`ai::driver::car::centre_of_mass`): 9.54 / 14.11 → 9.43 / 14.02 m/s against the
original's 9.44 / 14.02 (docs/evidence/grid-stance/). **Open**: the original's
wheel load +0x138 reads 3.7 % over the suspension force +0x24 through the
countdown (3.443 / 3.318 kN at the front; equal from the green's first update,
where the clone's is throughout); the straight-ahead direction 0x403AF0 takes
the car's 16-bit heading +0x40C, which reads in steps of 32 in the original
(its per-frame writer not traced; the clone uses the forward axis); the first
drag's grid (the clone's place 0.43 m off the original's).
(The race-start brake: see the gamepad section's input block.)

Physics classes were named from their replay-snapshot functions, which open
with a profiler scope `0x578CF0(0, "Name")`: Engine 0x594230, DriveTrain
0x5944B0, Wheel 0x59C990, Suspension 0x59E030, RigidBody 0x59B700,
TwoWheelAckermanSteering 0x59DEB0, PhysicsMover 0x5A47C0. PhysicsMover holds
suspensions at +0x28..+0x34, wheels at +0x38..+0x44, the engine at +0x48, the
drivetrain at +0x4C and the tuning block at +0x54.

### The step

* **Frame** (mover update 0x5B72E0): the gear choice (0x5ABC00), the brake
  and grip set-up (0x5A4D90) and the yaw limit (0x5A49B0) run once. Then the
  frame splits into ⌊dt / 4.4 ms⌋ + 1 equal sub-steps (0x7FC070). Each is one
  force pass (0x5B0530 → 0x5ABE00) and an explicit Euler step of every
  state (0x5AA490): all derivatives are taken first, then each object adds
  dt × its derivative and runs its tidy-up.
* **Orientation step** (verified): the rigid body's rows
  (+0x30/+0x40/+0x50, forward/left/up in world space) take a plain Euler
  step, row += h·(ω × row) with the sub-step's starting ω (0x59C130 inside
  the derivative 0x59C1F0, added by 0x5AA490 as `s + h·d`), not a
  rotation. The tidy-up 0x59C600 then takes ω = R·I⁻¹·Rᵀ·L (0x43C9D0,
  0x43C990 → 0x4013F0, 0x441000; held to 250) from these rows as they
  stand, and only after 0x5AA490 returns does the mover re-orthonormalise
  them with 0x5BB0E0 (0x5B7455–0x5B745D; x = normalise(row0), z =
  normalise(x × row1), y = z × x; 0x43CF10 gives (1, 0, 0) for a zero
  vector). The MeloMvr (0x5AB091/0x5AB09E) and the NIS mover
  (0x5AB51F/0x5AB52C) do the same. Bit for bit against 262 of the
  original's own sub-steps (subtrap A/B captures, every float operation
  in the original's order, h = t/1800/4 for the frame's t ticks):
  `drive::rigid::native_integrator_replay`,
  docs/evidence/native-integrator/. The clone keeps the exact rows between
  sub-steps (`CarState::body_rows`). The force pass natively runs in the
  post-step (0x5B04E0 → vt+8 = 0x5B0530); the clone's sub-step now takes it
  there too (see **Order of a frame and a sub-step** below), but on the
  cleaned rows: natively it sees the un-cleaned ones, before 0x5BB0E0
  (open, a rounding of order (ωh)²); the inverse inertia is the clone's 1/I
  (the original's +0x170/+0x1B0 differ from it by a few ulps in y).
* **Order of a frame and a sub-step** (**verified**: 0x5B72E0, 0x5AA490,
  0x5A0D30, 0x5B04E0, 0x5B0530, 0x5ABE00; the drivetrain functions below).
  The frame: 0x5AB9A0 (preload), 0x5A49B0 (yaw limit, the wall drive's rear
  share +0xEC), 0x5ABC00 (pedals, gear choice 0x5AA970, the engine's
  throttle +0x78, nitrous), 0x5A4D90 (brakes, grips), 0x5B6310 (wheel hop:
  its mode, car +0x46C, fixes the left/right split below), then N sub-steps
  of 0x5AA490 and the re-orthonormalisation 0x5BB0E0, then the export
  0x5A5550. One sub-step:
  1. **Derivatives** of every component first (0x5A0D30 → each vt+0x18),
     from the state and from what the last force pass stored: the wheel
     (0x59D360) averages its net torque, (+0x108 − r·+0x124 + +0x120) / 2,
     takes 0.002·ω rolling resistance off, zeroes a locked wheel's rate and
     takes a gripping wheel's slip +0x118 out within the step; the engine
     (0x5A12A0) is (0x594310's torque at the throttle and power as they
     stand + the stored reaction +0x7C) / inertia (data +0xC; the factor
     0x7A1928 for clutch states 0 and 2 is 1.0); the clutch (0x5949C0) the
     gearbox rate × +0xE8 × the drag grade × (−3, 0, 1.5, 0) by state; the
     rigid body (0x59C1F0) the stored force and torque sums.
  2. **Euler**: each state += h × its derivative (0x5AA490's loop).
  3. **Post-step** 0x5B04E0: every component's tidy-up (vt+0x1C, in list
     order: the engine 0x5AA770 clamps idle..max, the fuel cut, clears
     +0x7C; the clutch 0x5A1410 refreshes the input shaft +0x4C from the
     wheels (0x594600) and runs its four states; the suspensions measure
     0x5A87A0; the rigid body 0x59C600), then 0x5ABB10 (the rest test
     0x5A5E30, the bump stops 0x59E1B0, a zero-time refresh 0x5A0D90 when
     either acts), then the **force pass** vt+8 = 0x5B0530 for the next
     sub-step: aero 0x59E630, camber loads 0x59D170, then 0x5ABE00 —
     the clutch's torque 0x594CC0 (locked: the engine's 0x594310; slipping:
     coupling × engagement × (engine − +0x4C) held to ±300 rad/s; open:
     none) sent through 0x594A40 (the crank feels −T × max(down/4, 1/4) in
     +0x7C; each axle's share by +0xEC; left/right by the spins, below), the
     four tyres 0x5A8290, five rolling-constraint passes (Jacobi: the car's
     acceleration is taken once per pass, then every gripping, unlocked
     wheel's +0x124 moves; each pass ends with 0x594DB0, the locked clutch's
     extra crank torque through 0x594A40), the friction circles 0x59D610
     (wheel order), and 0x5AA840 (locked, in gear, a wheel down: under
     power with the driven slip +0x16C over 0.1 the input shaft +0x4C and
     the driven wheels are set to the engine, 0x5A12E0; else the coupling's
     pull on +0x7C; the 1st-gear relief); then the spin damping 0x5A5B70,
     the drift torque 0x5ECF80, the hydraulics, the held spring.
  So a frame's first sub-step integrates forces taken at the end of the
  previous frame, before this frame's pedals, brakes, steering, rear share
  and hop mode; only the engine reads the new throttle at once (its
  derivative evaluates 0x594310 itself). And the state a frame leaves (what
  the per-frame readers and a memory read see) is the one after that last
  force pass: the slip, the loads, the torques +0x108, the reaction +0x7C,
  and at a lock-up the input shaft and the driven wheels already set to the
  engine. The clone's sub-step had the force pass at its start
  (`CarState::substep` before this change): every input acted one sub-step
  early, and its frame-end state was a pass behind. Ported: the sub-step
  integrates the stored pass (`StoredForces`) and ends with the next one;
  a car with no pass, or moved since it was taken (a teleport, the harness's
  transplant), takes one first, and after a contact the re-step takes a
  fresh one (the refresh 0x593710 → 0x5B04E0 runs the post-step on the state
  after the impulse; the clone's refresh does not run the zero-time
  tidy-ups, open). Test
  `drive::native_phys::a_frames_first_substep_integrates_the_forces_taken_before_its_input`;
  measured against the original in docs/cornering-parity.md.
  * **Left/right split** (0x594A40, traced): per axle s = right wheel's spin
    (front: +0x28 of drivetrain +0x28; rear: +0x2C) / the pair's sum (0.5
    when the sum is within 0x7845A0 of 0); s under the band (0.35 with both
    wheels' +0x11C, else 0.5) gives 0.825, over 1 − band 0.175; hop modes 1
    / 2 fix s at 1 − band / band; the first wheel takes (1 − s), the second
    s, a locked wheel (+0x10C) none. The clone's per-wheel share is the
    same rule. No limited-slip term, and no inertia in it (the wheels'
    inertia +0x2C enters only the input shaft's acceleration 0x594850 and
    the locked coupling 0x594DB0). The unequal front forces at a
    downshift against the wall (docs/prop-hits.md §7c, −6.45 / −6.63 kN)
    are the wheels' own state: the wheel hop's grip (+0x474, 0.914 there,
    count 3: FL keeps 1 − 0.086·0.65 of its grip, FR 1 − 0.086·0.35) puts
    the two friction circles apart; the transplant carries it since the
    hop state is in `tyres_at`.
* **Brakes and grip** (0x5A4D90): foot brake per wheel 2 × share × pedal
  (share = 1 − +0x38C front, +0x38C rear). A burnout (throttle > 0.8,
  brake > 0.5, under 10 mph) puts mph × 0.05 on the rear instead.
  Handbrake [0, 0, 1, 1] × pedal, +0.5 above 0.2 when no wheel is down or
  the car slides (|slip| > 0.3 rad) under 80 mph. Grip base 1 (mover +0x6C),
  ×0.85 with no wheel down. The rear also gets a sliding bonus (0x5990D0)
  ramped with the handbrake (mover +0x70: +1/s released, −20/s held, down to
  0.5) and a post-crash graph (0x802570: (0, 0), (0.4, 1), (1, 1), (2, 0))
  over the 4 s after the car last touched a wall.
* **Yaw limit** (0x5A49B0): record +0x398 (17) in the air, +0x39C on the
  ground; 0 on the ground means none, except 20 while counter-steering with
  |slip| > 0.1.
* **Car modes** (car +0x38, 0x5A4A6C): 0 normal and 2 (in contact) switch
  the rigid body's angular damping (+0x154) and rest damping (+0x150) on,
  with gravity 9.8. 1 is crash mode, set by car-against-car contacts
  (0x59FCDA): no damping, gravity 11, throttle capped at 0.4. In game (flow
  state 4) the rest damping is then switched off again with the gas pedal
  (input +0x20C) over 0.3 (0x78758C), or with all four wheels down (car
  +0x424 = 4) and |forward.z| over 0.15 (0x7A1A24; the value is mover
  +0x58 → +0x58, i.e. car +0x40 + 0x58 = car +0x98, the forward row's z)
  (0x5A4BD1–0x5A4C08). **Corrected 2026-10-31** (physics5; it read
  "|speed| over 0.15 m/s"): so on level ground it is on whenever the gas
  is off, and a car coasting with the clutch open stops on it (live,
  Corolla: rigid body +0x150 = 1 while coasting, D at +0x90 0 above about
  4 m/s and climbing at 0.1/s below).
* **Rigid body terms** (0x59C1F0): forces and torques go through caps of
  1200 kN and 100 kN·m each (0x59BB10). Over top speed (record +0x3A0
  mph × the catch-up (body +0x208, see "Race catch-up in the physics")
  + 1.9, × 1.03 when body +0x15C is set (the mover constructor 0x5B5CD0
  copies the car-type record's byte +0xD8 there, 0 on every stock record
  in GLOBALB, so never), × (1 ± min(|fwd.z|, 0.3)/3) on slopes; the top
  speed is rewritten each frame from tuning +0x290 at 0x5A4D1E unless
  mover +0x84, which only constructors and 0x5992B4 clear) forward drive becomes
  a drag of max(5·slope, 0.5) kN — skipped entirely when the top speed
  (body +0x204, from record +0x3A0) is 0, which is what the performance
  ECUs 0x2D, 0x2E and 0x33 do to it (0x5AC500 clears tuning +0x290): an
  ECU removes the limiter. (Before this rule the clone held ECU-fitted
  cars to 1.9 mph; skill-fitted AI racers carry ECUs.) Angular damping is a curve over |ω|
  (0x7FC92C) × L. Rest damping D (+0x90) moves at +0.1/s toward a curve over
  max(|v|/4, |ω|/2) (0x7FC940), or −10/s when above it; when on, dL −= 2DL
  and dP −= 8DP. The yaw torque is clamped to the yaw limit. The tidy-up
  (0x59C600) caps |v| at 200 m/s and recomputes ω = I⁻¹L, capped at 250.
* **Low-speed spin damping** with fewer than two wheels down (0x5A5B70,
  graph 0x7FC8F8): −g(|ω|)·m·ω below 0.5 rad/s.

### Data and parts

* **Tuning block** = car-type record +0x110. Upgrade packages add into it
  (0x5997F0): mass ×(1 − weight reduction), torque curve += per-level deltas,
  tyre values += deltas. A by-enum accessor at 0x59B210 lists its 25 fields.
* **Performance upgrades** (verified): career chunk 0x34A15 holds 30
  packages of 0x17C bytes (registered by 0x500000): u32 category, u32
  level (1–3), u32 part count, up to four parts of 0x5C bytes: +0x00 id
  (0–0x43), +0x04 name (language hash), +0x08 price, +0x0C brand count and
  +0x10 eight brands, +0x30 four gains (−1 none), +0x40 five ids it
  stands in for, +0x54 two ids it needs fitted first (−1 none). Fitting a
  part (0x5A6330, unforced) does nothing when it is fitted or a part it
  needs is not; the parts it stands in for come off. No part in the data
  needs another. A car's fitted parts are bytes at its upgrade state +0x50 + id. A
  category's level (0x5A6590) sums its fitted parts' gains per channel
  (0x5A61B0), averages them over the category's channel count, clamps to
  0–1. The physics setup (0x5B4DD0 → 0x5AC500) copies the tuning block
  (record +0x110) and moves fields towards, or adds, the record's upgrade
  data by those levels: weight 0x5A6B60 (cat 0), transmission 0x5A6BE0
  (cat 1: a gearbox block at block +0x310 + level × 0x40, level =
  round(gain × 3)), nitrous (cat 2, inline), engine 0x5A6D10 (cat 3),
  turbo 0x5A6E40 (cat 4; its curve at block +0x114 (record +0x334) is
  always added into the torque, so stock turbo cars get it: RX-7 0.190 →
  0.293 kN·m), suspension 0x5A6FB0 (cat 5), brakes 0x5A7150 (cat 6), ECU
  0x5A71D0 (cat 7: rpm limits by round(level), parts 0x2D / 0x2E / 0x33
  clear block +0x290), tyres 0x5A7370 (cat 8), cat 9 0x5A74B0; then the
  inertia from mass and box (equal to the record's stock values for every
  car). Port: `performance::upgrade`, applied whenever a car type is
  read; the performance uniques follow it (see **Performance uniques**).
* **Aerodynamics from visual parts** (**verified** 2026-10-30;
  `performance::fit_aero`, `Game::fit_aero`): category 9 (part ids
  0x38–0x43, front gains 0x38–0x3D, rear 0x3E–0x43; no shop sells them)
  is set by the ride's visual parts, not its performance level. 0x639CF0
  runs 0x6399E0(refit = its argument ≠ 0, dress 0) whenever a ride's
  parts or performance change; without refit and with the static byte
  0x7FC290 = 1 (it is never written; the setup's own refit at 0x5AC5EF
  is skipped by the same byte) the branch 0x639AA2 takes category 9 off
  (0x5A6460(9)) and then: a WIDE_BODY (slot 6, ride +0x5A8) of level ≥ 1
  (part byte +5 >> 5) fits all of category 9 at level 3 (0x5A64B0(3,
  9)); else FRONT_BUMPER (slot 1) → 0x38/0x39/0x3A, REAR_BUMPER (2) →
  0x3E/0x3F/0x40, SKIRT (0xB) → 0x3B/0x3C/0x3D, SPOILER (0xC) →
  0x41/0x42/0x43 by the part's level (capped at 3; level 0 none),
  each through 0x4017E0 → 0x599690 → Perf_FitPart. 0x5A74B0 then adds
  (+0x590 + +0x5A0) × front gain to downforce +0x264 and (+0x594 +
  +0x5A4) × rear gain to +0x268 (tuning block offsets). **Under Wine**
  (Rachel's 350Z in the intro): the player's physics upgrade state at
  0x890460 has fitted flags (+0x50 + id, 0x8904B0) 0x3A, 0x3D, 0x40, 0x43
  where the preset's level 2 gives 0x39, 0x3C, 0x3F, 0x42 (the preset's
  own state at 0x8374F0 keeps those), and the tuning block 0x8901B0
  reads downforce 0.1845 / 0.2175 (stock 0.145 / 0.15 + 1.0 × 0.0395 /
  0.0675); the clone had 0.1711 / 0.1946 (gain 0.66) and now matches the
  block field for field up to +0x2A0 (`devprobes/src/bin/rec_dump.rs`). The
  clone applies it to the player's car (`player_car`) and its shop
  ratings (`perf_record`); AI rides take 0x6399E0(1, …) (the refit, not
  ported, see **AI car looks**). The flag at +0x50 + 0x44 (68) is also set
  in every state read; **open** what it is.
* **Performance uniques** (**verified**; `performance::apply_uniques`,
  `UNIQUE_BASE`, test `unique_tests`): the upgrade state (0x5AC500's
  second argument, physics object +0x2E0; the ride's +0x2F0, 0x94 bytes,
  copied to and from the career car by 0x503950 / 0x503900) holds the ten
  category levels at +0x00, a flag per performance unique i at +0x28 + 4i
  (0x521440 sets the ride +0x318 + 4i) and the fitted part bytes at +0x50.
  0x5B4DD0 runs 0x5997F0(block, state) right after 0x5AC500 (so after its
  inertia) and before the tuning; 0x599DA0 undoes it (the rating code
  0x5B2E57 / 0x5B2E9E, the counted copies 0x5A7DE3 / 0x5ACA94). Unique i
  is category i's: each adds the car type's unique data, tuning block
  +0x2B0 on (record +0x3C0), to the block: 0 weight — mass × (1 − +0x2B0);
  1 transmission — the rear torque share +0x1C0 (record +0x2D0) set to
  +0x2B4 (0.199 for the FWD cars, 0.801 else); 2 nitrous — +0x2AC plus
  +0x2B8; 3 engine — the torque curve +0x200.. plus +0x2D0..; 4 turbo — the
  torque and the turbo curve +0x224.. plus +0x318..; 6 brakes — +0x274 plus
  +0x2BC; 7 ECU — the torque plus +0x2F4..; 8 tyres — +0xDC / +0xE0 / +0xFC
  / +0x100 plus +0x2C0 / +0x2C8 / +0x2C4 / +0x2CC; 5 (suspension) and 9
  (flags +0x3C, +0x4C) are not read. The clone keeps the uniques per car
  (`Profile::perf_uniques`) and asks for them as part ids 0x80 + i
  (`Game::with_uniques`) for the career car's physics, dyno, ratings and
  tuning records.
* **Performance shop** (verified except where noted):
  UI_ChoosePerformanceCategory (0x559C40 → 0x556C90; options 0x530500 via
  0x530410, locked until a level unlocks): Engine 3, ECU 7, Transmission
  1, Suspension 5, Nitrous 2, Tires 8, Brakes 6, Weight Reduction 0, Turbo
  4. A level unlocks in a shop by the cost table (0x515630 → 0x515360)
  for the category's cost record (0x5021C0: 0→6, 1→5, 2→4, 3→1, 4→9,
  5→3, 6→8, 7→2, 8→7, 9→10).
  * Owned and fitted: a part is fitted when its byte at the upgrade state
    +0x50 + id is set (0x599620) and owned when its bit is set in the
    career car's record at +0x590 (0x530360; set on buying by 0x503E10,
    which also adds the price to the money spent, +0x7F0). Fitting an
    owned part again is free. The clone keeps the owned ids per car
    (`perf_owned`, cleared with the fitted ones on a new car).
  * UI_ChoosePerformancePackage (0x55C8E0 → 0x559CA0, options 0x556E70):
    Choose Individual Parts (its accept does nothing, 0x50C2F0), Street /
    Pro / Extreme Package (PERFORMANCE_PACKAGE_n,
    PERFORMANCE_ICON_PACKAGE_n), Return To Stock. Package Contents
    (0x547B70 hides PURCHASED_ICON_1–4, then 0x547910 fills a row per part,
    text by 0x53B7C0): PartName_n the name; PartPrice_n "Installed"
    (0xED56BE1F) when fitted or "Owned" (0x0371711C) when bought, both
    running PURCHASED (which sends PURCHASED to the row's icon), else the
    price `%$d` running INIT; rows past the package's parts hide. The
    total 0x0380B632 (0x5620E0 with 0x5415F0) is the price of the parts
    neither fitted nor owned (0x540FF0), 10% off when there are more than
    one (0x541050; 20% when 0x86344C's +0x38 is 0x7FC8D0D0, the shop
    CC_MEGALOW, "Megalow Parts" 0x6AA654C5, which also drops the Extreme
    level: with a visited shop, 0x863444, the package rows run from 1
    while below 3 instead of 4, 0x5627AD / 0x556F13; the clone's
    `perf_levels`), or "n/a"
    (0xA4C02484) when nothing is due. The discount itself goes to the line
    0x06E0C9A8 ("10%% Discount:" 0x6DA17A25 / "20%%" 0x66559034) and
    0xBE8693E1 (`%$d`, or "n/a"); the clone keeps those two hidden. The
    contents (group 0x3D3F9F04, INIT → HIDE) show only while an open
    package is highlighted: Choose Individual Parts and Return To Stock
    run PackageFadeOut, a locked option runs Hide (0x5620E0).
  * Accept on a package (0x559F90): fully fitted, "This performance
    package is already installed!" (0xB84A37FA); not fully owned, "Would
    you like to purchase and install this performance package?"
    (0x94F4C140; 0xFDDD0C23 when one of its parts is worse than the part it
    replaces, 0x5303D0) unless the bank is short (0xD1AC60BE); owned, "Would
    you like to install this performance package?" (0xF2D7FFAB /
    0x0217453F). Yes (0x9F00F07 / 0x51123592): the price from the bank and
    the parts marked owned (0x54FE60), then each part fitted in turn
    (0x5501F0 → 0x521350 → 0x5A6330). Return To Stock (0x55A150): "This
    performance category is already stock!" (0x89D4E03F) when nothing of
    it is fitted, else "Re-Install Stock Parts?" (0x5D336F96); Yes
    (0xE75749D4) takes the category's parts off (0x521A30).
  * UI_BuyPerformanceParts (0x56C160 → 0x569CD0, an option list 0x552790
    over the package screen): pushed while Choose Individual Parts is
    highlighted (0x5620E0 → 0x555F20), active from 180 ms after the
    package screen's INIT (its Event Handler sends 0x35F8620B). Its rows
    (0x5627A0): the category's level 1–3 packages' parts in order, each
    listed when owned or when its package level is unlocked in the shop
    (0x52A0A0 with 0x50F0D0's flags 6, 0x5155A0); with none, one row
    "None" (0x001A076F). A row (vtable 0x79B630): OPTION_NAME_n the part's
    name, OPTION_DATA_n (0x53B880) "Installed" / "Owned" / the price
    `%$d`. Rows lie from TITLE_MASTER and DATA_MASTER (0x51FF90 / 0x54F830),
    a title master's height (26) apart: the name's top left at the title
    master's, the data (centred) on the data master's centre at its top,
    OPTION_BACKING_n at the name's top left less (−271, −10) (0x52D810;
    the backings 2–12 are authored off screen). The highlighted row runs
    HIGHLIGHT on name, data and backing (0x52E130); the others run
    PURCHASED on name and data when owned or fitted, else UNHIGHLIGHT, and
    UNHIGHLIGHT on the backing (0x530620). The list's bar is
    "scrollbar_*" (0x542AD0); its handle and arrows hide while the rows fit.
    Up / Down (0x72619778 / 0x911C0A4B) move along it without wrapping
    (0x5390F0); Left / Right stay with the package screen's icons. Accept
    on a row (0x55A240): fitted, "The part you are trying to apply is
    already installed!" (0xDA15D7E5); not owned, "Would you like to
    purchase this part?" (0x44B7167F; 0x3949ABEE when worse) unless the
    bank is short (0xD1AC60BE); owned, "Would you like to install this part
    now?" (0xF5AC2B2A / 0xF1BD1A8A). "Worse" (0x521C20): the first fitted
    part it stands in for is of a higher package level. Yes: the price from
    the bank and the part owned (0x521B10), then fitted (0x5213E0 →
    0x5A6330). Parts and packages with brands (most of them) first go to
    UI_PerformanceBrandSelect (0x422F7587, the next screen 0x520930 gives):
    Yes on a part (0x9F00F07 → 0x569F7A) stores it in 0x839BB0 and opens
    the screen when the part has brands (+0x0C), else buys and fits at
    once; a package goes the same way (0x5501F0; 0x83855D clear).
  * UI_PerformanceBrandSelect (`options::BrandSelect`; class vtable
    0x79CAC4 built by 0x5532C0 → 0x550340, messages 0x55C940; **verified**
    except where noted): the UI_Options list (0x552790) with a row per
    part (option 0x521E40, vtable 0x79B6B0): the part alone (0x83855D set;
    the group 0x6D017BC5 hidden), or the package's first four parts with
    any brand (part +0x0C count, +0x10 hashes). A row shows the part's
    name (+0x04), the brand's shown name and the image DECAL_ICON_<name>
    (0x53B970 through the brand table 0x838400 = career chunk 0x34A14,
    0x44 bytes: +0x00 name, +0x20 shown name, +0x40 hash, 0x4FFF00 /
    0x4FFF40; 0x74CE8C0B without a texture), starting on the first brand.
    Left / Right (0x9120409E / 0xB5971BF1, 0x521EA0) step the brand
    forward / backward, wrapping (corrected 2026-11-04, docs/perf-shop.md); Up / Down (0x72619778 / 0x911C0A4B) hide the brand hint
    0xB34A6C2B on a row with fewer than two. Select (0x0C407210) plays
    0xDE6EFF34 on 0x47FF4E7C, whose end (0xE1FDE1D1) copies the rows'
    brands in order to 0x8389B4… (0x521F00) and buys and fits: the part
    (0x521B10 + 0x5213E0) or the package (0x54FE60 + 0x521350), then
    0x54F990 and back (0x55BD70). Buying appends 0x8389B4 (+4 × the
    package part's index; reset to 0xFFFFFFFF after each) to the car's
    brand list (0x503E10: the first empty of 0x44, never removed; the
    clone's `Profile::brands`, saved per slot). **Verified**: the arrow
    widget (0x51E1C0, vtable 0x79B388) keeps 0x521E40's sixth argument as
    a byte at +0x28. The brand row builder 0x550340 passes `1 < count`
    there (seta on the brand entry's count at +0x18, call 0x550425) and a
    literal 1 at 0x550392. **Verified**: that byte is the row's enabled
    flag. Each row goes into the list by 0x54F1A0 (calls at 0x55039C,
    0x5503B6, 0x550433), which at 0x54F3AE reads it and, when it is 0,
    calls the row's vtable slot +0x20 (0x51E770, its Disable), whose first
    call is 0x51E3F0: that clears the +0x28 byte and then, for each of the
    row's +0x2C, +0x30, +0x4C and +0x50 groups, looks up the item keyed
    "Disable" (0x78F570, hashed by 0x505450) in the group's +0x4C list and,
    when it is found, makes it the group's selection (+0x54). The setters
    0x50D7C0 / 0x50D7D0 are plain writes of 1 and 0 to +0x28. **Verified**:
    the row vtable 0x79B6B0 holds 0x52DA00 at +0x08 and 0x51E770 at +0x20.
    0x521E40 installs 0x79B6B0 (its only code reference) on the same object
    that its call to 0x51E1C0 built, so the row is that arrow widget.
    0x51E3F0 itself writes only the +0x28 byte, the selections (+0x54) of its
    four groups and item +0x10 fields; through 0x51CF80 it also sets the +0x24
    object's selection, and a selection change calls 0x517720 (below). So it
    neither unlinks nor zeroes the +0x30, +0x4C or +0x50 group. 0x52DA00 hits
    +0x4C, then +0x50 and +0x30 when +0x4C misses, each through 0x52D030, a
    point-in-rectangle test with inclusive bounds on 0x51DA00's rectangle. For
    the types below, that rectangle reads the group's type (+0x18) and its
    owner frame (group +0x2C: +0x1C, +0x20, +0x38, +0x3C), never +0x28 or the
    selection: types 1, 7 and 9 span the owner origin plus or minus half the
    owner's +0x38 (x) and +0x3C (y); types 3, 4, 6, 10 and 11 match no case in
    either helper, so they give a zero-area rectangle at the owner origin,
    which the inclusive test hits only at that exact point; type 5 is the
    union of its children (list at +0x64). Type 2 is content-sized by
    0x51D8B0, which calls 0x51BD70 on +0x24 (a null result branches to
    0x51D9EE, whose outputs were not traced), reads the group's +0x1C flag
    bit 0x2, and needs a non-zero size. 0x51D8B0's own code reads no +0x28 or
    selection, but its helpers (0x4FF9D0, 0x50D3C0, 0x43E000, 0x50BEC0,
    0x50BF40) were not traced, so that part is unverified for type 2. So
    0x51E3F0 does not by itself take the groups out of the hit test.
    **Inferred**: that Disable hides the arrows. The one traced route to their
    geometry is 0x517720 (run on the selection change), which writes the
    selected item's binding records (+0x1C count, +0x20 records) into target
    objects at +0x10 to +0x1C. Those records, the helpers above, and the
    virtual calls 0x52DA00 makes after a hit were not traced, and the
    LEFT_ARROW_n / RIGHT_ARROW_n visibility is not
    traced. **Superseded** (perf-shop round, docs/perf-shop.md): the
    rows' layout (logo on top, measured under Wine), Back does nothing,
    the brand slots 0x8389B4 (0 at boot, 0xFFFFFFFF once read; a purchase
    without the screen appends what is left), the spend +0x7F0, and the
    shop's state after the screen (Choose Individual Parts, first row). `--shot brandselect`.
  * Readouts (0x562540): the car as it is (a copy, 0x413440) and the car
    with the highlighted part fitted. Peak torque (0x5B5820) and power
    (0x5B56F0) of the performance object (record + 0xE0): the
    full-throttle torque sampled from idle in 50 rpm steps while under the
    maximum rpm; power = torque × 1000 × rpm × 0.00010471204 (kW). The
    texts print the gain `%$2.2f %s`: torque × 1000 × 0.7375621 "ft-lbs"
    (0xACB8DFF3) or × 1000 "Nm" (0x5B81D613) with metric units (0x83AA1A
    = 1), power × 1.34102209 "bhp" (0xCBBC6512) or "kW" (0x5B81D5BA); the
    clone uses the imperial pair. The bars (0x547E40: torque_* and
    horsepower_*) take the peaks clamped to the stat ranges [4..5] and
    [6..7] and scaled to 0–1. The package screen's bars follow the
    highlighted part (0xC516CD37 → 0x54FFA0).
  * Stat ranges: career chunk 0x34A32, 30 floats copied to 0x86B3D0 by the
    loader at 0x599472 (0x34A33 is kept at 0x86B338).
  * Ratings (0x5B53B0 acceleration, 0x5B6A30 top speed, 0x5B54A0
    handling; each clamped to 1 above and 0.03 when not above 0.03),
    computed on the free-roam set-up (0x5B4FD0 → 0x5B4DD0 mode 0). See
    `performance::ratings` for the terms: the acceleration area (0x59A360:
    the acceleration curve less v²·drag/mass summed in 3 mph steps while
    ahead, up to 100 mph) over [2..3] × 0.82 + the nitrous product
    (+0x3BC × +0x3B0) over [8..9] × 0.1 + round(+0x3C4) / 3 × 0.08; the
    computed top speed (0x5B5940) over [0..1] × 0.915 + the nitrous product
    × 0.085; nine handling terms weighted by 0x7A1A9C.
  * The bars (0x547A10 for the package screen's Accel_*, top_speed_*,
    handling_*; 0x540C20 builds them with end pads of 46 and the range
    0..1; update 0x527180): from the backing's top left (bx, by) and width
    w, the cap at (bx + 42, by + 4); the fills cover (w − 92 − 8) for the
    whole range; the current fill shows the lesser of current and preview
    from (bx + 54, by + 9); the preview fill shows the difference after it
    (hidden when none) running TOLESSER for a drop or TOGREATER for a gain;
    the handle sits at the current value's end less 9, at by + 7. The
    package screen (0x54FFA0) sets the car as it is against: the car with
    the highlighted part fitted (Choose Individual Parts), with the
    package's parts fitted (0x521350), or with the category's parts off
    (0x521A30); a locked option shows the car as it is.
  * The car lot's bars (0x497CD0) take the same three ratings of the car
    as it comes; their bar class (update 0x5273B0) is not ported: the
    clone scales the authored fills.
* **Engine** (constructor 0x5AA5B0, data = record +0x300): idle / redline /
  max rpm at +0x300/+0x304/+0x308, crank inertia +0x30C, four curves built
  from them:
  * +0x310, 9 points over idle..max: full-throttle torque;
  * +0x334, 9 points over idle..max: the turbo curve, added into the
    torque at setup (0x5A6E40; see Performance upgrades);
  * +0x358, 3 points over idle..max: closed-throttle drag;
  * nitrous, 9 points over idle..redline, from a table in the EXE
    (0x7FBF38), scaled by tuning +0x2AC and a per-level factor (0x7A17A0).

  Curves are linear tables clamped at both ends (0x5764E0).
* **Engine torque** (0x594310): `T(rpm)·scale·throttle·power`, plus nitrous,
  minus `(1 − throttle·power)·drag(rpm)` (doubled at the fuel cut). A
  negative total is ×0.65 in neutral, ×0.85 in gear. ω' = (T + R) / inertia,
  R being the driveline's reaction gathered during the sub-step (engine
  +0x7C; 0x5A12A0). The tidy-up (0x5AA770) clamps the engine to idle..max,
  clears R, and sets the fuel cut within 25 rpm of max, clearing it below
  max·(1 − 0.0125).
* **Gearbox** (constructor 0x5AF9E0, data = record +0x2C0): final drive
  front/rear +0x2C8/+0x2CC, rear torque share +0x2D0 (1 RWD, 0.5 AWD,
  0 FWD), clutch coupling +0x2D4 (kN·m per rad/s), forward gear count
  +0x2D8, clutch rate +0x2DC, ratios from +0x2E0 (reverse, neutral, 1st…;
  reverse is negative). +0x2C0 (0.8 on every car) places the shift out of
  neutral between idle and redline (0x5AC7E0). +0x2C4 (500) is unused
  (verified): the drivetrain keeps the block at +0x18 (tuning +0x1B0,
  0x5B6088), and nothing reads its +4 — the drivetrain's only +0x18 → +4
  reads (0x594680, 0x5A11F0) go through the engine (+0x20), whose +0x18
  is the engine block (redline). Tuning +0x1B4 is only copied (0x5A6C6A)
  and fed to the tuning checksum 0x59A530 (every field × 65536, hashed by
  0x43BCD0).
  The same block repeats at +0x460, +0x4A0 and +0x4E0. The transmission
  upgrade (0x5A6BE0) takes its block at tuning +0x310 + level × 0x40 (shl 6
  and lea +0x310, verified), so these are levels 1–3 and level 0 is +0x420.
* **Clutch states** (tidy-up 0x5A1410, after every integration). The input
  shaft speed (+0x4C) is first refreshed from the wheels (0x594600). Then:
  * 0, opening: power 0; at engagement ≤ 0 → 1;
  * 1, open: power 0, the gear becomes the requested one; on to 2 (with
    new shift offsets) unless the anti-stall test holds and no change is
    pending;
  * 2, closing: power 0.7; at engagement ≥ 1 it locks (→ 3) when engine and
    input shaft agree within 50 rad/s (0x594EE0), or in neutral; the
    anti-stall test opens it again (→ 0);
  * 3, locked: power 1; the anti-stall test, or a requested gear that
    differs (marking a change pending, +0x48), opens it (→ 0).

  The anti-stall test (0x594EA0) is throttle < 0.3 and input shaft ≤ idle.
  Engagement moves at the gearbox rate × (−3, 0, 1.5, 0) by state (0x5949C0
  as a derivative). A graded shift (+0x68, drag races only; see "Race
  modes") scales it further; in free roam that is 0. The mover reset (0x5A48B0) puts the
  drivetrain in the car's stored gear (+0x3F4, neutral at spawn) with the
  clutch locked.
* **Driveline coupling**, each sub-step:
  * At the start (0x594CC0): closing or opening, the clutch passes
    coupling × engagement × (engine − input) clamped to ±300 rad/s; the
    coupling halves when the slip in rpm swings sign beyond 2000 both ways
    (+0x58). Locked, it passes the engine's torque. Open, nothing.
  * Any crank torque T goes out through 0x594A40. Unless in neutral, the
    engine takes −T × max(wheels down/4, 1/4) as reaction, and the wheels
    get T × ratio × final drive × axle share, split left/right as below (a
    locked wheel, +0x10C, gets none).
  * In each of the five passes below, with the clutch locked (0x594DB0), a
    further crank torque
    Δ = ((T_e + R)/I_e − α_in) / (1/I_e + Σ axles share/I_w·(ratio·fd)²·½)
    goes out the same way. α_in is the input shaft's acceleration from the
    driven wheels, the mean (T − r·Fx)/I_w per driven axle × its final
    drive, halved with two driven axles, × the ratio (0x594850).
  * After the friction circles (0x5AA840), in gear, locked, with a wheel
    down: under power (throttle > 0.2) with the driven wheels' mean slip
    (+0x16C) over 0.1 m/s, the driven wheels are set to the engine's speed
    (0x5A12E0: each axle moves by the difference over its ratio; under
    40 mph, or with the axle in the air, the pair first takes its mean).
    Otherwise R += clamp((input − engine) × coupling, ±|T_e|). In 1st gear,
    when R < T_e and R + T_e < 0, R += c(R + T_e) × −0.7 × (R + T_e), c over
    (−10, 1), (−7.5, 0.96), (−3.5, 0.925), (−0.3, 0.875), (−0.05, 0)
    (0x7FC8C0; the −0.7 is table 0x7A18B4 = {0, −0.2, −0.4, −0.7} at the
    drivetrain's +0xF0, the car's performance class: 3 for the 240SX).
* **Torque split** (0x594A40): first between the axles by the rear share,
  then between left and right.
  * **Wall drive** (**verified** 2026-10-31, physics3): the rear share
    0x594A40 uses is the drivetrain's live +0xEC (the coupling's
    compliance, 0x594E32 / 0x594E66, too; which axles count stays the
    gearbox's +0x10). The constructor sets it to the gearbox's (0x5AFA69);
    each frame 0x5A49B0 (0x5A4D24–0x5A4D7B), for a tune whose rear share
    (tuning +0x1C0) is over 0.8 (0x7850EC), sets it to 0.65 (0x3F266666)
    while +0x430 × 2.237 < 60 mph (0x78426C, 0x787590; car +0x430 = |v| as
    the last frame's end or a velocity set since wrote it, not the signed
    forward speed) and the collision
    body ([car +0x3C] +0x120, its live wall records, see "Car against a
    prop up against a wall") is above 0, else back to +0x1C0. So a
    rear-driven car pressed against a wall sends 35% to the front. Live:
    a 240SX glancing off a wall at 14 m/s shows front wheel torque +0x108
    0.45–0.88 kN·m at once (0 before); a Corolla reversing off a kerb
    −1.12 on each front wheel. Clone: `Drivetrain::rear_share`,
    `CarState::wall_drive` (the records with |n.z| < 0.1).
  * Each wheel gets its share of the axle's wheel spin, held between 0.35
    and 0.65 (0x7FBFFC). Beyond that band the faster wheel gets 0.175.
  * With a wheel off the ground the band closes to 0.5.
  * Modes 1 and 2 of the car's +0x42C (car object +0x46C: the drivetrain's
    +0x34 is car +0x40) fix the share: 1 gives FR and RL 1 − band, 2 gives
    them the band. They are the last two counts of the launch wheel hop
    (0x5B6310, driver types 1–3: the player is type 1, so it runs in free
    roam too; see "Driving vs the original, round 14"). Its speed tests all
    read the exported forward speed (car +0x42C at 0x5B633A, the physics
    view's +0x3EC at 0x5B64F7 and 0x5B660C), not a fresh v·forward
    (verified). (Corrected in
    round 14: an earlier note said the player's mode stays 0.)
* **Chassis** (rigid body, constructor 0x5A7F40 from record +0x220): mass
  +0x220 (t); body-space inertia is the diagonal of a 4×4 matrix at +0x230
  (roll 0.46, pitch 2.25, yaw 2.37 t·m² for the 240SX); the centre of mass
  is +0x110..+0x118 (rigid body +0x1F0). The state is momentum P (+0x70)
  and angular momentum L (+0x80). The rigid body's data pointer (+0x14,
  tuning +0x110, 0x5B5D74) is read only at +0 (mass), +0x10 (inertia) and
  +0x50 (0x59B950). Tuning +0x114, +0x118 and +0x11C (record +0x224, +0x228,
  +0x22C) are the box's full edge lengths A, B, C. **Verified**: the race
  tune's inertia rebuild (0x5A7540, 0x5A76F3–0x5A7749) writes the diagonal
  as k·m·(B²+C²) at +0x230, k·m·(A²+C²) at +0x244 and k·m·(A²+B²) at +0x258,
  with m = +0x220 and k = 1/12 (0x7875B0, verified as 0.0833333). **Verified**:
  the race tune's inertia rebuild (0x5A7540, 0x5A76F3–0x5A7749) first sets the
  4×4 at +0x120 to identity (0x43C780), then writes the diagonal as k·m·(B²+C²)
  at +0x230, k·m·(A²+C²) at +0x244 and k·m·(A²+B²) at +0x258, reading A, B and
  C at +0x114, +0x118 and +0x11C of the same base. The pose at +0x30 has the
  same 4×4 layout (0x5A7F40 sets it to identity via 0x43C780). **Verified**: the
  ground ray (0x59D260, called at 0x5A881D) reads body pose row 2 (+0x50..+0x58,
  its fourth argument is body +0x30) as a direction input. The suspension update
  (0x5A87A0) reads wheel mount components 1 and 2 (+0x94, +0x98); the compression
  write-back was not traced. **Verified**: the tyre frame is copied with ECX =
  tyre (0x5A8A8D, 0x5A8A9B), and 0x5913E0 writes source row 1 to tyre +0x80,
  the axle slot. Whether that wheel frame is body-aligned was not traced.
  **Inferred**: slot k is body axis k, row 2 is up and the axle is the lateral
  spin axis. Then A is along x (length), B along y (width) and C along z
  (height), since each diagonal omits the edge along its own axis. The
  load-factor claim is removed: 0x59D170 is not a routine entry in the
  disassembly. The 240SX ratios A/B = 2.68 and B/C = 1.31 fit that order, but
  were not re-checked here.
* **Suspension** (constructor 0x5A86D0, update 0x5A87A0): the front block
  is at +0x280 and the rear at +0x2A0, each with progression, spring rate
  (kN/m), bump and rebound damping (kN·s/m), anti-roll (kN/m), compression
  limit +0.16 and extension limit −0.16 m.
  * The ground is found at the wheel's rest point (body pose × mount +0x90)
    by the track's height query (see "Collision"). Compression is
    (ground − point)·n / (n·up) + radius (0x59D260); no contact when
    n·up < 0.001, no ground, or past full droop.
    **Verified**: 0x59D260 takes as the ground height the query's
    result + wheel +0x18C (zeroed by the wheel constructor at 0x5A81CF and
    at 0x59CB7B; no other writer traced, the clone adds nothing) + the
    surface's bump 0x5D3CB0(point, surface): a 2-D value noise over x/y
    from the 16-entry table 0x800BF8 × the surface row's amplitude
    (0x800EFC + 0x98·surface: 0 for surfaces 0–7 and 22–28, 0.01–0.05 m
    for 8–21), also stored at wheel +0x168 (no reader traced).
    **Verified** (0x5D3CB0..0x5D3DED; `drive::surface_bump`): row r
    = 0x800EE0 + 0x98·surface (the surface from the ground query, before
    the drag/street-X override writes 1 at 0x59D2E4); X = f·x, Y = f·y
    with f = row +0x18 (0x800EF8: 1, but 3 for surfaces 11–12 and 1.1 for
    16); the cell corner is floor(X), floor(Y) (_ftol 0x75BED4 truncates,
    then 1 is taken off when the integer lies above, 0x5D3CE6 / 0x5D3D0C);
    t = X − ⌊X⌋, u = Y − ⌊Y⌋; corner (i, j) reads table entry
    (−5·i·j − 3·(i+1)·(j+1)) & 15 (wrapping i32); the bump is the bilinear
    blend of the four corners × the amplitude (row +0x1C). The table:
    −0.5 −0.2 0.5 0.2 −0.4 −0.1 0.2 0.5 −0.2 −0.5 0.4 −0.3 0.3 0 −0.2 0.1.
    The point is the wheel's rest point (its x, y); the clone adds the
    bump to the ground height in its suspension (`step`).
  * **Grid placement** (verified; `CarState::placed`): the racer
    set-up 0x604270 copies the start point (the grid slot, its z the
    ground from 0x5EB5E0) to racer +0x60 and ends with 0x5F5020 →
    0x5F4A00(+0x90 matrix, +0x60 point, heading +0x40C). 0x5F4A00 turns
    each wheel's record point (racer +0x20 = record +0x110; wheel i at
    +0x10 + 0x30·i) by the heading (0x5BAE70), adds the point, and drops
    it to the ground (0x5E72F0 with its found flag). With all four found:
    A = (P1 − P2) + (P0 − P3) and B = (P3 − P2) + (P0 − P1) over the ground
    points, normalised (0x43CF10), C = A × B (0x43D1E0), matrix rows A, B,
    C (B is not re-orthogonalised); point z = Σground / 4 + (Σ(radius −
    wheel z) / 4 + record +0x118) / |C.z|. Otherwise the point and the
    heading-only matrix stay. 0x5F5020 then sets racer +0x50 (the object
    at racer +0x40, its +0x10) to the point less the centre of mass
    (record +0x110..+0x118, or the rigid body's +0x1F0 when racer +0x34
    has one) — component by component, not turned by the heading
    (0x5F5072..0x5F5095) — and copies the matrix (0x592470 → 0x4012B0).
    **Inferred**: the rigid body takes +0x50 as the car's origin (the
    clone's `CarState::position`); the clone orthonormalises the axes.
    `Drive::place` and `line_up_rivals` use it (`Drive::place` on the road
    under the point + 3 m as before). Traffic spawns (0x5FBEE0 → 0x5F52D0)
    still put wheel 0's contact on the ground: **open**.
    Probe (`grid_launch 4702 COROLLA`, a braked second at rest): the old
    placement (wheel 0's contact for all) moved the body 11 cm with
    |v_z| up to 1.1 m/s; now 1.8 cm and 0.16 m/s (4001: 7.6 → 2.1 cm).
  * Force `k·x·(1 + p·max(x, 0)) + c·ẋ + preload + anti-roll·(x − x_other
    side)`, floored at 0. The preload is each wheel's static share of m·g
    plus speed × downforce × record +0x37C, split by where the centre of
    mass sits between the axles (0x5AB9A0). The tyre load is that force ×
    max(1 − 4·(axle·n)², 0.3) (0x59D170).
* **Tyres** (0x5A8290, friction circle 0x59D610): the tyre blocks are at
  +0x1E0 (front) and +0x200 (rear): slip-angle scale (12°), load scale,
  lateral scale, grip (×load) and sliding friction (×load).
  * Slip angle = atan2(vy, vx), measured from backward when rolling
    backwards. Slip = r·ω − vx (+0x118, +0x16C).
  * A wheel slides when |slip| > 0.5 m/s, the slip angle exceeds the scale,
    or its brakes lock. Sliding: `F = μ_slide·load·(slip, −vy)/|(slip, vy)|`,
    ÷μ under 1 mph.
  * Otherwise `Fx = torque / r`. Fy comes from seven lateral curves in the
    EXE (0x7FC7D4, one per 2° of slip angle, over 0.8·load), × lateral scale
    × 2.5, opposing vy.
  * Stability Control off (car +0x464 = 1; **verified**, REA + live):
    the sliding μ (0x5A84BD) and the circle limit (0x59D68F) of a rear
    wheel are × 0x59D4A0 = 0.85 (0x7FC71C) up to 13.41 m/s (30 mph),
    rising (× 0.11185 per m/s) to 1 at 22.35 m/s (50 mph), for a wheel
    that is not locked (+0x10C = 0), spins forwards (+0x28 ≥ 0) and has
    wheel-table index +0x18 ≥ 2 (the rear pair). The test reads
    `[wheel+0x20]+0x424` with wheel +0x20 = car + 0x40, i.e. car +0x464,
    not the wheels-down count +0x424. Car +0x464 is the controller's byte
    +0x21 (attach 0x5F3E20), for a human the options' Stability Control
    byte 0x83AA7A + 0x28 × player (0x5F1802; 1 shows "Off", the default);
    with one human the race start 0x403C20 (from 0x60A88C) copies player
    1's byte to every AI racer of the world list; the car constructor
    zeroes it (0x591ED5), so traffic is 0 (**inferred**: 0x4A7B90's
    writer is a message handler, not followed). Live: 1 for the player
    in free roam.
  * The friction circle caps |F| at grip·load. Drive force is ×1.5 before
    the check and only divided back if it fits. Above 1 m/s, sin(slip
    angle)·Fy·0.15 comes off Fx.
  * Brakes: foot brake × 4, handbrake +0x388 × 10. A wheel locks when
    1.2·brake > 0.1·|ω| + r·μ_slide·load·|vx|/|v| and brake > 1; just past
    it the wheel stops without locking (0x59D410). ABS scales a braking
    torque by 0.5/−slip unless the handbrake is on.
  * Wheel spin (0x59D360): the net torque T − r·Fx averaged with the last
    step's, ω' = (net − 0.002·roll·ω − 0.1·(+0x174)·ω) / 0.01 t·m²; a wheel
    that grips has its slip taken out within the step only when substep dt
    exceeds float 0x78435C = 0.001 (guard at 0x59D3DE). +0x174 is 1 only for
    wheels flagged in car +0x95C, which nothing sets in free roam.
  * **Surfaces** (table 0x800EC0, 29 rows of 0x98 bytes; the wheel's
    surface +0x114 is the floor face's byte 2, or 1 with no ground): grip
    +0x44 scales the friction circle, slide +0x48 the sliding friction, roll
    +0x4C the rolling resistance, and drag +0x50 — averaged over the wheels
    on the ground, 1 with none (0x5A49B0) — the air drag. Rows 0–7 and
    16–21 are all 1; 8, 9 and 22–28 lose 1–3% grip; 10–12 roll 1–2% heavier;
    13–15 lose 4% grip and roll 1.1–1.5× heavier, 15 also with 5× drag. L4RA
    uses 1, 8–11, 14–17, 20 and 26. The wheel's other factors: +0x188 is
    1 except on the rear wheels in drift events (the drift assist, "Race
    modes"); +0x184 is set every frame (0x599180) to car +0x468, which the
    player's input (0x4172D0) makes max(1, catch-up): with no race running
    (0x89E7B0 = 0) the catch-up is 0.75, so 1 in free roam.
* **Rolling constraint** (0x5ABE00): after the per-wheel forces, five
  passes run. Each pass sums all the forces in the car's plane (wheel forces
  at their contact points), giving the car's acceleration a at each contact
  (linear plus yaw) along the wheel. Every rolling wheel (not sliding +0x160,
  not locked +0x10C) then moves its drive force:
  Fx += (r·(T − r·Fx)/I − a) / r × I, with I = wheel +0x2C (0.01 t·m²)
  and r = the wheel entry's +0x10. The update is not a full solve — it
  closes about a third of the gap each pass. The pass ends with the
  coupling torque above.
  **Verified** (2026-10-31, physics2; 0x5ABE00 body): a is taken along the
  wheel's steer matrix *row* (+0xB0, +0xC0) = (cos δ, −sin δ) while the
  wheel's force goes to the body through the *column* (+0xB0, +0xB4) =
  (cos δ, sin δ): along = (aₓ − α·p_y)·cos δ − (a_y + α·p_x)·sin δ, with p
  the suspension mount (+0x90, +0x94) and α = the wheels' yaw torque ×
  the rigid body's inverse inertia [2][2] (+0x170, or +0x1B0 when +0x158
  is set). So a steered wheel reads the sideways acceleration mirrored:
  in a turn the front tyres push forward instead of dragging. Live (350Z,
  full lock + gas, 4.4 m/s): front +0x124 = +0.41 / +0.09 kN; the clone
  had −0.40 / −0.48 (it used the column) and now +0.44 / +0.13. The
  side-force cost after the circle (0x59D610: fx −= sin(angle)·fy·0.15
  above |vx| 1 m/s, angle from the contact velocity +0xF0 in the wheel
  frame, 0x43C570 = atan2(y, x) in u16 units) is unchanged.
* **Aerodynamics** (0x59E630, factors from the object built at 0x5B6283,
  mover +0x50; **verified** with REA and live): each frame the mover
  update 0x5ABC00 stores the engine throttle t (the gas after its
  0.15 / half-open rules, also engine +0x78) at aero +0x10 and car
  +0x424 × 0.25 (the wheels down / 4) at +0x14 (live, standing: 0 and
  1.0). Drag is (2 − t)·(+0x370)·surface drag·|v|·v (0x7FBED0 = 2), at
  the centre of mass, or with +0x14 ≥ 0.5 at a world offset (0, 0,
  −0.1·(1 − t)) (0x7FBED4). Downforce: shape max(v̂·fwd, 0)² (at least
  0.4; drift along⁴) × max(up.z, 0) (1 when +0x14 ≥ 0.5) × |v|; with
  +0x14 ≠ 0 the front value +0x374 is applied 1 m ahead of the centre
  of mass and the rear +0x378 1 m behind (0x59E852); with no wheel down
  40% and 41% of their sum there. Live check: the original's four wheel
  loads sum to m·g + (+0x374 + +0x378)·|v| (13.27 + 0.405·v kN for the
  350Z, 4–15 m/s).
  **Corrected 2026-10-31** (physics10, 0x59E677–0x59E6AC): the |v| that
  drag and downforce take is |v| / aero +0x08 (the race catch-up), and
  the drag is also × aero +0x0C = 1 − car +0x4DC (the draft, see
  "Draft"), both stored each frame by 0x599180 (0x5A3390 for the
  MeloMvr). Clone: `CarState::draft` (taken in `step_world` from the
  cars stepped together, **inferred** to be the list 0x890080, each
  sub-step rather than once a frame), `CarState::catch_up`.
* **Race catch-up in the physics** (0x599180, each frame from the mover
  update at 0x5A4C43; **verified** in the code, not live): the value is
  car +0x4E4 when that is non-zero and car +0x95C is clear, else car
  +0x468 (the catch-up, max(controller +0x770, 1) for an AI racer; 1
  outside a Quick Race with Catch Up). It goes to: engine +0x90, the
  torque curve's value × it (0x594330; the nitrous term not; × 1.05
  (0x7FBFF4) more when engine +0x74, cleared by the constructor
  0x5AFB14 and not found set); rigid body +0x208, the top speed × it;
  aero +0x08 (drag and downforce take |v| / it); the four wheels' +0x184,
  the friction circle × it (0x59D69E: surface grip × tyre grip × +0x188 ×
  +0x184 × +0x180 × load × 0x7FC718). With car +0x95C and wheel flag
  [car +0x3C] +0x18 the engine gets 0.4 × it. 0x7FC0A0 (1, no writers)
  would force 1. Clone: `CarState::catch_up` set for the rivals from
  `SpeedTarget::catch_up` (game.rs `step_race`); the player's stays 1
  (**verified** round 11: the writers of +0x468 are the constructors
  0x591EE0 / 0x591F86 (1.0), the AI reset 0x408C80 (1.0 with controller
  +0x770), the AI catch-up 0x41773E (controller in ebx) and
  Car_TwoPlayerCatchUp 0x5F6750 (from 0x6012CC), which writes a human
  car only with two players (0x8900BC = 2) and Catch Up on (0x89E7F8):
  graph 0x81CB60 + 0xA6C (4 points) of (own − other's race progress) ×
  the lap length 0x88F2B0 — so a lone human keeps 1 in every mode; read
  1.0 live in Free Run. Split screen is not in the clone. **Open**:
  +0x4E4).
* **Steering** (PlayerSteering 0x416BB0 with 0x405DC0 and 0x4060E0;
  Ackermann 0x59DF00):
  * The input goes through a response curve (0x7E9E60, 21 points over
    −1..1; half a stick gives 15%). Positive is left. The curve is picked
    by game mode (0x416C08: graph 0x7E9E4C + 20 × index): index 4
    (0x7E9E9C → points 0x7E9330: ±0.05, 0.085, 0.123, 0.16, 0.228, 0.319,
    0.492, 0.688, 1, 1) under the 110°/s condition below (offline: drift
    0x89E7D9 or URL 0x89E7E2), index 1 otherwise; a key's ±1 is 1 on
    both (**verified**, round 9; `STEER_RESPONSE_DRIFT`).
  * **Drift ratio boost** (**verified**, round 9): in a drift race
    (0x416C5C: neither drag flag 0x89E7D8 / 0x890108, and 0x89E7D9 or
    its online twins 0x890114 / 0x890104 — URL not included), when the
    shaped input × the car's yaw acceleration (car +0x128) is above 0
    (steering the way the yaw speeds up), the ratio (+0x380) is × the
    graph at 0x7E9F50 (0x40C840, points at 0x7E9384: (0, 1), (1, 1.1),
    (2, 1.2), (3, 1.25), (4, 1.3), (5, 1.35)) of |yaw acceleration|
    (rad/s²), for both the target and the rate (the ratio is read once
    into [esp+8] and used by both, 0x416CE4 / 0x416D32). Car +0x120 is
    the angular acceleration in the world, (ω +0x110 − last frame's
    +0x530) / dt, written by Car_UpdateRaceEvents 0x604750 (with +0x80 the
    linear one from +0x70 / +0x520); live it equals the frame's change of
    ω_z / 0.05 and the steering reads the previous frame's. Measured
    (240SX, Stadium Drift 2, A held from 12 m/s): inside front wheel 9.8,
    22.1, 33.6, 37.4° per frame with α_z 0 → 2.8 → 3.1 → 2.6 rad/s²; the
    clone without the boost 9.8, 19.8, 28.9, 30.6° (held at 38.9° against
    the original's 41.7°), with it 9.8, 21.1, 33.9, 38.0° (42.2°).
    Clone: `CarState::last_yaw_rate` / `last_steer_dt`.
  * **Input mean** (**verified** 2026-10-31, physics3): the gains below
    read not this frame's input but the steering object's running mean
    of it (PlayerSteering +0x20; the steering object is the human input's
    +0x220). The constructor 0x405C60 sets up a 0x576AB0 average at +0x14
    (0x579F90 with a 0.55 s window, 0x783AD8, and 120 samples a second,
    0x783AD4: 66 slots; a second one at +0x48 with 0.15 s, 0x783ADC). The
    dispatcher 0x41DFC0 (keyboard: 0x416BB0; a wheel device, 0x5C1010 →
    0x86FFB4/0x86E5BC, 0x4061C0; 0x81CB8C set, 0x406A40) feeds it once a
    frame after the steering with the shaped input 0x416BB0 returns and
    the clock 0x865188 × 0.00025 (0x41E03B). 0x576AB0: a sample replaces
    the slot at +7 (count +6 grows when that slot was empty, else its value
    leaves the sum +8), samples older than the window go from the oldest
    (+0x2C), mean +0xC = sum / count. Readers: the lock gain 0x7E9F00
    (0x405E5F), the held lock |mean| ≥ 0.5 (0x406077, 0x783AEC) and the
    rate gain 0x7E9F3C (0x4061AA). A key's input is ±1 at once (0x407750:
    the axis byte b → (b − 128) × −1/128, 0x784540 / 0x78453C; 255 → −1),
    so its lock and rate grow over the 0.55 s it is held. Live (240SX,
    25 Hz frames, A held): +0x1C (the sum) 1, 2, … 14, +0x20 = sum / 14,
    window 0.55, 66 slots; the front wheel angle 23° → 33° over that time
    at 11 m/s. Clone: `SteerAverage`, and the steering now runs once a
    frame before the sub-steps (it ran in every sub-step).
  * lock = gain(|mean|) (0x7E9F00, 1 → 1.35) × maxAngle(speed) (0x7E9ED8,
    40° at rest → 2.9°) × a pedal bonus,
    `(1 − ((throttle + 1) − (handbrake + brake)/2)/2) · k(speed) + 1`, with
    k = 0x7E9F14 (1 → 0.32). Off the throttle the lock grows by up to 50%,
    and more on the brakes. The pedals are the input block +0x20C
    (throttle), +0x210 (handbrake), +0x214 (brake). The brake code
    (0x5A4E80) also reads +0x20C and +0x214 as throttle and brake, and
    +0x210 as the handbrake (verified): × the per-wheel table 0x7FC074 =
    {0, 0, 1, 1} (0x5A4F25), and ≥ 0.5 (0x784260) holds the sliding-bonus
    ramp at −20/s (0x5A4DFE).
  * While |mean| ≥ 0.5 the lock never shrinks: the code subtracts dt × 0
    from the previous lock (the constant at 0x783AF0 is zero).
  * Drift flag (+0x10): set when |slip| > 15° at 13.41 m/s (30 mph) or
    more, cleared below 5°. Slip = heading − direction of travel, wrapped,
    and zero below 1 m/s (0x59FA20 → 0x591DA0).
  * Counter-steer: when the input and the slip have opposite signs and the
    car is drifting, or |slip| exceeds the lock, the lock becomes
    min(|slip|·c, 45·c), with c = 0x7E9EB0(|input|) (0.5 → 0.95).
  * The lock is capped at 45°. The target is lock × ratio (+0x380) × input,
    clamped to ±45°, approached at 180°/s × k(speed) × 0x7E9F3C(|mean|)
    (1 → 1.4) × the rate boost (+0x54, below) × ratio. (Some game modes
    use 110°/s instead.)
  * The speed axis is m/s: the drift threshold is 30 mph written in m/s.
  * **Stored angle** (**verified** 2026-10-31): the new angle
    is kept as a u16 binary angle at +0x7C: ftol(degrees × 65536,
    0x784238) / 360 toward zero (0x416D90…0x416DB4), read back next frame
    as word × 360/65536 (0x784248), less 360 (0x78450C) from 180
    (0x784510) up (0x416C20…0x416C4D); the input update returns that word
    to the car (0x41E048). So the angle moves in 1/65536-turn steps,
    truncated toward zero each frame. Before the store,
    0x416D71…0x416D8C replace the angle by 0 (0x7D7D3C) when |angle| <
    the .rdata constant 0x783AF4, which is 0.0: never. The widescreen
    fix's patch there (0x416D82 jp → jmp, always keep the angle) therefore
    changes nothing, and the steering comparisons of physics rounds 2–6
    (modded copies) stand for the vanilla original. Ported
    (`drive::quantise_steer`).
  * After a wall (0x405E8E): for 1 s after the car last touched a wall
    (body +0x140), the lock is × g + (1 − g)(1 − x), with x = forward speed
    / (0.7 × the car's computed top speed), at most 1, and g over the time
    since (0x7E9F58: (0, 0.2), (0.2, 0.5), (0.5, 0.7), (0.7, 1)). Skipped
    when the game-mode flag 0x890104 is set.
  * The rate boost (**verified**, round 8; `RateAverage`): PlayerSteering
    +0x48 is a second 0x576AB0 average (0x405C60: 0.15 s window 0x783ADC,
    120 samples a second). Each frame, before the rate, 0x4060E0 adds
    curve 0x7E9F28 (1, 1.05, 1.1, 1.5, 2.2, 3.1 over 0..10 /s) of |shaped
    input − last frame's (+0x84, stored at 0x41E042)| / dt, timed by the
    clock 0x865188 × 0.00025; the rate is then × its mean +0x54
    (0x406199). A key press or reversal is 20–40 /s at 20 frames a second
    (3.1), so for 0.15 s the wheel turns about 1.5 times as fast (four
    samples in the window: (3.1 + 3) / 4). The clock is whole ticks, so a
    sample exactly 0.15 s old stays. (Earlier notes said nothing read
    +0x48 back.) The keyboard takes 0x416BB0 (0x41DFC0: 0x406A40 when
    0x81CB8C is set, 0x4061C0 when 0x5C1010 finds an analog device for the
    player, 0x86FFB4 / 0x86E5BC; all three 0 in the Wine runs).
  * The inside front wheel takes δ, the outside one Lδ/(L + δW).
  * A straightening assist (0x5ECF80) turns the car toward a heading the
    input object returns from its vt+0x84. The player's input classes
    (vtable 0x7849C0 and the two derived from it) return 0 there
    (`xor ax, ax`, 0x405BD0), which leaves it off; it belongs to the
    drag-race input.
* **Automatic gears** (0x5AA970 with request −1, each frame from 0x5ABC00;
  shift points 0x5AC7E0, **verified** except the wheel test's meaning):
  * Shift points per gear: forward gears go up 100 rpm under the redline,
    the top gear at max + 500. A torque-matching search exists behind the
    flag 0x86B33C, but nothing sets the flag, so it is off. Down-shift points
    are the previous gear's up-shift rpm carried through the ratio change.
  * At every gear change, three offsets are re-drawn in −100..0 rpm
    (0x594720; drivetrain +0x78/+0x7C/+0x80, the last one for 2nd gear).
  * From neutral it asks for 1st. With the clutch locked:
    * Up: when engine rpm ≥ min(offset + up point, redline), and every
      wheel has +0x11C set and +0x118 < 4: on the ground and the slip
      (r·ω − v, signed) under 4 m/s, so a wheel slower than the road never
      holds the shift (**verified** live 2026-10-31, physics10: a 240SX
      kicked to 40 m/s in 1st shifted at rear slip −7.5 m/s; the clone
      had tested |slip| < 4 and stayed in 1st until the slip came within
      4 m/s — 1.2 s later in a 62 m/s kick).
    * Down: when rpm ≤ offset + down point and rpm ≤ ratio(g)/ratio(g−1) ×
      (previous gear's point − 100) − 200. Never out of 1st.
  * The gear request comes from the input object (vt+0x20, +0x1E4): −1
    automatic or 0 reverse.
* **Pedals** (player input update 0x407A30, then 0x5ABC00):
  * Reverse latch (+0x2F0): set while brake > 0.6 and gas < 0.5, cleared
    otherwise unless backing faster than 2 m/s. Latched, between −2 and
    0.5 m/s the brake pedal becomes a reverse throttle at 2 × pedal (at most
    1) and the brake output is 0; below −2 m/s the brake pedal is the
    reverse throttle and the gas pedal the brake.
  * When the reverse throttle beats the gas by more than 0.2 it becomes the
    gas and the request is reverse (+0x1E4 = 0); otherwise the request goes
    back to automatic (vt+0x30) and the gas is the pedal, clamped to 0..1.
  * Pedal conflicts (0x5ABC00): gas over brake (with the handbrake under
    the gas) clears a brake and a handbrake under 0.5; otherwise a gas
    under 0.5 is cleared.
  * The engine's throttle (+0x78): 0 up to 0.15; a pedal under 0.5 in a
    forward gear under 3000 rpm gives 0.5; else the pedal.
  * An e-brake timer (+0x2BC) forces gas 0 and handbrake 1 above 20 mph
    while it runs. Only AI and scripted inputs set it (0x4214AC,
    0x425DEA); for the player it stays 0.
* **Computed top speed** (0x5B4DD0 → 0x5B5AC0, 0x5B5940; the
  performance object at record + 0xE0, built at 0x5AC740): an acceleration
  table from a standstill in 3 mph steps (at most 92) through the gears,
  rpm = v / r · ratio · final drive (r = wheel radii +0x130 front / +0x190
  rear, r and final drive blended by the rear share), changing up at the
  automatic up-shift rpm, acceleration = torque(rpm) · ratio · final drive
  / r / mass. The game pads the table with zeros from its last entry on, so
  that entry reads 0. Resampled into a 64-point curve over the table's
  span; the top speed is the first 3 mph step where v² · (+0x370) / mass
  exceeds it, capped at +0x3A0 mph. The 240SX gets its cap, 130 mph; a few
  cars are drag-limited (the 106: 141 of 155 mph).
* **Game modes:** free roam (flow state 4) by default; the race-mode
  branches are under "Race modes" below.
* The 0° lateral curve's values are at 0x86B34C in zero-filled data. No
  code writes them or refers to them except the curve's descriptor
  (0x7FC968), so the curve is all zeros.
* Hydraulics (0x5A51C0): see "Hydraulics". Skipped: the race systems
  listed as open under "Race modes".

### Race modes (verified except where noted)

The handling keys race behaviour on flags set from the race type (race
parameters +0xF, 0x5323A0): type 4 → 0x89E7D8 **drag** (verified: it gates
the shift grading and the drag input), type 5 → 0x89E7D9 **drift**
(the drift events), type 2 → 0x89E7E3 (**street X**), set by the career
from the event (0x532377; event types 2, 4, 5); a URL event (+0x34 = 2)
clears them all and sets only 0x89E7E1, which the handling does not read.
0x89E7E2 (the clone's `Url` mode) is what the handling reads; offline only
Quick Race type 11 sets it (0x53FC58's switch → 0x526730), a type the
mode menu does not offer. Online twins: 0x890108, 0x890104/0x890114, 0x89010C,
0x890100/0x890110. The drift test is always "drift flag and not drag".
`NFS_RACE=drag|drift|streetx|url` in the world binary drives with one.

* **Performance class** (0x5B4DD0 → 0x5B5940): the area between the
  acceleration table and the drag deceleration, summed in 3 mph steps until
  drag wins, against 0x7A1A8C = {0, 273, 401, 500}: class 3 − (highest i
  with threshold < area); record +0x844 fixes class 3. It indexes the
  first-gear relief, the shift windows and the shift-bonus grip. Stock
  240SX: 3.
* **Race tune** (0x5A7540, run at set-up with mode 3 drift / 10 type 2 /
  2 drag): row by layout (rear share < 0.2 front, ≤ 0.8 all,
  else rear wheel drive) of 0x7FC2E4 (drift), 0x7FC33C (type 2), 0x7FC394
  (drag), 22 floats each (11 stock, 11 fully upgraded). Every value blends
  the halves by its category's upgrade state L (tuned object +0x2E0, 3 ×
  the mean level; s = L × 0.33333334, 0x786348) as (hi − lo)·s + lo
  (verified): brake balance = s(brakes)·(t11 − t0) +
  old + t0 (0..1), rear share = s(drivetrain)·(t12 − t1) + t1 + old,
  downforce front ×t2 / rear ×t3 (aero), record +0x270 ×t4 (the ground
  yaw-inertia factor) and mass ×t5 (weight), centre of mass z −= t6
  (suspension; ≥ 0), grip front ×t7 / rear ×t8 and sliding front ×t9 /
  rear ×t10 (tyres); then the inertia is rebuilt as a box of the car's
  dimensions and the top speed recomputed. A stock car takes the first
  half exactly (`Handling::upgrade_state`, `handling::upgrade_state`).
* **Shift grading** (drag; 0x5AA970 tail → 0x594F30): each up-shift is
  rated from the rpm against the class's window (0x7FBF60 × the
  difficulty table 0x7A19F0 = 3, 1.5, 1), giving +0x68 of
  0.85, 0.5, 0, or −1 when missed. While positive the clutch engages
  ×(1 + q); negative (from 2nd on) ×0.35. After the clutch locks, power is
  1 + 2q·(0.5 − t)/0.5 for 0.5 s, and every wheel grips × 1 + min(1, q / k)
  (0x5B6310, table 0x7FC2A8: front drive {0.55, 0.7, 0.85, 1}, all-wheel
  {5, 4.6, 4.3, 4}, rear {0.6, 0.7, 0.85, 1}, by class). Up-shifts cap at
  redline (0x594680).
* **Engine heat** (drag; +0x8C, −1 otherwise): +0.45/s above redline, else
  −0.1/s; ≥ 1.2 blows (2). Over 0.6 the torque is × 1 − (min(h,1) −
  0.6)/0.4 × 0.75.
* **Drag start** (0x407310 → input +0x2CC): for 0.25 s from the green
  light the gas is 2× and at least 0.7 (the window opens when the race
  start's hold is released, and again whenever a manual gearbox backs out
  of reverse; see Race types → Drag → **Start, hold and gearbox**).
* **Grip set-up** (0x5A4D90): the airborne ×0.85 and the rear sliding bonus
  (0x5990D0) are off in drift; the rear bonus is off in URL too.
* **Surface** (0x59CBA0): drag and street X put every wheel on surface 1.
* **Aero** (0x59E630): drift shapes downforce by along⁴ with no 0.4 floor.
* **Steering rate** (0x4060E0): 110°/s in drift and URL (0x783AE8), else
  180°/s (0x783AE4).
* **URL input** (0x407A30): the speed the reverse latch reads is divided
  by 4·|yaw rate| + 1, the gas goes through 0x7EA014 (11 points over 0..1:
  0, 0.353, 0.593, 0.71, 0.789, 0.847, 0.896, 0.931, 0.96, 0.984, 1), then
  is held within −1.5/s..+1/s of last frame's gas; 0x5ABC00 skips its
  gas/brake check.
* **Difficulty** (0x89E7F4): 0..2; see **Difficulty: settings and
  effects** (Race types) for its writers and readers. It picks the
  shift-window scale and the drift assist's damping (for every car in a
  drift race: 0x5AC317 reads the global). `NFS_DIFFICULTY=0..2` in the
  world binary; default 0.
* **Drift assist** (0x5AC270, end of each force pass, drift only): slip
  = heading (car +0x3CC) − direction of travel (0x43C570, an atan2; no
  speed gate). When slip × yaw rate > 0 and two or more wheels are down,
  a yaw torque k · g(|slip|) · ω_z about the car's up axis (car +0xB0),
  k by layout × difficulty from 0x7FC2D0 = {−10, −7, −4}, {−7.5, −5, −4},
  {−15, −10, −6}, g the graph at 0x86B3C8 (built at 0x77E040: (0, 0),
  (15°, 0.1), (30°, 0.45), (45°, 0.85), (60°, 0.95), (89°, 1.15), (90°, 0);
  the copy at 0x86B570 ends at 1 and serves cars of player kind 2). The
  rear wheels' grip factor +0x188 becomes c(w|ω_z| + (1 − w)|slip|) · s,
  c the curve 0x7FC910 (1.1 … 0.76 over 0..1), w = 0.35/0.55/0.5 and
  s = 1/1/1.125 by layout (0x7FC2DC, 0x7FC2E0).
* **Nitrous grip** (0x5A5106 → 0x59FA40): while a nitrous shot runs the
  wheels' grip +0x180 is × 0x7A17AC[stage] (1.0 at the only stage
  reachable offline); see Nitrous under Race types. Ported.
* **Open — need systems the clone does not have:**
  * The pre-race hold. A car reset (0x56D350, run from the level's
    fade-in at 0x557B42) holds a full-physics car (car +0x28 = 1, mover at
    +0x34): 0x599210 saves its pose (+0x90, +0xE0) and sets mover +0x84.
    Held, the aero factor (+0x30) is 0, the yaw-torque limit 1e−14 and
    +0x204 0 (0x5A49B0), and with the gas at exactly 1 the undriven
    wheels lose all grip (0x5A4D90), a burnout. Nothing in the physics
    clears +0x84 (0x599210 has one caller, 0x56D415, with 1; byte stores
    to mover +0x84 are only that and the constructor 0x5B5D4B). It goes
    with the mover: 0x5F46B0(mode) frees the car's mover (+0x34) and builds
    a new one whenever the mode changes. **Release** (verified): the reset
    starts the player wrapper's start sequence (0x56E6F0, state +4 = 1,
    target gear +8 = 2, input gear 2 via vtable +0x28). Per frame, by
    state (jump table 0x56E8E8): 1 — full gas, shifting the target up one
    (to car gear +0x434 = 4) each time the rpm (0x4018C0) passes 3000,
    gas 0 while a shift is pending; 2 — gas 1, brake 0, a timer (0x576980;
    car +0x440, or 0x7FBED8 = 100 when that is not 0) until 0x56D450 ≥ 1,
    then automatic gears (−1) → 3; 3 — gas 0, brake 1 above gear 2, until
    speed (car +0x430 × 2.237 mph) is 0 → 4; 4 — 0x56E690(1): mode 0
    (tearing the held mover down), then 0x5F6D60 re-activates the car in
    its wanted mode (0x5F4410) with a fresh mover; state 0. 0x54A956 calls
    0x56E690(0), which only sets state 3. The sequence is the dyno pull
    (verified): the manager is made only in the dyno race (0x56E620, race
    type 8; see "Dyno chart" under Rewards gallery). The spring's gate is the
    byte at mover+0x84, not +0x7C: 0x5B0530 reads `[ebx+0x7C]` and, with edi =
    ebx-8, calls the position spring 0x5A5C30 on edi when the byte is non-zero
    (0x5B068C; `je` 0x5B069A skips the call; k 150, damping 15, x/y only; the
    undriven wheels then spin with the ground). Edi is the mover: 0x5A5C30 reads
    the wheel array at +0x38 that the constructor fills. The constructor
    (0x5B5CD0, called at 0x5F47D5) zeroes the dwords at +0x74..+0x83 (k = 0..3,
    `[ebp+0x3C]` with ebp = mover+0x38+4k; store at 0x5B5F5E), which does not
    reach the gate. The allocator clears nothing (0x575630 is `jmp _malloc`
    0x75C6C8 → `__nh_malloc` → `__heap_alloc`, HeapAlloc with flags 0;
    `___sbh_alloc_block` writes block headers only), so the gate starts as
    whatever the heap held: **open** (writers of +0x84, the constructor's
    callees 0x5AB9A0, 0x5A48B0 and 0x5B04E0 for that offset, and `rep stos`
    clears are not traced). In 0x5A..0x5B the stores to +0x7C are not gate
    writes: 0x5AA7C9 clears it, 0x5AA8F3 and 0x5AA95E store after an x87
    read-modify-write (object not traced); 0x5A12A8, 0x5AA8ED and 0x5AA95B are
    reads, and the bytes at 0x5B7CF7..0x5B7CFF decode as data, not code. The
    clone has no such spring. (Earlier notes called 0x5A5C30 a drag lane lock;
    it is not.) **Ported** for the dyno pull (
    `CarState::set_hold` / `hold_spring`): 0x5A5C30 is a kinematic hold,
    not a force on the body: the point (+0xE0, velocity +0xF0) takes the
    last integrated force sum (+0x130) when it pushes along v (else the
    zero vector 0x86B390), 150 × (saved +0x90 − point) and −15 × its
    velocity, at h/m; it moves by its velocity, keeps the body's z, and
    the body's position x/y is set to it; the undriven wheels' spin
    becomes forward · its velocity / radius (0 under 0.03). The body's
    velocity runs on, so the pull's wheels and rpm are a free run's
    (tests/dyno_pull.rs unchanged: 1.417 / 0.850 / 5.133 s). Held, body
    +0x200 = 1e−14 (0x5A4C53), +0x204 = 0 (0x5A4D0A), and an undriven
    wheel's grip × 0 with the gas exactly 1 (0x5A5124..0x5A5198).
  * The staging rpm (0x5F50B0 → car +0x434/+0x440, fed by the race's
    launch input) and the URL events' 0.6 gas floor that reads it.
  * The race catch-up grip (0x4172D0 → car +0x468 > 1: needs the race
    standings).
  * Knock-over props: the attached models and effects and the props'
    contact effects (their surface index, 0x5D7B60 on info +0xD4, is not
    carried; the cars' and track's contact effects are ported, see
    "Particle effects" under Contacts; the impact sounds of cars, track
    and props are ported, see "Knock-over props" under Contacts). (Car against car: see "Car against car"
    under Contacts.)

## Collision (verified except where noted)

* The stream file has one `0x80034130` pack per section, parsed by
  0x5D8150 and registered by 0x5E78B0:
  * header `0x34131`: section, a grid of w×h 32 m cells, origin, bounds;
  * faces `0x34134`, 32 bytes each (decoded by 0x5D7F20 and 0x5D2CD0):
    * byte 0 is a barrier group, an index into the track bundle's
      `0x3412F` table (32-byte entries {u32 name hash, u32, u8 on,
      name}). Group 0 (`NONE`) is always present; the others count only
      while switched on (checked at 0x5E1037). The groups in L4RA are 60
      `PLAYER_BARRIERS_<race>` and 59 `BARRIERS_<race>`, which a race
      switches on for itself (0x578070), 5 `BARRIERS_CAREER<n>`, which
      free roam switches on for career stage n (wall-offs for districts
      not yet unlocked), and `BARRIERS_DEMO`. About 12,000 walls, all 6 m
      or taller, belong to a group. The clone keeps every face and switches
      the groups at run time as the game does (see "Barrier and scenery
      groups" below);
    * byte 1 packs, 2 bits each, which corner gives the face's x-min,
      y-min, x-max and y-max;
    * byte 2 is the surface (polygon +0xB);
    * byte 3 holds the flags (polygon +0xA): 0x01 a floor on one of several
      levels, 0x02 wall, 0x04 wall reaching ±2048 m (corners 0 and 3 down,
      1 and 2 up), 0x08 not ground for the wheels, 0x10 four corners (else
      three), 0x40 see the height query. L4RA uses 0x00, 0x01, 0x08, 0x10,
      0x11, 0x12, 0x16, 0x18, 0x41 and 0x51;
    * i16 at 4 is a base height in metres; i16 at 6 is the game's cache
      slot (0xFFFF on disk);
    * then x[4] and y[4] in i16 eighths of a metre, and z[4] in i16
      256ths of a metre above the base;
  * `0x34135` (u16s), references `0x34136` {u16 face, u16 4×4 sub-cell
    mask}, grid cells `0x34137` (u16 start, u16 count). Every cell lists its
    floors before its walls and 0x08 faces. The clone uses these cells for
    the height query and the contact cache; its own 16 m grid of triangles
    serves only tracks without packs and diagnostics.
* **Height query** (0x5E72F0, from 0x59D260 for each wheel at its rest
  point; state per car in its collision context, car +0x4,
  src/ground.rs):
  * The pack is that of the drivable section under the point, chosen
    again only once the point has moved more than 9 m plus how deep inside
    its section it was (0x5D7280) from where it was last chosen
    (Manhattan, then straight-line, 0x5E68E0 → 0x5DFEC0). Still inside the
    old section keeps it; else the first outline holding the point in
    most-recently-found order (0x5DB860), else the nearest outline if
    within 0.1 m (0x5DB780). A new pack empties the recent floors.
  * The last four floors found are tried first, newest first: one whose
    outline holds the point is taken if ordinary, or within 4 m under the
    point + 1 m.
  * Then the pack's 32 m cell under the point (truncated, clamped to the
    grid). References whose 4×4 sub-cell mask misses the point's 8 m
    sub-cell are skipped, as are faces whose stored x/y bounds (1/65536 m)
    miss it and switched-off barrier groups; the first wall or 0x08 face
    ends the cell. The outline test (0x5D3340) is the largest edge
    function, negative inside (floors wind counter-clockwise; triangles
    repeat corner 2 as corner 3). The first ordinary floor strictly inside
    is the ground, at whatever height. Floors flagged 0x01 compete: the
    highest at or below the point + 1 m, taken at once within 4 m of that
    unless flagged 0x40. Failing all that, the cell again with no height
    limit.
  * A floor found goes to the front of the recent four unless flagged
    0x40. Height = −(n.x·x + n.y·y − n·v0)/n.z with n from corners 0..2
    (0x5D7F20).
* L4RA has about 112,000 floor faces and 39,500 walls. At the drive start
  the floor is at 17.8 m, as drawn.
* **Barrier and scenery groups** (verified; `World::set_up_barriers`,
  `World::racer_barriers`, src/ground.rs, test
  `races_switch_their_barriers_on_and_free_roam_off`):
  * The barrier table (`0x3412F`, registered by 0x5E78B0 at 0x88F768:
    +0x828 count, +0x82C entries) has 1 in every entry's +8 on disk. The
    region loader (0x57F220 → 0x57DC70 on 0x865158: step 2 loads
    `TRACKS\%s.BUN` for the track record 0x883DAC +0x40, step 5 is
    0x57A590) sets every entry's +8 to (index == 0) once the bundle is in
    (0x5D8360 at 0x57A5DF), then hands on to the track loader (0x581590 →
    0x57F2E0 on 0x8654A8: race or free-roam files, Paths, Routes, then
    0x57A660). A race's or free roam's end (0x5815D0 on 0x8654A8) switches
    the scenery groups it holds off (0x5711E0: 0x5710F0 on +0x18..+0x2C,
    the list at 0x88A28C cleared) and hands the track loader's files (+4,
    +8, +0xC, +0x10, +0x14) to 0x580EA0; the barrier groups' +8 is left
    alone. The next one loads the region again only when its track
    record's region (+0x40) differs or it is a test run
    (0x89E7E6; strcmp at 0x58195A, 0x58196E..0x581994); else it goes
    straight to the track loader (0x581999..0x5819C5) with the region,
    and the switches, kept. The clone: `World::load` and
    `World::reset_groups` are a region load, `World::set_up_barriers` the
    end of the last one and 0x578070 (game.rs `drive_in`).
  * 0x57A660 (from the track loader 0x57F2E0) calls 0x578070 on the track
    loader 0x8654A8, n = the track record's i16 +0x8A (0x5D3E50 of the
    track number 0x89E7A0; free roam is 4000): first the groups named
    `Z1`..`Z19` (0x5D6D80 of 2601..2619, "%c%d") get +8 = 0 when the race
    lists stream sections (0x5DB620, the list at 0x883E98) without that
    one, else 1 — no shipped track (L4RA..L4RR) has such groups, not
    ported; then the road's barrier records are all opened (0x5D4FF0) and
    0x571170 runs for `BARRIERS_<n>` (held at +0x18) and
    `PLAYER_BARRIERS_<n>` (+0x1C), each followed by closing the road
    records naming it (0x5D5020, second argument unused). With no race
    (0x89E7B0 = 0): in a career (game mode 0x850078 = 1) `FREE_ROAM` when
    the stage 0x862838 is not 0, else `FREE_ROAM_STAGE0` when the byte
    0x86347D is set, else `FREE_ROAM_STAGE0A` (+0x28; 0x86347D is not
    traced, so the clone switches neither at stage 0); then in every mode
    `BARRIERS_CAREER<stage>` (stage 5 instead for game modes 0x200 and 8
    with 0x850088 = 8) (+0x2C) and its road records. 0x5F84A0 (three
    multiplayer and special flows, 0x5FCA89, 0x60980E, 0x60AD6A) redoes it
    in place: 0x5782C0 = 0x5711E0, 0x578070, 0x5F54B0 for each car. Only
    0x5D8360, the `Z` loop and the cars' lists (below) clear a barrier
    group's +8 (the writes found through the table pointer +0x82C,
    0x5D3C70's callers and the lists: 0x5D8452, 0x5780E3, 0x5711B4,
    0x578155, 0x5781E0, 0x5E7403, 0x5E76B2, 0x5E58E2, 0x5E5915).
  * 0x571170 (name): 0x5D7410 matches the name against the prefixes at
    0x800E00 — {`BARRIERS_`, 1}, {`PLAYER_BARRIERS_`, 1}, {`FREE_ROAM`,
    0} — case-blind (0x43DD10) over each prefix's length; no match does
    nothing. Else its hash (0x43DB50) goes into a list of 8 at 0x88A28C
    (0x5D7480; cleared by 0x5711E0 and the constructor at 0x5E6489, no
    reader found); with flag 1 the barrier group of that
    hash (0x5D3C70: entries 1.., so never `NONE`) gets +8 = 1; the scenery
    group of that hash (0x5DB350) is switched on (0x5710B0) and returned.
  * 0x5710B0 / 0x5710F0: each listed entry of the `0x34107` table
    (0x5D6440: 0x883DF0 + 8·index) has 0x10 cleared / set in its +4 byte,
    then 0x5DB0F0 writes its u16 at +4 into the instance's flags (+0x1A,
    found by the entry's section and index); +0x2C = 1 / 0. Every L4RA
    entry's u16 equals its instance's flags on disk, and every entry the
    non-`SMOKEABLE` groups list has 0x10: `XO_TrackBarrier*` (flags
    0x210), the start/finish graphics `ZPM_<n>_FW_StartFinGra` /
    `ZPM_<n>_BW_StartFinGra` in `BARRIERS_<n>`, and `FREE_ROAM`'s shop
    beams, spotlights and posts (0x010). 0x10 is in every view's cull
    flags, so these show only while a group lists them as on. 12
    `XO_TrackBarrier_` instances lack 0x10 and are drawn always. The clone
    places every instance (also 0x200 ones) and tests 0x10 per instance
    (the instance's cull radius negated, render/world.wgsl), the other cull
    bits per draw.
  * Race-only scenery: 0x5DB4C0 on a section, when (0x89E7A4 = 1, the
    race run backward) differs from the section's +9, stores it and, for
    each instance with 0x200 (+0x1B & 2), takes its rotation (0x5D6250,
    rows ×1/8192), multiplies diag(−1, 1, 1, 1) × it (0x43C990 → 0x4013F0,
    out = A × B row-major), keeps the position and writes it back ×8192
    (0x5D6330): the local x row negated, a mirror. It runs at a section's
    first load (0x5DFB75) and for every loaded section after 0x578070
    (0x5DFD10).
  * The cars' own lists: the car set-up 0x604270 calls 0x5F54B0, which
    for owner kind 1 or 2 (racers; [[car+0x14]+4]) empties the collision
    context's list (car +0x5C4: +0x2F0 count, +0x2F4 entries) and adds
    (0x5D8100, by hash among entries 1..) `PLAYER_BARRIERS_<r>` and
    `BARRIERS_<r>` with r = the road's +0x28 (0x88F2C0: the race route
    record's u16 +0, the track number, set by 0x5E5BC0; 0 with no race,
    0x5D9A70), and with no race in game mode 1 (or 4 with 0x83AAB4 = 8)
    `BARRIERS_CAREER<stage>`. The height query sets the listed groups' +8
    to 1 before its cell walk (0x5E7403) and to 0 after both passes
    (0x5E76B2); the gather wrapper does the same around 0x5E0D40 when it
    gathers (0x5E58E2, 0x5E5915). So the race's groups always count for
    the racers and, once one has looked, not for traffic (kind 3, no
    list). The clone keeps the list in `GroundQuery::barriers` (player
    and rivals, carried over a re-placement) and the switches shared by
    the ground lookup, the gather and the collision triangles
    (`collision::BarrierSwitches`, atomics).
* A track without packs falls back to the render triangles.

## Contacts (verified except where noted)

The game's world collision step (0x5FFA60) runs after the cars have moved.

* **Car shape:** the car's collision body (vtable 0x7A2478, constructor
  0x5AEC50) is the convex hull named `<CAR>_CV` in the car's GEOMETRY.BIN
  (chunk `0x80034020`: `0x39200` {u32 3, u32 3, u32, u32, u32 1, u32 vertex
  count, u32 name hash, name}, then `0x39201` vertices of 48 bytes with the
  position at +16, then `0x39202`). `240SX_CV` has 37 points spanning
  x −2.05..2.37, y ±0.88, z 0.09..1.04 in car space — the body above the
  axles. The `_W0n_CV` volumes are the wheels'. Without a hull the game uses
  a box x −2.217..2.165, y ±0.971, z −0.2..0.624 (0x5AED76).
* **Track shapes** (0x5B23C0 → 0x5AE940 → 0x5AA0C0; src/ground.rs
  `nearby`):
  * The car keeps a polygon cache in its collision context (0x5E7110 →
    0x5E0D40). The pack is first chosen again for the centre of the body's
    box (position ± radius), with no slack. The cache is kept while its box
    still covers the body's; otherwise it is gathered afresh over the box
    grown 0.1 m and, toward the velocity, by 0.2 s of travel (at most 3 m
    per axis). The gather walks the pack's cells under it, each with a
    sub-cell mask (a run of 1 + trunc((width + 8)/8) bits from the box's
    sub-cell, per axis), tests stored bounds and barrier groups, and keeps
    at most 160 faces: floors, then walls, then 0x08 faces.
  * From the cache, in that order, the first 48 polygons whose bounds meet
    the body's box and that pass the pre-test (0x5937C0) become
    `TrackPolygon<n>` bodies (type 3), sorted by face. The pre-test: with c
    = the car's up · the normal and dist = position · n − d, the plane must
    be nearer than √(1 − c²) · radius + |c| · (the hull's height above,
    or below when c · dist < 0); and the outline must lie within the radius
    in x/y (0x5D3460: 0 inside a non-wall, else the first edge nearer than
    radius + 0.1). The body's radius and heights come from the hull's
    vertices, with 0.05 m added to the heights (0x5AE530).
* **Detection:** a convex distance query (0x5B3D30 → 0x5B33A0) between hull
  and polygon: overlapping at ≤ 1e-5 m (compare at 0x59853E against 0x7A1934, stored at
  +0x15), touching at ≤ 0.001 m (compare at 0x59855D against 0x7A1930). A
  touching pair is a contact at the frame's end (back-time 0: 0x5B4BF0 returns
  0 when the pair is not overlapping); an overlapping one is moved back in time
  (0x5B4BF0):
  eight halvings between the end of the frame and its start, keeping the
  latest time at which they are clear, ending early when they just touch.
  Poses in between are the start and end matrices blended linearly
  (0x593290). The distance query (0x5B33A0, core 0x595050; src/narrow.rs) keeps up
  to four vertex indices per shape and remembers them per pair between
  calls (+0x118). **Verified** (0x5B33A0): +0x118 is set only when the
  first supports are apart (squared distance ≥ 1e-5); an overlap there
  returns 0 with it still clear, so the next call starts afresh. Each round it adds both shapes' extreme vertices along
  the current direction and measures the two small sets again, stopping
  when nothing is new or the distance stops shrinking (then going back a
  step). The core tries point/point, point/segment, segment/point,
  segment/segment, triangle/point and point/triangle, in that order, and
  takes the first candidate that separates the sets with its own vertices
  extreme, else the nearest separated one; none means overlap. In the
  triangle cases the direction is the triangle's normal (t1 − t0) × (t2 −
  t0), normalised, turned toward the other set; the inside test (each
  edge's cross product with the normal, dotted with the point) takes the
  **unturned** normal kept at esp+0xB0 (A's triangles 0x597245 / 0x597304 /
  0x5973C3, B's 0x597B17 / 0x597BD6 / 0x597C95; the turned copy goes only to
  the output direction, 0x5977CD–0x5977FA) — **verified**. (Before <work dir> the
  clone tested with the turned normal, so a triangle under the hull never
  matched; warm-started from remembered edges the query then found no
  candidate and reported an overlap 0.4 m above a road face, and each such
  false floor contact pushed the car up by at least 0.3 m/s: cars hopped on
  flat road and rolled over after a side hit. Probe:
  `devprobes/src/bin/weight_probe.rs`.) No candidate returns 0 (0x59811A loads
  0x7D7D3C = 0.0), which 0x5B33A0 takes as overlap. In the
  segment/segment case with both points inside, the direction is the best
  of the edges' cross product and the two perpendiculars, judged by the
  gap between the segments' lowest ends. Worked in double precision about
  the hull's first vertex (the x87's 64-bit intermediates; walls reach
  ±2048 m).
* **Order:** a contact's +0x8 is its back-time, the time before the frame's
  end (0x5B4FA0 stores 0x5B4BF0's result). The list is kept with the largest
  back-time first (0x43BEE0 with the comparator 0x5A0EC0, which skips each entry
  whose back-time is at least the new contact's), so the head is the earliest
  contact, resolved first (0x5FFA60, then 0x5B2620): the car goes back to that moment (0x593640: pose and velocity
  blended; the angular momentum stays), takes the impulse, its state there
  becomes the new start (0x593220), and its whole physics update runs again
  for the remaining time (0x6012C0). Its other contacts are then found
  afresh. At most 20 contacts a frame (0x802080) and two per pair (0x802084; the
  pair's count is at +0x240). Open: whether a track pair is always one car and
  one polygon. The sweep 0x5B23C0 pairs a swept body with any overlapping
  candidate (a type-2 one, +0x11, is first mapped by 0x5B3200) that passes its
  group-bit, vt+0x54 and 0x592970 filters; the creator 0x5A0350 rejects none,
  and the gate 0x5B5060 restricts only type-4 bodies (+0x11) to partners of
  type 0 or 1.
* **Suspension refresh at a resolution** (**verified**, round 7;
  `CarState::refresh_suspension`): 0x5B2620 calls each body's vt+0x38
  before and after 0x5AF0F0 (after the move back, 0x593640, and the
  push-out). For the car collision body (vtable 0x7A2478) that is 0x593710:
  with a rigid body (+0x5C) it jumps to the vt+4 of the object at +0x64,
  the drive mover's +8 subobject (vtable 0x7A25DC, live: mover + 8), i.e.
  0x5B04E0: every component in its list (+0xC; live order: rigid body
  0x59C600, then wheel 0x59D040 / suspension 0x5A87A0 for each wheel,
  0x5AA770, 0x5A1410) gets its vt+0x1C update, then 0x5ABB10, then vt+8
  (0x5B0530). The sub-step length 0x7FC800 is 0 then (the movers write it
  only inside their sub-step loops, 0x5AB084 / 0x5AB51A; read 0 between
  frames live), and the suspension update 0x5A87A0 stores the compression
  at the moved-back pose (+0x1C, clamped to the travel; +0x80 the contact
  flag) but writes its rate +0x20 only when the sub-step is over 0.0001 s.
  So the first sub-step of the rest of the span measures the damper's rate
  from the moved-back pose, not from where the span ended. (The clone took
  the stale end-of-span compression: a resolution 0.03 s back gave a rate
  of 9 m/s and loads of 37–48 against 3 — the roll and lift of the
  ≥10 m/s wall hits.) **Inferred** (not ported): the refresh's own force
  +0x24 (old rate) and the other components' dt-0 updates, including the
  rest test inside 0x5ABB10, have no lasting effect the clone needs; the
  clone does not store them.
* **Impulse** (0x5AF0F0): the contact point is midway between the two
  closest points; the normal is the polygon's (+0x70), turned by 0x593E00
  (**verified** from the disassembly: stdcall, ret 0x10, this in
  ECX; out, X = the normal provider at [+0xC], P = the other body at [+0x10],
  B = an optional override at [+0x14]). The gate: X+0x64 = 0 returns n
  unchanged (the flag DAT_007FBF18 is 1 in the file and only read there).
  Otherwise C = X->vtable[+0x7C](X, 0) (0x59E5A0 returns the pointer stored
  at X+0xC; for a track polygon 0x5AA0C0 sets it to the rigid body at X+0x70),
  A = C+0x1F0 when C+0x1D0 is set and C+0x1D4 > 0, else C+0x1E0; B = the override
  when given and P+0x11 is set, else P+0x100. The latch s = (this+0x14)->+0x248
  when P+0x11 = 1, else 0 (flag
  DAT_007FBF14 is 1, so the state machine at 0x593F0D runs). s = 0 tests
  (B−A)·n: negative gives −n and stores 2, else n and stores 1; s = 1 gives
  n and s = 2 gives −n (s = 1 and 2 leave the latch alone; any other s leaves the output unwritten, 0x593FFB). For a track polygon C+0x1D0 is 0 (0x5AA0C0 stores 0
  at 0x5AA155; 0x5A1AA0 stores no +0x1D0), so A is the mean of the face's
  corners (0x5A1AA0 sums the corner offsets from a zero vector passed at
  0x5AA24F and scales by 1/count, with 0x784250 = 1.0), on the face plane for a
  planar face; the 1 m thickness (the float at 0x7A1880, passed at 0x5AA225 and
  stored at C+0x1D4 at 0x5A1ADA) does not move A, since +0x1E0 is written before
  +0x1D4 is read. Against a track face the clone's test (drive.rs,
  resolve_track) now takes A as this corner mean, so it has the game's sign
  there too; smackable.rs resolve_track (props, not changed in the wf8 pass) still tests
  against corners[0], with the prop's own centre of mass (Body.centre, body
  +0x20) for B. drive.rs takes the car's centre of mass for B. The remaining
  differences are B (both places) and the latch. **X+0x64 gate (ported, wf9):**
  the writer 0x5AA0C0 stores (face[0] != 0) at 0x5AA1FD-0x5AA203 (`cmp BYTE PTR
  [edi],0; setne al; mov BYTE PTR [esi+0x64],al`, edi = arg3 = the face record:
  0x5AE940's loop passes `*(candidate+0xC)` (inferred to be the face record: the
  writer dereferences it, and the loop sorts by it in face order), and 0x5AE940's
  only caller is 0x5B23C0; the writer's other caller 0x5AA290 is not traced). So the face's byte
  0 alone decides it (the barrier group in collision.rs; the binary shows only the
  nonzero test), whatever its flags or surface. The same flag is also stored as 0
  at 0x5B62CD. The gate at 0x593E34-0x593E39 (`mov al,BYTE PTR [ecx+0x64]; test
  al,al; jne 0x593e5a`) runs the turn only when it is set, with DAT_007FBF18 = 1
  (read at 0x593E23; REA xrefs show no other reference; `je 0x593e5a` at 0x593E32
  would turn always if it were 0); otherwise the fall-through at 0x593E3B returns
  the raw normal. The clone keeps the flag per polygon (`TrackPolygon.turn` in
  ground.rs, group != 0; a track without collision packs has no face records and
  keeps the turn) and applies it in drive.rs resolve_track and smackable.rs
  resolve_track. Checked in <work dir>: the gate sits inside 0x593E00, the callee of
  all six direct call sites (the four listed below and the push-out calls
  0x5B435B and 0x5B437E), and no jump targets 0x593E34-0x593E3F, so every call
  runs it. DAT_007FBF18 is 1 in the file image; REA xrefs and the disassembly both
  give 0x593E23 as its only direct reference. The
  tested byte is the second argument's +0x64 (0x593E0D loads the second argument
  into ecx and 0x593E34 tests `[ecx+0x64]`). In 0x594010 and 0x5A0DD0 (`this` in
  ecx) let X = [this+0xC] and P = [this+0x10]; the gated object at each site is:
  0x5940B6 X (when X+0x12 = 0); 0x5940DB P (when X+0x12 != 0 and P+0x12 = 0);
  0x5A0E01 X (when X+0x11 = 3 and P+0x12 != 0); 0x5A0E4D P (when X+0x11 != 3,
  P+0x11 = 3 and X+0x12 != 0). The turn runs when that byte is nonzero; the raw
  normal returns when it is 0. The prop push-out 0x5B4300, called from 0x5B4D6D,
  gates the same way with the same X and P: 0x5B435B gates X (when X+0x11 = 3) and
  0x5B437E gates P (when X+0x11 != 3 and P+0x11 = 3), so the push-out runs the
  same gate on the +0x64 byte of X or P. Whether that byte is the face-record flag
  is **inferred, not verified**: the writer 0x5AA0C0 stores `(first byte of its
  third argument) != 0` at [this+0x64] (0x5AA1FD-0x5AA203: `cmp byte [edi],0`,
  `setne al`, `mov [esi+0x64],al`), and the call at 0x5AEBED (REA caller 0x5AE940)
  gives it a `this` equal to the result of 0x440E50 (a tail jump to 0x440D40). The
  other writer call,
  at 0x5AA345 in 0x5AA290, is not traced, and the X and P pointers of the contact
  pairs are not traced to that object. The clone already carries the per-polygon
  flag into both track-contact paths (drive.rs:1640, smackable.rs:551 and :999),
  so the <work dir> pass made no code change. **Open:** P+0x100 for a car body. 0x5A9DF0 (REA
  callers 0x5ADB30 (call at 0x5ADCF9), 0x5ADD60 (jumps at 0x5ADDEF and
  0x5ADE0D) and 0x5AEC30 (call at 0x5AEE75, the car collision body's
  constructor, which stores its vtable 0x7A2478 at 0x5AEC6F); it also calls
  itself at 0x5A9EC5 and 0x5A9F3E) stores
  P+0x100..+0x108 from the body's vtable slot 2 pointer (for the car collision
  body, vtable 0x7A2478, slot 2 is 0x593B50: the owner's +0x60, the owner stored
  at +0x130 by 0x5AEC30, when P+0x5C is 0, else [P+0x68]+0x20), widened by
  P+0x1C, through 0x5A0690
  (it sets P+0x20..+0x2C) and 0x43C9C0 on P+0x70 and P+0x110, plus P+0xB0.
  Nothing traced shows that is the centre of mass, so B stays the car's centre
  of mass (0x43C9C0 and P+0xB0 are not traced). Other register-based stores to
  +0x100 occur at 0x584EBD, 0x590B23, 0x5998A7 and 0x599E44, none traced to a
  car body (0x5B04B0 is `lea eax,[ecx+0x100]`, a vtable entry at 0x7A2600); the
  latch's reset (0x593E00 itself stores +0x248 at
  0x593FCC and 0x593FF1, and other routines store +0x248 too, e.g. 0x425B92 and
  0x559E51, none traced to the pair's record); and P+0x11 for the car.
  * Separating contacts (v·n ≥ 0) are skipped.
  * j = −(1 + e)·vn / (1/m′ + n·((0.4·I⁻¹(r × n)) × r)), with the car's
    mass taken as at least 1.85 t (0x593A10) and its inverse inertia × 0.4
    outside crash mode (0x593A50), × 1.0 in it: 0x593A50 and 0x5A0B10 test
    the car's own crash flag (car +0x38) whatever the partner, so against a
    track polygon too (verified; `contact::Body::crash`).
    j is at least 0.3·m′, a 0.3 m/s push.
    I⁻¹ here is the rigid body's **body-space** inverse inertia (+0x1B0 /
    +0x170, built once by 0x59B950), copied × 0.4 by 0x593A50 into body
    +0x150 and applied by 0x5AF0F0 straight to the world-space r × n
    (0x5AF3B4 vt+0x60 → 0x43D1E0 → 0x43C9C0 at 0x5AF3E1, the same for B at
    0x5AF43B..0x5AF468): it is not turned into the world, unlike the
    integrator's R·I⁻¹·Rᵀ (0x59C600, the 0x43C990 products after
    0x59C72A). **Verified** live (round 6): ported for track, car and
    prop contacts' car term.
  * Friction μ·j against the sliding direction, × the sliding speed under
    1 m/s, none under 1e-5.
  * The arm loses its part along the car's up (the push acts level with
    the centre of mass) only for a car with all four wheels down on a
    contact whose record has had a crash (record +0x10, set only by
    0x59FB80 below) — never for a track contact. (Earlier notes applied it
    to every contact with four wheels down; 0x5AF78F gates it.)
  * Applied (0x5A0B10 → 0x59BD90) scaled by m/1.85 for cars under 1.85 t:
    P += J (capped at 1000), L += 0.4·(r × J) (capped at 10).
* **Car against a prop up against a wall** (0x5AF0F0, 0x5AF8D8–0x5AF97B;
  **verified**; `smackable::against_wall`): when one body is a car
  (type 0, +0x11) and the other a type-1 body (a prop) whose int +0x120
  is above 0, the car's impulse z is × 0.25 (0x7A18B0) and floored at 0
  (0x7D7D3C). The slots are the impulses handed to vtable +0x50 at
  0x5AF991 / 0x5AF9A6: B's J at esp+0x80 and A's −J at esp+0xB0
  (0x5AF728..0x5AF783), so esp+0xB8 / esp+0x88 are A's / B's z. Body
  +0x120 counts live contact records with a wall: the record constructor
  0x5A9440 adds 1 to the pair's non-polygon body when 0x5A0170 finds a
  track polygon (type 3) with |+0x60 → +0x78| < 0.1, and the destructor
  0x5A9580 takes 1 off (floored at 0); car–car records count in +0x124
  instead. The clone counts the prop's live track records whose face had
  |n.z| < 0.1.
* **Materials** (0x5A0CA0, 0x5A0BF0; the car's win against a polygon,
  0x5941D0), by the polygon's normal z: walls (|n.z| < 0.1) e = 0.05,
  μ = 0.09; floors (|n.z| > 0.7) e = 0.1, μ = 0.3 × (1 + 0.17 per wheel off
  the ground), at most 1; the rest e = 0.1, μ = 0.3.
* Touching a wall-flagged polygon (0x02) restarts the post-crash grip
  timer (body +0x140, set by the pair handler 0x5B5060 through 0x592EE0).
* **Overlap for the whole frame** (0x5B4300): when the bisection never
  finds a clear time, the car is pushed out along the polygon's normal
  (facing the car): 0.005 m, then steps growing by 0.01 a try while under
  0.1, the vertical part held to ±0.15 m, until clear or 20 tries. The
  resolver adds that offset to the car's position before the impulse
  (0x5B2620).
* **Car against car** (verified except where noted; `drive::step_cars`):
  * One step for all bodies: each car's frame (0x6012C0: the mode update
    0x5FC060, then the mover), then 0x5FFA60 gathers track polygons and
    body pairs and resolves their contacts from one list sorted by time,
    earliest first (0x43BEE0 with 0x5A0EC0; ties keep the earlier found),
    at most 20 a frame and 2 per pair; after each, the two bodies' other
    contacts are found again. Contacts are gathered pair by pair in the
    pair list's order (0x86B490; 0x5A0350 appends new pairs at the tail),
    so of two equal times the older pair's wins. Track pairs are in the
    same list: 0x5FFA60 runs 0x5B23C0 for each moving body (+0x12 and
    +0x19 set) in the body list's order (0x86B4C8, by box high x +0x28);
    it gathers the car's polygons as TrackPolygon bodies (0x5AE940), then
    sweeps the list over its box and appends a pair (A the sweeping car)
    for each body it meets that has none yet — polygons and later cars in
    the list's order (the only other 0x5A0350 caller, 0x5AE3A4, restores
    saved pairs). After a resolution the moved bodies sweep again
    (0x5FFC78): only the event's own bodies, A then B, each by the same
    per-body sweep (its polygons and car pairs merged by high x; ported
    `resweep`). **Open**: the original then re-posts the
    moved bodies' pairs (count < 2, 0x802084) in pair-list order into its
    time-sorted list (0x43BEE0, comparator 0x5A0EC0); the clone searches
    every pair again and takes the lowest stamp of equal times, and the
    tie rule of 0x5A0EC0 is not read. Every pair the event search finds
    touching and not ghosted stamps both car bodies' contact clock (0x5B5060,
    +0x138; the clone's `since_collision = 0`), and the record and effects
    take the uncapped |J| while only the crash check 0x59FB80 holds it to
    40 (0x7A1860): both **verified** native. In the race start
    scene and countdown there are no car pairs (**observed**:
    0x593990 refuses with body +0x17 = 1, the countdown bodies have +0x19
    = 0; `CarState::start_hold`). The clone stamps every pair from one counter
    (`drive::next_pair`) in that order and takes, of equal times, the
    lowest stamp.
  * Pairs (0x5B23C0 sweep over the body list sorted by box edge; made by
    0x5A0350, A the body found first): two cars whose boxes (position ±
    radius in x/y, 0x5A9DF0; overlap 0x592970 with no slack) meet and
    that both collide with cars (body +0x190, 1 from the constructor
    0x5AEC50; the racer-ended handler 0x606CC0 clears it in drift and
    type-2 (Street X) races, online, or for a racer whose status is 8 or 9).
    A pair goes when its boxes part and it has no live record (0x592E70).
  * Detection (0x5B5060): the distance query between the two hulls; at
    ≤ 0.001 m, unless both cars have +0x12 set and either is ghosted (vt+0x6C:
    car +0x998, or +0x974/+0x978 a timer — 0x606CC0 ghosts a racer that
    ends its race for 2 s), a contact: only then do both cars' post-crash grip
    timers restart (+0x138; +0x140 through 0x592EE0, which counts a car as a
    wall) and the contact joins the sorted list (0x43BEE0). A body with +0x12
    clear always passes this test. Overlapping (≤ 1e-5), its time by
    eight halvings over the shorter of the two cars' spans, each car's
    hull blended over its own (0x5B4BF0); overlapping throughout, both
    are pushed apart along A → B by the growing steps of 0x5B4300 (A back,
    B on, the vertical part ±0.15). A → B runs between the bodies' positions at
    the moment (vt+0x34 = 0x593290 from vt+0x08 = 0x593B50: car +0x60, the
    centre of mass), not the car origins (verified, the drag-rub pass 2026-11-04;
    docs/tuning-physics-parity.md B).
  * Resolution (0x5B2620): both cars back to the moment, the impulse,
    then each car simulates the rest of the frame from there.
  * Point and normal (0x594010): midway between the closest points; the
    normal from A's point to B's (between the positions under 1e-6). The
    points are the pair's +0xC0 / +0xD0 as they stand: 0x5B2620 measures
    nothing between the time of impact's search (or the push-out) and
    0x5AF0F0, so they are that search's last query's, usually at another
    moment of the search than the one the bodies are moved back to (read
    under Wine for a car–wall contact at 120 km/h, docs/prop-hits.md
    section 7d). The clone does so for track and prop pairs; its car–car
    contact point is still measured again (**open**).
  * Impulse (0x5AF0F0, both bodies through their vtables): v = B's point
    velocity − A's; skipped when v·n ≥ 0. e = the larger of the cars'
    (0x5A0CA0: 0.1 against a car), μ the larger (0x5A0BF0, 0x5941D0: 0.3).
    j = −(1 + e)·vn / (1/m′A + 1/m′B + n·((I′A⁻¹(rA × n)) × rA) + the same
    for B), at least 0.3 / max(1/m′A, 1/m′B); m′ = max(m, 1.85 t)
    (0x593A10), I′⁻¹ the rigid body's current inverse inertia × 0.4 outside
    crash mode (0x593A50). Friction as against the track. B takes J, A −J,
    each through 0x5A0B10 (× m/1.85 under 1.85 t; P cap 1000, L cap 10,
    the angular part × 0.4 outside crash mode, whole in it).
  * Contact records (0x5A9B20 / 0x5A9440, one per pair, track or car):
    made when the pair's first contact is resolved (even if it then
    separates), refreshed by each push (0x5A9630), dropped 0.3 s after the
    last (0x592BE0, 0x7FBEE0). A body with a live record is in contact
    (+0x18).
  * Crash (0x59FB80, called by 0x5A9630 before the push is applied): with
    |J| (≤ 40) of 30 or more, a car not already crashed whose pair is car
    against car: the record's crash flag (+0x10), crash mode (0x5F6BF0:
    car +0x38 = 1, +0x54C = 0), and with d the push's direction on that car
    and v̂ its direction of travel: if v̂·d < −0.97 (head on) a sideways
    push −|v̂·d|·0.2·|J| along d less its part along v̂ (level); then a
    vertical push clamp(|v̂·d|·0.1·|J|·f, ±4.1·f), f = 1.1 (0x7A1864), 1.2
    (0x7A1868) in drag and online races (0x5A0010, verified; ported
    2026-11-04) at the contact point in the car's frame with its height
    dropped — sideways ×1.19 within ±1.19 × half the front track (record
    +0x124 − +0x154), or head on its front/back mirrored ×1.1 when the
    world's random number (0x5EA4F0, the generator 0x43C1C0 on
    0x890080 +0xF8, seeded per race) out of 100 is 50 or more and its
    sideways part ×0.2 past 0.13 m. Not ported: the crash camera, rumble,
    sparks and damage (0x593C00), and the AI's "got hit" notice
    (0x593C60). In drag races any push of 30 or more (or the body matrix
    +0x38 under −0.5) totals the car (+0x964; racer status 9, see "Drag
    drop-out") — ported as CarState `totaled`.
* **Knock-over props** (smackables; src/formats/smackable.rs,
  src/smackable.rs; verified except where noted):
  * Kinds: the track bundle's `0x34026` chunk (L4RA: 79), 0x100 bytes each
    from a 16-byte aligned start, read by 0x5B0E40: list links at +0,
    name at +8 (`ZCS_PylonA`; the `ZCS_` volumes in the bundle's
    `0x80034020` pack share the names), the key instances use at +0x80,
    four more hashes at +0xA0 resolved to infos at +0x90 (the
    `XO_BulletSS_*` kinds point at `ZCS_BulletSS`), a hash at +0xD4
    resolved by 0x5D7B60 into +0xD0, and the material hash at +0xDC whose
    index into the 19 names at 0x7FC808 (`NONE`, `GARBAGE_BAG`, …,
    `TRAFFIC_CONE`, …, `CHAIR`) goes to +0xD8. +0x20 is the rigid-body
    description (0x5B1550 → 0x5A7F40 → 0x59B950): +0x20 the mass in
    tonnes (body +0x164, 1/m at +0x168), the 4×4 at +0x30 the inertia
    (inverted by 0x43C7C0 into body +0x170); +0xB0 the centre of mass
    (prop space, body +0x1F0); +0xC4 the break impulse (copied to +0x150);
    +0xCC the restitution, held to ≤ 0.005, and +0xE0 the friction, held
    to ≥ 0.3 (both written back by 0x5B1550; returned by vtable +0x70 /
    +0x74, 0x5B1CC0 / 0x5B1CD0). L4RA: break 0 for most, 0.1–0.2 for
    signs, the water barrel and the door; μ 0.3–0.6.
  * Placements: one `0x34027` chunk per stream section (L4RA: 514
    props), a 16-byte header {u32, u32, i16 section, i16 first index into
    the made-already bit set 0x86AD78, i16 count}, then 0x70 bytes each:
    the kind's key, u16 the scenery instance it stands in for, world
    bounds at +0x10/+0x20 and a 4×4 row-vector matrix at +0x30
    (orthonormalised on first load, 0x5BB0E0).
  * The scenery link: the u16 indexes the track bundle's `0x34107` table
    (read by 0x5DF8B0 into 0x883DF0; 0x5D6440 returns entry + 8·i; L4RA:
    6431 entries), 8 bytes each: i16 the scenery section's own number
    (the scenery chunk's +0x0C, e.g. 104 — not the stream section, 124),
    i16 the instance's index in that section, u8 flags (0x10: a prop
    stands in for it, set by 0x5B0C40 and cleared by 0x5AD900, then
    0x5DB0F0), u8, u8 kind (+6), u8. Sorted by the section's low byte
    (0x5D63F0 keeps each byte's first entry at 0x883AC0). All 514 L4RA
    props resolve to the scenery placement at their own position, kind 1.
  * When a section loads (0x5B0C40) each placement whose bit is clear and
    whose kind's key is known gets flag 0x10 on its table entry; it
    becomes an unhit body (0x5AD900, `VirginSmokeable`, type 2, vtable
    0x7A23A8; only while the pool 0x86B370 has room) when the entry has
    kind 1, or else when the entry is listed in a scenery group that is on
    and is not named `SMOKEABLE` (the strcmp 0x43DC30 at 0x5B0D80 must be
    non-zero): the placement's world box (0x5A0690) with half its
    diagonal as radius, and no shape (+0xC = 0). The bit is set after the
    placement when its +0xC is 0; a placement whose key matches no kind
    gets the bit at once (the first four such keys are counted). Scenery
    groups: the track bundle's `0x34108` chunk (0x5DB2F0 links each into
    0x88F760; 0x5DB350 finds one by hash at +0x28): name at +0x08 (0x20
    bytes), +0x2C on, +0x30 a count, from +0x34 that many u16 indices into
    the `0x34107` table, each entry (count·2 + 0x37) & ~3 bytes. 0x571170
    switches a group on by name: the like-named barrier group (0x5D7410,
    0x5D7480, 0x5D3C70 +8 = 1) and the scenery group (0x5710B0: flag 0x10
    cleared on each listed entry, 0x5DB0F0, +0x2C = 1) — a race its
    `BARRIERS_<n>` / `PLAYER_BARRIERS_<n>` (0x578070), free roam
    `FREE_ROAM`, `FREE_ROAM_STAGE0` or `FREE_ROAM_STAGE0A` (by 0x862838 /
    0x86347D) and `BARRIERS_CAREER%d`. L4RA has 130 groups, all off on
    disk; `SMOKEABLE` lists 601 entries, covering all 514 props' (all kind
    1), and no other group lists a prop's, so the groups add no prop on
    L4RA (test `smokeable_group_lists_every_prop`). The clone reads the
    groups (`formats::smackable::parse_scenery_groups`) and lets an on,
    non-`SMOKEABLE` group's listed entries of another kind in
    (`Prop::pair`), with the groups as loaded: the game decides this as a
    section's props are placed, after a race has switched its groups, so
    a track whose race groups list prop entries would need the pairing
    redone (none on L4RA). How the groups switch: "Barrier and scenery
    groups" under Collision.
  * Waking (0x5B23C0 → 0x5B3200 → 0x5B13A0): a body sweeping the list
    whose x/y box meets an unhit prop's turns it into a prop body made by
    the factory at 0x86AF90 (0x5B41C0: pool 0x86B374, 0x510-byte slots,
    flags 0xB so slots come zeroed; none when the pool is full,
    0x43CD10), lifted clear of the ground under it with a draw offset
    (+0x168) to match, placed at rest at its pose (0x5ADB30: +0x6C, the
    body's last span, 5000; the start pose; the post-step once), the bit
    set so it does not come back while the track is loaded; the unhit
    body is deleted and the pair made with the new body (0x592970,
    0x5A0350).
  * The prop body (vtable 0x7A26A0, made by 0x5B3250 on the
    0x5A9CF0 base, type 1): its shape (+0xC, 0x5B1550) is the `ZCS_`
    volume whose header name hash is the info's key (0x5A18D0 over
    0x86B598; 71 of the 79 L4RA kinds; the volume's +0x40 takes the
    centre of mass, so its points are about it), else a box of the drawn
    solid's half size about the centre of mass (0x611040 on the model at
    +0x480, 0x598770). Its reach for the polygon pre-test is the car's
    (0x5AE530 over the shape). The rigid body's position (+0x20) is the
    centre of mass. While +0x150 (the break impulse) is non-zero, vtable
    +0x5C / +0x60 (0x5B1C20 / 0x5B1C50) give zero 1/m and I⁻¹ and +0x64
    says fixed, so the car takes the whole impulse as against a fixed
    body (0x5AF0F0: e, μ the larger of the two sides — the car's 0.1 /
    0.3 against a body, the info's; j at least 0.3 / the larger 1/m′).
    The impulse J then goes to vtable +0x4C (0x5B2080): with +0x150 = 0
    the prop starts moving (0x5ADFA0: +0x506, and the attached effects of
    the info's +0x88 list, 0x48BC50); otherwise s = max(|J|, |(c + r) ×
    J|) (c the centre-of-mass offset, r the arm); under +0x150 nothing
    happens; else it starts moving, takes −(t / s)·J at −c, +0x150 is
    cleared and the car's impulse is scaled by t / s. A moving prop's 1/m
    and I⁻¹ are its own (no 1.85 t floor, no 0.4); impulses go straight
    to its rigid body only while it moves (0x5B1CF0 → 0x59BC70, no caps
    traced). The ground inverse inertia (+0x1B0, +0x158 = 1) has the yaw
    term × info +0x70, 1 for every L4RA kind. In the contact impulse
    (0x5AF0F0) that I⁻¹ is the **body-space** tensor applied to the world
    r × n unturned (0x5B1C50 → +0x1B0, product 0x43C9C0), as for a car
    (0x593A50); only the rigid body's own updates turn it. **Verified**
    and observed (docs/drag-stage4-parity.md §7.1: a Skyline through
    a yawed ZCS_FenceH took |J| 5.13 natively; the turned tensor gave 3.36,
    the unturned one 5.12).
  * Its frame (0x5B1DE0): the start pose and span saved (0x593220:
    matrix, position, velocity, dt into +0x6C); if moving, the body's
    mass and inverse inertias refreshed (0x59B950), rest damping switched
    on (body +0x150 = 1), then ⌊dt / (1/60)⌋ + 1 parts (0x7FC858), each:
    * a quadratic drag −k·|v|·v when +0x4F0 > 0 — never: nothing writes
      +0x4F0 and the slot comes zeroed;
    * the integrator 0x5AA490 on the prop's list (+0x130, list +0x13C,
      holding the rigid body): every member's derivative first (vtable
      +0x18, 0x5A0D30), then state += h·derivative over its 32 floats
      (vtable +4, +0xC: body +0x20..+0x9C — position, the orientation
      rows, linear momentum P, angular momentum L, the rest damping D);
      the derivative is the car's rigid-body terms (0x59C1F0) with no top
      speed or yaw limit (+0x204, +0x200 = 0), angular damping on (+0x154
      = 1) and rest damping on: dx = v, d(row) = ω × row (0x59C130), dP =
      F − 8DP, dL = T − g(|ω|)·L − 2DL, dD = −10 or +0.1;
    * re-orthonormalising (0x5BB0E0: x normalised, z = x × y normalised,
      y = z × x);
    * the post-step (sim vtable +4, 0x5A8E80): the tidy-up of each member
      (0x5A0D90 → vtable +0x1C, 0x59C600: v = P/m, held to 200 m/s only
      over 0.7 t; ω = I⁻¹L held to 250; D ≥ 0; forces cleared and −m·g
      queued when the gravity +0x160 > 0), then the settle test
      (0x59E440, into +0x508): |v| and |ω| ≤ 2 with a live contact record
      (+0x18) or the last answer 2 → under 0.8 each and 0.1 s since a
      record with a car ended (prop +0x160, written by 0x59E540 when a
      record whose other body is a car goes) gives 2, else 1; otherwise
      0. 0: gravity 9.8. 1: P and L × 0.95, gravity 9.8. 2: P = L = 0,
      gravity 0. After 1 or 2 the tidy-up runs again.
    Then "No activity" (0x5A8DA0 → 0x59E410): 0.1 s since a car record
    ended, a live record whose last push (+8) is over 0.025 s old (the
    oldest, 0x5A0830(0)), |v| and |ω| under 0.1 → it stops moving (+0x506
    = 0) until pushed again. A draw offset +0x168 decays at 0.2/s
    (0x7FC85C). +0x154, a second threshold that halves with each impact,
    starts 0 (not ported).
  * Contacts: the same as the car's (0x5FFA60; the polygon gather,
    0x5B5060 with both shapes, the distance query, eight halvings,
    push-out, 0x5B2620). Against the track e and μ are the larger of the
    info's and the polygon's (vtable 0x7A22B0 +0x70 / +0x74 → 0x5AA060:
    0.001), j at least 0.3·m. Records (the pair's, refreshed by a push,
    gone 0.3 s after the last) count in +0x18 (0x5A05A0 / 0x59E540).
  * The clone (`src/smackable.rs`; `drive::step_world`, which
    `bin/game.rs` runs for the player in free roam, all racers in races
    and the cars in traffic, keeping the props' poses across renderers)
    follows 0x5FFA60: every woken prop's frame runs with the cars' before
    the collision step; the sweep wakes props when the cars are placed and
    again after each resolution; and the props' pairs — each moving prop
    against the track, each car against each woken prop whose box it
    meets — go into the one list with the cars' track and car–car
    contacts. 0x5FFA60 gathers every pair of the pair list 0x86B490
    (car–car, car–prop, and the bodies against the track, 0x5B5060) by
    sorted insertion (0x43BEE0, comparator 0x5A0EC0: the contact's time
    +0x8 from 0x5B4BF0, earliest first — its tie test against 0x7FBF10 =
    −5.0001 never passes — so of equal times the earlier inserted stays
    first), resolves the head (0x5B2620), re-sweeps the bodies it moved
    (0x5B23C0; a type-1 body with +0x154 = −1 brings its group, 0x5AE010)
    and re-gathers their pairs that have had fewer than 2 (0x802084) this
    frame, at most 20 (0x802080) a frame in all, the rest dropped. As for
    two cars, a car–prop contact moves both bodies back to its moment,
    each blended over its own span (0x593640), and the rest of the span is
    simulated again for both. Stuck (0x5B4300, by the bodies' types +0x11:
    3 the track, 1 a prop, 2 an unhit one): with a prop as B the push runs
    along A's velocity (vtable +0xC) when A is faster than 55.555557 m/s,
    else along the line from A to B (the car's centre of mass, car +0x60, to
    the prop body's +0x20); with a prop as A along minus B's
    velocity on the same test; A moves back by the offset and B on, each
    only when vtable +0x64 says it can move (a prop with its break impulse
    left cannot), 0.005 m then steps growing by 0.01 under 0.1, z held to
    ±0.15, at most 20 tries. A is the body with the smaller box edge when
    the pair is made (as for two cars). The props' pairs sit in the one
    list by when they were made, as the cars' do: the pair constructor
    0x5A0350 links every pair at the tail (0x86B494), so of equal times
    the older pair goes first (each prop pair stamped when first met,
    `PropEvent::made`; earlier notes put them after the cars'). The game swaps
    the drawn scenery instance for the moving model (flag 0x10 on the
    table entry); the clone moves the placement's instance
    (`WorldRenderer::move_placement`). The draw offset is ported: on
    waking (0x5B13A0 at 0x5B147E) the ground under the placement
    (0x5E7E80, a thunk to 0x5E72F0: height h, normal n) and the body's lowest point about the
    centre of mass (0x592A20) give lift = h − (z + (lowest + c.z)·n.z) +
    0.05 (0x7844F4); a positive lift raises the body and is stored at
    +0x168; the model is drawn that much lower (0x5ADC7D) and the offset
    eases out by 0.2 m/s (0x7FC85C) while the prop moves with +0x150 = 0,
    held at 0 (0x5B201F). The factor is n.z: 0x5B147E calls 0x5E7E80 (a thunk to 0x5E72F0) and
    0x5B14B0 multiplies by its third out-word, the polygon's normal z (+0x78). A prop's replay snapshot is the "Smokeable" section
    (0x5B1D10: the sim object, +0x506, +0x508, +0x150, +0x15C, +0x158,
    +0x160, +0x168, +0x164, the rigid body 0x59B700, 0x5ADD60), so the
    clone's replay snapshots carry the props.
  * Contact sounds and effects by material (traced; the sounds
    **ported**, `audio::collision`; the car and track effects **ported**,
    `particles`, see "Particle effects" below): a push on a contact
    record (0x5A9630: the impulse summed at +0x50, |J| at +0x60, the point
    and the sliding speed v kept at +0x40 — the third argument, see
    "Scrape" under the <work dir> sounds, the crash check 0x59FB80) within 50 m of
    the view (0x5F0FC0) takes the pair's surface (0x592C20: each body's
    vtable +0x58 — a prop's is info +0xD0, 0x5B1CB0, the index 0x5D7B60
    found for the hash at +0xD4 in the 0x98-byte table at 0x800EE4 — the
    one whose entry's float at 0x800F14 is larger); if v passes the
    entry's 0x800F70 it posts event 6 (0x5DC4D0) with (v − lo)/(hi − lo)
    of 0x800F70..0x800F74 (0 when the span is under 1e−5), clamped 0..1,
    and the faster body's velocity; if |J| passes 0x800F64 event 5 with
    its share of 0x800F64..0x800F68; within 100 m of the listener
    (0x5F1170) 0x4814E0 and 0x4792D0 run on the record: the impact sound
    (below). Then each car of the pair (type 0, its +0x130 object) with a
    sound object (+0x18) calls 0x5727C0(0, its +8, |J| × min(1, 1.3 / the
    car's mass +0x110)), × 10 when the other body is a prop (type 1),
    unless the game state 0x8900D8 is 5 or 0x5EB430 says no — the pad's
    vibration, not a sound (earlier notes said it played one): 0x5727C0
    maps the value through the effect record 0x7FB7D8 + 16·kind ({id,
    priority +4, enabled +5, envelope +8, range +0xC}) into one of three
    priority slots per port (0x5723A0 at 0x8653B0 + 0x60·port), and the
    frame's update 0x572620 hands the slots to 0x4022C0, a bare `ret` on
    the PC. The events are particle effects: 0x5DC4D0 (on 0x883E68,
    made by 0x5E07A0 → 0x5E0630) walks the emitter list at (event · 29 +
    surface) · 8, 29 surfaces of 0x98 bytes from 0x800EE0, and gives each
    emitter the point (matrix translation), the velocity (+0x80) and
    0x491E50(emitter, −share, 0). 0x5E0630 fills the lists by hashing
    (0x43DB50) the surface's name for each event (events 0–6 at +0x44,
    +0x50, +0x5C, +0x68, +0x74, +0x80, +0x8C) and gathering the emitter
    manager's entries with that hash (0x48B8B0, as the props' attached
    effects). Event 5 (impulse, +0x84/+0x88 thresholds): ROAD
    `XEBO_ROAD`, GRASS `XECS_GRASS`, WOOD `XECS_WOOD_WALL`, PLASTIC
    `XECS_PLASTIC_WALL`, GLASS `XECS_GLASS_WALL`, SOLID_WALL
    `XECS_SOLID_WALL`, CAR `XECI_CAR`; event 6 (closing speed,
    +0x90/+0x94): ROAD and SOLID_WALL `XECS_SOLID_WALL`, GRASS, WOOD,
    PLASTIC and GLASS as for 5; other surfaces none (events 0–4 are the
    tyre groups, e.g. `XETT_ROAD_SK`; no direct call posts them, since the
    only callers of 0x5DC4D0 push 6 (0x5A9804) and 5 (0x5A9890)). A track
    loader (0x615CD0) also
    registers placed emitters of types 1–5 into rows 0–4 by their surface.
    The thresholds and groups (0x800EE0 + 0x98·s, event 5 +0x80/+0x84/+0x88,
    event 6 +0x8C/+0x90/+0x94): ROAD (1) 10..15 / 5..50, GRASS (15)
    15..100 / 15..50, WOOD/PLASTIC/GLASS (17–19) 4..15 / 15..50,
    SOLID_WALL (20) 10..15 / 15..50, CAR (28) 10..15 / none; the pair's
    surface (0x592C20) is the one with the larger +0x34 (road and walls
    0.1, grass 1, wood/plastic/glass 0.5, car 0). The clone posts both
    events on every push of a car against the track (`drive`: a wall as
    SOLID_WALL and a floor as ROAD; the clone's own comment says the face's
    surface byte is not carried to the contact) and of two cars (CAR, the
    faster car's velocity); the props' contacts post none. Verified game side:
    a track body's surface is the polygon's byte +0x0B, the face's surface byte
    (vtable 0x7A22B0 +0x58 → 0x593DA0, which movsx's it; the ground query
    0x5E72F0 writes the same byte through its int* argument), and a prop's is
    info +0xD0 (vtable 0x7A26A0 +0x58 → 0x5B1CB0). The clone's wall/floor split
    is therefore an approximation, a port gap.
  * **Particle effects** (`src/particles.rs`; verified unless marked):
    * Descriptions: chunk `0x35021` (GLOBALB 92, L4RA 37), body aligned
      to 16, version 4 at +8, count +0xC, 0x220-byte records from +0x10
      (loader 0x491C70, list 0x830D08, GLOBALB's first). Fields: +8 name
      hash, +0xC texture hash (0x4901D0 → +0x14), +0x10 group hash; bytes
      +0x18 (0 any direction, else a cone), +0x19/+0x1A flip-book columns
      and rows (log2), +0x1B frames per second, +0x1C random start frame,
      +0x1D random spin flip, +0x1E carried by the owner, +0x1F aspect
      (0..3, 0x7F3944 {1×1, 1×0.5, 0.5×1}), +0x20 orientation (0 camera,
      1 XY, 2 XZ, 3 YZ; non-zero: the second list), +0x21 spawns at low
      detail; floats (value, spread fraction) on +0x24 / off +0x2C times,
      rate +0x34, life +0x3C, speed +0x44; +0x4C start-angle range (deg),
      +0x50 cone half angle, +0x54 velocity share, +0x60 box centre, +0x70
      box size, +0x80 spawn range from the views, +0x84 gravity, +0x88
      drag, +0x8C on-screen size bound (0 → 512 px); cubics in the age
      fraction (0x4881D0: row0·t³ + row1·t² + row2·t + row3) +0x100 RGBA
      and +0x140 (size, spin); names +0x180, texture +0x1C0, group +0x1F0.
      Placed emitters: `0x35020` (GLOBALB, version 6, 0xD0 bytes: hash +8,
      description hash +0xC → +0x1C by 0x48BC00, type +0x10, matrix +0x40,
      name +0x90), started by 0x48BC50.
    * Groups seen in races: `XECS_SOLID_WALL` (GlowA HEADLIGHTGLOW, SprksA
      XE_SPARKSA), `XECI_CAR` (SmokeA, SprksA), `XEBO_ROAD` (FlashA,
      SmokeA, SprksA), `XECS_GRASS`, `XECS_WOOD_WALL`; the tyres' event
      0–4 groups (`XETT_ROAD_SK` → `XETT_RoadSk_SmkA`, `_SP`, dirt, grass,
      cross-street, water); `CARFX_NITRO` → `XES_NOS_FlameA` (SFX_FIREB),
      the `CARFX_NOSBO_*` hood/wheel smokes and `CARFX_BREAK_*` glass: one
      system for all.
    * Spawn (0x491E50(emitter, amount, key)): nothing unless the emitter
      is within +0x80 of one of the views (up to three); a running
      emitter (+0x10 = 1) emits `amount` seconds (clamped to its on time
      left +0x14), a negative amount is a burst of −amount whatever its
      state. With the wait +0x18 under the amount: subtract the wait,
      spawn, draw the next wait 1 / (rate ± spread) (0.1 when not
      positive, at most 240), repeat while the wait fits; keep wait −
      left (a burst keeps 0). A particle: position = matrix · (centre +
      (rand − ½)·size), the flip-book frame rand·frames when +0x1C, life
      (±spread, at most 60 s, kept in 1/1024 s), speed ±spread, share
      clamp(+0x54 ± spread, 0..1) only when its base is not 0, velocity
      speed·dir + share·owner velocity (+0x80) unless +0x1E (then the
      share is kept for the update), angle rand·range − range/2 in
      65536ths (bit 0 the flip when +0x1D). None spawn while the pool of
      1000 (0x488750; 300 on one hardware class) is empty, nor without
      +0x21 while 0x870CEC (a video option, capped by the card, 0x5BE78F)
      is 0. Groups: a pool of 400 (0x488730).
    * Update (0x492E10, dt 0x865194, after the emitters' on/off timers:
      on → off for +0x2C ± +0x30, off → on for +0x24 ± +0x28, 0 for
      ever): life −= trunc(dt·1024), ≤ 0 frees; the frame advances by
      life/ticks − left/ticks, ticks = trunc(1024 / +0x1B), masked to the
      book; per axis v += −sign(v)·v²·drag·dt, z also −gravity·dt; x +=
      v·dt with the new v; carried ones also move by share·the key
      object's +0x70..+0x78 ·dt; speed and direction re-packed (1e10
      for |v| ≤ 1e−5).
    * Render (0x48BDF0, the list 0 batch 0x5C0E20, then list 1 quads
      0x5C7E90): t = 1 − life / +0x3C; colour and size from the cubics
      (RGBA clamped 0..255; a key object's +0x30 can replace the colour
      cubic); size bounded so size/depth·view +0xC ≤ +0x8C (view +0x0C =
      256 / tan(fov / 2), `render::cull_scale`; the clone had the screen's
      pixels per metre, 1.875× larger at 960 lines, which clamped the event
      markers' near rings to 4.8 m across where the original's apitrace
      draws them 7.1 m: their low rings then reached over the car's rear,
      the tint on the bumper the reports saw; fixed 2026-11-01); angle +0xA ±
      trunc(spin) (minus with the flip); half extents size·aspect/2;
      corners p − wR + hU, p − wR − hU, p + wR − hU, p + wR + hU with UVs
      (u0,v0), (u1,v0), (u1,v1), (u0,v1) of the frame's cell; ARGB colour.
    * **Marker effects** (verified, decompiled 0x605690, 0x605790,
      0x5F0CA0, 0x6054C0, 0x500DE0, 0x5002E0, 0x500340): twelve effect
      objects (0x890130, 0x30 bytes each) from the table 0x802300 (12
      bytes an entry: kind, id, name): kind 0 id 0 XEE_EVENT_CIRCUIT, 1
      XEE_EVENT_DRAG, 2 XEE_EVENT_STREET, 3 XEE_EVENT_DRIFT, 4
      XEE_EVENT_SPRINT, 5 XEE_EVENT_URL, 6 XEE_SHOWCASE, 15
      XEE_WORLD_CHALLENGE, 7 XEE_EVENT_SUV, 8 XEE_DVD; kind 1
      XEE_GAME_TIP; kind 2 XEE_BANK. 0x605690 gives each the emitter
      descriptions of the group `bin_hash(name)` (0x48B8B0). Each frame
      0x605790 turns them all off (0x602230) unless 0x8022F8 is set (1 in
      the data; 0x501290 clears it through 0x6022B0 and sets it through
      0x5F0C90, called from 0x499FA0 and 0x4CE770, **not traced**), the
      game is out of a race (0x89E7B0 = 0, or 1 with 0x89E7BC = 0) and
      there is a player. Then each slot (0x890020, 12 × {entry, d²})
      takes, for kind 0, the nearest marker table entry whose type
      (0x500DE0: a shop by its kind, 0x500340, 1 → 9, 2 → 10, 5 → 11, 3
      → 12, 4 → 13, 0 → 14, so no effect; a milestone 6; an event by
      0x5002E0: SUV (+0xD = 1) 7, else URL (+0x34 = 2) 5, else by +0xF 1
      → 4, 2 → 2, 4 → 1, 5 → 3, other 0; a challenge 15) is its id, for
      kind 1 the nearest game tip (career +0x600…+0x7FC) and for kind 2
      the nearest bank (+0x804…+0xC00), in each case under 200 m in x, y
      (d² < 40000) and nearer by a strictly smaller d² than the one held
      (0x5F0CA0). A held slot puts its effect at the entry's trigger (+4,
      +8, +0x14) through 0x6054C0: an effect on within 0.001 of that spot
      on each axis (0x43D190) stays; otherwise it goes and, with
      descriptions, starts there: for a z of 0 the ground under x, y by
      a query of its own (0x5E72F0, no car; none, and it stays off), its
      emitters made (0x48B830), on (+0x10 = 1), at the identity matrix
      moved to the spot (0x43C780, 0x4012B0), registered (0x48BC50), the
      spot kept as given. An empty slot turns its effect off. The clone
      (`Game::marker_effects`, `Effects::set_markers`) has neither tips,
      banks, challenges nor the DVD entry, so those four stay off.
      **Which events get a world marker** (**verified**, 0x533320,
      2026-10-31): the marker table (0x500EE0) is rebuilt after the open
      list (0x527D40) from, in order, each kind-0xE000 route trigger's shop
      (list 0x8383F0, 0xA0 bytes, +0x3C the trigger hash; kept with
      0x838441 set, or shop +0x9D ≤ the career's stage (+0x7AF8) and, for
      +0x51 set, the part 0x5003B0(+0x38) with +4 = 1, and 0x512870) and
      milestone (list 0x83840C, 0x40 bytes, +0x2C; 0x501150), then the
      open list's events (career +0x71C8, 0x4FFC50 by +0x28), then the
      challenges (career +0x7148); while a world challenge runs (+0xC08)
      only its goal (0x515060). So only open events get one (kind-0
      slots); shops have no effect (types 9–14), milestones XEE_SHOWCASE.
      **Open** (look112): in one session the original showed a green
      two-car icon at STAGE_0_CIRCUIT_0's spot (353, −485) and another at
      S5_SUV_1's (−960, −1382, a stage-5 SUV circuit, the car standing in
      it), neither in a stage-0 open list and no placed emitter
      (0x35020, STREAML4RA) within 120 m of either; a later session at the
      same spot showed none. Not explained by the table above; the
      session's state (teleports by memory writes) is suspect, not
      traced.
    * **Placed emitters** (verified, decompiled 0x491C70, 0x491D50,
      0x48BC00, 0x48BC50, 0x492E10, 0x5D7450, 0x5D7480, 0x5711E0): a
      track section's chunk 0x35020 (version 6 at aligned +8, count at
      +0xC) holds emitter instances, 0xD0 bytes each from aligned +0x10,
      used in place: +0x0C the description's name hash (0x48BC00 binds the
      first description in load order with that +0x08, into +0x1C), +0x10
      the state (0x48BC50: 1 emitting or 0 waiting gets that state's time,
      +0x24 ± +0x28 or +0x2C ± +0x30, and joins the running list; any other
      state is filed as 2 and never runs; a zero random state +0x24 is
      seeded), +0x2C a switch hash, +0x40 the matrix, +0x90 the name.
      0x492E10 runs one only while its switch is 0 or in the list of 8 at
      0x88A28C (0x5D7450 on 0x883E70 +0x641C, call at 0x492E6E). 0x571170
      (and the two inline copies in 0x578070 at 0x57813A, 0x5781C5) puts the
      hash of each name it switches into the first free slot (0x5D7480, none
      free: dropped); 0x5711E0 empties it. In Bayview the shops' markers
      (XEE_Event_CARLOT_IconA_01 … SmokeB, 92 instances in STREAML4RA)
      carry `FREE_ROAM`'s hash (0xEA5E404F), so they run in career free roam
      past stage 0 only. **Checked under Wine** (2026-11-01,
      `a private Wine copy`, IND_GRAPHICS): at career stage 0 (0x862838 = 0)
      the list at 0x88A28C held four other hashes and no marker showed;
      with `FREE_ROAM` (0xEA5E404F) written into a free slot the marker
      emitters ran. The four instances at the shop (descriptions
      0x949AB7FB, 0xADF4CC42, 0xEADCBF11, 0xEADD4B72) sit in memory
      exactly as in STREAML4RA: (454.63, −1554.06) at z 16.19, 13.63 and
      13.69 and (462.18, −1551.09, 21.73), the same axes — no offset and
      no ground snap. From the road at (431.1, −1516.7), facing −58°,
      both games show the ground glow in the gap behind the brick wall,
      so a marker that looks set into the wall there is native. **Open**:
      the clone draws the tilted 0xEADCBF11 (21.7 m up, its axes 45°
      down) as a large red cone from the lamp, where the original shows
      only a red halo there. The clone (`particles::parse_placed`,
      `World::switches`, `Effects::step`) starts them all at load; the game
      links and unlinks them with their section (0x491D50) — the clone has
      every section in.
    * **Wheel effects** (**corrected** in race9: 0x436050 is the NIS
      character update and 0x4335C0 / 0x433090 the flag girl's flares,
      not the wheels — see **Pooled flares** (b); the text below keeps
      the old reading of the same code): 0x436050, on a character object
      (render info at +0xC, animation controller at +0x14, the flare
      table index at +0x1C), calls 0x4335C0 when the index is ≥ 0. 0x4335C0
      needs the global 0x827B28 and the character's int at +0x24 ≠ −1. It
      takes four points (table 0x7EA114 + 0x34·index, through two bones
      and the character's matrix) and calls 0x433090 for slot 0 (p1, direction
      p1 − p2) and slot 1 (p3, direction p3 − p4). 0x433090 walks the
      flare object's emitter list (head at +0): each emitter gets the point as
      its matrix translation (+0x40), velocity 2 × the vector from 0x43CF10
      (+0x80..+0x88; its normalisation is not checked), +0x8C = 0, and a call
      to 0x491E50 with −1.0 (verified; reading that argument as a one-second
      burst is **inferred**). With no slip test in this path, each slot's call
      posts that burst (that is the flare's sparks, not a tyre's); the
      clone's tyre slip rule stays inferred. **Caller of 0x436050**
      (verified): no direct call reaches it; its only xref is the vtable
      slot at 0x785FCC, index 6 (+0x18) of the vtable at 0x785FB4. The
      stores of 0x785FB4 at 0x43122C and 0x43AE5D (`mov DWORD PTR [reg],
      0x785FB4`) set that vtable, so a call through slot 6 on such an object
      reaches 0x436050. The disassembly has 315 indirect calls through
      `[reg+0x18]`; none is yet tied to this vtable.
      0x436050 calls 0x4335C0 and 0x434B00 directly. The list is the
      XECH_FLAG_GIRL_FLARE group (0x439780); which code posts the tyre
      groups below stays **open**.
      The binary holds six surface pairs, `XETT_{GRASS,DEEPWATER,PUDDLE,
      RRCSSW,DIRTRD,ROAD}_{SK,SP}`, at 0x7A48D8 to 0x7A4B20, and the block also holds `_DR`
      and `_DRSL` names. No code operand in the disassembly points into the
      block (the only nearby one is a float at 0x7A4824), and REA finds no
      xref to 0x7A48E8. But the .data section holds 4-byte pointers to every
      name (VAs 0x800FBC to 0x801830); what reads them is **open**. The twelve
      bin_hash values (h × 33 + c, seed 0xFFFFFFFF) occur neither as immediates
      in the code nor as 32-bit words in the EXE. The clone's surface table (`particles.rs`, lines 627-649)
      is its own choice. The tyre
      object's dword +0x34 picks a branch by its bit 0x40 (0x434B00, `je` at
      0x434B5D; verified by disassembly); the rest of 0x434B00 is **open**.
    * **Emitter draws** (verified by the decompile of 0x491E50 and the
      operands of its `_ftol` calls, 0x75BED4, unless marked):
      - Seed: an emitter with +0x24 = 0 takes the global state 0x7F3940
        after one step (17/13/5), masked to 31 bits.
      - A draw is r = (state & 0x7FFFFFFF) × 2^−31 (0x78D864); a step is
        `x ^= x << 17; x ^= x >> 13; x ^= x << 5`.
      - Mode 0 (+0x18 = 0): three angles; the first is
        `_ftol((r × H − ½H) × 65536) / 360` with H = +0x50 (asm 0x492545 to
        0x492553, the ÷360 at 0x492558; operands 0x784238 and 0x784260), so it
        spans ±H/2 and is not a full turn. H is in degrees, as the game reads
        it (**verified**): H is the dword at +0x50 (0x4920f0, copied at
        0x4920ff). The magic-number ÷360 (0x492558 to 0x49256b) makes H/360
        the span in turns, so the quotient is in 65536ths of a turn. It is
        pushed at 0x49256d into the call at 0x49257b; 0x5BAD00 passes it
        unchanged (0x5BAD0A) to the sine helper 0x43C2B0, which reads a u16 and
        scales it by 0x784240 = 2π/65536, so the angle in radians is
        (r − ½)·H·π/180 up to the truncation. The full-turn form is the cone's
        a. M = A·RotX·RotY·RotZ:
        the builders 0x5BAD00, 0x5BADB0 and 0x5BAE70 (via 0x5BACA0) are
        confirmed. The product is called through the table entry 0x803E74 (the
        thunk 0x63CEEB; 0x63CED8 is the first-call path of the init 0x641723).
        The init copies the defaults from 0x803F90 (0x63CEF1 for entry 3), then
        installs 0x68FB59 (0x6713B1) or 0x68CE8C (0x671162, 0x670FEF) at entry
        3 on tests of the init argument, 0x7831A4 (queried with 7 and 6), the
        vendor string "GenuineIntel" (0x7AB4C4, tested by 0x641682, bit 8) and
        the config keys DisablePSGP and DisableD3DXPSGP passed to 0x641549. The
        gates are only partly traced (0x6713B1's flag word from 0x6712CB; its
        other branch, 0x6715F1, is **open**). The default 0x63CEF1 is traced: a
        row-major 4×4 product, dst = a·b with a the second argument ([ebp+0xc]),
        so the builder 0x5BAD00 gives its input times a local matrix; the two
        variant bodies 0x68FB59 and 0x68CE8C are **open** (not traced). The
        direction is row 2 of E·M, E the emitter matrix (+0x40). **Open**: the
        Y angle reads a copy at [esp+0x1c] (stored 0x492588, read 0x49258C) and
        Z one at [esp+0x28] (stored 0x4925E4, read 0x4925E8), the decompile
        shows no step between the draws, and A = DAT_00872E90 is written at run
        time (0x5BDDF0 from 0x5C0E20), so its value at spawn is not traced.
      - Ring (+0x18 ≠ 0; the clone's `direction` since 2026-11-01, before a
        cone about z): two angles a and b; cos a (0x43C370) and sin a
        (0x43C2B0, the sine of a u16 angle × 2π/65536). a is
        `_ftol((r × 360 − 180) × 65536) / 360` (0x492286), a full turn in
        65536ths. The direction is (cos a, sin a, 0) · (A·RotY(b)) · E. The
        spawn reads +0x50 (H) before the mode branch (0x4920F0): the cone's b
        takes ½H from [esp+0x58] at 0x4922D4, and mode 0 uses H and ½H
        (0x492545, 0x492549).
        So b = r·H − ½H (0x4922CE…0x4922D8) and the direction lies on a
        ring in the emitter's xy plane tilted about y by b: with H = 0 level.
        **Verified** under Wine (apitrace, the finished-career save's garage exit, 2026-11-01):
        XEE_Event_*_SmokeB_01 (+0x18 set, H 0, speed 7) has its 27 live
        particles about 3–4 m round the marker, all at its height (z 14.1 ±
        0.1), the green ground haze; the clone had sent them straight up
        as a column. A (0x872E90) is taken as the identity (**inferred**).
      - Position: centre + (r − ½) × size per axis from three successive
        steps, then × E.
      - Fields: life × 1024 (+8), speed min(· × 128, 65535) (+0xC), share ×
        255 (+0xE), direction × 127 (+0x1D, +0x1E; the operand of +0x1C is not
        traced), the angle u16 with bit 0 from the state when +0x1D is set.
        The wait takes one step: W = [esp+0x6c] + r × spread (0x492CF5), with
        spread = [esp+0x20] = (+0x38) × (+0x34) (0x492151). With [esp+0x6c] =
        rate − spread, W is a one-sided range [rate − spread, rate), not ±. The
        wait is 1 / W (fdivr by 1.0 at 0x492D0A), capped at 240 (0x78DA30); a
        W that is not positive takes the 0.1 path (0x492D27). **Open**: the
        value of [esp+0x6c] (not traced), and whether the other W, in
        [esp+0x1c] (0x4929E2) and also the +0xC speed operand (0x492BE7), is
        the speed.
    * **Colour** (verified, decompiled 0x48BDF0): on the quads path the RGBA
      cubic is clamped to 0..255 per channel and packed into one 32-bit
      colour from four `_ftol`s, the first in the top byte. The list-0 path
      passes four float components (locals 0x1B0..0x1A4) and makes no `_ftol`
      call. **Shading (verified, 2026-10-31, look112):** 0x5C0E20 packs the
      list-0 colour as `A<<24 | R<<16 | G<<8 | B` (four `_ftol`s of its
      struct +0x2C, +0x20, +0x24, +0x28, the clamped cubic as 0x48C5BA…
      0x48C628 stores it) and 0x5BB440 copies that D3DCOLOR unchanged to
      the four vertices (corners zeroed when alpha ≤ 0x14). The batch draws
      (0x5CE130 → 0x5C57F0: stream 0, stride 0x18, BeginPass(0) of the
      current effect, DrawIndexedPrimitive) with the effect 0x5CAE8B binds
      before both Particles_Render calls (0x5CAEC8 / 0x5CAED3): 0x86EB5C,
      worldnodepth — world.efx's pixel shader `2·tex·v0`, alpha
      `tex.a·v0.a`, its vertex shader writes no fog. 0x5B7DF0 (per view)
      sets ZWRITEENABLE 0; 0x5BCC40 binds each texture with its blend. So a
      particle's RGB is twice texel × colour. The clone halved the colour
      (texel × colour, half as bright: the marker icons and rings, the
      front end's steam, every smoke) and fogged the quads; it now passes
      the colour as it is through the no-fog pipeline (`render_particles`).
      Checked under Wine (wined3d) at the STAGE_0 circuit / sprint markers
      and the main menu's steam ($OB_PRIVATE/wine/compare/look112).
      **Open**: the link
      from the quads 0x5C7E90 to 0x5C5590, the blend states (0x5C5590 not
      traced), and the group cap (the clone does not bound it: 0x488730 creates the pool
      "AcidParticleSlotPool" (name at 0x78D8C8) of 1000 elements, or 300 when
      [0x850078] = 4 and [0x8654A4] != 3, via 0x440DF0; 0x20 is its first
      argument (0x440DF0 not traced). No 400 appears in 0x488730, so the 400
      cited for it in the spawn notes is **open**).
    * **0x870CEC** (disassembled): a flag. The spawn skips low-detail
      emitters (+0x21 = 0) only while it is 0 (0x492B83); 0x49BFED flips
      bit 0; it is written at 0x5BE797, 0x5BE8A1 and 0x5BE946 beside the
      capability check at 0x5BE78F, its address is pushed at 0x5BEC97 and
      0x5BF06E, and it is compared at 0x5CB41A and 0x5CB7FE. The value in
      the file image is not meaningful. **Open**: its default and the
      writers' conditions.
    * The nitrous flame (verified unless marked; game.rs
      `Effects::step`). Each car's update (0x5F5F0B) calls 0x61FB10(kind,
      1.0, 0, 0) while 0x59FA40 is non-zero: kind 10 when 0x5ECDC0 gives 0,
      else kind 13 (0x5F5FF9, the purge). 0x5ECDC0 gives 0 only with the
      gas (input +0x20C) over 0.001 in size and at least 5 mph (inlined at
      0x5F5F14..0x5F5F47; **verified** 2026-11-04): a shot held off the gas
      purges at any speed, no flame (internal notes). The kind names (0x430C90):
      `CARFX_NITRO` 10, `CARFX_EXHAUST_SMOKE` 11, `CARFX_EXHAUST_BLOWOFF`
      12, `CARFX_NOS_BLOWOFF` 13. 0x61FB10 needs render +0x554 and car
      +0x5B0 = 0; 0x615CD0 walks the kind's emitters (render object +0x458
      + 8·kind) and, per emitter, the markers of its group (+0x548 + 8·group,
      the emitter's +0x1C → +0x5C), placing it at car × (its own rotation,
      mirrored by the marker's flag bits, at the marker) with +0x80 the
      car's velocity, then 0x491E50(emitter, −1.0, owner key): a one-particle
      burst at the rate (XES_NOS_FlameA: rate 8, so 8 a frame). Kind 13 also
      fires kind 14. A turbo car also fires kind 10 scaled by the turbo
      level in neutral (0x5F5F5F..0x5F608D, armed by +0x924) and at a shift (0x5EBE10 from Car_Frame): **ported** 2026-11-05, see docs/sparks-fire.md (`carfx_pops`). **Verified**
      2026-11-04 (REA decompile of 0x615CD0, disassembly 0x639843..0x6398AF;
      internal notes): XES_NOS_FlameA's marker group (+0x5C)
      is 10, so its markers are render +0x548 + 0x50 = +0x598, the exhaust
      list 0x639843 files (entries: +0x10 the marker, +4..+0xC its position
      with the mirror bits cleared); the emitter keeps its own axes (the
      clone now takes the GLOBALB template's, as the purge does) and only
      its translation becomes the marker's position — the clone had used
      the marker's whole matrix. **Inferred**: the kind-10 list holds the
      `CARFX_NITRO` group's templates, and the owner key is the clone's own
      per car (carried particles follow its velocity). The list fill from
      car data is **open**; the purge smoke (13 / 14) is traced in "Specialties: hydraulics level 3 and the purge".
      Check: `NOS_SHOT=1 game --shot explore out.png` fits a level-1
      nitrous package, holds Left Alt from 1 s and steps the particles: a
      faint blue-orange flame shows under the 206's exhaust (`--shot race`
      stays in the countdown, so it shows none).
    * **Open**: the other car effects' mounting and trigger, the props' attached
      effects, and the key objects (+0x70 velocity, +0x30 colour).
  * Impact sounds (**verified** unless marked; `src/audio/collision.rs`):
    the record's constructor (0x5A9490 → 0x483E20 → 0x481CD0) makes one
    sound object for it in the manager's list (the list object is M+0x3C,
    M being the pointer at 0x82B884; its list header is +0x24; none when its
    +0xC is over 9, see the cap below): class 2 (vtable 0x78BB08) with a prop
    in the pair
    (0x592D60), 1 (0x78BE90) for two cars (0x592CA0), else 0 (0x78BEE0, a
    car against the track) — 0x459680. On a push 0x4797B0 finds it; its
    first update (vtable +4 → 0x46A1F0, once, +0x18) takes the record's
    summed |J| (+0x60), point, the push direction (+0x70 = J/|J|, written
    by 0x5A9630 through 0x43CF10), the flags (0x459600: 1 a car with a
    sound object, car +0x130 → +0x18; 2 a prop; 4 upright; 8; 0x10 a car),
    the kind (vtable +0x28) and the volume (vtable +0xC); the play test
    (vtable +0x14) queues it (0x479770) or marks it done (0x46A190,
    +0x20), and classes 0 / 1 never play once done, so a record sounds at
    most once, at its first push. Kinds: two cars (0x46A770) 1 when their
    flat headings (+0x90) are within 45° (|dot| > 0.707) and the point is
    off either one's nose–tail line (|forward · arm| ≤ 0.8), else 0; the
    track (0x46AA20) a wall (|n.z| ≤ 0.707) 4, or 5 when the car runs
    along it (|forward · n| ≤ 0.707); a floor 2 with the car's up (+0xB0)
    z below 0.1, 3 above 0.707, else 4; a prop 7 with a car in the pair,
    else 8 (0x4598B0, flags bit 0x10). Volume (0x479A20; props 0x47A550
    use |J| × the prop's 1/m, vtable +0x5C): t = |J| / (3 ·
    0x7884F4[kind]) ({18, 2.5, 15, 1, 20, 8, 1, 2.5, 2.5}), the byte
    trunc(127·t), 0 for t ≤ 0, at most 127; class 0 (0x47A700) adds 50
    (at most 127) when the car's up z is under 0.9 (0x7ED564; (x + y)·0 +
    z) and 0x5EC200 — 0.25 for each wheel flag +0x390..+0x393 — is under
    1, i.e. not all four wheels down. Play tests: class 0 (0x46ABA0) drops
    the sound while another object of the same car (+0x118) whose first
    update ran has kind 5 and its push direction (+0x40) is within 0.8
    (dot) of this record's latest one (+0x70) — the repeat along the same
    wall; its other test (the last started sound's direction 0x82BCB0
    within 0.95, point 0x82BCC0 within 1 m, a floor, and 0.95·|n| < |n′| <
    1.05·0.95·|n|) cannot pass for unit directions. Class 1 (0x459880)
    plays. Class 2 (0x46A670) needs flag 1 or a volume of 10, and no other
    started (+0x132) object of the same prop (0x592D90). The sample
    (vtable +0x4C: 0x47A610 cars, 0x47A770 track, 0x479B40 props): x =
    0x7ECB78[clamp(trunc(v · 0.15625), 0, 19)], a counter n per kind the
    member: cars 0 → 6 + 4L + n mod 4, 1 → 22 + 4L + n mod 4 (L =
    trunc(4x)); track 2 and 6 → 70 + 2L + n mod 2, 3 → 78 + 4L + n mod 4,
    4 → 38 + 4L + n mod 4, 5 → 54 + 4L + n mod 4, else 38 (class 0's L uses
    v + 60, at most 127, when car +0x118 → +0x18 → +0x58 is 6 (verified,
    0x47A770, which also clamps v + 60 at 0x7F). The per-frame update 0x604750
    (method: this = ecx, copied to edi at 0x604765; dt at [ebp+0x8]) writes
    [this+0x18]+0x58 (esi = [edi+0x18] at 0x6049DE): 6 at 0x604CFA, and 7 from
    6 (cmp at 0x604D26) in its crash branch (this+0x38 = 1 at 0x604D61, against
    3.0 at 0x604D87) or, when this+0x38 != 1, on the this+0x544 compare (fcomp at 0x604E7E).
    The test
    directions and the sound-state calls are not traced. **Open**: whether
    [this+0x18] is the block the gate reads (the gate's chain is
    [car+0x118]+0x18, another field), and the port (not done; whether the
    clone has such a block is not checked); props by material (+0x140 = info +0xD8,
    0x46A710; jump tables 0x47A4BC / 0x47A504) kind 7 → base + 3·trunc(2x)
    + n mod 3, kind 8 → base + 6 + 3·trunc(3x) + n mod 3, base by material
    1–18 {199, 259, 124, 139, 94, 94, 259, 259, 229, 214, 229, 244, 259,
    259, 154, 274, 169, 109}, 259 outside, one counter per base (two for
    259: SHOPPING_CART shares the default's) and kind — each material's 15
    records, 94..288. The index n picks a stitch record (0x43BE60 on the
    manager's +0xD0 list +4, node +8). The lists are built by 0x47BC30
    from `GLOBAL/InGameCommon.bun`'s two `0x8003B500` chunks: each
    `0x3B502` record (hash, +4 gain 0x7FFF, +6 its index, +8 the list —
    0 collision, 1 the whooshes at +0xC — and +9 the layer count) is
    appended in file order (0x479130), its 16-byte layers in the
    `0x3B503` chunk after it (+0x10); list 0 has 289 records, list 1 60
    (test `stitch_lists_load`). Starting (0x46A540 → 0x479900 → 0x46CDD0):
    a voice from the manager's +0xBC pool (none: done); the record's
    volume × 0x7F34A0[+6] >> 15 (list 1: 0x7F36E8); each layer (0x45B280
    → 0x458360): sound byte +0 in `SOUND/IG_GLOBAL/MB_Stich_Collision.abk`'s
    94-entry sound list (all 289 records' layers stay under 94), byte +1
    the bank kind (0 for all), volume V × (+2 − rand(+0xA)) >> 15 × record
    +4 >> 15 (≤ 0x7FFF), pitch (+4 − rand(|+0xC|)) × (0x1000 − p) / 0x1000
    (p = 0 at the start; ≤ 0x2000, 0x1000 = 1), azimuth +8 plus the
    object's, and +6 as a start delay (≤ 4000); rand(n) is 0x43C1C0 (seed
    0x7EA624 = 0x12345678 mod n, then an xorshift). The listener test
    0x5F1170 takes the nearest of the listener object's (0x890080)
    positions +0x80 / +0x90 (+0xA0 / +0xB0 with more than two, +0xC0 /
    +0xD0 with five). The clone posts each record's first push from the
    track, car and prop contacts (`drive`, `smackable::prop_sound`: the
    tipped boost, the wall repeat filter on the car's records, the props'
    test) and plays those within 100 m of the player through the mixer,
    each layer with its gain, pitch and delay. **Start volume V
    (verified, gap sweep 2026-11-05)**: V is the sound object's +0xC0, the
    second dword of its start block +0xBC that 0x479900 hands to
    0x46CDD0 (the stitch player's +4..+0x14) and 0x47BB30 hands again
    each frame (→ 0x46CF70: each playing layer's level = its volume ×
    V >> 15). The start itself (0x46A540) zeroes +0xC0..+0xC8 before
    0x479900; the object's mix-map client SFXObj_Collision (slot 0x28,
    0x461CE0) writes +0xC0 each frame: slot 0x38 (0x45E390) of the
    object's kind (+0x124: 1..8 → slots 1..8, else slot 0; the jump table
    0x461DC4) — section 5 class 0 of the object's copy at its point:
    slots 0–5 master 0 (−600 mB, 16365 at 1 m), 7–8 master 2 (−800 mB,
    12999), slot 6 unbound (0), each turned down by its master's 3D
    control by the distance (40 m: 6373 / 3175; test
    `mixmap::tests::impact_slots_by_kind`). The volume byte picks the
    sample only. The clone now plays each impact at its copy's level
    from the next mix pass (`Audio::impacts`, one frame after the push —
    **inferred** equivalent to the original's zero start corrected at the
    next update; which 0x47BB30 caller serves the collision objects is
    not traced) and, since the prop-hit fixes (docs/prop-hits.md), sets every playing layer to its
    volume × V >> 15 again each frame with V its copy's level that frame
    (0x47BB30 → 0x46CF70; observed under Wine: the AEMS_StichCollision
    layers' volumes fall with the object's +0xC0 as the car leaves, and
    +0xC0 against the clone's mix map at the same car / camera / point:
    median 0.12 dB off, test
    `collision::tests::impact_level_follows_the_original_per_frame`).
    **While Rachel speaks** (verified + measured): all three
    section-5 masters take sub channel 0 = section 0's event controls 0
    (the sound hold, −10000 mB) and 2 (−900 mB × MasterVol f10 on
    SFXObj_Speech f1, 0x4751A0) — so a line playing outside game state 3
    ducks every collision object 900 mB: 12999 → 4617 (kinds 7–8), 16365
    → 5812 (1–5), as read under Wine; test
    `mixmap::tests::impacts_duck_while_rachel_speaks` (docs/prop-hits.md §6).
    **Inferred**: +6 in milliseconds, every clone car having a sound object
    (flag 1), the sound object (and its copy) living while its record or
    its layers do. **Open**: the
    library's pan and distance attenuation (the clone's mixer is mono; the game's
    mixer channel law is under Audio, "Mixer channel law"; the layer and
    sub-voice requests below carry the azimuth and pan), the 10-object
    cap (traced, not ported: the clone's records sit in cars, pairs and
    props), the water drum's extra start (traced below, not ported). Partly
    traced toward pan and falloff: the per-frame chain 0x479970 (when
    0x7FAAA8 and +0x134: copies +0x30..+0x38 to +0x100..+0x108, steps the
    envelope 0x46B100 — {time, length, from, to, shape 1 hold / 2 ease-in
    / 3 ease-out / 5 random / else linear} — sets the voice's level
    0x576980(+0xDC + 0xFFFF), then vtable +4, +8, +0x30) and 0x479AD0,
    which feeds +0x24's +0x40 into a ramp 0x4599E0 ({out lo, out hi, in
    lo, in hi, in, out}: lerp by clamp((in − lo)/(hi − lo), 0, 1)) at +0xA4
    and keeps trunc(out) at +0x54: the track class's ramp 0x82BD40 maps
    0..40 → 0..1, the two-car class's 0x82BD08 5..15 → 0..1 (built at
    0x469EBA from 0x7884A4..0x7884F0, five ramps {0..3, 5..15, 0..3,
    0..40, 0..3} → 0..1). What +0x24 +0x40 is (a speed, not a distance,
    by the ranges — **inferred**). Traced since: the listener distance
    (verified: the first update 0x46A1F0 calls 0x5F1170 with the listener
    object 0x890080 and the object's point +0x30, and stores the float at
    +0xA8, 0x46A262). 0x5F1170 is the least distance from the point to the
    listener's views (+0x80, +0x90; +0xA0 and +0xB0 when +0x14 > 2 and a gate on 0x89CF69, 0x8669F4, 0x866A14 passes (branches verified, gate not traced),
    +0xC0 and +0xD0 when +0x14 >= 5 (same gate; the count is +0x14). The per-frame sub-voice
    block of a class 0 or 1 object (0x459770 on the handle +0xB8; made by
    0x4596E0 → 0x458010): +4 = +0xD4 clamped to 0..0x7FFF, +8 = +0xD8 to
    0..0x2328, +0xC = +0xDC to 0..0xFFFF, +0x14 = +0xE0 to 0..0x7F; then
    0x735580 applies it. 0x735580 is verified as a dispatcher, not the pan
    law: under the mutex DAT_008B4F38 it calls each node of the list at the
    object's +8 with (its argument, the node's +0xC); node +0 is the next
    node, +8 the callback. The FX sub-voice is queued by 0x458010 (queue
    `FX_Scrape`, 0x82C5D8) from 0x4596E0 with the object's +0xD4, +0xD8,
    +0xDC, +0xD0 (kind, 0..0x10, at +0x10) and +0xE0. The layer start
    0x458360 clamps (kind 0..0x10, sound index 0..0x7F, volume 0..0x7FFF,
    pitch 0..0x2000, azimuth 0..0x10000, delay 0..4000) and queues the layer
    (`AEMS_StichCollision`, 0x82C5E0). The fifth argument is the azimuth
    (**inferred** from its range; the formula is verified at 0x45B373 to
    0x45B38E and passed to the call at 0x45B39C): the layer's word +8 + the
    argument's +0xC (0x45B342), which is the object's +0xC8 (the fourth
    dword of the start block at +0xBC, which 0x46CDD0 copies to +0x10 when
    0x479900 starts the sound). The sign test is on that sum + 0x10000, so
    only a sum below -0x10000 goes negative (the sum's low 16 bits minus
    0x10000), and the clamp then makes it 0; from -0x10000 up the result is
    the sum mod 0x10000. A turn is 0x10000 (**inferred**: the clamp's range,
    not the scale). **Open**: what writes +0xD8, +0xDC and +0xC8 for these
    objects. 0x45FAA0 (from 0x47CD90, when the byte at +0xA0 is set) copies
    P+0xB0 and P+0xB4, P the object at this+0xB0, into +0xD8 and +0xDC (the
    old values go to +0xE0 and +0xE4), so it writes those fields for an
    object with a source at +0xB0; whether the class 0/1 objects reach it is
    not traced. 0x462480 (only caller 0x47E6E0, which keeps a float at +0xB8
    where the class 0/1 object keeps its handle) sets float defaults (0.15 at
    +0xD4, 1.0 at +0xD8, +0xEC = 1, among others): a default of another
    type. The falloff law (+0xA8 holds the float that 0x5F1170 returns,
    stored by the fstp at 0x46A267, after the call at 0x46A262, verified) is
    not yet traced into a level, and the library's pan law behind 0x735580
    (each voice's callbacks; what they do with the azimuth and pan) is not
    traced. The mixer's own channel law is under Audio, "Mixer channel law".
    Traced since (verified):
    the falloff input: 0x5F1170 leaves in st0 the least Euclidean distance
    from the point (esi +0x30 of the object: lea at 0x46A20D, push at
    0x46A261) to the listener views; 0x43CE10 ends in fsqrt (0x43CE38), and
    the compare chain at 0x5F11F6 keeps the smaller of its operands. Its
    level mapping is not traced. The pan side: 0x735580 is thiscall (ECX = the list object, one stack argument, `ret 4` at 0x7355C2);
    it waits on the mutex DAT_008B4F38 (call 0x783218 with -1) and releases
    it (0x783250), then calls each node cdecl with (block, node +0xC)
    (0x7355A0–0x7355AA). Ghidra names the IAT slots 0x783218 and 0x783250 WaitForSingleObject and ReleaseMutex (decompile of 0x735580). The FX owner is the item 0x735490 creates (cdecl, `ret`).
    0x7358C0 wraps the insert in the same mutex (wait 0x7358C9, release
    0x7358EF) and passes the handle as the insert's third argument (call
    0x7358DE). 0x458010 passes the handle (esi) on its first call (push
    0x458096, call 0x4580A0) and on the retry (push 0x4580BB, call 0x4580C2)
    after 0x735840 fails. The insert stores the new item through that
    argument (0x73553F), so [handle] is the owner. 0x459812 dispatches on it:
    0x459802–0x459812 loads ecx = [handle] and passes block = handle +4. The
    block fields are +0 volume, +4 pitch, +8 azimuth, +0xC kind, +0x10 pan;
    0x458010 writes all five. 0x459770 rewrites +0 (its +0xD4 source is from
    the base text, not re-read here), +4 (from +0xD8, clamp 0..0x2328), +8
    (from +0xDC, clamp 0..0xFFFF) and +0x10 (from +0xE0, clamp 0..0x7F), not
    kind. Whether any callback reads azimuth (+8) or pan (+0x10) is not
    traced. The insert first calls each node behind the queue head with (new
    item, block, node +0xC) (0x735520–0x735530), then 0x734DB0 (thiscall,
    `ret 4`) dispatches the new item's +8 list with (block, node +0xC)
    (0x735535). The insert does not replace the queue head on that path.
    0x734DB0 pops its argument (`ret 4` at 0x734DD3), so the store at
    0x73553F reads the right slot.
    Traced since (verified): the sub-voice block is the object +4 (0x45802C,
    `lea edi,[esi+4]`), and the owner goes to the object's first dword: 0x458010
    calls 0x7358C0 as (queue 0x82C5D8, block, object) at 0x4580A0 and retries
    after 0x735840 (argument 0x7EDDF0, whose dword 0x788344 points to "FX_Scrape"; 0x7351B0 not traced) at 0x4580AC–0x4580C2. 0x7358C0 is
    cdecl (queue, block, out) under the mutex and calls 0x735490 with the same
    three (0x7358DE). 0x735490 also has a direct caller, 0x720141 (AEMS code; not traced). 0x735490 makes a 16-byte owner: +0 = the queue's element
    (queue[0]), +4 = 1, +8 = 0 (its list), +0xC = 0. It calls cdecl (owner,
    block, node +0xC) on the element's chain from element[0] (0x735520–0x735526),
    then 0x734DB0 runs the owner's +8 list, and the owner is stored at out
    (0x73553F). A null queue head (queue[0] = 0) gives -6 (0x7354B3); queue[1] ≠ element +8 sets
    queue[0] = 0, queue[1] = -3 and returns -3 (0x7354C2–0x7354CC). The chain
    nodes come from 0x7355D0 and 0x735740 (their element checks are not re-traced), both called only from the AEMS instance code at 0x72113B, 0x721187
    and 0x721406. The instance listener 0x721096 is registered at 0x721406 with
    obj = instance +4, node = instance +0xC and arg = instance. Its nodes have
    callbacks 0x72103A (node +0x10 = 1), 0x72102C (node +0x18 = block +0, the
    volume) and 0x721046 (copies the block's first count dwords to node +0x14,
    count = byte at node +0x10). 0x72103A writes that byte as a dword (0x72103E), so a 0x721046 that runs after it copies one dword (the volume); the callback order is not traced. A fourth callback, 0x72106C (copies node +0x18 dwords from the block to node +0x1C and sets node +0x19 = 1), is installed at 0x72117D. The stream fill at 0x7260A1–0x7260CB calls
    slot +0x2C with 7 parameters, the writer 0x727030 between the two calls, and
    slot +0x4C with 4 parameters, on the buffer at 0x8B4780 (the slot names are
    inferred from the COM order and parameter counts), and the call-site
    arguments agree with that order (verified): Lock (+0x2C) takes this and 7
    arguments, Play (+0x30) is called as (0, 0, 1), GetStatus (+0x24) takes one
    out pointer, Unlock (+0x4C) takes 4 arguments and Restore (+0x50) takes none.
    The EXE carries no vtable names, so the slot names stay inferred. No call
    through +0x3C (SetVolume), +0x40 (SetPan) or +0x44 (SetFrequency) has the
    stream buffer [0x8B4780] as its receiver (verified). Its 19 references are 13
    reads (Release, GetCurrentPosition, Lock, Unlock, Play, GetStatus, Restore),
    the CreateSoundBuffer out pointer at 0x725B5B, and 5 stores of null (0x725B2D,
    0x725BC4, 0x725DCE, 0x725F1C, 0x725FE3). The stream code 0x725A85..0x7263B5
    has no +0x3C, +0x40 or +0x44 call, and neither does 0x700000–0x77FFFF. The
    stream buffer has dwFlags 0x18000 (DSBCAPS_GLOBALFOCUS |
    DSBCAPS_GETCURRENTPOSITION2, stored at 0x725B63), with no CTRLVOLUME, CTRLPAN
    or CTRLFREQUENCY bit, so no hardware gain is set on it (verified). The gains
    are most likely applied in software (inferred; the mixing code is not traced
    here).
    Traced since (verified in SPEED2.EXE unless marked Open or Inferred):
    The stream buffer is created in the procedure at 0x725A85. Its DSBUFFERDESC
    has dwSize 0x24 (stored at 0x725B60), dwFlags 0x18000 (stored at 0x725B63)
    and lpwfxFormat set to the 16-bit WAVEFORMATEX built on the stack. It is
    created by CreateSoundBuffer ([0x8B4778]+0xC, call at 0x725BAE) with the
    output pointer &[0x8B4780] (pushed at 0x725B5B). Before that, the primary
    buffer ([0x8B477C]) gets SetFormat (+0x38, call at 0x725B13). Its reads are at
    0x725B07 (that SetFormat), 0x725BCB and 0x725F22 (Release). It is also the out
    pointer of CreateSoundBuffer at 0x725E76 (pushed at 0x725E64, in the procedure
    at 0x725DB5) and at 0x72605C (pushed at 0x72604A), and it is zeroed at 0x725DC8,
    0x725BDA, 0x725F31 and 0x725FDD.
    The DirectSound setup just before the pump (0x72601A..0x7260E0) is inside the
    procedure at 0x725F52, whose ret is at 0x7260F6. It calls
    SetCooperativeLevel ([0x8B4778]+0x18, call at 0x72602F), then
    CreateSoundBuffer ([0x8B4778]+0xC, call at 0x72605C) with a DSBUFFERDESC whose
    dwSize is 0x24 (0x726046) and dwFlags is 1 = DSBCAPS_PRIMARYBUFFER (0x726056).
    The output pointer is stored at 0x8B477C (pushed at 0x72604A). The stores to
    both slots at 0x725DC8 and 0x725DCE write zero (edi is zeroed at 0x725DBF), so
    the buffer the pump locks, [0x8B4780], is the stream buffer from 0x725A85,
    which the setup calls at 0x72607C. The vtable order is the DirectSound one
    (inferred from the arities).
    The stream pump is the REA procedure at 0x7260F7 (its first compare is at
    0x726100). It does nothing when [0x8B4778] is null. With the buffer-lost
    bit (2) clear and the counter DAT_008B47C0 below 0x65 it mixes when the
    playing bit (1) is set, and calls Play (+0x30, flags 1) when it is not;
    otherwise it calls Restore (+0x50). It reads GetStatus (+0x24, call at
    0x72611A) and GetCurrentPosition (+0x10, call at 0x72614E). Each wrap
    segment of nonzero length is locked (+0x2C, call at 0x726268, 7 arguments
    after this), passed to 0x725C8E(pointer, frames), and unlocked (+0x4C, call
    at 0x72629C). There are at most two segments.
    0x725C8E calls the AEMS code (0x721E13, 0x721DE6; not traced), then
    0x729574(pointer to pointer, frames), which renders blocks of up to 0x100
    frames and calls 0x7292BC(channel pointers, frames) for each block. In
    0x7292BC, in order: (1) for each aux i whose entry at 0x8B4804 + 4i is
    nonzero and whose pending float at 0x8B4924 + 4i is nonzero, 0x72886C adds
    that float into the bus [0x8B47C4 + 4i]; (2) for each channel c, 0x727030
    (memset of frames × 4 bytes; call at 0x729326) zeroes the bus whose pointer
    is at 0x8B48F0 + 4c, then if the pending float at 0x8B490C + 4c is nonzero,
    0x72886C adds it into that bus; (3) the voice loop; (4) the post hook at
    0x8B4800, when set; (5) for each channel c, vtable +0 of the object at
    [0x8B4928 + 4c] is called with (frames, bus, channel pointer, 0), after an
    optional call on the object at [0x8B4940 + 4c] (its role is not traced).
    Voice loop: voices are 0x5C bytes from 0x8B4A00 (count at 0x8B47F8), state
    byte 2 = active. For an active voice, vtable +0 of the object at +0x40 is
    called with (count, the buffer pair at 0x8B48D0 and 0x8B48D4, 0) and
    returns the frames written; voice +0x3C takes the last float of the 0x8B48D4
    buffer. For each channel c with a nonzero float at voice +0x1C + 4c, the mix
    kernel [0x8B4A04] is called with (count, gain, the 0x8B48D4 buffer, bus c +
    skip). For each active aux i whose gain (read through the pointer at voice
    +0x38, indexed by i) is nonzero, the same kernel is called into
    [0x8B47C4 + 4i]. The kernel body and the skip count (0x728B48) are not
    traced.
    0x729574 renders through the planar channel pointers (0x8B4908 + 2·frames·c)
    and calls 0x72CD6A(frames, count, map, planar base, output pointer) (call
    at 0x729638, its only direct call) only when the flag at 0x8B47FA is
    nonzero. 0x72CD6A interleaves 16-bit samples for 2, 4 or 6 channels with
    the map bytes {1,0}, {3,0,2,1} and {4,1,0,5,3,2} (read from the stores in
    0x729574); other counts write nothing. When the flag is zero, 0x729574
    copies 0x8B47F9 consecutive dwords from its argument into a local array of
    output pointers and renders into them directly.
    The flag 0x8B47FA is read at 0x728DCB, 0x729147 and 0x72957E. No
    instruction stores to it by absolute address, and 0x8B47F8 lies past the
    file data of its section (the va.py read fails), so the flag is zero at
    load (inferred from the PE layout). Which branch the game takes is Open;
    the channel count at 0x8B47F9 has no absolute-address store either, so its
    writer is Open.
    0x72886C(p, bus) adds p times w[k] into bus[k] for k = 0..15, with w[0] =
    16/17 (table entry 0x7C6A20, fmul at 0x728876) down to w[15] = 1/17 (0x7C69E4,
    fmul at 0x728946), and then sets p to 0.
    0x728957(g0, g1, in, out), with 16 floats each, adds in[k] times
    (g0 + (k+1)·(g1 - g0)/17) into out[k] for k = 0..15 (the 1/17 is the
    constant at 0x7C69E4). It is one 16-sample gain ramp, not a two-input mix.
    REA lists its calling procedures as 0x72655A and 0x728B48; the call sites
    are 0x726671, 0x726717, 0x728C11 and 0x728C9D. The gains the callers pass
    are not traced.
    The four callbacks 0x72102C, 0x72103A, 0x721046 and 0x72106C are registered
    callbacks whose caller is not traced; offsets are from the second argument
    (the dispatcher's node pointer, not re-traced). 0x72102C stores block[0] at
    +0x18, the count for 0x72106C. 0x72103A stores the constant 1 at +0x10, the
    count for 0x721046. 0x721046 copies that many block dwords to +0x14 onward,
    and 0x72106C copies the dwords counted at +0x18 to +0x1C onward.
    0x72A164 is a constructor (vtable 0x72A138 at +0; its second argument is
    stored at +0x1C). It computes no gain.
    0x5F1170 returns a float10 in st0: the minimum of the FUN_0043CE10 values
    at +0x80 and +0x90, plus, where its mode test does not return early (the
    test reads DAT_0089CF69, DAT_008669F4, DAT_00866A14 and +0x14), those at
    +0xA0 and +0xB0 (and +0xC0 and +0xD0 when +0x14 is 5 or more). FUN_0043CE10
    is not traced, so "distance" is Inferred.
    **Open**: the ramp endpoints g0 = [V+0x34][i] and g1 = [V+0x38][i] (V: the
    voice record, see Mixer channel law) are passed to 0x728957 at 0x728C11
    (also called at 0x726671, 0x726717 and 0x728C9D). 0x728957 is not traced
    (the candidates for the azimuth and pan to L/R law). The channel count
    (0x8B47F9), the flag (0x8B47FA) and the pump's buffer (0x8B4780); how the
    +0xA8 distance becomes a level (0x5F1170 returns a minimum in st0, above);
    the AEMS sound-player node and the object at 0x8B4928. The voice gains are
    traced now (writers 0x72905D and 0x729087, snapshot 0x728F75; Audio,
    "Mixer channel law"). Not ported: the law is not fully traced.
    The water drum's extra start (verified from the disassembly at
    0x46AE00, material 5; Ghidra has no procedure there): after the call
    to 0x46A540 (its role is not traced), when +0x140 is 5, the record at the
    object's +0x144 has
    byte +0x505 non-zero and +0x50C clear (the function sets +0x50C to 1;
    nothing in it clears it), up to three requests are queued by 0x7358C0 at
    0x82C6B4 (the layers use 0x82C5E0). Each is a 0x20-byte node from
    0x457E20 with kind 3, sound 0, level 0, pitch 0, azimuth 1, delay 0 and
    +0x1C 0, stored at the object's +0x14C, +0x150 and +0x154 where the slot
    is empty (a failed allocation stores 0; a refused queue calls 0x735840
    with 0x7EDD58 and retries once). +0x505 (the prop knocked loose,
    0x5ADFA0) and the requests' per-frame level (0x481FD0) are traced in
    **Sound round 12**, which ports the splash.
    The +60 condition (verified, 0x47A770, the class 0 track sample): when
    the car at object +0x118 has +0x18 with +0x58 = 6, the volume byte gets
    +60 (at most 127) before the level is read. Car +0x18 is the player
    object (only a human's car has one) and its +0x58 the camera event
    kind (see **Crash cameras and slow motion**): 6 is the crash camera
    (the jump camera, 7, gets nothing). The +60 moves only the sample
    choice (the level group of kinds 2–6); the layers' volume is the
    record's. **Ported** 2026-11-01: `CarState::crash_camera` (the
    player's `EventCameras::kind` = 6, set each race frame) →
    `collision::Impact::crash_camera` → `play`'s level.
    The 10-object cap (**open**): the count gate (`cmp [ecx+0xC], 0xA` at
    0x481D4B, `jge` at 0x481D4F) returns when `9 < [S+0xC]`, where S is M+0x3C
    and M is the pointer at 0x82B884. 0x483E20 reloads ecx from +0x3C
    (0x483E29) before its `jmp 0x481CD0` (0x483E2C), and the list walk leaves
    ecx unchanged. The zeroing at 0x458634 is in M's constructor (0x4585A0),
    not S's, and S's constructor was not traced.
    No inc, dec, or add or sub of 1 on a non-esp [reg+0xC] was found in
    0x450000–0x5BFFFF, but add and sub of registers through [ecx+0xC]
    (0x6A0346, 0x6A0367, 0x700C7B, 0x700CBD) and 274 DWORD mov writes through
    [ecx+0xC] were not traced to S, so whether S+0xC reaches 10 is open. The
    global count is 0x82B944: +1 on each insert (0x481E13–0x481E21), −1 on
    each unlink (0x481C82, 0x469D98); the gate does not read it. One object
    per record holds (0x481CD0's key test on the list at +0x24).
  * Attached models and effects (traced, **not ported**): the info's +0x84
    / +0x88 hashes are gathered at load (0x5B4230) by 0x48B8B0 from the
    emitter manager 0x8357E0's list (+8): every entry whose description
    (+0x1C) has that hash at +0x10 is copied (0x48B830: 0xD0 bytes, the
    name at +0x90 and its hash at +8, the description, the matrix at
    +0x40) into the info's lists +0xF8 / +0xF0; a prop body copies them
    again into +0x4F4 / +0x4FC (0x5B1550, each +0x10 = 1); 0x5ADB30 sets
    each one's matrix to the prop's, and on first moving (0x5ADFA0, once:
    +0x505) each +0x4FC one is started by 0x48BC50, which queues it on
    the manager's list with a start delay drawn from its description's
    +0x2C / +0x30 (type 0) or +0x24 / +0x28 (type 1; type 2 is dropped).
    L4RA: `ZCS_BarrelFireA` +0x88 0x64C2C940, `ZCS_Barrel_A` +0x88
    0x95C9F5FA, `ZCS_WaterBarrel` +0x84 0x95C9F5FA / +0x88 0x95C9F5FB,
    `ZCS_XO_GreenHDoor_00` +0x88 0xE84348EA; the rest have neither.
    (Earlier notes said the `XO_BulletSS` / `XO_TableSS` kinds have +0x88
    = 1; they have 0.)
  * The `XO_BulletSS_nn` / `XO_TableSS_00` kinds (L4RA: 12 and 1) have one
    hash at +0xA0 (0x722DB2FC, 0x5CB9A13C), resolved at load (0x5B0F21)
    to the info with that key at +0x90; no L4RA placement uses them, and
    no reader of +0x90..+0x9C was found (none of an info in the prop code
    0x5AD000..0x5B4FFF; open: what the parts do).
* **Car modes** (car +0x38): crash mode (0x5A49B0 each frame) turns off
  rest and angular damping (+0x150, +0x154), sets gravity 11 (0x7FC084),
  the aero factor (+0x30, multiplying drag and downforce in 0x59E630) to 0
  and the surface drag to 1, and holds the gas at 0.4 before the throttle
  is set (0x5A49B0 runs before 0x5ABC00). It ends in the mode update
  (0x5FC060): after 0.1 s with all four wheels down and no contact (a
  contact or lifted wheel restarts it). Mode 2 (in contact) has the
  normal physics.
* **Ground inertia** (rigid body +0x158, 0x5A49B0): the rigid body keeps
  two inverse inertias (0x59B950): +0x170 from the record's tensor and
  +0x1B0 with its yaw term × record +0x270 (2.0–2.28 for the stock cars;
  scaled by the race tunes' t4). Outside crash mode with a wheel down it
  uses +0x1B0 — in ω = I⁻¹L (0x59C600), the rolling constraint's yaw
  acceleration (0x5ABE00 at 0x5AC053) and contacts (0x593A50) — else
  +0x170; switching to +0x170 divides L.z by the factor (switching back
  does not). (Earlier notes said nothing in the physics reads +0x270.)
