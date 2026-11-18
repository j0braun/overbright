# Temporal evidence (docs/temporal.md)
All at 1280 × 960 from `--bench` dumps (window path; `NFSU2_BENCH_DUMP`),
labels on each crop. Native = the bench's default (World Detail 5, 8x MSAA).
| file | what | how |
|---|---|---|
| static_native_taa_upscale.webp | static camera (camtrack), native / TAA / Upscaling Quality / Performance | `NFSU2_BENCH_CAMTRACK` one pose ×200, `--bench explore`, frame 150 |
| motion_native_taa.webp | player car at speed, native vs TAA | `--bench racing`, frame 600 |
| motion_upscaling.webp | same, Upscaling Quality / Performance | `enh.upscale 1/3` |
| ghost_rival_73mph.webp | rival passing at 73 mph | `--bench racing`, frame 880 |
| ghost_rain.webp | rain at speed | `NFSU2_RAIN_CLOCK=170 --bench racing`, frame 600 |
| ghost_drift_drag.webp | drift and drag cameras | `NFSU2_BENCH_KIND=drift|drag`, frame 700 |
| nis_start.webp | race-start NIS (cuts) | `--bench race`, frame 250 |
| interpolated_240hz.webp | 1 vs 4 draws per update | `NFSU2_BENCH_DRAWS=4` |
| volumetric_off_low_high.webp | Volumetric Fog off / Low / High | `--bench racing`, frame 300 |
Costs: the private script `bench.sh SIZE [igpu]` (the script and its raw logs are
kept with the maintainer's private evidence).
| mip_bias.webp | the LOD bias: native / TAA / Upscaling Quality | `--bench racing`, frame 200 |
