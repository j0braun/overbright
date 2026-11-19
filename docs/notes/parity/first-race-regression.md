# First career race: rivals cutting across, animation stutter (2026-11-04)

Issue: in the first career race the animation stuttered, and two of the three
opponents drove left and crashed into the player's car (after a restart, into
the other racer).

## Reproduction (`--flow firstrace`, `src/bin/game/dev/first_race.rs`)

A fresh profile past the intro (`skip_intro`, Peugeot), stage 0's first race
(`STAGE0_RACES[0]`), its start scene played through (not skipped), driven as
the window drives it: `Game::advance(interval)` at 1/Hz (since the post-race jitter fix a scene
that holds the cars updates the world at 1/30 s and is drawn in between,
docs/notes/front-end/postrace-jitter.md; before, the scene limiter's 1/30 s frames) (`NFSU2_FIRSTRACE_HZ`, default
60,144,240 and uncapped = 1–4 ms jittered frames), throttle held from the
green; then the pause's Restart and the same again. Per rival: its heading off
the race road's direction while it drives the racing line (JustDrive: once it
takes an avoidance or is hit, its path is that action's and the heading is no
longer checked; the original proposes such actions at the human from the
start too, 8 and 71 frames past the green of STAGE_1_CIRCUIT_1 in the sequence
log, docs/original/rng.md, with the traffic hundreds of metres away), its offset off the
racing line, the smallest gap
between any two cars (first 6 s after the green); per drawn frame: the
player's drawn position (the `draw_between` blend) against speed × interval.
`NFSU2_FIRSTRACE_LOG=1` prints a row per world tick (each rival's road
lateral, heading, steer, pedals, speed, NIS count), `NFSU2_FIRSTRACE_SKIP=1`
skips the scene with Enter, `NFSU2_FIRSTRACE_PACING=1` prints each frame's
interval / alpha / accumulator / drawn step ratio.

## Finding 1: rivals' path zig-zagged onto the racing line (not a regression of the listed commits)

Path dump at the green (grid slots 3 m apart, racing line 1.8–4.6 m off the
cars): the path points alternated between the car's own lateral and the racing
line every 8 m, e.g. the right-hand rival (+4.6 m off the line): (13.6, 1.8)
(21.2, 7.2) (35.2, 5.8) (42.1, 10.0) … (m ahead, m left). Every point inserted
by 0x418DA0 (every 8 m, inside records too since an earlier fix) was drawn at its
smoothed racing-line place (`FuturePoint::inserted`) without the lateral, so
the driver steered at the line: the rival turned 16° within 2 s and crossed
the player's lane ~6 m ahead of the player (closer with a quicker launch);
on Restart two rivals came within 1.66 m (contact).

Native (verified, disassembly): `RacerRoute::line_path` is 0x41D530's loop,
whose point is 0x415D50(path, lane, lateral, point) → 0x4122C0 on the
FuturePoint: control point geometry at its offset (+0x58 − +0x54 + s), lane
position (0x401C40) + adjust (+0x04) + the lateral, then vt+0x38. 0x4122C0 does
not read the +0x74 inserted flag. Fix: every path point is `line_point(…,
lateral)` (`src/ai/route/`); the same fix was made independently by the outrun-strength
outrun unit (`inserted_points_keep_the_lateral_on_the_path`,
docs/notes/outrun/ai-strength.md); this copy carries the identical code/comment.

Bisection (same probe built into each revision, 60 Hz): 3348acb (the merged branch
baseline, 2026-11-02), 5ca9380, 1a5ea61, 3feac52 and d6c3783 all give the same
rival paths (rival 3 lateral 8.7–12.8 m, min gap 1.66–2.1 m). None of the
listed candidates (a33d186, cc2245c, 3feac52, bf5969b, d6c3783) changed it;
it predates 2026-11-02. With the scene skipped (Enter) it is the same: the NIS
mover hand-off is not involved. Whether it showed less before that date is **open**
(the runs differ: car, launch, refresh rate).

## Finding 2: interpolation stutter at high / uncapped rates (regression, a33d186)

`frame_clock::Cadence` takes an update up to `SNAP` (2 ms) early so a 60 Hz
display's jitter does not alternate 0 and 2 updates. At a high rate that
early update leaves the accumulator negative; the drawn fraction is clamped
to 0 for that frame and the next, so the cars are drawn standing for one or
two frames and then jump, at every update (60 times a second). Measured (drawn
motion ÷ speed × interval, worst after 1 s): 144 Hz 24 % off, uncapped 180 %
(a frame of 0, then 1.5–2.4×); 5ca9380 (before the cadence) 5 % / 48 %
(variable dt). Fix: the snap applies only to frames of ¾ of an update or
longer (≤ 80 Hz). Also: the first fast frame after a slow one (≥ 25 ms, e.g.
the race-start scene's 1/30 s frames ending at the green, or a hitch) was
drawn between the state *before* the slow update and the one after it, a step
back of up to 1/30 s of motion; it now continues from the slow frame's state
(`Cadence::after_slow`). After: 0.05 at every rate (the 29/30/31-tick pattern
alone is ±3 %). World updates are unchanged (same dts at every rate; tests
`every_fast_display_steps_the_60_fps_world`, new `fast_frames_draw_steadily`,
`fast_frames_go_on_from_a_slow_one`).

## Checked and not the cause

* World simulation vs display rate: identical rival paths at 60/144/240 Hz
  and uncapped (the cadence works); 30 Hz differs slightly (native 1/30 steps).
* NIS mover hand-off: rivals end the scene 0.2–0.8 m from their grid slots;
  skipping the scene gives the same paths.
* `draw_between` restores every car's pose after the draw (pure).

## Open

* Two rivals still come within 1.64 m of each other after a restart (second
  grid of the run, before and after the fix); the Avoid action's inputs are
  not traced here (the outrun code owns it). Reported by the flow, not failed.

* The original's first career race under Wine was not captured (no save at
  that point); the paths now follow the traced 0x4122C0 construction.
* The rivals still rejoin the racing line from their grid slots (native
  JustDrive rate 0.05 m/m) and may Avoid each other; contact with a player who
  steers into their line is possible natively.
