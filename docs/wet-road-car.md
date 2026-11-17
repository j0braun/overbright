# Wet Road Interaction (clone deviation, opt-in)

The wet road interacts with the car visually. One row in the
Graphics page's **Weather** group, **Off** by default (native, byte-identical):

| row | values | options.txt | label id | presets |
|---|---|---|---|---|
| **Wet Road Interaction** | Off / On / High | `enh.wet_interaction 0..2` | 0xC104E0AC (`options::WET_INTERACTION`) | Ultra On; perf.sh Extreme High |

`qol::wet_interaction()`; it counts as a weather row (`qol::weather_fx()`). The row works on Wet Roads'
wet surface and the spray pool, so it turns both on while it is on (a road has to be wet to interact
with). The Wet Roads, Spray & Splashes and Wet Car rows keep their own meaning.

Nothing here exists in SPEED2.EXE: worldreflect and car.efx have no input for the cars on a wet road
(docs/wet-roads-ssr.md, docs/car-raindrops.md), so the original draws a wet road the same with or
without cars. **Nothing was traced for this work and nothing was observed under Wine** (there is no
native counterpart to capture). Everything below is **inferred / by eye**. It only follows native
values, all read and never written:
- the rain amount (weather +0x1F0), the road's wetness from the rain (`Weather::road_wet`, the +0x509C
  rule from 0 at load, docs/wet-roads-ssr.md) and the world clock 0x7FB718;
- the cars' drawn poses (the renderer's interpolated matrices) and wheel contacts;
- their light markers and the native lighting rule of 0x615640: the left and right headlights in the
  headlight colour (+0x380), the centre headlight never, and the brake lights brighter while the brake
  flag (car +0x944, `CarState::brake_lights`, held 0.5 s for racers) is on, the centre one only then;
- the wheels' slip along and across (`WheelState::slip`, `lateral_slip`) and contact (`on_ground`).

The simulation is untouched. `game.rs` `glow_quads` hands each car's flares, brake flag and wheel values to
`render::wet_road::push_car`, which returns at once with the row off.

## What it draws
**Trail map** (`render/wet_road.rs`, `wet_road.wgsl`): a world-space Rgba16Float map around the
camera, 1024² over 128 m (On, 0.125 m a texel) or 2048² over 160 m (High, 0.078 m). It is a torus:
texel = fract(world xy / size), so the camera moving never copies it. Texels that wrap in from the far
side are cleared in the same pass. Each frame, in its own command buffer submitted before the frame:
1. **Decay**: a full-map multiply by exp(−rate·dt), dt the world clock's advance (a pause holds it).
   - The track refills at 0.12 + 0.36·rain per second (had 0.3 + 0.9·rain): a half-life of
     1.4 s in heavy rain, 5.8 s on a drying road.
   - The wake decays at 1.6 per second.
2. **Stamps**: every drawn car (player, rivals, traffic) stamps each wheel's path since the last frame,
   matched by the predicted position, with Max blending, as nine torus copies so that edges wrap:
   - R: the track, 0.15 m half width, wider with sideways slip;
   - G: the wake's freshness;
   - B: closeness to the track × G (1.4 m half width);
   - A: the wake's height × G, from the speed.

   A wheel in the air does not stamp.

**Road** (`render/wet_road_world.wgsl`, appended to world.wgsl; read only inside Wet Roads' branch of
the main view, `wet_fx.window.w = 1`):
- **Tyre tracks** (values): the track takes up to 92 % of the wet reflection away, roughens it to
  ≥ 0.35 and undoes up to 80 % of a puddle's darkening (the asphalt shows through the water the tyre pushed
  aside; this work darkened the diffuse by 12 % instead). This gives the dry line behind every car, fading over
  seconds; SSR's mask pass runs the same `wet_surface`, so its trace sees the same weight, roughness and
  rippled normal.
- **Wakes and puddle ripples**: rings move out from the track as it ages (0.95 m/s), with their
  direction from the map's closeness gradient. They are strongest in puddles, and the fresh track is
  churned.
- **Puddles**: Wet Roads' procedural field, kept where the texture's authored reflection alpha allows.
  With the row on they dry more slowly than the film (× √level instead of × level), so puddles
  outlast the general sheen after the rain. The puddle mask is not taken from Surface Detail's maps:
  they hold normals, not heights (open).
- **The cars' lights mirrored**: the 16 (On) or 32 (High) nearest head and tail lights, at their drawn
  positions.
  - Each adds an anisotropic lobe: Ward-like, stretched along the view by 2 + 7·grazing², so it forms
    the long wet-road streaks.
  - The lobe takes water's Fresnel and the light's cone (headlights cos 0.25, tails −0.2) with range
    26 m / 12 m.
  - The tail lights also add a small red glow on the road beneath and behind them.
  - Brake lights are 2.6× brighter while braking.
- **The cars in the puddles' mirror** (without SSR): the planar capture has no cars (native, view 4).
  So the 8 nearest cars block the reflection with two boxes each (body and cabin), soft by roughness,
  dimming what the capture shows there to 35 % plus a dim ambient body colour (30 % on the film, 90 % in
  a puddle). With SSR on, its trace reflects the real cars and the boxes are off.
- Debug: `NFSU2_WET_ROAD_DEBUG=1` shows the map on the road (R track, G wake, B closeness) and prints
  the per-frame counts.

