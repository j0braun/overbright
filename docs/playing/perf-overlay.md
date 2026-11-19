# Performance overlay

An overlay in the style of MangoHud: frame rate, frame times, CPU and GPU readings on screen while you play.
It is Overbright's own tool, not part of the original. It is hidden until you press **F3** or turn it on in
the settings, and while hidden nothing of it runs.

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
the reference desktop's metrics; `NFSU2_SHOT_ENH="overlay.position 1;overlay.scale
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

## How it works

How it works, what it costs, how it was checked and what is open: [Performance overlay (MangoHud-style): how it works](../notes/performance/overlay.md).
