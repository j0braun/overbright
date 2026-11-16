# Frame time on an integrated GPU and four cores

Measured, not estimated: the development machine has a desktop GPU and an
integrated one (2 compute units, RDNA2, memory shared with the CPU), and the benches run on
either. The target is a small laptop: 4 cores, 15 W, an integrated GPU (a
Vega 8 class iGPU is within about ±30 % of the reference integrated GPU, by
the performance triage's scaling; that factor is an estimate).

**World Detail.** Unless a table says otherwise it was measured at the bench's default World Detail 5:
SPEED2.EXE's top preset, whose Customize Display values include Full Screen Anti-Aliasing 3 (the scene at
8× MSAA, the clone's **inferred** mapping), anisotropic filtering, Enhanced Contrast (the bleach bypass)
and rain splatter. A fresh install never starts there on an integrated GPU. The game's first-run estimate
(`gpu_rating::estimate_detail`, from `display_defaults::first_run_display`: 0.2 × the card's score, 1 for
any card not in the game's table, + 0.8 × the CPU's tier from its clock, truncated) gives World Detail 3
on this machine (0.2 + 0.8 × 4 = 3.4; the clock read is the boost clock where cpufreq has no
`base_frequency`, as here), 2 on a 15 W laptop whose base clock is under 2.4 GHz, and 0 on any Intel GPU.
`custom_presets(3)` has no MSAA (Full Screen Anti-Aliasing 0), trilinear filtering, no bleach bypass and no
rain splatter. What such a player's frame costs is "The Enhanced levels at World Detail 3" below; the
World Detail 5 tables are what a player gets who raises everything.

## Method

* **Adapter.** RADV's device select layer:
  `MESA_VK_DEVICE_SELECT=<vendor:device> MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE=1`
  (`scripts/perf/perf.sh`'s `IGPU=1` with `IGPU_DEVICE` from
  `scripts/local.env` does the same). The bench prints the adapter it got.
  Offscreen only: the integrated GPU drives no display here. The device
  line lists what the device was made with; multi-draw count and indirect
  first instance are offered by both GPUs here ("adapter offers") but not
  requested: no draw is indirect (the world's and the cars' draws are
  direct, culled on the CPU, ~1–2 k a frame), so nothing loses by it.
* **Cores.** `taskset -c 0-3` (four physical cores, no SMT siblings) as the
  laptop's stand-in; `taskset -c 0-1` for two. The render pool sizes itself
  from the affinity (`available_parallelism`): 3 workers on 4 cores, 1 on 2.
* **Runs.** `--bench` with `NFSU2_BENCH_JSON`, 300 frames (rain: 600 at a
  60 Hz schedule), arms alternated per configuration, 3–5 runs each; one
  bench at a time while the machine ran other work (load average 10–35).
  Values are medians over the runs (scatter = min–max of the runs). GPU time
  is the timestamp sum per frame; CPU is thread CPU (`main (game)`, the
  render workers summed). Scripts: `docs/evidence/perf-integrated-gpu/scripts/`
  (`bench.sh`, `tab.py`, `cparts.py`).
* **Scenes.** free roam (`explore`: the burnout start, smoke), race start
  (`race`: 4 cars in view), rain (`drive` at rain clock 162 s), night with
  many lamps (`explore` placed at the casino, -400,-524,19.5,180, frozen,
  clock 93). **Presets:** Original; Balanced (High Precision Post, MSAA 8x,
  Enhanced Paint); High as the integrated GPU class gets it
  (`Preset::apply_for(Integrated)`: Rain Detail On, Volumetric Fog off).

## Where the time goes (integrated GPU, before)

GPU ms per frame (median of 3 runs) and frame time; full pass lists in
`docs/evidence/perf-integrated-gpu/before-igpu.txt`.

| scene | preset | 1366×768 GPU / frame p50 / p99 | 1920×1080 GPU / frame p50 / p99 |
|---|---|---|---|
| free roam | Original | 4.6 / 4.7 / 8.3 | 10.6 / 10.8 / 15.6 |
| free roam | Balanced | 5.9 / 6.0 / 10.4 | 12.1 / 12.3 / 20.0 |
| free roam | High | 11.5 / 11.7 / 17.8 | 26.4 / 26.6 / 36.3 |
| race start | Original | 7.2 / 7.5 / 11.1 | 14.8 / 15.0 / 20.0 |
| race start | Balanced | 8.5 / 8.7 / 12.6 | 17.0 / 17.3 / 24.3 |
| race start | High | 20.6 / 20.9 / 24.1 | 44.3 / 44.8 / 50.6 |
| rain | Original | 6.8 / 6.9 / 9.4 | 13.7 / 14.0 / 20.1 |
| rain | Balanced | 8.0 / 8.2 / 11.3 | 15.4 / 15.6 / 21.3 |
| rain | High | 21.3 / 21.6 / 27.6 | 46.9 / 47.2 / 59.5 |
| lamps | Original | 5.3 / 5.5 / 8.1 | 11.1 / 11.3 / 14.2 |
| lamps | Balanced | 6.3 / 6.5 / 10.3 | 13.2 / 13.5 / 18.7 |
| lamps | High | 13.9 / 14.2 / 17.7 | 31.0 / 31.4 / 36.4 |

Main-thread CPU 0.7–2.5 ms and render workers 0.9–3.0 ms per frame on the
four pinned cores: every configuration is GPU-bound on the integrated GPU.

Original at 1920×1080 (ms and share of the frame's GPU time):

| pass | free roam | race start | rain | lamps |
|---|---|---|---|---|
| world: main view | 3.27 (30 %) | 6.24 (42 %) | 5.70 (41 %) | 4.82 (43 %) |
| post: bleach bypass (3 passes) | 1.84 (17 %) | 1.92 (13 %) | 1.25 (9 %) | 1.86 (17 %) |
| post: streaks | 1.41 (13 %) | 1.54 (10 %) | 1.43 (10 %) | 1.48 (13 %) |
| particles (the burnout's smoke) | 1.51 (14 %) | 0.85 (6 %) | 0.54 (4 %) | — |
| post: overbright (half size) | 0.98 (9 %) | 1.02 (7 %) | 1.07 (8 %) | 1.02 (9 %) |
| cars | 0.50 (5 %) | 1.89 (13 %) | 1.23 (9 %) | 0.52 (5 %) |
| post: lens drops | — | — | 1.03 (7 %) | — |
| ui | 0.57 (5 %) | 0.42 (3 %) | 0.59 (4 %) | 0.57 (5 %) |
| post: to the back buffer | 0.40 (4 %) | 0.46 (3 %) | 0.42 (3 %) | 0.40 (4 %) |
| post: rain tint | — | — | 0.26 (2 %) | — |
| world: environment cube | 0.18 (2 %) | 0.18 (1 %) | 0.18 (1 %) | 0.24 (2 %) |

High at 1080p adds Screen-Space Ambient Occlusion (depth, occlusion, blur,
apply: 3.6–11 ms, the largest single row), the depth pre-pass (0.8–1.9 ms)
and a main view 2–3× heavier (dynamic lamps, shadows, wet roads).

## What each Original row draws, and what is left when its input is empty

A row of the pass table is the GPU time from the frame's previous
timestamp to the row's own, so a row holds every pass recorded between the
two marks. Split by extra timestamps (a measuring build, 1920×1080, 2 runs:
`pass-split.txt`), free roam / race start / lamps, ms:

| row | what it holds in an Original frame | split |
|---|---|---|
| particles | free roam: the native particle quads (110–170 a frame: the burnout start's tyre smoke, drawn into the 8x multisampled scene, depth-tested, blended); race start: no particle at all, the row is the race start crowd (characters drawn after the shadows when ambient occlusion is off); rain: the rain's quads | free roam 1.44–1.53 smoke; race start 0.81–0.85 crowd |
| bleach bypass | the resolve of the multisampled scene (the post passes read it), then the three native passes (0x5C68E0), each a full-screen 8-bit hand-off | resolve 0.52–0.58, passes 0.63–0.73 / 0.30–0.34 / 0.37–0.38 |
| streaks | the streak buffer's decay (0x5CBB6A), the buffer copied into its multisampled copy, the glow sprites drawn there and resolved back (0x5CBBB2..0x5CBCE5), then A · factor (0x5CCB9A) | decay 0.29–0.44, copy 0.30–0.35, glows + resolve 0.43–0.57, add 0.27–0.38 |
| overbright (half size) | C and A at half size, A blurred twice across and down, A −= C · factor (0x5CCA10) | C + A 0.21–0.23, blur and subtraction 0.78–0.80 |
| to the back buffer | the image plus the streak buffer, one pass into the output | 0.41–0.51 |
| ui | the HUD; in the bench also a full-screen quad that sets the alpha back to 1 (the bench's offscreen target is not an opaque swap chain; a window with one skips it, `Alpha::opaque`) | 0.56–0.58 with the quad, 0.24–0.26 without |

None of them is a no-op in these frames, and each already does nothing
when its own input is empty: the native particles' pass is not begun
without a quad (`draw_native_particles`; the lamps scene has no particles
row), the GPU smoke's half-size targets and composite exist only with the
detailed smoke on (`gpu_particles::Detail`, off in Original), the glow pass
returns without a sprite, the lens drops, rain tint and grade run only with
their input. The rest work on the whole image every frame by the native
design: the bleach bypass changes every pixel of a game frame (the game's
frame has a bleach amount, a desaturation and a tint, `Frame::GAME` / the
world's tint maps; none is the identity), the decay and A · factor
change the streak buffer whenever it holds anything, and the output pass
is the frame. The free-roam bench starts with a burnout (the dumped frames
show the smoke behind the car), so its 1.5 ms particles row is real
drawing; it costs that much because the smoke quads are large and drawn
into the 8x multisampled scene.

Left as it is: the multisampled copy of the streak buffer (0.30–0.35 ms)
is made before the glow callback, also in a frame whose glow list is
empty, where it is drawn over nothing and dropped. Skipping it would be
exact, but such frames are rare (the streaks need the Light Glow switch,
and the street lamps are glows), and the chain would need to ask the
caller for the list's size first. The bench's ui row is 0.3 ms high at
1080p on the integrated GPU against a windowed run, by the alpha quad above.

Before and after this change on both GPUs (`passes-base-new.txt`, 3
interleaved runs, 1920×1080 Original, GPU ms per frame, median (min–max)):

| scene | integrated base | integrated new | desktop base | desktop new |
|---|---|---|---|---|
| free roam | 10.31 (10.09–11.31) | 10.26 (9.85–10.62) | 0.545 | 0.545 |
| race start | 14.50 (14.13–14.91) | 14.61 (14.32–14.72) | 0.705 | 0.706 |
| rain | 13.43 (13.38–13.49) | 13.37 (13.24–13.56) | 0.672 | 0.671 |
| lamps | 11.35 (10.97–11.61) | 11.31 (11.17–11.62) | 0.592 | 0.592 |

Nothing in a frame's passes changed (the moves are within the runs'
scatter).

## The candidates

* **Car draw index in immediates** (docs/perf-car-draw-index.md), against
  `NFSU2_GPU_DROP=immediates`, 5 runs each
  (`car-draw-index.txt`): the car pass at Original's 8x multisampling is
  fill-bound, −0.02..−0.06 ms; with Enhanced Paint (Balanced, race start
  1080p) 2.93 → 2.26 ms (−0.68 ms, −23 % of the pass). Whole-frame GPU
  moves within its scatter (±0.5–1 ms on the integrated GPU, which shares
  memory with the loaded CPU). Desktop GPU: no change (car pass < 0.1 ms).
* **Overbright C and A in one pass (tried, not kept).** The post passes'
  self-check (`post/fused.rs`) fuses the bleach bypass and the overbright
  pass together or not at all; on both GPUs here the fused bleach bypass
  differs (1.6 M of 2 × 2^24 texels, up to ±3: with `NFSU2_CHECK_STAGES`
  the first 8-bit hand-off already differs by ±1, so `q` cannot stand in
  for it on these devices) while the overbright's C and A are equal. Using
  the fused overbright alone (exact by the same self-check, frames AE 0)
  measured **slower** on the integrated GPU: overbright 1.00 → 1.13 ms
  (free roam 1080p), 1.03 → 1.25 (race start), 1.06 → 1.30 (lamps); the
  two-target pass with `q`'s lookups costs more there than the separate
  one-tap pass it saves (`overbright-fused-igpu.txt`). Not applied. The
  same reason makes a fused or one-dispatch glow / bleach a doubtful win on
  a 2-CU GPU: the `q` emulation of each 8-bit hand-off (a LUT sample and a
  bound lookup per channel) is ALU and texture work the separate passes do
  in fixed function.
* **Rain tint.** `weather::tint_multiplier` gives 2·k/255 per channel,
  never exactly 1, so the pass always changes the image: nothing to skip.
* **Streak buffer, lens drops, nitrous blur** (all on ticks,
  docs/uncapped-fps.md): their cost on the integrated GPU is in the table
  above (streaks 1.4–1.5 ms at 1080p, 0.6–0.7 at 1366×768; lens drops 1.0 /
  0.6 ms while it rains). The governor's choice there: below.

## Rain on four and two cores

The spray's per-element steps (`render/enh/weather/par.rs`) run only with
Wet Road Interaction on (its tier > 0), which no preset turns on for an
integrated GPU (`Preset::apply_for`: Ultra and Extreme set it to 0 there).
Measured with High + Wet Road Interaction 1 / 2, rain, 1366×768, 60 Hz
schedule, job split (default) against `NFSU2_WEATHER_HELPERS=0` (serial),
5 runs each (`rain-cores.txt`):

| GPU, cores | Wet Road Interaction | frame p50 split / serial | main CPU split / serial | render CPU split / serial |
|---|---|---|---|---|
| desktop, 4 | 1 | 2.60 / 2.83 | 2.26 / 2.43 | 2.08 / 1.97 |
| desktop, 4 | 2 | 3.77 / 3.78 | 2.63 / 2.77 | 2.26 / 1.98 |
| desktop, 2 | 1 | 2.55 / 2.68 | 2.16 / 2.36 | 1.67 / 1.83 |
| desktop, 2 | 2 | 2.54 / 2.72 | 2.51 / 2.66 | 1.90 / 1.72 |
| integrated, 4 | 1 | 37.0 / 36.3 | 3.71 / 3.96 | 2.84 / 2.58 |
| integrated, 4 | 2 | 40.6 / 40.1 | 3.91 / 4.29 | 2.94 / 2.75 |
| integrated, 2 | 1 | 37.6 / 37.2 | 3.76 / 3.44 | 2.48 / 2.23 |
| integrated, 2 | 2 | 39.6 / 41.2 | 3.61 / 3.89 | 2.40 / 2.33 |

The split moves 0.05–0.4 ms from the main thread to the workers; frame
time p50 is the same or better on 4 and on 2 cores (p99 scatters by 1–3 ms
either way, inside the runs' own spread). So it is no net loss on few
cores and the job count stays as it is (with one worker, on two cores, the
extra jobs find the call closed and return). `NFSU2_WEATHER_HELPERS=n`
(0 = serial) is the knob for such comparisons; the values are the same
either way (`par::tests::serial_and_parallel_give_the_same_values`, and
the spray digest of the rate tests). On the integrated GPU Wet Road
Interaction itself costs about +15 ms at 1366×768 (rain High 21.3 ms →
36–39 ms), which is why the integrated presets leave it off.

## The streak buffer's governor on the integrated GPU

`NFSU2_STREAK_FORM=auto` (the window's rule fed by the bench's frames),
against `cheap` and `exact`, at a 75 and a 144 Hz schedule, frames back to
back (`streak-governor.txt`, 3 runs each). Predicted probe load = the cost
of one scene render up to the commit / the tick period (16.7 ms).

| configuration | cost of a render | 75 Hz | 144 Hz |
|---|---|---|---|
| free roam Original 1920×1080 | 11.4–12.4 ms | load 68–73 %: cheap after ~20 frames | load 70–75 %: cheap after ~25 frames |
| free roam Balanced 1366×768 | 5.9–6.8 ms | 35–37 %: cheap after 100–130 frames | 38–41 %: cheap after 40–80 frames |
| free roam High 1366×768 | 11.6–13.2 ms | 70–79 %: cheap after ~18 frames | 71–73 %: cheap after ~22 frames |
| free roam Original 1366×768 | 4.8–6.1 ms | 33–37 %: cheap after 34 or 330 frames | **28–30 %: exact all run** (200 probes / 600 frames) |
| race start Original 1366×768 | 7.1–8.2 ms | 43–49 %: cheap after ~450 frames | **21–23 %: exact all run** (100 probes) |

At 1080p and at the Enhanced presets the rule goes cheap within a fraction
of a second: the right call. At 1366×768 Original on a 144 Hz schedule it
stays exact, and that costs: frame time p50 4.73 → 5.06 ms and p99 7.5 →
11.9 ms (free roam), p99 13.4 → 18.1 ms (race start) against the cheap
form. The predicted load counts the probes alone against the tick period;
on a small GPU the drawn frames already take most of it (4.7 ms of a
6.9 ms frame at 144 Hz), so 21–30 % more is over the budget although it is
under the rule's 30 % threshold.

**The rule now counts the frames' own share** (`render::streak::Governor`,
docs/uncapped-fps.md "Which form"): the drawn frames take one render per
frame shown, c / the display's period (the monitor's refresh with or
without vsync, or the frame cap's when longer; in the bench the
`NFSU2_BENCH_HZ` schedule), measured from the same cost c. Probes plus
frames over 90 % is cheap, exact again only under 75 %; the probe load's
own 30 / 18 % rule stays. With the period unknown (uncapped, no refresh
read) only the probe load counts, so the synthetic tests' decisions are
unchanged; `frames_filling_the_refresh_go_cheap` is this case. Measured
at a 144 Hz schedule, 600 frames, 3 runs each (`streak-governor-after.txt`):

| configuration | before: exact frames, frame p50 / p99 | after: exact frames, probes + frames | after: frame p50 / p99 | cheap forced: p50 / p99 |
|---|---|---|---|---|
| integrated, free roam Original 1366×768 | 599 of 599; 4.86 / 10.33 | 44–269 (cheap after 0.2–1.3 s); 101–111 % | 4.94 / 9.27 | 4.84 / 5.54 |
| integrated, race start Original 1366×768 | 599 of 599; 7.29 / 15.80 | 31–35; 125–133 % | 7.21 / 10.99 | 7.25 / 7.76 |
| integrated, free roam Original 1920×1080 | 24–26 (cheap by the probe load) | 24–27 | — | — |
| integrated, free roam Balanced 1366×768 | 118–161 | 41–42 | — | — |
| desktop, free roam Original 1366×768 / 1920×1080, race start 1366×768 | 599 of 599 | 599 of 599 (probes + frames 8–12 %) | GPU and frame times unchanged | — |

A 600-frame run's p99 is its 6th-worst frame, so the exact frames before
the switch (with their probes) still set it; after the switch the frames
are the cheap form's. On a 60 Hz panel with vsync, which most such laptops
have, no probe is drawn at all.

## The falling rain's drops under the lamps

"graphics: native particles" in rain at Medium and High was 0.60–0.70 ms here (an earlier 0.94–1.24 included what
ran before it): nine tenths the lamp shading of the falling rain's drops. Left before the shading where their alpha
is 0 (exact: 0.60 → 0.52 Medium, 0.68 → 0.64 High), then lit at their four corners: 0.05 (probe) + 0.27 Medium,
0.05 + 0.24 High (docs/lamps-shadows.md "The falling rain under the lamps").

## The laptop

GPU time is what bounds the frame on the integrated GPU (CPU ≤ 2.5 ms
main, ≤ 3 ms render workers on four pinned cores; Zen+ at 15 W is about
2.5× slower per thread by the triage's scaling, still under the GPU time).
Frame rates on the reference integrated GPU at World Detail 5 (1000 / GPU ms, the range over
the four scenes); a Vega 8 laptop is within about ±30 % of these
(estimate). This work changes none of them (nothing that moves the GPU
time was kept), so before = after. At World Detail 3, what a fresh install picks on this class,
Original is 4.0–5.8 ms at 1920×1080 (about 170–250 fps) and 2.3–3.8 ms at 1366×768 ("The Enhanced levels
at World Detail 3" below):

| preset | 1366×768 | 1920×1080 |
|---|---|---|
| Original | 138–217 fps (4.6–7.2 ms) | 67–94 fps (10.6–14.8 ms) |
| Balanced | 118–170 fps (5.9–8.5 ms) | 59–83 fps (12.1–17.0 ms) |
| High (integrated ladder) | 47–87 fps (11.5–21.3 ms) | 21–38 fps (26.4–46.9 ms) |

Settings for such a laptop (World Detail 5): Original or Balanced at the panel's native
1920×1080 keep 60 fps in every measured scene (Balanced's race start and
rain sit at 59–65 fps, so with a 60 Hz panel and vsync Original is the safe
choice for racing); High only at 1366×768, where it holds 47–87 fps, its
largest row being Screen-Space Ambient Occlusion (3.6–11 ms at 1080p, 2–4 ms
at 1366×768): turning that row off is the first step down if High stutters.
Leave Wet Road Interaction off (+15 ms in rain at 1366×768).

## The Enhanced levels, pass by pass

### The Enhanced levels at World Detail 3

What an integrated GPU's player runs on a fresh install (World Detail 3, "World Detail" above: no MSAA,
trilinear filtering, no bleach bypass, no rain splatter). `docs/evidence/perf-integrated-gpu/scripts/passes.sh`
with `NFSU2_BENCH_DETAIL=3`, `--bench <scene> 200`, 2 runs per configuration, the levels as
`src/qol/levels.rs` writes them, this tree (with the two exact changes below), every series under the
integrated GPU's measuring lock. Full pass lists: `docs/evidence/perf-integrated-gpu/levels-passes-detail3.txt`.
**State and load**: the machine was building and gating other work the whole time (load average 18–42), and
the integrated GPU shares its memory with that CPU: memory-bound passes then run up to 3× slower than on a
player's otherwise idle machine (the "slow state"; its marker is the SSAO occlusion row, about 1.0 ms at
1920×1080 in the fast state). Every 1920×1080 Medium and High run here was in the slow state or between (the
occlusion row 1.8–3.8 ms; a re-measure of those 16 configurations at load 24–32 gave the same, 1.8–3.8 ms), the
1366×768 ones mostly not (0.6–1.7 ms). So the Medium and High figures at 1920×1080 are **upper bounds** for a
player's machine by up to a few milliseconds (the occlusion, the depth pre-pass, the post passes); the main
view's lamp work, which is bound by its arithmetic, is not affected. Original and Low have no occlusion row to
tell the state by. GPU busy median, ms (explore / racing / race start / rain):

| level | integrated 1366×768 | integrated 1920×1080 | frames per second in the heaviest scene, 1366×768 / 1920×1080 |
|---|---|---|---|
| Original | 2.3 / 3.6 / 3.1 / 3.8 | 4.0 / 5.7 / 5.5 / 5.8 | ~260 / ~170 |
| Low | 2.8 / 3.7 / 3.6 / 4.4 | 4.6 / 6.9 / 6.5 / 7.7 | ~230 / ~130 |
| Medium | 8.8 / 13.0 / 12.8 / 14.6 | 17.5 / 24.5 / 25.5 / 28.4 | ~68 / ~35 |
| High | 19.6 / 25.4 / 25.8 / 37.0 | 37.5 / 46.4 / 47.6 / 67.7 | ~27 / ~15 |

The runs of a configuration were within 0.1–2.3 ms of each other (Medium 16.3–29.0, High 37.4–68.3 at
1920×1080 over the four scenes). The largest rows in the heaviest scene at 1920×1080 (ms): Original (rain
5.8) main view 2.1, half-size overbright 1.1, streaks 0.6, ui 0.5; Low (rain 7.7) main view 2.4, cars 1.5,
overbright 1.1; Medium (rain 28.4) main view 12.3, SSAO occlusion 3.0, cars 1.8, SSAO apply 1.6, high-precision
overbright 1.6, weather 1.2; High (rain 67.7) main view 15.5, high-precision overbright with the full glow 9.9,
volume fx march 7.6, SSR mask 5.3, SSR trace 4.9. So Original and Low hold well over 60 fps on this class at
1920×1080 and Medium is at most 25–28 ms: what stands between Medium and 60 fps there is at most 8–12 ms, nearly
all of it the main view's lamp work and the occlusion, not 27–35 ms as the World Detail 5 numbers suggested.

### Everything raised (World Detail 5)

Measured on the tree as it was then (no change to the frame came out of that
work: every candidate below was tried, matched the base byte for byte and
gave no gain beyond the runs' noise, so it was left out). `--bench <scene>
200` (integrated) / `300` (desktop), Anti-Aliasing Native (the scene at
8x), World Detail 5's Customize Display values (the bench's default), rain = drive at clock 162; the
levels as `src/qol/levels.rs` writes them; 2 runs per configuration, arms
interleaved, every series under the machine's measuring lock. Full pass
lists: `docs/evidence/perf-integrated-gpu/levels-passes.txt` (integrated),
`docs/evidence/perf-budget/levels-passes-desktop.txt` (desktop); scripts
`docs/evidence/perf-integrated-gpu/scripts/passes.sh`, `levels.sh`,
`pass.py`. Two identical arms differ by up to ±1.2 ms of whole-frame GPU
time on the integrated GPU (it shares memory and power with a busy CPU);
the pass rows move ±0.3 ms.

GPU busy median, ms (explore / racing / race start / rain):

| level | integrated 1366×768 | integrated 1920×1080 | desktop 1920×1080 | desktop 3840×2160 |
|---|---|---|---|---|
| Original | 5.1 / 7.0 / 7.7 / 7.2 | 10.2 / 14.4 / 14.5 / 13.5 | 0.52 / 0.69 / 0.72 / 0.68 | 2.05 / 2.45 / 2.63 / 2.49 |
| Low | 5.4 / 7.4 / 9.0 / 9.0 | 11.2 / 16.0 / 16.0 / 16.3 | 0.53 / 0.73 / 0.77 / 0.74 | 2.07 / 2.48 / 2.67 / 2.64 |
| Medium | 12.9 / 18.6 / 21.8 / 20.7 | 30.4 / 44.1 / 44.0 / 51.2 | 1.10 / 1.74 / 1.65 / 1.78 | 5.52 / 6.58 / 6.61 / 7.13 |
| High | 26.7 / 34.8 / 37.3 / 49.9 | 52.9 / 68.4 / 72.1 / 99.5 | 2.02 / 2.79 / 2.71 / 3.35 | 8.41 / 9.73 / 9.99 / 11.54 |

The five largest rows per level (heaviest scene; integrated 1920×1080, ms):

| level | rows |
|---|---|
| Original (race start 14.5) | main view 6.4, bleach bypass 1.7, cars 1.4, streaks 1.4, overbright 1.1 |
| Low (rain 16.3) | main view 5.7, cars 2.8, streaks 1.4, bleach bypass 1.1, overbright 1.0 |
| Medium (rain 51.2) | main view 20.5, SSAO occlusion 4.0, cars 3.3, depth pre-pass 3.3, atmosphere 3.2 (+ SSAO depth 2.7, weather 2.4, SSAO apply 2.3) |
| High (rain 99.5) | main view 26.1, volume fx march 12.2, overbright (high precision, full glow) 10.4, SSR mask 6.1, SSR trace 5.2 |

Desktop 3840×2160, the same order: Original main view 1.0, bleach bypass
0.5, streaks 0.5; Medium main view 1.8, streaks 1.4, atmosphere 1.0, SSAO
apply 0.6; High main view 2.2, SSR copy and depth pyramid 1.3, streaks
1.3, high-precision overbright 1.0, enhanced glass 0.9.

**What scales with what** (integrated 1920×1080 against 1366×768, 1.98×
the pixels): the full-screen passes scale with the pixels (bleach bypass
1.75 / 1.0, SSAO occlusion 3.4 / 1.2, atmosphere 2.5–3.2 / 1.2–1.4, depth
pre-pass 2.5–4.2 / 1.0–2.2, High's full glow 10.4 / 4.6); the main view
1.9–2.6× (fill and its per-pixel lights); constant: the environment cube
(0.2 Original, 0.6–0.7 Medium), the lamp-shadow layers (0.1–0.3), the UI
(0.4–0.6), the road reflection. **With the lights**: the main view's lamp
work (Medium, racing 1920×1080, one row down at a time: Dynamic Lamps
−9.6 ms, Lamp Shadows −7.8, Contact Shadows −2.7) and the atmosphere's
inject; **with the cars**: the car pass, the lamp shadows' car term and,
at the race start, the crowd (drawn before the occlusion with SSAO on,
inside the "ssao depth" row: 6.1 ms there against 2.6 racing). At
1366×768 a "(between submissions)" row of 2.3–4.5 ms appears in some
runs (time the GPU waited for the next command buffer).

**Advanced rows** (racing, on Medium unless named; integrated 1920×1080 /
desktop 1920×1080 / desktop 3840×2160, ms): Medium 39.9 / 1.75 / 6.60;
TAA 35.4 / 1.27 / 3.67; Upscaling Quality 22.3 / 1.06 / 2.68; Performance
18.6 / 0.99 / 2.32; FSR 1 Quality 25.1 / 1.20 / 3.73; FSR 2 Quality 35.2 /
1.39 / 4.31; MSAA 4x 43.3 / 1.49 / 5.02; SSAA 2x 55.9 / 1.71 / 6.02, 4x —
/ 2.89 / 11.26, 8x — / 6.12 / 23.38, 9x — / 6.71 / 25.59, 16x — / 12.11 /
47.19; Ray Tracing 83.6 / 3.76 / 12.83; Low 15.3, Low + Performance 14.6
(integrated). On the integrated GPU the temporal resolve is the largest
pass with TAA or Upscaling (6.4–10 ms at 1920×1080, whatever the input
size: 8.2 ms for Low + Performance), 50–60× its desktop time where the
other full-screen passes are 15–30×; FSR 2's accumulate is 10 ms there.

### Medium on the integrated GPU with everything raised: what stands between 44–51 ms and 16.7

World Detail 5 (8× MSAA and the bleach bypass on top of Medium's rows; at World Detail 3 Medium is 25–28 ms,
above). Racing 1920×1080, 41–44 ms. Medium less one row at a time
(`levels-passes.txt`, last block): Dynamic Lamps −9.6 (with their
shadows), Lamp Shadows Low −7.8, SSAO −7.8 (occlusion 3.8, depth 2.6,
apply 2.2, blur 0.4; the scene depth's first read moves 1.5 ms of it to
the next pass that samples the depth), Contact Shadows −2.7, Volumetric
Fog Low −2.4, Car Reflection 256 −1.2, High Precision Post −0.7; Light
Probes, Headlight Beams, Wet Roads, Spray: within the noise. The depth
pre-pass (2.1–3.1 ms) pays for itself many times: without it
(`NFSU2_DEPTH_PREPASS=0`) the frame is +8.9 / +12.0 / +9.5 ms (race start
/ racing / rain), the main view 14–18 → 26–32 ms. With all those rows off the frame
would still be ~20 ms (Low's 16 plus the pre-pass and the rest), so no
cheaper implementation of the same picture brings Medium to 60 fps at
1920×1080 on this GPU; at
1366×768 it is 13–22 ms (46–78 fps), and Medium with Upscaling
Performance at 1920×1080 18.6–19 ms (~53 fps).

Where Lamp Shadows Low's main-view cost goes (measuring constants in a
test build, `candidates.txt` E): the cars' soft-ellipsoid term inside the
lamp loop 2.9–3.3 ms, the lamps' shadow-map lookups 1.3–1.6, the beams'
shadows ~1.1, the headlight layer's visibility 0.3–0.4; all four 5.7. The
temporal resolve (F): its five-sample cross is ~5 ms of 10 (one sample
only: −5.1 / −5.4 ms), history 0.7–1.0, the car-motion loop 1.4–2.4 at
Performance.

**Tried for the same picture, not kept** (each byte-identical to the base
on the bench frames, A/B in `candidates.txt`): skipping the SSAO multiply
where its factor is exactly 1 (sky, unoccluded: +0.1–0.2 ms, the blend
was not what costs; too few pixels qualify at exactly 1, the blur leaves
unoccluded texels one 16-bit step under it: at that step the skip pays,
"Ambient Occlusion: the same occlusion for less" below); a lamp's shadow
skipped where the lamp adds nothing,
the cars within 12 m found once per pixel, a world-space bound before the
ellipsoid test, the nearest car's box-space position hoisted out of the
lamp loop (all within ±1 ms of the base in whole-frame time, the main
view ±0.4); the resolve's five depths from two gathers instead of five
loads (±0.9, no gain). The car term's cost is not its arithmetic per
lamp and car (a bound that skips most of it changed nothing), so it is
inferred to be the loop's presence in the shader (80 VGPRs, 12 waves per
SIMD and 25 KB of code against 72 / 14 / 15 KB without Lamp Shadows,
`RADV_DEBUG=shaderstats`); a profile at the instruction level is open.
That inference did not hold: the per-pixel bounds still tested every car
per lamp. Culling the cars per light cluster on the CPU (a bit mask beside
each entry of a cluster's lamp list, and per cluster for the headlight and
the contact shadow; docs/lamps-shadows.md "Per-pixel work") takes the
car tests from 13.9 to 0.2 per pixel and the main view (World Detail 3,
Medium, racing 1920×1080) from 11.2 to 8.3 ms; with the lamp lists cut to
the clusters a lamp reaches, the lamp loop's exact early outs and the
beams' cheap first test, 7.5 ms (rain 12.5 → 8.5, race start 9.4 → 7.9,
the casino 6.5 → 4.7, High racing 13.1 → 8.8), every frame byte-identical
(`docs/evidence/perf-budget/lamp-culling.txt`).

### The temporal resolve

Where its 6.4–10 ms go (integrated GPU, racing 1920×1080, Medium + TAA and
Medium + Upscaling Performance; measuring builds that take one part out
at a time, 5 interleaved pairs each, the pass's GPU time, median and the
pairs' range; `temporal-resolve.txt`). The fragment resolve is one wave64
pixel shader of 832 instructions (669 VALU, 15 texture), 56 VGPRs, 18
waves per SIMD, no scratch (`RADV_DEBUG=shaderstats`), so it is bound by
its arithmetic, not by memory:

| taken out (measuring build) | TAA 7.9 ms | Performance 6.4–6.8 ms |
|---|---|---|
| everything but one load (the pass's fixed cost: its barriers, the target's writes) | −7.4 (left 0.44) | −5.9 (left 0.55) |
| the car-motion loop | −1.9 (−1.3..−2.8) | −1.9 (−1.4..−2.1) |
| the cross's four side colours (load, tone map, YCoCg) | −1.8 (−1.2..−2.1) | −1.5 (−1.1..−1.7) |
| the history's Catmull-Rom (one load instead) | −1.5 (−0.2..−2.0) | −1.0 (−0.3..−1.5) |
| the cross's statistics (moments, min / max) | −1.2 (−1.1..−3.1) | −1.1 (−1.0..−1.6) |
| the tone map per sample | −0.9 (−1.6..+0.5) | −0.7 (−1.3..−0.3) |
| YCoCg per sample; the box clip | ±0.1–0.6 (noise) | ±0.1–0.6 |
| the cross's four side depths | +0.9 (slower: another closest sample) | +0.1 |

Measured, each against the base in its own interleaved series:

* **The same shader as a compute pass** (8×8 workgroups, the history
  written through a storage texture, the copy to the scene unchanged):
  TAA −1.5 / −2.2 ms (two series; pairs −0.5..−2.6), Performance (resolve
  into the history plus a copy to the scene instead of the two-target
  pass) +0.1 (−0.5..+0.8), Low + Performance −1.0 (−0.5..−1.4). Why it
  is cheaper is inferred, not profiled (no export, compute waves of 32).
* **Plus the car loop skipped per tile** (each 8×8 tile leaves out the
  cars whose screen rectangle it does not touch; docs/temporal.md "The
  resolve as a compute pass"): TAA −3.0 / −2.6 ms (pairs −1.3..−3.6),
  Medium + Performance −0.6 / −1.2 (pairs −0.5..−1.5), Low + Performance
  −1.6 (−1.3..−2.2), Medium + Quality at the race start +0.2 (−0.1..+0.6).
* **The cross from shared memory** (each workgroup loads, tone-maps and
  converts its input texels once into workgroup memory, the five samples
  read from there): **slower**, TAA +5.5 ms, Performance +3.3 (with the
  car loop out as well still +0.8 / +0.6). Its best case was the side
  colours' 1.5–1.8 ms above; the region bookkeeping and the reads from
  workgroup memory cost more on this GPU. Not kept; a pre-pass target of
  converted colours would save at most the tone map's 0.7–0.9 ms for a
  full-screen Rgba32Float write and read (+16 MB at 1080p), so it was not
  built.
* **The car loop gated per pixel in the fragment pass** (no loop outside
  the cars' rectangles' union): no gain (TAA +0.2, Performance −0.1).

**Kept**: TAA's resolve as the compute pass with the per-tile car test,
TAA only (Upscaling keeps the fragment pass: it does not meet a gain of
more than 1 ms in every pair there). Final A/B (`temporal-resolve.txt`,
the base binary against the new one, 5 interleaved pairs, ms):

| integrated 1920×1080, TAA | pass base → new (pairs) | frame GPU base → new (pairs) |
|---|---|---|
| racing Low | 9.17 → 6.16, −2.7 (−2.3..−3.6) | 18.0 → 14.9, −3.1 (−2.6..−3.3) |
| racing Medium | 8.68 → 5.79, −2.9 (−2.2..−3.9) | 36.1 → 32.8, −2.9 (−0.6..−5.2) |
| racing High | 8.09 → 5.61, −2.5 (−1.7..−2.8) | 56.4 → 54.3, −2.2 (−1.3..−6.2) |
| race start Low | 7.42 → 5.64, −1.7 (−1.2..−2.3) | 15.6 → 13.7, −1.8 (−1.4..−2.3) |
| race start Medium | 6.54 → 5.55, −1.0 (−0.6..−1.4) | 35.3 → 33.9, −1.1 (−0.8..−2.9) |
| race start High | 6.26 → 5.41, −0.8 (−0.2..−1.1) | 55.7 → 55.8, +0.0 (−0.8..+0.9) |

On the desktop GPU (compute pass asked for, 5 pairs) the pass moved by
−0.011 ms at 1920×1080 (Low, Medium, High racing; pairs within 0.002) and
at 3840×2160 racing Medium, but +0.030 at a 3840×2160 race start on High
(pairs +0.029..+0.030; the plain compute pass without the tile test was
+0.036 at 3840×2160): so the compute pass is chosen on integrated GPUs
only and discrete GPUs keep the fragment pass. On the integrated GPU the
race start on Medium and High stays under the 1 ms bar in some pairs
(−0.2..−1.4), never slower.

Every bench frame of TAA and of each Upscaling tier on Low, Medium and
High (racing, race start) is byte-identical to the base's on both GPUs
(the compute pass forced on the desktop GPU), and so is the history
itself (raw dumps of the measuring builds on the desktop GPU, TAA and
Performance on Medium, Performance on Low).

### The cluster loop

The lamps' light-cluster loop in its subgroup form (docs/lamps-shadows.md "The cluster loop": one cluster
per subgroup at a time, its lamp list read once per subgroup; the same sums, so the same pixels). Integrated
GPU, 1920×1080, World Detail 3, `--bench <scene> 200`, the base against the new binary, 7 interleaved runs
each, every series under the measuring lock (load average 7–24 and 22–38 in the two series; the arms
alternate run by run inside one lock hold, so a pair is two runs seconds apart; the state above was mixed,
the occlusion row 1.3–3.9 ms, and the main view, bound by its arithmetic, moves little with it); the main view's
GPU ms, each arm's median (min–max), and the paired difference (median, min..max;
`docs/evidence/perf-budget/igpu-cluster-loop.txt`):

| scene, level | base | new | paired |
|---|---|---|---|
| racing, Medium (series 1) | 11.32 (11.00–11.78) | 10.82 (10.35–11.04) | −0.34 (−1.42..−0.13) |
| racing, Medium (series 2) | 11.47 (10.64–11.90) | 10.66 (10.44–10.99) | −0.80 (−1.08..+0.33) |
| racing, High | 11.96 (11.79–12.10) | 11.35 (11.12–12.94) | −0.52 (−0.98..+0.85) |
| race start, Medium | 9.32 (9.09–9.51) | 9.13 (8.86–9.46) | −0.17 (−0.59..+0.14) |
| rain, Medium (series 1) | 12.21 (11.99–12.56) | 12.22 (12.00–12.69) | +0.01 (−0.18..+0.28) |
| rain, Medium (series 2) | 12.46 (12.20–14.41) | 12.21 (11.98–13.26) | −0.30 (−2.16..+1.06) |

So a third to three quarters of a millisecond of the main view in racing (all seven pairs of the first series
faster), less at the race start, none to be shown in rain. The whole frame moves inside its ±1–2 ms run
spread. A second shape of the round loop (a flag instead of the return) measured the same (+0.02 / +0.09 ms
against this one). On the desktop GPU the main view is under 1 ms; nothing to see there. Frames: byte-identical to the
base's on the desktop GPU (Medium and High racing, race start and rain; Medium with 8× MSAA; the player's
settings) and on the integrated GPU (Medium racing and race start, High racing), with the occlusion's
rectangles below in the same binary.

### The occlusion apply's car loop

Ambient Occlusion's apply pass leaves out each car box whose screen rectangle a pixel is outside, and the whole
loop outside their union (docs/glow-darkening.md: the rectangle bounds the box's own region, outside which its
terms are exactly 0; the same pixels). Integrated GPU, 1920×1080, World Detail 3, the tree before against the
new one, 7 interleaved runs each, under the measuring lock (load average 24–41, the slow state or between: the
occlusion row 1.3–3.8 ms; the apply reads memory, so its fast-state times and gain are likely smaller); the
pass's GPU ms (`docs/evidence/perf-budget/igpu-glow-rects.txt`):

| scene, level | before | after | paired |
|---|---|---|---|
| race start, Medium | 1.75 (1.45–1.80) | 1.33 (1.27–1.60) | −0.36 (−0.53..−0.03) |
| racing, Medium | 1.97 (1.81–2.09) | 1.97 (1.73–2.22) | −0.05 (−0.31..+0.25) |
| racing, High | 2.25 (1.98–2.40) | 2.12 (1.88–2.23) | −0.12 (−0.45..+0.20) |

At the race start (the grid's cars side by side, the rest of the screen far from them) about a fifth of the
pass; while racing the cars' rectangles together cover most of the screen and the gain is inside the noise. A
first form whose rectangles bounded the loop's 4 m reach was slower (+0.17 / +0.42 ms racing): the player's car,
5–7 m ahead of the chase camera, then reached the whole screen, and the tests cost more than they saved. The
loop removed altogether (an earlier measuring build at World Detail 3, not the same picture, two runs) gave
2.0 → 1.2 ms racing: what is left of that is spent at the pixels the boxes do reach.

### Ambient Occlusion: the same occlusion for less

The four passes (`render/ssao.wgsl`) keep their estimator: twelve taps on
a spiral turned per pixel by interleaved gradient noise at half size,
elevation over the pixel's plane less a bias, a 4 × 4 depth-weighted blur,
a 2 × 2 depth-weighted upsample multiplied into the scene. What changed
is how it is computed (each a step of its own, measured with an
interleaved A/B of the passes alone on frames taken from the game, both
GPUs; `docs/evidence/perf-budget/ssao-algorithms.md`):

1. **The taps' loads go out together.** A sky tap was skipped with a
   `continue`: every tap's load sat behind a branch and waited for the
   tap before it (17 loads in 14 separate batches, `RADV_DEBUG=shaders`).
   Chosen away instead (its share times nothing), the twelve loads are
   one batch. Same arithmetic: the occlusion is identical (integrated GPU,
   four frames; on the discrete GPU 2 of 16.6 M texels at 4K differ by one
   16-bit step, the compiler's order of the sum). Occlusion −4 % (integrated,
   fast memory state) to −11 % (slow), −12 to −25 % on the discrete GPU.
2. **Only pixels with something to darken are blended.** The blur leaves
   an unoccluded texel at 1 or at 1 − 2⁻¹¹ (its sum of ones over the sum of
   their weights comes out a hair under 1, and the 16-bit target rounds
   toward zero); the apply multiplied about 40 % of a frame by that step.
   It now skips the sky and every pixel whose four texels are both of
   those values: they keep their colour (at most one 16-bit step brighter
   than before; an 8-bit scene exactly as before). Apply −39 to −58 %:
   without the multiply those pixels also skip the car-glow loop.
3. **The occlusion's arithmetic.** One table of the spiral turned once per
   pixel (one sine and cosine, not twelve of each), the tap's sample point
   and view position one multiply-add each from the pixel's own (on the
   2:1 grid, where texel t's place is a linear A t + B; under SSAA, where
   the occlusion has the screen's half and its depth texels are picked
   from the scene's (docs/ssaa-screen-passes.md), each tap takes its
   picked texel's own place instead: a linear map would beat against the
   picks; one uniform branch per pixel chooses the loop, the 2:1 loop's
   registers and instructions are those it has alone), the
   centre and its four neighbours from two gathers: 697 → 470 ALU
   instructions per pixel (this integrated GPU's compiler). Occlusion −12 to −16 % on the
   integrated GPU in its fast memory state, no gain in the slow one; on the
   discrete GPU, where the pass waits on memory, +0.01–0.02 ms at 4K. Up to a few hundred pixels per frame
   take a tap one texel over (a rounding edge of the turned offset).
4. **The blur's early way out.** A texel whose sixteen neighbours are all
   unoccluded is 1 without the sixteen weights: blur −21 to −30 %.

Integrated GPU, Medium, 1920×1080, the passes alone (ms, median of 40,
interleaved; racing frames at the start line and under way, night rain):

| | depth | occlusion | blur | apply | all four |
|---|---|---|---|---|---|
| before | 0.19 | 1.25–1.75 | 0.39–0.40 | 1.75–1.90 | 3.70–4.17 |
| steps 1–4 | 0.19 | 1.02–1.33 | 0.28–0.31 | 0.81–1.08 | 2.31–2.96 (−28 to −39 %) |

On the base with the glow rectangles and the screen-half occlusion under
SSAA: every row up with ray tracing at 3840×2160 (SSAA 2×) 0.70 → 0.53 ms
(occlusion 0.18 → 0.15, apply 0.34 → 0.21; n 3, racing and rain);
integrated Medium 1080p racing 5.26 → 4.20, rain 5.25 → 4.47 (n 5–6). Rain
frame 100 at 4K SSAA 2× (where a linear position map once drew stripes on
the angled brick wall): 5.9 % of the pixels by one or two levels, none by
more than 1 %, the wall's crop 3 % by one level without any column
pattern.

In the game (bench GPU rows, integrated, n 4–8 runs each): Medium racing
4.87 → 3.80 ms (frame 25.2 → 23.9), rain 5.90 → 4.25 (29.4 → 27.8), race
start 5.41 → 4.59; High racing 6.13 → 4.58 (49.1 → 47.3), rain 6.33 →
4.82 (65.0 → 63.1). The occlusion row there is mostly in the slow state below.

In the slow memory state the integrated GPU takes now and then (the
occlusion at 3–3.6 ms for a whole run, the other passes unchanged; not the
allocation (seven of them in one process switch together) and not the core
clock (2200 MHz throughout); inferred: memory contention from the CPU) the
same steps give 6.43 → 4.89 ms (−24 %). Every row up with ray
tracing (discrete GPU, 3840×2160, SSAA 2×): 1.15 → 0.90 ms (−22 %).

Pictures (bench frame dumps, before against after, racing and rain in
motion over 16 consecutive frames, race start, free roam, a 4K racing
frame with every row up): 3.4–6.2 % of the pixels change by one level (largest
2–3 of 255, none by more than 1 % but one race-start crowd pixel), 1 − SSIM ≤ 1.04e-4, mean
luminance +0.01–0.02; the frame-to-frame change in motion is the same to
three decimals.

Measured and not kept (the passes alone, both GPUs; the occlusion's
factor image compared bit by bit):

* the apply drawn only over 16 × 16 tiles the blur marks: the marking's
  atomics cost more than the skipped tiles on the integrated GPU, −0.07 ms
  on the discrete one at 4K; the per-pixel skip instead;
* gathers for the blur and the upsample: two-channel texels, two gathers
  fetch each texel twice; slower;
* occlusion and blur in one compute dispatch, the 35 × 35 occlusion tile
  and the blur in workgroup memory (the 16-bit rounding reproduced: the
  same picture): slower on both GPUs (integrated 2.2–3.7 ms against 1.4 for
  the two passes, discrete 4K 0.61 against 0.40): the blur's apron is 20 %
  more occlusion work. The linearize cannot join it: the taps reach 48
  texels (68 under SSAA), a tile with that apron is 48 KB of workgroup
  memory;
* the apply folded into another pass: the depth pre-pass holds only the
  world (the cars and characters come after the world's colour pass, and
  the occlusion under the cars is the effect's most visible part), and
  every full-screen pass after the apply runs after the additive layers
  the occlusion must not darken (docs/glow-darkening.md); the apply
  without its blend is not cheaper on the integrated GPU (−18 % at 4K on
  the discrete one), so a fold could save little;
* the far taps from a half-size level of the linear depth (SAO's
  rotated-grid pick, from 16 and from 8 texels): no gain on the integrated
  GPU (it computes, it does not miss the cache: the reach capped at 8
  texels gains nothing either); discrete 4K occlusion −26 %, but 2 % of
  the pixels change by more than 1/255 (largest 11 %);
* the linear depth in 16 bits: occlusion −22 %, but depth steps read as
  occlusion: 8 % of the pixels darker by more than 1/255;
* the occlusion as a compute pass (the same picture): steady at 1.4–1.9 ms
  where the fragment pass swings with the machine's memory contention,
  but 0.3 ms slower than it on a quiet integrated GPU, and the same on the
  discrete GPU (with a 64 × 64 super-tile group order);
* packed 16-bit arithmetic for the taps (`enable f16`, positions relative
  to the pixel): occlusion −8 to −10 % (about −0.1 ms) on the integrated
  GPU, 0.3–1.1 % of the occlusion's pixels by one step or so (up to 290
  pixels by more than 1/255 at the race start); not built: it needs the
  16-bit shader feature on every device that would take it and a 32-bit
  fallback, for 3–4 % of the effect.

### Proposals (not built)

* **The cars into the lamp-shadow layers** (drawn as depth into the
  layers they stand in, as the scenery is) instead of the per-pixel,
  per-lamp ellipsoid loop: up to ~3 ms of Medium's main view; a changed
  look (hard depth instead of the soft ellipsoid), so a decision on the look, not a speed-up.
* **The full-screen multiplies into the 8x scene** (SSAO apply 2–3 ms,
  the atmosphere's apply 2.4–3.4 ms on the integrated GPU) are bound by
  blending eight samples per pixel; applying them after the resolve
  would be one pixel instead of eight, but the additive layers drawn in
  between (car shadow texture, particles, glows, horizon fog) would then
  be darkened too: a changed picture.
* **An integrated-GPU Medium** (Medium without Lamp Shadows and SSAO:
  ~28–36 ms at 1920×1080, ~14–18 ms at 1366×768 by the pixel ratio,
  inferred, at World Detail 5) would come near 60 fps at 1366×768 only; it turns two rows off, so it is a
  proposal, not a change of the level. At World Detail 3 Medium itself is 8.8–14.6 ms at 1366×768 and
  17.5–28.4 ms at 1920×1080 (above), so such a level would be measured there first.
* High's full glow (High Precision Post Ultra) is 10.4 ms at 1920×1080 on
  the integrated GPU (1.4 at High Precision Post High); High is not an
  integrated-GPU level in any case. With the blur only where it can be lit
  (docs/hdr-post.md) the glow chain measured 9.5 → 5.3 ms at High and
  1.56 → 1.03 ms at Medium (World Detail 3, racing, the build machine's
  slow state; docs/perf-budget.md "The glow's blur where it can be lit").

### What the machine proposes, per GPU class

The first start is Original everywhere (the native look; resolution and
World Detail from SPEED2.EXE's own first-run table, `game --probe`'s
`first_run.*`). The launcher's "Enhanced" writes Low on every GPU; the
game's Graphics Mode Enhanced starts on `Level::for_class`: Low on an
integrated GPU, Medium elsewhere. Against the tables above (integrated: World Detail 3, a fresh install's;
in brackets World Detail 5, everything raised):

| class, output | Original | Low | Medium | High | proposal |
|---|---|---|---|---|---|
| integrated, 1366×768 | 2.3–3.8 ms [5–8] | 2.8–4.4 [5–9] | 8.8–14.6 [13–22] | 19.6–37 [27–50] | Low (as now) |
| integrated, 1920×1080 | 4.0–5.8 [10–14.5] | 4.6–7.7 [11–16.3] | 17.5–28.4 [30–51] | 37.5–67.7 [53–100] | Low (as now; Medium is 35–57 fps there) |
| desktop (reference), 1920×1080 | 0.5–0.7 | 0.5–0.8 | 1.1–1.8 | 2.0–3.4 | Medium (as now) |
| desktop (reference), 3840×2160 | 2.1–2.6 | 2.1–2.7 | 5.5–7.1 | 8.4–11.5 | Medium (as now) |
| mid class (8–10 GB) | — | — | — | — | Medium (inferred; no such card measured) |

No automatic choice puts an integrated GPU on a level it cannot run: the
first start is Original and both "Enhanced" entries start it on Low, so
the proposal data (`Level::for_class`, `launcher::model::ENHANCED_LEVEL`)
is left as it is. On the reference desktop GPU High would fit 60 fps at
both sizes as well; starting it there is a change of the default
Enhanced look, so it is left as a proposal.

## Fog, volumes and rain: the same pictures for less (integrated GPU)

The reference integrated GPU, 1920 × 1080, World Detail 3, `--bench drive` at clock 162 (rain 0.70), 200 frames,
the passes timed apart by a measuring build (docs/perf-budget.md "Fog, volumes and rain"); median of 5–7 interleaved
runs [range]. Before → after, ms:

| pass | Medium (Volumetric Fog Low) | High (Volumetric Fog High, spray volume, SSR) |
|---|---|---|
| rain tint (folded into the fog's apply: docs/atmosphere.md) | 0.409 [0.399..0.439] → 0 | 0.387 [0.383..0.413] → 0 |
| fog apply | 1.062 → 1.132 (the prototype's run-time selects; the built entry point adds one product) | 1.188 → 1.218 |
| fog inject (light cull) | 0.124 → 0.126 (24 lights: neutral) | rain 0.759 → 0.663, racing 0.772 → 0.455 |
| wet spray resolve | – | 0.268 → 0.182 |
| wet spray march (specialised) | – | 7.11 → 6.63 |

The whole frame moves within its run-to-run spread on this GPU; the pass rows are the measure. With all six
changes (the game without the measuring marks, 7 interleaved runs each): Medium rain 28.66 [27.13..28.97] → 28.05
[26.93..28.59], racing 24.70 → 24.56, free roam 17.46 → 16.82; High rain 70.11 [68.90..73.62] → 70.61
[69.58..72.73]. The spread is the GPU's memory: while the machine measured on builds (all its CPU threads busy), the
memory-heavy passes of the integrated GPU run up to 3× slower (SSAO's occlusion 1.4 → 3.4 ms at Medium, in the same
arm); a player's machine runs in the fast state. Every High run above was in the slow state (occlusion 2.9–3.8 ms),
the Medium runs in both; the arms were interleaved run by run, so both saw the same mix.

What is left, measured (in the slow state): at High the wet spray's march (6.6–7.1 ms: the steps' own work, not the
dense samples' shading — erosion off 7.04, shading off 7.05, 40 steps at most 4.95, march divisor 3 3.19, steps × 2
5.32). Without changing its pictures: the specialised program (above, −0.3 to −0.5), the next step's density
sampled ahead (+0.16: dropped), clear 2³-voxel cells stepped over (−0.08: dropped; the steps are inside the
medium, not in empty space). Its resolution against the volume's detail: the grid's 0.3 m voxels span about 28 of
the march's pixels at 5 m, but the erosion's second octave reaches about 0.1 m (9 pixels), so a coarser march
changes the billows' edges (divisor 3: 0.2 % of the pixels > 1 %, at most 47 levels, at the depth edges); fewer
or longer steps change them too. Those are look changes, not taken.

And the SSR mask in rain (5.3 ms against 1.1 dry: the wet road's term worked out again for the mask). The mask's
term written by the main view instead (docs/wet-roads-ssr.md "The mask from the main view") saves the mask pass
4.6 ms here but costs the main view 4.3, whatever carries the values out (two targets, one, or storage writes with
the early depth test: 20.3, 20.3, 20.4 ms against 16.1; the pass split alone 15.9): the program, not its outputs.
Not taken on this GPU class.
