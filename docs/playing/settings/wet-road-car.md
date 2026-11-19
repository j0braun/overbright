# Wet Road Interaction

On a wet road, the cars change it: tyres leave tracks that dry off, the wheels throw up spray and mist, and
the cars' lights glow on the wet surface around them. In the original a wet road looks the same with or
without cars.

| Row (Advanced → Weather & Water) | Values | options.txt |
|---|---|---|
| **Wet Road Interaction** | Off / On / High | `enh.wet_interaction 0..2` |

The row needs a wet road and the spray, so it turns Wet Roads and Spray & Splashes on while it is on. Those
rows and Wet Car keep their own meaning. The Ultra preset sets On. On a dry road it costs nothing and draws
nothing.

**Cost** in heavy rain at night on the reference desktop GPU: On +0.23 ms at 1920 × 1080 and +0.76 ms at
3840 × 2160, High +0.28 / +0.82 ms. Most of it at 4K is the spray's mist.

## How it works

How it works, what it costs, how it was checked and what is open:
[Wet Road Interaction: how it works](../../notes/graphics/wet-road-car.md).
