# Particles High / Ultra: GPU particles and detailed smoke (clone deviation, opt-in)

Design note "can you make
smoke more detailed"). Nothing here exists in SPEED2.EXE and nothing new was traced: the native
particle system (docs/formats.md "Particle effects": spawn 0x491E50, update 0x492E10, render
0x48BDF0 → 0x5C0E20 / 0x5C7E90) is unchanged and still decides **where, when and how many**
particles spawn. Off (Original) is the default and byte-identical, except for the post-race scene smoke
fix below (a native-path fix, checked against the original).

| row (Graphics → Effects) | values | options.txt |
|---|---|---|
| **Particles** | Original / High / Ultra (/ Cinematic: docs/smoke-cinematic.md) | `enh.particles 0..2` (3) |

`qol::particles()`; label id 0xC104E070 (`options::PARTICLES`, chosen clear of 0x5C.. that other
later graphics work may take). Presets: Ultra → Particles Ultra (cost below); Modern / Original unchanged.

## What changes (`src/render/gpu_particles.rs`, `gpu_particles.wgsl`)
* **Fed by the native emitters.** `particles::System` got one clone-only field, `births`: while
  set (High / Ultra) every particle `emit` spawns is also copied there. `Effects::step` (game.rs)
  sets it per 1/60 s world update and hands the births to `Feeder::record`, which keeps those of
  **smoke** descriptions (`is_smoke`: names with Smk / Smoke in the XETT_ tyre, XEBO_ / XECI_ /
  XECS_ contact and XES_NOSBO_ purge groups) and queues the update (its dt, births, cars). The
  native particle keeps living in the CPU system (same update, same pool of 1000, same
  0x870CEC gate); only its quad is left out of the draw (`System::quads_where`). The event
  markers' smoke (XEE_), the front end's (XEFE_), the flag girl's flare (XECH_), **sparks and
  nitrous flames stay on the native quads, unchanged** (same look at every tier).
