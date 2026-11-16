# Post effects under SSAA / Render Scale (2026-11-04)

The Graphics page's SSAA (2x..16x) and Render Scale (50–200 %) draw the 3D scene at a size other than
the screen's; the post passes run on that scene, then `enhance.rs` resamples it to the screen. Every
pass that measures taps in texels then covers a different part of the screen than native at 1×.
Rule here: **each post effect covers the same part of the screen whatever the scene's size**; at 1×
(scene = screen) nothing changes (Original and High Precision Post 1/2 shots: AE 0 against the build
before, garage_neon, neon_car, explore, qrstart).

## Audit (pass · unit of its taps · status)
| pass | taps / radius | under SSAA / scale |
|---|---|---|
| overbright glow 0x5CCA10 (filter.efx ±1/±3 pair 0x7FF1CC / 0x7FF19C, two rounds; C/A/B half-size targets 0x5BB850) | half-size *scene* texels | **was** 2–4× narrower on screen, harder edge. **Fixed** |
| High Precision Post blur (σ `HDR_SIGMA_*`, docs/hdr-post.md) | glow texels | same issue. **Fixed** (same targets) |
| CAS sharpen (enhance.wgsl) | output pixels (runs after the resample) | scale-free; dark ring at bright glow. **Fixed** (limiter) |
| SMAA / FXAA | output pixels (after the resample) | scale-free, unchanged |
| motion blur 0x5CC470 | uv shifts (`motion_blur.rs`) | scale-free, unchanged |
| light streak buffer 0x5CBB10, glow sprites, lens drops | per-frame decay; geometry | scale-free, unchanged |
| SSAO reach cap (`MAX_PIXELS` 48 half texels) | scene texels | cap now × scene/screen height. **Fixed**; its 4×4 blur stays in scene texels (finer noise, averaged again by the resample). Under SSAA (scene larger than the screen) the occlusion and its blur are now on the screen's half, the grid of 1× (docs/ssaa-screen-passes.md); Render Scale below 100 % as here |
| SSR trace size / denoise radius | trace = the screen's size or half (was the scene's) / `trace height / 540` | scale-free |
| wet-road roughness blur (world.wgsl) | texture LOD / world space | scale-free, unchanged |
| lamp shadow soft taps (`lamps.rs`) | shadow-map texels, world-metre offsets | scale-free: checked in the reflections pass (docs/evidence/s2-reflections/README.md) |

Under SSAA with High Precision Post the scene is now resolved to the screen's size before the passes after the
bleach bypass (docs/ssaa-screen-passes.md): the glow's C is then native's 1× prefilter on the resolved image, and
the box prefilter below serves Render Scale, FSR 1 and the 8-bit chain under SSAA.

## Glow fix (postfx.rs `glow_size`, `box_prefilter`, `ensure_glow`; postfx.wgsl `fs_box*`)
When scene ≠ screen the glow targets A–C are made at the size native makes them **for the screen**
(screen/2; the screen itself for High Precision Post 2), so the ±1/±3 taps and σ, in glow texels, span
the same screen distance at every scale; their uv offsets are those of 1× (test
`scaled_blur_spans_the_same_part_of_the_screen`). C, native's one bilinear tap = the 2×2 box of the
screen's texels, becomes the box of the scene's texels under the same screen area: `n`×`n` bilinear
taps (`n` = ⌈2s/2⌉, ≤ 8), exact for whole s (test `box_prefilter_is_the_screens_2x2_box_at_whole_scales`;
render scale 50 %: the texel itself). HDR 2's 3×3 tent becomes a box of the same variance (√7 glow
texels, `HDR_FULL_BOX`). Then A = overbright(C) in its own pass (the fused MRT pass is kept for 1×).
The glow is added into the scene-size streak buffer through the bilinear sampler, as native's half-size
A is. Cost: the glow chain now runs on the screen's half instead of the scene's (16x: 1/16 the texels).
Inferred, not measured on native: the composite of box-downsample → native chain equals this chain up to
8-bit rounding and the clamp order (the add saturates at the scene's size, then the resample averages).

## Sharpen limiter (enhance.wgsl `fs_cas`)
Where the 3×3 neighbourhood's peak luma is bright (smoothstep 0.35 → 0.7) the result is held at or above
the neighbourhood's minimum (no undershoot): the negative lobe no longer draws a dark ring beside glow,
lamps, headlights; elsewhere CAS is unchanged. Clone deviation (Sharpen is opt-in).

## Results (1280×960 `garage_neon`, column x 700, y 776..800 step 3; internal evidence: post-scale/profiles.txt)
| config | before | after | 1× reference |
|---|---|---|---|
| SSAA 4x | 216 101 92 72 40 17 | 211 95 83 71 45 21 | 212 94 84 71 45 21 |
| SSAA 16x | 217 102 96 77 38 17 | 211 95 83 71 46 21 | same |
| Render scale 50 % | 193 86 78 69 52 29 | 211 95 84 71 47 22 | same |
| HDR 2 + SSAA 16x | 216 102 96 77 38 17 | 211 96 84 70 45 21 | HDR 2 1×: 211 96 84 70 45 21 |
| all rows max (16x, HDR 2, sharpen 100, …) | 217 101 92 69 35 16 | 212 93 81 62 42 20 | same rows at 1×: 210 93 82 71 43 19 |

Low-frequency difference to 1× (both blurred σ 6, magick RMSE in 16-bit units, garage crop 700×260+300+560):
SSAA 4x 597→123, 16x 831→122, 50 % 1243→292, HDR 2 + 16x 824→115, max vs the same rows at 1× 817→142.
Whole frames (1280×720, RMS grey): neon_car (`NFSU2_SHOT_ORBIT=60,5,1.8`) 16x 1.03→0.76, 50 % 1.17→0.90;
explore (lamps, headlights, motion blur at 27 mph) 16x 1.07→0.60; qrstart (race start, tail lights)
16x 1.41→0.81. Sharpen 100 at 1×: pixels beside bright pushed > 8 below the 1× 3×3 minimum 2444 → 151.

## Open
* Light streaks at speed: the streak buffer and its decay are scale-free by construction; no at-speed
  race shot was taken (qrstart's frames are before the green light).
* No native frame at SSAA exists (native has none); the reference is the clone's own 1× frame.
