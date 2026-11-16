# Performance overlay (MangoHud-style)

A clone tool, not
a game feature: nothing here is in SPEED2.EXE, nothing was traced. Default
(the rows Off, F3 not pressed): hidden and nothing runs, frames and audio
are the base binary's. The F3 overlay (agreed deviation) is its Minimal
layout.

## Keys and rows

* **F3** cycles hidden → Minimal → Standard → Full → hidden, anywhere;
  **Shift+F3** hides it. (the quality tooling's Mark Spot uses F9–F12; no other key taken.)
* Enhancements page, after Reduced Flashing (label ids 0xC1_04E0_EC..EF,
  unused in every copy and in main on 2026-11-05):

| row | id | values | options.txt |
|---|---|---|---|
| Performance Overlay | 0xC104E0EC | Off / Minimal / Standard / Full (the layout shown at start) | `overlay.layout 0..3` |
| Overlay Position | 0xC104E0ED | Top Left / Top Right / Bottom Left / Bottom Right | `overlay.position 0..3` |
| Overlay Scale | 0xC104E0EE | 75 / 100 / 125 / 150 / 200 % | `overlay.scale N` |
| Performance Log (CSV) | 0xC104E0EF | Off / On | `overlay.csv 1` |
| (no row) sampling rate | – | 2–10 Hz, default 4 | `overlay.hz N` |

Lines are written only when not the default; Restore Native resets them.
The overlay is not listed among the deviations (it changes no frame the
game draws beneath it). Environment: `NFSU2_OVERLAY=minimal|standard|full`
(start layout; also for `--bench`), `NFSU2_PERF_CSV=path` (log to that
file; also for `--bench`, e.g. from scripts/perf), `NFSU2_SHOT_FPS=
minimal|standard|full` (a `--shot` with the overlay; made-up frame figures,
this machine's metrics; `NFSU2_SHOT_ENH="overlay.position 1;overlay.scale
150"`).

## Layouts

The three layouts are one panel (`overlay/panel.rs`): the same backing
(near-black, half transparent), the same text (0.32 × tt21i), MangoHud's
label colours with white values, the same two columns and spacing, and the
same corner and scale rows. F3 adds rows; the look does not change.

* **Minimal** (the F3 overlay; no sampler): the panel's first rows, the
  frame figures:

  ```
  FPS          60    16.67 ms   1% 58   0.1% 55
  Frame        99%  16.9   max  17.4 ms
  CPU time     update  0.80   draw  4.00   trails  0.30 ms
  Waits        display 11.60   cap  2.50 ms
  GPU time      3.10 ms   busy 19%
  game slowed to 75% speed (frames too long)       (red, only then)
  Deviations   wide aspect, vsync-off mailbox, …
  ```

  "trails" shows only while light-trail renders were drawn in the last
  second, "cap" only while the frame limiter slept. The red line spans
  both columns. The deviations wrap at commas as in Standard (about 64
  characters a line). Every row is one of Standard's and the red line is
  narrower than Standard's frame-time graph (200 canvas pixels), so in the
  same corner and scale Minimal covers a part of what Standard covers
  (`panel.rs` tests; shots below).
* **Standard**: Minimal's rows (`FPS`: FPS, mean frame time, the 1 % and
  0.1 % lows; `Frame`: 99th percentile, longest; `CPU time`, `Waits`, `GPU
  time`; the deviations last), then `Ticks` (ticks per second, the
  simulation's time per tick, the game's speed, the light trails' form;
  red while the game is slowed, in place of Minimal's red line); CPU load / clock / temperature / package
  power / the game's own CPU; GPU load / core and memory clock / edge (hot
  spot) temperature / power; VRAM used / total, GTT, the game's own VRAM;
  RAM used / total, the game's RSS, swap; present mode, frame latency, Low
  Latency, frame cap; the frame-time graph (the last 200 drawn frames,
  scaled to twice the mean or the longest frame, with the mean's line;
  its label names the frames and their longest. When the longest of those
  frames (≥ 100 ms) spent at least half of it waiting for a swap-chain
  image, the label says "(N ms waiting for a display image)": wgpu waits
  at most 1 s for one, so a 1000-odd ms spike with that note is the
  compositor holding the images, e.g. a screenshot UI; the log gets a
  `swap chain: waited …` warning for every acquire over 250 ms).
* **Full**: Standard plus per-core loads, the CPU's average / highest
  clock, the game's GPU share and GTT, the game's I/O (what reached the
  disk, and all reads / writes including the page cache), the disks'
  throughput, the last track load's time (the loading gate's lifetime),
  window size, VRR state (`latency::vrr::probe`, every 2 s), the adapter,
  per-thread CPU (main, `render 0..3`, `perf sampler`, audio, loaders; the
  busiest 8), the GPU time per pass (`prof::gpu` timestamps; busiest 10,
  and their sum per frame without the gaps between submissions, "busy")
  and the frame pacing histogram (share of frames per |frame time − median|
  bin: ≤0.25, ≤0.5, ≤1, ≤2, ≤4, >4 ms). Full is a developer's layout: with
  every row filled it reaches the mini map at 16:9 and below.

