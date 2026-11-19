# Race AI parity with the harness (2026-11-04)

The race AI compared frame by frame with the original (SPEED2.EXE under
Wine, the maintainer's tools, docs/parity-harness.md) from the same save, grid,
cars and inputs. **Verified** = traced in SPEED2.EXE, **observed** = read
under Wine, **inferred** / **open** as marked. Raw runs are
kept outside the repository; reports, plans, car lists
and `aistats-after.txt` in the maintainer's private captures. The player only
holds the throttle in these scenarios (it hits walls and rivals), so after
the first seconds the comparison is statistical (aistats), not per frame.

## The parity harness's findings 1–5

| # | finding | result |
|---|---|---|
| 1 | rivals brake 1, gas 0.52, steer 3° at the green; original full gas, steer 0 | **fixed** (below): frame 0 and 1 now match (gas 1, brake 0, gear request −1, steer 0 / the same first steer) |
| 2 | player launches quicker in a sprint (5.06 vs 3.43 m/s at 1.2 s) | **not a launch difference**: with the same cars and grid the player's speed matches within 1.5 % until 1.2 s, where a rival rear-ends it (MIATA 6.5 → 5.8 m/s, player +1.7 m/s in a frame); the clone's rival arrived 14 frames earlier because the rivals launch quicker (open item A) |
| 3 | rest height 0.25–0.35 m lower | **reference point**: car +0x60 is the centre of mass, the clone's position the body origin (`CarState::placed`); the harness now compares origin + turned centre of mass: frame-0 z within 0.01 m |
| 4 | drag player gas drops to 0 after 0.45 s | **harness, inferred**: the throttle handler's value byte +0x2D0 turns from the injected 255 to 172 (0.5 s) and 0 (1.17 s) though the cave sends 255 before every update; another caller (the game's own poll of the unpressed key) overrides it (docs/parity-harness.md). Not native drag behaviour |
| 5 | player grid slot rule | **verified, random**: 0x5264B0 → 0x4FEDE0 (one human): swaps racer 0's slot +0x03 with that of racer 0x43C1C0(humans + AI) (itself possible); the clone has it (`GridOrder::Shuffled`). The harness now copies the whole frame-0 grid |

## 1. The start: scene, countdown, green (fixed)

Traced: the driver update **0x421030** with race status 3 (`*0x890118 == 3`)
asks gear 1 (+0x1E4), 0x5EB900(1.0) (brake +0x214) and 0x5EB920(1.0)
(gas +0x20C), after its speed control 0x41D8D0 (which starts from the
input's gas − brake, +0x20C − +0x214). With no path (+0x10 < 1) the
look-ahead 0x415F80 is straight ahead and the target +0x230 is 0.

Observed (`circuit_cd`, arm at the countdown's first frame, two runs):

| | original | clone before | clone after |
|---|---|---|---|
| scene (status 2) | engines idle in neutral (≈ 850–900 rpm, gear 1 at the countdown's first frame) | revving (countdown pedals) | idle, neutral, brake (`AiDriver::scene_hold`) |
| countdown | gas 1, brake 1, gear request 1, steer 0, no goal / action (+0x14, +0x204 read 0), target 0, wheels down | same pedals, steer 2.8–3.6°, JustDrive running | gas 1, brake 1, steer 0, no action |
| first status-4 frame | gas 1, brake 0, gear request −1, steer 0, target 0, no goal | brake 1 | same (`AiDriver::green_release`) |
| next frame | goal 1, action 6, target set, steer on the path (0.96–2.6°), gas 1 | gas 0.5 (0 + rate) | gas 1, action 6, steer within 0.05° in Street X, 1–2° in the circuit |

The race goal is given at the green (0x42C060 → 0x42AA20(1), 2 / 3 in a
drag race, **verified**); its first action runs in the next update
(**observed**). Who clears the brake in the green's update is **open**
(observed only). Drag: the original does the same at the green (gas 1,
brake 0, then the drag cap 0.93; drag_s1). The clone's green release applies in drag
races too since the drag shift sweep (docs/tuning-physics-parity.md C; the first_drag flow
passes its 24 runs with it). The scene
hold and countdown rules apply in drag races too.

## 2. Catch-up in career races (fixed)

Observed: in every career run the option 0x89E7F8 is **1** and the
controllers' catch-up multiplier (+0x770) falls to 0.78–0.80 for the
leading rivals after the row's hold (≈ 6 s); 0x89E7F8 = 1 and 0x89E7FC =
1.0 already in the boot's first frames (`runs/catchprobe.log`). The
career set-up 0x5322F0 does not write it (only 0x89E7A8 = event +0x36,
0x53236C), the Quick Race set-ups do (0x53F225 … from the mode record);
the boot-time writer is **open**. The clone gave career races no catch-up
at all (`race.catch_up = None`). Now a game-wide `catch_up_globals`
(option, Quick Race difficulty) starts at (on, scale 1) and is rewritten
by each Quick Race set-up; every race reads it, except drag and drift
races: 0x4172D0 clears its flag there (0x417304..0x41732A, **verified**).

| circuit_s1 (6–20 s, mean of 3 rivals) | original | clone before | clone after |
|---|---|---|---|
| catch-up multiplier | 0.774 / 0.777 / 0.798 | 1.0 | 0.796 / 0.798 / 0.808 |
| target speed (m/s) | 29.3 / 29.5 / 31.3 | 37–41 | 30.1 / 30.5 / 31.1 |

## 3. Drag lane target (enabled in the drag shift sweep, docs/tuning-physics-parity.md C)

Observed: the drag rivals' target +0x230 is **1000** (the controller cap
0x784550) from the first think; the clone keeps the line speed (59 m/s on
4201, which caps a rival at 213 km/h near the finish). Taking 1000 made a
rival tip in a rival-rival contact (hardest push 104) at 29 s in the
first_drag test's Restart run, so it is left for the crash/contact work;
the harness accessor reports action 7 (AIActionDriveTrafficLane).
The clone now takes 1000 (`DRAG_TARGET_SPEED`); the first_drag flow's
24 runs pass with it.

## Per race type (after; aistats: speed at 1 / 2 / 5 s, m/s)

| type (scenario) | launch at the green | first steer / path | v1s native → clone | catch-up 6 s+ | open |
|---|---|---|---|---|---|
| Circuit (circuit_s1, 4013) | match | path and look-ahead match; first steer 0.96 vs 2.8 (one rival) | 4.2–4.4 → 4.5–4.7 | 0.77–0.80 → 0.80–0.81 | A, B |
| Sprint (sprint_s1, 4101) | match | first steer 1.7–2.0 vs −1.2–0.9 (one rival) | 4.3–5.3 → 4.5–5.4 | (not recorded) → 0.78–0.79 | A, B |
| Street X (streetx_s4, 4601) | match | look-ahead point within 0.25 m, steer within 0.05° for 4 frames | 5.9–6.9 → 7.0–7.5 | 0.87–0.93 → 0.84–0.89 | A, B |
| Drag (drag_s1, 4201) | match (gas 0.93, steer) | lane path steer within 0.03° | 4.1–5.8 → 4.5–6.4 | — | A, C, §3 |
| Lap KO (lapko_qr: Quick Race circuit, Resort Loop, Lap KO on, race type 3) | match | — | 4.3–4.8 → 4.2–5.1; distance 469/477/474 → 456/474/474 m | 0.84–0.88 → 0.80–0.85 | A |
| URL (url_qr: Quick Race, Bayview Speedway Track 1, 5 rivals) | match | — | 4.5–6.0 → 4.5–6.1; at 2 s 8.6–11.1 → 10.7–12.7 | 0.80–0.89 → 0.80–0.89 | A, B (9–15 action changes vs 4–6) |

## 4. The controller's think schedule in races (fixed, B)

The rivals chose their action every frame; the original's racer
controller (0x42ABD0, owner class 2) runs its full think (goal search,
action choice and update) only on world updates whose counter 0x7FB724
mod 8 (mod 4 in a drag off mode 4) is the racer's place among the AI cars
and 0.2 s after its last; in between its fallback runs the goal's speed
and, for an action whose vt+0x10 is true (Block), the route and the
action update; otherwise the driver keeps the last path (**verified**,
docs/outrun-think-scheduling.md; ported for the outrun racers by the outrun-strength study).
**Observed** in races: rival k's action changes all fall on one residue
mod 8 (circuit_s1.v3: rival 1 frames 132, 308 ≡ 4; rival 2 45, 253, 301,
653, 701, 893 ≡ 5; rival 3 46, 670, 814, 942 ≡ 6). Now race rivals use the
same `ThinkSchedule` and keep the action's output between full thinks
(`Rival::think`, `Rival::retained`; drag lanes unchanged). After: the
clone's changes fall on the same residues (rival 2's at frames 45 and 253
as the original's); action changes per 20 s, original → before → after:
circuit 3/7/5 → 8/7/5 → 5/6/9, Street X 9/3/5 → 12/17/13 → 10/13/9.

"I just got hit" (0xB): its end test 0x41CC90 is 0x402AE0(planner, −1, 1)
= no live contact record (+0x60 ≥ −1 always); a record lives 0.3 s after
its last push (0x592BE0, 0x7FBEE0 = 0.3); the clone has both. The
original's 2.4 s hit was a rival rubbing side by side with the player
(3.0–3.4 m apart for 2.4 s); the clone's run had no such contact, so the
durations differ by situation, not by rule.

## 5. Rivals' performance (A)

Read under Wine (circuit_s1.v5: racer infos [0x89E810] + 0x940·i, state
floats +0x300, fitted part bytes +0x350, rating +0x28) against the
clone's fit (`NFSU2_DUMP_RIVAL_RECORDS`): FOCUS fitted 0x30, 0x32, float
7 = 0.6, rating 196.564; COROLLA 0x0F 0x10 0x11 0x12 0x1E 0x28 0x30 0x32,
floats 3 / 5 / 6 / 7 = 0.99 / 0.34 / 0.45 / 0.6 — the clone fits exactly
the same parts. Car +0xC is the stock record in the original (no
upgrades written into it), car +0x20 the upgrade state. So the quicker
launch is not the fit; it is the launch physics (open, below).

## 6. Lap KO and URL (D)

Neither has a career event the harness can start from a world trigger
(Lap KO is a Quick Race option; URL events start from the map), so the
harness got a Quick Race start (`kind = "quick_race"`, docs/parity-harness.md):
the original is driven from the main menu by the scenario's keys (short
`tap:` presses: a normal press moved two entries in the mode carousel and
the options list), the clone starts the same mode's first track
(`quick_event`, `quick_car`). Observed: Lap KO sets race type 0x89E7B0 = 3;
the Quick Race mode order is Circuit, Sprint, Drag, Drift, Street X,
Underground Racing League, Free Run; the Game Options rows start with
Track Direction, then Lap KO; Continue on the car opens the Transmission
dialog (Auto). Results in the table above.

## Open

* **A. Rival launch ≈ 10 % quicker**: resolved in docs/tuning-physics-parity.md A (the "5500 rpm/s fall"
  was the shown rpm's smoothing; the green's gear change came an update early; the replay manager's
  DUMMY_SNAPSHOT every 100 world updates is ported: launches within 0.1–0.3 m/s of the original at 1 / 2 / 5 s; tests
  `the_dummy_snapshot_lets_spinning_wheels_grip_for_a_sub_step`, `the_dummy_snapshot_drops_the_car_for_a_sub_step`).
* **C. Drag rivals' shifts**: resolved in the drag shift sweep (docs/tuning-physics-parity.md
  C): the drag AI's shift offset ranges from its skill row, the shown rpm
  for the choice and the grade, the slow clutch after a −1 grade.
