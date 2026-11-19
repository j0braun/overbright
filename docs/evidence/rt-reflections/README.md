# This page evidence (docs/notes/graphics/reflections.md "Ray-traced reflections (experimental)")
- `probe-rx7800xt.txt`: the maintainer's probe `rt_probe` output (wgpu 30 ray query on RADV NAVI32).
- `bench.txt` (+ `bench_summary.py`, run in the bench dir): gpu busy / trace pass / VRAM / draw CPU,
  explore, racing, rain × 1080p / 4K × SSR Ultra (ssr2) vs Ray Traced (ssr3), 2 runs each.
- `*_ssr_ultra_vs_rt.jpg`: 1280 × 960 `--shot` crops, left SSR Ultra, right Ray Traced (Wet Roads on;
  rain = `NFSU2_RAIN_CLOCK=170 explore`).