## In plain words (for testers)

* **FPS** — pictures drawn per second (over the last second).
* **ms after FPS** — how long one picture takes on average; 16.7 ms is
  60 FPS. **99%** / **max**: the slow ones; a single big max is one hitch.
* **1% / 0.1% low** — the frame rate of the slowest 1 % /
  0.1 % of pictures in the last 10 s: low values mean stutter.
* **CPU update** — the game's logic (physics, AI, traffic) per picture;
  **draw** — preparing the picture; **trails** — extra renders for the
  light trails (only some machines and frame rates).
* **wait display** — waiting for the screen or the graphics card to take
  the picture; large values mean the graphics card or vsync sets the pace.
  **cap** — the frame limiter holding the game back on purpose.
* **GPU** — the graphics card's work per picture; when it is close to the
  frame time, the graphics card is the limit.
* **Ticks** (Standard) — the game's logic runs 60 times a second at every
  frame rate (30 in cut-scenes); **speed 100%** means the game runs in real
  time. A red line **"game slowed to N% speed"** means pictures took so long
  (over 50 ms) that the game itself ran in slow motion: report it with the
  place and settings.
* **trails exact / cheap** (Standard) — how the light trails are made:
  "cheap" is chosen on a busy machine and looks nearly the same; it explains
  the "trails" time.

## What each frame figure is

The frame loop (`Game::redraw`, `shell/frame_loop.rs`) records every
**drawn** frame (`overlay::stats::FrameRecord`): not the ticks, not the
light-trail buffer's offscreen renders ("probes", docs/uncapped-fps.md
"Streak buffer on ticks"), not the loading screens a load draws from inside
itself. `overlay::stats::FrameStats` computes every figure from those
records; its tests drive it with the game's own tick clock at 30, 60, 75,
144 and 200 Hz, a frame cap, a stall, a slowed game, a scene's 1/30 s
ticks, timed / untimed / no GPU renders. All figures but the lows are over
the last second of drawn frames, refreshed four times a second.

