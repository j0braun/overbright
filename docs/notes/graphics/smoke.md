# Smoke: how the Particles tiers work

The technical part of the [smoke settings page](../../playing/settings/smoke.md): the GPU particles of
Particles High and Ultra, the Cinematic tier's simulation and its volumetric drawing, and the polish toward
current games. Each section below was a page of its own and keeps its measurements; where a later section
changes an earlier one, the later one says so.

## Particles High and Ultra: GPU particles and detailed smoke

More detailed smoke. Nothing here exists in SPEED2.EXE and nothing new was traced: the native
particle system (docs/formats.md "Particle effects": spawn 0x491E50, update 0x492E10, render
0x48BDF0 → 0x5C0E20 / 0x5C7E90) is unchanged and still decides **where, when and how many**
particles spawn. Off (Original) is the default and byte-identical, except for the post-race scene smoke
fix below (a native-path fix, checked against the original).

| row (Graphics → Effects) | values | options.txt |
|---|---|---|
| **Particles** | Original / High / Ultra (/ Cinematic: "Particles Cinematic: the simulation and the billboard drawing" (below)) | `enh.particles 0..2` (3) |

`qol::particles()`; label id 0xC104E070 (`options::PARTICLES`, chosen clear of 0x5C.. that other
later graphics work may take). Presets: Ultra → Particles Ultra (cost below); Modern / Original unchanged.

### What changes (`crates/ob-render/src/render/gpu_particles.rs`, `gpu_particles.wgsl`)
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
* **Order / TAA** (docs/notes/graphics/temporal.md): drawn right after the native particles: into the scene
  (MSAA or not) without the temporal pass, and with TAA / Upscaling in the late pass after it at
  the output's size with `late_viewport` (no jitter, no history): no ghosting
  (`drift_taa_orig_ultra.webp`). Only the main view; the road reflection and mirror do not show
  the puffs (they do not show native particles either).
* Frames between world updates (uncapped rendering) draw the puffs as of the last update.

### Cost (GPU busy ms/frame, `--bench racing 600`, drift event: tyre smoke throughout; `scripts/bench.sh`, 2 runs, reference desktop GPU (RADV) and the reference 2-CU iGPU)
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
GPU on average). With the play setup (the play setup's options file, SSAA 2x at 1280 × 960,
`--bench racing 410`, drift): mean 1.72 → 1.80 (High) / 1.84 (Ultra), p95 1.85 → 2.30 / 2.48 ms.

### Evidence (the maintainer's private captures, Original | High | Ultra unless named)
* `burnout_qrstart_orig_high_ultra.webp`: the race start burnout (`--shot qrstart`,
  NFSU2_FRAMES=450 NFSU2_THROTTLE=1 NFSU2_SHOT_STEP=1): the trail behind the car lingers and breaks
  into wisps; under the bumper it fades into the road instead of a hard sprite edge.
