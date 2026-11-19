# Ambient Occlusion: the passes before and after the four steps (docs/notes/performance/integrated-gpu.md)

Method. The passes alone: the bench dumped the scene depth and the pass's uniform of real frames (Medium
at 1920×1080 World Detail 3: racing at the start line (call 120) and under way (240), the race start (60),
night rain (60); every row up with ray tracing at 3840×2160 with SSAA 2× (5431×3055 scene, 120 and 60)); a
throwaway harness ran today's four passes and each variant on them, interleaved per round (ABAB…), GPU
timestamps around every pass, 40 (integrated) or 90 (discrete) samples per variant; medians (10th–90th
percentile in brackets). Integrated: an APU's GPU with 2 compute units; discrete: a desktop GPU with 60.
Each series under the machine's measuring lock, load average 25–50 (other units' builds).

Steps: s1 the taps' loads in one batch (no branch per tap); s2 + the apply skips the sky and unoccluded
pixels; s3 + the occlusion's arithmetic (spiral table turned once per pixel, multiply-add positions, two
gathers); s4 + the blur's early way out.

## Integrated GPU (ms)

| frame | variant | depth | occlusion | blur | apply | all four |
|---|---|---|---|---|---|---|
| racing, start line | before | 0.29 | 1.26 | 0.39 | 1.75 | 3.70 |
| | s1 | 0.29 | 1.21 | 0.39 | 1.74 | 3.64 |
| | s2 | 0.30 | 1.21 | 0.39 | 1.01 | 2.92 |
| | s3 | 0.30 | 1.06 | 0.39 | 1.02 | 2.77 |
| | s4 | 0.30 | 1.07 | 0.30 | 1.02 | 2.68 (−28 %) |
| racing, under way | before | 0.19 | 1.25 | 0.40 | 1.90 | 3.76 |
| | s4 | 0.19 | 1.03 | 0.28 | 0.81 | 2.31 (−39 %) |
| night rain | before | 0.26 | 1.75 | 0.40 | 1.77 | 4.17 |
| | s4 | 0.25 | 1.33 | 0.31 | 1.08 | 2.96 (−29 %) |
| race start (slow memory state) | before | 0.19 | 3.60 | 0.39 | 2.25 | 6.43 |
| | s4 | 0.19 | 3.30 | 0.31 | 1.07 | 4.89 (−24 %) |

The occlusion's slow state (2.5–3× the time, the other passes unchanged) comes and goes over time: seven
separate allocations of the linear depth in one process were all fast or all slow together, in runs a few
seconds apart, and the GPU's core clock (hwmon `freq1_input`, sampled every millisecond during the timed
rounds) was at its 2200 MHz top in 99–100 % of the samples in slow and fast runs alike. Not the allocation,
not the core clock; inferred: memory or fabric contention (the CPU's 16 threads were building for other
units), not verified with counters. Within one run both arms are in the same state: the steps save in both.

## Discrete GPU (ms)

| frame | variant | depth | occlusion | blur | apply | all four |
|---|---|---|---|---|---|---|
| 4K racing, every row up | before | 0.175 | 0.419 | 0.072 | 0.479 | 1.147 |
| | s1 | 0.177 | 0.370 | 0.072 | 0.480 | 1.101 |
| | s2 | 0.178 | 0.381 | 0.072 | 0.278 | 0.908 |
| | s3 | 0.174 | 0.392 | 0.073 | 0.279 | 0.916 |
| | s4 | 0.174 | 0.391 | 0.057 | 0.278 | 0.899 (−22 %) |
| 1080p racing, under way | before | 0.010 | 0.044 | 0.015 | 0.042 | 0.112 |
| | s4 | 0.015 | 0.034 | 0.016 | 0.033 | 0.099 |

## In the game (bench, GPU timer rows, medians of n runs, min–max)

