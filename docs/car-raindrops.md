# "My car looks like it has raindrops on it" (player report)

Development build. The player's settings: the Ultra preset and then every
graphics row at its maximum (internal evidence: render scale 200, SSAA 16x,
MSAA 16x, filter 4, Sharpen 100, High Precision Post 2, road reflection full, car cube 1024, reflection
every frame, Enhanced Reflections, anisotropy 16, reversed-Z, Ultra shadows, SSAO 100, detail 300, car
detail 300, LOD fade, keep loaded, Enhanced Car Paint, Wet Roads, SSR Full).

## Cause (verified by reproduction and bisection)
**Enhanced Car Paint's metallic flake (`src/render/car_paint.wgsl`), made much worse by Sharpen 100.**
- Each 4 mm cell (≥ ~1.5 px) took one random tilted normal and glinted towards the dominant light with a
  power-20 lobe, switched on/off per cell (`rnd.x > 0.6`), at up to 0.5·light·tint. On a smooth lit panel
  that is a sparse set of isolated bright cells — droplets. A ±3 % per-cell grain added a fine speckle.
- Sharpen (post) amplifies both into dense white dots.
- Bisection (`--shot intro`, 1920x1080, Rachel's 350Z trunk, speckle metric = mean |gray − median3x3|,
  `bisect-speckle.txt`): Original 2.59; all max 7.65; all max minus Sharpen 2.64; minus Enhanced Car
  Paint 5.25 (the no-paint level of this sharpened image; no dots visible); every other row off leaves
  7.6–8.5 (SSR, Wet Roads, SSAO, reflections, shadows, AA, scale: not involved). Each row alone on
  (Clean/Modern/Ultra presets and single rows, intro and garage): only Enhanced Car Paint shows dots.
- Original (all off) shows none and is unchanged by this fix (intro: 0 differing pixels vs before).

## Native (from existing traces; not re-run under Wine for this work)
car.efx has no wetness or rain input: its constants are HarmonicCoeff, matrices, LocalEyePos,
Diffuse/Specular/Envmap Min/Range, SpecularPower, EnvmapPower (CTAB, formats.md "car.efx registers",
verified against apitrace in the car-paint comparison). The original's rain drops are a screen/lens pass only (rain_drop.efx,
0x5C7AC0, `src/render/rain.rs`), and only while it rains. So a dry or wet car is shaded the same; nothing
drop-like belongs on the car. Wet Roads' track-load wetness (docs/wet-roads-ssr.md) does not reach the
car shader (the bisection agrees).

## Fix (enhancement path only)
The flake is now filtered as real sub-pixel flakes are seen: each cell gets the mean lobe of its flakes,
`FLAKE_MEAN·(r·l)^FLAKE_POWER` = 0.072·(r·l)³ (Monte Carlo of the old model: 40 % of cells, tilt
±0.35/axis, power 20; matches within 0.02 for 0–40°), varied per cell by ±40 % (`FLAKE_SPREAD`), and
the grain is ±1 % (`FLAKE_GRAIN`). Same mean brightness and colour as before, no isolated islands, and
Sharpen 100 no longer creates dots (all max after the fix: 5.54, visually as with the paint off).
Clearcoat, crisp env, pearl shift, octave/fade logic unchanged.

Test: `render::car::paint_tests::flake_is_filtered_not_droplets` (no binary per-cell glint, no
tilted-normal glint, spread ≤ ±50 %, grain ≤ 0.02, filtered lobe = old mean).

## Evidence (the maintainer's private captures)
- `intro-paint-only-orig-before-after.jpg`: Original | Enhanced Car Paint alone, before | after.
- `intro-max-before-after-paintoff.jpg`: user's all-max set, development build 1441 | fixed | max with paint off.
- `garage-metpaint-max-before-after.jpg`: `--shot main`, `NFSU2_SHOT_FIT=0x3F:104` (METPAINTSILVER), all
  max, before | after.
- `bisect-speckle.txt`, `max-options.txt`.

## Open
- Sharpen 100 still sharpens other fine detail (vinyl edges, texture noise) as designed; only the paint
  dots are fixed here.
- Not checked on the player's display size (shots at 1920x1080).
