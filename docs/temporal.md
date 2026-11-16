# Temporal tier: TAA, Upscaling, Volumetric Fog (clone deviations, opt-in)

Wave G this work (roadmap `research/roadmap-2026-11-04.md`, research report
items 7 and 9). Nothing here exists in SPEED2.EXE (no temporal pass, no
upscaler, no volumetric light: its fog is world.efx's per-vertex fog plus
the horizon fog grid 0x5C7480); nothing was traced and nothing is native.
All rows are off by default; with them off the image is the native one
(same passes; see "Native byte-identity").

| row (Graphics group) | values | options.txt |
|---|---|---|
| **Anti-Aliasing** (Image): new value **TAA** | … / SSAA 16x / TAA | `enh.post_aa 11` |
| **Upscaling** (Performance) | Off / Quality 67 % / Balanced 58 % / Performance 50 % (FSR 1: 4..6, docs/modern-extras.md; FSR 2: 7..11, docs/fsr2.md, run by this pass in place of its resolve) | `enh.upscale 0..3` |
| **Volumetric Fog** (Lighting) | Off / Low / High | `enh.volumetric_fog 0..2` |

`qol::temporal()` (TAA or Upscaling), `qol::upscale()`, `qol::volumetric_fog()`.
Presets: Ultra gets Volumetric Fog High; TAA and Upscaling stay manual
(the AA row's help: "Temporal AA: smooth, stable edges in motion, but
softer; MSAA is the original's look"). Modern gets nothing new (TAA is
measured cheap but visibly softer than native 8x MSAA, so not "clearly
better"; see Results).

## TAA (`render/temporal.rs`, `temporal.wgsl`)
* **One AA setting**: TAA forces the scene to 1 sample (`qol::msaa_combined`
  returns 1 for it, whatever MSAA Override says) and turns FXAA / SMAA off
  (`qol::fxaa`, `qol::smaa`); the MSAA Override and AA Post Filter rows are
  greyed under it, Customize Display's Full Screen Anti-Aliasing reads
  "Disabled - Enhancements". Sharpening stays available after it (the glow-under-SSAA pass
  CAS with its limiter, `enhance.rs`), the recommended companion.
* In game only (the world drawn: free roam, races, NIS scenes, in-game
  menus over the world; `qol::set_temporal_frame`). The front end's and
  the garage's scenes are drawn as native, **with the native MSAA**, when
  TAA / Upscaling is chosen (`qol::msaa_in_frame`; `--shot qrcars` with TAA
  is md5-identical to native).
* **Jitter**: `Camera::jitter` (clip-space offset applied in
  `Camera::projection_between`) is set on the main view's camera for the
  scene's draws only (world main view and its reflections, cars, shadows,
  characters, horizon fog) and cleared after; Halton (2, 3), 8 phases
  (upscaling: 8 × pixel ratio, ≤ 32).
* **Texture LOD bias** (wgpu samplers have none): while the pass runs the
  main view's world and car pipelines use biased copies of their modules
  (`temporal::biased_source`: `textureSampleBias` on world.wgsl's
  `sample_tex` and car.wgsl's diffuse, pipeline constant `MIP_BIAS`;
  `SceneMode::bias_q`), log2(render share) − 0.5 levels: TAA −0.5,
  Quality −1.0, Balanced −1.25, Performance −1.5. The native modules are
  never touched. Laplacian energy against native 8x MSAA (racing, frame
  200): TAA car 41 → 59 %, billboard 68 → 77 %; Upscaling Quality
  billboard 101 % (`mip_bias.webp`).
* **Motion vectors without a velocity buffer**: for each output pixel the
  closest depth of its input neighbourhood, a cross of five samples
  (dilation; a 3 × 3 cost 8.4 ms on the iGPU against 5.2 for the cross) is turned into
  a camera-relative position (`clip_to_rel`, f32-safe across Bayview),
  reprojected with last frame's unjittered view (`prev_rel_to_clip`). A
  position inside a drawn car's box (shadow box ∪ wheels, padded 0.15 m,
  floor 6 cm above the tyres' contact) moves rigidly with that car:
  drawn pose to drawn pose, i.e. **render frame to render frame with the
  interpolated poses** (`interpolate.rs` swaps them in before the draw; the
  pass records what was drawn). Up to 32 cars, the nearest first, keyed
  by slot (player 0, companion 1, rivals 100+, encounters 200+, traffic
  1000+; a key whose pose jumped > 12 m or is new takes no history).
  Points in a wheel's cylinder are **reactive** (blend ≥ 0.35, tighter
  clip): the rims' spin moves texture where the rigid motion does not.
  Characters (race-start crowd) get the camera's motion only (clip
  handles their animation; **inferred** adequate, not measured).
