# High Precision Post: how it works

The technical part of [High Precision Post](../../playing/settings/hdr-post.md): how it works, what it costs, how it was checked and what is open. The player part (the rows and what they do) is on that page.

## Overview

Graphics page row **High Precision Post**: Off (default, native) /
Half-Size Glow / Full-Size Glow. options.txt `enh.hdr_post` 0 / 1 / 2
(written only when not 0). The High Precision Post work. Nothing here
is native or traced; it is a cleaner rendering of the traced chain in
docs/formats.md "Screen filters".

## Why
SPEED2.EXE's end-of-frame chain (0x5CAAE0 from 0x5CB778; targets made by
0x5BB850, all A8R8G8B8) runs on 8-bit targets: the scene (back buffer),
two screen copies, the streak buffer and three half-size targets A/B/C.
Each pass rounds to 8 bits (bleach bypass three times, the streak decay
every frame, the glow blur four times at half size), which bands smooth
gradients (night sky, fog, glow halos), and the glow is built at half size
with filter.efx's ±1/±3-texel taps (no centre tap), which leaves blocky,
combed blooms. The clone's native path reproduces that exactly (and stays
the default).

## What the option changes (`render::postfx`, postfx.wgsl `*_hdr`, `fs_out*`)
* The scene, its multisampled copy, the two screen copies, the streak
  buffer (and its multisampled copy) and A/B/C are `Rgba16Float`
  (`render::HDR_FORMAT`). Every scene pipeline follows through
  `SceneMode::hdr` / `SceneMode::target` (world main view, shadows,
  particles, glows into the scene and into the streak buffer, characters,
  cars, horizon fog, SSAO's apply). The road reflection, mirror, cube and
  cover/garage reflection targets keep their native 8-bit formats.
* Same passes, same constants (overbright level/offset/add, subtract
  0xB0/0xA0, streak decay 0xFF785AAA, gain and ramp maps, tint). Wherever a
  native 8-bit write saturates, this path saturates too: the bleach
  passes' hand-offs (`fs_bleach_hdr`, the three passes in one), C, A − C·k
  held at 0, and the streak buffer after its adds (`fs_one` with
  BLENDOP_MIN), and A = overbright(C), whose (x·g)² reaches 3.1 at white
  (missing it first made the glow +0.05 brighter). Only the rounding goes.
* Glow: C/A at half size (Half-Size Glow: one bilinear tap, as native) or
  at the screen's size (Full-Size Glow: a centred 3×3 tent, four bilinear
  taps at ±0.5 px). The two rounds of the native 4-tap blur become one
  separable Gaussian (across, then down + the subtraction in the same
  pass) with the same variance: σ = 2.708 half-size texels
  (2 rounds × 11/3 texel²), 5.454 px at the screen's size (the native
  2×2 box and bilinear upsampling included, the tent taken off). Same
  energy, same spread, no comb. The streak buffer adds A bilinearly.
* Back to 8 bits in the last pass (`fs_out`, `fs_out_add`, `_a0`): clamp
  to [0, 1] per channel, as the native image saturates (no curve: a curve
  would be a regrade; "tonemap" here is NFSU2's own clip), plus a fixed
  per-pixel triangular dither of ±1 step, left out where a channel is 0 or
  1 (black and white stay clean; shots stay deterministic).
* The Graphics page's passes after it (FXAA, SMAA, CAS, SSAA/Render Scale
  resampling) read that 8-bit image as before. Under SSAA the scene is
  resolved to the screen's size first and this last pass runs there
  (docs/notes/graphics/ssaa.md "Effect and post passes at the screen's size under SSAA").

Known difference (inferred, by design): inside the scene, additive draws
can now exceed 1.0 before a later blend over them (native saturates at
each write); the final clip removes it where nothing blends over it.

## Blur where it can be lit (`post/glow_skip.rs`, glow_skip.wgsl, glow_need.wgsl)
Exact: the same shaders draw every texel they drew before; a texel left
out is one whose every tap is 0, so its result is the 0 its target holds.
* The prefilter's entries (`fs_overbright_mrt_hdr_flag`, `fs_overbright_sat_flag`:
  the plain entries' functions, and a flag) mark each 8 x 8 tile of the
  glow where A can be other than 0: where a channel of overbright's input
  is above OVERBRIGHTOFFSET (at or below it, x and so A are 0). The flag
  is taken from the input, not from A: testing A made the driver keep A
  at full precision and write it with another rounding (one half-float
  step on a few texels; found by comparing the shaders' machine code).
  Which entry runs follows the glow's source: at 1× and under SSAA (the
  image resolved to the screen's size first, docs/notes/graphics/ssaa.md "Effect and post passes at the screen's size under SSAA")
  the 1× prefilter `fs_overbright_mrt_hdr_flag` on that image; with a
  scaled glow (Render Scale, FSR) `fs_box_hdr` into C, then
  `fs_overbright_sat_flag` on C.
* Two small dispatches widen the marks by the blur's reach (`reach`: the
  farthest texel `gauss` reads, 10 at Half-Size, 18 at Full-Size; in tiles
  `reach_tiles`, 2 / 3): the pass across can be other than 0 only within
  that many tiles across of a marked tile (its taps sit on the texel's own
  row; the rows beside it are fetched at a weight of 0 at most), the pass
  down within that many down of those.
