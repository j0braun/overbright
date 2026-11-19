# Motion blur and depth of field: how it works

The technical part of [Motion blur and depth of field](../../playing/settings/motionblur-dof.md): how it works, what it costs, how it was checked and what is open. The player part (the rows and what they do) is on that page.

## Overview

Research
report item 8). The original's own effects stay the default and are not
touched: its camera speed blur (0x5CC470, `src/motion_blur.rs`, docs/formats.md
"Motion blur"; verified) and its depth of field switch (0x870CF0; its pass
0x5CCCE0 never runs behind the chase camera and is not ported, docs/formats.md
"depth pass"). Everything below is new, nothing of it is in SPEED2.EXE, and
nothing was traced for it.

| row (Graphics → Effects) | tiers | stored |
|---|---|---|
| **Motion Blur** | Off / On (the original's, Customize Display's Motion Blur) / **Camera** / **Per-Object** | `custom.11` + `enh.motion_blur 1..2` |
| **Depth Of Field** | Off / On (the original's switch) / **Cinematic** | `custom.13` + `enh.depth_of_field 1` |

One concept, one row: the first tiers ARE Customize Display's values
(`graphics_table` `native: Some(11 / 13)`); the clone's tiers sit above them
and hold the native value at World Detail 5's (1). Camera and Per-Object
replace the original's speed blur (its call under `Switches::motion_blur`
is skipped while `qol::motion_blur() != 0`; at 0 it is the unchanged native
path). The presets leave both rows alone (stylistic, like TAA).
`qol::motion_blur()`, `qol::depth_of_field()`.