* **Resolve**: at 1× the pixel's own jittered sample is the current frame
  (the jitter's accumulation is the filter); upscaling reconstructs from
  the cross's five input samples with Blackman-Harris-like weights by each
  sample's true distance in output pixels. History Rgba16Float (colour and
  the scene's alpha, NFSU2's bleach mask), Catmull-Rom (5 taps), clipped in
  YCoCg to the cross's box ∩ mean ± γσ (γ 1, upscaling 1.25),
  blended in a luma-weighted space; blend 0.1 (upscaling 0.08) × coverage,
  + 0.2 per pixel of motion up to +0.3. Cars are tested against a
  bounding sphere first, and only those not behind the eye are sent. At 1×
  the result is copied over the scene (it is the pass's input); with
  Upscaling the history and the scene are written in one pass (MRT). The
  native post chain (streaks, bleach bypass, overbright glow, rain tint)
  runs after it unchanged.
* **Composited after the pass, without jitter** (`late_viewport` moves the
  viewport by −jitter so the jittered main view's matrices draw them where
  native would): particles (nitrous flames, tyre smoke, sparks) and the
  rain quads (`WorldRenderer::upload_particles` in the scene,
  `draw_particles` after the pass), glow sprites into the streak buffer
  (`draw_glows_at`; into the scene when streaks are off), light streaks,
  lens drops and the HUD (native chain / UI, all after the pass).
* **History reset**: a frame drawn without the pass (menus over the front
  end, garage), `FrameInput::key` change (mode, `Game::camera_mover` —
  which numbers each shot of a scene's camera track, so every NIS
  cut resets — shop / NIS scene: menu ↔ world, camera mode changes, NIS start/end), `is_cut`
  (camera moved > 12 m, turned > 0.6 rad, game-FOV presence or > 15 %
  FOV change in one drawn frame: NIS cuts, teleports, replay camera
  switches), a size change, reversed-Z toggled.

SSAO, SSR (with the reflections pass's car mask, glows drawn alone into the rays' target,
screen-size tracing) and Volumetric Fog run on the jittered scene before
the pass (`PostFx::pre_passes`, the same code `apply` runs without it).
The post-race jitter fix's scene cadence (1/30 s updates, frames between interpolated) needs
nothing of its own: the motion is taken between the poses drawn.

## Upscaling (same pass)
The scene is drawn at the tier's share of the screen in a stage of its own
(`PostFx::set_upscale`: a second `PostFx` at the render size holding the
scene, motion blur 0x5CB31F, horizon fog, SSAO, SSR, Volumetric Fog); the
temporal pass reconstructs it into the main `PostFx`'s scene at the
screen's size, writes the depth at the screen's size (`fs_depth`, the
late sprites' depth test), and the native post chain runs at the screen's
size as native does. Render Scale is held (greyed, "Upscaling Quality")
while Upscaling is on; Render Scale < 100 % without Upscaling stays the
plain resample path; SSAA / MSAA are off under it.

## The resolve as a compute pass (TAA)
With TAA (the input at the output's size) the resolve into the history runs as a compute pass
where the device gives the same history with it (`render/enh/post/temporal/compute.rs`,
`temporal_cs.wgsl`); the copy to the scene after it is the same fragment pass as before. The
compute module is temporal.wgsl itself (the same `resolve` per output pixel, its position the
pixel's centre as a fragment's) with one addition in the car-motion loop: a whole 8×8 tile skips
the cars whose screen rectangle it does not touch (`car_rect`: the car's bounding sphere's box
projected with this frame's jittered projection, mapped to output pixels as the cross maps them,
three input and four output pixels of margin; the whole plane when a corner is not in front of
the eye). A pixel outside a car's rectangle cannot pass that car's sphere test, so every pixel
takes the same car, in the same loop order, with the same values; the test runs once per tile
(uniform), not per pixel.

Chosen once when the pass is made (`Temporal::new`, so in the load's and the settings' warm-ups
and in `game --prepare`), never per frame, on integrated GPUs: there it was measured 1–3 ms
faster; on the desktop GPU it was as fast within 0.01 ms at 1920×1080 and 0.03 ms slower at a
3840×2160 race start, so discrete GPUs keep the fragment pass (`NFSU2_TAA_COMPUTE=1` asks for the
compute pass on any device, `=0` on none). The fragment pass also stays where the device has no
storage binding for Rgba16Float or no 8×8 workgroups, and where the self-check differs. The self-check runs both passes on a made-up 44×44 frame (half-float colours
and history to 16, a valid history reprojected inside the image, two cars, one with a wheel, one
fresh with a small sphere whose rectangle leaves most tiles out, a span of 1.7 ticks) and compares
the two histories byte for byte; the verdict is kept per device for the process. It guards the
one place the two passes could differ: how the device rounds the float result into the half-float
history (a colour target's export and a storage write are free to round differently). On the
machine's two GPUs (RADV) the two are the same: the self-check, the bench frames, and the history
images of the measuring builds (desktop GPU), byte for byte. Other drivers (DX12, other vendors) were not
run here; there the self-check decides.

Upscaling keeps the fragment pass: its second target is the scene, not a storage texture, and the
resolve into the history plus a copy into the scene measured no faster there (integrated GPU,
Medium + Performance −0.6..−1.5 ms, Medium + Quality at the race start +0.2; desktop 4K +0.04).
Measured attribution and the candidates: docs/perf-integrated-gpu.md "The temporal resolve".

## Volumetric Fog
Every tier (Low, High, Next-Gen) is the one froxel medium of docs/atmosphere.md (`render::atmosphere`), at
the tier's grid, range and light counts: "The tiers: one medium at every fog tier" there. The earlier
analytic cone march of Low / High (`volumetric.wgsl`, a half-size march with a depth-aware upsample) is
gone; `render/volumetric.rs` keeps a frame's inputs and the nearest-lamps pick other effects share.

## Tuning / debug aids (not options)
`NFSU2_TAA_COMPUTE=0` keeps TAA's resolve in the fragment pass (the A/B of the compute pass).
`NFSU2_TAA_DEBUG=1` motion (R, G = |x|, |y| px / 2; B history valid),
`=2` car / wheel / fresh flags, `=4` reconstructed distance, `=5` no
history, `=6` zero motion; `NFSU2_TAA_VEL`, `NFSU2_TAA_VEL_MAX`;
`NFSU2_BENCH_KIND=drift|drag` (bench `racing` on the first event of that
kind); `NFSU2_BENCH_DRAWS=n` (bench: n drawn frames per world update, the
first n − 1 interpolated by `draw_between`, as a display at n × 60 Hz).

## Results
Machine: reference desktop GPU (RADV), and the reference integrated GPU, an RDNA2 (2 CUs,
`MESA_VK_DEVICE_SELECT='1002:164e!'`, measured). GPU busy ms/frame,
`--bench explore 600` at night (`NFSU2_RAIN_CLOCK=93`), mean of 2 runs
(internal evidence; data in the work dir). Original
plays with native 8x MSAA (World Detail 5), which TAA and Upscaling replace.

| | 1080p | 4K | iGPU 1080p |
|---|---|---|---|
| Original (native 8x MSAA) | 0.68 | 2.38 | 13.75 |
| no AA (MSAA Override Off), for reference | 0.43 | 1.08 | 7.94 |
| TAA | 0.56 (−0.12) | 1.58 (−0.80) | 15.23 (+1.48) |
| Upscaling Quality 67 % | 0.53 (−0.15) | 1.46 (−0.92) | 13.18 (−0.57) |
| Upscaling Balanced 58 % | 0.68 (noisy) | 1.48 (−0.90) | 13.10 (−0.65) |
| Upscaling Performance 50 % | 0.55 (−0.13) | 1.42 (−0.96) | 12.41 (−1.34) |
| Volumetric Fog Low | 0.92 (+0.24) | 2.65 (+0.27) | 18.88 (+5.13) |
| Volumetric Fog High | 0.94 (+0.26) | 2.84 (+0.46) | 20.59 (+6.84) |

The temporal resolve itself: 0.11 ms at 1280 × 960, ~0.5 ms at 4K, 5.2 ms
on the iGPU at 1080p (+1.2–1.6 ms for the copy / depth pass): on the iGPU
TAA costs as much as the scene, and Upscaling only pays off against 8x
MSAA, not against no AA. The scene is so cheap on the discrete GPU that
the three Upscaling tiers cost the same (the resolve at the screen's size
dominates). Cost classes in the table: TAA / Upscaling "saves", Volumetric
Low Low / High Med; the help text names the iGPU figures.

Image (1280 × 960; `docs/evidence/temporal/`):
* Sharpness, Laplacian energy against native 8x MSAA: static TAA 58 %,
  moving (29 mph) 41–48 % on the player's car: **TAA is softer** (the
  jitter box-filters texture detail; no LOD bias). `static_native_taa_upscale.webp`,
  `motion_native_taa.webp`, `motion_upscaling.webp`.
* Ghosting: a rival passing at 73 mph (body, wheels, tail light): no trail
  (`ghost_rival_73mph.webp`); rain at speed: drops and particles are drawn
  after the pass, no smear (`ghost_rain.webp`); drift and drag cameras
  (83 mph in the tunnel) clean (`ghost_drift_drag.webp`); race-start NIS
  with its cuts (`nis_start.webp`): per-frame RMSE to native 0.010–0.027
  through the scene, no doubled images after cuts.
* Interpolated rendering: 4 drawn frames per update (3 by `draw_between`,
  240 Hz) against 1: RMSE 0.007, same edges, no trails (`interpolated_240hz.webp`).
* Volumetric Fog Off / Low / High on the night race (`volumetric_off_low_high.webp`):
  soft cones under the lamps; road and sky between lamps unchanged.
* TAA is not added to the Modern preset: cheap on a discrete GPU but
  visibly softer than the native 8x MSAA (not "clearly better").

## Native byte-identity
All temporal rows off: md5 of `--shot` 1280 × 960 against the base binary
(this copy before the temporal tier, its own target): explore, intro, qrcars, race,
qrstart, drift, drag **identical**; main / career differ as the base does
from itself (turntable: base vs base 352 / 345 px, base vs new 322 / 40).
The window path (`--bench racing`, dumps of frames 100 and 300, native
8x MSAA): **identical**. Off, the camera's jitter is zero (the projection
is not touched), particles draw as before (`render_particles` = upload +
draw), `PostFx::stage()` is the PostFx itself.

## Tests
`render::temporal::tests` (jitter sequence and clip/viewport math,
camera reprojection of world points with the real chase camera of the
racing bench within 0.05 px, rigid car motion, cuts resetting the
history, upscaler sizes and phases), `render::volumetric::tests` (the
native weight² integral, density by tier and rain), `qol` round trip
(36 deviations, TAA / Upscaling / Volumetric values and ranges, MSAA
under TAA), `options::tests::graphics_page_steps_and_restores` (row
order, TAA after SSAA 16x, Render Scale held under Upscaling, AA row text).
`render::temporal::compute::tests`: the compute module is temporal.wgsl plus the tile's test and
validates; points of a car's sphere seen by a jittered chase camera at six turns and four
distances fall inside the car's rectangle with the cross's reach to spare, a sphere around or
behind the eye covers the plane; `the_compute_pass_gives_the_fragment_pass_history` (ignored,
needs a GPU) runs the self-check on the machine's adapter. `tests::first_use::
the_temporal_rows_are_ready_before_their_first_frame`: TAA
and every Upscaling tier (TAAU, FSR 1, FSR 2) at Low, Medium, High and Extreme with the native
FSAA on make no program in a free roam's first ten frames, loaded straight away or after the
menus' programs made ahead (the copy to the scene used to be made by TAA's first frame; the
world's, cars', horizon fog's, blur's and rain's programs for the single-sampled temporal frame
were made by the first frames while the warm-ups took the menu frame's sample count, and FSR 2's
passes by every first FSR 2 frame: docs/first-use-builds.md).

## Open
* TAA is still softer than native 8x MSAA (59–77 % of its edge energy
  with the LOD bias) — use Sharpening with it. The costs and the other
  crops were measured before the port onto main; the bias adds no
  pass.
* Garage / front-end scenes get no temporal pass (native MSAA instead).
* Characters and the shadow-texture decals move with the camera's motion
  only (clipping handles them; ghosting not measured on the crowd).
* Upscaling Performance (50 %) shows stair-steps on long edges where the
  history is clipped; Quality is the recommended tier.
* Volumetric High is not a froxel grid (above); the cone's shape is
  inferred.

## Volumetric Fog Low / High budget (2026-11-05, docs/look-direction.md rule E4)
`volumetric::DENSITY` 0.2 → **0.04**, `RAIN_DENSITY` 1.5 → **0.3**. At 0.2 the cones lifted the look direction
spots' mean luma +2.8 (Low) / +8.0 (High; +14 in rain) and the native overbright bloomed them again;
the pass is purely additive (no extinction), so its whole output is added energy and is budgeted.
