# GPU frame time and render encoding (2026-11-05)

The GPU profile per pass, then faster ways to the same frames — less CPU spent encoding the
render passes on the render workers, the LOD Fade bookkeeping on the main
thread, the pipeline cache for the last pipelines built without it.
Nothing here was traced in SPEED2.EXE and no native behaviour changed:
every step draws the same pixels (AE 0 on every gate), plays the same
sound and leaves the same simulation (state hashes identical). Base: main
`9080b94`. Results, gates and profiles: internal notes.

## Method

* Profile (plan item 1): `perf.sh bench`, new `SET=d4` (explore, racing,
  rain, race start × Original, Ultra, Extreme × 1920×1080, 3840×2160; 24
  configs, 2 runs each, 600 frames) with the bench JSON's per-pass GPU
  timestamps and per-thread CPU; every pass with its share per config:
  internal notes (written by
  internal notes). samply profiles (`perf.sh
  profile`, 900 Hz, 3000 frames) of racing Original / Extreme for the CPU
  side, and a line-level attribution of the hot function from the samply
  addresses (`addr2line`, the line-tables build).
* Counting before changing: a temporary counter in `draw_pass_faded`
  (not kept) gave the world passes' state changes per pass.
* Steps: each a source tree = the base + the steps before + its hunks
  (`perf-work/src/sN`), built with `perf.sh build` (full release), md5 of
  every pair different (`gates.txt`). Each against the one before:
  `perf.sh parity` with all gates (Original images 1280×960 + 4K, the
  144 Hz window path, Modern / Ultra, 15 shots, explore / race WAVs, the
  state hash over firstrace / firstdrag / postrace / the playthrough /
  circuit_s1), the Extreme / Extreme FSR gate on racing, explore, rain and
  the race start, and the five front-end shots on `NFSU2_SHOT_CLOCK=10`.
  Then `perf.sh bench` with the four binaries interleaved (new `SET=d4ab`:
  racing / explore / race start × Original / Extreme at 1080p, racing
  Ultra / Extreme at 4K; 8 runs) and `perfstat.py compare` per pair.

## The profile (base)

GPU busy (median of the frames, timestamps) and the largest passes, reference desktop GPU
mean per frame over the run:

| config | busy ms | main / workers CPU ms | largest passes (ms, share) |
|---|---|---|---|
| explore orig 1080p | 0.60 | 0.39 / 0.62 | main view 0.15 (24 %); streaks 0.11 (18 %); bleach 0.09 (14 %); overbright 0.07 (11 %) |
| explore ultra 1080p | 2.64 | 0.84 / 1.20 | main view 0.73 (28 %); volumetric 0.26 (10 %); HP overbright 0.25 (9 %); SSR copy+pyramid 0.20 (8 %) |
| explore extreme 1080p | 3.26 | 1.08 / 1.50 | smoke particles 2.74 (48 %); main view 0.67 (12 %); volumetric 0.26 (5 %); HP overbright 0.25 (4 %) |
| racing orig 1080p | 0.75 | 0.60 / 1.34 | main view 0.28 (36 %); cars 0.11 (14 %); streaks 0.10 (14 %); bleach 0.09 (11 %) |
| racing ultra 1080p | 4.08 | 1.59 / 2.79 | main view 1.97 (47 %); volumetric 0.44 (11 %); HP overbright 0.24 (6 %); SSR copy+pyramid 0.20 (5 %) |
| racing extreme 1080p | 4.87 | 1.99 / 3.45 | main view 2.02 (41 %); volumetric 0.45 (9 %); motion blur gather 0.27 (6 %); glass 0.26 (5 %) |
| rain orig 1080p | 0.74 | 0.56 / 1.02 | main view 0.22 (28 %); cars 0.12 (15 %); streaks 0.11 (14 %); overbright 0.06 (8 %) |
| rain ultra 1080p | 4.64 | 1.28 / 2.10 | main view 2.06 (44 %); volumetric 0.47 (10 %); HP overbright 0.24 (5 %); SSR trace 0.22 (5 %) |
| rain extreme 1080p | 5.64 | 1.57 / 2.87 | main view 2.16 (37 %); volumetric 0.47 (8 %); glass 0.30 (5 %); motion blur gather 0.28 (5 %) |
| race orig 1080p | 0.88 | 0.46 / 1.29 | main view 0.29 (31 %); cars 0.23 (25 %); streaks 0.11 (12 %); bleach 0.10 (11 %) |
| race ultra 1080p | 4.43 | 1.04 / 2.27 | main view 1.69 (39 %); volumetric 0.45 (10 %); cars 0.43 (10 %); HP overbright 0.24 (6 %) |
| race extreme 1080p | 5.38 | 1.51 / 3.14 | main view 1.87 (35 %); glass 0.51 (10 %); volumetric 0.46 (9 %); HP overbright 0.24 (5 %) |
| explore orig 4K | 2.26 | 0.51 / 0.71 | main view 0.71 (31 %); bleach 0.46 (20 %); streaks 0.45 (20 %); particles 0.16 (7 %) |
| explore ultra 4K | 10.06 | 1.23 / 1.63 | main view 2.66 (27 %); HP overbright 1.08 (11 %); volumetric 1.02 (10 %); SSR copy+pyramid 0.71 (7 %) |
| explore extreme 4K | 12.66 | 1.43 / 1.75 | smoke particles 10.83 (48 %); main view 2.44 (11 %); HP overbright 1.08 (5 %); volumetric 1.03 (5 %) |
| racing orig 4K | 2.73 | 0.93 / 1.75 | main view 1.00 (37 %); streaks 0.47 (17 %); bleach 0.46 (17 %); cars 0.27 (10 %) |
| racing ultra 4K | 14.53 | 1.82 / 2.83 | main view 6.11 (42 %); volumetric 1.78 (12 %); HP overbright 1.10 (8 %); SSR copy+pyramid 0.78 (5 %) |
| racing extreme 4K | 17.14 | 2.02 / 3.17 | main view 5.83 (34 %); volumetric 1.81 (10 %); motion blur gather 1.22 (7 %); HP overbright 1.10 (6 %) |
| rain orig 4K | 2.64 | 0.80 / 1.29 | main view 0.91 (34 %); streaks 0.44 (16 %); lens drops 0.37 (14 %); cars 0.26 (10 %) |
| rain ultra 4K | 17.18 | 1.65 / 2.47 | main view 7.05 (41 %); volumetric 1.86 (11 %); HP overbright 1.14 (7 %); SSR copy+pyramid 0.76 (4 %) |
| rain extreme 4K | 21.02 | 1.98 / 3.20 | main view 7.05 (33 %); volumetric 1.89 (9 %); motion blur gather 1.25 (6 %); HP overbright 1.16 (5 %) |
| race orig 4K | 3.23 | 0.65 / 1.57 | main view 1.05 (33 %); cars 0.68 (21 %); bleach 0.51 (16 %); streaks 0.46 (15 %) |
| race ultra 4K | 15.83 | 1.57 / 2.95 | main view 5.65 (36 %); volumetric 1.83 (12 %); cars 1.38 (9 %); HP overbright 1.13 (7 %) |
| race extreme 4K | 20.93 | 1.43 / 2.96 | main view 6.05 (31 %); volumetric 1.82 (9 %); glass 1.78 (9 %); HP overbright 1.07 (5 %) |

