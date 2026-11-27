# Car look notes

Five investigations of how the car looks: its paint, reflections and rims against the original, the bright
highlight on the trunk, a chrome trunk's flicker, raindrop-like spots on a dry car, and how the darkening
settings meet the car's glows and shadow. The settings themselves are on the
[settings pages](../../playing/settings/README.md); the index of graphics notes is [README.md](README.md).

## Car paint, reflections and rims against the original

The 2026-10-29 note "matte vs saturated" (wine/compare/DIFFERENCES.md) was re-checked
with the current clone (after the physics merge, Enhanced Reflections off = default) against the original
on DXVK/RADV (`fastgpu.sh`, vanilla, WidescreenFix off, 1280x960, max detail).

### Matched frames
The original's Quick Race car select shows each car standing still, so its frames match the clone's
pixel for pixel in pose and camera. New shot aid: `NFSU2_SHOT_QRCAR=<car type> game --shot qrcars`
puts that car on the platform as the list opens (no swing), as the original after Right×n
(240SX = Right×3, stock 350Z = Right×7, Peugeot 206 = none). `main_dday` matches the original's
main menu with a fresh profile (DDAY_PLAYER_CAR, Rachel's 350Z, same turntable pose); `intro`
matches the free-roam start after Rachel's SMS (`fastnav.py freeroam`).

Region means (0-255, R G B; orig → clone delta), after the fix below. Crops in
the maintainer's private captures (orig | clone | |diff|×4):

| Frame | car crop Δ | body Δ | rim Δ | glass / hood Δ |
|---|---|---|---|---|
| 240SX stock, QR car select (`qr-240sx.jpg`) | +1.0 +0.8 −0.3 | +0.1 +0.1 −0.9 | −0.2 −0.2 −0.7 | glass +0.9 +0.5 +0.5, hood −1.0 −0.8 −1.8 |
| 350Z stock, QR car select (`qr-350z-stock.jpg`) | +0.4 +0.3 −0.5 | −0.3 −0.1 −1.1 | +0.1 +0.2 −0.4 | glass +5.3 +4.6 +5.9, hood +5.3 +2.9 +1.6 |
| Peugeot 206, QR car select (`qr-206.jpg`) | +1.3 +1.1 +0.3 | 0.0 +0.1 −0.9 | +0.4 +0.7 +0.3 | hood −0.6 −0.4 −1.2 |
| Rachel's 350Z, main menu (`main-dday-350z.jpg`) | +0.8 +0.9 +0.5 | +0.1 0.0 −0.2 | +0.1 −0.2 +0.6 | hood +1.6 +5.3 +2.5 |
| Rachel's 350Z, free roam (`intro-350z.jpg`) | +0.8 +0.3 −0.4 | upper body +0.7..+1.5 / −0.7..−1.8 | – | trunk +0.5 +1.9 −0.8 |

Full numbers and rectangles: `region-means.txt`. The +5 on the stock 350Z's windshield and hood
are its near-white highlights (orig 235/215/238): same shape, slightly brighter in the clone;
**inferred** to be the overbright/glow passes on saturated pixels, not car.efx (the constants are
equal, below).

The car crops agree to about one level in every channel. The remaining pixels that differ are
edges (sub-pixel rasterisation/filtering), the small blinking warning lamp behind the car, and the
mouse cursor in the original. The dark body sides are ~1 level lower in blue in all three cars
(not traced; within the post chain's rounding). The 10/29 "matte grey" was the clone before the
post-chain rounds (see formats.md "Re-compared 2026-10-30").

### Constants compared (apitrace)
An apitrace of the original (QR car select, stock 240SX; frame 10600 of the trace, both the floor
reflection pass and the main pass, 197 car.efx draws, VS8 = car.efx vs_1_1) was dumped with the
trace tool's `--consts` option. Every draw's material constants (VS c18 DiffuseMin, c19
DiffuseRange, c20/21 Specular Min/Range, c22 SpecularPower, c23 EnvmapPower, PS c0/c1 Envmap
Min/Range) equal one of the clone's light-material constant sets (0x5BD4B0 on GLOBALB 0x135200) to
5e-6: paint METPAINTSILVER, rims MAGSILVER, glass WINDSHIELD / WINDOWMASK, CHROME, MOLDINGS, RUBBER,
BRAKEDISC(_DIFFUSE), BOTTOM, DECAL, HEADLIGHTGLASS, BRAKELIGHT(GLASS), CLEARPLASTIC, ALUMINUM,
DEFAULT, LICENSEPLATE, DULLPLASTIC (PLAINNOTHING has the same constants as DRIVER). The 21 distinct
sets are internal evidence (count, 22 values, matched material).
HarmonicCoeff / LocalEyePos / the env cube / the post chain were checked against traces on
2026-10-29..11-01 (formats.md, "Car shading checked against the running original") and are not
re-traced here; the matched-frame means above show no change since.

### Fix: car.efx flags are per material record (verified)
The trace had four draws (DULLPLASTIC, 54 and 31 primitives in the main pass) with EnvmapMin/Range
= 0 although their meshes (240SX_BASE_A, 240SX_KIT00_TRUNK_A) have header flags 0xC180. The flags
0x5BD4B0 tests come from the draw, not the mesh header:
- 0x5C5BB1 (in the mesh draw loop, esi = the 0x3C-byte material record of chunk 0x134B02: +0x1C
  texture slot, +0x20 light material) pushes `[esi+0x38]` as the third argument of 0x5CEB40;
- 0x5CEB40 stores it at the queue entry's +0x14 (0x5CEBEF), next to +0x20 the light material;
- the flush 0x5CF430 calls 0x5BD4B0(entry+0x20, entry+0x14) (0x5CF559…0x5CF55E);
- 0x5BD4B0: `(flags & 0x100) == 0 || g_CarEnvironmentMapEnable 0x870CB8 == 0` → no env term;
  else `(char)flags >= 0` (0x80 clear) → no diffuse/specular.
Those two ranges carry 0x4080 in their records. The clone used `Solid::mesh_flags`; it now reads the
record's +0x38 into `solid::Material::flags` and `render/car.rs` passes it to
`LightMaterial::constants`. With it, the clone's front-end LOD-A draws of the 240SX give exactly the
trace's 21 constant sets and no others (test below; the engine's MAGGUNMETAL is excluded because the
original does not draw the engine under the shut hood). Visible effect: no env reflection on those
dull-plastic ranges (underbody, trunk inner), a few pixels in these views.

### Test
`car::shading_trace_tests::stock_240sx_constants_match_the_original_frame` (skips without
NFSU2_ROOT): trace sets ⊆ clone sets and clone sets ⊆ trace sets (per material, engine excepted),
DULLPLASTIC both with and without the env term.

### Open / observed elsewhere
- **Front-end floor** (not car shading): the turntable floor left of the car is 10-12 levels
  brighter in the clone (region 300x40+100+700: 59/58/48 vs 71/70/56 on main_dday; same on every QR
  car), with yellowish patches under the car's front. Not traced in this work. **Leftover**: the floor's tiles stood at another angle (the platform's turn; internal notes); fixed.
- **Engine under the shut hood**: the original does not draw 240SX_KIT00_ENGINE_A on the car select
  (no draw with its 305/317-primitive ranges in the trace); whether the clone draws it is not
  checked (hidden either way).
- **Quick Race grid**: the player's grid slot and the rivals are random draws in both games
  (docs/notes/parity/race-ai-parity.md, row 5), so the in-race frames are not matched; the in-game comparison uses the
  free-roam `intro`.
- The in-game env cube contents differ slightly (C-pillar reflection in `intro`), as the dynamic
  cube's face order/timing is not frame-matched; car-crop means still within 1 level.

### Reproduce
Original: `fastgpu.sh <copy> <disp> --fresh` (+ `--apitrace T`), `fastnav.py <copy> main`, then
Return×4 (Circuit, track, options → car select), Right×n, mouse moved off with
`DISPLAY=:N xdotool mousemove 1275 955`, `peek.py shot`. Regions: `regions.py ORIG CLONE
name=WxH+X+Y…` (ImageMagick means). Clone: `NFSU2_SHOT_QRCAR=240SX game --shot qrcars out.png`.

## Car highlight from the scene, not the camera rig

Issue: a bright light spot on the car's trunk at the higher settings, most visible on Rachel's car. The bright yellow-green patch in the middle of the 350Z's trunk (the D-Day intro, the
chase camera) is the **native** car light: it is in the original too (frames of the original under Wine and the look-direction spots' frames, kept with the maintainer's private evidence). The Original
look keeps it exactly; the car shading enhancements now light the highlight from the scene instead.

### The native light (traced)

* 0x620AF0 builds each car's light context; in game 0x6316A6 (also 0x631889, 0x6318ED) passes the static
  rig **0x7F3F00** (`lighting::IN_GAME_RIG`): three white directional lights, all flagged camera-fixed
  (+0x20 records of 0x1C bytes, read from SPEED2.EXE: (1, θ 200°, φ 90°, 0.5), (1, 42°, 33°, 0.35),
  (1, 219°, 10°, 0.1)). 0x48A310 turns a flagged light into the camera's axes, so the key light (0.5)
  always comes from straight above the camera, the fill from behind the camera's right shoulder.
* 0x48ED60 adds each light as a second-order harmonic lobe (0x4895E0, plus a −0.05 lobe opposite,
  0x489AF0); the w channel is the light's luminance; 0x489ED0 scales w to a length of 0.8 (0x7F39BC = 1,
  0x7F39C0 = 0.8, read from the EXE); 0x5BD150 uploads it as HarmonicCoeff.
