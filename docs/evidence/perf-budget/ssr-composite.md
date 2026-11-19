# Screen-space reflections: the composite by the blend, and the march (docs/notes/graphics/reflections.md "Wet Roads and Screen-Space Reflections")

GPU timestamps per pass, `--bench racing 200`; the desktop GPU at 3840x2160 with the play setup at 4K
(SSAA 2x, ray-traced reflections, High Precision Post), the integrated GPU at 1920x1080, World Detail 3,
High (SSR at half the screen's size). Medians of interleaved runs, ranges in brackets; load average 15–42.

## The composite by the blend (a half-float scene), against the build before it
| | copy + depth pyramid | composite | frame | runs |
|---|---|---|---|---|
| desktop 4K | 0.737 (0.729–0.758) → 0.560 (0.555–0.578) | 0.389 → 0.308 | 18.70 (18.63–18.79) → 18.28 (18.24–18.32) | 4 / 4 |
| integrated High | 1.07 (1.02–1.12) → 0.80 (0.79–0.81) | 0.74 → 0.60 | within the spread (42.0–44.0 both) | 4 / 4 |
The integrated GPU's runs were in its slow state (the build machine's memory contention, SSAO occlusion about
3 ms instead of 1.0–1.3): the copy and composite are memory-bound and likely cheaper on a player's machine,
before and after alike.
Picture (the same frames, ImageMagick and a per-pixel count; largest channel difference): desktop 4K racing
frames 100–104 and 200 in motion: 2.5–4.9 thousand pixels of 8.3 million differ, at most 2 steps, none by
more than 1 %; night rain 100–102 and 140: about 22 thousand (0.26 %), at most 2; a still (frames 100, 149):
at most 1; 1 − SSIM ≤ 2.3e-6, luma mean, p5 and p95 unchanged; the integrated GPU High 60–62 and 120: 400–600
pixels, at most 1. Each frame's difference is the half-float sum rounded once by the blend (no history:
nothing builds up from frame to frame). Again after SSAA's resolve (the later tree, frames every 40th): 4K racing
at most 1 step on up to 3.5 thousand pixels, night rain at most 1 on up to 23 thousand (0.28 %); 1920x1080
High on both GPUs at most 1; an 8-bit scene (the copy's form) identical. The gate's Extreme sets
(1920x1080): at most 1 step on 236–865 pixels; with FSR 1 (the 1114x626 scene upscaled and sharpened) at
most 12 steps, more than 3 % on 3 pixels of 29 million, more than 1 % on 0.004–0.015 %, 1 − SSIM ≤ 4.4e-6,
not visible at 400 %.

## The march (the counting build: per ray, 240 frames, racing)
| | desktop, full trace 3840x2160 | integrated High, half trace 960x540 |
|---|---|---|
| rays marched per frame | 3.6 million (43 % of the trace) | 0.22 million |
| steps per ray (cap) | 40.3 (64) | 29.7 (48) |
| rays that end at the cap without a hit | 20.5 % | 20.9 % |
| hits | 52 % | 52 % |
| steps down a level / behind a surface at level 0 | 29.9 % / 22.0 % | 33.9 % / 23.0 % |
| rays with no confidence whatever they meet (r.z <= -0.4) | 4.5 % (3.2 % of the steps) | 4.8 % (3.2 %) |
| a wave's lanes busy (8 x 8 tiles' longest ray) | 80.9 % | 74.6 % |
Those zero-confidence rays are no longer marched (exact): the trace −2.6 % (desktop, no ray tracing) and
−1.8 % (integrated). Tried and dropped, each against the same frames alternating: the nearest depth value
linearised once in the pyramid's base (exact, −0.01 ms), the cell by exponent instead of a reciprocal (exact,
−0.3 % / −1.3 %), the pyramid's seven levels in two compute dispatches (exact, +32 % slower than the seven
small passes), a step that re-reads a cell the walk just left taken from a kept copy (exact, +23 % / +50 %
slower: the second step in the loop costs more than the read it saves). Load balancing across a wave would
win at most the 19–25 % its lanes idle; not tried.
