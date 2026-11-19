# Animated Water

In the original the fountains' water is a flat texture. Animated Water draws the fountains and their pools
as moving water.

| Row (Advanced → Weather & Water) | Values | options.txt |
|---|---|---|
| **Animated Water** | Off / On / Realistic / Next-Gen | `enh.water 0/1/2/3` |

- **On:** moving water surfaces, sheets and spray at the fountains and pools.
- **Realistic:** the same water drawn closer to a modern game, with wet stone and the scene seen through
  the water.
- **Next-Gen:** past Realistic: simulated water with jets made of the art's sheets and drops, and mist.

Only the main view changes: the road reflection, the car's reflections and the rear-view mirror keep the
original's water, and so do the far ocean and the reflective canals. The Ultra preset sets On; Realistic and
Next-Gen are set by hand.

**Cost** of On when a fountain is in view, on the reference desktop GPU: +0.24 to +0.45 ms at 1920 × 1080 and
up to +0.62 ms at 3840 × 2160 (the Beacon Hill steps, the worst case). With no water in view it costs nothing.

## How it works

How each tier works, what it costs, how it was checked and what is open:
[Animated Water: how it works](../../notes/graphics/water.md).
