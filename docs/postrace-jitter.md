# Post-race animation jitter (2026-11-04)

Player report: "post race
animation still jitters". User display settings: 3840×2160 full screen,
VSync off (Mailbox), monitors at 59.98 / 74.96 Hz.

## Reproduction (`--flow postrace`, `src/bin/game/post_race.rs`)

A race (start scene skipped) raced on the player's autopilot to the line,
then recorded frame by frame as the window drives it (`Game::advance`):
the 3 s race-over hold, the switch to the ending scene (EndingNis*,
`play_race_ending`) and 8 s of the scene behind Event Results, at 60, 75,
144 Hz and uncapped (1–4 ms frames). Per drawn frame: interval, updates,
alpha, scene cap, scene frame, the drawn world time, the drawn car and
camera (as `draw_between` blends them), the camera mover
(`NFSU2_POSTRACE_CSV=<dir>`). Checks: (1) drawn world time vs real time
per frame (≤ 15 %); (2) a display model: each vblank (the run's rate; 59.98
and 74.96 Hz for uncapped, Mailbox) shows the newest frame done 1 ms after
its start, and ≤ 2 % of vblanks may repeat the last state; (3) no scene
update under the limiter's 1/30 s; (4) no drawn frame where the ending car
stands between two frames that move it; (5) 29–31.5 scene updates a
second. `NFSU2_FLOW_LOAD=a community save` (save in NFSU2_FLOW_DATA) races the save's
first open circuit/sprint and drag; default: a fresh career's first race.

## Finding 1 (main cause, the player's save): the ending car stalls every third update

Measured on a community save, event 81 (circuit) and 197 (drag), main as of
18:15 (with `Cadence::hold`): the moving ending car stood still in 60–69
drawn frames of 8 s at 60 / 75 Hz / uncapped, then leapt (speed per frame
0.12, 0, 2.33, 0, 1.64, 1.55, 0, 5.41, 2.51, 0 …,
internal evidence: the community-save ending speed log).
Cause: a scene frame (≥ 25 ms, `SLOW_FRAME`) was one update of its own
measured length through `FrameClock::frame` **without** the scene limiter's
floor. With 1/30 s frames the 1/4000 s carry gives 60, 60, 60, 60, 61, 59
ticks (78–80 updates under 60 ticks in 8 s). The NIS mover only steps on a
whole scene frame (`drive::nis::SCENE_FRAME`, `elapsed ≥ 1/30 × 0.999`,
`scene_mover_step`), so it skipped every 59-tick update and stepped the
next one with two frames' time. Native (verified earlier, frame_clock.rs /
docs/nis-audio-patterns.md): the limiter 0x435F40 sets 0x865558 = 1/30, so
0x57EAE0's `V = max(raw + carry, 1/30)` never goes under 60 ticks; every
scene clock captured under Wine runs 60, 60, 60, 60, 61. Not caused by
`Cadence::hold` (present before and after it); the slow-frame path is the first-race regression fix's.

## Finding 2: the 30 fps render judders on 75 / 144 Hz

Scene frames were held to 1/30 s (`display::frame_period`). On the
display model 50 % (60 Hz), 60 % (75 Hz) and 79 % (144 Hz) of vblanks
repeated the last frame, unevenly at 75 / 144 Hz (2–3 / 4–5 vblanks per
frame: ±20–25 % motion error per shown frame); at 60 Hz any timer/GPU
phase near a vblank flips frames between 1 and 3 vblanks.

## Finding 3: smaller pacing faults found on the way

* The race → ending switch frame was blended part-way between the chase
  camera and the scene's when they stood under 30 m apart (144 Hz, fresh
  career: 8 % of the way, then a 6 m jump): a scene camera is a mover of
  its own now, and each shot of its track (`track_camera::shot`, entries
  that do not chain) too, so cuts are never blended.
* 75 Hz (race and hold): the 60 Hz `SNAP` applied from ¾ of an update
  (80 Hz) took every update 2 ms early at 75 Hz; the drawn time stood
  for a frame then jumped: ±18 % per frame. The snap now applies only to
  frames of 1/60 s − 2 ms or longer (≤ 68 Hz).

## Fix

* `frame_clock::Rate` / `Cadence::frame_at`: `RACE` (unchanged: 1/60 s,
  30/29/31 ticks) and `SCENE` (one update per 1/30 s of real time,
  `frame_limited(RAW_30, 1/30)`: 60, 60, 60, 60, 61; a frame ≥ 50 ms is one
  update of its length floored at 1/30 s). `Game::advance` takes `SCENE`
  while `scene_frame_cap()` (a scene that holds the cars).
* `Game::redraw` no longer waits for 1/30 s in scenes (`NFSU2_FRAME_CAP`
  still applies); frames between scene updates are interpolated like the
  race's (cars, camera, and the scene characters' frame).
  **Note for review / user**: this changes the 2026-10-31 rule "cut-scenes
  keep the native 1/30 s" from a render cap to the world cadence: the
  simulation still runs the original's 30 updates a second with its ticks;
  only the drawing is at the display's rate (as decided 2026-11-04 for
  60 fps). `--bench scene` still measures the old limiter path.
* `Game::camera_mover` (scene shots), `first_race::interval` (no 1/30 s
  cap), `SNAP_FRAME`.

After (`flow-before-after.txt`): every run 0 repeated vblanks, 0 short
updates, 0 stalls, scene 30.1 updates/s, drawn time within 1.7 % (scene)
and 3.3 % (race, the 29/30/31 pattern) of real time; world updates at all
rates identical (frame_clock tests).

## Open

* Ending car first frame: `play_race_ending` steps the scene with dt 0, so
  the NIS mover's first real step pulls the car 1.7–6 m to its channel in
  one frame (nis-mover.md; native placement at 0x4309D0 not re-traced).
* a community save event 81's ending camera drops 2 m within two updates at scene frame
  ~152 (same chained shot; the scene camera's ground step, before and
  after); the drag's race-over camera jumps ~11 m once at 0.87 s past the
  line without a mover change (blended over one update at high rates).
  Neither traced here.
* `firstdrag` (default seed 4) and its test now fail on the restart run:
  rival MIATA rolls 30.4° (limit 30°) after a rival–rival hit (106) at
  29.5 s, identically at 60 / 144 Hz / uncapped. The start scene's ticks
  (now the native 60/61) shift the race; main had the same MIATA–MIATA hit
  at 144 Hz (120, a drop-out). Over seeds 1–8 at 60 Hz: main 1 failure in
  16 runs (seed 3, player did not finish), this work 1 in 16 (seed 4).
* The original's post-race pacing under Wine was not captured here (the
  limiter and its ticks are from the earlier traces cited above).
