<!-- loadgate.sh run, 10 runs per arm, mode and scene; the bench's loadgate.log lines: -->
bench 2026-11-19T19:22:24+01:00 reps 10 modes warm cold cores scenes boot explore frames 20 low cpus 0-3 cold root copy; loadavg 8.04 11.14 16.24
arm base <out>/bin/base f6f213bb8b201706 assets original
arm new <out>/bin/new 75a440fbec8d2c3a assets original
rep 1 done 2026-11-19T19:23:50+01:00 loadavg 17.91 13.32 16.55
rep 2 waited 120 s for the load average under 12
rep 2 done 2026-11-19T19:28:26+01:00 loadavg 12.74 13.70 16.04
rep 3 waited 120 s for the load average under 12
rep 3 done 2026-11-19T19:31:14+01:00 loadavg 13.78 16.55 16.95
rep 4 waited 80 s for the load average under 12
rep 4 done 2026-11-19T19:33:29+01:00 loadavg 21.65 17.51 17.18
rep 5 waited 80 s for the load average under 12
rep 5 done 2026-11-19T19:35:53+01:00 loadavg 10.68 14.11 15.92
rep 6 done 2026-11-19T19:36:32+01:00 loadavg 17.41 15.48 16.31
rep 7 waited 120 s for the load average under 12
rep 7 done 2026-11-19T19:39:33+01:00 loadavg 12.08 14.47 15.81
rep 8 waited 120 s for the load average under 12
rep 8 done 2026-11-19T19:43:05+01:00 loadavg 14.62 15.26 15.96
rep 9 waited 120 s for the load average under 12
rep 9 done 2026-11-19T19:46:15+01:00 loadavg 14.19 16.97 16.64
rep 10 waited 100 s for the load average under 12
rep 10 done 2026-11-19T19:48:42+01:00 loadavg 9.47 14.59 15.85

# Load-speed gate: base (base) vs new (new)

## warm (69 measures; p threshold 0.01/69)

| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |
|---|---|---|---|---|---|---|---|---|
| boot | start -> first frame | 86.0 | 78.5 | +4.0 [-70.0..+223.0] | 30.4 | 60.8 | 0.43 | same |
| boot | start -> entry | 45.0 | 46.0 | +1.0 [-66.0..+37.0] | 21.5 | 43.0 | 0.49 | same |
| boot | start -> title | 114.0 | 105.5 | -6.0 [-80.0..+227.0] | 25.2 | 50.4 | 0.7 | same |
| boot | stage World::load | 11.7 | 44.5 | +21.2 [+0.0..+51.3] | 24.7 | 49.5 | 0.05 | same |
| boot | stage Car::load_tuned: solids, textures | 15.9 | 46.1 | +16.0 [-14.7..+57.1] | 33.3 | 66.7 | 0.15 | same |
| boot | stage World::load: from the cache | 9.1 | 35.4 | +14.8 [+0.0..+36.9] | 13.4 | 26.9 | 0.043 | same |
| boot | stage Game::new | 31.6 | 32.2 | +0.9 [-25.9..+19.5] | 13.6 | 27.2 | 0.46 | same |
| boot | stage cache: open | 63.8 | 56.0 | +0.2 [-37.0..+44.1] | 30.3 | 60.6 | 0.49 | same |
| boot | stage Car::load_tuned: shared parts and the rest | 5.4 | 1.8 | +0.0 [-10.2..+11.6] | 8.0 | 16.1 | 0.58 | same |
| boot | stage Car::load_tuned | 28.6 | 13.1 | +0.0 [-29.7..+44.8] | 32.2 | 64.4 | 0.52 | same |
| boot | stage preload car: renderer | 1.4 | 0.6 | +0.0 [-2.2..+3.1] | 1.7 | 3.4 | 0.55 | same |
| boot | stage collision ahead | 3.9 | 4.2 | +0.0 [-7.7..+8.4] | 6.2 | 12.5 | 0.4 | same |
| boot | stage programs ahead | 15.9 | 13.7 | -0.3 [-7.3..+5.7] | 6.4 | 12.7 | 0.54 | same |
| explore | load + first frame | 344.9 | 285.1 | +18.6 [-169.0..+167.0] | 157.7 | 315.4 | 0.43 | same |
| explore | load | 333.7 | 263.6 | +14.6 [-163.0..+175.2] | 168.7 | 337.4 | 0.4 | same |
| explore | first frame | 11.6 | 12.8 | +0.4 [-6.7..+6.6] | 5.8 | 11.7 | 0.46 | same |
| explore | stage World::load: from the cache | 17.8 | 35.0 | +18.5 [+12.8..+20.3] | 2.7 | 5.4 | 5.4e-06 | SLOWER |
| explore | stage World::load | 46.3 | 44.2 | +13.0 [-26.2..+27.9] | 10.6 | 21.3 | 0.29 | same |
| explore | stage WorldRenderer::new: draws and textures | 43.5 | 57.9 | +10.8 [-6.3..+28.3] | 15.0 | 30.1 | 0.083 | same |
| explore | stage WorldRenderer::new | 101.0 | 107.6 | +7.0 [-19.2..+33.5] | 29.7 | 59.5 | 0.14 | same |
| explore | stage WorldRenderer::new: texture arrays | 24.4 | 31.3 | +6.5 [-4.4..+19.3] | 11.2 | 22.5 | 0.072 | same |
| explore | stage WorldRenderer::new: texture arrays: upload | 24.2 | 31.1 | +6.4 [-4.5..+19.3] | 11.3 | 22.5 | 0.072 | same |
| explore | stage drive_in | 262.0 | 211.0 | +1.8 [-119.1..+112.3] | 99.8 | 199.5 | 0.51 | same |
| explore | stage WorldRenderer::new: groups and draws | 9.5 | 11.6 | +1.1 [-3.0..+5.3] | 2.5 | 5.1 | 0.32 | same |
| explore | stage Encounters: cars joined | 14.7 | 12.1 | +0.7 [-24.8..+76.9] | 8.2 | 16.5 | 0.37 | same |
| explore | stage Car::load_tuned: part database | 3.9 | 3.7 | +0.5 [-2.2..+2.1] | 2.3 | 4.6 | 0.37 | same |
| explore | stage Encounters::new: picks | 2.2 | 3.2 | +0.4 [-0.8..+1.6] | 0.8 | 2.0 | 0.16 | same |
| explore | stage warm: world renderer | 4.1 | 5.0 | +0.3 [-5.4..+3.0] | 1.6 | 3.1 | 0.32 | same |
| explore | stage drive_in: warm | 9.9 | 11.0 | +0.2 [-6.6..+3.3] | 2.3 | 4.6 | 0.46 | same |
| explore | stage WorldRenderer::new: pipelines | 12.0 | 12.4 | +0.1 [-2.6..+2.0] | 1.3 | 2.5 | 0.46 | same |
| explore | stage WorldRenderer::new: solid spheres | 0.9 | 1.3 | +0.1 [-0.5..+1.1] | 0.6 | 2.0 | 0.34 | same |
| explore | stage drive_in: uploads | 2.4 | 3.8 | +0.1 [-2.6..+4.8] | 3.6 | 7.2 | 0.46 | same |
| explore | stage drive_in: car renderer | 3.3 | 3.3 | +0.0 [-0.5..+0.9] | 0.4 | 2.0 | 0.4 | same |
| explore | stage car renderers ahead | 3.4 | 3.7 | +0.0 [-1.3..+1.2] | 0.7 | 2.0 | 0.49 | same |
| explore | stage warm: post chain | 2.4 | 2.2 | -0.2 [-0.3..-0.0] | 0.1 | 2.0 | 0.99 | same |
| explore | stage warm: car renderer | 5.2 | 5.2 | -0.3 [-3.2..+0.8] | 0.8 | 2.0 | 0.74 | same |
| explore | stage collision ahead | 8.0 | 7.5 | -0.4 [-2.5..+0.6] | 0.7 | 2.0 | 0.8 | same |
| explore | stage WorldRenderer::new: buffers, uniforms | 8.0 | 7.0 | -0.8 [-3.2..+0.2] | 1.1 | 2.1 | 0.96 | same |
| explore | stage Encounters::new | 32.1 | 27.2 | -1.2 [-9.7..+4.7] | 6.0 | 12.1 | 0.6 | same |
| explore | stage Car::load_tuned: shared parts and the rest | 28.4 | 25.9 | -1.5 [-53.2..+11.9] | 8.6 | 17.3 | 0.68 | same |
| explore | stage Car::load_tuned: solids, textures | 64.7 | 36.2 | -4.6 [-198.9..+174.4] | 44.6 | 89.3 | 0.84 | same |
| explore | stage Game::new | 29.5 | 23.0 | -5.0 [-14.2..+0.9] | 4.3 | 8.6 | 0.94 | same |
| explore | stage Car::load_tuned | 175.5 | 79.2 | -6.0 [-207.3..+319.8] | 152.4 | 304.8 | 0.66 | same |
| explore | stage cache: open | 51.0 | 27.7 | -6.1 [-425.1..+191.7] | 33.0 | 66.0 | 0.86 | same |

