# Tyre smoke toward current games (clone deviation, opt-in)

Design note: "smoke looks better but still not like in new games". Scope (user's scope freeze): polish
of the **existing** Particles row's Cinematic tier (the Cinematic smoke work / the volumetric smoke `volume_fx` / the smoke integration, docs/smoke-volumetric.md),
in place: no new row, no new tier, no new option label. Nothing here exists in SPEED2.EXE and nothing new
was traced; the native emitters still decide where, when and how much smoke spawns (docs/formats.md
"Particle effects"). Original stays the default and byte-identical. All constants are by eye (**inferred**).

## 1. How current games do it (research notes)

Sources read: Unity's six-way lighting docs and blog (VFX Graph "Six Way Smoke Lit", 2022.2+), the
Real-Time VFX thread on Skull & Bones' smoke lightmaps, "Real-time Neural Six-way Lightmaps" (arXiv
2604.03748, which credits Müller 2023 / Ao 2023 for the technique), Hillaire 2015 (Frostbite unified
volumetrics), Schneider 2015 (HZD clouds). What the individual racing games do is **not documented by
their studios in anything found here**; the per-game lines below are the common understanding of their
look, from watching them, not confirmed sources.

* **Six-way lightmapped flipbooks** (the AAA default for smoke; confirmed as a technique, Unity / UE5
  Niagara ship it): an offline fluid sim (Houdini / EmberGen) rendered as 8 × 8 flipbook frames with the
  light baked from six directions; at runtime the scene's lights blend them, so each puff has a lit side,
  a shadowed side and dark crevices at sub-metre scale. Soft (depth-faded) particles. Weakness: sprites
  from a fixed view; no real volume, intersections and the camera inside the cloud are fakes.
* **Froxel / volumetric fog injection** (Frostbite, Hillaire 2015): particles add density to the
  view-frustum fog volume, which is lit by every clustered light with shadows and integrated temporally;
  smoke then occludes and scatters headlights and lamps correctly.
* **Temporal reprojection of low-resolution volumetrics** (Frostbite, HZD): march with a turning jitter
  at reduced resolution, reproject the last frame's result and clamp it — fine detail and smooth edges at a
  fraction of the steps.
* **FH5 / NFS Heat / Unbound / GT7 / CarX burnouts** (look, not sources): thick white-grey billows with
  crisp curling edges at the wheels, lit tops and darker undersides, coloured by tail lights / neon; the
  cloud builds up and drifts downwind, older smoke thins to a haze rather than a wall; the car stays
  readable through it; at speed the smoke streaks with motion blur.

## 2. Our Cinematic smoke against them (before this work, same views)

At the explore burnout (`--bench explore`, frames 50 / 110 / 150), the race-start burnout (`--shot
qrstart`), the drift event (`--bench racing` drift, frame 500), lamp-lit steam (rain) and the yellow
underglow, Original | Cinematic the smoke integration | Cinematic this work (internal evidence: smoke-nextgen/sbs_*.webp):

| gap | seen as | This page |
|---|---|---|
| **density build-up**: a long burnout became one flat wall over the whole view | explore 110: mean +30, median +31, p5 +8.5, the sky +58 against Original; the car and the yard gone | **fixed in part**: +8.9 mean, p5 +2.9, sky +11 (old smoke thins, less haze) |
| **shape / billow detail**: soft 1–2 m blobs, no curling edges; a finer octave aliased | 100 % crops: smeared fog | **fixed**: a third, curl-warped octave near the eye, integrated by a temporal resolve (no grain) |
| **lighting response**: billows shaded only straight up | flat, ambient-dominated cloud | **better**: the shading turns toward each voxel's dominant lamp / head / tail light |
| **blacks**: the scene ambient took 45 % of the whole view's mean (bright sky and lamps) | smoke in a dark corner as bright as the lamps: milky | **fixed**: 50 % the light round the cloud, 25 % the road, 25 % the mean |
| **edge softness / stability** | a fixed per-pixel jitter pattern | turning jitter + reprojected, variance-clipped history |
| hangs and disperses | old smoke piled up | spread 2.5 → 3.5, haze 0.6 → 0.3 |
| **camera inside the cloud** (driving off through it) | murky brown wall | **open**: unchanged (a 5.5 m near fade was tried and cut the burnout at the wheels: rejected) |
| motion blur at speed | smoke drawn after the blur, sharp | **open** (no smoke at > 30 m/s in the benches; see Motion) |
| shadows toward each lamp, reflections through smoke | — | open (docs/smoke-volumetric.md "Open") |