* One quad per tile writes the need into a Depth16 mask (0 both passes,
  0.5 the pass down only, 1 neither, cleared to 1); the two blur passes are
  drawn at depths 0.25 and 0.75 against it (compare Greater, vertex shaders
  `vs_glow_across` / `vs_glow_down`: `vs_main`'s triangle and uv at another
  depth). B is cleared before the pass across and A before the pass down
  (after the pass across read it): where the pass down does not run, A is
  0 (every such tile is outside the marked ones, where the prefilter's A
  is 0 too).
* Cost (docs/evidence/perf-budget/glow-blur.md): the play setup at 4K
  (SSAA 2x: the glow's source resolved to the screen's size first) 0.92 →
  0.53 ms racing, 0.98 → 0.74 ms in the night rain; the integrated GPU at
  1920x1080, High (Full-Size) 9.5 → 5.3 ms, Medium (Half-Size) 1.56 → 1.03
  ms. The two clears are cheap with the driver's colour compression (DCC,
  RADV's default: a clear writes its metadata only); with it off
  (`RADV_DEBUG=nodcc`, the parity gates' setting for their pictures) they
  write the whole targets and the gain at 4K shrinks to about 0.1 ms or
  none. Open: at Medium on the
  integrated GPU the streak buffer's add after it measures about 0.07–0.10
  ms slower with this path, however A is written (cleared, kept, zeros,
  or whole again); not explained.
* The marked tiles in the racing bench: 12–44 % of the Full-Size glow at
  3840x2160, 17–28 % at 1920x1080; the pass across ran on 23–51 %, the pass
  down on 35–68 % of the tiles (racing, rain, free roam).
* Devices without storage writes in a fragment shader or storage reads in a
  vertex shader (`glow_skip::supported`) keep the full passes.
* Tried and dropped (docs/evidence/perf-budget/glow-blur.md): the blur in
  one compute dispatch through workgroup memory (the Gaussian's taps as
  loads and multiply-adds instead of the bilinear pairs), separable or as
  2D tiles, with the prefilter in the kernel or read: slower on both GPUs
  (the bilinear pairs take two texels a fetch for free); 4 x 4 tiles (a
  tighter need, but its widening and quads cost more than they save).

## Fallbacks
`render::hdr_supported` (at device creation): `Rgba16Float` must be
renderable, blendable, filterable and resolvable at every MSAA count the
page offers; else `qol::hdr_active()` is false and the native path runs
(the row still shows the choice). WebGPU core guarantees this format for
Vulkan, DX12 and Metal at 1× and 4×.

## Native byte-identity
With the row Off nothing changes: same targets, pipelines and passes
(`SceneMode::hdr` false, `Targets::hdr` None; only the uniform buffer grew
by one unused 256-byte slot). md5 of the shots against <base>: see
docs/evidence/hdr-post/README.md.

## Cost (`--bench explore 600`, reference desktop GPU, Vulkan, gpu ms/frame)
1080p: Off 0.64, Half 0.92, Full 1.09. 4K: Off 2.28, Half 4.37, Full 5.07
(half-float scene bandwidth ~+0.6, bleach +0.46, glow +0.77, streak
buffer passes +0.8). Breakdown in the evidence README.

## Tests
`qol` round-trip / range (`enh.hdr_post`), options row order and step,
`scene_mode_tests` (format mapping), `hdr_sigma_matches_the_native_blur_variance`;
`glow_skip::tests` (the reach is `gauss`'s farthest texel; the need covers
every texel the blur can light, random glows against the exact supports;
the flagging and tested passes are the plain ones);
GPU (ignored, run explicitly): `postfx::tests::hdr_passes_build_and_run`
(every pass, both glow sizes, MSAA 4, alpha 0, no validation error,
formats back to native when off).

## HDR Output
docs/playing/hdr.md: HDR Output forces this chain on (at least Half-Size Glow),
grades through the captured 32³ LUT (`fs_grade_lut`) instead of
`fs_bleach_hdr`, and its last pass (`fs_out_hdr*`) also writes the
unclipped image for the display transform.

## Open
* Not compared with anything native (nothing to compare: a deviation).
  Judged against the clone's native shots side by side.
* Scene-internal blends over values > 1 (see above) are not emulated.
* A curve-based tonemap was left out on purpose (native look); the
  half-float range is only used for precision.
* The streak buffer's decay and saturation are two full-screen passes;
  fusing them would cut ~0.3 ms at 4K.
