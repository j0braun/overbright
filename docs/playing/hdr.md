# HDR Output

On an HDR display, HDR Output lets headlights, neon, glows and the sky's highlights shine brighter than
normal white, while the rest of the picture keeps the original game's colour grade. It is optional and off
until you turn it on. The original game only has normal (SDR) output; what Overbright keeps from it is the
grade.

## Turning it on

Open **Options → Display → Enhancements → Advanced → Display**. This page has the HDR rows and a calibration
pattern.

| Row | Values | Default |
|---|---|---|
| HDR Output | Off / On | Off (normal SDR output, as the original (choice: HDR is an extra, SDR is the native output)) |
| Peak Brightness | Auto / 400…10000 nits | Auto |
| Paper White | Auto / 80…400 nits | Auto |
| SDR Gamma | 2.2 / sRGB / 2.4 | 2.2 |

The rows are greyed out ("Not Supported") when your display path offers no HDR signal. The graphics presets
leave them alone, as display settings, like the field of view. In options.txt they are `enh.hdr_output`,
`enh.hdr_peak`, `enh.hdr_paper` and `enh.hdr_gamma`, written only when not at the default.

## Calibrating

While the Display page is open, a test pattern fills the page's empty rows. From left to right:

1. A 10,000-nit box holding a square at the **Peak Brightness** value. Raise Peak Brightness until the square
   disappears into the box.
2. Paper white beside half of it: set **Paper White** so that this looks like normal white to you.
3. Black steps of SDR codes 0, 1, 2, 3, 4, 6, 8 and 12 through the chosen gamma.
4. Alternating black and white lines beside a solid patch of the brightness they should average to under the
   chosen gamma. Pick the **SDR Gamma** where they match.

**Auto** reads the display's peak and its SDR white level from Windows (DXGI). Elsewhere Auto is 1000 nits
peak and 203 nits paper white (the BT.2408 reference white).

Keys, pad and mouse work on the rows as on any page; the pattern takes no input. It scales and moves with the
menu at every size and aspect ratio, and never covers a row, the help text or the scroll bar. A page with less
than one row's height free shows no pattern.

## Where it works

| Platform / backend | HDR10 PQ (`Rgb10a2Unorm`, BT2100_PQ) | scRGB (`Rgba16Float`, EXTENDED_SRGB_LINEAR) | How known |
|---|---|---|---|
| Linux, Vulkan (RADV 26.2.5), GNOME 51 Wayland (the reference desktop) | **offered by the surface** (HDR10_ST2084 on A2B10G10R10 / A2R10G10B10 and 8 others) | not offered (BT709_LINEAR is not extended) | `vulkaninfo` surface formats |
| Linux, Vulkan, KDE Plasma 6 Wayland | expected (Mesa WSI + wp_color_management_v1) | expected | inferred, not run |
| Linux, X11 / Xwayland | not offered (no colour management on X) | no | inferred |
| Windows, DX12 | yes (on `Rgb10a2Unorm`) | yes | wgpu 30 docs; not run |
| Windows, Vulkan | depends on the driver (AMD and NVIDIA expose HDR10_ST2084) | yes (EXTENDED_SRGB_LINEAR) | wgpu 30 docs; not run |
| macOS, Metal | yes | yes (EDR) | wgpu 30 docs; not run |

Overbright uses the surface's own list of capabilities (`hdr_output::choose`). PQ is preferred where both
signals are offered (`NFSU2_HDR_SIGNAL=scrgb` prefers scRGB).

**Not yet seen on an HDR display.** The reference desktop's monitors (DP-1 Dell S2721D, DP-3 Dell S2725QS) carry
no HDR static metadata block in their EDID: they are SDR panels, and GNOME's HDR mode is off. The compositor
still offers the PQ colour space, so configuring the swap chain would likely be accepted, but it was not tried
(test builds run without a window). What was checked without a display: the transform's maths (unit tests),
the GPU pass against the CPU reference on both signals, the whole in-game path through `NFSU2_HDR_FAKE=pq|scrgb`
(the same post passes and transform, into an HDR texture instead of a swap chain), and the grade table below.

