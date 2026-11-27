# Modern extras: Low Latency, FSR, replay cameras, accessibility

Optional settings that the original game does not have, on the **Options → Display → Enhancements** page.
Each is off, or at the original's value, until you change it. With all of them off, the pictures, sound and simulation are exactly as without them.

| Setting (where) | Values | What you get | options.txt |
|---|---|---|---|
| **Low Latency** (Advanced → Performance) | Off / On | Less delay between your input and the picture | `enh.low_latency 1` |
| **Upscaling** (Graphics → Performance), FSR tiers | … / FSR Quality 67 % / FSR Balanced 58 % / FSR Performance 50 % | A faster frame drawn at a lower size, sharpened up to the screen | `enh.upscale 4..6` |
| **Replay Cameras** (Gameplay & Accessibility) | Native / Extended | Four more replay cameras and smooth slow motion | `enh.replay_cameras 1` |
| **HUD Colours** (Gameplay & Accessibility) | Native / Deuteranopia / Protanopia / Tritanopia | HUD colours for colour-vision deficiency | `enh.hud_palette 1..3` |
| **Minimap Size**, **Speedometer Size**, **Race Info Size** (Gameplay & Accessibility) | 70–150 % (100 is the original) | Size each part of the HUD separately | `enh.hud_minimap`, `enh.hud_speedo`, `enh.hud_info` |
| **Reduced Flashing** (Gameplay & Accessibility) | Off / On | Weaker full-screen flashes | `enh.reduced_flashing 1` |
| **In the Background** (Advanced → Performance) | Native (pause) / Keep running / Draw less | What the game does while its window is not focused or is minimised | `enh.background 1..2` |
| **Power Saving** (Advanced → Performance) | Off / Menus / Menus + Battery | Menus, and on battery the whole game, held to the display's refresh rate | `enh.power_caps 1..2` |

## What each setting does

**Low Latency** keeps only one frame waiting for the screen and starts each frame as late as it safely can,
so your input is read closer to the moment the picture shows. On a 60 Hz screen this cuts the time from input
to picture from about 48 ms to about 10–14 ms (measured on a model of the display, below). On a variable
refresh rate (VRR) screen on Linux, frames are also held 3 % under the screen's top rate. With vsync off the
frame rate stays as high as without it: the game only keeps the graphics card from queueing frames behind
your input. The game's world
still updates exactly as before.

**FSR** (AMD FidelityFX Super Resolution 1) draws the world at 67, 58 or 50 % of the screen size per side
and sharpens it up to full size. Compared with the temporal upscaler (TAAU), moving cars stay sharp (your
number plate stays readable down to 50 %) and anti-aliased edges stay clean, but fine texture such as road
grain and distant signs shows the lower size at 50 %. TAAU is steadier on still detail
([temporal.md](settings/temporal.md)). FSR 2 is a separate set of Upscaling tiers ([fsr2.md](settings/fsr2.md)).

**Replay Cameras: Extended.** In a replay, C steps through the original's six cameras and then four more:

- **Trackside**: poles along the race road that the camera watches from, zooming with distance;
- **Helicopter**: high behind and to the side, slowly circling;
- **Wheel**: on the body beside the front left wheel, looking ahead;
- **Orbit**: circling the car.

Then it goes back to the automatic camera. The Depth Of Field row's Cinematic setting works on the new
cameras as on the original's track cameras. Slow motion (`,`) steps 1 → ½ → ¼ → 1 and plays smoothly, instead
of the original's slow motion, which holds three frames of every four. Skipping ends slow motion, as in the
original.

**HUD Colours** swaps the HUD's colours that people with each kind of colour-vision deficiency confuse for
ones they can tell apart (the Okabe–Ito palette). Brightness and transparency stay; greys, whites and weak
colours stay.

- Deuteranopia and protanopia: reds (the tachometer's red zone, red text) become amber, and greens (your
  minimap arrow, "Player" in the leader board) become sky blue. Protanopia also makes the amber brighter,
  because reds look dark to it.
