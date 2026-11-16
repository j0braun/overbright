# World systems against the original (2026-11-04, 2026-11-05)

Traffic, collisions, cameras, audio mix and saves compared with SPEED2.EXE under Wine
through the parity harness (docs/parity-harness.md). **Verified** = traced in the
disassembly, **observed** = read under Wine, **inferred** / **open** as marked. Raw runs:
`<work dir>/world-work/runs/<name>/` (Wine copy `a private Wine copy`, display :223,
wined3d); reports, car lists, statistics in the maintainer's private captures. The audio and camera parity: raw runs in
`<work dir>/p5b-work/runs/<name>/` (Wine copy `a private Wine copy`, display :233,
wined3d), tables and frames in the maintainer's private captures, harness additions in
docs/parity-harness.md.

## Harness additions

* **Where the traffic lives** (observed, free roam and the stage-1 circuit): in the same
  world list as the racers, `[0x890080] +0x1C + 4·i`, at slots +0x18 .. +0x10 (+0x10 the
  total count, +0x14 the humans, +0x18 humans + racers). Free roam: 20 = 1 + 3 encounter
  racers + 16 traffic; circuit 4013: 16 = 1 + 3 racers + 12 traffic. Traffic cars have
  car +0x24 = 4 and controller type ([car+0x14]+4) 3; the racers +0x24 = 3, type 2.
  the parity harness's harness read only +0x18 cars.
* `[run] traffic = true` records the pool too (fields with `traffic = true`: pos, vel, fwd,
  speed, and the traffic-only `mover` car +0x28, `traffic_action` [[car+0x2C]+0x6F8]+0x204,
  `parked` [car+0x2C]+0x77C — the byte 0x4112A0 reads, verified — `drawn` car +0x550).
  diff.py leaves traffic out of the car match (`clone.tsv.traffic` lists the clone's slots).
* `$OB_PRIVATE/tools/parity/trafficstats.py OUT`: pool statistics per side (placed, within 50–250 m,
  the jam share of 0x4112A0, placements per minute and where, speeds, mover and action
  shares). `$OB_PRIVATE/devprobes/src/bin/traffic_lane_offsets OUT`: each traffic sample's signed distance from
  the nearest lane centre line running its way. `$OB_PRIVATE/devprobes/src/bin/traffic_lanes_near X Y R`.
* `[start] kind = "free_roam"`: the save resumed to free roam, the cave holds at once
  (trigger: the traffic level 0x89E7A8 as it is), arms on the next frame; the clone puts its
  player on the original's frame-0 pose (kept when within 1 m / 2.6°). `place` (a teleport
  while held) froze the original's car for a second and then reset it to a road spot: not
  usable at a standstill.
* Camera fields (global, view 1 = 0x8760E0): `cam_eye` +0x40, `cam_fwd` +0x50, `cam_fov`
  +0xC4 (binary angle); `$OB_PRIVATE/tools/parity/camstats.py OUT` prints the eye relative to the car.
* Native AI fields are not recorded for free-roam encounter racers that are not out: their
  [car+0x2C]+0x6F8 read 1.0f and faulted the cave (page fault at 0x782C4C).

Scenarios: `freeroam_stand` (90 s standing at the garage exit, stage 1, level 3),
`circuit_traffic` (STAGE_1_CIRCUIT_1 with its traffic, 60 s, throttle only), `crash_wall`
(free roam, full throttle with right steering for 1.5 s into a wall at ~15 m/s).
In `circuit_traffic` the player's car meets the clone's own traffic, which is not replayed from
the original, so its end distance stops being a valid measure once it touches a traffic car the
original did not have there (with the hull query's support offset the car stays on the original's
spot until frame ~1975, touches such a car at ~2020 and stops; docs/prop-hits.md §7a). The
first-half mean distance from the original's car is the measure: 3.2 m before the support offset,
1.7–2.0 m with it.

## 1. Traffic

**Found and fixed: the path's front is kept between thinks.** 0x420D40 (verified, Ghidra)
plans afresh (0x41D160(0, 0)) only when 0x41BFE0 says so; otherwise, when the driver's
byte +0x1D9 is set, it drops the path points behind the car along its heading
(0x404EF0 / 0x40F7A0), cuts the path after the first point max(30, |v| / 2) m on and lets
0x41D530 extend it from the last point. +0x1D9 read **1 for all 16 traffic cars** under
Wine (car +0x30 → +0x1D9; the player 0). The clone rebuilt the traffic path every think
from the point after the car's nearest, so a car 20 m short of a control point already
steered for the next one: on record 538 (the street north of the garage exit) its cars
ran 2.5–4 m inside the original's lanes, oncoming cars met 2.0–2.6 m apart (the
original's: 3.3–4.3 m), took each other for the car ahead ("Avoid Cars", 0 within 15 m)
and stood for up to 20 s; a standing placed car is 0x4112A0's jam, which stops the placer
in free roam. Now `TrafficLane::update` keeps the path as 0x420D40 does (the 1.5 m
re-plan test as the racers' `line_path`, 2D distance: **inferred**; a lane change plans
afresh: **inferred**). Test: `ai::traffic::tests::traffic_path_keeps_its_front_until_passed`.

| freeroam_stand, 20–90 s | original | clone before | clone after |
|---|---|---|---|
| placed (of 16) | 15.0 | 11.3 | 14.5 |
| within 50 / 100 / 250 m | 2.5 / 4.6 / 11.8 | 1.7 / 4.7 / 8.6 | 2.0 / 4.3 / 10.3 |
| jam share (0x4112A0) | 0.20 | 0.58 | 0.26 |
| placements per minute | 44.6 | 25.7 | 41.1 |
| put at (p10/50/90 m) | 253 / 276 / 306 | 253 / 276 / 327 | 252 / 276 / 315 |
| speed on the TrafficMover (p10/50/90) | 7.9 / 12.9 / 20.4 | 7.9 / 16.1 / 20.4 | 7.8 / 14.8 / 20.4 |
| off the lane, TrafficMover p5 / p95 (m) | −0.24 / +0.51 | −1.83 / +0.67 | −0.28 / +0.35 |
| over 1 m off the lane | 7 % | 17 % | 2 % |
| actions 2 / 7 / 17 | .45 / .26 / .29 | .46 / .34 / .20 | .43 / .25 / .32 |

Race traffic (circuit_traffic, 5–60 s, after): pool 12 / 12, placed 7.7 / 6.9, within
250 m 3.5 / 3.1, placements 26.2 / 25.1 per minute, put at 265/347/456 vs 269/338/456 m,
83 % / 70 % put ahead, jam share 0.34 / 0.49, TrafficMover speed p90 20.4 / 16.9.

**Open (traffic; the traffic-mover parity §8 traced and ported the walk, the lane window and the BeltMover):** the MeloMvr share near the player (0.86 native, 0.64 clone): the
original's cars switch to the MeloMvr (a new "Avoid Cars", 0x42A013) mostly right after
placement at 240–300 m (43 of 2→3 switches), the clone's less often; the placed car's first
think and its first road entries are the suspects (not traced). The race's jam share and
top lane speeds. Reactions to the player driving (horn, swerving) were not compared: the
scenarios have the player standing or throttle-only.

Traffic Density (opt-in, docs/assets-distance.md) scales only free roam's pool
(× 1.5 / × 2, at most 32); the placer, jam rule and races stay as above.

## 2. Collisions (crash_wall)

**The response matches; the slide is the approach.** After the tuning-physics parity's centre-of-mass
push-out the clone still slides (−105° → −126°, 1.8–2 m/s along the wall). With the
original's state at frame 190 (0.07 s before the hit: centre of mass, velocity, forward;
`[run] transplant = 190`, scenario `crash_wall_tp`) put on the clone's car, the clone hits at
the same frame and spot and stops dead like the original: 15.0 → 0.47 m/s (horizontal) in
two frames vs 14.9 → 0.48, at rest at 455.68 / −69.63 vs 455.73 / −69.65 (0.05 m), the eye
height within 0.02 m; the heading drifts −111.7° → −113.2° in 0.8 s where the original's
stays (1.5°, **open**, small). So the 20° yaw came from the 6.5° shallower incidence, i.e.
from the turn before the wall (the tuning-physics parity's handling; the tuning sweep's area, not changed here).
Frames: internal evidence.

