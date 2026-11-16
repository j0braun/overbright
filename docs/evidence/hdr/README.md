# HDR Output evidence (2026-11-04; docs/hdr.md)

reference desktop GPU, RADV Mesa 26.2.5, Vulkan, headless (`NFSU2_HDR_FAKE=pq`, the
in-game post passes and display transform into an HDR texture; no HDR
display on this machine). Machine shared with other jobs (load 9–29).

## Grade LUT against the original (Wine, apitrace replay)
`$OB_PRIVATE/devprobes/src/bin/grade_lut.rs wine IN OUT R G B W`; IN = the back buffer after the
frame's last scene draw (input of StretchRect → screen copy 0), OUT = the
render target after screenfilter.efx's p2 draw. Snapshots by
`$OB_PRIVATE/wine/runs/<run>/retrace.sh` (d3dretrace on DXVK, display :220)
of <work dir>'s traces; raw PNGs in `<work dir>/wine/`.

```
fr.trace frame 13971 (free roam), calls 139478949 → 139479441
amount 0.760 (fitted; TINTCOLOR (0.1659345, 0.1385889, 0.0827063, 0.1200001))
original vs ported native passes: 1228800 texels: exact 99.9987 %, within ±1 100.0000 %, max ±1, mean |d| 0.0000 steps/channel
original vs LUT, tetrahedral: 1228800 texels: exact 36.8315 %, within ±1 99.1366 %, max ±2, mean |d| 0.2961 steps/channel
original vs LUT, trilinear: 1228800 texels: exact 35.5426 %, within ±1 99.0868 %, max ±2, mean |d| 0.3010 steps/channel
input colours: 182517 distinct

dark.trace frame 15478 (free roam, night), calls 164460096 → 164460588
amount 0.848 (fitted; TINTCOLOR (0.1695995, 0.1213803, 0.1089093, 0.1250000))
original vs ported native passes: 1228800 texels: exact 99.9840 %, within ±1 100.0000 %, max ±1, mean |d| 0.0001 steps/channel
original vs LUT, tetrahedral: 1228800 texels: exact 32.8982 %, within ±1 99.1064 %, max ±3, mean |d| 0.3177 steps/channel
original vs LUT, trilinear: 1228800 texels: exact 32.4145 %, within ±1 99.3548 %, max ±3, mean |d| 0.3163 steps/channel
input colours: 127971 distinct
```

## Grade LUT against the ported passes, every 24-bit colour (`grade_lut verify`)
```
front end: lattice codes 32768 texels: exact 100.0000 %, max ±0
front end: tetrahedral 16777216 texels: exact 32.9381 %, within ±1 98.8016 %, max ±3, mean |d| 0.3182
front end: trilinear   16777216 texels: exact 34.4759 %, within ±1 98.9880 %, max ±3, mean |d| 0.3064
game, tinted: lattice codes 32768 texels: exact 100.0000 %, max ±0
game, tinted: tetrahedral 16777216 texels: exact 36.3811 %, within ±1 99.6026 %, max ±3, mean |d| 0.2904
game, tinted: trilinear   16777216 texels: exact 38.5725 %, within ±1 99.7205 %, max ±2, mean |d| 0.2749
game, no tint record: lattice codes 32768 texels: exact 100.0000 %, max ±0
game, no tint record: tetrahedral 16777216 texels: exact 36.8129 %, within ±1 98.4313 %, max ±3, mean |d| 0.2920
game, no tint record: trilinear   16777216 texels: exact 38.5388 %, within ±1 98.5420 %, max ±3, mean |d| 0.2799
```
`game.strip.png` (Frame::GAME) and `freeroam_fr13971.strip.png` (the
fitted free-roam frame): the 32³ LUTs, texel (r + 32·b, g), exact RGBA8.
`grade_lut cube OUT.cube [R G B W AMOUNT]` writes them as .cube.

## HDR shots (1280 × 720, PQ, paper 203 nits, peak 1000 nits, gamma 2.2)
`sbs_<shot>.jpg`: left the native image (HDR Output Off), right the HDR
signal tone-mapped for SDR viewing (`hdr_output::preview`: up to 0.8 ×
paper white as SDR, the rest compressed to white at the peak; only the
highlights look different by design). `garage_neon_crop_pq.png`: 16-bit
PNG of the PQ signal (BT.2020) around the neon pool (crop 480 × 200 at
400, 430 of the garage_neon frame).

| shot | brightest | above paper white | SDR frame (HDR on) vs native, px > 3 steps |
|---|---|---|---|
| explore (dusk) | 618 nits | 0.12 % | 6 |
| explore, rain (`NFSU2_RAIN_CLOCK=170`) | 545 nits | 0.07 % | 15 |
| garage_neon (Corolla underglow) | 758 nits | 3.81 % | 322 |
| qrstart (grid, tail lights) | 689 nits | 0.51 % | 14 |
| main (front-end garage) | 1000 nits | 0.85 % | 208 |
| garage_neon, scRGB | 756 nits | 3.89 % | – |

`calibration_page_preview.jpg`: Graphics → Display with HDR Output on (the
pattern: peak box, paper white / half, black steps, gamma lines vs solid).
`calibration_page_sdr_unsupported.jpg`: the same page without an HDR
signal (rows greyed, "Not Supported").

## Cost (`--bench explore 600`, gpu busy ms/frame, mean)
| size | Original | High Precision Post (Half) | HDR Output, PQ | HDR Output, scRGB |
|---|---|---|---|---|
| 1920 × 1080 | 0.62 | 1.01 | 1.00 | 1.06 |
| 3840 × 2160 | 2.31 | 4.44 | 4.71 | 4.82 |

4K breakdown, HDR Output PQ: grade LUT 0.94 (HP's bleach 0.92), last pass
(two targets) 0.22 (HP 0.14), display transform 0.15; the rest is High
Precision Post's half-float chain (main view 1.37 vs 0.73, streaks 1.33
vs 0.44). Over Original +2.40 ms at 4K, +0.38 at 1080p.

## Native byte-identity (HDR Output Off), 1280 × 960 vs <base>
main, explore, intro, qrcars, race, qrstart, drift, drag, garage_neon,
options_graphics_effects: md5 identical. career: 158 px differ; two <base>
runs differ by 53 px (turntable, not deterministic).
