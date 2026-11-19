# Atmosphere: Volumetric Fog and Sky "Next-Gen"

A night atmosphere closer to current racing games, in the original's colours and grade: haze that light
shines through, halos around lamps and headlights, light shafts in rain, and a sky with moving, moonlit clouds.
The original's fog, horizon fog and painted sky stay underneath and unchanged.

| Row (Advanced → Lighting & Effects) | Values | options.txt |
|---|---|---|
| **Volumetric Fog** | Off / Low / High / **Next-Gen** | `enh.volumetric_fog 0..3` |
| **Sky** | Original / **Next-Gen** | `enh.sky 0/1` |

Next-Gen fog is one volume for the whole view: height fog with drifting noise, thicker in rain, lit by the
street lamps and the cars' lights. Low and High are described on the [Temporal tier](temporal.md) page.
No preset sets Next-Gen or the Sky row; the Ultra preset keeps Volumetric Fog High.

**Cost** at 3840 × 2160 on the reference desktop GPU, at night: Next-Gen fog about +0.6 ms over fog Off (less
than High's +0.9 to +1.0 ms in lamp-dense streets), the Sky row about +0.6 ms, both together +1.1 to +1.2 ms.

## How it works

How it works, what it costs, how it was checked and what is open:
[Next-gen atmosphere: how it works](../../notes/graphics/atmosphere.md).
