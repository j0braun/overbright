# Neon underglow quality (Car Shadow / Neon Ultra), 2026-11-04

Player report: the neon underglow looks low quality on Ultra (Ultra preset, every graphics row at max).

## What the neon is (traced, see formats.md "Car neon")
* Ground glow: the shadow grid drawn a second time (0x622D3F..0x622F93; front end 0x616D40 → 0x617417)
  with the car's `%s_NEON` (render info +0x344), SRCALPHA / ONE, colour +0x354. world.efx: `2·tex·colour`.
* Tubes: NEONBLUR (GLOBALB 64×64 DXT3) camera-facing quads, 0x61ACA0 → 0x615A20 → 0x60D800.
* Gate: g_CarShadowEnable 0x870CC0 (the Graphics row "Car Shadow / Neon"; clone Ultra = `enh.shadows 2`).

## What was low quality (measured)
* Every `%s_NEON` is a 128×64 **DXT1** grey mask (COROLLA_NEON, PEUGOT_NEON: `bin_textures`/`tex_get`).
  Decoded, a column runs `#121512 #181C18 … #201C20 #252225`: 5-bit red/blue steps of 8/255, 6-bit
  green of 4/255 (green/magenta tint where they round apart), flat 4×4 block plateaus. world.efx doubles
  them: 16/255 output steps.
* On the ground a texel is ~3.5 cm, 15–25 px at 4K close up, so the 4×4 blocks show as **stair-stepped,
  blocky glow edges and contour bands** (internal evidence: blue channel, contrast
  stretched, Peugeot MAGENTA, `--shot neon_car`, orbit camera, 4K max). Native has the same texture; at
  1280×960 a texel is ~5 px, so it is far less visible (native-garage-corolla-1280x960.png).
* Not the cause (each row isolated at 1280×960 / 1080p, AE vs Original): Ultra's shadow-grid refine
  (`Shadow::refine`) changes < 0.01 % of pixels; anisotropy, LOD rows, SSAO, reversed-Z ≈ 0.
* Also seen, **not fixed here (not the neon path)**: the post chain (overbright glow, 0x5CCA10) runs at
  the scene's internal size, its taps in half-size texels, so with SSAA / render scale > 100 % the bloom
  around the neon is 2–4× narrower on screen and its edge harder (garage profile across the glow's lower
  edge, 1280×960: Original 223→88→56→30, max 242→106→47→17; native 205→82→62→38).
  CAS sharpen 100 adds a dark ring. Open for the post/SSAA owner: scale those taps by internal/output.

## Fix (clone deviation, opt-in: Car Shadow / Neon **Ultra** only)
`nfsu2::neon_quality::enhance`, used by `WorldRenderer::add_shadow_texture` for a `*_NEON` texture when
`qol::shadow_quality() >= 2` and no texture pack replaces it: grey texels (channel spread ≤ 12) to luma
(no 565 tint), Gaussian σ 0.85 texel across the block seams, 4× Mitchell–Netravali upsample (512×256),
full box mip chain, **Rgba16Float** texels, border faded to 0 (see "Cutoff line" below). Grid, UVs, colour, blend, pulse and glow size stay native; a
tier change re-uploads the texture. Original and Low/High are byte-identical (Original 4K neon_car and
1080p garage_neon shots: AE 0 against the build before the change).

Result: after-4k-max-blue.png vs before: blocks and contours gone, same footprint; mean blue in the crop
51.4 → 51.0 (−0.7 %). Unit test `neon_quality::tests::quantised_ramp_comes_back_smooth`: a 565-quantised
block ramp (native steps 8 levels) comes back grey with every neighbour step < 1.5 levels, mean within 2 %.