## 3. What changed (`render::volume_fx`, `render::smoke_volume`)

All new knobs are fields of `volume_fx::Look`, **off** in `volume_fx::LOOK`, `wet_spray::LOOK` and
`water_ng::MIST_LOOK` (spray and mist keep the smoke integration's march; test `g10d_knobs_and_layout`); the smoke turns
them on in `smoke_volume::FX`.

| what | how |
|---|---|
| **fine octave** (`Look::fine` 1.0) | `eroded()`: a third noise octave at 4.6× the second's frequency (~0.35 m billows), warped by both coarser octaves' gradients (curls round the larger billows), 22 % of the erosion noise; faded out where the march's step is longer than 0.4 m / step scale (~20 m from the eye) and to 30 % without a history (a still, the first frame after a cut: one frame's samples would grain) |
| **directional billow shading** (`Look::directional` 0.7) | `cs_light` also stores the direct light's luminance-weighted direction (new half-res `Rgba8Snorm` volume, its length = how one-sided); the march's billow-shading sample turns from straight up toward it by the sample's direct share: tops / sides facing a lamp lit, the far side dark (what six-way maps give a sprite) |
| **temporal resolve** (`Look::temporal` 0.85) | new pass `fs_temporal` (own shader section) after the march, over the same occupied-brick rect: the march's start offset turns by the golden ratio each frame; the last result is reprojected by the camera at the medium's mean distance along the ray (the march's second target, now `Rgba32Float`, b), clamped to this frame's 3 × 3 mean ± 1.25σ (variance clip), weight 0.85 (× 0.6 when the pixel moved > 12 low-res px); none after a cut (eye > 25 m) or a resize. Two ping-pong low-res targets |
| **longer steps** (`Look::step` 1.4) | the march's step × 1.4 (0.28–1.12 m), paid back by the temporal resolve |
| **brick threshold** (all users) | `cs_resolve`: densities below 0.005–0.01/m fade to zero and bricks mark from > 0 (was: kept, but skipped when the brick was empty — marked and unmarked bricks disagreed in thin haze) |
| **scene ambient shares** (smoke only: the only user with `scene` > 0) | 0.35 / 0.2 / 0.45 (round / road / view mean) → 0.5 / 0.25 / 0.25 |
| smoke constants | `smoke_volume::LOOK`: haze 0.6 → 0.3, size 1.15 → 0.95, spread 2.5 → 3.5; `FX`: shade 0.35 → 0.6 (with the directional shading the dark side faces away from the light, not just down) |

`NFSU2_SMOKE3_LOOK` takes `fine, dir, temporal, step` too.

## 4. Look rule (docs/look-direction.md, `tools/look`)

**The look direction audit spots** (street, harbour_neon, coal_docks, jackson_hills, rain_street, qrstart; the original's
cameras, `poses-dxvk.tsv`; `audit-spots.txt`): Original + Particles Cinematic and the max preset (which
has Cinematic) are **identical before and after** (Δ 0.00 for the row; max −3.75 / p5 +0.60 / p95 −8.83
both: The look direction's own values, from other rows). There is no smoke at those spots (no burnout): R1–R6 hold
trivially. The smoke itself is judged in the smoke scenes below.

**Smoke scenes** (1080p, the player's enh settings + the row, against the clone's Original with the native
smoke at the same frame; `smoke-scenes-stats.txt`). No Wine frame of a burnout at the same camera was
taken (see Open); the look direction measured the clone's Original within ±1 of the original's luminance at its spots.

| scene | Δmean before → after | Δp5 | Δp95 | Δsky |
|---|---|---|---|---|
| explore burnout f50 | +5.6 → **+0.3** | +4.5 → +0.9 | +19.9 → +4.4 | 0 → 0 |
| explore f110 | +30.0 → **+8.9** | +8.5 → +2.9 | +5.3 → −1.1 | +57.6 → +11.4 |
| explore f150 (2.5 s burnout) | +19.5 → **+13.0** | +10.5 → +5.6 | −13.8 → −8.8 | +71.4 → +37.6 |
| yellow neon f50 / f110 / f150 | +9.8 / +32.1 / +20.2 → **+2.1 / +10.4 / +13.6** | +3.9 / +8.4 / +10.3 → +1.9 / +3.6 / +5.3 | | |
| rain steam f50 / f110 | −2.5 / +7.3 → −6.0 / **+2.0** | −2.3 / +5.7 → −5.4 / +2.2 | | |
| race start (shot) | +0.1 → −2.0 | −0.2 → −0.2 | +7.7 → +4.4 | 0 |
| drift f500 | +1.5 → +0.4 | | | |

Every smoke scene moved toward Original; the large remaining deltas are a 2.5 s standing burnout whose
cloud covers the sky and the lamps (content, not a lift of the image: saturation and contrast are back
within 0.02 / 2 of Original at f110). Rain f50 is darker (−6; inferred: the steam's shadow darkens the wet road under it, where Original's
sprites only added light; not isolated).

## 5. Motion and temporal stability (look rule E7)

* Racing bench (circuit, 1080p, user settings + Cinematic + Per-Object blur, frames 1100–1103 at 67 mph):
  **pixel-identical** to the smoke integration (no smoke there; the temporal pass writes nothing outside smoke). The
  benches reach no smoke at > 50 m/s; that check is open.
* Drive-off through the burnout (`--bench explore` frames 170–330, `motion_driveoff_*`): no ghost of the
  car or trail at its edges in consecutive frames 250 / 251 at 100 %; the history follows the camera.
* FSR 1 (`enh.upscale 5`, `sbs_fsr_110.webp`) and TAA (the player's): the smoke is drawn after the temporal
  pass at output size (unchanged); no shimmer seen. The grain of an untemporalised fine octave
  (`grain_rejected_v2_100pct.webp`, a still) was the reason for the history gate.

## 6. Cost (GPU busy ms/frame, reference desktop GPU (RADV), 4K, perf.sh Extreme with the row Original / Cinematic, 600 frames, 2 runs; `cost-4k.tsv`, `scripts/cost3.sh`)

| scene | smoke passes the smoke integration | **This page** (build / march / temporal / composite) | frame Δ over Original, the smoke integration → this work |
|---|---|---|---|
| explore, heavy burnout | 0.67 / 0.68 | **0.67 / 0.70** (0.10 / 0.42 / 0.03 / 0.10) | +0.36 / +0.38 → +0.40 / +0.49 |
| racing drift, light | 0.44 / 0.44 | **0.43 / 0.43** (0.06 / 0.25 / 0.03 / 0.05) | +0.43 / +0.44 → +0.34 / +0.29 |

Same cost as the smoke integration (the longer steps pay for the third octave, the light direction and the resolve; the
build is cheaper: fewer thin voxels marked). Frame means are within run-to-run noise (±0.1 ms). Without
the step scale (step 1.0) the march cost 0.53 ms in the burnout.

## 7. Honest self-assessment

Clearly better than the smoke integration: the burnout no longer whites out the frame into a flat, lifted wall; the cloud
has crisp curling edges near the car, lamp-facing lit sides, darker cores, the car and yard stay readable,
and it is stable in motion at the same cost. Against FH5 / Heat / Unbound it is **still not at their
level**: (1) the camera inside a long burnout still sees a murky brown haze (games fade and stylise it);
(2) the fine detail is ~0.35 m and only within ~20 m — no sub-0.1 m wisps at the tyre contact like a
six-way flipbook; (3) a still (the first frame after a cut) has a third of the fine detail; (4) no motion
blur of the smoke at speed; (5) the smoke's lighting is still mostly the screen-space scene ambient — at
night the lower cloud is a dim brown-grey where games show more headlight / tail-light scattering;
(6) shadows still straight down. A six-way flipbook layer from our own offline sim was not built: it
would be a new rendering path (scope freeze) and fights the volume's soft intersections.

## Parity, tests

Original: nothing of this runs (Particles Original / High / Ultra take other paths); spray and mist
keep their march (knobs off) except the brick-threshold fix (thin haze < 0.01/m). Tests:
`volume_fx::tests::g10d_knobs_and_layout`, `gpu_pool_and_render` (GPU, ignored: three temporal frames,
ping-pong, kept view), `smoke_volume::tests::look_overrides` (the smoke's knobs).

## Open

* A Wine frame of a burnout at the same camera (the original driven into a standing burnout) — not taken.
* Camera inside the cloud; smoke motion blur at speed; a > 50 m/s smoke motion check.
* Credits: Karis 2014 (TAA neighbourhood clamp), Salvi 2016 (variance clipping), Hillaire 2015, Schneider
  2015, Jimenez 2014 (IGN); no third-party code.
