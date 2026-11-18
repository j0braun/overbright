# Modern extras: Low Latency, FSR, replay cameras, accessibility

Clone
deviations, every one opt-in on the Enhancements page (Display →
Enhancements) or the Graphics page's Upscaling row, every default native.
With them off the frames, audio and simulation are the base binary's (parity
gates below). Nothing here is in SPEED2.EXE; what was traced in it is said
per section.

| row | values | options.txt |
|---|---|---|
| **Low Latency** (Enhancements) | Off / On | `enh.low_latency 1` |
| **Upscaling** (Graphics → Performance): new tiers | … / FSR Quality 67 % / FSR Balanced 58 % / FSR Performance 50 % | `enh.upscale 4..6` |
| **Replay Cameras** (Enhancements) | Native / Extended | `enh.replay_cameras 1` |
| **HUD Colours** (Enhancements) | Native / Deuteranopia / Protanopia / Tritanopia | `enh.hud_palette 1..3` |
| **Minimap Size**, **Speedometer Size**, **Race Info Size** | 70–150 % (100 native) | `enh.hud_minimap`, `enh.hud_speedo`, `enh.hud_info` |
| **Reduced Flashing** (Enhancements) | Off / On | `enh.reduced_flashing 1` |

Code: `src/extras.rs` (the values, HUD element scale, flash cap, CPU palette),
`src/latency.rs` (pacer, display model, VRR probe), `src/render/enh/post/fsr.rs` +
`fsr.wgsl` (FSR 1), `src/replay_cams.rs` (extra cameras, smooth slow
motion), hooks in `qol.rs` (one `extras` field; FSR tiers), `enhance.rs`
(FSR in place of the resample), `ui.rs` / `ui.wgsl` (palette pipelines),
`options.rs` / `graphics_table.rs` (rows), `bin/game.rs` (loop, replay,
HUD, `--bench latency`).

## 1. Low Latency and VRR-aware pacing

**Native (traced earlier, not new):** the loop 0x5811D0 reads input at its
top and presents at the bottom; g_VSyncOn (0x870D24) gives
D3DPRESENT_INTERVAL_ONE (0x5B9B13). With the driver's queue full, the
wait for a back buffer falls *after* the input. The clone's window loop has
the same shape (`Game::redraw`: input and world in `advance`, the image's
acquire in `draw`, the present on a render thread).

**On** (`latency::Pacer`, `Game::redraw`):
* the swap chain asks for one frame in flight
  (`desired_maximum_frame_latency` 1 instead of 2), so no hidden queue;
* **phase-locked start**: with the monitor's refresh rate known (winit),
  a blocked acquire (or a blocked wait for the last present) returns at a
  refresh — that fixes the refresh grid. Each frame then sleeps so that it
  is presented `SLACK` (1.5 ms) before the next refresh it can make: input
  at refresh − p90(work of the last 60 frames) − slack, the target being
  the refresh *after* the one that shows the previous frame (with one
  frame in flight its image is free only then). A frame that blocks again
  (work grew, rate drifted) re-anchors the grid. Without a known rate: an
  integrator on the blocked time (the fallback);
* **VRR**: wgpu exposes no VRR state, so the platform is asked
  (`latency::vrr::probe`, every 2 s): on Linux the DRM connector property
  `vrr_capable` of each connected output and the CRTC property
  `VRR_ENABLED` (read-only KMS ioctls on `/dev/dri/card*`, no master, no
  connector probe; works under X11 and Wayland). While a panel is driven
  variably, frames are held 3 % under its top rate (`VRR_HEADROOM`), so
  FIFO never queues and the panel stays in its range. **Open**: Windows /
  macOS are not probed (VRR "unknown": no cap; the pacer still works).
  The reference desktop: DP-1 and DP-3 `vrr_capable` 1, `VRR_ENABLED` 0 on the
  desktop (checked with `modetest`, the probe agrees).
* **The world cadence is untouched**: the pacer only moves when a frame
  starts; `frame_clock::Cadence` takes the real time that passed (fixed
  1/60 s updates, 60 fps tick pattern, interpolated draws), so the
  simulation is bit-for-bit the same with Low Latency on (the input is
  still read at the updates). Uncapped rendering still works (Off: no
  change; On with Mailbox / Immediate: nothing blocks, the pacer sleeps 0).
