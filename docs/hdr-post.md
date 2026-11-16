# High Precision Post (clone deviation, opt-in)

Graphics page row **High Precision Post**: Off (default, native) /
Half-Size Glow / Full-Size Glow. options.txt `enh.hdr_post` 0 / 1 / 2
(written only when not 0). Wave G the High Precision Post work (design note). Nothing here
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
  (docs/ssaa-screen-passes.md).

Known difference (inferred, by design): inside the scene, additive draws
can now exceed 1.0 before a later blend over them (native saturates at
each write); the final clip removes it where nothing blends over it.

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
GPU (ignored, run explicitly): `postfx::tests::hdr_passes_build_and_run`
(every pass, both glow sizes, MSAA 4, alpha 0, no validation error,
formats back to native when off).

## HDR Output
docs/hdr.md: HDR Output forces this chain on (at least Half-Size Glow),
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
