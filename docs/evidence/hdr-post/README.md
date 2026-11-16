# High Precision Post: before / after (2026-11-04)

Clone shots only (an enhancement: nothing to capture in the original).
1920 × 1080, `--shot`, detail 5, lossless WebP. `*_native` = row Off,
`*_hdr` = `NFSU2_SHOT_ENH="enh.hdr_post 2"` (Full-Size Glow).

| pair | shot | env |
|---|---|---|
| city | explore (Bayview, dusk) | – |
| wet | explore, night shower | `NFSU2_RAIN_CLOCK=170` |
| garage | main (front-end garage) | – |
| racestart | qrstart | – |

`zoom_glow_*.png`: explore at 1280 × 960, 400 × 200 crop around the
player's tail lights and headlight pool, 2× point-scaled: the native
half-size comb blur's hard top edge vs the smooth Gaussian halo.

Mean brightness native → hdr (whole frame): city 0.4033 → 0.4034
(1280 × 960), wet 0.3867 → 0.3869, garage 0.3573 → 0.3571, racestart
0.3875 → 0.3876. Distinct colours in a 1200 × 150 sky/background crop
(banding indicator): city 4662 → 7280, wet 2833 → 3162, race 8736 → 9752.

Before the fix found while matching (overbright A unsaturated: (x·g)²
reaches 3.1 at white, native's 8-bit A clips it) the glow region was
+0.05 brighter; after, within 0.002.

## Native byte-identity (row Off) vs <base>, 1280 × 960, md5
explore, intro, qrcars, race, drift, drag: identical. ending: two
run-to-run variants in both builds, the same two md5s
(b8f2d58f…, b0277348…). main, career: no two runs agree in either build
(base vs base differ by 15–690 pixels); the new build's runs are as close
to base's (min 14.6 / 26.7 pixels) as base's are to each other (14.7).

## Cost, `--bench explore 600`, reference desktop GPU, Vulkan (gpu busy ms/frame, mean)
| size | Off | Half-Size Glow | Full-Size Glow |
|---|---|---|---|
| 1920 × 1080 | 0.64 | 0.92 | 1.09 |
| 3840 × 2160 | 2.28 | 4.37 | 5.07 |
4K Full breakdown (Off → on): main view 0.77 → 1.37 (half-float scene),
bleach 0.45 → 0.91, overbright 0.16 → 0.93, streaks 0.45 → 1.25, last
pass 0.08 → 0.13. The machine was shared with other builds (load
~20–30); figures are indicative.

All-on check (no validation error, no panic): `--shot explore` and
`--bench explore` at 1080p with SSAA 4x + MSAA 4x + SMAA+FXAA + CAS 50 +
Enhanced Reflections + SSAO 50 + Reversed-Z + LOD Fade + Full-Size Glow;
`--bench race` with MSAA 8x + Half-Size Glow.
