# Reflections

Sharper and more complete reflections on the car and the road, wet roads that shine only where it rained, and
reflections traced on screen or with ray tracing. Every row is off by default; then the game draws the
original's reflections: a small planar capture of the road and a cube map around the car, with no cars in the
road's reflection.

## The rows

| Row (group on the Advanced page) | Values | What you see |
|---|---|---|
| **Enhanced Reflections** (Image Quality) | Off / On | The car's reflections and highlight worked out per pixel, a capture with the main view's detail, and a sharper planar reflection on wet roads. |
| **Car Reflection**, **Road Reflection**, **Reflection Rate** (Image Quality) | resolution and update rate | Larger and more often updated reflection captures. They add GPU work, more so with SSAA. |
| **Wet Roads** (Weather & Water) | Off / On | The road looks wet where the rain wetted it, and dries again. |
| **Screen-Space Reflections** (Image Quality) | Off / Half / Full / Ray Traced | Reflections of what is on screen, the cars included, on the road and the cars. **Ray Traced** is experimental and only offered on a GPU that can trace rays. |

What it costs (GPU time per frame on the reference desktop GPU, free roam):

| Setting | 1920 × 1080 | 3840 × 2160 |
|---|---|---|
| Wet Roads | +0.04 ms | about 0 |
| SSR Half | +0.52 ms | +1.20 ms |
| SSR Full | +0.54 ms | +1.98 ms |
| Ray Traced over SSR Ultra (free roam) | +0.25 ms | +0.79 ms |

On a small integrated GPU (2 compute units, 1080p) SSR is expensive: +14.5 ms at Half, +20.4 ms at Full. The
measurements, their setups and the other scenes are on the technical page (below).


## How it works

How it works, what it costs, how it was checked and what is open: [Reflections: how it works](../../notes/graphics/reflections.md).
