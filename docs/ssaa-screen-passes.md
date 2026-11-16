# Effect and post passes at the screen's size under SSAA

The Graphics page's SSAA (docs/ssaa.md) draws the scene larger than the screen (2x at 3840×2160: 5431×3055,
twice the pixels) and resolves it to the screen at the end (`enhance.wgsl` `fs_resample`: every scene texel under a
screen pixel, weighed by the area it covers). Until this change every pass after the scene ran at the scene's size
too, also the ones whose result is low-frequency, or a per-texel grade followed by the average anyway. This page
lists those passes, what each reads and writes, and what moved to the screen's size.

Clone deviation of an enhancement (SSAA itself is opt-in). **Without SSAA nothing changes**: every pass below takes
its old path when the scene is not larger than the screen both ways (1×, Render Scale below 100 %, FSR 1, the
temporal stage); the shots without SSAA are byte-identical. Under SSAA the picture changes by small amounts
(picture report below). `NFSU2_SSAA_SCENE_SIZE=ssao`, `=post` or `=all` keeps the named passes at the scene's size
as before, for comparisons (`render::debug_env::ssaa_scene_size`; the old paths are kept).

## The passes

| pass | reads | writes | before (SSAA) | now (SSAA) |
|---|---|---|---|---|
| SSAO depth, occlusion, 4×4 blur (`ssao.rs`, `ssao.wgsl`) | the scene's depth | linear depth, occlusion (R32F / RG16F) | half the scene (2715×1527) | half the screen (1920×1080), the grid it has without SSAA (`ssao::Grid`) |
| SSAO apply | depth, blurred occlusion | multiplies the scene's colour | the scene | the scene: it must come before the additive layers (shadows, underglow, particles, glows), which the resolve is after |
| bleach bypass (High Precision Post, `fs_bleach_hdr`) | the image so far | screen copy 0 | each scene texel, written at the scene's size | each scene texel as before, inside the resolve (`fs_resolve`), which averages the graded texels under each screen pixel at once: no scene-size image written or read again |
| overbright glow's C | the bleached image | C and A at the glow's size (the screen's with Full-Size Glow) | the box of the scene under each glow texel (√7 glow texels, docs/post-scale.md) | native's 1× prefilter on the resolved image (the 3×3 tent; Half-Size Glow: one tap) |
| streak buffer (decay, glow sprites, A added, saturated) | itself, the glow sprites, A | itself | the scene | the scene (tried at the screen's size: below) |
| to the back buffer (`fs_out_add`) | image + streak buffer | the 8-bit output | the scene, then the resample | the screen (`fs_out_add_resolved`: the streak buffer's box under the pixel) |
| the resample to the screen (`graphics: render scale`) | | | 0.26 ms | gone |

Only with High Precision Post (the float chain): SSAA with the native 8-bit chain and with HDR Output keeps the
scene-size chain. The rain's lens drops and tint, the motion blur and the horizon fog stay before the resolve, at the
scene's size, as do SSR and the ray-traced reflections (their own passes).

What is the same and what is not, under SSAA:
* SSAO: the occlusion is sampled on the screen's half (the grid, the noise and the 4×4 blur in the screen's half
  texels, as without SSAA) and taken to the scene by the same depth-aware 2×2 upsample. Its reach and cap cover the
  same part of the screen as before.
* Bleach bypass: graded per scene texel, then averaged, as before (the order of the grade and the average is kept;
  bleaching the average instead was tried and changed the colour of edge pixels by up to 76 levels, below).
  `fs_resolve` reads the box's texels one by one (at most 3 × 3 at SSAA 2x and 4x, a fixed loop; texel by texel
  above) and grades each it covers.
* Streak buffer: added after the average instead of per scene texel, so where image + streak saturate on one side
  of an edge the edge pixel is brighter than the average of the saturated texels was.
* The dither of High Precision Post's last pass is made once per screen pixel instead of per scene texel and
  averaged; the 8-bit rounding happens once.
* The glow's prefilter is native's 1× one on the resolved image instead of the box of the scene: both are the same
  low-pass before the glow's blur.

## Tried, not kept

* **The streak buffer at the screen's size** (decay, the A add and the saturation on 8.3 instead of 16.6 MP; the glow
  sprites drawn at the screen's size against the scene's depth copied to that size, one texel per pixel): the
  buffer's passes 0.48 → 0.25 ms, but the depth copy costs 0.17 ms (a depth-writing full-screen pass over the
  scene's depth), net −0.05 ms, under the noise; and the sprites lose the supersampling at their depth edges.
