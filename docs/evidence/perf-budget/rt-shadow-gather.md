# Modern + RT's gather: measurements behind docs/perf-budget.md "the colour shader's registers"

Reference desktop GPU, 3840x2160, the play setup (SSAA 2x: scene 5431x3055, 4.15 M blocks),
`--bench racing 300`, one GPU-timed run per arm unless "paired" (interleaved A/B pairs).

## Where the gather's cost sits (prototypes, main view ms; base 6.08–6.20 over 9 runs)
| arm | main view | world PS VGPRs needed / allocated / waves | reflect PS |
|---|---|---|---|
| base | 6.08–6.20 | 126 / 144 / 10 | 161 / 168 / 9 |
| gather left out (traced lights take their fallback) | 5.05–5.10 | 112 / 120 / 12 | 143 / 144 / 10 |
| gather left out in the world function only | 5.67 | | |
| gather left out in the reflect function only | 5.62 | | |
| gather without its 11 loads (same arithmetic) | 6.04 | | |
| exact short cuts (uniform blocks, no light) alone | 6.06 | 126 / 144 / 10 | 161 / 168 / 9 |
| taps in a loop | 6.67 | 126 / 144 / 10 | 161 / 168 / 9 |
| world module without the ray query (wave64) | 6.57 | | |
| Modern (no RT), wave64 / forced wave32 | 4.99 / 5.12 | | |

## Block statistics (a counting shader, racing frames 100 and 200)
| | frame 100 | frame 200 |
|---|---|---|
| sky / ceiling or no rays / ranked blocks | 20.7 / 8.2 / 71.1 % | 18.9 / 8.0 / 73.0 % |
| ranked blocks with a light in slot 0 / slot 1 | 38.3 / 0.2 % | 31.5 / 0.6 % |
| geometry blocks with no traced light in their 3x3 | 63.8 % | 69.1 % |
| wave32 tiles tracing any ray; lanes tracing inside them | 30.2 %; 90 % | 27.5 %; 84 % |
| lights per cluster (mean, p90); distinct clusters per wave | 6.3, 7; 1.16 | 5.3, 7; 1.18 |

## The gather first (exact)
Frames identical to before: racing 0, 100, 200, 300; race start 0, 100, 200; night rain (clock 162) 0, 120, 240.
| | main view | frame |
|---|---|---|
| before (6 runs) | 6.12 [6.02–6.27] | 19.03 [18.72–19.22] |
| after (6 runs) | 6.01 [5.85–6.12] | 18.98 [18.66–19.14] |
| after − before, per pair | −0.15 [−0.22 .. +0.05] | −0.12 [−0.22 .. +0.17] |

## + the gather's short cuts (exact)
Frames identical to before (racing, race start, night rain as above; the same short cuts in the prototype).
GPU test `gather_short_cuts_are_exact`: 8192 pixels over 32 x 32 blocks (lit, shadowed, penumbra, no light,
two lights, a light's edge, a depth step, a normal crease; on and 0.5 m off the plane): ids and sums equal.
| round (base, step 1, this) | main view | frame |
|---|---|---|
| 1 | 6.10, 6.01, 5.88 | 18.85, 18.86, 18.69 |
| 2 | 6.12, 6.04, 5.87 | 18.88, 18.91, 18.68 |

## The blocks follow the screen under supersampling (non-exact under SSAA)
Without supersampling: frames identical (racing 0, 100, 300 at 4K with SSAA off). With SSAA 2x at 4K
(8.29 M output pixels), against the frames before (pixels differing at all / by > 1 % / by > 3 %,
largest difference in levels, 1 − SSIM):

| frame | differ | > 1 % | > 3 % | max | 1 − SSIM |
|---|---|---|---|---|---|
| racing 0 / 100 / 200 / 300 | 0.67 / 0.69 / 1.14 / 0.69 % | 0.19 / 0.21 / 0.30 / 0.08 % | 0.041 / 0.041 / 0.049 / 0.016 % | 47 / 77 / 68 / 61 | 9.8e-5 / 1.0e-4 / 1.6e-4 / 3.8e-5 |
| race start 0 / 100 / 200 | 0.29 / 0.39 / 0.61 % | 0.05 / 0.05 / 0.14 % | 0.008 / 0.011 / 0.031 % | 53 / 61 / 64 | 2.1e-5 / 2.0e-5 / 9.1e-5 |
| night rain 0 / 120 / 240 | 1.02 / 0.80 / 1.09 % | 0.18 / 0.10 / 0.11 % | 0.025 / 0.013 / 0.013 % | 35 / 58 / 40 | 1.0e-4 / 6.6e-5 / 4.0e-5 |
| casino canopy at rest 0 / 60 | 1.51 / 1.60 % | 0.47 / 0.49 % | 0.085 / 0.090 % | 51 / 53 | 2.3e-4 / 2.1e-4 |
| casino canopy in rain 0 / 60 | 1.39 / 1.43 % | 0.34 / 0.33 % | 0.048 / 0.043 % | 32 / 39 | 1.8e-4 / 1.7e-4 |

Luma mean, p5 and p95 unchanged in every frame (e.g. casino 90.40 / 12.2 / 214.4 both). The largest
differences are one- or two-pixel slivers where a lamp's penumbra crosses a curved silhouette (palm
trunks, the casino's facade edges); at 200 % the frames look the same. In motion (20 consecutive
frames, racing 150–169 and night rain 100–119): the mean frame-to-frame change and its second
difference are within 0.01 % of before; the difference image's temporal standard deviation averages
0.04 / 0.02 levels (99.9th percentile 3.9 / 2.7): no added flicker.

## The road's dry-road pipelines (exact)
Frames identical to before: dry racing 0, 100, 300; night rain (the road wetting in its first frames) 0, 1,
2, 3, 120 (a second run of each identical; one first run differed in 10 pixels by one level at frame 120,
not repeated). Shader statistics of the road's function (needed / allocated VGPRs, waves per SIMD):
Modern + RT 153 / 168 / 9 → 123–125 / 144 / 10; Modern 143 / 144 / 10 → 115–116 / 120 / 12; integrated
GPU Medium 79–80 / 80 / 12 → 60–62 / 64 / 16. Pipelines at a drive's load: 442 → 448 (the six dry ones).

| runs (step before, this) | main view | frame |
|---|---|---|
| Modern + RT racing, 3 pairs | 5.80 [5.80–5.84], 5.62 [5.58–5.63] | 18.12, 17.99 |
| Modern racing, 2 pairs | 4.93 [4.92–4.94], 4.62 [4.60–4.63] | 16.33, 16.01 |
| night rain (wet: the full set), 1 pair | 6.58, 6.55 | 22.31, 22.24 |
| integrated GPU Medium, 2 pairs (slow state) | 11.44 [11.33–11.55], 10.27 [10.18–10.35] | 24.64, 23.44 |
| integrated GPU High, 2 pairs (slow state) | 11.88 [11.81–11.95], 11.02 [10.99–11.04] | 44.93, 43.81 |
Cold (no pipeline cache, the driver's cache off), one run each on a loaded machine: `--prepare` with the
play setup's options 353 pipelines in 2.96 s → 359 in 3.42 s; a drive's load at the play setup at 4K
436 → 442 pipelines, load 6.8 → 4.9 s (noise from other jobs: no increase seen).