Both cars take the same line (within 0.3 m for 1.7 s), but the original yaws 6.5° further
in the turn (heading −111.7° vs −105.2° on the straight) and hits the wall 2.3 m further
west. Original: 14.9 → 0.8 m/s in two frames, no rebound, heading unchanged, at rest 0.1 s
later. Clone: 14.8 → 0.3 m/s over three frames, then slides along the wall at 1.8–2 m/s and
yaws 20° (−105° → −125°) in 0.7 s. The incidence differs by 6.5°, so the spin is not
attributed yet (**open**; the turn rate itself is the tuning-physics parity's physics). The original's inputs
reach the pedals 3 frames later than the clone's from the same frame-0 events (gas at
frame 4; harness or native latency **open**), and its resumed car stands in first gear
(gear 2) where the clone's is in neutral. Damage / total thresholds were not reached.

## 3. Cameras

**Found and fixed: the chase frame's pitch.** 0x445C20 (verified) fades the chase
direction's vertical part by 1 − (0x865188 − mover +0x58) × 0.00025 / 2; 0x44DE90 restamps
+0x58 with the sim clock after that call whenever car +0x5CC is set and car +0x424 > 1
(0x44DF2B..0x44DF41, verified; also the ctor 0x4516D5). Under Wine +0x58 equals 0x865188
frame after frame in free roam (`[[0x8900B4]+0x238]`, vtable 0x787F28), and view 1's
forward z follows the car's pitch for minutes. The clone faded it over the camera's first
2 s, so its frame stayed level: cam_fwd z 0.000 always, the original's −0.06 … +0.11 on the
circuit. Now `ChaseCamera` keeps a ground stamp (`since_ground`); circuit_traffic cam_fwd z
error median 0.003 (p90 0.021). Test: `chase::tests::the_chase_frame_keeps_its_pitch_while_wheels_are_down`.

**Every player view and the look-back compared and fixed.** The scenario
`audio_freeroam` (idle 6 s, throttle 4 s, brake 2 s, look back 15–17 s) ran in the original
once per camera setting (`camera_bumper|hood|near|high`, the profile's setting patched,
`[start] camera`). Per frame, eye relative to the centre of mass in the car's frame
(internal evidence), clone after the fixes:

| setting (kind) | eye at rest, original / clone | worst while driving | look back |
|---|---|---|---|
| 0 bumper (2) | 0.73 / 0.72 ahead, 0.79 up, pitch −2.72° both | 0.02 m, 0.05° | −1.30 / −1.31, 0.80 up |
| 1 hood (4) | 0.51 / 0.50, 0.90 up, 90° both | 0.01 m, 0.05° | −1.09 / −1.09, 0.91 |
| 2 near (1) | −4.99 / −5.00, 1.40 up | 0.05 m, 0.6° yaw | 4.41 / 4.41, 1.37 / 1.36 |
| 3 far (0) | −6.09 / −6.10, 1.40 up | 0.04 m, 0.3° yaw | 5.51 / 5.51, 1.36 / 1.35 |
| 4 Super Far | far's −6.09, 1.40 in both (was kind 5, 3.15 m up) | as far | as far |

* **Look back (fixed, verified).** With mover +0x51 set, 0x445EB0 takes 0x7EC400's row
  (−17.2, 0, 6) only while the player's +0x2B is clear (0x5EB530); 0x453570 multiplies the
  eased eye and look-at point by −1 (0x7842A0) only while it is set (0x4539A5..0x4539BD),
  and the 3 m wall radius (0x445A60) again needs it clear. The two are exclusive; the clone
  did both (glided out to 17 m ahead and 5.6 m up over 1.5 s). The player's +0x2B read 1
  under Wine in free roam and in a race (its writer is not found: **open**), and the
  original looked back from the mirror of its own eye about the car's origin in the first
  frame, no ease, in all four views. `ChaseCamera` now mirrors the view's own entry
  (`LOOK_BACK_TABLE = false`). Test `chase::tests::look_back_mirrors_the_views_own_eye_at_once`.
* **Super Far (fixed, verified).** Setting 4 (kind 5) is a drift-race view: the Quick Race
  drift set-up 0x4FF070 (0x4FF0CC) and the career's 0x5322F0 for an event with +0x0F = 5
  (0x532569) set 0x83AA74 = 4; the default, drag and other set-ups (0x4FEFE0, 0x4FF05F,
  0x4FF146, 0x4FF1BF, 0x53257E; free roam through 0x526980 → 0x4FF160) turn a 4 back into 3.
  The camera button cycles 0–3 outside a drift (0x4B7765). The clone kept a saved 4 in free
  roam (kind 5) and never forced it in drift races. Now `Settings::race_camera` at each race
  set-up and free-roam entry. Test `options::tests::drift_races_take_super_far_and_the_rest_give_it_back`.
* **Eye height with acceleration (open item): resolved.** With this work's chase-pitch fix the
  eye's height above the centre of mass follows the original's within 0.01–0.02 m through
  launches, braking, the wall hit and the circuit (1.12–1.60 m), so the eye lag's input (car
  +0x80) behaves as the clone's acceleration; its writer 0x43C9C0 stays untraced.
* Drift and drag cameras: `drift_s1` / `drag_s1` with the camera fields (see below);
  replay cameras: [replay-cameras.md](replay-cameras.md) (the director's holds and the typed
  track cameras ported).

Compared per frame (camstats): the near chase distance matches within 0.05–0.1 m while
the cars match (6.1–6.8 m behind), field of view 71.5° on both. **Open:** the eye height
above the car: the original's moves 1.19–1.77 m with acceleration and the road (1.30 at rest
against the wall, 1.24 on the grid), the clone's stays 1.38–1.43 (the eye lag reads car
+0x80, whose writer's transform 0x43C9C0 is not traced; formats.md, Chase camera). Frame 0
of a race start reads the clone's scene camera (the harness moves the cars after it).
Bumper / hood / far views and the look-back: The audio and camera parity above; replay cameras still not run.

## 4. Audio mix

**The audio and camera parity (2026-11-05).** Method: the parity harness with `[run] audio = true` (the original's
`--audio-file` WAV placed on the run's frames through the sim clock, every live AEMS
instance's block every ~40 ms; the clone's offline mix and NFSU2_EVENT_LOG for the same
frames; the maintainer's tool `audiomix.py`). Scenario `audio_freeroam` (the stage-1 save, the 206,
default volumes: master 1, speech 1, menu music 0.7, game music 1, effects 1, engine 1,
tires 1 — the player's a community save has exactly these; their the finished-career save has master 0.95, all else 1,
and the option volumes enter only through the mix map's fields, verified in sound2 round 8, so
it is the same mix 0.45 dB lower). Evidence: internal evidence.

* **Event blocks, per window (idle, throttle, brake, stand, look back): equal.** The player's
  CAR block (p1 rpm map, p3 throttle, p4 / p5 level −30791 idle, −30589 / −30661 throttle,
  p14 1624), FX_ROADNOISE left / right (class, level, pitch 1500–2100, azimuths 60423 /
  5112), FX_ROADNOISE_TRANS, FX_WIND (p6 52 / 130–137), FX_WIND_Weather (73 / 64–68),
  the player's two FX_SKID (2749 3883 2912 at 60423 / 5112), FX_SHIFTING_01 (140), CAR_Sputter
  (rpm / throttle) match within a few units; the azimuths turn by 180° while looking back in
  both. Small inaudible differences: CAR p17 150 vs 0 and p21 23920 vs 24000 (the graph does
  not read p17, p21 is a kind-6 send; **open**), CAR_Sputter p1 (a pointer-like 62912128 vs 1).
* **Turbo sound on a car without a turbo (fixed, verified).** The clone ran FX_TURBO_01
  (spool level 260 at full throttle) for the stock 206; the original has no instance.
  CARSFX_Turbo's reset 0x45EBC0 (vtable 0x78AC10 +0x1C) takes the largest point of the
  tuning's boost curve (tuning +0x1F0 → 0x457B00; record +0x334) / 0.25 and at 0 clears the
  object's +0xA0, so its update 0x47CAD0 never starts the event. `CarInput::turbo`
  (handling `curve_334` > 0); `PlayerSfx::turbo_off`.
