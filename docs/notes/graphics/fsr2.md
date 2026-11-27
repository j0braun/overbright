# FSR 2 temporal upscaling: how it works

The technical part of [FSR 2 temporal upscaling](../../playing/settings/fsr2.md): how it works, what it costs, how it was checked and what is open. The player part (the rows and what they do) is on that page.

## Overview

Nothing here exists in SPEED2.EXE (no temporal pass, no upscaler): nothing
was traced and nothing is native. Off by default; with it off the image is
The temporal tier's / the native one (gate: Original byte-identical, below).

**Third-party code:** AMD FidelityFX Super Resolution 2, v2.2.1
(https://github.com/GPUOpen-Effects/FidelityFX-FSR2, tag v2.2.1, MIT,
Copyright (c) 2022-2023 Advanced Micro Devices, Inc.); full notice in
THIRD-PARTY-NOTICES.md. Ported to WGSL in `crates/ob-render/src/render/fsr2.wgsl` (its
header names the source files per pass), driven by `crates/ob-render/src/render/enh/post/fsr2.rs`.

## Rows
| row | values | options.txt |
|---|---|---|
| **Upscaling** (Performance) | … / FSR 2 Native AA (1.0) / FSR 2 Quality (1.5) / FSR 2 Balanced (1.7) / FSR 2 Performance (2.0) / FSR 2 Ultra Performance (3.0) | `enh.upscale 7..11` |

No new row, so **no new label ids** (the tiers live in the existing
Upscaling row, label 0xC1_04E0_55). FSR 1 (4..6) and the temporal tier's TAAU (1..3) stay.
Under an FSR 2 mode the **Anti-Aliasing row is greyed** and reads
"FSR 2 (Upscaling)": SSAA / MSAA / FXAA / SMAA are off (one sample, as
under TAA; `qol::msaa_choice`, `temporal_for`); the MSAA Override and AA
Post Filter rows are greyed as under TAA; Render Scale reads "Upscaling
FSR 2 …" and is held. The Sharpen row (CAS) still applies after it.
`qol::is_fsr2`, `fsr2()`, `is_temporal_upscale`; render sizes are FSR 2's
(each side divided by the ratio, truncated: 4K Quality 2560 × 1440,
Balanced 2258 × 1270, Performance 1920 × 1080, Ultra Performance
1280 × 720), jitter phases FSR 2's (8 × ratio², Halton 2,3: 18 / 23 / 32
/ 72; Native AA 8), texture LOD bias FSR 2's (log2(render/display) − 1:
−1, −1.5, −1.75, −2, −2.5 levels).

