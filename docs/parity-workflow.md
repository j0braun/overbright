# Proving a simulation fix against the original (parity workflow, 2026-11-11)

Every change that alters the clone's simulation on purpose (physics, contacts, AI, race flow,
cameras) comes with a comparison against the running original: the same scenario recorded in
SPEED2.EXE under Wine and run in the clone, before and after the change. This page is the
step-by-step, the coverage of the planned fix families, and what is still missing. The harness
itself (the cave, the scenarios, the fields, the diff) is described in docs/parity-harness.md.

**Verified** = traced in SPEED2.EXE, **observed** = read under Wine, **inferred** / **open** as
marked.

## 1. The cycle a fix follows

1. **Pick the scenario and the fields** from the coverage table (§4). If a field the fix needs
   is missing, add it to the maintainer's `fields.toml` with `clone = "context"` first and check
   that it reads sensible values in one native run (§6 lists the ones added for the planned
   fixes, all checked). Give it a clone accessor in `src/bin/game/dev/parity.rs` (one match
   arm in `car_value` / `global_value`) and a tolerance once the fix lands; the test
   `every_field_accessor_is_known` fails until the accessor exists.
2. **Record the original once** (it does not change with the fix):
   the private tool `parity.py run SCENARIO.toml OUT` writes `native.tsv`, `native.json`,
   `native.rec` + `native.layout.json` (exact bits), then runs the clone and the diff. Keep OUT.
   Use a pinned scenario (§3) when the comparison needs the same opponents, grid and draws.
3. **Before**: with the unchanged clone, `parity.py clone SCENARIO.toml OUT` →
   `report.txt`, `summary.txt`, `diff.tsv`. Save them as `before-*`.
4. **Implement** the fix.
5. **After**: rebuild (`cargo build --release --bin game --features dev-harness`; the
   `--flow parity` entry exists only with the dev-harness feature), confirm the binary changed
   (md5sum against the before build), then `parity.py clone SCENARIO.toml OUT` again.
6. **Judge** with the rules of §2 and put the before/after summary lines and the fix's
   field rows (a few frames around the change) into `docs/evidence/<topic>/`.

Wall time on the reference desktop (one game instance, wined3d on the GPU): a full `run` of a 20 s
career-event scenario takes **3–4 min** (boot to free roam 45–100 s, race load and countdown
~20 s, the 20 s recording ~50 s of wall time under the fixed frame time, the clone 7 s, the
diff ~50 s); a free-roam scenario of 8–12 s **45–70 s**; `clone` + diff alone **~1 min**.
So a before/after pair against one native recording costs ~2 min once the recording exists.

## 2. What counts as "matches the original"

Rows are aligned by frame from the arm; both sides record the state before that frame's inputs
and update, and the clone steps the original's own tick count for each frame (§5), so world
time agrees exactly. The diff names the first frame where any field leaves its tolerance and
the first frame out of tolerance per field.

A fix **matches** when, on its scenario:

* the field(s) it targets agree within their tolerance **over the window the fix acts in**
  (the frames where the changed rule fires: the scene and countdown for start-scene
  collisions, the frames after a contact for the impulse, the frame of a key press for the
  nitrous key …), or their first divergence moves past that window;
* **no other field's first divergence moves earlier** than in the before run (a fix must not
  trade one difference for an earlier one);
* where the scenario cannot reproduce itself in the original (unpinned races, free-roam
  traffic), metric-level agreement (§4 "metric") lies inside the original's own spread over
  at least two native runs.

Tolerances by field kind (fields.toml `tol`; the values there are the defaults):

