# Temporal tier: TAA, Upscaling, Volumetric Fog

Three settings that use earlier frames to make the picture smoother or faster, and light shafts in the air:

| Row (Advanced page group) | Values | options.txt |
|---|---|---|
| **Anti-Aliasing** (Image Quality) | … / **TAA** | `enh.post_aa 11` |
| **Upscaling** (Performance) | Off / Quality 67 % / Balanced 58 % / Performance 50 % | `enh.upscale 0..3` |
| **Volumetric Fog** (Lighting & Effects) | Off / Low / High | `enh.volumetric_fog 0..2` |

- **TAA** smooths edges and keeps them stable in motion, but the picture is softer than the original's 8x
  MSAA.
- **Upscaling** draws the game at a lower size and rebuilds the full size from several frames. The same row
  also offers FSR 1 ([Modern extras](../modern-extras.md)) and [FSR 2](fsr2.md).
- **Volumetric Fog** lights the air around lamps and headlights. Next-Gen fog is on the
  [Atmosphere](atmosphere.md) page.

All three are off by default. The Ultra preset sets Volumetric Fog High; TAA and Upscaling are set by hand.

**Cost or gain** at night in free roam on the reference desktop GPU, against the original's 8x MSAA:
TAA −0.80 ms and Upscaling Quality −0.92 ms at 3840 × 2160. On a small integrated GPU TAA costs +1.48 ms at
1920 × 1080.

## How it works

How it works, the measurements and what is open:
[Temporal tier: how it works](../../notes/graphics/temporal.md).