* car.efx's specular is `harmonic(R).rgb × clamp(harmonic(R).w)^SpecularPower × (SpecularMin + (n·v)^p ×
  SpecularRange) / 2`, R the reflection vector (`render/car.wgsl`). With the rig fixed to the camera, the
  highlight's peak sits on whatever panel reflects the eye towards "above the camera": from the chase camera,
  the trunk (or a hatch's upper rear, a bumper's top). It does not move when the car turns under the
  camera, which is why it reads as a light source glued to the trunk.
* formats.md's open item ("which light of the context makes the trunk patch") is answered by the
  geometry: the key light (0.5, above the camera) — a reflection off the trunk lid from a chase camera
  points up (chase camera pitched 10°, lid tilted 25°: w at R = 0.50 from the key, 0.15 from the fill, 0.03 from the rim; **computed**, not measured per light under Wine).

Bisect (the intro, the play setup, `work/bisect`: mean luma of the trunk centre, 160×60 px): Original
206.3; play setup 213.0; play setup minus Modern Lighting 207.8; minus Enhanced Paint 212.6; minus Enhanced
Reflections 212.3; minus Headlight Beams (Off or On instead of Next-Gen), Light Probes or Dynamic Lamps
213.0; every car row off 206.2. So no enhancement makes the patch; Modern Lighting adds ~+5 on top.
After this change: **159.9** (the paint and its environment reflection).

### The change

`qol::scene_car_specular()` — the rule `qol::scene_lights_car` (section "Which
settings" below; it was Enhanced Car Paint ∨ Enhanced Reflections ∨ Modern Lighting before; no new row, no
new label). In game
(`WorldLights::car` → `CarLighting::compute_with(…, scene_specular)`), the context's **w channel holds the
scene's lights only**: the light packs' and scene groups' lobes (0x48E3D0's lights), at the native scale
(0.8 over the length of every light's w, the rig included, so a lamp's highlight is exactly as strong as
natively and never boosted). The colour channels are untouched: the rig still fills the body's diffuse
(and tints the highlights), only its camera-fixed highlight goes. Every consumer of w follows: the native
per-vertex specular, Enhanced Reflections' per-pixel specular and Enhanced Paint's coat and flake lobes
(their "dominant light" is w's linear terms, so they now point at the nearest street lamp,
not at the camera). The environment map (the world cube) is the sky/scene reflection, as before. The front
end (garage, showroom, car lots) keeps its light setups: there the rig is the scene's studio light.

Original: `compute` = `compute_with(…, false)` (the same sums; byte-identical, gate parity below).

### Look rules (docs/notes/graphics/look-direction.md, the maintainer's tools, the look direction spots with the original's cameras, 1280×960)

The change alone, play setup (perf.sh USER; the look direction's `user`) after − before, 16 spots: Δmean **−1.18**
(R1 ±2.5), Δp5 −0.18 (R2 ≤ +1), Δp95 **−2.38** (R3 ±4), Δglow −0.09 (±0.2), ΔRMS −0.88 (R4 ≥ −2), Δsat
+0.005 (R5 ±0.02), Δsky 0.00; crushed 2.57 → 2.67 %. Ultra −1.30 / −0.25 / −2.48 / −0.09 / −0.95;
max −1.34 / −0.13 / −3.61 / −0.11 / −1.24. Against the original (Wine, 11 dry spots): user mean 68.4 vs
69.6, p5 11.3 vs 11.1, p95 200.7 vs 208.2; Ultra 70.3 / 12.1 / 200.2. The highlight was among the
frame's brightest pixels, so p95 falls (per spot −0.2 … −5.0, the most behind a near player car: street,
downtown, coal_west, jackson_hills); Ultra's total p95 vs clone Original is −3.75 (16 spots) / −4.21
(11 Wine spots), at the edge of R3; nothing is lifted, the blacks are unchanged, saturation is up slightly.
Table: internal evidence.

### Evidence (the maintainer's private captures)

`intro_350z_car_200.webp` (Rachel's 350Z, the intro, 200 %: Original (Wine) | clone Original | play setup
before | play setup after); three other cars at 100 % (clone Original | play setup before | after, the Wine frame
first where the look direction has one): `street_lancer_100.webp`, `downtown_car_100.webp` (Lancer), `qrstart_206s_100.webp`
(two 206s), `drift_grid_100.webp` (Focus, 206, RX-8 from behind). Full frames (spots, before,
after) are kept outside the repository.

### Self-assessment and open

* The patch is gone at every tier above Original, and the bodies read as paint lit by the street (lamp
  highlights where the lamps are, the world in the reflection). The cost is a less glossy car where no lamp
  is near (the bumper top band and roof lose the rig's broad sheen; Δmean −1.2): it is the honest scene
  light, but darker than the original's studio look on an empty road. A world-fixed replacement of the key
  light was rejected: from a chase camera "above the camera" and "above the car" put the peak on the same
  trunk lid.
* Not done: the street lamps' highlight is still the harmonic's broad lobe (no per-pixel GGX from Dynamic
  Lamps' four lights outside Modern Lighting).
* Which rig light makes the patch is computed from the geometry; a per-light check under Wine (writing the
  rig at 0x7F3F00 with peek.py) was not run.

### Energy (the first graphics-audit round, 2026-11-12, graphics audit fix 1; superseded by "Higher settings" below)

The audit found the cost above much larger than measured here: with Enhanced Paint, Enhanced Reflections or
Modern Lighting alone the car box (street, 420..880 × 520..880) fell 69.4 → 55 (−20 %), the hood camera's
hood −51 %, the rear window black at speed. **Cause** (checked by putting the rig's lobes back into w
under Enhanced Paint, which restores the Original's car-box values within 0.6 %): `compute_with`'s
`scene_specular` keeps only the scene lights' lobes in w, and where no lamp is near (most of the time)
w ≈ 0, so `harmonic(R).rgb × w^SpecularPower` — the per-vertex specular (car.wgsl `vs_main`), Enhanced
Reflections' per-pixel copy (`car_colour`) and Paint's coat/flake "dominant light" — has nothing left;
the environment term is untouched, so nothing replaced it.

**Change**: with the scene specular, w = the scene lights' lobes + `lighting::environment_specular`, the
rig's energy (its luminance × `ENV_GAIN` 0.9) as **world-fixed** lobes: 60 % (`ENV_SKY`) from the zenith
(the night sky's glow), 40 % as a ring of four lobes at 30° elevation (the lit street around the car),
each with the native −0.05 opposite lobe, at the native 0.8 / length scale. It does not follow the
camera, so the highlight moves over the body as the car turns. CPU only (the same harmonic upload): no
GPU cost (`--bench racing` 1080p, 2 runs: Original 0.68/0.70 → 0.69/0.70, Paint 0.70/0.70 → 0.69/0.72 ms).
The constants are **inferred**: tuned on the audit's scenes so the car is as bright as the native one.
A pure horizon ring (no zenith) matched the chase and hood views but over-lit the front-facing cars of
the race start (+5.4 mean over Original); the zenith-heavy mix is the compromise.

Car box (mean / p95 vs Original, `audit/gfx/b1/table-after.txt`): every scene × {Paint, Reflections,
Balanced, High, Ultra} within ±5 % (worst: hood Ultra +4.8 %, rain Ultra +4.4 %); Modern Lighting alone:
street −1.4, rain −1.6, hood +5.3, tunnel −6.1, racing frame 420 −9.4 % — the rest of Modern Lighting's
gap is its own car relighting (it was −7..−9 % below the other two options before too), not the rig.

**Open / for review**: energy back from a chase camera means light on the trunk again: the 350Z's
trunk centre in the intro (this work's measure) is Original 108.8, this work 38.2, now 83.3 — a softer, greener
sheen than the Original's yellow patch, but the patch is partly back (`audit/gfx/b1/sheets/intro-350z-paint.jpg`).
Lower `ENV_GAIN` trades car brightness for less of it. The race start's cars (front view) are +1.3 mean
over Original with Paint or Reflections alone (they were −1.4 / −3.7).

The chain in `lighting_world.rs` is now one function of a per-frame `CarLightOptions` (scene specular,
Dynamic Lamps, Car Lights Next-Gen, Light Probes) with the layers in order; merging the four lamp
models (Dynamic Lamps, Modern Lighting's, Next-Gen's, probes) into one light list is left to the
lighting merge (audit §4).
Evidence: `docs/evidence/gfx-b1/` (Original | before | after sheets: street Ultra, hood Balanced, neon
Light Probes, race start Car Lights Next-Gen, racing frame 420 High, the 350Z trunk; `table.txt` the full
scene × option table; the car boxes (an internal script), on the audit's `lstats.py` metric).

### Which settings

The rule: Original — native rig highlight, exactly; lower Enhanced levels — native, untouched;
higher Enhanced levels — removed, the car lit by the scene; any ray-traced setting — removed. One rule in one
place, `qol::scene_lights_car(CarShadingRows)` (src/qol/store.rs), read through `qol::scene_car_specular()`
by `CarLightOptions::current` (values in force: a ray-traced tier counts only while the device traces):

* **higher** (removed): Modern Lighting at any tier (Low, Modern, Modern + RT); Screen-Space Reflections Ray
  Traced; Enhanced Car Paint or Enhanced Reflections together with Light Probes **or** with Screen-Space
  Reflections (Half / Full). Presets: High, Ultra, Extreme, at every GPU class; a common play setup
  (Paint + Reflections + SSR Full, no probes, no Modern Lighting).
* **lower** (native): Original; Balanced (Paint); Paint and/or Reflections alone; every other row alone
  (Light Probes, Dynamic Lamps, Headlight Beams Next-Gen, SSR without Paint/Reflections, …).
* **The cases between** (decided here): (1) Paint/Reflections **with SSR but no probes**
  counts as higher — SSR is the scene reflected in the paint, so that play setup counts as higher. (2) Light Probes, Dynamic Lamps, Car Lights Next-Gen alone stay
  lower (native). Paired images, kept (top) | removed (bottom), intro 350Z:
  `gfxt-between-kept-vs-removed.jpg` (probes, lamps, beams NG, Paint + Reflections). Dynamic Lamps alone
  brightens the native patch (trunk 118.9 vs 108.8): the lamps add on top of the rig. (3) The streamlined
  levels (audit §4) do not exist in the tree yet: Low → lower, Medium/High → higher when they come (they hold
  the lighting merge and probes).
* Tests: `qol::store::car_highlight_rule_tests` (every preset × GPU class, rows alone and in pairs, that
  play setup).

### Higher settings: no stand-in (GFX-T, 2026-11-12)

The first consolidation unit's environment highlight (world-fixed zenith + ring lobes in w at ENV_GAIN × the
rig's energy) brought the trunk patch back from a chase camera (intro trunk 83.3). Removed: with the scene
specular the w channel holds the scene's lights only (lamps of the light packs / scene groups, the native
0.8 / length scale). Tried and rejected (intro 350Z, High preset): a 10° horizon ring at 0.9 (trunk 72.6,
p95 2.27 × the roof's) and an isotropic w (trunk p95 1.57–2.56 × the roof's): any energy in the harmonic's w
lands on the trunk again, because the specular's tint is the rig's camera-fixed colour lobes and car.efx's
SpecularMin + (n·v)^p · SpecularRange favours the panel that faces the camera. What carries the body instead
is the environment reflection (the world cube, the scene the paint really reflects): `SCENE_ENV_GAIN` 1.15 on
EnvmapMin/EnvmapRange (`CarLighting::env_gain`, crates/ob-render/src/render/native/car/instance.rs). **Inferred** (tuned):
1.6 lifted the roof and quarters (which reflect the lit walls) past the trunk (trunk 57 vs roof 77) and
2.2 made the 350Z brighter than the Original (car 101 vs 84.3) with a wet-plastic look; 1.15 keeps the trunk
at its panels' level. Lower settings and the front end: `env_gain` 1, the native material.

Intro 350Z trunk (`gfxt-intro-trunk.txt`; trunk 560..720 × 670..730, roof 580..700 × 548..562, quarters
460..500 / 780..820 × 690..740, hot = largest 4-connected region of the lid 540..740 × 640..720 above the
Original's trunk p90 222.8): Original 108.8 (p95 237.3, hot 2916 px); Balanced 109.3; after, higher:
High 52.1 (roof 55.6, quarters 80.2 / 59.3; p95 1.13 × the roof's), Ultra 52.5 (1.13), play setup 39.3 (roof
40.3; 1.15), Modern Lighting 46.9 (roof 62.1, quarters 93.0 / 49.4; 0.88), Next-Gen 49.8 (1.09), SSR RT alone
36.1 (roof 54.9; 0.60), play setup + SSR RT 38.2 (0.66). The largest region above the threshold is ≤ 32 px in
every higher setting: the chrome Nissan badge (present in the Original too). Below the roof: Modern Lighting
(−15) and SSR RT (−19) — the roof reflects the lit wall, the trunk the night sky.

**Cost** (car box, `gfxt-carbox-table.txt`): with the rig gone and no stand-in the higher settings' cars are
darker than the Original wherever no lamp is near: street −16..−18 %, rain −11..−15 %, tunnel −16..−18 %,
racing frame 420 −9..−11 %, hood camera −47..−49 % (the hood's broad white sheen was the rig's key light), drag
Skyline −21..−25 % (the silver rear reads near black on the unlit lot: `gfxt-drag-skyline.jpg`; an env gain of
3 lifts it only to −15 %, the cube is dark there). Nothing is brighter than the Original. This is the
rule taken literally; whether the higher levels should carry more scene energy (e.g. a brighter car cube /
sky term) is an open taste question (paired: `gfxt-street-hood.jpg`, `gfxt-drag-skyline.jpg`).

## Chrome trunk flicker and dry-road reflection strength

Issues:
- on a community save's Lancer the chrome trunk's reflection flickered;
- dry roads showed too much reflection.

### Cause of the flicker: the carbon trunk drawn as a mirror (docs/notes/graphics/car-materials.md)
The save (a community save; the maintainer's probe `save_parts`) fits these parts to the
LANCEREVO8:
- KIT_CARBON (0x5D) and TRUNK_CARBON (0x60);
- the TRAP CARBON FIBRE hood;
- the APOCALYPSE wide body;
- REGPAINTYELLOW gloss paint.

There is no chrome paint. The "chrome trunk" is the carbon trunk. The clone drew carbon with the
paint's light material at the carbon texture's alpha 1, twice the skin's gloss weight. With the yellow
gloss paint (Enhanced Car Paint: Gloss, SSR roughness 0.02) that is a mirror. Its reflections of the
moving scene (car cube and SSR) changed shape every frame (internal evidence: eight frames of the community-save trunk at 144 Hz).

The original draws carbon with `CARBONFIBER`: env 0.12 and a dull sheen (docs/notes/graphics/car-materials.md, traced
and compared in the same Body Shop scene). With that fix the mirror patches are gone.

### Measurements (`--bench explore` with a community save)
New bench aids:
- `NFSU2_BENCH_HZ=n`: frames at that display rate through the windowed loop's cadence + interpolation.
- `NFSU2_BENCH_FREEZE=n`: the car and camera held from frame n on.
- `custom.N V` in NFSU2_BENCH_ENH / NFSU2_SHOT_ENH.

The metric is `scripts/flicker.py` (luma: per-pixel temporal std; mean |second temporal difference|
"acc", which cancels steady motion). Maps come from `scripts/flicker_map.py`.

Trunk lid 200×36+540+612, driving off, acc_mean (acc>16 %):

| setting | 60 Hz | 144 Hz | 240 Hz |
|---|---|---|---|
| native, before / after | 2.73 / 2.54 | 1.42 / 1.36 | 0.85 / 0.81 |
| max setup, before | 6.57 (8.9 %) | 3.95 (4.1 %) | 2.24 (1.6 %) |
| max setup, after | 5.98 (7.8 %) | 3.67 (3.6 %) | 2.09 (1.4 %) |

At 144 Hz, dropping one row at a time from max (after):

| row dropped | acc_mean |
|---|---|
| Sharpen 100 | 1.98 |
| SSAA 2x | 3.20 |
| car reflection 1024 | 3.55 |
| SSR | 3.53 |
| SSAO | 3.49 |
| Enhanced Car Paint, Enhanced Reflections, Reflection Rate, lamp shadows, Wet Roads | ≈ 3.67 (no change) |

Car frozen (`FREEZE=100`), whole trunk, std_mean:
- native 0.13;
- max before 0.52, after 0.45;
- max without Sharpen 0.18.

The env cube schedule is not a factor: Reflection Rate High refreshes every face every frame, and
dropping the row changed nothing. Interpolation is not a factor either: frozen 60 vs 144 Hz is the
same.

**Remaining:** Sharpen 100 is the largest amplifier of the shimmer that is left. CAS sharpens the
512² carbon weave and the edges as they move. That is the row's own trade-off and the CAS pass is
shared by everything, so it is left as is. A lower Sharpen setting removes most of it. **Open:** the
original's carbon trunk in motion was not captured under Wine (only in the Body Shop).

### Dry-road reflection strength
Natively the road always reflects: worldreflect at the texture's authored alpha, wetness never read
(docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections").

Measurement (the maintainer's `road_reflection.py`, 1280×960 shots, lower half):
- contribution = shot − the same shot with `custom.7 0` (road reflection off; shots now honour custom.7);
- mean over the pixels where any variant reflects;
- ratio to native.

| row (alone) | explore | qrstart | intro (night) |
|---|---|---|---|
| Road Reflection Full | 1.00 | 0.99 | 1.00 |
| Enhanced Reflections (Schlick weight) | 0.48 | 0.75 | 0.75 |
| Wet Roads (dry road: native path) | 1.00 | 1.00 | 1.00 |
| **SSR Full, before** | **1.24** | **1.26** | **1.48** |
| SSR Full, after | 0.87 | 1.09 | 1.15 |
| max setup, before | 1.17 | 0.66 | 1.06 |
| max setup, after | 0.54 | 0.46 | 0.66 |

`dry_road_reflection.txt` has the share of pixels changed by more than 4 levels. The before and after
masks differ slightly; the ratios are within each run.

Only SSR raised the strength. The traced scene colour is brighter than the planar capture it
replaces. It is not the glow sprites (NFSU2_SSR_NOGLOW: same) and not the fog removal (off: same).

**Fix (`ssr.wgsl` fs_trace):**
- On a dry road (`ssr::Mask::wet`: Wet Roads' uniform not on this frame) a road ray's traced colour is
  held to the planar term's luminance at that pixel.
- Darker detail stays as traced.
- Hits on cars stay as traced. They add cars the native capture lacks, which is what keeps SSR
  slightly above 1 in qrstart and at night (**inferred** from where the excess sits).
- A wet road (real wetness) is unchanged and may go beyond native.

### SSR neon streaks (from the neon unit)
With SSR Half / Full, the neon underglow showed as vertical magenta streaks behind the car.

- **Cause:** the trace added the glow sprites at the hit. Road rays reaching up under the car hit the
  neon tube, an additive mesh outside the mask, and picked up the underglow's sprites. The
  vertically stretched filter then drew them as streaks. The native capture has no cars and none of
  their glows.
- **Fix:** no glow at hits whose mask texel is a car's, and glow only from a 2–4 m path on. The world's
  lamp glows are further off, and the composite keeps the planar's bright flares.
- **Result:** `neon_ssr_streaks_before_after.jpg` (rows SSR alone / max; columns without SSR, before,
  after). The streaks are gone and the pool is as without SSR.

### Native stays native
- With every row off, `--shot explore / intro / qrcars / qrstart` are identical to the base build
  (check lines in the report).
- The carbon fix changes only carbon parts, which is native behaviour.

### Tests
- `car::shared_part_tests::carbon_parts_take_the_carbonfiber_material` (the a community save trunk's lid included).
- `render::ssr::tests::traced_glow_skips_cars_and_short_paths`.

**Open:** no automated GPU test pins the trunk's temporal variance. The bench aids above reproduce it:

```
NFSU2_FLOW_DATA=<dir with NFS Underground 2/a community save/a community save> NFSU2_FLOW_LOAD=a community save NFSU2_BENCH_HZ=144 NFSU2_BENCH_ENH="…" NFSU2_BENCH_DUMP=… NFSU2_BENCH_DUMP_FRAMES=300-347 game --bench explore 347
```

then `scripts/flicker.py 200x36+540+612 frames…`.

## Spots like raindrops on a dry car

Development build. The setup: the Ultra preset and then every
graphics row at its maximum (internal evidence: render scale 200, SSAA 16x,
MSAA 16x, filter 4, Sharpen 100, High Precision Post 2, road reflection full, car cube 1024, reflection
every frame, Enhanced Reflections, anisotropy 16, reversed-Z, Ultra shadows, SSAO 100, detail 300, car
detail 300, LOD fade, keep loaded, Enhanced Car Paint, Wet Roads, SSR Full).

### Cause (verified by reproduction and bisection)
**Enhanced Car Paint's metallic flake (`crates/ob-render/src/render/car_paint.wgsl`), made much worse by Sharpen 100.**
- Each 4 mm cell (≥ ~1.5 px) took one random tilted normal and glinted towards the dominant light with a
  power-20 lobe, switched on/off per cell (`rnd.x > 0.6`), at up to 0.5·light·tint. On a smooth lit panel
  that is a sparse set of isolated bright cells — droplets. A ±3 % per-cell grain added a fine speckle.
- Sharpen (post) amplifies both into dense white dots.
- Bisection (`--shot intro`, 1920x1080, Rachel's 350Z trunk, speckle metric = mean |gray − median3x3|,
  `bisect-speckle.txt`): Original 2.59; all max 7.65; all max minus Sharpen 2.64; minus Enhanced Car
  Paint 5.25 (the no-paint level of this sharpened image; no dots visible); every other row off leaves
  7.6–8.5 (SSR, Wet Roads, SSAO, reflections, shadows, AA, scale: not involved). Each row alone on
  (Clean/Modern/Ultra presets and single rows, intro and garage): only Enhanced Car Paint shows dots.
- Original (all off) shows none and is unchanged by this fix (intro: 0 differing pixels vs before).

### Native (from existing traces; not re-run under Wine for this work)
car.efx has no wetness or rain input: its constants are HarmonicCoeff, matrices, LocalEyePos,
Diffuse/Specular/Envmap Min/Range, SpecularPower, EnvmapPower (CTAB, formats.md "car.efx registers",
verified against apitrace in the car-paint comparison). The original's rain drops are a screen/lens pass only (rain_drop.efx,
0x5C7AC0, `crates/ob-render/src/render/rain.rs`), and only while it rains. So a dry or wet car is shaded the same; nothing
drop-like belongs on the car. Wet Roads' track-load wetness (docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections") does not reach the
car shader (the bisection agrees).

### Fix (enhancement path only)
The flake is now filtered as real sub-pixel flakes are seen: each cell gets the mean lobe of its flakes,
`FLAKE_MEAN·(r·l)^FLAKE_POWER` = 0.072·(r·l)³ (Monte Carlo of the old model: 40 % of cells, tilt
±0.35/axis, power 20; matches within 0.02 for 0–40°), varied per cell by ±40 % (`FLAKE_SPREAD`), and
the grain is ±1 % (`FLAKE_GRAIN`). Same mean brightness and colour as before, no isolated islands, and
Sharpen 100 no longer creates dots (all max after the fix: 5.54, visually as with the paint off).
Clearcoat, crisp env, pearl shift, octave/fade logic unchanged.

Test: `render::car::paint_tests::flake_is_filtered_not_droplets` (no binary per-cell glint, no
tilted-normal glint, spread ≤ ±50 %, grain ≤ 0.02, filtered lobe = old mean).

### Evidence (the maintainer's private captures)
- `intro-paint-only-orig-before-after.jpg`: Original | Enhanced Car Paint alone, before | after.
- `intro-max-before-after-paintoff.jpg`: an all-max set, development build 1441 | fixed | max with paint off.
- `garage-metpaint-max-before-after.jpg`: `--shot main`, `NFSU2_SHOT_FIT=0x3F:104` (METPAINTSILVER), all
  max, before | after.
- `bisect-speckle.txt`, `max-options.txt`.

### Open
- Sharpen 100 still sharpens other fine detail (vinyl edges, texture noise) as designed; only the paint
  dots are fixed here.
- Not checked on the player's display size (shots at 1920x1080).

## Darkening settings against emissive light and the car's shadow

Clone deviations only (nothing traced; no native counterpart): the opt-in rows that darken
(SSAO, Contact Shadows, Lamp Shadows, Volumetric Fog's extinction, TAA's history clamp) checked
against the additive layers (underglow, glow sprites, streaks, flames, headlight pools), and the
car's shadow stack at the play setup (`<the play setup's options file>`;
report on development build 20261104-1930: "shadows and neon on highest settings look a bit off").
All rows off: md5-identical to the build before (explore, race, qrstart, neon_car, garage_neon,
intro, drift, drag, qrcars 1280×960); Ultra alone, Lamp Shadows, Volumetric Fog: AE 0.

