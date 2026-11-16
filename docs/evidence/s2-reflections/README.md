# Reflection / wet-road leftovers (2026-11-04)

Build: <work dir>, reference desktop GPU, RADV; shots `--shot` at 1920×1080
(`NFSU2_SHOT_ENH`), scenes as in docs/graphics-settings-audit.md (explore = dusk free
roam, rain = `NFSU2_RAIN_CLOCK=170 explore`, qrstart, motion = circuit after 600 frames
at full throttle, intro = night free roam).

## Native (Wine, SPEED2.EXE, own prefix on :209)
`native-wetness-race.txt`: weather [0x832EB4] (view 1 0x832E50 + 0x64) in a dry Quick
Race: amount +0x1F0 = 0, target +0x5094 = 0, wetness +0x509C 0.0478 → 0.0223 over 8 s
(the 0.9985 step at ~65 steps/s), flare stretch 0x7F39A0 = 1.2 + 8.8·wet. An earlier
read 19 s after the load: 0.185 (1 at load). The road itself looked dry
(worldreflect at its authored strength).

## Changed pixels vs Off (any channel > 2 levels), before → after
| | explore | rain | qrstart | motion |
|---|---|---|---|---|
| Wet Roads | 5.15 % → **0 (md5 = Off)** | 11.63 → 11.63 % | 6.07 % → **0** | 2.54 % → **0** |
| SSR Half | 8.19 → 9.00 % | 6.76 → 7.38 % | 5.59 → 5.48 % | 5.20 → 4.64 % |

Glow sprites in the hits (`NFSU2_SSR_NOGLOW=1` vs not, SSR + Wet Roads): rain 0.21 %,
qrstart 0.12 %, explore 0.16 % of pixels, max 64 levels (the tail lights' glow on the
wet road below the car, `glow_rain.jpg`).

## Crops
`cars_qrstart.jpg` (Off | SSR before | after: car paint and glass traced),
`glow_rain.jpg`, `wet_dry_qrstart.jpg` (Wet Roads at a dry start: before wet, after =
Off), `wet_rain.jpg` (ripples at the world clock), `fog_explore.jpg` (fog once),
`blend_motion.jpg` (Enhanced Reflections off: short fade into the native capture),
`lamps_scale_intro.jpg` (item 8, below).

## Lamp shadows under SSAA / Render Scale (lamp shadows vs docs/post-scale.md)
The soft taps are shadow-map texels (1.5 texel steps of a fixed 512²/1024² array,
`shadow_visibility`), the offsets world metres, the normal normalised, the clusters
16 × 9 screen tiles in NDC: no term depends on the scene's pixel size. Measured
(Dynamic Lamps + Lamp Shadows High + Contact Shadows, the lamp term = On − Off, both
blurred σ 6, RMSE of full range): intro lamp term 0.110, its change 1× → SSAA 4x
0.00065, → 50 % 0.0017; qrstart term 0.053, change 0.00085 / 0.0020 (the scene alone
1× → 4x: 0.0035 / 0.0019). Scale-free; no change needed.

## GPU (ms/frame busy, `--bench <scene> 300`, NFSU2_BENCH_SIZE, one run, base → the reflections pass)
| scene size | Off | Wet (dry) | SSR Half | SSR Full | Ultra | Ultra + SSR Half |
|---|---|---|---|---|---|---|
| explore 1080p | 0.65 / 0.66 | 0.68 / 0.66 | 1.04 / 1.12 | 1.19 / 1.24 | 1.55 / 1.54 | 2.15 / 2.08 |
| race 1080p | 1.00 / 0.99 | 1.02 / 1.00 | 1.45 / 1.57 | 1.70 / 1.85 | 2.73 / 2.71 | 3.48 / 3.49 |
| explore 4K | 2.28 / 2.28 | 2.29 / 2.30 | 3.51 / 3.69 | 4.19 / 4.38 | 6.00 / 5.92 | 8.19 / 7.77 |
| race 4K | 3.41 / 3.42 | 3.44 / 3.46 | 5.01 / 5.40 | 6.16 / 6.63 | 9.89 / 9.77 | 12.82 / 12.56 |
Ultra = the preset's rows (SSAA 2x, HDR 2, …) without SSR. Inside Ultra at 4K the
passes (explore): trace 0.52 → 0.35, filter 0.52 → 0.28, copy + pyramid (+ glow
sprites) 0.39 → 0.68.
