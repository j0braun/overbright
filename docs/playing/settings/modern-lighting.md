# Modern Lighting

Lights the whole city with real light sources: street lamps, neon and signs, and the cars' lights, with
shadows, while keeping the original's painted light as the base. The strength row runs from Classic (close
to the original's look) to Bold.

| Row (Advanced → Lighting & Effects) | Values | options.txt |
|---|---|---|
| **Lighting** | Original / Modern Low / Modern / Modern + RT | `enh.modern_lighting 0..3` |
| **Lighting Strength** | Classic / 25% / 50% / 75% / Bold | `enh.modern_strength 0..100` |

**Original** is the default and the original's baked light. **Modern + RT** uses ray tracing for the shadows;
it is greyed on a GPU without ray tracing and takes effect after a restart. Lighting Strength is greyed while
Lighting is Original. The presets leave the row at Original.

**Cost** at 1920 × 1080 in a race on the reference desktop GPU: Modern Low +0.30 ms, Modern +1.38 ms,
Modern + RT +2.56 ms; at 3840 × 2160 +0.82, +2.78 and +5.58 ms.

## How it works

The design, how it was measured and checked, and what is open:
[Modern Lighting: how it works](../../notes/graphics/modern-lighting.md).
