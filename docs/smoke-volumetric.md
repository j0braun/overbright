# Volumetric effects and the volumetric Cinematic smoke (clone deviation, opt-in)

**2026-11-06**: the fine octave, directional billow shading, the temporal resolve, longer steps
and the smoke's density constants are in docs/smoke-nextgen.md (same cost; the constants below are the smoke integration's).

Design note after playing Particles **Cinematic** (the Cinematic smoke work's billboards, docs/smoke-cinematic.md): "looks
cheap and not modern". Lead 2026-11-05: the "next-gen" bar (simulated, volumetric, lit by the scene) now
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

## Code

| file | what |
|---|---|
| `src/render/volume_fx.rs`, `volume_fx.wgsl` | **the reusable module**: renderer (`VolumeFx`) and GPU particle pool (`ParticlePool`); nothing smoke-specific |
| `src/render/volume_fx_ambient.wgsl` | The smoke integration: the scene ambient (block averages of the target) |
| `src/render/smoke_volume.rs`, `smoke_volume.wgsl` | the smoke-specific layer: Cinematic puffs → volume particles, the smoke's lights and look |
| `src/render/smoke_cinematic.rs` / `.wgsl` | The Cinematic smoke work's tier: its simulation (native births → puffs) unchanged except steam; draws through `SmokeVolume` |
| `src/bin/game.rs` | hooks: the player's underglow colour (`shadow::neon_rgb`, the smoke integration) and the frame's fog (`FogState::drawn`, the smoke integration) into the smoke's `View` |
| `src/fog.rs`, `src/shadow.rs`, `src/render/postfx.rs` | The smoke integration: `FogState::drawn`; `neon_rgb`; the multisampled scene readable with Cinematic |
| `src/frontend/graphics_table.rs` | the tier's help text and cost |

## The reusable module (`render::volume_fx`)

### API

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

### What a frame does

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

## The smoke (`render::smoke_volume`)

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

## Cost (GPU busy ms/frame, reference desktop GPU (RADV), `--bench … 600`, perf.sh's Extreme preset with the row at Original / Ultra / Cinematic-billboards (`NFSU2_SMOKE2_G10=1`) / Cinematic-volumetric, mean of 2 runs; `scripts/cost2.sh`)

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

## Quality checks (performance rule)

* 100 % / 200 % crops (`explore_crop_*`, `neon_crop_100pct_*`): no blockiness, no halos at the car's edges
  (bilateral), no noise pattern. A fine third detail octave was tried and **rejected**: at the march's step it
  aliased into a visible hatch pattern.
* Motion: `drift_motion_g10_volumetric.webp` (frames 360–420 every 4th) and `drift_frames_400_403_100pct.webp`
  (4 consecutive frames): no popping or shimmer (world-snapped grid, fixed jitter, the puffs' noise moves with
  them).
* SSIM / FLIP against Ultra or the Cinematic smoke work are meaningless for a different look and were not used.

## Evidence (the maintainer's private captures, Original | Ultra | Cinematic-billboards | Cinematic-volumetric)

A player's real settings (`the player's options file`'s enh lines) + the row; `scripts/cap.sh`.
* `start_*`: `--shot qrstart` NFSU2_FRAMES=300 NFSU2_THROTTLE=1 NFSU2_SHOT_STEP=1 (burnout at the start line).
* `drift_*`: `--bench racing` NFSU2_BENCH_KIND=drift, frame 400, 1080p.
* `post_*`: `--shot ending` NFSU2_SHOT_CAR=240SX NFSU2_ENDING_NIS=1 NFSU2_SHOT_MS=2500 (post-race burnout).
* `explore_*`: `--bench explore` frame 150, 1080p (a standing burnout under warehouse lamps).
* `rain_*`: the same with NFSU2_RAIN_CLOCK=130 (rain 0.75): lamp-lit steam.
* `neon_*`: the same with NFSU2_NEON=MAGENTA: the underglow lighting the smoke from inside.

## Self-assessment (honest, against FH5 / NFS Heat-Unbound / GT7 burnouts)

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

## The blue tint and the integration (2026-11-05)

User after playing this work: "looks out of place … can't explain, and sometimes the smoke gets bluish like the
NOS bar meter colour".

### Root cause of the blue tint (found, fixed)

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

### Integration ("out of place")

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

### Evidence (the maintainer's private captures, 1080p, the player's enh settings + Cinematic)

* `neon_yellow_orig_g10b_g10c.webp`: Original | This page | The smoke integration, `--bench explore`, NFSU2_NEON=YELLOW.
* `g10c_explore_g10b_g10c.webp`: This page | The smoke integration, the standing burnout in the yard (frame 150), and `g10c_explore_crop_100pct.webp`.
* `g10c_finished_explore.webp`: the finished-career save Skyline (sandbox copy, NFSU2_FLOW_LOAD=the finished-career save): Original | This page | The smoke integration.
* `g10c_drift_g10b_g10c.webp`: `--bench racing` drift, frame 400: This page | The smoke integration.
* `g10c_community_orig_g10b_g10c.webp`: the player's a community save Lancer (no neon part) driving off in the airport loop.
* `g10c_rain_g10b_g10c.webp`: lamp-lit steam on the wet road, this work | The smoke integration.
* Capture: `scripts/cap.sh` / `scripts/g10c_var.sh` (sandbox copies of the player's saves via NFSU2_FLOW_DATA).

### Cost (GPU busy ms/frame, reference desktop GPU, `scripts/cost2.sh`, 4K, perf.sh Extreme + the row, mean of 2 runs, same session)

| scene | Original | This page volumetric | **The smoke integration** | The smoke integration's march (this work's) |
|---|---|---|---|---|
| explore, heavy burnout | 11.85 | 12.22 (+0.37) | **12.28 (+0.43)** … 12.40 in a later pair | 0.41 (0.29) |
| racing drift, light | 14.91 | 15.33 (+0.42) | **15.46 (+0.55)** | 0.37 (0.29) |

Heavy smoke is within budget (≤ +1.5 ms). Light smoke is **over** the +0.3 ms budget: This page already measured
+0.42 here (its doc: +0.34), the smoke integration adds ≈ 0.1 ms in the march (the scene ambient pass and lookups ≈ 0.04, the
per-sample fog and split lighting the rest; the surface shadow ≈ 0, its taps skip empty bricks). Build and
composite are unchanged (0.09 / 0.08 ms drift). Run-to-run noise of a frame mean is ±0.1 ms.

### Self-assessment (honest)

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

## The resolve's bounds and bricks per workgroup
`cs_resolve` (the smoke's, the wet spray's, Ambient Detail's and the water mist's) read the twelve
particle bounds with atomic loads in every one of its threads, cleared the accumulators with an atomic exchange
each and marked the occupied bricks with eight atomic maxima per voxel with density. Now a workgroup reads the
bounds once into workgroup memory and leaves at once when none of its 4³ voxels lies in them; a voxel reads its
nine accumulators and zeroes them with plain atomic loads and stores (no other pass touches them meanwhile);
the bricks its voxels mark are gathered in workgroup memory (its 4³ voxels lie in one brick, the footprint
reaches the 3 × 3 × 3 round it) and each set once. The integer sums, the volumes and the brick marks are the
ones the old pass made. The wet spray's resolve at 4K with the player's rows 0.031 → 0.023 ms; on the reference
integrated GPU at level High in rain 0.269 → 0.190 ms (7 interleaved runs each).

## The march specialised to its renderer's look
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
`set_plain_march` makes every march with all three). The wet spray's march at 4K with the player's rows 0.414 [0.412..0.417] →
0.368 [0.365..0.378] ms; on the reference integrated GPU at level High in rain 7.11 [7.07..7.21] → 6.63 [6.56..6.69]
ms; Ambient Detail's (the light set, racing at 4K) 0.203 → 0.187 (5 interleaved runs each).


## Parity

Original: `Effects::step` records nothing, `particle_quads` takes the native path, nothing of this runs; the
Extreme presets keep `enh.particles 2`. `scripts/checks/gate.sh … <work dir>/g10 extreme`
(2026-11-05): base and new binaries (differ); checks
PASS, 28 shots 0 differ, 144 Hz window path, enhanced Modern / Ultra / Extreme, audio and five simulation
hashes identical: **PARITY: all gates passed**.

2026-11-05: `scripts/checks/gate.sh … <work dir>/g10b extreme`: base
44b9a38f7cf13bb9926952682f0a699c vs new e2840c8c65ca2ce85202d9d0d8956daf (differ); checks PASS (1031 tests,
selftest, 28 shots, first race / drag, post race, playthrough), 28 shots 0 differ, window path, enhanced
Modern / Ultra / Extreme, audio and five simulation hashes identical: **PARITY: all gates passed**.

## Tests

`render::volume_fx::tests`: `grid_is_snapped_and_ahead`, `phase_and_multiple_scattering`,
`opacity_to_extinction`, `lights_nearest_and_capped`, `wake_trails_behind_the_car`, `march_divisor`,
`particle_layouts`, `gpu_pool_and_render` (GPU, ignored: emits → pool → export, CPU particles, build, draw);
`render::smoke_volume::tests`: `smoke_lights`, `look_overrides`; `render::smoke_cinematic::tests::gpu_spawn_update_and_draw`
now also checks the steam share and builds the volume.

## Credits (algorithms; no third-party code vendored, so no THIRD-PARTY-NOTICES entry)

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

## Open

* Volumetric Fog is still added in the post chain over the scene depth: a lamp's fog cone in front of
  the smoke is right, one behind it shows through the smoke unattenuated (the fog pass does not see the
  smoke's transmittance).
  (Volumetric Fog **Next-Gen** takes the smoke's density into its integration — the haze behind the smoke
  is hidden; Low / High unchanged. docs/atmosphere.md.)
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
* The stronger wet interaction (docs/wet-road-car.md "Clearly visible, volumetric") is the second player: Wet Road Interaction's spray through its own
  `VolumeFx` (CPU particles, a flatter grid nearer the eye via `set_anchor`, more march steps for long thin media).