* `drift_orig_ultra.webp`, `drift_taa_orig_ultra.webp` (TAA), `drift_user_settings_orig_high_ultra.webp`
  (the play setup's options file): `--bench racing`, NFSU2_BENCH_KIND=drift, frame 400.
* `purge_orig_high_ultra.webp`: `NOS_SHOT=1 NOS_PURGE=1 --shot explore` (hood and wheel purge
  plumes, recoloured by the purge part as native).
* `night_lamps_orig_ultra.webp`: the burnout at night (NFSU2_RAIN_CLOCK=93, Dynamic Lamps on).
* `scripts/bench.sh` (costs), `scripts/ident.sh` (byte identity).

### Scene smoke: the post-race burnout (native path)
Issue: the burnout in the post-race scene had no smoke. **Cause** (clone): the race's ending
scene (EndingNis01..04) plays while the post-race menus are up (`Mode::Menu`), and
`Game::particle_effects` returned early for every mode but Explore / Race, so the NIS mover's
sliding wheels (`drive::nis` 0x5AB550: skid above 6 m/s, `docs/original/nis-mover.md`) never reached the
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

### Native byte-identity
All the standard shots at defaults (Original), md5 against the binary before this work (same copy,
own target): explore, intro, qrcars, race, qrstart, drift, drag **identical**; main / career
differ as the base does from itself (the base against itself: 0 / 176 px on main). On Original
`System::births` stays None, `quads` = `quads_where(|_| true)` (same quads, same order), and
`Detail::prepare` is not called (`particle_quads`).

### Tests
`render::gpu_particles::tests`: `emit_counts_unchanged_vs_native` (240 updates with and without
recording: the native particles identical bit for bit, every smoke spawn handed over, sparks
not), `soft_particle_math` (depth linearisation against `Camera::projection_depth`, normal and
reversed; fade and softness), `original_tier_takes_nothing` (tiers; quads_where = quads),
`ring_wraps_at_the_pool`, `smoke_classification_over_globalb` (asset test),
`gpu_spawn_and_update` (GPU, ignored: 10 births → 40 puffs, all dead after 2.5 s); options row
order (Particles after LOD Fade), qol round trip.

### Open
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

## Particles Cinematic: the simulation and the billboard drawing

> **Superseded look (2026-11-05):** the Cinematic tier now renders the puffs described here as
> **volumetric smoke** ("Volumetric effects and the volumetric Cinematic smoke" (below): `render::volume_fx` + `render::smoke_volume`). The
> simulation below (births → billows + haze, noise flow, wakes) still drives it; the billboard / OIT drawing
> below is kept only for comparisons (`NFSU2_SMOKE2_G10=1`). Steam changed: it starts from the current rain
> (it started dry), lives shorter and evaporates. The cost table below is this work's.

Goal: better-looking smoke than the native look. Nothing here exists in
SPEED2.EXE and nothing new was traced: the native particle system (docs/formats.md "Particle effects":
spawn 0x491E50, update 0x492E10, render 0x48BDF0) is unchanged and still decides **where, when and how
many** smoke particles spawn. Original stays the default and byte-identical (see "Parity").

| row (Graphics → Effects) | values | options.txt |
|---|---|---|
| **Particles** | Original / High / Ultra / **Cinematic** | `enh.particles 0..3` |

The existing row (label 0xC104E070, `options::PARTICLES`) got a fourth tier; tier names are plain
strings, so **no new label id** (checked against the main checkout: it has no tier 3 and no
`smoke_cinematic`). Presets unchanged (Ultra preset → Ultra); Cinematic is manual. Help text names
what it does and that it is cheaper than Ultra in thick smoke. `qol::PARTICLE_STEPS` = 0..3;
`enh.particles` clamps at 3; the describe line says "particles cinematic".

### Code (new files; High / Ultra's path untouched)
`crates/ob-render/src/render/smoke_cinematic.rs`, `crates/ob-render/src/render/smoke_cinematic.wgsl`. It reuses only
`gpu_particles::Feeder` (the native births, recorded per 1/60 s world update) and a few constants;
`gpu_particles.rs` / `.wgsl` are not edited (the second performance audit works on their cost). game.rs: an `Effects::cinematic`
field, `Effects::step` records into it when the row is 3, `particle_quads` prepares it (and drops High /
Ultra's GPU half), and its `draw` / `warm` sit next to `Detail`'s at the three draw sites (main view,
late TAA/TAAU pass with its viewport, `--shot`). As High / Ultra, the native quads of smoke
descriptions are left out of the draw; sparks, nitrous flames, markers and the flag girl's flare stay
native.

### What it does
* **Billows and haze** (`cs_spawn`): each native smoke birth becomes 3 billows (0.8 × the native size,
  1.9 × the life) and one **haze** puff (1.3 × size, 3.2 × life, faint): the long-lived haze that drifts
  and fades. Fewer, larger puffs than Ultra's 8: the look comes from shading, not count (less overdraw).
* **Motion** (`cs_update`): the description's own drag and gravity (as 0x492E10); a **divergence-free
  noise flow** (the cross product of two gradients of the noise volume: curls and rolls without sinks);
  buoyancy while young; older smoke follows a light wind (`gpu_particles::WIND`); **car wakes**
  (`wake`): behind a moving car a widening tube dragged at 35 % of its speed, rolling up and out
  (trailing vortices), decaying with distance; parked cars keep Ultra's local pull. Carried
  descriptions follow their owner as native. Expansion: size × (1 + 0.22 √age), opacity / √growth.
* **Wet roads**: with rain (`Weather::amount`, smoothed) new smoke is **steam**: 40 % thinner, 35 %
  shorter-lived, rising faster, whiter, evenly lit. (Tyre spray is the Wet Road Interaction work's.)
* **Shape and light** (`fs_puff`): a lumpy ball, the radial profile displaced by a tiling 3-D
  Perlin–Worley noise volume (64³ RGBA8, value + gradient per fetch, rank-equalised; two octaves, the
  second domain-warped) whose slice rolls with age and turns against the puff's spin: billows that boil
  and roll, breaking up as they age. Opacity: the native colour cubic's alpha × the native sprite's mean
  opacity × 2.5 × 1.5 (same smoke mass as the sprite, a little denser), so the description still sets
  how much smoke there is. Albedo: the native `2·tex·colour`. Normal: a sphere bent by the density
  gradient. **Self-shadow**: one density fetch a step toward the light. Ambient from above (dark
  underside, darker dense core). Lights: the 16 nearest street lamps (light-pack falloff, or with Modern
  Lighting `NFSU2_MODERN_LIGHTING` its physical windowed inverse-square, `lamp_falloff`), the cars'
  headlights (forward-scattering phase) and tail lights (as Ultra), per vertex with a weighted light
  direction.
* **Reduced resolution + OIT** (`draw`): (1) the scene depth to a nearest view depth at 1/2 (≤1440p) or
  1/3 (4K) resolution (`lowres_div`, 4 loads a pixel); (2) the puffs into a **weighted blended OIT** pair
  (RGBA16F weighted sum, R16F revealage; McGuire & Bavoil 2013 eq. 10): no sorting, no popping, soft
  against the low-res depth; (3) **composite** over the scene with a **bilateral upsample** (bilinear ×
  depth weights `bilateral`, 4 taps): no halos at car or pole edges. Pixels whose four revealage texels
  are clear are rejected with one filtered fetch (most of the screen). Composite blend as High / Ultra
  (premultiplied colour, the bleach-mask alpha as SRCALPHA / INVSRCALPHA). MSAA (sample 0 of the depth),
  SSAA, reversed-Z, TAA and TAAU (late pass, viewport scaled to the low-res buffer) all checked.
* Diagnostics: `NFSU2_SMOKE2_DIV=1..4` forces the divisor; `NFSU2_SMOKE2_DEBUG=1` shows depths and
  coverage, `=2` flat puffs.

All look constants are by eye (**inferred**; no native reference exists for a deviation).

### Cost (GPU busy ms/frame, reference desktop GPU (RADV), `--bench … 600`, perf.sh's Extreme preset with the row at Original / Ultra / Cinematic, mean of 2 runs; `scripts/run.sh`; the smoke column is the smoke draw pass alone)

| scene | size | Original mean / p95 | Ultra mean / p95 (smoke) | **Cinematic** mean / p95 (smoke) |
|---|---|---|---|---|
| explore (a burnout) | 1080p | 3.13 / 3.51 | 5.88 / 22.68 (2.79) | **3.55 / 5.33 (0.50)** |
| explore (a burnout) | 4K | 11.98 / 13.50 | 22.64 / 88.50 (10.96) | **12.27 / 13.79 (0.69)** |
| racing, drift event | 1080p | 3.93 / 4.33 | 4.05 / 4.44 (0.10) | **4.06 / 4.45 (0.12)** |
| racing, drift event | 4K | 15.08 / 16.62 | 15.30 / 16.82 (0.30) | **15.29 / 16.87 (0.29)** |

In a thick cloud the smoke pass is **5.6× (1080p) / 16× (4K) cheaper** than Ultra and the p95 spikes are
gone (88.5 → 13.8 ms at 4K); in light drift smoke it costs the same as Ultra. The row's cost hint:
+0.21 ms at 4K (drift, like High / Ultra's). The steps: reduced resolution and fewer puffs (the
fill); the revealage early-out in the composite took the light-smoke 4K pass from 0.60 to 0.29 ms;
the downsample reads 4 depth texels a pixel. Nothing of Ultra's path changed.

Quality vs Ultra (performance rule): this tier is not an optimisation of Ultra but a different look;
it is not bit-comparable. Checks done: crops at 100 / 200 % (`burnout_racestart_200pct.webp`: no halo
at the bumper / wheels, no blockiness), a 16-frame motion sequence (`drift_motion_ultra_cinematic.webp`:
no shimmer or popping thanks to OIT; billows roll). SSIM/FLIP against Ultra are meaningless for a
different look and were not used.

### Evidence (the maintainer's private captures, Original | Ultra | Cinematic unless named)
* `burnout_racestart_orig_ultra_cinematic.webp`, `burnout_racestart_200pct.webp`: `--shot qrstart`
  NFSU2_FRAMES=300 NFSU2_THROTTLE=1 NFSU2_SHOT_STEP=1 (the launch; 1280 × 960).
* `drift_orig_ultra_cinematic.webp`: `--bench racing` NFSU2_BENCH_KIND=drift, frame 400, 1080p.
* `drift_motion_ultra_cinematic.webp`: Ultra | Cinematic, frames 360–420 every 4th (animated).
* `postrace_burnout_orig_ultra_cinematic.webp`: `--shot ending` NFSU2_SHOT_CAR=240SX
  NFSU2_ENDING_NIS=1 NFSU2_SHOT_MS=2500 NFSU2_SHOT_STEP=1 (under Event Results).
* `explore_burnout_orig_ultra_cinematic.webp`: `--bench explore` Extreme 1080p frame 150.
* `rain_burnout_orig_ultra_cinematic.webp`: the same with NFSU2_RAIN_CLOCK=130 (rain 0.75 there;
  162 is dry at that spot): steam.
* `scripts/`: the shot / bench / crop helpers used.

### Parity
Original: `Effects::step` records nothing (`births` None), `particle_quads` takes the native path,
`Cinematic::prepare` is not called; the Extreme presets keep `enh.particles 2`. `scripts/checks/gate.sh … <base> extreme`
(2026-11-05): base and new binaries (differ);
images, 144 Hz window path, enhanced (Modern / Ultra / Extreme / Extreme FSR), 28 shots (0 differ),
audio and five simulation hashes: **PARITY: all gates passed**.

### Tests
`render::smoke_cinematic::tests`: `tier_routing`, `reduced_resolution`, `bilateral_prefers_matching_depth`,
`wake_trails_behind_the_car`, `lamp_falloff_windows`, `noise_volume_tiles_and_has_gradients`,
`gpu_spawn_update_and_draw` (GPU, ignored: births → billows + one haze each, a draw through all three
passes, all dead after 9 s); qol `enh.particles` clamp now 3.

### Credits (algorithms, no code vendored)
* Weighted blended order-independent transparency: M. McGuire, L. Bavoil, "Weighted Blended
  Order-Independent Transparency", JCGT 2(2), 2013.
* Off-screen low-resolution particles with depth-aware upsampling: I. Cantlay, "High-Speed, Off-Screen
  Particles", GPU Gems 3 ch. 23, 2007.
* Improved gradient noise: K. Perlin, "Improving Noise", SIGGRAPH 2002; cellular noise: S. Worley,
  SIGGRAPH 1996; Perlin–Worley remap for cloud density: A. Schneider, "The Real-time Volumetric
  Cloudscapes of Horizon Zero Dawn", SIGGRAPH 2015.
* Divergence-free noise as ∇a × ∇b: I. DeWolf, "Divergence-Free Noise", 2005 (cf. Bridson et al.,
  "Curl-Noise for Procedural Fluid Flow", SIGGRAPH 2007).

### Open
* Exhaust smoke / blow-off (CARFX kinds 11 / 12) are still not emitted by the clone: no constant
  0x61FB10(11|12) call: the car update's calls (0x5F5999 … 0x5F616B, and 0x5EBEF9) push kinds 1–6, 9,
  10, 13 only; the variable-kind
  callers are the scene effects (0x4396xx) and 0x633F8F (a 2-bit state table at 0x803714, not
  resolved). Without a native trigger nothing is invented, so there are no exhaust/backfire puffs to
  restyle (inferred from the call sites; open).
* Rain streaks are drawn before the smoke (as with High / Ultra), so a cloud hides the streaks behind
  it but also those in front of it.
* No light-space shadow of the cloud onto the road or the car; no coupling with Volumetric Fog.

## Volumetric effects and the volumetric Cinematic smoke

**2026-11-06**: the fine octave, directional billow shading, the temporal resolve, longer steps
and the smoke's density constants are in "Tyre smoke toward current games" (below) (same cost; the constants below are the smoke integration's).

Particles **Cinematic** (the Cinematic smoke work's billboards, "Particles Cinematic: the simulation and the billboard drawing" (above)) looked
cheap and dated in play. The "next-gen" bar (simulated, volumetric, lit by the scene) now
applies to every enhanced effect, and this system is to be reused by spray, fountain mist,
rain, sparks / nitrous fire and atmosphere.

Nothing here exists in SPEED2.EXE and nothing new was traced. The native particle system (docs/formats.md
"Particle effects": spawn 0x491E50, update 0x492E10, render 0x48BDF0) still decides **where, when and how
many** smoke particles spawn. Original stays the default and byte-identical (see "Parity"). All look
constants are by eye (**inferred**).

| row (Graphics → Effects) | values | options.txt |
|---|---|---|
| **Particles** | Original / High / Ultra / **Cinematic** (now volumetric) | `enh.particles 0..3` |

The Cinematic tier keeps its row, value and label (0xC104E070); its help text now says what it does. **No
new label id.** the Cinematic smoke work's billboard look stays reachable for comparisons only (`NFSU2_SMOKE2_G10=1`).

### Code

| file | what |
|---|---|
| `crates/ob-render/src/render/volume_fx.rs`, `volume_fx.wgsl` | **the reusable module**: renderer (`VolumeFx`) and GPU particle pool (`ParticlePool`); nothing smoke-specific |
| `crates/ob-render/src/render/volume_fx_ambient.wgsl` | The smoke integration: the scene ambient (block averages of the target) |
| `crates/ob-render/src/render/smoke_volume.rs`, `smoke_volume.wgsl` | the smoke-specific layer: Cinematic puffs → volume particles, the smoke's lights and look |
| `crates/ob-render/src/render/smoke_cinematic.rs` / `.wgsl` | The Cinematic smoke work's tier: its simulation (native births → puffs) unchanged except steam; draws through `SmokeVolume` |
| `src/bin/game.rs` | hooks: the player's underglow colour (`shadow::neon_rgb`, the smoke integration) and the frame's fog (`FogState::drawn`, the smoke integration) into the smoke's `View` |
| `src/fog.rs`, `src/shadow.rs`, `crates/ob-render/src/render/postfx.rs` | The smoke integration: `FogState::drawn`; `neon_rgb`; the multisampled scene readable with Cinematic |
| `src/frontend/graphics_table.rs` | the tier's help text and cost |

### The reusable module (`render::volume_fx`)

#### API

```rust
let mut fx = VolumeFx::new(gpu, BUDGET_HIGH, LOOK);         // grid, particle capacity, steps; shading
// producers fill fx.particles() (GPU) or fx.write_particles(gpu, offset, &[Particle]) (CPU)
let mut pool = ParticlePool::new(gpu, 8192);                // optional generic simulation
pool.step(dt, emits /* Vec<Emit> */, Fields { wind, turbulence, drag, buoyancy, cooling, glow, cars, .. });
let n = pool.export(gpu, encoder, &fx, offset);             // runs the queued steps, writes particles
fx.frame(gpu, View { eye, forward, view_proj, near, far, reversed, time }, &SceneLights { lamps, spots, points, modern });
fx.build(gpu, encoder, offset + n);                         // splat, resolve, light (once a frame)
fx.warm(gpu, format, samples, depth_samples);               // pipelines before first use
fx.set_anchor(ahead, below);                                // optional: the grid's centre ahead of / under the eye (default AHEAD 22, BELOW 6)
fx.draw(gpu, encoder, target, format, samples, depth, depth_samples, temporal_viewport); // march + composite
```

* **`Particle`** (80 B): position, radius; albedo, peak extinction (1/m; `sigma_for_opacity(α, r)` turns an
  opacity into it); emitted radiance (fire, embers) and lumpiness 0..1 (billowy vs. soft); a noise offset and
  spin (its shape rolls with it); age fraction (old breaks up), thinness 0..1 (wet / steam: wispier, whiter).
* **`Emit`** (64 B) for the pool: position, life, velocity, start radius, albedo, start extinction,
  **temperature** 0..1 (lift, black-body glow, cools at `Fields::cooling`), growth (m/√s), lumpiness, thinness.
* **`Fields`**: wind, turbulence (divergence-free: ∇a × ∇b of the noise volume), drag, buoyancy per unit
  temperature, cooling, glow gain, and up to 8 cars (position, velocity) whose **wakes** drag and roll the
  air (`wake`, a widening tube behind a moving car with two trailing vortices).
* **`SceneLights`**: lamps (the light packs' `(1 − d²/r²)²` or, with Modern Lighting, its physical windowed
  pool), spot lights (headlights: cone, reach), point lights (tail / brake lights, underglow, fire). The
  nearest 64 / 8 / 32 are used.
* **`Budget`** (grid voxels and size, particle capacity, march steps; `BUDGET_HIGH` = 160 × 160 × 40 voxels
  of 0.4 m, 64 Ki particles, 160 steps) and **`Look`** (ambient, phase g, erosion, detail frequency,
  thin whiteness, billow shade, edge contrast, light gains) are the knobs.

#### What a frame does

1. **Splat** (`cs_splat`, one workgroup a particle): a lumpy ball (a smooth radial profile displaced by a
   tiling Perlin–Worley noise volume in the particle's own spinning frame) into a **world-aligned voxel grid**
   around the view (centred 22 m ahead, from 6 m under the eye, snapped to whole voxels: it never swims),
   with fixed-point atomics: density, thinness, age, albedo, emission. A radius under a voxel is widened with
   the mass kept. NaN-safe (a broken particle is skipped, never spans the grid).
2. **Resolve** (`cs_resolve`): density / albedo / emission volumes, only over this frame's and the last
   frame's particle bounds (the rest holds zeros); marks the occupied **8³ bricks**; `cs_bounds` keeps the
   occupied bricks' box. Faded at the grid's sides and top.
3. **Light** (`cs_light`, a half-resolution light volume, empty bricks skipped): for every light its falloff,
   the **self-shadow** (4 density samples toward it), and a **multiple-scattering phase**: single scattering
   with Henyey–Greenstein toward the eye plus two octaves with 0.35× extinction, 0.55× weight, half the
   asymmetry (Wrenninge 2013; Hillaire 2016) — thick smoke stays bright and soft inside, looking into a lamp
   through smoke gives a glowing halo. Plus the sky / city ambient occluded by the medium above and beside,
   × albedo, + emission.
4. **March** (`fs_march`, reduced resolution: ½ up to 1440p, ⅓ at 4K; a quad over the occupied box only):
   from the eye or the grid's entry to the scene's depth (soft against every surface: the march stops at it)
   or the exit, empty bricks skipped, steps 0.2–0.8 m with an interleaved-gradient start offset (a fixed
   pattern: stable, no shimmer), energy-conserving integration (Hillaire 2015). The density is **eroded** by
   two octaves of detail noise drifting with the wind and rising (thin parts hardest: round billows at the
   edge, an opaque core; old parts more: they break up), with an edge contrast for crisp outlines, and each
   sample is **shaded per billow** (the eroded density 0.6 m up: lit tops, darker undersides). The same pass
   writes the block's nearest view depth (second target) and flags the 16 × 16 tiles that hold smoke.
5. **Composite** (`fs_composite`, instanced over the flagged tiles and their neighbours, no overlap):
   depth-aware (bilateral) upsample, premultiplied over the scene; alpha written as the native alpha blend
   writes it. Into the scene **before** the post chain (exposure, grade, HDR output apply; native fog does
   not, see Open); with TAA / TAAU in the late pass with the temporal viewport (no ghosting).

Diagnostics: `NFSU2_VFX_DEBUG=1` (red coverage, green peak density, blue steps), `NFSU2_VFX_DIV=1..4`.

### The smoke (`render::smoke_volume`)

* `cs_smoke_particles`: each live Cinematic puff → a `Particle`: The Cinematic smoke work's size (the native size cubic over the
  native life, √age growth) × 1.15, the Cinematic smoke work's opacity (the native colour cubic's alpha × the sprite's mass, fades)
  × 3 for billows / × 0.6 for the haze puffs, as an optical depth through the centre; the native
  `2·tex·colour` as albedo, 60 % desaturated and lifted to a smoke grey (0.78; the volume takes the lights'
  colours itself); billows lumpy, haze soft.
* **Steam** (wet road, `Weather::amount`): 60 % thinner and **evaporating** (density × e^(−1.2·age)), whiter,
  shorter-lived (the Cinematic smoke work's sim: life × (1 − 0.6 wet), haze × (1 − 0.5 wet)). Fixed: The Cinematic smoke work's wetness smoothing
  started dry, so a load's queued ticks (10 s of a burnout in the bench) all spawned as dry smoke — it now
  starts from the current rain.
* **Lights** (`scene_lights`): the 16 nearest lamps (as the Cinematic smoke work), per car within 80 m a headlight cone and two
  tail lights (× 0.8), and the **underglow**: two lights under the sills in the neon's pulsing colour × alpha
  (× 4), from the player car (`game.rs`, `nfsu2::shadow::neon_color`, only with a drawn `_NEON` texture).
* `NFSU2_SMOKE3_LOOK=k=v,…` overrides the constants by name (tuning only).

### Cost (GPU busy ms/frame, reference desktop GPU (RADV), `--bench … 600`, perf.sh's Extreme preset with the row at Original / Ultra / Cinematic-billboards (`NFSU2_SMOKE2_G10=1`) / Cinematic-volumetric, mean of 2 runs; `scripts/cost2.sh`)

| scene | size | Original mean / p95 | Ultra mean / p95 | The Cinematic smoke work mean / p95 | **volumetric** mean / p95 (its passes) |
|---|---|---|---|---|---|
| explore (heavy burnout) | 4K | 11.74 / 13.20 | 22.38 / 87.73 | 12.12 / 13.62 | **12.10 / 13.39 (0.59)** |
| racing, drift (light) | 4K | 15.02 / 16.65 | 15.17 / 16.74 | 15.18 / 16.69 | **15.36 / 16.90 (0.48)** |
| explore (heavy burnout) | 1080p | 3.03 / 3.37 | 5.70 / 22.36 | 3.42 / 5.18 | **3.40 / 3.98 (0.46)** |
| racing, drift (light) | 1080p | 3.85 / 4.22 (run 2; run 1 had a 6.4 ms p95 outlier) | 3.89 / 4.34 | 3.99 / 4.43 | **4.27 / 4.83 (0.39)** |

Against the budget (≤ +1.5 ms at 4K in a heavy burnout, ≤ +0.3 ms in light smoke): heavy **+0.36 ms** at
4K (the same as the Cinematic smoke work, 16× cheaper than Ultra, p95 13.4 ms vs Ultra's 88 ms); light **+0.34 ms** at 4K
(the Cinematic smoke work +0.16, Ultra +0.15) — **0.04 ms over** the light-smoke budget; at 1080p light smoke +0.4 ms (the march
is a larger share at ½ resolution). The row's cost hint: 0.35 ms (4K, drift).

Per pass (4K, run 1): explore burnout — build (splat, resolve, light) 0.16, march 0.30, composite 0.12;
drift — build 0.09, march 0.31, composite 0.08. At 4K with 8x MSAA the march's first read of the depth
includes the driver's depth decompression (moving `NFSU2_VFX_DIV` from 3 to 4 does not change the march
there), which Original pays later in another pass: the frame difference is smaller than the pass sum.

Steps and their gains (4K drift, smoke passes): full-grid resolve, full-screen march + downsample +
composite 0.92 ms → bounded resolve, brick threshold 0.01, steps ≥ 0.2 m: 0.62 → downsample folded into the
march (second target): 0.53 → tile-flagged composite: 0.48. Image differences per step: the step / threshold
change is a different (equally smooth) sampling; the downsample fold and the tiles are exact
(`explore` pixel-identical but 1 px by 1/255; PSNR 111 dB).

### Quality checks (performance rule)

* 100 % / 200 % crops (`explore_crop_*`, `neon_crop_100pct_*`): no blockiness, no halos at the car's edges
  (bilateral), no noise pattern. A fine third detail octave was tried and **rejected**: at the march's step it
  aliased into a visible hatch pattern.
* Motion: `drift_motion_g10_volumetric.webp` (frames 360–420 every 4th) and `drift_frames_400_403_100pct.webp`
  (4 consecutive frames): no popping or shimmer (world-snapped grid, fixed jitter, the puffs' noise moves with
  them).
* SSIM / FLIP against Ultra or the Cinematic smoke work are meaningless for a different look and were not used.

### Evidence (the maintainer's private captures, Original | Ultra | Cinematic-billboards | Cinematic-volumetric)

The play setup (the play setup's options file's enh lines) + the row; `scripts/cap.sh`.
* `start_*`: `--shot qrstart` NFSU2_FRAMES=300 NFSU2_THROTTLE=1 NFSU2_SHOT_STEP=1 (burnout at the start line).
* `drift_*`: `--bench racing` NFSU2_BENCH_KIND=drift, frame 400, 1080p.
* `post_*`: `--shot ending` NFSU2_SHOT_CAR=240SX NFSU2_ENDING_NIS=1 NFSU2_SHOT_MS=2500 (post-race burnout).
* `explore_*`: `--bench explore` frame 150, 1080p (a standing burnout under warehouse lamps).
* `rain_*`: the same with NFSU2_RAIN_CLOCK=130 (rain 0.75): lamp-lit steam.
* `neon_*`: the same with NFSU2_NEON=MAGENTA: the underglow lighting the smoke from inside.

### Self-assessment (honest, against FH5 / NFS Heat-Unbound / GT7 burnouts)

Better than the Cinematic smoke work by a wide margin: the smoke now has volume, an opaque core, billowing outlines, lit tops and
shaded undersides, glowing red halos around the tail lights, the underglow lighting the cloud from inside,
lamp-lit steam that thins out, and it intersects the car and road softly. Compared with the references it is
**not yet at their level**: (1) the detail is coarse (0.4 m voxels, the erosion noise ~1.5 m) — FH5 / Heat
show finer curling wisps near the wheels; a finer octave needs a smaller step (cost) and was rejected for
aliasing; (2) no shadow of the cloud on the road / car and no motion blur near the wheels; (3) lighting is
ambient-dominated at night — single-scattered lamp light gives halos but less dramatic shafts than Heat's;
(4) the rate and life of the smoke is still the native emitter's, so light drifts give thin trails where the
references show heavier walls. It reads as modern volumetric smoke at normal viewing distance; in a 200 %
crop it is soft rather than richly detailed.

### The blue tint and the integration (2026-11-05)

User after playing this work: "looks out of place … can't explain, and sometimes the smoke gets bluish like the
NOS bar meter colour".

#### Root cause of the blue tint (found, fixed)

`nfsu2::shadow::neon_color` returns the underglow's colour as the **D3D colour bytes B, G, R, A** (what
world.efx's diffuse takes; its test: MAGENTA base R 230, G 26, B 189 → `[189, 26, 230, 255]`). This page's
game.rs hook read them as R, G, B, so red and blue swapped. The player's finished-career Skyline
(the maintainer's probe `save_parts`: slot 0x9A **YELLOW**, 0x9B CORAL) has a yellow neon: R≈1, G≈0.8,
B≈0 became (0, 0.8, 1) — exactly the NOS gauge's light blue — and, × 4 gain over a 3.2 m radius under the car,
it lit the whole lower cloud. Evidence: `neon_yellow_orig_g10b_g10c.webp` (Original | This page: cyan smoke round a
yellow neon | The smoke integration: a yellow glow low round the car). MAGENTA (this work's only neon capture) hides the swap
(R and B are close). Fix: `shadow::neon_rgb` (R, G, B × alpha; test `neon_colour_and_pulse`), used by the
hook.

Two smaller blue sources, also fixed: the native road smoke colour itself is a lavender (XETT_RoadSk/Sp_SmkA
150, 150, 175 → 129, 129, 174; XES_NOSBO_* nitrous smoke 147, 147, 158 — read from L4RA's emitter library),
and this work kept 40 % of its hue in the albedo; under a flat white ambient that read as a cold blue-white cloud
in a warm-lit street. The albedo now keeps 12 % (`Look::tint`); the colour comes from the scene's lights.
Nitrous adds no light to the smoke (no flame light is fed to it); its smoke is just smoke.

#### Integration ("out of place")

This page lit the cloud with a **flat ambient of 0.9** whatever the street: a bright self-lit layer on a dark
night scene, and black lumps where the billow shading darkened it. Changes (all in `volume_fx`, reusable):

| what | how |
|---|---|
| scene ambient | `volume_fx_ambient.wgsl`: the target as drawn so far (the lit world, cars, shadows — before the medium) in 32 × 18 blocks, 256 point taps each, eased 50/50 with the last frame's, plus their mean. The march lights each pixel's medium with the blocks round it (4 taps ±9 % / ±7 %), the road under it (+15 % down) and the view's mean, 60 % saturation, × `Look::scene` 2.6 — dark in a dark street, warm under sodium lamps, purple-tinged under the sky. A target the pass cannot read (no TEXTURE_BINDING) keeps the flat ambient. The multisampled scene is made readable only with Particles Cinematic (postfx `Msaa::readable`). |
| darker cores | the light volume now stores the direct light × albedo in rgb and the ambient's occlusion (0.5 + 0.35 e^(−0.35 τ_up) + 0.15 e^(−0.35 τ_side)) × albedo in a; cores greyer, never black. Billow underside shading 0.6 → 0.35, and it darkens the ambient only half (an underside still sees the lit road). |
| native fog | `View::fog` (the FogValue and colour the world was drawn with this frame: `FogState::drawn`, set where game.rs builds the frame's params; None with the fog switch off): each march sample `L·f + FogColor·(1 − f)`, f = clamp((x − z)·y, z_min, 1) as world.efx. |
| exposure / grade | unchanged: composited into the scene before exposure, grade, bleach and HDR output (no separate brightness). |
| smoke shadow | the march writes, per low-res pixel, the light the medium blocks over the surface it sees (optical depth over 3.5 m straight up, the night's light is overhead) into its depth target's g (Rg32Float); the composite blends `scene × (1 − a) · s` (`Look::shadow` 0.3): road and car darken under the cloud. |
| near the lens | the medium fades in over 0.3–3.5 m from the eye (`Look::near`). |
| dissipation | a puff's optical depth × growth^−(2.5 × age fraction) (`smoke_volume::Look::spread`): young billows dense, old ones thin into haze instead of piling into a wall. Opacity 3.0 kept. |
| coloured lights | the underglow is a **ground light** (negative radius in `SceneLights::points`): radius 2.8 m, faded from 0.3 to 1.2 m above it (the body over it), gain 4 → 1.6 — a local glow in the low smoke, never the whole cloud. Lamps, headlights and tail lights unchanged. |
| other lights | `smoke_cinematic::View::points` → `scene_lights(.., points, ..)`: any further scene point lights (position, radius, colour). The narrow way in for Modern Lighting's light list (Modern Lighting phase 2 on main): fill it with its coloured lights near the view; lamps already pass `modern` for the physical falloff. |

`NFSU2_SMOKE3_LOOK` takes the new names too (`tint, spread, scene, near, shadow`).

#### Evidence (the maintainer's private captures, 1080p, the player's enh settings + Cinematic)

* `neon_yellow_orig_g10b_g10c.webp`: Original | This page | The smoke integration, `--bench explore`, NFSU2_NEON=YELLOW.
* `g10c_explore_g10b_g10c.webp`: This page | The smoke integration, the standing burnout in the yard (frame 150), and `g10c_explore_crop_100pct.webp`.
* `g10c_finished_explore.webp`: the finished-career save Skyline (sandbox copy, NFSU2_FLOW_LOAD=the finished-career save): Original | This page | The smoke integration.
* `g10c_drift_g10b_g10c.webp`: `--bench racing` drift, frame 400: This page | The smoke integration.
* `g10c_community_orig_g10b_g10c.webp`: the player's a community save Lancer (no neon part) driving off in the airport loop.
* `g10c_rain_g10b_g10c.webp`: lamp-lit steam on the wet road, this work | The smoke integration.
* Capture: `scripts/cap.sh` / `scripts/g10c_var.sh` (sandbox copies of the player's saves via NFSU2_FLOW_DATA).

#### Cost (GPU busy ms/frame, reference desktop GPU, `scripts/cost2.sh`, 4K, perf.sh Extreme + the row, mean of 2 runs, same session)

| scene | Original | This page volumetric | **The smoke integration** | The smoke integration's march (this work's) |
|---|---|---|---|---|
| explore, heavy burnout | 11.85 | 12.22 (+0.37) | **12.28 (+0.43)** … 12.40 in a later pair | 0.41 (0.29) |
| racing drift, light | 14.91 | 15.33 (+0.42) | **15.46 (+0.55)** | 0.37 (0.29) |

Heavy smoke is within budget (≤ +1.5 ms). Light smoke is **over** the +0.3 ms budget: This page already measured
+0.42 here (its doc: +0.34), the smoke integration adds ≈ 0.1 ms in the march (the scene ambient pass and lookups ≈ 0.04, the
per-sample fog and split lighting the rest; the surface shadow ≈ 0, its taps skip empty bricks). Build and
composite are unchanged (0.09 / 0.08 ms drift). Run-to-run noise of a frame mean is ±0.1 ms.

#### Self-assessment (honest)

Fixed and clearly better: no blue cloud (the neon swap was the player's "NOS-bar blue"; a yellow neon now
gives a yellow glow low round the car only); the smoke takes the street's light level and colour (warm in the
sodium-lit yard, darker away from the lamps, purple-grey under the sky) instead of glowing white on a dark
night, with lamp halos and tail-light glow scattering in it; no black lumps (greyer cores, soft undersides);
it darkens the road and car under it; old smoke thins into haze; fog applies at the smoke's own depth.
Against FH5 / NFS Heat / Unbound at night it now reads as smoke in the scene rather than a pasted layer at
normal viewing distance, but it is **not at their level**: no fine curling wisps at the tyres, no motion blur
at speed, the screen-space ambient cannot see light from behind the camera (a camera fully inside a closed
burnout, the finished-career save's warehouse frame, becomes a murky wall), shadows only straight down, reflections and
Volumetric Fog cones still show through it (Open). Light drift smoke is subtler than this work's (less bright),
which reads more natural in the stadium but gives less "wall of smoke" drama than Heat's.

### The resolve's bounds and bricks per workgroup
`cs_resolve` (the smoke's, the wet spray's, Ambient Detail's and the water mist's) read the twelve
particle bounds with atomic loads in every one of its threads, cleared the accumulators with an atomic exchange
each and marked the occupied bricks with eight atomic maxima per voxel with density. Now a workgroup reads the
bounds once into workgroup memory and leaves at once when none of its 4³ voxels lies in them; a voxel reads its
nine accumulators and zeroes them with plain atomic loads and stores (no other pass touches them meanwhile);
the bricks its voxels mark are gathered in workgroup memory (its 4³ voxels lie in one brick, the footprint
reaches the 3 × 3 × 3 round it) and each set once. The integer sums, the volumes and the brick marks are the
ones the old pass made. The wet spray's resolve at 4K with the play setup's rows 0.031 → 0.023 ms; on the reference
integrated GPU at level High in rain 0.269 → 0.190 ms (7 interleaved runs each).

### The march specialised to its renderer's look
`fs_march` holds every renderer's branches: the fine octave (`Look::fine`), the billow shading turned toward the
lamps (`Look::directional`, with the scene ambient looked up for it) and the medium's shadow on the surfaces
(`Look::shadow`). A renderer whose look never takes one of them still paid for it in registers: the program was
144 registers (10 waves a SIMD) for every user. The three are override constants now (`FINE`, `DIRECTIONAL`,
`SHADOW`, `march_constants`), set from the renderer's look when its march is made: a branch its look never takes is
left out of its program, the values the others read are the uniform's as before. The wet spray's and the water
mist's march (none of the three) is 72 registers, 20 waves, 1,465 instructions (was 5,336); the Cinematic smoke's
keeps all three (the program as before). The warm-up makes one march for each set the users take (`WARM_LOOKS`).
The same arithmetic; only the compiler's rounding of the smaller program differs: one level on at most seven pixels
of a 480 × 270 frame of night rain (`tests::fog_rain_paths::volume_march_specialised_keeps_the_frames`,
`set_plain_march` makes every march with all three). The wet spray's march at 4K with the play setup's rows 0.414 [0.412..0.417] →
0.368 [0.365..0.378] ms; on the reference integrated GPU at level High in rain 7.11 [7.07..7.21] → 6.63 [6.56..6.69]
ms; Ambient Detail's (the light set, racing at 4K) 0.203 → 0.187 (5 interleaved runs each).


### Parity

Original: `Effects::step` records nothing, `particle_quads` takes the native path, nothing of this runs; the
Extreme presets keep `enh.particles 2`. `scripts/checks/gate.sh … <out dir> extreme`
(2026-11-05): base and new binaries (differ); checks
PASS, 28 shots 0 differ, 144 Hz window path, enhanced Modern / Ultra / Extreme, audio and five simulation
hashes identical: **PARITY: all gates passed**.

2026-11-05: `scripts/checks/gate.sh … <out dir> extreme` (second run): base
44b9a38f7cf13bb9926952682f0a699c vs new e2840c8c65ca2ce85202d9d0d8956daf (differ); checks PASS (1031 tests,
selftest, 28 shots, first race / drag, post race, playthrough), 28 shots 0 differ, window path, enhanced
Modern / Ultra / Extreme, audio and five simulation hashes identical: **PARITY: all gates passed**.

### Tests

`render::volume_fx::tests`: `grid_is_snapped_and_ahead`, `phase_and_multiple_scattering`,
`opacity_to_extinction`, `lights_nearest_and_capped`, `wake_trails_behind_the_car`, `march_divisor`,
`particle_layouts`, `gpu_pool_and_render` (GPU, ignored: emits → pool → export, CPU particles, build, draw);
`render::smoke_volume::tests`: `smoke_lights`, `look_overrides`; `render::smoke_cinematic::tests::gpu_spawn_update_and_draw`
now also checks the steam share and builds the volume.

### Credits (algorithms; no third-party code vendored, so no THIRD-PARTY-NOTICES entry)

* Multiple-scattering approximation: M. Wrenninge, "Oz: The Great and Volumetric", SIGGRAPH 2013 talk;
  S. Hillaire, "Physically Based Sky, Atmosphere and Cloud Rendering in Frostbite", SIGGRAPH 2016 course.
* Energy-conserving integration of scattering: S. Hillaire, "Towards Unified and Physically-Based Volumetric
  Lighting in Frostbite", SIGGRAPH 2015.
* Perlin–Worley cloud noise and density erosion: A. Schneider, "The Real-time Volumetric Cloudscapes of
  Horizon Zero Dawn", SIGGRAPH 2015.
* Henyey–Greenstein phase function: L. Henyey, J. Greenstein, 1941.
* Interleaved gradient noise: J. Jimenez, "Next Generation Post Processing in Call of Duty: Advanced
  Warfare", SIGGRAPH 2014.
* Divergence-free noise ∇a × ∇b: I. DeWolf 2005; Bridson et al., "Curl-Noise for Procedural Fluid Flow", 2007.
* Off-screen low-resolution particles with depth-aware upsampling: I. Cantlay, GPU Gems 3 ch. 23, 2007.

### Open

* Volumetric Fog is still added in the post chain over the scene depth: a lamp's fog cone in front of
  the smoke is right, one behind it shows through the smoke unattenuated (the fog pass does not see the
  smoke's transmittance).
  (Volumetric Fog **Next-Gen** takes the smoke's density into its integration — the haze behind the smoke
  is hidden; Low / High unchanged. docs/notes/graphics/atmosphere.md.)
* No motion blur of the smoke near the wheels (it is drawn after the blur pass); no finer wisp / curl
  octave near the wheels (this work's attempt aliased at the march's step; not retried).
* Road reflections (SSR / wet roads) are composited after the particles, so the car's reflection shows
  through smoke on a wet road (as with High / Ultra / the Cinematic smoke work). Fix path: the composite also scales the SSR
  mask's term by (1 − a) (needs the mask at the particle draw).
* The scene ambient is screen-space: light from surfaces off screen is only the view's mean; smoke that
  fills the whole view (the camera inside a closed-yard burnout) is lit by what little of the scene shows.
* The smoke's shadow is straight down (the overhead light), not toward each lamp.
* Smoke beyond the grid (more than ~56 m ahead or ~10 m behind) is not drawn.
* The generic `ParticlePool` is exercised by its GPU test only; its first users are the next units.
* The stronger wet interaction (docs/notes/graphics/wet-road-car.md "Clearly visible, volumetric") is the second player: Wet Road Interaction's spray through its own
  `VolumeFx` (CPU particles, a flatter grid nearer the eye via `set_anchor`, more march steps for long thin media).

## Tyre smoke toward current games

Goal: smoke closer to current games. Scope (frozen): polish
of the **existing** Particles row's Cinematic tier (the Cinematic smoke work / the volumetric smoke `volume_fx` / the smoke integration, "Volumetric effects and the volumetric Cinematic smoke" (above)),
in place: no new row, no new tier, no new option label. Nothing here exists in SPEED2.EXE and nothing new
was traced; the native emitters still decide where, when and how much smoke spawns (docs/formats.md
"Particle effects"). Original stays the default and byte-identical. All constants are by eye (**inferred**).

### 1. How current games do it (research notes)

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

### 2. Our Cinematic smoke against them (before this work, same views)

At the explore burnout (`--bench explore`, frames 50 / 110 / 150), the race-start burnout (`--shot
qrstart`), the drift event (`--bench racing` drift, frame 500), lamp-lit steam (rain) and the yellow
underglow, Original | Cinematic the smoke integration | Cinematic this work (internal evidence):

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
| shadows toward each lamp, reflections through smoke | — | open ("Volumetric effects and the volumetric Cinematic smoke" (above) "Open") |

### 3. What changed (`render::volume_fx`, `render::smoke_volume`)

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

### 4. Look rule (docs/notes/graphics/look-direction.md, the maintainer's tools)

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

### 5. Motion and temporal stability (look rule E7)

* Racing bench (circuit, 1080p, play setup + Cinematic + Per-Object blur, frames 1100–1103 at 67 mph):
  **pixel-identical** to the smoke integration (no smoke there; the temporal pass writes nothing outside smoke). The
  benches reach no smoke at > 50 m/s; that check is open.
* Drive-off through the burnout (`--bench explore` frames 170–330, `motion_driveoff_*`): no ghost of the
  car or trail at its edges in consecutive frames 250 / 251 at 100 %; the history follows the camera.
* FSR 1 (`enh.upscale 5`, `sbs_fsr_110.webp`) and TAA (the player's): the smoke is drawn after the temporal
  pass at output size (unchanged); no shimmer seen. The grain of an untemporalised fine octave
  (`grain_rejected_v2_100pct.webp`, a still) was the reason for the history gate.

### 6. Cost (GPU busy ms/frame, reference desktop GPU (RADV), 4K, perf.sh Extreme with the row Original / Cinematic, 600 frames, 2 runs; `cost-4k.tsv`, `scripts/cost3.sh`)

| scene | smoke passes the smoke integration | **This page** (build / march / temporal / composite) | frame Δ over Original, the smoke integration → this work |
|---|---|---|---|
| explore, heavy burnout | 0.67 / 0.68 | **0.67 / 0.70** (0.10 / 0.42 / 0.03 / 0.10) | +0.36 / +0.38 → +0.40 / +0.49 |
| racing drift, light | 0.44 / 0.44 | **0.43 / 0.43** (0.06 / 0.25 / 0.03 / 0.05) | +0.43 / +0.44 → +0.34 / +0.29 |

Same cost as the smoke integration (the longer steps pay for the third octave, the light direction and the resolve; the
build is cheaper: fewer thin voxels marked). Frame means are within run-to-run noise (±0.1 ms). Without
the step scale (step 1.0) the march cost 0.53 ms in the burnout.

### 7. Honest self-assessment

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

### Parity, tests

Original: nothing of this runs (Particles Original / High / Ultra take other paths); spray and mist
keep their march (knobs off) except the brick-threshold fix (thin haze < 0.01/m). Tests:
`volume_fx::tests::g10d_knobs_and_layout`, `gpu_pool_and_render` (GPU, ignored: three temporal frames,
ping-pong, kept view), `smoke_volume::tests::look_overrides` (the smoke's knobs).

### Open

* A Wine frame of a burnout at the same camera (the original driven into a standing burnout) — not taken.
* Camera inside the cloud; smoke motion blur at speed; a > 50 m/s smoke motion check.
* Credits: Karis 2014 (TAA neighbourhood clamp), Salvi 2016 (variance clipping), Hillaire 2015, Schneider
  2015, Jimenez 2014 (IGN); no third-party code.
