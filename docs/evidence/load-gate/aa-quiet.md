<!-- loadgate.sh run, 10 runs per arm, mode and scene; the bench's loadgate.log lines: -->
bench 2026-11-19T20:07:25+01:00 reps 10 modes warm cold cores scenes boot explore frames 20 low cpus 0-3 cold root copy; loadavg 5.64 6.17 9.25
arm base <out>/bin/base f6f213bb8b201706 assets original
arm base2 <out>/bin/base2 f6f213bb8b201706 assets original
rep 1 done 2026-11-19T20:08:35+01:00 loadavg 10.52 7.60 9.52
rep 2 done 2026-11-19T20:09:13+01:00 loadavg 8.49 7.46 9.40
rep 3 done 2026-11-19T20:09:57+01:00 loadavg 8.56 7.48 9.30
rep 4 done 2026-11-19T20:10:37+01:00 loadavg 11.63 8.50 9.57
rep 5 done 2026-11-19T20:11:25+01:00 loadavg 12.23 9.15 9.75
rep 6 waited 10 s for the load average under 12
rep 6 done 2026-11-19T20:12:11+01:00 loadavg 7.82 8.40 9.46
rep 7 done 2026-11-19T20:12:39+01:00 loadavg 6.28 7.97 9.29
rep 8 done 2026-11-19T20:12:55+01:00 loadavg 5.92 7.81 9.21
rep 9 done 2026-11-19T20:13:12+01:00 loadavg 5.78 7.69 9.15
rep 10 done 2026-11-19T20:13:29+01:00 loadavg 7.37 7.97 9.21

# Load-speed gate: base (base) vs base2 (new)

## warm (65 measures; p threshold 0.01/65)

| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |
|---|---|---|---|---|---|---|---|---|
| boot | start -> first frame | 67.0 | 68.0 | +1.0 [-5.0..+8.0] | 5.9 | 11.9 | 0.31 | same |
| boot | start -> entry | 33.0 | 33.0 | +0.0 [-4.0..+5.0] | 3.0 | 5.9 | 0.44 | same |
| boot | start -> title | 84.5 | 87.0 | +0.0 [-8.0..+8.0] | 6.3 | 12.6 | 0.5 | same |
| boot | stage cache: open | 21.7 | 26.4 | +2.7 [-3.6..+22.3] | 5.2 | 10.4 | 0.22 | same |
| boot | stage Car::load_tuned: solids, textures | 22.6 | 24.8 | +2.5 [-4.1..+13.4] | 5.3 | 10.5 | 0.26 | same |
| boot | stage collision ahead | 7.6 | 8.2 | +0.2 [-2.2..+2.2] | 0.6 | 2.0 | 0.38 | same |
| boot | stage World::load: from the cache | 21.4 | 22.1 | +0.1 [-8.3..+7.5] | 2.8 | 5.5 | 0.47 | same |
| boot | stage Game::new | 22.0 | 22.5 | +0.0 [-2.0..+2.8] | 2.0 | 3.9 | 0.49 | same |
| boot | stage garage ahead | 12.4 | 12.7 | +0.0 [-12.0..+13.0] | 18.4 | 36.8 | 0.48 | same |
| boot | stage WorldRenderer::new: groups and draws | 8.2 | 8.2 | +0.0 [-8.0..+8.3] | 4.1 | 8.2 | 0.48 | same |
| boot | stage World::load | 28.6 | 28.9 | -0.1 [-10.6..+11.1] | 3.7 | 7.4 | 0.56 | same |
| boot | stage preload car: renderer | 3.9 | 3.6 | -0.1 [-3.0..+2.1] | 2.5 | 4.9 | 0.57 | same |
| boot | stage programs ahead | 11.2 | 10.8 | -0.5 [-4.5..+1.2] | 1.8 | 3.5 | 0.71 | same |
| boot | stage Car::load_tuned | 38.9 | 34.0 | -0.8 [-20.1..+15.4] | 17.2 | 34.5 | 0.68 | same |
| boot | stage Car::load_tuned: shared parts and the rest | 16.8 | 12.1 | -3.0 [-14.3..+4.5] | 7.8 | 15.7 | 0.8 | same |
| explore | first frame | 8.6 | 9.8 | +0.4 [-3.0..+3.9] | 3.1 | 6.3 | 0.32 | same |
| explore | load + first frame | 176.5 | 171.6 | -3.6 [-26.2..+11.5] | 10.0 | 20.0 | 0.76 | same |
| explore | load | 168.0 | 162.0 | -3.6 [-28.4..+6.9] | 8.8 | 17.6 | 0.8 | same |
| explore | stage Car::load_tuned: solids, textures | 30.0 | 31.0 | +0.9 [-4.7..+4.0] | 2.6 | 5.2 | 0.29 | same |
| explore | stage collision ahead | 7.5 | 7.9 | +0.3 [-1.0..+0.9] | 0.5 | 2.0 | 0.2 | same |
| explore | stage warm: world renderer | 3.9 | 4.1 | +0.2 [-0.7..+0.9] | 0.6 | 2.0 | 0.34 | same |
| explore | stage WorldRenderer::new: pipelines | 11.4 | 11.6 | +0.1 [-0.6..+0.7] | 0.6 | 2.0 | 0.43 | same |
| explore | stage warm: car renderer | 5.0 | 5.1 | +0.1 [-0.4..+0.7] | 0.4 | 2.0 | 0.37 | same |
| explore | stage Encounters::new: picks | 2.1 | 2.1 | +0.1 [-0.3..+0.5] | 0.3 | 2.0 | 0.4 | same |
| explore | stage WorldRenderer::new: groups and draws | 9.3 | 9.4 | +0.0 [-1.3..+1.5] | 1.5 | 3.0 | 0.49 | same |
| explore | stage Encounters: cars joined | 9.7 | 9.7 | +0.0 [-2.3..+1.3] | 0.4 | 2.0 | 0.43 | same |
| explore | stage drive_in: car renderer | 3.0 | 3.1 | +0.0 [-0.1..+0.2] | 0.1 | 2.0 | 0.43 | same |
| explore | stage warm: post chain | 2.4 | 2.4 | +0.0 [-0.3..+0.4] | 0.3 | 2.0 | 0.51 | same |
| explore | stage WorldRenderer::new: buffers, uniforms | 6.8 | 6.7 | -0.0 [-0.5..+0.5] | 0.5 | 2.0 | 0.6 | same |
| explore | stage drive_in: warm | 9.7 | 9.4 | -0.1 [-1.1..+0.9] | 0.8 | 2.0 | 0.57 | same |
| explore | stage car renderers ahead | 3.4 | 3.2 | -0.1 [-0.9..+0.3] | 0.4 | 2.0 | 0.71 | same |
| explore | stage World::load | 22.2 | 21.9 | -0.2 [-2.3..+1.8] | 1.6 | 3.3 | 0.66 | same |
| explore | stage cache: open | 26.3 | 26.1 | -0.3 [-3.4..+3.0] | 2.3 | 4.5 | 0.54 | same |
| explore | stage Game::new | 22.3 | 22.2 | -0.4 [-1.6..+1.2] | 1.0 | 2.1 | 0.71 | same |
| explore | stage Encounters::new | 23.2 | 22.5 | -0.4 [-2.5..+1.5] | 1.3 | 2.5 | 0.74 | same |
| explore | stage World::load: from the cache | 15.6 | 15.4 | -0.4 [-1.8..+0.9] | 1.2 | 2.3 | 0.76 | same |
| explore | stage Car::load_tuned: part database | 3.4 | 2.9 | -0.5 [-1.7..+0.3] | 0.5 | 2.0 | 0.95 | same |
| explore | stage Car::load_tuned: shared parts and the rest | 21.8 | 21.3 | -0.6 [-3.2..+1.6] | 2.3 | 4.5 | 0.84 | same |
| explore | stage WorldRenderer::new: texture arrays | 17.7 | 16.0 | -1.4 [-8.1..+3.7] | 4.7 | 9.4 | 0.82 | same |
| explore | stage WorldRenderer::new: texture arrays: upload | 17.5 | 15.8 | -1.4 [-8.1..+3.7] | 4.7 | 9.4 | 0.82 | same |
| explore | stage WorldRenderer::new | 77.0 | 75.0 | -1.5 [-8.4..+5.3] | 6.1 | 12.1 | 0.71 | same |
| explore | stage drive_in: uploads | 3.4 | 1.5 | -1.8 [-4.5..+0.3] | 2.0 | 4.0 | 0.93 | same |
| explore | stage WorldRenderer::new: draws and textures | 33.8 | 31.1 | -1.8 [-8.1..+5.1] | 5.7 | 11.5 | 0.76 | same |
| explore | stage Car::load_tuned | 63.4 | 61.9 | -2.1 [-8.4..+2.9] | 3.9 | 7.8 | 0.71 | same |
| explore | stage drive_in | 126.6 | 123.1 | -2.7 [-28.7..+7.5] | 9.5 | 19.1 | 0.76 | same |