**Spray** (`wet_road::emit` into `weather_fx::Spray`; the stronger wet interaction: the volume puffs and the mist through the
volumetric renderer, see "Clearly visible, volumetric" below; the droplets drawn by the weather pass and lit by its lamps
and the cars' head and tail lights). Per wheel on the ground, × road wetness, High × 1.6:
- **Rooster tails**: droplets and a mist plume. The rear wheels throw the most, scaled by
  smoothstep(4, 30 m/s)² plus the slip (wheelspin, sliding). The plume is thrown sideways with the
  lateral slip, so a drift fans it out.
- **Puddle splashes**: CPU puddles at the contact, the shader's field (`puddle_at`). A sheet of
  droplets is thrown to both sides, scaled by the speed.
- **Arch spray**: droplets flung up and back off the top of the tyre.
- **Wake mist**: a large slow cloud behind cars above 18 m/s.
- **Body sheeting**: droplets leave the roof's and boot's rear edge once the car's film (`car_wet`) is
  over 0.15 and the speed over 8 m/s.

A full pool now reuses its slots in turn instead of `Vec::remove(0)` (O(n) per particle).

**Car**: with Wet Car off, the row alone still gives the car Wet Car's **spray film** (the lower body
below the sill and the tyres darker and glossier, streaking back with the speed; `wet_car::spray_only`,
film and rain zeroed). With Wet Car on, Wet Car draws as before.

**Drying**: when the rain stops, the road's wetness falls by the native rate (0.0015 of the gap a
step). The wet look fades from a level of 0.15, the puddles shrink with √level, the track refills more
slowly (2.3 s half-life) but has less to take away, and the spray stops with the wetness.

## Bindings (ported onto <base>, 2026-11-05)
Group 0 of the world views: the lamps 3–8, Surface Detail 9–10, Modern Lighting phase 2 Modern + RT's traced scene 11 (its
layout only, `frame_layout_with(.., Some(rt))`), this row 12–14 in every form of the layout. The plain
main group, the lamps' group and the RT scene group (`frame_group_rt`, `WorldCore::scene_group`) all
carry the row's entries; `rebind_lamps_wet` rebuilds the RT group too
(`wet_road::tests::frame_bindings_do_not_collide`). Modern Lighting phase 2's matrix (every Lighting tier × user / Ultra /
Extreme × explore, race, racing, rain) with this row at High: all 36 runs clean
(internal evidence).

## Native byte-identity and dry identity
- The row off: no trail map, no passes. Bindings 12–14 of the world views' group 0 hold placeholders
  (a zero uniform and a 1×1 map) and the branch is never entered.
- Fast build, 1280 × 960:
  - Original `explore_rain` and Wet Roads + Spray + Wet Car in `explore_rain` are md5-identical to
    main.
  - The row at High in the dry `explore`, `qrstart` and `race` shots is identical to Off.
- Parity gate (`scripts/checks/gate.sh … extreme`, <base>): see the unit report and
  `docs/evidence/wet-road-car/README.md`.

## Cost
`--bench drive 600` at world clock 162 (night, heavy rain 0.70, a pack of cars), GPU busy ms/frame,
median of 3 interleaved runs, reference desktop GPU, a shared machine. Raw numbers:
internal evidence.

| row (over Original) | 1080p | 4K | of which the weather pass (spray) 1080p / 4K |
|---|---|---|---|
| Original frame | 0.85 | 2.75 | – |
| Wet Roads + Spray (for reference) | +0.08 | +0.18 | 0.08 / 0.23 |
| **Wet Road Interaction On** | **+0.23** | **+0.76** | 0.16 / 0.58 |
| **Wet Road Interaction High** | **+0.28** | **+0.82** | 0.20 / 0.75 |

Within the plan's budget (≤ +1 ms at 1080p). At 4K most of the cost is the spray's mist (fill rate,
the weather pass); the road's branch and the map are the rest. Row costs in graphics_table.rs: 0.76 / 0.82
(4K). Dry scenes cost nothing (no map pass, the native branch).

## Clearly visible, volumetric

User after playing: "I don't see much interaction from the car with the rain on the road", then "rain interaction
does look, but out of place, not next gen like the other effects". Same row, same label (0xC104E0AC), same
values (Off / On / High); **no new label id**. Help texts say what it now does.

### 1. Does it run at the play setup? (checked, `--bench drive` at clock 162, the play setup's options + the
plan's rows: wet_interaction 1, wet_roads 1, ssr 1, wet_car 1, rain_detail 2, spray 1, lens_drops 1, Modern
Lighting 3 @ 75 %, SSAA 2x)
- Trail map: running (NFSU2_WET_ROAD_DEBUG=1: every car's wheels stamp it; ~14 cars, ~500 stamps a frame).
- Spray: emitted (2–2.6 k billboard particles at 40–60 mph), but small and faint: what the player saw.
- Light streaks: present. The player's Light Glow (custom.9) is 1, so `glow_quads` runs. **Fixed anyway**: with
  Light Glow off the row's hook (lights, wheels, now the underglow) still runs (`game.rs`, `glow_quads` called
  for the hook alone, its sprites dropped); before, the road mirrored no car lights and the spray took no slip.
- Modern Lighting on: yes (its lamp falloff now also lights the spray volume).
- Tracks: present but they refilled in 0.6 s (half-life) in heavy rain, so behind a car at speed they were
  gone within ~15 m and the chase camera never saw them.