* **The bleach bypass on the resolved image** (one grade per screen pixel; a prototype, not kept in the tree): edge
  pixels of saturated colours (neon signs, foliage) up to 76 levels off and a little more flicker at edges in motion
  (numbers below).
* **The resolve as a compute pass** (a tile of the scene graded once per texel into workgroup memory, each pixel its
  box of it): 1.31 ms against the fragment pass's (below): the graded tile's chain of dependent texture reads before
  the barrier leaves the GPU waiting; dropped.

## Measured

A 4K play setup's options.txt (3840×2160, SSAA 2x = a 5431×3055 scene, High Precision Post Full-Size
Glow, SSAO 100 %, ray-traced reflections and shadows, …), `--bench <scene> 150` offscreen on the reference desktop
GPU, the game's GPU timestamps per pass. One binary; the arms by `NFSU2_SSAA_SCENE_SIZE` (`all` = the frame as
before, byte-identical to the base's: checked on frames 0, 100, 101). 7 runs per arm (SSAO 9), interleaved in
blocks of 6–7 runs, each block one hold of the measure lock (under 2 minutes) with the desktop GPU idle before it;
each run paired with the runs of the frame as before in its block. Load average 11–47 (other units' builds).

| racing, ms per frame | before | SSAO at the screen's half | resolve first | both |
|---|---|---|---|---|
| SSAO occlusion / blur / apply | 0.46 / 0.075 / 0.34 | 0.18 / 0.045 / 0.33 | | |
| bleach bypass + glow (overbright, high precision) + to the back buffer + resample | 0.45 + 1.10 + 0.26 + 0.26 | | resolve 0.62 + 0.90 + 0.16 | |
| the passes' gain, every pair (min … max) | | −0.318 … −0.326 (9 pairs; their depth pass shares a timestamp with the scene's draws: that bucket ±0.02) | −0.384 … −0.397 (7) | −0.710 … −0.724 (7; GPU busy −0.63 … −0.94) |
| GPU busy median (spread of the run medians) | 19.19 (19.06–19.31) | 18.86 (18.71–18.96) | 18.88 (18.78–18.98) | **18.50** (18.19–18.59) |
| fps (GPU-bound) | 52.1 | 53.0 | 53.0 | **54.0** |

Both, the other scenes (2 pairs each): free roam 13.04 → 12.39 ms (76.7 → 80.7 fps), race start 20.26 → 19.58 (49.4 →
51.1), night rain 23.40 → 22.54 (42.7 → 44.4). Own VRAM peak within its run-to-run spread (racing 3286 → 3249 MiB: the
scene-size 8-bit image before the resample is gone, a screen-size half-float one added).

Tried: the streak buffer at the screen's size −0.058 … −0.065 ms (5 pairs; its passes −0.23, the depth copy +0.17);
grading the resolved average −0.59 ms (7 pairs; picture below); the compute resolve +0.55 ms (1 pair, resolve 1.31 ms).
Evidence: docs/evidence/perf-budget/ssaa-screen-passes.txt.

## Picture report

Every candidate against the frame as before (`all`), same deterministic frames: racing at speed (frames 100–107, 200),
night rain (drive at clock 162: 100–103, 200), the race start's scene (frames 20–23 with the crowd, 280–283 with the
camera moving), photo mode (below). 1 − SSIM (ImageMagick), share of pixels off by more than 2 / 8 levels (max channel), the
largest difference; motion: per pixel the frame-to-frame change of the image's high-pass (what shimmers or crawls),
its mean against the frame as before.