* **FX_Scrape is audible in the original (found; fixed in the traffic-scrape parity, see below).** In the turn before the wall
  the original's two FX_Scrape instances carry {5812, 4096, 64584 / 64497, 0, 9…24} (p0
  5812 = −1501 mB, falling to 2906 in the 0.1 s fade); the clone's, at the same frames and
  with the same p4, carry {0, 0, 0, 0, p4}: silent. `audio::scrape`'s reading (0x46A460
  zeroes +0xD4 / +0xD8 / +0xDC and nothing writes them again) is refuted by the Wine blocks;
  the per-frame writer of +0xD4 / +0xD8 / +0xDC (a mix-map level −1501 mB, 0x1000 and the
  azimuth) is **not traced** (no mix map master has a −1500 base; the clone's mix keeps no
  copies for collision objects). Impact sounds (AEMS_StichCollision) are not in the clone's
  event log, so their levels were not compared.
* **The mix (the 3–5 dB was the harness, see below).** RMS per second, default volumes: the original −21.6 … −25.3 dBFS while
  idling and driving, the clone −17.3 … −20.5 (3–5 dB louder). The clone's parts (volume
  variants of the same run, `clone-volume-variants.txt`): its EA Trax song ≈ −18 dBFS and a
  Rachel line in the first seconds (the stage-1 career line, D-Day kind 5, tuple (1, 1, 0x20))
  ≈ −19; with both off −30. In the original the speech volume at 0 left the mix where it was
  (`audio_freeroam_nospeech`: no line in this window); the run with game music 0 died at the
  arm three times (as did two race runs and a free-roam run with traffic avoid fields; the
  game process vanished, cause **open**), so the original's music share is not measured.
  Whether the 3–5 dB is the song's level, a different song, or the line's timing is **open**.
* FX_TRAFFIC: the original had 1–2 instances near the garage exit (p2 −1251 … 573); the
  clone's log has none in the first 16 s (it has 204 blocks from 16.6 s, a context
  attaching to a car within 70 m; see below). The encounter racers' CAR events (p4 −32767,
  silent) are not started by the clone (inaudible).
* Not reached: a race start / race at speed (`audio_race`: the original died at the end of
  the record), rain and the garage (no captures this pass).

## 5. Saves

Every stage's save loads in the clone and is written back exactly as the original's own
load-and-save leaves it (the four hint dwords 0x529240 zeroes, nothing else): the
original's saves at stage 1's first win, stage 4 start and first win, stage 5 start and
first win, the finished career, and the clone-written `clone_s5_final` (test
`every_stage_save_rewrites_as_the_original_resaves_it`). So a clone save of any of these
stages is byte for byte the original's own resave, which the original loads
(finished_resaved.sav, docs/save-points.md); `clone_s5_final` was loaded in the original before
(internal notes). Saves after clone *play* at each stage were not cross-loaded
under Wine this pass.

## 6. Split screen

**Not reachable offline (verified).** Game mode 4 (split screen, 0x850078) has one
writer, 0x54045C, gated on g_NumPlayers 0x7FAA1C == 2 (0x540453). 0x7FAA1C has two writers:
0x53FF19 stores 1 and 0x5801FC stores the value 0x57E2B0 reads from a recorded input stream
(the replay / demo reader; 0 when 0x8650CC matches). The profile's player count (0x502930)
is written only from UI_QRModeSelect (0x4B2C00: 2 in game mode 4, else 1), 0x4DF2B7 (1) and
the profile reset (1). So no menu path reaches two players; only a recording that holds 2
could (none observed). Consistent with menus114; nothing to port.

## 7. 2026-11-05: scrape level, the mix, traffic movers

Wine copy `a private Wine copy` (display :281, wined3d), raw runs in `<work dir>/p5c-work/runs/`,
evidence in the maintainer's private captures.

**FX_Scrape's level (fixed, verified).** The writer of the scrape object's +0xD4 / +0xD8 / +0xDC
is its mix-map client SFXObj_Collision (registry id 0x50000, name 0x788BFC, factory 0x47E220,
ctor 0x474C60, vtable 0x78B078). Slot 0x28 (0x461CE0) takes the sound object ([client +0xB0]
+0x44, attached by slot 0x14, 0x461AE0) and writes +0xC0 / +0xD4 = slot 0x38(kind) / slot
0x38(9) (0x45E390: [+0x58 + 4·slot] & 0x7FFF; kind = sound object +0x124, 1..8), +0xC4 = +0xD8
= 0x45E3D0's pitch (0x1000; 0 while the manager's +0x14 pause is set) and, once while the
client's +0xBC is set, +0xC8 / +0xDC = slot 0x3C (the azimuth word). Its field update (slot
0x24, 0x461B60) copies the sound object's point +0x100 to the client's +0xD0, the position its
SFXCTL_3DScrapePos (0x50010, ctor 0x471D20, vtable 0x78AB30) reads through slot 0x2C (the
client's slot 0x1C, 0x461B30); SFXCTL_3DColPos (0x50000, 0x78AAF8) gets +0xC0. Both run the 3D
position fields of 0x45E100 → 0x45DF00. In MAPOUTPUT.mxb slot 9 of section 5 class 0 is
master 1: −600 mB + sub channel 0 (section 0's event controls 0 / 2 — the hold and the speech
duck of −900 mB — and control 3, the effects volume) + 3D control 1 (4..30 m, view 0 by the
distance to the player's car, curve 4). So the player's scrape plays at −604 mB (16365) and at
−1501 mB (5812) under a speech line — exactly the audio and camera parity's Wine reading {5812, 4096, az, 0, level}.
The clone now runs section 5 with one copy per live collision object at its point
(`Scrapes::points`, `mixmap::Scene::collisions`, `Scrapes::set_mix`); `audio_freeroam` gives
{5812, 4096, 64639, 0, 9…24} against the original's {5812, 4096, 64584, 0, 9…24}
(`fx_scrape-clone-after.txt`). The start frame sends 0x46A460's zeros (the mixer pass runs
apart from the object update; **inferred** order). `audio::scrape`'s doc comment is corrected.
Tests: `mixmap::tests::scrape_slot_nine`, `scrape::tests::the_mix_gives_the_scrape_its_level`.
The impact block (+0xC0, slot by kind: master 0 slots 0–5 at −600 mB, master 2 slots 7–8 at
−800 mB, slot 6 unbound) follows the same client; the clone's impact start volume is ported in the gap sweep
(§11).

**The 3–5 dB louder mix was the harness (fixed in `audiomix.py`).** The sim clock 0x865188 the
sync file maps through starts again at the world's load (observed: 439794 → 4368 at WAV 110 s);
audiomix took the first rising stretch, so every native window lay in the boot / front end.
With the last stretch (the run's): `audio_freeroam_musiconly` (new scenario: speech, effects,
engine, tires 0 by the profile — game music 0 still killed the original in the audio and camera parity, this run
lived) gives native −19.6 … −21.0 dBFS, clone −18.0 … −19.5; `audio_freeroam` (default
volumes) native −19.4 … −23 while driving, clone −17.3 … −21. Matching each WAV against the
decoded songs (the maintainer's probe `trax_dump`, `trax_match.py`): the original played EA Trax song 6 at
−9.04 / −9.81 dB under the decoded stream (corr 0.98 / 0.91), the clone song 18 at −9.02 dB;
in the front end the original's song 0 sat at −14.08 dB. Those are the mix map's levels
(section 0 class 0 slot 1 = 11598 → >> 8 = 45 → 45 / 127 = −9.0 dB in game, 0x482180 →
0x46C800 → 0x723E9D: × 1/127 = 0x78A94C; slot 0 = 6447 → −14.1 dB in the front end), so the
music gain stage is native. The remaining 1–2 dB is the song choice (random playlist; the two
original runs picked songs 6 and 21) and the scene, not a gain. The audio and camera parity's "3–5 dB" is withdrawn.

**FX_TRAFFIC (no missing system).** The clone runs CARSFX_TrafficEngine (`audio::traffic`); in
The audio and camera parity's and this `audio_freeroam` it starts at 16.6 s, when a traffic context attaches to a car
within 70 m; the original had a car within 70 m from the start. A placement difference, not
the sound.

**Traffic → MeloMvr after placement (traced, observed; not fixed).** The switch is the proposal
of "Avoid Cars as a traffic car" (0x4145B0, priority 300): 0x402ED0 with look 99999 walks the
road entries from the car's own index down (entries sorted by signed squared distance, built
by 0x6148F0 from the world object list 0x8A2CA4: objects with no car (+0x1C = 0), drawn cars
(+0x550) not excluded by 0x5EC070, and the own car, within 0x404910's reach or 0x403A40's
second chance, at most 15 / 16), takes the first ahead within the lateral window at time 0
(any gap under the look), and 0x4145B0 keeps it when that time < 5 s, 0x4140E0 agrees and the
target is not over 20 m/s on a racing controller. Read under Wine through /proc (the audio and camera parity's cave
fields had faulted the game; `avoidprobe.py`, `native-avoid-targets.tsv`): the targets are
other traffic cars 20–123 m ahead along the heading, often 5–23 m (once 84–88 m) to the side of
the heading on curving roads; the planner is the controller's +0x6E4 ([+0] the car, +0x308 the
own index, +0x30C the lane, 4 or 3). The clone ports the same test (`TrafficLane::car_ahead`);
a debug pass over `freeroam_stand` showed its entries ahead mostly just outside the window
(254 within 5 m of it, 115 inside), so the clone's lateral off the lane (0x4123C0 / 0x60BFB0:
the walk point's vt+0x34 less the lane offset) or the walk point an entry is put at differs
from the original's — **open**; the clone's switches stay at 24 (native 45) in 70 placements.

