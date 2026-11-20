<!-- loadgate.sh run, 10 runs per arm, mode and scene; the bench's loadgate.log lines: -->
bench 2026-11-19T20:14:11+01:00 reps 10 modes warm cold cores scenes boot explore frames 20 low cpus 0-3 cold root copy; loadavg 8.68 8.19 9.23
arm base <out>/bin/base f6f213bb8b201706 assets original
arm new <out>/bin/new 75a440fbec8d2c3a assets original
rep 1 done 2026-11-19T20:15:00+01:00 loadavg 6.65 7.75 9.03
rep 2 done 2026-11-19T20:15:21+01:00 loadavg 6.49 7.64 8.96
rep 3 done 2026-11-19T20:15:37+01:00 loadavg 6.15 7.48 8.88
rep 4 done 2026-11-19T20:15:54+01:00 loadavg 6.69 7.53 8.87
rep 5 done 2026-11-19T20:16:10+01:00 loadavg 6.16 7.37 8.80
rep 6 done 2026-11-19T20:16:27+01:00 loadavg 5.40 7.14 8.70
rep 7 done 2026-11-19T20:16:43+01:00 loadavg 5.07 6.95 8.61
rep 8 done 2026-11-19T20:17:12+01:00 loadavg 7.90 7.40 8.71
rep 9 done 2026-11-19T20:17:53+01:00 loadavg 8.18 7.62 8.72
rep 10 done 2026-11-19T20:18:30+01:00 loadavg 13.08 8.86 9.10

# Load-speed gate: base (base) vs new (new)

## warm (64 measures; p threshold 0.01/64)

| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |
|---|---|---|---|---|---|---|---|---|
| boot | start -> entry | 34.0 | 33.0 | +0.0 [-2.0..+3.0] | 1.5 | 5.0 | 0.53 | same |
| boot | start -> first frame | 67.5 | 65.0 | -1.0 [-4.0..+2.0] | 2.6 | 5.2 | 0.81 | same |
| boot | start -> title | 88.5 | 82.5 | -6.0 [-10.0..-2.0] | 3.7 | 7.4 | 1 | same |
| boot | stage World::load: from the cache | 24.9 | 34.2 | +9.4 [+6.7..+11.9] | 1.8 | 3.6 | 0.00075 | slower, not proven |
| boot | stage World::load | 32.6 | 41.1 | +9.0 [+6.3..+11.4] | 1.9 | 3.7 | 0.00075 | slower, not proven |
| boot | stage collision ahead | 7.3 | 7.7 | +0.3 [-0.8..+0.9] | 0.6 | 2.0 | 0.14 | same |
| boot | stage Game::new | 22.6 | 23.1 | +0.1 [-1.8..+1.7] | 1.4 | 2.8 | 0.46 | same |
| boot | stage preload car: renderer | 6.0 | 6.4 | +0.0 [-2.6..+1.3] | 1.2 | 2.4 | 0.51 | same |
| boot | stage Car::load_tuned: shared parts and the rest | 12.1 | 11.3 | -1.1 [-3.9..+7.6] | 3.8 | 7.6 | 0.63 | same |
| boot | stage programs ahead | 11.3 | 9.2 | -1.5 [-3.4..+0.4] | 1.9 | 3.8 | 0.95 | same |
| boot | stage garage ahead | 14.5 | 13.3 | -1.9 [-13.0..+8.6] | 5.2 | 10.4 | 0.73 | same |
| boot | stage WorldRenderer::new: groups and draws | 8.7 | 4.0 | -2.3 [-8.7..+0.2] | 6.0 | 12.0 | 0.95 | same |
| boot | stage cache: open | 20.2 | 18.3 | -2.5 [-5.2..-0.8] | 0.9 | 2.0 | 1 | same |
| boot | stage Car::load_tuned: solids, textures | 21.0 | 15.8 | -4.8 [-8.5..-2.1] | 2.1 | 4.2 | 1 | same |
| boot | stage Car::load_tuned | 35.7 | 30.1 | -7.0 [-14.6..+1.6] | 5.4 | 10.7 | 0.96 | same |
| explore | load | 168.3 | 189.0 | +20.3 [+14.3..+25.3] | 4.0 | 8.4 | 3.8e-05 | SLOWER |
| explore | load + first frame | 175.8 | 197.6 | +20.0 [+13.0..+23.9] | 3.1 | 8.8 | 6.5e-05 | SLOWER |
| explore | first frame | 9.0 | 8.6 | -0.4 [-2.1..+1.9] | 1.7 | 5.0 | 0.71 | same |
| explore | stage drive_in | 129.3 | 148.0 | +19.6 [+13.5..+23.6] | 3.9 | 7.8 | 2.2e-05 | SLOWER |
| explore | stage World::load | 22.3 | 40.5 | +18.3 [+17.2..+19.3] | 0.8 | 2.0 | 5.4e-06 | SLOWER |
| explore | stage World::load: from the cache | 15.4 | 34.0 | +18.2 [+17.5..+19.2] | 0.8 | 2.0 | 5.4e-06 | SLOWER |
| explore | stage Game::new | 21.9 | 23.1 | +1.2 [-0.2..+3.1] | 1.6 | 3.1 | 0.072 | same |
| explore | stage Encounters::new | 22.1 | 22.8 | +0.6 [-0.8..+2.3] | 1.0 | 2.0 | 0.2 | same |
| explore | stage drive_in: uploads | 1.8 | 2.7 | +0.6 [-0.3..+2.2] | 1.5 | 3.0 | 0.12 | same |
| explore | stage Car::load_tuned: part database | 3.2 | 3.5 | +0.3 [-0.6..+1.0] | 0.6 | 2.0 | 0.32 | same |
| explore | stage drive_in: warm | 9.3 | 9.9 | +0.3 [-0.6..+1.5] | 0.8 | 2.0 | 0.12 | same |
| explore | stage Encounters::new: picks | 1.9 | 2.2 | +0.2 [-0.2..+0.6] | 0.2 | 2.0 | 0.22 | same |
| explore | stage collision ahead | 7.8 | 7.9 | +0.1 [-0.4..+0.7] | 0.4 | 2.0 | 0.29 | same |
| explore | stage Encounters: cars joined | 9.3 | 9.3 | +0.1 [-0.8..+1.5] | 0.8 | 2.0 | 0.37 | same |
| explore | stage warm: car renderer | 5.0 | 5.0 | +0.1 [-0.3..+0.6] | 0.3 | 2.0 | 0.34 | same |
| explore | stage drive_in: car renderer | 3.0 | 3.0 | +0.1 [-0.1..+0.2] | 0.1 | 2.0 | 0.24 | same |
| explore | stage WorldRenderer::new: buffers, uniforms | 6.6 | 6.7 | +0.1 [-0.3..+0.5] | 0.5 | 2.0 | 0.43 | same |
| explore | stage Car::load_tuned: shared parts and the rest | 21.7 | 22.1 | +0.0 [-1.3..+1.7] | 1.1 | 2.2 | 0.46 | same |
| explore | stage warm: world renderer | 3.9 | 3.8 | +0.0 [-0.4..+0.9] | 0.3 | 2.0 | 0.46 | same |
| explore | stage WorldRenderer::new: groups and draws | 8.9 | 9.1 | -0.1 [-1.1..+0.7] | 0.6 | 2.0 | 0.54 | same |
| explore | stage car renderers ahead | 3.3 | 3.3 | -0.1 [-0.4..+0.4] | 0.4 | 2.0 | 0.63 | same |
| explore | stage warm: post chain | 2.5 | 2.4 | -0.1 [-0.3..+0.1] | 0.2 | 2.0 | 0.78 | same |
| explore | stage WorldRenderer::new: pipelines | 11.9 | 11.4 | -0.3 [-1.0..+0.4] | 0.8 | 2.0 | 0.9 | same |
| explore | stage WorldRenderer::new: texture arrays: upload | 18.9 | 17.7 | -1.0 [-6.4..+3.0] | 5.5 | 11.0 | 0.76 | same |
| explore | stage WorldRenderer::new: texture arrays | 19.1 | 17.9 | -1.0 [-6.4..+2.9] | 5.4 | 10.9 | 0.76 | same |
| explore | stage WorldRenderer::new: draws and textures | 34.3 | 33.4 | -1.1 [-7.6..+2.4] | 4.7 | 9.4 | 0.78 | same |
| explore | stage WorldRenderer::new | 79.5 | 79.2 | -1.1 [-5.3..+3.4] | 3.3 | 6.6 | 0.71 | same |
| explore | stage cache: open | 26.0 | 23.3 | -2.7 [-4.4..-1.5] | 1.2 | 2.4 | 1 | same |
| explore | stage Car::load_tuned | 64.6 | 60.8 | -3.0 [-6.3..+1.0] | 3.3 | 6.7 | 0.93 | same |
| explore | stage Car::load_tuned: solids, textures | 31.8 | 28.5 | -3.2 [-4.6..-1.6] | 1.3 | 2.6 | 0.99 | same |