## Cost

`--bench explore 600` on the reference desktop GPU, GPU busy ms per frame (a shared machine, so indicative):

| Size | Original | High Precision Post Half | HDR Output PQ | HDR Output scRGB |
|---|---|---|---|---|
| 1920 × 1080 | 0.62 | 1.01 | 1.00 | 1.06 |
| 3840 × 2160 | 2.31 | 4.44 | 4.71 | 4.82 |

Over Original: +2.40 ms at 4K (the row's cost tag, `HDR_OUTPUT_COST_4K`) and +0.38 ms at 1080p. Over High
Precision Post alone only +0.27 ms (grade table 0.94 against the float bleach 0.92, the two-target last pass
+0.08, the display transform 0.15). Breakdown: docs/evidence/hdr/README.md.

## How it works

The rest of this page is for developers.

### The grade as a LUT (`render::postfx`, postfx.wgsl `grade_lut`)

The original's per-pixel grade is screenfilter.efx technique 0: a bleach bypass in three ps_1_4 passes
0x3600 / 0x2D70 / 0x2484 run by 0x5C68E0, with the gain map 0x5BBE90, the ramp 0x5BC050 and TINTCOLOR /
FILTERBLEND from 0x5CB8A9… (traced in [formats.md](../formats.md) "Screen filters"). Overbright, streaks, rain and
lens drops work on neighbouring pixels and stay passes; only the bleach bypass maps a colour to a colour, so
only it becomes the table (LUT).

- **Capture:** Overbright's ports of the original passes (`fs_bleach0/1/2`, with 8-bit targets between them,
  as in the original; traced: docs/formats/world.md, Screen filters) run on an **identity strip** of 32³ colours (`identity_strip`, texel (r + 32·b, g)) whose
  lattice sits on 8-bit codes k(i) = round(i·255/31) (0, 8, 16, 25, …, 255). So the LUT is the original grade
  exactly at every lattice point. It is rebuilt on the GPU (three passes over 1024 × 32 texels) whenever the
  frame's grade values change (bleach amount, TINTCOLOR.w, TINTCOLOR.rgb; the screen tint moves while
  driving).
- **Use:** HDR Output's grade pass `fs_grade_lut` (trilinear between lattice codes, the cell found on the codes,
  not on i/31).
- **Export:** a developer probe (`grade_lut cube OUT.cube [R G B W AMOUNT]`, not in the public repository)
  writes a standard `.cube` (32³; its domain is uniform, and the lattice codes are within 0.5/255 of i/31) and
  the exact strip as PNG.

**Checked against the original** (Wine, apitrace replays; `grade_lut verify`, `grade_lut wine`). For each frame:
the back buffer after its last scene draw (the input that the StretchRect 0x5CC8C0 copies into screen copy 0)
and the render target after the bleach's third draw, from `d3dretrace --snapshot`:

| Frame (trace, calls) | TINTCOLOR (.rgb scaled, .w) | Amount (fitted) | Ported passes vs original | 32³ LUT (trilinear) vs original |
|---|---|---|---|---|
| free roam, fr.trace frame 13971: 139478949 → 139479441 | 0.16593, 0.13859, 0.08271, 0.12 | 0.760 | **99.9987 % exact, max ±1** (182 517 colours) | 35.5 % exact, 99.09 % ±1, max ±2 |
| free roam night, dark.trace frame 15478: 164460096 → 164460588 | 0.16960, 0.12138, 0.10891, 0.125 | 0.848 | **99.984 % exact, max ±1** (127 971 colours) | 32.4 % exact, 99.35 % ±1, max ±3 |

FILTERBLEND (the gain map's amount) is not in the shader constants; it was fitted (0.2 to 1 in steps of 0.001,
the value with most exact pixels). 0.760 = lerp(0.2, 1, 0.70) and 0.848 = lerp(0.2, 1, 0.81): plausible screen
tint amounts, not read from memory (**inferred**).

On every 24-bit colour (the LUT against the ported passes; the passes' own self-check already equals them to
the fused pass):

