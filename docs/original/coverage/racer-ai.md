# Racer AI coverage map

Every function of `SPEED2.EXE` that decides how the AI racers drive (driving line, speed, overtaking,
blocking, catch-up, mistakes and recoveries, shortcuts, per race type), and how far Overbright
reproduces each one. The status words and columns are explained in [README.md](README.md). Addresses
and offsets are explained in the address map, [formats/ai.md](../../formats/ai.md) ("AI driver", "AI
route layer"), and for drag and drift in [formats/career-1.md](../../formats/career-1.md) and
[formats/career-2.md](../../formats/career-2.md).

"Ours" names the file under `src/` and the function; `ai/` is `src/ai/`, `sim/` is
`src/bin/game/sim/`. Evidence words: **verified** (read in the original's code), **observed** (read in
the original under Wine), **inferred**, **open**. Comparisons in races use the
[parity harness](../../developing/parity-harness.md) (same save, grid, cars and inputs in both games)
and are summarised in [race-ai-parity.md](../../notes/parity/race-ai-parity.md).

The function list was found by walking the calls from the racer controller's vtable (0x785370), the
road and drag goals (0x784768, 0x7847D0), the action vtables (JustDrive 0x785684, Pass 0x7856D8,
Block 0x785728, GetUnstuck 0x785784, IJustGotHit 0x7857B4, AccidentRecovery 0x7857E8,
AvoidAvoidable 0x785858, drag hit 0x78588C, drag wall 0x7858D4), the driver's vtable (0x784EB0) and
the constructors 0x4260B0 / 0x425AB0, inside 0x401000–0x431000: 456 functions. 297 of them already appear in the address map or the code and are covered by
the rows below by area (a helper in the group row of its caller); the other 159 had no trace before
this map and have their own section.

## Summary

| Status | Rows |
|---|---|
| PORTED | 39 |
| SIMPLIFIED | 11 |
| GUESSED | 8 |
| MISSING | 11 |
| NOT NEEDED | 6 |
| **Total** | **75** |

Launches, the countdown, the racing line, catch-up, the think schedule, Pass, Block, IJustGotHit,
AvoidAvoidable, GetUnstuck and the junction odds (shortcuts) are traced and match the original in the
harness runs. The gaps that remain sit in four places: the corner apex of a turn junction, the drag
race's own hit and wall actions, the nitrous asks outside drag, and how free-roam encounter cars get
past traffic.

## Controller, think schedule and goals

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x4260B0 | — (racer controller ctor, type 3) | controller plus driver: CarDriver 0x425AB0, DriftDriver 0x425E40 in drift | `sim/ai/racer.rs`, `ai/driver/mod.rs` | **PORTED** | verified | low |
| 0x42ABD0 | AIRacer_Think | full think on the frame counter's slot (mod 8, mod 4 in drag) and 0.2 s after the last; fallback in between | `sim/ai/racer.rs:racer_think`, `ai/encounter_controller.rs:ThinkSchedule` | **PORTED** | verified; observed: action changes fall on one residue mod 8 per rival in both games | low |
| 0x420810 | — (controller think) | catch-up refresh, path hand-over, driver step | `ai/route/path.rs:take_replanned`, `sim/ai/racer.rs:racer_driver_step` | **PORTED** | verified | low |
| 0x42C060, 0x42AA20 | —, AIController_SetGoal | the race goal at the green (1, or 2 / 3 in drag) | `sim/ai/racer.rs:racer_think` | **PORTED** | verified; observed goal 1 and action 6 on the frame after the green | low |
| 0x42D0F0 | GoalWinRoadRace_Choose | GetUnstuck, IJustGotHit, AvoidAvoidable, AccidentRecovery, Block, Pass, JustDrive by priority | `ai/actions/manager.rs:think`, `choose` | **PORTED** | verified | low |
| 0x408280 | — (controller vt+0x9C, willingness) | refuse an object for 1 s, else a world-generator draw under the row's odds | `ai/actions/manager.rs:willing` | **PORTED** | verified, the draw on the shared generator | low |
| 0x403120, 0x401990, 0x4234A0, 0x403560 | AIAction_Status, AIAction_IsDone, AIAction_Ctor | action status, replace by priority, base ctor | `ai/actions/manager.rs:done` | **PORTED** | verified | low |
| 0x41BCF0, 0x413CC0 | —, RaceGoal_TakeJunction | goal slot 10: take a junction by the skill row's odds (shortcuts) | `ai/route/walk.rs`, `sim/ai/racer.rs:racer_think` | **PORTED** | verified (0x413E33–0x413EC9, world generator at 0x413DCB / 0x413E57) | low |
| 0x5EF000 | — (start hold) | racers 100–105 wait 12 − 0.5·(n − 100) s before branches, passes and unsticking | `ai/route/walk.rs:start_hold` | **GUESSED** | verified rule; the racer number 100 + i is observed in Quick Race only, inferred for career | low |
| 0x40D980 | AICtl_HumanSpeedJitter | every 1 s a factor 1 ± 0.1 (world generator) for a human-class car with car +0x9A8 set, else 1 | — | **MISSING** | verified in this map's trace; the reader of controller +0x790 and the meaning of car +0x9A8 are **open**; never runs for AI racers (class 2) | low (draw order when it runs) |

## Route, driving line and speed

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x42CDC0, 0x413590, 0x5E2E50 | — | road node search: race records by mask, coordinate range first | `ai/route/walk.rs:find`, `restart` | **SIMPLIFIED** | verified; the old-node direction blend is not used and the walk's start passes no race coordinate (the record decides in a race) | low |
| 0x422220, 0x41EC20, 0x418C50, 0x418DA0, 0x41A180 | AIRoute_Walk, … | the racer's walk: points, 8 m fill points, Hermite bend of the lane offsets | `ai/route/walk.rs` | **PORTED** | verified; read under Wine (record 476 and 655 fill points) | low |
| 0x41EE20, 0x418A50, 0x40B0B0 | AIWalk_AddTurn | corner apex at a bit-26 junction: trim, insert, walk to the apex | `ai/route/walk.rs` (lane line at s_t) | **SIMPLIFIED** | verified; the clone takes the lane line at s_t and skips the trim and insert; the comparands' vectors are **open** | med: line through turn junctions |
| 0x422780, 0x419000 | —, AIWalk_BendTurnLanes | after the speeds, bend the lane offsets of each turn record (+0xD0, 0x80 bytes) on Hermite curves | — | **MISSING** | verified in this map's trace (0x419000 adds 0x4123C0 of a Hermite blend to each walk point's +0x04 over a turn's points) | med: same as the apex row |
| 0x40A8C0, 0x411DD0 | — | lane speed bytes per point; speed at s | `ai/route/lane_speeds.rs`, `ai/route/query.rs` | **PORTED** | verified | low |
| 0x5E9E90, 0x5DDFD0, 0x5E8790 | — | racing-line speeds rewritten at race load (50 rounds), by speed mode and class | `ai/route/lane_speeds.rs:racing_speeds` | **SIMPLIFIED** | verified; observed career class 0 at load; the writer of that 0 is **open** and taken as observed | low |
| 0x42B340, 0x41D530, 0x420D40, 0x40FC80 | — (JustDrive path) | the next 20 points, back to the line at 0.05 m per m (0.2 in Street X) | `ai/route/path.rs:line_path`, `ai/actions/manager.rs:just_drive` | **PORTED** | verified; observed: JustDrive held the plain plan in 971 of 973 samples, the clone the same | low |
| 0x41C5E0 | — (target speed) | speed at s + a 5-s offset draw × row factor × catch-up | `ai/route/target_speed.rs`, `sim/encounters/ai.rs:goal_speed` | **PORTED** | verified; draw on the shared generator | low |
| 0x4172D0, 0x4084A0, 0x413390 | — (catch-up) | every 0.3 s: gap curves by skill, line-speed term, fade; off in drag and drift | `ai/route/target_speed.rs:catch_up_multiplier` | **PORTED** | verified; observed +0x770 0.6267 / 0.6514 equal to the formula; career circuit 0.774–0.798 vs 0.796–0.808 | low |
| 0x40F690, 0x5EB210 | — | the human's line-speed mean for the catch-up | `sim/race/step.rs:race_ai_pass`, `ai/route/target_speed.rs` | **SIMPLIFIED** | verified; the human's road node uses the human-mask search, which is still partly **open** (racer +0x8FC) | low |
| 0x415F80, 0x404EF0 | AIDriver_LookAhead, AI_PathFirstAhead | look-ahead point on the path, corner excess × 7 in the first 5 s | `ai/driver/car.rs` | **PORTED** | verified; observed look-ahead within 0.25 m in Street X | low |
| 0x415F80 curve 0x7E9DE4 | — | look-ahead class curve on movers 3 / 4 | — | **NOT NEEDED** | verified: no racer car takes it in the retail data | none |
| 0x419E10 | AI_ListBend | the bend used by the pass side | `ai/route/query.rs:bend` | **GUESSED** | the value from three line points (0x419F39 on) is **inferred** | low |

## Low-level driver

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x425AB0 | CarDriver_Ctor | driver, throttle cap +0x22C by difficulty for class 2 | `ai/driver/mod.rs` | **PORTED** | verified (0x425B44–0x425B67) | low |
| 0x421030 | AIDriver_Update | mode by mover, look-ahead, steering, pedals, countdown, nitrous hold | `ai/driver/car.rs` | **PORTED** | verified; observed frame 0 and 1 at the green match | low |
| 0x409B50, 0x409DA0, 0x579F30, 0x576980 | — (adaptive PID, running mean) | steering and speed PIDs | `ai/driver/pid.rs` | **PORTED** | verified | low |
| 0x41D8D0, 0x4183F0, 0x576BC0 | AIDriver_SpeedControl, — | speed control and its trackers | `ai/driver/car.rs` | **PORTED** | verified | low |
| 0x43E6D0 | — (angles) | arcsine table angles | `ai/driver/math.rs:angle16` | **PORTED** | verified | low |
| 0x59FA40 | — (nitrous hold) | when the driver holds nitrous | `ai/driver/car.rs` (`nitrous_hold`) | **PORTED** | verified | low |
| 0x4057C0 | — (slip trackers) | slip of the driven pair | `ai/driver/car.rs:track_slip` | **PORTED** | verified, observed | low |
| 0x425E40, 0x416120 | CarDriver_DriftCtor | DriftDriver: hold, target ≤ 31.29 m/s | `ai/driver/drift.rs` | **PORTED** | verified | low |
| 0x421030 at status 3 | — (countdown and green) | gas 1, brake 1, gear 1 in the countdown; release at the green | `ai/driver/car.rs:scene_hold`, `green_release` | **PORTED** | observed in circuit, sprint, Street X, drag (`circuit_cd`) | low |
| — (who clears the brake at the green) | — | the green's update releases the brake | `ai/driver/car.rs:green_release` | **GUESSED** | observed only; the writer is **open** | low |

## Actions: overtaking, blocking, avoiding

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x42B570, 0x4235A0 | — (JustDrive) | plain plan; hand-back of a pass / block offset when clear | `ai/actions/manager.rs:just_drive` | **PORTED** | verified | low |
| 0x40E390 | AIDriver_NitrousRequest | nitrous ask in JustDrive, Pass and AvoidAvoidable (far) | `ai/actions/manager.rs` (JustDrive only) | **MISSING** | verified calls; not ported in Pass (target within 5 m/s) and AvoidAvoidable (far); the racers' nitrous outside drag is **open** | med: rivals pull away less on straights |
| 0x42B5B0, 0x429890, 0x41D2D0 | — (Pass proposal, plan beside, side) | find a racer ahead to pass; offset beside it | `ai/actions/plans.rs:find_pass`, `pass_plan`, `side` | **PORTED** | verified; observed Pass at rate 0.05, offsets ±4.0…6.2 m vs ours ±2.9…6.0 m | low |
| 0x42B940, 0x42B7F0, 0x40E4A0 | — (Pass update, done, given up) | keep the side plan, retarget, end after 3 s | `ai/actions/manager.rs:pass` | **PORTED** | verified | low |
| 0x403A40 | AI_CanReach | can the car reach a target: accelerations against the gap | `ai/actions/entries.rs:reachable` | **GUESSED** | verified formula; the accelerations are taken from the velocity change over a frame (**inferred**) | low |
| 0x414130, 0x423A10, 0x41CC40, 0x40E5D0 | — (Block) | block a faster racer behind; offset onto it | `ai/actions/plans.rs:find_block`, `ai/actions/manager.rs:block` | **PORTED** | verified; rows 200 / 200 / 400 by racer row (observed 1 for every rival); not seen under Wine (no follower close enough) | low |
| 0x428DF0, 0x428CE0, 0x4275E0, 0x420B80 | — (clear test, sweep) | sweep the car along its path against the others' paths for 3 s | `ai/actions/sweep.rs:clear`, `sweep`, `predicted` | **SIMPLIFIED** | verified; not ported: a car going backwards (0x425910 / 0x425980), the human's path (taken as a traffic car's prediction, `ai.actions.swept`) | low |
| 0x6148F0 | AIPlanner_BuildRoadEntries | the objects around the car | `ai/actions/entries.rs:entries` | **SIMPLIFIED** | verified; extents from the handling dimensions (**inferred**), no race-coordinate window and object filter (0x5EC070, car +0x550) | low |
| 0x404910 | AI_EntryReach | the reach max(125 m, speed × vt+0x60) | `ai/actions/entries.rs:reach` | **SIMPLIFIED** | the vt+0x60 term is **open** (`ai.actions.predicted`) | low |
| 0x42BA20, 0x4243B0, 0x42F330, 0x428FD0, 0x42BA80 | — (AvoidAvoidable) | step the offset in the room, else the 999 fallback; near / far speed | `ai/actions/manager.rs:avoid`, `ai/actions/plans.rs:avoid_plan` | **SIMPLIFIED** | verified; observed rate 999 and −6.5 m; not ported: +0x220's road-section test (never set in races seen), the nitrous ask; contact kind taken as any car contact (**inferred**) | low |
| 0x423CC0, 0x403320, 0x402AE0, 0x423E20, 0x41CC90 | — (IJustGotHit) | move off the hitter at rate 0.25; speed by who hit | `ai/actions/manager.rs:got_hit` | **SIMPLIFIED** | verified; observed type 0xB at 400, rates 0.25 / 0.05; 0x40DF50's road-edge state is taken as never (**open**) | low |
| 0x423E20 tail (0x424083 …) | — | reversing out past an edge: target from 0x40DEE0, rest skipped | — | **MISSING** | verified, not ported | low |
| 0x4241C0, 0x424210, 0x4033D0, 0x6012C0 | — (AccidentRecovery) | a rolled racer waits 5 s, then the reset on its road node | `ai/actions/manager.rs:recover`, `sim/race/frame.rs:reset_rival` | **GUESSED** | verified; the node is the walk's current FuturePoint (**inferred**); a drag lane's lateral (0x403DB0 / 0x5E2DB0) not ported | low |
| 0x5ECF30 | Car_SetDebugTint | debug colour from Block and AvoidAvoidable | — | **NOT NEEDED** | verified: a debug tint | none |

## Mistakes and recovery (GetUnstuck)

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x40E7C0, 0x423C30, 0x426230 | — (stuck test, ctor) | off route over 20 m, leaving the road, or a front / side contact while slow; once per 6 s | `ai/driver/unstuck.rs:update` | **PORTED** | verified; start hold and the AccidentRecovery priority rule ported | low |
| 0x420940, 0x404CD0 | — (GetUnstuck begin) | reverse at −6 m/s (−15 for class 3) along the route, steering mirrored | `ai/driver/unstuck.rs:first_path`, `travel_frame` | **PORTED** | verified | low |
| 0x40E9F0, 0x414440, 0x4143F0, 0x4032A0 | — (given up, done, facing point) | end on a rear contact, leaving the road, or facing the point | `ai/driver/unstuck.rs:update`, `facing`, `ai/route/path.rs:facing_point` | **PORTED** | verified | low |
| 0x40DF50, 0x401C10, 0x402A40 | — | leaving the road; past the edge; contact side | `ai/driver/unstuck.rs:leaving_road`, `past_edge`, `contact_side` | **PORTED** | verified | low |
| 0x4083E0, 0x40D4F0, 0x42C2F6 | — | minimum run 1.3 s, 1.6 s when off the road and the progress mean is low | `ai/driver/unstuck.rs:travel_frame` | **GUESSED** | verified; the clone treats every race as a coordinate race (**inferred**) | low |

## Movers

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x42C0C7–0x42C2DE, 0x402450, 0x5F4410 | —, RacerCtl_OnContact | physics hold 5 s, then MeloMvr (3) or BeltMover (4) by the world loaded | `sim/ai/mover.rs:racer_mover`, `belt_wanted`, `leave_belt` | **PORTED** | verified; observed rivals on mode 3 after the hold, 4 out of the loaded world | low |
| 0x5A9040, 0x59EC90 | — (BeltMover) | belt line with constant acceleration | `ai/belt.rs:build`, `step` | **SIMPLIFIED** | verified; the hand-back to physics on a belt contact stays out (**open**: the clone's belt lines cross race barriers); lap times within 0.5–1.5 s by row | med: a rival on a belt can lose time against a barrier |

## Per race type

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x42D470 | AI_ChooseAction | GetUnstuck, type 0x12, wall hit 0xF, hit 0xC, AvoidAvoidable, else Drive Traffic Lane | `ai/drag.rs`, `ai/actions/drag.rs:drag_avoid` | **SIMPLIFIED** | verified list; GetUnstuck, AvoidAvoidable and the lane action ported; the chooser's tests for 0xF / 0xC / 0x12 not traced | med |
| 0x403450, 0x4249C0 | — (wall hit in drag, 0xF) | after a wall contact: nearest lane, offset 0, rate 0.151 | — | **MISSING** | verified apply test and update; not ported | med: drag rival after a wall touch |
| 0x403410, 0x424560 (0xC), 0x42DCB0 (0x12) | — (drag got-hit, type 0x12) | drag hit and type 0x12 actions | — | **MISSING** | ctors traced only; update and end **open** | med |
| 0x4242D0, 0x420ED0, 0x410CA0 | — (Drive Traffic Lane in drag) | the drag lane; target 1000 | `ai/drag.rs`, `ai/actions/drag.rs` | **PORTED** | observed target 1000; lane path steer within 0.03° | low |
| — (drag shifts) | — | shift offset from the skill row, shown rpm, grade | `ai/drag.rs` | **PORTED** | verified and observed (drag shift sweep) | low |
| — (Street X) | — | line rate 0.2, no Block, facing point 1 ahead, catch-up bytes 0 | `ai/route/path.rs:line_path`, `ai/actions/manager.rs` | **PORTED** | verified; observed steer within 0.05° for 4 frames | low |
| 0x42D32D, 0x42D92D (Street X block skip) | — | a block keyed on the racer row in two choosers | — | **MISSING** | **open**: not traced further | low |
| — (Lap KO, URL) | — | the road goal with race type 3 / URL tracks | as above | **PORTED** | observed launches and distances within a few metres; URL action changes 9–15 vs 4–6 per 20 s | low |
| 0x5260C0, 0x5B6AE0, 0x5B54A0 | — (upgrades from skill) | the rivals' parts from rating and handling score | `bin/game/flow/race_setup.rs:start_round` | **PORTED** | observed: identical fitted parts and rating 196.564 | low |

## Encounter racers in free roam

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x4172D0 (race type 0) | — | free-roam encounter racers aim at 0.75 × their route speed | `sim/encounters/ai.rs:goal_speed`, `ai/encounter_controller.rs` | **GUESSED** | verified rule; the free-roam value is not read under Wine | low |
| 0x42A5E0, 0x42A360, 0x42D6F0, 0x42DA70 | Outrun_PlaceOpponents, AI_FollowerUpdate, — | encounter placement, follower, chase and offset-pursuit choosers | `ai/outrun_search.rs`, `ai/actions/pursuit.rs`, `ai/encounter_controller.rs` | **PORTED** | see [outrun.md](../outrun.md) | low |
| — (encounter cars passing traffic) | — | AvoidAvoidable and Pass against traffic cars | `ai/actions/` | **MISSING** | observed: encounter–traffic contact records 8.5 per 5 min vs 1.5, over 30 force: 3.0 vs 0.33; traffic tracks an encounter car in Avoid Cars 0.035 vs 0.019 of placed samples. The rule that differs is not found | med: traffic stopped for good, queues |

## Functions with no earlier trace

These 159 functions are reached from the racer's vtables, but no doc or code named them before this
map. They are sorted by their code (calls, stores, size); 0x419000 and 0x40D980 have their rows
above.

| Address | Name | What it does | Ours | Status | Evidence | Risk |
|---|---|---|---|---|---|---|
| 0x402720, 0x404820, 0x4049F0, 0x405350, 0x408EF0, 0x409240, 0x409AD0, 0x409CC0, 0x40AC20, 0x40DAC0, 0x40DE20, 0x40E450, 0x40E590, 0x40E760, 0x40EB80, 0x40EBD0, 0x40EC50, 0x40ECD0, 0x40ED10, 0x411790, 0x413910, 0x413960, 0x428260, 0x429A70 (24) | — | replay snapshot sections of the controller, driver, goals and actions | `sim/replay/snapshot.rs` (own format) | **NOT NEEDED** | verified role: each calls 0x578CF0 (opens a named section) and 0x571650 / 0x571780 / 0x5716D0 (write or read a field) | none |
| 0x402170, 0x402350, 0x404810, 0x405780, 0x405B30, 0x405B40, 0x405B50, 0x405B90, 0x408380, 0x408390, 0x409C50, 0x409F80, 0x40A210, 0x40CA70, 0x40CAB0, 0x40CF90, 0x40DA80, 0x40DAF0, 0x40DB20, 0x40DB70, 0x40DD90, 0x40FEA0, 0x4101E0, 0x4103A0, 0x411740, 0x411B50, 0x415DB0, 0x4169B0, 0x41BE00, 0x41BE90, 0x41C290, 0x41E480, 0x41E710, 0x420500, 0x420FD0, 0x422110, 0x422130, 0x423570, 0x4235D0, 0x4235E0, 0x4236D0, 0x4236E0, 0x423710, 0x423780, 0x423850, 0x423960, 0x423970, 0x4239A0, 0x423B60, 0x423CB0, 0x423D70, 0x423D80, 0x423DB0, 0x424200, 0x424290, 0x4244B0, 0x4244C0, 0x4244F0, 0x424600, 0x424610, 0x424640, 0x424980, 0x4249B0, 0x424B00, 0x424C40, 0x425100, 0x425590, 0x4255D0, 0x425630, 0x425860, 0x429B00 (71) | — | constructors, destructors, deleting stubs, type, name and priority getters, goal setters (0x429B00 sets controller +0x6FC to a goal of vtable 0x784AF8) | the clone's own types | **NOT NEEDED** | verified by shape: a vtable store with an exception frame, or at most four instructions | none |
| 0x403780, 0x40F010, 0x40F070, 0x40F0D0, 0x414AF0, 0x414BF0, 0x414D20, 0x40EB60 (8) | — | debug markers drawn along the path (solid texture remap, bin hash) and GetUnstuck's debug tint | — | **NOT NEEDED** | verified by calls (0x488E20, 0x43DB50, 0x6147F0, 0x5ECF30) | none |
| 0x405BE0, 0x407110 (2) | — | the human's input hooks (hydraulics) reached through the driver | human input | **NOT NEEDED** | verified by calls (0x407050) | none |
| 0x4246B0, 0x4034A0 (2) | — | drag hit action's update (vtable 0x78588C slot 2) and drag wall contact test (0x5A0830, 0x5A0170) | — | **MISSING** | verified by calls; part of gap 3 | med |
| 0x4233E0 | — | lane path when both entries exist, else the lane at the place (0x40F4A0 → 0x405410) | — | **MISSING** | verified in this map's trace; caller 0x42CDC0 | low |
| 0x410120 | — | driver slot +0x84: a ±90° side (0x4000 / 0xC000) drawn on the world generator for a done racer | — | **MISSING** | verified in this map's trace; the reader is **open** (likely the pull-over after the finish) | low |
| 0x4259C0 | AI_PredictOwnPath | the car's own predicted path over max(125 m, reach × speed) | `ai/actions/sweep.rs:predicted` | **PORTED** | verified call into 0x420B80 (traced in the sweep row) | low |
| 0x401120, 0x401173, 0x4011E3, 0x401730, 0x401A30, 0x401CA0, 0x401F20, 0x401FA0, 0x402330, 0x403180, 0x4039F0, 0x403FC0, 0x404950, 0x405270, 0x4052C0, 0x405460, 0x405680, 0x4056A0, 0x4056C0, 0x405720, 0x405790, 0x405910, 0x405D00, 0x408330, 0x409F40, 0x40A4D0, 0x40A5D0, 0x40C9D0, 0x40CA00, 0x40E6E0, 0x40F150, 0x40F6B0, 0x40FC40, 0x40FFD0, 0x410010, 0x410580, 0x4105B0, 0x410620, 0x413860, 0x4138B0, 0x4183F0, 0x41A030, 0x41BEC0, 0x41BEF0, 0x41BF10, 0x420B40, 0x428250 (47) | — | small helpers: min / max, a point beyond a record end, a plan copy (0x403FC0), the braking rate (0x4039F0, as 0x40DEE0), the bend at a point (0x41A030 → 0x419E10), the lateral at a point (0x40E6E0), speed-tracker steps (0x4183F0), controller and driver getters | inside the ported callers | **GUESSED** | role read from the code for this map; not compared value by value with the ports that inline them | low |

## Gaps, ranked

Ranked by what a player notices. Each fix is checked with the parity harness from the same start
([parity-harness.md](../../developing/parity-harness.md), statistics with `aistats`) or by memory reads
of the original under Wine (controllers found by vtable 0x785370; action +0x6F8 → +0x200 / +0x204 /
+0x218; lane record +0x54 +0x30C). All belong to #141.

| # | Gap | Rows | What a player sees | How to verify |
|---|---|---|---|---|
| 1 | Encounter cars hit traffic about six times as often | Encounter cars passing traffic (MISSING) | free-roam racers plough through traffic; traffic cars stand for good with queues behind | traffic harness `bot` with the contact log against the original's census: encounter–traffic records per 5 min 8.5 → about 1.5, stops for good 3.9 → about 2.1 |
| 2 | Corner apex at turn junctions not ported (apex insert, turn lane bend) | 0x41EE20 (SIMPLIFIED), 0x422780 / 0x419000 (MISSING) | rivals take turn junctions on the lane line instead of the apex | trace which walks meet bit-26 junctions; per-frame line comparison on a race that crosses one (rival lateral off the line, lap time) |
| 3 | Drag race's own hit and wall actions | 0x42D470 (SIMPLIFIED), 0x403450 / 0x4249C0, 0x424560 / 0x42DCB0 (MISSING) | after touching a wall or a rival, a drag rival keeps its offset instead of going back to its lane | Wine: drag rival action type +0x204 (0xF, 0xC, 0x12) after a contact; harness drag runs with the bot: totals and finish times |
| 4 | Nitrous asks in Pass and AvoidAvoidable | 0x40E390 (MISSING) | rivals use less nitrous when passing on straights | Wine: the rival's nitrous flag and amount during Pass; harness: speed on straights after a pass |
| 5 | BeltMover hand-back to physics | 0x5A9040 / 0x59EC90 (SIMPLIFIED) | a rival far away can lose time on a barrier | first-lap times by row against the original (six seeds × 3 cars); belt contact records |
| 6 | Reversing out past an edge (IJustGotHit and GetUnstuck tails) | 0x423E20 tail (MISSING), IJustGotHit (SIMPLIFIED) | a hit rival recovers differently off the road | Wine: target +0x1F0 after a hit near an edge |
| 7 | Small untraced parts: start-hold number in career, the bend 0x419E10, reach term vt+0x60, sweep of a reversing car | GUESSED / SIMPLIFIED rows | rare differences in passes and start behaviour | harness Pass offsets and action counts per 20 s |
| 8 | Helpers named in this map without a value-by-value check | Functions with no earlier trace (GUESSED, MISSING rows) | none expected | decompile each against its inlined port |
