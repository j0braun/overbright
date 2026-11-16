# Particles Cinematic: modern smoke (clone deviation, opt-in)

> **Superseded look (2026-11-05):** the Cinematic tier now renders the puffs described here as
> **volumetric smoke** (docs/smoke-volumetric.md: `render::volume_fx` + `render::smoke_volume`). The
> simulation below (births → billows + haze, noise flow, wakes) still drives it; the billboard / OIT drawing
> below is kept only for comparisons (`NFSU2_SMOKE2_G10=1`). Steam changed: it starts from the current rain
> (it started dry), lives shorter and evaporates. The cost table below is this work's.

Design note: "can the smoke be better looking, still looks like native". Nothing here exists in
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

## Code (new files; High / Ultra's path untouched)
`src/render/smoke_cinematic.rs`, `src/render/smoke_cinematic.wgsl`. It reuses only
`gpu_particles::Feeder` (the native births, recorded per 1/60 s world update) and a few constants;
`gpu_particles.rs` / `.wgsl` are not edited (the second performance audit works on their cost). game.rs: an `Effects::cinematic`
field, `Effects::step` records into it when the row is 3, `particle_quads` prepares it (and drops High /
Ultra's GPU half), and its `draw` / `warm` sit next to `Detail`'s at the three draw sites (main view,
late TAA/TAAU pass with its viewport, `--shot`). As High / Ultra, the native quads of smoke
descriptions are left out of the draw; sparks, nitrous flames, markers and the flag girl's flare stay
native.

## What it does
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

## Cost (GPU busy ms/frame, reference desktop GPU (RADV), `--bench … 600`, perf.sh's Extreme preset with the row at Original / Ultra / Cinematic, mean of 2 runs; `scripts/run.sh`; the smoke column is the smoke draw pass alone)

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

## Evidence (the maintainer's private captures, Original | Ultra | Cinematic unless named)
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

## Parity
Original: `Effects::step` records nothing (`births` None), `particle_quads` takes the native path,
`Cinematic::prepare` is not called; the Extreme presets keep `enh.particles 2`. `scripts/checks/gate.sh … <base> extreme`
(2026-11-05): base and new binaries (differ);
images, 144 Hz window path, enhanced (Modern / Ultra / Extreme / Extreme FSR), 28 shots (0 differ),
audio and five simulation hashes: **PARITY: all gates passed**.

## Tests
`render::smoke_cinematic::tests`: `tier_routing`, `reduced_resolution`, `bilateral_prefers_matching_depth`,
`wake_trails_behind_the_car`, `lamp_falloff_windows`, `noise_volume_tiles_and_has_gradients`,
`gpu_spawn_update_and_draw` (GPU, ignored: births → billows + one haze each, a draw through all three
passes, all dead after 9 s); qol `enh.particles` clamp now 3.

## Credits (algorithms, no code vendored)
* Weighted blended order-independent transparency: M. McGuire, L. Bavoil, "Weighted Blended
  Order-Independent Transparency", JCGT 2(2), 2013.
* Off-screen low-resolution particles with depth-aware upsampling: I. Cantlay, "High-Speed, Off-Screen
  Particles", GPU Gems 3 ch. 23, 2007.
* Improved gradient noise: K. Perlin, "Improving Noise", SIGGRAPH 2002; cellular noise: S. Worley,
  SIGGRAPH 1996; Perlin–Worley remap for cloud density: A. Schneider, "The Real-time Volumetric
  Cloudscapes of Horizon Zero Dawn", SIGGRAPH 2015.
* Divergence-free noise as ∇a × ∇b: I. DeWolf, "Divergence-Free Noise", 2005 (cf. Bridson et al.,
  "Curl-Noise for Procedural Fluid Flow", SIGGRAPH 2007).

## Open
* Exhaust smoke / blow-off (CARFX kinds 11 / 12) are still not emitted by the clone: no constant
  0x61FB10(11|12) call: the car update's calls (0x5F5999 … 0x5F616B, and 0x5EBEF9) push kinds 1–6, 9,
  10, 13 only; the variable-kind
  callers are the scene effects (0x4396xx) and 0x633F8F (a 2-bit state table at 0x803714, not
  resolved). Without a native trigger nothing is invented, so there are no exhaust/backfire puffs to
  restyle (inferred from the call sites; open).
* Rain streaks are drawn before the smoke (as with High / Ultra), so a cloud hides the streaks behind
  it but also those in front of it.
* No light-space shadow of the cloud onto the road or the car; no coupling with Volumetric Fog.