## Where it runs
On the temporal tier's temporal path (docs/notes/graphics/temporal.md): the game's world only, the scene
drawn jittered at the render size in the upscaling stage
(`PostFx::set_upscale`, also at 1.0 for Native AA), its pre-passes (SSAO,
SSR, volumetric fog), then `Temporal::run`, which for an FSR 2 mode calls
`Fsr2::run` instead of the temporal tier's resolve; the late sprites (particles, smoke,
spray, rain, glow sprites, streaks), the native post chain (bleach,
overbright glow, rain tint) and the HUD come after it at the display size,
unjittered — so particles / smoke / spray / rain / glows never enter the
history and need no reactive mask. Cuts, history keys and the per-car
rigid motion are the temporal tier's. Front end / garage: native MSAA, as TAA.
**Motion blur and depth of field (the clone's the motion blur and DoF work tiers) run after the
upscaler** (FSR 2's guide): on the display-size image with the
display-size depth (`fs_depth`) and the unjittered camera, before the late
sprites — the same place in the chain relative to the additive layers as
before (`blur_after_upscale`, game.rs). The native motion blur
(0x5CB31F, drawn after the world and before the cars) keeps its native
place under every mode; TAA / TAAU / FSR 1 / no upscaling are unchanged.

## Passes (per frame; compute 8 × 8 unless noted)
| pass | size | what (FSR 2 file) | 4K Quality ms |
|---|---|---|---|
| animation mask (render pass) | render | the main view's flip-book / UV-scroll / water draws (and the wet road) again with their own depth, each scaled to its (reactive, composition) values (below) | 0.015 |
| snapshot / composition | render | the scene before SSR, then what SSR changed: the transparency-and-composition mask (clone; FSR 2's T&C semantics) | ~0.05 (with SSR) |
| motion | render | each pixel's motion (uv) from the depth and the drawn cars' poses (the temporal tier's reprojection), + reactive (spinning wheel 0.5, car without a last pose 1, the animation mask) and the T&C mask (SSR's composition, the animation mask) | 0.10 |
| reconstruct | render | nearest depth 3 × 3, dilated motion, previous depth scattered (atomic max into a buffer), lock luma (`reconstruct_dilated_velocity_and_previous_depth.h`) | 0.22 |
| luma | render / 32 | shading-change luma: mean log luma per 32 × 32 (FSR 2's SPD mip 4) | 0.02 |
| depth clip | render | disocclusion, prepared YCoCg colour, dilated reactive / T&C masks with motion divergence (`depth_clip.h`) | 0.28 |
| lock | render | thin-feature locks (`lock.h`) | 0.05 |
| accumulate | display | Lanczos 2 upsample with FSR 2's kernel bias, Lanczos 2 history with deringing, rectification, luma instability, temporal reactive factor, lock status (`accumulate.h`, `upsample.h`, `reproject.h`, `postprocess_lock_status.h`) | 0.88 |
| output (fragment) | display | RCAS (0.25 stops) into the scene, then the display-size depth for the late sprites | 0.35 |

Total ≈ 1.9 ms at 4K Quality on the reference desktop GPU (`NFSU2_BENCH_JSON`
gpu_parts, racing, play setup). Optimisations measured on the way:
16-bit motion textures and one 3 × 3 motion load shared by divergence and
masks (depth clip 0.93 → 0.28 ms), FSR 2's approximate Lanczos for the
history (no change, kept), 8-bit masks and luma history (accumulate
1.08 → 0.88); RCAS and the depth written in one pass measured slower
(0.43 vs 0.35 ms: shader-written depth) and was dropped.

## Deviations from FSR 2 (all from the clone's inputs; measured)
* **Motion vectors** are made here (the renderer writes no velocity
  buffer): The temporal tier's depth + per-car rigid reprojection, verified by
  `temporal::tests` (0.05 px).
* **No auto exposure** (exposure 1; the scene is display-referred; with
  High Precision Post the float scene uses FSR 2's HDR path, tonemapped
  accumulation).
* **Locks on** (`fsr2::LOCKS`, `NFSU2_FSR2_LOCKS=0` off). This page had
  them off: NFSU2's animated textures carried no mask and locks held their
  old frames as dotted chevron trails in a still view
  (`locks_ablation_static.webp`). The animation mask (below) now keeps
  locks off them (FSR 2's T&C semantics); with it locks measure +0.002
  SSIM on the full still frame, +0.007 on the thin trees (below).
* **Box scale capped at 2** (`BOX_SCALE_MAX`; FSR 2 up to 20, 11.4 at
  Quality): the looser box let the same flip-books smear.
* **History-disagreement reactive** (`AUTO_REACTIVE` 4): history whose
  luma lies outside the current 3 × 3's range by more than a quarter of it
  raises the reactive factor (≤ 0.9): animated textures converge as fast as
  under TAA's clamp; converged static history (a mean of such samples) is
  inside the range and keeps full accumulation
  (`chevrons_motion_250pct.webp`).
* 32-bit / 16-bit textures where FSR 2 uses packed 16/8-bit formats
  (WebGPU storage formats); new locks and the reconstructed depth in
  storage buffers.
* The scene's alpha (NFSU2's bleach mask) is accumulated alongside (lock
  status' third channel).

## Animation mask
FSR 2 asks the application to flag animated textures (reactive /
transparency-and-composition masks); this work had none and the chevron
barriers (on every race route) showed an inner double line in motion and
a stipple at Performance. The world renderer knows those draws
(`WorldRenderer::anim_mask`, `render/mod.rs` `AnimMask`):
* **which draws**: `TextureRef::Animated` (flip-books: signs), textures
  with a UV scroll of kind 1 or 2 (`scroll_params`; the chevron barriers
  are kind 2, speed (0, −0.78), a step every 0.2 s — 12 frames at 60 Hz),
  the water textures (`water::kind_of`, also when Animated Water draws them
  itself), and the road (`Effect::Reflect`) while Wet Roads has wetted it
  (the ripples). Drawn after the main view, the main view's order and
  vertex shaders (`vs_main` / `vs_reflect`, same jitter and scroll), into an
  Rg8 target with a depth of its own (FSR 2 frames only; nothing is made
  otherwise).
* **values** (`anim_values`, the pipeline's blend constant scales the
  fragment's coverage): a texture that **stepped since the last drawn
  frame** (`scroll_motion`, `flip_stepped`; time = the frame uniform's
  clip.y, the same the shader scrolls by) reactive **1** and T&C 1: its
  history is the old frame of the texture and is replaced; **between
  steps** reactive 0, T&C 1 (it accumulates — the anti-aliasing comes
  back — but stays out of locks, the wide box and the luma instability);
  continuous scrolls and water reactive 0.5, T&C 1; the wet road T&C 0.5
  only. Coverage: the texel's alpha (alpha-tested: 1); an additive draw
  between its steps the light it adds (`fs_anim_mask_light`, so the
  barrier's black panel, through which the scene shows unmoved, keeps
  its history); at a step the whole panel (the old chevron's light is
  gone too).
* **visibility** (fsr2.wgsl `animation`): the mark holds where the mask's
  surface is not behind the final scene's (1.5 % + 0.3 m): a car in front
  of a barrier is not marked; the blended barrier (no depth in the scene)
  over what shows through it is.
* the masks go through FSR 2's own dilation (depth clip,
  PreProcessReactiveMasks); the history-disagreement reactive and the box
  cap (below) stay (without them the full frame measured worse: FLIP
  +0.002 / +0.007).

## Results (reference desktop GPU, 4K, the play setup + Modern Lighting 2)
Play setup = the enh lines of perf.sh's `user` preset (SSAA 2x, SSAO 25, HP Post 2, SSR 2, lamp shadows 2, volumetric
Low, …) + `enh.modern_lighting 2`; `--bench racing` (`scripts/capture.sh`).

### Cost / benefit
GPU busy median ms/frame, `--bench racing 600`, 4K, play setup (one
run each, `capture.sh`; racing is the heavy case):

| config | ms | vs SSAA 2x (play setup) |
|---|---|---|
| SSAA 2x (play setup) | 11.82 | — |
| SSAA 4x (reference) | 19.84 | +8.0 |
| TAA (native res) | 7.91 | −3.9 |
| TAAU Quality (temporal tier) | 5.23 | −6.6 |
| FSR 1 Quality | 4.71 | −7.1 |
| FSR 2 Native AA | 10.95 | −0.9 |
| **FSR 2 Quality** | **6.94** | **−4.9 (−41 %)** |
| **FSR 2 Balanced** | **6.20** | **−5.6 (−48 %)** |
| FSR 2 Performance | 5.56 | −6.3 |
| FSR 2 Ultra Performance | 4.67 | −7.2 |

Against the Original (native 8x MSAA) on the light case, `--bench explore
600` at night, 4K, mean of 2 runs (`cost.tsv`; the Upscaling row's cost
column): Original 2.30, TAA 1.49, TAAU Q 1.42, FSR 2 Native AA 3.90
(+1.6), Quality 2.81 (+0.5), Balanced 2.59 (+0.3), Performance 2.40
(+0.1), Ultra Performance 2.19 (−0.1).

### Quality
SSIM / FLIP (NVIDIA FLIP, LDR) against **SSAA 4x** (the best reference
the reference desktop renders at 4K), HUD masked; edge energy = Laplacian energy
relative to the reference (>100 % = sharper than it: RCAS and the LOD
bias). Static = car and camera frozen at frame 300, frame 480 (180 frames
of accumulation); motion = racing frames 450 and 600 (`scripts/metrics.py`).

| | static SSIM / FLIP / edge | motion f600 | motion f450 |
|---|---|---|---|
| SSAA 2x | 0.988 / 0.018 / 70 % | 0.991 / 0.016 / 74 % | 0.987 / 0.019 / 71 % |
| TAA | 0.978 / 0.028 / 52 % | 0.983 / 0.028 / 72 % | 0.969 / 0.035 / 62 % |
| TAAU Quality | 0.955 / 0.039 / 39 % | 0.965 / 0.038 / 54 % | 0.955 / 0.041 / 42 % |
| FSR 1 Quality | 0.953 / 0.035 / 87 % | 0.964 / 0.030 / 83 % | 0.952 / 0.036 / 83 % |
| FSR 2 Native AA | 0.978 / 0.027 / 155 % | 0.983 / 0.028 / 106 % | 0.973 / 0.032 / 100 % |
| FSR 2 Quality | 0.974 / 0.031 / 113 % | 0.967 / 0.040 / 91 % | 0.957 / 0.044 / 87 % |
| FSR 2 Balanced | 0.971 / 0.035 / 103 % | 0.960 / 0.044 / 95 % | 0.948 / 0.048 / 94 % |
| FSR 2 Performance | 0.966 / 0.041 / 91 % | 0.949 / 0.050 / 112 % | 0.942 / 0.057 / 89 % |
| FSR 2 Ultra Perf. | 0.946 / 0.068 / 53 % | 0.925 / 0.071 / 101 % | 0.910 / 0.088 / 87 % |

Reading: FSR 2 Native AA equals TAA on SSIM / FLIP and is far sharper
(TAA keeps 52–72 % of the reference's edge energy, FSR 2 Native AA ≥ 100 %;
in a still view 155 %: RCAS + the −1 LOD bias, some of it over-sharpening).
FSR 2 Quality: in a still view close to TAA (0.974 vs 0.978) and far above
TAAU / FSR 1 Quality; **in motion** its SSIM is at TAAU / FSR 1 level
(0.967 / 0.957 vs 0.965 / 0.955) and its FLIP is *worse* than both
(0.040 / 0.044 vs FSR 1 0.030 / 0.036): it keeps the reference's sharpness
(87–113 % vs TAAU 39–54 %) but carries more temporal error on the animated
chevron barriers (FLIP counts it). Caveat: the reference is a single
supersampled frame, which penalises every temporal method in motion. SSAA
2x stays the best image (2× the pixels). Crops: `static_4k_100pct.webp`,
`chevrons_motion_250pct.webp` (motion, 250 %: SSAA 2x, TAA, TAAU Q,
FSR 2 Native / Quality / Performance); ghosting: `ghost_rain_nis_1080p.webp`
(rival passing at speed frame 880, rain clock 170, race-start NIS with
cuts: no trails, no doubled images, no reset artefacts after cuts).

### Honest assessment
* FSR 2 Quality replaces the player's SSAA 2x at 59 % of its GPU time
  (−4.9 ms at 4K racing: ~85 → ~145 fps GPU-bound) with a sharper image
  than TAA / TAAU; it is **not** SSAA 2x quality (SSIM 0.974 vs 0.988
  still, 0.967 vs 0.991 in motion) and in motion it is not better than
  FSR 1 / TAAU on FLIP. FSR 2 Native AA is the quality option (TAA's
  stability, ≥ reference sharpness) at −0.9 ms vs SSAA 2x.
* Remaining artefacts (before the animation mask): the far chevron
  barriers in motion kept a faint inner double line at Quality and below
  (250 % crop; fixed by the FSR 2 masks pass, below); Ultra Performance is
  visibly soft (as AMD documents) and Performance shows a visible
  stipple on the animated chevrons in motion: Quality / Balanced are the
  recommended modes; Native AA is clean.
* On light scenes (explore Original) FSR 2's fixed ~1.9 ms makes it no
  cheaper than the Original's 8x MSAA at 4K: it pays off against SSAA and
  heavy settings, not against the native look.

### Animation mask, locks, motion blur / DOF after the upscaler
Against **SSAA 4x** at the same frames (<base> binary "before", the FSR 2 masks pass
"after"; the play setup, `--bench racing`, 4K; NVIDIA FLIP LDR;
`scripts/d14b/`). Motion = the mean over two consecutive-frame windows
(frames 440–463: far chevrons ~70 m ahead, two texture steps; 590–600:
near barriers), crops of the barriers; temporal = mean |(X_t − X_t−1) −
(R_t − R_t−1)| ×255 in the crop (change the reference does not have:
ghost trails, flicker). Still = the camera frozen at 300, frame 480 /
window 460–480. 54 m/s = Skyline, frozen at 1350 and the car and camera
moved 0.9 m a frame (a racing-speed pass of a barrier), window 1351–1374.

| | far chevrons SSIM / FLIP / temporal | near chevrons | full frame f450 / f600 SSIM / FLIP |
|---|---|---|---|
| Quality before | 0.855 / 0.165 / 12.5 | 0.915 / 0.105 / 6.6 | 0.9456 / 0.0524 · 0.9545 / 0.0498 |
| **Quality after** | **0.880 / 0.135 / 10.8** | **0.923 / 0.099 / 5.9** | 0.9457 / 0.0521 · 0.9557 / 0.0487 |
| Balanced before → after | 0.829 / 0.177 / 13.6 → 0.860 / 0.142 / 11.7 | 0.899 / 0.113 → 0.908 / 0.107 | |
| Performance before → after | 0.789 / 0.196 / 15.2 → 0.829 / 0.158 / 12.9 | 0.874 / 0.125 → 0.886 / 0.118 | |
| Native AA before → after | 0.926 / 0.114 / 8.7 → 0.928 / 0.110 / 8.4 | 0.962 / 0.065 → 0.962 / 0.064 | |
| Quality + MB Per-Object + DOF before → after | 0.868 / 0.162 / 11.8 → 0.893 / 0.130 / 10.0 | 0.915 / 0.106 → 0.924 / 0.099 | 0.9510 / 0.0505 → 0.9558 / 0.0497 · 0.9612 / 0.0473 → 0.9656 / 0.0459 |

| Quality | still full SSIM / FLIP / temporal | still chevrons | thin trees | 54 m/s chevrons SSIM / FLIP / temporal | 54 m/s frame |
|---|---|---|---|---|---|
| before | 0.9655 / 0.0392 / 0.19 | 0.801 / 0.183 | 0.933 / 0.041 | 0.898 / 0.109 / 6.7 | 0.9477 / 0.0554 / 3.94 |
| after, locks off | 0.9661 / 0.0386 / 0.21 | 0.866 / 0.130 | 0.933 / 0.041 | | |
| **after (locks on)** | **0.9681 / 0.0379 / 0.20** | **0.873 / 0.126** | **0.940 / 0.040** | **0.916 / 0.094 / 6.0** | 0.9485 / 0.0548 / 3.91 |

Reading: at a step the old chevron frame is gone in the next drawn frame
(`d14b_chevron_step_frames_200pct.webp`: before, the step at frame 441
blends over three frames — the "double line"; after, the new phase at
once, then it re-converges in ~4 frames); in motion and at 54 m/s the
inner double stroke is gone (`d14b_chevrons_far_motion_300pct.webp`,
`d14b_chevrons_54ms_200pct.webp`). The still chevrons' temporal number
rises (1.4 → 2.1): the steps now show at their frame (the reference has
them) instead of smeared, and a just-reset texture is aliased for ~4
frames at 5 Hz — not visible as flicker at 100 / 200 % in the windows
(judged frame by frame). Motion blur / DOF after the upscaler: the dark
jagged silhouette of the blurred car roof (blur of the jittered
render-size image, then accumulated) is gone
(`d14b_motion_blur_dof_after_upscale_100pct.webp`); +0.005 SSIM on top of
the mask's +0.001. Look (rules; Rec.709 luma, HUD masked; reference
SSAA 4x / before / after): racing f600 mean 43.3 / 42.9 / 43.0, p5
13.1 / 13.0 / 13.0, p95 128.0 / 125.2 / 126.3; still 68.1 / 67.6 / 67.7,
p5 16.0 / 15.5 / 15.4, p95 197.6 / 197.3 / 197.3; 54 m/s 56.9 / 56.7 /
56.7: no energy change (FSR 2 only re-weights history).

**Modern Lighting / RT shadows under FSR 2** (the effects-performance pass's half-resolution
compute pass runs from the jittered render-size depth): still window,
`enh.modern_lighting 3` (RT) vs 2, after: full temporal 0.20 / 0.20,
road-pool region temporal 0.285 vs 0.274 (before 0.287 / 0.277), SSIM
0.9678 vs 0.9681: the traced shadows add no shimmer under FSR 2 and the
mask does not touch them.

**Water** (Animated Water Next-Gen, the casino fountain orbit
`track_C`, explore night, 4K; frames 300–323): FSR 2 Quality SSIM / FLIP /
temporal 0.601 / 0.243 / 21.9 before, 0.601 / 0.243 / 22.0 after; with the
water's reactive at 0.8 or 1.0 the same (0.601 / 0.243). The water loses
its fine specular sparkle and ripples under **every** temporal path (TAA
0.645 / 0.216, TAAU Quality 0.604 / 0.244, FSR 2 Native AA 0.637 / 0.214,
SSAA 2x 0.739 / 0.163; `d14b_water_temporal_paths_100pct.webp`): it is the
render size and the temporal resolve of a surface whose look changes
every frame, not FSR 2's history (a mask cannot bring back detail that is
not rendered). Open (below).

**Cost** (GPU busy median, `--bench racing 600`, 4K, the play setup,
before / after interleaved, 3 runs each; `scripts/d14b/cost2.sh`): FSR 2
Quality 7.053 → 7.059 (the mask pass 0.015 ms), Performance 5.740 →
5.747, Original 2.459 → 2.435 (untouched), Quality + Per-Object motion
blur + Cinematic DOF 7.454 → 8.068 (+0.61 ms: the motion blur and DoF work passes run at the
display's 3840 × 2160 instead of 2560 × 1440: gather 0.24 → 0.56, resolve
0.05 → 0.22; the quality gain above; FSR 2's guide). iGPU (reference integrated GPU, 2 CU,
1080p, the same settings, 300 frames, 3 runs; `d14b_cost_igpu.txt`):
FSR 2 Quality 66.5 → 65.9 (the mask 0.023 ms), Performance 51.00 →
51.04, Original 13.83 → 13.79, Quality + motion blur + DOF 69.8 → 77.0
(+7.2 ms: the display-size the motion blur and DoF work passes, gather 1.4 → 4.6 ms). On the iGPU
class the play setup is far beyond its budget either way; FSR 2's
own passes are ~22 ms of the 66 (accumulate 10.4 at 1080p). Motion blur /
DOF after the upscaler costs 10 % there: a candidate for a render-size
fallback on small GPUs (open).

## Native byte-identity
Upscaling 0..6 untouched: `Temporal::run` takes the FSR 2 path only for
values 7..11; the Native-AA depth fix (`game.rs`: the stage's own depth
when staged at 1.0) only applies when staged (upscaling on). `gate.sh … <base> extreme` (2026-11-05):
CHECKS PASS (1136 tests, selftest, 28 shots, flows, playthrough), PARITY
all gates passed (Original images / window path / Modern-Ultra-Extreme
dumps AE 0, 28 shots 0 differ, audio identical, sim hashes identical),
binaries differ (base and new binaries).

`gate.sh … <base> extreme` (CHECKS=full, 2026-11-06): CHECKS PASS
(1229 tests, selftest, 275 shots, determinism, flows, playthrough),
PARITY all gates passed (Original images / window path / enhanced dumps
AE 0, 29 shots 0 differ, audio identical, sim hashes identical), binaries
differ (base and new binaries). The mask, its pipelines and the
moved motion blur / DOF exist only under an FSR 2 mode.

## Tests
`render::native::world::plan::anim_mask_tests` (the FSR 2 masks pass: the chevrons' stepped scroll is
reactive at its steps only, continuous scrolls every frame, fixed offsets
never, a 15 fps flip-book steps 15 times a second; the values; the mask's
entry points in every native world module form, which parse and
validate), `render::fsr2::tests` (modes' render sizes / phases / LOD bias are FSR
2's; the depth factors return view distance in both depth directions;
every pass parses and validates as WGSL), `qol::tests::fsr2_modes` (parse,
names, temporal path, one sample), `options::tests::graphics_page_steps_and_restores`
(the AA row greyed "FSR 2 (Upscaling)").

## Tuning / debug aids (not options)
`NFSU2_FSR2_DEBUG=1` motion, `=2` masks (R reactive, G composition, B
depth clip; `debug_motion_masks_locks.webp`, with the animation mask
`d14b_debug_masks.webp`), `=3` locks, `=6` zero motion, `=7` as 2 without
the animation mask's depth test; the FSR 2 masks pass `NFSU2_FSR2_STEP_REACTIVE`,
`NFSU2_FSR2_STILL_REACTIVE`, `NFSU2_FSR2_ANIM_REACTIVE`,
`NFSU2_FSR2_ANIM_TC`, `NFSU2_FSR2_RIPPLE_TC` (the mask's values); `NFSU2_FSR2_SHARPEN` (RCAS stops, ≥ 9 off), `NFSU2_FSR2_LOCKS`,
`NFSU2_FSR2_BOX`, `NFSU2_FSR2_AUTO`, `NFSU2_FSR2_TC_GAIN`,
`NFSU2_FSR2_WHEEL`, `NFSU2_FSR2_LUMA_BLOCK`.

## Open
* Done in the FSR 2 masks pass: the animation mask (flip-books, scrolls, water, ripples),
  locks back on, motion blur / DOF after the upscaler, RT shadows and
  water checked, iGPU cost measured.
* Water's fine specular detail under temporal paths: **resolved in the water polish**
  (docs/notes/graphics/water.md "Water polish"). Not a temporal or FSR 2 issue: the default 8x
  MSAA lost it too; the Next-Gen jets' depth filter was capped at 10 px of
  render-size targets, so the smoothing in the view grew as the render size
  fell. The cap is now a share of the view; vs SSAA 4x, FSR 2 Quality
  0.605 → 0.790 SSIM, Native AA 0.653 → 0.880, TAA 0.661 → 0.881.
* Motion blur / DOF after the upscaler cost +0.6 ms at 4K (reference desktop GPU)
  and +7.2 ms at 1080p on the reference integrated GPU; a render-size fallback (blur
  before the upscaler) for iGPU-class devices is not made.
* The wet road's ripples: T&C 0.5 on the whole wet road is a judgement
  (no rain spot was measured against the reference in motion here).
* Draws whose texture animates by other means than a flip-book, a scroll
  or water (none found in the racing / explore routes) carry no mark.
* FSR 3 frame generation: out of scope.

## Perf gate
`perfgate.sh run` against the accepted baseline (the effects-tiers pass's), 8 configs,
5+5: **PASS** (no significant regression > 3 %; reported only: rain-max
4K VRAM +2.8 %, a config without FSR 2); `perfgate-d14b/perfgate.md`.

This page:
`perfgate.sh run` (new binary vs the accepted baseline, 8 configs, 5+5,
machine load 36–135): **FAIL twice** on CPU metrics of configs FSR 2 does
not touch (run 1: explore-extreme 1080p draw_cpu_p10 +9.9 %, rain-max 4K
cpu_render +16.4 %; run 2: rain-max 4K cpu_render +17.4 %, p 0.032). The
base binary in the same conditions: pass. A direct interleaved A/B of
base vs new on rain-max 4K (`perf.sh bench`, CONFIGS rain-max-3840x2160,
7 + 7 runs): cpu_render +0.04 % "same", GPU median +1.2 % "same" (16
comparisons: 15 same, 1 not significant). Logs:
`docs/evidence/fsr2/perfgate/`. Read as load noise against an old
baseline; the maintainer should rerun the gate on a quiet machine.
