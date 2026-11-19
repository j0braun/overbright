# Sparks & Fire

Two things on this page:

1. **The original's flame pops**, at every setting: the exhaust flames a turbo car shoots at a gear shift and
   when revving in neutral, without nitrous. Overbright did not fire them before; now it does, as the
   original does. This was the remaining cause found for nitrous-style flames sometimes missing.
2. **Next-Gen sparks and fire**, optional: the original's sparks and flames drawn as simulated, glowing sparks
   and flames that light their surroundings, with heat haze. Where and when they appear stays the original's.

| Row (Advanced → Lighting & Effects) | Values | options.txt |
|---|---|---|
| **Sparks & Fire** | Original / Next-Gen | `enh.sparks_fire 0..1` |

**Cost** of Next-Gen at 3840 × 2160 on the reference desktop GPU: about 0.1 ms in the frames that show two
cars' flames; the row's hint says 0.2 ms for a long scrape against a wall (not measured).

## How it works

The trace of the original's flame triggers and how Next-Gen works, what it costs and what is open:
[Sparks & Fire: how it works](../../notes/graphics/sparks-fire.md).
