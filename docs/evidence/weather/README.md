# This page weather evidence (docs/notes/graphics/weather.md)

All crops show **Off | On**. "Off" is the play setup
(`<the play setup's options file>`, via the maintainer's private capture; SSAA 2x, Enhanced
Paint, Wet Roads, SSR, lamps, …). "On" is the same settings plus the weather rows.

| file | what |
|---|---|
| `explore_dry_light_heavy_after.webp` | `--shot explore` 1920×1080. Rows from top: dry night (clock 93, identical), light rain (clock 121, amount 0.46), heavy (165, 0.75), just after the rain stopped (176: amount 0, car film 0.24, road 0.95). Wet Car, Rain Detail High, Spray, Lens Drops. |
| `night_lamps_standing_speed.webp` | `--bench drive` at clock 162 (night, rain 0.70, lamps), frames 100 (race start, 11 mph) and 400 (49 mph): wet cars, lit rain, splashes, spray behind the rivals, lens drops. |
| `wet_car_4k_off_on.webp` | 3840×2160, clock 165, Wet Car alone: darker, glossier paint, film on the roof. |
| `wet_car_4k_glass_zoom_off_on.webp` | The same, rear window and roof ×2.5: beads on the glass (top: dry look, Off). |

Scripts: the maintainer's private capture (the play setup as NFSU2_SHOT_ENH / NFSU2_BENCH_ENH) and
the maintainer's private capture (costs).
| `options_graphics_base_new.webp` | The only parity-gate shot difference: the Graphics page's new "Weather" group row. |
| `gates.txt` | `perf.sh parity` <base> vs this copy (full release). |
| `cost/` | bench.sh summaries: reference desktop GPU before (`summary-r1`) / after (`summary-r2-mist`) the mist optimisation, and the iGPU (`summary-igpu`). Columns: config, size, rep, GPU busy mean, median, weather pass ms. |