* **Inferred**: the frame's end in `redraw` stands in for its present (the
  render thread presents right after); the GPU's own time sits in the
  slack.

**Measured** (`--bench latency 600`: the racing bench's race on the
autopilot, the window loop's own `advance` + `draw_between` on the real
clock, 1080p, through `latency::Display`, a model of the swap chain and
panel — FIFO with 2 queued frames (Off) or 1 (On), fixed 60 / 144 Hz or VRR
144 Hz; the model's waits are slept for real so the pacer sees them as an
acquire). Proxies per frame that took a world update: input sample → frame
presented and done on the GPU, and → the refresh that shows it (model).
Quiet-GPU runs (the frame's end + GPU ≤ 3 ms; other jobs' GPU work made
some runs noisy):

| display (model) | Off: input → present | Off: input → photon (p95) | On: input → present | On: input → photon (p95) | On: refreshes missed |
|---|---|---|---|---|---|
| FIFO 60 Hz | 16.7 ms | 47.5–47.9 ms (49.8) | 5.7–8.2 ms | 9.5–13.7 ms (11.4–27.5) | 3.5–7.5 % |
| FIFO 144 Hz | 8.1–9.1 ms | 13.4–16.6 ms (19.6–20.6) | 6.1–7.2 ms | 9.3–11.9 ms (13.7–18.0) | ¹ |
| VRR 144 Hz | 7.6–7.9 ms | 10.8–14.9 ms (20.1–20.4) | 5.8–6.2 ms | 6.4–6.6 ms (9.8–10.9) | ¹ |

