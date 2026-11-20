<!-- loadgate.sh run, 10 runs per arm, mode and scene; the bench's loadgate.log lines: -->
bench 2026-11-19T16:48:14+01:00 reps 10 modes warm cold cores scenes boot explore frames 20 low cpus 0-3 cold root copy; loadavg 29.61 33.36 24.56
arm base <out>/bin/base f6f213bb8b201706 assets original
arm base2 <out>/bin/base2 f6f213bb8b201706 assets original
bench 2026-11-19T17:26:56+01:00 reps 10 modes warm cold cores scenes boot explore frames 20 low cpus 0-3 cold root copy; loadavg 23.58 16.75 17.53
arm base <out>/bin/base f6f213bb8b201706 assets original
arm base2 <out>/bin/base2 f6f213bb8b201706 assets original
rep 1 waited 1800 s for the load average under 12
rep 1 done 2026-11-19T17:58:35+01:00 loadavg 35.33 47.94 54.16
bench 2026-11-19T18:14:03+01:00 reps 10 modes warm cold cores scenes boot explore frames 20 low cpus 0-3 cold root copy; loadavg 36.08 31.89 39.23
arm base <out>/bin/base f6f213bb8b201706 assets original
arm base2 <out>/bin/base2 f6f213bb8b201706 assets original
rep 1 waited 120 s for the load average under 12
rep 1 done 2026-11-19T18:16:09+01:00 loadavg 40.70 35.77 39.77
rep 2 waited 120 s for the load average under 12
rep 2 done 2026-11-19T18:23:05+01:00 loadavg 32.92 37.27 39.67
rep 3 waited 120 s for the load average under 12
rep 3 done 2026-11-19T18:31:19+01:00 loadavg 68.14 50.39 43.75
rep 4 waited 120 s for the load average under 12
rep 4 done 2026-11-19T18:34:25+01:00 loadavg 20.71 37.75 40.21
rep 5 waited 120 s for the load average under 12
rep 5 done 2026-11-19T18:38:38+01:00 loadavg 23.95 28.71 35.67
rep 6 waited 120 s for the load average under 12
rep 6 done 2026-11-19T18:44:45+01:00 loadavg 31.99 27.82 33.02
rep 7 waited 120 s for the load average under 12
rep 7 done 2026-11-19T18:50:00+01:00 loadavg 18.89 24.26 30.13
rep 8 waited 120 s for the load average under 12
bench 2026-11-19T19:19:17+01:00 reps 10 modes warm cold cores scenes boot explore frames 20 low cpus 0-3 cold root copy; loadavg 9.69 13.62 18.00
arm base <out>/bin/base f6f213bb8b201706 assets original
arm base2 <out>/bin/base2 f6f213bb8b201706 assets original
rep 1 done 2026-11-19T19:19:23+01:00 loadavg 10.60 13.74 18.01
rep 2 done 2026-11-19T19:19:23+01:00 loadavg 10.60 13.74 18.01
rep 3 done 2026-11-19T19:19:23+01:00 loadavg 10.60 13.74 18.01
rep 4 done 2026-11-19T19:19:23+01:00 loadavg 10.60 13.74 18.01
rep 5 done 2026-11-19T19:19:23+01:00 loadavg 10.60 13.74 18.01
rep 6 done 2026-11-19T19:19:23+01:00 loadavg 10.60 13.74 18.01
rep 7 done 2026-11-19T19:19:23+01:00 loadavg 10.60 13.74 18.01
rep 8 done 2026-11-19T19:20:12+01:00 loadavg 8.99 12.83 17.48
rep 9 done 2026-11-19T19:20:54+01:00 loadavg 9.32 12.35 17.11
rep 10 done 2026-11-19T19:21:36+01:00 loadavg 7.76 11.62 16.67

# Load-speed gate: base (base) vs base2 (new)

## warm (64 measures; p threshold 0.01/64)

| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |
|---|---|---|---|---|---|---|---|---|
| boot | start -> entry | 135.5 | 179.0 | -5.0 [-180.0..+146.0] | 159.0 | 318.0 | 0.55 | same |
| boot | start -> first frame | 245.5 | 329.5 | -22.5 [-425.0..+239.0] | 275.0 | 550.0 | 0.6 | same |
| boot | start -> title | 479.5 | 432.5 | -41.5 [-495.0..+327.0] | 459.6 | 919.2 | 0.63 | same |
| boot | stage cache: open | 276.2 | 226.1 | +15.5 [-251.6..+368.4] | 284.4 | 568.7 | 0.4 | same |
| boot | stage Game::new | 76.2 | 85.9 | +0.3 [-83.8..+67.3] | 71.6 | 143.3 | 0.51 | same |
| boot | stage Car::load_tuned: solids, textures | 42.4 | 0.0 | +0.0 [-323.2..+60.9] | 62.9 | 125.9 | 0.73 | same |
| boot | stage World::load: from the cache | 0.0 | 12.1 | +0.0 [-18.5..+44.9] | 17.9 | 35.7 | 0.27 | same |
| boot | stage World::load | 0.0 | 27.5 | +0.0 [-21.1..+139.3] | 40.8 | 81.6 | 0.22 | same |
| boot | stage collision ahead | 0.0 | 5.2 | +0.0 [-7.3..+11.7] | 7.8 | 15.5 | 0.32 | same |
| boot | stage exe_tables::build | 1.2 | 0.4 | -0.1 [-1.1..+0.7] | 0.8 | 2.0 | 0.66 | same |
| boot | stage programs ahead | 35.5 | 27.9 | -5.7 [-12.7..+8.2] | 7.5 | 15.0 | 0.74 | same |
| explore | load | 666.2 | 728.7 | +79.7 [-370.1..+536.8] | 469.7 | 939.4 | 0.37 | same |
| explore | load + first frame | 679.5 | 739.8 | +69.6 [-365.1..+531.0] | 480.1 | 960.1 | 0.34 | same |
| explore | first frame | 13.4 | 13.9 | -1.5 [-6.0..+18.7] | 8.1 | 16.1 | 0.6 | same |
| explore | stage Car::load_tuned | 453.0 | 657.0 | +105.3 [-528.3..+715.9] | 572.2 | 1144.3 | 0.34 | same |
| explore | stage Car::load_tuned: solids, textures | 306.2 | 439.8 | +45.8 [-346.4..+401.9] | 402.1 | 804.1 | 0.37 | same |
| explore | stage drive_in | 517.5 | 562.2 | +41.2 [-291.8..+404.0] | 382.0 | 764.1 | 0.37 | same |
| explore | stage cache: open | 414.1 | 505.5 | +25.1 [-459.8..+495.0] | 481.5 | 963.0 | 0.46 | same |
| explore | stage WorldRenderer::new | 162.0 | 155.7 | +13.8 [-43.0..+72.2] | 49.6 | 99.2 | 0.4 | same |
| explore | stage Encounters: cars joined | 70.2 | 95.2 | +9.1 [-84.9..+121.7] | 87.5 | 175.1 | 0.37 | same |
| explore | stage World::load | 35.9 | 87.5 | +5.3 [-49.9..+125.8] | 77.6 | 155.1 | 0.26 | same |
| explore | stage Encounters::new | 38.8 | 44.5 | +5.1 [-10.5..+20.0] | 17.0 | 33.9 | 0.22 | same |
| explore | stage WorldRenderer::new: texture arrays | 39.5 | 42.9 | +4.9 [-15.9..+20.1] | 15.5 | 30.9 | 0.37 | same |
| explore | stage WorldRenderer::new: texture arrays: upload | 39.0 | 42.6 | +3.8 [-16.7..+18.3] | 14.9 | 29.8 | 0.4 | same |
| explore | stage Encounters::new: picks | 3.3 | 3.9 | +1.0 [-0.2..+3.0] | 1.0 | 2.0 | 0.072 | same |
| explore | stage drive_in: warm | 22.0 | 23.6 | +0.8 [-8.8..+9.0] | 11.4 | 22.9 | 0.43 | same |
| explore | stage Car::load_tuned: part database | 4.0 | 3.7 | +0.5 [-2.6..+2.2] | 2.4 | 4.8 | 0.34 | same |
| explore | stage collision ahead | 10.3 | 10.4 | +0.4 [-1.2..+1.8] | 1.3 | 2.5 | 0.34 | same |
| explore | stage WorldRenderer::new: buffers, uniforms | 9.8 | 9.4 | +0.4 [-3.4..+4.0] | 3.9 | 7.9 | 0.32 | same |
| explore | stage warm: car renderer | 6.7 | 7.3 | +0.4 [-1.5..+9.9] | 2.2 | 4.3 | 0.37 | same |
| explore | stage World::load: from the cache | 24.9 | 24.6 | +0.2 [-4.6..+8.2] | 4.1 | 8.2 | 0.49 | same |
| explore | stage explore Traffic::load | 2.7 | 2.4 | +0.1 [-2.1..+4.6] | 2.7 | 5.3 | 0.37 | same |
| explore | stage WorldRenderer::new: solid spheres | 2.2 | 2.3 | +0.1 [-3.7..+0.9] | 1.3 | 2.6 | 0.46 | same |
| explore | stage warm: post chain | 2.7 | 2.5 | -0.1 [-2.8..+0.6] | 0.7 | 2.0 | 0.6 | same |
| explore | stage drive_in: uploads | 3.0 | 2.7 | -0.1 [-3.1..+6.9] | 2.6 | 5.2 | 0.54 | same |
| explore | stage drive_in: car renderer | 5.0 | 4.9 | -0.1 [-1.6..+0.9] | 0.8 | 2.0 | 0.78 | same |
| explore | stage warm: world renderer | 11.7 | 14.3 | -0.1 [-5.7..+7.1] | 7.9 | 15.8 | 0.54 | same |
| explore | stage WorldRenderer::new: pipelines | 27.5 | 27.0 | -0.5 [-12.3..+10.3] | 13.5 | 27.0 | 0.54 | same |
| explore | stage WorldRenderer::new: draws and textures | 74.7 | 70.6 | -0.8 [-33.2..+24.2] | 20.8 | 41.7 | 0.54 | same |
| explore | stage WorldRenderer::new: groups and draws | 18.1 | 17.4 | -0.9 [-11.5..+5.0] | 6.6 | 13.1 | 0.63 | same |
| explore | stage car renderers ahead | 7.3 | 5.6 | -1.1 [-3.9..+1.2] | 2.2 | 4.4 | 0.8 | same |
| explore | stage Game::new | 38.6 | 35.8 | -5.6 [-20.4..+7.4] | 12.6 | 25.3 | 0.82 | same |
| explore | stage Car::load_tuned: shared parts and the rest | 99.5 | 50.6 | -9.8 [-176.5..+76.1] | 74.5 | 149.0 | 0.74 | same |

