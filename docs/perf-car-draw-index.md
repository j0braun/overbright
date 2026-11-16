# Car draw index in immediates

The car modules (car.wgsl and every composed form: Enhanced Paint, Wet Car, Surface Detail, the
temporal bias, Modern Lighting, Enhanced Glass, the SSR mask) read each draw's 560-byte
record from one storage buffer (`draws`, group 1). Before this change the entry came from the
instance index in `vs_main` and, in every fragment stage, from a flat interpolant
(`in.draw_index`). The compiler cannot tell that either is one value per draw, so the record is
read with vector loads per vertex and per pixel and held in vector registers.

## What changed

- `shader_variants::immediate_form`: the same module text with `var<immediate> car_draw: u32;`
  after the `draws` declaration and every `draws[draw_index]` / `draws[in.draw_index]` read as
  `draws[car_draw]`. That is the only change, and it is pinned by
  `immediate_forms_change_only_the_draw_index`: slot 0 in full against car.wgsl, every other form by
  undoing the edit, plus naga validation of all 74 forms. `module_source_for`,
  `glass_module_source_for` and `ssr_mask_source_for` choose the form for the device.
- `CarShared::immediates` = the device has `wgpu::Features::IMMEDIATES` (requested already for the
  world's texture arrays; `render/core/gpu.rs`). With it, the car and glass pipeline layouts take
  4 bytes of immediates, and every car draw goes through `draw_entry`: `set_immediates(entry)` and
  then `draw_indexed(.., entry..entry + 1)`. The draw paths are the main view, the front end and
  garage, covers, reflections and the mirror (`record_lod_draws`), the SSR mask
  (`record_ssr_mask`) and Enhanced Glass (`record_glass_draws`).
- Without the feature, the modules are the old text and the layouts the old ones, so the output
  is byte-identical. The backend's "runs without optional features" line names it as
  `immediates (no world texture arrays …; car draws read their values by instance index)`.
  `NFSU2_GPU_DROP=immediates` gives the same fallback for checking. Feature support in wgpu 30:
  Vulkan (push constants), DX12 (root constants) and GL (uniforms) all offer `IMMEDIATES`.
- Pipelines: the sets and their count are unchanged (each set is built from the chosen form).
  `warm_tests` still passes, and no pipelines are built after the first frame.

## Measured (this machine; docs/evidence/perf-car-draw-index/)

The race start, 1080p, has 4 cars on screen. Per frame that is about 495 car draws and about
27,900 triangles, with 52 pipeline switches, 197 texture bind-group switches, 4 group-1 binds
(one per car) and 1 pass. This change does not alter those counts; it adds 495 `set_immediates`
per frame.

| Car module stage | VGPR | waves/SIMD | VMEM | latency |
|---|---|---|---|---|
| fs_main, desktop GPU (RDNA3) | 84 → 48 | 18 → 32 | 28 → 2 | 6483 → 3016 |
| fs_main, integrated GPU (RDNA2) | 80 → 32 | 12 → 32 | 28 → 2 | 6607 → 3150 |
| fs_alpha_test, integrated | 96 → 32 | 10 → 32 | 28 → 2 | 6970 → 3520 |
| vs_main, integrated | 96 → 48 | 10 → 20 | 24 → 3 | 810 → 718 |

GPU time of the car pass on the integrated GPU at 1080p (timestamps, 600 frames, base and new alternated,
3 runs each):

- With 1 sample (FSAA off): 0.72–0.76 → 0.50–0.54 ms, a cut of 0.22–0.24 ms (−31 %). The spread is
  ±0.02, well under the change.
- With the native default (8 samples): 1.80–1.93 → 1.66–1.72 ms, a cut of 0.13–0.21 ms (−8 %).
- The whole frame's GPU busy time (7 ms or 13.7 ms) moves by less than its own run-to-run spread
  (±0.15–0.3 ms).
- The desktop GPU at 4K, 8 samples: the car pass is 0.21 ms in both builds. It is fill-bound there.
- Render-worker CPU ("cars: recording") rises by 0.00–0.01 ms per frame (0.03 → 0.04).

What bounds the pass. These are experiment binaries on the integrated GPU at 1080p, each with the new path:

| Experiment | 8 samples | 4 samples | 1 sample |
|---|---|---|---|
| Normal car pass | 1.70 ms | 1.06 ms | 0.51 ms |
| Fragment stage cut to the interpolated colour | 1.67 ms | 0.83 ms | 0.24 ms |

Two more runs at 8 samples narrow it down further. With each draw cut to its first triangle the
pass takes 0.13 ms, and with no draw calls at all it takes 0.04 ms. So:

- At 1 sample, the pass was shader-bound, as the triage read it. The record loads were most of
  the shader's cost, and the shader is half of the pass after the fix.
- At the default 8 samples, the pass is bound by multisample fill: depth and colour traffic for 8
  samples, on shared memory. The fragment shader is now under 0.05 ms of it, and draw overhead is
  about 0.1 ms. The triage's −0.5..−1.2 ms "of 3.1" does not hold on this tree, where the integrated GPU's car
  pass is 1.8 ms. The right next step for that part is the multisample traffic, not the shader.
  One example is car draw order for hierarchical Z; that would need a byte-identity check of its own.

## Laptop estimate (an older integrated GPU: the integrated figures ±30 %)

The estimated cut is −0.15..−0.3 ms per 1080p frame with FSAA off, and −0.1..−0.3 ms at the
default 8x. This is an estimate, not a measurement. GCN (Vega) runs wave64 with a 256-VGPR budget
per lane, so 80–96 VGPRs allow 2 waves per SIMD and 32–48 allow 5–8. The occupancy gain is
likely at least as large as on RDNA2, but the laptop's DDR4 makes the 8x part more
bandwidth-bound.

## Tester recipe (shipping build, no harness flags)

1. Start the game, then open the newest `game-<time>.log` in the logs folder
   (docs/platform-dirs.md). No line saying `runs without optional features: … immediates (…)`
   means the new path is on. If that line names `immediates`, the
   device lacks the feature and nothing changes.
2. Set the screen to 1080p and the Original look. Set World Detail's Full Screen Anti-Aliasing to
   Off, then start a Quick Race circuit on the same track with the same car. On the grid during
   the countdown, press F3 and note the average frame time over about 10 s. Do this with the
   previous test build and with this one, twice each, alternating.
3. A drop of at least 0.15 ms (about 1 fps at 70 fps) in this build confirms the gain. Repeat with
   FSAA at its default for the smaller drop expected there (about 0.1–0.2 ms).