(2 runs per config, machine load 5–50 from other jobs; GPU numbers from timestamps, CPU = thread CPU. Every pass of every config: internal notes.)

* **Original** costs 0.6–0.9 ms at 1080p and 2.3–3.2 ms at 4K: the main
  view 24–36 %, the post chain (streaks, bleach bypass, overbright, to
  the back buffer) 35–55 % at 4K, the cars 9–25 % (race start: the line-up
  and the crowd). Unchanged from the performance research's playbook; nothing in it is large
  enough on this GPU to be worth a non-exact change, and the exact
  options were done before (load/store ops, the stuck-sound fix multiview cube).
* **Ultra / Extreme 4K** 10–21 ms: the main view 27–42 % (SSAA 2x scene ×
  the lamps' shading), Volumetric Fog High 9–12 %, the high-precision
  overbright 5–11 %, the SSR chain 10–16 % in four passes, motion blur's
  gather 3–7 % (Extreme), enhanced glass 5–9 % at the race start.
* **explore Extreme: "graphics: particles draw" 48 % of the mean GPU
  time** (2.7 ms at 1080p, 10.8 ms at 4K) — spiky (1080p: busy median
  3.2 ms, mean 5.7 ms): the smoke of the bench's opening burnout close to
  the camera. Fragment-bound (it scales with the pixel count). Measured
  by cutting the shader (dev build, not kept): no fragment work 0.37 ms,
  texture + blend only 0.89 ms, without the noise erosion 1.21 ms, full
  2.71 ms — the two value-noise octaves (16 hashes a fragment) are 55 %.
  No exact cheaper form found (the noise's inputs vary per pixel; the
  per-puff part is one `floor`/`fract` of a constant). Open: a
  pre-computed 3-D noise texture would be the usual answer but changes
  the output (filtering), so it is not an "identical" optimization.
* **Render workers** (encoding): 4 threads, 1.0–1.6 ms/frame summed at
  Original, 2.3–3.4 ms at Ultra / Extreme (60–64 % of the process's
  CPU), mostly wgpu-core's `encode_render_pass`, the binder's
  `assign_group`, `register_init_action`, `flush_bindings_helper`.
* **Main thread** (Extreme racing): `WorldRenderer::update_fades` 16.9 %
  self / 22.1 % inclusive (LOD Fade).

Counted in the world passes (racing, 600 frames): Original 340 draws a
pass, a pipeline change every 7th draw, a texture change on 90 % of the
draws and **a change of texture array — `set_bind_group(1)` — on 81 % of
them** (371 593 of 460 123 texture changes); Extreme 580 draws a pass, the
same shares. Each bind-group switch costs wgpu-core a tracker insert, a
usage-scope merge, the binder's `assign_group`, and at the next draw the
rebinding of every group from 1 on (`flush_bindings_helper`: the init
actions of groups 1 and 2 — `register_init_action` — and two
`vkCmdBindDescriptorSets`). Redundant calls were few: a pipeline change
with the texture unchanged re-sent the immediates in 0 of 70 735 changes
(Original) and 3 787 of 217 991 (Extreme); not worth a step.

## The steps

1. **Bound texture arrays** (`render/mod.rs`, `render/temporal.rs`). With
   `TEXTURE_BINDING_ARRAY` (requested with `PARTIALLY_BOUND_BINDING_ARRAY`
   when the adapter has them) the world's texture arrays (68 in Bayview)
   sit in one group 1 as a binding array of up to `BOUND_ARRAYS` = 256
   (more: one group per 256), and the draw's array is an index in the
   immediates' last word, which was padding (`TextureImmediates._pad[2]`,
   0 before; the native-array path still sends 0 there). A draw changing
   array now changes only the immediates it already set. The shader
   samples `tex[imm.array]` with the same sampler, coordinates and layer
   (a uniform index, so no non-uniform qualifier); every other line of
   the world module is the same text (test). The world's extra textures
   (car shadows, the start scene's characters) keep a group of their own
   (the array at index 0). Without the feature (or with
   `NFSU2_NO_BOUND_ARRAYS=1`) the old path, unchanged. Test:
   `bound_array_modules_match_the_array_ones` (every world module form —
   native / lamps, multiview, mip bias — parses and validates in naga with
   and without bound arrays, 48-byte immediates with the old members
   first, the sampling call the same on the indexed array, the rest of the
   text identical; the slot → group / index / layer mapping). Counted
   after: 1 array bind per pass (1 499 for 1 500 passes) instead of 371 593.
2. **LOD Fade's walk** (`render/mod.rs` `update_fades`, `FadeGroups`,
   `fade_slots`). The fades walked every group of every drawn section and
   tested its key (sky, LOD bits, cull flags) per frame; only ~8 000 of
   the 111 352 instance slots are sized per frame (Extreme racing:
   ~3 500–4 300 main, ~3 500–5 100 far), so the group tests and the
   indexing were most of its 0.1 ms. Now the groups each section's main and
   far walk keeps are listed once per (main LOD, depth direction) — the
   same tests on the same fixed keys, in the same order, the far list
   flagged where the main LOD has the group too (left out while the main
   list draws that section) — and the per-slot body runs over slices
   (`fade_slots`, the old body verbatim, bounds checks hoisted). The
   statistics (`NFSU2_POP_STATS`) are summed in the same slot order (pop
   lines identical, base vs new, Original with Keep All Loaded + World
   Detail Extreme). Test: `fade_slots_match_the_old_loop` (the old loop as
   the reference, 78 frames of a moving eye over 600 slots, fades on and
   off, jumps, leaving sections, statistics: states, values, flags,
   changed lists and pop sums bit-identical).
