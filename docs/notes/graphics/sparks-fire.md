# Sparks & Fire: native flame triggers, and the Next-Gen tier: how it works

The technical part of [Sparks & Fire: native flame triggers, and the Next-Gen tier](../../playing/settings/sparks-fire.md): how it works, what it costs, how it was checked and what is open. The player part (the rows and what they do) is on that page.

## Overview

Two parts:

1. **Native** (default, every tier): the turbo's **flame pops** — car effect kind 10 (`CARFX_NITRO`,
   the nitrous flame's own emitters at the exhaust markers) fired at a shift and when revving in
   neutral, with no nitrous shot. The clone never fired them; ported (`src/carfx_pops.rs`). This is
   the remaining native cause found for the player's nitrous flames sometimes missing.
2. **Next-Gen** (opt-in row **Sparks & Fire**, clone deviation): the native spark and flame births
   drawn as simulated, emissive, light-casting sparks and flames with heat haze
   (`src/render/sparks_fire.rs`, `sparks_fire.wgsl`). Emit rules unchanged.

## 1. Native triggers (traced)

### The car effect kinds and their callers (**verified**)
* The kind → name table is **0x8027C0** (8 bytes an entry, `g_CarFxKindNames`): 0 CARFX_NONE,
  1 DRIVE_OVER, 2 DRIVE_OVER2, 3 SKID_SMOKE, 4 TIRE_SPEW, 5 BOTTOM_OUT, 6 DAM_RADIATOR, 7 DAM_ENGINE,
  8 BLOWN_TIRE, 9 DRIVE_ON_FLAT_TIRE, **10 NITRO, 11 EXHAUST_SMOKE, 12 EXHAUST_BLOWOFF, 13 NOS_BLOWOFF**,
  14 NOS_BLOWOFF_TIRES, 15.. CARFX_BREAK_* (side mirrors … windows).
* 0x61FB10 (CarRender_FireEffect) has exactly four callers (REA `procedure_callers`, and every
  `call 0x61fb10` in the disassembly): 0x4395F0 (the scene event dispatch, kinds from the event
  records: 10–13), **0x5EBE10** (kind 10), 0x5F5740 (the car update: constant kinds 3, 4, 1, 2, 5,
  6, 10 at 0x5F5F5A and **0x5F603D**, 13 at 0x5F5FF9 and **0x5F60C0**, 9) and **0x633F00** (the
  part damage table).
* **0x633F8F** (`CarDamage_ApplyParts` 0x633F00, REA decompile): for each changed 2-bit damage
  state of the 15 rows of **0x803714** (0x24 bytes a row) it fires the row's +0 kind at
  0x803914[state] (0.1 / 0.4 / 1.0). The rows' kinds are 0x0F..0x1D (the CARFX_BREAK_* parts) and 6
  (Radiator) — **never 11 or 12**.
* So **kinds 11 (EXHAUST_SMOKE) and 12 (EXHAUST_BLOWOFF) are fired only by scene event records**
  (CAR_TRIGGER_EFFECT, internal notes). There is no driving trigger to
  port: a native "backfire on a shift" is not 11/12 but the kind-10 pop below.

### Shift pop: 0x5EBE10 (**verified**, REA decompile + disassembly 0x5EBE10..0x5EBF5C)
Called every car frame by Car_Frame 0x6012C0 (0x6012EB) with the gear (car +0x434):
* gear ≠ the last (+0x4D0): store it; sound context (car +0x968) slot +0x48 event 1; armed when the
  car is on the physics mover (car +0x28 = 1) or when (0x865188 − +0x9D8) × 0.00025 (0x784268) >
  0.5 (0x7A5620); +0x9D8 = now.
* on the physics mover with the gear above 2 (0 reverse, 1 neutral, 2 first): armed = **the clutch
  locking**: +0x448 (the transmission's state 3, docs/formats.md "Gear colour") set and +0x4D4 (its
  value at the last call) clear. +0x4D4 = +0x448 on every return.