## cold (60 measures; p threshold 0.01/60)

| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |
|---|---|---|---|---|---|---|---|---|
| boot | start -> title | 330.0 | 368.5 | +48.5 [-56.0..+159.0] | 143.1 | 286.1 | 0.14 | same |
| boot | start -> first frame | 123.0 | 130.5 | +9.0 [-10.0..+133.0] | 24.5 | 48.9 | 0.081 | same |
| boot | start -> entry | 60.0 | 68.5 | +7.0 [-4.0..+57.0] | 12.2 | 24.5 | 0.041 | same |
| boot | stage cache: open | 301.4 | 427.1 | +6.9 [-98.8..+318.6] | 249.8 | 499.5 | 0.43 | same |
| boot | stage Car::load_tuned: solids, textures | 209.5 | 226.6 | +4.2 [-126.2..+168.6] | 125.1 | 250.3 | 0.47 | same |
| boot | stage programs ahead | 15.0 | 17.3 | +2.0 [-2.9..+13.8] | 5.4 | 10.7 | 0.18 | same |
| boot | stage Game::new | 35.8 | 37.2 | +1.2 [-4.5..+26.0] | 5.1 | 10.2 | 0.29 | same |
| boot | stage World::load: from the cache | 0.0 | 7.0 | +0.0 [-1.5..+16.2] | 10.3 | 20.6 | 0.3 | same |
| boot | stage World::load | 0.0 | 61.3 | +0.0 [-112.3..+127.5] | 91.0 | 181.9 | 0.35 | same |
| boot | stage collision ahead | 0.0 | 3.7 | +0.0 [-0.1..+7.8] | 5.5 | 11.0 | 0.22 | same |
| explore | first frame | 9.6 | 9.7 | +1.5 [-3.0..+5.2] | 4.3 | 20.0 | 0.24 | same |
| explore | load + first frame | 385.5 | 429.0 | -8.9 [-157.4..+133.4] | 163.5 | 327.0 | 0.68 | same |
| explore | load | 377.4 | 416.7 | -10.1 [-150.8..+129.9] | 154.1 | 308.2 | 0.66 | same |
| explore | stage Encounters: cars joined | 30.5 | 36.4 | +3.1 [-2.7..+12.1] | 6.8 | 13.6 | 0.2 | same |
| explore | stage WorldRenderer::new: texture arrays | 21.0 | 23.1 | +1.6 [-8.1..+8.7] | 7.0 | 14.0 | 0.34 | same |
| explore | stage WorldRenderer::new: draws and textures | 37.4 | 37.7 | +1.4 [-20.7..+11.8] | 7.7 | 15.4 | 0.4 | same |
| explore | stage Car::load_tuned | 336.7 | 397.1 | +1.3 [-116.6..+157.7] | 119.1 | 238.2 | 0.49 | same |
| explore | stage Game::new | 38.1 | 38.7 | +1.2 [-15.0..+7.1] | 6.5 | 13.0 | 0.37 | same |
| explore | stage explore Traffic::load | 1.2 | 2.0 | +0.7 [-0.2..+3.3] | 0.6 | 10.0 | 0.11 | same |
| explore | stage WorldRenderer::new: texture arrays: upload | 20.3 | 22.4 | +0.6 [-8.6..+7.4] | 6.8 | 13.5 | 0.43 | same |
| explore | stage Car::load_tuned: part database | 3.0 | 3.3 | +0.3 [-0.6..+1.6] | 1.0 | 10.0 | 0.32 | same |
| explore | stage Encounters::new | 23.9 | 24.4 | +0.2 [-16.7..+4.7] | 2.1 | 10.0 | 0.43 | same |
| explore | stage collision ahead | 7.8 | 7.6 | +0.1 [-1.7..+1.2] | 0.8 | 10.0 | 0.32 | same |
| explore | stage warm: post chain | 2.3 | 2.4 | +0.1 [-0.3..+0.5] | 0.3 | 10.0 | 0.32 | same |
| explore | stage WorldRenderer::new: buffers, uniforms | 7.4 | 7.6 | +0.1 [-0.5..+1.5] | 0.7 | 10.0 | 0.46 | same |
| explore | stage drive_in: car renderer | 3.3 | 3.4 | +0.0 [-1.6..+0.4] | 0.3 | 10.0 | 0.49 | same |
| explore | stage car renderers ahead | 3.5 | 3.4 | +0.0 [-1.6..+0.4] | 0.4 | 10.0 | 0.49 | same |
| explore | stage warm: world renderer | 4.1 | 4.2 | -0.1 [-3.7..+0.8] | 1.0 | 10.0 | 0.54 | same |
| explore | stage drive_in: warm | 10.2 | 10.7 | -0.1 [-4.6..+1.3] | 1.1 | 10.0 | 0.54 | same |
| explore | stage warm: car renderer | 5.1 | 4.9 | -0.1 [-1.5..+0.3] | 0.5 | 10.0 | 0.68 | same |
| explore | stage WorldRenderer::new: pipelines | 12.4 | 11.7 | -0.3 [-7.4..+0.3] | 1.2 | 10.0 | 0.82 | same |
| explore | stage Encounters::new: picks | 2.5 | 2.5 | -0.4 [-1.0..+0.4] | 0.6 | 10.0 | 0.86 | same |
| explore | stage World::load: from the cache | 15.7 | 15.2 | -0.4 [-11.5..+1.7] | 1.7 | 10.0 | 0.68 | same |
| explore | stage drive_in: uploads | 3.1 | 2.8 | -0.4 [-2.3..+3.2] | 2.5 | 10.0 | 0.57 | same |
| explore | stage WorldRenderer::new | 84.4 | 85.6 | -0.5 [-41.2..+17.3] | 7.7 | 15.5 | 0.63 | same |
| explore | stage Car::load_tuned: solids, textures | 174.6 | 202.4 | -0.6 [-56.7..+133.2] | 87.7 | 175.3 | 0.51 | same |
| explore | stage Car::load_tuned: shared parts and the rest | 109.9 | 113.9 | -0.8 [-45.0..+22.9] | 25.9 | 51.8 | 0.57 | same |
| explore | stage WorldRenderer::new: groups and draws | 9.5 | 8.6 | -0.9 [-7.3..+0.5] | 1.1 | 10.0 | 0.94 | same |
| explore | stage World::load | 136.3 | 158.9 | -4.8 [-50.8..+91.1] | 85.8 | 171.6 | 0.68 | same |
| explore | stage cache: open | 273.8 | 315.6 | -6.2 [-108.6..+240.2] | 146.8 | 293.7 | 0.6 | same |
| explore | stage drive_in | 310.8 | 331.6 | -11.5 [-137.7..+109.8] | 140.1 | 280.1 | 0.74 | same |