| Constants | Lattice codes | Trilinear | Tetrahedral |
|---|---|---|---|
| menus (0.84, w 0.2, tint 0.5) | 32 768 / 32 768 exact | 34.48 % exact, 98.99 % ±1, max ±3, mean 0.31 | 32.94 %, 98.80 %, ±3 |
| game, tinted (0.76, w 0.35) | exact | 38.57 %, 99.72 %, max ±2, mean 0.27 | 36.38 %, 99.60 %, ±3 |
| game, no tint record (0.2, w 0.2) | exact | 38.54 %, 98.54 %, max ±3, mean 0.28 | 36.81 %, 98.43 %, ±3 |

**Not byte-identical.** A 32³ interpolated table cannot reproduce the original grade byte for byte. The gain
map is a 32 × 32 texture read bilinearly at its own texel centres ((j + 0.5)/32, not on the table's lattice),
and each of the three passes rounds to 8 bits, so between lattice codes the original function has kinks and
steps that no 32³ interpolation follows (measured: ±1–3 steps, 1–1.5 % of colours off by more than 1). Byte
identity holds where it matters: with HDR Output Off none of this runs (the original passes; identical shots
below). A 256³ table (64 MiB, every code a lattice point) would be exact by construction (**inferred**, not
built). With HDR Output on, the chain is High Precision Post's float chain anyway (no 8-bit rounding between
passes), so its SDR range is within those ±1–3 steps of the original, not identical.

### The display transform (`render::hdr_output`, hdr_output.wgsl)

The frame is made as before (scene, post passes, UI) into an SDR texture in the renderers' format; a last pass
writes the swap chain.

- An SDR value v (the frame, UI included) becomes light = EOTF(v) · paper white (EOTF: gamma 2.2 by default,
  the sRGB curve, or 2.4). Every pixel up to SDR white is exactly the SDR image at paper white, so **midtones
  keep the grade**.
- **Highlights only:** with HDR Output on, the post chain keeps the scene unclipped (High Precision Post's
  half-float chain, forced on at least Half-Size Glow). The grade is applied through the LUT to the hue (t / m,
  m the largest channel) and scaled back by m, and the last pass writes a second target, the unclipped image
  (`fs_out_hdr*`, MRT). Where that is above 1 (additive glows, headlights, neon, sun and sky highlights, streaks
  over the graded image) and the frame still shows it (|frame − clip(unclipped)| < 3–8 steps, so the HUD and
  menus drawn over it stay SDR): light = EOTF(t / m) · shoulder(m^γ) · paper, with
  shoulder(r) = 1 + (h − 1)(1 − e^−(r−1)/(h−1)) and h = peak / paper: slope 1 at white, never above the peak.
- Encoding: **HDR10** is BT.709 → BT.2020 (the BT.2087 matrix) and ST 2084 PQ, on `Rgb10a2Unorm` +
  `SurfaceColorSpace::Bt2100Pq`. **scRGB** is linear BT.709 with 1.0 = 80 nits, on `Rgba16Float` +
  `ExtendedSrgbLinear`.
- The streak buffer still saturates at 1, as in the original (traced: docs/formats/world.md, Screen filters) (its steady state would blow out otherwise). The
  extra brightness comes from the scene's additive draws and from the graded image + streaks.

### Calibration

The pattern is drawn by the transform (`hdr_output.wgsl` `pattern`). `OptionsScreen::calibration_area` takes the
list's empty row slots, from a quarter row under the last row shown to a quarter row above the help text, and
from the names' left edge to the right arrows' right edge, on the menu's 640 × 480 canvas. The frame maps it to
the output like the rows (`hdr_output::pattern_rect`). The resolution matrix's `hdr` check covers every size,
aspect ratio and Aspect Ratio option. Auto's values come from `Surface::display_hdr_info` on Windows.

### The SDR swap chain's format (`display::sdr_surface_format`)

Every renderer draws in the window's format (`WindowState::config.format`) and writes display-encoded values,
so the SDR swap chain must be a non-sRGB format that the compositor shows as sRGB.