* L = 0x5995F0(4) on ride +0x2F0 (the turbo's rounded upgrade level), + 0.5 (0x784260) when ride
  +0x328 is set; armed and L ≥ 2.0 (0x7844C0): 0x61FB10(10, L × 0.2857143 (0x7A5D3C), gear, 0),
  then sound slot +0x48 event 9.
* The sound context's slot +0x48 is 0x459550, which acts on commands 7 and 8 only: events 1 and 9
  make no sound.

### Rev pop in neutral: 0x5F5F5F..0x5F608D (**verified**, disassembly)
In the car update 0x5F5740, after the shot's flame / purge: gear (+0x434) = 1, L as above ≥ 2; r =
car +0x440 (0x7FBED8 is a 100 = "off" override), R = engine +0x1F8 − +0x1F0 (car +0x20 = record
+0x110: record +0x308 max − +0x300 idle). Not armed (+0x924 = 0): arm when r ≤ 0.5 R (0x7A5638).
Armed: r ≥ 0.75 R (0x7A5634) fires 0x61FB10(10, L × 2/7, gear, 0), sound event 9, disarms.

### Pre-green purge: 0x5F608D..0x5F60C0 (**verified**, not ported)
While a shot runs (0x59FA40) and the race status (0x890118 → +0) is 0..3 (before the green; free
roam is 4), 0x61FB10(13, 1.0) — the purge kind — fires as well. The player cannot start a shot
before the green in the clone; **open** whether the original lets one run there.

### Under Wine (**observed**, a private Wine copy, :215, fresh career, D-Day free roam, Rachel's 350Z)
* ride +0x2F0 floats: cat 4 = 1.98 → 0x5995F0(4) = 2; ride +0x328 = 0 → L = 2 (intensity 0.571).
  The clone gives her car turbo level 2 (`car::tests::rachels_turbo_level_fires_the_pops`).
* Standing: gear 2 (first), +0x924 = 1. Driving off on the keys: gear 2 → 3 with the clutch open,
  locked 0.07–0.1 s later; the kind-10 emitter's random state (+0x24) and velocity (+0x80) changed on
  the sample where the clutch locked in gear 3, and at no other time (no nitrous fitted, so only a
  pop drives that emitter). internal evidence.
* 60 fps captures did not catch the pop's flame (≈5 particles a marker, 0.2 s, under the bumper).

### Clone (`src/carfx_pops.rs`, `Effects::step` in game.rs)
`Pops::shift` / `Pops::rev` per car; `CarIn::of`: the physics mover = a drivetrain and no MeloMvr;
gear = drivetrain / MeloMvr gear; locked = `Shift::Locked`; r = `shown_rpm` (+0x440); L =
`CarType::turbo_level` (`performance::state_levels(...)[4]`, set by `parse_car_type_tuned` from the
fitted packages). The pops burst through `Effects::flame_burst` (the same 0x615CD0 path as the shot
flame) with the car effects, after the particle update.
**Inferred / open**: ride +0x328 (0 on every car read; it changes only the intensity, never
whether a pop fires); a car's last gear before its first frame (the clone takes the current gear: no
pop on load); Car_Frame's order against the particle update. **Observed difference** (not this
unit): the clone's car stands in neutral (gear 1) where the original's stood in first (gear 2), so
the clone pops once when pulling away (N → 1st is a change) where the original did not.

### Why flames were "sometimes missing"
The original shows a flame at the exhaust at every completed shift (and on neutral revs) of a car
with turbo level ≥ 2 — Rachel's 350Z included — with or without nitrous. The clone showed flames
only during a shot, so in the player's car flames appeared "sometimes" relative to the original. The
earlier causes (high speed: emit order; start line: scene records) are fixed
(internal notes). Not confirmed with the player.

## 2. Next-Gen tier (clone deviation, opt-in)

| row (Graphics → Effects) | values | options.txt | label |
|---|---|---|---|
| **Sparks & Fire** | Original / Next-Gen | `enh.sparks_fire 0..1` | 0xC104E0D8 (`options::SPARKS_FIRE`, checked unused in the main checkout and the parallel copies) |

Not in any preset (Modern / Ultra / Extreme unchanged). Original: nothing records, nothing draws,
the native quads as before.

* **Fed by the native emitters.** `particles::System::births` (the GPU particles work's hook) is also set while the
  row is on; `Effects::step` hands each world update's births to `sparks_fire::Sim::record`.
  Descriptions by name (`kind_of`): `*Sprks*` = sparks (XECS_SolidWall_SprksA, XEBO_ROAD_SprksA,
  XECI_Car_SprksA, the flag girl's), `*NOS_Flame*` = flame (XES_NOS_FlameA, the only CARFX_NITRO
  description, so the shot flame, the pops and the start-line flames alike). The native particles
  live on unchanged; only their quads are left out (`SparksFire::replaces`).
* **Sparks** (as tuned by the sparks tuning, below): each spark birth → 0.25…2 fragments by its contact's push, thrown
  with most of its velocity plus a small scatter off the surface, gravity 9.81, air drag, a bounce off the
  road under the contact, cooling on a black-body ramp (white → yellow → orange → red); drawn as thin
  anti-aliased **motion streaks**, emissive into the scene (HDR: up to 1.6× so the glow chain blooms them),
  soft against the depth. The contact's native glow and flash are drawn as a soft hot spot (`Kind::Glow`).
* **Flames**: the native flame particles themselves (life, motion, owner, interpolation unchanged):
  a procedural fire per particle — a teardrop along its motion, licked by fbm noise scrolling down
  the axis, a white-blue core at the exhaust, blue body, orange tips; seen end-on a round burst;
  each emitter owner's flames share the brightness of about six (a shot's steady stream does not clip to
  flat white; a pop keeps its own).
