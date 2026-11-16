# Enhancements vs emissive light and the car's shadow (2026-11-04)

Clone deviations only (nothing traced; no native counterpart): the opt-in rows that darken
(SSAO, Contact Shadows, Lamp Shadows, Volumetric Fog's extinction, TAA's history clamp) checked
against the additive layers (underglow, glow sprites, streaks, flames, headlight pools), and the
car's shadow stack at a player's real settings (`<the player's options file>`;
report on development build 20261104-1930: "shadows and neon on highest settings look a bit off").
All rows off: md5-identical to the build before (explore, race, qrstart, neon_car, garage_neon,
intro, drift, drag, qrcars 1280×960); Ultra alone, Lamp Shadows, Volumetric Fog: AE 0.

## Rule
Occlusion darkens what surfaces reflect, never light emitted over them; and two layers that stand
for the same occlusion (the native shadow texture, Contact Shadows, SSAO under the car) do not
multiply.

## Changes
* **SSAO** (`ssao.rs` `run_into`, `PostFx::ssao_into`, game.rs frame + shot): multiplied into the
  scene (the multisampled one with MSAA; every sample takes its pixel's factor, = the old
  multiply after the resolve) right after the world and the cars, before the shadow texture and
  underglow, particles, glow sprites (streaks off), horizon fog. With SSAO on, the race start's
  characters are drawn before the shadow pass (so they occlude and receive it); order unchanged
  with it off. The old post-resolve path (`pre_passes`) is kept but no longer gets a frame in game.
  Inside a car's footprint the occlusion gives way to its underglow
  (`glow_boxes`, `glow_share` in `fs_apply`). the second graphics-audit round (audit fix 9): `glow_boxes` takes every car (≤ 8, the
  nearest first; centre.w the underglow, 0 none) and the car's **body** (inside its box, soft 8 cm rim, from
  10 cm over its floor) takes no occlusion — screen-space self-occlusion only dimmed paint, glass and lamps
  (street car box −2.2 % → −0.3 %, neon −1.7 % → −0.6 %); under a lit underglow the footprint takes none
  (was: by the glow's light). The ground round the car keeps its contact darkening. Neon / lamp boxes
  (street lamp, arcade lanterns, shop-front neon) were and stay within 0.3 %: the glow sprites and the
  additive layers come after the multiply.
  **Speed (the same pixels)**: each box carries a screen rectangle (`ssao::glow_rect`,
  `Uniform.glow_rects`, and their union): the screen's bounds of the box-aligned region outside which
  both of the box's terms are exactly 0 (x within ±max(half length + 8 cm, 0.85 × half length + 0.35 m
  + 0.15 × half width), y likewise with 0.8, z from the footprint's floor 0.9 m under the box, or the
  body's 4 cm over its floor without an underglow, to 8 cm over its top; 5 cm more each way). There
  every smoothstep of `glow_share` is at its end, so the box multiplies the share by exactly 1; a pixel's
  reconstructed position lies on its ray, so a position in the region is inside the projection of the
  region's corners (one pixel of margin). A region reaching the eye's plane gets the whole screen. The
  pass tests the union first (outside it: no loop, the share 1 as before) and each box's rectangle before
  its loads. A first form bounded the loop's 4 m reach instead: the player's own car, 5–7 m ahead of the
  chase camera, then covered the whole screen, and the tests cost more than they saved (apply +0.4 ms
  racing on the integrated GPU). Proof: `ssao::tests::a_box_changes_no_pixel_outside_its_rectangle`
  (the shader's f32 arithmetic at depths every 2 cm round each box: no factor other than 1 outside its
  rectangle; cars ahead with and without an underglow, beside, turned, far off, near the eye),
  `the_rectangles_follow_the_shaders_terms` (the constants are ssao.wgsl's),
  `tests::first_use::the_occlusion_glow_rectangles_draw_the_same` (a city circuit's start and first frames
  with and without the rectangles, High, Ultra, Extreme, Ultra at 25 %), the bench dumps at Medium, High,
  Medium with 8x MSAA (the multisampled apply) and the player's settings, byte-identical; speed in
  docs/perf-integrated-gpu.md "The occlusion apply's car loop".
* **Contact Shadows** (world.wgsl `contact_visibility`, lamps.rs `contact_share`): (1) a car's
  contact shadow keeps 1 − glow of its depth, glow = the underglow's brightest channel × drawn alpha
  (`shadow::neon_light`, occluder centre.w); (2) full inside 75 % of the car's box, 0 at the box
  edge (body + 0.1 m), was full to 85 % and 0.35 m + 0.15·half-width past it; (3) the native
  shadow texture over it is drawn through `fs_car_shadow` (`Effect::CarShadow`, lamps module only,
  alpha-blended shadow textures): its alpha cut so the ground ends at min(1 − a, vc), the darker
  of the two, not their product (inferred exact for a black shadow texel; SHADOWIG is dark grey).
* `--bench` takes NFSU2_NEON=<part> (the neon-driving pass's lines); `--shot` takes NFSU2_SHOT_NO_CAR_SHADOW=1.

## Glow area (MAGENTA, Peugeot, 1280×960; `glow-numbers.txt`, scripts/shots.sh + glow.py)
Mean R over the Ultra glow mask / glow pixel count, vs Car Shadow/Neon Ultra alone:

| row (on Ultra) | chase before | chase after | orbit before | orbit after |
|---|---|---|---|---|
| Contact Shadows | R −18.5 %, area −25.1 % | −0.6 %, −1.3 % | −17.3 %, −19.6 % | −0.5 %, −0.7 % |
| SSAO 100 | −13.1 %, −21.0 % | −0.6 %, −1.6 % | −14.1 %, −14.1 % | −0.7 %, −0.9 % |
| Lamps + Lamp Shadows High | +0.5 %, +1.1 % | same | +0.2 %, +0.4 % | same |
| Volumetric Fog High | 0 | same | +1.1 % | same |
| TAA (bench drive, frame 450) | — | −0.2 %, −0.7 % | — | — |
| all of these | −29.9 %, −37.7 % | −0.8 %, −1.7 % | −28.8 %, −31.3 % | +0.2 %, −0.2 % |

Driving (`--bench drive`, LANCEREVO8, frame 450, after): SSAO −1.7 %, Contact −1.1 %, TAA −0.2 %,
TAA + SSAO −1.9 %; Lamp Shadows +1.0 %; no darkening row left. TAA: the glow is low frequency, its
clip does not darken it. Volumetric Fog is additive (its extinction applies to its own scatter).

## The player's settings (scripts/stack.sh + an internal box script; `boxes-before.txt` / `boxes-after.txt`)
Road luma (0–255) 0–0.5 m behind the bumper, chase cam, standing; 'far' ≈ 1 m behind = 59.7 / 57:

| | Original | user before | user after | user, Contact off |
|---|---|---|---|---|
| no neon | 40.7 | **20.9** (−49 %) | 28.7 (−29 %) | 35.8 |
| MAGENTA underglow | 76.2 | **45.6** (−40 %) | 73.2 (−4 %) | 74.4 |

Per layer (no neon, on Ultra): native shadow alone 40.7; + Contact 23.8 → 32.7; + SSAO 25 39.2;
+ Lamp Shadows High 40.8 (no lamp near; Dynamic Lamps off for the player); stack 23.1 → 31.6. The
rest of the player's 'user − contact' drop (40.7 → 35.8) is the grade of the other rows (High
Precision Post 2, Wet Roads, SSR: 'far' 59.7 → 57.1). Car Shadow/Neon Ultra's grid refine:
≤ 0.4 levels anywhere in these crops (shape, offset unchanged). Garage (`garage_neon`): user =
Original within 0.05 levels, unchanged by this work (no SSAO/contact in the front end).
Crops: `user_chase_{explore,neon_car}_orig-before-after.webp`, `user_high_*`; driving with the
user's values (frames 440–450): glow-box luma 97.9 Original / 99.2 user, frame-to-frame |ΔY|
6.0 / 7.4 (SSR + wet road; Contact on/off 7.4 / 7.5: no shimmer from the shadows).

## Open
* **SSR** reflects the underglow's tubes / glow as vertical magenta streaks under the car (crops,
  `user_chase_neon_car_*`, right): the main "off" left at the player's settings; SSR owner.
* Contact Shadows still darken the road under the car's body (−20 % in the box behind the
  bumper, which reaches under the overhang); judged against Original only, no native frame.
* SSAO's residual on the road beside the glow (~1 %) and in the world pass's own additive
  textures (worldnofog signs, drawn in the world pass before the occlusion): left, small.
* The min() combine assumes the shadow texel is black (src ≈ 0); inferred, not measured per car.
