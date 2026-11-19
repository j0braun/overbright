# FSR 2 temporal upscaling

AMD FidelityFX Super Resolution 2 draws the game at a lower size and rebuilds a sharp full-size picture from
several frames, so the game runs faster.

| Row (Advanced → Performance) | Values | options.txt |
|---|---|---|
| **Upscaling** | … / FSR 2 Native AA / Quality / Balanced / Performance / Ultra Performance | `enh.upscale 7..11` |

At 3840 × 2160 the game draws at 2560 × 1440 (Quality), 2258 × 1270 (Balanced), 1920 × 1080 (Performance) or
1280 × 720 (Ultra Performance); Native AA draws at full size and only smooths edges. Under FSR 2 the
Anti-Aliasing row is greyed and reads "FSR 2 (Upscaling)": SSAA, MSAA, FXAA and SMAA are off. Sharpen still
applies after it. FSR 1 and the temporal upscaler's tiers stay in the same row.

**Cost or gain** in a race at 3840 × 2160 on the reference desktop GPU: against SSAA 2x (11.82 ms a frame),
FSR 2 Quality takes 6.94 ms (−41 %) and Balanced 6.20 ms (−48 %).

FSR 2 is AMD's code (v2.2.1, MIT licence), ported to WGSL; the notice is in THIRD-PARTY-NOTICES.md.

## How it works

How it works, what it costs, how it was checked and what is open:
[FSR 2 temporal upscaling: how it works](../../notes/graphics/fsr2.md).