| shown | what it is |
|---|---|
| FPS | drawn frames in the last second ÷ the sum of their intervals |
| frame time (ms) | the mean interval from one drawn frame's start to the next's: everything, the frame cap's sleep and the waits for the display included (= 1000 / FPS) |
| 99%, max | the 99th percentile (index ⌊0.99 n⌋) and the longest interval of the last second; a stall or a load is one long frame |
| 1% / 0.1% low | the frame rate of the 99th / 99.9th percentile interval of the last 10 s (MangoHud's reading); a stall holds them down for 10 s |
| update | per drawn frame: the main thread stepping the simulation's ticks (`advance`), without the light-trail renders between them. A frame between two ticks adds 0, one that took three ticks all three: at 144 Hz it is about 60/144 of a tick's cost |
| draw | per drawn frame: the main thread drawing (layout, recording, submission), without the waits below and the light-trail renders |
| trails | per drawn frame: the light-trail renders (probes) on the main thread, without their waits; their GPU time is in "GPU" |
| wait display | per drawn frame: waiting for the frame before to be submitted and presented (`settle`, every one of the frame's) and for a swap-chain image (`acquire`): with vsync, the wait for the refresh is here |
| cap | per drawn frame: the frame limiter's sleep before the frame (the frame cap, Power Saving's cap, Low Latency's pacer) |
| GPU | per drawn frame: the GPU's busy time — the sum of its passes between timestamps, without the gaps between submissions (`prof::gpu`, the timestamps Full's per-pass list reads), the light-trail renders' passes included; read back a few frames late, so the second's sum read is divided by the second's drawn frames. Every layout shows it with "busy N %", that time's share of the second. The timestamps run while the overlay is shown or logging. "n/a (no GPU timestamps)" on a device without timestamp queries, and in `--bench`, which times the GPU itself |
| ticks/s | ticks stepped per second of real time: 60 (30 in a scene with the native 1/30 s limit); fewer when frames are too long |
| each | the simulation's main-thread time per tick |
| speed | 1 − the real time the tick cap dropped ÷ the last second: a frame longer than 3 ticks (3/60 s; 2/30 s in a scene) steps only those, the game runs slower than real time; 100 % otherwise |
| trails exact / cheap | the light-trail buffer's form in the last second (share of frames in the exact form when mixed); not shown when the buffer is off (light streaks off, a temporal pass, no scene) |

The parts add up: frame time ≈ update + draw + trails + wait + cap, the
rest (the event loop, the overlay itself) under 0.2 ms here. The figures
are those of the frame loop in the window; `--bench` pushes its own
update / draw and the renders' GPU times, no waits.

Numbers are drawn with tabular digits (every digit and the figure-space
padding in one cell width, `overlay/panel.rs`), so they do not jitter as
they change.

### Verified (window on a private Xvfb display, software presentation)

A measuring build logged the time of every swap-chain present (on the
render thread, apart from the overlay's sums). Each CSV row's FPS was
compared with the presents' rate over the same 1 s window (rows aligned on
a 0.5 s stall made by stopping the process): median difference 0.0–0.5 %,
mean |difference| 2–6 % (the row's time is known to ±0.25 s on a heavily
loaded machine), in runs uncapped, with the frame cap at 30 / 144 / 200
and with vsync (Fifo), through the boot movie, the main menu, the garage,
free roam with the throttle held and the pause menu. The stall showed as
one frame of 502–535 ms in both, and the speed fell to 43–56 % for the
second after it. Ticks per second stayed 60 at every cap (two a frame at
30), the frame cap's sleep was its own part (30.0 of 33.4 ms at a 30 cap)
and the parts added up to the frame time. The GPU figure is the sum the
bench reports as "gpu busy" (the same timestamps): free roam at 1280 × 960
0.37 ms a frame in the window, 0.40 in `--bench explore`. The light-trail
buffer's own cost timer is not this figure: it times what one offscreen
light-trail render draws (the world's passes and the post chain up to the
trail buffer's commit; docs/uncapped-fps.md, "Streak buffer on ticks"), not
a drawn frame's output, screen passes and UI.
Xvfb has no vertical blank: Fifo did not hold the frame rate there, so
the vsync wait could not be seen in this setup.

Evidence (headless `--shot`, 1280 × 960 unless said; JPEG crops):
`docs/evidence/overlay/` — `minimal-topleft.jpg`, `standard-topleft.jpg`,
`full-topleft.jpg` (explore), `full-topright-75.jpg` (race start),
`standard-bottomright-150.jpg`, `minimal-bottomleft-main-half.jpg` (main
menu, half size), `full-4k-crop-half.jpg` (3840 × 2160, the player's size),
`minimal-corners-race.jpg` (the race start at 1280 × 720, Minimal in the
four corners at 100 % and 200 %), `minimal-many-deviations.jpg` (1280 × 720,
a long list of deviations wrapped), `bench-sample.csv` (the first rows of a
`--bench explore` log). The `full-*` and `standard-bottomright-150.jpg`
crops show older row texts in the same look.

Placement check (shots, not in the checks): Minimal and Standard in every
corner at 100 % and 200 % (Minimal also at 75, 125 and 150 %), at 1280 ×
720, 2560 × 1080, 1280 × 960 and 1366 × 768 (Minimal also at 3840 × 2160),
at the race start (`--shot qrstart`, `NFSU2_FRAMES=300 NFSU2_THROTTLE=1`:
the race HUD up) and in the main menu, and with a long list of deviations;
each panel's box taken from the shot minus the same shot without the
overlay, the HUD's pixels from that shot minus one without any UI
(`NFS_NO_UI=1`). In all 166 no panel reaches the screen's edge, and
Minimal's box lies inside Standard's at the same corner and scale, over at
most as many of the HUD's pixels. At 21:9 the panel keeps to the 16:9
area the HUD uses.

## Sources (Linux; `src/overlay/sources.rs`)

| metric | source | this machine (reference desktop GPU + reference integrated GPU) |
|---|---|---|
| CPU load, per core | `/proc/stat` deltas (busy = all − idle − iowait) | yes |
| CPU clock | cpufreq `scaling_cur_freq` per core | yes |
| CPU temperature | hwmon `k10temp` Tctl / `zenpower` Tdie / `coretemp` Package id 0 / `cpu_thermal` | yes (k10temp) |
| CPU power | powercap `intel-rapl:0/energy_uj` deltas | **no**: root-only on this kernel → n/a |
| GPU load, VRAM, GTT | `/sys/class/drm/cardN/device/gpu_busy_percent`, `mem_info_{vram,gtt}_{used,total}` (amdgpu) | yes |
| GPU clocks, temps, power | the card's hwmon: `freqN_input` (sclk / mclk), `tempN` edge / junction, `power1_average` (dGPU) or `power1_input` (APU) | yes |
| which card | the wgpu adapter's PCI vendor / device matched to `cardN/device/{vendor,device}`; else the first card with `gpu_busy_percent` | yes (card0 0x1002:0x747E, not the iGPU card1 0x164E) |
| game's VRAM / GTT / GPU share | `/proc/self/fdinfo` drm-usage-stats (`prof::gpu_memory`) | yes |
| RSS; RAM, swap | `/proc/self/status` VmRSS; `/proc/meminfo` | yes |
| game I/O | `/proc/self/io` (read_bytes / write_bytes; rchar / wchar) | yes |
| disks | `/proc/diskstats` whole disks (sdX, nvmeNnM, vdX, mmcblkN; not partitions, zram, loop), 512 B sectors | yes |
| threads | `/proc/self/task/*/{comm,schedstat}` | yes |
| VRR | `latency::vrr::probe` (KMS properties) | yes (probe) |
| NVIDIA | NVML via `dlopen("libnvidia-ml.so.1")` (`overlay/nvml.rs`), device 0 | **open**: no NVIDIA GPU here, untested |
| Intel GPU | – | **open** (TODO: i915/xe `gt_cur_freq_mhz`; busy via fdinfo) |
| Windows, macOS | stubs (TODO notes in `sources.rs`: PDH, DXGI QueryVideoMemoryInfo, ADLX / NVAPI / NVML, GetProcessMemoryInfo / IoCounters; IOKit, host_processor_info) | – |

A metric that is not readable shows "n/a" and logs an empty CSV field.
Not covered: the game's I/O split by kind (cache / packs / streaming)
needs loader hooks (**open**); VRR "low framerate compensation" state.

## CSV log (`src/overlay/csv.rs`)

`<data home>/overbright/perf/overbright-<UTC time>.csv` (or
`$NFSU2_PERF_CSV`), one row per sample (4 Hz), flushed each row. Columns:
`time_s, fps, frame_ms, frame_p99_ms, frame_max_ms, low1_fps, low01_fps,
update_ms, draw_ms, acquire_ms, cpu_pct, cpu_mhz_avg, cpu_mhz_max,
cpu_temp_c, cpu_power_w, gpu_pct, gpu_core_mhz, gpu_mem_mhz, gpu_temp_c,
gpu_hotspot_c, gpu_power_w, vram_used_mb, vram_total_mb, gtt_used_mb,
own_vram_mb, own_gtt_mb, own_gpu_pct, rss_mb, ram_used_mb, ram_total_mb,
swap_used_mb, io_read_kbs, io_write_kbs, io_rchar_kbs, disk_read_kbs,
disk_write_kbs, process_cpu_pct, vrr_active, cores_pct, threads_pct,
trails_ms, wait_ms, cap_ms, gpu_ms, ticks_per_s, tick_ms, speed_pct,
trails_per_s` (`cores_pct` and `threads_pct` `;`-separated, quoted when
needed). Frame columns are the draw thread's figures at its last refresh
(≤ 0.25 s old), as defined above (`acquire_ms`: the swap-chain part of
`wait_ms`).

## Cost

* Draw thread: `--bench explore 1800`, CPU by part, two runs each
  (ms/frame): `ui` off 0.02 / 0.03, Minimal 0.05 / 0.04 / 0.04 (three
  runs), Standard 0.10 / 0.09, Full 0.13 / 0.13; `hud and menu layout`
  +0.01–0.02. So about +0.02 (Minimal), +0.07 (Standard), +0.10 ms (Full)
  per frame, almost all in the UI renderer's per-glyph work. The machine
  was shared with heavy jobs: whole-frame means moved by more than that
  between runs of one arm.
* No allocation per frame in the steady state: a frame's record goes into
  ring buffers; the text and quads are rebuilt four times a second and
  copied in between.
* Per frame push: 6.7 ns; the 4×/s refresh: 0.033 ms at 1 000 FPS
  (10 000-frame window, O(n) selection, not a sort).
* Sampler thread ("perf sampler", off the frame's path): 0.49 ms CPU per
  sample, 4 Hz ≈ 0.2 % of one core.
* GPU timestamps: the device is asked for timestamp queries wherever the
  adapter has them (`render::Gpu::new`); the overlay's timing
  (`prof::gpu`, a timestamp per pass) starts when the overlay is first
  shown or logging and pauses (`prof::gpu::set_active`) while it is hidden
  and not logging. The bench keeps its own timing.

## Code

`src/overlay/{mod,stats,sources,sampler,csv,nvml}.rs`; fixtures
`tests/fixtures/overlay/root/` (a small hand-written /proc + /sys tree:
two AMD cards, k10temp, nvme hwmon, cpufreq, diskstats with partitions,
zram and loop). Hooks: `frontend/options.rs` (Settings `overlay` field,
`overlay.*` lines, four rows), `prof.rs` (`gpu::ready/active/set_active`),
`bin/game.rs` (F3, the frame push, the quads, `gpu::want` before the
window's device, `NFSU2_SHOT_FPS` / `NFSU2_SHOT_ENH`, the bench's push,
the loading gate's time), `lib.rs`.
