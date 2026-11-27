# World data and rendering

Part of [docs/formats.md](../formats.md), the traced behaviour of `SPEED2.EXE`; the tags
(**verified**, **inferred**, **open**) are explained there.

## Chunk container (verified)

`u32 id, u32 size, body`. If bit 31 of the id is set, the chunk contains other
chunks. Id 0 marks padding. Payload structs are aligned in *file* offsets. The
gap is filled with zero bytes or `0x11` bytes:

| struct | alignment |
|---|---|
| solid header 0x00134011, mesh header 0x00134900, scenery instances 0x00034103 | 16 |
| vertex buffer 0x00134B01 | 16, then whole 16-byte runs of `0x11` (the result is 128-aligned) |
| TPK pixel data 0x33320002 | 128 |

The first version of the parser stripped every leading `0x11` byte. Twelve
meshes broke because their vertex data really does start with `0x11`.

## Strings → hashes (verified)

`h = 0xFFFFFFFF; for c in name: h = h*33 + c`. Checked against the hash stored
in a solid's header: `bin_hash("PAN_STADIUM_Z_E96") == 0x3BE35F00`.

## Solids (verified)

* Header (aligned): hash at +0x10, bbox min/max at +0x20/+0x30, name at +0xA4.
* Mesh header (aligned): material count at +0x10, vertex count at +0x34. A
  vertex count of 0 means a placeholder solid.
* Vertex: `f32 pos[3], D3DCOLOR (B,G,R,A), f32 uv[2]`, 24 bytes.
* Material: 0x3C bytes. Index count at +0x0C, texture slot at +0x1C (into
  0x00134012, which is a list of `u32 hash, u32 0` pairs), first index at +0x34.
* Indices: u16 triangle list, global to the mesh's vertex buffer.

Check: across STREAML4RA, all 10,735 meshes parse, every index is in range,
and the sum of the material ranges equals the index count.

## Scenery (verified)

* Info, 0x44 bytes: `char name[32]`, then 3 LOD solid hashes at +0x20.
* Instance, 0x40 bytes: bbox (6×f32), `u16 info`, `u16 flags`, `u32`,
  `f32 position[3]` at +0x20, `i16 rotation[9]` at +0x2C (scaled by 8192),
  one row per local axis, applied as row vectors (`p * M`).
* Check: across 500 rotated instances, transforming each solid's bbox with
  the row-vector convention reproduces the stored instance bbox (mean error 3.9
  units, against 9.5 for column vectors). Signs render with readable text.
* Flags (u16 +0x1A; read by the cull 0x5D6580 and the draw 0x5D6AE0):
  * 0x0200 appears only on `XO_TRACKBARRIER*`, so it marks race-only
    barriers: 0x5DB4C0 (the test at 0x5DB505) mirrors them for a race run
    backward. Nearly all also have 0x10 and show only while a scenery
    group lists them (see "Barrier and scenery groups" under Collision).
  * 0x0100: no matrix — the draw entry gets a null transform (0x5D6752);
    all 4,393 such instances in STREAML4RA have the identity rotation and
    position (0, 0, 0). Verified.
  * 0x0400 becomes draw flag 0x2000 (0x5D6B9B). The road reflection's
    pass (flag 8) skips such draws; passes with flags 0x02 or 0x10 draw
    only them (0x5CAAE0 never passes those). It also picks the LOD for
    cull flags 0x1800 (0x5D672A). Verified; the clone leaves them out of
    the reflection (2,942 in L4RA).
  * 0x8000: draw flag 0x200 to 0x5C5940, cleared once drawn (0x5D6CD8).
  * The low byte (verified; `WorldView::culls` in render/mod.rs): the cull
    skips an instance when (flags ^ 0x60) & the cull flags' low byte
    (0x5D65D5). 0x5B9C30 gives each view a cull context through 0x4888A0
    (context +0x44): base 0x6000 for views 1 and 2 (0x1000 when 0x8707A4
    is set, a BSS flag nothing writes), 0x1000 for view 3 and the cube
    faces 0x10–0x15, 0x800 for view 4; then | 0x10 always, | 0x02 for views
    1 and 2, | 0x08 when the race runs forward and | 0x04 when reversed
    (0x89E7A4), | 0x40 when the base has 0x1800, | 0x20 for view 3, | 0x01
    when the screen layout 0x8707DC (0x5C2160) is 3 or 4. So in L4RA:
    * 0x0002 (78: XO_TRAFFICLIGHT*, XO_BANDEROLLE*): not in views 1–2;
    * 0x0004 (89, `ZPM_*_FW_ST`) only forward, 0x0008 (77, `ZPM_*_BW_ST`)
      only reversed;
    * 0x0010 (344: door pieces, train graffiti, spotlights): never drawn
      by this cull;
    * 0x0040 (14,875): needed in the cube faces, the road reflection and
      view 3; 0x0020 (17,592): needed in view 3;
    * 0x0001, 0x0080: on no L4RA instance.
    The clone draws views 1, 3 (the rear-view mirror), 4 and the cube; it
    has no split screen. A base with 0x1800 also picks the instance's lower LODs
    (0x5D672A; see Road reflections, LOD).

## Texture packs (verified except blending)

* Entry, 0x7C bytes: name +0x0C, hash +0x24, data offset/size +0x30/+0x38,
  palette offset/size +0x34/+0x3C, width/height +0x44/+0x46, compression +0x4A,
  mip count +0x4E.
* Compression byte 0x22 = DXT1, 0x24 = DXT3, 0x08 = P8. Each matches the
  per-texture D3D format record in 0x33310005 for all 1,836 textures.
* Render state per texture (**verified**, from the bind function 0x5BCC40).
  It packs `[alpha test, ref, blend enable, src, dst]` into the effect's
  `BlendState` int[5] with `SetIntArray`:

  | entry byte | effect |
  |---|---|
  | +0x55 == 1 | alpha test, ref 95 (anything else: no test) |
  | +0x56 = 0 | no blending, z-write on |
  | +0x56 = 1 / other | SRCALPHA / INVSRCALPHA, z-write off |
  | +0x56 = 2 | SRCALPHA / ONE, z-write off |
  | +0x56 = 5 | DESTALPHA / INVDESTALPHA |
  | +0x56 = 6 | DESTALPHA / ONE |
  | +0x52 | scroll type (3 = fixed speed from +0x5E/+0x60 × −1/1024) |

  +0x55 == 2 (roads, lane paint, sidewalks, water) does not blend. On those
  textures the alpha channel is a reflection mask.
* world.efx pass p0 sets CULLMODE = NONE and ZFUNC = LESSEQUAL (read from the
  effect's state table with a small fx_2_0 parser).

* Version-4 headers (0x33310001 +0 = 4; six car vinyl files only): 0x494380 reads
  0x33310004 as the 0x7C-byte entry table and 0x33310005 as the format records and
  creates the textures from the table when the counts match (0x4919E0); the block
  callbacks 0x494C10 / 0x494C80 take the entry from the end of each unpacked block
  (block + size − 0x9C), the version-5 layout (**verified**). No version-4 pack is ever
  loaded (cars-1.md, "Car types: count, ids and references").

## Tracks, streaming and visibility (verified)

* `L4RA.BUN` 0x00034110: 0x50-byte records giving the section name, number,
  stream offset (+0x14) and size (+0x18). There are 393 sections.
* The first record of every region is `--`: number 0, +0x10 = 0 (1 in all
  the others), offset 0, a range over whole chunks of other sections (in
  L4RH and L4RR Z0's scenery section, elsewhere solid lists or nothing
  whole). The game loads sections only by number (0x5DACC0, from 0x5E5120's
  fixed list 2500 / 2400 / 2600, the visible list and up to five extra
  numbers, each taken only when not zero, and from 0x5E5230…0x5E52C8, positive
  numbers only); no visible list holds 0, so `--` is never loaded and the
  SKYDOME is placed once (`world::parse::loads`).
* Letters A–R are city regions, `X` holds shared props (solids only), `Y`
  holds shared texture packs, and `Z0` (2600) holds the global scenery: the
  `SKYDOME`.
* Sections numbered x90–x99 in each region are its low-detail panorama.
* 0x00034152: drivable-section outlines. Each is `link[8], u16 section,
  u16 n, bbox[4]` (at +0xC), a point inside it (+0x1C), then n outline
  points from +0x24 (read there by 0x5D7280 and 0x5DB860). The loader
  (0x5E9C62) lists only letters A–T with numbers 1 up to the detail limit
  as drivable, in file order; the rest (region outlines such as x73, x96,
  x99) go to a second list.
* 0x00034151: first u32 is the detail limit, 20 in L4RA (0x800DFC).
  Sections limit..2·limit of a letter are the same places at other detail
  and share the collision pack of section − limit (0x5D3C20).
* The first record of every region's table is `--`: number 0, +0x10 = 0 (1 in all
  others), offset 0, a range that overlaps other sections' chunks (in L4RH and L4RR it
  holds Z0's scenery section 2600). The original never loads it (**verified**, static):
  sections are marked only by number through 0x5DACC0, fed by 0x5E5120's fixed
  {2500, 2400, 2600} plus the current visible list and extra numbers each added only when
  non-zero, and by lists looked up for positive numbers (0x5E5230…0x5E52C8); no 0x34153
  list in the eight regions contains 0. The clone parses every record's range
  (`src/world/parse.rs`), which places L4RH's and L4RR's SKYDOME twice.
* 0x00034153: 255 visible-section lists, each `link[8], u32 section, u32,
  u32 count, u16 sections[72]`.
* The renderer draws only the list for the camera's section. Drawing every
  section at once puts the panorama on top of the detailed city. Following the
  lists matches how the game streams.

## Compressed texture packs and JDLZ (verified)

`TRACKS/LOC4DYNTEX.BIN` replaces the 0x33310004 entry table with 0x33310003,
which has a 0x18-byte record per texture: hash, offset from the start of the
pack, packed size, unpacked size, flags. Each block is compressed with JDLZ.
Unpacked, a block holds the pixel data, then the 0x7C entry, then the 0x20
format record. The entry's data offset refers to the uncompressed pack, so it
is ignored. All 55 JDLZ blocks decode to exactly their declared size. The
other 3 blocks use EA's Huffman codec (`HUFF`). It was ported from the
decoder at 0x573850 (the dispatcher at 0x574680 also knows RAWW and COMP),
and all 3 decode to their declared size.

How the original reads a block (the streamer, 0x48CFE9…0x48D22A; the
texture callback 0x494C10), which replacement packs (HD sky mods) rely on
and `formats::texture` follows: the record's offset is a file offset and
`packed` bytes are read there; with flags bit 0x100 the block is decoded
by its magic (JDLZ version 2, HUFF, RAWW version 1, COMP:
`formats::comp`), without it the bytes are used as stored; the entry is
read at the record's unpacked size minus 0x9C. Nothing bounds the size:
a 4096² A8R8G8B8 sky unpacks to 67,109,020 bytes (the clone's bound is
`Limits::max_texture_block`). Each block is its own texture: one that
cannot be read costs that texture only, with a warning.

## Shaders (verified — transcribed from SPEED2.EXE)

The D3D effects are compiled `fx_2_0` blobs stored as RCDATA resources
(`IDI_WORLD_FX`, `IDI_SKYMORPH_FX`, … 23 in all). Their vs_1_1 / ps_1_1
bytecode was disassembled with a small SM1 disassembler:

* world.efx pixel shader: `r0 = tex * v0; r0.rgb *= 2`, alpha `tex.a * v0.a`.
* world.efx vertex shader: `oFog = clamp((FogValue.x - viewZ) * FogValue.y,
  FogValue.z, 1)`, and `oT0 = uv + TextureOffset` (this is how textures
  scroll; scrolling is not implemented yet).
* skymorph.efx: `lerp(tex1, tex0, SkyMorphAlpha) * v0 * 2`, with the same fog. The `mul_x2` writes all four components, so its alpha is doubled too
  (clamped by the target), unlike world.efx's (verified, ps at blob 0x2168).
* The maths runs on gamma-space values with no sRGB states, so the clone uses
  UNORM textures and targets.

## Screen filters: bleach bypass, overbright, light streaks (verified unless marked)

The frame function 0x5CAAE0 ends with full-screen passes (0x5CB778…0x5CBA29),
in the front end (mode 0x8654A4 ≠ 6) and in game alike. The clone runs them in
`render::postfx` (`PostFx`), over the scene drawn into its own back buffer, for
the driving view and the garage under the menus (`game.rs` `redraw` / `shot`).

**Switches.** g_*Enable are registry overrides (strings 0x7A371C…, read at
0x5BEC0F…0x5BED81): MotionBlur 0x870CDC, LightStreaks 0x870CE0, LightGlow
0x870CE4, OverBright 0x870CFC, BleachByPass 0x870D00, Tinting 0x870D04
(HorizonFog 0x870D0C, Rain 0x870D10). Their defaults come from the detail
level 0x87099C (0x5BE880): above 1 glow and tinting; above 2 overbright,
streaks, motion blur, horizon fog, DOF; above 3 bleach bypass (then capped by the caps
maxima 0x8764xx, 0x5BE6FA). `Switches::from_detail`; `--shot` uses detail 5.
Per frame (0x5CB778): overbright runs only with glow and streaks on
(0x8707CE); tint (0x8707D1) only when the main view's tint record has +0x0C
or +0x24 set; DOF 0x8707CC; rain 0x8707D2.

**Order** (0x5CB82C…): 0x5CC8C0 (copy the back buffer: with bleach / DOF /
rain to screen copy 0 (0x86EB0C/0x86EB88) and, for DOF or overbright, a
4-tap box (0x7FF1FC: ¼ at (±0.5, ±0.5) texels) to half-size A; otherwise
StretchRect to A and a 1-tap copy to C), 0x5CC9B0 (A blurred, see below),
rain 0x5C7AC0, tint 0x5C7870, DOF 0x5CCCE0, bleach 0x5C68E0, overbright
0x5CCA10. The last pass that runs writes the back buffer.

**Targets** (0x5BB850): all A8R8G8B8 (format 21): two screen-size copies,
the screen-size streak buffer D (0x86FF70 surface / 0x86F2E4 texture) and
three half-size targets A, B, C (0x86FFA8/AC/B0, textures 0x86FF74/78/7C),
half = screen (0x870980 × 0x870984) >> 1.

**filter.efx** (IDI_FILTER_FX, 4 techniques of 4, 3, 2, 1 taps):
`r0 = Σ tᵢ · FilterWeights[i]`. 0x5C5F80(dst, src, n, taps, flag) sets each
tap's weight (replicated) and offset (texels of the source) from
{weight, du, dv} triples; flag ≠ 0 uses overbright.efx instead. Blur
(0x7FF110 = 2 rounds): across 0x7FF1CC then down 0x7FF19C, each ⅓ at ±1 and
⅙ at ±3 texels. 0x5C6BF0 draws textures through the same shader with a
D3DCOLOR per texture as the weight, and alpha blend, src and dest factors.

**overbright.efx** (ps_1_1 at blob 0xCD8): `x = sat(4·(t0 − c0.x))`,
`out.rgb = (x · dot(x, c1))²`, alpha 0. OVERBRIGHTOFFSET (handle +0x664,
SetFloat, 0x5C642B) = 0x7FF10C / 255 × 0.5; OVERBRIGHTGREYSCALE
(+0x63C, SetVector, 0x5C63FB) = (150, 150, 150, 200) / 255.
**0x7FF10C and 0x7FF114 are not constants (verified, apitrace of the
original):** their .data values 205 / 170 are only start values. Every frame,
after 0x5CAAE0, 0x5D2B6A calls 0x5BC190, which picks `a` = 0.2 (0x7FF7C8)
when the byte 0x870904 is set (nothing writes it), else 0.45 (0x7A20B0) in
the front end, else in game the main view's effect-controller slot 4 value
(+0x1E0, when its enable +0x3C is set; camera-track bloom), else 0
(0x7D7D3C); then 0x5BC110(x = 0.2 + 0.8·a, 0x7FF0FC / 0x7FF100) smooths
`L = 0.8·L + 0.2·x` (0x800B90; L at 0x876848) and writes 0x7FF114 =
trunc(10 + L·(72 − 10)) (0x7FF118 / 0x7FF11C) and 0x7FF10C =
trunc(216 + L·(62 − 216)) (0x7FF120 / 0x7FF124). Settled: in game L = 0.2,
add 22 and offset 185 (OVERBRIGHTOFFSET 0.3627451, BLENDFACTOR 0x16161616 in
the trace of free roam); front end L = 0.56, offset 129 (0.2529412 in the
menu trace; its add stays 80). `render::postfx::OverbrightLevel`; a still
(`--shot`) uses the settled level, the window steps it per frame. Slot 4
is not modelled.

**Overbright** (0x5CCA10, top settings; with the in-game add of 22 the glow
is faint, the old constant 170 made the parking lines and the headlight
pool flare): C = the bleached screen copy at half
size (1 tap, weight 1 — 0x7FF190); A = overbright(C); A blurred 2 rounds;
A −= C · BLENDFACTOR (BLENDOP 3 = REVSUBTRACT, factors BLENDFACTOR / ONE;
0xB0B0B0B0 in game, 0xA0A0A0A0 in the front end, 0x5CCAED); D += A ·
BLENDFACTOR (0x7FF114, 22 settled in game, see above; 80.0 = 0x7875B8 in the front end;
0x5CCB59); the screen copy += D (ONE / ONE) and is copied to the back
buffer.