3. **Pipeline cache** (`render/{rain,ssr,lamps,weather_fx,car_glass}.rs`):
   the five pipelines still built with `cache: None` (lens drops, SSR's
   test depth, the lamp shadow pipelines, weather fx, the glass copy) take
   the device's on-disk `PipelineCache` like the others. They are
   built lazily on first use, not on the load path; there is nothing to
   measure on the reference desktop (Mesa's own shader cache is warm): it only
   saves a driver compile on a cold cache.

## Results

reference desktop GPU, full-release binaries, `SET=d4ab`, 600 frames, the four
binaries interleaved, 8 runs each (internal notes;
machine load 4–10). Medians of the runs; ✔ = flagged by `perfstat.py`
(p < 0.05 and beyond 2 × noise and the tolerance).

**Step 1, bound arrays** — render workers' CPU (4 threads summed, ms/frame):

| config | s0 | s1 | change |
|---|---|---|---|
| racing Original 1080p | 1.221 | 1.089 | −10.8 % (p 0.002) |
| race start Original 1080p | 1.364 | 1.230 | −9.8 % (p 0.005) |
| explore Original 1080p | 0.682 | 0.653 | −4.2 % (n.s.) |
| racing Extreme 1080p | 2.781 | 2.202 | −20.8 % ✔ |
| race start Extreme 1080p | 2.747 | 2.359 | −14.1 % ✔ |
| explore Extreme 1080p | 1.287 | 1.181 | −8.2 % (p 0.003) |
| racing Ultra 4K | 2.382 | 1.974 | −17.1 % (p 0.002) |
| racing Extreme 4K | 2.923 | 2.371 | −18.9 % ✔ |

Process CPU −7…−14 % (racing Extreme 1080p 4.317 → 3.708 ms ✔), main
thread and GPU unchanged (GPU median within ±0.4 % everywhere but racing
Ultra 4K +1.75 %, p 0.11, not significant; s3 vs s0 there: +0.05 %).
Changes under the flag threshold with p < 0.01 are listed as "same" by
the rule (noise 4–6 % on the worker CPU); the 4 of 8 flagged and the
other 3 with p ≤ 0.005 all point the same way.

**Step 2, LOD Fade's walk** — the `world: LOD fade` CPU part (main thread,
ms/frame, median of the runs): racing Extreme 1080p 0.172 → 0.137
(−20 %), race start Extreme 0.138 → 0.105 (−24 %), explore Extreme 0.054 →
0.041, racing Extreme 4K 0.188 → 0.149. Main thread racing Extreme 1080p
1.510 → 1.457 ms (−3.5 % ✔), draw CPU p10 −3.3 % (p 0.001). Without LOD
Fade (Original) nothing runs there: unchanged.

**Step 3, pipeline cache** — no change anywhere (0 of 128 flagged; as expected,
the pipelines are built on first use and Mesa's shader cache is warm).

**Total, s0 → s3** (`step-s0-s3/compare.md`): render workers −13…−20 %
(flagged: racing Original 1080p 1.221 → 1.059, racing / race start Extreme
1080p, racing Extreme 4K 2.923 → 2.345), process CPU −10…−15 % at Extreme
(flagged in three configs), main thread racing Extreme 1080p 1.545 →
1.479 (−4.3 % ✔); GPU busy unchanged (−1.4…+0.2 %); no regression in any of the
4 × 128 comparisons.

### Tried and dropped
* **SSR's scene copy as a texture copy** (`copy_texture_to_texture`
  instead of the `fs_copy` full-screen pass; exact — the pass loads each
  texel into a texture of the same format): slower on RADV, 4K Ultra
  racing "ssr copy and depth pyramid" 0.79 → 0.85 ms (3 runs each, same
  binary, `NFSU2_SSR_COPY_PASS` A/B; dev build). Not kept.

## Open (with reasons)
* **Render bundles**: the culled draw lists change most frames (the
  cull runs per view per frame) and after step 1 the encoding per draw is
  an immediates write and a draw; a bundle replays the same commands at
  the hal level. Not tried.
* **Volumetric Fog's per-tile lamp lists** (playbook §4.5 item 1): the
  shader already skips a lamp whose bounding sphere the ray misses; a tile
  list would save only that test (≈15 ALU per lamp), while the cost is
  the 8-step march of the lamps a wave's rays hit (a wave of 64 pixels
  runs the march if any lane hits). Expected gain small (inferred); not
  attempted.
* **SSR depth pyramid in one compute pass** (SPD), **HP overbright's blur
  in LDS**, **motion blur gather**: exact only with the float order kept;
  not attempted in this work (each an L-size change with its own gate).
* **The smoke's noise** (explore Extreme spikes, above): no exact cheaper
  form.
* **Car Shadow Ultra ground-query cache** (item): `Shadow::refine`'s
  ground queries go through `TrackGround::query` / `choose_pack_with`,
  whose pack choice carries state from the previous query (the existing
  test `refinement_matches_reference_and_stateful_query_order` guards the
  query order for that reason). A cache that skips queries would change
  that state and with it later answers; a correct invalidation needs the
  pack state in the key. Not done.
* **TLAS rebuild** (Ray Traced reflections): measured with the Ray Traced tier on
  (Ultra + `enh.ssr 3`, racing, one run each, s3): the ray-query trace is
  the cost — "graphics: rt trace" 0.93 ms at 1080p / 3.51 ms at 4K
  against SSR's 0.17 / 0.62 ms trace (GPU busy median 3.94 → 4.80 ms,
  14.06 → 17.15 ms). The TLAS / BLAS build is its own encoder submitted
  from `rt::frame` before the frame's command buffers; its timestamp mark
  ("graphics: rt tlas") is not in the bench's per-pass list (it lands
  outside the frame's timestamp slot), so its cost is only bounded by the
  "(between submissions)" part, which rose 0.076 → 0.082 ms at 1080p
  (0.166 at 4K): ≤ 0.01–0.1 ms (inferred). Main thread +0.1 ms in one run
  (noise level). Nothing to gain at the build; the trace is the RT tier's
  own work.
* **The device's upkeep off the main thread** (step 5): unchanged, wgpu
  31.