### 2. What changed
**Volumetric spray** (`render/wet_spray.rs`, over the volumetric smoke's `render::volume_fx`; an additive `VolumeFx::set_anchor` /
`grid_origin_at` there, defaults unchanged for the smoke). The CPU spray pool (`weather_fx::Spray`) keeps its
droplets and gets a second pool of **volume puffs** (`CLOUD_MAX` 12 000), emitted by `wet_road::emit` per wheel on
the ground × road wetness (High × 1.6):
- **tyre fan**: 260 / s (rear) · 90 / s (front) × (speed ramp 3→25 m/s + slip) short-lived puffs (0.35–0.6 s,
  0.14 → 0.6 m) torn off the contact patch: a continuous sheet right behind the tyre;
- **rooster tail**: 46 / 16 per s larger puffs (0.8–1.5 s, 0.22 → 1.5 m) on ballistic arcs up and back, fanned
  sideways by the lateral slip (drifts);
- **puddle sheets**: 50 / s × the puddle under the wheel, thrown to both sides;
- **wheel-spin / drift cloud**: 34 / 12 per s × smoothstep(2, 8 m/s slip), low and slow;
- **wake mist**: 16 / s × smoothstep(12, 40 m/s) behind a fast car, filling its slipstream.

The puffs ride **the air the cars drag along** (`volume_fx::wake`, the same wake model that rolls the smoke;
drag 2.2 / s toward it, the still air's drift otherwise, a fall of 3 m/s² fading with age), spread on the road
instead of sinking, and thin as they grow (opacity × 0.5 / r). With the row on, the weather package / this work **mist goes
into the volume too** (no more flat billboards). Each frame they become `volume_fx::Particle`s (water albedo
0.86–0.9, thin = wet, lumpy 0.3 / 0.55, radius capped at 1.5 m for the splat's cost), splatted into the spray's
own grid (On 128 × 128 × 16 voxels of 0.3 m, High 160 × 160 × 24 of 0.25 m, centred 14 m ahead from 3 m under
the eye, world-snapped), **lit** by the 64 nearest lamps (Modern Lighting's physical falloff when on), every
drawn car's headlights as one cone (its pool colour) and its tail / brake lights as points (brake 2.6× brighter,
`push_car`'s colours) and the player's underglow, with the volumetric smoke's Henyey–Greenstein forward scattering (g 0.6) and
multiple scattering and self-shadow, marched soft against the scene's depth at ½ / ⅓ resolution with the
bilateral upsample, composited **before the post chain** (exposure, overbright / bleach grade, HDR output) — in
the same layer as the smoke, and with TAA / TAAU in the late pass with the temporal viewport. Puffs within 1–5 m
of the eye fade (the chase camera's own car throws its spray past the lens, not over its own bodywork).
- **Droplets** (weather pass): streaked along their motion over a 1/45 s exposure, relative to the camera,
  their light spread over the streak (motion blur on fast spray; `weather_fx.wgsl` `vs_spray`, only with the row).
- **Spray on the lens**: `Spray::hit` (the volume puffs within 2.5 m of the eye, leaving out the car nearest
  to the eye: the player's own) adds up to 40 short-lived lens drops a second (`LensSim::splash`, Lens Drops'
  pass); driving in the spray of the car ahead spatters the view.
- **Dry line**: tracks last ~2.5× longer (above) and take more of the reflection; wake ripples ~1.6× stronger.

### 3. Tiers
On: the grid above, 16 Ki volume particles, 96 steps. High (cinematic): finer grid, 24 Ki particles, 128 steps,
× 1.6 the spray, plus this work's finer trail map and 32 mirrored lights.

### Cost
`--bench drive 600` at clock 162 (night, rain 0.70, 3 rivals + traffic), the play setup **without SSAA**
(`enh.post_aa 0`), GPU busy ms/frame, reference desktop GPU (a shared machine; mean / median of 2 runs; raw:
internal evidence, round "final"):

| row | 1080p mean / median | 4K mean / median | the volume's passes (build + march + composite) 1080p / 4K |
|---|---|---|---|
| Off (Wet Roads, Spray, Wet Car, Rain Detail … on) | 2.96 / 2.85 | 9.65 / 9.24 | – |
| **On** | **3.40 / 3.31 (+0.45)** | **10.61 / 10.22 (+0.97)** | 0.38 / 0.68 |
| **High** | **3.74 / 3.59 (+0.78)** | **10.73 / 10.42 (+1.09)** | 0.46 / 0.77 |

Within the plan's ≤ +1 ms at 1080p On. The row's cost hints (graphics_table.rs): 0.97 / 1.09 (4K). What paid for
it: the mist's radius capped at 1.5 m (the splat costs (r / voxel)³: the build went 1.4 → 0.5 ms), a flatter grid
(16 / 24 voxels high instead of 24 / 32, the spray is never higher), and the build moved into the frame's
encoder (its own submit before the frame left an idle gap that the timestamps counted: 0.5–3 ms means with
spikes, now 0.16 ms). The march takes 160 / 200 steps: with the volumetric smoke's default 96 the steps ran out in the long thin
spray and the volume ended in brick-shaped edges. The droplets' weather pass got cheaper (0.14 → 0.03 ms at
1080p: the mist is no longer billboards). Original and the row Off pay nothing (no volume, no pool).

### Quality checks (performance rule)
- 100 % / 200 % crops (`on_0501_crop100/200`, `high_0500_crop100`): soft volume, no block or sprite edges, no
  noise pattern; the High grid is finer near the tyres.
- Motion: four consecutive frames (`on_0501_0510_consecutive_crop100`) and the two animated sequences: the spray
  moves with the cars, no popping or shimmer (world-snapped grid, the volumetric smoke's fixed jitter).
- Two artefacts found and fixed while tuning: brick-shaped edges (step budget, above) and faceted balls right in
  front of the camera (puffs now fade in from 1.5 m to 6 m from the eye); and a too dense early tune hid the car
  ahead entirely (now a veil: the car ahead and its lights read through it, `before_after_0501`).
- SSIM / FLIP against this work are meaningless (a new effect); not used.

### Evidence (the maintainer's private captures, before = this work at c1983de On, after = the stronger wet interaction On; the play setup
at 1080p, `--bench drive` from clock 162)
| file | what |
|---|---|
| before_after_0201 | launch / cornering at 28 mph: wheel-spin cloud lit by the tail lights |
| before_after_0300 | straight, 39 mph, rivals ahead: their rooster tails and mist |
| before_after_0501 | 59 mph drafting a rival, brakes on: the spray veil lit red, the car ahead through it |
| before_after_0798 | 67 mph behind traffic: a headlight cone forward-scattered in the spray |
| before_after_0999 / 1101 | 75 / 67 mph, rivals alongside and ahead |
| off_on_high_0500 | the row Off / On / High (the player's other rows on) |
| on_0501_crop100 / crop200, high_0500_crop100, on_0501_0510_consecutive_crop100 | quality checks |
| sequence_240_330 / 960_1050_before_after.webp | animated, every 3rd frame (motion, braking, traffic) |
| cost.txt | round 1 (an earlier tune, separate submit) and "final" |
| gate.txt | `scripts/checks/gate.sh … <base> extreme`: PARITY all gates passed, md5 of base and new |

Drift: the rain never falls on the drift track (stadium); the slip-driven clouds show at the launch and in the
street corners; their rates are in `wet_road::tests`.

### Tests
`render::wet_spray::tests` (`puffs_fade_and_skip`, `spray_lights_follow_the_cars`, `eye_in_the_spray`),
`weather_fx::tests::interaction_clouds_for_the_volume` (puffs on the road, bounded pool, near fade, the own car's
spray does not hit the lens, the row off makes none), `weather_fx::tests::spray_hits_the_lens`, and the updated
`wet_road::tests::trails_fade_over_seconds` (new rates) and `spray_follows_speed_slip_and_puddles`. The volume
module's GPU test (`volume_fx::tests::gpu_pool_and_render`, ignored: needs a GPU) passes with the anchor change.

### Self-assessment (honest, against FH5 / NFS Heat-Unbound / GT7 in the rain)
Much stronger than this work and now in the same layer as the volumetric smoke: cars at speed trail lit spray clouds,
tail and brake lights glow red inside them, a headlight behind spray blooms, the car ahead is veiled, launches and
slides churn a cloud, the lens is spattered behind traffic, the dry line lasts. At normal viewing it reads like
Heat's night rain more than like a 2004 game. Not yet at the references' level:
(1) the rooster tail's fine structure — FH5 shows streaky sheets and droplet ribbons off each tyre; here the near
tyre spray is a soft 0.25–0.3 m-voxel cloud plus small streaked droplets, so at 200 % it is soft, not detailed;
(2) the spray has no shadow and is not reflected in the wet road (it is composited after the world, as the
smoke); (3) the ambient is a constant (0.32) rather than the scene's (adds a scene ambient pass to
volume_fx; the spray will take it with the merge), and native fog is not applied inside the volume yet (the smoke integration
too); (4) tyre tracks are visible mostly where the road mirrors bright lights — on a dark road the dry line is
subtle; (5) no true motion blur on the volume (its puffs are soft; the droplets are streaked); (6) every
constant is by eye.


## Polish of the wet drive ("driving on wet roads still looks off")

Scope (frozen): the existing rows only — Wet Road Interaction, Spray & Splashes, Wet Car,
Rain Detail; no new row, no new label id. Nothing here exists in SPEED2.EXE and nothing was traced or
observed under Wine for it (there is no native spray, track or splash to capture): **inferred / by eye**,
held to docs/look-direction.md's rules with the maintainer's tools numbers. The reference frame is the clone's
Original path at the same bench frame (it matches the original's luminance; the drive cannot be
replayed under Wine at the same frame).

### Diagnosis (before = the base with the look direction)
Recorded with `--bench drive` (a career sprint, the player on the racer AI, 3 rivals + traffic; the frames
repeat run to run) at 1920×1080 and 3840×2160, the play setup (the play setup's options file,
perf.sh USER) plus the weather rows at their tops (Wet Road Interaction High, Rain Detail High, Wet Car,
Spray, Lens Drops): heavy rain (clock 162: rain 0.70), the rain stopping and the road drying (clock 196: rain
0.7 → 0 by frame ~300, road wetness 1 → 0.32 by frame 1200), chase and bumper cameras, 33–75 mph, launch,
cornering, braking, drafting three cars, oncoming traffic. Per frame: Original, the same with the weather rows
but Wet Road Interaction off ("rows off"), and the row on, so the row's own share is measured.

How the references show it (published gameplay footage, described; no assets): **Forza Horizon 5** (storms) —
each tyre throws a short translucent plume that thins within a car length or two, a low grey veil behind a car
at speed, red where its tail lights shine through, fine droplet streaks; **Gran Turismo 7** (Spa / Nürburgring
rain) — long dense rooster tails at speed that cut the view behind a car, lit red by the rain lights, a dry line
forming over laps; **Driveclub** — volumetric spray clouds lit by the tail lights, drops on the screen; **NFS
Heat / Unbound** — the wet road's neon reflections carry the look, spray is a light mist, rain impacts are tiny
sparkles. Common to all: the spray is **dark unless a light is in it**, translucent, low and short-lived; it
never lifts a dark street to grey.

Ranked by how much it hurt (measured at the frames above):

| # | problem | evidence | status |
|---|---|---|---|
| 1 | **The spray read as white tyre smoke**: a flat ambient of 0.32 (the stronger wet interaction's own open item: the volume ignored the scene's light) lit every puff grey-white in a dark street, and each car's headlight cone (2× gain, 30 m at full strength, forward-scattered) turned the spray in front of it into a white wall that hid the car | row's share at the drafting frame (810, 65 mph): **Δmean +7.7, Δp95 +9.2, Δglow +0.56, ΔRMS +6.0** (R3 broken); +2.4..+2.5 at 33–56 mph | fixed |
| 2 | **Shape: fog banks**, not thrown water: rooster puffs grew to 1.9 m and lived 1.5 s, the weather package's mist balls and the wake mist (up to 3 m) joined them in the volume | crops `before_after_*`: a cloud bank behind each car | fixed (smaller, shorter, lower; still blobs, see the assessment) |
| 3 | **Rain Detail's splashes: "popcorn"**: 8–16 cm crowns, doubled in size at 30 m, as bright domes all over the road (each lit with most of the flat ambient) | `splash_before_after_1110` | fixed |
| 4 | **A damp road threw the shower's spray**: the rates followed the road's wetness alone, so with the rain stopped (road wetness 0.9–0.7) the rooster tails stayed full | `damp_before_after_0510` | fixed |
| 5 | **A lattice in the spray at 4K with supersampling**: the march's start jitter (interleaved gradient noise) is meant for a temporal pass; at 4K + SSAA 2x a march pixel is ~3 output pixels and the pattern showed at 100–200 % (the stronger wet interaction's crops were 1080p) | `crop200_before / _after` | fixed |
| 6 | **Cost**: the main view's mirrored car lights looped over the 32 nearest lights per wet pixel, including cars behind the camera | 0.7 ms of the 4K main view (counts.x = 0 experiment) | fixed (culled) |
| 7 | Tyre tracks show where the road mirrors a bright light; on a dark road the dry line is subtle | — | open (unchanged) |
| 8 | No water sheets peeling off the tyre / droplet ribbons, no spray shadow, spray not in the road's reflection, no beading on the paint beyond Wet Car's film | — | open |

Timing (sound-independent): spray starts with the wheel's contact and speed in the same frame; no lag seen.
Motion (look rule E7): the spray moves with the cars, no popping or shimmer in consecutive frames before or after
(`consec_*_801_804_crop100`).

### What changed
1. **Spray energy anchored to the scene** (`wet_spray.rs` `LOOK`): the ambient is the scene's around the
   medium (the smoke integration's block averages, `scene` 1.1: about the light the road and walls around it give) instead of a
   flat 0.32; the fallback for an unreadable target 0.08. Headlight cones: gain 2.0 → 0.7, reach 30 → 24 m
   (`SPOT_REACH`), cone cos 0.80/0.94 → 0.84/0.95; lamps 1.4 → 1.0. The native fog (FogValue / FogColor)
   now applies inside the spray as world.efx applies it (`view_of(.., fog)`, the smoke integration's path; was `None`).
   Detail noise finer and crisper (detail 0.32 → 0.5 /m, sharp 1 → 1.5, erosion 0.7 → 0.75): wisps, not balls.
2. **Spray shape** (`wet_road::emit`): rooster puffs 0.22 → 0.8–1.9 m over 0.8–1.5 s → 0.16 → 0.5–1.15 m over
   0.55–1.0 s, alpha 0.24 → 0.34 (a denser, shorter sheet that falls back); the tyre fan at half the rate
   with denser puffs (140/50 per s, alpha 0.42; still several overlaps at speed); plume and wake mist smaller
   and fainter (≤ 1.2–1.6 m); the weather package's mist at half rate while the row is on; the largest splatted ball 1.5 → 1.2 m.
3. **Spray follows the standing water** (`wet_road::water_depth`): every rate × 0.3 + 0.7·min(rain / 0.6, 1)
   (`SprayCar::rain`, the native rain amount +0x1F0): full in a shower, a thin mist once the rain has stopped.
   **One wetness**: `water_depth` is gone; the spray (both rows), its emitters and the car's spray film read the one
   wetness's film (`materials::world::wetness`, road_wet × (0.35 + 0.65·min(rain / 0.6, 1)); docs/wet-roads-ssr.md
   "One wetness").
4. **Splashes** (`weather_fx.wgsl` `vs_splash`, Rain Detail On / High; Next-Gen untouched): 5–9 cm crowns
   (× 1 + d/60 instead of × 1 + d/30), 0.2 s instead of 0.3 s, faded from 15 m (was 25 m), lit by the lamps and
   cars only (60 % of the flat ambient taken off), the flash a thin ring on the surface instead of a dome.
5. **No lattice** (`volume_fx`, additive: `VolumeFx::set_step_scale`, `set_jitter`, `march_tuning`; defaults
   leave the uniform's look4.y/z at 0, so the smoke and the fountain mist march exactly as before): the spray
   marches half-length steps with a quarter of the jitter (`STEP_SCALE` 0.5, `JITTER` 0.25); High's step cap
   200 → 170 (the spray is lower now).
6. **Cheaper mirrored lights** (`wet_road::gather_lights`, `ViewCull`): only lights whose mirror image or glow
   can be on screen (in front of the eye, within the screen's width + 1–3 m) go to the shader; and in the loop
   a pixel off a streak's line (lobe < e^-12) skips the rest (`wet_road_world.wgsl`; the tail glow is kept).

### Look rules (docs/look-direction.md, the maintainer's tools; internal evidence, `look-spots.txt`)
Driving frames (9: heavy chase 230/510/810/1110, bumper 510/810, damp 230/510/810), Δ against the clone's
Original at the same frame:

| | Δmean | Δp5 | Δp95 | ΔRMS | Δglow | Δsat | Δsky |
|---|---|---|---|---|---|---|---|
| the row alone (on − off), before | +2.51 (max **+7.62**) | +0.16 | +3.05 (max +9.2) | +1.29 | +0.07 (max +0.56) | 0.00 | 0.00 |
| the row alone, **after** | **+0.96** (max +2.51) | +0.06 | +0.85 | +0.35 | 0.00 | 0.00 | 0.00 |
| user + weather rows, before | +3.57 | +0.69 | **+4.44** | +1.61 | +0.07 | +0.01 | +1.07 |
| user + weather rows, **after** | **+1.84** | +0.59 | +1.84 | +0.52 | 0.00 | +0.01 | +1.07 |

R1 (row ±4 in rain; preset +3/−4), R2 (p5 ≤ +1 / +1.5), R3 (p95 ±4, glow ±0.2), R4, R5 and R6 now hold at every
frame; before, the drafting frame broke R1 and R3 and the preset's p95 was over budget. Crushed % moves by
the weather rows (−0.64 mean, the drying frame's 5.0 → 0.9 % is Rain Detail / Lens Drops, rows off already),
not by this row (−0.01). The look direction spots (7: street, harbour neon, coal docks, Jackson Heights, rain street, rain
harbour, race start; standing cars, so no spray): the row and Ultra / max are unchanged by this work (Δ ≤ 0.04);
Rain Detail High's splashes p95 +1.37 → +1.27 at the rain spots.

### Cost (internal evidence)
`perf.sh bench` A/B, rain (drive at clock 162), 3840 × 2160, "max" (the play setup + every newest row),
3 reps × 600 frames interleaved, GPU busy median; a shared machine (load 15–170), noise ≈ ±0.3 ms:

| row (max − max with the row off) | before | after |
|---|---|---|
| Wet Road Interaction High | 2.11 | **1.71** (−0.40) |
| Wet Road Interaction On | 1.96 | **1.75** (−0.21) |

Where it went: the mirrored-light cull took the main view from +0.86 to +0.35 ms over the row off (the loop alone
was 0.70 ms: the experiment with no lights); the lattice fix costs back ~0.17 ms of march (half steps). The splat
proved cheap (the build's timestamp interval holds the drain of the passes before it: halving the puffs or
capping their radius moved nothing; skipping the whole volume saves 0.3–0.75 ms). The spray's march already runs
at ¼ of the supersampled resolution at 4K (`march_div` 4), so "half-resolution mist" was not available as a
saving; a coarser march is what showed the lattice. Not done (would look cheaper or is too small): fewer mirrored
lights, the trail map's decay every other frame (0.15 ms pass). Cost hints unchanged (0.97 / 1.09 in
graphics_table.rs, hints-4k.tsv 1.13 / 1.35: within the hint test's tolerance of the new numbers).

### Self-assessment (honest, against FH5 / GT7 / Driveclub / NFS Heat)
Better: the spray no longer reads as tyre smoke or a white wall; it is a translucent, dark veil that shows where
lights hit it (tail lights red, lamps, oncoming headlights as a glare spot), hugs the road and thins within a car
length or two, as FH5's does; a dry road after the rain throws little; the road is no longer covered in white
splash domes; the car ahead stays readable while drafting; NFSU2's dark street and neon keep their level (the
row adds +1 on average where it added +2.5 to +7.6). Not at the references' level: (1) the spray is still soft
volume blobs with fine droplets — no droplet ribbons or water sheets peeling off the tread (FH5, GT7); (2) at
56 mph it is subtler than GT7's rooster tails (chosen: the look rules and a readable car ahead over a thick
plume); (3) no spray shadow, not reflected in the road, no beading or running water on the paint beyond Wet Car's
film; (4) tyre tracks are still mostly visible only where the road mirrors a bright light; (5) every constant is
by eye. Verdict: **keep** (Ultra On, Extreme High as before).

### Tests
`wet_road::tests::spray_follows_speed_slip_and_puddles` (+ the damp road: a third of the shower's spray,
`water_depth`), `wet_road::tests::lights_follow_the_drawn_car` (+ the view cull: a car behind the eye or far to
the side mirrors nothing), `wet_spray::tests::look_is_anchored_to_the_scene`, `volume_fx::tests::
march_tuning_defaults_are_inert` (the smoke and the fountain mist march as before),
`weather_fx::tests::interaction_clouds_for_the_volume` (in the rain).

### Open
- Droplet ribbons / sheets off the tread, spray shadow and reflection, paint beading (item 8 above).
- The row alone at the two rain spots, standing: p95 +4.4 (the mirrored tail-light streaks, unchanged since this work;
  just over the neon pass's ±4 for one row; driving frames are within it).
- `NFSU2_SPRAY_LOOK=k=v,…` (tuning aid) and `NFSU2_SPRAY_STEP` / `NFSU2_SPRAY_JITTER` override the constants.


## The glow shell, the washed road and the distance veil at max settings

Issue (screenshot internal evidence, 4K, SSAA 2x, every Next-Gen
tier, rain, the finished-career save's Skyline with a yellow underglow at 217 km/h in the chase camera): a cyan glow shell
around the car's silhouette (a bluish smoke that should not be there), the road around and ahead of the car
washed out, and lights that should reach further into the distance looked like a transparent black
texture that disappeared when coming close. Nothing here is native code: **inferred / by eye**, held to
docs/look-direction.md with the maintainer's tools numbers. Base: <base> (main + the maintainer's `shadow::neon_rgb`
fix of the spray's underglow colour, B,G,R bytes read as R,G,B: yellow lit cyan).

**Reproduction** (measuring aids, not native, no effect without their variables): `--bench explore` takes
`NFSU2_SHOT_SAVE` (a native save's profile and car), and with `NFSU2_BENCH_AT` + `NFSU2_FX_SPEED=km/h`
+ `NFSU2_FX_GEAR=n` the placed car rolls at that speed in that gear with the gas held (the explore shot takes
`NFSU2_FX_GEAR` too). The scene: the plaza street (−307.8, −345.7, 23.0, 90°; the screenshot's CAFE / HOTEL
signs), clock 167 (rain 0.4), 217 km/h in 4th, the overlay's rows (`scripts/checks/halo.sh` holds them).
A single `--shot` frame shows no spray (the pool starts with the first drawn frame), so the bench is used.

### Causes, ranked (bisected one row at a time at frame 120, 3840 × 2160)
1. **The spray volume grew into the car's body** (Wet Road Interaction, `weather_fx::Spray::volume_particles`):
   at speed a rooster-tail puff is 0.5–1.15 m in radius, born 0.35–0.6 m behind the rear contact patch, i.e.
   inside the rear overhang, and thrown up to ~1.5 m. The depth test cut every ball where it lay inside or
   behind the body, leaving its outside as a rim that hugs the spoiler, the flanks and the rear window: the
   shell. With the row off the shell was gone; every other row left it (`bis/` grids in the work dir).
2. **The underglow lit that volume strongly** (`wet_spray::scene_lights`): two point lights 0.15 m up, reach
   3.2 m, gain ×4: the whole plume around the body glowed in the underglow's colour (cyan with a colour bug
   since fixed, yellow after it).
3. **The road wash** around the car: The Next-Gen car-lights work's underglow area light on the road (`car_lights::LOOK.neon_road`
   0.45, reach 1.8 m) added on top of the native underglow pool on a wet road: p95 +13.8, glow +0.64 against
   the same frame without it (R3: ±4 / ±0.2). The rest of the white road ahead is the **native** look in the
   rain: the clone's Original path is as white at that frame (p95 214 vs 222 at max), and the original under
   Wine washes the rain scene out the same way (`$OB_PRIVATE/research/rain-compare/orig-rain-*`; no Wine
   capture at this exact spot: **open**). Headlight Beams On, wet roads, SSR / RT, Rain Detail, Enhanced
   Reflections, Lens Drops, Wet Car and motion blur moved p95 by ≤ 1.
4. **The distance veil** (Volumetric Fog Next-Gen, `atmosphere::LOOK.veil` 0.2): the haze's extinction, a
   fifth of it, still darkened the scene behind it with distance, while the medium's light that pays it back
   comes from the lamps marched near the eye only. Drive towards the lamps on the rain highway (787, −1523):
   the far road was 0.89 of the Original's (0.97 with the row off) and the far lamp heads −10 %, lifting to
   1.12 near the car: a dark veil over the distance that lifts as one drives in. Dry, the same street showed
   no veil (the haze is thinner). Modern Lighting, Dynamic Lamps, lamp shadows, probes, World Detail Extreme,
   LOD fade, Ambient Detail, SSAO were measured and do not darken the distance. Part of the dimmer far lamps
   is SSAA itself (a sub-pixel lamp head is averaged over its samples: the Original + SSAA 2x shows the same
   −50 % peak on one far lamp head); that is correct coverage, not changed.

### Fixes
1. `weather_fx::Body` / `body_clearance`: each drawn car's body is a box from its wheels (axles ± 1.05 m,
   track + 0.22 m, from 0.32 m over the road to 1.5 m); a puff's radius is capped by its distance to the
   nearest box (min 0.08 m) and its opacity fades out within 0.25 m of it (none inside). Under the sills the
   spray stays; it flies behind and below the car, not around it.
2. `wet_spray`: the underglow's spray lights at 0.05 m, reach 1.6 m, gain 1.2 (`NEON_*`): the low spray beside
   and behind the car catches it, the plume above does not.
3. `car_lights::LOOK`: `neon_road` 0.45 → 0.15, `neon_reach` 1.8 → 1.5 m (p95 +3.7, glow +0.2 over none).
4. `atmosphere::LOOK.veil` 0.2 → 0: the native fog alone attenuates the scene (rule E1 to the letter); the
   medium only adds its local light.

### Look numbers
Screenshot view (4K, the maintainer's tool `lstats.py`), Original | before | after: frame 30 mean 63.5 | 65.6 | 66.8,
p95 216.2 | 227.2 | 217.2, glow 2.06 | 2.74 | 2.32; frame 120 mean 46.7 | 53.4 | 54.0, p95 156.7 | 183.2 |
172.7, glow 0.68 | 0.77 | 0.66, satB 0.20 | 0.39 | 0.33. Body box (halo.py): mean |diff| 6.5 → 1.6.
Drive towards the lamps (rain highway, 1080p, far road / near road vs Original): before 0.89–0.95 / 1.12,
after 1.02–1.03 / 1.23 (the near excess is the other rows at max: 1.16 with Volumetric Fog off).
The look direction spots (7: street, harbour neon, coal docks, Jackson Heights, rain street, rain harbour, race start; the
original's cameras), Δ against Original, before → after: Volumetric Fog Next-Gen mean +0.66 → **+2.06**
(rain street +3.9; R1 ±2.5 / rain ±4), p5 +0.60 → +0.68, p95 +0.41 → +2.69, sky −0.17 → +1.34 (R6 ±2); the
max preset mean −4.80 → **−3.36** (R1 +3 / −4: back inside), p95 −12.65 → −9.95, RMS −5.78 → −5.02. Headlight
Beams Next-Gen and Wet Road Interaction High unchanged at the standing spots (no underglow lit, no speed).

### Regression check
`scripts/checks/halo.sh OUTDIR [GAME]`: two 1080p bench runs of the scene (with the rows / with Wet Road
Interaction off and Headlight Beams On), `halo.py` compares the car's upper body: mean |diff| ≤ 3.5 and no
blue in the light the rows add there. <base>: 6.49 / −6.43 → FAIL; 1.61 / −1.46 → PASS. Unit tests:
`weather_fx::tests::spray_keeps_clear_of_the_body`, `wet_spray::tests::underglow_lights_only_the_low_spray`,
`car_lights::tests::underglow_on_the_road_stays_in_budget`, `atmosphere::tests::look_keeps_the_native_fog_and_glow`
(veil 0).

### Open
- A Wine capture of the original at the plaza in the rain (the white road's native share is taken from the
  clone's Original path, which the look direction measured against Wine in the dry, and the existing rain captures).
- The blue-tint half of halo.py was checked by its arithmetic (cyan added light gives a positive excess),
  not against a frame of the old bug at this scene.
- The max preset at the rain highway is still +6..8 mean over the Original (rows other than these).

## Rain splash report ("splash effect of rain sometimes is overlayed on objects")
- At the play setup (the play setup's options file) the weather package rows are off: Rain
  Detail's splashes are not drawn at all.
- The oval "splashes" on the median barrier are the **native lens drops** (rain_drop.efx, 0x5C7AC0,
  `render/rain.rs`): screen-space drops that refract the frame copy. They appear identically with every
  enhancement off (`evidence/wet-road-car/splash-report-*.png`). That is native by trace; not compared
  under Wine in this work (open).
- The weather package splashes were hardened anyway (`weather_fx.wgsl` `splash_ground`). A splash is not placed:
  - at a depth edge: the point and its neighbours on both sides must lie on one plane;
  - on or under a car's footprint: the nearest 8 cars, from their head and tail points;
  - under a cover: nothing may be drawn in front of the points 3 m and 7 m above it, so bridges,
    tunnels, overhangs and trees are excluded.

## Tests
- `render::wet_road::tests`:
  - `trails_fade_over_seconds`: the rates, the half-lives and a paused clock;
  - `puddles_grow_with_the_wetness`: the CPU field, none when dry;
  - `spray_follows_speed_slip_and_puddles`: none on a dry road or standing still; more with speed, slip,
    puddles and film; a wheel in the air throws nothing;
  - `lights_follow_the_drawn_car`: the lights use the drawn pose, nearest first, capped by the tier;
    the boxes;
  - `world_module_carries_the_branch`: both world modules hold the bindings, and the uniform size
    matches its WGSL.
- `render::wet_car::tests::spray_only_keeps_the_road_water`.
- `qol::tests::wet_interaction_round_trips_and_native_default`: parse, write, deviations, Ultra and
  Original; `enhancements_round_trip` (64 deviations).
- `frontend::options` row order with the new row and unique label ids.

## Open
- The spray volume: no shadow, not in the road's reflection, constant ambient and no native fog inside
  it until the smoke integration's volume_fx changes are merged; no motion blur of the volume itself.
- The trail map's own submit still shows idle-gap spikes in the GPU timings (`graphics: wet road map`,
  this work's; the frame is not slower, the measurement is); moving it into the frame's encoder is the fix.
- **Inferred, by eye**: every constant (rates, widths, lobes, spray rates). There is nothing in the
  original to compare against.
- The puddles are procedural; Surface Detail's maps carry no height to pool water in.
- The car boxes are a stand-in when SSR is off: two boxes, one dim colour, not the car's paint.
- (fixed) The lights no longer depend on the Light Glow switch.
- Other cars' cover zones are not tracked (as in the weather package).
- The trail map's pass has no timestamp of its own (its own command buffer). Its cost is in the frame
  totals above.