## cold (53 measures; p threshold 0.01/53)

| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |
|---|---|---|---|---|---|---|---|---|
| boot | start -> title | 657.0 | 706.0 | +24.0 [-275.0..+308.0] | 238.7 | 477.4 | 0.4 | same |
| boot | start -> first frame | 333.5 | 402.5 | +15.0 [-191.0..+272.0] | 252.8 | 505.6 | 0.46 | same |
| boot | start -> entry | 200.5 | 199.0 | +0.0 [-110.0..+120.0] | 133.8 | 267.6 | 0.51 | same |
| boot | stage cache: open | 333.7 | 383.6 | +17.5 [-113.8..+356.8] | 89.9 | 179.8 | 0.34 | same |
| boot | stage Game::new | 109.3 | 122.8 | +7.4 [-56.4..+99.4] | 70.0 | 139.9 | 0.37 | same |
| boot | stage Car::load_tuned: solids, textures | 141.0 | 0.0 | +0.0 [-308.2..+153.1] | 209.0 | 418.0 | 0.61 | same |
| boot | stage programs ahead | 23.9 | 23.6 | -2.6 [-15.1..+8.4] | 9.5 | 19.0 | 0.68 | same |
| explore | first frame | 19.1 | 15.6 | -3.1 [-10.0..+4.9] | 7.6 | 20.0 | 0.78 | same |
| explore | load | 835.7 | 796.1 | -63.6 [-325.1..+388.0] | 248.0 | 496.0 | 0.74 | same |
| explore | load + first frame | 853.2 | 807.5 | -67.2 [-325.8..+380.4] | 239.6 | 479.1 | 0.74 | same |
| explore | stage WorldRenderer::new: texture arrays: upload | 35.3 | 32.8 | +2.3 [-12.0..+18.2] | 18.9 | 37.9 | 0.46 | same |
| explore | stage World::load: from the cache | 22.7 | 24.7 | +1.3 [-5.8..+11.0] | 8.1 | 16.2 | 0.32 | same |
| explore | stage WorldRenderer::new: texture arrays | 37.5 | 37.1 | +0.7 [-14.6..+16.2] | 18.0 | 36.0 | 0.49 | same |
| explore | stage collision ahead | 8.9 | 11.2 | +0.7 [-2.1..+4.1] | 2.6 | 10.0 | 0.37 | same |
| explore | stage explore Traffic::load | 3.3 | 2.4 | +0.2 [-2.1..+5.3] | 2.1 | 10.0 | 0.43 | same |
| explore | stage drive_in: car renderer | 5.0 | 5.4 | +0.1 [-1.4..+1.8] | 1.4 | 10.0 | 0.46 | same |
| explore | stage Car::load_tuned: part database | 3.4 | 3.2 | -0.1 [-1.8..+1.1] | 1.1 | 10.0 | 0.63 | same |
| explore | stage drive_in: uploads | 3.3 | 2.3 | -0.1 [-3.8..+3.3] | 3.1 | 10.0 | 0.6 | same |
| explore | stage Encounters::new: picks | 3.5 | 3.1 | -0.1 [-1.4..+1.0] | 1.3 | 10.0 | 0.66 | same |
| explore | stage exe_tables::build | 1.1 | 0.7 | -0.2 [-1.4..+0.2] | 0.6 | 10.0 | 0.84 | same |
| explore | stage warm: world renderer | 10.9 | 11.3 | -0.2 [-6.5..+5.4] | 7.1 | 14.2 | 0.54 | same |
| explore | stage WorldRenderer::new: pipelines | 22.4 | 26.3 | -0.2 [-11.1..+16.9] | 15.2 | 30.3 | 0.54 | same |
| explore | stage warm: post chain | 3.1 | 2.6 | -0.3 [-2.3..+0.6] | 1.0 | 10.0 | 0.82 | same |
| explore | stage car renderers ahead | 5.1 | 4.4 | -0.3 [-5.4..+1.1] | 0.9 | 10.0 | 0.66 | same |
| explore | stage warm: car renderer | 7.0 | 6.6 | -0.4 [-5.1..+1.3] | 1.8 | 10.0 | 0.66 | same |
| explore | stage WorldRenderer::new: buffers, uniforms | 10.2 | 8.4 | -0.8 [-4.3..+3.3] | 2.4 | 10.0 | 0.76 | same |
| explore | stage Encounters: cars joined | 124.0 | 127.9 | -1.6 [-182.8..+43.9] | 28.4 | 56.7 | 0.63 | same |
| explore | stage WorldRenderer::new: groups and draws | 23.3 | 14.1 | -1.6 [-15.1..+13.7] | 8.5 | 16.9 | 0.68 | same |
| explore | stage WorldRenderer::new: solid spheres | 3.6 | 1.7 | -1.7 [-7.6..+0.6] | 1.6 | 10.0 | 0.88 | same |
| explore | stage WorldRenderer::new: draws and textures | 73.2 | 74.5 | -2.0 [-39.0..+36.7] | 36.6 | 73.3 | 0.54 | same |
| explore | stage drive_in: warm | 24.3 | 21.0 | -3.4 [-13.5..+8.0] | 12.3 | 24.7 | 0.74 | same |
| explore | stage Encounters::new | 44.2 | 37.2 | -5.2 [-14.9..+12.7] | 12.7 | 25.4 | 0.68 | same |
| explore | stage WorldRenderer::new | 167.1 | 152.7 | -5.7 [-68.2..+72.2] | 71.1 | 142.3 | 0.63 | same |
| explore | stage World::load | 366.0 | 363.7 | -10.5 [-111.6..+58.1] | 61.6 | 123.2 | 0.63 | same |
| explore | stage Game::new | 103.4 | 93.6 | -15.9 [-145.7..+11.1] | 32.0 | 63.9 | 0.86 | same |
| explore | stage Car::load_tuned: solids, textures | 512.0 | 564.1 | -18.2 [-185.9..+153.2] | 141.7 | 283.5 | 0.54 | same |
| explore | stage cache: open | 831.1 | 851.8 | -19.9 [-307.6..+204.5] | 168.5 | 336.9 | 0.68 | same |
| explore | stage Car::load_tuned: shared parts and the rest | 373.2 | 349.2 | -20.0 [-455.8..+124.4] | 58.9 | 117.8 | 0.71 | same |
| explore | stage drive_in | 646.3 | 602.3 | -26.1 [-241.7..+245.6] | 222.0 | 444.0 | 0.66 | same |
| explore | stage Car::load_tuned | 883.8 | 945.4 | -50.2 [-584.1..+262.1] | 214.5 | 429.0 | 0.6 | same |

