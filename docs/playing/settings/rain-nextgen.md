# Next-Gen rain

Rain at the level of current games: drops in three layers around you that the wind carries and that blur
with your speed, splashes on the road, and haze in the light of lamps and headlights. Where and when it rains,
and how much, stays the original's weather.

| Row (Advanced → Weather & Water) | Values | options.txt |
|---|---|---|
| **Rain Detail** | Off / On / High / **Next-Gen** | `enh.rain_detail 0..3` |

Off, On and High are described on the [Weather](weather.md) page. No preset sets Next-Gen.

**Cost** in rain at 3840 × 2160 on the reference desktop GPU: about +0.85 ms over the original (the rain pass
0.94 ms against High's 0.39 ms).

## How it works

How it works, what it costs, how it was checked and what is open:
[Next-Gen rain: how it works](../../notes/graphics/rain-nextgen.md).