## cold (55 measures; p threshold 0.01/55)

| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |
|---|---|---|---|---|---|---|---|---|
| boot | start -> entry | 64.0 | 72.5 | +8.0 [-6.0..+21.0] | 11.5 | 23.0 | 0.07 | same |
| boot | start -> first frame | 118.0 | 130.0 | +7.5 [-14.0..+26.0] | 16.3 | 32.6 | 0.15 | same |
| boot | start -> title | 260.5 | 237.0 | -15.5 [-115.0..+60.0] | 81.5 | 163.1 | 0.63 | same |
| boot | stage Car::load_tuned | 0.0 | 95.8 | +95.8 [+0.0..+153.4] | 0.0 | 10.0 | 0.012 | slower, not proven |
| boot | stage Car::load_tuned: shared parts and the rest | 0.0 | 31.2 | +30.7 [+0.0..+35.4] | 0.9 | 10.0 | 0.0074 | slower, not proven |
| boot | stage Car::load_tuned: solids, textures | 172.4 | 158.7 | +8.3 [-84.9..+146.1] | 87.8 | 175.5 | 0.48 | same |
| boot | stage preload car: renderer | 0.0 | 1.4 | +1.3 [+0.0..+1.7] | 0.2 | 10.0 | 0.015 | same |
| boot | stage Game::new | 36.8 | 41.4 | +0.8 [-3.8..+13.0] | 8.2 | 16.4 | 0.37 | same |
| boot | stage programs ahead | 13.1 | 12.8 | -0.5 [-7.3..+2.8] | 3.0 | 10.0 | 0.6 | same |
| boot | stage cache: open | 310.7 | 280.3 | -24.4 [-132.4..+94.4] | 132.0 | 263.9 | 0.6 | same |
| explore | load + first frame | 332.7 | 347.7 | +21.9 [-23.6..+145.0] | 36.0 | 72.0 | 0.2 | same |
| explore | load | 323.7 | 336.4 | +20.1 [-22.4..+146.1] | 31.1 | 62.3 | 0.2 | same |
| explore | first frame | 8.5 | 10.9 | +1.9 [-1.1..+4.8] | 2.2 | 20.0 | 0.062 | same |
| explore | stage Car::load_tuned: solids, textures | 195.9 | 237.4 | +46.0 [-9.8..+108.9] | 41.3 | 82.7 | 0.053 | same |
| explore | stage Car::load_tuned | 305.0 | 351.7 | +43.0 [-10.1..+143.9] | 47.3 | 94.7 | 0.045 | same |
| explore | stage cache: open | 305.9 | 331.2 | +31.4 [-55.7..+150.7] | 67.6 | 135.2 | 0.18 | same |
| explore | stage World::load: from the cache | 16.0 | 35.9 | +19.9 [+18.5..+21.1] | 1.0 | 10.0 | 0.00053 | slower, not proven |
| explore | stage World::load | 134.2 | 142.2 | +14.9 [-26.4..+74.2] | 33.1 | 66.2 | 0.2 | same |
| explore | stage drive_in | 256.8 | 250.9 | +8.9 [-29.9..+98.9] | 31.4 | 62.8 | 0.34 | same |
| explore | stage Encounters: cars joined | 35.3 | 41.4 | +8.3 [+0.0..+18.0] | 5.9 | 11.9 | 0.022 | same |
| explore | stage WorldRenderer::new: draws and textures | 34.8 | 36.7 | +1.8 [-1.9..+5.1] | 3.0 | 10.0 | 0.16 | same |
| explore | stage WorldRenderer::new | 80.9 | 82.0 | +1.0 [-2.2..+4.6] | 3.2 | 10.0 | 0.24 | same |
| explore | stage WorldRenderer::new: texture arrays | 18.1 | 20.5 | +0.9 [-2.0..+4.7] | 3.0 | 10.0 | 0.24 | same |
| explore | stage WorldRenderer::new: texture arrays: upload | 17.5 | 19.9 | +0.8 [-2.0..+4.6] | 2.9 | 10.0 | 0.26 | same |
| explore | stage Car::load_tuned: part database | 2.5 | 3.3 | +0.8 [-0.2..+2.6] | 1.0 | 10.0 | 0.072 | same |
| explore | stage explore Traffic::load | 1.1 | 1.8 | +0.6 [+0.0..+1.8] | 0.4 | 10.0 | 0.014 | same |
| explore | stage Encounters::new | 25.6 | 25.7 | +0.2 [-1.6..+3.7] | 1.1 | 10.0 | 0.34 | same |
| explore | stage drive_in: car renderer | 3.3 | 3.6 | +0.2 [-0.1..+0.8] | 0.2 | 10.0 | 0.14 | same |
| explore | stage warm: world renderer | 4.1 | 4.1 | +0.1 [-0.2..+0.7] | 0.3 | 10.0 | 0.4 | same |
| explore | stage Encounters::new: picks | 2.2 | 2.1 | +0.0 [-0.3..+0.4] | 0.3 | 10.0 | 0.46 | same |
| explore | stage collision ahead | 7.8 | 7.7 | +0.0 [-0.6..+0.7] | 0.6 | 10.0 | 0.43 | same |
| explore | stage WorldRenderer::new: groups and draws | 9.0 | 9.2 | +0.0 [-1.2..+1.1] | 1.0 | 10.0 | 0.49 | same |
| explore | stage warm: post chain | 2.2 | 2.3 | +0.0 [-0.3..+0.2] | 0.2 | 10.0 | 0.43 | same |
| explore | stage car renderers ahead | 3.3 | 3.3 | +0.0 [-0.3..+0.2] | 0.2 | 10.0 | 0.41 | same |
| explore | stage WorldRenderer::new: pipelines | 11.7 | 11.4 | -0.1 [-0.9..+0.5] | 0.7 | 10.0 | 0.71 | same |
| explore | stage WorldRenderer::new: buffers, uniforms | 7.4 | 7.3 | -0.2 [-0.9..+0.3] | 0.4 | 10.0 | 0.8 | same |
| explore | stage warm: car renderer | 5.1 | 4.8 | -0.3 [-0.6..+0.1] | 0.3 | 10.0 | 0.96 | same |
| explore | stage drive_in: warm | 10.4 | 9.6 | -0.5 [-1.3..+0.4] | 0.8 | 10.0 | 0.88 | same |
| explore | stage Game::new | 35.6 | 34.4 | -0.6 [-2.6..+2.1] | 1.4 | 10.0 | 0.68 | same |
| explore | stage drive_in: uploads | 3.3 | 1.5 | -0.9 [-3.6..+0.5] | 2.1 | 10.0 | 0.9 | same |
| explore | stage Car::load_tuned: shared parts and the rest | 100.5 | 97.2 | -1.5 [-21.7..+12.8] | 8.2 | 16.5 | 0.68 | same |

