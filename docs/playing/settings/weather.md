# Weather

Better rain effects, the car included. Where and when it rains, how much, how wet the road gets and the fog
stay the original's weather.

| Row (Advanced → Weather & Water) | Values | options.txt | Presets |
|---|---|---|---|
| **Wet Car** | Off / On | `enh.wet_car 0/1` | Modern, Ultra |
| **Rain Detail** | Off / On / High (/ [Next-Gen](rain-nextgen.md)) | `enh.rain_detail 0..3` | Modern On, Ultra High |
| **Spray & Splashes** | Off / On | `enh.spray 0/1` | Modern, Ultra |
| **Lens Drops** | Off / On | `enh.lens_drops 0/1` | none (set by hand) |

- **Wet Car:** the car looks wet in the rain and dries afterwards.
- **Rain Detail:** finer rain streaks, splashes and light around lamps.
- **Spray & Splashes:** spray thrown up by the cars' wheels and splashes on the road.
- **Lens Drops:** more rain drops on the camera's lens, through the original's own lens-drop effect.

The road's wetness and its reflections are on the [Reflections](reflections.md) page (Wet Roads), the cars'
effect on the wet road on [Wet Road Interaction](wet-road-car.md).

**Cost** in heavy rain at night on the reference desktop GPU: all four rows together +0.14 ms at
1920 × 1080 and about +0.45 ms at 3840 × 2160; on a small integrated GPU about +3.5 ms.

## How it works

How each row works, what it costs, how it was checked and what is open:
[Weather package: how it works](../../notes/graphics/weather.md).