## Cutoff line while driving (design note: "underglow has a cutoff line")
**Cause (measured).** The glow ends in a straight line at the shadow grid's edge (the car's box × 1.05,
UV 0..1, 0x622D3F..0x622F93; formats.md "Car shadow"). Every `%s_NEON` mask (LANCEREVO8, COROLLA,
PEUGOT, 350Z: the same 128×64 image) is still lit on its border: the rear / front columns hold 8–18 / 255
over their middle rows (row 32 from u = 0: 8, 13, 13, 16, 24 …; column 125: up to 18), the side rows up
to 8. world.efx doubles that and the neon colour scales it, so at full alpha 15–36 / 255 are added up
to the grid edge and nothing beyond it. Profile across the rear edge, user's Lancer, MAGENTA, chase cam
(`--shot neon_car`, NFSU2_SHOT_SAVE = the player's a community save): Original 1280×960 R−G 40 → 27 → 9 within 3 px;
2560×1440 max settings 30 → 24 → 9 within 4 px. The sampler (Repeat, as D3D's default WRAP) blends the
border with the opposite border, which is lit too.
**Native.** The same texture, grid and UVs are drawn by the original, which shows the same straight edges
(internal evidence: The neon pass's Wine bumper-cam frame, the finished-career save's
Corolla WHITE PULSE, the glow's front and side edges are straight cuts). Original therefore stays native
(AE 0 against the build before this change, `neon_car` 1280×960).
**Fix (Ultra, clone deviation).** `neon_quality::enhance` now multiplies the rebuilt mask by a smoothstep
fade to 0 over the outer `EDGE_TEXELS` = 6 source texels (≈ 20 cm on the ground; capped at 1/8 of an
axis), the outermost texel exactly 0, and fades again every mip level the fade still spans a texel of
(box mips would pull the glow back onto the border). Peaks (rear / front bands at 16 and 111 texels,
side bars at 10 / 53) are untouched. After: the rear-edge profile at 2560×1440 max runs 24 → 16 → 12 →
10 → 9 over 16 px (chase-rear-edge-max-before-after-2560.png, top before, bottom after;
orbit-side-original-vs-ultra-2560.png). The Graphics row has only Original / Ultra (SHADOW_STEPS 0, 2).
Tests: `neon_quality::tests::border_fades_to_zero`, `tests/neon_edge.rs` (real masks: native border ≥ 8,
rebuilt border 0, peak within 3 %).

**Rows checked while driving** (`--bench drive` with NFSU2_NEON=MAGENTA, NFSU2_BENCH_CAR=LANCEREVO8,
frame 450 of a fixed run; drive-frame450-rows-a/-b.png, each row alone on top of Ultra; the means
are from the standing `neon_car` shot at 1280×960, glow region 120×45 px behind the car):
* Render scale 200, Wet Roads, road reflection, High Precision Post, Enhanced Reflections, Sharpen 100,
  Dynamic Lamps + Lamp Shadows: the glow as Ultra alone (region means within 1 %).
* **SSR (1 and 2)**: reflects the neon tubes / glow as vertical magenta streaks behind the car and the
  glow looks cut off above them (rows-b, `ssr`, `ssr1`, `max`). Not the neon path: open for the SSR owner
  (exclude additive/no-fog draws such as the underglow from the SSR source, or fade streaks).
* **Contact Shadows**: darken the road under the car's footprint (lamp_shade, fogged draws only); the
  additive glow is not multiplied, but on the darker road its outer part reads dimmer and more compact
  (glow-region mean R −17 %, rows-b `contact`). Open for the lamps owner (skip the footprint where an
  underglow lights it).
* **SSAO 100**: the post multiplies the finished scene, the glow included (glow-region mean R −9 %).
  Open (apply AO before additive passes).
* Driving strip (drive-strip-original-top-max-bottom.png, frames 150..900 every 150): no flicker or crawl
  of the grid seen at 30-frame spacing; frame 900 shows no glow in either (car pitched / strips
  skipped, native rule, not investigated further).

## Shots
`NFSU2_SHOT_ORBIT=deg,dist,height` (new, shots only): the camera at `deg` from the car's nose, `dist` m out,
`height` m up, looking at the car. Pin: `NFSU2_SHOT_ORBIT=60,5,1.8 NFSU2_SHOT_ENH="enh.shadows 2"
game --shot neon_car out.png` (Peugeot, MAGENTA). `garage_neon` (the finished-career save's Corolla, WHITE PULSE) matches the
original's Garage Car Select (Wine, the finished-career save, slot 1).

## Open
* NEONBLUR tubes keep the native DXT3 texture (drawn through the world's glow textures, not this path).
* The SSAA-scaled post bloom above. No native in-game side view of the underglow was captured (the
  original's camera presets only; bumper cam shows the glow around the car, frames in the work dir).
* No new Wine capture of the player's Lancer driving (the player's a community save has no neon part); the
  native edge comparison uses the neon pass's bumper-cam frame. Temporal checks at single-frame spacing and the
  glow over kerbs/slopes against native are still open. `--bench` takes NFSU2_NEON=<part> now.
* Contact Shadows and SSAO no longer darken the underglow (docs/glow-darkening.md).
