# Replay cameras against the original (2026-11-05)

The automatic replay camera (index 0 of the race camera director 0x8900B4) traced in
SPEED2.EXE and compared with the original's replay under Wine. **Verified** = traced in the
disassembly / Ghidra, **observed** = read under Wine, **inferred** / **open** as marked.
The rest of the replay (manager, markers, keys, route-camera motion and visibility) is in
formats.md "Replays".

Evidence: `docs/evidence/replay-cameras/` (the original's director per frame and its shots
by group, the clone's shots before and after, frame sheets of both). Raw run:
`<work dir>/p7-work/runs/replay1/` (Wine copy `a private Wine copy`, display :288,
wined3d; Quick Race circuit 4014 from `stage1_win_streetx.sav`, the Peugeot driven 60 s by
`bot.py`, then Pause → Replay → `[` to the green light; `dirlog.py` polls the director).

## What the original does (observed)

* The replay is mostly **typed track cameras**, not route cameras: 3 s shots from the six
  typed groups of `GLOBAL/InGameCommon.bun` (chunk 0x8003B202, camera manager +0x14). In
  the 4014 replay: Speed 32.7 s, chase 14.3 s, Corner 5.3 s, Burnout 4.2 s, PowerSlide
  0.65 s; no route camera (L4RA's chunk 0x8003B200) at all. Each typed shot lasted exactly
  its track's 3.0 s of replay time (sim clock 20.43 → 23.46 → 26.46 → 29.49 s), then the
  next was drawn on the same frame (shot after shot while the car is fast).
* From the green light (speed under 10 m/s) the chase view held 8.7 s until the car passed
  10 m/s (the Speed weight); after the Burnout shot ended a chase view was drawn at once and
  held 5.5 s (the car slow).
* The type table 0x7EC1F8 is filled at runtime (read under Wine: NOS 0xC9EF 0x441AF0,
  Jump 0x187BBB 0x441B40, Speed 0x3C7EEB0 0x441BF0, Corner 0x576869E8 0x441C80 / flag
  0x441D00, Burnout 0x57A894E 0x441D30, PowerSlide 0x81409F9D 0x441E90 / flag 0x441F40).
  formats.md's older "only NOS filled in the file" read the static image; it is corrected.

## The director (0x5FB060, RCSTATE 5, index 0; verified)

* **+0x214** is stamped (real tick 0x8651A8) by every mover creation (0x5F1FE0, unless the
  same non-track type is asked again), by a chase draw, and by the reset 0x5FAF90 (every
  snapshot restore: the replay's start and each skip; it also drops the track, +0x220 = 0,
  goes back to the chase mover and sets **+0x224 = 1**). +0x224 is cleared when a track
  camera starts (0x5FB5BD). 0x5EA160 = real seconds since +0x214 (0 when unset), 0x401820
  "+0x214 set" (not 0, not 0x7FFFFFFF). **Observed**: +0x214 changed at every cut.
* On a **chase mover** with 1.5 s (0x802148) < t < 4 s: the frame ends (hold).
* bVar5: unless +0x214 is set, +0x224 clear and t < 1.5 s. With no track held a track is
  drawn only with bVar5; a held track whose entry at the fraction is type 0 (or whose
  fraction left 0..1) is drawn again whatever bVar5; with bVar5 the typed mover's +0x84 or
  0x441B40 > 0 or car +0xB8 < 0 also redraw.
* The draw (0x44FE80 → 0x44F9A0): the route group first; then the typed groups (below).
  A drawn entry of type > 1 with 0x8900C4 = 0 and the car's +0x430 ≥ 0 becomes the camera
  (mover 0x12, the track into the ring 0x828774 of three). Otherwise +0x220 = 0 and, if a
  track was showing, back to the chase mover (0x5FA830) and **a chase view drawn at once**
  over all five (weights 1, 0x7A553C), +0x214 stamped.
* On a chase mover with no track: +0x218 ≠ 0 (index just back to 0) draws a view other
  than the one showing; else with car +0x430 ≥ 10, +0x214 set and t > 4 s, chance 0.3 a
  frame.

## The typed draw (0x44F9A0's second half; verified)

Each typed group's weight is its type's weight fn on the car (≤ 0.1 none); the total sums
every group's weight. A group with weight gives its tracks whose first entry is a camera
(type ≠ 0); under weight 10 a track must also pass 0x448280 at fraction 0 (with the group's
mirror flag) and not be in the ring of the last three; the group's weight is shared among
its candidates. r = random(total) walks the candidates (the first whose share exceeds what
is left); a walk that runs out draws nothing. The flag fn's result (0x828649) mirrors the
entries' y (0x447A30 / 0x447A80) for the shot. A typed track's fraction is the time since it
began over its duration (0x442B70).

Weights (0x441AF0..0x441F3A, constants read under Wine):

| type | weight | flag |
|---|---|---|
| NOS | 0.5 + 0.25 × tank fraction (0x5925C0) while a shot runs (0x59FA40) | — |
| Jump | 5 when car +0xB8 < 0 (the up row's z: upside down, **inferred** from the matrix layout, 0.9998 read on the road) or 0x5ED230 and the flight prediction 0x5EC3E0(3.0) gives height > 1 and airtime > 1.5 | — |
| Speed | car +0x42C > 10: 0.3 + 0.7 × +0x42C / top, at most 1; top = [car +0x10] +0x20 (the gearing top speed, 0x5B5AC0's +0x10; 73.76 m/s for the Peugeot), else 60 | — |
| Corner | car +0x42C > 5 and car +0x18: 20 × \|0x5EB320\|, at most 1 (0x5EB320: the signed curvature 0x5D4780 of the last three points of the ring at camera +0x264) | curvature < 0 |
| Burnout | 0x441D30 (**verified** in the gap sweep, constants read from the image): with the forward speed +0x42C within −10..20 (0x785BAC / 0x7850D8), clamp(Σ wheels' longitudinal slip +0x3B4 + 8i × 0.1 − 1, 0, 1); plus 10 × 1.1 / (1 + 1e-6) (0x786FB0, 0x787598, 0x787594) when the forward speed is also within −1..1 and the rpm is over 3000 (0x78759C; the rpm is 0x7FBED8 unless that is 100 — it is in the image — then car +0x440) | — |
| PowerSlide | car +0x430 > 4: (\|slip\| − 5°) × 1.4324 + 0.1 within 0..1 (slip 0x59FA20 → 0x591DA0) | slip < 0 |

## The clone (src/bin/game.rs `ReplayCamera`, `replay_typed_input`; src/track_camera.rs)

**Before**: the route cameras only, drawn at any frame a chase view showed (no 1.5 / 4 s
holds, no reset flag), a chase view drawn only by the 4 s chance (none at a track's end).
On the bench race (STAGE_1_CIRCUIT_1, 4013) the replay was chase views only
(`clone-shots-before-typed.txt`): L4RA's route groups cover 1–2 % of 4013's race records
(`devprobes/src/bin/replay_camera_coverage`; 4014: 5–10 %).

**Now**: the director's gate (`ReplayCamera::gate`: the 1.5–4 s hold, bVar5 with +0x224),
the reset at every restore (`reset`: +0x214, +0x224, the shots dropped), +0x214 stamped at
a track's start (`track_started`), the forced chase draw at a track's end, the forward-speed
test, the chase draws on the forward speed (+0x430 was the velocity's length), the cycle no
longer clearing +0x214; the typed groups (`track_camera::draw_typed`, `follow_typed`,
`typed_weight`, `typed_flag`, the ring of three) after the route draw. Same bench replay
after (`clone-shots-after.txt`): chase 4.5 s from the green, then 3 s Speed shots back to
back with a Corner shot and one 4 s chase view — the original's pattern.

**2026-11-05**: Burnout's weight ported (`typed_weight`, `TypedInput::wheel_slip` /
`rpm`); the typed weights read the exported forward speed (car +0x42C, `exported_forward_speed`,
not the velocity along the heading); a held track (route or typed) is drawn again under
bVar5 while the car is upside down (0x5FB060: 0x441B40 > 0 or car +0xB8 < 0). Replay bench
(`--bench replay 3600`, 30 s replayed) unchanged shot for shot
(internal evidence: p8-gaps/clone-replay-shots-p8.txt vs `clone-shots-after.txt`: the bench car
never slows under 20 m/s). **Still open**: Jump's flight prediction 0x5EC3E0 (traced in outline:
it steps the car's four wheel points (+0x130 + 0x40i, the set-up's +0x20 + 0x30i) along the velocity
+0x70 with gravity 0x7FBEF0 plus the downforce term ((set-up +0x264 + +0x268) × 0.3 × speed /
+0x110) against the road height 0x5E72F0, in ftol(3.0 × rate) steps — the rate is the FPU value
0x75BED4 converts, not traced), the typed mover's +0x84 request and pushes (0x444530 / 0x44B040),
the generic groups and +0x1C34, the route group key.

**Inferred** in the clone: the Corner curvature from the velocity's turn over the frame
(the ring +0x264's writer and cadence are not traced; for a car on a circle any three points
give its radius); the Speed top from `launch_times` (0x5B06B0's port, as the belt reads it);
the replay clock as the typed fraction's time. **Open**: Jump's
flight prediction (only upside down counts), the typed mover's pushes (0x444530 /
0x44B040) and its +0x84 re-draw, the airborne re-draw's flight part, the
generic groups and the camera manager's fixed track (+0x1C34), and the route group key
(0x5EC0B0's record +0x0A, compared by the clone as the AI route record id). The random
draws use the clone's own random state, so the shot sequence is not the original's frame
for frame; the grammar (holds, chains, forced draws, weights) is.

Tests: `tests::replay_chase_views_hold_before_a_track_camera`,
`tests::replay_camera_cycles_and_picks` (game.rs), `track_camera::typed_tests::*`.
Check aid: `NFSU2_REPLAY_CAMLOG=file` (one line per playback frame) with `--bench replay`
(`NFSU2_REPLAY_FROM=post`).

The opt-in "Replay Cameras: Extended" is unchanged: its extras come after the six
native indices and only on index 6 on; default off (`replay_extended_cameras_follow_the_native_six`).