* Windowed present cost (wgpu #9937) and the iGPU set: as in
  `docs/perf-gates.md` (headless-only rule; the integrated GPU is the desktop's).

## Audit round 2 (2026-11-05)

The second performance audit. The rule it follows: the **enhancements** may use non-bit-exact
forms when the gain is large and nothing looks cheaper; the Original look stays byte-identical. Base: the
loading-and-pacing copy (main + that pass). Results, gates, image metrics and crops:
internal notes. Nothing here was traced in SPEED2.EXE; no
native behaviour changed (Original images, the 144 Hz window path, the 15
shots, the WAVs and the state hashes over firstrace / firstdrag /
postrace / the playthrough / parity identical at every step).

### The profile (s0, plan item 1)

`perf.sh bench SET=d4` (explore, racing, rain, race start × Original,
Ultra, Extreme × 1080p, 4K): every pass and its share in
internal notes, the main thread's CPU
parts in `profile-s0-top.md`. Unchanged from this work's table above within the
noise. Hotspots ≥ 1 % of their domain, before this work:

* **GPU, Original** (0.6–0.9 ms at 1080p, 2.4–3.2 ms at 4K): main view
  24–37 %, streaks 13–20 %, bleach bypass 10–20 %, cars 9–26 % (race start
  22–26 %), overbright 5–10 %, particles 3–8 %, lens drops (rain) 7–14 %,
  to the back buffer 2–4 %, environment cube 2–4 %.
* **GPU, Ultra / Extreme 4K** (10–21 ms): main view 27–42 %, **smoke
  ("particles draw") 48 % in explore Extreme** (10.9 ms mean; frames to
  107 ms), volumetric fog 9–12 %, HP overbright 5–11 %, the SSR chain
  (copy + pyramid, trace, filter, composite) 10–16 %, enhanced glass 3–9 %,
  motion blur gather 3–7 %, cars 4–9 %, SSAO 3–6 %, DOF gather (race
  start) 4 %, streaks 3–5 %.
* **CPU, main thread** (0.4–0.8 ms Original, 0.8–2.4 ms Extreme): road
  reflection + main view culling/recording 10–22 %, shadows / particles /
  glows 10–20 % (Car Shadow Ultra's ground queries 5–6 % of the thread at
  Extreme: `TrackGround::choose_pack_with` 3.1 %, `query` 2.5 %, half of
  it from the pooled reflection flares' fresh queries), cars 8–15 %, HUD
  8–12 %, LOD Fade (`fade_slots` 9 % self at Extreme), memcpy 8–12 %, wgpu
  `write_buffer` 2 %, `lamps::build` 2 %. Render workers: wgpu-core's
  encoding (as the GPU encoding pass).

### The steps

1. **Half-size detailed smoke** (s1; `render/gpu_particles.rs`
   `HalfSize`, `gpu_particles.wgsl` `shade` / `fs_half`, new
   `render/smoke_half.wgsl`; Particles High / Ultra only). The puffs are
   drawn into two Rgba16Float targets of half the scene (or viewport)
   size per axis: target 0 the premultiplied colour "over" (One /
   InvSrcAlpha) with the transmittance Π(1 − a) in alpha, target 1 the
   native alpha blend's own sum (a·a + (1 − a)·r: the bleach mask). One
   full-size pass puts them over the scene with dual-source blending
   (scene · T + C, alpha · T + A — algebraically the per-puff "over" of
   the same puffs, test `half_size_targets_composite_to_the_native_over`),
   reading the four half-size texels around each pixel weighted
   bilinearly and by how close the depth each texel's puffs were softened
   against is to the pixel's own (no smoke bleeding onto a car's edge, no
   hole cut into the smoke); a pixel whose four texels hold no puff is
   discarded (the blend would leave it as it is). The soft-particle depth test reads the
   full-size depth texel under each half-size fragment (`depth_map`).
   Needs `Features::DUAL_SOURCE_BLENDING` (requested when offered); without
   it, or with `NFSU2_SMOKE_FULL=1`, the full-size draw as before (gate:
   AE 0 against s0 on all Extreme / Extreme FSR dumps).
2. **Half-size Volumetric Fog** (s2; since replaced: Low and High are tiers of the froxel medium,
   docs/atmosphere.md "The tiers"; was `render/volumetric.rs`,
   `volumetric.wgsl` `map` / `fs_up`; the deep research's item 10). The march
   runs at half the scene size per axis (each texel marching to the depth
   of the full-size pixel under its centre) into an Rgba16Float target,
   then `fs_up` adds it to the scene: the four texels around the pixel,
   weighted bilinearly and by the gap between the distance each texel
   marched to and the pixel's own (relative, 1 % scale); a pixel whose four
   texels hold no light is discarded. `NFSU2_VOL_FULL=1`:
   the full-size pass as before (gate: AE 0 against s1 on Ultra / Extreme /
   Extreme FSR).
3. **Car shaders without the scratch spill** (s3; `car.wgsl` `car_lamps`;
   item 7 of a performance study kept with the maintainer's private evidence).
   `car_lamps` read `draw.lamps[i * 2u]` from the private copy of the
   draw's record with a loop index, which made the compiler keep the whole
   560-byte record in scratch memory for every pixel of every car draw, in
   every mode. It now reads `draws[in.draw_index].lamps[…]` (the same
   values). Every car module form (native, paint, wet, Surface Detail,
   glass) inherits it (test `car_modules_index_no_private_record`). Exact:
   AE 0 on every gate including Extreme / Extreme FSR.

### Tried and dropped
* **Smoke noise shared across the 2×2 quad** (subgroup `quadBroadcast` of
  the 8 lattice hashes when the quad is live and in one cell; the loading and pacing pass's open
  item): −14 % of the smoke pass (explore Extreme 4K 10.95 → 9.41 ms),
  but not bit-exact (max diff 3–16 on 30 k–340 k pixels: the driver
  contracts the hash arithmetic differently in the two forms), and the
  half-size smoke takes most of the pass anyway. Dropped (rule: keep the
  exact path where the gain is small).
* **SSR composite discarding unchanged pixels** (exact with a `Load` of
  the scene): 0.44 → 0.39 ms at 4K Ultra (−0.3 % of the frame). Not kept.
* **Volumetric Fog per-tile lamp lists**: measured first — with the march
  cut out the pass costs the same with 0 or 48 lamps (0.42 ms at 4K
  Ultra), i.e. the per-lamp sphere test is free and the 8-step march of
  the lamps a pixel's ray hits is the whole cost (1.28 of 1.70 ms). No
  exact gain; replaced by step 2.
* **HP overbright's Gaussian weights per pixel** (`exp` in `gauss`):
  replacing them with constants changed nothing (1.10 ms either way):
  bandwidth-bound, no ALU to save.

### Results

reference desktop GPU, full-release binaries, `perf.sh bench SET=d6` (new set: the
configs the steps touch), the four binaries interleaved, 3 runs each, 600
frames; machine load 8–58 from other jobs (GPU numbers from timestamps).
Medians of the runs (internal notes; perfstat per
pair in `ab-compare-*.md` — with 3 runs no Mann-Whitney p reaches 0.05, so
the ranges are the evidence: every change below has disjoint ranges).

| config | GPU mean s0 → s3 | step gains (ms/frame) |
|---|---|---|
| explore Extreme 1080p | 5.86 → 3.72 (−36 %); p95 22.9 → 8.2 | smoke 2.79 → 0.84; fog 0.27 → 0.15; glass 0.18 → 0.14 |
| explore Extreme 4K | 22.6 → 14.3 (−37 %); p95 88.9 → 31.7 | smoke 10.97 → 3.24; fog 1.04 → 0.56; glass 0.66 → 0.53 |
| racing Ultra 4K | 14.70 → 13.47 (−8.4 %) | fog 1.80 → 0.75; cars 0.41 → 0.25 |
| rain Ultra 4K | 17.63 → 16.21 (−8.0 %) | fog 1.89 → 0.76; cars 0.47 → 0.29 |
| racing Extreme 4K | 17.41 → 16.28 (−6.5 %) | fog 1.84 → 0.77; glass 0.85 → 0.64; **smoke 0.14 → 0.36 (+0.22)** |
| race start Extreme 4K | 19.94 → 18.31 (−8.2 %) | fog 1.83 → 0.76; glass 1.79 → 1.11 |
| race start Original 1080p | 0.941 → 0.820 (−13 %) | cars 0.231 → 0.106 |
| race start Original 4K | 3.21 → 2.79 (−13 %) | cars 0.688 → 0.305 |
| racing Original 1080p | 0.823 → 0.777 (−5.6 %) | cars 0.111 → 0.071 |

Image differences (enhanced only; Original byte-identical): smoke SSIM ≥
0.9961 on every gate frame (Extreme + TAA + FSR, worst 8×8 block −0.33 on
the car's rear window where the glass reflects the smoke; plain Extreme
≥ 0.9993, max diff ≤ 99 on ≤ 17 pixels of 2 M); fog SSIM ≥ 0.9993 (max diff
≤ 54 on ≤ 7 pixels > 32); motion: the half-vs-full error changes by 0.02
(smoke) and 0.009 (fog) levels per frame on average — no shimmer measured;
crops at 100 / 200 % and animated crops in internal notes.
The light-smoke cost: where few puffs are on screen (racing Extreme) the
two half-size targets' clears and the full-size composite cost more than
the puffs saved (+0.22 ms at 4K); the composite discards pixels with no
puff, which took it from +0.34 to +0.22 ms.

### What is left (≥ 1 % of a domain, after the second audit)
* GPU 4K Ultra / Extreme: world main view 33–48 % (depth pre-pass and
  item 9, lamp shader specialisation, of the deep research; not done here),
  smoke in heavy smoke 23 % (explore Extreme, was 48 %), HP overbright 7–8 %,
  the SSR chain 10–15 %, volumetric fog 4–6 % (was 9–12 %), enhanced
  glass 4–6 %, motion blur / DOF gathers 3–7 %, SSAO 3–5 %, streaks
  3–4 %, cars 2–3 % (was 3–9 %).
* GPU Original: main view 30–40 %, streaks 13–20 %, bleach bypass 10–20 %,
  cars 9–13 % (was 9–26 %), overbright 5–10 %, particles 3–8 %, lens drops
  (rain) 7–14 %.
* CPU: `docs/perf-main-thread.md` "Audit round 2".
* Not covered by this work from the deep research's list: items 8, 9, 11,
  12, 13 (item 7 = step 3, item 10 = step 2).

## Depth pre-pass and lamp shader specialisation (2026-11-05)

The depth and lamps pass (items 8 and 9 of the same study). Enhanced lighting only: every change below is in the
lamps' module path (`SceneMode::lamps`: Dynamic Lamps, Lamp Shadows,
Contact Shadows, Headlight Beams, Surface Detail or Modern Lighting on).
The Original path, its module text and its pipelines are untouched (test
`native_module_untouched`; Original images AE 0 in the gates). Nothing
here was traced in SPEED2.EXE; no native behaviour changed. Results,
gates and metrics: internal notes.

### Step 1: depth pre-pass for the main view

`render/mod.rs` (`draw_depth_prepass`, `DepthPrepass`, `DepthKey`,
`make_depth_pipeline`, `in_depth_prepass`, `WorldShared::{depth,
equal}_pipelines`), `world.wgsl` (`fs_depth_alpha_test`, `fs_depth_fade`,
`fs_depth_alpha_test_fade`, LAMPS block).

* Before the main view's colour pass a depth-only pass draws the opaque
  draws of the same draw list (blend None, effects world / worldnofog /
  worldreflect; not the sky, the blends or skymorph) with the colour
  pipelines' own vertex entry points (`vs_main`, `vs_reflect`, their
  `_fade` forms), depth state and sample count. Alpha-tested draws repeat
  the colour shaders' test (`texel.a * color.a <= ALPHA_REF`, the same
  `sample_tex`); LOD Fade's dithered groups repeat the screen-door mask
  (`fade_mask`). With MSAA's alpha-to-coverage the alpha-tested draws stay
  out of the pre-pass (their sample mask is not repeated).
* The colour pass loads that depth; the pre-pass's draws take EQUAL
  pipelines without depth writes (`DepthPrepass::equal`, by LOD Fade mode:
  native / fades / dither), so a hidden layer fails early depth before
  its shading, also for the alpha-tested draws (no depth write left for
  their discard to hold back). Every other draw keeps its pipeline.
* Why the same pixels: the pre-pass writes the minimum depth G of the
  opaque draws' fragments, which is what LESSEQUAL with writes leaves
  natively; the native last writer at G passes EQUAL, and every draw after
  it (blends included) tests against G in both. Draws not in the
  pre-pass test and write as before.
* Same depth bits in two pipelines: the lamps' module's clip position is
  `@invariant` (only there; no change on RADV, AE 0). Proven by dumps
  (AE 0 against the single pass: Modern MSAA 8×, Ultra, Extreme, the
  play setup, racing / explore / rain / race start, Modern Lighting
  tiers 1–3).
* `NFSU2_DEPTH_PREPASS=0`: the single pass as before; `=2`: the pre-pass
  with the colour pass's own LESSEQUAL pipelines (measurement only).
* Cars are not in the pre-pass (inferred gain ≤ 0.3 ms at 4K: the cars
  cover ~10 % of the racing view; the native motion blur between the
  world and the cars and the car glass both need the world behind the
  cars drawn). Open.

### Step 2: the lamps' module specialised per feature set

`world.wgsl` (`D9_*` overrides, LAMPS / MODERN lines only),
`render/mod.rs` (`SceneMode::lamp_features`, `lamp_features()`,
`SceneMode::lamp_constants`).

* Pipeline constants turn off, per pipeline, the branches of options that
  are off: Dynamic Lamps (lamps.x), Contact Shadows (lamps.y), Lamp
  Shadows (lamps.z, and its PCF radius from the quality in force),
  Headlight Beams (cluster_near.z), Surface Detail (cull.z), Modern
  Lighting's Low tier and debug view. A constant is `false` only where the
  option is off, so its uniform is 0 in every frame and the removed
  branch never ran; the compiler drops the dead code and its registers.
  The feature set is part of `SceneMode`, so toggling an option rebuilds
  the scene's pipelines (from the shared caches). `NFSU2_LAMP_SPECIALISE=0`
  keeps every branch (A/B).
* Shader statistics (RADV, `RADV_DEBUG=shaderstats`): Ultra's world
  fragment shaders 7 757 → 4 690 instructions, 108 → 84 VGPRs, 14 → 18
  waves/SIMD; Modern Lighting (Modern) 9 729 → 6 481 instructions, 144 →
  120 VGPRs, 10 → 12 waves.
* Exact for Dynamic Lamps / Ultra / Extreme / the play setup / Modern
  Lighting Low (AE 0). Modern Lighting Modern and Modern + RT: the folded
  Surface Detail branch changes the compiler's rounding by 1/255 on 1
  pixel in 1–3 of 61 frames (racing, race start; 0 in explore; SSIM
  1.000000, a different pixel each time): measured in
  internal notes, for −0.6 ms of the step's −0.9.
* Tried and dropped: skipping the shadow taps of a lamp whose weight or
  N·L is 0 (exact): no measurable change (2.18 → 2.21 ms).

### Results

`perf.sh bench SET=d8 REPS=3` (new set: racing / explore / rain / race
start × Ultra / Extreme at 4K, racing Ultra 1080p, racing Modern 4K,
racing Original 1080p as the control), s0 / s1 / s2 interleaved, 600
frames, medians of the runs (internal notes):

| config | GPU busy mean s0 → s1 → s2 (ms) | s0 → s2 | main view colour (+ pre-pass) |
|---|---|---|---|
| racing Ultra 4K | 13.42 → 10.55 → 9.84 | −3.58 (−26.7 %) | 6.02 → 2.86 + 0.32 → 2.10 + 0.32 |
| rain Ultra 4K | 16.29 → 13.51 → 12.51 | −3.78 (−23.2 %) | 7.11 → 4.16 + 0.23 → 3.04 + 0.24 |
| race start Ultra 4K | 14.00 → 11.26 → 10.61 | −3.40 (−24.2 %) | 5.65 → 2.56 + 0.36 → 1.90 + 0.36 |
| explore Ultra 4K | 9.33 → 8.42 → 7.92 | −1.42 (−15.2 %) | 2.64 → 1.66 + 0.13 → 1.15 + 0.13 |
| racing Extreme 4K | 16.24 → 13.83 → 13.61 | −2.63 (−16.2 %) | 5.74 → 2.98 + 0.38 → 2.72 + 0.38 |
| rain Extreme 4K | 21.89 → 19.55 → 19.06 | −2.83 (−12.9 %) | 7.62 → 4.97 + 0.33 → 4.51 + 0.33 |
| race start Extreme 4K | 18.36 → 15.42 → 15.15 | −3.22 (−17.5 %) | 6.05 → 2.79 + 0.41 → 2.52 + 0.41 |
| explore Extreme 4K | 14.32 → 13.55 → 13.32 | −1.00 (−7.0 %) | 2.44 → 1.66 + 0.14 → 1.46 + 0.13 |
| racing Modern 4K (MSAA 8×) | 8.00 → 7.64 → 7.53 | −0.46 (−5.8 %) | 3.37 → 2.80 + 0.21 → 2.69 + 0.21 |
| racing Ultra 1080p | 3.89 → 3.11 → 3.38 (median 3.84 → 3.01 → 2.96) | −0.50 | 1.97 → 0.99 + 0.16 → 0.91 + 0.19 |
| racing Original 1080p | 0.75 → 0.80 → 0.80 | noise (one slow run per arm; ranges overlap) | 0.29 → 0.31 → 0.31 |

Extreme gains less from step 2 (Headlight Beams and Surface Detail are on
there, so their branches stay). Modern Lighting (dev binary, 4K Ultra +
tier, racing, A/B with `NFSU2_LAMP_SPECIALISE`): Modern 12.27 → 11.47 ms,
Low 10.96 → 10.21 ms. The render workers' CPU for the pre-pass's extra
draws: +0.2–0.5 ms per frame summed over the workers (off the main thread;
`perfstat` "same" / not significant).

### What is left (≥ 4 % of the frame, racing 4K after steps 1 and 2)
* Ultra (9.84 ms): main view 21 %, HP overbright 11 %, SSR copy + pyramid
  8 %, volumetric fog 8 %, SSR trace 6 %, streaks 5 %, SSAO occlusion 5 %,
  SSR composite 4 %, bleach bypass 4 %.
* Extreme (13.61 ms): main view 20 %, motion blur gather 9 %, HP overbright
  8 %, volumetric fog 6 %, SSR pyramid / trace 5 % each, enhanced glass 5 %.
* Open from this work: the cars in the pre-pass (above); the Modern
  Lighting module's 120 VGPRs (12 waves/SIMD: the relight loop itself).

## SSR, motion blur tiles, submissions (2026-11-05)

The SSR / blur / submit pass (items 11, 12, 13 of the same study and the depth and lamps pass's open "cars in the pre-pass"). Enhanced passes
only (Screen-Space Reflections, Motion Blur tiers); nothing traced in
SPEED2.EXE, no native behaviour changed; the Original path is untouched
(gate: Original images, shots, audio, state hashes identical). Every step
is exact (AE 0 in dumps and in the gate, Extreme / Extreme FSR / the
play setup). Results: internal notes.

### Step 1: SSR only where the mask needs it
`render/ssr.rs`, `ssr.wgsl`, `render/mod.rs` (`Mask::depth`); details in
`docs/wet-roads-ssr.md` "SSR, motion blur tiles, submissions".
* Scene-sized copy and composite depth-tested against the world's own
  mask depth (NotEqual its clear value): unmasked pixels are not shaded;
  the composite loads the scene. The trace samples the scene itself, so
  the copy feeds only the composite's `base`.
* `fs_linear` writes a trace-sized Depth16 mask (the trace's and the
  filter's early-out tests on the same texel) and the mask's kind/depth
  per trace pixel; the trace and filter are depth-tested against it (early
  Z skips unmasked tiles, cleared 0 = what they returned), the filter's 12
  taps read the trace-sized kinds. Scene depth by `textureGather` (exact:
  the same texel set, min is order-free).
* The march: level size from the uniform, the ray depth carried from the
  step before (same expression, same value), the overlay test (≤ 10 loads)
  evaluated only when it decides.
* Tried and dropped: the trace writing the kinds and the filter's skip
  depth itself (linear back to its old cost; trace then shades every
  pixel): no better (chain 2.78–2.81 vs 2.72–2.76, dev runs).

### Step 2: motion blur tiles in two steps, gather skips unchanged pixels
`render/blur_dof.rs`, `blur_dof.wgsl`; details in `docs/motionblur-dof.md` "Motion blur tiles".
Tile max as rows then tiles (first longest, strict `>`: the same pick;
CPU-model test and AE 0 dumps); the single-sampled gather discards the
pixels it returned unchanged. DOF: not changed (open, see there).

### Step 3: submissions — measured, no change
Per-encoder gap labels (temporary): the 4 scene buffers' boundaries cost
0.01–0.02 ms per frame together, the UI's 0.01; the only material gap is at
the scene → post *submission* (0.02–0.56 ms run to run, 0.32–0.35 typical
with other jobs' GPU jobs), which is where other processes' GPU jobs
interleave. With everything in one submission (`NFSU2_ONE_SUBMIT=1`, AE 0)
it is 0.01, GPU busy unchanged (8.87–8.96 vs 8.89–9.54). So fewer command
buffers save ≤ 0.02 ms; one submission only moves other processes' work
and would delay the GPU's start on the scene in CPU-bound frames (lower
GPU utilisation at 1080p Original): not adopted. The aid stays (off).

### Step 4: cars in the depth pre-pass — not done (does not pay)
Measured at 4K Extreme: the cars' opaque pass is 0.17 (racing) / 0.44 ms
(race start); enhanced glass (blended, never in a pre-pass) 0.69 / 1.27.
In the world's pre-pass the cars would hide the world behind them, which
the native motion blur between world and cars and the car glass both need
drawn. A car-only pre-pass before the car colour pass can at most
remove the cars' own overdraw (inferred ≤ 30–50 % of 0.17–0.44 ms) and adds
a second geometry pass of the high-detail car meshes at SSAA size: inferred
≤ 0.1 ms net, at race start only. Left open.

### Results
`perf.sh bench SET=d11 REPS=3` (new set; `user` = player-settings's
Enhancements values), base / new interleaved, 600 frames, medians
(internal notes; base and new ranges disjoint in every 4K
config):

| config | GPU busy base → new (ms) | change | SSR chain | motion blur tiles |
|---|---|---|---|---|
| racing user 4K | 9.54 → 8.55 | −10.4 % | 3.45 → 2.55 | — |
| explore user 4K | 8.54 → 7.81 | −8.6 % | 3.19 → 2.40 | — |
| rain user 4K | 11.80 → 10.93 | −7.4 % | 4.21 → 3.32 | — |
| race start user 4K | 10.80 → 9.81 | −9.1 % | 4.29 → 3.20 | — |
| racing Ultra 4K | 9.68 → 8.89 | −8.2 % | 2.13 → 1.34 | — |
| racing Extreme 4K | 13.27 → 12.52 | −5.7 % | 2.13 → 1.41 | 0.38 → 0.23 |
| race start Extreme 4K | 16.14 → 15.13 | −6.3 % | 2.47 → 1.70 | 0.36 → 0.23 |
| rain Extreme 4K | 19.66 → 18.83 | −4.2 % | 2.46 → 1.77 | 0.37 → 0.23 |
| explore Extreme 4K | 11.76 → 10.97 | −6.7 % | 2.06 → 1.35 | 0.34 → 0.23 |
| racing user 1080p | 2.69 → 2.48 | −7.8 % | 0.85 → 0.63 | — |
| racing Ultra 1080p | 2.79 → 2.63 | −5.7 % | 0.52 → 0.35 | — |
| racing Original 1080p | 0.714 → 0.718 | noise | — | — |

The player's SSR chain at 4K: 3.45 → 2.55 ms (racing; the plan's target
~2.4): what remains is the full-size march (1.25 ms) and the pyramid's
base (it now also classifies the mask). CPU per thread: "same" / not
significant in every config (explore Extreme's main thread 1.06 → 1.28 ms
has disjoint ranges but no other config moves; the SSR/blur CPU work per
frame is a few bind groups; inferred noise from the shared machine).

### What is left
* The player's SSR: the trace's dependent hi-Z loads (1.25 ms full size);
  only a non-exact change (fewer steps, half-size tier) would cut it.
* Pyramid levels as one SPD compute pass (−0.05…0.08 ms, inferred); the
  base can't move to compute (it writes the Depth16 skip mask).
* DOF: tile classification of in-focus blocks (open, needs a proof that
  their half-size gather is never read with a weight).

## Render-target memory (2026-11-05)

The VRAM and post pass (the same study's §4.11 "Memory"). Memory only: no pass, shader or draw
changed, nothing traced in SPEED2.EXE (no native behaviour touched). The
Original look keeps its own targets; every step is exact (gate below).

### Measuring: the allocator report
`NFSU2_VRAM_REPORT=file` (`prof::allocator_report`, written at a
`--bench`'s end; nothing when unset): wgpu-hal's gpu-allocator report
grouped by label (count, MiB), with the allocated and reserved totals and
the process's own VRAM/GTT now (amdgpu fdinfo). The bench JSON's
`vram_peak_kib` is the number compared below (process peak, fdinfo).
Reports: the maintainer's private captures.

The profile showed that the largest waste was not overlapping lifetimes
but the allocator: at 4K Ultra 2051 MiB were allocated in 3072 MiB of
blocks (Extreme 2680 in 3648, max 3068 in 3648; 1080p Original 380 in
512–832, different run to run).

### Step 1: allocator block sizes (`render/mod.rs` `memory_hints`)
wgpu's default `MemoryHints::Performance` sub-allocates device memory in
128–256 MiB blocks; a screen-sized 4K target of 130 MiB (SSAA 2x Rgba16F)
then sits alone in a 256 MiB block, and a block freed by a resize is
rarely refilled. Now `MemoryHints::Manual { 4..32 MiB }`: small resources
pack into blocks of 4 MiB growing to 32 MiB, every target over 32 MiB gets
a block of exactly its size. `NFSU2_GPU_BLOCK_MIB=min,max` / `perf` (wgpu's
default) for A/B. Tried: 8–64 (wgpu's `MemoryUsage`), 16–128, 4–16: 4–32
and 4–16 equal within 30 MiB, 8–64 +170 MiB at max, 16–128 +370 MiB.
Where a resource sits in memory changes no pixel.

### Step 2: the target pool (`render/pool.rs`), opt-in per pass
`gpu.pool.get(device, Slot, label, size, format, samples, usage)`: every
request for the same slot and descriptor gets the same texture (held by
`Arc`, the pool keeps a `Weak`: freed with its last holder, so a resize or
a tier switched off frees it). Users of a slot must each rewrite all of it
before reading it and read it only before the next user's first write in
queue order (wgpu's barriers order them). `NFSU2_RT_POOL=0`: every request
its own texture (A/B). Slots and users, in frame order:

| slot | users (frame order) | why it is safe |
|---|---|---|
| `SceneCopy` | Enhanced Glass's scene copy (car pass) → motion blur / DOF copy (`blur_and_dof`, after the cars) → SSR's scene copy (post pre-passes) | each clears + copies (or resolves) the whole scene first; nothing reads it after its own pass |
| `SceneAux` | motion blur velocity / DOF circle of confusion → SSR's glow sprites | each cleared (or fully resolved into) before use |

Plus, inside `PostFx::multisampled`: with High Precision Post the
multisampled streak copy has the scene's format and is the multisampled
scene itself. The scene is resolved (`pre_passes`) before the streak copy
is first written (SSR's glows clear it; `apply`'s glows rewrite every
sample from the streak buffer first), nothing draws into the multisampled
scene after that resolve, and each frame's main view clears it. Without
High Precision Post (the Original look) the formats differ (Bgra8 scene,
Rgba8 streaks): two textures, as before.

Not pooled (lifetimes cross the frame or overlap): the native screen copies
(full 0/1, half A–C, the streak buffer: the streaks decay across frames),
SSR's mask (world pass → SSR, overlaps the glass and blur copies), the
temporal / FSR history, water, smoke and volume_fx targets (other units'
internals; they can opt in through the pool).

### Step 3: targets of tiers that are off are released
`PostFx::release_off_tiers` (each frame's pre-passes): SSR (and its glow
target) when SSR is off, SSAO, Volumetric Fog, the motion blur / DOF
targets when both rows are off and no photo DOF; the world's SSR mask when
SSR is off (`render/mod.rs`); Enhanced Glass's copy when the row is off
(`car_glass.rs`); the "before the graphics passes" intermediate when no
Graphics-page pass runs. Before, a row switched off mid-session kept its
targets until the resolution changed (at 4K max SSR's targets and mask are ~0.8 GB).
They are made again when switched back on.

### Results (own VRAM peak, MiB, `--bench racing` 300 frames, reference desktop GPU)

| config | before | after | Δ | of which pool (step 2) |
|---|---|---|---|---|
| Original 4K | 1369 | 1354 | −15 | 0 |
| Modern 4K (MSAA 8x + HP Post) | 2103 | 1549 | −554 | −499 |
| Ultra 4K | 3273 | 2439 | −834 | 0 |
| Extreme 4K | 3936 | 2695 | −1241 | −446 |
| Extreme FSR 4K | 2070 | 1532 | −538 | (MS streak −176) |
| play setup 4K | 3574 | 2572 | −1002 | — |
| max 4K | 4008 | 3162 | −846 | −443 |
| race start max 4K | 4492 | 3163 | −1329 | — |
| Original 1080p (3 runs) | 617 / 973 / 878 | 624 / 636 / 629 | −0…−349, now steady | 0 |
| Ultra 1080p | 1238 | 1152 | −86 | — |
| Extreme 1080p | 1639 | 1250 | −389 | — |

The plan's expectation (−0.3…0.8 GB at 4K) is exceeded at every enhanced
4K preset; Original moves only by the allocator's packing (its native
targets are its own). GTT (host-visible blocks) 66 → 6 MiB.

### D16 (compute Original post chain for iGPUs): measured, not done
reference integrated GPU, 1080p Original (`IGPU=1`, one run each, the fastbuild binary
before the VRAM and post pass): racing GPU busy 12.8 ms, of it the native post chain 4.45 ms
(35 %): bleach bypass 1.71 (three fullscreen passes), streaks 1.37
(decay, glows, add A), overbright 1.0 (seven half-size passes), to the
back buffer 0.36; race start 14.9 ms with the same post (4.6 ms).
Why it was not built in this work:
* The bleach bypass is the largest part, and the neon pass's fused bleach (the same
  float order, each 8-bit hand-off emulated by `q`) already fails the
  self-check on this device (1 629 412 of 2^24 colours ±3, the RAMPMAP
  lookup at the re-quantised (g, b)): a compute version that fuses the
  three passes meets the same rounding and would not be AE 0; a compute
  version that keeps the three passes saves nothing (same bandwidth).
* The streaks need the glow sprites (rasterised, additive) between the
  decay and the add, so only add A + "to the back buffer" can fuse
  (one fullscreen streak read, inferred ≈ 0.1–0.15 ms on the iGPU).
* Overbright's C and A already pass the self-check fused (MRT); they are
  off only because the fused flag is shared with the bleach. Splitting the
  verdict per pass would save one half-size pass (inferred ≈ 0.1 ms iGPU)
  exactly — the next small step (needs the verdict cache's format per
  pass).
Open: find the RAMPMAP coordinate rule that makes `q` match the hardware's
8-bit hand-off (then fused bleach in one pass, ≈ −1.1 ms on the iGPU,
inferred from 3 → 1 fullscreen passes).

### Gate
`gate.sh … <base> extreme` with `GATE_ENH_PRESETS="modern ultra user
extreme extreme_fsr"` (Modern = 8x MSAA + High Precision Post, the
aliased multisampled streak copy): checks PASS (1144 tests, selftest,
29 shots, flows, playthrough); binaries differ (md5 ef543af7… base,
eebb2581… new); Original images 1280×960 + 4K, window path, enhanced
(10 sets incl. Modern/user/Extreme/Extreme FSR), 29 shots: AE 0; audio
identical; state hashes identical (firstrace, firstdrag, postrace,
playthrough, parity). Gate logs: internal evidence.

Perfgate (against the frame-budget gate baseline 99e726f): FAIL on one metric,
racing Ultra 4K cpu_render +9 % (p 0.016); every GPU metric same or better,
own VRAM −5…−31 % (`perfgate.txt`). Rerun of that config against this
unit's own base (<base>, REPS=10, `ab10-base-vs-new.md`): cpu_render
3.44 → 3.34 ms (−3 %, same), GPU 8.90 → 8.79 (same): not confirmed (a
chance flag on the shared machine; the per-frame code added is four
atomic loads). Load times: same within noise (+30 % medians, p 0.17–0.35,
noise 40–53 %); more, smaller blocks mean more `vkAllocateMemory` calls at
load (inferred cost < 1 ms): open, needs `SET=loads REPS≥10`.

## The native particles' own mark

The native particles' pass (`draw_native_particles`, render/native/world/sprites.rs) had no GPU mark of its own:
its time went to the next mark in the encoder, the spray volume's build ("graphics: volume fx build") with Wet Road
Interaction on, else the weather effects ("graphics: weather fx"). Measured with the passes' dispatches timed apart
(night rain, `--bench drive` at clock 162): at 4K with the play setup's rows the "volume fx build" of 0.48 ms was the
build's 0.07 ms (splat, resolve, bounds, light) and the native particles' 0.39 ms; on the reference integrated GPU at
level High 0.32 ms and 1.15–1.24 ms. The pass is now marked "graphics: native particles" (a timestamp after it;
nothing drawn changes), so the build's and the weather effects' rows hold only their own work.
