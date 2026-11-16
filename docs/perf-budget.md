# Frame budget and performance regression gate (2026-11-05)

Two tools and the numbers they gave on main
`99e726f`: a per-effect GPU/CPU **budget table** (every enhancement row in
its heavy scene at 1080p and 4K on the reference desktop GPU, preset totals, an iGPU
class) and a **regression gate** (`scripts/checks/perfgate.sh`) the maintainer
runs after each merge. Measuring aids only: nothing native is involved,
no game code changed (scripts and docs only), so every image, sound and
simulation is unchanged by construction.

## Method

* Binary: main built with `perf.sh build` (the full release
  profile), sha256 `939f08f9419c…`. Bench: `perf.sh bench` (fixed 60 Hz
  step, pipelined, 2 frames in flight, the bench JSON), **4 runs** per
  config, **600 frames**, `ORDER=alternate` (the config list backwards on
  even repetitions, so a slow drift of the shared machine hits neighbours
  alike). 136 configs, 544 runs, **6455 s**; load average 11–67 (other
  builds, Wine sessions and benches; the shadow-softness pass, the volumetric smoke, the quality tooling and the performance overlay active).
* GPU = the run's median GPU busy ms/frame (timestamps), the median over
  the 4 runs; p95 the same for the 95th percentile; CPU = main thread /
  render workers ms/frame (thread CPU; noisy on this machine, the perf harness:
  12–35 % run to run).
