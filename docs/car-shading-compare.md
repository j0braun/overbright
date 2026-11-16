# Car paint, reflections and rims: clone vs original (native path, 2026-11-04)

The 2026-10-29 note "matte vs saturated" (wine/compare/DIFFERENCES.md) was re-checked
with the current clone (after the physics merge, Enhanced Reflections off = default) against the original
on DXVK/RADV (`fastgpu.sh`, vanilla, WidescreenFix off, 1280x960, max detail).

## Matched frames
The original's Quick Race car select shows each car standing still, so its frames match the clone's
pixel for pixel in pose and camera. New shot aid: `NFSU2_SHOT_QRCAR=<car type> game --shot qrcars`
puts that car on the platform as the list opens (no swing), as the original after Right×n
(240SX = Right×3, stock 350Z = Right×7, Peugeot 206 = none). `main_dday` matches the original's
main menu with a fresh profile (DDAY_PLAYER_CAR, Rachel's 350Z, same turntable pose); `intro`
matches the free-roam start after Rachel's SMS (`fastnav.py freeroam`).

Region means (0-255, R G B; orig → clone delta), after the fix below. Crops in
`docs/evidence/car-shading/` (orig | clone | |diff|×4):

| Frame | car crop Δ | body Δ | rim Δ | glass / hood Δ |
|---|---|---|---|---|
| 240SX stock, QR car select (`qr-240sx.jpg`) | +1.0 +0.8 −0.3 | +0.1 +0.1 −0.9 | −0.2 −0.2 −0.7 | glass +0.9 +0.5 +0.5, hood −1.0 −0.8 −1.8 |
| 350Z stock, QR car select (`qr-350z-stock.jpg`) | +0.4 +0.3 −0.5 | −0.3 −0.1 −1.1 | +0.1 +0.2 −0.4 | glass +5.3 +4.6 +5.9, hood +5.3 +2.9 +1.6 |
| Peugeot 206, QR car select (`qr-206.jpg`) | +1.3 +1.1 +0.3 | 0.0 +0.1 −0.9 | +0.4 +0.7 +0.3 | hood −0.6 −0.4 −1.2 |
| Rachel's 350Z, main menu (`main-dday-350z.jpg`) | +0.8 +0.9 +0.5 | +0.1 0.0 −0.2 | +0.1 −0.2 +0.6 | hood +1.6 +5.3 +2.5 |
| Rachel's 350Z, free roam (`intro-350z.jpg`) | +0.8 +0.3 −0.4 | upper body +0.7..+1.5 / −0.7..−1.8 | – | trunk +0.5 +1.9 −0.8 |

Full numbers and rectangles: `region-means.txt`. The +5 on the stock 350Z's windshield and hood
are its near-white highlights (orig 235/215/238): same shape, slightly brighter in the clone;
**inferred** to be the overbright/glow passes on saturated pixels, not car.efx (the constants are
equal, below).

The car crops agree to about one level in every channel. The remaining pixels that differ are
edges (sub-pixel rasterisation/filtering), the small blinking warning lamp behind the car, and the
mouse cursor in the original. The dark body sides are ~1 level lower in blue in all three cars
(not traced; within the post chain's rounding). The 10/29 "matte grey" was the clone before the
post-chain rounds (see formats.md "Re-compared 2026-10-30").

## Constants compared (apitrace)
An apitrace of the original (QR car select, stock 240SX; frame 10600 of the trace, both the floor
reflection pass and the main pass, 197 car.efx draws, VS8 = car.efx vs_1_1) was dumped with
<work dir> --consts`. Every draw's material constants (VS c18 DiffuseMin, c19
DiffuseRange, c20/21 Specular Min/Range, c22 SpecularPower, c23 EnvmapPower, PS c0/c1 Envmap
Min/Range) equal one of the clone's light-material constant sets (0x5BD4B0 on GLOBALB 0x135200) to
5e-6: paint METPAINTSILVER, rims MAGSILVER, glass WINDSHIELD / WINDOWMASK, CHROME, MOLDINGS, RUBBER,
BRAKEDISC(_DIFFUSE), BOTTOM, DECAL, HEADLIGHTGLASS, BRAKELIGHT(GLASS), CLEARPLASTIC, ALUMINUM,
DEFAULT, LICENSEPLATE, DULLPLASTIC (PLAINNOTHING has the same constants as DRIVER). The 21 distinct
sets are internal evidence: car-shading/fe-240sx-car-efx.txt (count, 22 values, matched material).
HarmonicCoeff / LocalEyePos / the env cube / the post chain were checked against traces on
2026-10-29..11-01 (formats.md, "Car shading checked against the running original") and are not
re-traced here; the matched-frame means above show no change since.

## Fix: car.efx flags are per material record (verified)
The trace had four draws (DULLPLASTIC, 54 and 31 primitives in the main pass) with EnvmapMin/Range
= 0 although their meshes (240SX_BASE_A, 240SX_KIT00_TRUNK_A) have header flags 0xC180. The flags
0x5BD4B0 tests come from the draw, not the mesh header:
- 0x5C5BB1 (in the mesh draw loop, esi = the 0x3C-byte material record of chunk 0x134B02: +0x1C
  texture slot, +0x20 light material) pushes `[esi+0x38]` as the third argument of 0x5CEB40;
- 0x5CEB40 stores it at the queue entry's +0x14 (0x5CEBEF), next to +0x20 the light material;
- the flush 0x5CF430 calls 0x5BD4B0(entry+0x20, entry+0x14) (0x5CF559…0x5CF55E);
- 0x5BD4B0: `(flags & 0x100) == 0 || g_CarEnvironmentMapEnable 0x870CB8 == 0` → no env term;
  else `(char)flags >= 0` (0x80 clear) → no diffuse/specular.
Those two ranges carry 0x4080 in their records. The clone used `Solid::mesh_flags`; it now reads the
record's +0x38 into `solid::Material::flags` and `render/car.rs` passes it to
`LightMaterial::constants`. With it, the clone's front-end LOD-A draws of the 240SX give exactly the
trace's 21 constant sets and no others (test below; the engine's MAGGUNMETAL is excluded because the
original does not draw the engine under the shut hood). Visible effect: no env reflection on those
dull-plastic ranges (underbody, trunk inner), a few pixels in these views.

## Test
`car::shading_trace_tests::stock_240sx_constants_match_the_original_frame` (skips without
NFSU2_ROOT): trace sets ⊆ clone sets and clone sets ⊆ trace sets (per material, engine excepted),
DULLPLASTIC both with and without the env term.

## Open / observed elsewhere
- **Front-end floor** (not car shading): the turntable floor left of the car is 10-12 levels
  brighter in the clone (region 300x40+100+700: 59/58/48 vs 71/70/56 on main_dday; same on every QR
  car), with yellowish patches under the car's front. Not traced in this work. **Leftover**: the floor's tiles stood at another angle (the platform's turn; internal notes); fixed.
- **Engine under the shut hood**: the original does not draw 240SX_KIT00_ENGINE_A on the car select
  (no draw with its 305/317-primitive ranges in the trace); whether the clone draws it is not
  checked (hidden either way).
- **Quick Race grid**: the player's grid slot and the rivals are random draws in both games
  (docs/race-ai-parity.md, row 5), so the in-race frames are not matched; the in-game comparison uses the
  free-roam `intro`.
- The in-game env cube contents differ slightly (C-pillar reflection in `intro`), as the dynamic
  cube's face order/timing is not frame-matched; car-crop means still within 1 level.

## Reproduce
Original: `fastgpu.sh <copy> <disp> --fresh` (+ `--apitrace T`), `fastnav.py <copy> main`, then
Return×4 (Circuit, track, options → car select), Right×n, mouse moved off with
`DISPLAY=:N xdotool mousemove 1275 955`, `peek.py shot`. Regions: `regions.py ORIG CLONE
name=WxH+X+Y…` (ImageMagick means). Clone: `NFSU2_SHOT_QRCAR=240SX game --shot qrcars out.png`.