- The surface lists formats in the driver's order, and a colour-managed Wayland desktop puts 16-bit formats
  before the 8-bit ones. RADV 26.2.5 under GNOME 51 lists `Bgra8UnormSrgb, Rgba8UnormSrgb, Rgba16Float,
  Rgba16Unorm, Rgb10a2Unorm, Bgra8Unorm, Rgba8Unorm` (a developer probe on a headless mutter, not in the public
  repository).
- Taking the first non-sRGB format gave a 16-bit float swap chain, and every read-back of a target in that
  format (photo mode's picture, the showcase and magazine-cover pictures) assumed 4 bytes per pixel and failed
  wgpu's validation (a crash).
- The game now takes 8-bit BGRA, then 8-bit RGBA, then `Rgb10a2Unorm`, then `Rgba16Float` (asked for in the
  sRGB colour space: `Auto` would make a 16-bit float swap chain extended linear sRGB), then any other non-sRGB
  format, then the first one.
- Read-backs of a window-format target go through `render::read_back_rgba8`, which converts each of those
  formats to RGBA8 (an 8-bit value written into any of them reads back unchanged) and returns an error for any
  other format.
- The log file's `display:` line names the adapter, the chosen format, colour space and present mode, and the
  offered list.
- HDR Output configures its own format and colour space over this one (`WindowState::surface_config`). The
  loading screens then draw into the SDR frame texture and go through the display transform like every other
  frame.

### Without a window

`NFSU2_HDR_FAKE=pq|scrgb NFSU2_SHOT_ENH="enh.hdr_output 1" game --shot <name> out.png` writes `out.png` (the
SDR frame), `out.hdr.png` (a 16-bit PNG of the PQ signal, BT.2020; scRGB is written as PQ) and
`out.hdr-preview.png` (tone-mapped: light up to 0.8 × paper white as SDR, the rest compressed to white at the
peak), and prints the brightest pixel and the share above paper white. `--shot options_graphics_display` is
the calibration page (`settings_display`; from the pause menu `pause_settings_display`).
`NFSU2_HDR_PATTERN=1` forces the pattern on other screens, in a fixed band across the lower middle
(`PATTERN_FALLBACK`).

### With HDR Output Off, the picture is the original's

md5 of `--shot` against the build before HDR Output: main, explore, intro, qrcars, race, qrstart, drift, drag,
garage_neon and options_graphics_effects identical. career differs by 158 px, and two runs of the earlier
build differ from each other by 53 px (the menu turntable is not deterministic). The new Display group adds
one row to the Graphics page.

### Tests

`render::hdr_output::tests` (PQ points and round trip, BT.2020 matrix white and luminance, shoulder continuity
and bounds, midtones = SDR at paper white, highlights up to the peak with their hue, UI over a highlight stays
SDR, encode / decode); `gpu_tests::display_transform_matches_the_cpu_reference` (ignored, needs a GPU; passes
on the reference desktop GPU); `postfx::tests::grade_lattice_sits_on_8bit_codes`,
`grade_lut_matches_the_native_passes` (ignored, GPU: lattice exact, max ±3, > 98 % within ±1);
`qol::tests::hdr_output_values`; `options::tests::hdr_rows_are_greyed_without_an_hdr_signal`,
`hdr_calibration_tiers_round_trip`, `graphics_page_steps_and_restores` (the Display group).

## Open

- Not seen on an HDR display. Windows and macOS not run.
- No HDR metadata (MaxCLL, mastering) is sent: wgpu 30 has no API for it.
- The 32³ table is within ±3 steps of the original grade, not exact; a 256³ exact table is not built.
- The streak buffer keeps the original's saturation; neon and glow sprites get extra brightness only through
  the scene's own additive excess and the final add.
- Paper White and Peak Brightness Auto read the display only on Windows (DXGI).
- Seen twice in about 30 `--bench explore` runs of this build on a shared machine (once with HDR Output off,
  once on, both while other GPU and build work ran): the bench stopped before its "load" line, every thread in
  a futex wait, no CPU use. Not reproduced in 15 reruns (5 off, 5 HDR + TAA, 5 plain) nor in 9 runs of the
  earlier build; HDR Output does not touch loading. Cause open.
