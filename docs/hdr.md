# HDR Output with the original's grade (clone deviation, opt-in)

Graphics → **Display** (a new group page, the calibration screen): **HDR
Output** Off (default, SDR, native) / On, **Peak Brightness** Auto /
400…10000 nits, **Paper White** Auto / 80…400 nits, **SDR Gamma** 2.2 /
sRGB / 2.4. options.txt `enh.hdr_output`, `enh.hdr_peak`, `enh.hdr_paper`,
`enh.hdr_gamma` (written only when not default). The presets leave them
alone (display settings, like FOV). (Research
report item 1). Nothing here exists in SPEED2.EXE (D3D9, A8R8G8B8 back
buffer, SDR only); what is native is the grade it keeps.

## The grade as a LUT (`render::postfx`, postfx.wgsl `grade_lut`)
The original's per-pixel grade is screenfilter.efx technique 0 (bleach
bypass, three ps_1_4 passes 0x3600 / 0x2D70 / 0x2484 run by 0x5C68E0, gain
map 0x5BBE90, ramp 0x5BC050, TINTCOLOR / FILTERBLEND from 0x5CB8A9…; the
trace is docs/formats.md "Screen filters"). Overbright, streaks, rain and
lens drops are spatial and stay passes; only the bleach bypass is a
colour → colour function, so only it becomes the LUT.

* **Capture**: the clone's ported native passes (`fs_bleach0/1/2`, 8-bit
  targets between them as native) run on an **identity strip** of 32³
  colours (`identity_strip`, texel (r + 32·b, g)) whose lattice sits on
  8-bit codes k(i) = round(i·255/31) (0, 8, 16, 25, …, 255), so the LUT is
  the native grade *exactly* at every lattice point. Rebuilt on the GPU
  (three passes over 1024 × 32 texels) whenever the frame's grade values
  change (bleach amount, TINTCOLOR.w, TINTCOLOR.rgb — the screen tint moves
  while driving).
* **Use**: HDR Output's grade pass `fs_grade_lut` (trilinear between
  lattice codes, the cell found on the codes, not on i/31).
* **Export**: a private developer probe (`grade_lut cube OUT.cube [R G B W AMOUNT]`) writes
  a standard `.cube` (32³; its domain is uniform, the lattice codes are
  within 0.5/255 of i/31) and the exact strip as PNG.

### Verification (reference desktop GPU, RADV; `grade_lut verify`, `grade_lut wine`)
Against the original (Wine, apitrace replays, private copy
a private Wine copy, display :220; traces kept outside the repository). Each frame's
back buffer after its last scene draw (the input the StretchRect 0x5CC8C0
copies into screen copy 0) and the render target after the bleach's p2
draw, snapshots of `d3dretrace --snapshot`:

| frame (trace, calls) | TINTCOLOR (.rgb scaled, .w) | amount (fitted) | ported passes vs original | 32³ LUT (trilinear) vs original |
|---|---|---|---|---|
| free roam, fr.trace frame 13971: 139478949 → 139479441 | 0.16593, 0.13859, 0.08271, 0.12 | 0.760 | **99.9987 % exact, max ±1** (182 517 colours) | 35.5 % exact, 99.09 % ±1, max ±2 |
| free roam night, dark.trace frame 15478: 164460096 → 164460588 | 0.16960, 0.12138, 0.10891, 0.125 | 0.848 | **99.984 % exact, max ±1** (127 971 colours) | 32.4 % exact, 99.35 % ±1, max ±3 |