## Code
`src/render/enh/post/blur_dof.rs`, `blur_dof.wgsl` (new module); hooks: one field in
`PostFx` (`blur_dof`), `game.rs` `blur_and_dof` / `blur_cars` (the game's
frame and the shots) and `draw_garage` (the turntable), two `Game` fields
(`world_time`, `draw_alpha`: the drawn poses' time), the rows
(`graphics_table.rs`, `options.rs` `RowId::MotionBlur / DepthOfField`) and
the values (`qol.rs`).

## Order (as docs/notes/graphics/temporal.md: nothing that must stay sharp is in it)
World → cars → their shadows → race-start characters → SSAO →
**motion blur → depth of field** → particles (nitrous flames, smoke, rain) →
glow sprites → horizon fog → (TAA / Upscaling) → native post chain (light
streaks, bleach, overbright glow, rain splatter) → HUD. The scene is
resolved (MSAA) or copied once per pass; each pass writes its result back
over every sample of the multisampled scene (as SSAO's apply), so the
native resolve and everything after it run unchanged. Glows, streaks,
particles and the HUD never ghost or smear: they are drawn after it.
Under an **FSR 2** mode (docs/playing/settings/fsr2.md) both passes run after the
upscaler instead (FSR 2's guide): on the display-size image with the
display-size depth and the unjittered camera, before the late sprites
(particles, glows), which keeps their place relative to the additive
layers; cost at 4K Quality +0.61 ms (display-size passes), the jittered
silhouette halo gone. Every other mode keeps the order above.

## Motion blur
* **Velocity pass** (full size, Rgba16Float: streak px, distance, kind): the
  camera's motion from the depth (camera-relative reprojection with the last
  drawn view, f32-safe across Bayview, as the temporal tier; the current jitter on both
  views, so TAA's jitter is no motion). Inside a drawn car's box (the temporal tier's
  `temporal::Car`: shadow box ∪ wheels, padded 0.15 m; floor raised to
  `FLOOR` 0.15 m over the tyres' contact so the road beside a car keeps
  the camera's motion): the player's car (key 0) is **masked** (sharp) in
  both tiers; in **Per-Object** every other car moves by its own rigid
  motion (drawn pose to drawn pose), and a wheel's pixels are flagged for
  an **arc** about the axle (spin rad/s × shutter, ≤ 2.5 rad; car-space
  axle = y, steering not applied: **inferred** adequate). In **Camera**
  cars take the camera's motion, no arcs, and the player's wheels are
  masked with the body. A car with no last pose (new, or > 12 m jump) takes
  the camera's motion.
* **Strength**: Camera tier = camera motion × `speed_ramp` (smoothstep 8 →
  40 m/s of camera speed: nothing in town traffic, full at ~90 mph);
  Per-Object's cars take their full motion over the shutter.
* **Shutter**: `SHUTTER` = 1/120 s (180° at 60 fps). The motion between two
  drawn frames is scaled by `SHUTTER / dt`, dt the drawn poses' time step
  (`Game::world_time − (1 − alpha)·last update`, so interpolated frames
  between the 60 Hz updates count their own time): the same streak at 60,
  144 or 240 Hz (test, and measured below). A pause (dt 0) or a stall
  (dt > 0.1 s), a cut (`temporal::is_cut`) or a change of what draws the
  view (`temporal_key`: mode, camera mover / NIS shot, scene) draws none.
  Longest streak 4 % of the scene's height.
* **Gather**: tile max (tile = half the longest streak), 3 × 3 neighbour
  max, then McGuire et al.'s reconstruction (15 taps along the neighbourhood's
  dominant motion, depth-soft foreground/background cones, per-pixel noise)
  so moving silhouettes spread over a sharp background and a sharp
  foreground (the player's car) never smears. Wheel pixels: 12 taps on the
  arc (plus the car's own motion), taps leaving that wheel dropped. The
  scene's alpha (the bleach mask) stays the pixel's own.
* **A still** (`--shot`): no last frame; one is made from the velocities a
  shutter ago (camera position − velocity·SHUTTER, `MotionBlur::velocity`;
  cars by `pose_before` with their velocity and spin). The camera's
  rotation is not in it (**open**, shots only).

## Depth of field (Cinematic)
* **Where**: only while watched, never while driving (`dof_scene`): a scene's
  camera track (mover 0x100 + shot: race starts, endings, the career start,
  shop drive-ins), the replay camera (0x12), photo mode
  (`blur_dof::set_photo_mode`; the photo-mode work sets its own focus and f-number with
  `set_photo_dof`, docs/playing/photo-mode.md), and the garage / showroom turntable
  (`draw_garage`, after the car and its shadow, before neon and flares). Not
  the chase (1), crash / jump (0x0F / 0x10) or the magazine showcase (0x14).
* **Focus**: the car the camera looks at (the nearest to the view's axis
  within 80 m, else the player's), its whole box in focus: the band from
  its nearest to its farthest corner (−0.3 / +0.6 m; from the eye when the
  camera is inside it).
* **Blur**: circle of confusion from the distance beyond / before the band
  (largest 1.2 % of the height), per pixel once (`fs_coc`); the scene and
  the CoC halved (2 × 2 mean / nearest); 40 taps on a fixed golden-angle
  disc at half size reading the half-size scene bilinearly (a 4 × 4
  footprint each: no ghost copies; a per-pixel turn of the disc cost 3× and
  looked grainy); a tap behind the pixel counts only up to the pixel's own
  blur (the car's edges stay sharp), a tap in front spreads its own and
  records its cover; "bokeh-lite": taps brighter than 0.85 weigh up to 1.3×.
  The composite (full size) mixes the half-size result in by the pixel's own
  CoC or the foreground's cover: in-focus pixels are the scene's own.

## Motion blur tiles (2026-11-05, exact; docs/notes/performance/gpu.md "SSR, motion blur tiles, submissions")
* Tile max in two steps: `fs_tile_rows` takes, per tile and sampled row
  (every other row, `tile.div_ceil(2)` per tile, stacked in the
  "blur tile rows" target), the first longest velocity of the row;
  `fs_tile` then the first longest of the tile's rows, top down. Strict
  `>` in both: the same vector as the single scan (ties: the first in
  scan order), proved on a CPU model (`two_step_tile_max_matches_one_scan`)
  and by dumps (AE 0). 4K Extreme: 0.36 → 0.23 ms for tiles + neighbours
  (the single scan ran a few thousand fragments of ~960 serial loads).
* The gather over a single-sampled scene discards the pixels it would
  return unchanged (the player's car, still tiles: `SKIP_UNCHANGED`
  pipeline constant) instead of writing the copy's value back. A
  multisampled scene keeps the write (the resolved colour goes into every
  sample there).
* Depth of field: unchanged (open: a tile classification for in-focus
  blocks needs a proof that their half-size gather is never read with a
  weight; not done).

## Cost
GPU busy median ms/frame, `--bench racing 600` (night race, 3 rivals,
traffic, 8x MSAA at World Detail 5), 3 runs each interleaved (iGPU 2),
`scripts/bench.sh`; Δ over Original, and this work's passes' own timestamps.
Depth of field timed behind the chase camera with `NFSU2_DOF_ALWAYS=1` (in
the game it runs only in scenes, replays and the garage). The machine was
shared with other jobs' GPU work (GPU noise ≈ 1–2 %).

| | 1080p | 4K | iGPU 1080p (reference integrated GPU) |
|---|---|---|---|
| Original | 0.76 | 2.53 | 16.5 |
| Camera | +0.27 (own 0.31) | +1.13 (own 1.34) | +15.4 (own 14.7) |
| Per-Object | +0.27 (own 0.31) | +1.16 (own 1.37) | +15.6 (own 15.7) |
| Cinematic DOF | +0.28 (own 0.31) | +1.30 (own 1.39) | +10.4 (own 10.1) |

Own > Δ for the blur: the native speed blur it replaces is no longer drawn.
4K split (one run, both on): motion blur resolve ~0.2 (8x MSAA) +
velocity 0.30 + tiles 0.19 + gather 0.25; DOF resolve 0.20 + CoC / half
scene 0.35 + gather 0.57 + composite 0.17. Optimised in this work: tile max
on every other pixel (0.48 → 0.19 ms at 4K), DOF from a full-size per-pixel
turned gather (1.58 ms) to the half-size prefiltered one (0.57). On the iGPU
both cost as much as the scene: the help texts say so; a cheaper iGPU tier
(half-size velocity / gather) is open.

## Native byte-identity
Default values (`enh.motion_blur 0`, `enh.depth_of_field 0`): `blur_and_dof`
returns before any allocation (no `BlurDof`, no pass), the native speed
blur's call is unchanged. Parity gates (`perf.sh parity`, base = this copy
before this work, both fast-build binaries): **all gates passed**
(internal evidence): Original images of the
5 bench scenes at 1280×960 and 3840×2160 (70 frames), the window path at
144 Hz (40 frames), Modern / Ultra racing and explore (28 frames), 15 shots
— all AE 0; audio WAVs identical; state hashes identical over firstrace,
firstdrag, postrace, the playthrough (82 515 steps) and the parity scenario.

## Evidence (`docs/evidence/motionblur-dof/`)
See `docs/evidence/motionblur-dof/README.md` (crops, numbers, scripts).

## Tests (`render::blur_dof::tests`)
`shutter_is_the_same_at_any_display_rate` (a camera at 40 m/s turning
0.5 rad/s drawn at 60 / 144 / 240 Hz: the streaks through the pass's own
matrices agree within 1 %; pause and stall draw none),
`camera_tier_grows_with_speed`, `masks_and_per_object_motion` (player
masked in both tiers, no own motion / arcs in Camera, rival motion back to
its last pose, arcs held to 2.5 rad, fresh cars camera-only, road under a
car outside its box, cars behind the eye not sent),
`still_poses_come_from_the_velocities`, `dof_runs_in_scenes_and_keeps_the_car_sharp`,
`original_is_untouched_and_order_is_before_the_additive_layers` (early
return before any allocation, the native call site unchanged, the passes
after shadows / characters and before particles in the frame and the
shots), `graphics_rows_share_the_native_switches`; `qol` round trip (39
deviations), `options` row order.

## Tuning / measuring aids (not options)
`NFSU2_BLUR_DEBUG=1`: velocity (R, G = |x|, |y| / 32 px), masked red, wheel
arcs green; the DOF's circle of confusion (R) and near field (G).
`NFSU2_DOF_ALWAYS=1`: depth of field behind every camera (to time it).

## Open
* Wheel arcs: the axle is the car's y axis (front wheels' steering not
  applied); seen edge-on (chase camera) the arc is short on screen, so the
  crops show little; not verified against a side view in motion.
* Camera tier: rivals and traffic take the camera's motion (as specified),
  so a car keeping pace beside the player is blurred; Per-Object fixes it.
* Shots (`--shot`) have no camera rotation in their stand-in last frame.
* iGPU: ~10–16 ms per effect at 1080p; no cheaper tier yet.
* Glow sprites, particles and streaks are not blurred by DOF either (drawn
  after, by design: they must never ghost); in a defocused background their
  sprites stay crisp.
* Not compared with anything native (no native counterpart), so "looks
  right" is judged by eye only.

## Bright lamps and light sprites in the depth of field (2026-11-13)
In the posters 05–07 wall lamps showed as hard-edged white discs over the blurred background. They are the lamps'
own (emissive) meshes, blurred to their circle of confusion (with the depth of field off the same lamp is a small
square fixture), but in the HDR scene (HDR post on; off, the disc is dim and soft) the gather's bright-tap weight
`1 + 2 · max(l − 0.85, 0)` was unbounded: a lamp at 10 weighed 19 taps, and even a plain average spreads it over the
circle above white, so the whole circle clipped with a 1 px rim. Now the weight is that boost within the display
range only (at most 1.3) and falls over white as `1 / (1 + (l − 1))` (Karis' average). Poster 05's lamp (1080p, its
box): ≥ 250 pixels 872 → 287, edge gradient p99.5 30 → 17; the frame's mean −1.0 / p95 −1.5 (bokeh a little dimmer,
never brighter). Separately, the native **glow sprites** are drawn after the pass (into the scene or the light-streak
buffer) and stayed sharp: `blur_dof::defocus_quads` now spreads each out-of-focus sprite over the circle of
confusion at its depth (up to 12 copies on a Vogel disc in the sprite's plane, the light shared out, the quad buffer's
1000 respected); the pass hands its focus to `WorldRenderer::upload_glows` for that frame only (`take_sprite_dof`;
cleared by `reset` / `motion`). Under FSR 2 the blur runs after the sprites, so they take the previous frame's focus.
Nothing changes without the depth of field (scenes and photo mode only).