* **GPU spawn and update** (compute, `Detail::prepare` at the next draw, replaying each queued
  update in order, ≤ 600 updates / 16384 births per draw): each native birth becomes
  High 4 / Ultra 8 puffs (size × 0.62 / 0.48 ± 25 %, opacity × 0.85 / 0.72) near it, with its
  velocity ± a spread; life × 1.35 / 1.7 (lingering). Update: the description's own drag
  (per axis, −sign(v)·v²·drag) and gravity as 0x492E10 applies them, plus value-noise
  turbulence growing with age, a drift toward a light wind (0.55, 0.35 m/s; **inferred**, the
  game has none), the wake of cars within ~2 m (pulled toward 35 % of the car's velocity), and the
  carried share of the owner car's velocity for carried (+0x1E) descriptions, as native. A ring
  pool (High 16384, Ultra 49152 puffs; the oldest reused).
* **Draw** (instanced billboards, no vertex buffers): the native colour cubic over the stretched
  life and the native size cubic over the native life (growing 35 % more while lingering), the
  native 0x48BDF0 on-screen bound, the native shading `2·tex·v0` with the description's own
  texture (SFX_SMOKE_A/B_CS, one array layer each), turned by a random angle and slow spin;
  **noise detail** (2-octave value noise eroding the puff more as it ages: wisps, natural
  dissipation); **soft particles** (the scene's depth read per pixel, linearised for normal and
  reversed-Z; fade over a quarter of the puff's size, 0.15–0.6 m; no hard edge on the road or the
  cars; also faded in the first 1.5 m from the eye); **lit**: The lamps-and-shadows work's light-pack lamps
  (`volumetric::nearest_lamps`, High 8 / Ultra 16, native weight²), each drawn car's headlights
  (a cone from its front, colour = the car's headlight pool colour; reach 28 m) and tail lights
  (two red points 2.1 m behind, ±0.6 m, 0.85 m up; **inferred**: the cars have no tail-light
  markers), evaluated per vertex with a weighted light direction and wrapped on a sphere-like
  normal per pixel; **thickness**: a dense core is darker and lets less of that light through;
  premultiplied over, the alpha (bleach mask) blended as native's SRCALPHA / INVSRCALPHA.
* **Order / TAA** (docs/temporal.md): drawn right after the native particles: into the scene
  (MSAA or not) without the temporal pass, and with TAA / Upscaling in the late pass after it at
  the output's size with `late_viewport` (no jitter, no history): no ghosting
  (`drift_taa_orig_ultra.webp`). Only the main view; the road reflection and mirror do not show
  the puffs (they do not show native particles either).
* Frames between world updates (uncapped rendering) draw the puffs as of the last update.

## Cost (GPU busy ms/frame, `--bench racing 600`, drift event: tyre smoke throughout; `scripts/bench.sh`, 2 runs, reference desktop GPU (RADV) and the reference 2-CU iGPU)
| | 1080p mean / p99 | 4K mean / p99 | iGPU 1080p mean / p99 |
|---|---|---|---|
| Original | 0.75 / 0.94 | 2.68 / 3.15 | 14.89 / 20.2 |
| High | 0.80 (+0.05) / 1.45 | 2.81 (+0.13) / 5.27 | 17.69 (+2.8) / 51.1 |
| Ultra | 0.83 (+0.08) / 1.78 | 2.91 (+0.23) / 6.57 | 19.53 (+4.6) / 72.7 |

The mean is small; the cost is fill rate when a cloud covers much of the screen (p99 at 4K
+2.1 / +3.4 ms). A first version lit every pixel (all lamps per fragment, 3 noise octaves) and
cost up to +13.8 ms (p99) at 4K; lighting per vertex and 2 octaves brought it here.
On the iGPU it is expensive: +2.8 / +4.6 ms on average and frames of 50–73 ms inside a thick
cloud (the row's help names it). Graphics-row costs: High 0.13, Ultra 0.23 ms at 4K. Ultra preset: Ultra (cheap on a discrete
GPU on average). With a player's real settings (the player's options file, SSAA 2x at 1280 × 960,
`--bench racing 410`, drift): mean 1.72 → 1.80 (High) / 1.84 (Ultra), p95 1.85 → 2.30 / 2.48 ms.

## Evidence (the maintainer's private captures, Original | High | Ultra unless named)
* `burnout_qrstart_orig_high_ultra.webp`: the race start burnout (`--shot qrstart`,
  NFSU2_FRAMES=450 NFSU2_THROTTLE=1 NFSU2_SHOT_STEP=1): the trail behind the car lingers and breaks
  into wisps; under the bumper it fades into the road instead of a hard sprite edge.
* `drift_orig_ultra.webp`, `drift_taa_orig_ultra.webp` (TAA), `drift_user_settings_orig_high_ultra.webp`
  (the player's settings file): `--bench racing`, NFSU2_BENCH_KIND=drift, frame 400.
* `purge_orig_high_ultra.webp`: `NOS_SHOT=1 NOS_PURGE=1 --shot explore` (hood and wheel purge
  plumes, recoloured by the purge part as native).
* `night_lamps_orig_ultra.webp`: the burnout at night (NFSU2_RAIN_CLOCK=93, Dynamic Lamps on).
* `scripts/bench.sh` (costs), `scripts/ident.sh` (byte identity).

## Scene smoke: the post-race burnout (native path, player report)
Player: "burnout in the post-race scene is missing smoke". **Cause** (clone): the race's ending
scene (EndingNis01..04) plays while the post-race menus are up (`Mode::Menu`), and
`Game::particle_effects` returned early for every mode but Explore / Race, so the NIS mover's
sliding wheels (`drive::nis` 0x5AB550: skid above 6 m/s, `docs/nis-mover.md`) never reached the
tyre emitters. The race-start burnout (IntroNis, Car1) was not affected (Race mode; the clone
and the original both smoke there, internal evidence).
* **Native**: the particle update 0x492E10 runs inside the world update 0x609CF0 (call at
  0x60A191, docs/formats.md), which keeps running under the post-race screens (the scene plays
  and holds behind them, observed under Wine). Under Wine (the merged branch's capture, 2026-11-02,
  the maintainer's private capture, 240SX win, EndingNis01 with brake 0.2 /
  additive 2.5 on mode 5): a tyre-smoke cloud behind the car under Event Results.
* **Fix**: `particle_effects` also runs while an Ending scene runs, with the player's car and
  the scene's AI winner (`ShopRun::companion`). Emission is the same tyre path as driving
  (`Effects::step`: sliding wheels, `particles::tyre_group`, amount dt·|slip|; that rule itself
  stays **inferred**, its driver 0x434B00 is open), so rate, size and colour are the
  descriptions' (XETT_RoadSk/Sp_SmkA on the road). Particles High / Ultra apply on top.
* Evidence: `ending_burnout_native_orig_ultra.webp` (the original under Wine | the clone's
  Original | Ultra; `NFSU2_SHOT_CAR=240SX NFSU2_ENDING_NIS=1 NFSU2_SHOT_MS=1000
  NFSU2_SHOT_STEP=1 --shot ending`; the camera frames differ, the clone's results list is its
  own race). Not compared particle by particle (amount open).
* Test: `ending_burnout_emits_tyre_smoke` (game.rs, ignored like the other ending tests: real
  assets, isolated XDG): EndingNis01 under the menus, at most 43 tyre-smoke particles (0
  before the fix: the step never ran).
* `--shot ending` steps the particles with NFSU2_SHOT_STEP; NFSU2_SHOT_CAR picks the shot
  profile's car.

## Native byte-identity
All CLAUDE.md shots at defaults (Original), md5 against the binary before this work (same copy,
own target): explore, intro, qrcars, race, qrstart, drift, drag **identical**; main / career
differ as the base does from itself (the base against itself: 0 / 176 px on main). On Original
`System::births` stays None, `quads` = `quads_where(|_| true)` (same quads, same order), and
`Detail::prepare` is not called (`particle_quads`).

## Tests
`render::gpu_particles::tests`: `emit_counts_unchanged_vs_native` (240 updates with and without
recording: the native particles identical bit for bit, every smoke spawn handed over, sparks
not), `soft_particle_math` (depth linearisation against `Camera::projection_depth`, normal and
reversed; fade and softness), `original_tier_takes_nothing` (tiers; quads_where = quads),
`ring_wraps_at_the_pool`, `smoke_classification_over_globalb` (asset test),
`gpu_spawn_and_update` (GPU, ignored: 10 births → 40 puffs, all dead after 2.5 s); options row
order (Particles after LOD Fade), qol round trip.

## Open
* Exhaust smoke (CARFX_EXHAUST_SMOKE, kind 11) and blow-off (12) are not emitted by the clone
  (their trigger is not traced), so they have nothing to detail.
* Sparks and nitrous flames are not moved to the GPU path (kept on the native quads, which is
  the "same look as native" the plan asks for); rain splashes untouched.
* Wind, wake, tail-light placement and every look constant are by eye (inferred), judged
  against the clone's Original frames; there is no native reference for a deviation.
* Puffs are not sorted (one blend over, similar colours); a lighter puff behind a darker one
  can show through in thick clouds.
* No self-shadowing by a light-space density map (the thickness term only); no volumetric fog
  coupling.