| kind | examples | per-frame tolerance | note |
|---|---|---|---|
| discrete state | `gear`, `race_state`, `flow_state`, `car_mode`, `ai_action`, `ai_goal`, `lap`, `place`, `road_node_id`, `hpath_n` | 0 (exact) | a transition may land ±1 frame when the trigger is a continuous value at a threshold; say so in the evidence |
| counts per frame | `n_pair_test`, `n_contact`, `n_world_draw`, `n_human_path` … (row differences) | 0 for "never / once a frame" rules; the per-state sums of framestats.py otherwise | |
| position | `pos` (centre of mass) | 0.25 m | 0.01 m at frame 0 of a placed start |
| velocity / speed | `vel`, `speed`, `xspeed` | 0.5 m/s | |
| direction | `fwd`, `rb_row*`, `cam_fwd` | 0.03 | |
| engine | `rpm` | 300 | |
| inputs | `gas`, `brake`, `handbrake` | 0.05; `steer` 1° | |
| AI scalars | `ai_target` 1.0 m/s, `ai_catch` 0.02 | | |
| race clock | `race_clock` | 40 (one 1/4000 s quantum step) | |
| single-step physics | rigid-body rows, ω, L before/after one sub-step | float ulps (exact bits from `native.rec` / subtrap `body_hex`) | per-frame driving rows cannot show an integrator change (§4) |

Context fields (`tol = -1`, clone `context`) are recorded and printed but never count.

## 3. Determinism of the original (observed 2026-11-11)

Two recordings of the same scenario, compared bit for bit (the private tool `detstats.py A B`
over `native.rec`); outputs in `docs/evidence/parity-workflow/`):

* **Unpinned career race** (`circuit_s1`, twice): different from frame 0. The opponents'
  models differed in all three slots (PEUGOT/240SX/PEUGOT106 vs COROLLA/PEUGOT106/PEUGOT), the
  player's grid slot differed (20 m), and the world generator state differed. Cause: the shared generator 0x7EA624 (one dword, 0x43C170) is seeded from the performance
  counter at boot (0x57ECC0, **verified**) and the player's slot is drawn from it (0x4FEDE0,
  **verified**); that the opponents' draw comes from it too is **observed** (pinning it fixes
  them, below). 168 of 574 columns (constants) agreed.
* **Unpinned free roam** (`crash_wall`, twice): the player's state agreed bit for bit at frame
  0 except the drivetrain's idle clutch cycle (`shift` 0 vs 2, `clutch`, the random shift
  offsets `dt_78/7c/80`); the scripted throttle at t = 0 reached the player on frame 8 in one
  run and frame 9 in the other (`key_gas`; keys scripted later show on the next row, as they
  should: freeroam_node t = 2 s, nitro_gas_edge_finished t = 1 s), so the drive parted from frame 8. The world
  generator and the encounter cars' models differed (boot seed). 418 of 523 columns agreed.
* **Pinned race** (`circuit_s1_pinned`, `[start] pin_rng`: 0x7EA624 written just before the
  race is accepted): same opponents, same grid (frame-0 positions bit-equal), every car's state
  bit-equal for frames 0–15, the world generator equal to frame 32. Then the player parted at
  frame 16: the replay manager's DUMMY_SNAPSHOT (every 100 world updates, 0x57D010) fell on
  frame 16 in one run and 15 in the other, because the world update counter 0x7FB724 differed
  by one at the green (384 vs 385). The race clock's 1/4000 s phase differed too (the frame-time
  carry 0x865144). `[start] pin_phase = true` now also writes both at the hold
  (0x7FB724 = 1050, carry 0). Two runs pinned both ways agreed bit for bit in **558 of 575
  columns over all 1 201 frames**: every car's kinematics, rigid body, drivetrain, tyres, AI
  and race fields. The rest: the absolute sim clock 0x865188 and the placement stamps taken
  from it (an offset of 3 ticks, constant), the front-end frame counter, the shared generator
  itself (its later draws do not reach the race), the AI's object pointer `ai_obj` (a heap
  address), and the camera eye / direction from frame 26 at the 1e-4 level.