## cores (69 measures; p threshold 0.01/69)

| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |
|---|---|---|---|---|---|---|---|---|
| boot | start -> entry | 39.0 | 38.5 | +1.0 [-4.0..+7.0] | 3.7 | 7.4 | 0.4 | same |
| boot | start -> title | 96.0 | 101.0 | +1.0 [-20.0..+15.0] | 12.6 | 25.2 | 0.52 | same |
| boot | start -> first frame | 66.5 | 70.0 | +0.0 [-9.0..+8.0] | 7.0 | 14.1 | 0.56 | same |
| boot | stage World::load: from the cache | 23.9 | 39.6 | +16.2 [-21.3..+21.7] | 2.8 | 5.6 | 0.056 | slower, not proven |
| boot | stage World::load | 33.2 | 47.9 | +15.3 [-32.3..+22.2] | 5.8 | 11.6 | 0.056 | slower, not proven |
| boot | stage cache: open | 44.8 | 52.9 | +7.3 [-8.8..+18.1] | 7.6 | 15.2 | 0.2 | same |
| boot | stage Car::load_tuned: solids, textures | 35.7 | 40.8 | +3.9 [-16.7..+13.1] | 9.3 | 18.7 | 0.44 | same |
| boot | stage Car::load_tuned: shared parts and the rest | 10.1 | 13.6 | +0.3 [-7.5..+13.2] | 10.8 | 21.5 | 0.45 | same |
| boot | stage collision ahead | 7.5 | 7.6 | +0.0 [-7.2..+1.1] | 0.5 | 2.0 | 0.38 | same |
| boot | stage Car::load_tuned | 37.7 | 39.1 | +0.0 [-16.5..+29.6] | 12.9 | 25.8 | 0.52 | same |
| boot | stage preload car: renderer | 1.7 | 1.4 | -0.1 [-0.5..+1.3] | 0.4 | 2.0 | 0.63 | same |
| boot | stage Game::new | 27.0 | 26.3 | -0.5 [-5.2..+3.4] | 4.2 | 8.4 | 0.71 | same |
| boot | stage WorldRenderer::new: solid spheres | 2.3 | 0.0 | -2.2 [-2.3..+0.0] | 0.1 | 2.0 | 0.98 | same |
| boot | stage WorldRenderer::new: groups and draws | 17.4 | 0.0 | -2.3 [-18.0..+0.0] | 0.5 | 2.0 | 0.96 | same |
| boot | stage programs ahead | 17.5 | 12.7 | -5.1 [-9.8..-0.7] | 4.8 | 9.6 | 0.99 | same |
| explore | load | 185.8 | 201.0 | +16.3 [+3.8..+25.7] | 8.7 | 17.4 | 0.0093 | same |
| explore | load + first frame | 193.9 | 209.5 | +15.7 [+2.0..+26.5] | 11.6 | 23.3 | 0.014 | same |
| explore | first frame | 8.5 | 7.5 | -0.3 [-3.3..+2.9] | 2.7 | 5.4 | 0.63 | same |
| explore | stage World::load: from the cache | 13.8 | 33.2 | +19.8 [+16.5..+21.0] | 1.1 | 2.3 | 5.4e-06 | SLOWER |
| explore | stage World::load | 25.1 | 43.0 | +17.9 [+14.1..+19.9] | 2.3 | 4.6 | 0.00075 | slower, not proven |
| explore | stage drive_in | 146.9 | 160.5 | +15.4 [+3.2..+26.7] | 6.9 | 13.7 | 0.0026 | slower, not proven |
| explore | stage WorldRenderer::new: draws and textures | 34.0 | 36.8 | +1.5 [-4.3..+5.7] | 4.3 | 8.6 | 0.4 | same |
| explore | stage drive_in: uploads | 2.2 | 4.2 | +1.0 [-1.2..+3.2] | 1.9 | 3.7 | 0.24 | same |
| explore | stage Encounters: cars joined | 6.1 | 6.9 | +0.8 [-2.0..+2.3] | 1.1 | 2.3 | 0.24 | same |
| explore | stage Encounters::new: picks | 2.2 | 3.0 | +0.7 [-0.1..+1.1] | 0.4 | 2.0 | 0.032 | same |
| explore | stage collision ahead | 7.6 | 8.3 | +0.6 [-0.1..+1.6] | 0.8 | 2.0 | 0.032 | same |
| explore | stage drive_in: warm | 12.2 | 13.0 | +0.6 [-0.8..+1.9] | 0.7 | 2.0 | 0.26 | same |
| explore | stage WorldRenderer::new: groups and draws | 9.7 | 10.5 | +0.4 [-1.4..+1.9] | 1.3 | 2.5 | 0.24 | same |
| explore | stage Game::new | 25.9 | 26.5 | +0.4 [-1.8..+3.1] | 1.8 | 3.6 | 0.34 | same |
| explore | stage WorldRenderer::new: texture arrays | 16.6 | 19.0 | +0.1 [-4.2..+4.7] | 2.3 | 4.6 | 0.51 | same |
| explore | stage WorldRenderer::new: solid spheres | 1.0 | 1.1 | +0.1 [-0.1..+0.4] | 0.2 | 2.0 | 0.26 | same |
| explore | stage WorldRenderer::new: buffers, uniforms | 6.7 | 6.9 | +0.1 [-0.6..+0.7] | 0.7 | 2.0 | 0.37 | same |
| explore | stage WorldRenderer::new: texture arrays: upload | 16.3 | 18.8 | +0.1 [-4.2..+4.6] | 2.3 | 4.6 | 0.51 | same |
| explore | stage car renderers ahead | 3.4 | 3.2 | -0.1 [-0.5..+0.9] | 0.3 | 2.0 | 0.63 | same |
| explore | stage warm: car renderer | 6.2 | 6.0 | -0.1 [-1.4..+1.4] | 1.3 | 2.7 | 0.57 | same |
| explore | stage drive_in: car renderer | 3.1 | 3.0 | -0.1 [-0.2..+0.0] | 0.1 | 2.0 | 0.95 | same |
| explore | stage Car::load_tuned: part database | 1.9 | 1.6 | -0.1 [-0.5..+0.4] | 0.4 | 2.0 | 0.78 | same |
| explore | stage warm: post chain | 3.4 | 2.8 | -0.2 [-1.4..+1.1] | 1.4 | 2.8 | 0.63 | same |
| explore | stage Encounters::new | 22.3 | 22.0 | -0.2 [-2.5..+1.6] | 1.1 | 2.2 | 0.63 | same |
| explore | stage warm: world renderer | 8.0 | 7.7 | -0.3 [-1.5..+0.9] | 1.2 | 2.5 | 0.66 | same |
| explore | stage WorldRenderer::new: pipelines | 18.5 | 17.5 | -0.7 [-4.7..+1.7] | 2.7 | 5.5 | 0.76 | same |
| explore | stage Car::load_tuned: solids, textures | 26.1 | 25.5 | -0.8 [-3.6..+2.1] | 2.2 | 4.4 | 0.71 | same |
| explore | stage cache: open | 28.8 | 25.7 | -2.6 [-5.5..+0.6] | 2.0 | 4.1 | 0.96 | same |
| explore | stage WorldRenderer::new | 90.1 | 85.9 | -3.5 [-13.2..+6.1] | 5.3 | 10.7 | 0.71 | same |
| explore | stage Car::load_tuned: shared parts and the rest | 32.7 | 24.7 | -5.7 [-12.6..+1.4] | 6.0 | 11.9 | 0.93 | same |
| explore | stage Car::load_tuned | 71.4 | 62.1 | -7.4 [-18.8..+1.2] | 10.0 | 20.0 | 0.96 | same |