**Race traffic at the cap (traced; not fixed).** Both sides drive the TrafficMover at the lane's
speed byte (`lane_speed_at` on the original's mover-2 samples: 14, 11, 20, 15, 18 m/s bytes
match their speeds). The difference is the mover: at the green 11 of the original's 12 race
cars run on mode 4, the BeltMover, at 5–10 m/s, and go to mode 2 later
(`circuit_traffic-movers-at-green.txt`); the clone's are all on mode 2, coasted to a stop in
the scene, then accelerating to the lane speed. Mode 4 comes from 0x42ABD0 (0x7E8D70 set and
0x402180 true → 0x5F46B0(4); back through 0x5F4410 → 2 for a +0x24 = 4 car) and 0x4022D0 (the
controller's slot +0x10 at a world load change: unloaded → mode 4, docs/symbols.tsv); whether the
traffic's belt comes from the unloaded world at the race's far placements is **inferred**. Porting the belt for traffic is **open**.

**Not reached:** the 1.5° yaw after the wall crash and the look-back byte +0x2B writer.
No byte store to +0x2B in 0x5E0000–0x60FFFF (15 sites in the whole disassembly, none there);
0x5EB530 reads it on the object in ECX, and nearby car code compares a dword at +0x28, so a dword
or block write is likely — not traced further (**open**); the yaw was not re-run.

## 8. 2026-11-05: the traffic's walk, its lane window and the BeltMover

Wine copy `a private Wine copy` (display :285, wined3d), raw runs in `<work dir>/p5d-work/runs/`
(`walkprobe1.tsv`, `walkprobe2.tsv`: every traffic car's walk and stale road entries read through
`/proc` with `walkprobe.py`, no cave fields), evidence in the maintainer's private captures. Every clone
figure below is the mean over six traffic seeds (`NFSU2_TRAFFIC_SEED`, a harness knob that varies the
clone's draws; one seed is too noisy to compare), the original's from the audio and camera parity / the traffic-scrape parity runs.

**0x4123C0 / 0x60BFB0 (verified).** 0x60BFB0 reads a road entry (ECX): +0x14 for lane 0, +0x18 + 4·(lane
− 2) for lanes 2–5, 0 otherwise. 0x4123C0 (ECX a walk point, the FuturePoint) returns the point's vt+0x34
lateral of the position (forward 0x40D1B0: (p − control point) × its +0x18 direction; backward 0x40D410
on 0x401AA0's) less the lane's offset (0x401C40 on vt+0x8's lane) **less the point's own offset for that
lane** (+0x04 lane 0, +0x08 + 4·(lane − 2)), the term the clone lacked. 0x6148F0 locates each object at
its nearest walk point by the place on **lane 0** (0x419C10 passes lane 0; 0x4198C0: 2D distance clamped
to 0..99999 plus 10 × |control point z (+0x12 / 8) − z|, skipping ⌊0.02·d⌋ − 2 points after one d metres
off), s = +0x54 − +0x58 + the distance along the control point (vt+0x2C).

**The walk (verified, observed).** The route manager 0x428030 (from 0x428720 at 0x428A66) extends the
walk until its last point lies max(125 m (0x7E8E60), 4 s × speed) past the car's point and it holds more
than 9 points, at most 40 (0x422220); the car's point is the walk's first (walk +0xAD read 0 in all 368
samples, `walkprobe2.tsv`). Before each new control point 0x418DA0 puts trunc(segment / 8) − 1 points
every 8 m (0x783C68) on the previous one (+0x58 = 8k, flag +0x74), and 0x41A180 then gives those points
the offsets of a cubic Hermite per lane (0x574840 / 0x574C40) from one control point's lane place to the
next, tangents 0.75 × the run's length (0x43CEB0) along the chord from the point before and to the next
control point (at the walk's first point vt+0x20, at its last vt+0x1C). The control points' own offsets
stay 0 in a traffic walk (0x426810 starts them at 0; 0x40AA20 only decays lane 0's). **Observed**:
walks of 126 / 135 / 150 m (p10 / 50 / 90) with 13–18 points, points every 8 m, offsets up to ±2.2 m on
the curving record 734. The clone walked a fixed 21 control points (200–400 m) without the points or
offsets. **Now** `TrafficLane::extend` stops at the native reach and count, `walk_points` builds the
8 m points and the Hermite offsets, `entries` locates objects on lane 0 with the skip and the control
point height, `off_lane` subtracts the offset. The clone's offsets on record 734 match the original's
within 0.02 m (0.04 m on the walk's first run, whose tangent the original took from a point it had
dropped). Inside / near the window (within 5 m of it) was 52 % native (stale entries), 58 % clone.
Tests `the_walk_holds_the_originals_points_and_smooth_lane_offsets`,
`the_walk_reaches_as_far_as_the_originals`. **Not ported**: the walk's points in the driver's path
(0x415D50 takes every walk point, so the original's path bends through the 8 m points; the clone's path
keeps one point per control point), 0x418C50's 5 m skip, lane 0's decaying offset (+0x04, only in
0x4198C0's place: at most a few metres along, **inferred** harmless).

**The first think after placement (inferred).** The clone's placed car thought on its placement frame,
so its avoidance switch to the MeloMvr happened there and the parity rows never saw mode 2 first; the
original's switch comes 0.05–0.48 s after placement (p10 / median). Now a placed car drives its lane's
path (the path build without the action choice) until its turn (`think_slot`, every 32 frames with 16
cars).

| freeroam_stand, 20–90 s, 6 seeds | original | clone before | clone the traffic-mover parity |
|---|---|---|---|
| 2 → 3 (MeloMvr) switches | 45 | 27.5 | 42 (31–50) |
| switch after placement, median | 0.48 s | 1.3–24 s | 0.5–1.5 s |
| MeloMvr share within 250 m | 0.86 | 0.67 | 0.82 |
| mover switches 4 → 2 / 2 → 4 / 3 → 4 | 28 / 10 / 11 | 0 / 0 / 0 | 13–26 / 9–13 / 6–14 |
| placed / within 250 m | 15.0 / 11.8 | 14.9 / 10.8 | 14.9 / 10.5 |
| jam share / placements per minute | 0.20 / 44.6 | 0.21 / 43.6 | 0.20 / 44.3 |
| actions 2 / 7 / 17 | .45 / .26 / .29 | .44 / .26 / .30 | .55 / .35 / .10 |

**The BeltMover for traffic (verified; observed).** 0x5F46B0 turns any request for a mode other than 0
or 4 into mode 4 for a traffic car while car +0x5C0 is clear (the world under it not loaded); 0x5F6CE0
on a change calls slot +0x10, 0x408C40: loaded → 0x5F4410 = mode 2, not loaded → 0x4022D0(0) → the
mover's slot +0x10 (0x570A70, 1) → mode 4. The think's end 0x422150 rebuilds the belt on mode 4
(0x4221A2 → 0x5918B0). A belt without a path stops the car (0x59EC90). So a race's fill on unloaded
sections is on mode 4 with no path; the scene's pause (0x4099B0, mode 0) moves nothing; at race state 3
0x409A30 asks for mode 2 (→ 4) and the resumed thinks build the belts, which accelerate through the
countdown. **Now** `traffic_stream` applies the rule each frame (stream test `streamed_in`, observed
under Wine in <work dir>), the belt is built and stepped as the racers' (`build_belt_state`, `step_belt_state`, kinematic in
the collision step), and `traffic_coast` runs the belt cars' thinks from the scene's cut (race state 3;
until the traffic-mover parity the clone's traffic did not think before the green). Parity `mover` reads 4.

| circuit_traffic, 6 seeds | original | clone before | clone the traffic-mover parity |
|---|---|---|---|
| at the green: modes, mode-4 speed median | 11 × 4 + 1 × 2, 8.0 m/s | 12 × 2 (standing) | 10–12 × 4, 6.3–8.1 m/s |
| 1 s / 5 s after: mode-4 cars, speed | 10, 10.1 / 6, 15.3 m/s | none | 8–11, 9–11 / 6–7, 8–15 m/s |
| speed-cap share (mover 2 at ≥ 20 m/s, 5–60 s, within 250 m) | 0.19 | 0.38 | 0.38 |
| jam share (`racestats.py`, 5–60 s) | 0.34 | 0.69 | 0.76 |

**Open (the traffic-in-race parity §9 took the obey share up):** the race's cap share and jam share (unchanged by the belt: after the switch back the clone's
TrafficMover cars sit at the lane's speed byte more often than the original's — the original's lane
drive is slower in free roam too, p50 13.1 vs 15–17 m/s, and slower approaching intersections, obey
p50 9.6 vs 12–17 m/s, so it spends 0.29 of its time in "Obey traffic intersections" against the
clone's 0.10 now that the walk is native; neither traced); the walk's points in the driver's path; what
the controller hands the driver before its first think; impact sound start volume (open item,
not reached).

## 9. 2026-11-05: turns at intersections, the obey share

Wine copy `a private Wine copy` (display :290, wined3d), raw runs in `<work dir>/p5e-work/runs/`
(`nat5/cache.tsv`: every traffic car's junction cache and walk read through `/proc` with
`cacheprobe.py`, no cave fields; the cave was given the player's fields only — with the encounter
racers' fields the cave faulted at 0x782C53 reading 0x00FFFF14 in three runs). Evidence in
the maintainer's private captures. `$OB_PRIVATE/devprobes/src/bin/traffic_intersections [REGION] [REC CP]` lists the intersections, their crossing
points and the control points flagged with each (or one point's junctions); `obeywin.py`, `turns.py`,
`xtime.py` (evidence dir) bin the obey share by distance to a crossing point, count turns and time
the passes through intersection 3. Clone figures are means over six traffic seeds (0, 11 … 55); the
original's from the audio and camera parity runs as in §8.

**Why the obey share fell (traced, observed).** "Obey traffic intersections" (0x11, priority 600,
0x4149E0) runs while walk +0xC4 (planner +0x3E0) is set: 0x418AE0 sets it to the first walk point
from the car's (+0xAD) whose control point carries an intersection (0x5D8930: flag bits 19..24 an
index whose two roads have record bit 6, 0x410BF0) — the crossing control points only (each of the
38 intersections flags just its two crossing points). The action is done when +0xC4 is −1 (0x41CE30).
Both sides' obey windows are the same (the original 1.00 from 0 to 100 m before a crossing point,
`obeywin-*.txt`), but the clone's traffic **turned** at intersection 3 (record 734 cp 16 → record 492
cp 4 and the like, junctions of high nibble 4 with bit 26) in 51 of 104 passes and so skipped both
crossing points; the original turned in **0 of 18** (`turns-before.txt`). The branch odds are ½
(0x413BB0, via the controller's junction cache 0x41CFA0 and goal slot +0x28; the lane fit for a turn
as the clone had it), but:
* each route-manager pass (0x428030) first runs **0x40A620**, which drops the walk's trailing points
  whose +0x58 is not 0 — the 8 m points and a turn's corner point and target, which **0x41EE20**
  adds with offsets along their segments (observed: `734:17:116.6:11.0 492:4:122.8:0.9`);
* 0x41EC20 considers a turn (bit 26, not a must-take high nibble 2) only when the walk's last
  point lies more than **30 m** past walk +0x354, the s the walk's last point had when the last turn
  was taken (the junction's point before 0x41EE20 cuts the corner; set at 0x41EDC7, −99999 at the reset 0x411C0C, rebased with the points dropped behind, 0x41970E);
* after a turn 0x422220 counts the walk's length as the target's s plus the straight distance to it
  once more (0x4226C7), so an extension that turns where the walk reaches its length stops right
  after the target.
So a turn decided as the walk grows at its end (every think) is gone at the next think and cannot be
taken again from the same point: the car goes straight on. Under Wine car 0x3e95ff0's walk took
native junction 947 (= the file's 949; native index = file index − 2) at 13.98 s, cache 947:1, +0x354
121.6, and 1.9 s later held 734:16 → 15 → 14 straight with +0x354 113.6 and the cache entry gone
(`native-turn-dropped-0x3e95ff0.tsv`); 11 cache decisions for the two turns there were 2 accepted,
both dropped. Only a turn inside a longer extension (a placement's first build, 0x426810) stays.
**Now** `TrafficLane::extend` drops a trailing turn's target (`turned`), re-walks its point with the
junctions before the turn passed over (declined before; **inferred** as the cache keeps them) and
gates turns by `TURN_GAP` on `last_turn` (walk distance kept in `walked`); the turn's stop test uses
the junction's s + 2 × the straight distance to the target (the corner cut of 0x41EE20 is not ported,
**inferred** equivalent for the stop test). Test `the_walk_drops_a_turn_taken_at_its_end`.

| freeroam_stand, 20–90 s, 6 seeds | original | clone the traffic-mover parity | clone the traffic-in-race parity |
|---|---|---|---|
| turns at intersection 3 (passes) | 0 of 18 | 51 of 104 | 0 of 90 |
| obey share 0–100 m before a crossing point | 1.00 | 0.37–0.66 | 1.00 |
| actions 2 / 7 / 17 | .45 / .26 / .29 | .55 / .35 / .10 | .55 / .23 / .23 |
| MeloMvr share within 250 m / 2 → 3 switches | 0.86 / 45 | 0.82 / 42 | 0.88 / 36.5 |
| jam share / placements per minute | 0.20 / 44.6 | 0.20 / 44.3 | 0.31 / 36.7 |
| pass time 60 m before → 20 m past intersection 3, p50 (N–S / E–W) | 8.0 / 7.7 s | 5.7 / 4.7 s | 6.1 / 4.6 s |

**New gap (the traffic-jam parity §10: native behaviour, the original jams there too):** with the traffic now going straight through intersection 3, two of six seeds (11,
44) jam (0.54, 0.50; the others 0.15–0.28): a MeloMvr car standing about 2.6 m left of its lane
(`off_lane`) in "Avoid Cars" takes the oncoming cars on the far lane (about 3.6–4 m across, inside
0x402ED0's window of +0x08 + 0.5 m + +0x08 from where it stands) for the car ahead, creeps at 1 m/s
and a queue forms behind it; how the car got off its lane and how the original leaves such a spot
were not traced. The original's cars also cross the intersection slower (table) and obey at lower
speeds (p50 8–12 vs 11–15 m/s): its corner speed limits (0x422780, per turn registered in walk
+0xD0) do not apply to straight passes, so the slower approach is still not traced.

**Race traffic (not changed; the traffic-jam parity §10 compared it).** circuit_traffic over six seeds after: cap share 0.35 (0.55, 0.26,
0.16, 0.40, 0.45, 0.25; the traffic-mover parity 0.38, native 0.19), jam share 0.77 (the traffic-mover parity 0.76, native 0.34). Part of the
jam is cars stopped for good by 0x408DE0 (a contact record's summed |J|, +0x60, over 30, or under
1 m/s while wanting over 1: `TrafficCtl_StuckTest`, as ported): one debug run (seed 22) logged 12
such stops in 60 s, half of them hits at 5–12 m/s. Whether the original's race traffic hits as often
is open (not compared).

**Not reached:** the driver's path through the walk's 8 m points (0x415D50); what the driver has
before its first think (0x42C950 → 0x42A190 / 0x42A360 run only the route manager, 0x428030; the
driver's side not traced); impact sound start volume.

## 10. 2026-11-05: the free-roam jam, race stops, the walk's steps

Wine copy `a private Wine copy` (display :292, wined3d), raw runs in `<work dir>/p5f-work/nat/`
(`fsA` … `fsC`: free roam 200 s, `rcA`, `rcB`, `race2`: STAGE_1_CIRCUIT_1; the cave given the player's
fields only, `PARITY_HUMAN_ONLY=1` in the maintainer's tool `native.py`, because with the encounter racers' or the
rivals' it faulted at 0x782C53 reading 0x00FFFF14/15 every time; the traffic read through `/proc` by
`stuckprobe.py` every ~0.3 s). Evidence and scripts in the maintainer's private captures. Clone
figures are means over six traffic seeds (0, 11 … 55), the native's over the runs named.

**The jam is native (observed).** The traffic-in-race parity jam spot, record 538 (id 734) cp 22 → 21 at 450 / 95 heading
south, jams the original too: in run `fsB` four cars stood there in "Avoid Cars" (type 2) on the MeloMvr
for the last 25 s (450,95 · 451,100 · 452,79 · 455,70; `native-fsB-jam-spot.tsv`), and in the traffic-mover parity's
`walkprobe2` run a car stood at 450.6 / 96.6 (mode 3, type 2) for all 40 s. There, as in the clone,
the car's own road entry lies on a later walk point: entry +0x28 = 2 (734:21) while the walk starts at
734:22 + 24 m, and its lateral off its lane reads −3.56 m (`native-own-entry-p5d-walkprobe2.txt`;
over all 368 samples own index 0 / 1 / 2 = 276 / 69 / 23, 69 off by over 1.5 m). **Verified** why:
0x4198C0 locates an object at the walk point with the least 2D distance plus **10 × |the point's control
point height − z|** (+0x12 << 13 / 65536), so on the slope there a car 15–25 m down a segment is put on the
next control point, whose direction bends 10°; 0x4123C0's lateral off that line is metres off.
0x402ED0 (verified) has **no direction or lane-side filter**: an object is taken when its lateral off
the lane is within its +0x08 + 0.5 + the car's +0x08 of the car's own, ahead (its s at least the car's),
with a time (gap / closing under −0.01, or 0x402CC0's) or within the look / 20 m standing. So the
oncoming cars 3.5–6 m across are in the window, the car slows (0 within 15 m, else 6 m/s, 0x424CB0) and
the queue behind it tracks it at 6 m/s. Nothing in 0x424CB0 / 0x41CD10 / 0x4145B0 / 0x4140E0 moves a
traffic car back to its lane: the lane changes run only for a tracked human car (0x614890), and standing
(under 0.5 m/s) it takes the lane it stands in (0x424F24: **0x40F4A0** — the lane 2–5, of those the
record has that way, whose 0x401C40 offset is nearest the car's lateral off its walk point's control
point, 0x401ED0 — then **0x405410**, which also restarts the lane's offset). Ported:
`standing_lane`, test `a_standing_avoider_takes_the_lane_it_stands_in` (on a one-lane road it keeps
the lane; no measurable effect on the jam).

| freeroam_stand, 20–90 s | original (fsA, fsB, fsC) | clone the traffic-in-race parity | clone the traffic-jam parity |
|---|---|---|---|
| jam share (0x4112A0) | 0.20, 0.11, 0.42, 0.15 (mean 0.22) | 0.31 (0.15–0.54) | 0.36 (0.13–0.69) |
| placements per minute | 44.6, 30.9, 26.6, 62.6 (mean 41.2) | 36.7 | 33.3 (15–49) |
| MeloMvr share within 250 m / 2 → 3 switches | 0.86 / 45 | 0.88 / 36.5 | 0.85 / 32.8 |

(fsA–fsC from the probe: jam = a sweep with a car of mover ≠ 0 under 1 m/s, placements = jumps over 50 m;
The audio and camera parity's from its cave fields. The original's runs differ as much as the clone's seeds.)

**The walk's steps (verified, observed; fixed).** 0x422220 adds to a new walk point's s
**0x40CBC0**: the next control point's offset from the last, projected (2D) on the last's direction of
travel (backward 0x401AA0's), and 0x418DA0 puts trunc(that / 8) − 1 points every 8 m between. Within a
record that is about the point's segment; across a junction it is the jump between the two roads
(observed in the traffic-mover parity walks: 540:1 → 495:1 at + 4.9 m, 731:10 → 477:24 at + 13.3, where the records'
segments are 25.1 and 5.0 m). Outside drag races (0x89E7D8 / 0x890108) a new point under **3 m**
(0x783C7C) on is passed over and the walk goes on from it, at most 100 times (0x4223AE..0x42241B). The
clone took every step's own segment: on the 540:6 → 734:23 joint (3.7 m) it put 8 m points 16–40 m along
540 past the joint and the car's entry onto them. **The walk's first point (verified):** 0x428030 locates
the car by 0x4198C0 (flags 0, the car's lane, controller vt+0x1C) from walk +0xAD and, for traffic, drops
the points before it (0x419670; a controller whose vt+0x28 is true keeps 20, 10 in race types 0 / 7),
so the walk starts at the car's nearest point, 8 m points included (observed: first points 734:22 + 24,
734:21 + 8 / 16; the car 4 m before to 12 m past it, `native-walk-first-along.txt`). The clone dropped
whole control points by their distance alone. Now `TrafficLane` keeps the control point whose 8 m point
the car is at (`lead`), measures the walk's reach and point count from there (0x422220), builds the
road entries from it, and `gap` / `seg` give every step. Tests `the_walk_steps_by_the_jump_at_a_junction`,
`the_walk_passes_over_points_under_3_m`.

**Race traffic: the stops (observed, fixed in part).** The original's race traffic stopped for good
(controller +0x710) 3, 4 and 2 times in runs of 75–90 s (`race2`, `rcA`, `rcB`;
`native-probe-summary.txt`), all at 10–18 m/s at walk ends (0x428720 false / 0x417750's dead end):
335.5 / −277.2, 336.4 / −276.0, 339.9 / −271.2, 288.0 / −278.5 and 650.3 / 45.6 on mode 4, −568.4 / 316.0
and −264.5 / 556.7 (three times) on mode 2; none by a contact. The clone has its walk ends at the same places (334–346 / −262…−282, −564 / 318, −264 / 557, 578 /
−128). The clone also stopped 4–6 cars a race by 0x408DE0 against **track** contacts on the BeltMover:
mode 4 is a traffic car over a world that is not loaded (0x5F46B0 on car +0x5C0), where the original
has no track to touch, but the clone keeps all of the world's collision. Now `step_world` gives a
traffic car on the belt no track contacts (**inferred** from the streaming rule; test
`traffic_on_the_belt_meets_no_track`). Stops per 60 s race (debug count, the build before the walk-first lane change): before 6–13 (mean 10.7), after 4–8 (6.3;
dead ends 3–6, car–car 0–3).

| circuit_traffic, 5–60 s, 6 seeds | original | clone the traffic-in-race parity | clone the traffic-jam parity |
|---|---|---|---|
| speed-cap share (mover 2 at ≥ 20 m/s within 250 m) | 0.19 | 0.35 | 0.29 (0.07–0.44) |
| jam share (`racestats.py`) | 0.34 | 0.77 | 0.65 (0.20–0.96) |

The race's remaining jams: two overlapping records with different lane layouts (e.g. race records 16,
two lanes each way at ±5.91 / ±1.97, and 17, one at ±1.81, over the same road at 352 / 387): a car on
each meets the other head-on 2 m across and both stand (0x402ED0 takes either); the original's data are
the same, how often it happens there is **open** (not seen in the three native race runs).

**Open:** the stale contact record a re-placed car carries for a few frames (its old pair's +0x60 summed
pushes, which 0x408DE0 reads on the first frame at its new place — whether the original's placement
0x5FBEE0 / 0x5F52D0 clears the body's records is not traced: 0x59F890 resets the car, not the pair list);
the slower approach to intersections (obey p50 8–12 vs 11–15 m/s); the driver's path through the 8 m
points (0x415D50); the impact-sound start volume (not reached).

## 11. 2026-11-05: race stops re-measured, unloaded bodies, the path's walk points, impact volume

Traced in SPEED2.EXE (Ghidra / the disassembly); no new Wine run — the native figures are the traffic-jam parity's
probe runs (`<work dir>/p5f-work/nat/`) re-read. Evidence and scripts in
the maintainer's private captures. Clone figures over six traffic seeds (0, 11 … 55), `multirace.sh`;
`NFSU2_TRAFFIC_STOPLOG=<file>` (harness knob) logs each car stopped for good with the rule and
the nearest other car; parity fields `stopped` (controller +0x710) and `recs` (body +0x18) added.

**The native stop rate was per wall second (corrected).** the traffic-jam parity's "3, 4, 2 stops in 75–90 s" divided
by the probe's wall time; under the cave the original ran ~0.41 game s per wall s (native.tsv:
3601 frames in 147 s). By the sim clock 0x865188 (4000 ticks/s, as 0x592BE0 reads it) the probe
windows were 33.3 / 38.0 / 32.0 game s: **5.2 stops per 60 game s** (4.6 without rcA's car standing
from the first sample), all at walk ends, none by a contact (`native-race-stop-rate.txt`). The clone:
6.5 per 60 s before the gap sweep (dead end 3.0, walk end 1.3, car contact 1.7, stall 0.5), 6.7 after; in the
first 35 s after the green walk / dead ends 2.5 a race against the original's 2–3. **The remaining
excess is racer–traffic contacts**: every clone contact stop had an AI racer 2–5 m away (rear-ending a
traffic car on the MeloMvr at 23–25 m/s), and the clone's racers spend 2–20 × the original's frames
within 6 m of a traffic car, closing at 3–20 m/s where the original passes at 24.5
(`racer-traffic-proximity.txt`; no native sample had a contact record on a traffic car). Why the
original's racers pass (AvoidAvoidable's slow-test room check and 0x60BF80, the avoidance re-plan —
see formats.md, not ported) is **open**.

**Stale records after placement (verified, no change).** The placement 0x5FBEE0 → 0x5F52D0 → 0x59F890
resets the car, not its contacts: a record goes only when its last push is 0.3 s old (0x592BE0,
0x7FBEE0) at the end of the collision step 0x5FFA60 (→ 0x5B2280 → 0x5AE480, which also calls both
bodies' vt+0x84 = 0x5929D0, `dec byte [body+0x18]`), and a pair with a record is never deleted
(0x592E70 → 0x5FFAD7). So the original's re-placed car carries them too, as the clone's does.

**Unloaded bodies meet nothing (verified, ported).** The car body's partner filter 0x593990
(vtable 0x7A2478 +0x54) refuses every partner while [body +0x60] +8 is null — set exactly while the
world under the car is loaded — and the sweep 0x5B23C0 asks the body's own vt+0x54(0) first, so an
unloaded car gets no track, car or prop pair (car pairs also need both bodies' +0x190). The traffic-jam parity's
**inferred** "belt traffic has no track contacts" is now this rule, for every car: `CarState::streamed`
(racers 0x5F46B0's stream test, encounter cars, traffic), gating `step_world`'s car pairs, track
contacts and props. One race seed changed (44: two stops between unloaded traffic cars gone).

**The path through every walk point (verified, ported).** 0x41D530 runs over the walk from the
car's point + 1 to its end and 0x415D50 appends each walk point — the 8 m points included — while
the path holds under 20 (count byte < 0x14), at 0x4122C0's place: along the point's control point
by its +0x58, across by the lane's offset (0x401C40) + the point's own Hermite offset for the lane
(+0x04 lane 0, +0x08 + 4·(lane − 2)) + the path's lateral (0x40FC80 per point). `TrafficLane::update`
now builds it so (`kept` keyed by control point and +0x58). Tests
`traffic_path_keeps_its_front_until_passed`, `lateral_returns_to_the_lane` (rewritten for the 8 m
points).

**Obey speeds (traced, still open).** The intersection action's update 0x425150 starts +0x1F0 from
the controller's vt+0x7C (0x41E7A0: +0x79C, 55 mph), takes a car ahead within 30 m (stop within
15 m, else ≤ 6 m/s), and without the right of way (0x4109A0, as ported) ≤ 6 m/s within 30 m of the
line; going, min(55 mph, +0x1F0). Nothing slower for a straight pass was found; the original's obey
p50 8–12 vs the clone's 9–20 m/s (`freeroam-obey-speeds.txt`; native p90 14.7 vs clone 20.4) stays
**open** (next: the driver's speed on the 8 m path, 0x41C380's look on obey, the TrafficMover's
response).

| circuit_traffic, 6 seeds | original | clone the traffic-jam parity | clone after the gap sweep |
|---|---|---|---|
| stops per 60 s (game time) | 5.2 (4.6) | 6.5 | 6.7 |
| … by a contact or a stall | 0 | 2.2 | 1.8 |
| speed-cap share / jam share | 0.19 / 0.34 | 0.28 / 0.65 | 0.27 / 0.63 |

| freeroam_stand, 20–90 s, 6 seeds | original | base | + unloaded rule | + walk-point path |
|---|---|---|---|---|
| jam share | 0.22 | 0.36 | 0.42 | 0.44 |
| placements per minute | 41.2 | 33.3 | 30.4 | 29.0 |
| MeloMvr share within 250 m | 0.86 | 0.85 | 0.83 | 0.80 |

(The free-roam differences come from two or three seeds whose runs diverge — seed 0's jam 0.22 → 0.55
with the unloaded rule alone; the others are identical or within 0.05. Seed-to-seed spread 0.13–0.82.)

**Impact sound start volume (verified, ported).** See formats.md, Impact sounds: V is the sound
object's +0xC0, which SFXObj_Collision's slot 0x28 (0x461CE0) writes each frame from slot 0x38 of
the object's kind — section 5's level at the object's point (slots 0–5 −600 mB, 7–8 −800 mB, slot 6
none; then the master's 3D distance control) — and 0x47BB30 → 0x46CF70 keeps the playing layers at
volume × V; the volume byte only picks the sample. The clone plays each impact from the next mix pass
at its copy's level (`Audio::impacts`); before, V was the byte scaled to 0x7FFF. Test
`mixmap::tests::impact_slots_by_kind`.

**Open:** racer–traffic passing (above; the racer-traffic parity §12 traced it and re-measured it over six native races); the obey speeds; the controller's hand-over before the
first think.

## 12. 2026-11-05: the racers' traffic avoidance (AvoidAvoidable end to end)

Traced in SPEED2.EXE (Ghidra + the disassembly); observed under Wine (copies `a private Wine copy` :312 and
`a private Wine copy` :313, wined3d; raw runs in `<work dir>/p12-work/nat/`). Scenarios
`circuit_traffic_p12` / `sprint_traffic_p12` record only the fields the statistics read (`[run]
skip`): with the full field set the cave faulted at 0x782C53 reading 0x00FFFF15 again (as in the traffic-in-race parity /
The traffic-jam parity), with the reduced set all twelve runs went through. New fields: `ai_obj` (action +0x218),
`ai_reach` (+0x21C), `ai_cap` (+0x1F0), `ai_line` (+0x1FC), `ai_plan_off` / `ai_plan_rate` /
`ai_plan_stamp` (the lane record, controller +0x360 + 4 / 8 / 0xC: planner +0x30C, the native values
0.05 / 0.10 / 0.15 / 999 confirm the place), `t7a0` (traffic controller +0x7A0), `recs` now for
traffic too. Evidence and scripts in the maintainer's private captures.

**Traced (verified).** The whole chain is in formats.md (Racer actions → 0x428DF0, AvoidAvoidable):
* the look-ahead 0x42BA20 → 0x428CE0 → **0x4275E0** (the swept test): the other car moves along its
  driver's path, a traffic car along a prediction (0x425A40 → 0x420B80: 0.1 s Euler steps of its
  velocity and acceleration, points under 1 m apart refused, at most 20 — about 2 s; 0x420A60's
  100 m line when |a| > 0.75 |v|); per-point boxes (0x4040A0), a moving traffic car's 1.3 times as
  large; stepped point to point (0x404EF0 the start) and tested by a 2D separating-axis sweep
  (0x4044C0 / 0x4043F0 / 0x404240). The clone moved the other car straight on at its velocity in
  0.1 s steps with unscaled boxes for 3 s (**inferred** until now);
* the room check, **0x428FD0** (the plan search): the stamp +0x0C is set **only for the 999
  fallback** (0x42925D / 0x42932F; the clone stamped the stepped plans and not the fallback);
  `slow` (+0x42C < 11.18 m/s) gives a side whose object edge leaves the room at a walk point between
  the car and the object the cost 99999, and with neither side finite the plan stays (not ported
  before);
* the brake / speed decision, **0x42F330** (the 0x42F505 tests): near / far run only when no plan
  was found and the object is neither traffic (0x6148D0) nor ghosted (0x60BF80 → 0x5EBFB0, the ghost
  timer car +0x974 / +0x978; taken as false before); for traffic the racer keeps the line speed and
  only steers;
* the end, **0x42BA80**: +0x220 (the traffic controller's +0x7A0, a refcounted placement object)
  read 0 in every traffic sample of six races, so the 10 m-behind rule is the one that runs.

**Observed (native).** Over six circuit races the lane record's stamp was non-zero in exactly the
3088 AvoidAvoidable samples at rate 999 and zero in all 17,348 others (`native-avoid-record.txt`).
The racers' contact with traffic is not rare natively: race n2 had 820 frames with traffic in a
racer's lane 0–12 m ahead, 480 frames of GetUnstuck and 413 traffic frames with a contact record;
n1 had none of these. The gap sweep's "the clone's racers spend 2–20 × the original's frames within 6 m" came
from one native race (272 frames); the six-race native mean is 385 (92–968).

| circuit_traffic_p12, 60 s from the green | native (6 races) | clone before (12 seeds) | clone the racer-traffic parity (12 seeds) |
|---|---|---|---|
| racer–traffic frames within 6 m / 4 m | 385 (92–968) / 142 (9–457) | 362 / 155 | 498 (177–1554) / 179 |
| 4 m approaches per race | 5.0 (1–7) | 7.0 | 6.9 |
| frames with traffic in the racer's lane 0–12 m ahead | 234 (0–820) | 130 | 228 (2–975) |
| racer − traffic speed within 15 m, p50 (m/s) | 15.3 | 15.9 | 16.2 |
| racer speed within 30 m of traffic, p50 (m/s) | 22.9 | 26.3 | 26.2 |
| AvoidAvoidable frames (all racers) | 3405 | 3196 | 4283 |
| … on traffic / on racers or the human | 1283 / 2122 | — | 1134 / 3149 |
| … at the fallback rate 999 | 515 (0–912) | 550 | 806 |

(Clone runs use native race n1's tick counts and player slot; `cmp.py`, `objkind.py`; the native's
object kind is told by whether the pointer is ever a Pass / IJustGotHit target.)

Both clone builds lie within the native's spread on every row; with the native sweep the frames
AvoidAvoidable spends on traffic match the original's (1134 vs 1283), the in-lane following too
(228 vs 234). What stays above the original is the racers' avoiding **each other** (3149 vs 2122
frames on racers / the human) and with it the fallback share; the human's path in the sweep is a
prediction in the clone (**inferred**: the native reads the human driver's +0x10 path, not kept by
the clone), and the racers' bunching is not traced here. Traffic stopped for good by a car impulse
(0x408DE0, the clone's stop log): 0.9 per race before, 1.5 after (native: not recorded in these runs).

**Open:** a car going backwards in the sweep (0x425910 / 0x425980); the human's path; the reach's
vt+0x60 term (0x404910, the prediction's length cap); the 0.151 rate the record takes at the same
frame (≈ 39.5 s) in two native races (not traced).

| sprint_traffic_p12 (STAGE_1_SPRINT_1), 60 s | native (6 races) | clone before (12 seeds) | clone the racer-traffic parity (12 seeds) |
|---|---|---|---|
| racer–traffic frames within 6 m / 4 m | 109 (0–430) / 57 (0–239) | 93 / 45 | 118 (0–429) / 56 |
| 4 m approaches per race | 1.3 (0–5) | 1.7 | 1.7 |
| frames with traffic in the racer's lane 0–12 m ahead | 50 (0–222) | 62 | 55 |
| racer speed within 30 m of traffic, p50 (m/s) | 22.4 | 20.9 | 19.6 |
| AvoidAvoidable frames / on traffic / at rate 999 | 2920 / 312 / 96 | 1714 / — / 206 | 2614 / 427 / 301 |

(Clone runs on native sprint s1's ticks; `sprint-table.md`.)

**Outrun rivals (lead's scope addition; traced, observed).** The challenge's rival runs goal 6
(Chase car, vtable 0x784B58, chooser 0x42D6F0: the same proposal list as 0x42D0F0) and goal 5 when
the player leads; both goals' slot +0x20 is 0x40CA60 (= 1, as "Win road race" 0x784768; read from
the vtables), so 0x42F330 runs 0x428FD0 and the sweep 0x4275E0 exactly as for race racers. The clone's
encounter controller calls the same `Actions::think`, with the free-roam traffic in its road entries.
Under Wine (`outrun_traffic_p12`: the finished-career save's save, the first encounter car engaged, the player's throttle
held, 40 s, the traffic recorded — `native.py` now keeps the traffic cars in an outrun when the
scenario asks; 8 outruns) the rival's AvoidAvoidable objects were nearly all traffic (248 frames per
outrun on traffic, 8 on the player), with the 999 fallback in some. Statistics while the outrun runs
(rival within 300 m; `OUTRUN=1 cmp.py`, `outrun-table.md`):

| outrun_traffic_p12 | native (8) | clone before (12) | clone the racer-traffic parity (12) |
|---|---|---|---|
| outrun length (s) | 8.6–32.8, mean 13.4 | 21–40, mean 28.8 | same |
| rival–traffic frames within 6 m / 4 m per outrun | 29 / 9 | 75 / 25 | 71 / 27 |
| … per second of outrun | 2.2 / 0.7 | 2.6 / 0.9 | 2.5 / 0.9 |
| frames with traffic in the rival's lane 0–12 m ahead (per s) | 4.4 (0.33) | 32 (1.1) | 38 (1.3) |
| rival speed within 30 m of traffic, p50 (m/s) | 40.9 | 27.3 | 28.8 |
| rival − traffic speed within 15 m, p50 | 28.7 | 18.3 | 21.1 |
| AvoidAvoidable frames / at 999 | 187 / 16 | 171 / 38 | 251 / 68 |

Per second of outrun the rival's time near traffic matches; what differs is the rival's **pace**: the
original's rival ran p50 35–47 m/s and opened the 300 m in 9–14 s in six of eight outruns, the
clone's 26–41 m/s (one seed 12) and 21–40 s, so it meets more traffic and follows it in lane four
times as long. That pace gap (the goal-5/6 speed target, `SpeedTarget::update_outrun`,
internal notes) is outside this work and **open**: it is the player's "outrun racers
too easy" report, not the avoidance. **The outrun-pace pass** (`outrun-pace.md`): from the same starts the gap was
the MeloMvr's drag (twice the original's at full gas), not the speed target; fixed, the rival's
speed near traffic is now 41.3 m/s (native 38.8), in-lane following 10.4 frames (native 5.8, before
22.2). The "9–14 s" above came largely from the original's player crashing into traffic.