**Update (2026-11-12)**: four pinned recordings of `circuit_s1_pinned` (00:25 of
2026-11-12, one of 2026-11-11, two in a private Wine copy) agree on every car's place only to frame 264–280;
past the rivals' first-corner contacts they part, the player 8–131 m apart at 20 s
(internal evidence). Compare this scenario per frame only to
~frame 260; beyond it, judge against the original's own spread. The clone pins the original's
world generator at the arm (plan line `world_rng`, from the recording's frame 0) so its AI draws
compare draw by draw (accessor `world_rng`).

Consequences: compare a race against a pinned recording; record the same pinned scenario twice
before trusting a per-frame verdict past the first snapshot; in free roam start scripted inputs
after the first second (the onset jitter above) or re-time the clone's key to the frame the
original's key byte changed (§7).

## 4. Coverage of the planned fix families

"Scenario" = an existing or new file in the maintainer's tools; "fields" = fields.toml names
(new ones in §6); "clone accessor" = what the fix must add to `parity.rs` for a per-frame
comparison.

| family | native rule (verified unless marked) | scenario | fields / metric | status |
|---|---|---|---|---|
| Orientation integrator: Euler row step + re-orthonormalise each sub-step (0x5AA490 → 0x59C130, 0x5BB0E0; ω from the un-orthonormalised rows) | | subtrap.py capture (stops before/after each 0x5AA490 of one car; `body_hex` = the body's raw bytes) on any driving scenario; per-frame `rb_row0..2`, `rb_spin`, `rb_angmom` | one-sub-step replay test in the clone: native pre-state → clone step → native post-state to float ulps | per-frame rows cannot show it (a step differs by ~(ωh)³; the drive parts for other reasons first). Replay test: **done**, bit for bit (the maintainer's private captures) |
| Track contact impulse 1.0 in crash mode (0x593A50 / 0x5A0B10) | | `crash_wall`, `crash_wall_tp` (car–wall, never crash mode); a car–car hit then a wall: **missing** | `car_mode` (1 = crash), `vel`, `yaw_rate`, `rb_spin`, `recs` | needs a scenario that sets crash mode (§7) |
| No car–car pairs in the start scene / countdown (0x593990 refuses in the scene, bodies +0x19 = 0 in the countdown) | | `startscene_counts` (new, arm at the scene; the road-node, path and AI fields are skipped there: one of their pointers is not valid in the scene and faulted the cave) | `n_pair_test`, `n_contact` per race state (framestats.py), `cb_scene`, `cb_sweep`, `n_world_draw` | native side works. **Observed**: scene 178 frames, countdown 171 frames: 0 pair tests, 0 contacts; racing (371 frames): 1 456 pair tests (≤ 12 a frame), 50 contacts. World draws: 0 in the scene, 15 in the countdown. Flow 3 in the scene, 4 from the countdown. The clone side needs a scene arm and a pair-test count accessor |
| Nitrous key reads the previous frame's gas / speed (0x6034C6 before 0x5EB924) | **observed** | `nitro_gas_edge_finished` (new; the stage-4 save's car has nitrous) | `nos_left`, `nos_start`, `key_gas`, `gas`, `speed`; the SPRAY SHOW result | runs. **Observed**: throttle and nitrous injected on the same frame: `key_gas` and the shot start (`nos_start`) appear on the same row and the tank `nos_left` rose 24 000 → 25 600 at that press (not at the second press with the throttle held); what the +1 600 is (**open**: a SPRAY SHOW reward is a guess) and the style result's address are open; the car stands against an obstacle at the resume pose (speed ≤ 0.5 m/s), so add a `[start] place` before using speeds |
| Sticky player road node (0x411490 / 0x40F180) | | `freeroam_node` (new); every race scenario (all racers) | `road_node` (x, y, race coordinate), `road_node_id`, `n_node_update`, `n_node_search` | native works. **Observed**: 0x411490 ran on 86 of 720 frames (3.2–4.6 s and once at 6.8 s), not every update; 0x40F180 on one frame (2 calls); in `circuit_s1_pinned_counts` on 17 of 1 200 race frames. The calling condition is **verified**: 0x42CAA0 → 0x42A190 runs the search only when the human's walk cannot keep the car (docs/race-ai-parity.md). **Ported**: the clone's per-frame node and counts with `NFSU2_NODE_LOG`: 86 update frames in both, 85 in common, the node's record equal on 719 of 721 frames (internal evidence) |
| Human path rebuilt per frame (0x42CAA0) | | `freeroam_node`, every race scenario | `hpath_n` (5 below ~10 m/s, 13–20 above), `hpath_p0`, `hpath_p1`, `n_human_path` | native works: exactly 1 build per frame (720/720) |
| Traffic walk length rebased (0x419670 / 0x422220) | | `circuit_traffic`, `freeroam_stand` + `trafficstats.py`, `traffic_lane_offsets` | metric: traffic turn positions after walks > 200 m | the walk's +0x54 / +0x354 are not recordable (the traffic controller's walk offset is **open**); metric only |
| Reach = max(125 m, 3 s · speed) (0x404910) | | `streetx_s4`, `drag_s4_*`, `outrun_pace_p14` (speeds above 41.7 m/s) | `speed` (reach derived), `ai_obj`, `ai_action`, `ai_target` | the reach itself is not stored natively; compare the avoid / pass decisions above 41.7 m/s |
| GetUnstuck "since placed" clock (car +0x9C0, 0x5ED590) | | `circuit_cd` (grid stamp), a stuck rival: **missing** | `placed_at` (observed: the grid placement's tick), `ai_action` | needs a scenario that sticks a rival (§7) |
| Encounter cars' goal-9 pursuit before a challenge (0x42A860 → 0x428630) | **observed** | `freeroam_encounters_finished`; `freeroam_encounters_poll`, `encounter_approach`, `encounter_follow`, `encounter_pass`, `outrun_poll` (`[run] poll = "encounters"`) | `n_goal9_set`, `n_chase_car`; the poller's table (OUT/native-enc.tsv, the clone's clone-enc.tsv via `NFSU2_ENC_POLL`), `encstats.py` | runs (`human_only`). **Observed**: 0x428630 ran 3 times on 1 633 of 1 800 free-roam frames (2 on 164, 4 on 3), i.e. about once per encounter car per frame (**inferred**: three cars out); ChaseCar 0 times. The goal 9 / action 0x13 / side states are read per sample by `encpoll.py`; ported, compared on `encounter_pass` (docs/race-ai-parity.md, "Encounter pursuit") |
| Race flow state 3/4/7 timing ([[0x8900D8]]) | **observed** | `startscene_counts` (3 → 4), every race scenario (4); after the finish: **missing** | `flow_state` | the 4 → 7 step ≈ 3 s after the player finishes needs a finished race (§7) |
| Cameras (chase shake, cylinder push) | | `camera_bumper|hood|near|high`, `crash_wall`, `audio_freeroam` | `cam_eye`, `cam_fwd`, `cam_fov`; `camstats.py` | works (the camera starts differ at frame 0 in races: compare from a transplanted / free-roam start) |
| Shared-RNG draws per world build / per frame | | any scenario with `[run] counters = ["world_draw"]` | `n_world_draw`, `world_rng`, `shared_rng` | native works; the clone's draw count needs an accessor |

## 5. Frame cadence

* **The original has no fixed step**: 0x57EAE0 turns the measured frame time (call 0x43BE20 at
  0x5812EC) into whole 1/1800 s ticks with a carry in 1/4000 s units, floored by the limiter
  period 0x865558 (1/120 s; 1/30 s in scenes) and capped at 3/60 s (docs/uncapped-fps.md).
* **How the harness pins it**: the cave replaces that call and returns a constant 1000/60 ms
  with the limiter period 0 (`Cave.fixed(60)`), from before the race is accepted. Every frame
  is then a 60 fps frame in game time whatever the wall clock; the original's world steps
  the 60 fps tick pattern, the 3-cycle **30, 29, 31** ticks (16.667, 16.111, 17.222 ms; the
  sim clock 0x865188 steps 67, 64, 69). Measured: every frame of every fixed recording
  checked (circuit_s1 ×2, crash_wall ×2, freeroam_node, circuit_cd, outrun_stand; 4 927 frames)
  is on the 3-cycle, mean exactly 16.6667 ms; 0 deviations (the maintainer's tool `framestats.py`).
  Only the phase of the cycle differs between runs (it starts at 30 or 31).
* **The original at its own pace** (`freeroam_cadence`, `[run] fixed_fps = 0`, the cave only
  recording): 1 200 frames with world dt from 8.33 ms (15 ticks, the 1/120 s cap; 145 frames)
  to 31.7 ms, mean 11.5 ms (~87 fps) on a loaded machine; 1 190 of 1 200 frames off the 60 fps
  cycle. Recordings without the pin cannot be replayed frame by frame.
* **Dropped frames**: with the pin a slow wall-clock frame is not a dropped frame; the game
  just runs slower in wall time (~37 game frames per wall second in a race here). The clone
  steps `ticks[f+1] / 1800` s for frame f, the original's own count, so world time agrees
  exactly; its fixed 1/60 s cadence with the 60 fps tick pattern is the same pattern.
* **Rule**: compare per frame (equality within the §2 tolerances) where the recording's ticks
  are on the 3-cycle, which framestats.py checks; a recording with deviations (an unpinned run,
  or a scene whose limiter re-asserts its 1/30 s period) is compared by world time only:
  resample both sides to common world times and compare windowed means (0.25 s windows) or
  metrics, never row by row.
* **Scenes keep their 1/30 s**: in the start scene the limiter period comes back although the
  cave zeroed it, so scene frames step **60, 60, 60, 60, 61** ticks (33.3 ms; 179 frames in
  `startscene_counts`), and the countdown and the race the 60 fps 3-cycle. The arm points used
  by the race scenarios (`countdown`, `green`) are past the scene, so their rows are all on the
  3-cycle; a scene-armed comparison must step the clone's scene at the recorded ticks too
  (parity.py already passes every frame's ticks in the plan).

## 6. Fields and tools added for the planned fixes (2026-11-11)

All read in a native circuit / free-roam run and found sensible (values in brackets observed):

| field | native | what |
|---|---|---|
| `flow_state` | [[0x8900D8]] | flow state (4 racing / free roam) |
| `shared_rng` | 0x7EA624 | the shared generator |
| `world_rng` | [0x890080] +0xF8 | the world generator (changes on ~5 % of race frames) |
| `car_mode` | car +0x38 | 0 normal, 2 in contact (seen racing), 1 crash |
| `placed_at` | car +0x9C0 | last placement tick (grid: the same tick for every racer) |
| `cb_scene`, `cb_sweep` | [car +0x3C] +0x17 / +0x19 | collision body scene / sweep bytes (sweep 0 at the green, 1 once moving) |
| `road_node`, `road_node_id` | [[car +0x2C] +0x0C] +0 (x, y, race coordinate) / +0x0C | the controller's road node (the player's and every racer's) |
| `hpath_n`, `hpath_p0`, `hpath_p1` | [car +0x30] +0x10 / +0x20 / +0x30 | the input's path record (the human's: 5 points at rest, 13–20 driving) |
| `rb_row0..2`, `rb_spin`, `rb_angmom` | [[car +0x34] +0x20] +0x30/+0x40/+0x50, +0xC0, +0x80 | rigid-body rows (row 0 = car +0x90), ω, L |
| `ai_goal_tgt`, `ai_goal_off` | [[car +0x2C] +0x6FC] +0x18 / +0x20 | goal target and lateral offset (AI racers only) |
| `n_<name>` | 0x8BBF00 + 4k | entry counters (counters.py), only with `[run] counters` |

Tools: `encpoll.py` / `encstats.py` (above, gap 7); the clone's `NFSU2_NODE_LOG=<tsv>`
writes the player's road node (record id, point) and the 0x411490 / 0x40F180 counts per frame.
Tools: `counters.py` (entry counters: `pair_test` 0x5B5060, `contact` 0x5B2620, `world_draw`
0x5EA510, `human_path` 0x42CAA0, `node_update` 0x411490, `node_search` 0x40F180, `goal9_set`
0x428630, `chase_car` 0x4285A0; stubs at 0x782E00, counts at 0x8BBF00; each site's displaced
bytes checked against the image before patching), `framestats.py` (cadence, counters per race /
flow state), `detstats.py` (run-to-run comparison, exact bits). Scenario keys: `[run] arm =
"scene"`, `[run] counters`, `[run] fixed_fps`, `[run] human_only`, `[start] pin_rng` (career events), `[start] pin_phase`.
`native.py` writes `native.rec` / `native.layout.json`; `subtrap.py` snapshots carry `body_hex`.

## 7. Open gaps and recipes

1. **Clone side of the start scene** (scene arm, pair-test count). Add `arm scene` to the
   clone flow (start recording at the scene's first frame, race_state 2) and a global accessor
   counting car–car pair tests and contacts per frame; then `startscene_counts` compares both
   sides and parity.py stops skipping the clone for `arm = "scene"`.
2. **Integrator replay test.** Run any driving scenario with `--keep`, attach
   `subtrap.SubTrap` to the player during the hold and collect ~200 A/B pairs (`body_hex` before
   and after each 0x5AA490); store them under `docs/evidence/<topic>/`; a clone test feeds each
   A state (rows, L, P, I⁻¹, h = the `sub` value) through one sub-step of the clone integrator
   and compares with B. Before the fix it differs in the rows' and ω's low bits; after it the
   rows must match to a few ulps (x87 vs SSE rounding is the floor; record the observed spread).
   **Done** (2026-11-12): `drive::rigid::native_integrator_replay` replays 262 pairs of
   the first drag bit for bit (the maintainer's private captures; the maintainer's tool `subtrap_fixture.py`).
3. **Crash-mode contact scenario.** A car–car hit followed by a wall contact: free roam with
   `[start] place` putting the player 15 m behind a parked traffic / encounter car pointed at a
   wall, throttle held; `car_mode` must read 1 after the hit. Or the lockstep controller
   (`Cave.lockstep` / `Cave.post`) steering into the nearest rival after the green, its inputs
   written into the plan for the clone (not wired yet).
4. **Free-roam input onset.** A key scripted at t = 0 of a free-roam scenario reaches the
   original 8–9 frames late, varying run to run (keys scripted later land on time; why the
   first frames after the hold drop it is **open**, likely the game's own key poll re-sending
   the real key state). Recipe: start scripted inputs ≥ 0.5 s after the arm (the existing
   `crash_wall` and `crash_wall_tp` scripts start at 0 and need this), or in
   `parity.py plan` re-time each key to the first frame the original's key byte shows it
   (`key_gas` for the throttle, `steer` for steering).
5. **Stuck rival** (GetUnstuck): hold a race after the green, `place.put` one rival nose-first
   into a wall at standstill (both sides: a plan line `pose_rival` in the clone flow is
   needed), record `placed_at`, `ai_action` and the reset placement.
6. **Race finish** (flow 4 → 7, ~3 s after the finish): `drag_s1` with lane taps to the line
   (dragshift.py `--replan`) plus `PARITY_EXTRA`, or a circuit with the player placed 9 m
   behind the leader every 0.3 s from Python; record `flow_state` and `racer_status`.
7. **Encounter goals in free roam**: **done**: the maintainer's tool `encpoll.py` reads every
   encounter car out (owner kind 3, off the parking height) through the supervisor's /proc
   reads, two round trips a sample (the controller pointers cached): the car, its goal
   [[car +0x2C] +0x6FC] (+0x14 type, +0x18 target, +0x1C distance, +0x20 offset, +0x24), its
   action [+0x6F8] (+0x1F0 speed, +0x1FC line flag, +0x200 priority, +0x204 type, +0x210..+0x21C)
   and the manager's slot ([0x890118] + 0xC40 + 0x1B10 + 0x10·k), keyed by the sim clock;
   ~35 samples per second of game time while the cave records. `[run] poll = "encounters"` runs
   it beside a recording (OUT/native-enc.tsv). Start kind `encounter_approach` (`[start] back`,
   `moving`, `speed_factor`): the player put behind a driving encounter car; native.py writes
   pose.txt (both cars' frame-0 poses and the car's action / near / side state) for the clone's
   `encounter` plan line. `encstats.py` prints both tables as the car's place relative to the
   player.
8. **Nitrous result**: find the style manager's SPRAY SHOW result (the 0x5ECDC0 caller) and
   what raised the tank by 1 600 at the first press, add them as fields, and give
   `nitro_gas_edge_finished` a `[start] place` on an open road.
9. **Reach above 41.7 m/s**: confirm a rival in `streetx_s4` / `outrun_pace_p14` exceeds it
   long enough to change a decision; otherwise a stage-4 sprint on a stage-4 save.
10. **Traffic walk offsets**: **done**: the walk object is controller +0x370
    (planner +0x54, its walk +0x31C); the maintainer's tool `trafwalk.py` reads every traffic car's walk
    (+0x54 / +0x58 / record / control point / junction per point, +0x354, the junction cache),
    mover mode, heading +0x40C and speed +0x42C through /proc, two round trips a sample
    (`[run] poll = "traffic_walk"`, OUT/native-walk.tsv; scenarios `freeroam_walk`, `qr_walk_4014`).
    Its last column is the human's road node with the placer's walk-byte inputs (record
    +0x70, car +0x430, the player object's wrong-way byte +0x248, 0x88F2BE).
11. **Unmatched pieces of the old notes**: `nos_left` (car +0x45C, i32) read float bit
    patterns in early runs (e.g. 1065353216 = 1.0f); check its type before relying on it.

## Appendix: local setup

Setup-specific; not part of the method.

* Wine tools: the maintainer's private Wine folder (README.md, fastgpu.sh, peek.py; `$PARITY_WINE`). Each
  run uses its own copy of it (`--copy` or `$PARITY_COPY`; native.py refuses copies outside the
  `wine-*` copies beside it) and its own Xvfb display ≥ 140 (`--display` or `$PARITY_DISPLAY`); never the
  shared `wine/`, never the desktop's display.
* A fresh copy of `wine/` updates its prefix on the first start, and the update hangs in
  Wine Mono's installer (`control.exe appwiz.cpl install_mono`). Update it once offline first:
  start a private Xvfb first, make a private runtime folder (`mkdir -p <copy>/xdg && chmod 700 <copy>/xdg`), then `env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE XDG_RUNTIME_DIR=<copy>/xdg DISPLAY=:<your Xvfb> WINEPREFIX=<copy>/prefix WINEDLLOVERRIDES="mscoree=;mshtml=" wineboot -u`, and pin the prefix to X11 with the same environment: `wine reg add 'HKCU\Software\Wine\Drivers' /v Graphics /d x11 /f` (never run a Wine command with DISPLAY cleared: without an X display Wine falls back to the desktop session's default Wayland socket even when WAYLAND_DISPLAY is unset), then
  `wineserver -w` (that prefix only).
* Game files: `$NFSU2_ROOT` (parity.py's default is the maintainer's verified copy).
  Build: `. scripts/local.env; CARGO_TARGET_DIR=<own target> cargo build --release
  --bin game --features dev-harness`; `$PARITY_GAME` or `$CARGO_TARGET_DIR/release/game`.
* Work dirs: `$PARITY_WORK` (the default is an older private run folder; set it); keep runs out of /tmp.

## Appendix: random-number draws per call site

* the private tool `rngsites.py patch <your Wine copy>` writes a patched SPEED2.EXE into **your own Wine
  copy's** game folder (the original kept as `SPEED2.EXE.orig`; `restore` puts it back): each of the 206 direct
  calls to a draw function (shared 0x43C1C0 / 0x43C1F0, world 0x5EA4F0 / 0x5EA510, the state-pointer forms
  0x43C140 / 0x43C170, the seed 0x43E6B0, speech 0x738750 / 0x738860, AEMS 0x724903) and the two inline seed
  steps of the particle generator 0x7F3940 count into a dword of their own from the process's first
  instruction (stubs in the nop padding between functions; counts at 0x8BB600 +4·site, totals per draw
  function at 0x8BBB80). The stubs live in function padding below 0x780000, so the cave, `counters.py` and
  `subtrap.py` still work on the patched copy (the `Probes` of an earlier Wine run, which uses 0x8BB600, does not).
* the private tool `rngdraws.py OUT [--event circuit|drag|none] [--shop X,Y,Z] [--audio]` runs one session on it
  (boot, 600 menu frames, free-roam entry, 600 free-roam frames, then the event — shared generator pinned at
  the dialog — or the shop) with the per-site table recorded every frame through the cave;
  `rngdraws.py report OUT` prints per phase and per frame. The results (a port list per call site and
  the captures) are kept with the maintainer's private evidence. Each row also holds player 1's NIS id (`[0x8900AC] +0x234`, `nis`), the camera
  director's gate (docs/rng.md).
* Clone side (`dev-harness` builds): `NFSU2_RNG_LOG=<file>` logs every `GameRng` draw with its caller and
  generator instance (src/opponents.rs `draw_log`); the maintainer's tool `rnglog.py` sums two such logs by site.