### Rule
Occlusion darkens what surfaces reflect, never light emitted over them; and two layers that stand
for the same occlusion (the native shadow texture, Contact Shadows, SSAO under the car) do not
multiply.

### Changes
* **SSAO** (`ssao.rs` `run_into`, `PostFx::ssao_into`, game.rs frame + shot): multiplied into the
  scene (the multisampled one with MSAA; every sample takes its pixel's factor, = the old
  multiply after the resolve) right after the world and the cars, before the shadow texture and
  underglow, particles, glow sprites (streaks off), horizon fog. With SSAO on, the race start's
  characters are drawn before the shadow pass (so they occlude and receive it); order unchanged
  with it off. The old post-resolve path (`pre_passes`) is kept but no longer gets a frame in game.
  Inside a car's footprint the occlusion gives way to its underglow
  (`glow_boxes`, `glow_share` in `fs_apply`). the second graphics-audit round (audit fix 9): `glow_boxes` takes every car (≤ 8, the
  nearest first; centre.w the underglow, 0 none) and the car's **body** (inside its box, soft 8 cm rim, from
  10 cm over its floor) takes no occlusion — screen-space self-occlusion only dimmed paint, glass and lamps
  (street car box −2.2 % → −0.3 %, neon −1.7 % → −0.6 %); under a lit underglow the footprint takes none
  (was: by the glow's light). The ground round the car keeps its contact darkening. Neon / lamp boxes
  (street lamp, arcade lanterns, shop-front neon) were and stay within 0.3 %: the glow sprites and the
  additive layers come after the multiply.
  **Speed (the same pixels)**: each box carries a screen rectangle (`ssao::glow_rect`,
  `Uniform.glow_rects`, and their union): the screen's bounds of the box-aligned region outside which
  both of the box's terms are exactly 0 (x within ±max(half length + 8 cm, 0.85 × half length + 0.35 m
  + 0.15 × half width), y likewise with 0.8, z from the footprint's floor 0.9 m under the box, or the
  body's 4 cm over its floor without an underglow, to 8 cm over its top; 5 cm more each way). There
  every smoothstep of `glow_share` is at its end, so the box multiplies the share by exactly 1; a pixel's
  reconstructed position lies on its ray, so a position in the region is inside the projection of the
  region's corners (one pixel of margin). A region reaching the eye's plane gets the whole screen. The
  pass tests the union first (outside it: no loop, the share 1 as before) and each box's rectangle before
  its loads. A first form bounded the loop's 4 m reach instead: the player's own car, 5–7 m ahead of the
  chase camera, then covered the whole screen, and the tests cost more than they saved (apply +0.4 ms
  racing on the integrated GPU). Proof: `ssao::tests::a_box_changes_no_pixel_outside_its_rectangle`
  (the shader's f32 arithmetic at depths every 2 cm round each box: no factor other than 1 outside its
  rectangle; cars ahead with and without an underglow, beside, turned, far off, near the eye),
  `the_rectangles_follow_the_shaders_terms` (the constants are ssao.wgsl's),
  `tests::first_use::the_occlusion_glow_rectangles_draw_the_same` (a city circuit's start and first frames
  with and without the rectangles, High, Ultra, Extreme, Ultra at 25 %), the bench dumps at Medium, High,
  Medium with 8x MSAA (the multisampled apply) and the play setup, byte-identical; speed in
  docs/notes/performance/integrated-gpu.md "The occlusion apply's car loop".
* **Contact Shadows** (world.wgsl `contact_visibility`, lamps.rs `contact_share`): (1) a car's
  contact shadow keeps 1 − glow of its depth, glow = the underglow's brightest channel × drawn alpha
  (`shadow::neon_light`, occluder centre.w); (2) full inside 75 % of the car's box, 0 at the box
  edge (body + 0.1 m), was full to 85 % and 0.35 m + 0.15·half-width past it; (3) the native
  shadow texture over it is drawn through `fs_car_shadow` (`Effect::CarShadow`, lamps module only,
  alpha-blended shadow textures): its alpha cut so the ground ends at min(1 − a, vc), the darker
  of the two, not their product (inferred exact for a black shadow texel; SHADOWIG is dark grey).
* `--bench` takes NFSU2_NEON=<part> (the neon-driving pass's lines); `--shot` takes NFSU2_SHOT_NO_CAR_SHADOW=1.

### Glow area (MAGENTA, Peugeot, 1280×960; `glow-numbers.txt`, scripts/shots.sh + glow.py)
Mean R over the Ultra glow mask / glow pixel count, vs Car Shadow/Neon Ultra alone:

| row (on Ultra) | chase before | chase after | orbit before | orbit after |
|---|---|---|---|---|
| Contact Shadows | R −18.5 %, area −25.1 % | −0.6 %, −1.3 % | −17.3 %, −19.6 % | −0.5 %, −0.7 % |
| SSAO 100 | −13.1 %, −21.0 % | −0.6 %, −1.6 % | −14.1 %, −14.1 % | −0.7 %, −0.9 % |
| Lamps + Lamp Shadows High | +0.5 %, +1.1 % | same | +0.2 %, +0.4 % | same |
| Volumetric Fog High | 0 | same | +1.1 % | same |
| TAA (bench drive, frame 450) | — | −0.2 %, −0.7 % | — | — |
| all of these | −29.9 %, −37.7 % | −0.8 %, −1.7 % | −28.8 %, −31.3 % | +0.2 %, −0.2 % |

Driving (`--bench drive`, LANCEREVO8, frame 450, after): SSAO −1.7 %, Contact −1.1 %, TAA −0.2 %,
TAA + SSAO −1.9 %; Lamp Shadows +1.0 %; no darkening row left. TAA: the glow is low frequency, its
clip does not darken it. Volumetric Fog is additive (its extinction applies to its own scatter).

### The play setup (scripts/stack.sh + an internal box script; `boxes-before.txt` / `boxes-after.txt`)
Road luma (0–255) 0–0.5 m behind the bumper, chase cam, standing; 'far' ≈ 1 m behind = 59.7 / 57:

| | Original | user before | user after | user, Contact off |
|---|---|---|---|---|
| no neon | 40.7 | **20.9** (−49 %) | 28.7 (−29 %) | 35.8 |
| MAGENTA underglow | 76.2 | **45.6** (−40 %) | 73.2 (−4 %) | 74.4 |

Per layer (no neon, on Ultra): native shadow alone 40.7; + Contact 23.8 → 32.7; + SSAO 25 39.2;
+ Lamp Shadows High 40.8 (no lamp near; Dynamic Lamps off in the play setup); stack 23.1 → 31.6. The
rest of the play setup's 'user − contact' drop (40.7 → 35.8) is the grade of the other rows (High
Precision Post 2, Wet Roads, SSR: 'far' 59.7 → 57.1). Car Shadow/Neon Ultra's grid refine:
≤ 0.4 levels anywhere in these crops (shape, offset unchanged). Garage (`garage_neon`): user =
Original within 0.05 levels, unchanged by this work (no SSAO/contact in the front end).
Crops: `user_chase_{explore,neon_car}_orig-before-after.webp`, `user_high_*`; driving with the
play setup's values (frames 440–450): glow-box luma 97.9 Original / 99.2 user, frame-to-frame |ΔY|
6.0 / 7.4 (SSR + wet road; Contact on/off 7.4 / 7.5: no shimmer from the shadows).

### Open
* **SSR** reflects the underglow's tubes / glow as vertical magenta streaks under the car (crops,
  `user_chase_neon_car_*`, right): the main "off" left at the play setup; open for SSR.
* Contact Shadows still darken the road under the car's body (−20 % in the box behind the
  bumper, which reaches under the overhang); judged against Original only, no native frame.
* SSAO's residual on the road beside the glow (~1 %) and in the world pass's own additive
  textures (worldnofog signs, drawn in the world pass before the occlusion): left, small.
* The min() combine assumes the shadow texel is black (src ≈ 0); inferred, not measured per car.
