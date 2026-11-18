# Car highlight from the scene, not the camera rig (clone deviation)

Issue: a bright light spot on the car's trunk at the higher settings, most visible on Rachel's car. The bright yellow-green patch in the middle of the 350Z's trunk (the D-Day intro, the
chase camera) is the **native** car light: it is in the original too (frames of the original under Wine and the look-direction spots' frames, kept with the maintainer's private evidence). The Original
look keeps it exactly; the car shading enhancements now light the highlight from the scene instead.

## The native light (traced)

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

## The change

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

## Look rules (docs/look-direction.md, the maintainer's tools, the look direction spots with the original's cameras, 1280×960)

The change alone, play setup (perf.sh USER; the look direction's `user`) after − before, 16 spots: Δmean **−1.18**
(R1 ±2.5), Δp5 −0.18 (R2 ≤ +1), Δp95 **−2.38** (R3 ±4), Δglow −0.09 (±0.2), ΔRMS −0.88 (R4 ≥ −2), Δsat
+0.005 (R5 ±0.02), Δsky 0.00; crushed 2.57 → 2.67 %. Ultra −1.30 / −0.25 / −2.48 / −0.09 / −0.95;
max −1.34 / −0.13 / −3.61 / −0.11 / −1.24. Against the original (Wine, 11 dry spots): user mean 68.4 vs
69.6, p5 11.3 vs 11.1, p95 200.7 vs 208.2; Ultra 70.3 / 12.1 / 200.2. The highlight was among the
frame's brightest pixels, so p95 falls (per spot −0.2 … −5.0, the most behind a near player car: street,
downtown, coal_west, jackson_hills); Ultra's total p95 vs clone Original is −3.75 (16 spots) / −4.21
(11 Wine spots), at the edge of R3; nothing is lifted, the blacks are unchanged, saturation is up slightly.
Table: internal evidence.

## Evidence (the maintainer's private captures)

`intro_350z_car_200.webp` (Rachel's 350Z, the intro, 200 %: Original (Wine) | clone Original | play setup
before | play setup after); three other cars at 100 % (clone Original | play setup before | after, the Wine frame
first where the look direction has one): `street_lancer_100.webp`, `downtown_car_100.webp` (Lancer), `qrstart_206s_100.webp`
(two 206s), `drift_grid_100.webp` (Focus, 206, RX-8 from behind). Full frames (spots, before,
after) are kept outside the repository.

## Self-assessment and open

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

## Energy (the first graphics-audit round, 2026-11-12, graphics audit fix 1; superseded by "Higher settings" below)

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

## Which settings

The rule: Original — native rig highlight, exactly; lower Enhanced levels — native, untouched;
higher Enhanced levels — removed, the car lit by the scene; any ray-traced setting — removed. One rule in one
place, `qol::scene_lights_car(CarShadingRows)` (src/qol/atomics.rs), read through `qol::scene_car_specular()`
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
* Tests: `qol::atomics::car_highlight_rule_tests` (every preset × GPU class, rows alone and in pairs, that
  play setup).

## Higher settings: no stand-in (GFX-T, 2026-11-12)

The first consolidation unit's environment highlight (world-fixed zenith + ring lobes in w at ENV_GAIN × the
rig's energy) brought the trunk patch back from a chase camera (intro trunk 83.3). Removed: with the scene
specular the w channel holds the scene's lights only (lamps of the light packs / scene groups, the native
0.8 / length scale). Tried and rejected (intro 350Z, High preset): a 10° horizon ring at 0.9 (trunk 72.6,
p95 2.27 × the roof's) and an isotropic w (trunk p95 1.57–2.56 × the roof's): any energy in the harmonic's w
lands on the trunk again, because the specular's tint is the rig's camera-fixed colour lobes and car.efx's
SpecularMin + (n·v)^p · SpecularRange favours the panel that faces the camera. What carries the body instead
is the environment reflection (the world cube, the scene the paint really reflects): `SCENE_ENV_GAIN` 1.15 on
EnvmapMin/EnvmapRange (`CarLighting::env_gain`, src/render/native/car/instance.rs). **Inferred** (tuned):
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