* **Heat haze**: per flame a refraction quad: the scene (resolved from MSAA, or copied) offset by
  the gradient of drifting noise, strongest past the tip, fading over the flame's life. Skipped
  where the target can be neither resolved nor copied.
* **Light**: per owner the flames' weighted centre (blue-white, reach 2.5–4 m), and round each contact
  (the glows and the youngest fragments, 2 m cells, the 4 strongest, reach 1.2–2.2 m): a deferred
  splat over each light's screen bound — position from the depth (eye-relative, so the inverse keeps its
  precision), normal from the nearer neighbours, wrapped Lambert × (1 − d²/r²)², multiplied by the
  surface's own colour (road, walls, the car's bumper). Works with or without Dynamic Lamps / Modern
  Lighting (not part of the clusters).
* **Order / TAA**: drawn right after the smoke tiers, into the scene before the post chain; with TAA
  in the late pass with the temporal viewport (no jitter, no history), as the GPU particles work.

### Tuning (development build 20261105-2142: too many sparks, the contact texture looked off)

**What was wrong** (reproduced at the play setup and with the row on: `work/scr`, `work/before` of the unit):
* every native spark birth threw 6 fragments with a scatter of 1.5–5 m/s **times up to 3 at speed**
  (`0.6 + 0.08·speed`) plus up to 2.5 m/s of lift, living up to 1.6 s (XEBO_ROAD_SprksA's 2 s): a
  130 km/h scrape kept ~780 fragments alive in a fountain several metres high; a 60 km/h brush along a
  wall (slide 20 m/s, |J| 3) showered the screen;
* the sparks' lights (3 m cells, reach 2.5–5.5 m, × up to 2.2 on the surface, 16 of them) washed the wall
  and road in flat orange: contact region +12…+27 luma, full frame up to +23 mean / +80 p95 (table below);
* the **contact texture**: the native `XECS_SolidWall_GlowA` (HEADLIGHTGLOW) and `XEBO_ROAD_FlashA`
  sprites stayed as native camera-facing quads; sitting on the wall they are cut by it along hard vertical
  lines (the original shows the same stripes under Wine, `nx5b_native_wine_*.webp`), now under the new
  flood light; and the light splat's normals from screen derivatives lit a fringe of pixels along the
  car's silhouette.

**The original's sparks (Wine, DXVK, a private Wine copy, a race on the Coal Harbor wall, 30 fps frames,
`nx5b_native_wine_scrape.webp` / `_hit.webp`; observed)**: a compact, white-yellow glow trail at the contact
(the GlowA sprites, 1.2 m, inheriting 0.5 of the car's velocity), small spark dots within ~1 m, staying at
the contact's height, a little smoke; no light on the surroundings, no spray above the car.
**Reference behaviour of current racers** (from published footage, described, no assets): FH5 / NFS Heat /
Unbound / GT7 show a short, very bright hot spot at the contact and a low jet of thin, short-lived streaks
trailing behind it (they keep most of the car's motion and drop behind as they slow), a small local light,
and hardly anything on a light touch.

**What changed** (`src/render/sparks_fire.rs`, `sparks_fire.wgsl`; Original untouched):
* **Count by the contact**: each burst now carries its push (`particles::Burst` `slide`, `impulse`,
  `normal`; clone-only fields, the native burst ignores them; the wall normal is turned toward the car).
  `intensity(slide, |J|)` = smoothstep(3, 30 m/s slide) × (0.4 + 0.6 · min(|J| / 12, 1)), at least
  0.6 · smoothstep(0.5, 1, |J|/12) for a hard hit; fragments per native birth 0.25…2.0 (stochastic
  rounding), × up to 3 for one hard push (`hit_burst`, |J| 15…40), at most 40 new a world update and 2048
  alive. Births already scale with the native share; a light touch below the native thresholds makes
  none (as native), a brush a few.
* **Motion**: 0.45…0.95 of the birth's velocity (most of the car's: the jet trails behind the contact as
  it slows), a scatter along the surface 0.6…2.4 m/s (× 0.5…1 by intensity, half vertical), 0.3…1.5 m/s off
  the surface, 0.4…1.8 m/s up; drag 0.06/m; life 0.2…0.6 s of the description × 0.5…1.2; bounce on the
  road with the old restitution / friction plus a little turn from the rough road, −20 % heat a bounce.
* **Look**: streaks thinner (half width 3.5…7 mm), a 1/60 s shutter capped at 0.4 m, hotter for longer
  (heat × (1 − x²)), the per-pixel energy floor raised (fade ≥ 0.45) so short thin streaks read bright.
* **Contact glow** (new `Kind::Glow`, pass 4 `fs_glow`): the GlowA / FlashA particles (life, motion, native
  alpha × brightness cubic unchanged) drawn as a soft hot spot: a capsule along the slide (≤ 0.35 m), core
  radius 0.3 × the native size, white-yellow cooling to orange with age, a faint halo, **soft against the
  depth over 4 radii** (no cut line on the wall), a flicker. The native quads of these two descriptions
  are left out (`SparksFire::replaces`).
* **Light**: only round the contact — the glows and fragments younger than 0.15 s in 2 m cells, the 4
  strongest, reach 1.2…2.2 m, at most × 1.5 at the centre (`SPARK_LIGHT` 0.5, warm white); the normal
  from the nearer neighbour in x and y, and depth-edge pixels take none (no silhouette fringe).
* Flames, haze and the flame lights: unchanged.

**Measured** (1280×960, 26 frames of 7 scenarios at the play setup + the row, vs Original; the look direction
`lstats.py` full frame, HUD masked; `nx5b-look-table.txt`; contact region 640×480 `nx5b-contact-energy.txt`):

| | Δmean | Δp5 | Δp95 | ΔRMS | Δsat | Δglow |
|---|---|---|---|---|---|---|
| before, mean (max) | +9.35 (+22.7) | +0.37 (+2.1) | +27.8 (+79.6) | +11.0 (+30.2) | +0.01 | +1.34 (+6.06) |
| after, mean (max) | **+0.28** (+3.1¹) | +0.02 (+0.9) | +0.04 (+2.4) | +0.18 (+3.2) | −0.00 | −0.00 (−0.10) |

¹ bottom_2: the turbo pop's flame and its light at the exhaust (flame part, unchanged), not the sparks.
Contact region, scrape at 130 km/h: Original 60–66, before 66–93, after 62–67 mean luma; car-to-car
100 km/h: after −0.4…−2 vs Original (thin streaks instead of the native's round blobs). At the look direction spots
(street, harbour neon, coal docks, Jackson Heights, rain street, Quick Race start), Original + the row and
the play setup + the row are **byte-identical** to without it (nothing records without a contact):
R1–R6 Δ = 0. The row stays out of every preset.

Scenarios (shot aids, not native): `NFSU2_FX_SPEED=km/h[,down m/s]` (the placed car rolling / falling),
`NFSU2_FX_STEER_FOR=n` (release the steer after n frames: a scrape), `NFSU2_FX_POST=surface,slide,|J|[,from,to]`
(a contact posted at the right flank each frame: car-to-car 28, a brush), `NFSU2_FX_CONTACTS=1` (log each
burst's push and intensity), `NFSU2_FX_DUMP=1` (fragments relative to the car). Highway wall
`NFSU2_SHOT_AT=787.2,-1523.0,22.75,-162.6 NFSU2_FX_SPEED=120 NFSU2_FX_STEER=d NFSU2_FX_STEER_AT=10
NFSU2_FX_STEER_FOR=8 NFSU2_FX=1 NFSU2_FX_FRAMES=72..80` (130 km/h scrape); `…STEER_AT=0` (no FOR: a 116 km/h
hit); `NFSU2_FX_SPEED=60 NFSU2_FX_POST=20,20,3,20,40` (brush); `=100 …POST=28,15,12,20,40` (car-to-car);
`NFSU2_FX_SPEED=120,20` (bottom-out: the road push posts the ROAD closing group, XECS_SOLID_WALL); dusk:
`NFSU2_RAIN_CLOCK=40`; the kerb scrape at 55 km/h as this work.

**Self-assessment (honest)**: the sparks now read as the original's compact contact glow done with modern
means — a soft hot spot with a low jet of thin streaks and a small light — at the original's energy, and
no longer as a fountain. Against FH5 / Heat the streaks are right in count, life and motion but sub-pixel
thin at 1280×960 and partly hidden behind the car in the chase view (they trail low behind the contact, as
real ones do); the hot spot is a soft emissive capsule, not a simulated grinding contact (no decal or
scorch left on the wall, no sparks bouncing off the wall). **Open**: car-to-car and bottom-out were
compared clone to clone only (no Wine capture of those); the native XEBO_ROAD impulse group (|J| ≥ 10 on
the floor) was not reached by the bottom-out aid; wall collisions of fragments; the per-frame extrapolation
between world updates (as this work).

### Not done / open
* **Brake glow** (opt-in visual): not done — there is no brake-disc geometry hot enough to read as a
  disc glow without inventing one; left for the Next-Gen car-lights work / a later unit.
* Uncapped frames between world updates draw the fragments as of the last update (the flames move
  with their owner's blend, as native); a per-frame extrapolation is open.
* The lights do not cast shadows; the haze is screen-space (nothing outside the view refracts).
* Rain: no hiss/steam where sparks land on a wet road; the sparks are not reflected by the road.
* Self-assessment against FH5 / NFS Heat (honest): the sparks (thrown, bouncing, cooling streaks
  that light the road) are close to current racing games at the clone's resolution; the flames are
  a clear step up from the sprite (shape, colour gradient, glow on the bumper and road, haze) but
  stay a billboard procedural fire — not a simulated volume like FH5's backfire, and the haze is
  subtle. Below the "next-gen" bar for fire; at it for sparks.

## Evidence (the maintainer's private captures)
* `native-shift-transitions.txt`: the Wine memory log (gear / clutch / emitter).
* `scrape_orig_nextgen.webp`, `pop_orig_nextgen.webp`, `nitrous_orig_nextgen.webp`: the clone,
  Original | Next-Gen, 100 % crops (play setup + the row, NFSU2_RAIN_CLOCK=93).
* `scrape_motion_nextgen.webp`: four consecutive updates of the scrape.
* `nx5b_scrape130_orig_before_after.webp` (crops, Original | This page | The sparks tuning), `nx5b_scrape130_motion.webp`
  (five frames 1/15 s apart, rows Original / this work / the sparks tuning), `nx5b_{car100,brush60,kerb55,dusk_scrape}_*_orig_before_after.webp`
  (full frames, 40 %), `nx5b_native_wine_scrape.webp` / `_hit.webp` (the original under Wine), `nx5b-look-table.txt`,
  `nx5b-contact-energy.txt`.
* Commands: `NFSU2_SHOT_AT=1695.2,-883.6,24.1,120 NFSU2_FX=1 NFSU2_FX_FRAMES=144 --shot explore`
  (scrape along the kerb at ~55 km/h); `NFSU2_FX_TURBO=3` and `NFSU2_FX_FRAMES=118` (the pop at the
  1st → 2nd shift); `NOS_SHOT=1 --shot explore` (nitrous); `NFSU2_SHOT_ENH="…;enh.sparks_fire 1"`.
  `NFSU2_FX*` are new shot aids (not native); `NFSU2_SF_PASSES=<mask>` (tuning aid) picks the passes.

## Cost (reference desktop GPU (RADV), `--bench racing 600` at 3840×2160, `NFSU2_BENCH_RIVAL_NOS=8` (rivals' shot
flames at the green), fast-build binary, 2 runs each; `NFSU2_BENCH_JSON` per-pass timestamps)
| | GPU busy mean / p95 (run 1, run 2) | the tier's pass (frames it ran) |
|---|---|---|
| Original | 2.67 / 3.66, 2.49 / 2.81 | — |
| Next-Gen | 2.61 / 3.51, 2.57 / 3.50 | **0.108 / 0.099 ms** (54 of 600 frames: lights, haze resolve + quads, flames) |

Frame means differ within run-to-run noise; the pass itself is ~0.1 ms at 4K with two cars' flames
(budget ≤ 0.5 ms typical). The crash / long-scrape case (≤ 4096 fragments, 16 lights) was **not
benched** (no bench drives into a wall): the row's hint says 0.2 ms; its CPU side (the fragment
step and instance build) is within the `particles` span (≤ 0.02 ms/frame here). Not measured on an
iGPU.

## Tests
`carfx_pops::tests` (5: the lock edge in 2nd, level 2 gate, into first / reverse, other movers' 0.5 s,
neutral rev arm / fire), `car::tests::rachels_turbo_level_fires_the_pops` (asset),
`render::sparks_fire::tests` (7: kinds, fall / bounce / cool, smoke and flames not thrown, flames
and lights follow the native particles, ramps; the sparks tuning `light_touches_make_no_shower`,
`contact_glow_and_its_light`), qol round trip (`enh.sparks_fire`), options row order.