**Light streaks** (0x5CBB10, in the view loop when glow and streaks are on):
D *= BLENDFACTOR 0xFF785AAA (ZERO / BLENDFACTOR: R 120, G 90, B 170 / 255)
every frame, then the light glow sprites are drawn into D with the main
depth buffer (0x491860, 0x61AEA0, 0x61ACF0, 0x61B200, 0x61ACA0, 0x494270;
see **Light glow sprites** below); without overbright D is added to
the back buffer there (ONE / ONE). So the glow fades per channel, blue
slowest: the steady state of a still image is k / (1 − decay) per channel.
`--shot` settles it over 16 frames.

**Bleach bypass** (0x5C68E0, screenfilter.efx = effect 0x11): amount t and
TINTCOLOR.w from 0x5CB8A9: front end t = 0.8, w = 0.2 (0.4 / 0.14, then
× 0.5 / × 0.7, when the effect's +0x0C is set, i.e. its second technique);
in game, the main view's tint record (0x48B1E0(1) + 0x60): t = +0x188,
w = +0x1CC, TINTCOLOR.rgb = +0x178…+0x180 / 255 when its +0x30 is set, else
t = 0 and w unchanged (static 0.3, 0x7FF188; the front end leaves 0.2).
**Corrected (verified, apitrace):** 0x5CB9F6 passes 0x5C68E0 not t but
lerp(0x7FF180 = 0.2, 0x7FF184 = 1, t), so the front end's amount is 0.84
and a game frame without the record 0.2 (L4RA's airport: f3 0.88 → 0.904).
That amount is FILTERBLEND, the curves' lerp, and the TINTCOLOR.rgb scale
0x7FF16C = lerp(0x8708EC (BSS, 0), 0x7FF128 = 0.25, amount); TINTCOLOR.rgb
is that × 0x7FF15C…64 (the record's f0…f2 / 255, or 0.5 from the front
end). Trace: in free roam TINTCOLOR = (0.1675, 0.0549, 0.1001, 0.091),
front end (0.105, 0.105, 0.105, 0.2).
The record is `*(view + 0x60)`, the view's effect controller: nine slots
of 12 bytes at +0x04 (state, type, **enable** at +0x0C) and nine 0x58-byte
records at +0x70 (callback at +0x84). See **Screen tint** below for how
slot 3 (enable +0x30, record +0x178) is filled.
The gain map (0x5BBE90, 32 × 32, 0x86FFA4) holds at column j, row i
(u = j/31, v = i/31) A = gain(u), R = gain(v), G = gain(u), B = gain(v),
each curve's g = lerp(0.5, 0.2, t) (0x7FF12C…/0x7FF13C…) and
gain(x, g) (0x5BBDC0) = x / (a + 1) below the pivot 0.4 (0x7FF7AC), else
(a − x) / (a − 1), a = (1 − x/0.4)(1/g − 2); values ×255 truncated. The
ramp (0x5BC050, 1 × 256, 0x86F8AC): R = G = A = x, B = x / (1 + (1/0.45 −
2)(1 − x)) (0x7FF18C). Technique 0 (3 passes, the one 0x5D1E30 picks:
effect +0x0C = 0, +0x14 = 3 read from the running game) runs ping-pong
between the screen copies. **Corrected (verified by the shader text the
original creates and binds per pass in an apitrace of free roam and the
menus):** its pixel shaders are the blob's ps_1_4 at 0x3600, 0x2D70 and
0x2484, not the first three ps of the blob (0x1394 and 0x1BF0 are the
ps_1_1 passes of the other technique). p0 (0x3600): lum =
dp4(rgba, (0.3, 0.59, 0.11, 0)); GAINMAP read at (lum, r) and (g, b);
k = sat(1 − gain(lum)); `rgb = rgb·(1 − k) + (gain r, gain g, gain b)·k`,
alpha `gain(r)·(1 − k) + gain(lum)·k`. p1 (0x2D70, SATURATIONSCALE c0 =
0.4 from 0x7A27DC at 0x5C6737, TINTCOLOR c1): `rgb += sat(1 − (max(r, g) −
b)) · 0.4 · TINTCOLOR.rgb`, alpha kept. p2 (0x2484, TINTCOLOR c0): blue =
RAMPMAP(g, b).b, then `rgb = rgb·(1 − w) + alpha·w` (alpha from p0), alpha
`(1 − w)² + w`. Blend off (0x5C6530 → 0x5BCBD0(0, 2, 1)). So the grade
does not read the back buffer's alpha (the **Bleach mask** below is moot
for technique 0); it crushes the darks by luminance and warms grey/blue
pixels (the sky's red-violet, the road's beige). Checked numerically: the
three passes turn the original's pre-filter sky (72, 61, 80) into
(61, 41, 56); the original shows (59, 39, 55).
**Checked against the original's own passes (2026-11-04, docs/playing/hdr.md):**
apitrace snapshots of two free-roam frames (fr.trace frame 13971,
dark.trace frame 15478), back buffer before the copy → render target after
p2: the ported three passes give 99.9987 % / 99.984 % of the 1 228 800
pixels exactly, the rest ±1 (FILTERBLEND fitted: 0.760 / 0.848; **inferred**,
not read from memory).
**Inferred**: the screen filter's and gain/ramp samplers are bilinear,
clamped (filter.efx's ±0.5 texel box needs bilinear).

**Screen tint (verified)** — `src/screen_tint.rs`. Every frame 0x5DBE90
(called at 0x5CBAEC, after the filters: the values reach the next frame)
walks views 1–2; for each active view it applies descriptor 0x876B00
(built by 0x5DBDE0: slot 3, state 2, callback 0x5D7800, amount 0.5) to the
controller via 0x5D7730 → 0x5D75F0. 0x5D75F0 copies the descriptor's 0x58
bytes into the slot's record, sets the enable (+0x30), calls the callback,
and clears the enable when the record's amount (+0x188) is below 0.01
(0x78425C) or NaN (0x5D7701). 0x5D7800 takes the camera position (0x5F0C60:
camera +0x40, +0x44), smooths "Sunrise Screen Tint" (objects 0x88EA08 +
0x30·(view − 1)) and "Night Screen Tint" (0x88F210 + …) each on its own
with 0x5FA670 over 50 units (0x7A47FC), reads fields 0–6 (0x5F0B50, via the
map's offset table) and writes `sunrise·w + night·(1 − w)`, w = 0x60FA90()
(1 in the shipped game, so Sunrise alone): +0x178…+0x180 = f0…f2,
+0x184 = 128, +0x188 = f3, +0x1C4…+0x1CC = f4…f6. A map that is not loaded
(+0x0C = 0) gives zeros. 0x5CB91B then uses FILTERBLEND = f3, TINTCOLOR.w
= f6, TINTCOLOR.rgb = f0…f2 / 255 (0x784548). In L4RA both maps have 7
fields; Sunrise's 16 rows have f3 0.7–1.0 and f6 0.09–0.19 (row 0 is all
zero: slot off, amount 0, TINTCOLOR.w left as it was). **Open**: who reads
f4 / f5 (+0x1C4, +0x1C8); the one-frame lag is not reproduced.

**Bleach mask: back-buffer alpha (verified; technique 1 only, see above).** The back buffer is A8R8G8B8
(present parameters 0x86F870, BackBufferFormat ← 0x7FEE68 = 21 at 0x5B9AF1,
only ever written with 21 at 0x5D2717). p0 bleaches by `1 − gain(alpha)`,
so only pixels with alpha < 1 are graded. world.efx (fx_2_0) technique
`world` pass p0 sets ColorWriteEnable 0xF, ps `tex t0; mul r0, t0, v0;
add r0.rgb, r0, r0` (alpha = tex.a·v0.a), vs `mov oD0, v1` (no colour
scale); pass p1 (ColorWriteEnable 8, ZFUNC EQUAL, ps `dp3 r0, (1,0,0), v0`)
writes depth into alpha and runs only when 0x8707CD (= the DOF flag
0x8707CC, set by 0x5D29E1/0x5D2A25 from the camera's +0xB4/+0xB8) is set,
on the main target and effect types 0, 1, 4, 5 (0x5CF739). car.efx
technique `car` pass p0 has ColorWriteEnable 7: cars never write alpha
(the clone's car pipelines now mask alpha). No effect or code enables
SEPARATEALPHABLENDENABLE, so blended draws blend alpha with the colour
factors (the clone's world pipelines now do too). In L4RA most world
vertices have alpha 255 (≈ 834k) or ≈ 128 (≈ 171k), so the bleach mostly
acts on half-alpha prelit surfaces, which is why it changes the shots
little. The DestAlpha bytes (**verified**, 0x5BCC40 switch
table 0x5BCF90): 5 → DESTALPHA / INVDESTALPHA, 6 → DESTALPHA / ONE, for
colour and alpha alike, world and car pipelines
(`pipelines::native_blend_factors`; the clone had One / Zero and One /
One, and the car's took them as SrcAlpha pairs). **Open**: whether 0x5BA3D0 clears the main target
(the clone clears to the fog colour, alpha 1; 0x5D2860 clears to 0 after
Present). The end-of-frame alpha-only quad (0x5CBA9F..0x5CBAE6:
COLORWRITEENABLE 0xA8 = 8, 0x5C7150 with colour 0, then 0xF again) leaves
alpha 0 for the front end drawn after it, which is what its masks need:
Minimap_Mask and NOS_Mask are black additive (0x56 = 2) textures whose
alpha the destination-alpha textures after them (the TRACKMAP, 0x56 = 5;
Tach_NOS as NOS_GameBreaker, 0x56 = 6) show through. The clone's UI pass
(`render/ui.rs`, 2026-10-29) blends each quad by its entry byte with the
alpha channel on the colour factors, clears the alpha to 0 first when a
destination-alpha texture is drawn, and puts it back to 1 after (the
clone's own choice, to keep the frame opaque). This replaced the
minimap's circle clip and the NOS mask drawn as a black box. Byte 0 (no
blending) still draws alpha-blended (its alpha test is **open**).

**Tint pass (verified; slot 2 drawn since the rain work, see Rain; slot 0 since the visual-rating parity).** 0x5C7870 runs when slot 0 (+0x0C) or
slot 2 (+0x24) is enabled. Slot 0's descriptor 0x8769DC (callback
0x5D7A70) is applied by the NIS event SCREEN_FLASH (0x4392E0 type 10 →
0x5DBD80(1) on views 1–2, docs/original/nis-mover.md "SCREEN_FLASH"), by Take
Picture (0x501250 → 0x5DBD80(1); not ported) and when 0x883E30 is set,
which nothing writes (only 0x5DBEC3 reads and clears it); slot 0's colour
and amount replace slot 2's in the pass; slot 2 is set by the rain (0x61F1DB: colour
from +0x4EDC…+0x4EE4, amount +0x1F0, only while 0x571260 says it rains) and
by the camera-track effects (0x454763 / 0x45477B; START_FX_TINT etc. are
only debug printf strings at 0x7877C6…). So in dry driving the tint pass
never runs; in the rain it draws slot 2 (see **Rain**). Motion blur and the
horizon fog: below. DOF never runs behind the chase camera (see the depth
pass under the road reflection's **Contents**), so 0x5CCCE0 is not ported.

**Horizon fog (verified unless marked)** — `src/horizon_fog.rs`,
`PostFx::horizon_fog`, called in game after the glow sprites and before
`PostFx::apply`.
* When: 0x8707CF = (mode 0x8654A4 = 6) && g_HorizonFogEnable (0x870D0C,
  registry 0x7A36FC; detail above 2 in 0x5BE880, capped by 0x876524 =
  the effect loaded, 0x5BE808); 0x5CB81C calls 0x5C7480(0) first in the
  end-of-frame chain, after the view loop's glow sprites.
* Sources: chunk 0x34250 of the track file (L4RA: 8 records). 0x5E01A0
  takes version 2 (+8 of the 16-aligned body), count +0xC, 0x64-byte
  records from +0x10, and links each into list `type` (+8) of eight at
  0x88F1C8 (type 0 with +0x30 = 0 becomes 5; counts 0x883E04); the horizon
  fog is list 6 (0x88F1F8). Record: colour +0x0C…+0x14 (0–255 floats),
  position +0x20…+0x28, full distance +0x2C, start distance +0x34
  (L4RA: 600/300, 500/250, 750/375). 0x5E0230 unlinks them.
* 0x5DB910(view 1), per frame: camera position (camera +0x40) to
  0x88EAF0; visibility = "Horizon Fog Visibility" (0x88A2C0 +
  0x30·(view − 1), string 0x7A5354) field 0 smoothed by 0x5FA670 over
  20 units (0x41A00000), or 1 when the map is not loaded (+0x0C = 0);
  L4RA's map has 0.25–1. The list is merge-sorted nearest first (0x43E650
  → 0x43BF40, comparator 0x5D74B0); each source gets +0x60 = visibility,
  screen pixels +0x44/+0x48 from 0x5BC4B0 (`(x/w + 1)·W/2`,
  `(1 − y/w)·H/2` with the view's world-to-clip matrix), and weight
  +0x54 = the cosine between the camera direction (+0x50) and the unit
  way to the source, replaced when positive by 0 (nearer than +0x34),
  1 (at or beyond +0x2C) or the linear ramp between.
* 0x5DBB10(view, x, y, &r, &g, &b, &a), per grid vertex: over sources
  with weight > 0, `d = max(dx, dy) + min(dx, dy) >> 2` (integer pixels,
  the source position truncated; the y offset is −H only for view 2 in
  split screen, 0x8707DC = 3), `f = (128 − 0.45·d) / 128` (0x800E50,
  0x784540, 0x787AF0); f < 0 skips; with `A` the summed `f·w` so far,
  colour += `0.5·c·f·w·(1 − A)` unless 1 − A < 0, then `A += f·w`. If
  A > 0 the sums × `1.8·(head's +0x60)·1.8` (0x800E4C), each capped at
  255; results truncated (__ftol). `*a` is scaled by min((r+g+b)/384, 1)
  but 0x5C7480 does not read it.
* 0x5C7480: flushes (0x5CF430), sets the main target 0x86E770 / depth
  0x86E774 and its viewport, identity world / view / projection
  (0x43C780), then for row 0–9 (bottom to top) and column i 0–9 writes a
  0x18-byte vertex: x = −1 + 0.2222·i, y = −1 + 0.2222·row, z = 0.9995 −
  0.0001·row (0x7A3C64, 0x7843EC), D3DCOLOR (alpha, r, g, b) with r, g, b
  from 0x5DBB10 at pixel (i·(W div 9), (9 − row)·(H div 9)) and alpha 0
  for 0.1·row < 0.4, else ftol((0.1·row − 0.4)·425) (0x784358, 0x7A1E54,
  0x7A3C68): 0, 0, 0, 0, 0, 42, 85, 127, 170, 212; UV (i/9, (9 − row)/9).
  The colour table is 0x86ECA0 (16 bytes a vertex). It draws 100 vertices
  / 0xA2 triangles from the index buffer 0x8708FC, which 0x5BB760 fills
  per quad b = 10·row + i with (b, b+10, b+11), (b+11, b+1, b).
* Effect 21 (0x86EB7C; table 0x8003CC "fx\horizon_fog.efx", generic
  class 0x7A4544 whose begin / end (+4 / +8) are empty): no shaders, FVF
  0x142 (XYZ, DIFFUSE), FogEnable 0, ColorWriteEnable 7,
  AlphaTest 0, AlphaBlend 1 with SRCALPHA / ONE, cull none, ZEnable 1,
  ZWriteEnable 0; sampler `samp` = DiffuseMap, wrap, linear / linear /
  no mip. Nothing sets DiffuseMap for this effect (its only users are
  0x5BE365 and 0x5C755A; the DiffuseMap handle +0x13C is set only by
  other draws), so the clone outputs the vertex colour. ZFUNC is
  LESSEQUAL as 0x5C2440 sets it (0x5C246E): only pixels whose depth is at
  least the grid's (sky, far scenery) get the fog.
* **Inferred**: a fixed-function stage with no texture passes the
  diffuse colour and alpha (the D3D9 rule); state op 116 (0x74, idx 0–2,
  values 0–2) is taken to be no-op texture-stage state. **Open**: the
  game's near plane (camera +0xBC), which sets where z = 0.9986–0.9995
  falls (the clone keeps its 0.5).

**Motion blur (verified unless marked)** — `src/motion_blur.rs`,
`PostFx::motion_blur`, run after the world and before the cars.
* When: in the main view's block of 0x5CAAE0 after the world, sky and
  0x566410 and before 0x6316E0 / 0x630DF0 and the car bodies (0x5CB622):
  0x5CB31F calls 0x5CC470 when g_MotionBlurEnable (0x870CDC, detail above
  2, capped by 0x8764F4 from device caps at 0x5BE341) and 0x7FEE38 (1,
  never written) are set.
* Level 0x870900 (persists, starts 0): target 1 when view 1's controller
  slot 1 is on (+0x18), else 0; when they differ by more than 1/255
  (0x7A4078), +0.2 (0x7FF7C0) if below, else −0.1 (0x7FF7C4), once a
  frame. Slot 1 is set by 0x5DBE90 (end of frame, so a frame late)
  through 0x5DBD80(0) → descriptor 0x8768B8 (slots 1, 4, 5; 0x5DBDE0)
  when the player's nitrous stage (0x59FA40 on car +0x40) is 1–3, the
  speed +0x42C > 0 and 0x5ECDC0 gives 0, and cleared by 0x5DBDA0(0) when
  the stage is 0 or 0x5ECDC0 is nonzero. 0x5ECDC0 gives 0 when
  |(+0x30 → +0x20C)| ≥ 0.001 and +0x430·2.237 ≥ 5 mph, else 0x59FA40.
  The +0x20C term is the gas of the car's input block (**verified**
  2026-11-04, REA decompile of 0x5ECDC0; the clone passes the frame's
  pedal gas, `CarState::gas`, see internal notes). **Open**:
  +0x430 (the clone uses its forward speed); slots 4 / 5 (callback
  0x5D7780).
* Speed: camera +0x1E0 = (position − last position) / dt (+0x274;
  0x4474A4). Its length s gives `ramp` = 0 for s ≤ 25, 1 for s ≥ 125
  (0x7FF7B0 / 0x7FF7B4), linear between. `amount = level·1.1 + ramp`
  (0x7A4070), at most 0.65 (0x7FF7B8); below 1/255 (0x784548) nothing
  is drawn.
* Direction: the velocity times the camera matrix (camera +0, w = 0;
  rows right, down, forward as the clone's `CameraAxes`) is normalised
  as a 4-vector (0x43D010) when s > 1e-5 (0x7845A0), else 0: `o`.
  `k = (level·0.5 + 1)·amount·0.00625·1.2` (0x7FF7BC, 0x78A0EC); the
  corner shifts are `(o.x + o.z, o.y + o.z, o.x − o.z, o.y − o.z)·k`.
* Passes: StretchRect back buffer → screen copy 0 (0x86EB0C, no filter);
  0x5C5C10(copy 1 0x86EB10 ← 0x86EB88, 4 taps of ¼): filter.efx's
  4-tap technique on a quad whose tap i has corner UVs (−i·a, −i·b),
  (1 − i·c, −i·b), (1 − i·c, 1 − i·d), (−i·a, 1 − i·d), i.e. tap i at
  `uv − i·(lerp(a, c, u), lerp(b, d, v))` — a radial part (o.z) and a
  sideways part; then copy 0 ← copy 1 (0x86EB8C) with 4× the shifts
  (0x7A4068); then 0x5C6BF0 draws copy 0 over the back buffer with
  BLENDFACTOR ftol(min(amount, 1)·255) replicated (D3DRS 0xC1) and
  BLENDFACTOR / INVBLENDFACTOR (0xE / 0xF), or INVDESTALPHA / DESTALPHA
  (8 / 7) while DOF (0x8707CC) is on. **Inferred**: corner UV (0, 0) is
  the screen's top left (0x5C5C10's vertex layout, through its
  declaration, is not decoded); filter.efx sampling as in the other
  passes (bilinear, clamped).

## Light glow sprites (verified unless marked)

`src/glow.rs`, flare packs in `formats/lights.rs`, drawn by
`WorldRenderer::upload_glows` / `draw_glows`.

* **Where** (0x5CB575): with g_LightGlowEnable (0x870CE4) and
  g_LightStreaksEnable (0x870CE0) on, 0x5CBB10 sets the streak buffer D as
  the target (0x86F2E0 then StretchRect when multisampled, 0x870D08),
  the main depth (0x870854), COLORWRITEENABLE 7, effect 0x86EB5C
  (worldnodepth: its ps is byte for byte world.efx's 60-byte ps, its vs
  writes no fog; vertex declaration 0x7FF7D0 + 13·0x8C = effect 0's:
  FLOAT3 position, D3DCOLOR, FLOAT2 uv), ZENABLE 1, ZWRITEENABLE 0, then
  calls 0x491860, 0x61AEA0, 0x61ACF0, 0x494270, 0x61B200, 0x61ACA0
  (0x5CB606…0x5CB629; corrected in race8, the pools come fourth). With
  the glow on and the streaks off the same calls draw into the back
  buffer (0x5CB5BC). With the glow off only the front end draws 0x61ACA0.
  The clone draws the glows after the particles: into D after each decay
  (`PostFx::apply`'s `glow`), or into the scene.
* **Flare packs** (container 0x80135100, loader 0x48E0EB): 0x135101
  header (16-aligned; version +0x08 must be 3, box +0x30 / +0x40, u16
  count +0x50, section +0x54, list +0x58; the section record
  0x883EC0[section] +0x08 points at it), 0x135102 flares of 0x30 bytes
  (+0x08 hash, +0x10 position, +0x1C 999 = ground not yet cached, +0x20
  direction, +0x2C type, +0x2D flags). Bayview (L4RA): 250 packs, 2121
  flares, all type 9, flags 2 (8 with 4), pointing down.
* **0x491860** (in game, 0x8654A4 = 6; 0x7F3D40 = 1, never written): for
  each section of the eye's visible list (0x5E00E0) with a pack whose box
  passes 0x5BC310, every flare through 0x490920(view, flare, identity
  0x872E90, 1.0, 0, mode 0, 0, 0). The clone skips the box test (each
  flare is tested on its own).
* **0x490920 mode 0** (the glow pass's): P = position under the matrix;
  drop unless P is inside the six view planes (0x5BC460). d² = |eye − P|²:
  drop above 40000 (0x78D79C), fade 1 − (d² − 32000)/8000 above 32000
  (0x78D798; mode 1 3900/4000, mode 2 as mode 0). N = unit(eye − P);
  facing = (direction with w = 0 under the matrix)·N, |·| with flag 1,
  else 0 when negative. Per layer of the type (table 0x7F3CC8, two
  pointers per type, 15 types; layers of 0x2C bytes 0x7F3A30…): I = the
  intensity, or with flag 4 min(1, facing^+0x18 · intensity) (_CIpow
  0x75C9B0); size = +0x00 + (+0x04 − +0x00)·I, skipped if ≤ 0, times
  clamp(depth / view +0x0C, +0x20, +0x24) (depth = P in the camera matrix,
  view +0x0C = 256 / tan(fov/2)); Q = P + N·(+0x1C); RGB = I·(+0x08,
  +0x0C, +0x10), A = min(255, 2·I·(+0x14)·fade), or with a colour argument
  (the cars' headlights, render info +0x380) A<<24 | that colour. The
  camera's right and down columns (0x491140) are turned about its forward
  by (screen x of P / screen width)·240° (0x7F3D50, as the binary angle
  trunc(trunc(deg·65536)/360), 0x401020; matrix 0x5BA8C0 applied as a row
  vector) and scaled by size; corners Q − R − U, Q + R − U, Q + R + U,
  Q − R + U, UVs (0,0), (1,0), (1,1), (0,1) (0x5BB1D0), drawn as a fan by
  0x5C7E90 → 0x5C5590, which swaps colour bytes 0 and 2, so +0x08 is the
  red. Textures (0x7F39CC → 0x82CC10 by 0x4908F0): HEADLIGHTFLAREINNER,
  HEADLIGHTFLAREOUTER, HEADLIGHTGLOW (GLOBALB, `+0x56 = 2`, additive);
  mode 2 (the reflection) always uses HEADLIGHTGLOW (0x3394FE62).
* **Types** (size min/max, RGB, A, exponent, toward eye, scale clamp,
  texture): 0 = headlight: (0, 64, 100 120 170, 80, 1.45, 0.8, 0.01–0.048,
  inner) + (28, 28, 100 120 140, 20, 3, 0.8, 0.01–0.015, outer); 1 =
  brake: (20, 30, 128 25 25, 120, 0.45, 0.2, 0.01–0.03, glow); 7, 8 = 0;
  9 = street lamp: (30, 30, 110 100 75, 110, 1, 4, 0.001–0.1, glow); 2–6,
  10–14 in `glow::TYPES`. 0x61AEA0 rewrites type 0's exponent (1.45 in
  game, 0x7F39F0; 2.8 in the front end; ×0.75 and sizes ×2.7 in view 3)
  and type 1's +0x1C (0.2, 0x7F3A20; 0.34 in views 4, 5) each frame.
* **Car flares** (0x6204E0, from the constructor at 0x6389E6): over the
  LOD A group 0 model of slots 0..0x3E (render info +0x17A8, 0x20 apart),
  each marker (0x48D8A0) named LEFT_/RIGHT_/CENTRE_HEADLIGHT (type 0) or
  LEFT_/RIGHT_/CENTRE_BRAKELIGHT (type 1) becomes a flare: position =
  marker translation − render info +0x30 (the BODY slot 5 LOD A solid's
  header +0x70, 0x488EB0; ≤ 2.4 cm in the shipped cars), direction = the
  marker's z row, flags 4, linked at render info +0x3F0.
* **0x61AEA0** (needs 0x8026C8 = 1, 0x7F3D40, the glow switch): over the
  car render objects (0x890080, at most 99) passing 0x60D580 (+0x550,
  0x60D500 = 0, view bit of +0x5B4), 0x615640(view, position 0x60BCA0,
  matrix 0x60BCD0, 0, 0, 0). 0x615640: drop when 0x48ACB0(P, 3) < 4 or the
  box test fails; base 1 (0 when owner +0x14 → +4 is 3); headlights =
  base + 1 when the lights are on (+0x93D); LEFT / RIGHT_HEADLIGHT at that,
  colour render info +0x380; CENTRE_HEADLIGHT never (not on the list);
  LEFT / RIGHT_BRAKELIGHT 0.85 (16.85 with owner +0x944), CENTRE 0 (8.425
  with +0x944); each clamped to 1; mode 0. The debug words 0x8A1BF0 /
  0x8A1BF4 force the headlights (flag 1) / brake lights (flag 2: 1.85,
  centre 1) on. Class byte (render info +4 → +0x8E4 → +4) 0x0F drops the
  brake glows, 0x0E the headlight glows (0x615917).
  Clone: a traffic car with +0x550 clear (`TrafficCar::active`) shows no
  glow (`traffic_drawn`, as its body).
* **Brake flag** (owner +0x944, **verified**, 0x5F62C0 from the car
  update 0x6049D9): b = the input block's brake pedal (car +0x30 →
  +0x214; 0 without one). b > 0.05 (0x7A5640) stamps +0x948 with the
  clock (0x865188, 4000 ticks/s, 0x784268). For owner kinds (+0x14 → +4)
  2 and 4 the flag is set while the stamp is non-zero and at most 0.5 s
  old (0x7A5644): the racers' brake lights hold half a second; for the
  others (the player 1, traffic 3) it is b > 0.05. The clone: `CarState::
  note_brake` / `brake_lights` (frame_setup's pedal; the TrafficMover
  cars note their driver's pedal in `traffic_think`), rivals and
  encounter racers holding. No reverse-light flare, marker or texture
  name exists (0x615640 knows only the six light hashes).
  Before the green (2026-10-31, hud112): the race start's hold makes the
  human driver brake fully (0x5EB900(1.0), see **Start, hold and
  gearbox**) and the AI's countdown pedals brake 1, so with the car update
  running every car's brake lights, centre glow included, are lit on the
  grid (**observed** in the original's drag start,
  compare/race112/sbs-drag-pregreen.jpg). The clone held the cars still
  through the race start scene (no car update, brake 0); it now runs them
  under the hold as in the countdown (`Game::update`, scene branch;
  compare/hud112/drag-pregreen-brakes.png). That the update also runs
  while the scene's own camera is up (before mark 2) is **inferred**.
  **Open**: the original's tail-light glows look slightly wider than the
  clone's square sprites (look112 taillights-crop); 0x490920's camera
  axes (right / down scaling) were not traced.
* **Open**: 0x48ACB0's size threshold and the box tests (the clone draws
  every car's flares); +0x14 → +4 = 3 (base 0: the traffic's headlights
  at 1, the same after the clamp); the class bytes 0x0E / 0x0F (not
  ported); the centre brake glow's zeroing when render info +4 → +0x5C0
  has a part above level 0 and 0x60C8B0 holds; the speed trails of 0x61AEA0
  (0x802920 = 8 interpolated copies of the brake lights at half intensity
  over the last three positions +0x2150, when +0x42C > 0 and race state 7
  or +0x208C = 1 or 0x59FA40); 0x61ACF0 (list 0x8A30F0); 0x61B200 /
  0x61ACA0 → 0x615A20 → 0x60D800 (the car neon's glow, left to the neon
  work); the other call sites of these routines in 0x5CAAE0
  (0x5CADE0, 0x5CAEE2); the front end's car glows; the sign of the camera's second column (the
  clone takes it as down, as the environment map's camera space).

* **The reflection's flares, mode 2** (2026-10-31, race3; verified in
  the disassembly unless marked; **ported** in race7 for the section
  flares, `glow::reflection_sprites`, `world_reflection_sprites`, drawn
  into the reflection image after the headlight pools): in view 4's block of
  0x5CAAE0, with g_LightGlowEnable (0x870CE4), 0x5CAC9C calls
  0x491860(view 4, 2) — every section flare through 0x490920 with mode 2
  — and 0x5CACB5 0x494270(view 4, 0, 1), whose pools (0x82CC30) go
  through 0x490920 with mode 2 and param 5 = 1 (0x494324), so the street
  lamps (and any pooled flares) are drawn into the 320 × 240 reflection
  image the road samples. 0x490920 in mode 2: (a) the streak length
  0x7F39A0 = 1.2 (0x7F39A4) + (10 (0x7F39A8) − 1.2) · the view's wetness
  (weather +0x509C); (b) the mirror height +0x1C: when it is 999
  (0x4479C000, as loaded) or param 5 is 1, and param 7 is 0, P = the
  flare's +0x10 under the matrix, a ground probe at P (0x486ED0 /
  0x5E72F0 between 0x5E0B50 / 0x5E0C00) gives h: none → 9999
  (0x461C3C00); |h − flare z +0x18| < 0.05 → 9999; else +0x1C = h − (P.z
  − h), P's z mirrored about the ground; (c) skip when +0x1C is 999 or
  9999; the point (P.x, P.y, +0x1C) with a box of half size 0x7F39A0
  (0x43D540 / 0x43D610) must pass 0x5BC310; (d) fade over 32000–40000 as
  mode 0; every layer takes HEADLIGHTGLOW (0x4901D0(0x3394FE62)); the
  quad's two axes (the camera's columns 0 and 1, esp+0x14 / esp+0x54) are
  scaled by the layer size (0x42FAF0) and the second axis again by
  0x7F39A0, or by 0x7F39A0 × 0.5 when the flare stands less than 4 m
  (0x78D7B0) above its mirror height (+0x18 − +0x1C < 4): a vertical
  streak that grows from 1.2× to 10× as the road gets wet; the value 255
  (0x784544) replaces the mode-0 one at esp+0xAC. **Race7** (verified):
  that value is the colour's top byte (the first `_ftol`, shifted left 24
  at 0x49160C…0x49163C), so the alpha is 255 and the fade (local +0xC8)
  is not applied in mode 2 — only the cut past d² 40000; the colour's
  RGB is the layer's × intensity (param 8 = 0). The quad centre: P under
  the matrix with its z replaced by +0x1C (0x487080 at 0x490BE9) — the
  flare mirrored about the ground — then + N × the layer's +0x1C toward
  the eye (0x43D0C0 at 0x49130A), as mode 0; N, d² and the depth scale
  (P under the view matrix / view +0x0C) from that mirrored P. The facing
  term takes the direction with its z negated (0x487120 / 0x48B7F0 /
  0x487320 / 0x487080 at 0x490C53…0x490CAC). The screen-angle turn is mode
  0 only (0x49139E), so the axes are the camera's columns 0 and 1 as they
  are. View 4's weather object is view 1's (0x5C310F, ebx = 1), so the
  wetness is the rain's (`Weather::wet`). 0x491860 passes param 5 = 0 and
  param 7 = 0, so a section flare is probed once (its +0x1C kept; the
  clone's `MirrorHeights`, with `World::ground_for` on a fresh query for
  0x5E72F0) and its pack's box must pass 0x5BC310 (box against the six
  planes, 0x5BC310 decompiled: out when centre distance + |n|·half < 0
  for a plane). The sprite batch 0x86B7A4 holds 1000 quads (**verified**:
  0x5BB230 with 0x3E8 from 0x5D1DDE / 0x5D27F0; 0x5BB440 drops a quad
  once the write pointer reaches the end), so at most 1000 flares are
  drawn (`MAX_REFLECTION_GLOWS`; the clone had 4096). **Inferred**: the sprite batch's render state in view 4
  (taken as the glow pass's: depth-tested, not depth-written,
  HEADLIGHTGLOW's own blend). The pooled flares of 0x494270 (view 4,
  param 5 = 1) are ported in race8 (see **Pooled flares**).

* **Pooled flares** (2026-10-31, race8; **verified** in the disassembly
  unless marked; ported, `glow::Pools`, `pool_sprites`,
  `pool_reflection_sprites`, `fill_flare_pools`, `Solid::light_markers`).
  **Pools** 0x82CC30: two lists of 200 flare record pointers (pool p at
  0x82CC30 + 800·p), counts 0x82DAE8[p], capacities 0x7F3D44 = 200, 50, a
  mask byte per slot at 0x82D398 + 200·p + i. **0x48AB20**(pool, mask)
  returns the next record (0 when full) and stores the mask. **0x48AC00**,
  the world frame's last call before the sky cycle (0x5CBAF1 in 0x5CAAE0),
  zeroes both counts and runs the blink: 0x8358E4 += the frame time
  (0x865150); above 0.5 (0x784260) it is zeroed and 0x82DAF0 inverted
  (0 / −1, BSS). The culls (0x5B9C30) run at the next frame's start
  (0x5CAB03), before any view draws, so every draw sees this frame's
  pools. **Fillers** (the only callers of 0x48AB20): (a) the scenery cull
  **0x5D6580** (0x5D6830…0x5D69AB): for an instance with a matrix (flag
  0x0100 clear, 0x5D674A) and the LOD model it takes, in views 1 and 2
  only (0x48B1E0(1) / (2)), each position marker of the model's solid
  (**0x48D8A0**: model +0x0C → solid +0x80, count byte +0x1B, 0x50-byte
  records) goes into pool (marker +0x04 == 3) with mask (view ≠ view 1) + 1;
  the record gets +0x10 = the instance matrix × (marker +0x40..+0x48 −
  solid +0x70..+0x78), type +0x2C = marker +0x04 + 10, and for kind 3
  flags 4 and direction (P − the matrix's translation) in xy normalised in
  2D (0x43CE50, (1, 0) for zero), z 0; else flags 2. Bayview's marker
  kinds: 0 on the XW_GUARDRAIL_* and XO_CATSEYEORANGE models (type 10,
  amber reflectors), 1 on XO_POSTBOOTA (type 11, red), 3 on
  XO_TRAFFICLIGHTC and XS_MEDIANPOLE* (type 13, amber blinkers); none of
  kind 2 (type 12, blue). (b) **0x433090** (from 0x4335C0): pool 1, mask 3,
  type 14, flags 2, at its point + 0.4 × its vector — **the flag girl's
  flares** (race9, **verified** in the disassembly; ported,
  `SceneCharacters::flare_glows`, `Pools::add_flag_girl_flare`,
  `flag_girl_flares`): 0x433090's ECX is [0x827B28], the props object
  0x4398A0 makes once (0x439780: 0x114 bytes, the emitter group
  "XECH_FLAG_GIRL_FLARE" (0x48B8B0) whose emitters 0x433090 moves to the
  point with velocity 2 × unit(v) and fires (0x491E50(−1.0)), the name
  "Flag Girl Flare Lights", two 0x68-byte slots from +0x3C that 0x433090
  fills per flare: +0x50 0x80A991DF, +0x54 the point, +0x60 3.7, +0x74
  0, +0x78 14000, +0x7C 10 — a light: see **Flag girl flare lights**
  under **Race finishes, round 11**). **Emitters** (race10,
  **verified** in the code; ported, `Effects::flag_girl`,
  `SceneCharacters::flares`): for each flare (0x4335C0 passes its tip p1
  and the vector p1 − p2) 0x433090 walks the group's emitter list (the
  props object's +0, made once by 0x48B8B0 from the GLOBALB descriptions
  of "XECH_FLAG_GIRL_FLARE": XECH_Flag_Girl_Flare_GlowA, _SmokeA,
  _SprksA), sets each emitter's matrix to the identity with p1 as its
  translation (0x43C780, 0x4012B0), +0x80 = 2 × unit(v) (0x43CF10) and
  +0x8C = 0, and fires it with 0x491E50(e, −1.0, 0) — a one-unit burst
  each character update. **Inferred**: the update runs before the
  world's particle update in the frame. `--shot qrstart` with
  NFSU2_INTRO_NIS=12 NFSU2_SCENE_FRAME=150 shows the sparks and glow at
  both hands (`race10-tmp/qs12c.png`); `NFSU2_SCENE_FRAME` runs the scene
  (and the particles) to that scene frame. 0x4335C0 is called by
  the NIS character update **0x436050** (vtable 0x785FB4 slot 6; ctor
  0x431220 sets +0x1C = +0x24 = −1) when +0x1C ≥ 0 and needs 0x827B28
  and +0x24 ≠ −1. The set-up 0x437BE0 sets them for a skinned character
  (skeleton and animation bound) whose skeleton has 0x23 bones: +0x1C =
  the index of the record's model hash (+0x50) in the table **0x7EA110**
  (0x34 bytes an entry: name pointer, then four bind-space points;
  FLAG_GIRL_MODEL_5/7/8/9_FLARES), +0x24 = 0; with 0x18 bones +0x24 = 1
  and with any other count −1. 0x4335C0 takes the entry's points p1, p2
  through bone [0x7EA1E8 + 0x10·(+0x24)] = 18 and p3, p4 through bone
  [0x7EA1EC] = 28 (the skin matrices, render info +0x14 → +0x2C, 0x40 a
  bone) and then the character's world matrix (render info +0x10, which
  0x60BB20 copies from +0x50 with no parent), and calls 0x433090(0, p1,
  p1 − p2) and (1, p3, p3 − p4). Scenes with a flare model:
  IntroNis12 and 20 (FLAG_GIRL_MODEL_9_FLARES; race start scenes) and
  18, 22. **Observed** (race9, Wine, Quick Race Street X, Industrial
  Park, at the green): the starter between the cars holds two orange
  flare glows (`compare/race116/a private Wine copy.jpg`).
  **Clone**: each frame while the start scene's characters play, the
  same pose as drawn (**inferred**: the original fills the pool from the
  character update before the frame's draws).
  **Draws** (0x494270(view, p2, p3): pools from (p3 & 5 ≠ 0) to
  ¬(p3 >> 1) & 1, slots whose mask has bit (p2 ≠ 0) + 1; pool 1's type 13
  only while 0x82DAF0 is set; 0x490920(view, flare, identity, 1.0, p3 & 1,
  mode p3 & 1 ? 2 : 3 + (p2 ≠ 0), 0, 0)): the glow pass (0x5CB61C: view, 0,
  0 → pools 0 and 1, mode 3); the reflection (0x5CACB5: view 4, 0, 1 →
  pool 1, mode 2, param 5 = 1: the mirror height probed each time); 0x5CADE9
  (view, 0, 4: pool 1, mode 3) and 0x5CBC9B. **0x490920 in modes 3 and 4**
  (0x490D96…0x490ED8): mode 0 without the screen-angle turn (0x49139E is
  mode 0's); for a type other than 13 and 14, with car c = 0x5F0E40([0x890080],
  0x5ED990(mode ≠ 3)) (the human racer's car of 0x89E7A0 +0x68[i]), v =
  unit(flare +0x10 − c +0x60) (0x401250, 0x43CF10) and d = v · c +0x90
  (the car's forward row): dropped when d < 0.96 (0x803A70), else the
  intensity is (d − 0.96) / (1 − 0.96) — the reflectors glow only in the
  player's headlight cone. **Clone**: each frame `fill_flare_pools` takes
  view 1's instances with markers (section visible from the eye, not
  culled by flags, size ≥ 17 at the main view's scale, flag 0x0100 clear;
  smackable props are drawn by 0x61D160 and add none), the solid the
  main view draws; the instance box test and its visibility-cell bit
  (0x5D6580's first test) are left to the flares' own tests (**inferred**
  equivalent), and the cull's instance order to the placements' order
  (**inferred**; only a full pool would show it).
  **Observed** under Wine (race8, a private Wine copy, wined3d, Quick Race Free
  Run, Peugeot 206, the car placed by memory writes; side-by-sides in
  compare/race115): at the traffic light at (1169.9, −304.0) facing +y the
  amber XO_TRAFFICLIGHTC glow blinks (on in three of four grabs ~1 s
  apart) and matches the clone's in place and colour
  (`sbs-blinker-trafficlight.jpg`); before the tunnel at (−1701.3, 2130.0)
  facing 86.5° the centre-line cat's eyes and the guardrail reflectors
  ahead glow orange in both (`sbs-catseyes-tunnel*.jpg`); in the rain at
  night (clock 170, the clone with `NFSU2_RAIN_CLOCK=170`) the wet road at
  the traffic light matches, with no visible lamp or blinker streak in
  either (`sbs-wet-night-*.jpg`). The shot's sky / rain fog was warmer than
  the original's greenish night rain: **resolved** in race9 — the clone's
  `NFSU2_RAIN_CLOCK` jumped the clock without the sky cycle, so the shot
  kept the sunrise dome and "Normal Fog"; the original reaches clock 170
  only through the change at 60 s (0x60FAB0). Read under Wine (race9,
  vanilla, Free Run, the car at the light, the tick clock 0x865188 written
  to 55 s, the change let run, then to 160 s): at clock 170 sky state 0,
  index 2 (night), 0x8A1C90 = 1 (fog weight 0: "Night Normal Fog"), rain
  (view 1 +0x64 → +0x1F0) 0.700, 0x89E850 = (0.405, 0.100, 0.519, 63.5,
  0.760) and colour 0x80696E67, the tint +0x4EDC = (103, 110, 105), slot
  2 (view 1 +0x60 → +0x120..+0x130) (103, 110, 105, 0.70). The clone
  with the cycle replayed (`TimeOfDay::after`, in the shot and at the
  load): rain 0.696, rain colour (103, 110, 105); the sky band's mean
  (34.5, 40.0, 31.4) against the original's (33.5, 39.3, 30.2)
  (`compare/race116/sbs-night-rain-clock170.jpg`, `-before.jpg`). The tint
  pass 0x5C7870 packs red = the first channel (+0x120, f2), as ported
  (0x5C7A38…0x5C7A67). No palette change was needed. The first set was taken with ThirteenAG's
  WidescreenFix loaded (game/dinput8.dll, which patches SPEED2.EXE); the
  blinker and cat's-eye views were taken again on the vanilla game
  (`*-vanilla.jpg`: the same framing, the blinker on in two of four grabs,
  the same glows); the wet-night pair is from the patched run only.

## Sky (verified)

`SKYDOME` (Z0, hash 0xBE43EDBB) references `SKY_SUNRISE_A_CAP` in slot 0
(DXT3, blended) and `SKY_SUNRISE_A` in slot 1. Both are HUFF-compressed in
LOC4DYNTEX.BIN, which is why the first version of the parser could not
resolve them. Its mesh flags are 0. `crates/ob-render/src/sky.rs` has the cycle.

* **Placement.** The scenery loader skips the SKYDOME's hash (and
  0x72923C81) when binding scenery models (0x5DFAC6), so the Z0 instance is
  never drawn. 0x60F890 draws the solid itself (called at 0x5CAD50 and
  0x5CB237). In views 1–3 it uses an identity rotation, scale 1, and the
  camera position as translation. Other views get scale 0.004525. Only views
  0x10–0x15 (the environment cube) then draw anything, and only when
  0x802950 is set: solid 0x72923C81, scaled by 0.004525². Bayview has no such
  solid, and the road reflection view 4 gets no sky.
* **Textures.** 0x60F890 remaps `SKY_SUNRISE_A`/`_CAP` (0x488E20, table
  0x8A2CAC) to a pair from the sky table at 0x8A07A8, which 0x60FAB0 fills
  on its first in-world frame: SUNRISE_A, NIGHT_A, SUNSET_A, NIGHT_A, each
  followed by its `_CAP`. The pair is the one at index 0x8A1C94 when 0x5BF1C0
  (`0x870D14 == 1`) holds, and NIGHT_A otherwise. OVERCASTA is never put in
  the table.
* **Effect and fog.** 0x5C5940 switches the dome to skymorph (effect 20)
  only while the cycle is fading (0x60FE00: state == 4) and 0x870D14 is
  set. Otherwise the dome is a world.efx mesh, fogged like anything else.
  Because it is ~9,700 units away it gets the fog floor
  `FogValue.z = min(1 − f0, 0.6)`. In the main target the world effect
  stays; any other target (`*0x870848 != 0`) turns every non-car effect into
  worldnofog.
* **Cycle (0x60FAB0, every frame from 0x5CBAF6).** It runs only in game
  state 6 with 0x8A1C78 (0x60F690 at track load, 0x5FF6AA) and 0x802958 = 1
  set. The clock 0x7FB718 is the EA 4000 Hz tick 0x865188 × 0.00025, reset to
  1.0 s by 0x576D00 on each track load (0x5FF54D). (The clone keeps this
  clock in its loaded world, `weather::set_clock(world, …)`, not a
  process global: two games in one process — the tests side by side —
  set each other's rain and so the cars' grip, which made
  `rivals_keep_the_race_route` fail now and then.) At `ftol(clock) % 300 ==
  60`, with state 0x8A1C8C = 0, the state becomes 2. The rest is gated by
  0x5BF1C0:
  * State 2 adds 2 to the index (mod 8) and goes to state 4. The old pair it
    copies to 0x8A1CA0/4 is never read.
  * State 4 accumulates frame time in 0x8A1C9C and sets
    `0x8A1C88 = 0.5 + 0.5·min(t/20, 1)`. Once t > 20 s it returns to state 0
    with 0x8A1C88 = 0 and toggles 0x8A1C90.
  * State 1 (a 0–0.5 ramp) is never entered.
  * Reset (index 0, everything 0) is 0x60F750, run on unload (0x5816A7).
* **When the clock runs and restarts (2026-11-01; verified, and
  observed under Wine).** 0x576D00 sets the tick
  0x865188 to 4000 and the clock 0x7FB718 to 1.0 s (and the frame times
  0x86514C / 0x865150 / 0x865154 to 0). It is called from the world
  object's ctor 0x5FF4A0 (World_Start 0x60A690 at 0x60A743) and its dtor
  0x5FF700 (World_Stop 0x60A9B0 at 0x60AA2C). The track unload 0x5815D0
  (game state 7) runs World_Stop and Sky_Reset 0x60F750. A race's start
  from free roam (the flow's track switch 0x581830 → 0x5815D0), Return to
  Explore Mode, leaving a shop (the city start 0x5404B0, see Shops) and
  resuming a career all load the world, so each starts the clock at 1 s
  and the sky on the sunrise pair. Under Wine (vanilla, fastgpu):
  * New career: the airport scene runs on the clock. At the SMS's end it
    read 27.8 s, state 0, index 0. At 61.9 s state 4, index 2. At 82.3 s
    state 0, toggle 1 (night). At 361.9 s the sunset fade (index 4), and
    at 673 s the fade to night (index 6). This is the cycle above, step for
    step.
  * The clock stops while the pause menu or the SMS inbox is open
    (394.17 s held over 10 s).
  * Quit to Main Menu, then Resume Career: the clock ran on in the front
    end (35.7 s at game state 5). The first in-world read was 1.93 s.
  * At the same clock the clone's frames match the original's: airport
    sky mean (320, 0, 540×190) and ground within 1–2 levels at clocks 28,
    58, 88, 118, 328, 367, 392 and 679 (`sbs-airport-cycle.jpg`,
    `measurements.txt`). The car lot at clocks 15 and 94 matches as well
    (`sbs-lot-clk*.jpg`).
  * **Clone, fixed:** `Drive::tick_clock` ran only from `Drive::step` /
    `hold`. Free roam with traffic or encounters (`step_explore`'s field
    path) and the race after the green (`race_frame`) never advanced it.
    So the clock, the sky cycle and the rain held at the start scene's
    end (14.5 s), and free roam kept the sunrise sky for good: the reported
    "dusk". Both paths tick it now. A drive-in that reuses the drive
    (same region and car: free roam ↔ race) restarts the clock, the cycle
    as a load does (`Drive::restart_clock`). The proposed weather-object
    reset was not recovered: it was inferred, not confirmed against the
    original. Its reset semantics remain open. Test
    `world_clock_runs_in_free_roam_and_races`.
  * **Open:** the clock during a replay (the clone holds it at the race's
    end). The rain fade-in at clock 147.8: both frames show rain, but the
    clone's `NFSU2_RAIN_CLOCK` shot (5 s of weather stepped) is warmer and
    darker. The sky is equal; the rain amount and tint at onset were not
    compared.
* **Fade.** 0x5CF687 binds the previous pair (index − 2) as SKYMORPHTEXTURE
  (effect handle +0x894). It sets SKYMORPHALPHA (+0x844) to `t/20`
  (0x60FDF0) and SKYMORPHALPHA2 (+0x86C) to `ftol(t/20·255) << 24`. The
  handle names come from the table built at 0x5D1980. The shader lerps from
  the old pair to the new one.
* **Shipped behaviour (corrected 2026-10-29, fe72, verified at run
  time).** 0x870D14 is +0x64 of the options object 0x870CB0, which the
  static constructor 0x77E550 builds with 0x5B7610 (+0x00 = 1, +0x04 =
  0x78, +0x38 / +0x64 / +0x68 = 1, the rest 0); the hardware maximum
  object 0x8764C8 is built the same way (0x77E560) and 0x5BE3FF sets its
  +0x64 (0x87652C) to 1, so the clamp at 0x5BE839 keeps 1 and the
  registry never touches it. Read from the running original under Wine
  (free roam after the D-Day SMS): 0x870D14 = 1; clock 0x7FB718 = 48.4 s,
  state 0, index 0, list SUNRISE_A → SUNRISE_A (info 0x068B8C80); at 63.3
  s state 4, index 2, 0x8A1C88 = 0.582, the list SUNRISE_A → NIGHT_A
  (0x8A9A05CF) with its info resolved, CAP → NIGHT_A_CAP; at 86 s state
  0, 0x8A1C90 = 1, and the screenshot shows the night dome
  (wine/compare/fe72/sky-cycle-orig.png). So the gate is open, the cycle
  runs (sunrise for the first minute, then night, sunset, night, every
  300 s, each fade 20 s through skymorph), and the fog weight follows. The
  clone now runs it by default (`TimeOfDay::default`; `NFS_SKY_MORPH=0`
  holds it for comparisons).
* **Which pair the shipped game shows (corrected 2026-10-29, apitrace of
  free roam at the airport, t2.trace call 35363485).** The dome is the
  first draw of the main view: 96 triangles, opaque, ZWRITE on, texture
  0x6237290 (512×128 DXT1), then 48 triangles alpha-blended, ZWRITE off,
  texture 0x6237870 (256×256 DXT3), both world.efx (vs 0xCF2DE0, ps
  0x26A3030) with FogValue (250, 0.02, 0.6) and FOGCOLOR 0xFF493763. The
  level-0 uploads of those two textures (calls 10989795 / 10989811) decode
  to the **SKY_SUNRISE_A / SKY_SUNRISE_A_CAP** pixels (LOC4DYNTEX blocks at
  0x14400 / 0x1D5C0); the night pair is uploaded too (0x6263428 /
  0x6263A08, calls 10989760 / 10989776) but not bound in that frame. So the
  shipped sky is the authored sunrise pair, not `SKY_NIGHT_A`. The static
  path says otherwise: 0x60F890 writes the remap list 0x8A2CAC (16-byte
  entries {from hash, to hash, info (−1 = resolve), slot}; from = the
  table's SUNRISE_A / _CAP, to = NIGHT_A / _CAP with the gate closed) and
  hands it to the dome object 0x8A1DFC through 0x488E20 (+0x10 list, +0x14
  count 2); 0x5C5940 applies it with 0x4905A0 (resolve each `to` with
  Texture_GetInfo(hash, 1, 0), swap the info into the solid's texture table
  +0x2C where the hash matches) and undoes it with 0x488E40 after queuing
  the draws (0x5CEB40 keeps the swapped info in the texture group, which
  0x5CF430 binds with Texture_Bind). **Resolved:** the remap is
  SUNRISE_A → SUNRISE_A during the first minute (index 0, gate open; see
  Shipped behaviour) — the trace was taken then.
  Measured (`NFS_NO_POST=1 game --shot intro`): the sky at the top of the
  frame is (73, 66, 89), the original's pre-post (73, 66, 89).
* **Depth and blend (verified** from the effect blobs' pass assignments,
  16-byte {state, index, value, type} records). skymorph.efx's pass sets
  CULLMODE (table index 8) NONE and ZFUNC (9) LESSEQUAL, as world.efx's
  pass p0 does, plus state 0x1E = 0 and sampler state 0xA6 on stage 1
  = 1; neither pass assigns ZWRITEENABLE, ALPHABLENDENABLE or the blend
  factors, so both inherit those from the game's state at the draw. The
  clone draws the dome with the world pipeline's states.

## Parameter maps and fog (verified)

L4RA.BUN container 0x8003B600 holds 13 named, position-dependent tables.
`crates/ob-formats/src/formats/parameter_map.rs` documents the layout and the quadtree lookup
(SPEED2.EXE 0x5F0940). Fog is computed as follows:

* 0x614310 samples "Normal Fog" (objects at 0x8A1E90 + 0x30·(view − 1))
  and "Night Normal Fog" (0x8A31F0 + …) at the camera. The names come from
  the static constructors at 0x77F780/0x77F7B0. Row fields: f0 strength,
  f1 start distance, f2–f4 colour (0–255).
* 0x5FA670 smooths each map's sample on its own over 50 units of camera
  movement.
* 0x614310 then mixes the two rows: `normal·w + night·(1 − w)`, with
  `w = 0x60FA90()` (see Sky). In the shipped game w is always 1, so
  Bayview's fog is **"Normal Fog" alone** (the night map is not used).
* When it rains (view +0x64 → +0x1F0 > 0), 0x614310 also blends every
  field toward "Night Raining Fog" by the rain amount (see **Rain**;
  `FogState::update_rain`).
* 0x5CAAE0 sets `FogValue = (250, 1/(250 − f1), min(1 − f0, 0.6))` and
  FOGCOLOR from the packed colour 0x614310 leaves at its output +0x14:
  **corrected 2026-10-29 (verified, asm 0x614595…0x6145C2 and
  0x5CB0D1…0x5CB199, apitrace):** 0x614310 truncates f4, f3, f2 (`_ftol`)
  into `0x80000000 | f4 << 16 | f3 << 8 | f2`, so **red is f4 and blue
  f2**; 0x5CAAE0 multiplies each byte by 0.75 (0x785510), truncates, and
  sets that as FOGCOLOR (effect handle +0x45C). The clone used (f2, f3, f4)
  unscaled: at the airport ("Normal Fog" row 12, f2..f4 = 132, 74.8, 99) it
  fogged toward (132, 75, 99) — magenta — where the original's trace has
  D3DRS_FOGCOLOR 0xFF493763 = (73, 55, 99). `fog::fog_color`.
  The output struct is 0x89E850 (.bss): +0 f5, +4 f6, +8 f7, +0xC f1,
  +0x10 f0, +0x14 the colour. 0x5F0B50(i) reads field i of a smoothed map
  object (+0xC map, +0x18 current row; field offsets at map +0x10),
  0x5FA580 blends each float field (type 0) by the distance factor and
  truncates int fields (type 1), 0x5FA4F0 returns a row by quadtree
  (0x5F0940). 0x8A1D70 (never written) would force a fixed row from
  0x800E18… (0.62, 0.31, 1.3, f0 0.74, f1 212, colour 110/105/75).
* **No fog maps (the front end's garage scenes; verified, apitrace of the
  Quick Race car select).** 0x614310 returns 0 without writing when the
  map objects are not bound, and 0x5CAAE0 builds the constants from the
  untouched .bss struct anyway: world.efx's c8 = (250, 0.004, 0.6),
  FOGCOLOR 0xFF000000, FOGENABLE on. So the garage's far scenery (the
  skyline and the buildings beyond 100 units) is darkened up to 40 %
  toward black. The clone had no fog there (`Fog::UNSAMPLED` now): the
  background at (640, 300) went 39 → 31 (original 32), at (1000, 380) 99 →
  86 (original 86). After a game session the struct keeps the last in-game
  sample (**verified** in memory under Wine, 0x89E850 bit-identical
  in the main menu and the Career screen after a race and after free roam;
  the map pointers cleared): `Fog::frontend` gives the last
  `FogState` output, zero before any. The garage view draws with it
  (`Fog::frontend` in src/bin/game/garage/draw.rs, the shop fixes).
* world.efx's vertex shader (shader text in the trace): `viewZ = dp4(v0,
  c6)` (the WorldView z row), `oFog = min(max((c8.x − viewZ)·c8.y, c8.z),
  1)`, `oT0 = v2 + c7`.
* **Resolved (2026-10-29): the "darker garage floor" was not the
  floor.** The comparison point (80, 900) / (640, 780) fell under the
  clone's front-end quads (the stat bars and the help bar), which the
  original's pre-post snapshot (taken before the FE draws) does not
  have, and the two frames were framed differently (see **Front-end
  projection under Wine** below). With `NFS_NO_UI=1` and the trace's
  projection (`NFS_FE_HW=0.72057`) the clone gives (80, 900) 34 / 34,
  (640, 780) 68 / 66, (300, 850) 43 / 43, (1000, 850) 56 / 53 (clone /
  original). What did differ under the car — the missing shadow — is
  fixed (Car shadow, *pulled toward the eye*), and the car now goes into
  the floor reflection (Road reflections, *front end*).
* **Intro (2026-10-29, measured, no change needed):** with the views
  aligned, the road and the headlight pool's halo match before and after
  the screen passes to within 1–3 levels (`NFS_NO_UI=1 [NFS_NO_POST=1]
  game --shot intro` against orig-intro[-prepost].png, regions free of the
  original's HUD): far road left (94, 92, 80) / (94, 92, 80) pre-post,
  (101, 87, 65) / (101, 87, 66) final; mid road (157, 134, 104) / (156,
  133, 105); the pool's edge (196, 180, 151) / (197, 180, 152); above the
  pool (129, 110, 77) / (131, 112, 80); sky (58, 39, 54) / (56, 38, 55).
  Earlier "darker road" readings sampled under the original's minimap.

## Rain (2026-10-30; verified unless marked)

`src/weather.rs`, `crates/ob-render/src/render/rain.rs`, `fog.rs` `update_rain`,
`postfx.rs` `RainFrame`. The view's weather object is `*(view + 0x64)`
(constructor 0x61E160; views from 0x48B1E0(i) = 0x832DE0 + 0x70·i). At
track load 0x61E100 sets both views' wetness +0x509C = 1 and calls
0x613060(5, 1.0): RAINDROP / SNOWFLAKE textures (+0x4D7C…), 400 drops
(+0x4D00), all of type 5 (hidden), at random places in the box, the 10
per-frame drop motions (+0x5120: (rand 0.02 − 0.01, rand 0.02 − 0.01,
−rand 0.03 − 0.45)) and the fall table (+0x4D4C, from 0x8039CC…). The
frame function calls 0x61F130 on view 1's object (0x5CB47B), in game
(mode 6) only (0x5CB40D; the front end's garage scenes draw no rain:
docs/formats/front-end.md, "Garage scenes: no weather"):

* **Paused** (0x571260: race state [0x8900D8] 6 and not online): only
  the tint and the drop draw run, with the values as they are.
* Skipped when |dt| ≤ 0.009 s (0x7A873C; the frame time 0x865150). The rain tint's slot 2
  (0x5D75F0 at 0x61F1DB) is applied only on the paused path and at the end of the update
  (0x61F3ED…0x61F467 → 0x61F1D9), so a skipped frame applies no tint either.
  **Reference captures (observed under Wine, DXVK):** the native frame period is 1/120
  (0x7FB710 → 0x865558, see the frame limiter), and at 1/120 = 0.00833 s every frame is
  skipped on a fast machine: the rain amount, the wetness and the tint stay as they are
  (wetness 1 from the load, the amount 0 unless written, slot 2 never on). With 0x865558
  written to 1/60 the original runs at 60 frames a second, the weather steps (the
  wetness falls 1 → 0.42 in a few seconds after the shower) and the tinted frames match
  the clone's Original (which plays as the original at 60, docs/original/uncapped-fps.md): at the
  traffic light (1169.9, −304), the shower 0.7, mean luma without HUD and car 34.1 vs
  32.9; at 120 frames a second 40.0. Compare rain at 60 frames a second, with every
  option global 0x870CB0 + 8 … + 0x68 at its maximum (0x8764C8, same layout) read back
  from memory: one capture session ran with the performance-level-3 defaults (0x87099C =
  3; Bleach Bypass 0x870D00 and Rain 0x870D10 off, Car Env Map 1, Road Reflection 2,
  World LOD 2, Texture Filtering 1) and read 20 luma brighter in rain. Otherwise: the lens drops
  (0x614100); 0x8A1D50 += 45·dt, wrapped at 360 (its reader 0x61356C
  is not traced); wetness: +0x5098 = (rain ≠ 0), +0x509C steps toward it
  by 0.05 a frame when rising, 0.0015 when falling (0x7A67C8 / 0x7A67CC;
  only 0x490920 reads it: the reflection flares' size factor 0x7F39A0 =
  1.2 + (10 − 1.2)·wet, mode 2, `glow::streak_factor`).
* **Target** (+0x5094) = 0x61E390(camera x, y) (0x5F0C60: camera +0x40 /
  +0x44), × 0.125 with 0x5B98B0 (screen layout) ≥ 3. 0x61E390: `s =
  0x6134F0(x)`; the "Rain" map (object 0x8A1F28, name 0x7A9410, static
  ctor 0x77F700; data pointer 0x8A1F34) sampled at (x, y) gives `[amount,
  threshold]`; the target is the amount when threshold ≥ s, else 0.
  0x6134F0: 0 when 0x8A1D48 (BSS), 1 on a dyno run (0x89E7E6), else
  `|0.5·cos16(0x401020(clock / 1 + x / 50)) + 0.5|` (kept at 0x89F048),
  the clock 0x7FB718 = sim ticks 0x865188 × 0.00025 (0x57A1A2; 0x576D00
  sets 4000 ticks / 1.0 s at load). L4RA's map: rows [0.5, 0.5], [0.9,
  0.15], [0.75, 0.05], [0.95, 0.25], [0.8, 0.2], [0.7, 0.1], [0.8, 0.2]:
  each place rains for part of every 360 s cycle, the phase shifted by
  x / 50 degrees. Checked under Wine at the airport (x 1735, clock 12.53:
  0x89F048 = 0.836; with the ticks written to 584800, clock 146: target
  0.75 (row 2), rain within a few seconds).
* **Amount** (+0x1F0): `+= (target − rain) / (1 / dt · 1.0)`; 0 when the
  target is 0 and it is under 0.01; capped at 1. 0x6139C0 (0x8A1D38 is a
  BSS debug override to 0x803968 = 1): +0x4D30 = ⌊(1 − rain)·400⌋ (type 5's
  quota), +0x1F4 = 400 − that; 0x613A40 sets the type quotas +0x4D1C[t] =
  ⌊+0x1F4 · share⌋ with shares +0x50D4 (type 0, 1.0 each frame), +0x50D8,
  +0x50E4 (0).
* **Drops** (0x61E710, then 0x613A80 draws): 0x6136C0 first: the "Wind"
  map (0x8A3180 + 0x30·(view − 1)) at the camera, smoothed over 50, its
  f1 as degrees → (sin, cos, 0) × 0.01 (f0 unused) in the car's frame;
  a gust machine (global 0x803E60, 2 at start): 2 → wait rand 5 + 1 s
  (+0x4ED0), target +0x4EC0 = (rand 0.05 − 0.025)·25 + wind, state 1; 1
  → +0x4EB0 / +0x4EB4 move toward the target by dt for one second
  (0x8A44E4), then 0; 0 → wait. Then the rain space: the player car
  (0x5F0E40(0x5ED990)) position with the yaw of its forward vector (car
  +0x90, matrix 0x5BACA0); the yaw change since last frame (0x8A44F4,
  binary angle, ±400) turns the drops (y += Δ·5e-5·x). The camera's
  velocity (camera +0x1E0) in that space × −0.02 (0x803A14), its y
  zeroed, is the push; its length (≤ 1) the streak stretch +0x5090
  (only when the car's +0x18 → +0x238 → +0x0C is 1; **inferred** true
  for the player). The position buffers swap (+0x1F8 / +0x1FC); each
  falling drop: new = old + push + wind·fall[type][sub] + motion[k % 10];
  z < 0 → state 1; else one wrap of x into 8 ± 10 or y into ±3.5
  (0x8039AC / 0x8039B0 / 0x8039B8; centres 0x8A1D3C / 0x8A1D40 BSS), both
  buffers; flag +0x27 = x ≥ 0 and the unit position's x ≥ 0.96. Drops in
  state 1 / 2 move to the first other type under its quota (while any
  count differs from its quota), and unless type 4 / 5 restart: state 1
  at z = 6 (0x8039B4) with both buffers equal. **Cover** (2026-10-31,
  race2; verified, 0x61EC58–0x61ED77; ported, `Weather::update_cover`):
  before the drop loop +0x1E4 and +0x1E8 are cleared; four 0x5D9C80
  searches over the route data 0x88F298, each handed the previous one's
  result (0x5D9C80(point, mask, prev) starts at prev's kind and goes on
  after prev within it, so a mask below prev's kind finds nothing): P0 =
  the car's place (+0x50 / +0x54) with mask 0x20 (kind 5) then 0x40 (kind
  6), P1 = P0 + 9.5 (0x7A67D8) × the car's unit forward axis (+0x90,
  its x, y) with 0x20 then 0x40; a hit whose +0x14 is above the car's z
  (+0x58) sets +0x1E4 or +0x1E8. View 1 copies +0x1E4 to
  0x8A1D4C (no reader by address found). At the end of each drop's step
  (every state), a drop whose new x (rain space, forward) is over 9.5
  with +0x1E8, or under 9.5 with +0x1E4, becomes type 5 (+0x4D04 counts
  moved), state 2 — hidden; the quota pass may bring it back as type 0
  without a restart at the top (state 2). The lens drops (0x614100,
  before 0x61E710 in the frame, so last frame's flag) are 0 while +0x1E4.
  Nothing else tests the flags: the amount, the wetness (+0x5098 /
  +0x509C, 0x61F296…0x61F2F0), the tint and the fog follow the rain map
  at the camera, so the road under a roof stays as wet as in the open
  and the road reflection's flares keep their streak. **Observed** under
  Wine (fixed 1/60 s frame, free roam, read eight seconds after the clock
  was set): the Observatory tunnel (−1701.3, 2205) amount 0.800,
  wetness 1.000, +0x1E4 / +0x1E8 1 / 1; its mouth (−1701.3, 2130) 0.800,
  1.000, 0 / 0; the covered lot under the elevated line (136.7, −839)
  0.700, 1.000, 1 / 1; an open street (−237.3, 79.8) 0.700, 1.000, 0 / 0.
  The clone gives the same values at the same places and clocks, and
  Wet Roads' road wetness follows the same rule
  (`the_road_stays_wet_under_cover_as_in_the_original`). The clone reads
  the kind 5 / 6 triggers from the route data the track load reads: free
  roam's L4RA PathsFreeRoam.bin, a race's own Paths<track>.bin when it is
  there (`RouteTriggers::load_kind_for`).
* **Drop draw** (0x613A80, 0x803ABC = 2: the batch 0x5BB400 / 0x5BB440
  / 0x5CC260 with the rain space as the world matrix): per type 0–3 with
  its texture (type 0 RAINDROP), each falling drop: dir = unit(older −
  newer), A = (the camera's look direction camera +0x50, a unit vector
  read under Wine, under +0x5050 × dir)·+0x4DB0 (0.03), B = dir·(+0x4DB4
  (0.7) + stretch); corners c + A + B, c − A + B, c − A − B, c + A − B;
  colour template +0x4F90's 0x1C646464 (+0x4F10's 0x80808080 for drops
  with +0x27); 0x5BB440 skips colours with alpha ≤ 0x14. **Inferred**:
  the UVs (the batch's struct +0x14…+0x20 are not written in 0x613A80;
  taken as 0–1). **Verified** (2026-10-31, look112): the effect is world.efx
(0x86EB28, bound at 0x5CB42C before 0x61F130 at 0x5CB47B; fogged), the
colour the template as it is (0x5BB440 copies it; the pixel shader doubles
it).
* **Lens drops** (0x614100 / 0x5C7AC0 / rain_drop.efx; the original under
  Wine runs the widescreen fix with `RainDropletsScale = 0.5`, which makes
  them smaller round drops: not native): see
  `crates/ob-render/src/render/rain.rs`. 20 drops (10 split screen) while rain ≠ 0 and not
  under cover (+0x1E4), each (ctor 0x6140A0) life rand 1.5 + 0.3, speed
  rand 0.1 + 0.02, size rand 1; a live one slides down 0.2·speed·dt and
  out by |camera velocity|·1e-4 along unit(x − 0.5, y − 0.1); expired or
  off screen → new random place. Drawn when 0x8707D2 (game, g_RainEnable
  0x870D10, Particle System 0x870CEC).
* **Fog**: 0x614310's rain branch (0x6145C5…): "Night Raining Fog"
  (0x8A1E30 + 0x30·(view − 1)) smoothed over "Fog Blend Distance"
  (0x8A3250 + 0x1C·(view − 1), f0; 50 without it), every field mixed
  `rain·raining + (1 − rain)·normal`, the colour packed red = f4 as
  before and handed to 0x613030 (+0x4EDC… = f2, f3, f4 truncated).
* **Tint**: 0x61F1D9 applies slot 2 (0x5D75F0) with {f2, f3, f4, 128,
  rain, 0}; 0x5C7870 (g_TintingEnable): per channel c = ⌊((⌊v⌋/128 − 1)·
  rain + 1)·127.5⌋, R from f2, a full-screen WHITE16X16_NOALPHA quad
  (0x5C7150) blended DESTCOLOR / SRCCOLOR (0x5BCBD0): dest × 2c/255.
* **Sound**: CARSFX_Rain's update (0x474770) reads +0x1F0 against 0.001
  (0x78435C): `Weather::raining()`; ported in Sound round 5 (`roadwind::Rain`).
* **Grip**: 0x5ED5E0 = 0x61E390(car +0x60, +0x64): the player's base grip
  × (1 − 0.025·w) (0x5A4E75, 0x7A19EC) and Melov8r's grip +0x140 likewise
  (0x5AB01F, 0x7A198C), which scales its soft-slip threshold, soft gain
  and force cap (0x5A3AF0 / 0x5A3B09 / 0x5A3CBA).
* **The clock on the play path** (2026-11-01; verified, Wine
  with the finished-career save): 0x576D00 (4000 ticks, clock 1.0) runs from the
  world object's ctor 0x5FF4A0 (World_Start 0x60A690, each track load:
  free roam, a race, back from the garage) and its unload 0x5FF700; the
  front end's clock runs on (119 s in the garage) until that load. Only
  the online sync (0x601BBC / 0x601C37 from 0x869234) writes it otherwise.
  The ticks run every game frame (0x57A180) and stop in the pause menu
  (911.33 for 3 s). Idle at the crib's exit (camera −64.7, −1473.9; Rain
  row [0.7, 0.1]) the original rained from clock 143.79–144.78 to
  217.86–218.85 and again 504.09–505.10 to 577.80–578.81, as the formula
  gives (144.42–218.17, + 360). Under Wine at 120 fps most frames are ≤ 9
  ms, so the object steps only on the slower frames there (the wetness
  holds for seconds, the amount ramps unevenly). The clone (fixed
  since): `step_explore` with traffic or an encounter racer out and
  the live race never ran `Drive::tick_clock`, so the clock stood at 1.0 in
  real play and it never rained (nor did the sky reach night). Recovery
  ticks `step_explore` and the live `race_frame`, keeping the countdown
  calls intact so the clock advances once per frame. `--flow rain` checks the shower at the crib's exit (144.44 to
  218.17), the selftest the clock in free roam with traffic.
* **The clone**: the object steps at a fixed 60 Hz (**inferred**: the
  per-frame rules assume the original's frame rate; the 9 ms skip would
  freeze it at the clone's rates); its random numbers are its own
  GameRng (the original shares 0x7EA624). `NFSU2_RAIN_CLOCK=<s>` starts
  the clock of a track load there (and a `--shot` steps the weather 5 s
  up to it). Compared under Wine at the airport (clock 152–155, rain
  0.75; a side-by-side kept with the maintainer's private evidence).

## Effect selection (verified)

The 22 effects live in a descriptor table at 0x7FF850 (0x6C bytes each) and
are loaded into the array at 0x86EB28 in the same order: 0 world,
1 worldheadlight, 2 worldreflect, 3 worldheadlightreflect, 4 worldbone, 5 car,
6 worldmin, 7 worldnofog, … 13 worldnodepth, … 20 skymorph.

* Per mesh (0x5C58B0, at load time), from the mesh header flags at +0x0C,
  using the mask table at 0x800464: 0x01 → worldbone, 0x40 → worldreflect,
  0x80 → car, otherwise world. In Bayview 0x40 is set on 1,054 road meshes and
  550 others (sidewalks and similar), and 0x80 on 115 shiny props. Water and
  panorama meshes have no flags.
* Per draw (0x5C5940):
  * a texture with +0x56 == 2 (additive) on effect 0 or 1 switches to
    worldnofog, so the original never fogs glows;
  * solid hash 0xBE43EDBB uses effect 20 (skymorph), but only while the
    time of day fades and 0x870D14 is set (never, in the shipped game; see Sky);
  * two texture hashes use a special effect (runway lights);
  * remap tables at 0x7FF090/0x7FF0B4/0x7FF0D8 switch to the headlight
    variants, or turn reflect into world when `g_RoadReflectionEnable`
    (0x870CD4) is 0.

## Road reflections (verified except where noted)

worldreflect.efx: `rgb = 2·tex·v0 + ReflectedTex[screen]·(tex.a·v0.a)`,
alpha = 1, with the usual fog. Its vertex shader projects each vertex to
screen space for the lookup, and does not add TextureOffset. On these roads,
texture alpha is the reflection strength. That is why the +0x55 == 2 road
textures have alpha but do not blend.

The frame function (0x5CAAE0) renders view 4 (0x48B1E0(4) = 0x832DE0 +
4·0x70) before the main view, when `g_RoadReflectionEnable` (0x870CD4) is
non-zero:

* **Target.** 0x5BA3D0(1) selects render-target record 1, which 0x5BA220
  builds for view 4. 0x5C21F0 sizes view 4 at **320 × 240** (0x140 × 0xF0).
* **No fog.** 0x5BA3D0 stores the record in 0x870848. For any record other
  than 0 (the back buffer), 0x5C5940 replaces every effect except car
  (type 5) with worldnofog (0x86EB44). The same applies to the environment
  cube's targets.
* **No sky.** 0x60F890 gives views outside 1–3 no SKYDOME (see Sky).
* **Clear.** 0x5BA3D0(1) clears colour and depth to opaque black
  (0xFF000000).
* **Contents by level.** 0x6154A0 is always drawn: it is not the car
  bodies but each car's headlight pool (see **Headlight pools** below).
  The car bodies (0x61B200 →
  0x615A20) are only drawn in the main views (0x5CB622, 0x5CBC8C), so
  **no car is reflected**. The world (0x5D6AE0 with flag 8, which becomes
  cull flags 0x2000), 0x566410, 0x631260 and 0x630DF0 are added when the
  level is > 1. 0x5C7E50 is added at level 3 in game.
* **Camera.** 0x5C2FA0 points each view's +0x40 at its camera: view 0 →
  0x872C00, 1 → 0x8760E0, 2 → 0x872F20, 9 → 0x870A20, 3 → 0x8718C0,
  **4 → 0x8760E0** and 5 → 0x8760E0 as well. View 4 is view 1's camera,
  unmirrored, with the same 0x5C7ED0 projection factors (see Chase
  camera); at 320 × 240 its aspect matches a 4:3 screen. Worldreflect's
  ReflectedProj (0x5BC7C0) is the current view's view-projection times the
  texture bias (0.5, −0.5, 0.5, 0.5), so the lookup is the pixel's own
  screen position.
* **The mirroring is per instance.** 0x5B9C30 gives view 4 cull base 0x800
  (0x4888A0 → context +0x44). The scenery draw (0x5D6787) copies an
  instance's matrix and, when the context has 0x800 (0x5D67B4), negates its
  m22 (+0x28, the local z axis's z): each prop is flipped about its own
  origin, so it hangs upside down below its base, where the road samples
  it. Instances with no matrix (flag 0x0100) are drawn unflipped, but in
  L4RA all but 3 of those that pass view 4's cull (flag 0x0040) also have
  0x0400 and are left out, so the image holds the flipped props on black.
  The prop draw 0x61D160 does the same (0x61D2E8) and, with 0x800, draws
  its +0x0C model instead of +0x08. The clone (`vs_common`, frame clip.w)
  flips every placement with a matrix (instance m0.w = 1) the same way.
* **LOD.** Views whose cull base has 0x1800 (view 4, the cube faces) take
  LOD C (info +0x38), or LOD B (+0x34) for flag 0x0400, and draw nothing
  when that solid is missing (0x5D672A; `Placement::low_solid`). That
  +0x30/+0x34/+0x38 hold the solids of the hashes at +0x20/+0x24/+0x28 is
  verified (the scenery loader, Solid order below). Those views also need a
  screen size (0x5D64E0: `scale · r / (|d| − r)`, context +0x80, 0 behind
  the eye) of at least 32 where the main view needs 17 or 23. The clone
  applies the gate per instance in `vs_common` (verified, see the size
  scale below).
  * Solid order (verified): the scenery loader 0x5DFA8A walks each info's
    three hashes (+0x20, +0x24, +0x28) and stores the model of each at the
    same index from +0x30 (`[hash slot + 0x10]`, 0x5DFB3D), reusing an
    earlier slot's model when the hash repeats and leaving the slot null
    for 0, the SKYDOME (0xBE43EDBB) and the cube's sky solid 0x72923C81.
    So +0x30/+0x34/+0x38 are LODs A/B/C of +0x20/+0x24/+0x28, as the clone
    takes them.
  * The size test (verified apart from the scale): 0x5D6580 passes the
    instance's position (+0x20) and `info +0x3C + 6` (0x7A47D4) as the
    radius r; with the forward distance d (eye at context +0x60, forward at
    +0x70) it returns 0 when d ≤ −r, the scale (+0x80) itself when
    |eye − p| − r ≤ r, else `scale · r / (|eye − p| − r)`, truncated
    (0x75BED4); 1 or less is culled (0x5D6656). Cull bases with 0x1800
    need 32 (0x5D6730 / 0x5D673E) — view 3 too, whose base 0x1000 has
    0x1800 (see Rear-view mirror); a 0x20 context without 0x1800 would
    need 16 (0x5D666F, not reached by view 3) — the main views 23 in
    the split-screen layouts (0x5B98B0 ≥ 3) and else 17 (0x5D66A4), where
    a further test (0x5D66AD..0x5D6717, **verified**) picks LOD A or B:
    split-screen takes LOD B; otherwise x = 0 without a LOD A
    solid or its +0x0C object, 4 when that object's u16 +0x14 is under
    0x28, else (with its f32 +0x94 > 0) the screen size / max(+0x98, 6),
    and LOD A is drawn only when x ≥ 4 and 0x5BE1D0 returns 2, i.e.
    g_WorldLodLevel (0x870CF4, registry "g_WorldLodLevel") is 2 or 3 —
    else LOD B. The clone's main view keeps LOD A, as at the top level
    (**open**: the +0x0C object's fields are not identified).
  * The size scale (verified; `cull_scale`). 0x5B9C30 ends by jumping to
    0x5E9A40, which fills every context each frame from its view (+0x40):
    +0x60 the eye (camera +0x40), +0x6C..+0x74 camera +0x50, +0x80 the
    view's +0x0C and +0x84 the eye's visibility cell (or −1). View +0x0C
    is written by the projection 0x5C7ED0 (0x5C80BD): `256 / tan(a)`
    (0x78D860), or 10 when the tangent is 0 (0x784560), where `a` is the
    camera's field of view (+0xC4) shifted right once — the camera's own
    angle, before the 1.1 widening. So the chase views (71.5°) have a
    scale of about 356, view 4 the same (view 1's camera), the 90° cube
    faces 256. Null-transform scenery is sized about the origin (its
    position is 0), and its radius (info +0x3C, e.g. 3,800 for
    `TRN_SH_RoadA_CHOP_A1_R2`) reaches it from there, so it always
    passes. The clone carries the radius + 6 in m1.w of each instance
    and drops the instance in the vertex shader below 17 (main view) or
    32 (view 4, cube faces).
* **Scenery left out** (verified): instances flagged 0x0400, and the view's
  cull flags 0x854/0x858 (instances without 0x40; see Scenery).
* **Front end: the car is reflected (verified, apitrace t1 Quick Race car
  select, calls 10634774…10639755; 2026-10-29, fx70).** The garage's
  view-4 image (render target 0x5D79FF0, 320 × 240, cleared to
  0xFF000000, worldnofog for the world) holds, in order: two 2-triangle
  fans (world.efx, alpha test 95, texture 0x630C880; **open** what they
  are), the FE car's opaque parts (car.efx, CULLMODE NONE), BACKDROP
  (6267 vertices) and PLATFORMBASE (150 vertices) flipped, then the car's
  blended parts (CULLMODE CW, as in the main view). The car is mirrored
  about z = 0.15 (live 0x7F45A8, also the tyre contact height).
  Same-frame main/reflection WorldView uploads reconstruct the reflected
  RX8 mesh within 1.3e-6 m of `T(0,0,0.30)·S(1,1,−1)·base`, followed
  by Car_Draw's +0.01 world-z bias. The older z=0 claim compared WVPs
  with different depth projections and did not establish a plane.
  HarmonicCoeff's z-odd terms (c5 xz, c6 yz, c9 z) are negated, and
  LocalEyePos uses the reflected base axes (then clamped to 8, 0x48A6A0).
  The floor/discs sample it through worldreflect. In game the car bodies
  stay out of native view 4 (above). Clone: `render_with_reflection` calls
  the garage's `render_reflection`, with `garage::reflected_car_world`
  and lighting computed for that base, in a separate submission because
  main-view uniforms overwrite it. Evidence: `docs/evidence/fe-reflection-plane`.
  `NFS_REFLECTION_PNG=path game --shot …` saves the image.
* **Headlight pools** (verified; `render::HeadlightPool`). 0x6154A0 has
  one caller, 0x5CAC17, inside view 4's block of 0x5CAAE0, so the pools
  exist only in the reflection image and reach the screen through the
  road's worldreflect lookup. It runs after the world, 0x566410, 0x631260
  and 0x630DF0, with world.efx itself bound (0x5CABDF: 0x86EB28, effect
  0; not 0x5C5940's per-draw remap). Over the car render objects
  (0x890080) it takes each with +0x550 set whose 0x60D500 is 0 (the
  camera's +0x998 byte, unset in play) and bit `view` of +0x5B4 (all set,
  0xFFFFFFFF at 0x6042C6); the matrix is car +0x560 / +0x5A0 when bit
  `view` of +0x5B0 is set, else +0xD0 with the position +0x50 — +0x5B0 is
  only ever zeroed (0x6042C0), so always the car's own. After a box test
  (0x5BC310 on the render info's box × 10, 0x802908) and only while
  0x802904 = 1 and the car's +0x93D is set, 0x60D5E0 draws, if the
  matrix's z-axis z (+0x28) is at least 0.707, a fan (0x5C5590,
  D3DPT_TRIANGLEFAN of four 0x20-byte vertices) in car space: x from 3 to
  23 (13 ∓ 10: 0x8028CC, 0x8028E4/E8/F4/F8), y ±4.9 (0x8028EC/F0/FC,
  0x802900) about 0x8A1C4C (BSS, never written: 0), z 0, UVs (0,0),
  (1,0), (1,1), (0,1). The texture is `2PLAYERHEADLIGHT1` (0x60C6B0 →
  0x8A1C28; GLOBALB, DXT3, +0x56 = 2, so additive through 0x5BCC40). The
  vertex colour is render info +0x380 | 0x80000000 (alpha 0x80): 0x635D62
  packs the lights part's (slot 0x9F, ride +0x80C) RED, GREEN and BLUE
  attributes (strings 0x79853C/2C/34; 0 when missing) as R | G<<8 |
  B<<16, or 0x90AA90 when the car has no such part (0x635D9B); 0x5C5590
  turns bytes +0x60..+0x63 into ARGB with R the low byte. +0x93D (lights
  on) is set by the constructor (0x6045B0) and again by 0x5F61A0 unless a
  flicker (+0x925, 0x5EC170) is running, which the clone does not have.
  The clone draws a pool for the player, the race's rivals and explore
  traffic; it skips the box test (the GPU clips).

## Rear-view mirror (verified except where noted)

The Bumper and Hood cameras show a rear-view mirror at the top of the
screen when the Rearview Mirror option is on. It is a second world view,
view 3, drawn into its own target and put on screen as one FE quad. The
clone: `crates/ob-render/src/rear_view.rs`, `WorldView::RearView` (render/mod.rs),
`Game::rear_view_on`, `rear_view_cars`; `--shot hood` / `--shot bumper`
(`NFS_REAR_PNG=path` writes the mirror's image), `NFSU2_VIEW=k` for the
bench.

* **When** (HUD_BuildFeatures 0x6000A0): HUD_FEATURE_RVM, low 0x1000
  (feature name table 0x7F6728, index 12), is set only in the branch for
  a race in progress or free roam (racer +0x28 = 0, 0x60048C..0x6004B7):
  the option byte 0x83AA18 (profile settings +0x14, default 1) set, the
  look-back key not held (player +0x249, set by event 0x7E in 0x602BE0)
  and the player's camera setting (0x83AA74 + player × 0x28: 0 bumper, 1
  hood) 0 or 1. So no mirror before the green light (race status 3, the
  pre-green branch), at the race over (0x5FFF60), in the letterbox
  states (race status 1, 2), in the D-Day script's branch, with a camera
  override (player +0x238 kinds 2, 3, 4, 0x12 return before), nor in any
  state that returns no features (+0x58 = 1 or 6..7, 0x82A3A4 > 0,
  0x5012C0 / 0x5012D0). Ported in `hud::features` (`Input::rear_view`).
* **The view.** Every frame 0x6005A0 compares the features with the last
  ones (player +0x258 / +0x25C); on a change 0x4CA090 calls 0x5F1E20 with
  (new & 0x1000) when the RVM bit changed: it deletes the player's
  mirror mover (+0x244) and whatever mover view (player +0x1C = 3) holds,
  and with 1 makes 0x44BA10(view, car) (0x30 bytes, base 0x44AB90 type
  6, vtable 0x787D98, +0x2C = the car, player +4). 0x5C2130 / 0x5C21F0
  pick the screen layout 0x8707DC: 2 (view 1 and the mirror) while view
  3 has a mover and view 1's mover's vtable +0x24 is false — 0x445A90,
  true for the chase kinds (far, near, high) — else 1; layout 2 sets
  view 3's active byte +8 (0x5C23C5). The clone draws the mirror when the
  HUD update asked for it and the chase camera is not a chase kind
  (`ChaseCamera::is_chase_kind`), with the player's own camera (not a
  replay's or the showcase's).
* **Camera** (0x44BA80, the mover's update; view 3's camera is 0x8718C0,
  0x5C30E3): M = transpose(car +0xD0) (0x5BAF90 → D3DX 0x63D129), then
  M × RotateX(0x4000) × RotateY(0x4000) × RotateZ(0) (0x5BAD00 / 0x5BADB0 /
  0x5BAE70, row vectors), translated by −(car +0x50) first: view x = the
  car's left, view y = the car's down, view z = the car's back — the view
  looks out of the car's back, upright. Then the view's translation
  + (0, 0.5, 0) (0x7EC4C0): the eye 0.5 m above the car's origin along
  its up axis. Field of view 20000 (binary angle, 109.86°; camera +0xC4,
  written unless 0x828640, read 0 at run time), near 0.75 (0x7870D4), far
  192 (0x7870D8). 0x44BA10 sets camera +0x280 = the car (unless
  0x828640) and +0x284 = 0, so World_DrawCars (0x6316E0) skips the
  player's own car in view 3. **Inferred**: car +0xD0 is the body's
  rotation with its rows the car's axes (so its transpose maps world to
  car space); the clone uses the car state's orientation.
* **Projection** (0x5C7ED0): view 3 is not among views 1, 2, 4, 5, so
  there is no 1.1 factor: x = 1/tan(fov/2), y = −1/tan(fov·(h/w)/2) with
  h/w from its target record (256 / 256 = 1): square, 109.86° both ways.
  0x5C7ED0's scenery cull scale is 256/tan(fov/2).
* **Target** (render-target record 3, 0x86E7BC, 0x5BA220 case 3; view
  3's +0x58 at 0x5C30EA): 256 × 256 (0x7FEE80 / 0x7FEE84), made by
  0x5BA1B0: CreateTexture 1 level, D3DUSAGE_RENDERTARGET, A8R8G8B8
  (0x7FEE78 = 21) → 0x8708A4, its surface 0x8708A8, D16 depth (0x7FEE7C =
  80) 0x8708AC.
* **What is drawn** (0x5CAAE0, 0x5CAE4B..0x5CAFFC, after the cube faces
  and before view 4 and the main view, when view 3 +8 is set):
  0x5BA3D0(record 3, 1) clears colour and depth to opaque black, the
  world 0x5D6AE0(view 3, 1), the cars 0x6316E0(view 3), then the
  particles (0x48BDF0 for lists 0x835860 / 0x8357E0), with
  g_LightGlowEnable the car glows 0x61AEA0(view 3) (headlight exponent
  ×0.75, sizes ×2.7) and 0x491860(view 3), and last the mirror's frame
  (0x5CAF4D..0x5CAFF9, effect 0x86EB48 fe.efx): with `RVM` (0x8707E8, hash
  0xDBD4, 128 × 64 DXT3: black, alpha ≈ 0.53 inside a trapezoid with
  rounded corners) the effect's colour mask (+0x31C) 0xF and
  0x5C27C0(RVM, 0xFFFFFFFF, SRCALPHA, INVSRCALPHA) — the glass's tint,
  about 0.47 of the image — then the colour mask 8 (alpha only) and
  0x5C27C0(RVM_MASK, 0xFFFFFFFF, ONE, ZERO) (0x8707EC, 0xCF29D0DF: alpha 1
  inside a slightly smaller trapezoid, 0 outside) — the mirror's shape in
  the target's alpha — and the mask back to 0xF. 0x5C27C0 draws a quad
  over the whole target ((−1, −1)…(1, 1), z 0, UV (0, 0) top left,
  triangle fan), stages 0–3 clamped, no depth test or write, alpha test
  off. 0x5B98C0 looks both up (and WHITE16X16_NOALPHA, 0x8707F0). No SKYDOME: 0x60F890 is called only at 0x5CAD50
  (cube faces) and 0x5CB237 (main view). Off the back buffer every
  non-car effect becomes worldnofog (0x5C5940), as in view 4.
  Scenery (0x5B9C30: base 0x1000 for view 3, then 0x4888A0): cull flags
  0x1000 | 0x10 | 0x20 | 0x40 | 0x08 forward / 0x04 reversed, so an
  instance needs 0x20 and 0x40; the base has 0x1800, so 0x5D6666 takes
  the 0x5D672A path: at least 32 pixels and LOD C (or B for instance flag
  0x0400). (**Corrected**: the 0x20 test at 0x5D666F, 16 pixels and LOD
  B, comes after the 0x1800 test and so is not reached by view 3.)
  Cars: 0x6316E0 also skips a car whose view mask (car +0x5B4, bit 3)
  is clear or that 0x60D500 culls, at most 99 cars. **Ported**: the world
  (LOW_LOD, 32 pixels, the cull flags above, worldnofog, no sky) and the
  cars (the rivals, the free-roam encounter racers, the traffic; not the
  player's) at their LOD for view 3, then the RVM / RVM_MASK passes.
  **Not ported (open)**: the particles and glows in view 3, the car shadows drawn by
  0x6316E0's callees, the car +0x5B4 mask (writer not traced; taken as
  set) and 0x60D500.
* **On screen** (0x5CBF50, the frame's tail, after the post passes and
  before the letterbox 0x5CBDD0 and the front end 0x5378D0; only with
  0x7FEE40 = 1, never written, and view 3 +8 set): a quad initialised by
  0x5BB1D0 (UVs (0, 0), (1, 0), (1, 1), (0, 1)) with corners (440, 15),
  (200, 15), (200, 95), (440, 95) on the 640 × 480 canvas, colour
  0xFFFFFFFF, effect 0x86EB48 (fe.efx) with its DiffuseMap set to
  0x8708A4, stage 0 ADDRESSU / V = 3 (CLAMP; back to WRAP after), alpha
  test off and blending SRCALPHA / INVSRCALPHA (0x5BCB80(0, 1),
  0x5BCBD0(1, 5, 6) into the BlendState array, 0x5BDFA0), drawn by
  0x5C5030(quad, 0, 0). u runs from 1 at the left edge to 0 at the right:
  the image is flipped across, a mirror. The 256 × 256 image fills a
  240 × 80 canvas box (3:1), so it is squeezed 3× vertically. Its alpha
  (RVM_MASK's) cuts the trapezoid out; fe.efx doubles the rgb (texture ×
  0xFF × 2), which brings the tinted image back to about 0.94 of the
  view. The clone: `render::rear_view::RearViewFrame` (the two passes),
  `rear_view::quad()` first in the frame's UI quads with colour 2.0
  (sampled clamped, `UiRenderer::set_texture_clamped`). **Inferred**:
  D3D9's half-pixel offsets of the two full-target quads (sub-texel).
* **Observed** (2026-10-31, wined3d, Quick Race circuit 1, Peugeot 206,
  vanilla, a private Wine copy): Far camera → C → Bumper before the green
  light: no mirror; at the green light the mirror appears, a trapezoid at
  about (417…860, 72…150) of 1280 × 960 (top edge wider), the street
  behind flipped across, the clear (no sky) and the near ground outside
  the mask. The clone's `--shot qrstart` with `NFSU2_VIEW=0
  NFSU2_FRAMES=1200` puts the same trapezoid on the same pixels with the
  same content (mean rgb over 400 × 60 inside: 72.9 / 71.2 / 59.5 in the
  original, 66.5 / 70.2 / 58.8 in the clone; the clone's car sits at
  another spot on the grid). **Open**: the mirror while paused (the
  clone shows none: the HUD's update does not run under the menus) and
  in free roam after the D-Day script (not captured).

## Texture animation and scrolling (verified)

* 0xB0300100 (stream file): flip-books. Each has a name, a hash, a frame
  count, fps and a frame-hash list. Water (`ANM_WATERA_`, 14 frames at 12 fps)
  works this way, and so do blinking signs and the star layer.
* Scroll parameters, read by 0x5BCC40 → 0x5B7D50/0x5B7D90 → 0x48B450 from
  texture entry +0x52 (kind), +0x58 (step/256), +0x5A/+0x5C (u/v
  speed/1024) and +0x5E/+0x60 (fixed offset). Kind 1:
  `frac(t·speed)`; kind 2: `frac(trunc(t/step)·speed)`; kind 3: fixed;
  frac(x) = `x − trunc(x)`. `t` is EA's 4000 Hz tick × 0.00025, i.e. seconds.

## Open

* Region textures (0x57FC5E): GLOBAL/GLOBALTEXTURESNA.BIN for region 0
  (US), else GLOBALTEXTURESROW.BIN; the region 0x7FA73C (0 US, 1 and 5–9
  by language, 2/10 KR, 3 JP, 4 TW, 11 CN, 12 TH; 0x571390) is 6 in this
  executable's data and never written, so ROW (a US install, docs/playing/region.md, takes NA), which holds the Best Buy
  billboards' texture. Six other solid texture references in L4RA
  (trailer, factory and power tower props) are in no file of the game.
  **Verified:** they get the default texture. GetTextureInfo (0x4901D0;
  hash, return-default, include-unloaded) returns 0x82DB08 for a hash it
  cannot find when return-default is set, and the pack loader (0x48FDE0)
  points 0x82DB08 at the texture with hash 0x66994F4A, GLOBALB's
  DEFAULTALPHA: 32×32 DXT3, all texels (0, 0, 0, 0), +0x56 = 2 (additive).
  A solid's texture list (+0x2C, +0x19 entries) holds the default for each
  miss; 0x493843 re-resolves those when packs load. So the six draw
  nothing visible. The clone binds DEFAULTALPHA, with its blend state, for
  every unresolved reference.
* The headlight pass (verified except where noted; `headlight_uniform`,
  `headlit` in world.wgsl). 0x870CC4 is g_CarHeadlightEnable (string
  0x7A385C; toggled at 0x49BBFD, 0 at 0x5BE892 and 1 at 0x5BE90C once the
  card level 0x87099C is above 1, so on at the clone's top settings) adds
  view 9 to 0x5B9C30's cull contexts (base 0; view 9 is active in the
  full-screen layout, 0x5C2385) and makes
  the main view's frame run 0x5D6D30 on it with the callback 0x488B90,
  which ORs 0x80 into byte +0x1B of every instance in view 9's frustum:
  instance flag 0x8000. 0x5D6AE0 turns that into draw flag 0x200 and
  0x5D6CD8 clears it once drawn. With 0x200 and 0x870CC4, 0x5C5940
  remaps the mesh effect by table 0x7FF090 (reflections on: world →
  worldheadlight, worldreflect → worldheadlightreflect) or 0x7FF0B4
  (reflections off: both → worldheadlight); without, 0x7FF0D8 applies
  when 0x870CD4 = 0 (worldheadlight and worldreflect → world). Effect
  order (descriptor table 0x7FF850): 0 world, 1 worldheadlight, 2
  worldreflect, 3 worldheadlightreflect, 4 worldbone, 5 car, 6 worldmin,
  7 worldnofog, 8 fe, 9 depth_world, 10 depth_worldheadlight, 11
  depth_worldreflect, 12 depth_worldheadlightreflect, 13 worldnodepth, 14
  worldheadlightnodepth, … worldheadlight.efx (IDI_WORLDHEADLIGHT_FX,
  ps_1_1): `rgb = sat(2·tex·(HeadlightTex·HeadlightClipTex·c0 +
  v0))`, alpha `tex.a·v0.a`; its vs puts HeadlightWVP·pos in T1 and the
  HeadlightClipMat depth into T2.x. At flush 0x5CF430 calls 0x5BDAB0 for
  effects with a HEADLIGHTWVP parameter (effect +0x59C): HeadlightWVP from
  view 9's camera (+0x80), HeadlightClipMat scaled by 0.005 (+0.5),
  HeadlightColor = 0x60F520 of the player (0x8900B4 +4) as bytes / 128.
  It is c0 of the ps_1_1 pixel shaders (worldheadlight, worldheadlightreflect),
  and D3D9 clamps a ps_1_x float constant to [-1, 1] (**verified** under
  Wine, 2026-11-01, apitrace of the finished-career save's garage exit: c0 was set to (1.17,
  1.80, 1.99) from render info +0x380 = 0xFFE696, the pool came out white,
  (231, 238, 224) before the post passes, the far wall at v 0.70 barely
  lit). The clone clamps each channel to 1 (`headlight_uniform`); it had
  used up to 2, a strong blue-white pool over the ground and the wall.
  * Camera. 0x5D2965 calls 0x5C2AF0(view 9, 0) every frame; in game
    (0x8654A4 = 6) it takes car 0 (0x5ED990(0) → 0x5F0E40): eye = its
    position (+0x50) + 0.75 × its up row (+0xB0, 0x7A27BC), direction =
    its forward row (+0x90) with 0.1 taken off z (0x7A27C8), normalised,
    up (0, 0, 1), through the look-at 0x5BA9B0 into 0x447280; near 1
    (0x7A27B4), far 100 (0x7FEE58), field of view 2.5 rad (0x7A27CC ×
    0x78423C, 26075 binary); it also zeroes the camera's focus +0xB4.
    0x5C7ED0 gives views 9 and 10 factor 1 across and 0.6 in place of
    h/w, so `x = 1/tan(1.25)`, `y = 1/tan(0.75)`. The clone takes the
    first headlight pool's matrix (the player's car). Which way the
    texture's v runs follows the look-at's −up (its view y points down,
    as for the chase camera, whose screen is upright with the same
    negative m11): row 0 of the texture is the low edge of the cone.
  * HeadlightWVP (0x5BDAB0) = World × view 9's view-projection (record
    +0x80, `D3DXMatrixMultiply` 0x63CEEB) × the identity with 0.5 in
    m30/m31 (0x80052C/0x800530), so `u = x/w + 0.5`, `v = y/w + 0.5`.
    That is the branch for the record in 0x870910 with byte +0x18 set
    (0x5BDB14-0x5BDB1E, verified); the other premultiplies the inverse of
    the current view (0x5BAFA0, 0x870848) instead of World, which the
    clone does not follow. +0x18 is not "the technique found is not the
    first" (verified): 0x5D1E30 walks the techniques in order
    (GetTechnique(i), 0x5D1F88), reads the int annotation "shader"
    (string 0x7A458C) of each through GetAnnotationByName/GetInt
    (0x5D1FAC-0x5D1FC6), and stops at the first one whose vtable call at
    0x5D2168 (slot 0xF8, ValidateTechnique by the ID3DXEffect order,
    **inferred**) returns S_OK (0x5D2170), after vendor and caps skips
    (0x5D1FDD-0x5D203E, device-dependent). 0x5D21A1-0x5D21A8 then sets
    +0x18 = (that annotation's int != 0); a technique without the
    annotation writes 0 (the local is zeroed at 0x5D1FB1). Static,
    verified by substring count: each of the 12 two-technique world
    blobs (IDI_WORLD_FX, IDI_WORLDHEADLIGHT_FX, ...; fx_2_0 0xFEFF0901
    RCDATA) contains the name "shader" once, and WORLDMIN (3 techniques)
    contains it not at all, so WORLDMIN always leaves +0x18 clear (code
    path verified). Which technique owns the name is **inferred**:
    technique 0, with technique 1 having none, from the blob layout.
    Open: the annotation's int value (**not decoded**; the layout is not
    documented in the repo), so whether +0x18 is set for the 12 world
    effects when technique 0 is found is unverified. Runtime: if
    technique 0 is skipped or fails validation, the next technique is
    found and +0x18 follows its annotation (**inferred**: none, so
    clear). The clone has no per-effect switch: headlight_uniform
    (crates/ob-render/src/render/mod.rs:109) takes only the pool, so it does not follow
    this branch (only that function was read).
  * HeadlightTex is the texture `HEADLIGHTS` (0x5BC700: 0x4901D0 with
    return-default; 128 × 64 BGRA, the beam), HeadlightClipTex is the
    64 × 1 A8R8G8B8 texture 0x5BC5E0 builds: texels 33..47 grey
    `trunc(255 · sqrt(1 − (i − 33)/15))`, the rest 0. The effect's vs
    (second technique; its first pair writes FocalRange depth) samples it
    at `u = 0.005 · w + 0.5` (its own c13 = (0.005, 0.5, 1, 0); the
    HeadlightClipMat parameter is unused by it), so the light runs from
    about 3 m to 48 m ahead. Sampler addressing is not read from the
    effect: the clone clamps (**inferred**; both textures' borders are 0).
  * Which draws. View 9's cull is the ordinary 0x5D6580 on its own
    context, so it needs size 17 at view 9's scale (`256/tan(1.25)` ≈
    85) from the projector: the clone tests that per instance in the
    vertex shader (`lit`). Its flags (0x10 | 0x08/0x04) cull nothing the
    main view draws, and the frustum test is left to the projection
    (the beam texture and the clip texture are 0 outside it). Additive
    textures on worldheadlight still go to worldnofog (0x5C5AE5: effect
    0 or 1 with +0x56 = 2), so they get no light. The clone does not
    move lit draws into effect 1's / 3's buckets: they keep world's /
    worldreflect's place (**inferred**, see Draw order).
* The depth pass is depth of field: 0x8707CD follows 0x8707CC (0x5D2AE6),
  which 0x5D29E1 / 0x5D2A25 set from two focus values. While it is set
  and the target is the back buffer, 0x5C5940 moves reflective meshes to
  depth_world… (effects 9–12 by the effect index) and other draws to
  worldnodepth / worldheadlightnodepth (13 / 14), opaque reflective ones
  to 11 / 12. When it runs (verified): 0x5D299E, behind the option
  0x870CF0, reads view 1's camera focus +0xB4 / +0xB8 and sets 0x8707CC
  only when either is non-zero. The race chase camera (0x453570) writes
  15 / 65 there (0x78707C / 0x787078) only while its +0x53 is 0, else 0
  (0x453925..0x45396F); +0x53 is the constructor's sixth argument
  (0x4516B4), and all three 0x5F1FE0 sites (0x5F2268, 0x5F2AD1,
  0x5F2E74) pass 1 (left on the stack from the 0x5EB410 call, which
  does not pop it). So no depth of field runs behind the player's car,
  and the clone, which only has that camera in play, rightly draws none.
  Other camera movers write the focus (stores at 0x44B…–0x44F…, not
  identified: **open**, e.g. replay cameras). The showcase camera's
  (0x44EF60: mover +0x54 / +0x58) is 0 in every MAGAZINES_SHOWCASE camera.
* Cars draw with car.efx (see Cars); the 115 world meshes with flag 0x80
  (shiny props) still use world. Traced so far: 0x5C5940 hands every draw
  its light material (the solid's table, +0x3C, at the material's +0x20
  index, else 0x48AC50's default) and, for the car effect (type 5)
  without one passed in, the light context at the view's +0x54
  (0x5C5B83); the flush uploads them through 0x5BD150 / 0x5BD4B0. Open:
  what fills view +0x54 (no direct store at 0x832DE0 + n·0x70 + 0x54;
  the view constructor 0x491940 zeroes it), and the vertex format of
  those meshes.
* Draw order (verified; `WorldRenderer::new`). Nothing is depth-sorted.
  0x5C5940 hands each material to 0x5CEB40, which files it under its
  texture in the list for (the 5th argument's +0x51, final effect index) — heads at
  0x870D30 + (order·23 + effect)·8, or at 0x8712F0 when the 5th argument's
  +0x54 is set — textures in first-use order, draws in submission order.
  0x5CF430 flushes the lists in memory order: the 0x870D30 set before the
  0x8712F0 set, rendering order 0..7, effect 0..22. Blend takes no part:
  an additive texture of order 0 draws among the opaque ones. The clone
  sorts by (sky, +0x54, +0x51, effect index) with a stable sort, so the
  clone keeps construction order within a list: each draw is one material
  of one section, with an instanced range fixed at construction, and there
  is no per-view cell walk (the scenery cull runs per instance in the
  shader). Within a list the game draws texture groups in creation order, each
  texture's draws in walk order (verified): 0x5D69C0 walks the cell tree
  depth-first with a LIFO stack;
  each popped cell that passes its test calls 0x5D6580 for its instance
  entries in list order and pushes its child cells in list order, so the
  last child is walked first. 0x5D6580 appends each instance it keeps to
  the view's buffer in call order; 0x5CAAE0 runs four 0x5D6AE0 passes (filters
  0x8, 0 from ebx, 0x1 and 0x20), with 0x5CF430 flushes between them in code
  order; each pass hands its passing entries to 0x5C5940 in buffer order. 0x5CEB40 opens a
  new texture group only when the texture has none open (its +0x1c, reset
  at flush) or its open group has another effect, so each bucket holds
  texture groups in creation order (walk A, B, A draws A, A, B). Neither 0x5CEB40 nor 0x5CF430 sorts, and the
  bucket does not read blend, so overlapping alpha-blended draws in one
  bucket composite in that order. Additive draws commute only with other
  additive draws: an additive draw and an overlapping alpha-blended draw give
  different results in either order. The clone's order therefore can change
  the output of overlapping non-additive
  draws; matching it needs a per-view CPU cell walk and per-view instance
  order, which is not done here; the reflection and cube views, where everything
  becomes worldnofog, keep the main view's order (unverified here: in the game they
  would merge into effect 7's lists), and headlit draws (effects 1 and 3 in
  game) keep world's and worldreflect's place (unverified here).

## Renderer performance: what the clone may skip, merge or reorder (2026-10-29)

The performance pass changed how the frame is recorded, never what it
draws: the shots and the benchmark's frames are byte-identical before and
after (the proof is in README.md, **Performance**). The rules it relies on:

* **A draw that puts no pixel on screen may be left out.** `PassCull`
  (render/mod.rs) drops a group of instances (the draws of one solid in
  one section with one set of flags and LODs, which share their
  instances) only when it is wholly outside a plane of the view, or when
  the vertex shader's scenery cull (`cull_size` in world.wgsl, 0x5D6580 /
  0x5D64E0) drops every one of its instances — tested on the CPU with a
  margin, so an instance near the threshold is always left to the shader.
  An instance with the hidden flag (0x10, in every view's cull flags) is
  dropped by every view.
* **Bounds.** An instance's sphere is its solid's sphere moved by its
  matrix and scaled by the matrix's largest singular value, not its
  longest column: 1,514 of Bayview's 48,317 placements have a 3×3 that is
  not a scaled rotation (up to 1.24× the longest column). The road
  reflection (view 4) flips each prop about its own origin (m22 negated,
  0x5D67B4), so its groups have bounds of their own (`mirrored_bounds`);
  it is culled by the main view's planes (it shares view 1's camera and
  projection, 0x5C30F6 / 0x5C7ED0).
* **The cube faces** (views 0x10–0x15, near 0.5 and far 64: 0x7FEE88 /
  0x7FEE8C) clip at their far plane too; together they see the cube of
  ±64 m about the eye in the camera's axes, so a frame's candidates are
  found once and each face tests only those. The face schedule is the
  game's (0x5C397B, table 0x7FEEA0, periods 0x7FEE90): at the top update
  setting (rate 3, row 12) all six faces every frame, so the clone keeps
  six; rates 1 and 2 already render fewer (`display::env_faces`).
* **The world shaders' height discard was never taken.** `clip.x` is
  f32::MIN in every pass (main, reflection, cube faces, garage), so
  `world_z < clip.x` never held; with it gone the shaders without the
  alpha test have no discard and keep the early depth test on every GPU.
  worldheadlight's projector texture is read only where its light falls.
* **Independent passes may change places.** The cube faces now go to the
  GPU before the motion blur: they share no target or input. The frame's
  command buffers are recorded on several threads but submitted in the
  recording order, after every `write_buffer` their passes read.
* **Post passes merged** (each exact on any GPU):
  * motion blur's StretchRect copy of the back buffer (0x5CC470) is not
    made: the first blur reads the scene itself, the same 8-bit texels;
  * the streak buffer's decay (ZERO / BLENDFACTOR, 0x5CBB6A) multiplies
    its source by zero, so its pass reads no texture (`fs_zero`), and the
    first frame's clear is a clear;
  * a one-tap copy followed by an ONE / ONE add of the streak buffer (into
    the back buffer, or screen copy 1 when only the streaks are on) is one
    pass, `fs_add`: the sum of two 8-bit values, saturated, is what the
    blend unit makes of them;
  * motion blur keeps a uniform slot of its own, so the end-of-frame
    passes' uniform writes never touch what the scene reads.
  Not merged: passes whose 8-bit intermediate the next pass reads (the
  bleach bypass's three, overbright's half-size chain, the blur rounds):
  folding them into one shader would need the conversion to 8 bits
  emulated, which no API pins down to the last bit at a tie.
* **Car draws.** A part's matrix and light context are made once for all
  its draws, and the draws of parts the car's LOD leaves out get no
  values (the shader never reads them); every car renderer of a device
  shares car.efx's shader, layouts and pipelines (`car::CarShared`).

### Allocator and PGO (clone only, 2026-10-31, decisions155)

Nothing here is the original's: SPEED2.EXE allocates from its own pools
over the MSVC heap (e.g. 0x575630 `jmp _malloc`, the pool allocator
0x440D40). These only change how fast the clone's loaders get memory and
how its code is laid out; no frame, sound or save byte changes.

* **Allocator** (`src/allocator.rs`, README "Allocator"): mimalloc (fast)
  by default; the system allocator when the machine has under 10 GiB of
  RAM or under 4 GiB available at start; `NFSU2_ALLOC=fast|lean|system`
  forces one. Picked once at the first allocation and kept.
* **PGO** (`scripts/pgo.sh`, README "PGO"): an optional build that
  trains on the benches. The normal `cargo build` does not use it.

## Pop-in: what appears when, original vs clone (2026-11-01)

Issue: pop-in still looked harsh. Compared at full speed: the
original (vanilla, a private Wine copy, wined3d-Vulkan on the GPU —
DXVK hit `Command submission failed: -13` at the profile save twice that
morning, as other copies did — 1280 × 960, free roam after the career
start) driven along six long straights (the car teleported with speed,
Up held 8 s, starts from the maintainer's probe `lod_straights`),
30 fps frames (ffmpeg x11grab, wall-clock pts) and view 1's camera
(0x8760E0: eye +0x40, forward +0x50, matrix +0x00, angle +0xC4) polled at
~85 Hz; the clone rendered the same eye, orientation and angle per frame
(`NFSU2_BENCH_CAMTRACK`, `TrackPose` in bin/game.rs; `lod-tmp/cl.sh`).
Side by sides (kept outside the repository): `cmp_r1.png`, `r1_57.png`,
`r1_85.png`, `r6_76.png` (original top / left).

* **Which sections are drawn** (**verified**, per frame, per view):
  0x5E9A40 fills each cull context from its view and calls 0x5E99D0, which
  takes the list 0x5E96C0 gives and walks each listed, loaded section's
  box tree (0x5D69C0: 0x24-byte nodes, the frustum test 0x5BC310) down to
  the instance cull 0x5D6580. 0x5E96C0 = the Z sections 2600–2699 that are
  loaded, then the list of 0x5E00E0(eye) — each entry only when its pack
  is loaded (0x883EC0[s] and its +4) — at most 0x80. 0x5E00E0 walks the
  visible lists (MRU order) for the one whose drivable outline holds the
  eye (box 0x43D410, polygon 0x5D6DE0), moving it to the head; else
  0x5DB780 gives the nearest drivable outline (0x5D70E0) and its list is
  taken only when that distance is under 0.1 m; else no list, and
  0x5E96C0 takes every loaded scenery pack (0x88EAD4, filled by the
  loader 0x5DF96C) numbered n % 100 < 10. So the drawn set changes the
  frame the eye crosses an outline, as in the clone (`visible_sections_at`
  of the camera's position). L4RA's 255 listed outlines never overlap
  (5 m grid: 0 points in two), so the MRU order changes nothing. On the
  six routes the eye was inside an outline in all 1,446 frames (the
  clone's own fallback, the nearest box centre, never ran); the section
  switches fell on the same frames in both games (r3 32, r4 154, r5 6,
  r6 76 / 231). **Not ported**: the loaded test (the clone holds every
  section; the original's streamer — 0x5E5310 / 0x5E7B00 and the
  predictive load, not traced — can leave a listed section out until it
  arrives, e.g. after a teleport) and the no-list fallback.
* **Classes and distances** (k = view +0x0C = 355.5 at the chase
  camera's 71.5°, independent of the resolution):
  * World sections / geometry: whole sections appear on a list change,
    at whatever distance the data puts them (r6 frame 76: sections 726 /
    728 of the far district, 500 m and more, behind the towers — no
    visible change in either game). Same frames, same lists.
  * World LOD (high / low detail): the lists themselves pick a place's
    copies (base n, detail n + limit, panorama x90–x99). Inside a
    section the main view picks LOD A or B per instance (0x5D66AD…:
    screen size / max(+0x98, 6) ≥ 4 and World Detail 2–3, **open**:
    +0x0C object fields); the clone keeps LOD A, so it has none of those
    swaps (less pop than the original, not more; left so — porting it
    adds pops and needs the open fields).
  * Props, lights, trees, signs: the 17 px size cull, size =
    ftol(k · r / (d − r)), r the instance's cull radius: drawn from d ≈
    r (1 + 355.5 / 17) ≈ 21.9 r (a 1 m-radius sign at ~22 m, a 5 m lamp
    at ~110 m). The clone's per-instance gate (`vs_common`, `cull_size`)
    is the same, and the lamps' flare pools use it too.
  * Car LOD: as **Car LOD** above (traffic B ~30 m, C ~56 m, D ~110 m;
    racers B ~30 m, C ~39 m, D ~56 m; no car past size 4, ~270 m).
  * Shadows / reflections: the car shadow is drawn with the car (its LOD
    gates); the road reflection uses 32 px and LOD C (view 4). No
    separate pop seen on the routes.
  * Fog / far plane: the art's fog; no far-plane edge seen.
* **Result**: no non-native pop found — matched-pose frames hold the same
  objects; the clone's pops are the original's (same frames, same
  rules). Seen in passing (not a pop): the bench's free roam sky is the
  dusk one while the original's teleported free roam was night; a
  brighter sky makes the same pops easier to see. So the options are
  the Graphics page's Detail Distance and Car Detail Distance (above).
* **2026-11-06**: World Detail Ultra / Extreme's pop-in work (scenery
  and cars drawn to 1.5 output pixels, 1 s section dissolves, traffic ring
  fade, lamp light / shadow fades), the pop meter (`NFSU2_POP_STATS`) and
  the pixel oracle (`NFSU2_POP_ORACLE`): docs/notes/graphics/pop-in.md. Clone deviations;
  the native rules above are unchanged.