## cores (63 measures; p threshold 0.01/63)

| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |
|---|---|---|---|---|---|---|---|---|
| boot | start -> first frame | 69.5 | 71.0 | +3.0 [-7.0..+12.0] | 9.3 | 18.5 | 0.25 | same |
| boot | start -> title | 107.0 | 103.0 | +1.5 [-14.0..+25.0] | 15.6 | 31.1 | 0.44 | same |
| boot | start -> entry | 39.5 | 42.0 | +1.0 [-6.0..+11.0] | 7.8 | 15.6 | 0.37 | same |
| boot | stage Car::load_tuned: solids, textures | 39.2 | 39.7 | +1.3 [-8.3..+9.5] | 7.0 | 14.0 | 0.43 | same |
| boot | stage programs ahead | 17.0 | 19.4 | +0.3 [-4.2..+4.7] | 2.4 | 4.9 | 0.49 | same |
| boot | stage preload car: renderer | 1.8 | 1.9 | +0.1 [-0.2..+0.6] | 0.3 | 2.0 | 0.29 | same |
| boot | stage cache: open | 46.6 | 48.6 | +0.1 [-9.2..+6.6] | 4.2 | 8.4 | 0.51 | same |
| boot | stage collision ahead | 7.9 | 7.7 | -0.1 [-2.4..+1.1] | 1.0 | 2.0 | 0.57 | same |
| boot | stage WorldRenderer::new: solid spheres | 2.2 | 2.2 | -0.1 [-2.2..+2.1] | 0.5 | 2.0 | 0.66 | same |
| boot | stage Game::new | 27.9 | 28.9 | -0.2 [-5.4..+4.8] | 5.3 | 10.7 | 0.51 | same |
| boot | stage Car::load_tuned: shared parts and the rest | 13.2 | 15.3 | -0.4 [-12.7..+12.6] | 13.8 | 27.7 | 0.68 | same |
| boot | stage Car::load_tuned | 42.0 | 44.1 | -1.6 [-24.3..+14.4] | 11.3 | 22.5 | 0.6 | same |
| boot | stage World::load | 36.2 | 31.0 | -4.4 [-9.9..+0.5] | 4.3 | 8.5 | 0.97 | same |
| boot | stage World::load: from the cache | 25.4 | 20.2 | -4.6 [-8.6..-0.1] | 3.1 | 6.2 | 0.98 | same |
| explore | first frame | 8.4 | 8.2 | +0.2 [-3.1..+3.7] | 3.2 | 6.4 | 0.46 | same |
| explore | load | 183.3 | 183.5 | +0.0 [-19.2..+17.7] | 14.3 | 28.6 | 0.51 | same |
| explore | load + first frame | 193.0 | 191.6 | -0.6 [-23.9..+18.7] | 15.3 | 30.5 | 0.57 | same |
| explore | stage Car::load_tuned: solids, textures | 27.3 | 30.9 | +3.2 [-3.1..+7.6] | 3.0 | 6.1 | 0.11 | same |
| explore | stage Car::load_tuned: shared parts and the rest | 26.9 | 30.2 | +3.0 [-6.3..+8.3] | 5.2 | 10.3 | 0.22 | same |
| explore | stage Car::load_tuned | 68.2 | 70.4 | +2.4 [-8.7..+14.7] | 11.4 | 22.8 | 0.29 | same |
| explore | stage WorldRenderer::new | 85.6 | 86.7 | +2.0 [-11.4..+10.5] | 7.7 | 15.4 | 0.37 | same |
| explore | stage cache: open | 28.2 | 29.9 | +1.4 [-3.0..+8.0] | 2.7 | 5.5 | 0.18 | same |
| explore | stage drive_in | 142.1 | 139.6 | +1.3 [-18.1..+13.4] | 13.2 | 26.5 | 0.46 | same |
| explore | stage WorldRenderer::new: draws and textures | 33.5 | 34.4 | +1.1 [-5.9..+5.5] | 4.2 | 8.5 | 0.32 | same |
| explore | stage WorldRenderer::new: texture arrays | 17.4 | 18.1 | +0.5 [-4.0..+4.1] | 3.4 | 6.7 | 0.46 | same |
| explore | stage WorldRenderer::new: texture arrays: upload | 17.1 | 17.9 | +0.5 [-4.1..+4.1] | 3.4 | 6.8 | 0.46 | same |
| explore | stage WorldRenderer::new: groups and draws | 9.9 | 10.4 | +0.5 [-1.8..+2.3] | 1.3 | 2.6 | 0.32 | same |
| explore | stage WorldRenderer::new: pipelines | 19.6 | 20.0 | +0.4 [-2.8..+4.9] | 3.9 | 7.8 | 0.4 | same |
| explore | stage WorldRenderer::new: buffers, uniforms | 6.8 | 6.6 | +0.2 [-0.5..+0.8] | 0.7 | 2.0 | 0.26 | same |
| explore | stage Encounters::new | 23.5 | 23.9 | +0.2 [-2.7..+7.3] | 3.4 | 6.8 | 0.4 | same |
| explore | stage Encounters::new: picks | 2.5 | 2.5 | +0.1 [-0.4..+0.8] | 0.5 | 2.0 | 0.34 | same |
| explore | stage drive_in: uploads | 1.0 | 1.6 | +0.1 [-1.3..+2.0] | 1.0 | 2.1 | 0.37 | same |
| explore | stage WorldRenderer::new: solid spheres | 1.0 | 1.1 | +0.1 [-0.1..+0.2] | 0.1 | 2.0 | 0.18 | same |
| explore | stage drive_in: car renderer | 3.1 | 3.1 | +0.0 [-0.3..+0.4] | 0.3 | 2.0 | 0.4 | same |
| explore | stage Car::load_tuned: part database | 2.1 | 2.1 | -0.0 [-2.1..+0.8] | 0.7 | 2.0 | 0.51 | same |
| explore | stage collision ahead | 8.3 | 8.1 | -0.2 [-1.2..+1.1] | 1.4 | 2.7 | 0.68 | same |
| explore | stage warm: post chain | 3.4 | 3.1 | -0.2 [-1.7..+1.3] | 1.3 | 2.5 | 0.71 | same |
| explore | stage warm: car renderer | 6.1 | 5.6 | -0.2 [-2.7..+1.2] | 0.9 | 2.0 | 0.78 | same |
| explore | stage Encounters: cars joined | 7.7 | 7.6 | -0.6 [-2.9..+2.1] | 2.0 | 4.0 | 0.66 | same |
| explore | stage World::load: from the cache | 15.1 | 14.6 | -0.7 [-2.8..+1.5] | 2.7 | 5.4 | 0.78 | same |
| explore | stage car renderers ahead | 3.9 | 3.2 | -0.7 [-1.2..+0.1] | 0.3 | 2.0 | 0.96 | same |
| explore | stage drive_in: warm | 14.2 | 11.9 | -0.8 [-3.7..+4.2] | 2.3 | 4.7 | 0.71 | same |
| explore | stage World::load | 24.2 | 23.2 | -0.9 [-3.4..+2.3] | 1.8 | 3.7 | 0.74 | same |
| explore | stage warm: world renderer | 9.6 | 7.3 | -1.5 [-3.5..+2.2] | 1.5 | 3.0 | 0.86 | same |
| explore | stage Game::new | 28.3 | 25.6 | -2.0 [-5.5..+3.9] | 2.5 | 5.0 | 0.86 | same |

loadgate: warm: same (boot start -> entry 33 -> 33 ms; boot start -> first frame 67 -> 68 ms; boot start -> title 84 -> 87 ms; explore load 168 -> 162 ms; explore first frame 9 -> 10 ms; explore load + first frame 177 -> 172 ms)
loadgate: cold: same (boot start -> entry 60 -> 68 ms; boot start -> first frame 123 -> 130 ms; boot start -> title 330 -> 368 ms; explore load 377 -> 417 ms; explore first frame 10 -> 10 ms; explore load + first frame 386 -> 429 ms)
loadgate: cores: same (boot start -> entry 40 -> 42 ms; boot start -> first frame 70 -> 71 ms; boot start -> title 107 -> 103 ms; explore load 183 -> 184 ms; explore first frame 8 -> 8 ms; explore load + first frame 193 -> 192 ms)
LOADGATE: PASS: no load slower beyond its noise limit