## cores (68 measures; p threshold 0.01/68)

| scene | measure | base median | new median | shift [95 % CI] | noise | limit | p (slower) | verdict |
|---|---|---|---|---|---|---|---|---|
| boot | start -> first frame | 91.0 | 101.0 | +5.0 [-44.0..+35.0] | 24.5 | 48.9 | 0.34 | same |
| boot | start -> entry | 52.5 | 52.5 | +1.0 [-20.0..+22.0] | 16.3 | 32.6 | 0.44 | same |
| boot | start -> title | 181.0 | 169.5 | -5.0 [-72.0..+180.0] | 57.5 | 114.9 | 0.55 | same |
| boot | stage cache: open | 81.1 | 88.5 | +6.1 [-28.5..+71.8] | 31.4 | 62.8 | 0.34 | same |
| boot | stage programs ahead | 20.6 | 27.9 | +1.2 [-17.2..+14.3] | 8.6 | 17.1 | 0.51 | same |
| boot | stage Car::load_tuned: solids, textures | 52.5 | 55.8 | +0.8 [-50.0..+81.2] | 39.2 | 78.5 | 0.39 | same |
| boot | stage collision ahead | 8.7 | 9.8 | +0.2 [-8.8..+2.6] | 1.6 | 3.3 | 0.45 | same |
| boot | stage WorldRenderer::new: solid spheres | 2.6 | 2.9 | +0.1 [-0.5..+0.7] | 0.5 | 2.0 | 0.21 | same |
| boot | stage WorldRenderer::new: groups and draws | 20.1 | 10.4 | +0.0 [-21.0..+10.1] | 15.4 | 30.8 | 0.65 | same |
| boot | stage Car::load_tuned: shared parts and the rest | 17.3 | 1.9 | +0.0 [-47.3..+9.1] | 25.6 | 51.3 | 0.64 | same |
| boot | stage Car::load_tuned | 75.8 | 25.4 | +0.0 [-92.6..+51.6] | 45.4 | 90.7 | 0.58 | same |
| boot | stage preload car: renderer | 1.6 | 0.8 | +0.0 [-1.9..+1.6] | 2.3 | 4.6 | 0.64 | same |
| boot | stage World::load: from the cache | 29.1 | 30.0 | -1.0 [-26.0..+6.4] | 5.3 | 10.5 | 0.54 | same |
| boot | stage Game::new | 43.0 | 39.0 | -1.7 [-20.1..+17.1] | 13.5 | 27.1 | 0.68 | same |
| boot | stage World::load | 42.1 | 44.0 | -2.4 [-32.6..+10.6] | 10.7 | 21.5 | 0.57 | same |
| explore | first frame | 13.3 | 16.4 | -0.7 [-17.0..+8.2] | 8.9 | 17.9 | 0.6 | same |
| explore | load + first frame | 312.9 | 261.7 | -7.1 [-110.7..+274.6] | 91.1 | 182.3 | 0.71 | same |
| explore | load | 294.0 | 248.3 | -7.8 [-98.2..+149.3] | 81.7 | 163.3 | 0.6 | same |
| explore | stage Encounters: cars joined | 8.9 | 13.4 | +3.1 [-3.9..+8.5] | 5.0 | 9.9 | 0.12 | same |
| explore | stage Car::load_tuned | 98.4 | 91.2 | +2.7 [-53.2..+111.0] | 36.8 | 73.5 | 0.49 | same |
| explore | stage warm: car renderer | 7.7 | 8.9 | +1.2 [-2.9..+3.6] | 1.5 | 2.9 | 0.14 | same |
| explore | stage WorldRenderer::new: groups and draws | 15.8 | 16.0 | +1.1 [-11.4..+8.5] | 5.7 | 11.5 | 0.43 | same |
| explore | stage Encounters::new | 34.6 | 35.6 | +1.0 [-8.5..+23.4] | 10.5 | 21.1 | 0.46 | same |
| explore | stage car renderers ahead | 5.8 | 7.1 | +1.0 [-4.6..+6.4] | 4.1 | 8.3 | 0.22 | same |
| explore | stage warm: post chain | 3.5 | 3.9 | +0.8 [-2.0..+2.7] | 1.3 | 2.6 | 0.22 | same |
| explore | stage Car::load_tuned: shared parts and the rest | 40.8 | 40.7 | +0.2 [-14.7..+12.4] | 12.9 | 25.8 | 0.49 | same |
| explore | stage drive_in: uploads | 2.2 | 3.1 | +0.2 [-1.7..+3.7] | 2.6 | 5.2 | 0.37 | same |
| explore | stage WorldRenderer::new: solid spheres | 1.8 | 1.6 | +0.1 [-1.6..+0.8] | 0.9 | 2.0 | 0.46 | same |
| explore | stage Car::load_tuned: part database | 4.0 | 3.5 | +0.1 [-2.9..+3.2] | 3.0 | 6.0 | 0.46 | same |
| explore | stage Encounters::new: picks | 3.4 | 3.4 | -0.0 [-0.3..+0.4] | 0.2 | 2.0 | 0.57 | same |
| explore | stage Game::new | 39.6 | 42.3 | -0.3 [-14.2..+22.3] | 13.6 | 27.2 | 0.54 | same |
| explore | stage drive_in: car renderer | 4.6 | 4.1 | -0.3 [-1.6..+0.7] | 1.2 | 2.4 | 0.84 | same |
| explore | stage WorldRenderer::new: buffers, uniforms | 8.2 | 7.5 | -0.4 [-3.2..+1.0] | 1.4 | 2.7 | 0.82 | same |
| explore | stage warm: world renderer | 10.7 | 10.0 | -0.5 [-3.6..+5.7] | 2.0 | 4.0 | 0.66 | same |
| explore | stage drive_in: warm | 16.9 | 15.3 | -0.5 [-5.3..+8.4] | 2.6 | 5.1 | 0.68 | same |
| explore | stage collision ahead | 10.9 | 10.2 | -0.8 [-3.0..+0.3] | 1.0 | 2.0 | 0.92 | same |
| explore | stage Car::load_tuned: solids, textures | 38.4 | 43.4 | -0.9 [-14.0..+40.7] | 18.5 | 36.9 | 0.6 | same |
| explore | stage WorldRenderer::new: pipelines | 29.8 | 23.6 | -1.2 [-12.7..+16.5] | 10.3 | 20.6 | 0.76 | same |
| explore | stage WorldRenderer::new | 146.9 | 118.8 | -1.3 [-53.4..+60.9] | 41.8 | 83.6 | 0.51 | same |
| explore | stage WorldRenderer::new: texture arrays: upload | 34.9 | 31.1 | -2.4 [-17.9..+11.7] | 13.7 | 27.5 | 0.71 | same |
| explore | stage cache: open | 47.4 | 39.9 | -3.3 [-19.9..+17.4] | 17.4 | 34.7 | 0.68 | same |
| explore | stage World::load: from the cache | 21.2 | 17.9 | -3.7 [-15.2..+2.5] | 4.9 | 9.9 | 0.9 | same |
| explore | stage World::load | 40.7 | 31.3 | -4.7 [-20.1..+11.8] | 12.9 | 25.9 | 0.78 | same |
| explore | stage WorldRenderer::new: texture arrays | 38.5 | 31.4 | -4.9 [-24.0..+11.2] | 17.1 | 34.1 | 0.78 | same |
| explore | stage drive_in | 234.5 | 193.5 | -6.0 [-83.8..+82.3] | 68.7 | 137.3 | 0.66 | same |
| explore | stage WorldRenderer::new: draws and textures | 69.5 | 53.1 | -6.5 [-39.2..+20.4] | 27.8 | 55.5 | 0.6 | same |

loadgate: warm: same (boot start -> entry 136 -> 179 ms; boot start -> first frame 246 -> 330 ms; boot start -> title 480 -> 432 ms; explore load 666 -> 729 ms; explore first frame 13 -> 14 ms; explore load + first frame 680 -> 740 ms)
loadgate: cold: same (boot start -> entry 200 -> 199 ms; boot start -> first frame 334 -> 402 ms; boot start -> title 657 -> 706 ms; explore load 836 -> 796 ms; explore first frame 19 -> 16 ms; explore load + first frame 853 -> 808 ms)
loadgate: cores: same (boot start -> entry 52 -> 52 ms; boot start -> first frame 91 -> 101 ms; boot start -> title 181 -> 170 ms; explore load 294 -> 248 ms; explore first frame 13 -> 16 ms; explore load + first frame 313 -> 262 ms)
LOADGATE: PASS: no load slower beyond its noise limit