- Tritanopia: blues (the minimap's water) become vermilion and yellows become pink.

Only the in-game HUD changes, not the mirror's picture or the menus.

**Minimap Size, Speedometer Size, Race Info Size** multiply the HUD Scale for one part each: the minimap
(bottom left), the speedometer and tachometer (bottom right), and the race information (top half: position,
laps, times, leader board).

**Reduced Flashing** holds bright full-screen flashes to a quarter of their strength. The original's
full-screen flash is the scenes' SCREEN_FLASH; the weather has no visual flash (thunder is sound only). It
does not yet cover Take Picture's flash.

**In the Background.** Native: like the original (observed: docs/evidence/focus-loss/, docs/original/focus-loss.md), the game pauses while its window is not the focused one or
is minimised. The race, the drawing and the sound stop, and keys you held are let go. When you come back it goes
on from where it stood; the time away is not caught up. **Keep running** lets the race and the sound go on.
**Draw less** does too, but draws only 30 frames a second, which saves power. The game's world still runs at its
own pace. Details: [focus-loss.md](../original/focus-loss.md).

**Subtitles size** was not added. The original has subtitle files, but this edition never shows them, so
showing them would be a new feature (see "Accessibility" below).

## How it works

The rest of this page is for developers. Nothing here is in `SPEED2.EXE`; what was traced in it is said per
section.

Code: `crates/ob-settings/src/extras.rs` (the values, HUD element scale, flash cap, CPU palette), `src/latency.rs` (pacer,
display model, VRR probe), `crates/ob-render/src/render/enh/post/fsr.rs` + `fsr.wgsl` (FSR 1), `src/replay_cams.rs` (extra
cameras, smooth slow motion). Hooks in `qol.rs` (one `extras` field; FSR tiers), `enhance.rs` (FSR in place
of the resample), `ui.rs` / `ui.wgsl` (palette pipelines), `options.rs` / `graphics_table.rs` (rows),
`bin/game.rs` (loop, replay, HUD, `--bench latency`).

### 1. Low Latency and VRR-aware pacing

**The original** (traced earlier): the loop 0x5811D0 reads input at its top and presents at the bottom;
g_VSyncOn (0x870D24) gives D3DPRESENT_INTERVAL_ONE (0x5B9B13). With the driver's queue full, the wait for a
back buffer falls after the input. Overbright's window loop has the same shape (`Game::redraw`: input and
world in `advance`, the image's acquire in `draw`, the present on a render thread).

**With Low Latency on** (`latency::Pacer`, `Game::redraw`):

- The swap chain asks for one frame in flight (`desired_maximum_frame_latency` 1 instead of 2), so there is
  no hidden queue.
- **Start locked to the refresh:** with the monitor's refresh rate known (winit), a blocked acquire (or a
  blocked wait for the last present) returns at a refresh, which fixes the refresh grid. Each frame then
  sleeps so that it is presented `SLACK` (1.5 ms) before the next refresh it can make: input at refresh −
  p90(work of the last 60 frames) − slack. The target is the refresh after the one that shows the previous
  frame (with one frame in flight its image is free only then). A frame that blocks again (work grew, the
  rate drifted) re-anchors the grid. Without a known rate, an integrator on the blocked time is the fallback.
- **VRR:** wgpu exposes no VRR state, so the platform is asked (`latency::vrr::probe`, every 2 s). On Linux
  that is the DRM connector property `vrr_capable` of each connected output and the CRTC property
  `VRR_ENABLED` (read-only KMS ioctls on `/dev/dri/card*`; no master, no connector probe; works under X11 and
  Wayland). While a panel is driven variably, frames are held 3 % under its top rate (`VRR_HEADROOM`), so
  FIFO never queues and the panel stays in its range. **Open:** Windows and macOS are not probed (VRR
  "unknown": no cap; the pacer still works). On the reference desktop DP-1 and DP-3 report `vrr_capable` 1
  and `VRR_ENABLED` 0 (checked with `modetest`; the probe agrees).
- **The world's update rate is untouched:** the pacer only moves when a frame starts. `frame_clock::Cadence`
  takes the real time that passed (fixed 1/60 s updates, the 60 fps tick pattern, interpolated draws), so the
  simulation is bit-for-bit the same with Low Latency on; input is still read at the updates. Uncapped
  drawing still works (Off: no change; On with Mailbox / Immediate: nothing blocks, the pacer sleeps 0).