Integrated, Medium 1920×1080 detail 3 (the SSAO rows: depth + occlusion + blur + apply; "the draws
before" is the new row that holds the characters' and mirror's draws, before part of "ssao depth"):

| scene | before | after | whole frame before → after |
|---|---|---|---|
| Medium racing | 4.87 (4.08–5.97), n 8 | 3.80 (2.93–4.77), n 8 | 25.23 → 23.90 |
| Medium rain | 5.90 (4.54–6.18), n 8 | 4.25 (3.87–5.18), n 7 | 29.35 → 27.76 |
| Medium race start | 5.41 (4.38–5.77), n 7 | 4.59 (3.12–5.11), n 7 | 26.42 → 25.83 |
| High racing | 6.13 (5.48–6.22), n 8 | 4.58 (4.44–4.80), n 8 | 49.09 → 47.34 |
| High rain | 6.33 (5.91–6.56), n 7 | 4.82 (4.59–5.02), n 7 | 64.97 → 63.13 |
| Medium, World Detail 5 (8× MSAA) racing | 6.66 (6.40–6.75), n 3 | 5.82 (5.40–6.06), n 3 | 43.46 → 43.91 (spread 40–45) |

The apply alone (Medium): 2.02 → 1.12 (racing), 1.99 → 1.26 (rain), 1.69 → 1.11 (race start); the blur 0.43–0.47 →
0.34–0.38. Under 8× MSAA the apply gains less (2.17 → 1.74): the multisampled depth read before the skip.
The occlusion row is bimodal in the game as in the harness (1.2–3.4 ms).

Discrete, every row up with ray tracing, 3840×2160 (SSAA 2×), n 6–8 each: racing 1.01 → 0.80 ms (frame
18.94 → 18.67), rain 1.01 → 0.81 (23.11 → 22.94), race start 0.99 → 0.83 (20.53 → 20.39).

## Pictures (bench frame dumps, before → after s3; s4 adds ≤ 0.03 % of the occlusion's texels by one 16-bit step)

| frames | pixels changed | > 1 % | > 3 % | largest | 1 − SSIM | luma mean |
|---|---|---|---|---|---|---|
| racing in motion, 16 consecutive (Medium) | 4.6 % | 0 | 0 | 2 | 7.9e-5 | 62.80 → 62.81 |
| night rain in motion, 16 consecutive | 5.0 % | 0 | 0 | 2 | 6.7e-5 | 66.24 → 66.25 |
| race start (crowd varies run to run) | 3.4 % | 1 | 0 | 3 | 2.8e-5 | 40.85 → 40.85 |
| free roam in motion, 11 consecutive | 6.1 % | 0 | 0 | 2 | 1.0e-4 | 73.53 → 73.55 |
| 4K racing (every row up), 6 consecutive | 5.4 % | 0 | 0 | 2 | 8.9e-5 | 53.89 → 53.91 |
| at rest (frozen view), free roam Medium, 11 frames (step 4) | 5.6 % | 0 | 0 | 2 | 8.8e-5 | 69.76 → 69.78 |
| at rest (frozen view), 4K every row up, 5 frames (step 4) | 4.3 % | 0 | 0 | 2 | 7.1e-5 | 44.44 → 44.45 |

Frame-to-frame change (mean absolute, consecutive frames): in motion racing 7.174 → 7.173, rain 5.602 →
5.601, free roam 3.746 → 3.742; at rest 0.4718 → 0.4720 (free roam) and 1.1110 → 1.1112 (4K): no flicker added. The changed pixels are those the old apply multiplied by
one 16-bit step under 1 (now left as they were) and a few hundred per frame where a tap lands one texel
over.

## On the base with the glow rectangles and the screen-half occlusion under SSAA

In the game (bench GPU rows, base vs the four steps, n 3 each unless noted; the 2:1 grid on the integrated GPU,
the screen-half grid under SSAA 2× on the discrete):

| | depth | occlusion | blur | apply | SSAO rows | frame |
|---|---|---|---|---|---|---|
| discrete 4K racing, before → after | 0.14 → 0.14 | 0.18 → 0.15 | 0.04 → 0.04 | 0.34 → 0.21 | 0.70 → 0.53 | 17.68 → 17.45 |
| discrete 4K rain | 0.11 → 0.11 | 0.21 → 0.17 | 0.05 → 0.04 | 0.34 → 0.21 | 0.70 → 0.52 | 21.57 → 21.50 |
| integrated Medium racing (n 6) | 0.17 → 0.18 | 2.93 → 2.51 | 0.44 → 0.33 | 1.72 → 1.14 | 5.26 → 4.20 | 24.63 → 24.21 |
| integrated Medium rain (n 5) | 0.18 → 0.18 | 2.85 → 2.79 | 0.47 → 0.37 | 1.77 → 1.17 | 5.25 → 4.47 | 28.59 → 27.83 |

Pictures against that base: rain frames 96–105 at 4K SSAA 2× 5.9 % of pixels by ≤ 2 levels, none > 1 %, 1 − SSIM
8.6e-5 (frame 100's angled brick wall: 3 % of its crop by 1 level, no column pattern); 4K racing 6.0 %, ≤ 2;
Medium 1080p racing in motion 4.6 %, ≤ 2 (the same as on the earlier base). Frame-to-frame change identical.