* **Drag target 1000 and the drag green release**: enabled in the drag shift sweep (the
  first_drag flow's 24 runs pass).
* The countdown length differs between native runs (171 and 237 frames
  from the hold); the clone's is 169. The skill-row jitter (0x41C5E0, the
  car's random state +0xF8) is each game's own.
* Speed differences grow with contacts and the throttle-only player.

## Rival pace in a career race

Measured on both sides from one stage-3 save (Medium), the player's car
parked at the grid for 60 s (and once driving in the field), per rival
every 0.25–0.5 s: the original by memory reads under Wine (car +0x60
position, +0x42C speed, +0x28 mover, controller +0x770 catch-up, driver
+0x230 target speed), the clone by `tests::rival_pace::record_rival_pace`
(diagnostic; `RIVAL_PACE_EVENT`, `RIVAL_PACE_SAVE`, `RIVAL_PACE_DRIVE`,
`RIVAL_PACE_QUICK`). Tables: docs/evidence/rival-pace/pace.txt.

* **The career catch-up runs in both** (0x4172D0 / 0x4084A0): S3_CIRCUIT_12
  rivals read +0x770 = 0.651–0.686 from ~20 s on with the human parked
  (the clone's `SpeedTarget::catch_up` the same values), 0.795–0.83 with
  the human driving among them. An earlier "45 against 26 m/s at 20 s"
  came from a recorder that started the event through the harness's quick
  start (`start_race(_, true, _)` without a Quick Race mode), which leaves
  the race's catch-up off; started as a career race the clone's rivals run
  31.6–33.3 m/s at 20 s against the original's 32.6–37.8.
* **Fixed: belt rivals backing up in corners.** 0x40DF50's forward test
  reads car +0x42C (0x40E142, against 0.0 at 0x7D7D3C), the speed the
  car's mover exports; `leaving_road` read the body's velocity along the
  heading. Through the corner near (200, −1035) of S3_CIRCUIT_12 the body
  of a rival on the BeltMover slid up to 90° off its heading (velocity
  along the heading −0.57 and −2.95 m/s while the belt exported 20.7), so
  the stuck test (0x40E7C0: 0x40DF50(car, 1) = 1 under 25 m/s) started
  GetUnstuck and two of three rivals reversed and were about 400 m behind by 60 s.
  Under Wine no rival read a reverse target in two idle-player runs (the original backs up only from a standstill after a contact: the driving run, Street X). Now
  `leaving_road` reads `CarState::exported_forward_speed`; distances at
  60 s 1557–1647 m against the original's 1306–1723 (two runs).
* Other kinds, idle player, after the fix (60 s, clone / original): a
  stage-2 sprint (S2_SPON_SPRINT_1) 1428–1544 / 1523–1607 m; Street X
  (S2_SPON_STREET_1) skill 5 1363 / 1425 m (the original's two skill-4
  rivals collided off the grid); an SUV circuit (S2_SUV_2) 1290–1356 /
  1200–1361 m. No URL event was reachable on the save (none open at
  stage 3; the World Map's stage-2 list has none).
* **Fixed: career racing lines on the cars' own class.** URL races
  measured from the stage-4 and stage-5 test saves (S4_URL_1, track 4714,
  5 laps; S5_URL_1, 4712, 3 laps; 5 rivals; both started from the event
  trigger, catch-up on, 60 fps on both sides, the player parked): the
  clone's rivals ran 2–4 m/s under the original's at every control point
  with the same skill rows (info +0x901: 5, 5, 5, 6, 6), the same catch-up
  (+0x770 0.677 / 0.702) and the same speed factor; their path points
  (controller +0x6E4 → +0x31C list, point +0x40) carried higher lane
  speeds (record 411 cp 48–50: 41, 39, 40 against 37, 36, 37). The
  racing lines are computed for the field's class (0x5DDFD0, route
  manager +0x4674); in a career race the AI racers' class bytes still read
  0 at the track load, so the class is 0, where the clone took its rivals'
  own class (2 here). Polled under Wine through the load: URL, Street X
  and circuit all class 0 (formats.md, racing-line speeds). Now
  `ai::route::field_class` gives 0 in a career race with AI racers; a
  Quick Race keeps the cars' class. Distance at 60 s, rivals per skill
  row, no contacts (original runs / clone before → after, two seeds):
  S4_URL_1 skill 5 1599–1632 / 1519–1541 → 1617–1638, skill 6
  1704–1753 / 1608–1649 → 1675–1759; S5_URL_1 all 1754–2059 /
  1796–1963 → 1851–2067 (the original's runs spread wide: contacts);
  S3_CIRCUIT_12 1615–1734 / 1552–1646 → 1664–1755; Street X S3_STREET_6
  (4603) 1209–1265 / 1130–1193 → 1120–1277 (the skill-4 rival 1162 /
  1120 against the original's 1221 / 1255, **open**). Tables:
  docs/evidence/rival-pace/url-pace.txt.
* With the player driving (the original: a line-following script on the
  clone's autopilot path; the clone: the autopilot), the humans' own pace
  differs (URL 1714–1740 m against 1911 m at 60 s; circuit 1635 against
  1957–1975), so the catch-up differs and these runs are not judged.
* **Open:** why the belt-placed body's velocity (`place_on_road`: the
  direction of the frame's displacement) points up to 90° off the heading
  in that corner; the original's +0x70 for a mode-4 car was read only as
  a magnitude (equal to +0x42C within 0.1 m/s).

## Tests and flows

* `first_race::tests::rivals_hold_the_line_and_launch_at_full_throttle`
  (asset): the scene's idle engines, the countdown's pedals (steer 0, gas
  1, brake 1, neutral, no action), the green's update (brake 0, gear −1,
  gas 1), full throttle after, career catch-up on (fails on the old code).
* `--flow firstrace`, `firstdrag`, `postrace` pass (`raceai-work/flows/`).
* Scenarios added: `circuit_cd.toml` (countdown arm), `streetx_s4.toml`,
  `lapko_qr.toml`, `url_qr.toml` (Quick Race starts).
* The same test also checks the think schedule: every action change of
  rival i falls on a race update ≡ i mod 8 (the race's own update count
  `Race::ai_frames` and clock stand for 0x7FB724 / 0x865188, so the
  replay, which restores them with the rivals' schedules, reproduces the
  race: `tests::replay_reproduces_the_race`).

## GetUnstuck's end waits for the think slot (2026-11-05)

The clone dropped the controller's retained steer while its unstuck ran, so
the frame after the unstuck ended had no action present and ran a full
think at once (`full_think`: `!action_present`), changing the action off
the racer's slot (first_race test, rival 1 GotHit → Avoid at world update
≡ 4 mod 8 after reversing; exposed by the drag/drift/outrun parity's line-up order). Natively
GetUnstuck is itself the running action (priority 500), so an action is
present when it ends and the next choice comes on the full think
(0x42ABD0's slot, **verified** schedule above). Now the unstuck keeps a
retained steer (its lane path) and replaces the action as `think` with
`fresh` does (kind reset, priority −1: the next choice starts afresh,
`Actions::get_unstuck`); the driver keeps that path until the slot.
**Inferred**: what the original's driver does between the end and the slot
(the fallback 0x42AD16 keeps the last path, so the unstuck path).

## Native fixes against the pinned circuit (2026-11-12)

Scenario `circuit_s1_pinned` (native recording of 2026-11-12 00:25, re-recorded twice in
a private Wine copy; the clone pins the original's world generator at the arm, plan line `world_rng`,
and steps its world update counter from `world_n`). **Verified** = traced in SPEED2.EXE,
**observed** = read under Wine. Evidence: the maintainer's private captures; fix list:
`docs/native-fixes/N-AI.tsv`.

* **Before**: first divergence frame 1 (rival 1's target speed 34.40 vs 37.80, rival 3's
  37.37 vs 40.69).
  * Cause 1 (verified, 0x41C6E0..0x41C731): the target's distance offset is drawn from the
    world generator ([0x890080] +0xF8 via 0x5EA510), the racers in turn; the clone drew it from a
    generator of the racer's own. Observed: the original's world generator steps exactly three
    times on the first racing frame; the clone's now ends that frame in the same state
    (−341504639) and the three targets agree (34.396 / 34.348 / 37.36).
  * Cause 2 (observed): the racers' full-think turn (0x42ABD0, world update counter 0x7FB724
    mod 8 = the racer's place among the AI cars) read the clone's own count of AI passes, two
    behind the world counter; and the 0.2 s gate's tick was truncated from the float race
    clock (799 for 800 ticks), so a turn the original takes was skipped. Rival k's action
    changes (native rows 83 / 36 / 45) now fall on the same frames.
* **After**: first divergence frame 58: the 240SX's contact with the player one frame early
  (physics); every AI action change through frame 300 on the original's frame.
* **The original repeats itself only to ~4.4 s.** Three pinned recordings of this scenario
  (2026-11-12 00:25, one of 2026-11-11, and two in a private Wine copy) agree on every car's place
  to frame 264–280; then the rivals' first-corner contacts part, and at 20 s the player's place
  differs between the original's own runs by 8–131 m (rivals up to 310 m). The ≈ 10 m player
  error at 20 s against one recording is inside that spread; per-frame comparisons of this
  scenario are meaningful only to ~frame 260.
* World draws per frame (`[run] counters`, scenario `circuit_s1_pinned_counts`): the original
  draws 3 offsets at the green and again after 5 s, one odds draw (0x408280) per pass / avoid
  taken (rows 36, 45, 83: the clone the same), 3 per drivetrain reroll (0x594720, every shift;
  the clone's drivetrains now draw them from the world generator too, docs/rng.md "Drivetrain rerolls") and
  further draws the clone does not make: 4 at every whole second (rows 60 k) and ~5 more on the
  frame an avoid starts (**open**: not traced; candidates the AI nitrous strategy 0x416F70 and
  the avoid plan's trial paths).
* Other fixes of the unit (all with a unit test): find_pass examines index 0 (0x42B5B0); the
  road entries' reach max(125 m, 3 s × v) (0x404910 / 0x408240) and the signed look-ahead;
  a failed road-node search searches again every update (0x428720 / 0x42CDC0, within the
  search radius 999999 m); the traffic walk's exit test `length ≤ reach` (0x422300); no start hold for the
  player's car and the human walk's junction types 2 / 8 (0x413CC0 / 0x5EF000); a rival's reset
  re-stamps its placement (0x5F52D0 / 0x5ED590); the human's path for the sweep below 10 m/s
  (0x42CAA0 → 0x425910 → 0x420A60, reach max(125 m, 2 s × v)) and no contact for a racer's empty
  path (0x4275E0).
* **The belt's speed limit (a question from the camera work)**: 0x4038D0 reads [car +0x10] +0x20; car +0x10
  points into a static array 0x890170 + 0x940 · slot (+0x00 the car type index, +0x18 the
  performance rating 196.70 / 195.92 / 179.79, observed); +0x20 = 73.76 / 67.05 / 75.10 m/s
  for the 206 / 240SX / MIATA, exactly 55 / 50 / 56 steps of 3 mph. Not the clone's
  quarter-mile speed (41.5 / 41.4 / 40.5) nor its top-speed loop (55.9 capped / 65.7 uncapped
  for the 206). **Settled** (traced): the writer is 0x5B5CB5 in 0x5B5AC0, the gearing-limited
  top speed (the acceleration table's loop end); the clone uses `Handling::gearing_top_speed`
  for the belt, the chase shake and the typed replay camera.
* **The traffic walk's relative turn length**: taken below ("The traffic walk's turns"); the
  objection here came from a test that moved the car a whole control point a think.
* **The human's road node (traced; ported below, "Encounter pursuit")**: 0x411490 is the human controller's vt+0x30
  (vtable 0x785230) and runs from its update 0x42CAA0 → **0x42A190** only when the human's walk
  (controller +0x6C count, +0x70 points; built by 0x428030 with reach max(125 m, 2 s × v),
  vt+0x60 = 0x40A240 = 2.0) cannot keep the car: the list is empty, or the car's distance along
  it is outside [first point's s − 50 m, last point's s], or its lateral distance past the
  road edge (0x40ADA0) exceeds half the car body's width ((+0xF4 − +0xE4) / 2 of the body,
  0x5EC260). Otherwise the node is the list's point at the car (0x401ED0) and the points
  before it are dropped (0x4120C0); after a search the list is cleared (0x411BE0) and rebuilt
  from the node. That is why 0x411490 ran on 86 of 720 free-roam frames and 17 of 1 200 race
  frames (observed). The clone has no human walk yet; its traffic placement still takes the
  nearest segment's start (`road_node`, inferred). Port: a human `RacerRoute` (kind 1:
  junction types 2 / 8 first, no hold), kept by 0x42A190's test, searched by 0x411490 /
  0x40F180 (the audit's rule), its node handed to `TrafficLane::spawn_from`.

## Encounter pursuit, the human's road node, the road entries (2026-11-12)

**Verified** = traced in SPEED2.EXE, **observed** = read under Wine. Fix list:
`docs/native-fixes/AI-T.tsv`; evidence in the maintainer's private captures; game-side wiring for review in the
unit's `ait-game-side.md` (the game binary is being restructured).

* **Encounter cars before a challenge (goal 9, verified + observed)**. The encounter manager's
  state 3 runs 0x42A860 every update: per slot (manager +0x1B10 + 0x10·k: +0 stamp, +4 side state,
  +0xC the car; side state 1 from the placement, 0x42A840) 0x41C100 gives the car's distance along
  its list ahead of the player and its offset off the line less the player's (from the road
  entries; else from the list when both lie on its span; else 0x5D5D60's signed 2D distance and
  0); the side machine (`ai::actions::PursuitSide`) turns them into the offset 0 / +5 / −5 and
  0x428630 sets goal 9 (AIGoalOffsetPursuit, vtable 0x784BF8: target +0x18 the player, +0x1C 10,
  +0x20 the offset). Its chooser 0x42DA70 takes GetUnstuck, IJustGotHit, AvoidAvoidable and
  AccidentRecovery as the race chooser does, never Block or Pass, and otherwise its default action
  0x13 "Drive at offset" (0x426F20, vtable 0x785A18, priority 0, no end). The action (0x42BC50):
  d = −(the player's s less the car's + 10); near (+0x21C) when |d| < 30, kept to |d| ≤ 50; near,
  the lane record {0, the player's offset + the goal's, 0.15}, the driven path kept within 1.5 m
  of the car's offset (cut at max(0.75·v, 30 m), `RacerRoute::offset_path`) else 0x420D40, and the
  speed +0x1F0 = the player's speed (|d| < 2), −2 (d ≥ 0) or +2 m/s, at least 0 and for an AI car
  at least 60 mph (a floor; observed 28.4–46.6 m/s); not near, JustDrive's path and the
  controller's own speed (+0x1FC 1, +0x1F0 read 1000). The controller then takes the least of that,
  the goal's cap (0x40DC00, `outrun::pursuit_cap`) and the line speed × catch-up (0x41C8B1..0x41C962).
  Observed (`encounter_pass`): action 19 near from frame 0, side 2 → 1 → 3 with the offsets −5 / 0 /
  +5, the car 10–60 m ahead of the player at the 26.82 m/s floor, then the player's speed + 2.
* **Encounter cars in an outrun (observed, verified dispatch)**: 0x42EDF0 state 4 gives the race
  order's first (the challenged car) goal 6 on the second (the player) via 0x42A9E0 → 0x428510, and
  every other racer of car kind 2–4 that is out goal 5 (ChaseCar 0x4285A0) on the first;
  `outrun_poll`: MUSTANGGT goal 6 → the player; GOLF and CELICA goal 5 → the MUSTANGGT until parked.
  The challenged car's goals 5 / 6 were ported; the others' goal 5 is ported
  (traced: 0x4285A0 is itself the assigner, new goal 5 unless already type 5, else retarget).
* **Before → after** (`encounter_pass`, the clone forced to the original's model and given the
  original's frame-0 controller state): before, the encounter car drove the plain line under the
  speed cap with no action, side or offset; after, the clone's car runs action 19 near (side 2,
  offset −5 → side 1) like the original, the same car speeds (24.0 / 26.7 / 26.8 m/s at 0 / 1 / 2 s)
  and leaves the near state at 3 s where the original kept it to 9 s: the clone's player
  accelerates slower from the frame-0 pose (5.8 vs 8.8 m/s at 1 s, the player's physics), so the
  car is 67 m ahead (|d| = 57 > 50) where the original's was 59.5 m (|d| ≈ 49.5).
* **The road entries' object locator (verified)**: 0x6148F0 files every object by 0x419C10 →
  0x419B20: 0x4198C0 with flags 0 takes the list point with the least 2D distance to its racing
  point + 10 × the height difference (points from 150 m away skipped, distance capped at 99999), no
  walk to a neighbour; s = 0x40AE30 (+0x54 − +0x58 + the along), extrapolated before the first point
  and past the last; only an empty list files nothing. The clone's `locate` (walked, none off the
  span) had dropped the player from an encounter car's entries 35 m behind it.
  `circuit_s1_pinned`: first divergence frame 58 before and after, the place error per second equal
  to 16 s; at 18 / 20 s player 19.7 / 2.9 → 18.8 / 5.4 m (beyond frame ~265 the original does not
  repeat itself).
* **The human's road node (verified; ported)**: `ai::route::HumanRoad` per world update in the
  controllers' pass (0x42CAA0): 0x40A390's way check (the node's tangent, reversed when backward,
  against the velocity above 10 mph else the forward axis; below −0.707 the way flag flips and the
  walk is cleared), 0x42A190's keep test (`locate_object`; s from 50 m before the first point to the
  last; at most half the body's width outside the road, 0x40ADA0) or 0x411490 / 0x40F180 (the runtime-read rework's
  model), then the walk rebuilt to max(125 m, 2 s·v). `freeroam_node`: 0x411490 on 86 frames in
  both (85 in common: 193–277 and 406 native, 193–277 and 456 clone), 0x40F180 on frame 276 /
  274, the node's record equal on 719 of 721 frames. `circuit_s1_pinned_counts`: the node's record
  equal on every frame to 472; the original's 17 updates fall at frames 1178–1194, past its own
  repeatable span (the clone: none). **Inferred**: half the body's width from the handling
  dimensions (0x5EC260's shape extents not read); 0x40CC40 / 0x40CBC0's backward variants taken as
  the forward formula (the rework's model matched 33 of 34 placements).
* **Trap speed (open)**: car +0x10 points at the array 0x890160 + 0x940·slot plus 0x10; the array's
  elements are copies of the racer infos ([0x89E810], 0x5FC140 / 0x413440; 0x5ED910 indexes it,
  0x5ED6AC sets the base). Every reader passes [car +0x10] + 0x10 to the performance functions
  (0x5B5940 the top speed, 0x5B4DD0 the class), so [car +0x10] +0x18 (196.70 for the 206) is that
  object's +0x08, 0x5B4DD0's area (the clone's `performance_rating`), and +0x20 is its +0x10.
  0x5B5940 (the top speed: from the force table's first speed in 3 mph steps while the drag
  v²·[+0x290]/[+0x140] is below the force, capped by the limiter +0x2C0 mph) is the only loop that
  yields whole 3 mph steps, but the clone's port of it gives 65.71 m/s uncapped for the 206, not
  73.76; the store into +0x10 was not found (not 0x5B4DD0, 0x5B5940, 0x5B06B0, 0x5B5AC0, 0x5B4FA0).
  Needs a write watch on racer info +0x30 during the race set-up 0x5260C0. The clone users to switch
  once known: the chase camera shake threshold, the typed replay camera's Speed weight, the AI belt.
* **The human's path at 10 m/s and above (ported)**: 0x42CAA0 at +0x42C ≥ 0x783C34
  calls 0x4221F0(path, lateral) = 0x41D160(0, 0) then 0x41E860(path, lateral, lateral, 0): the
  human walk's points (now `HumanRoad::walk`) from the one at the path's end (0x4198C0 flags 1,
  else controller +0x115) each placed at its +0x54 (0x4122C0) with the lateral clamped into the
  point's room (vt+0x0C / vt+0x10), appended by 0x404CD0 up to 20 points. The lateral is the
  car's offset off the walk smoothed into +0x3DC (+0x3D8 the last raw offset, blend 0x783C3C;
  0x42CB85..0x42CBF5); with 0x81CBB0 set and the speed × 0x783C44 above 0x440F70's value, a second
  0x41E860 continues from 0x41D290's point with 0x4123C0's lateral and 0x783C40. Needs: those
  constants and 0x41D290 read, then a check against `freeroam_node`'s `hpath_n` / `hpath_p0` /
  `hpath_p1`.
  **Still open, one fact missing**: the lateral is the car's **raw** offset off
  the road entries' line (0x419BC0 → 0x419B20); +0x3DC is a smoothed lateral *rate* the path never
  reads (its users: the replay serializer 0x418580 and the resets), and the 0x41D290 branch is dead
  (0x81CBB0 is never written). Read in the disassembly: 0x4116A0 gives the lane k (0 outside a drag
  race); the room is vt+0x0C / vt+0x10 of the point's class (forward 0x784A48: 0x40B2A0 / 0x40B1F0,
  backward 0x784AA0: 0x40B840 / 0x40B780) = −half width (+0x14 forward / +0x16 backward, raw / 256)
  and +half width (the other side) less lane k's line (0x401C40) plus the point's value k (+0x04, the
  adjust, for k = 0), i.e. `line_path`'s room without the margin; c = min(hi, max(lo, cur)); the
  point 0x4122C0(+0x54, c, k) is `line_point(p, p.offset, c)`; 0x404CD0 gets (point, seg +0x0C &
  0x7FF, 0x5D89C0(k, 1, car), k, 1, car) — the earlier pushes stay on the stack as its arguments 4–6:
  the first point needs ≥ 1 m from the car (+0x60) and 0x4036E0's ahead test, or more than 20 m
  (8 m with the path's +1, which 0x41D160 has just cleared); later ones ≥ 1 m from the last and,
  when under 20 m, not turning back against the last segment, whose start is the car's place while
  the path has one point. The start entry of an empty path, the byte +0x115 of the controller (0x41E88D), is its road list's
  +0xAD (list at +0x68): the walk's current entry, written by the route manager 0x428030 from 0x4198C0
  (0x4280C2) and shifted when entries go (0x41973E): `RacerRoute::current`. **Ported**:
  `HumanRoad::road_path` / `human_append`, used for the human's `RoadCar::path` at +0x42C ≥ 10 in
  races and free roam (`human_road_path`). **Checked** against the free-roam recording
  (the maintainer's private capture, 128 frames at ≥ 10 m/s): the clone's walk driven by the
  recorded car, the path built from the car before the frame's motion — every count equal (10–15)
  and the first two points within 6.9e-4 m (median 3e-4; the recording's 6 significant digits);
  `tests::human_road_path::human_road_path_matches_the_recording`.
* **The traffic walk's relative turn length**: ported below ("The traffic walk's turns").
* **Event 4 (0x420810 against the action resets, open)**: not taken up.

## The traffic walk's turns, the "circling" car (2026-11-12)

**Verified** = traced in SPEED2.EXE, **observed** = read under Wine (a private Wine prefix, display :161;
scenarios `freeroam_walk` and `qr_walk_4014`, poller the maintainer's tool `trafwalk.py`, workflow gap 10).
Fix list `docs/native-fixes/TRAF-FIX.tsv`; evidence `docs/evidence/traf-fix/`.

* **The traffic walk (verified).** The route pass (0x428030, at each think: the walk changes every
  0.53 s with 16 cars, observed) drops the points behind the car (0x419670: their s off every
  point's +0x54 and off walk +0x354, −99999 below 0) and the trailing points with +0x58 ≠ 0
  (0x40A620), then extends from the last point (0x422220): per point 0x41EC20 goes through its
  junctions — a must-take link (goal slot +0x44) at once; otherwise a junction is asked of the
  controller's 16-entry cache 0x41CFA0 (a cacheable junction's decision drawn once by 0x413BB0 and
  kept), a turn (bit 26) only while the walk's last point lies more than 30 m past +0x354 (the
  walk's flag +0xAC, set only for the human's second walk 0x42677E, would lift that); a taken
  turn sets +0x354 to that last point's s. A turn's point gets s = last + jt (the 2D distance
  from the junction's control point to the target's), 0x41EE20 cuts the corner (points within
  it dropped, one at corner + 3 m, the target moved on along its record, +0x58 its offset; how
  its +0x54 is formed was not read to the end — observed below),
  and the length goes on from the new last point's +0x54 + jt (0x422668..0x4226DF). The loop
  ends when the length is over the reach (strict), the walk holds 10 points or more and the
  turn's length is not above it, or at 40 points (0x422300).
* **Observed**: under Wine every turn at intersection 3 (junctions 947 / 948 / 951, record 734
  → 492) entered a walk at its end and was gone at the next think (37 episodes of ≈ 0.5 s; e.g.
  J 734:14 cut, corner 734:13 at s 115.9 (+0x58 3.0), target 492:6 at s 123.3 (+0x58 0.4), +0x354
  122.5, reach 125: the length 123.3 + 9.1 = 132 stops the extension, the next pass drops both
  points and the 30 m gate passes the turn over). Links and non-turn branches stayed for seconds.
* **Ported**: the clone added the distance walked since the walk's start to the length
  after a turn, so once the car had moved every extension stopped at a turn's target; now the
  length is relative to the walk's first point, as the rebased +0x54. The target's s is still
  taken as the junction's + jt (the corner cut makes it 4–5 m shorter in the cases read,
  **inferred** stand-in). Turns per choice point (a control point with a turn or a branch;
  `traffic_turn_stats`, passes / turned):

  | | original | clone before | clone after |
  |---|---|---|---|
  | free roam stand (462 / −46), 734:14 · 734:16 · 734:22 · 492:4 · 492:6 | 0/44 · 0/33 · 0/13 · 0/7 · 0/5 | 0/114 · 0/114 · 0/74 · 0/23 · 0/46 | the same |
  | free roam, all 12 choice points (1 × 300 s / 6 × 300 s) | 0/115 (1 branch of 2 at 423:6) | 0/484 (14 of 19 at 423:6) | 0/483 |
  | Quick Race 4014, traffic maximum, 477:6 · 477:8 · 435:11 | 0/9 · 0/10 · 0/3 | 0/23 · 3/24 · 0/2 | the same |

  The rule changes nothing measurable in these runs (the car moves 5–8 m a think, so a turn is
  rarely 2·jt short of the reach); the clone's three turns at 477:8 come from placement builds.
  The earlier objection (51 turns in 104 passes with this rule) came from the unit test moving the
  car a control point (10–15 m) a think; it now moves 7.4 m (`the_walk_drops_a_turn_taken_at_its_end`).
* **The "circling" car is not the walk (traced, partly observed; open).** `traffic_does_not_circle`'s
  PARCELVAN (track 4014, ~80 s) drives straight down record 11; it is on the BeltMover (mode 4) in
  "Avoid Cars" behind a car on the next lane, 0x424CB0 asks for 0 (its distance within 15 m) with
  action +0x1F4 = 0, which reaches the belt as the driver's +0x234 (0x41CC2F → 0x4054A0; vt+0x78 =
  0x410000) — 0.5 s in 0x5916A0. The belt's rate crosses 0 half a second after the build and the
  heading (the velocity 0.4 s ahead, 0x59EC90 → 0x591B80) 0.1 s after it; the clone's yaw spike
  (123 rad/s in one update) is that turn-round, repeated by each think's rebuild. The port is the
  original's line for line, and the original does it too: two belt cars of `qr_walk_4014` whose
  belt ran out turned round by 180° as +0x42C crossed 0 and ran backwards (−9 m/s,
  `native-belt-flip.txt`). What differs is how often a belt car gets there: in "Avoid Cars" on mode
  4 the original spent 123 of 7 914 belt samples (1.6 %) and never went under 3 m/s (free roam: 1 211,
  minimum 4.75 m/s, the 6 m/s branch); the clone's race belt cars 22 % of their time, 56 % of it under
  3 m/s. The CircleWatch rule finds no circling car in either original log. **Open**: why the clone's
  belt cars meet a car within 0x402ED0's window so often — needs the action's tracked object +0x218,
  +0x1F0 and the lateral 0x402ED0 measures, read for mode-4 cars under Wine. 0x4022D0 / 0x408C40 (the
  switch to mode 4) leave the action alone (verified). The test stays `#[ignore]`d with that reason.
* **Long soak after the port** (`traffic_circling_soak` defaults: 12 spots × 240 s, Quick Races
  × 240 s — four ran, tracks 4014, 4123, 4201, 4001 — 215 923 frames; `soak.txt`): 2 circling cars, neither on its walk (lanes turned 6° / 22°):
  a belt car in "Avoid Cars" (the case above) and a MeloMvr car orbiting a path point 2.75 m away at
  83° off its heading (**open**, not traced); the races none, 2 spins after hits. On the final tree
  (with the placer's node, below): 218 213 frames, 1 circling car — a belt car near its walk's end in
  race 4014 (walk of 2 points, 3.5 m/s, not avoiding: the same turn-round), the spots none.
* **The placer's road node (a follow-up to the encounter pursuit work)**: 0x42E9F0 searches from the player's road
  node (controller +0x0C); `TrafficLane::spawn_from` now takes the human controller's
  (`HumanRoad::node`) and falls back to `road_node` only without one (free roam's human road
  is updated with the encounters; before the first update). The indices mean the same in both:
  the traffic's file is the same file (free roam RoutesFreeRoam.bin — the encounters' copy is pruned
  by `finalize_free_roam_paths`, which truncates the junction table only; races
  Routes<track><F|B>.bin on both sides). Turns with it: free roam 0/483, the race 4/146 (477:8 1 of
  27, 435:9 1 of 3, two at points passed under three times); belt cars in "Avoid Cars" in the races 14 054 car-frames (16 830 before), still far above the
  original's. The human road is updated after the traffic's think, so the
  placer reads the previous update's node (**inferred** order). The race's replay snapshot now carries
  `human_road` (`sim/replay/snapshot.rs`): without it a replay started with another node and placed
  other traffic (`replay_reproduces_the_race` failed in the second gate). Test
  `the_placer_starts_from_the_players_road_node`. This moves where traffic is placed; its gate lines
  are told apart from the walk rule's by the two gates (`gate-traf`, then `gate-traf2`).
* **Draws (world generator, free roam standing at the stand, 60 s)**: the traffic stream 0.28 a
  frame (`pick_lane` 0.19, the placer's search and lane draws 0.09); the original 6.30 a frame of
  which the drivetrain rerolls 6.00 (not wired in the clone then; now ported, docs/rng.md), the rest 0.30 (RNG-T: lanes
  0.20–0.38, placer 0.08). The earlier "12 lane picks a frame" is gone from today's tree
  (`pick_lane` is called only from the placers). The encounter placer's search walk (racer_search,
  `sim/encounters`, 0x415130) drew a further ≈ 0.7 a frame — **open**, not this unit's files.


## Belt cars in "Avoid Cars", measured on both sides; car +0x550 (2026-11-12)

**Verified** = traced in SPEED2.EXE, **observed** = read under Wine (a private Wine prefix, display :166,
`qr_walk_4014` × 3 and `freeroam_walk`, poller the maintainer's tool `trafwalk.py`, which now also reads car
+0x550 / +0x998 / +0x8FC, the action's +0x1F0 / +0x218 (the tracked node's car) / +0x21C / +0x220,
the road entries the car's last think left (controller +0x5C, changed ones only) and controller
+0x710 / +0x77C). Fix list `docs/native-fixes/TRAF-2.tsv`; evidence `docs/evidence/traf-2/`.

* **One definition on both sides** (`docs/evidence/traf-2/stats.py`): per belt (mode 4) sample,
  the share in "Avoid Cars" (action type 2), the share of those under 3 m/s and the share asking
  0 (+0x1F0). The clone's side is `traffic_turn_stats` with `TURN_DUMP` (trafwalk's columns),
  `TURN_THROTTLE=1` (throttle only, as the scenario drives) and the samples from the green (the
  scenario arms at the green). The traffic walk's "22 % / 56 % under 3 m/s" compared the clone's race on
  the **autopilot**, counted from the countdown, with the original's **throttle-only** player (who
  is stuck against a wall by 90 s): another traffic distribution. Measured alike:

  | belt cars, Quick Race 4014, traffic max | Avoid Cars | of it under 3 m/s | of it asking 0 |
  |---|---|---|---|
  | original, 4 runs (9.9 / 17.2 / 4.1 / 1.6 %) | 9.2 % | 4.0 % | 1.5 % |
  | clone before, 6 streams (18–52 %) | 39.4 % | 10.7 % | 9.8 % |
  | clone after (+0x550) | 37.8 % | 10.3 % | 9.2 % |
  | **free roam stand** (original 2 runs / clone 300 s, before = after) | 13.6 % / 17.9 % | 0 / 0 | 0 / 0 |

  The run-to-run spread is large on both sides (platoons of belt cars following each other). The
  rules themselves were re-read and match the port: the proposal 0x4145B0 (0x402ED0 with look 99999,
  time ≤ 5 s, 0x4140E0; its fast-target skip needs controller vt+0x68, false for traffic), the update
  0x424CB0 (the slow rule whenever 0x402ED0 finds an object with the look: the tracked gap
  [esp+0x1C] is 0 unless the human branch ran — asm 0x424D0D / 0x424F74), the end 0x41CD10 (asm
  0x41CD4E–0x41CE23), 0x402ED0 and 0x402CC0 (its time is 99999 / |closing part| also outside the
  box, as the clone's), the entries 0x6148F0 (race-coordinate test: |Δ+0x8FC| stayed under 3 ×
  the reach for every pair read), 0x417750's dead end (the original's cars stop at the same three
  dead ends of 4014 as the clone's). **Where the extra avoidance comes from (observed)**: the clone's
  belt cars have a car within 60 m in 54 % of their samples (original 10–39 %) and then avoid in
  60 % (original 0–30 %); their tracked car is mostly another belt car 15–120 m ahead that is
  itself avoiding at 6 m/s (chains), and the slow rule then asks 6 m/s for any closing car in the
  window — so the extra share follows from where and how densely the clone's belt cars run, not
  from a different decision rule. Its cause (placement / the throttle-only player's different stop
  place in the clone) is **open**.
* **Car +0x550, "active" (verified, observed; ported).** 0x6148F0 keeps a node's car only when
  0x5EC070 (car +0x998) is false and +0x550 is set, or when it is the car itself. A car the placer
  cannot place is paused (0x42FAAB → **0x4020D0**: body 0x593100, mode 0, **0x5ED5D0(0)**, +0x77D);
  placed again (0x5FBEE0: 0x5F52D0 moves it, the controller's slot +0x34 0x42A050 → **0x402110**:
  body 0x593110(0), then mode and 0x5ED5D0(1) only when the body passes **0x593990**(0) — [body
  +0x60] +8 = car +0x5CC, the world under its **old** place, as this update's 0x5F6CE0 loop left
  it — +0x77D cleared), then 0x5FBEE0's own **0x5F6CE0**: +0x5C0 0 → 1 at a loaded new place calls
  0x408C40 → **0x4022D0(1)**, which activates a car that is not paused. So a parked car placed over
  unloaded world from an unloaded parking place runs on the BeltMover **inactive** — invisible to
  every road entry — until the world under it loads. Observed: every placement from parked onto
  mode 4 read +0x550 = 0 (4 of 19 placements from parked in one run; the 15 onto mode 2 read 1);
  inactive belt time 25.1 / 6.9 / 13.9 % in the three races, 0 in free roam (all 16 cars active from
  the start). Clone: `TrafficCar::active` / `held`, `placed_active`, the load transition in
  `traffic_stream`, `traffic_others` lists active cars only (a car's own entry is built from its
  state when it is not listed); the race fill's cars start active (0x408CA0 / 0x409A30 at race
  states 3–4). Its share in the clone's races: 3.3 % of belt time; it moves the race numbers above
  by 1.6 points. Test `a_parked_car_placed_over_unloaded_world_stays_inactive`. World_DrawCars
  (0x6317A2) also skips inactive cars, and so does the clone (below, Inactive cars not drawn).
* **Soak** (`traffic_circling_soak` defaults: 12 spots × 240 s, 4 Quick Races × 240 s; 218 213
  frames): 1 circling car, the same as after the traffic walk fix (race 4014, 154.3 s, car 10, walk
  of 2 points, not avoiding, 3.5 m/s: the belt turn-round near its walk's end); not reproduced by
  the race alone (`SOAK_SPOTS=0`), so it depends on the free-roam spots' world stream; **open**.
  `traffic_does_not_circle` passes on the default stream and on streams 1–5.
* **Turns** (`traffic_turn_stats`): free roam 0/6 before and after; the race (autopilot, from the
  green) 1/42 (after the traffic walk fix: 4/146).
* **Draws**: none added, moved or removed.
* **Still open**: the MeloMvr car orbiting a path point 2.75 m away (not reproduced in this soak);
  the corner cut's target s; the order of the placer and the human controller's pass.


## Traffic placement and density, measured on both sides; the race placer's rules (2026-11-12)

**Verified** = traced in SPEED2.EXE, **observed** = read under Wine (a private Wine prefix, display :168,
`qr_walk_4014` ×2 more runs, poller the maintainer's tool `trafwalk.py`, which now also reads world +0x14,
the world's slots +0x80/+0xE0 and each racer's place, +0x8FC, +0x430 and controller kind).
Fix list `docs/native-fixes/TRAF-3.tsv`; evidence `docs/evidence/traf-3/` (`dens3.py`: one method
for both sides — the clone's `traffic_turn_stats` dump now carries the player's place, facing,
speed, race coordinates, the walk's front and parked cars as mode 0 rows; `race-placement.txt`,
`freeroam.txt`, `native-paused-coast.txt`).

* **Placement, measured alike** (race 4014, traffic maximum, throttle only from the green; a
  placement is a car moving > 40 m between samples): the original places 20–28 cars a minute
  (5 runs), parks 8–15 a minute, puts them 250–300 m off (2D) with 60–70 % within 60 m of another
  car at once, ahead of the player 2/3 of the time; the clone's distances and gaps at placement
  were already the same; it placed 29.2 / min and parked 9.0 / min (12 streams).
* **Ported (verified, the asm)**: in a coordinate race 0x42E9F0 (a) refuses a car that is not
  parked (+0x77C) a place ahead of the player farther than its own race coordinate lies, or behind
  farther back (0x42ED4C..0x42EDA9; the pass then parks it, 0x42FAAB); (b) measures a try's point
  by the signed race-coordinate difference when 0x5DCC90 says so (a point behind never fails the
  500 m reach), else in 2D (0x42EBBB..0x42EC19); (c) gates with 0x418110 (`drag_gate`). 0x42F910
  draws the spacing once a pass before the slot loop, 0x42F800 draws which human and the teleporter
  odds for each car tried (docs/rng.md). 0x417BB0 / 0x4112A0 measure race coordinate alone in every
  race; 0x5F12B0's world slots hold the player alone (world +0x14 = 1, **observed**).

  | race 4014, belt (mode 4) samples | within 60 m | Avoid Cars | inactive | placed / min | parked / min |
  |---|---|---|---|---|---|
  | original, 5 runs | 11.8 / 26.7 / 38.8 / 18.9 / 54.1 % | 2.3–18.5 % (pooled 10.8) | 7.9–38.7 % | 20.0–28.2 | 8.4–15.3 |
  | clone before (12 streams) | 55.4 % | 43.1 % | 4.0 % | 29.2 | 9.0 |
  | clone after | 53.1 % | 38.3 % | 5.2 % | 24.5 | 9.3 |
  | free roam stand, belt Avoid (8 × 300 s; original 13.6 / 22.3 %) | 22.1 → 33.6 % | 29.1 → 35.0 % (streams 17–42 → 26–43 %) | | | |

  The density share was never outside the original's range (54.1 % in run 5); what differs is how
  often belt cars avoid **given** a car near (clone 60–80 %, original 0–44 %). Free roam changes
  only by the draw order (no free-roam rule moved); its shift is within the streams' spread.
* **Why the clone's belt cars avoid more (traced, observed; open)**: (1) the clone's belt time is
  young — 64 % of it within 10 s of a placement (original about a third): its placed cars switch
  to the MeloMvr on a new Avoid (2→3: 46 a run vs 13–43) and leave the loaded world on it (3→4: 23
  a run vs 4), carrying the avoidance onto the belt. (2) **The paused car coasts** (verified,
  0x4020D0, 0x5F6CE0 → 0x408C40 → 0x5F46B0(0x5F4410), 0x4022D0(0); observed,
  `native-paused-coast.txt`): a car the placer parks (or the walk's end pauses) keeps its walk and
  driver; when the world under it loads it gets its TrafficMover back and coasts on its last path
  at its last speed (20.39 m/s, think off: the walk does not change), still paused and inactive,
  then the BeltMover when the world unloads — so 8–39 % of the original's belt time is such
  invisible, non-avoiding cars, against 4–5 % in the clone, which freezes a parked car without a
  lane. Porting it needs the paused car's collision (0x593100 on the body), whether the driver
  runs on the stale plan each frame, and the render rule (below). (3) When the player is at 55 mph
  or more the original placed 81 % ahead, the clone 37 %: the walk byte's flip on the node record's
  +0x70 bit 1 matches the binary (verified 0x42EB04; the record flags in memory = the file's), so
  the difference lies in the node or the road beyond it — needs the human node (controller +0x0C)
  read per placement under Wine.
* **Render (for the render unit)**: World_DrawCars (0x6317A2) skips a car whose +0x550 is clear;
  the clone draws every traffic car with a lane: in `render/` skip `TrafficCar` with `active ==
  false` (and, once the paused car coasts, those too).
* **Soak** (`traffic_circling_soak` defaults, 217 433 frames): 1 circling car (before: 1) — race
  4123, 155.1 s, car 7 with no mover (physics), avoiding, steer 45°, gas 1; not the belt case;
  **open**. Free roam's spots: none.
* **Tests**: `a_car_ahead_is_not_placed_farther_ahead`, `the_placers_reach_is_signed_on_race_coordinate`,
  `the_placer_draws_the_racer_and_the_odds_first` (ai::traffic::tests); the probe
  `traffic_turn_stats` (dump columns).


## The paused car coasts; inactive cars not drawn; honk draws, outrun end, driver event 4 (2026-11-12)

**Verified** = traced in SPEED2.EXE, **observed** = read under Wine (the placement work's runs), **measured** =
the clone's probe. The answers with addresses are kept with the maintainer's private evidence; fix list
`docs/native-fixes/TRAF-4.tsv`; evidence `docs/evidence/traf-4/` (`episodes.py`, `switch.py`,
`measures.txt`).

* **The paused car (verified; ported).** 0x4020D0 (placer 0x42FAAB, walk end 0x429F0D) leaves the
  controller's walk and the driver's path alone: the clone keeps them (`TrafficCar::coast`, the
  path in `plan`; `lane` is None, so the car is no road entry, no collision body and not drawn).
  (a) 0x593100 raises the body's pause count +0x17, so 0x593990 refuses and 0x5FFA60 / 0x592E70
  drop every pair with it: nothing hits a coasting parked car (world contacts **inferred** none).
  (b) 0x429D90 returns on +0x77D (no think, no stuck test), but 0x609CF0 runs every car's driver
  ([car+0x30] vt+4 = 0x421030) each frame: the coast steers along the old path. (c) It stays
  inactive (0x4022D0(1) needs +0x77D clear), and World_DrawCars skips it (0x6317A2). (d) The world
  under it loading → 0x408C40 → 0x5F46B0(0x5F4410) = the TrafficMover at its speed; unloading →
  0x4022D0(0): the TrafficMover's slot +0x10 (0x570A70) is true → the belt from that path; mode 0's
  (0x50D510) false → a car paused in mode 0 stays put; a placement (the placer retries a parked car
  every round, 0x417BB0) ends it. Test `a_parked_car_coasts_inactive_on_its_old_walk`.
* **Measured alike** (race 4014, traffic 3, throttle only from the green, 12 streams of 150 s;
  `dens3.py`, `switch.py`, `episodes.py`): the coast is there and looks like the original's
  (episodes 0>2, 0>2>4, 0>2>4>2… ending at a placement: clone 41 in 34 min, original 18 in 15 min),
  but **the avoidance gap did not close**:

  | race 4014, belt (mode 4) | Avoid Cars | inactive | placed / min | parked / min | 2→3 / run | 3→4 / run | belt ≤ 10 s after placement |
  |---|---|---|---|---|---|---|---|
  | original, 5–6 runs | 2.3–18.5 % (pooled 10.8) | 7.9–38.7 % | 20.0–28.2 | 8.4–15.3 | 11–43 | 3–6 | 0–50 % |
  | clone before | 38.3 % | 5.2 % | 24.5 | 9.3 | 46 | 23 | 64 % |
  | clone, coast only | 37.8 % (streams 1–55) | 9.7 % | 26.9 | 9.8 | 49 | 20 | 64 % |
  | clone after (all of this section) | 42.5 % (streams 22–71) | 11.2 % | 26.1 | 8.6 | 53 | 23 | 54 % |

  Inactive belt time is now inside the original's range; Avoid Cars is not (the 37.8 → 42.5 move
  is the streams' spread: the honk draws and event 4 reroll every later world draw). The excess
  stays where the placement work left it: the clone's placed cars switch to the MeloMvr on a new Avoid and
  leave the loaded world on it (3→4: 23 a run against 3–6). **Open**: the placer's side at speed
  (original 81 % ahead at ≥ 55 mph, clone 37 %) — the human's road node per placement was not read
  under Wine in this unit.
* **Inactive cars not drawn** (0x6317A2): a patch in the private set (`traf4-render.patch`) against the
  base (render is a Phase 4 unit's): `traffic_drawn` (placed and active) in the car pass, the
  road-car list and the rear view; test `world_draw_cars_skips_an_inactive_car`. The car glows
  (0x61AEA0 takes a car only when 0x60D580 does: +0x550 set) and the headlight pools (0x6154A0: the
  same byte) skip such a car too, so the clone's head and brake glows, headlight pools and the
  enhanced passes' car lists (car lights, occluders, reflections, motion inputs) take `traffic_drawn`
  as well: no lamps floating where no car is drawn.
* **Honk / flash draws** (0x40D5C0, docs/rng.md): ported at all 9 sites: 0x426E4F is the same
  chooser code as 0x426CFF (traffic goals 8 and 7), so the traffic proposal's one call covers both (docs/rng.md).
* **Outrun end**: `Encounters::ended` keeps the last outrun from its end to manager state 0;
  the cars still out keep goal 5 / 6 (the clone put them on goal 9). Corrected later: the end's
  0x42EFB0 first calls 0x42CD60, which gives every car out goal 1, "Win road race" (formats/ai.md,
  Outrun, "The end"; test `outrun_cars_take_goal_1_after_the_end`).
* **Driver event 4** (0x420810): posted when the think planned the racer's path afresh
  (`RacerRoute::take_replanned`: 0x420D40's 0x41D160(0, 0), GetUnstuck's first path); the driver's
  trackers skip their rate for that update (race rivals and encounter racers; never traffic).
  Whether action constructors stamp a path is **open**. Test
  `a_path_planned_afresh_posts_driver_event_4`.
* **Soak** (`traffic_circling_soak` defaults: 12 spots × 240 s, 4 races ran; 219 711 frames): 1
  circling car — free roam spot 5, car 7 on the MeloMvr orbiting its walk point 2 m behind it
  (the traffic walk's open MeloMvr case), not a paused car; the placement work's race-4123 car-7 case did not recur
  (the streams moved). `traffic_does_not_circle` passes.


## The placer at speed (the wrong-way byte); the mover switches traced end to end (2026-11-13)

**Verified** = traced in SPEED2.EXE, **observed** = read under Wine (a private Wine prefix, display :183,
`qr_walk_4014` × 2 more runs n1 / n2; the maintainer's tool `trafwalk.py` now also reads the human's road node
with the placer's walk-byte inputs), **measured** = the clone's probe (`traffic_turn_stats`, 12 streams
× 150 s). Fix list `docs/native-fixes/TRAF-5.tsv`; evidence `docs/evidence/traf-5/` (`measures.txt`,
`native-wrongway.txt`, `avep.py` / `exit3.py` / `ahead.py` / `plnode.py` / `sw2.py`).

* **2 → 3, every trigger (verified).** (a) The think 0x429D90 replacing its action (0x429FF8) by one
  of type 8, 1 or 2 while car +0x5CC is set and the car is in mode 2 (0x42A004–0x42A028). The traffic
  goal 0x426D70 proposes only Obey (0x11, 600, 0x4149E0), Avoid (2, 300, 0x4145B0) or its parent's
  Drive Lane (7), and only when the running action is not of the same type and not of a higher
  priority — so for traffic the trigger is a new "Avoid Cars". (b) A contact: 0x5FFA60's resolved pairs
  → 0x5B2620 → each body's vt+0x48 (0x593950) → 0x5F4970 → controller vt+0x38 = 0x408ED0 → mode 3
  while +0x5CC (the racers' 0x404890 / 0x402450 differ). No distance, visibility or racer rule; the
  only other mode-3 caller (0x5F8B00, from 0x5FBBF0) needs controller +0x900 (the human's recorded
  drive). **Back to 2**: only 0x5F6CE0 on a load change (0x408C40: 0x5F4410 gives traffic (+0x24 = 4)
  mode 2), a placement (0x5FBEE0 → 0x5F52D0) or a resume (0x42A050 → 0x402110). The clone has all of
  these (`traffic_lane_think`, `traffic_settle`, `traffic_stream`). Observed (qr1–qr5, `sw2.py`): of 138 native 2 → 3 switches 127 came on the sample the action became Avoid, 10
  with Avoid already running a sample before (a new Avoid replacing one) and 1 under Obey (a
  contact); a new Avoid on mode 2 stayed on mode 2 3 times.
* **3 → 4 and what crosses (verified, observed).** 0x4022D0(0) → 0x5F46B0(4); neither it, 0x408C40
  nor 0x5F46B0 touches the controller's action, its speed target or the avoidance's time:
  the avoidance is carried onto the belt on both sides (native 2–10 a run, `avep.py`).
* **"Avoid Cars" on the belt (verified, measured).** Mode-independent: the same think, proposal
  0x4145B0 (0x402ED0 with look 99999 on the controller's entries, its time under 5 s, 0x4140E0), update
  0x424CB0 and end 0x41CD10; the entries (0x6148F0) hold every active car (+0x550, not +0x998) of any
  kind and mode — player, racers, traffic on 2 / 3 / 4 — and never a paused / coasting (inactive) one.
  Per minute of belt time the clone started as many belt avoidances as the original before this unit
  (1.6 against 0.6–2.1) and they lasted as long (median 5.6 s against 2.4–9.1 s): the excess was the
  avoidance carried in from the MeloMvr (231 3 → 4 carries in 12 streams against 2–10 a native run).
* **The cause: the placer's walk byte at speed (verified, observed; ported).** 0x42E9F0 sets the byte
  at 55 mph (+0x430 ≥ 24.5865) only while the human's player object (car +0x18) has its byte +0x248
  clear (0x42EAB3..0x42EAFF); +0x248 is 0x600720's wrong-way answer (0x602688). Then a coordinate race
  flips it on the node record's +0x70 bit 1 (0x42EB04). The throttle-only scenario's player stops at
  record 550 and turns round 53 s after the green; from then on the game counts him as going the wrong
  way (+0x248 = 1 in 79 % of the rows at speed; 0x88F2BE = 1 throughout) and every placement
  at speed went forward along the records — ahead of him (30 of 35). The human node is the same on
  both sides (controller +0x0C → record / control point, the same sequence 476:2 → 655 → 475:23 … at
  the same places): the input that differed was +0x248, which the clone's placer never read. Ported:
  `RacePlace::wrong_way` (the race's `wrong_way`, the clone's 0x600720) — the race hands it to
  `traffic_think`; outside races 0x600720 has no road node and answers 0. Which of the two runs first
  in an update (the player's 0x602660 or the placer 0x42F910) is **inferred** (the clone reads the
  previous frame's answer). Test `the_placer_turns_round_for_a_wrong_way_player`.

  | race 4014, throttle only | belt Avoid (streams) | 2→3 / run | 3→4 / run | ahead at ≥ 55 mph | belt ≤ 10 s after placement |
  |---|---|---|---|---|---|
  | original (qr1–qr5, n1, n2) | 2.3–18.5 % (n1 18.2, n2 10.1) | 11–44 | 3–16 | 76–90 % | 13–50 % |
  | clone before | 39.6 % (21.9–70.5 after the paused-car work) | 54 | 26 | 24 % | 58 % |
  | clone after | 13.2 % (3.5–22.7) | 46 (25–61) | 8.8 (4–15) | 87 % | 43 % |

  Belt Avoid, 3 → 4 and the side at speed are now inside the original's spread; 2 → 3 lies at its top
  (its rate per minute of mode-2 time, 7.9, is inside the original's 1–9.8; the clone keeps more cars
  on mode 2). The earlier ranges (2 → 3 11–43, 3 → 4 3–6) grew with n1 (44 / 16).
* **Soak** (`traffic_circling_soak` defaults, 219 798 frames): 2 circling cars — free roam spot 5, car
  11 on its TrafficMover following a lane that itself turned 306° in the 10 s (the spots' code path is
  unchanged here; spot 5 car 7 was reported on an older tree), and race 4123 car 1 on the MeloMvr, not avoiding, gas 0, steer 28.7° with its target
  point 8 m ahead at 10° (the races' traffic moved with the placement; the same class as the placement work's
  race-4123 case). Both **open**: no mover rule changed in this unit.
