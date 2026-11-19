# This page evidence: motion blur and depth of field (docs/notes/graphics/motionblur-dof.md)

Clone frames only (these effects are not in SPEED2.EXE; nothing to compare
under Wine). 1280 × 960, reference desktop GPU, World Detail 5 (native 8x MSAA).
Frames: the maintainer's private capture (made with `scripts/frames.sh`); costs: the maintainer's private capture.

| file | what |
|---|---|
| `speed_night_rival.webp` | `--bench drive` frame 900, 74 mph at night beside a rival: Original (native speed blur) / Camera / Per-Object. Road and barrier streak; the player's car and the HUD stay sharp; in Per-Object the rival (moving with us) is sharper than in Camera |
| `drift.webp` | `--bench racing` (NFSU2_BENCH_KIND=drift) frame 500, 38 mph: Original / Camera / Per-Object (the Camera tier's speed ramp is low at this speed) |
| `wheels.webp` | the drift frame's rear wheel ×2, Original vs Per-Object (seen almost edge-on: the arc is short on screen) |
| `rate_60_240.webp` | Per-Object at 60 Hz (1 drawn frame per update) vs 240 Hz (`NFSU2_BENCH_DRAWS=4`): the same streaks. RMSE blur vs none 0.0142 (60) / 0.0146 (240); 60 vs 240 with blur 0.0078, without 0.0050 (the interpolated poses alone) |
| `cutscene_dof.webp`, `cutscene_dof_crop.webp` | `--shot qrstart` (the race start's scene camera): Depth Of Field Off vs Cinematic; the cars in focus, the city behind soft, glow sprites sharp (drawn after) |
| `garage_dof.webp` | `--shot qrcars` (the car lot's turntable): Off vs Cinematic |
