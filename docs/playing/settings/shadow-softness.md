# Shadow softness under Modern Lighting

How shadows look with [Modern Lighting](modern-lighting.md) on: soft edges from lights the size of a real
lamp or sign, no flickering blotches under bright clusters of lights, and a picture that stays the same while
the car and the camera stand still. This page has no row of its own; with the Lighting row at Original
nothing changes.

Three cases were fixed: a dark, flickering pattern on the road under the casino canopy's light balls, hard
lines on the flag girl at the first race's start, and harsh shadows on a car at the airport. Lamps and signs
are now area lights with smooth penumbrae, and Modern + RT traces rays to the four strongest lights of each
pixel.

**Cost** in a race at 3840 × 2160 on the reference desktop GPU, with the settings these cases were checked at:
Modern 9.95 → 11.12 ms a frame, Modern + RT 13.30 → 17.78 ms. Part of Modern's rise is a fix: before it, half
the lights under the casino canopy were missing.

## How it works

The causes, the fixes, the measurements and what is open:
[Shadow softness and stability under Modern Lighting: how it works](../../notes/graphics/shadow-softness.md).