* Scenes (perf.sh): `explore` (free roam, the opening burnout: the smoke's
  heavy scene), `racing` (a circuit at speed), `race` (the race start: cars,
  glass), `rain` (drive at clock 162: night, heavy rain 0.70: the wet
  road's heavy scene), `water` (new: explore with the camera orbiting the
  Beacon Hill fountain steps, docs/water.md's worst case N).
* Presets (perf.sh): `orig`; `modern` (the perf harness's Modern line); `ultra26` (new:
  `Preset::Ultra` exactly as qol.rs has it on 99e726f); `extreme`;
  `user` (new: the player's play setup, `the player's options file`:
  its enh lines and World Detail custom values — SSAA 2x + MSAA override,
  SSR full, SSAO 25 %, Volumetric Fog Low, …); `max` (new: `user` + every
  newer row at its top: Modern Lighting + RT, Particles Cinematic, Water
  Realistic, Wet Road Interaction High, Rain Detail High, Wet Car, Spray,
  Lens Drops, Motion Blur Per-Object, DOF Cinematic, Headlight Beams, Light
  Probes, Enhanced Glass, Surface Detail Materials + Normals, Smooth Cars,
  World Detail Extreme, Dynamic Lamps).
* Per effect: **leave-one-out** from `max` — max minus (max with that one
  row down) in the row's heavy scene; "step up" rows the other way. So the
  cost is what the row costs in the player's real setup, with every
  interaction (a row feeding SSR, Modern Lighting, …) included. p = the
  run-level Mann-Whitney U (4 + 4 runs: smallest p 0.029), noise = pooled
  1.4826 MAD / median. "Own passes" = the row's own GPU passes (timestamps)
  in the max run; where it is larger than the net cost, the row replaces
  native work (water: the native water draws leave the main view).
* Reproduce: `scripts/perf/budget.sh OUT BIN` (then `budget.py OUT` writes
  `OUT/budget.md`); iGPU: `IGPU=1 REPS=2 budget.sh OUT BIN`.
  Evidence: `docs/evidence/perf-budget/` (`budget-rx7800xt.md` the full
  tables with p and noise, `budget-rx7800xt-medians.json` every config's
  run values, medians and GPU passes ≥ 0.05 ms, the bench log).

## Preset totals, reference desktop GPU (GPU busy median ms/frame [p95])

| preset | explore 1080p | racing 1080p | race 1080p | rain 1080p | water 1080p | explore 4K | racing 4K | race 4K | rain 4K | water 4K |
|---|---|---|---|---|---|---|---|---|---|---|
| Original | 0.59 | 0.74 | 0.78 | 0.77 | 0.61 | 2.17 | 2.44 | 2.73 | 2.51 | 2.24 |
| Modern | 1.23 | 2.28 | 1.93 | 2.14 | 1.38 | 5.73 | 8.03 | 7.85 | 7.99 | 6.08 |
| Ultra (Preset::Ultra) | 2.60 [6.8] | 3.76 | 3.92 | 4.85 | 3.04 [17.6] | 10.14 [26.4] | 13.33 | 14.09 | 17.78 | 11.00 [59.6] |
| Extreme | 3.21 [7.2] | 4.62 | 4.96 | 5.97 | 3.78 [15.0] | 12.95 [28.3] | 16.23 | 19.26 | 22.11 | 14.39 [27.8] |
| user's setup | 2.42 | 3.25 | 3.55 | 3.61 | 2.38 | 9.66 | 11.89 | 12.97 | 13.84 | 8.77 |
| **everything max** | 4.19 | 8.57 | 8.28 | 8.56 | 5.50 [19.2] | 15.58 | **26.09** | **30.07** | **30.53** [37.5] | 19.51 [45.0] |

CPU (main thread / render workers ms/frame): Original 0.6–0.9 / 0.8–1.7,
Ultra 1.1–2.0 / 1.4–3.0, max 1.4–2.6 / 1.6–3.4 at either size (the CPU
does not scale with the size). **The player's setup at 4K runs at 9–14 ms
(70–110 fps); everything max at 4K at 16–31 ms (32–64 fps): not 60 fps in
racing, the race start or rain.** p95 brackets only where p95 > 1.3 × the
median: the spikes are Ultra's smoke (explore; Cinematic removes them,
The Cinematic smoke work) and, in the water orbit, **"world: light probes" (3.5–6.4 ms/frame
mean at 4K)**: the probes re-bake while the camera moves away from the
car (inferred from the pass; the orbit track flies the camera, gameplay
cameras stay near the car — open for the effects-performance pass). **The frame-budget fixes**: found and fixed (baked probes re-baked every frame with no probe within 40 m; "The budget table's findings fixed" below).

## Per effect (max minus the row; GPU ms at 1080p / 4K; heavy scene)

| row (max → down) | scene | 1080p | 4K | own passes 4K | UI hint 4K | plan budget | verdict |
|---|---|---|---|---|---|---|---|
| Modern Lighting + RT → Original | racing | **+3.64** | **+8.81** | (in main view) | 5.58 | ≤ +2 ms 1080p Modern | Modern alone ≈ +1.6 (1080p): inside; the RT part is the rest |
| … RT part (Modern + RT → Modern) | racing | +2.06 | **+5.71** | | 3.3 | | hint low by 2.4 ms |
| Particles Cinematic → Original | explore | +0.36 | +0.88 | 0.92 | 0.21 | ≤ +1.5 ms 4K heavy | inside |
| Particles Cinematic vs Ultra | explore | −0.03 | −0.47 | | | | Cinematic cheaper (and no spikes) |
| Water Realistic → Off | water | +0.13 (n.s.) | +0.65 | 3.64 | 1.65 | ≤ +1.5 ms 4K fountain | inside (net; passes replace native draws) |
| Water Realistic → On | water | +0.38 | +1.32 | | | | |
| Wet Road Interaction High → Off | rain | +0.82 | **+2.65** | 0.22 (+1.0 weather fx) | 0.82 | ≤ +1 ms 1080p | inside at 1080p; 4K 3× the hint |
| Wet Roads → Off | rain | +0.11 n.s. | −0.11 n.s. | | | | ≈ 0 |
| Rain Detail High / Spray / Lens Drops / Wet Car → Off | rain | ≤ +0.17 n.s. | ≤ +0.14 n.s. | weather fx 1.84 (shared) | 0.27 / 0.25 / 0.01 / 0.02 | | each below the noise under max |
| SSR full → Off | rain | +1.59 | **+5.35** | 5.40 | – | | the largest non-AA row at 4K |
| SSR full → SSR + RT (step up) | rain | +0.52 | +2.65 | | | | |
| Volumetric Fog Low → Off / → High | rain | +0.23 / −0.02 | +0.12 n.s. / +0.60 | 0.33 | 0.27 / 0.46 | | inside the hints |
| Headlight Beams → Off | rain | **−0.51** | **−2.58** | | 0.21 | | turning Beams **off** is slower: the main view +2.6 ms. **The frame-budget fixes**: Modern Lighting's traffic headlights binned as 28 m spheres; the shadow-softness pass already cut it to −0.41, the frame-budget fixes' cone binning to −0.04 (n.s.) at 4K, −0.05 at 1080p |
| Surface Detail Mat + Normals → Off | rain | +0.37 | +0.80 | | 0.09 | | hint low (feeds lamps / wet road) |
| SSAO 25 % → Off / → 100 % | racing | +0.40 / −0.14 n.s. | +0.91 / +0.05 n.s. | 0.94 | – | | 100 % costs the same as 25 % |
| Lamp Shadows High → Off | racing | +0.35 | +0.70 | 0.05 | – | | |
| Dynamic Lamps → Off | racing | +0.15 n.s. | −0.03 n.s. | | – | | ≈ 0 under Modern Lighting (it supersedes them) |
| Road Reflection Ultra / Enhanced Reflections → Off | racing | +0.21 / +0.24 | +0.21 / +0.23 n.s. | 0.11 | – | | small |
| High Precision Post (full glow) → Off | racing | +0.66 | +2.48 | 1.07 | – | | |
| Motion Blur Per-Object → Original | racing | +0.58 | +2.06 | 2.24 | 1.16 | | hint low by 0.9 ms |
| Depth of Field Cinematic → Original | racing | +0.20 | +0.02 n.s. | 0.00 | 1.30 | | only in scenes / replays / photo (0 while racing) |
| Light Probes → Off | racing | +0.18 | +0.12 n.s. | 0.02 | 0.00 | | (see the water spikes above) |
| **SSAA 2x → Native 8x MSAA** | racing | **+2.69** | **+10.80** | | 0.01 | | the largest single cost of the player's setup; hint wrong (0.01) |
| Enhanced Glass → Off | race | +0.15 | +0.65 | 1.88 | 0.33 | | hint low |
| Contact Shadows → Off | race | +0.27 | +0.94 | | – | | |
| Car Reflection 1024 / Car Shadow Ultra / Enhanced Paint / Smooth Cars → native | race | ≤ +0.15 | ≤ +0.17 | | – | | small |
| World Detail Extreme / LOD Fade / Detail 200 % + Keep All Loaded → native | explore | ≈ 0 | ≤ +0.14 n.s. | | 0.10 | | GPU ≈ 0; CPU main +0.1–0.35 ms each (noisy) |

"n.s." = p ≥ 0.05 or within 2 × noise. Sum of the leave-one-out costs
exceeds max − Original in places (interactions are counted in each row).
CPU deltas per row: mostly within ±0.6 ms main / workers — at or under
this machine's CPU noise; none is a clear CPU cost except Modern Lighting
(+0.3–0.6 ms main) and the World Detail rows (+0.1–0.35 ms main).

**Budgets for the NX units** (allowed extra GPU ms in the player's max setup,
measured here; a unit must stay inside, the gate catches a regression):

| effect | budget | now |
|---|---|---|
| Smoke (Particles, Cinematic; the volumetric smoke volumetric) | ≤ +1.5 ms 4K explore burnout, light smoke ≈ +0 | +0.88 |
| Water (Realistic) | ≤ +1.5 ms 4K with a fountain on screen | +0.65 net (passes 3.6) |
| Wet road (Interaction) | ≤ +1 ms 1080p rain (On); High: ≤ +1 ms 1080p, 4K reported | +0.82 1080p, +2.65 4K |
| Modern Lighting (Modern Lighting phase 2 / the shadow-softness pass shadows) | ≤ +2 ms 1080p Modern; RT reported | ≈ +1.6 (Modern), RT +2.1 / +5.7 |
| Everything max, the player's play setup at 4K | ≤ 16.7 ms in every scene is the goal (60 fps) | 16–31 ms: over in racing / race / rain; first targets SSAA 2x (10.8), Modern Lighting RT (5.7), SSR (5.4), wet interaction (2.7), HP post (2.5), motion blur (2.1) — the effects-performance pass |

**UI cost hints** (graphics_table.rs `*_COST_4K`) measured alone over
Original; under max several are low: SSAA 2x (0.01 vs 10.8 at 4K),
Modern + RT (5.58 vs 8.8), wet interaction (0.82 vs 2.65), motion blur
(1.16 vs 2.06), glass (0.33 vs 0.65); DOF's 1.3 applies only to scenes.
Not changed here (no behaviour change in this work): open for the settings unit.
Every hint the table measures is now its 4K number (below).

## IGPU class (the reference integrated GPU: a 2-CU RDNA2 iGPU, `IGPU=1`)

`IGPU=1 REPS=2 budget.sh` (300 frames, 2 runs, 1920×1080 only, 1470 s;
the same binary; internal evidence; World Detail 5, the bench's default:
everything raised, 8× MSAA. A fresh install on this class runs World Detail 3: Original 4.0–5.8 ms,
Medium 17.5–28.4, High 37.5–67.7 at 1080p, upper bounds measured under build load, docs/perf-integrated-gpu.md
"The Enhanced levels at World Detail 3"). GPU busy median ms/frame [p95]:

| preset | explore | racing | race | rain |
|---|---|---|---|---|
| Original | 10.2 [12.9] | 12.6 [15.2] | 15.3 [18.1] | 12.5 [15.1] |
| Modern | 33.8 | 68.4 | 57.4 | 70.3 |
| Ultra (Preset::Ultra) | 128.5 [249.9] | 142.2 | 155.9 | 186.6 |
| user's setup | 87.7 | 137.0 | 133.5 | 151.9 |
| everything max | 158.7 | 220.8 | 271.9 | 257.1 |

At World Detail 5 Original holds 60 fps on the iGPU at 1080p (10–15 ms); Modern is 15–30
fps; Ultra, the player's setup and max are 4–11 fps (the SSAA 2x scene alone
doubles the fill). The iGPU budget is therefore Original / a Low tier:
any row offered to integrated GPUs should be measured with `IGPU=1` (per-row
leave-one-out there is not in this run: 2-CU timings at ~150 ms/frame make
a 544-run matrix ~10 h). CPU (main / workers) is the same as on the
discrete GPU (1.1–3.3 / 1.3–3.7 ms).

## Memory (2026-11-05)

Own VRAM peak (fdinfo, `--bench racing`, 4K, reference desktop GPU) after the VRAM and post pass'
allocator blocks, target pool and off-tier release (`docs/perf-gpu.md`
"the VRAM and post pass"): Original 1354 MiB, Modern 1549, Ultra 2439, Extreme 2695,
Extreme FSR 1532, the player's setup 2572, max 3162 (race start 3163);
before 1369 / 2103 / 3273 / 3936 / 2070 / 3574 / 4008 (4492). 1080p
Original ~630 MiB (before 617–973, run to run). reference integrated GPU (VRAM
carve-out + GTT, 1080p): Original racing 669 → 560 MiB, race start
910 → 559, Modern 1129 → 619. Every preset at 4K now needs at most ~3.2 GB of
the game's own VRAM (before up to 4.5 GB). `NFSU2_VRAM_REPORT=file`
lists what is allocated by label.

## The regression gate (`scripts/checks/perfgate.sh`)

```
scripts/checks/perfgate.sh run    OUT NEW_BIN [BASELINE_DIR]   # A/B vs the accepted baseline; exit 1 = regression
scripts/checks/perfgate.sh accept OUT [NOTE]                   # OUT's new binary becomes the baseline ("current")
scripts/checks/perfgate.sh seed   OUT NEW_BIN [NOTE]           # first baseline: an A/A run, then accept
```

* **Set** (`perf.sh` `SET=gate`, 8 configs): racing / explore / race start
  Original 1080p (the main thread and the native path), racing Original 4K,
  racing Ultra 4K (preset), explore Extreme 1080p (smoke, CPU of
  the enhancements), rain `max` 4K (the player's play setup plus every newest
  row, the heaviest config: ~28 ms), race start `max` 1080p. 5 runs per arm
  (Mann-Whitney 5 + 5: smallest p 0.008; 3 + 3 can never flag), 300 frames,
  arms adjacent per config and rotated each repetition (`perf.sh bench`).
* **Rule** (`perfstat.py gate`, reusing `verdict`): FAIL metrics GPU busy
  median, draw CPU p10, render workers CPU, main thread CPU; a FAIL is a
  REGRESSION: p < 0.05 **and** |Δ| > max(**3 %**, the metric's tolerance,
  2 × the pooled run-to-run noise) **and** > the metric's floor. Reported
  only (never fail): GPU p95, process CPU, own VRAM, RAM, load. Exit 1 and
  `PERFGATE: FAIL (…)` naming config, metric, change and p.
* **Baseline** (`$OB_PRIVATE/perf-baselines/`, not in the repo; override
  `PERF_BASELINES`): one dir per accepted binary — `game` (the perf.sh-built
  binary), `summary.json` (its runs: values per metric and config),
  `compare.md`, `meta.txt` — and `current` → the accepted one. A/B against
  the baseline binary is the gate (same time, same machine load). A dir
  without `game` or `PERFGATE_JSON=1`: the new binary alone against
  `summary.json` — indicative only (another time and load), it never fails
  unless `PERFGATE_JSON_FAIL=1`.
* **Runtime** (measured, seed run, load average 9–17): bench **319 s**
  (88 runs incl. the warm-up), the stats < 1 s. Builds are not included
  (gate.sh builds anyway). OUT keeps ~2 MB (the ~550 MB of asset caches are
  removed; `PERFGATE_KEEP=1` keeps them).
* **A/A check** (the seed, same binary in both arms): 8 configs × 9 metrics,
  no FAIL, no "worse"; GPU noise 0.5–3 % (needed 3 %), main / workers CPU
  noise 6–30 % (so CPU changes below ~10–60 % cannot fail the gate on this
  shared machine: the CPU columns catch only large CPU regressions; the
  GPU column is the precise one). internal evidence.

### How the maintainer runs it after each merge

```
scripts/checks/gate.sh $OB_PRIVATE/lead/gates/<unit> <base>          # as before; leaves <out>/b/game
scripts/checks/perfgate.sh run $OB_PRIVATE/lead/gates/<unit>/perf <out>/b/game
#  PERFGATE: pass           -> scripts/checks/perfgate.sh accept $OB_PRIVATE/lead/gates/<unit>/perf "<unit> <commit>"
#  PERFGATE: FAIL (…)       -> rerun that config alone with REPS=10 (CONFIGS="cfg" perf.sh bench … base=… new=…);
#                              a confirmed regression needs an approved reason (a new opt-in row costs only
#                              where it is on: Original configs must never move) — then accept with that note.
```
Run it alone (no builds, parity or other benches of your own at the same
time); record the load average it prints. Accept only after a pass (or an
approved reason), so `current` is always the last accepted main.

## Quiet-machine re-measure and re-baseline

No other build, bench or Wine session ran; load average 0.8–1.5 from the runs themselves. One
thing could not be cleared: another program held the discrete GPU's graphics queue busy the whole
time (its fdinfo engine time ≈ 100 % of wall time). The A/B gate is unaffected (both arms
interleaved), and GPU busy (our own timestamps) moves little; **frame times on the discrete GPU are
not trustworthy** in this round: a 4K Original frame of 2.5 ms GPU waited ~28 ms for its fence. The
integrated GPU was free.

**The gate against the old baseline** (main against the binary accepted on the earlier main, three
runs): every run failed on the same metrics, so the change is real, not load
(`docs/evidence/perf-budget/quiet-rebaseline.txt`):

* explore Extreme 1080p GPU +17.4..+18.2 %: from the collision support-rule commit (the hull query's
  offset and climb). The bench's car now ends its 20 s throttle run held against something and moves
  7.1 m in the measured 300 frames instead of 30.1 m, so more of its own burnout smoke stays in
  view ("graphics: particles draw" 1.39 → 1.77 ms). The scene changed; no pass got dearer.
* CPU render workers +3..+18 %, main thread +6..+10 % (Original configs too; +0.03..0.2 ms a frame):
  about half arrived with the tick clock, the rest with the following tick work, nothing after; in
  rain max the spray step moved from the main thread (−15 %) to the workers (+17 %). Spread over
  many commits; no small exact undo.
* Load time +30..+160 % (reported only): already in the first half; noisy, not bisected further.

**Re-baselined** on today's main (`perfgate.sh accept` of the third run's binary): `current` now
compares against today. Only the baselines folder changed (a new dir, `current` moved).

**The Enhanced levels** (`--bench` 300 frames, Anti-Aliasing Native, World Detail 5 (the bench's default; at a
fresh install's World Detail 3 the integrated GPU's heaviest scene at 1080p is 5.8 / 7.7 / 28.4 / 67.7 ms:
docs/perf-integrated-gpu.md); the heaviest scene's GPU busy
median in ms; load and first frame: median over the four scenes; discrete GPU 3 runs, integrated 2;
`docs/settings-page.md` lists the levels):

| level | discrete 1080p | discrete 4K | integrated 1080p (frame p50 / p99) | integrated 1366×768 (frame p50 / p99) |
|---|---|---|---|---|
| Original | 0.81 | 2.72 | 14.07 (14.3 / 15.6) | 8.59 (8.9 / 9.6) |
| Low | 0.85 | 2.77 | 15.55 (15.8 / 17.9) | 9.51 (9.8 / 11.1) |
| Medium | 1.89 | 7.32 | 49.40 (49.8 / 61.3) | 27.68 (28.0 / 34.9) |
| High | 3.49 | 11.73 | 103.2 (103.9 / 130.0) | 57.5 (58.0 / 72.9) |

Load (median, warm cache): discrete 237–393 ms at 1080p, 163–354 ms at 4K; integrated 128–269 ms.
First frame: Original / Low 23–48 ms, Medium / High 37–88 ms on the integrated GPU and 365–460 ms on
the discrete one (not pipelines: Light Probes' arrival burst, which waited for the busy GPU once per
probe; "Load times" below). Integrated frame
times track its GPU time (p50 within 0.4 ms); on the discrete GPU they were held by the busy queue
(above), so only GPU busy is given.

**The starved case** (4K, SSAA 16x over Original, racing, 75 Hz schedule, 30 s, 3 runs each): GPU busy
21.3 ms in every run; frame mean 34.0 ms with the light-trail rule's own choice (cheap after 9–10
frames, 6 probes) and 38.4–38.5 ms with the cheap form forced. The earlier 21.4–21.9 ms medians do
not compare: this round's discrete-GPU frame times carry the other program's queue (above).

## Load times

What a player waits for, where the time goes, why the bench's `load_ms` grew, and what the first
frame after a load costs. Tables and raw distributions: `docs/evidence/perf-budget/load-times.txt`.

**In a player's own sessions.** The shipping game writes every wait (from the key to the new screen's
first frame, with a drive load's parts) and every drive's frame times into its own log
(docs/waits-log.md): the waits on the player's display and machine, which the harness's runs here cannot
show. Its cost: 10–16 ns a frame for the hooks' logic, no measurable change in the main thread's CPU time
per frame (7 interleaved pairs, `docs/evidence/perf-budget/waits-log-cost.txt`).

**Conditions.** The machine was shared for every series: other builds and comparisons ran (load
average 6–30 on 16 hardware threads) and another program kept the discrete GPU's queue busy the whole
time (its busy counter 72–100 %); the integrated GPU was otherwise idle. So every discrete-GPU wall
time below is the time **on a GPU shared with another program** and is not trustworthy as an
absolute (each wait for the GPU costs one of the other program's turns, 25–90 ms); the integrated
GPU's times and the thread-CPU and structure columns (which thread waits for what, pipelines built
where) are. A quiet machine with the discrete GPU to itself was not available: **open**. The OS's
file cache held the game's files in every run of this section's harness tables (a start after a reboot
in the real flow: "A start after a reboot" below), and the window, its surface and the audio device are
not opened by the harness (the windowed game logs its own `game start: done after … ms`): open.

**Measuring points.** `--bench boot` (the developer harness): the process's age when the game's data is
loaded (the loader and `Game::new`), the GPU device, the first boot screen's first frame drawn and
done on the GPU, then Enter every frame until the title screen's first frame is drawn. `--bench
transitions` (with `NFSU2_BENCH_MENU_FIRST=1`: the main menu's preload first, as a start of the
game has it): each load call plus its first frame drawn and done. `--bench explore|race|drive`:
`load_ms` and the first frame. Levels: Original and the three Enhanced levels' lines of
`docs/evidence/settings-levels/levels.sh`, Anti-Aliasing Native. `scripts/perf/blocksample.py`
runs a bench and samples what every thread is doing: running, waiting for another thread, or
blocked in which system call (the GPU's fence wait, its submit call, its memory calls).

### What a player waits for (warm cache; ms, median [min..max] of 5 runs)

| wait | discrete 1080p (shared GPU) Original | High | discrete 4K (shared GPU) Original | High | integrated 1080p Original | High |
|---|---|---|---|---|---|---|
| process start → first frame | 74 [66..186] | = | 90 [83..157] | = | 69 [66..86] | = |
| process start → title screen, Enter held | 125 [82..217] | = | 191 [108..275] | = | 85 [75..101] | = |
| menu → free roam (world and car preloaded by the menus) | 62 [37..144] | 145 [106..245] | 134 [46..195] | 220 [133..340] | 63 [59..64] | 178 [170..214] |
| menu → free roam, nothing preloaded | 232 [147..324] | 405 [277..453] | 240 [227..333] | 267 [174..621] | 107 [102..139] | 202 [159..549] |
| menu → Quick Race circuit | 269 [226..367] | 487 [332..581] | 247 [197..319] | 276 [242..340] | 134 [123..174] | 220 [181..294] |
| menu → Quick Race drag | 180 [106..183] | 235 [131..348] | 197 [104..227] | 198 [165..288] | 58 [55..83] | 134 [123..262] |
| free roam → race (same region) | 44 [29..77] | 75 [43..149] | 74 [38..86] | 118 [88..250] | 27 [25..42] | 60 [53..112] |
| race → free roam | 39 [27..69] | 73 [39..83] | 65 [28..85] | 71 [64..83] | 20 [19..24] | 65 [27..77] |
| free roam → another region's race, nothing preloaded | 188 [147..248] | 292 [162..364] | 188 [146..243] | 163 [126..375] | 64 [61..89] | 141 [111..356] |
| that race → free roam, nothing preloaded | 334 [263..367] | 397 [266..471] | 270 [227..303] | 283 [219..333] | 113 [107..157] | 205 [197..280] |
| into the garage | 53 [25..61] | 56 [42..74] | 55 [30..77] | 68 [62..108] | 22 [19..32] | 56 [36..57] |
| out of the garage | 34 [14..41] | 65 [36..78] | 53 [38..72] | 55 [36..88] | 8 [8..13] | 17 [16..27] |
| into a shop | 55 [16..76] | 53 [39..70] | 55 [54..73] | 56 [36..65] | 18 [17..21] | 24 [22..34] |
| out of a shop | 26 [22..66] | 60 [33..69] | 53 [28..73] | 65 [60..80] | 18 [17..26] | 50 [48..51] |
| out of a shop, a part fitted | 74 [50..118] | 208 [174..217] | 106 [71..109] | 158 [119..208] | 50 [45..66] | 120 [110..134] |

Low and Medium lie between the two columns (evidence §1); the start's two rows have no level (`=`).
Every warm wait is under 0.6 s; loads
over 200 ms show PC_Loading as the rule says (of 60 runs: menu → free roam with nothing preloaded
29, that race → free roam 28, menu → Quick Race circuit 27; the others 0–5). No transition's first
frame and no frame of the second after it built a pipeline or a shader module, at any level (1073
transitions, all 0). On the integrated GPU at Medium and High the spans that follow a menu drawn over
the world ("rivals loaded ahead", "preloaded": evidence) come out *longer* than the same load with
nothing preloaded, by one or two frames of 50–100 ms: the bench then starts the load with frames
still on the GPU, and the load's first step waits for them.

**Empty caches** (the game's cache folder and the driver's shader cache both new for every run;
`load_ms` of the first load of a start, nothing preloaded; ms, median [min..max] of 5):

| first load | discrete 1080p (shared GPU) Original | Low | Medium | High | integrated 1080p Original | Low | Medium | High |
|---|---|---|---|---|---|---|---|---|
| free roam | 744 [569..1333] | 1293 [1006..1414] | 1578 [1520..2275] | 1882 [1854..4508] | 526 [465..552] | 1066 [792..1459] | 2297 [1512..3911] | 1726 [1601..4998] |
| circuit race | 852 [643..2047] | 1320 [988..1668] | 1797 [1559..2459] | 1699 [1595..2277] | 435 [367..566] | 1279 [831..1508] | 2261 [1411..2572] | 2002 [1666..3068] |
| drag race | 570 [392..761] | 1013 [934..1366] | 2142 [1484..2685] | 1856 [1608..2259] | 480 [305..595] | 1287 [920..2007] | 1912 [1383..3465] | 1695 [1537..2036] |

Process start → first frame 166 [132..394] ms and → title 275 [165..470] ms (discrete), 144
[116..191] and 163 [127..204] ms (integrated); 4K is the same within the spread. These runs are
CPU-bound work on many threads on a machine at load average 10–30: the ranges are wide and the
medians are upper bounds for a quiet machine. Where a first start's time goes (one traced run each,
discrete, made while builds ran, so slower than the medians above): Original 2.0 s, of it the
track's parse 0.69 s, the cars' 0.9 s summed over 11 loads on workers, the world's own pipelines
0.27 s (the driver compiles) and the warm-up 0.25 s; High 3.3 s, of it the warm-up 2.4 s (the
driver compiles about 210 more programs). With a warm cache the same stages are 23 ms, about 65 ms
summed, 12 ms and 8 / 140–190 ms: the asset cache and the pipeline cache on disk (wgpu's, Vulkan:
`Gpu::pipeline_cache`, saved after each load) both work.
The menus' preload hides the world's and the car's part of a first load, not the programs'.

### Where a load's time goes (warm cache, the tree before this work; ms)

wall median [min..max] of 5 sampled runs, then in brackets the main thread's time (running /
waiting for its workers / waiting for the GPU) and the other threads' running time summed.

| stage | explore Extreme 1080p, integrated GPU | explore Extreme 1080p, discrete (shared) | rain max 4K, discrete (shared) |
|---|---|---|---|
| `Game::new` (before the load: front end, career data) | 23 (3 / 20 / 0; 106) | 22 (3 / 19 / 0; 98) | 23 (3 / 20 / 0; 100) |
| `World::load`: the track from the cache (a hit: mapped, no parse) | 23 (1 / 22 / 0; 118) | 22 (1 / 20 / 0; 114) | 22 (0 / 21 / 0; 131) |
| `Car::load_tuned`, beside it | 21 (0 / 20 / 0; 117) | 21 (1 / 19 / 0; 112) | 20 (0 / 20 / 0; 130) |
| `WorldRenderer::new` | 73 [70..74] (45 / 25 / 0; 167) | 169 [121..235] (49 / 117 / 0; 226) | 177 [157..194] (49 / 128 / 0; 239) |
| – groups and draws | 8 | 8 | 8 |
| – texture arrays: upload (workers) | 14 [13..16] (their GPU memory and submit calls 12) | 103 [60..174] (the same calls 202) | 115 [96..133] (188) |
| – buffers, uniforms | 5 | 7 | 7 |
| – the world's own pipelines | 11 | 11 | 12 |
| car renderer | 3 | 3 | 3 |
| the uploads' wait for the GPU | 2 | 34 [8..75] | 61 [43..94] |
| warm-up: the frame's programs (pipelines, modules, water) | 156 [150..185] (21 / 138 / 0; 685) | 142 [141..149] (14 / 130 / 0; 633) | 191 [186..214] (15 / 175 / 0; 899) |
| `drive_in` in all | 269 [266..295] (87 / 185 / 2; 1008) | 402 [315..444] (81 / 267 / 33; 976) | 471 [436..518] (82 / 325 / 60; 1274) |
| `load_ms` (with traffic, encounter cars or the grid) | 307 [300..330] | 437 [351..477] | 501 [466..546] |
| first frame: draw | 84 [80..120] (81 / 1 / 1) | 473 [270..560] (86 / 1 / 380) | 1466 [756..1523] (230 / 3 / 796, and 368 in the submit call) |
| first frame: the bench's wait for the GPU | 110 [106..112] | 92 [25..107] | 93 [48..116] |

Original (explore 1080p): integrated `load_ms` 169 [164..173], `drive_in` 133 (79 / 53 / 2), warm-up
18, first frame 28 (draw 5, then the bench's wait 23); discrete, shared, 290 [195..371], texture
upload 125 [22..135], the uploads' wait 51 [3..72], first frame 79 [17..93] (draw 3).

* The main thread works for about 80 ms of a load at every level; the rest is waiting for workers.
  No file is read with a system call (the cache files are mapped).
* The same load is 120–130 ms longer on the discrete GPU while another program uses it, all of it
  in two places: the texture arrays' upload (the workers sit in the driver's buffer-creation and
  submit calls) and the one wait for the GPU after the uploads. When the GPU happened to be free
  at that wait (under 10 ms; 10 of 56 Original runs) the upload took 31 ms, `drive_in` 182 ms and
  the first frame 31 ms; with it busy 96, 281 and 56 ms (thread CPU 80 ms either way; evidence §6).
  This is the two-humped `load_ms` of the gate's Original configurations (about 125 and 245 ms on
  a quiet CPU; both binaries of the earlier comparison show both humps, evidence §5).
* The warm-up is 140–190 ms of wall time on the first load of a session at Extreme or the maximum
  setup (0.6–0.9 s of CPU on its threads: about 280 programs translated and made, the driver's side
  from its cache), 8–18 ms at Original; later loads of the session find them made (a race from free
  roam: 27–118 ms in all).

### Why `load_ms` grew

Measured on eight binaries interleaved (the old baseline's, the commit before and at each of three
changes, main; 7 runs each, both GPUs; evidence §3). Three changes moved work from the first frame
into the load, each by design, and one stage carries all of it: the load's warm-up
(`drive_in: warm`), which did not exist before. Every other stage's thread CPU is the same in all
eight binaries (`World::load` 5–10 ms, `Car::load_tuned` 15–21, `WorldRenderer::new` 50–71).

| change | pipelines by the load / in the first frame (Original; Extreme) | warm-up wall, integrated GPU (Original; Extreme) |
|---|---|---|
| before | 115 / 62; 115 / 253 | – |
| the scene's pipelines at the frame's sample count built at the load | 158 / 19; 273 / 100 | 6; 126 (on the main thread; 99 once it ran on a worker beside the loading screen) |
| the Enhanced passes' programs made before their first frame | 175 / 0; 395 / 3 | 8; 191 |
| the water's programs and mesh at the load | 175 / 0; 398 / 0 | 9; 183 |
| now | 175 / 0; 396 / 0 | 10; 162 |

`load_ms` and the first frame, integrated GPU, old baseline → now (median [min..max]): Original
204 [169..368] → 185 [149..383] and 26 [22..42] → 18 [15..33]; Extreme 175 [135..311] → 335
[286..599] and 397 [324..1806] → 222 [173..601]; their sum 608 → 594 at Extreme. So at Original
nothing grew, and at Extreme the load took over what the first frame did. The gate's "explore
Original 139 → 258" was the discrete GPU's two humps (above), not a change; "rain max 4K 150 →
337" is the warm-up (148–326 ms there). The first frame's own numbers do not compare across this
range for a second reason: the explore bench's timed frames now start on a fixed line, another
place with another number of probes near it.

### The first frame at Medium and High on the discrete GPU

Not pipelines (none is built in a first frame) and not the pipeline cache. Light Probes (on from
Medium) bake every probe within 70 m when the car arrives where none is baked: a load's first
frame, a jump. Each probe was drawn (six small faces), copied and **waited for** before the next:
5–9 waits for the GPU in the first frame's draw (`bake_probes`; evidence §7). On a GPU to itself a
wait is about a millisecond, which is why the integrated GPU showed 37–88 ms; on the discrete GPU
each wait cost one turn of the other program's queue (25–220 ms each): 280–800 ms.

**Fixed** (`render/enh/lighting/probes/bake.rs`): the burst's probes are each drawn into a
read-back buffer of their own and submitted without waiting, then all maps are waited for together:
one wait for the burst. The draws, their order and the stored cubes are the same (the game test
`the_arrival_burst_bakes_the_same_field_with_one_wait` compares the field bit for bit against the
serial form, which `NFSU2_PROBE_BURST_SERIAL=1` keeps as a check and measuring aid). One binary,
arms interleaved, 7 runs per arm (discrete: the second of two rounds; evidence §4), first frame's
draw and the whole first frame, ms:

| configuration | draw, a wait per probe | draw, one wait | first frame before | after |
|---|---|---|---|---|
| discrete (shared) 1080p Medium, race start | 345 [36..465] | 54 [12..70] | 391 [42..527] | 103 [20..124] |
| discrete 1080p High, race start | 409 [42..471] | 62 [43..79] | 457 [67..535] | 110 [73..134] |
| discrete 4K High, free roam | 241 [9..322] | 49 [17..56] | 301 [21..387] | 95 [43..109] |
| discrete 4K High, race start | 397 [348..453] | 60 [16..74] | 454 [405..519] | 115 [29..136] |
| discrete 1080p Extreme, free roam | 365 [243..401] | 124 [108..143] | 422 [316..468] | 193 [144..223] |
| integrated 1080p Medium, race start | 11 [10..16] | 10 [9..11] | 64 [58..72] | 63 [59..65] |
| integrated 1080p High, free roam | 9 [9..12] | 9 [8..17] | 76 [74..80] | 75 [70..83] |
| integrated 1080p Extreme, free roam | 84 [79..99] | 86 [78..102] | 191 [185..218] | 194 [182..220] |

The low ends of the "per probe" ranges are runs in which the other program was idle for a moment.
`load_ms` is unchanged in every row (evidence), and no later frame takes the work: the burst still
ends inside the frame it started in. On the maximum setup at 4K (three sampled runs each) the
waits fell from 782–1140 ms to 4–67 ms and the draw from 1601–2073 to 595–822 ms.

### Built: programs made ahead, the traced scene at the load, Quick Race loaded ahead, the prepare step

What the list below this section named as left, built (docs/first-use-builds.md "Ahead of the
load", docs/prepare.md). Nothing drawn, heard or simulated changes (the gate: identical).

* **The drive's programs are made from the window's second frame on** (the boot screens, the
  title, every menu), on a thread of their own, for the settings in force, and again when those
  change; what needs a world is made beside the world the menus load ahead, and the water and the
  smoke objects built there are handed to the drive. A load asks as before and finds them made:
  `tests::first_use::a_load_after_the_menus_makes_no_program` (no pipeline and no module in a
  free-roam load after the menus, nor in its frames, at every preset, and after a preset is
  switched to in the menus).
* **The traced scene** (Modern Lighting with RT, traced reflections) is made by the load's warm-up
  and beside a world loaded ahead (`rt::prepare`), not gathered by the region's first frame on the
  main thread (97–105 ms there before). Race start, maximum setup, 1080p, discrete GPU, 5 runs per
  arm interleaved: first frame 261 [251..273] → 164 [153..171] ms (its draw 217 → 118), `load_ms`
  382 [365..389] → 384 [331..389] (the gathering runs on a worker beside the warm-up), the two
  together 649 → 537.
* **Quick Race** loads its track's region ahead from its options and car-select screens.
* **`game --prepare`** and the launcher's step: docs/prepare.md.

Before → after, warm cache, load + first frame, ms, median [min..max]; base and new interleaved
under the machine's measuring lock, load average 3–9; the discrete GPU still shared with another
program (its busy counter 26–100 %: its columns scatter, the integrated ones are the measure):

| wait (load + first frame) | integrated 1080p Original | High | discrete 1080p (shared GPU) Original | High | discrete 4K (shared GPU) Original | High |
|---|---|---|---|---|---|---|
| process start → first frame | 138 [74..201] → 128 [63..194] | 138 [74..201] → 128 [63..194] | 64 [64..65] → 71 [70..72] | 64 [64..65] → 71 [70..72] | 82 [68..95] → 74 [71..76] | 82 [68..95] → 74 [71..76] |
| process start → title, Enter held | 170 [117..222] → 147 [77..217] | 170 [117..222] → 147 [77..217] | 112 [109..114] → 260 [171..349] | 112 [109..114] → 260 [171..349] | 118 [112..124] → 119 [117..121] | 118 [112..124] → 119 [117..121] |
| menu → free roam (after the menus were up) | 93 [92..94] → 80 [67..92] | 184 [184..185] → 146 [146..147] | 104 [88..120] → 128 [123..134] | 110 [80..139] → 78 [42..115] | 70 [58..81] → 85 [54..116] | 100 [91..110] → 136 [129..144] |
| menu → free roam, nothing loaded ahead | 124 [123..125] → 114 [113..115] | 160 [159..162] → 166 [163..169] | 182 [181..183] → 254 [245..263] | 246 [242..249] → 227 [221..233] | 302 [176..427] → 212 [181..243] | 182 [179..186] → 230 [210..251] |
| menu → Quick Race circuit (from the bench: nothing loaded ahead) | 138 [119..157] → 122 [118..125] | 208 [192..224] → 190 [189..191] | 190 [186..193] → 220 [204..235] | 324 [258..390] → 356 [331..381] | 180 [171..188] → 172 [132..212] | 210 [207..213] → 253 [240..266] |
| menu → Quick Race drag (the same) | 104 [85..122] → 123 [93..153] | 138 [134..143] → 112 [94..131] | 164 [145..182] → 212 [206..218] | 210 [132..289] → 202 [192..211] | 118 [116..119] → 156 [130..181] | 118 [116..119] → 136 [113..159] |
| free roam → race, rivals loaded ahead | 54 [53..54] → 54 [54..55] | 202 [196..207] → 210 [208..212] | 40 [40..41] → 59 [55..63] | 44 [41..46] → 56 [53..58] | 39 [31..47] → 57 [41..73] | 64 [64..65] → 79 [69..89] |
| race → free roam, encounter racers loaded ahead | 64 [64..65] → 64 [63..64] | 264 [261..268] → 269 [266..272] | 44 [40..48] → 61 [58..64] | 44 [36..52] → 53 [44..62] | 32 [27..36] → 33 [28..38] | 70 [70..71] → 102 [86..119] |
| free roam → another region's race, loaded ahead | 84 [80..87] → 79 [78..80] | 241 [237..245] → 236 [233..239] | 80 [80..80] → 96 [92..99] | 124 [117..130] → 122 [105..138] | 114 [87..142] → 80 [56..103] | 124 [117..131] → 163 [162..164] |
| that race → free roam, loaded ahead | 79 [70..88] → 76 [72..81] | 353 [327..379] → 299 [268..330] | 76 [76..77] → 101 [91..111] | 170 [157..184] → 132 [102..162] | 80 [70..91] → 117 [98..136] | 136 [135..137] → 162 [159..165] |
| into the garage | 23 [23..23] → 21 [21..21] | 50 [38..62] → 62 [61..62] | 34 [30..38] → 36 [25..47] | 29 [27..31] → 47 [39..55] | 36 [31..40] → 48 [48..49] | 38 [37..39] → 44 [23..66] |
| out of the garage | 9 [8..10] → 9 [9..9] | 18 [17..18] → 16 [15..17] | 18 [17..20] → 30 [26..34] | 35 [30..40] → 34 [34..35] | 26 [25..28] → 36 [32..40] | 30 [29..30] → 38 [32..44] |
| into a shop | 18 [17..20] → 20 [19..21] | 25 [25..25] → 26 [25..27] | 32 [26..38] → 33 [30..36] | 33 [33..33] → 46 [42..51] | 34 [27..42] → 40 [27..53] | 36 [31..41] → 18 [17..19] |
| out of a shop | 22 [19..24] → 19 [19..19] | 52 [50..53] → 53 [53..53] | 19 [18..20] → 35 [31..39] | 40 [39..42] → 40 [34..46] | 40 [26..53] → 40 [34..46] | 42 [35..50] → 58 [55..62] |
| out of a shop, a part fitted | 66 [55..76] → 52 [50..53] | 118 [115..120] → 113 [112..114] | 66 [65..66] → 90 [82..98] | 125 [113..137] → 155 [144..166] | 101 [67..135] → 75 [71..79] | 102 [93..110] → 141 [123..159] |

2 runs per arm (the measuring lock was shared with two other series; more were not to be had).
The tree as it is now, 3 runs (the Quick Race rows start from the car select, as a player does):

| | integrated 1080p Original | High | discrete 1080p (shared GPU) Original | High |
|---|---|---|---|---|
| process start → first frame | 68 [64..71] | = | 65 [65..66] | = |
| process start → title, Enter held | 81 [79..82] | = | 102 [75..106] | = |
| menu → free roam (after the menus were up) | 60 [59..61] | 131 [108..134] | 64 [26..74] | 91 [32..92] |
| Quick Race circuit from its car select | 66 [66..67] | 143 [140..146] | 76 [76..88] | 141 [129..142] |
| the same start with nothing loaded ahead | 112 [106..119] | 188 [184..193] | 175 [170..187] | 270 [268..273] |
| Quick Race drag from its car select | 54 [53..54] | 116 [114..117] | 63 [63..76] | 103 [102..107] |
| the same with nothing loaded ahead | 55 [53..56] | 132 [129..134] | 110 [108..117] | 137 [124..162] |

The bench's "nothing loaded ahead" rows (a load fired the moment a menu opens, which no player
does) are in the evidence; they change little, as expected: there the load still does the work.

**What is under 200 ms and what is not.** With the menus' work ahead done (0.2–0.5 s after a menu
opens), the after-arm's medians of every wait a player has (the "nothing loaded ahead" rows left
out): integrated GPU Original 9–80 ms, Low 8–86, Medium 22–212, High 16–299; discrete GPU 1080p
30–128, 32–123, 24–160, 34–155; discrete 4K 33–117, 24–110, 34–122, 18–163. So **every wait is
under 200 ms on the discrete GPU at every level at 1080p and 4K, and on the integrated GPU at
Original and Low**. Over the line, on the integrated GPU only: at Medium the return from another
region's race (212, before 234), at High the four waits that follow a menu drawn over the 3-D
world (race ↔ free roam behind the Engage dialog or the post-race menu: 210, 236, 269, 299;
before 202, 241, 264, 353). There a frame takes 50–100 ms and the load begins by waiting for the
one or two frames in flight; the load itself is 50–130 ms. It needs a faster frame at that level
(a lower level, or a lower resolution, on that GPU), not a shorter load. On tonight's calmer
discrete GPU the before-arm is under 200 ms in these rows too: what changed is the first load of a
session at Medium and above (integrated High 184 → 146, then 131 with the smoke and water
handed over as well), Quick Race (112 → 66 and 188 → 143 integrated, 175 → 76 and 270 → 141
discrete), and a first start (below). The discrete columns of the first table are two runs on a
GPU whose other user came and went: differences under about 40 ms there are not findings.

**The menus while the work ahead runs** (`--bench menu`, integrated GPU, main menu frames = update
+ draw, 3 runs; whole machine / two cores by `taskset`):

| | work ahead takes | longest menu frame during it | p99 / longest menu frame after it |
|---|---|---|---|
| Original, programs ahead | 176 [155..209] / 232 [204..272] ms | 18.5 [16.2..18.6] / 9.8 [8.3..15.4] ms | 11.7 / 13.7 and 10.4 / 11.7 ms |
| Original, without them (loads ahead only) | 174 [142..254] / 275 [262..334] ms | 9.2 [8.6..15.1] / 11.2 [10.7..11.7] ms | 11.8 / 13.4 and 10.0 / 11.4 ms |
| High, programs ahead | 307 [289..361] / 508 [487..529] ms | 24.1 [22.1..25.4] / 23.6 [23.2..24.1] ms | 24.6 / 26.6 and 30.8 / 40.0 ms |
| High, without them | 141 [136..153] / 333 [309..357] ms | 20.4 [19.3..20.9] / 2.7 ms (no menu frame of its own scene yet) | 25.2 / 26.5 and 28.8 / 36.8 ms |

No menu frame during the work is longer than the menu's own frames after it, except one frame at
Original on the whole machine (18.5 ms against a steady p99 of 11.7 ms; under a 60 Hz frame).
Memory, peak in the menus with / without the programs ahead: resident 879 / 848 MB (Original),
909 / 884 MB (High); the process's GPU memory 647 / 591 MB and 871 / 820 MB (the water and smoke
objects and the renderer's warmed targets exist earlier; a drive holds them anyway).

**A first start** (empty caches, High, discrete GPU, 3 runs): the first load of a new process
1455–1491 ms; after `game --prepare` (1.7–2.2 s once, 0.6–1.0 s when run again) 397–425 ms, a warm
load. Without the prepare step the same programs are compiled behind the boot screens and menus
from the second frame on.

**A cold file cache** (the game's data files, the cache folder, the game binary and the driver's
cache dropped from the OS's cache with `posix_fadvise(DONTNEED)`, 2.1 GB, before each run; SSD;
integrated GPU, Original, 5 runs, warm → cold): process start → first frame 83 [81..88] → 238
[236..245] ms, → title 148 [146..153] → 340 [334..342] ms; the first free-roam load of a process
that loads at once 160 [159..168] → 375 [358..393] ms (`Game::new` 23 → 77 ms, the track from the
cache 21 → 110 ms, the rest in the cars and the renderer's first touch of the mapped cache). In
the game the menus' load ahead reads the same pages before the load. A hard disk was not measured:
open. No file layout change was made: on this SSD the whole cold penalty of a start is 0.16–0.19 s.

### A start after a reboot, the real flow (the shipping game, the title's hold included)

The shipping game in a window on a private display, keys as a player in a hurry presses them (Enter as
soon as each boot movie takes it, Enter at the title until its native 4 s hold lets it through, the
profile message, the main menu 1 s, then Career → the garage → free roam, or Quick Race → a race); times
from the game's own log (key → the new screen's first frame), the main thread's state sampled every 4 ms
from /proc (running, waiting for another thread, blocked on the disk). "After a reboot": the game folder
and the run's own cache folders (asset cache, driver shader cache) dropped from the OS's file cache
(`posix_fadvise(DONTNEED)`); the game binary, the driver's libraries and the kernel stay warm. The machine
was shared (load average 17–68 on 16 threads, the disk's I/O pressure "full" 2–55 %): the wall times are
upper bounds; which thread waits for what is the trustworthy part. Tables:
`docs/evidence/perf-budget/read-ahead.txt`.

* **Which waits grow after a reboot** (integrated GPU, Original, 6–7 runs; median [min..max] ms): the
  start to the first frame 310–536 [189..1303] (the main thread blocked on the disk 112–182 ms of it);
  each boot movie's open on Enter, which reads the whole file on the main thread (the intro 178–179
  [167..360], the PSA 77–80 [43..98]; with the files in memory 12–19); the main menu's first frame
  306–359 [169..893], spent waiting for the garage visit's FE car load on its thread; the garage
  84–107 [36..485]; free roam's entry 355–406 [115..836], spent waiting for the encounter racers' cars.
  The city's track cache and the player's car are read by the menus' work ahead (`Preload`) during the
  boot screens and the title's hold: in every run they were in memory by the time anything else asked.
* **The read-ahead** (docs/read-ahead.md: after the first frame, the boot movies, the main menu's FE car
  and showroom, the garage's platform, free roam's first files; ~50 MB): after a reboot, Original,
  integrated GPU, 6 interleaved pairs off → on: the intro's open 178 [167..360] → 18 [13..179], the PSA's
  80 [70..88] → 16 [8..57], the main menu's first frame 359 [261..893] → 94 [25..307] (the wait for the FE
  car's thread 298 → 33 ms), the garage 107 [48..485] → 44 [23..300], free roam 406 [211..836] → 332
  [105..670] (spreads overlapping), the start 310 → 346 (the reader starts after it). Every Graphics row
  up at 4K, discrete GPU, 4 pairs: intro 196 → 18, main menu 192 [126..617] → 51 [34..337], garage 70 → 34.
  A first start ever (3 pairs): intro 306 → 17, main menu 180 → 48, garage 86 → 40. With the disk
  saturated by other programs (I/O pressure 35–55 %, 7 pairs) no wait moved beyond the spread; with
  the files in memory nothing got slower (8 pairs). The reader's queue is done 0.4–3.8 s after the
  first frame (it gave way to long frames for up to 1.5 s of that).
  A first version that also read the start's own files beside `Game::new` made the start no faster
  (`Game::new` 304 → 395 ms median, spreads overlapping): left out; in that version's 7 pairs the main
  menu did not improve either (306 → 397), unlike the three later series: not explained.
* **On a hard disk** (estimate from the bytes and the order, not measured): a movie's open costs a seek
  and its size at ~100 MB/s (the intro ~0.15 s, the PSA ~0.06 s), the showroom ~45 ms, the FE car's ~15
  MB ~0.2 s, the garage's platform ~35 ms, free roam's ten small files ~0.15 s of seeks. The list reads
  in about 0.8 s (0.5 s of bytes, 24 files' seeks), inside the boot screens and the title's hold; after a
  slow piece the reader pauses as long, so it never takes more than half of a slow disk's time.
* **The first start's menu freeze** (empty caches, every Graphics row up at 4K): in the real flow the
  title's hold gives the programs made ahead (`ProgramsAhead`) 5 s or more; the main menu's first frame
  took 417 and 3558 ms on the discrete GPU (load 20 and 28) and 268–272 ms on the integrated GPU; at
  Original 113–529 ms. The 3.6 s case is the CPU saturated by other programs: the frame waited 1.7 s for
  the programs thread and ran 1.45 s itself. The harness's fastest player (no title hold, load 49–68):
  the garage scene's own set alone (programs ahead off) is 180–372 pipelines, 3.1–14 s; with the
  programs ahead running 2.6–19 s. Ordering the garage's programs first therefore cannot remove it
  under such load: what is left open is a loading screen (the 200 ms rule) in place of the frozen frame,
  and the launcher's prepare step, which removes it.

### The start and the menus: side by side and made ahead (the shipping game, keys)

docs/start-and-menus.md (what changed and why it is the same work). The shipping game in a window on a private
display, the files read right before each run, before and after interleaved, 7 runs a cell (tour F 5), the wait
lines' "from the key", median [min–max] ms; load average 12–45 on 16 threads, the discrete GPU shared with another
program at times; integrated-GPU numbers not split by its slow state (the waits here are CPU and disk bound).
Tables, the first start and the harness's transitions: `docs/evidence/perf-budget/start-and-menus.txt`.

| wait | discrete, owner rows 4K | discrete, Original | integrated, owner rows 1080p | integrated, Original |
|---|---|---|---|---|
| the start's window part (event loop to swap chain) | 93 → 29 | 55 → 18 | 75 → 22 | 48 → 19 |
| process start → first frame | 154 → 162 | 92 → 80 | 136 → 104 | 92 → 88 |
| profile message → main menu | 72 → 42 | 47 → 29 | 74 → 33 | 36 → 26 |
| garage → free roam, first of the session | 259 → 87 | 39 → 37 | 290 → 127 | 54 → 43 |
| its first frame | 198 → 43 | 17 → 19 | 167 → 77 | 14 → 16 |

The start's total moves less than its window part: the game's data (`Game::new`) now ends with the event loop
made beside its loaders (its part grows by what the event loop took), and its spread is the disk's. The
harness's transitions at the reference player's rows (discrete, 4K, 3 runs each): menu → free roam with the world loaded
ahead 158 → 72, another region's race loaded ahead 140 → 108, back from it 306 → 221, menu → Quick Race circuit
422 → 324; the first frames' "world: lamps" 61–69 → 18–19 ms (Surface Detail's array zeroed on the GPU).
The table's "after" is an earlier build in which the main menu's scene made ahead also made its renderers; the
merged build leaves those to the menu's first tick (the GPU stays with the title's frames). Its own runs (owner
rows, 3 an arm, during a gate's builds): free roam's first entry 135 → 56 (discrete) and 213 → 84 (integrated),
the main menu 48 → 42 and 26 → 33 (within the spread); the title's largest frames and its sound the same in both
arms (the evidence file).

### Left, with the measured size

* **The maximum setup's first frame at 4K**: 250–620 ms of it is the main thread
  blocked inside the driver's submit call, eight submissions of 25–70 ms each, on the shared
  discrete GPU. Inferred, not traced: the kernel making room in video memory for the frame's new
  render targets while another program holds most of it. Whether it happens with the GPU to itself:
  open.
* **The benches' first Extreme frame** carries Surface Detail's maps: with the row off its draw is
  27 ms instead of 98–225 ms (integrated GPU, 2 runs each; "world: lamps" 55–63 ms and the main
  view 60–68 → 6 ms). The headless modes load the maps before the first frame for deterministic
  pictures (`surface_maps::set_blocking`); the window makes them in the background (from the code,
  not measured here).
* **The load's one wait for the GPU** (2 ms on the idle integrated GPU; 34–61 ms on the shared
  GPU) frees the uploads' staging memory before the first frame, and the texture arrays' upload
  runs on the load's workers while the main thread waits for them; with the world loaded ahead in
  the menus both happen on the menus' thread, not in the player's wait. A free-roam load uploads
  68 arrays (49.8 MB), each level through a staging buffer of its own and one submission per
  array. Fewer, larger ones, measured on the integrated GPU (`--bench explore 60`, Original and
  High, a measuring build with the variants behind a switch (not kept), 7 runs each interleaved; the
  upload's wall time, median [min..max]): as now 15.9 [12.6..37.8] / 15.6 [13.5..41.5] ms; one
  staging buffer per array 14.6 [12.8..30.1] / 13.6 [12.6..39.6]; one submission for all arrays
  20.4 [15.1..56.5] / 25.6 [16.9..67.2]; both 21.6 / 16.4. The load's wait 2.3–3.1 ms in every
  variant; `load_ms` scattered 150–830 ms with the machine's other work in every variant. No
  gain beyond the spread on the idle GPU (one buffer per array: 1–2 ms, inside it; one
  submission: slower, **inferred**: the GPU's copies then start only once the last array's
  staging is written):
  not built (evidence: `docs/evidence/perf-budget/car-change-ahead.txt` §3).
* **A load's last frames in flight**: a load begins by waiting for the frames already on the GPU.
  Where a frame takes 50–100 ms (High on the integrated GPU) that alone is 60–200 ms of a load
  started from a menu drawn over the world; it needs a GPU that draws the level faster, not a
  shorter load.
* **The shop exit with a part fitted** on the integrated GPU at High: the GPU's pace (two shop
  frames in flight and the first frame, 57 ms); the load's own work is made in the menus now
  (below, "A car change made ready in the menus"). A load with no menu frame since the drop (the
  developer harness, the dyno's test run) still resets the world in place (13–15 ms, 5.5 of it the
  ground read and dropped). The smoke's GPU half and the water's Realistic and Next-Gen halves are
  made new in the menus, not kept with a reset of their GPU state (no test could prove such a
  reset equal to a new one without comparing the fields, which a new one makes unnecessary).

### Kept across a car change: Animated Water

A car change on the same world (the shop exit with a part fitted, a career's new car) drops the
drive and makes a new one; its objects were all new, so Animated Water's mesh and GPU half (its
buffers and the layers' textures with their mipmaps, made on the CPU) were built again by the
load's warm-up. They are now kept with the world (`RenderState.resident_water`,
`Water::for_new_drive`): the mesh and the GPU half hold nothing that changes while driving (the
frame's uniform is written by each frame's `prepare` before its draw), the frame's draw list and
the Next-Gen tick pick start empty, and the Realistic and Next-Gen halves are dropped and made
again as for a new drive. Kept only with the kept world renderer (the same device).
`tests::first_use::a_water_kept_across_a_car_change_is_a_new_one`: two games do the same exit at
the fountains, one taking the kept water and one with it dropped: the same water state and the
same pictures of the first five frames at High, Ultra and Extreme (a variant that keeps the whole
water fails it at Ultra).

`--bench transitions` (`NFSU2_BENCH_MENU_FIRST=1`), the integrated GPU, one binary per arm,
interleaved, 7 runs each, each round under the measuring lock; ms, median [min..max]:

| level | transition | before: load / + first frame | after |
|---|---|---|---|
| High | out of a shop, a part fitted | 105 [103..126] / 110 [108..132] | 74 [66..147] / 80 [71..165] |
| High | out of a shop (control) | 19 [17..44] / 58 [48..86] | 28 [18..59] / 64 [46..89] |
| High | free roam, first load (control) | 105 [99..118] / 134 [122..150] | 101 [96..127] / 134 [127..157] |
| Original | out of a shop, a part fitted (no water) | 69 [47..147] / 71 [49..162] | 76 [52..152] / 78 [53..166] |

The Original row and the controls move within their spread. Memory: the kept water is what the
drive held, kept while the drive is dropped (no second copy).

### A car change made ready in the menus

Where the shop exit with a part fitted went (integrated GPU, High, one traced run of the tree before
this work, `NFSU2_TIMING`; ms of wall / of the main thread's CPU): the kept world's reset 14.8 / 14.2
(`World::reset_to_loaded`; its read split with temporary marks, 8 reads of the city's world in one
run: of 12.4–14.6 ms the ground's packs 5.3–6.1 — which the reset reads and drops: it keeps the
kept world's ground —, the collision faces 3.3–4.8, the placements 1.9–2.5), the renderer's
placements 8.2 / 8.1 (`reset_placements`), the car's renderer 0.9, the smoke's GPU half 1.8, the
uploads' wait 20–67 (**inferred**: the frames the GPU was still drawing), and with nothing loaded
ahead the car's load 9 and the encounter racers' 5. In the game the drop is followed by menus (the part's question,
the browser again, the exit's question), which already loaded the new car and the encounter racers
ahead; the rest is now made there too (docs/first-use-builds.md "Kept across a car change", `loads::kept`):
the world's reset, the renderer's instances and the world's emitters, the water's and the smoke's
per-drive halves on a worker from the first menu frame, the car's renderer on the preloaded car's
thread. A load with no menu frame since the drop does all of it in place as before.

**The developer harness's transitions run their menu frames unpaced**: nothing holds them to the
window's two frames in flight, so a load after them first drains a deeper backlog than a player's
(the first measurement of the row below: 96–188 ms at High on the integrated GPU, wall time equal
before and after while the main thread's work fell from 28.5 to 1.8 ms). The new row paces its shop
menus as the swap chain does (`throttle`, two frames in flight); the other rows are unchanged.

`--bench transitions` with `NFSU2_BENCH_TRANSITIONS_SHOPS=1` (`NFSU2_BENCH_MENU_FIRST=1`), one binary
per arm built with the same harness, interleaved, 7 rounds under the measuring lock (load average
8–14 from other work); load + first frame wall ms and `drive_in`'s main-thread CPU ms, median [min..max]:

| configuration | out of a shop, a part fitted, after the shop's menus: before | after | main thread CPU before → after |
|---|---|---|---|
| integrated 1080p Original | 41 [35..43] | **30 [29..31]** | 30.4 → 1.1 |
| integrated 1080p High | 58 [56..90] | 57 [56..91] | 26.1 → 1.4 |
| discrete 1080p (shared GPU) Original | 30 [30..36] | **12 [11..12]** | 20.4 → 1.4 |
| discrete 1080p (shared GPU) High | 36 [34..46] | **15 [13..16]** | 22.0 → 1.0 |

The row without menu frames between the drop and the exit (the harness's old "out of a shop, a part
fitted") is unchanged, as designed (integrated Original 55 → 52, High 77 → 75, discrete 44 → 46 and 50 → 50 ms; main thread 21–29 ms in both arms). The shop's menus' longest frame while the work
runs: integrated 10.7 → 10.9 and 47.9 → 48.4 ms, discrete 2.5 → 2.8 and 2.4 → 2.8 ms (the work is on workers; on the integrated GPU the menus' frames are the GPU's either way). On the integrated GPU at High the exit stays at the GPU's pace: its wall time
is the two shop frames in flight (48 ms each there) and the first frame, with the main thread's
work under 2 ms now; under 50 ms there needs a cheaper shop frame at that level, not a shorter load.

Memory: the objects made ahead are the next drive's own (its smoke and water halves, its car's
renderer), made while the dropped drive's are freed, and held from the menus instead of from the
load (**inferred**, not measured: no rise of the peak; the menus' garage scene's own car is held
beside them as before). Not built: skipping the ground's read in the reset (it would need the
world cache's layout to give the ground's length or put it last) — every car change a player makes
now resets in the menus.

### Ambient Detail's props: made in the menus, kept for the drive

With the Advanced row Ambient Detail on, a drive's first frame placed the street props again on its
main thread whenever the world's placements differed from those the menus' preload had placed them
for: every load that took a preloaded world (the first free roam of a session, free roam after a race
in another region, Quick Race from its car select, another region's race) — about 0.29 s of a
0.45–0.5 s race start at 4K. Attribution (`--bench transitions` with `NFSU2_AMBIENT_STATS=1`, the
base binary): each such load printed two placements, the preload's and one inside the first frame's
"world: road reflection, main view" span (287–370 ms of a 365–423 ms span). What changed
(docs/ambient-detail.md "When the props are placed"):

1. **The slot decisions once per (texture, worldreflect)** instead of once per material of every
   placement (the material class and the name tests): the city's surfaces 16–22 ms of CPU instead of
   ~0.2 s. Same triangles in the same order, same props
   (`ambient_props::tests::kept_slot_decisions_give_the_same_props`, the city as loaded, at free
   roam's and a race's barriers, forward and backward, and a drift track).
2. **What the props are made from** (`SurfaceInputs`: the solids of use and, for each placement
   drawing one, its hidden flag, section and transform): in the city none of them is switched by any
   barrier set-up (all 146 its groups name), so the props the menus placed are the drive's; on a drift
   track, some drag strips and the URL circuits the race's barriers switch road pieces on and the
   load's warm-up places them again, a job of its own (`AmbientWarm::run`). The first frame checks
   again and places them in place only if the world changed after the warm-up.
3. **The ground grid and the props' cells with the fast hasher** (`fxhash`; both are only looked up,
   never walked): the city's ground 85–101 → 46–57 ms of CPU.

`--bench transitions` (`NFSU2_BENCH_MENU_FIRST=1`), one binary per arm, interleaved, 7 rounds each under
the measuring lock (each hold under 10 minutes). 4K: a player's own options file at 3840×2160 (Ambient Detail
Next-Gen, Modern Lighting, Light Probes, Next-Gen water), the discrete GPU, load average 13–25 at the run starts
and the GPU's busy counter up to 99 % (another program on it): the wall times are an upper bound, the main
thread's CPU (the load and its first frame) is the trustworthy column. Wait = the call to its first frame drawn,
median [min..max] ms (`docs/evidence/perf-budget/ambient-props-ahead.txt`):

| 4K, discrete GPU | wait before | after | main-thread CPU before | after |
|---|---|---|---|---|
| Quick Race circuit from its car select (the race start a player makes) | 637 [504..1015] | **271** [203..323] | 435 | **144** |
| free roam, the first load after the menus | 602 [461..929] | **208** [156..532] | 418 | **124** |
| menu → free roam, preloaded | 530 [448..900] | **186** [118..374] | 406 | **116** |
| back to free roam from another region's race, preloaded | 690 [617..834] | **361** [257..687] | 419 | **120** |
| another region's race (a URL circuit: placed again in the load), preloaded | 291 [188..578] | 197 [165..459] | 143 | **106** |
| free roam → race (the world kept: no placement before either) | 251 [67..293] | 149 [75..268] | 38 | 37 |
| Quick Race drag from its car select (no placement before either) | 212 [135..680] | 239 [147..268] | 92 | 82 |
| menu → free roam, at once (placed in the load before and after) | 726 [544..2135] | 502 [447..806] | 183 | 200 |
| back to free roam from another region's race, at once | 642 [589..804] | 725 [433..1860] | 188 | 246 |

The integrated GPU (the reference 2-CU iGPU) at 1920×1080, the Enhanced level High with Ambient Detail on,
load average 10–35, the GPU idle at the run starts:

| 1080p, integrated GPU, High + Ambient Detail | wait before | after | main-thread CPU before | after |
|---|---|---|---|---|
| Quick Race circuit from its car select | 575 [394..1490] | **164** [140..382] | 344 | **53** |
| free roam, the first load after the menus | 644 [505..869] | **312** [130..631] | 463 | **46** |
| menu → free roam, preloaded | 1101 [424..2200] | **182** [106..222] | 494 | **32** |
| back to free roam from another region's race, preloaded | 927 [609..4160] | **456** [346..1120] | 309 | **42** |
| another region's race (a URL circuit: placed again in the load), preloaded | 313 [273..1032] | 292 [230..518] | 106 | **52** |
| free roam → race (the world kept: no placement before either) | 182 [63..405] | 270 [68..827] | 28 | 30 |

The integrated GPU's wall times scatter widely in both arms (single loads of 1–9 s with the machine's other
work); the main thread's CPU moves only where a placement left the first frame, and stays within its spread on
the rows that never had one (the kept world's race, the shops, the garage).

On the 4K series the last row is not resolved: the new arm splits into three runs faster than every
base run (433–459 ms) and four slower ones whose first frame (which places nothing either way) is
slow too (150–490 ms against ~100), which points at the shared machine (other programs dropping the
game folder from the file cache, the shared GPU); the warm-up's own spans (`NFSU2_TIMING`) put the placement
there at 0.19–0.21 s as its own job against 0.3–0.65 s inside the world renderer's before. "At once"
is a load fired before the menus' preload is done, which no player path measured reaches.

A city placement now: surfaces 16–22, the record 2–4, the scatter 22–30 on the calling thread (rayon;
90–160 ms wall at load average 76), the ground 46–57 ms. Memory: the record, ~0.18 MB per world
(2 316 placements of use × 76 bytes, a flag per solid); the slot decisions are freed after the
placement; no second set of props is held (a stale set is dropped before the new one is placed).

### Ambient Detail's props on a world kept for the next drive

A world kept for the next drive (the last race's region: the drive itself, its world and renderer for another car,
or the world kept beside the menus after a car change) carried the last drive's props unchecked, so a second Quick
Race on another drift track or URL circuit of the region drew the first race's props (or debris ground). A new drive
now checks them as props made ahead are checked (`WorldRenderer::new_drive_ambient`, then the warm-up's job and the
first frame; docs/ambient-detail.md "When the props are placed"): placed again in the load when the record differs,
kept when it matches. Cost (`docs/evidence/perf-budget/ambient-kept-world.txt`: `--bench transitions` with
`NFSU2_BENCH_TRANSITIONS_KEPT=1`, the integrated GPU at 1920×1080, the Enhanced level High with Ambient Detail On,
before = this tree without the check; load average 14–37):

| the load | warm-up before | after | its Ambient Detail job, thread CPU | the wait (7 rounds) before | after |
|---|---|---|---|---|---|
| another drift track, the drive kept (L4RC 4312 → 4311) | 3.1 [1.2..37] | 28.5 [12..207] | 13.6 | 94 [77..311] | 117 [85..259] |
| another drift track, another car (→ 4313) | 3.2 [2.2..15] | 24.5 [13..79] | 9.7 | 148 [126..270] | 128 [120..254] |
| another URL circuit, the drive kept (L4RG 4701 → 4712) | 2.8 [1.1..42] | 24.7 [18..53] | 19.0 | 154 [82..418] | 107 [87..264] |
| another city circuit, the drive kept | 2.1 [1.3..21] | 1.6 [1.5..85] | 0.5 (the record's comparison) | 87 [72..511] | 81 [76..195] |

The affected loads get the placement on the warm-up's own job: ~20–30 ms of warm-up (10–59 ms of placement with its
rayon workers), below the wall times' spread on this shared machine (the rows whose code is the same in both arms move
by up to ±130 ms). The main thread places nothing; a city drive pays the record's comparison only (0.2–0.9 ms on the
job and once in the first frame, as a drive with props made ahead does). Memory: unchanged (the stale set is dropped
before the new one is placed).

### Lamp Shadows' layers over a kept drive's barrier switch

A lamp's shadow layer is drawn once, when it is handed out; a drive kept on the same world kept its layers over the
next drive's barrier switch. A set-up's `update_placement` now gives up each held layer whose view holds a placement
it shows, hides or moves and the layers draw (docs/lamps-shadows.md); it is handed out again and drawn at the next
steps. Cost on the integrated GPU, Lamp Shadows Low (the High preset), GPU timestamps of the lamp shadow passes, 7
runs (`docs/evidence/perf-budget/ambient-kept-world.txt`): L4RG's URL circuits 4701 → 4703 from one grid, 3 layers
drawn again, +0.59 ms [0.53..0.84] over the switch's first two frames (the worst frame 0.59 against 0.20); a drag strip
pair, 1 layer drawn again, +0.1–0.2 ms; free roam after a city circuit, none (the barriers switched there are
blended, in no layer). The frames where nothing switched draw nothing more; the check itself runs in the set-up only.

## The budget table's findings fixed (2026-11-05)

Main + the frame-budget fixes. Measured with this work's harness (`perf.sh bench`, the
`max` / leave-one-out presets of `budget.py`, reference desktop GPU, GPU busy median of the run
medians; load average 8–60 from other jobs: GPU medians move ≤ ~1 %, CPU columns
are not used here). Every fix is exact: the Original path is untouched, and the
enhanced images are byte-identical (below).

### 1. Headlight Beams Off slower than On (Modern Lighting's car headlights)

* **Cause** (traced in the code, not native: Modern Lighting is a clone deviation):
  with Headlight Beams off, `modern_light_bold::car_lights` adds every other car's
  headlights as Modern Lighting cluster lights (kind 3, `SUB_HEAD`, reach 28 m, cone
  cos 0.82); with Beams on, the beam pass draws them instead. The clusters binned each
  headlight as a 28 m sphere around the lamp (`lamps::build`), although world.wgsl
  lights only its cone (`ml_now`: 0 past the cone, `ml_near_pool` 0 past the reach).
  Every pixel of those clusters looped over the traffic's headlights for nothing.
  On this work's binary (99e726f) the RT part also traced these lights (+2.6 ms at 4K);
  the shadow-softness pass (177f22d: rays only to the strongest non-car lights) removed most of that:
  on <base> Off was +0.41 ms at 4K (main view 13.61 vs 13.30 ms).
* **Fix**: `lamps::bin_sphere` — a car headlight is binned with the smallest sphere
  around its spherical sector (centre reach / (2 cos θ) along the axis, the same
  radius: 17.1 m instead of 28 m, a fifth of the volume, and in front of the car).
  The candidates, their order and the upload are unchanged; a cluster loses only
  lights that cannot reach it (cut lists, i.e. overflow / Modern Low's first-N,
  now hold more of the lights that do).
* **Result** (rain `max` + Beams 0, 4K, 3 runs each): base 29.12 → 28.84 ms; main
  view 13.61 → 13.27 ms. Beams On − Off at 4K: −0.41 (base) → −0.08 ms — the
  row no longer makes the frame slower when turned off. Images: rain `max` + Beams 0
  1080p, 7 frames, base vs the frame-budget fixes **0 pixels differ** (and base vs base 0).
* Test: `render::lamps::tests::headlight_bin_sphere_holds_its_cone` (10 000+ points
  of the sector inside the sphere; other lights keep their reach).

### 2. Light Probes re-baking in the fountain orbit

* **Cause** (`render/probes.rs`): the arrival test ("no probe within 40 m baked")
  was `near(eye, 40 m).all(unbaked)`, true also when **no** probe is within 40 m;
  then the burst baked every probe within 70 m (up to 48, *baked ones included*)
  synchronously, every frame. The orbit's camera flies 36 m around the fountain off
  the road splines: 818 bakes of 118 distinct probes in 600 frames (`NFSU2_PROBE_LOG`).
  Probes were also baked around the camera, not the car they light.
* **Fix**: `ProbeField::plan` — an arrival needs probes in reach, all unbaked; the
  burst takes unbaked probes only; a baked probe is never baked again; the bake
  centre is the player's car (the first headlight pool; the eye without a car).
  Gameplay cameras stay within ~10 m of the car, so the probes a car uses should be
  the same (**inferred**: they are baked before they come within 40 m of the car;
  the gate's shots and enhanced images, below, compare base and the frame-budget fixes).
* **Result** (water `max`, 600 frames, 3 runs): bakes 818 → 100 (each probe once);
  "world: light probes" GPU 1.08 → 0.025 ms/frame at 1080p, 0.52 → 0.02 at 4K; GPU
  median 4.98 → 4.78 (1080p), 18.24 → 17.89 ms (4K); p95 12.07 → 5.93 ms (1080p),
  21.9 → 21.1 (4K; the base's spikes, frames 400–450 of the orbit, are gone). (this work's
  2.7 ms pass / 45 ms p95 were on 99e726f; <base> already measured lower in the
  first 300 frames, the re-bakes start later in the orbit.)
* Test: `render::probes::tests::baked_probes_are_never_baked_again`.

### 3. The Graphics page's cost hints

The hint of every row the table measures is now its 4K number under "everything max"
(the tier's cost in the player's play setup with every newest row on; intermediate
tiers as (max − Off) − (max − tier)); before, most were measured alone over
Original, several far off (SSAA 2x 0.01 vs 10.15 ms). Measured on the frame-budget fixes' binary
where the row changed since 99e726f or the tier was not in this work's table (29 configs,
3 × 300 frames at 4K, internal evidence), else this work's. The list
with scenes and sources is internal evidence; the test
`frontend::graphics_table::cost_tests::cost_hints_follow_the_budget_table` keeps every
listed hint within 1.5× or 0.3 ms of it (45 tiers).

| row | tier: old hint → new (4K ms, scene) |
|---|---|
| Anti-Aliasing | SSAA 2x 0.01 → **10.15**, 4x 1.57 → **28.39** (racing; over native 8x MSAA); 8x / 9x / 16x 5.87 / 6.6 / 14.65 → 66.3 / 75.8 / 142.2 (**derived**: 4x's ~9.5 ms per extra screen of pixels, not measured) |
| Modern Lighting | Low 0.82 → 1.06, Modern 2.78 → 2.06, Modern + RT 5.58 → **8.99** (racing) |
| SSR | High 1.41 → 3.27, Ultra 2.1 → **5.22** (rain), Ray Traced 3.3 → 7.87 (Ultra + this work's +2.65 step) |
| Particles | High 0.13 → 2.37, Ultra 0.23 → 3.68, Cinematic 0.35 → 1.35 (explore burnout; the volumetric smoke's Cinematic is the cheapest heavy tier) |
| Wet Road Interaction | On 0.76 → 1.13, High 0.82 → 1.35 (rain; this work's 2.65 was on 99e726f) |
| High Precision Post | High 1.89 → 1.84, Ultra 2.5 → 2.79 (racing) |
| Motion Blur | Camera 1.13 → 1.61, Per-Object 1.16 → 1.93 (racing) |
| Surface Detail | Materials 0.04 → 0.88, + Normals 0.09 → 1.47 (rain) |
| SSAO | 0.55 → 0.91 / 0.93 / 0.96 (racing) |
| Lamp Shadows | Low 0.16 → 0.23, High 0.33 → 0.47 (racing, n.s.) |
| Contact Shadows | 0.02 → 0.94 (race start) |
| Volumetric Fog | Low 0.27 → 0.12, High 0.46 → 0.72 (rain) |
| Enhanced Glass 0.33 → 0.65; Enhanced Paint 0.01 → 0.15; Car Reflection Ultra 0.03 → 0.17; Car Shadow Ultra 0 → 0.17; Smooth Car Shapes 0 → 0.15 | race start, this work |
| Enhanced Reflections 0.01 → 0.23; Road Reflection Ultra 0.04 → 0.21 | racing, this work |
| Animated Water | On 0.62 → −0.67 (shown as a saving: the native water draws leave the main view), Realistic 1.65 → 0.65 (fountain orbit, net of the native draws) |
| Headlight Beams 0.21 → 0; Light Probes 0 → 0.2; Rain Detail 0.24 / 0.27 → 0.11 / 0.12; Spray 0.25 → 0.01; Wet Car 0.02 → 0; Lens Drops 0.01 → 0; Wet Roads 0.04 → 0; Dynamic Lamps 0.03 → 0; LOD Fade 0 → 0.1; World Detail Extreme 0.1 → 0 | below the noise under max (n.s.) |

Ambient Occlusion since its passes were made cheaper (docs/perf-integrated-gpu.md, "Ambient
Occlusion: the same occlusion for less"; internal evidence): its four
passes at 4K with every row up 1.01 → 0.80 ms racing, 1.01 → 0.81 rain (the GPU timer rows; the hint
0.96 stays within the test's 0.3 ms).

Kept (not in the table): FXAA / SMAA / MSAA / sharpen / TAA / upscaling / FSR / render
scale / HDR Output, and Depth of Field (1.3: it draws only in scenes,
replays and photo mode; the racing leave-one-out, 0.02, is not its scene).

### 4. Shop part changes (Customize)

* **Measured**: the garage builds a changed FE car on a thread (`draw_garage`:
  `Car::load_upgraded` + its renderer; the frame never waits). The part shows when
  the build is done. the maintainer's probe `part_latency` replays that work (350Z): 17–25 ms
  load + 3–7 ms renderer = 21–32 ms per change, first and repeated alike (vinyls the
  most: VINYLS.BIN, 17 MB, was read whole per change). In the game (windowed,
  private Xvfb, the maintainer's `preview_xtest.sh` body + graphics shops, 42 changes; new log
  line `preview shown after N ms` with `NFSU2_PREVIEW_LOG`): **median 17.3 ms (body
  shop), 24.8 ms (graphics shop), max 90 ms** — 2–11 frames at 120 Hz.
* **Fix** (exact: the same deterministic build, only earlier):
  * `GarageRequest::prefetch`: the rides a step up or down the current list would
    ask for (`Game::shop_preview_neighbours`: a parts browser's list, UI_Paint's
    swatches; made as the request is, `preview_ride_with` + the performance parts'
    visuals), built ahead on threads (2 at a time) while no change is being built;
  * the built cars and the car shown before are kept (3, newest first) and swapped in
    the frame a step asks for them (`ready`); a build under way for the asked ride
    becomes the change's job; everything is freed when no garage is up;
  * VINYLS.BIN is mapped instead of read whole (`car::vinyl_layers`).
  * `NFSU2_GARAGE_NO_AHEAD=1` turns the prefetch and the kept cars off (A/B aid).
* **Result** (same harness, same binary, with / without NFSU2_GARAGE_NO_AHEAD): body
  shop median **0.0 ms** (the part shows in the frame of the step; p90 0, max 55 ms),
  graphics shop median **0.0 ms** (p90 33, max 79 ms). What is still over 8 ms: a
  jump that is not a neighbour (the paint's page key, the first entry of a list
  before its neighbours are built, vinyls the first time), which still pays the
  21–32 ms build. Memory: up to 5 extra FE cars (2 under way, 3 kept), each its own
  textures and buffers, only while a shop is up.
* Checks: `--shot finished_shop_checks` asserts that every list step asks for a ride
  that was built ahead (25 steps in the body shop, carbon fibre, specialties and neon
  lists); `$OB_PRIVATE/scripts/preview_xtest.sh run` (windowed on its own Xvfb) 0 failures with
  and without the prefetch.

### Gate

* `scripts/checks/gate.sh … <base> extreme`: CHECKS PASS (fast; 1122 tests), PARITY all
  gates passed (Original images, 144 Hz window path, enhanced incl. Extreme: 0 pixels;
  28 shots 0 differ; audio identical; sim hashes identical); binaries differ
  (md5 base binary, the frame-budget fixes 1c74fcd6…).
* `perfgate.sh run` against `current` (this work's seed, 99e726f): FAIL rain `max` 4K GPU
  +3.9 % (27.62 → 28.68 ms, p 0.008; load average 85–110). Against the base (
  the same set, a temporary baseline dir with gate.sh's base binary): **pass**, rain
  `max` 4K 28.82 → 28.77 ms (−0.2 %). The +3.9 % is in main between the budget base and
  the fixes base (the shadow-softness pass / the volumetric smoke / a texture-material pass since removed / the depth and lamps pass), not the frame-budget fixes; `current` should be re-accepted on the
  merged main. Reports: internal evidence.

## The new effects' costs (2026-11-05)

Base main. Measured with `perf.sh bench` (3 runs × 300 frames per arm, arms
adjacent, reference desktop GPU, load average 5–60 from other jobs), GPU busy median of the run medians;
internal notes (compare.md, summary.txt, image-metrics.txt). Presets: `urt` = the player's play
setup (perf.sh `USER`) + Modern Lighting Modern + RT at strength 75 %, `um` the same with Modern,
`max` = perf.sh `MAX`. Nothing native is involved (Modern + RT has no counterpart in SPEED2.EXE);
Original, Ultra, Extreme, the player's setup and Modern Lighting Low / Modern are byte-identical (gate).

### 1. Modern + RT shadows: half-resolution rays outside the colour shader (non-exact, approved class)

Measured first: the RT tier cost +7.4 ms over Modern at racing 4K (user's setup); with the ray count
forced to 0 still +1.5 ms (the ray-query code's registers in every world pixel's shader), 2 rays to
the strongest light alone +7.2 (the other three lights' rays +0.26). Steps (racing 4K urt, GPU ms):

| step | RT − Modern | main view + rt pass | image vs before |
|---|---|---|---|
| before (rays in the colour shader) | +7.42 | 10.15 | — |
| 1a compute pass at ½ res after the pre-pass, 2 × 2 box gather, ranking + second loop in the shader | +3.1 | 5.98 + 1.00 | SSIM 0.9993; 2-px steps on shadow edges at 200 %; start line's shadow 10 px off |
| 1b decals (blended overlays): the blocks under the pixel (`ml_vis_find`) | | | start line back in place |
| 1c characters keep in-shader rays (own pipelines, `D9_ML_CHAR`) | | | flag girl as before |
| 1d traced visibility inside the light loop (no ranking / second loop) | +2.1 | 5.13 + 0.94 | = 1c |
| 1e balanced 3 × 3 tent gather (no 2-px steps), unrolled | **+2.68** | 5.50 + 0.92 | SSIM 0.9993–0.9998, edges as smooth as before at 200 % |

Final (s0 = base, s1 = the effects-performance pass; GPU ms, 3 runs each, ranges disjoint):

| config | before | after | Δ |
|---|---|---|---|
| racing urt 4K (Modern 11.55) | 19.02 | **14.23** (RT +2.68, target ≤ +3) | −4.79 |
| racing urt 1080p (Modern 3.53) | 5.60 | 4.26 (RT +0.73) | −1.34 |
| race start urt 4K | 21.72 | 17.93 | −3.79 |
| rain urt 4K | 22.49 | 17.60 | −4.89 |
| racing max 4K / 1080p | 22.26 / 6.88 | 18.11 / 5.46 | −4.15 / −1.42 |
| race start max 4K / 1080p | 27.42 / 7.29 | 23.26 / 6.01 | −4.16 / −1.28 |
| rain max 4K | 27.76 | 22.80 | −4.96 |
| racing um 4K / 1080p (control) | 11.60 / 3.56 | 11.55 / 3.53 | same (images AE 0) |

Image check (4K, urt): casino canopy still SSIM 0.9993 (0.6 % of pixels > 2 levels: shadow edges),
rain drive 0.9994–0.9998, race start 0.99998, racing 0.9993–0.9998; luminance mean / p5 / p95 unchanged
(casino 93.43 / 9.1 / 220.9 both); motion (rain drive 40 frames, casino crawl 5 cm / frame): temporal
SD, frame deltas and second differences within ±0.2 % of before (no new shimmer). Crops 100 / 200 %:
the maintainer's private captures (left before, right after). FLIP: not run (no FLIP tool on this
machine; SSIM + per-pixel deltas + crops instead). Design: `docs/shadow-softness.md` "2026-11-05: the rays at half resolution, outside the colour shader".

### 2. Volume_fx light smoke: already inside its budget on <base> (no change)

racing (light drift smoke) 4K, Extreme + Cinematic vs Particles Off: +0.25 ms (own passes: build
0.07, before-draw 0.01, march 0.15, composite 0.03 = 0.27); under `max`: +0.22 (17.38 → 17.60).
Heavy burnout (explore 4K): +0.25 (Extreme) / +0.39 (max), passes 0.64. The light-smoke budget
(≤ +0.3 ms) holds; the smoke integration's +0.55 was measured before the depth and lamps pass / the SSR / blur / submit pass and the next-gen atmosphere's work. Left as is (the next-gen atmosphere
works in volume_fx's passes now).

### 3. Next-Gen water: measured, not changed

Default settings (native 8x MSAA) 4K: City Core +1.66, Beacon Hill +2.28 ms (budget 1.5); under
`max` (SSAA 2x): City Core −0.36, Beacon Hill +0.61 (the tier's passes replace native draws). At
default settings 0.3 + 0.4 ms of it are the wet-stone pass and the half-size scene copy, the first
passes to read the 8x multisampled depth / colour (their timestamps take the targets' decompression).
Tried: the wet stone's noise only where it can wet (exact): no change (0.30 → 0.30). Open: a
quarter-size thickness pass, a scissored copy (the copy feeds the surfaces' screen-space reflections,
which reach the whole view), sharing one resolved depth among the MSAA readers.

### 4. Wet Road Interaction High: measured; trail-map change dropped

rain `max` 4K: High +2.89, On +1.95 ms over Off (main view +0.96: the wet road branch, High's 32
mirrored lights only +0.08 over On's 16; weather fx +0.89: spray, High +0.37 over On; ssr mask +0.11).
The trail map's timestamp (0.4 ms) is the gap before its own submission: decaying only the live
tiles (exact, 50 s drive AE 0) moved the GPU total by nothing measurable (24.55 → 24.39–24.73), so it
was dropped. Open: the spray's mist at half resolution with an upsample (the next-gen effects wave / the Next-Gen rain work work in
weather_fx), the wet branch's cost per pixel.

### Gate
`scripts/checks/gate.sh … <base> extreme`: CHECKS PASS (fast: build, 1146 tests, selftest, 29 shots,
flows, playthrough); PARITY all gates passed (Original images 1280×960 / 4K AE 0, 144 Hz window path
AE 0, enhanced Extreme / Extreme FSR AE 0, 29 shots AE 0, audio and five state hashes identical);
binaries differ (md5 base binary, the effects-performance pass 6884d236…). `perfgate.sh run` against the <base> binary:
**pass**; rain `max` 4K 27.74 → 22.89 ms (−17.5 %, p 0.008), race start `max` 1080p 7.42 → 6.12
(−17.6 %), every other config ±0.5 % (load average up to 294). internal notes.

## The Next-Gen tiers — costs, tiers, GPU classes (2026-11-06)

Base main. Nothing native is involved: every row here is a clone deviation; Original is
untouched (gate below). Measured with `perf.sh bench` through internal evidence
(reference desktop GPU: 3 runs × 300 frames, `ORDER=alternate`; reference integrated GPU, `IGPU=1`: 2 runs × 300 frames at 1080p),
GPU busy median of the run medians, `NFSU2_BENCH_RIVAL_NOS=8` (the rivals' flames for Sparks & Fire). The
row's step is measured over **max** (perf.sh `MAX`, the player's play setup + every newest row) on the reference desktop GPU
and over **Modern** (the preset with its weather rows) on the iGPU. Load average 30–170 (other jobs).
Raw tables: internal evidence, `nx-igpu-modern.txt`, `carlights-sky-steps.txt`.

### Per effect (GPU ms; reference desktop GPU over max, its own pass in brackets; iGPU over Modern 1080p)

| row → tier (heavy scene) | budget 4K | 1080p before | 4K before | 4K after the effects-tiers pass | iGPU before → after |
|---|---|---|---|---|---|
| Rain Detail High → **Next-Gen** (rain) | ≤ 1.5 | +0.09 | +0.45 [weather fx +0.34] | same (weather_fx left to the wet-driving pass) | +2.1 (Off → NG +4.1) |
| Sparks & Fire **Next-Gen** (racing, rivals' flames) | ≤ 0.5 / 1 | +0.02 | +0.07 [0.10] | same (the sparks tuning's code on main) | +2.6 [1.5] |
| Headlight Beams On → **Next-Gen** (racing / rain) | ≤ 1 | +0.34 / +0.22 | +0.66 / +0.72 [0.62 / 0.65] | **+0.60–0.77 [0.52 / 0.53]** | +15.1 / +16.7 [14.0 / 14.8] → [**10.3 / 10.2**] |
| Volumetric Fog Low → **Next-Gen** (explore / rain) | ≤ 1.5 (fog + sky) | +0.06 / +0.10 | +0.12 / +0.26 | same | Modern + NG +4.2 / +5.3 |
| Sky **Next-Gen** (explore) | (with the fog) | +0.39 | +0.85 [panorama 0.08] | +0.78 [**0.025**] | +5.2 [2.50] → +3.5 [**0.68**] |
| Fog + Sky Next-Gen (explore) | ≤ 1.5 | +0.45 | **+0.94** | ≈ +0.9 (sum of the two) | — |
| Ambient Detail **On** (racing) | ≤ 0.5 | +0.00 | +0.03 | same | +0.0 |
| Ambient Detail **Next-Gen** (racing) | ≤ 0.5 | +0.22 | **+0.37** [volume fx march 0.24] | same (volume_fx left to the next-gen smoke polish) | +7.8 [march 6.7] |
| every Next-Gen row at once (racing / rain) | — | +1.12 / +0.98 | +2.31 / +2.55 | − the car lights' 0.1 | — |

Every row was already inside its budget at 4K on the reference desktop GPU; the work went where the cost was out of line:
the integrated GPU.

* **Sky panorama: no idle lanes (exact).** `cs_panorama` updates a quarter of the 1024 × 256 panorama's rows a
  frame, but every row was dispatched and three quarters of the invocations returned at once — every 8 × 8
  workgroup ran with a quarter of its lanes busy. Now only the frame's rows are dispatched (`sky_ng.rs update`,
  `cs_panorama` maps the invocation onto row y·4 + phase; the first frame still fills all rows). The same
  texels: Sky Next-Gen shots AE **0** against the base (qrstart, rain street, casino neon, drift). Pass
  2.50 → 0.68 ms on the iGPU, 0.080 → 0.025 ms at 4K.
* **Car lights: the halos' air density once per lamp (near-exact).** The point lights' in-scattering
  multiplied every pixel's halo by `air(lamp)` — two octaves of value noise (16 hashes) evaluated per pixel and
  per lamp (≤ 64), the same value for every pixel. Breakdown on the iGPU (racing, pass 13.9 ms): without the
  points 7.1, without the cones 9.6, without the points' noise 10.1, without the cones' noise 11.5. Now
  `car_lights::air_at` makes it on the CPU (op for op the shader's f32 formula) into the point's `colour.w`;
  the cones keep their per-step noise (it varies along the ray). Pass **14.0 → 10.3 ms** (iGPU racing),
  **0.62 → 0.52 ms** (4K racing max). Images: Next-Gen shots against the base differ in < 1 pixel in a million
  (peak 3–4 / 255: the CPU's rounding of the density), SSIM 1.0000; motion is unchanged by construction (the
  same per-lamp value, per frame).
* Left as measured (other units' passes, plan): Rain Next-Gen's haze in `weather_fx`, Ambient Next-Gen's
  own volume in `volume_fx` (the next-gen smoke polish: its second grid merged with the smoke's is theirs to do), Sparks & Fire (the sparks tuning
  reworked it on main after this base; its rain-scene pass was 0.35 ms at 4K with every Next-Gen row on).

### Tier ladders (every new row)

| row | tiers (4K hint, reference desktop GPU under max; iGPU 1080p) |
|---|---|
| Rain Detail | Off · On 0.11 (iGPU 0.7) · High 0.12 (2) · Next-Gen 0.57 (4) |
| Sparks & Fire | Original (native sprites) · Next-Gen 0.2 (2.5 with flames) |
| Headlight Beams | Off · On −0.04 (iGPU: Off is 2.3 ms slower) · Next-Gen 0.7 (+12) |
| Volumetric Fog | Off · Low 0.25 (2) · High 0.26 (3) · Next-Gen 0.38 (4–5) (Low / High: tiers of the froxel medium, docs/atmosphere.md "The tiers") |
| Sky | Original · Next-Gen 0.78 (3.5) |
| Ambient Detail | Off · On 0.03 (0) · Next-Gen 0.37 (8) |

Each ladder rises in cost and stays in the effect's family (Off / On / Next-Gen where an effect has no useful
middle step; Sparks & Fire and Sky: Original / Next-Gen, the native look being the lower tier). No tier was
added (scope freeze). Hints and help texts: `graphics_table.rs` (`*_COST_4K`, "integrated GPU: …"), checked by
`cost_hints_follow_the_budget_table` (`hints-4k.tsv` gains the effects-tiers pass rows).

### GPU classes: the presets' tiers per machine (`qol::GpuClass`, `Preset::apply_for`)

`render::Gpu::new` sets the class once: **Integrated** (integrated or software adapter), **Mid** (discrete under
11.5 GiB of video memory, **inferred** as the proxy, read from the DRM sysfs node as `upscale_pack`'s memory check), **High** (11.5 GiB or
more, or unknown: the reference desktop GPU). `NFSU2_GPU_CLASS=integrated|mid|high` overrides it (shots, benches). Only
`Preset::apply` reads it — choosing a preset on the Graphics page; nothing changes a value the player set, nothing
switches while playing, dynamic resolution stays manual. A preset reads as itself on that machine
(`Enhancements::preset` re-applies with the same class); the preset's help says "Effect tiers for this … GPU"
when the class changed something.

| preset | High (as measured on the reference desktop GPU) | Mid | Integrated (reference integrated GPU, measured) |
|---|---|---|---|
| Original, Clean | unchanged | unchanged | unchanged |
| Modern | unchanged | unchanged | unchanged (its rows are already the light ladder: Beams On, Rain On, no fog) |
| Ultra | Ultra without Enhanced Glass (below) | = High: **no step** (the obvious one, MSAA 8x for SSAA 2x, measured 0.5–0.9 ms *slower* at 4K here; no mid card to measure — open) | MSAA 8x, half-size glow (Post High), native smoke, Rain Detail On, no Volumetric Fog, no Animated Water, no Wet Road Interaction |

Measured (GPU ms, internal evidence): iGPU 1080p explore / racing / rain — Modern 29.5 / 47.6 /
51.4, Ultra (High class) 79.2 [p95 289] / 97.6 / 152.1, **Ultra (Integrated class) 39.2 / 62.7 / 72.0** (2 runs).
reference desktop GPU, Ultra without Enhanced Glass: racing 4K 9.78 → 9.35, race start 4K 10.67 → 10.28, racing 1080p
2.79 → 2.67 (3 runs).

No preset sets a Next-Gen tier or Ambient Detail on any class (manual; below). Test
`qol::tests::presets_follow_the_gpu_class` (classification; High = `apply`; classes only step down; Original /
Clean untouched; Beams kept; Mid = High; no Next-Gen in any preset).

### The look direction's verdicts applied (docs/look-direction.md "Per-row verdict")

Original + one row at the look direction's six audit spots (street, casino neon, Coal Harbor docks, Jackson Heights, rain
street, Quick Race start; the original's cameras, the maintainer's tools), Δ against clone Original
(internal evidence):

| row | Δmean | Δp5 | Δp95 | ΔRMS | Δglow | Δsky | R1–R6 | verdict |
|---|---|---|---|---|---|---|---|---|
| Ambient Detail On / Next-Gen | −0.03 | 0.00 | −0.11 | −0.04 | 0 | 0 | met | **stays manual**: at the chase camera it changes ≤ 0.04 % of the frame at eleven spots (`ambient_on_street_orig_vs_on_150.webp`): no visible gain for a preset; Next-Gen +7.8 ms on the iGPU |
| Headlight Beams Next-Gen | +1.33 | 0.00 | +1.17 | +0.68 | +0.06 | 0 | met | manual (reworks the car lights) |
| Rain Detail Next-Gen | +1.27 | +0.49 | +0.64 | +0.18 | 0 | +0.59 | **R1 fails at the rain street (+7.6 > ±4)** | manual; its haze needs the E4 budget (weather_fx) |
| Rain Detail On / High | +0.14 / +0.18 | 0 | +0.11 / +0.16 | | 0 | | met | keep (Modern / Ultra) |
| Volumetric Fog Next-Gen, Sky Next-Gen | +1.41 / +0.41 | +1.07 / +0.06 | | | | +1.06 / +0.51 | met | manual |
| Sparks & Fire Next-Gen | 0 (no sparks in a still) | | | | | | The sparks tuning's spark scenes: +0.28 | manual |
| Ultra (Preset::Ultra, base) | +1.87 | +1.03 | −0.23 | −1.02 | −0.04 | +0.89 | met (preset budget) | — |
| **Enhanced Glass** ("keep, review") | | | | | | | | **left Ultra**: a glass-focused check (race start, drift start, street at the original's cameras, Ultra with and without it) changes 0.02–0.16 % of the frame, and on the player's car it **hides the NFSPEED rear-window decal** (`glass_street_ultra-noglass_vs_ultra_200.webp`: the composite draws the pane over the decals drawn before it); −0.65 ms at 4K. Manual; the decal order is open (car_glass) |

Motion Blur / Depth of Field stay out of the presets (they are); Dynamic Lamps, Beams, Volumetric Fog Low / High
keep their the look direction tuning.

### Gate and baseline
* `CHECKS=full scripts/checks/gate.sh … <base> extreme`: CHECKS PASS (full: build, 1219 tests, selftest, 275 shots,
  determinism, first race / drag, post race, playthrough, showcase); PARITY all gates passed (Original images AE 0,
  144 Hz window path AE 0, enhanced incl. Extreme / Extreme FSR AE 0, 29 shots 0 differ, audio identical, five sim
  hashes identical); binaries differ (md5 base binary, the effects-tiers pass 22c80efc…).
* `perfgate.sh run` against `current` (this work's seed, 99e726f): FAIL, rain `max` 4K CPU only (draw p10 +39 %, main
  +40 %, workers +18 %); every GPU column better (rain max 4K 27.71 → 22.96 ms, Ultra 4K 14.00 → 9.06). Against the
  <base> binary (same set, a temporary baseline dir): **pass**, every config within ±0.1 % GPU, rain max 4K CPU
  +4–7 % n.s. — the CPU change is main's between the two bases, not the effects-tiers pass (changes nothing max draws).
  Reports: internal evidence.
* **New baseline** accepted from the effects-tiers pass gated binary: `$OB_PRIVATE/perf-baselines/20261106-0446-b7ee3bcf464b`
  (`current`; sha256 b7ee3bcf…, the <base> A/B run's arm).

### Open
* Rain Next-Gen's haze over R1 at the rain street (weather_fx, after the wet-driving pass). Ambient
  Next-Gen's own volume (+7.8 ms on an iGPU; merging its grid with the smoke's is volume_fx, after the next-gen smoke polish). The Mid
  class has no measured step (no mid card here). Car lights Next-Gen is still ~10 ms on the reference integrated GPU (the cones'
  per-step noise ~2.4 ms, the 64-lamp loop ~2.9 ms): a cheaper march would be non-exact for every GPU.

## Final performance / look pass on the effects (2026-11-06)

Base main <base> (FSR 2). Scope freeze: no new effect or row. Nothing native is
involved; Original is byte-identical (gate below). Benches internal evidence (perf.sh bench,
`ORDER=alternate`, 300 frames; reference desktop GPU 3 runs, reference integrated GPU `IGPU=1` 2 runs), GPU busy median of the run
medians; raw tables internal evidence; look metrics `look-audit-{base,dx3,ultra-glass}.txt`
(the look direction's six audit spots + the rain harbour, the original's cameras, the maintainer's tools, Δ vs clone Original).

| item | before | after | look (Δ vs Original, 7 spots) |
|---|---|---|---|
| 1 Rain Detail Next-Gen haze (`weather_fx::HAZE_LOOK`) | rain max 4K +0.49 ms [weather fx 0.437] | +0.45 [0.420] | **rain street +7.6 → +0.45, harbour +13.4 → +1.54**; row mean +2.99 → +0.28, p5 +0.89 → +0.05, p95 +1.95 → +0.31, sky +2.31 → −0.03: R1–R6 met |
| 2 Enhanced Glass (decals after the pane, rect copy, `MIP_BIAS` fix) | Ultra 4K racing +0.53 / race start +0.34 ms [pass 0.59 / 1.06] | **+0.11 / +0.35** [0.28 / 0.97] | Original + row −0.03 / p5 +0.14 / p95 −0.32; Ultra with vs without −0.05 / +0.13 / −0.28: met; the NFSPEED decal is back (`glass_street_noglass_before_after_200.webp`) |
| 3 Car lights Next-Gen (lattice noise texture, point tiles) | 4K racing max +0.64 [0.52]; iGPU pass 10.0–10.3 | **+0.43 [0.45]**; iGPU **9.2–9.4** | SSIM ≥ 0.9994, mean ±0.2 (drift start: the red haze's noise field moved); motion stats within 0.05 % |
| 3 Ambient Detail Next-Gen, iGPU (racing) | +7.3 ms (volume fx march 7.6) | not changed | — |
| 3 Wet Road Interaction High, iGPU (rain) | +23.2 ms over Off (volume fx march 19.4: the wet spray's volume, main view +4.7) | not changed | — |
| 3 Next-Gen water, iGPU (fountain orbit) | +30.5 ms (water 15.9, shade 3.9, volume fx 3.9) | not changed | — |
| 4 Next-Gen water at the defaults (8x MSAA), 4K orbit | +2.41 ms (budget 1.5) | not changed (see below) | — |

* **1** the Next-Gen rain work's mist put the native FogColor back over the scene at 0.9 of its extinction (E1's double fog: the
  darks went grey) and the lamps' in-scatter over their whole reach whitened the native pink glow sprites. Now:
  veil 0.2, the mist's own light 0.1 × FogColor, not over the sky (0.3 of the in-scatter there), the lamps scatter
  over half their reach (same coefficient). Grids `rain_{street,harbour}_orig_ng-before_ng-after.webp`. Cheaper by
  0.02 ms (fewer lamps reach a ray). Test `rain_haze_stays_in_the_look_budget`.
* **2** docs/lighting2.md "Final performance / look pass on the effects". Exact apart from the decal order (Ultra shots with / without the rect AE 0). On the
  reference integrated GPU (Modern + glass, MSAA 8x, race start) it costs +3.9 ms (pass 7.8: the 8x resolve), so the
  Integrated class keeps it off. Pre-existing crash fixed: Enhanced Glass with TAA / TAAU / FSR 2 panicked
  (pipeline constant `MIP_BIAS` not in the glass module) — the max + FSR 2 bench below hit it.
* **3** docs/car-lights.md "Final performance / look pass on the effects". Only the lattice is non-exact; the tiles are exact (AE 0 with and without them).
  Tried and dropped: the tiles' set bits as the loop (12.1 ms, the lamps' loads became vector loads). Ambient
  Next-Gen, Wet Road High and Next-Gen water on the iGPU are all volume_fx / water passes whose cost is the content
  in view (a march over occupied bricks, the fountain's own passes); every cheaper form tried or considered
  (coarser march, fewer steps, quarter-size thickness) softens the image, so they stay as measured (not in any
  iGPU preset, help texts give the iGPU cost).
* **4** Leave-one-out of the Next-Gen water passes at 4K, defaults (`benches.txt`, a measuring build): wet stone
  0.34, half-size copy 0.37, surfaces + basins 0.85, jets' depth + thickness 0.46 (thickness 0.31), filter / shade /
  composite 0.6. The first pass to read the 8x targets pays their decompression (without the wet pass the copy
  rises 0.37 → 0.50; without the copy the jets' passes rise). Tried: the jets' thickness at quarter size (−0.2 ms):
  the aerated white streaks visibly softer at 200 % — dropped. **Open**: still ~0.9 ms over 1.5 ms at 8x MSAA
  (under the player's SSAA 2x setup it is within budget). The water polish (docs/water.md "Water polish"): +2.24 → +2.10 ms by
  exact changes; the "decompression" reading does not hold (the 8x targets cost per fetch / per touched sample:
  bandwidth; a resolve costs 0.20 ms, a hardware depth test in the composite saved nothing); still ~0.6 ms over.
* Preset changes: **Enhanced Glass back in Ultra** (High / Mid class); Integrated class Ultra sets it off. Hints:
  Enhanced Glass 0.65 → 0.35, Headlight Beams Next-Gen 0.7 → 0.43, Rain Detail Next-Gen 0.57 → 0.45
  (`hints-4k.tsv`); iGPU help texts (glass ~4 ms with MSAA, car lights ~+8 ms).

### 5. The max setup at 4K: SSAA 2x vs FSR 2 Quality (goal ≥ 60 fps)

`max` = perf.sh MAX (the player's play setup — SSAA 2x — plus every newest row at its top), `ng` = max + every
Next-Gen row (rain, beams, fog, sky, ambient, sparks, water), `…q` = the same with `enh.upscale 8` (FSR 2 Quality,
which turns SSAA / MSAA off). reference desktop GPU, 3 runs × 300 frames. GPU busy median [frame time p50 / p99]:

| scene | max (SSAA 2x) | max FSR 2 Q | ng (SSAA 2x) | ng FSR 2 Q |
|---|---|---|---|---|
| explore | 14.14 [14.6 / 23.3] | **7.60** [7.9 / 9.1] | 15.61 [16.2 / 19.7] | **8.25** [8.6 / 10.6] |
| racing | 18.29 [18.9 / 21.7] | **8.94** [9.5 / 16.4] | 20.36 [20.9 / 24.2] | **10.13** [10.8 / 17.5] |
| race start | 23.49 [24.0 / 27.2] | **9.12** [9.7 / 25.1] | 25.28 [25.9 / 36.5] | **10.29** [10.8 / 24.9] |
| rain | 22.78 [23.5 / 49.9] | **10.54** [11.5 / 34.3] | 25.24 [27.0 / 42.0] | **11.94** [12.4 / 31.0] |

**Verdict: with FSR 2 Quality the max setup runs at 4K above 60 fps** — 87–127 fps median (GPU 7.6–10.5 ms; every
Next-Gen row on: 81–116 fps, GPU 8.3–11.9 ms), main thread 1.7–4.1 ms. With SSAA 2x it does not (37–69 fps:
14.6–27 ms frame p50). Frame p99 with FSR 2 stays under 16.7 ms in explore and near it racing, but reaches 25–34 ms at the
race start and in rain (spikes, not the GPU median: the bench machine's load average was 13–60 from other jobs;
**open**: confirm the p99 on an idle machine). The image cost of FSR 2 Quality vs SSAA 2x is the FSR 2 port's (docs/fsr2.md:
SSIM 0.974 vs 0.988 against SSAA 4x).

### Gate
* `CHECKS=full scripts/checks/gate.sh … <base> extreme`: CHECKS PASS (full: build, 1239 tests, selftest, 275 shots,
  determinism, first race / drag, post race, playthrough, showcase). Parity: Original images 1280×960 / 4K AE 0, 144 Hz
  window path AE 0, 29 shots AE 0, audio identical, five sim hashes identical; enhanced explore Extreme / Extreme FSR AE 0,
  **racing Extreme / Extreme FSR differ (expected)**: perf.sh's EXTREME has Enhanced Glass, whose decals now stay over
  the pane — the same racing frames with `enh.enhanced_glass 0` are AE 0 base vs the final effects pass. Binaries differ (md5 base
  4ffe6ae1…, the final effects pass 7f1bf27c…).
* `perfgate.sh run` against `current` (20261106-0446): **pass**; GPU ±0.5 % on every config (rain max 4K
  22.89 → 22.70, −0.8 %), VRAM rain max 4K +2.7 % (reported only, +88 MB, not explained: the car-lights tile buffer and noise lattice are
  ~70 KB; rain max has Enhanced Glass on, and with it the same copy target as before); load average 20–65.

## CPU frame time and hitches

Where the CPU time of a frame goes on the main thread and the four render workers, what the rise against the
older accepted baseline is made of, and the tail of long drives: every frame over twice the median with its cause.
Tables, run lists and the worst frames: `docs/evidence/perf-budget/cpu-frame/`.

**Conditions.** The machine was shared throughout: two other performance units measured in turn under the
machine-wide measuring lock (every series here ran inside it), gates and builds of other units ran beside them
(load average 3–17), and another program kept the discrete GPU's queue busy the whole time: a racing frame at
Original 1080p with 0.83 ms of GPU work took 12.9 ms (frame p50). **Wall times on the discrete GPU are therefore
not trusted here**; thread CPU is the quantity, and the wall times come from the integrated GPU. The bench's
update / draw CPU columns are read from the scheduler's per-thread statistics, which a running thread updates
only at scheduler events: over a run they are right, a single frame's split between update and draw is
approximate. The frame log's own main-thread column (below) reads the thread's CPU clock and is exact.

**Tools.** `perf.sh profile` (samply, the measured profile's code with line tables; 3000 frames, the load left
out), the bench's JSON (per-thread CPU), and a new **frame log**: `NFSU2_BENCH_FRAMELOG=file.tsv` writes one line
per timed frame (update / draw / throttle wall, the main thread's CPU clock, the render workers' and the other
threads' CPU, pipelines and modules built, the camera's section, traffic and encounter cars, the frame's
`prof::acc` parts over 0.25 ms), and `scripts/perf/frametail.py` turns it into p50 / p95 / p99 / max and the slow
frames with their causes. The tick now has `prof::acc` parts of its own (`tick: update`, `tick:   sim step`,
`tick:   free roam`, `tick: enhanced fx`, `tick: preload`, `tick: audio`, `tick: particles`, `tick: snapshot
before`, `tick: snapshot capture`): one cached flag load each when no bench runs.

### Where a frame's CPU goes (the four gate scenes)

ms per frame of thread CPU in the profiled run (the bench's one tick per frame), and the largest parts
(inclusive unless said, ms per frame):

| scene | main thread | render workers (4) |
|---|---|---|
| racing Original 1080p | **0.87**: the draw 0.42 (world scene 0.26, HUD layout 0.07, post 0.05, UI 0.04), the race tick 0.18 (car physics, traffic and racer thinking), wgpu's clean-up of finished command buffers 0.16 (`Device::maintain` → the driver freeing them, in the frame's poll) | **1.32**: wgpu encoding the render passes 0.82, the world's view recording 0.17, the queue submits 0.11 |
| explore Extreme 1080p | **1.33**: copies (`memcpy`) 0.13, LOD Fade 0.04, motion blur's wrap 0.04, ground queries 0.03, Keep All Loaded's look-ahead 0.03, the section lookup 0.03 (self time; nothing over 0.15) | **1.77**: encoding as above |
| rain max 4K | **2.75**: the world's plan 0.61 (LOD Fade 0.33, the lamp clusters 0.22), the command-buffer clean-up 0.38, buffer writes 0.30 (a staging buffer made per `write_buffer`), the spray step's own share 0.22 | **4.17**: encoding 2.4, the spray step's chunks 0.59 and its volume 0.21 |
| race start max 1080p | **2.19**: the world's plan 0.56 (lamps 0.27, LOD Fade 0.23), the native car shadows 0.21 (their ground queries 0.15), the clean-up 0.30, buffer writes 0.20, bind groups made per frame 0.08 (screen-space reflections, atmosphere, volume fx, motion blur) | **3.78**: encoding 2.6 |

At a 144 Hz schedule (racing Original 1080p, 7200 frames) the main thread is 0.91 ms per drawn frame: 0.81 draw
(each frame) and 0.10 tick (0.23 per tick), the clean-up 0.12, the HUD's layout 0.08, the frame's interpolated
poses 0.05.

### What the rise against the older baseline is made of

The same profile of the older accepted baseline's main commit (built and run alike, interleaved with the current
tree) for the two scenes with the largest rise. Functions compared by name (the module moves since then renamed
paths), ms per frame:

| scene | thread | older → now | what it is |
|---|---|---|---|
| rain max 4K | workers | 3.20 → 4.17 (+30 %) | the spray step and its volume's wake moved for the most part from the main thread (about −0.4 ms there) onto the workers in chunks (about +0.6 ms): some 0.2 ms more CPU in the parallel form; encoding +0.24 |
| rain max 4K | main | 2.94 → 2.75 (−6.5 %) | the spray step gone (above) |
| race start max 1080p | workers | 3.20 → 3.78 (+18 %) | encoding +0.52 ms (`encode_render_pass`, binding, vertex buffers): more draws, not dearer ones — the bench's car now drives 1057 m through 8 sections with 9 traffic cars on the road at the end, before 730 m through 4 sections with 7 (the physics and collision work since changed the drive) |
| race start max 1080p | main | 1.87 → 2.19 (+17 %) | the lamps' plan +0.07, the clusters' build +0.04, the look-ahead +0.03, copies +0.06, ground queries +0.02: the same scene change (more sections and cars in view) |

So the CPU rise is made of a changed scene (the race start bench drives further, through twice the sections)
and of the spray's parallel form; no function got dearer per item. **Avoidable without changing a result**,
looked for and measured:

* work per frame that is only needed per tick: the two snapshot copies of the tick (the state before it and
  after it, particles and rain included) cost 0.003 + 0.002 ms per drawn frame at 144 Hz — not worth a change;
  the HUD's layout (0.08 ms per frame) animates with the frame's clock, so it is needed per frame;
* allocations per frame: the interpolated poses clone each drawn car's whole state (its collision caches
  included) per frame, 0.014 ms; Vec growth across the draw 0.035–0.057 ms; wgpu's staging buffer per
  `write_buffer` 0.07–0.1 ms in the max configurations — each below this machine's CPU noise;
* a per-frame sort or rebuild: Keep All Loaded's look-ahead searched every visible list for each of the map's
  outlines on every call, twice a frame (fixed below); bind groups made each frame by enhanced passes (0.08 ms,
  the enhanced passes' code);
* locks: none measurable.

### The tail of long drives

A career sprint with traffic and three rivals from the start to its finish (4.2 km, 33 sections, 160 s of
ticks) and a career circuit with three rivals and traffic to its finish (3.3 km, 103 s), driven by the racer AI,
plus the circuit's race start (1500 frames), at Original and at the Enhanced level High, on the bench's fixed
step (one tick per frame, as fast as the machine goes), a 60 Hz and a 144 Hz display schedule; 1920 × 1080.
Whether the sprint passes through a tunnel was not checked: **open**. Main thread CPU per frame (ms; the
bench's update + draw thread CPU), its worst frame with and without the race's last frame (the race end), and
the render workers' CPU:

| run | frames | main p50 | p95 | p99 | max (without the end) | workers p50 / p99 / max |
|---|---|---|---|---|---|---|
| sprint, Original, fixed step | 9572 | 0.94 | 1.45 | 1.84 | 12.5 (2.9) | 0.99 / 2.48 / 3.4 |
| sprint, Original, 60 Hz | 9475 | 0.97 | 1.63 | 2.05 | 11.5 (5.0) | 1.06 / 2.53 / 5.0 |
| sprint, Original, 144 Hz | 22740 | 0.98 | 1.77 | 2.19 | 13.9 (4.9) | 1.16 / 3.43 / 8.4 |
| sprint, High, fixed step | 9572 | 2.11 | 3.18 | 3.60 | 19.2 (14.7) | 2.64 / 5.91 / 11.2 |
| sprint, High, 60 Hz | 9475 | 1.73 | 2.85 | 3.52 | 15.9 (14.8) | 2.15 / 5.00 / 10.3 |
| sprint, High, 144 Hz | 22740 | 1.70 | 3.92 | 4.71 | 20.4 (20.4) | 2.56 / 7.78 / 16.5 |
| circuit, Original, fixed step | 6176 | 0.93 | 1.28 | 1.71 | 10.4 (3.4) | 1.00 / 2.59 / 6.2 |
| circuit, Original, 60 Hz | 6344 | 1.01 | 1.75 | 2.20 | 15.0 (4.2) | 1.12 / 3.33 / 5.4 |
| circuit, Original, 144 Hz | 15226 | 1.02 | 1.97 | 2.40 | 13.3 (3.9) | 1.28 / 3.69 / 7.7 |
| circuit, High, fixed step | 6176 | 2.43 | 3.55 | 4.03 | 14.1 (11.5) | 3.02 / 6.96 / 16.6 |
| circuit, High, 60 Hz | 6344 | 2.18 | 3.33 | 3.86 | 12.9 (12.2) | 2.87 / 6.11 / 10.3 |
| circuit, High, 144 Hz | 15226 | 1.84 | 4.16 | 5.23 | 15.7 (10.9) | 3.04 / 9.83 / 20.0 |
| race start, Original, fixed step | 1500 | 0.89 | 1.21 | 1.35 | 3.1 | 1.16 / 2.38 / 5.0 |
| race start, High, fixed step | 1500 | 1.74 | 2.50 | 3.02 | 7.3 | 2.48 / 7.09 / 13.5 |

The frame's wall time from the **integrated GPU** (Original at World Detail 5, the only level it runs near 60 fps
there; at World Detail 3, a fresh install's, Original's GPU time is 4–6 ms and Low's 5–8; ms; over the
budget = frames longer than the schedule's frame):

| run | frames | wall p50 | p95 | p99 | max (without the end) | over the budget | main CPU p50 / p99 |
|---|---|---|---|---|---|---|---|
| sprint, 60 Hz | 9475 | 13.3 | 17.3 | 18.8 | 29.9 (25.4) | 845 (8.9 %) | 1.40 / 2.98 |
| circuit, 60 Hz | 6344 | 14.7 | 16.3 | 17.1 | 21.2 (21.2) | 120 (1.9 %) | 1.27 / 2.34 |
| sprint, 144 Hz | 22740 | 14.0 | 29.4 | 33.2 | 37.0 | all | 1.35 / 3.63 |
| circuit, 144 Hz | 15226 | 15.3 | 30.9 | 32.8 | 41.2 | all | 1.26 / 3.45 |

The integrated GPU is GPU-bound at Original 1080p (its GPU busy 10–15 ms a frame, "IGPU class" above): at 60 Hz
the frames over 16.7 ms are the GPU's (814 of 845 and 116 of 120 spent under 3 ms in the update and the draw and
the rest waiting for the GPU; which views make those frames heavier was not traced), not the CPU (main thread p99
under 3 ms); a 144 Hz schedule cannot be met at all there. Its worst
frames without a GPU cause: the race end (as below), the lens drops' first frame (7.3 ms draw; its pass is now made ahead, "First-use
hitches built ahead"), and draws of
6–9 ms whose parts are all small (**inferred**: the draw waiting for the GPU).

The causes of the worst frames (frame log parts, pipeline log `NFSU2_PIPELINE_LOG=1`, samply's samples in the
frame's window):

* **The race's end** (every run's last frame, 8–13 ms of main thread CPU, the tick's update 7–13 ms): the
  finish's results, the profile saved (`Profile::store`, its native form, about 3 ms) and the ending scene
  loaded (`Scene::load` and its characters' bank parsed and decoded, about 4 ms), its characters and car
  renderers drawn first. The original shows its loading screen here (`play_race_ending` starts a load); it is a
  load, not a hitch while driving. The ending scene's files are now read ahead ("The race end" below); the profile's store
  is left where it is.
* **Keep All Loaded's first conflict** (High: the race start's frame 742, 7.7–11.7 ms; the circuit's frame 223,
  7.1–8.2 ms): the look-ahead's first
  set that needs the two relations built the whole map's tables at once in that frame — every list's section
  pairs and every placement's ground cells. **Fixed** (below).
* **The spray's first cloud** (High: the sprint near its end, frame 8884 on the fixed step, 8751 at 60 Hz,
  21003 at 144 Hz; 12–16 ms of thread CPU in `weather fx prepare`): Wet Road Interaction's spray volume
  (`SprayVolume::new`, traced with spans: 12.6 of the frame's 14.6 ms, of it 11.1 ms its accumulators' zeros
  written and 1.2 ms its noise volume uploaded), and the lens drops' pass (`LensPass::new`, three pipelines, 0.3 ms)
  made in the same frame. **Fixed** ("First-use hitches built ahead" below).
* **A car renderer built in the frame a car first shows** (`car renderer builds`, 2.0–3.6 ms; the race start's
  frame 359, the sprint's frame 3050): once or twice a race. The race start's is the race's traffic, paused during
  the countdown and drawn from its end. **Fixed** ("Car renderers built ahead" below).
* **On the discrete GPU** every other slow frame (up to 40 ms wall, at High often 10–20 ms of draw wall) is
  the main thread waiting inside the draw (for the previous frame's end, `WindowState::settle`) with 1–3 ms of CPU:
  the other program's turn on the GPU; samply's long runs of main-thread samples there are ordinary draw work spread
  over neighbouring frames, no single function.
* Ordinary ticks: the race tick reaches 1–2 ms on some frames (`tick:   sim step` 0.6–1.3 ms); the HUD's layout
  0.3–0.8 ms on some frames. Looked at again (the circuit to its finish, fixed step, one run each, the discrete GPU,
  with temporary spans in the race's step): at Original no tick part reached 0.5 ms (`sim step` at most 0.46 ms,
  the frame's p99 1.18 ms); at High `sim step` reached 1.07 ms, of it the race's world step (the physics sub-steps,
  `step_world` up to 0.33 ms each) up to 0.58 ms and no other part over 0.3 ms, the HUD's layout at most 0.47 ms.
  The simulation is the same at every level (the state hashes), so the High ticks are the same work slower beside
  the render workers' (**inferred**: the cores' and caches' sharing); no single dear step, nothing changed.
  **Looked at a third time**: the step reads one value of the settings (`TickInput.settings`: the field of view's
  offset, the same at both levels; `Session::step` and the race's step read no Graphics row), so no part of it
  does level-dependent work. Its thread CPU per tick (frame log with every part, `NFSU2_BENCH_FRAMELOG_MIN_US=0`;
  `--bench racing 1500`, fixed step, frames 11 on, 3 runs each, mean ms): discrete GPU Original 0.111, High
  0.129, High with the process on one hardware thread per core 0.133; integrated GPU 0.144 / 0.163 / 0.162. The
  same Original drive costs 0.112 alone and **0.186** with a second game (a High racing bench) running on the
  other cores (discrete GPU, the process on cores 0–3: High alone 0.131, beside it 0.176), and 0.111 against
  0.144 by the GPU its frames go to alone. So the step's CPU follows what runs around it — the shared cache,
  the memory, the cores' clocks, the waits for the GPU between ticks — by more than High's own render workers
  add (+16 %): a measuring artefact, not work; nothing to fix in the step (evidence:
  `docs/evidence/perf-budget/car-change-ahead.txt` §4).

### Fixed (exact)

Keep All Loaded's look-ahead (clone deviation, `World::lookahead_sections`, src/world/query.rs):

1. **Its lenders' box test first** (`World::lookahead_near`): the outlines within the ring are found with the
   outline's box test before the search for the outline's visible list, instead of searching the lists for
   every drivable outline of the map on each call (twice a frame). Same outlines, same order, same distances
   (test `look_ahead_lenders_are_the_plain_filters`, every outline of Bayview at four eyes, both rings).
2. **Its two relations per section, made when asked** (`listed_together`: the lists that name the section, then
   a search of those; `sections_overlap`: each section's ground cells made the first time it is asked for, from
   its own placements in their order): before, the first conflict built both for the whole map in one frame.
   The same answers for every pair of Bayview's listed and placed sections (test
   `look_ahead_relations_per_section_are_the_whole_map_ones`, against the whole-map tables).

Both are pure functions of the world, so the drawn sets, the pictures and the simulation are unchanged (gate
below). Measured on one binary per arm, both with the frame log, arms interleaved, 7 runs each:

| configuration (fixed step, 1080p) | metric | before | after |
|---|---|---|---|
| race start, High (1500 frames) | the first-conflict frame (742), main thread CPU | 8.4 [7.7..11.7] ms | **3.1 [2.7..4.7] ms** |
| race start, High | the run's worst frame | 8.4 [7.7..11.7] (always frame 742) | 6.9 [6.0..7.4] (other frames: a car renderer's build, ordinary frames) |
| race start, High | main thread, ms per frame | 2.27 [1.78..2.45] | 2.07 [1.74..2.38] (U 31 / 49: within the noise) |
| circuit racing, High (2400 frames) | the first-conflict frame (223), main thread CPU | 7.4 [7.1..8.2] ms | **3.1 [2.6..3.5] ms** |
| circuit racing, High | main thread, ms per frame | 1.91 [1.81..1.98] | 1.87 [1.77..1.94] (U 34 / 49: within the noise) |
| circuit racing, High | render workers, ms per frame | 2.35 [2.18..2.38] | 2.34 [2.19..2.43] |
| circuit racing, Original (control: no look-ahead) | main thread / workers, ms per frame | 1.031 [1.013..1.046] / 1.056 | 1.033 [1.028..1.047] / 1.064 |

Medians [min..max] of 7 runs per arm; the frame's main thread CPU from the frame log's thread clock. The
first-conflict frame was the "10.9 ms draw" of the circuit's tail at High and the race start's 7.3 ms one; what
is left in it (about 1 ms over its neighbours) is, **inferred**, the first set's own work: the asked sections' cells and
the set.
The per-frame saving of the box test first is below this machine's noise (−0.04 ms median at High).

### Proposed, not built

* **The command-buffer clean-up off the main thread** (0.12–0.30 ms of main thread per frame at every level; 13–17 %
  of the main thread at Original): the frame's `device.poll` in `WindowState::settle` is where wgpu frees the
  finished frames' command encoders (the driver's command pools). Polling on a worker would move it, but the
  map callbacks (the staging belt's chunks, the light streaks' probe read-back) would then run there, at another
  moment: the streak's read-back decides its form, so this is not exact without a fixed hand-over point. Gain
  ~0.1–0.3 ms main; risk: timing of read-backs; touches the window's frame end.
* **Fewer, larger command encoders** for the passes the main thread records back to back (the workers' encoding
  is wgpu's per-command validation and tracking, 0.8–2.6 ms a frame; fewer encoders also mean less clean-up):
  not exact to reason about without a design of the encoder split; open.

### First-use hitches built ahead

Objects a drive's frames made the first time they were needed, now made with the drive's other objects
(docs/first-use-builds.md); the objects, their contents and their order of use are the same, so nothing drawn
changes (the gate).

* **The spray volume** (Wet Road Interaction, its row on): made by the load's warm-up as a job of its own beside
  the others, and beside a world loaded ahead in the menus (`WorldRenderer::spray_volume_ahead`), not by the first
  frame with a spray cloud. A volume that has drawn nothing holds no state (its `prepare` with no particles returns
  before touching it), so the one made ahead is the one the cloud made. Its memory, held from the load whether it
  rains or not: 16.7 MiB of GPU memory at Wet Road Interaction's first tier, read from the game's allocator
  report (`NFSU2_VRAM_REPORT`, High, the rain drive: `vfx accumulators` 9.00, `vfx density` 2.06, `vfx emission`
  2.00, `vfx particles` 1.25, `vfx albedo` 1.00, `vfx noise` 1.00, `vfx light` 0.25, `vfx light dir` 0.12 MiB);
  about 38 MB at the second tier (160 × 160 × 24 voxels), computed from the sizes in `VolumeFx::new` (not read).
  Its screen-sized targets (`vfx depth`, `vfx resolved 0 / 1`, `vfx march`: 21.6 MiB at 1080p) are still made by
  the first frame that draws a cloud, as before. That is 2–4 % of a High drive's GPU memory; it was not made conditional on the weather,
  since free roam's rain follows the world clock and every drive can reach it.
* **The lens drops' pass** (0x5C7AC0, native, every level) and **the rain's tint** pipelines (0x5C7870) are made by
  the post chain's warm-up (`PostFx::warm_frame`: the load, the programs made ahead, `--prepare`), not by the
  first frame with drops or a tint. The pass draws nothing until RAINDROPPC is set, as before.

Measured: one binary per arm, arms interleaved, 7 runs each, under the measuring lock; the rain drive
(`--bench drive 300` with `NFSU2_RAIN_CLOCK=162`: rain from the start, the first cloud in frame 2), the frame log's
main thread CPU (ms, median [min..max]):

| configuration | metric | before | after |
|---|---|---|---|
| integrated GPU, High | the first-cloud frame (2) | 13.0 [11.5..13.6] | not a slow frame; the run's worst of frames 0–59: 3.6 [3.3..4.6] |
| integrated GPU, High | `weather fx prepare` in that frame | 11.3 [9.6..11.5] | under 0.25 (not logged) |
| integrated GPU, High | `load_ms` | 283 [277..286] | 281 [278..289] |
| integrated GPU, Original | `load_ms` / worst of frames 0–59 | 135 [128..136] / 1.4 [1.3..1.6] | 136 [127..140] / 1.5 [1.3..1.7] |
| discrete GPU (shared), High | the first-cloud frame (2) | 13.3 [13.1..13.7] | the run's worst of frames 0–59: 2.5 [2.4..2.7] |
| discrete GPU (shared), High | `load_ms` / first frame | 280 [278..284] / 26.9 [25.1..27.3] | 279 [278..345] / 24.8 [23.7..25.3] |

The long sprint (one run per arm, discrete GPU, fixed step): High, frame 8884 13.6 ms with 3 pipelines and 1
module built → an ordinary frame, no pipeline after the first frame, the run's worst frame the race end;
Original, frames 8883 / 8884 built 1 / 3 pipelines (the tint, the lens drops) → none. A first version that made
the volume inside the world renderer's own warm-up (the load's longest job) added 29 ms to the integrated GPU's
High `load_ms` (291 → 320, 7 runs each); as a job of its own it adds nothing measurable (above).

**The menus' garage scene and the boot screens** (main menu's GarageMain: the platform's renderer, the FE car's,
its floor reflection): after the boot screens with the work ahead done (free roam's world loaded ahead and the
programs made, which run behind the boot movies and the title), the main menu's first frames made one pipeline,
the FE car's reflection set (`CarRenderer::prepare_reflection`); it is now in the cars' programs made ahead
(`car::warm_programs`), and they make none (`tests::first_use::the_menus_garage_scene_after_the_boot_screens_makes_no_program`,
every preset). `--prepare` already covered both (one run each, Original, the integrated GPU, the driver's shader
cache and the game's cache folder new for each arm): the boot screens' post chain and 2-D renderer took 13.7 and
9.6 ms of pipeline making unprepared, 2.0 and 1.2 ms prepared (as warm); the main menu garage's world pipelines
1224 ms unprepared, 86 ms prepared (a warm run of the same: 86 ms), its cars' 76 → 5.6 ms. A player who holds Enter
through the boot screens reaches the garage only after the title's 240 or so frames (about 0.2 s): measured
(`--bench boot` with `NFSU2_BENCH_BOOT_MENU=1`, the integrated GPU, one run each, Original and High, warm and empty
caches) the work ahead is done by then and the frame that makes the garage scene builds no pipeline (29.5–44.6 ms
with the GPU's wait: the platform's and the FE car's loads); nothing was reordered. The shops' garage scenes after a
free-roam load made the FE car's reflection set (one pipeline); a load's warm-up now makes it too
(`car::warm_reflection_set`), and every shop kind's scene makes none at every preset
(`tests::first_use::the_shops_garage_scenes_after_a_load_make_no_program`).

### Car renderers built ahead

The traffic pool's and the encounter racers' renderers are built on a worker once the cars exist and are not yet
drawn, and taken by the frame that first draws the car when built for the same device, format, sampler and texture
packs (docs/first-use-builds.md "Car renderers ahead"); otherwise built there as before. Measured: the race start
(`--bench race 1500`, the circuit's traffic paused for the countdown), discrete GPU (shared), fixed step, one
binary per arm, interleaved, 7 runs each; main thread CPU from the frame log, ms, median [min..max]:

| level | metric | before | after |
|---|---|---|---|
| Original | the run's worst frame (frames 10 on; before: always the countdown's end, a traffic car's renderer) | 2.79 [2.68..2.82] | 1.46 [1.31..1.67] |
| Original | frames with `car renderer builds` over 0.25 ms | 1 | 0 |
| Original | main thread per frame / `load_ms` / peak resident | 0.82 / 137 / 635 MB | 0.83 / 135 / 645 MB |
| High | the run's worst frame (frames 10 on) | 3.52 [3.29..4.27] | 2.70 [2.50..3.24] |
| High | frames with `car renderer builds` over 0.25 ms | 1 | 0 |
| High | main thread per frame / `load_ms` / peak resident | 1.52 / 293 / 722 MB | 1.51 / 292 / 732 MB |

The renderers are the ones the frames would build, so the memory they hold is the same once every car has shown;
the peak resident set is 10 MB higher (**inferred**: the worker's working memory and the frames' and the worker's
textures in flight at once). Measured again (discrete GPU, fixed step, interleaved): free roam's
encounter racers (`--bench explore 3600`, Original, 5 runs each; three racers drive onto the road at frames
about 601, 902 and 2324): the frames a racer appears in 2.05 [0.88..3.89] → 0.97 [0.88..1.33] ms of main thread
CPU, the frames that built a car renderer (over 0.25 ms) 4–6 → 0 a run, the run's worst frame unchanged (4.64
[4.23..4.86] → 4.74 [4.04..6.03] ms, other work). The sprint's frame 3050 (High, 3 runs each) did not show a
car renderer's build in these runs (1.77 [1.74..1.95] → 2.00 [1.58..2.15] ms, within the noise): the earlier
"2.0–3.6 ms at frame 3050" was not reproduced.

**The race's rivals** (docs/first-use-builds.md "Car renderers ahead"): their renderers were made by the race's
first frame; each is now built on its car's load thread in the race's set-up and offered to that frame (behind
the Engage dialog, on the thread that loads the car ahead). `--bench transitions` with
`NFSU2_BENCH_TRANSITIONS_RACE=1`, three rivals, one binary per arm, interleaved, 7 rounds under the measuring
lock; the first frame's draw (main thread, ms) and load / load + first frame (wall ms), median [min..max]:

| configuration | row | first frame's draw before → after | load before → after | load + first frame |
|---|---|---|---|---|
| integrated Original | free roam → race | 4.0 [3.8..7.5] → **2.0 [1.8..2.1]** | 24 → 25 | 29 → 28 |
| integrated High | free roam → race | 21.7 [18.5..23.2] → **19.0 [15.6..20.0]** | 33 → 34 | 55 → 54 |
| discrete Original | free roam → race | 4.3 [3.9..6.0] → **1.9 [1.6..2.0]** | 23 → 24 | 28 → 26 |
| discrete High | free roam → race | 6.3 [6.1..6.6] → **4.2 [3.9..4.6]** | 30 → 32 | 37 → 37 |
| integrated Original | after the Engage dialog | 4.4 → 1.8 | 36 → 37 | 54 → 54 |
| integrated High | after the Engage dialog | 8.1 → 4.4 | 76 → 78 | 201 → 203 |
| discrete Original | after the Engage dialog | 4.2 → 1.8 | 21 → 21 | 25 → 24 |
| discrete High | after the Engage dialog | 4.8 → 2.4 | 27 → 31 | 32 → 33 |

So 2–3.7 ms of the first frame's main thread go to the load's threads, where each renderer follows its car's
load: the load grows by 0–4 ms and the wait (load + first frame) moves by −2 … +2 ms, inside its spread — a
lighter first frame, no shorter wait. The bench's Engage row took none of the loads made ahead (0 of 3): the
prediction still drew the race field's seed before the line-up, a draw that had moved behind the line-up and
the looks (the world build, docs/rng.md), so it lined up from a stream one draw ahead of the race's. Without
that draw the race takes all of them (below, "The Engage dialog's line-up"). `--bench
race 120` (integrated GPU, 7 runs each): the first frame's `car renderer builds` 4.0 → 1.8 ms (Original), 4.3
→ 2.0 ms (High; what is left there is the race's traffic, **inferred**). Memory: the renderers are the frame's
own, made a frame earlier. **An ending's cars**: the ending's candidates are the race's own cars and draw with
their renderers; what is new at the finish is a lost race's companion (the AI winner riding along: a copy of
its car with an id of its own, `SceneCar::from_rival`), now built ahead during the race-over hold (below,
"A lost race's ending companion built ahead").

### The Engage dialog's line-up

**Cause** (the clone's two code paths, no native question): `predict_rivals` drew one `Rand_Int(0x7FFFFFFF)`
on its copy of the shared generator before the line-up, for the race field's seed; the race's set-up draws
that seed in the world build after the line-up and the looks (`start_round`, docs/rng.md), and nothing else
draws between the dialog's Engage and the line-up. Removed; the prediction also covers an event's first race
when it has several (URL events, tournaments). In an instrumented run before the fix, stage 1's eight events
took 0–1 of 3 cars each; after it every event of every kind takes all of them
(`tests::rivals_ahead::the_engaged_race_takes_every_rival_loaded_ahead`: 37 events, circuit, sprint,
Street X, drag, drift (a downhill drift has no rivals: none predicted), URL single and multi-race, stages 1–5).
The race is unchanged: the same rivals, cars, grid slots, generator after the set-up and state hashes over the
first 30 steps as the same start with nothing loaded ahead (the test), the pictures of the first five frames
identical (`tests::first_use::the_race_rivals_take_renderers_built_with_their_loads`).

Measured: `--bench transitions` with `NFSU2_BENCH_TRANSITIONS_RACE=1`, 1920×1080, the integrated GPU, one
binary per arm (the base and this tree, both with the bench's main-thread CPU print), interleaved, 7 rounds
per level under the measuring lock; wall ms and the main thread's CPU ms, median [min..max]:

| level | row | cars taken | load | load + first frame | the load's main-thread CPU | the first frame's main-thread CPU |
|---|---|---|---|---|---|---|
| Original | after the Engage dialog | 0 → 3 of 3 | 33 [33..48] → **22 [21..24]** | 54 [52..63] → 52 [48..53] | 9.2 [8.3..15.2] → 7.9 [7.4..10.4] | 2.8 [2.2..4.0] → 2.6 [1.8..3.4] |
| High | after the Engage dialog | 0 → 3 of 3 | 51 [48..68] → **38 [35..49]** | 105 [101..109] → 104 [94..106] | 13.6 [12.8..25.9] → 12.2 [11.7..22.3] | 3.6 [2.4..4.5] → 4.4 [2.7..5.5] |
| Original | free roam → race (control) | 0 of 3 | 22 → 22 | 25 → 25 | 8.8 → 8.4 | 2.6 → 2.9 |
| High | free roam → race (control) | 0 of 3 | 32 → 31 | 36 → 36 | 14.6 → 15.8 | 4.7 → 4.7 |

The load is 11–13 ms shorter (the three cars' parses and textures, and their renderers, done behind the
dialog); the wait (load + first frame) moves by 1–2 ms, inside its spread: after a menu drawn over the world
the first frame waits for the dialog's frames still on the GPU (High: 50–60 ms), which the shorter load does
not shorten. The first frame's main-thread CPU moves inside its spread (its renderers were already the load's
before). Memory: the loads ahead were made before too and dropped unused; now they are the race's own cars.
These rows are lower than the earlier table above (76 / 201 ms at High): another day's machine; only the
interleaved pairs here compare. Evidence: `docs/evidence/perf-budget/engage-line-up.txt`.

### The temporal rows' programs made ahead

**Cause** (the clone's code): the warm-ups (a drive's load, the menus' programs made ahead, `game
--prepare`) took the scene's sample count from the frame drawing when they started (a menu, the loading
screen: the native Full Screen Anti-Aliasing), while a drive's frames under TAA or a temporal Upscaling
tier draw single-sampled; so with the native FSAA on, a drive's first frame made the world's, the cars',
the horizon fog's, the blur's and the rain's programs for the temporal frame. They now take the drive
frames' count (`display::drive_msaa_samples`). Besides: FSR 2's passes made their bind group layouts outside
the device's cache, so its pipelines (keyed on them) were made again by the first FSR 2 frame whatever was
made before; FSR 1's EASU / RCAS pipelines were made by its first frame (`frame_passes` made the pass but
not its pipelines); the native glow's prefilter into a smaller glow (`fs_box`) was not in the post chain's
warm-up. Listed with the pipeline log and `tests::first_use::the_temporal_rows_are_ready_before_their_first_frame`
(TAA, Upscaling 1–11 × Low / Medium / High / Extreme, the native FSAA on; free roam's first ten frames):
before (that test's 320×240 window), TAA at Low 141 programs, FSR 2 Native AA 160–206 at every level, FSR 2's
other tiers 19, FSR 1 2–3 (at 1920×1080, below, TAA and TAAU at every level too); after, none, loaded straight away or after the menus' programs made ahead (whose load makes none).

Measured: `--bench explore 30`, 1920×1080, the integrated GPU, `NFSU2_BENCH_LEVEL` (a developer knob added
for this) and the row by `NFSU2_BENCH_ENH`, the bench's own settings (native FSAA on), one binary per arm,
interleaved, 7 runs each under the measuring lock (the machine at load 20–25 from other work); the first
frame's update + draw (main thread, wall ms) and the programs it made, median [min..max]:

| level | row | first frame before → after | its programs | frames 1–9: main-thread CPU (sum) / programs |
|---|---|---|---|---|
| Low | TAA | 45.6 [41.5..65.7] → **6.4 [5.6..10.7]** | 124 → 0 | 13.6 → 12.9 / 11 → 0 |
| Low | Upscaling Quality | 51.9 [40.9..397.6] → **6.8 [4.6..10.4]** | 124 → 0 | 13.0 → 13.3 / 11 → 0 |
| Low | FSR 1 Quality | 6.9 → 7.3 | 3 → 0 | 13.5 → 12.8 / 0 |
| Low | FSR 2 Quality | 67.6 [56.4..138.4] → **7.3 [5.8..10.4]** | 134 → 0 | 15.0 → 14.1 / 11 → 0 |
| Medium | TAA | 103.6 [92.6..216.3] → **12.8 [9.7..52.0]** | 158 → 0 | 19.2 → 20.3 / 11 → 0 |
| Medium | Upscaling Quality | 120.4 [97.3..142.9] → **11.0 [9.4..21.2]** | 158 → 0 | 19.2 → 18.0 / 11 → 0 |
| Medium | FSR 1 Quality | 11.8 → 12.3 | 2 → 0 | 18.3 → 20.1 / 0 |
| Medium | FSR 2 Quality | 127.4 [100.6..193.4] → **13.0 [10.2..14.5]** | 168 → 0 | 20.2 → 20.6 / 11 → 0 |
| High | TAA | 124.8 [97.4..129.2] → **14.4 [10.9..24.8]** | 163 → 0 | 24.8 → 22.7 / 13 → 0 |
| High | Upscaling Quality | 110.5 [99.6..2192.6] → **14.3 [12.3..16.5]** | 163 → 0 | 21.9 → 20.9 / 13 → 0 |
| High | FSR 1 Quality | 17.2 → 19.5 | 2 → 0 | 21.7 → 22.4 / 0 |
| High | FSR 2 Quality | 129.4 [103.9..1447.7] → **15.9 [12.5..28.6]** | 173 → 0 | 24.0 → 25.2 / 13 → 0 |
| Medium, native FSAA off | TAA | 11.6 → 10.6 | 0 → 0 | 20.1 → 19.4 / 0 |
| Medium, native FSAA off | FSR 2 Quality | 24.6 [23.4..30.5] → **12.1 [10.8..16.8]** | 10 → 0 | 20.0 → 19.0 / 0 |

(the last two rows 5 runs each). So with the native FSAA off (the player's settings) only FSR 2 changes for
the first frame; with it on, every temporal row. The load (`load_ms`, nothing loaded ahead in the bench)
moved inside its spread (e.g. Medium FSR 2 450 → 436, TAA 452 → 503 ms; FSAA off FSR 2 421 [407..458] →
475 [441..497]: the programs the first frame made are made by the load's warm-up there; after the menus'
work ahead, in the menus). FSR 1 had little to gain (its two pipelines were 0.4 ms).
`game --prepare` on an empty cache folder (warm driver cache, the integrated GPU, 3 runs each, the native
FSAA on): TAA 193 → 191 programs (the same sets at the drive's sample count instead of the menu's),
0.48–0.76 → 0.46–0.80 s, the pipeline cache 149 → 151 KB; FSR 2 Quality 211 → 219 programs (FSR 2's own),
0.52–0.77 → 0.50–0.76 s, 174 → 203 KB; the cache folder 179 MB either way. The menus' programs made ahead
run the same `drive_programs`, so they grow by the same programs (**inferred**, not measured separately).
Memory: the programs are the frames' own, made earlier. Evidence:
`docs/evidence/perf-budget/temporal-programs-ahead.txt`.

### A lost race's ending companion built ahead

The ending's companion (the AI winner riding along, `SceneCar::from_rival`) had its renderer made by the
ending's first frame that draws it. It is now built on a thread during the race-over hold (about 3 s from
the player's finish to the ending), once the player has finished behind a rival that has finished
(`loads::CompanionAhead`, docs/first-use-builds.md "Car renderers ahead"). Measured on that frame: a
stage-1 sprint driven by its rivals until the first finishes, the player put over the line 0.5 s later, the
ending's first frame with the companion at 1920×1080 on the integrated GPU, the work ahead on / off in
one measuring build (not kept), interleaved, 7 runs each under the measuring lock; its draw's main-thread
CPU (ms) and wall (ms, mostly the wait for the GPU's frames in flight), median [min..max]:

| preset | main-thread CPU off → on | wall off → on |
|---|---|---|
| Original | 6.2 [5.5..9.9] → **3.0 [2.8..3.8]** | 45.7 [38.7..55.9] → 47.3 [40.0..57.2] |
| High | 7.1 [4.7..15.2] → **4.1 [3.1..5.5]** | 137.0 [98.8..576.4] → 132.0 [112.1..161.0] |

About 3 ms of that frame's main thread go to the hold's thread; the frame's wall time is the GPU's (the
ending's first frame on this GPU), unchanged within its spread. Memory: one car renderer, the one the frame
would build, about 3 s earlier; the car loaded for it is dropped once it is built. Evidence:
`docs/evidence/perf-budget/companion-ahead.txt`.

### The race end

The finish's ending (`play_race_ending`) loaded one of EndingNis01..04 in the finish's frame: the NIS
table, the scene, the cameras and the scene's characters' bundle parsed. They are readings of the game's
files alone, so all four are now read on a thread from the race's start (`flow::endings_ahead`), and the
finish takes the one it picks (the pick and its random draw unchanged); a finish before the thread is done,
or a reading that failed, loads in place as before. `tests::scenes::an_ending_read_ahead_is_the_one_loaded_at_the_finish`:
for each scene, won and lost, the ending played with the readings taken is the one loaded in place (the
scene, its slot, its camera, its characters, the generator after). The profile's store at the finish
(`Profile::store`, about 3 ms) is left where it is: off the frame its file would be written later than the
finish's frame, which the durable-saves timing does not allow without a change to it.

Measured: the career sprint to its finish (`--bench drive`, fixed step, Original, the discrete GPU; thread CPU),
one binary per arm, interleaved, 5 runs each (each pair under the measuring lock): the race end's frame (9572),
main thread CPU 10.58 [9.46..14.07] → 7.61 [7.19..10.47] ms, of it the tick's update 7.12 [6.88..10.77] →
4.94 [4.49..6.71] ms; the run's median frame 0.83 → 0.87 ms (within the spread). Memory: the four scenes and
their characters are held from the start to the finish; the peak resident set did not move beyond its spread
(658 [644..658] → 652 [637..680] MB; their own size was not measured).

**The part map made ahead.** Of the store, the owned-part bits' map of each of the profile's cars
(`NativeContext::owned_bits`, a pure function of the part database and the car type, kept once made) was made
by the first store that needed it: a car never loaded from a save in this session, that is the first finish
after a car is bought or taken, or a new career's (a career loaded from its file has its cars' maps made by the
load; a part bought makes its car's). Now, when the profile's cars change, the map of a car it was never made
for is made on a thread (`Game::preload_owned_bits`, once a tick with the other work ahead: a comparison of the
slots' car names); the store takes it when it is made and makes it itself, as before, when it is not. Exact by
construction: the same function, and whichever is made first is the one kept
(`tests::part_group_owned_bits::a_new_cars_part_map_is_made_ahead`: the map made ahead is the map made in place;
no thread when nothing is new). One car's map: 2.0 ms of one thread here (240SX).

Measured: a drag race to its end (`--bench drive 4000` with `NFSU2_BENCH_KIND=drag`, fixed step, the bench's
new profile: its car's map was never made, as after a car bought; the frame log's thread CPU), one binary per
arm (the base and this tree), interleaved, 7 runs each under the measuring lock, load average 12–14; ms,
median [min..max]:

| the race end's frame (3642) | before | after |
|---|---|---|
| main thread CPU | 7.65 [7.01..9.21] | **5.58 [4.83..8.24]** |
| of it the tick's update | 4.39 [3.95..5.28] | **2.10 [1.82..3.57]** |
| the run's median frame (main thread CPU) | 0.71 [0.68..0.75] | 0.69 [0.68..0.78] |
| peak resident set (MB) | 563 [521..568] | 561 [519..571] |

So −2.1 ms of that frame here (the map, 2.0 ms), once per new car or career a session; a career loaded from its
file had its cars' maps made by the load before and gains nothing. Memory: the map is the one the store kept
(a few KB a car type). Evidence: `docs/evidence/perf-budget/race-end-part-map.txt`.

### Free roam's encounter racers ahead

**Cause** (the clone's own code, no native question): the menus' prediction (`predict_encounters`) drew a
traffic seed on its copy of the shared generator before the set-up's draws whenever free roam's traffic was not
loaded (from the menus with no drive, after every race), a draw free roam's load no longer makes (docs/rng.md:
the set-up's draws come first, the world build and the traffic's models after them). It predicted from a stream
one draw ahead and missed all three cars, but on a shop's exit, where the traffic stays, and when the stream fell
back in step in the names' draws (`draw_names` draws again for a name already taken; the bench's main-menu row
below is such a case: the old prediction's state is one draw ahead and it still takes 3 of 3). Removed; the
prediction is also made again on the first tick its inputs moved (it waited for a two-second slot), a question
over a screen no longer restarts the screen's half second, and the garage (Jump to Garage keeps the drive) and
the screens after the post-race ones (the save point's questions) count as on the way to free roam
(docs/first-use-builds.md "Encounter racers ahead"). Before, in the test's walks: Resume Career and the
garage, a race's end and its post-race menu 0 of 3 at every screen time, a shop's exit 3 of 3 after half a
second; after, every way in 3 of 3 (Resume Career with the garage up a third of a second, a race's end with its
results and save point, a shop's exit, Jump to Garage, a Quick Race left for the career; a fresh career and
careers at stages 2 and 4: `tests::encounters_ahead::every_free_roam_entry_takes_its_encounter_cars_loaded_ahead`,
which also finds free roam's racers, generator and first 60 ticks' state hashes the same as with nothing loaded
ahead). Evidence: `docs/evidence/perf-budget/encounters-ahead.txt`.

Measured: `--bench transitions` with `NFSU2_BENCH_TRANSITIONS_ROAM=1` (free roam, a city race, its post-race menu
up until the menu's loads ahead are in, back to free roam; then the main menu as at a game's start, no drive and
no traffic, up until its loads ahead are in, then free roam), 1920×1080, the integrated GPU, `NFSU2_TIMING=1` for
the load's wait for the encounter cars (`Encounters: cars joined`), one binary per arm (the base with the bench's
prints, this tree), interleaved, 7 rounds each under the measuring lock; warm, and with the cache's and the cars'
files dropped from the OS file cache before the menu (`NFSU2_BENCH_DROP_PAGES=1`, as after a reboot); the load
average was 19–40 (other builds on the machine), so the wall columns are loose; ms, median [min..max]:

| row | cars taken | the load | load + first frame | the load's wait for the encounter cars | the load's main-thread CPU |
|---|---|---|---|---|---|
| post-race menu → free roam, warm | 0 → 3 of 3 | 41 [31..65] → 36 [20..81] | 59 [33..77] → 64 [45..87] | **8.2 [3.8..17.5] → 0.6 [0.3..3.5]** | 11.9 → 10.1 |
| post-race menu → free roam, cold | 0 → 3 of 3 | **94 [35..279] → 35 [23..37]** | **102 [57..281] → 56 [50..72]** | **52.6 [6.7..73.8] → 0.2 [0.2..0.4]** | 6.6 → 5.9 |
| main menu → free roam, warm (control: the old prediction in step here) | 3 of 3 both | 61 → 75 | 65 → 82 | 0.3 → 0.3 | 23.2 → 23.2 |
| main menu → free roam, cold (control) | 3 of 3 both | 68 → 70 | 76 → 73 | 0.2 → 0.2 | 22.5 → 20.6 |

So the load no longer waits for the encounter cars: 8 ms warm here, 53 ms (up to 74) with their files to read
from the disk; with a cold cache the post-race menu's way back to free roam is 59 ms shorter (94 → 35, and its
worst run 279 → 37). The main-thread CPU does not change: the in-place loads ran on threads before too, the
main thread waited for them. Memory: the three cars loaded ahead are the set-up's own; before, the three loaded
ahead were dropped and three more loaded.

### A race raced again

Restart and a tournament's Next Race race the same field in the original (docs/formats.md "Race types:
Restart and Next Race keep the field": the track switch for messages 9 and 10 runs the event's set-up
without the line-up, the looks and the grid; seen under Wine for Restart). The clone drew all three again
on a restart, so none of its rivals' loads was the race's (0 of 83 keys over 30 events). It now keeps the
field (`Field::looks`, `Field::slots`; a sim change on restart paths only, as natively), and the set-up
takes the race before's cars for the same keys (`Loads::raced_cars`: the paused race's, the finished race's
kept for its replay or, without a recording, kept at its end) instead of loading them. Exact: the key is
the load's arguments (car, parts, performance packages), and the only part of a car the race changes, its
handling, the set-up makes again from the car's stock record (`set_race`, the difficulty, the drag shift
set-up); each car's renderer is built again from it on the car's thread as for a load. Check:
`tests::race_again::a_restart_with_the_cars_kept_is_the_restart_that_loads_them` (17 races: every event
kind of stages 1 and 3 and five Quick Race modes; the same field, generator and 30 steps of state hashes as
a restart that loads the cars), `restart_races_the_same_field_again`,
`a_tournament_keeps_its_field_from_race_to_race`. Memory: the three (URL: five) rivals' cars stay from the
pause or the race's end to the next set-up instead of being dropped and loaded again; they go when free
roam loads or the drive is dropped.

`--bench transitions`, `NFSU2_BENCH_TRANSITIONS_RESTART=3` (a stage-1 city circuit, three restarts from its
pause after 2 s each), integrated GPU, one binary per arm, 7 interleaved series (21 restarts an arm), load
average 13–16; ms, median [min..max]:

| restart | the restart's load + first frame | its rivals (`line_up_rivals`) |
|---|---|---|
| warm | 47 [26..189] → **13 [12..46]** | 20 [13..41] → **2 [2..10]** |
| the cache's and the cars' pages dropped (`NFSU2_BENCH_DROP_PAGES=1`, as after a reboot) | 340 [127..541] → **25 [16..65]** | 316 [115..515] → **2 [2..11]** |

The restart's first frame after the load: the race's traffic models (`Traffic::load`: the original draws
them again too, its track loader's state 6 at 0x412BE0 runs on a restart's reload, 0x581999 → 0x57F2E0;
2.5–5 ms warm), the start scene's set-up (2.5 ms) and the frame's draw (2–19 ms): 9–31 ms over six
windowed restarts on the integrated GPU (one after 15 s of driving, as the sample session's). The 308 ms
first frame of the sample session (docs/waits-log.md) was not reproduced; the frames after a restart that
did run long here (150–306 ms in its first 3 s, as long as the drive's own before it, up to 2.1 s) spent
their time outside every CPU span of the frame (in the GPU's or the present's waits on the shared
integrated GPU: **inferred**), nothing a restart does.

### A Quick Race's rivals ahead

A Quick Race took none of its rivals from the menus' loads ahead (`rivals 0 of 3`, 425 ms of a 466 ms load
in the sample session): nothing predicted its line-up. Its rivals now load ahead while the transmission
question is up (docs/first-use-builds.md, `Game::predict_quick_rivals`): the line-up is drawn after the
track's load from the event slot the options set up, with the picked car's SUV pool and the player's car as
`drive_in` makes it; nothing draws on the shared generator between the pick and the line-up. Check:
`tests::rivals_ahead::the_quick_race_takes_every_rival_loaded_ahead` (every racing mode, three cars, an SUV
among them: every car taken, the same rivals, generator and 30 steps of state hashes). A drag race and a
player who turned the question off start at once (nothing ahead).

### The wait before the load

The transmission question's answer wrote options.txt (`Settings::save`) on the main thread between the key
and the race's load, though the answer changes only the profile's byte (0x83AA79; options.txt does not hold
it): a temporary file, a write and a rename, 8–17 ms in instrumented runs here and longer when the disk is
busy (one run had 237 ms between the key's tick and the set-up, inferred to be this write). The answer now
updates the current profile only (`Settings::save_to_profile`; options.txt's bytes are the same as before).
The rest of the part before the load is the key's wait for its tick (up to a frame) and the set-up before
the track's load (the route file, 1–10 ms). The sample session's 532 ms before the load did not reproduce as
such (29–80 ms in most windowed runs here, 595 ms once with the old binary, the answer's tick then
unspanned).

### Gate

`GATE_PRIO=3 RATE=1 CHECKS=fast gated.sh scripts/checks/gate.sh … <base> extreme`: CHECKS PASS (fast: build, 1900
tests passed, 0 failed, 60 ignored; selftest; 31 shots; first race / drag, post race, playthrough); PARITY all
gates passed (Original images AE 0, the 144 Hz window path AE 0, enhanced Extreme / Extreme FSR AE 0, 31 shots
0 differ, audio identical, the five state hashes identical); RATE PASS (identical cells 600 / 600); binaries
differ.

## The Enhanced levels and the Advanced rows, pass by pass

The per-pass table of Original / Low / Medium / High in four scenes on
both GPUs, the rows that scale with the pixels, the lights and the cars,
Medium on the integrated GPU row by row, the candidates tried for the
same picture and the proposals: docs/perf-integrated-gpu.md "The Enhanced
levels, pass by pass". Desktop GPU (otherwise idle in most runs), GPU
busy median ms, 2 runs, 300 frames, worst of explore / racing / race
start / rain: Original 0.72 / 2.63, Low 0.77 / 2.67, Medium 1.78 / 7.13,
High 3.35 / 11.54 (1920×1080 / 3840×2160); the Advanced rows on Medium,
racing: TAA 1.27 / 3.67, Upscaling Quality 1.06 / 2.68, Performance 0.99
/ 2.32, FSR 1 Quality 1.20 / 3.73, FSR 2 Quality 1.39 / 4.31, MSAA 4x 1.49
/ 5.02, SSAA 2x 1.71 / 6.02, 4x 2.89 / 11.26, 8x 6.12 / 23.38, 9x 6.71 /
25.59, 16x 12.11 / 47.19, Ray Tracing 3.76 / 12.83 (Medium itself 1.75 /
6.60). Evidence: `docs/evidence/perf-budget/levels-passes-desktop.txt`.

## Ray-traced reflections: the rays from a list in a compute pass (exact)

The Ray Traced reflections' rays left the SSR trace's fragment pass (docs/rt-reflections.md "The ray
list"): the fragment pass keeps the screen march and lists the pixels where `rt_fill` would trace; a
compute pass traces each listed pixel on a lane of its own and writes its result into the trace target.
The same values: every picture and every bench frame compared is byte-identical (below). Reference
desktop GPU, 3840x2160, the reference player's settings (SSAA 2x, Ray Traced reflections, Modern + RT);
7 interleaved runs per arm, 150 frames, the median of the runs' GPU busy medians [min .. max], each series
under the measuring lock with the desktop GPU idle before it (load average 31-56 from other units):

| scene | frame before | frame after | fps | trace before → after (march + rays) |
|---|---|---|---|---|
| racing | 19.02 [18.90..19.21] | 18.55 [18.43..18.57] | 52.6 → 53.9 | 3.17 → 2.55 (1.60 + 0.94) |
| race start | 20.27 [20.13..20.37] | 19.95 [19.88..20.08] | 49.3 → 50.1 | 3.31 → 2.99 (1.73 + 1.26) |
| rain | 23.45 [23.37..23.53] | 22.87 [22.76..22.92] | 42.6 → 43.7 | 3.62 → 2.91 (1.86 + 1.06) |
| free roam | 13.06 [13.01..13.20] | 12.74 [12.69..12.82] | 76.6 → 78.5 | 2.79 → 2.45 (1.56 + 0.89) |

The share of the trace's pixels that need a ray (`NFSU2_RT_LIST_STATS`): racing 41-45 %, race start
27-36 %, rain 32-37 %, free roam 35 %. That many rays leave little to gain from waves of sparse rays, so
the gain is the fragment form's own cost, not the 1.3-1.8 ms a sparse list was estimated at: chiefly its
ray-query wave size (wave32 on RADV; forced to wave64, the old form's trace takes the list's 2.53 ms:
"Wave sizes" below) and its scratch-memory stack; the rays themselves (about 3.6 M a racing frame in
0.94 ms) are the rest. `rt_fill`'s two casts on one ray query instead of two: rays 1.14 → 0.97 ms
(racing, kept). Memory: the list, 20 bytes per trace pixel: 41.5 MB at 1080p, 73.7 MB at 1440p,
165.9 MB at 4K. Pictures (`--shot race / qrstart / intro / explore_rain` at those settings, race and
qrstart with RT reflections under Modern lighting and with the native MSAA 8x) and the bench's dumped
frames (racing, race start, rain, free roam, every 40th of 200) md5-identical before and after. The
RT trace's pipelines were made in the first traced frame before; the warm-ups make them now. Evidence:
`docs/evidence/perf-budget/rt-ray-list.txt`.

**The NIS characters' rays (not moved).** The characters (the flag girl, the crowd) keep their rays
in their colour shader (`D9_ML_CHAR`, docs/shadow-softness.md). Measured at the race start (the
settings above, the new tree, 3 runs each): the bucket that holds their draws ("graphics: ssao depth
(and the resolve before it)") is 2.84-2.87 ms; with the driver skipping every ray query
(`RADV_DEBUG=nort`, a measuring aid) 1.80 ms, so their rays cost about 1.05 ms there; with Modern
instead of Modern + RT the bucket is 1.33 ms (frame 20.04 / 17.63 / 16.38 ms). Moving their rays out
the same values would need the rays of every fragment their pass shades, also those later drawn over
or blended under (they are depth-written, with their textures' blending, crowd members overlapping),
and of each 2 × 2 quad's helper lanes, whose values enter the shadows' quad-mean resolve
(`ml_quad_mean`, `dpdxFine` / `dpdyFine`): a helper lane cannot write storage, so a pass before
theirs could not list it. And the rays follow the per-fragment ranking of the lights, so such a pass
would run the light loop again (the RT variant's ranking alone is 0.47 ms of the bucket over Modern's
1.33): more than the rays it would move (traced from a list they would still cost a part of the
1.05 ms). Tracing them at the blocks' half resolution (the world's rt_shadow pass) would change the
values. Left as is; evidence: `docs/evidence/perf-budget/rt-ray-list.txt`.

**Wave sizes (RADV).** RADV compiles every shader that holds a ray query in the ray-query wave size
(32), whether the query is reached or not; the Modern + RT world colour shaders hold the characters'
(dead outside their pipelines) and so run in wave32, the list's compute pass too, its fragment pass in
the pixel shaders' wave64. Measured with `RADV_PERFTEST=rtwave64` as an instrument (racing): the old
fragment form's trace was 0.64 ms faster in wave64 (the list reaches the same 2.53-2.55 ms at the
default size; its compute pass is the same in both), the world colour pass 0.5 ms slower. A module of
their own for the characters (the other pipelines without a ray query, so wave64) was built and dropped
for that: pictures identical, the world colour pass 6.34 → 6.75 ms, the frame 18.68 → 18.93 ms. So the
characters' rays stay in the shared module on purpose. wgpu 30 cannot ask for a wave size.

## SSAA: the passes that gain nothing from the larger scene, at the screen's size

docs/ssaa-screen-passes.md: under SSAA (with High Precision Post) SSAO's occlusion runs on the screen's half and the
scene is resolved to the screen's size before the bleach bypass (each scene texel still graded before the average),
the glow's source and the last pass then at the screen's size. A 4K play setup (SSAA 2x, High Precision Post, ray
tracing, SSAO 100 %), reference desktop GPU, 7 interleaved runs per arm: racing 19.19 → 18.50 ms GPU busy (52.1 → 54.0
fps), those passes −0.71 ms in every pair (SSAO −0.32, the post chain −0.39); free roam 13.04 → 12.39, race start
20.26 → 19.58, rain 23.40 → 22.54 ms (2 pairs each). Without SSAA nothing changes. Tried and not kept: the streak
buffer at the screen's size (−0.06 ms: its glow sprites' depth copy eats the saving). Evidence:
`docs/evidence/perf-budget/ssaa-screen-passes.txt`.

## Fog, volumes and rain: the same pictures for less

The night-rain frame at the player's settings (4K, SSAA 2x, every row up, ray tracing: `--bench drive` at clock 162,
rain 0.70) and the integrated GPU's Medium and High levels (1080p, World Detail 3), pass by pass with each pass's
dispatches timed apart (a measuring build; raw runs and tables: `docs/evidence/perf-budget/fog-rain-passes.txt`).
Each change below computes the same effect with less work; the frames it leaves byte-identical say so.

| change | what it does instead | pass, before → after (ms, median of 5–7 interleaved runs [range]) | frames |
|---|---|---|---|
| Lens drops without the frame's copy (High Precision Post) | copies only the texels the drops sample, draws them onto the frame itself (docs/weather.md) | 4K rain: copy 0.458 [0.456..0.460] → 0, drops 0.032 → 0.048 | identical |
| The rain's haze left out under Volumetric Fog | its in-scatter and extinction are 0 there: both its passes are left out (docs/rain-nextgen.md) | 4K rain: haze 0.074 + 0.090 → 0 | identical |
| The fog's light injection culled per workgroup | each 8 × 8 cell workgroup tests the lights against its slice's box once, the cells walk the lights that reach it (docs/atmosphere.md) | 4K Next-Gen rain / racing: 0.114 → 0.061 / 0.172 → 0.055; integrated GPU High 0.668 → 0.548 / 0.772 → 0.455 | identical |
| The volumes' resolve per workgroup | the bounds read once a workgroup, plain loads and stores for the accumulators, the bricks gathered there (docs/smoke-volumetric.md) | 4K spray 0.031 → 0.023; integrated GPU High 0.269 → 0.190 | identical |
| The volumes' march specialised to each renderer's look | override constants leave the fine octave, the directional shading and the surface shadow out of the programs whose look never takes them (the spray's: 144 → 72 registers, 10 → 20 waves) (docs/smoke-volumetric.md) | 4K spray march 0.414 → 0.368; integrated GPU High 7.11 → 6.63 | the compiler's rounding: 1 level on ≤ 7 pixels of a 480 × 270 frame |
| The rain's tint folded into the Volumetric Fog's apply | where the tint comes right after the medium (no lens drops, no car lights' streaks, High Precision Post): the apply multiplies the light it adds and, by its blend, the scene by the tint; the tint pass is left out (docs/atmosphere.md) | integrated GPU Medium rain: tint 0.412 → 0, apply 1.158 → 1.123 | the half float's rounding: ≤ 2 levels, 0 % of pixels > 1 % |
| The SSR mask's road term from the main view | the main view's road writes the mask's term and surface from the `wet_surface` its colour takes; the mask pass draws the road's depth and kind, and works the term out only for the road the main view hides; wet road, discrete GPU (docs/wet-roads-ssr.md "The mask from the main view") | 4K rain: mask 0.738 → 0.220, main view 6.87 → 7.18, frame 22.72 → 22.41; integrated GPU High rain: mask 5.51 → 0.92, main view 15.80 → 20.14 (−0.25: not taken there) | identical |

The whole frame with the six changes (the game without the measuring marks, before → after, GPU busy ms, median of 7
interleaved runs [range]): at the player's settings in 4K rain 23.42 [23.20..23.50] → 22.79 [22.69..22.89] (−0.63:
lens drops 0.47 → 0.05, weather fx 0.24 → 0.08, atmosphere 0.42 → 0.36, volume fx march 0.56 → 0.51), racing 19.11
→ 18.99, free roam 14.14 → 14.12. On the reference integrated GPU at 1080p, World Detail 3: Medium rain 28.66
[27.13..28.97] → 28.05 [26.93..28.59] (rain tint 0.40 → 0), racing 24.70 → 24.56, free roam 17.46 → 16.82; High rain
70.11 [68.90..73.62] → 70.61 [69.58..72.73] (rain tint 0.41 → 0, volume fx march 7.42 → 7.10, atmosphere 2.18 →
2.06). The integrated GPU's frame moves by more than these from run to run (a slow state of its memory under the build
load of the machine measured on, docs/perf-integrated-gpu.md "Fog, volumes and rain": every High run here was in it); its pass rows are the
measure. Pictures, 19 frames each (0, 60, 120 and 16 consecutive from 200), before against after: 4K the player's
settings at most 1 level, 0 % of pixels > 1 %, 1 − SSIM ≤ 8e−8; 1080p Medium and High at most 2 levels, 0 % > 1 %,
1 − SSIM ≤ 1e−4; the difference's own frame-to-frame change 0.0001 / 0.034 levels (no flicker added). Two runs of
the same build give the same bytes, so these are the two rounding-level steps' differences.

## The world colour pass's lamp work culled per cluster

The pixels of the lamps' world shader tested every car per lamp (Lamp
Shadows), every car again for the headlight and for Contact Shadows, every
lamp of a list built from the lamps' screen boxes, and every beam with two
square roots before its range test. The CPU now culls the cars per light
cluster (bit masks in the clusters buffer), the lamp lists to the clusters
a lamp's sphere reaches, and the loops take exact early outs
(docs/lamps-shadows.md "Per-pixel work"). Every bench frame byte-identical
(Medium, High and every row up; at rest and while driving). Main view, GPU
ms: integrated GPU 1080p World Detail 3 Medium racing 11.2 → 7.5 (frame
24.5 → 21.0), rain 12.5 → 8.5, race start 9.4 → 7.9, High racing 13.1 →
8.8; desktop GPU 4K every row up racing 6.1 → 4.5 (frame 18.9 → 16.9, 53
→ 59 fps), rain 6.6 → 5.3, race start 5.5 → 4.8. Main-thread CPU +0.15–0.27
ms per frame. Evidence: `docs/evidence/perf-budget/lamp-culling.txt`.

## Modern + RT's traced shadows: the colour shader's registers

Reference desktop GPU, the player's 4K settings (SSAA 2x, Modern + RT, ray-traced reflections),
`--bench racing 300`, GPU ms per pass; A and B interleaved, the paired difference's median over the
pairs; load average 20–45 from other jobs. Shader statistics: `RADV_DEBUG=shaderstats` (needed /
allocated VGPRs per lane, waves per SIMD in wave32 units). Design: `docs/shadow-softness.md`
"The gather first". Evidence: `docs/evidence/perf-budget/rt-shadow-gather.md`.

| step | world PS | reflect PS | main view | frame |
|---|---|---|---|---|
| before | 126 / 144 / 10 | 161 / 168 / 9 | 6.12 [6.02–6.27] | 19.03 |
| the gather first, ids packed (exact) | 120 / 120 / 12 (22 of 33 pipelines) | 151 / 168 / 9 | 6.01 [5.85–6.12], **−0.15** paired | 18.98, −0.12 paired |
| + the gather's short cuts (uniform blocks, no light; exact) | 120 / 120 / 12 (22 of 33) | 153 / 168 / 9 | 5.88 / 5.87 vs 6.10 / 6.12 (2 rounds), **−0.23** | 18.69 / 18.68 vs 18.85 / 18.88, −0.18 |

Then, under supersampling only (non-exact there; identical without it): the blocks follow the screen
(`docs/shadow-softness.md` "The blocks follow the screen under supersampling"). Against the step
before, interleaved pairs, paired median (GPU ms):

| scene | rt shadows | main view | frame |
|---|---|---|---|
| racing (3 pairs) | 0.86 → 0.45 (−0.40) | −0.02 | 18.67 → 18.21 (**−0.45**) |
| race start (2) | 0.91 → 0.48 (−0.44) | −0.01 | 20.45 → 19.99 (−0.46) |
| night rain (2) | 1.01 → 0.52 (−0.49) | −0.05 | 22.86 → 22.28 (−0.58) |
| photo mode (2) | 0.82 → 0.43 (−0.39) | −0.05 | 18.68 → 18.21 (−0.47) |

Then the road's dry-road pipelines (exact; `docs/wet-roads-ssr.md` "The road's dry-road pipelines"),
dry racing against the step before, paired medians:

| | road PS registers needed / allocated / waves | main view | frame |
|---|---|---|---|
| Modern + RT, the player's 4K settings (3 pairs) | 153 / 168 / 9 → 123–125 / 144 / 10 | 5.80 → 5.62 (−0.22) | 18.12 → 17.99 (−0.19) |
| Modern, the same settings (2 pairs) | 143 / 144 / 10 → 115–116 / 120 / 12 | 4.93 → 4.62 (−0.31) | 16.33 → 16.01 (−0.32) |
| integrated GPU, Medium, 1920 × 1080, detail 3 (2 pairs) | 79–80 / 80 / 12 → 60–62 / 64 / 16 | 11.44 → 10.27 (−1.17) | 24.6 → 23.4 (−1.2) |
| integrated GPU, High, the same (2 pairs) | | 11.88 → 11.02 (−0.87) | 44.9 → 43.8 (−1.1) |

Night rain (the road wet: the full set) is unchanged (−0.03 main view, one pair). The integrated GPU's
runs were in its slow state (another job's build load: SSAO occlusion 2.0–3.5 ms instead of ~1.0);
the main view's difference was the same in each pair. All four steps against before them, racing at
the player's 4K settings: 18.9 → 18.0 ms (53 → 55.6 fps).

## The glow's blur where it can be lit

High Precision Post's two blur passes run only on the 8 x 8 tiles of the glow their result can light
(docs/hdr-post.md "Blur where it can be lit"; exact: frames identical). GPU timestamps, racing, medians
of interleaved runs (docs/evidence/perf-budget/glow-blur.md):

| | glow chain (prefilter, need, blur) | frame |
|---|---|---|
| desktop GPU, the player's 4K settings (SSAA 2x, Full-Size Glow; the glow from the resolved image) | 0.92 → 0.53 ms | 17.69 → 17.23 ms |
| the same, night rain | 0.98 → 0.74 ms | 21.2 → 21.0 ms |
| integrated GPU 1920×1080, World Detail 3, High (Full-Size) | 9.47 → 5.25 ms | 47.8 → 44.5 ms |
| integrated GPU 1920×1080, World Detail 3, Medium (Half-Size) | 1.56 → 1.03 ms (the streak add after it +0.07–0.10: open) | 25.2 → 25.0 ms |

## Screen-space reflections: the composite by the blend

With a half-float scene (High Precision Post) the reflections' composite is added onto the scene by the
blend instead of read from a copy of it (docs/wet-roads-ssr.md; at most two 8-bit steps on 0.03–0.28 % of
the pixels). Racing, the player's 4K settings: copy + depth pyramid 0.74 → 0.56 ms, composite 0.39 → 0.31 ms,
frame 18.70 → 18.28 ms (again on a later tree, after SSAA's resolve: 0.73 → 0.57 and 0.38 → 0.31 ms; with the glow's
blur where lit, the frame 17.89 → 17.27 ms); integrated GPU 1920×1080 High: 1.07 → 0.80 and 0.74 → 0.60 ms
(docs/evidence/perf-budget/ssr-composite.md, also the march's counts).