loadgate: warm: SLOWER: totals: explore load +20.3 ms, explore load + first frame +20.0 ms; slower stage: explore World::load: from the cache +18.2 ms (+118 %), explore drive_in +19.6 ms (+15 %), explore World::load +18.3 ms (+82 %)
loadgate: warm: slower, not proven (repeat with more runs): boot stage World::load: from the cache +9.4 ms (p 0.00075), boot stage World::load +9.0 ms (p 0.00075)
loadgate: cold: same (boot start -> entry 64 -> 72 ms; boot start -> first frame 118 -> 130 ms; boot start -> title 260 -> 237 ms; explore load 324 -> 336 ms; explore first frame 8 -> 11 ms; explore load + first frame 333 -> 348 ms)
loadgate: cold: slower, not proven (repeat with more runs): boot stage Car::load_tuned: shared parts and the rest +30.7 ms (p 0.0074), boot stage Car::load_tuned +95.8 ms (p 0.012), explore stage World::load: from the cache +19.9 ms (p 0.00053)
loadgate: cores: SLOWER: totals: no total beyond its limit; slower stage: explore World::load: from the cache +19.8 ms (+144 %)
loadgate: cores: slower, not proven (repeat with more runs): boot stage World::load: from the cache +16.2 ms (p 0.056), boot stage World::load +15.3 ms (p 0.056), explore stage World::load +17.9 ms (p 0.00075), explore stage drive_in +15.4 ms (p 0.0026)
LOADGATE: FAIL: slower: warm: explore World::load: from the cache; cores: explore World::load: from the cache (9 not proven)
