# Wet Road Interaction evidence (docs/notes/graphics/wet-road-car.md)

All clone frames; there is no native counterpart. "before" = Wet Roads + Spray & Splashes + Wet Car (the wet-roads work / the weather package
rows); "after" = the same + Wet Road Interaction On. Fast release build, reference desktop GPU, 1920 × 1080 unless noted.
Rain scenes at world clock 162 (`NFSU2_RAIN_CLOCK`, night, rain 0.70).

| file | scene |
|---|---|
| race-start-rain_before_after.webp | `--shot qrstart`, standing cars (tail lights mirrored, little else: no speed) |
| drive-speed-00300/00500_before_after.webp | `--bench drive`, frames 300 / 500 (39 / 58 mph) |
| drive-speed-*_crop100 / _crop200.webp | the road behind the player's car at 100 % and 200 %: the tail lights' streaks, spray droplets, the darker lower body |
| cornering-rain_before_after.webp | frame 200, the turn at the first junction |
| after-rain-00900 / 01500_before_after.webp | `--bench drive` from clock 196: the rain stops at ~209 s; frame 900 ≈ 220 s (road wetness 0.45), frame 1500 ≈ 230 s (0.19) |
| trail-map-debug.webp | `NFSU2_WET_ROAD_DEBUG=1`: the trail map on the road (R track, G wake, B closeness) behind every car |
| sequence-drive-rain.webp | frames 240–300 every 3rd, 640 × 360, animated (lights mirrored moving with the cars, spray) |
| cost.txt | `--bench drive 600` at clock 162, GPU busy mean / median, weather pass; orig, wetroads, on, high × 1080p / 4K × 3 reps |
| splash-report-*.webp | a "splash on the barrier" seen in play: `--shot explore_rain`; the oval on the median barrier is the native lens drop (rain_drop.efx, 0x5C7AC0), present with every enhancement off (Original) and unchanged by Wet Roads or SSR (crop: Original / play setup without Wet Roads / without SSR, 300 %) |

Drift in the rain: the drift events' track (the stadium) has no rain, so the slip-driven spray is shown on the street
circuit's corners only; the slip's effect on the spray is in `wet_road::tests::spray_follows_speed_slip_and_puddles`.

Dry identity (md5, 1280 × 960): `explore`, `qrstart`, `race` with `enh.wet_interaction 2` = Off. Rain identity
against main: Original `explore_rain` and Wet Roads + Spray + Wet Car `explore_rain` md5-identical.