| candidate | 1 − SSIM | > 2 levels | > 8 levels | max | luma mean | motion (high-pass change, ratio; pixels more by > 2) |
|---|---|---|---|---|---|---|
| SSAO at the screen's half | 0.0001–0.0006 | 0.2–1.3 % | 0.005–0.07 % | 29–92 | ±0.01 | racing 0.9995, rain 0.9992, start 0.9995; 0.02–0.09 % |
| resolve first (graded per texel) | 0.0015–0.0020 (the dither, ±1 everywhere) | 0.02–0.14 % | 0.003–0.034 % | 55–99 | +0.01…+0.03 | racing 0.9980, rain 0.9963, start 0.9990; 0.006–0.02 % |
| both | 0.0016–0.0021 | 0.27–1.42 % | 0.015–0.094 % | 55–95 | +0.01…+0.03 | racing 0.9975, rain 0.9956, start 0.9985; 0.03–0.11 % |
| tried: grading the average | 0.0019–0.0024 | 1.6–2.4 % | 0.35–0.59 % | 60–106 | −0.08 | racing 1.0088, rain 1.0054, start 1.0109; 0.83–0.90 % |
| tried: streak buffer on screen | < 0.00005 | 0.003–0.12 % | < 0.03 % | 28–89 | 0 | 0.9995–0.9999 |

Photo mode (`--photo … race`: the race paused, photo mode's frame, its camera moved, a 3840×2160 picture): within the
same ranges, the resolve at most 4 levels off on photo mode's frame and 3 on the picture (the one larger difference
there is a file name in the help bar, which differs between any two runs).

Where they differ, at 100 % and 200 % (the crops stay with the measurements: game imagery is not kept in the tree):
SSAO, on the crowd's clothes and limbs and at contact edges (the occlusion's finer detail is the screen's half's now,
as without SSAA); the resolve, a one-pixel outline
around the brightest lights inside their glow (the streak buffer added after the average: the edge pixel up to
~90 levels brighter, where before the saturated texels were averaged), elsewhere only the dither. Neither changes
the light level (luma mean, p5, p95 within 0.3) or adds flicker in motion. A first SSAO version read the depth at
one texel of each footprint and placed it at another: on a wall at an angle the two grids' beat drew vertical
stripes (rain frame 100, 0.30 % of pixels > 8 levels); fixed before these numbers (`ssao::Grid`: the texel holding the
footprint's centre, its own position).

## Tests

* The occlusion's cheaper taps (docs/perf-integrated-gpu.md, "Ambient Occlusion: the same occlusion for less") keep
  this: on this grid each tap's place is its picked texel's (`picked_xy`), the multiply-add form only on the 2:1 grid.
* `ssao::tests::occlusion_grid_is_half_the_scene_without_ssaa_and_half_the_screen_with_it`: the grid and the
  shader's values are exactly native's 2:1 ones without SSAA (so every expression gives the old values bit for bit),
  half the screen under SSAA 2x / 4x / 16x, the reach and its cap the same part of the screen.
* `postfx::tests::only_ssaa_with_high_precision_post_resolves_first`.
* `postfx::tests::resolved_chain_matches_the_scene_size_chain` (GPU, run explicitly): a scene at exactly twice the
  screen. In 2 × 2 one-colour blocks: the resolved chain within 3 steps (mean about one: the dither) of the
  scene-size chain. Every texel its own colour: p99 2 steps, max 12 (the per-texel grade kept; a grade of the
  average fails this). Every last-pass variant (alpha 0, no overbright pass, no streaks, no bleach bypass) without a
  validation error.
* `validate_tests::postfx_text_is_the_old_file` re-pinned (new entry points only).
* The gate against the base: every image without SSAA identical (Original dumps 1280×960 and 3840×2160, the 144 Hz
  window path, the 31 shots, the enhanced Extreme FSR preset whose scene is smaller than the screen), audio, the
  state hashes and the rates identical; the enhanced Extreme preset (SSAA 2x, High Precision Post, SSAO) differs as
  above: 1920×1080 dumps, > 2 levels 0.1–0.55 %, > 8 levels 0.004–0.06 %, max 80, luma mean +0.02 (most pixels
  differ by the dither's one step).
