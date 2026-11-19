# Smoke (Particles)

More detailed tyre smoke, burnout clouds and contact smoke. The **Particles** row (Advanced → Lighting &
Effects) has four values; **Original** is the default and draws the original game's smoke exactly.

| Value | What you see |
|---|---|
| **Original** | The original's smoke. |
| **High** | Each puff of the original's smoke becomes 4 smaller puffs that drift, linger and are lit by the scene's lights. |
| **Ultra** | As High with 8 puffs per original puff: denser, more detailed clouds. |
| **Cinematic** | Volumetric smoke: rolling billows lit by the scene, steam that starts from the rain and evaporates. In thick smoke it is much cheaper than Ultra. |

The original's particle system still decides where, when and how much smoke appears, at every value. Sparks,
nitrous flames, the event markers' smoke and the front end's smoke stay the original's at every value (sparks
and flames have their own row: [Sparks & Fire](sparks-fire.md)). The options.txt line is
`enh.particles 0..3`; the Ultra preset sets Ultra, Cinematic is set by hand.

## What it costs

GPU time per frame on the reference desktop GPU, average over a run, as the extra over Original. High and Ultra
in a drift race are from the GPU-particles bench, the rest from the Extreme preset's bench, so the rows are not
exactly comparable:

| Scene | Size | High | Ultra | Cinematic |
|---|---|---|---|---|
| Race, drift (light smoke) | 1920 × 1080 | +0.05 ms | +0.08 ms | about +0.4 ms |
| Race, drift (light smoke) | 3840 × 2160 | +0.13 ms | +0.23 ms | +0.34 ms |
| Free roam, heavy burnout | 3840 × 2160 | | +10.6 ms (p95 88 ms) | +0.36 ms (p95 13.4 ms) |

The cost of High and Ultra is fill rate: when a cloud covers much of the screen single frames get slow. On a
small integrated GPU (2 compute units, 1080p) High costs +2.8 ms and Ultra +4.6 ms on average, with frames of
50 to 73 ms inside a thick cloud. The full tables, their setups and how each tier works are in
[notes/graphics/smoke.md](../../notes/graphics/smoke.md).