- **Inferred:** the frame's end in `redraw` stands in for its present (the render thread presents right
  after); the GPU's own time sits in the slack.

**Measured** with `--bench latency 600`: the racing bench's race on autopilot, the window loop's own
`advance` + `draw_between` on the real clock, 1080p, through `latency::Display`. That is a model of the swap
chain and panel: FIFO with 2 queued frames (Off) or 1 (On), fixed 60 / 144 Hz or VRR 144 Hz; the model's
waits are slept for real, so the pacer sees them as an acquire. Values per frame that took a world update:
input sample → frame presented and done on the GPU, and → the refresh that shows it (model). Runs with a quiet
GPU (the frame's end + GPU ≤ 3 ms):

| Display (model) | Off: input → present | Off: input → photon (p95) | On: input → present | On: input → photon (p95) | On: refreshes missed |
|---|---|---|---|---|---|
| FIFO 60 Hz | 16.7 ms | 47.5–47.9 ms (49.8) | 5.7–8.2 ms | 9.5–13.7 ms (11.4–27.5) | 3.5–7.5 % |
| FIFO 144 Hz | 8.1–9.1 ms | 13.4–16.6 ms (19.6–20.6) | 6.1–7.2 ms | 9.3–11.9 ms (13.7–18.0) | ¹ |
| VRR 144 Hz | 7.6–7.9 ms | 10.8–14.9 ms (20.1–20.4) | 5.8–6.2 ms | 6.4–6.6 ms (9.8–10.9) | ¹ |

Three runs, on a machine with a load average of 117–371 on 16 threads from other jobs (the frame's end + GPU
took 2–6 ms instead of about 1 ms on an idle machine), so the 144 Hz rows rarely held 144 frames/s, Off or
On. ¹ The gaps there come from the load; Off has as many. Off at 60 Hz is the classic two-frame queue (about 3
refreshes from input to photon); On is the frame's own work plus the slack. The pacer's misses at 60 Hz (a
frame shown a refresh late) come from work spikes past the p90 + 1.5 ms margin under that load (**open**: a
larger margin under load trades latency).

Unit tests on the pure model (`latency::tests`): FIFO 60 / 144 Hz with 2 or 5 ms of work, On is under half
of Off and within work + 4.5 ms with no missed refresh; ±1 ms of jitter at 144 Hz holds; VRR with the cap
leaves the queue empty; a display that never blocks leaves the pacer at 0.

#### Low Latency with vsync off (report #349)

**The fault in 0.7.0:** with vsync off (Mailbox, the default, or Immediate) Low Latency cut the frame rate
to about the refresh rate (a player: 170–200 fps → 50–60 fps, GPU use 99 % → 30–40 %). Two causes, both
in the vsync-off path:

- **One frame in flight in every present mode.** wgpu's swap chain then has two images: the acquire waits for
  the GPU work of the frame before (Vulkan's acquire fence; on DX12 the frame-latency waitable), so the CPU
  and the GPU no longer overlap, and a Mailbox image comes back only at the next refresh.
- **The pacer locked to the refresh even without vsync.** Any blocked wait (here the GPU's) set its refresh
  grid, and it then started one frame per refresh.

**Now** (`latency::frames_in_flight`, `Pacer::for_present`, `WindowState::jit_wait`, `Game::frame_due`):
with vsync on nothing changes (one frame in flight, the pacer locked to the refresh). With vsync off:

- two frames in flight, as with Low Latency off;
- a **just-in-time GPU wait**: once the last frame's end has gone to the GPU (its render thread done, the
  frame's `settle`) and before the swap chain's image is acquired, the frame waits until the GPU has finished
  the frame before the last one (that frame's last submission). The last frame is then still running on the
  GPU, so the GPU does not go idle, and no more than about one frame is queued (no full drain). Waiting there,
  not at the frame's start, keeps the render thread's submission out of the wait, so the wait is the GPU's
  alone;
- the pacer's integrator (no refresh grid) on the time the frame was blocked after its input (that wait plus
  the acquire). It moves that time to a sleep before the input, so the frame reaches the GPU about `SLACK`
  (1.5 ms) before the GPU finishes the one ahead of it. When the CPU is the limit nothing blocks and the sleep
  is 0.

**The waits run in the event loop.** The frame cap, Power Saving's cap and Low Latency's sleep used to run
inside the frame, after the window's events had been read, so a key pressed during the sleep reached the
ticks one frame later. Now `about_to_wait` sleeps until 1.5 ms before the frame's start (`WaitUntil`) and
the frame waits out only the rest, so input that arrives during the sleep goes into that frame's ticks.

**Measured** (`NFSU2_LATENCY_GPU=1 --bench latency 500`: the racing bench on the autopilot, its real frames
on the real GPU, offscreen, with the swap chain's waits applied as wgpu's Vulkan swap chain applies them; 4K,
the Ultra preset; an AMD graphics card, two runs, load average about 10 from other jobs):

| 4K Ultra | frames/s | main thread ms/frame | GPU busy ms/frame | input → frame done on the GPU |
|---|---|---|---|---|
| Off | 77.5 / 96.7 | 7.6 / 5.2 | 6.2 / 6.3 | 35.2 / 31.0 ms |
| 0.7.0 On (Immediate) | 55.9 / 56.7 | 2.7 / 3.5 | 9.3 / 8.9 | — |
| 0.7.0 On (Mailbox, 60 Hz) | 30.0 / 30.2 | 6.9 / 10.8 ¹ | 8.5 / 8.8 | — |
| On now | 82.0 / 90.9 | 3.0 / 4.6 | 6.8 / 6.6 | 18.6 / 19.9 ms |

¹ includes the wait for the refresh. At 1080p Original (CPU-bound, about 550–950 fps Off) 0.7.0 On held
59–60 fps; On now runs at Off's rate or above. The model of the same pipeline (`latency::pipeline`,
GPU-bound 5–5.5 ms frames, 1.5–2 ms of CPU, Mailbox / Immediate at 60 and 144 Hz): 0.7.0 held 60 fps at
60 Hz and 96 fps at 144 Hz; now the frame rate equals Off's and input → frame done is 1.5–7 ms shorter
(`vsync_off_low_latency_keeps_the_frame_rate`).

#### How fast input reaches the car

The world runs on fixed 1/60 s ticks (docs/original/uncapped-fps.md). A key or click is queued for the tick
whose 1/60 s window holds its arrival (`input::queue`), so its effect is the same at any frame rate; it is
taken at the start of that tick, which runs in the first drawn frame after the window ends. Measured with
synthetic input (`input_reaches_its_tick_within_a_frame_of_its_window`): from a key's arrival to the frame
that runs its tick, 8.3 ms on average (16.6 ms at worst) at 60 fps, 11.3 ms (22.4 ms) at 170 fps and 10.0
ms (19.9 ms) at 200 fps: never more than the rest of its window plus one frame, about 1–2.5 drawn frames in
between. No press is lost: a press and its release inside one tick both apply in that tick, and a shift
lands in its tick at 30, 60, 120, 170 and 200 fps alike (`a_quick_shift_lands_on_its_tick_at_every_frame_rate`).
A controller is read at each tick (a button held for less than one tick between two reads is not seen, as
the original polls once per frame). The rest of the time to the screen is the render queue: with vsync off
and the GPU at its limit, two frames in flight add up to two GPU frames, which Low Latency's just-in-time
wait removes.

### 2. FSR as an upscaler choice

**Why FSR 1 here:** FSR 1 (EASU + RCAS, AMD FidelityFX, MIT; `fsr.wgsl` is a port of ffx_fsr1.h's FsrEasuF /
FsrRcasF with exact reciprocals) is two small passes. It needs no history, so there is nothing to reset at
cuts and no ghosting, it keeps the original's MSAA as its anti-aliasing, and it gives a different trade-off
from TAAU: sharper, free of aliasing when the input is anti-aliased, but no gain in temporal stability. FSR 2
is a temporal upscaler (jitter, motion vectors, reactive and transparency masks, about 5 compute passes, its
own history), the job TAAU already does with Overbright's own motion; it was ported later as its own tiers
([fsr2.md](settings/fsr2.md)).

**Where it runs:** the game's world only (not the menus or the garage, as TAAU). The scene and the original
post chain run at the tier's share (67 / 58 / 50 % per side, TAAU's) with the original's MSAA, the Render
Scale path. EASU + RCAS (0.2 stops) then take the result to the screen's size in place of the resample
(`enhance::Plan::fsr`, `set_fsr_frame`). FXAA, SMAA or CAS run after it if chosen; the HUD and menus are drawn
after it at the screen's size. The glow, light streaks and bleach run **before** FSR at the render size, and
only the HUD after. Reasons: AMD's FSR 1 integration places EASU after post-processing in perceptual space;
Overbright builds the glow at the size the original builds it for the screen anyway
([docs/notes/graphics/ssaa.md "Post effects under SSAA and Render Scale"](../notes/graphics/ssaa.md#post-effects-under-ssaa-and-render-scale)); and a TAAU-style split would need single-sample versions of the world's
particle and glow pipelines for the late sprites (the scene keeps its MSAA under FSR). Particles and rain are
part of the upscaled image.

**Cost** (GPU busy ms/frame, `--bench explore 600` at night, mean of 2; Original = the original's 8x MSAA,
World Detail 5):

| | 1080p | 4K |
|---|---|---|
| Original (8x MSAA) | 0.72 | 2.39 |
| TAAU Quality 67 % | 0.63 (−0.09) | 1.45 (−0.94) |
| TAAU Balanced 58 % | 0.52 (−0.20) | 1.50 (−0.89) |
| TAAU Performance 50 % | 0.52 (−0.20) | 1.44 (−0.95) |
| FSR Quality 67 % | 0.57 (−0.15) | 1.56 (−0.83) |
| FSR Balanced 58 % | 0.53 (−0.19) | 1.45 (−0.94) |
| FSR Performance 50 % | 0.47 (−0.25) | 1.32 (−1.07) |

FSR's own two passes (GPU timestamps, `graphics: fsr easu / rcas`): at 4K 0.30–0.31 ms EASU + 0.10 ms RCAS,
at 1080p 0.09 + 0.04 ms (per frame, mean). FSR keeps the 8x MSAA at the lower size, which TAAU drops; at 4K
the two cost the same within noise (other jobs' GPU work: ±0.1 ms). The Graphics row's cost classes use the
4K differences (`FSR_COSTS_4K`).

**Picture:** compared at racing frame 200 in 1080p (Original, TAAU 67 / 58 / 50 % against Original, FSR 67 /
58 / 50 %) and at 4K (Original, TAAU 67 %, FSR 67 %, FSR 50 %). The runs' cars stand a little apart, so the
frames are not pixel-aligned. In motion TAAU smears the moving cars (the player's number plate is unreadable
at every TAAU share); FSR keeps the plate legible down to 50 % and keeps the MSAA's clean edges; at 50 % FSR
shows the lower resolution in fine texture (road grain, distant signs).

### 3. Replay cameras, slow motion, cinematic depth of field

**The original** (traced earlier, [formats.md](../formats.md) "Replays"): the replay director 0x5FAFE0 /
0x5FB060 has index 0 automatic (the track cameras, mover 0x12, else a drawn chase view) and 1–5 the chase
views 0x802130 (2, 4, 1, 0, 5; bumper is one of them). Slow motion 0x4A4FB0 / 0x581060 holds three render
frames of four (0x865148 & 3: ¼ speed at 15 updates a second, choppy). These stay the default and the first
six cameras on C.

**Extended** (`replay_cams`):

- **Trackside:** a pole every 60 m along the race road (`RoadRoute`), 9 m to the side, alternating sides,
  2.2 m above the car; the next pole is placed 60 m ahead once the car is 22 m past. The field of view zooms
  with distance, 10°–50°.
- **Helicopter:** 22 m behind and to the side, 16 m up, slowly circling.
- **Wheel:** on the body beside the front left wheel, looking ahead.
- **Orbit:** 7 m away, circling with replay time.
- Each is a camera mover of its own (0x200 + index), so a switch is a cut with no interpolation or TAA history
  across it, and a "watched" camera for Cinematic depth of field (`blur_dof::dof_scene`, mover ≥ 0x100).
- **Smooth slow motion:** the replay's time runs at the chosen share, and every drawn frame is placed between
  the two recorded frames by the replay's own time (`slow_alpha`; the poses before the last played frame are
  kept, `Game::smooth_slow_update`).

Measured (`NFSU2_REPLAY_SMOOTH_PROBE=144`, 2 s of the window loop at 144 Hz, the player's drawn step per
frame): original 1× mean 0.154 m, sd 7 %; original slow motion mean 0.032 m, sd 220 %, 224 of 287 frames
standing still; smooth ½ 0.070 m, sd 6 %, none standing; smooth ¼ 0.033 m, sd 5 %, none standing.

### 4. Accessibility

- **HUD Colours** (`ui.wgsl` `palette`; `extras::palette_rgb` is the same on the CPU): applied to the in-game
  HUD's quads only (`ui::set_hud_palette` by index range), drawn by their own pipelines (`fs_palette`), so the
  original `fs_main` is untouched.
- **Per-element HUD scale** (`extras::apply_hud`, `qol::scale_hud_by`): the parts the existing HUD Scale finds
  (quads that touch, anchored to their third) get the HUD Scale times their element's row: minimap
  (bottom-left third), speedometer and tachometer (bottom right), race info (top half), following
  HUD_*_Track's layout as drawn.
- **Reduced Flashing:** a bright (luma > 0.5) quad over ≥ 90 % of the canvas is held to alpha 0.25, and the
  scenes' SCREEN_FLASH amount is held to 0.25 (`extras::FLASH_AMOUNT`; the tint pass brightens by × 1.25
  instead of × 1.99 on its first three frames, `screen_flash`). The original's full-screen flash is that event
  ([nis-mover.md](../original/nis-mover.md) "SCREEN_FLASH": slot 0 of the view's effect controller, drawn by the tint pass
  0x5C7870 as dest × up to 1.99 over six frames). The weather has no visual flash (thunder is sound only,
  0x4852A0).
- **Subtitles** (traced, [formats.md](../formats.md) "Subtitles"): the files exist (SUBTITLES\Scene01 …), but
  0x57DAB0 starts them only for regions 2 / 4 / 5 / 0xC (0x50D350), and this executable's region is the
  constant 6 (0x7FA73C), so the original never shows them.

### Checked against the original look

The comparison checks (`perf.sh parity`, against the build before these settings) with everything Off: the
Original pictures of the 5 bench scenes at 1280 × 960 and 3840 × 2160, the window path at 144 Hz, Modern and
Ultra racing and free roam, 15 shots: all identical (AE 0). Audio recordings identical. State hashes identical
over firstrace, firstdrag, postrace, the playthrough (82 493 steps) and the parity scenario. A first run had
1–2 HUD pixels off by 1 at 4K because the UI shader's original entry had been wrapped for the palette; the
palette now has its own entry and pipelines, `fs_main` is the original text, and the second run was identical.

### Tests

`latency::tests` (6: model, pacer, VRR cap, ioctl request sizes), `replay_cams::tests` (3), `extras::tests`
(3: round trip, palettes, elements and flashes), `render::fsr::tests` (2; the GPU one ignored: flat stays
flat, edges bounded), `render::ui::palette_tests`, the `qol` round trip (57 deviations), the `options` row
order and labels, the game test `replay_extended_cameras_follow_the_native_six`.

## Open

- VRR detection on Windows and macOS (no API probed). The present time is inferred from the frame's end. The
  pacer was measured on a display model, not a real panel; a check on a real FIFO or VRR panel is still to do.
- FSR 1's cost on an integrated GPU was not measured.
- The extra cameras have nothing in the original to compare with; the wheel camera's offset is the same for
  every car.
- Reduced Flashing does not cap Take Picture's flash (0x501250 → 0x5DBD80(1), the same slot; not ported).
- The palette uses fixed hue ranges, so HUD art with baked colours outside them (for example the orange
  destination mark) stays as it is.