## cold (56 measures; p threshold 0.01/56)

| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |
|---|---|---|---|---|---|---|---|---|
| boot | start -> entry | 146.5 | 160.0 | +4.5 [-87.0..+77.0] | 81.9 | 163.8 | 0.49 | same |
| boot | start -> first frame | 266.0 | 241.0 | -5.0 [-125.0..+96.0] | 109.7 | 219.4 | 0.62 | same |
| boot | start -> title | 491.5 | 505.5 | -15.0 [-158.0..+115.0] | 151.2 | 302.5 | 0.66 | same |
| boot | stage Game::new | 70.7 | 70.4 | +3.1 [-37.5..+43.4] | 32.1 | 64.2 | 0.34 | same |
| boot | stage programs ahead | 17.0 | 15.7 | -0.1 [-4.6..+8.1] | 4.3 | 10.0 | 0.54 | same |
| boot | stage cache: open | 467.6 | 276.6 | -59.0 [-418.3..+48.0] | 284.8 | 569.7 | 0.8 | same |
| explore | first frame | 21.1 | 15.4 | -3.3 [-8.6..+1.2] | 5.4 | 20.0 | 0.92 | same |
| explore | load | 685.1 | 652.8 | -10.0 [-218.6..+132.7] | 156.7 | 313.5 | 0.6 | same |
| explore | load + first frame | 703.3 | 665.5 | -13.3 [-227.2..+125.7] | 158.1 | 316.3 | 0.63 | same |
| explore | stage World::load: from the cache | 17.1 | 39.7 | +20.5 [+16.2..+26.6] | 4.1 | 10.0 | 5.4e-06 | SLOWER |
| explore | stage Car::load_tuned: shared parts and the rest | 261.3 | 240.0 | +9.4 [-161.0..+135.8] | 125.6 | 251.2 | 0.49 | same |
| explore | stage Encounters: cars joined | 96.2 | 98.8 | +8.5 [-47.7..+52.5] | 40.6 | 81.1 | 0.4 | same |
| explore | stage WorldRenderer::new: texture arrays | 26.5 | 28.6 | +6.4 [-4.8..+18.1] | 7.9 | 15.7 | 0.11 | same |
| explore | stage Game::new | 64.2 | 69.0 | +6.1 [-16.3..+38.5] | 26.6 | 53.3 | 0.34 | same |
| explore | stage WorldRenderer::new: texture arrays: upload | 25.8 | 26.7 | +6.0 [-4.8..+16.1] | 10.1 | 20.1 | 0.16 | same |
| explore | stage World::load | 328.0 | 332.2 | +1.6 [-79.2..+80.0] | 73.9 | 147.9 | 0.49 | same |
| explore | stage Encounters::new | 27.7 | 30.5 | +1.5 [-3.6..+13.0] | 4.8 | 10.0 | 0.22 | same |
| explore | stage explore Traffic::load | 1.4 | 2.2 | +0.9 [-0.0..+1.9] | 0.7 | 10.0 | 0.038 | same |
| explore | stage drive_in: warm | 12.0 | 12.9 | +0.8 [-2.6..+4.8] | 2.4 | 10.0 | 0.4 | same |
| explore | stage collision ahead | 7.9 | 8.5 | +0.4 [-0.4..+1.9] | 0.8 | 10.0 | 0.14 | same |
| explore | stage Encounters::new: picks | 2.1 | 2.7 | +0.3 [-0.3..+1.1] | 0.6 | 10.0 | 0.32 | same |
| explore | stage warm: world renderer | 4.9 | 5.5 | +0.3 [-1.7..+2.8] | 1.3 | 10.0 | 0.37 | same |
| explore | stage car renderers ahead | 3.7 | 4.1 | +0.3 [-0.3..+1.2] | 0.7 | 10.0 | 0.18 | same |
| explore | stage WorldRenderer::new: buffers, uniforms | 8.1 | 8.8 | +0.2 [-1.6..+1.7] | 1.2 | 10.0 | 0.37 | same |
| explore | stage warm: car renderer | 5.4 | 5.5 | -0.0 [-0.6..+0.9] | 0.6 | 10.0 | 0.54 | same |
| explore | stage warm: post chain | 2.5 | 2.3 | -0.1 [-0.5..+1.5] | 0.3 | 10.0 | 0.71 | same |
| explore | stage WorldRenderer::new: solid spheres | 1.2 | 0.9 | -0.2 [-2.3..+0.1] | 0.3 | 10.0 | 0.9 | same |
| explore | stage drive_in: car renderer | 4.5 | 3.8 | -0.4 [-1.8..+0.2] | 1.0 | 10.0 | 0.89 | same |
| explore | stage Car::load_tuned: part database | 3.2 | 2.8 | -0.4 [-1.4..+0.9] | 1.1 | 10.0 | 0.76 | same |
| explore | stage drive_in: uploads | 6.3 | 3.3 | -1.7 [-7.0..+3.6] | 3.8 | 10.0 | 0.78 | same |
| explore | stage WorldRenderer::new: draws and textures | 52.3 | 47.2 | -1.9 [-20.9..+16.4] | 15.5 | 31.0 | 0.66 | same |
| explore | stage WorldRenderer::new: pipelines | 17.6 | 14.4 | -2.2 [-7.6..+2.4] | 4.2 | 10.0 | 0.78 | same |
| explore | stage WorldRenderer::new: groups and draws | 15.5 | 11.1 | -2.6 [-9.0..+3.8] | 5.4 | 10.7 | 0.86 | same |
| explore | stage WorldRenderer::new | 123.7 | 101.2 | -5.4 [-37.3..+21.4] | 27.5 | 55.0 | 0.68 | same |
| explore | stage drive_in | 509.5 | 510.0 | -13.9 [-121.6..+93.9] | 95.9 | 191.7 | 0.63 | same |
| explore | stage Car::load_tuned | 793.6 | 698.3 | -15.7 [-293.1..+187.0] | 219.5 | 439.0 | 0.54 | same |
| explore | stage Car::load_tuned: solids, textures | 506.6 | 495.8 | -19.5 [-140.5..+81.9] | 96.5 | 192.9 | 0.71 | same |
| explore | stage cache: open | 801.6 | 762.0 | -46.9 [-260.0..+150.2] | 174.3 | 348.6 | 0.74 | same |