FILTERBLEND (the gain map's amount) is not in the shader constants; it was
fitted (0.2…1 by 0.001, the value with most exact pixels). 0.760 = lerp(0.2,
1, 0.70) and 0.848 = lerp(0.2, 1, 0.81): plausible screen-tint amounts, not
read from memory (**inferred**).

On every 24-bit colour (the LUT against the ported passes; the passes'
own self-check already equals them to the fused pass):

| constants | lattice codes | trilinear | tetrahedral |
|---|---|---|---|
| front end (0.84, w 0.2, tint 0.5) | 32 768 / 32 768 exact | 34.48 % exact, 98.99 % ±1, max ±3, mean 0.31 | 32.94 %, 98.80 %, ±3 |
| game, tinted (0.76, w 0.35) | exact | 38.57 %, 99.72 %, max ±2, mean 0.27 | 36.38 %, 99.60 %, ±3 |
| game, no tint record (0.2, w 0.2) | exact | 38.54 %, 98.54 %, max ±3, mean 0.28 | 36.81 %, 98.43 %, ±3 |

**Not byte-identical.** A 32³ interpolated LUT cannot reproduce the native
grade byte for byte: the gain map is a 32 × 32 texture read bilinearly at
its own texel centres ((j + 0.5)/32, not on the LUT's lattice) and each of
the three passes rounds to 8 bits, so between lattice codes the native
function has kinks and steps no 32³ interpolation follows (measured: ±1–3
steps, 1–1.5 % of colours off by more than 1). Byte identity holds where
it matters: with HDR Output Off nothing of this runs (the native passes,
md5-identical shots below). A 256³ table (64 MiB, every code a lattice
point) would be exact by construction (**inferred**, not built). With HDR
Output on the chain is High Precision Post's float chain anyway (no 8-bit
rounding between passes), so its SDR range is within those ±1–3 steps of
native, not identical.

## The display transform (`render::hdr_output`, hdr_output.wgsl)
The frame is made as before (scene, post passes, UI) into an SDR texture of
the renderers' format; a last pass writes the swap chain:

* SDR value v (the frame, UI included) → light = EOTF(v) · paper white
  (EOTF: gamma 2.2 default, sRGB piecewise, or 2.4). Every pixel up to SDR
  white is exactly the SDR image at paper white: **midtones keep the
  grade**.
* **Highlights only**: with HDR Output the post chain keeps the scene
  unclipped (High Precision Post's half-float chain, forced on at least
  Half-Size Glow), the grade applied through the LUT to the hue (t / m, m
  the max channel) and scaled back by m, and the last pass writes a second
  target, the unclipped image (`fs_out_hdr*`, MRT). Where it is above 1
  (additive glows, headlights, neon, sun/sky highlights, streaks over the
  graded image) and the frame still shows it (|frame − clip(unclipped)| <
  3–8 steps, so the HUD and menus drawn over it stay SDR), light =
  EOTF(t / m) · shoulder(m^γ) · paper, shoulder(r) = 1 + (h − 1)(1 −
  e^−(r−1)/(h−1)), h = peak / paper: slope 1 at white, never above the peak.
* Encoding: **HDR10** BT.709 → BT.2020 (BT.2087 matrix), ST 2084 PQ, on
  `Rgb10a2Unorm` + `SurfaceColorSpace::Bt2100Pq`; **scRGB** linear BT.709,
  1.0 = 80 nits, on `Rgba16Float` + `ExtendedSrgbLinear`. PQ is preferred
  where both are offered (`NFSU2_HDR_SIGNAL=scrgb` prefers scRGB).
* The streak buffer still saturates at 1 as native (its decay steady state
  would blow out otherwise); the excess comes from the scene's additive
  draws and graded image + streaks.

### Calibration (the Display page)
While the page is open the transform draws a pattern in a band under the
rows (`hdr_output.wgsl` `pattern`): a 10 000-nit box holding a square at
the Peak Brightness value (raise it until the square disappears); paper
white beside half of it; black steps of SDR codes 0, 1, 2, 3, 4, 6, 8, 12
through the chosen gamma; alternating black/white lines beside the solid
code of their mean light under the chosen gamma. Auto: Windows reports the
display's peak and SDR white level (DXGI, `Surface::display_hdr_info`);
elsewhere 1000 / 203 nits (BT.2408 reference white).

## The SDR swap chain's format (`display::sdr_surface_format`)
Every renderer draws in the window's format (`WindowState::config.format`)
and writes display-encoded values, so the SDR swap chain must be a non-sRGB
format the compositor shows as sRGB. The surface's list is in the driver's
order, and a colour-managed Wayland desktop puts 16-bit formats before the
8-bit ones: RADV 26.2.5 under GNOME 51 lists `Bgra8UnormSrgb, Rgba8UnormSrgb,
Rgba16Float, Rgba16Unorm, Rgb10a2Unorm, Bgra8Unorm, Rgba8Unorm`
(`devprobes surface_formats` on a headless mutter; maintainer only (private developer probes, not in the public repository)). Taking the first non-sRGB
one gave a 16-bit float swap chain, and every read-back of a target in that
format (photo mode's picture, the showcase and magazine-cover pictures) assumed
4 bytes a pixel and failed wgpu's validation (a crash). The game now takes
8-bit BGRA, then 8-bit RGBA, then `Rgb10a2Unorm`, then `Rgba16Float` (asked
for in the sRGB colour space: `Auto` would make a 16-bit float swap chain
extended linear sRGB), then any other non-sRGB format, then the first one.
The read-backs of a window-format target go through
`render::read_back_rgba8`, which converts each of those formats to RGBA8 (an
8-bit value written into any of them reads back unchanged) and returns an
error for any other format. The log file's `display:` line names the adapter,
the chosen format, colour space and present mode, and the offered list.
HDR Output configures its own format and colour space over this one
(`WindowState::surface_config`); the loading screens then draw into the SDR
frame texture and go through the display transform like every other frame.

## Platform support matrix
| platform / backend | HDR10 PQ (`Rgb10a2Unorm`, BT2100_PQ) | scRGB (`Rgba16Float`, EXTENDED_SRGB_LINEAR) | how known |
|---|---|---|---|
| Linux, Vulkan (RADV 26.2.5), GNOME 51 Wayland (the reference desktop) | **offered by the surface** (HDR10_ST2084 on A2B10G10R10 / A2R10G10B10 and 8 others) | not offered (BT709_LINEAR is not extended) | `vulkaninfo` surface formats, 2026-11-04 |
| Linux, Vulkan, KDE Plasma 6 Wayland | expected (Mesa WSI + wp_color_management_v1) | expected | inferred, not run |
| Linux, X11 / Xwayland | not offered (no colour management on X) | no | inferred |
| Windows, DX12 | yes (on `Rgb10a2Unorm`) | yes | wgpu 30 docs; not run |
| Windows, Vulkan | driver-dependent (AMD/NVIDIA expose HDR10_ST2084) | yes (EXTENDED_SRGB_LINEAR) | wgpu 30 docs; not run |
| macOS, Metal | yes | yes (EDR) | wgpu 30 docs; not run |

Detection is the surface's own capability list (`hdr_output::choose`); the
row is greyed ("Not Supported") without either signal.

### What was verified on the reference desktop and what could not be
* The monitors (DP-1 Dell S2721D, DP-3 Dell S2725QS; EDID) carry **no HDR
  static metadata block**: they are SDR panels, and GNOME's HDR mode is off
  (`~/.config/monitors.xml` has no colour mode). HDR output **could not be
  seen** here. The compositor still offers the PQ colour space
  (vulkaninfo), so configuring the swap chain would likely be accepted; it
  was not tried (clone binaries run headless only, unit rule).
* Verified headless: the transform math (unit tests), the GPU pass against
  the CPU reference on both signals (`gpu_tests::display_transform_matches_
  the_cpu_reference`, ignored test, passes on the reference desktop GPU), the whole
  in-game path through `NFSU2_HDR_FAKE=pq|scrgb` (shots and bench: the
  same post passes and transform, into an HDR texture instead of a swap
  chain), and the LUT (above).

## Headless use
`NFSU2_HDR_FAKE=pq|scrgb NFSU2_SHOT_ENH="enh.hdr_output 1" game --shot
<name> out.png` writes `out.png` (the SDR frame) plus `out.hdr.png` (16-bit
PNG of the PQ signal, BT.2020; scRGB is written as PQ) and
`out.hdr-preview.png` (tone-mapped: light up to 0.8 × paper white as SDR,
the rest compressed to white at the peak), and prints the brightest pixel
and the share above paper white. `--shot options_graphics_display` is the
calibration page; `NFSU2_HDR_PATTERN=1` forces the pattern elsewhere.

## Native byte-identity (HDR Output Off)
md5 of `--shot` against a <base> build: main, explore, intro, qrcars,
race, qrstart, drift, drag, garage_neon, options_graphics_effects
identical; career differs by 158 px, and two <base> runs differ from each
other by 53 px (menu turntable, not deterministic). The new Display group
changes the Graphics page (one row more).

## Cost
`--bench explore 600`, reference desktop GPU, gpu busy ms/frame (shared machine, indicative):

| size | Original | High Precision Post Half | HDR Output PQ | HDR Output scRGB |
|---|---|---|---|---|
| 1920 × 1080 | 0.62 | 1.01 | 1.00 | 1.06 |
| 3840 × 2160 | 2.31 | 4.44 | 4.71 | 4.82 |

Over Original: +2.40 ms at 4K (the row's cost tag, `HDR_OUTPUT_COST_4K`),
+0.38 at 1080p; over High Precision Post only +0.27 (grade LUT 0.94 vs the
float bleach 0.92, the two-target last pass +0.08, the display transform
0.15). Breakdown in docs/evidence/hdr/README.md.

## Tests
`render::hdr_output::tests` (PQ points and round trip, BT.2020 matrix
white/luminance, shoulder continuity/bounds, midtones = SDR at paper
white, highlights to the peak with hue, UI over a highlight stays SDR,
encode/decode); `gpu_tests::display_transform_matches_the_cpu_reference`
(ignored, GPU); `postfx::tests::grade_lattice_sits_on_8bit_codes`,
`grade_lut_matches_the_native_passes` (ignored, GPU: lattice exact, max ±3,
> 98 % within ±1); `qol::tests::hdr_output_values`; `options::tests::
hdr_rows_are_greyed_without_an_hdr_signal`, `hdr_calibration_tiers_round_
trip`, `graphics_page_steps_and_restores` (the Display group).

## Open
* Not seen on an HDR display (none here). Windows / macOS not run.
* No HDR metadata (MaxCLL / mastering) is sent: wgpu 30 has no API for it.
* The 32³ LUT is within ±3 steps of the native grade, not exact (see
  above); a 256³ exact table is not built.
* The streak buffer keeps native saturation; neon and glow sprites get
  headroom only through the scene's own additive excess and the final add.
* Paper white / peak Auto read the display only on Windows (DXGI).
* Seen twice in ~30 `--bench explore` runs of this build on the shared
  machine (once with HDR Output off, once on, both while other GPU/build
  work ran): the bench stopped before its "load" line, every thread in a
  futex wait, no CPU. Not reproduced in 15 reruns (5 off, 5 HDR + TAA, 5
  plain) nor in 9 <base> runs; HDR Output does not touch loading. Cause
  open.