Three runs (internal evidence), machine load average 117–371 on 16
threads from other jobs (the frame's end + GPU 2–6 ms instead of ~1 ms
on an idle machine, so the 144 Hz rows rarely held 144 frames/s Off or
On; ¹ the gaps there are the load's, Off has as many). Off at 60 Hz is the
classic two-frame queue (≈ 3 refreshes from input to photon); On is the
frame's own work and the slack. The pacer's misses at 60 Hz (a frame
shown a refresh late) come from work spikes past the p90 + 1.5 ms margin
under that load (**open**: a larger margin under load trades latency).

Unit tests on the pure model (`latency::tests`): FIFO 60 / 144 Hz with 2
or 5 ms of work, On is under half of Off and within work + 4.5 ms with no
missed refresh; ±1 ms of jitter at 144 Hz holds; VRR with the cap leaves
the queue empty; a display that never blocks leaves the pacer at 0.

## 2. FSR as an upscaler choice (FSR 1, not FSR 2)

**Choice:** FSR 1 (EASU + RCAS, AMD FidelityFX, MIT; `fsr.wgsl` is a port of
ffx_fsr1.h's FsrEasuF / FsrRcasF with exact reciprocals). FSR 2 is a
temporal upscaler (jitter, motion vectors, reactive / transparency masks,
~5 compute passes, its own history) — the job the temporal tier's TAAU already does with
the clone's own motion, at a large porting effort and a second set of
ghosting risks; FSR 1 is two small passes, needs no history (nothing to
reset at cuts, no ghosting), keeps the native MSAA as its anti-aliasing
and gives a genuinely different trade-off (sharper, aliasing-free when the
input is anti-aliased, no temporal stability gain).

**Where:** the game's world only (not the front end / garage, as TAAU).
The scene and the native post chain run at the tier's share (67 / 58 /
50 % per side, TAAU's) with the native MSAA — the Render Scale path —
and EASU + RCAS (0.2 stops) take the result to the screen's size in place
of the resample (`enhance::Plan::fsr`, `set_fsr_frame`); FXAA / SMAA /
CAS run after it if chosen; the HUD and menus are drawn after it at the
screen's size. **Deviation from the plan** ("glow/streak/HUD after it as
with TAAU"): the glow, light streaks and bleach run *before* FSR at the
render size, the HUD after. Reasons: AMD's FSR 1 integration places EASU
after post-processing in perceptual space; the clone's glow is built at the
size native builds it for the screen anyway (docs/post-scale.md); and the
TAAU-style split would need single-sample variants of the world's
particle / glow pipelines for the late sprites (the scene keeps its MSAA
under FSR). Particles and rain are part of the upscaled image.

**Costs** (GPU busy ms/frame, `--bench explore 600` at night, mean of 2,
internal evidence; Original = native 8x
MSAA, World Detail 5):

| | 1080p | 4K |
|---|---|---|
| Original (native 8x MSAA) | 0.72 | 2.39 |
| TAAU Quality 67 % | 0.63 (−0.09) | 1.45 (−0.94) |
| TAAU Balanced 58 % | 0.52 (−0.20) | 1.50 (−0.89) |
| TAAU Performance 50 % | 0.52 (−0.20) | 1.44 (−0.95) |
| FSR Quality 67 % | 0.57 (−0.15) | 1.56 (−0.83) |
| FSR Balanced 58 % | 0.53 (−0.19) | 1.45 (−0.94) |
| FSR Performance 50 % | 0.47 (−0.25) | 1.32 (−1.07) |

FSR's own two passes (GPU timestamps, `graphics: fsr easu / rcas`): at
4K 0.30–0.31 ms EASU + 0.10 ms RCAS, at 1080p 0.09 + 0.04 ms (per frame, mean). FSR keeps the native 8x MSAA at the lower
size, which TAAU drops; at 4K the two cost the same within noise (other
jobs' GPU work: ±0.1 ms). The Graphics row's cost classes use the 4K
deltas (`FSR_COSTS_4K`).

**Image** (internal evidence: racing
frame 200, 1080p, top row Original / TAAU 67 / 58 / 50 %, bottom row
Original / FSR 67 / 58 / 50 %; `fsr_vs_taau_4k.webp`: Original, TAAU 67 %,
FSR 67 %, FSR 50 % at 4K; the runs' cars stand a little apart, the frames
are not pixel-aligned): in motion TAAU smears the moving cars (the
player's number plate is unreadable at every TAAU share), FSR keeps the
plate legible down to 50 % and the MSAA's clean edges; FSR at 50 % shows
the lower resolution in fine texture (road grain, distant signs). TAAU's
advantage is temporal stability on static detail (docs/temporal.md).

## 3. Replay cameras, slow motion, cinematic DOF

**Native (traced earlier, docs/formats.md "Replays"):** the replay
director 0x5FAFE0 / 0x5FB060: index 0 automatic (the track cameras, mover
0x12, else a drawn chase view), 1–5 the chase views 0x802130 (2, 4, 1, 0,
5 — bumper is one of them); slow motion 0x4A4FB0 / 0x581060 holds three
render frames of four (0x865148 & 3: ¼ speed at 15 updates a second,
choppy). These stay the default and the first six on C.

**Extended** (`replay_cams`): C goes on after the six through
**Trackside** (a pole every 60 m along the race road `RoadRoute`, 9 m to
the side, alternating sides, 2.2 m over the car, the next one placed 60 m
ahead once the car is 22 m past; the field of view zooms with distance,
10°–50°), **Helicopter** (22 m behind-side, 16 m up, slowly circling),
**Wheel** (on the body beside the front left wheel, looking ahead) and
**Orbit** (7 m, circling with replay time), then back to automatic. Each
is a camera mover of its own (0x200 + index: a switch is a cut, no
interpolation / TAA history across it) and a "watched" camera for
Cinematic depth of field (`blur_dof::dof_scene`, mover ≥ 0x100), so the
Depth Of Field row's Cinematic tier runs on them as on the native track
cameras. **Smooth slow motion**: `,` steps 1 → ½ → ¼ → 1; the replay's
time runs at that share and every drawn frame is placed between the two
recorded frames by the replay's own time (`slow_alpha`; the poses before
the last played frame are kept, `Game::smooth_slow_update`), instead of
the native held frames. Skips end it, as native.

Measured (`NFSU2_REPLAY_SMOOTH_PROBE=144`, 2 s of the window loop at
144 Hz, the player's drawn step per frame): native 1× mean 0.154 m, sd 7 %;
**native slow** mean 0.032 m, sd 220 %, 224 of 287 frames standing; smooth
½ 0.070 m, sd 6 %, none standing; smooth ¼ 0.033 m, sd 5 %, none standing.
Frames: internal evidence (with Cinematic
DOF).

## 4. Accessibility

* **HUD Colours** (`ui.wgsl` `palette`, `extras::palette_rgb` the same on the
  CPU): on the in-game HUD's quads only (not the mirror's picture, not the
  menus; `ui::set_hud_palette` by index range), drawn by their own
  pipelines (`fs_palette`) so the native `fs_main` is untouched. Saturated
  hues the kind confuses go to Okabe–Ito ones it tells apart, value and
  alpha kept, greys / whites / weak colours untouched: deuteranopia and
  protanopia — reds (the tachometer's red zone, red text) to amber
  (protanopia also brighter: reds look dark to it), greens (the player's
  minimap arrow, "Player" in the leader board) to sky blue; tritanopia —
  blues (the minimap's water) to vermilion, yellows to pink.
  `hud_palettes.webp`.
* **Per-element HUD scale** (`extras::apply_hud`, `qol::scale_hud_by`):
  the existing HUD Scale's parts (quads that touch, anchored to their
  third) get the HUD Scale times their element's row: minimap (bottom-left
  third), speedometer / tachometer (bottom-right), race info (top half:
  position, laps, times, leader board) — HUD_*_Track's layout as drawn.
  `hud_sizes.webp`.
* **Reduced Flashing**: a bright (luma > 0.5) quad over ≥ 90 % of the
  canvas is held to alpha 0.25, and the NIS SCREEN_FLASH's amount is
  held to 0.25 (`extras::FLASH_AMOUNT`; the tint pass brightens by × 1.25
  instead of × 1.99 on its first three frames, `screen_flash`). The
  original's full-screen flash is that event (traced and ported in the visual-rating parity,
  docs/nis-mover.md "SCREEN_FLASH": slot 0 of the view's effect controller,
  drawn by the tint pass 0x5C7870 as dest × up to 1.99 over six frames);
  the weather has no visual flash (thunder is audio only, 0x4852A0).
* **Subtitles size**: not added. **Traced** (docs/formats.md "Subtitles"):
  the files exist (SUBTITLES\Scene01 …) but 0x57DAB0 starts them only
  for regions 2 / 4 / 5 / 0xC (0x50D350), and this EXE's region is the
  static 6 (0x7FA73C), so the original never shows them; showing them
  would be a new feature, not an accessibility setting of an existing one.

## Native byte-identity

`perf.sh parity`, base = this copy before this work, both fast-build binaries
(internal evidence): Original images of the 5
bench scenes at 1280×960 and 3840×2160, the window path at 144 Hz, Modern /
Ultra racing and explore, 15 shots — **all AE 0**; audio WAVs identical;
state hashes identical over firstrace, firstdrag, postrace, the
playthrough (82 493 steps) and the parity scenario. A first run had 1–2
HUD pixels off by 1 at 4K: the UI shader's native entry had been wrapped
for the palette; the palette now has its own entry and pipelines and
`fs_main` is the original text (second run: all image gates AE 0).

## Tests

`latency::tests` (6: model, pacer, VRR cap, ioctl request sizes),
`replay_cams::tests` (3), `extras::tests` (3: round trip, palettes,
elements and flashes), `render::fsr::tests` (2; the GPU one ignored:
flat stays flat, edges bounded), `render::ui::palette_tests`, `qol`
round trip (57 deviations), `options` row order and labels, game test
`replay_extended_cameras_follow_the_native_six`.

## Open

* Windows / macOS VRR detection (no API probed); the present time is
  inferred from the frame's end; the pacer was measured on a display
  model, not a real panel (headless rule) — a real FIFO / VRR panel check
  is for the player's machine.
* FSR 2 was ported later as its own Upscaling tiers (docs/fsr2.md); FSR 1 costs on the iGPU not measured.
* The extra cameras are not compared with anything native; the wheel
  camera's offset is generic (not per car).
* Reduced Flashing does not cap Take Picture's flash (0x501250 → 0x5DBD80(1), the same slot; not ported).
* The palette uses fixed hue ranges; HUD art with baked colours outside
  them (e.g. the orange destination mark) stays.