## cores (69 measures; p threshold 0.01/69)

| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |
|---|---|---|---|---|---|---|---|---|
| boot | start -> title | 133.0 | 135.0 | +8.5 [-42.0..+59.0] | 37.1 | 74.1 | 0.37 | same |
| boot | start -> entry | 43.5 | 47.0 | +3.0 [-4.0..+14.0] | 7.8 | 15.6 | 0.21 | same |
| boot | start -> first frame | 78.0 | 80.0 | +0.0 [-11.0..+18.0] | 16.3 | 32.6 | 0.52 | same |
| boot | stage World::load | 33.5 | 53.9 | +18.6 [-7.9..+29.8] | 9.6 | 19.2 | 0.07 | same |
| boot | stage World::load: from the cache | 25.3 | 43.5 | +18.2 [+1.8..+28.7] | 7.5 | 15.1 | 0.013 | slower, not proven |
| boot | stage Game::new | 32.3 | 33.7 | +1.7 [-4.5..+10.5] | 6.6 | 13.1 | 0.4 | same |
| boot | stage Car::load_tuned: solids, textures | 53.5 | 58.8 | +0.0 [-56.3..+32.7] | 35.9 | 71.7 | 0.58 | same |
| boot | stage Car::load_tuned: shared parts and the rest | 14.0 | 17.7 | +0.0 [-24.9..+30.4] | 24.9 | 49.7 | 0.52 | same |
| boot | stage Car::load_tuned | 78.7 | 81.3 | +0.0 [-79.7..+65.0] | 53.2 | 106.4 | 0.45 | same |
| boot | stage preload car: renderer | 1.3 | 1.6 | +0.0 [-1.4..+1.5] | 2.0 | 4.0 | 0.55 | same |
| boot | stage WorldRenderer::new: solid spheres | 2.4 | 1.1 | -2.1 [-2.4..+0.4] | 1.6 | 3.2 | 0.94 | same |
| boot | stage collision ahead | 9.6 | 8.3 | -2.1 [-7.5..+0.9] | 2.5 | 5.1 | 0.92 | same |
| boot | stage cache: open | 75.9 | 74.4 | -3.4 [-40.7..+26.5] | 25.6 | 51.2 | 0.54 | same |
| boot | stage programs ahead | 16.9 | 13.7 | -4.3 [-11.1..+3.3] | 5.7 | 11.4 | 0.94 | same |
| boot | stage WorldRenderer::new: groups and draws | 23.0 | 0.0 | -13.9 [-26.6..+0.0] | 6.1 | 12.1 | 0.97 | same |
| explore | load | 245.5 | 267.4 | +28.6 [-11.7..+67.3] | 36.6 | 73.2 | 0.16 | same |
| explore | load + first frame | 253.3 | 278.2 | +24.0 [-22.9..+67.1] | 33.7 | 67.3 | 0.14 | same |
| explore | first frame | 16.9 | 10.5 | -3.4 [-11.7..+3.3] | 7.4 | 14.9 | 0.93 | same |
| explore | stage drive_in | 180.7 | 217.5 | +30.8 [-7.8..+67.0] | 29.9 | 59.9 | 0.083 | same |
| explore | stage World::load | 26.6 | 49.7 | +23.1 [+14.4..+32.7] | 9.3 | 18.6 | 0.00075 | slower, not proven |
| explore | stage World::load: from the cache | 16.1 | 37.1 | +20.6 [+16.7..+24.9] | 5.0 | 10.1 | 5.4e-06 | SLOWER |
| explore | stage cache: open | 31.0 | 40.7 | +8.8 [+0.4..+20.4] | 4.5 | 8.9 | 0.022 | same |
| explore | stage WorldRenderer::new | 105.0 | 116.6 | +5.7 [-15.9..+32.1] | 23.8 | 47.6 | 0.34 | same |
| explore | stage WorldRenderer::new: draws and textures | 45.3 | 49.5 | +4.7 [-8.6..+21.1] | 9.5 | 19.1 | 0.26 | same |
| explore | stage WorldRenderer::new: texture arrays | 22.0 | 27.5 | +4.1 [-4.3..+18.5] | 7.8 | 15.7 | 0.16 | same |
| explore | stage Car::load_tuned: solids, textures | 32.1 | 36.2 | +3.7 [-4.6..+13.3] | 7.5 | 14.9 | 0.2 | same |
| explore | stage WorldRenderer::new: texture arrays: upload | 21.7 | 23.2 | +1.3 [-6.8..+12.2] | 8.4 | 16.8 | 0.4 | same |
| explore | stage WorldRenderer::new: pipelines | 19.8 | 19.5 | +0.8 [-1.8..+4.3] | 2.5 | 5.1 | 0.2 | same |
| explore | stage Encounters: cars joined | 11.3 | 12.6 | +0.6 [-4.6..+3.8] | 3.5 | 7.1 | 0.43 | same |
| explore | stage Car::load_tuned | 79.5 | 81.6 | +0.4 [-22.2..+17.1] | 15.8 | 31.6 | 0.46 | same |
| explore | stage warm: car renderer | 6.1 | 6.8 | +0.4 [-1.2..+1.8] | 1.6 | 3.3 | 0.34 | same |
| explore | stage Encounters::new: picks | 2.7 | 3.6 | +0.3 [-0.5..+1.6] | 0.9 | 2.0 | 0.22 | same |
| explore | stage WorldRenderer::new: buffers, uniforms | 7.1 | 7.3 | +0.2 [-0.6..+0.8] | 0.3 | 2.0 | 0.18 | same |
| explore | stage car renderers ahead | 3.9 | 4.0 | +0.1 [-1.2..+1.8] | 1.0 | 2.0 | 0.37 | same |
| explore | stage WorldRenderer::new: groups and draws | 13.7 | 13.5 | +0.0 [-2.2..+1.7] | 1.5 | 2.9 | 0.51 | same |
| explore | stage WorldRenderer::new: solid spheres | 1.1 | 1.2 | -0.0 [-1.0..+0.3] | 0.3 | 2.0 | 0.57 | same |
| explore | stage collision ahead | 9.7 | 8.7 | -0.2 [-1.9..+1.2] | 1.3 | 2.6 | 0.63 | same |
| explore | stage Encounters::new | 30.6 | 29.7 | -0.2 [-7.8..+7.4] | 9.4 | 18.7 | 0.54 | same |
| explore | stage drive_in: warm | 15.1 | 14.1 | -0.3 [-4.6..+2.4] | 3.2 | 6.4 | 0.54 | same |
| explore | stage warm: post chain | 3.8 | 3.3 | -0.4 [-1.7..+0.6] | 0.8 | 2.0 | 0.78 | same |
| explore | stage drive_in: car renderer | 4.2 | 3.5 | -0.5 [-1.4..+0.5] | 0.7 | 2.0 | 0.89 | same |
| explore | stage Car::load_tuned: part database | 3.3 | 2.8 | -0.5 [-3.3..+0.8] | 1.4 | 2.8 | 0.76 | same |
| explore | stage warm: world renderer | 8.6 | 8.6 | -0.5 [-3.8..+2.1] | 1.9 | 3.7 | 0.76 | same |
| explore | stage drive_in: uploads | 4.2 | 3.2 | -1.1 [-4.6..+2.9] | 3.5 | 7.0 | 0.76 | same |
| explore | stage Car::load_tuned: shared parts and the rest | 33.4 | 30.2 | -2.1 [-9.0..+4.5] | 6.5 | 13.1 | 0.74 | same |
| explore | stage Game::new | 33.7 | 29.8 | -2.5 [-7.6..+1.7] | 5.8 | 11.6 | 0.86 | same |

loadgate: warm: SLOWER: totals: no total beyond its limit; slower stage: explore World::load: from the cache +18.5 ms (+104 %)
loadgate: cold: SLOWER: totals: no total beyond its limit; slower stage: explore World::load: from the cache +20.5 ms (+120 %)
loadgate: cores: SLOWER: totals: no total beyond its limit; slower stage: explore World::load: from the cache +20.6 ms (+128 %)
loadgate: cores: slower, not proven (repeat with more runs): boot stage World::load: from the cache +18.2 ms (p 0.013), explore stage World::load +23.1 ms (p 0.00075)
LOADGATE: FAIL: slower: warm: explore World::load: from the cache; cold: explore World::load: from the cache; cores: explore World::load: from the cache (2 not proven)
