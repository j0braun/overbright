# Differential parity harness

The step-by-step for proving a simulation fix against the original, the coverage table and the
open gaps are in docs/developing/parity-workflow.md.

One scenario, run in the original (SPEED2.EXE under Wine) and in the clone
from the same start state with the same inputs on the same frames, recorded
every world frame and diffed field by field. The report names the first
frame where a field leaves its tolerance, with the rows around it.

**Verified** = traced in SPEED2.EXE (disassembly), **observed** = read under
Wine, **inferred** / **open** as marked.

## Use

The harness's driver (`parity.py`, `native.py`, the scenarios) is one of the maintainer's private
tools; the clone side is `--flow parity` in the developer harness.

```
. scripts/local.env; cargo build --release --bin game --features dev-harness   # --flow parity is dev-harness only
export NFSU2_ROOT=…; export CARGO_TARGET_DIR=…            # the game binary is $CARGO_TARGET_DIR/release/game
parity.py run circuit_s1.toml [OUT]   # original, clone, diff
parity.py clone circuit_s1.toml OUT   # clone + diff again (native kept)
parity.py diff OUT
parity.py summary OUT1 OUT2 …                                # the summary table
```

`run` takes 3–5 minutes (boot ~2 min, the fixed-step race at about 110 game frames
per wall second on DXVK); `clone` takes seconds. OUT (default
`$PARITY_WORK/<name>`, a scratch folder of your choice) holds:

| file | what |
|---|---|
| `native.tsv`, `native.json` | the original's rows (one per frame from the arm), its cars (world list slot, model, owner kind +0x24, controller type), the input plan, the tick counts |
| `clone.plan` | what the clone flow runs (written by `parity.py` from the scenario and `native.json`) |
| `clone.tsv`, `clone.tsv.cars` | the clone's rows and car models |
| `report.txt` | car matching, frame-0 race clocks, **the first divergence**, the first frame out of tolerance per field, context rows, position error per second |
| `diff.tsv` | per frame: the worst error / tolerance and its field, each matched car's position error (m) |
| `summary.txt` | the one-line summary `parity.py summary` prints |

Options of `native.py` (passed after OUT): `--copy` (the Wine copy, default
a private one, or `$PARITY_COPY`), `--display` (default 206, or `$PARITY_DISPLAY`), `--keep`
(leave the session running), `--wined3d` (the original on wined3d instead of DXVK; see
"Renderer and frame rate"), `--min-fps` / `--min-rate` (the frame-rate guard). The session is
started and stopped through `fastgpu.sh` (by PID, never by name).

## Renderer and frame rate

Every native tool (`native.py`, `trafh_native.py` and `trafh.py --wine`, `dragshift.py`,
`recdump.py`, `rngdraws.py`, `snapdiff.py`, `tools/rating/rating.py`, all through
`native.Session`; `tools/look/wine_ref.py`) starts the original on **DXVK** by default. On the
reference desktop GPU the original then runs at its own rate: about 114 fps in a Quick Race, the
race at 100.9 % of real time on a quiet machine. wined3d's Vulkan renderer reaches only about
55 fps there (77 % of real time in a Quick Race), half the original's rate, so it is used only
when asked for: `--wined3d` on a tool, or `NATIVE_RENDERER=wined3d` for every tool of the
process. An older note said DXVK froze at the profile load ("Command submission failed: -13");
that did not reproduce on a quiet machine — the DXVK device loss shows up under heavy load
(docs/notes/parity/drive-parity.md), one more reason to measure only on a quiet machine. The
navigator's screen references were taken under wined3d; under DXVK its main-menu crop scores
an RMSE of about 0.093 (every other screen 0.29 or more), so its match threshold is 0.12 under
DXVK and 0.08 under wined3d (`nav.THRESHOLD`; `NAV_THRESHOLD` overrides, `NAV_DEBUG=1` logs
each match).

**The frame-rate guard.** Each native run measures the original's frame rate in the running
game (`native.FpsMeter`): world updates (0x7FB724, +1 per frame the world ran) per game second
(the clock 0x865188), and game seconds per wall second. A real-time run counts only when it
reached **at least 100 fps and 95 % of real time** (`--min-fps` / `--min-rate`, or
`NATIVE_MIN_FPS` / `NATIVE_MIN_RATE`). `trafh_native.py` writes the line
`# fps F rate R wall_fps W renderer … load L min_fps 100 min_rate 0.95 valid 1|0 (why)` at the
end of its table and exits with code 3 when the run is invalid; `trafh.py run` then keeps that
table aside as `native-<scen>-r<k>.slow` (the next `run` boots that run again) and exits
non-zero, and `trafh.py stats` never counts a table marked `valid 0` and names the native tables
that have no `# fps` line (from before the guard: their rate is unknown). `native.py` writes the
same numbers as `fps` into `native.json`; a fixed-step run (the default `fixed_fps = 60`: every
frame is 1/60 s of game time whatever the wall clock) only logs them, a `fixed_fps = 0` run is
guarded and exits with code 3 when too slow (`parity.py run` reports it).

**Fixed 60 for AI measurements.** The original's AI is frame-rate dependent, so a real-time run
measures it at whatever rate the machine gave. Its world step follows the measured frame time
(quantised to 1/1800 s, so physics hardly moves with the rate), but the AI is counted in frames:
each traffic car thinks once every 2N frames (0x429D90, its slot on the frame counter 0x7FB724:
0.2 s at 120 fps, 0.4 s at 60), a racer or encounter car thinks on its frame slot (mod 8, mod 4 in
a drag) once at least 0.2 s have passed (0x42ABD0: about 0.2 s at 120 fps, 0.27 s at 60), and
steering and avoidance run every frame (docs/formats/ai.md "Traffic", docs/original/coverage/racer-ai.md).
A real-time run at 72-86 fps matches neither the original's own 120 fps cap nor this game's
world, which always ticks at 1/60 s. `trafh_native.py` (and `trafh.py run --wine`) therefore
runs the original with the cave's fixed frame time by default: `--fixed-fps 60` installs the
cave (no recording, no lockstep) and sets every frame to 1/60 s of game time, free running, as
`native.py`'s default `fixed_fps = 60` and docs/evidence/engine-bed/scripts/drive_rt_f.py do.
With the limiter off the original then runs free (about 480 fps on the reference GPU: 8 game
seconds per wall second, only 2-3 polls per game second, too few for the bot), so the run also
sets DXVK's own frame cap to N (`DXVK_CONFIG` `d3d9.maxFrameRate`, a wall-clock pacer in the
presenter; nothing in the game changes): a quiet run then holds about 60 fps and 0.99 game
seconds per wall second, with 20-25 polls per game second (`--no-pace` leaves it free). The bot's
keys, the shove and the polls are on the game clock; the pace only sets how much game time one
poll covers. A run whose game clock stops for 60 s ends with an error. The
guard of a fixed run wants **the cave still in place at the end** (the main loop's call, FIXED,
1000/N ms, the limiter period at most 1/N s: `cave.Cave.fixed_held`) **and at least N − 1 world
updates per game second** (`# fps … fixed 60 valid 1|0`) instead of the 100 fps / 95 % test. More
than N is allowed: frames in which the game clock stands (a race's finish and results) add updates
without game time, so a 40 s drag reads about 65. In a drag the original sets the limiter period
to 1/30 s at the race end, when every car stands at the finish (observed; inferred to be the
race-ending scene, the writer is not trapped), so the guard fails a `qr:drag` run that goes past
it: keep a drag run shorter than its race, or cut its rows at the period change and say so. The rate is logged only, and the machine load
does not change the result. The table header carries `fixed=60`; `trafh.py
stats` puts each fixed step in its own column (`native-fixed60`) and real-time runs in
`native`, never mixed. Use fixed 60 for every AI comparison with this game (traffic, encounters,
racers, a drag); `--fixed-fps 120` measures how far the original itself moves between 60 and its
own cap. Keep real time (`--fixed-fps 0`, the guard above) for wall-bound things such as a load
or streaming delay, which a fixed frame would stretch or shrink; an outrun defaults to real time
until its fixed run has been checked.

**A valid native measurement** (for a "native" verdict or a native number in a note):
- the original ran on DXVK, on a quiet machine: no gate or other heavy check running, no other
  session of the original, load well under the core count, the GPU idle before the run;
- its run carries the guard's line with `valid 1` (or `native.json` `fps.valid`), i.e. a
  real-time run reached at least 100 fps and 95 % of real time, a fixed run kept the cave in place
  and at least N − 1 world updates per game second — a run without the line, or a real-time run on wined3d, is indicative
  only and is said to be so where it is quoted;
- the note quoting it gives the renderer and the measured fps with the number.

## Adding a scenario

A scenario is a small TOML file in the maintainer's tools:

```toml
name = "sprint_s1"
save = "tests/fixtures/native_saves/stage1_win_streetx.sav"   # native save (repo-relative or absolute; this one is in the private test set)
profile = "NAME"                                               # the profile folder it is installed as
[start]
kind = "career_event"            # or "outrun"
event = "1FD60BD2"               # the event's hash (+0x08): parity_events SAVE lists the open ones
trigger = [-514.0, 311.0, 1.669] # its world trigger box centre (same example)
[run]
seconds = 20                     # frames recorded after the arm = seconds × 60
arm = "green"                    # frame 0: race status 4 (career events) / race type 7 (outrun)
inputs = [ { t = 0.0, key = "throttle" }, { t = 2.0, key = "left" }, { t = 2.5, key = "left", down = false } ]
[clone_env]                      # optional: environment for the clone flow
```

Keys: `throttle brake left right handbrake nitro shift_up shift_down`; `t`
in seconds after the arm (frame = round(t × 60)); a negative `t` is held
from the race's start (scene and countdown). A private developer probe (`parity_events SAVE
[HASH…]`) prints a native save's open events (and any listed by hash) with
their trigger boxes.

## Adding a field

One line in the maintainer's `fields.toml`:

```toml
speed = { scope = "car", native = "0x430", type = "f32", clone = "speed", tol = 0.5 }
```

`native` is a path from the car pointer (scope car) or an absolute address
(scope global); each `>` reads a pointer and adds the next offset
(`"0x2C>0x6F8>0x204"` = [[car+0x2C]+0x6F8]+0x204). Types: `f32 i32 u8 u16
f32x3 angle16`. `clone` names the accessor in `src/bin/game/dev/parity.rs`
(`car_value` / `global_value`); a new accessor is one match arm there, and
the test `every_field_accessor_is_known` fails while it is missing.
A negative `tol` records the field without ever counting it as a divergence.

### Fields now

| field | native | clone | tol |
|---|---|---|---|
| race_state | race status [0x890118] +0: 2 scene, 3 countdown, 4 racing (**observed**) | clock < 0 → 3, else 4 | 0 |
| race_clock | race status +0x14, 1/4000 s since the green (**observed**) | race clock × 4000 | 40 |
| race_type | 0x89E7B0 (1 circuit observed, 7 outrun) | 7 in an outrun, else none | 0 |
| cash | career 0x85AD40 + 0x7134 | profile cash | 0 |
| pos / vel / fwd | car +0x60 / +0x70 / +0x90 | position / velocity / forward | 0.25 m / 0.5 m/s / 0.03 |
| speed, gear, rpm | car +0x430, +0x434 (0 R, 1 N, 2 first), +0x440 (smoothed rpm) | forward speed, drivetrain gear, rpm | 0.5, 0, 300 |
| steer | input (car +0x30) +0x1E0, 16-bit wheel angle, degrees | `steer_degrees` | 1.0 |
| gas, brake, handbrake | input +0x20C, +0x214, +0x210 (the pedals the mover reads) | `last_gas`, `brake_input`, AI handbrake | 0.05 |
| nos_left | car +0x41C, tank left (ticks) | tank `left` | 400 |
| ai_action | [[car+0x2C]+0x6F8]+0x204, the running action's type (AI racers only) | `Actions::kind.type_id()` | 0 |
| ai_goal | [[car+0x2C]+0x6FC]+0x14 (context only) | — | −1 |
| lap, place, progress | racer (car +0x1C) +0x08 lap, +0x0A place (**observed**), +0x14 lap + fraction | Laps lap, race order, Laps progress | 0, 0, 0.01 |

## How the original is driven (the maintainer's tool `cave.s`, cave.py)

* **The cave.** 489 bytes at 0x782B00 (zero padding at the end of .text,
  which ends at 0x782AF1), control block at 0x8BBC00 (0x8BB600.. is never
  referenced by SPEED2.EXE; checked in the disassembly). The main loop's
  `call 0x43BE20` at **0x5812EC** (the measured frame time in ms, returned
  on the FPU; **verified**) is pointed at the cave, once per main-loop
  iteration, before FrameLimit_Wait 0x57EAE0 and the world update
  (0x5813D4..0x581417). `cave.uninstall()` points it back.
* **Fixed frame time.** With FIXED the cave returns 1000/60 ms and the
  limiter period 0x865558 is 0: every frame is a 60 fps frame in game time
  (ticks 30, 29, 31 of 1/1800 s, 0x57EAE0), whatever the wall clock (the
  game runs at ~50 frames per wall second under wined3d, ~110 under DXVK). This is the uncapped-FPS study's
  method (docs/original/uncapped-fps.md), which repeated runs to 3 decimals. Wine
  timing variance therefore does not reach the simulation; the clone steps
  the same per-frame ticks (from `native.json`).
* **Arm.** Each frame the cave tests the trigger (a dword through up to 3
  pointer reads, equal / not equal): race status +0 = 4 for a race, race
  type 0x89E7B0 = 7 for an outrun. Frame 0 is the first frame start that
  sees it, i.e. the state after the world update in which it changed.
* **Inputs.** Each frame's scripted events are calls of the native JOY
  event handlers, `handler(device, event, value, self)` (cdecl, as the
  dispatcher 0x5809D0 calls them, **verified** from the handlers'
  prologues): throttle 0x416DE0 event 0x91, brake 0x407EB0 0x94, e-brake
  0x407F40 0x99, steering 0x407750 0x8F (value 0x00 left alone, 0xFF right
  alone, 0x80 none, as the STEER row 0x571CD0), shift up 0x407FC0 0x9B,
  down 0x4080B0 0x9C, nitrous 0x605C60 0xA3 (→ 0x602BE0 on the player
  object). `self` and the device come from the handler registry
  (0x579420: lists at 0x8655A0 + device × 0x1C8; node +8 event, +0x10
  handler, +0x14 self, 0x579370) and are checked against the player car
  (input +0x2A8). No key is pressed: the keyboard poll sends events only on
  a change, so nothing overrides the injected state. A key's press and
  release land on exact frames, before that frame's update. A held level
  key (throttle, brake, e-brake, steering) is sent again every frame
  while down, as a physically held key would be: in the first drag run the
  throttle sent once at the green was gone half a second later (raw pedal
  +0x2D4 = 0; the launch window's 0.7 floor hid it until then). The likely
  cause is a handler node registered anew (+0x1C pending, 0x579370, makes
  the dispatcher send the key's real state); **inferred**, not traced.
  Edge keys (nitrous, shifts) are sent once.
* **Record.** After the trigger, each frame start copies the field table
  (built from fields.toml: per car `[0x890080] + 0x1C + 4·slot → path`)
  into a VirtualAlloc'd ring (64 MB, through the IAT slot 0x7831F4), with
  a header: frame, frames since enable, 0x86513C (the ticks of the update
  that led here), 0x865188. Pointers under 0x10000 record zeros. AI-only
  fields are recorded only for cars whose controller type ([car+0x14]+4)
  is 3.
* **Lockstep** (cave LOCK/ACK, `Cave.lockstep` / `Cave.post`): the game
  waits after each record until a controller in Python posts that frame's
  inputs; measured 28 frames per wall second in free roam. It is there for
  closed-loop native drivers (a bot whose inputs are recorded and replayed
  in the clone; not wired into scenarios yet), and it is how the start is
  held (`Cave.hold` / `Cave.rearm`).

## How the start state is made

* **Career event** (`start.kind = "career_event"`). The save is installed
  as the only profile of the private Wine copy (it auto-loads); the front
  end is driven Career → Resume Career → (garage) Return to Explore Mode by
  key presses and reference crops (the maintainer's tool `nav.py`, `refs/`); the
  car is put in the event's trigger box (rigid body +0x20/+0x30/+0xD0/+0x70/
  +0x80 and car +0x60/+0x70 written with the game stopped, then read back:
  `place.py`, the outrun units' method); Enter opens the race dialog,
  Left + Enter accepts. The cave **holds the game** (its lock, see below)
  at the first frame of the countdown (race status 3: the world list then
  holds the racers and the human input its handlers); the cars, the
  handler owners and the field table are read and written while it waits,
  then it is released and arms at the green. (Polling for the countdown
  from Python missed the short drag countdown once.)
  The clone loads the same save (`profiles::write`, Resume to free roam),
  starts the same event (`Game::start_race`), runs the start scene and
  countdown as the window does and starts recording at the first frame
  with its race clock ≥ 0.
* **The player's grid slot.** The original draws it: two runs of the
  same circuit from the same save put the player in two different slots
  (observed: (−29.6, 363.4) and (−36.4, 368.3)); the drag and the sprint
  runs had it in another slot than the clone's. So at the green the
  clone's player swaps place (position, orientation, velocity) with the
  car nearest the original player's frame-0 place (`player_slot` in the
  plan; `same_grid = false` under `[run]` turns it off). Which rule draws
  the slot natively is **open**.
* **Outrun** (`kind = "outrun"`): the first encounter car out and driving
  is followed (`outrun.py`, the outrun parity's method): the player is put 60 m and
  then 10 m behind it at its speed with the throttle held (a cave event)
  until the career offers the challenge (0x860A58 = 2), which Enter takes.
  The cave holds the game at the first frame with race type 7 while it is
  configured; that frame is frame 0; both cars' frame-0 pose goes
  to `pose.txt`, from which the clone places its player and an encounter
  car (forced to the native model with `NFSU2_ENCOUNTER_CAR` in
  `[clone_env]` when wanted) and engages.

## Alignment and tolerances

* the maintainer's tool `selftest.py` checks the diff side offline (synthetic runs:
  car matching with a different order, the first divergence frame).
* Rows are aligned by frame from the arm; both sides record the state
  before that frame's inputs and update. The clone steps `ticks[f+1] /
  1800` s (the original's own tick count for that update), so world time
  agrees exactly; the race clocks at frame 0 are printed in the report.
* Cars are matched at frame 0 (the human to the player; the others by
  position, a different model costing 50 m; none beyond 15 m). The clone
  lists its race traffic too; the original's world list holds the racers
  (and, in free roam, the encounter cars) — traffic there is **open**.
* Tolerances are absolute per component; the first frame with any field
  beyond its tolerance is the first divergence. Later frames still show
  how far apart the runs drift (position error per second in the report,
  `diff.tsv`).

## Known limits

* **Random numbers.** The AI's draws (route choices, skill rows, traffic,
  the encounter model) come from each game's own random state; the clone's
  is not the original's (`opponents::GameRng`). A divergence that starts
  with an AI decision may be this.
* **Before the arm.** The start scene and countdown run under the fixed
  frame time on both sides, but the scene limiter (1/30 s) and the NIS
  hand-off are each game's own; the frame-0 rows show where the cars stand
  at the green.
* **Renderer.** The original runs on DXVK (see "Renderer and frame rate"); the
  results below were taken on wined3d's Vulkan renderer, when DXVK was thought
  to freeze at the profile load. In a fixed-step record the renderer does not
  touch the simulation; in a real-time run it sets the frame rate, and with it
  every per-frame decision (see the frame-rate guard).
* **Fields not traced end to end** are marked observed/inferred above (the
  race status state and clock, the racer's place byte).
* The clone's global `race_type` is only known for outruns.

## Results (2026-11-04)

All four starter scenarios ran end to end on both sides (original:
a private Wine copy on :206, wined3d; raw runs kept with the maintainer's private evidence; reports, plans and car
lists in the maintainer's private captures). 20 s from the arm each
(1201 frames; the outrun 1199). Every run diverges at frame 0, i.e. the
start states already differ; what differs is the finding:

| scenario | first divergence | what the report shows |
|---|---|---|
| circuit_s1 (STAGE_1_CIRCUIT_1) | f0, player pos | the original's back-right grid slot is 1.7 m from the clone's nearest slot (the others within 0.5 m); every rival at the green: clone brake 1, steer 2.8–3.6°, rpm 7450, action 6 (JustDrive) vs original brake 0, steer 0, rpm 6920–7940; the clone's rivals' gas is 0.52 on the first frame, the original's 1.0; rival 1 is 63 m off after 20 s; player 0.7 m off at the end |
| drag_s1 (STAGE_1_DRAG_1, 4201) | f0, player pos (0.44 m) | the launch window's gas ramp 0.617 → 0.75 is identical on both sides for 10 frames; from 0.45 s the original's player gas is 0 although the throttle is re-sent every frame (rpm idles at 800, ~1 m/s), the clone's stays 1 and it drives off; the clone's player leaves the race (rows end) near 10 s. **Open**: whether the original's drop is native (a drag input gate) or the harness (see Inputs); not resolved |
| sprint_s1 (STAGE_1_SPRINT_1, 4101) | f0, player pos (0.31 m) | grid within 0.5 m after the slot swap; rivals' frame-0 state as in the circuit (brake 1, steer, rpm); the player's launch is quicker in the clone (5.06 vs 3.43 m/s at 1.2 s, rpm 1948 vs 1472); 160 m apart after 20 s |
| outrun_finished (the finished-career save, Skyline vs COROLLA) | f0, rival steer 1.37° vs 3.33° | the pose transplant puts both cars on the original's frame-0 pose (0.0 m), gears included; rival's gas 1 vs 0 at f0 and rpm 8156 vs 4744 at f1 (the MeloMvr's rpm is not transplanted); the player's rpm 1160 vs 2296 at f1 and speed +0.7 m/s by f4, an up-shift 1.12 s in where the original stays in first; within 4.4 m for 6 s, then the cars part (47 m at 8 s) |

Common to all: the opponent models differ (the opponent draw; the
circuit's COROLLA/MIATA/240SX against the clone's MIATA/FOCUS/MIATA), the
car position z is 0.25–0.35 m higher in the original at rest (car +0x60
against the clone's position: a reference-point difference or a ride
height, **open**), the original's lap byte reads 0 or −1 where the clone's
reads −1 before the line, and `ai_action` reads 0 for every original rival
at the green ([[car+0x2C]+0x6F8] may be null then: zeros; **inferred**).
The native race status and racer offsets used are listed under Fields.

## Additions (2026-11-04, docs/notes/parity/race-ai-parity.md)

* **Same cars.** For a career event the clone's rivals are forced to the
  original's models (`NFSU2_BENCH_RIVAL_CARS`, set by `parity.py`), in
  racer order: the racer number car +0x0A (100 + entry, the field `racer`;
  it equals the world list order in every run so far), else the world
  list order. `same_cars = false` under `[run]` turns it off;
  `PARITY_RIVAL_CARS=A,B,C` overrides it by hand. The skill rows follow
  the entry (event opponent i), so entry order matters.
* **Same grid.** Every racer is put on the original's frame-0 place and
  heading (`grid` lines in the plan, matched by model; the player as
  `player`), keeping its own rest height. `player_slot` (the swap) is the
  fallback when the models differ. The slot draw itself is native and
  random: 0x5264B0 → 0x4FEDE0 swaps racer 0's slot (+0x03) with that of a
  racer drawn by 0x43C1C0(humans + AI) (**verified**; the clone's
  `grid_slots`, `GridOrder::Shuffled`), so two runs differ.
* **Position = centre of mass.** Car +0x60 is the body's centre of mass;
  the clone's position is the body origin less the turned offset
  (`CarState::placed`). The `pos` accessor now adds it: the 0.25–0.35 m
  "rest height" gap of the first harness run was this reference point (frame-0 z now within
  0.01 m), not a ride height.
* **`arm = "countdown"`**: frame 0 is the countdown's first frame (race
  status 3; the clone: the first frame past the start scene's cut). The
  `race_state` accessor reads 2 while the scene's camera runs.
* **New fields** (fields.toml): `wheels` (+0x390..+0x393 one byte each),
  `ai_target` (driver +0x230), `ai_look` (+0x224/+0x228), `ai_path_n`
  (+0x10), `ai_catch` (controller +0x770), `catch_opt` (0x89E7F8),
  `gear_req` (+0x1E4), `racer` (+0x0A), `car_rng` (+0xF8), `raw_gas`
  (+0x2D4), `key_gas` (+0x2D0); context-only ones read `context` (NaN) on
  the clone side.
* **Navigation**: later-stage saves (world list up behind the garage, the
  SMS inbox popping up in free roam, slow loads) are handled (`nav.py`:
  free roam only with no garage screen and the clock running; Escape on
  the inbox; 240 s load wait; the main menu's Left retried).
* the private tool `aistats.py OUT…`: per matched rival, speed at 1/2/5 s,
  distance, mean target and catch-up from 6 s, action changes, reversing
  time, the worst crash drop.
* **Drag input (open item 4)**: in drag_s1 the player's event-0x91 value
  (+0x2D0) changes from the injected 255 to 172 at 0.5 s and to 0 at
  1.17 s although the cave sends 255 before every update, and the gas
  follows (0.7, then 0). No scripted event carries 172, so another caller
  of the throttle handler overrides the injected key after the cave's
  call: the game's own input poll sending the real (unpressed) key state
  (**inferred**: a real held key would read as held there). Harness
  artefact, not native drag behaviour; injecting at the DirectInput key
  state instead of the handlers is the open fix. Drag comparisons hold for
  the rivals (their launch matches at the green), not the player.
* **Quick Race start** (`[start] kind = "quick_race"`, `mode`, `lap_ko`,
  `car`, `menu` = keys from the main menu, `menu_wait`): the original is
  booted to the main menu (`nav.boot_to_main`) and driven by the keys (a
  key `tap:K` is a 50 ms press; `A+B` several), a screenshot after each
  (`OUT/menu-NN.png`), then held at the countdown as a career event; the
  clone starts `quick_event(mode)` with `quick_car` (plan line `quick
  MODE KO CAR`). Scenarios `lapko_qr`, `url_qr`.
* **Car data**: native.json keeps each car's record (car +0xC, the stock
  record), car +0x20 (the upgrade state) and the racer infos
  ([0x89E810] + 0x940·i: floats +0x300, fitted part bytes +0x350, rating
  +0x28); `NFSU2_DUMP_RIVAL_RECORDS=DIR` writes the clone's rivals'
  upgraded records and fitted part lists.

## Additions (2026-11-04, docs/notes/parity/world-systems-parity.md)

* **Traffic**: the original's traffic is in the same world list, slots +0x18 .. +0x10
  (+0x10 the total; observed in free roam and a circuit). `[run] traffic = true` records it
  (fields with `traffic = true`; traffic-only `mover`, `traffic_action`, `parked`,
  `drawn`); diff.py does not match traffic cars; `trafficstats.py OUT` compares the pools,
  a private developer probe (`traffic_lane_offsets OUT`) the lane keeping.
* **`[start] kind = "free_roam"`**: held at once in free roam, armed the next frame; the
  clone takes the original's frame-0 player pose. Scenarios `freeroam_stand`,
  `crash_wall`, `circuit_traffic`.
* **Camera**: global `cam_eye`, `cam_fwd`, `cam_fov` (view 1, 0x8760E0 +0x40 / +0x50 /
  +0xC4); `camstats.py OUT`.

## Additions (2026-11-04, docs/notes/parity/drag-drift-outrun-parity.md)

* **Deeper fields**: a field may follow 5 pointers (the cave's entry is
  addr, count, 5 offsets, length); a pointer past 0x7FFF0000 reads as null
  (SPEED2.EXE is not large-address aware).
* **Drift fields** (`drift_state`, `drift_pts`, `drift_bank`, `drift_mult`,
  `drift_total`, `drift_level`, `drift_surf`, `drift_near`) only with
  `[run] drift = true` (the managers exist in a drift race only);
  `[run] skip = [...]` leaves fields out; `yaw_rate` (car +0x118), `behind`
  (racer +0x24), `racer_status` (+0x10).
* **Outrun**: only the player and the challenged car are recorded (the
  other encounter cars' racer pointers are stale); recording past the
  outrun's end faulted the game, so `[run] tail = S` reads the race type,
  clock, the rival's +0x24 and status from Python after the record until
  race type 0 (`tail.tsv`); the clone's encounter car takes the original's
  model (`NFSU2_ENCOUNTER_CAR=<model>:-` from `pose.txt`). A longer clone
  run: copy `clone.plan` with a larger `frames` (missing ticks step 1/60 s).
* **Tutorial movies**: `accept_race` presses Return every 10 s from 20 s
  while the race has not loaded (the first drift / drag plays its tutorial),
  with a screenshot each (`accept-N.png`).
* a private developer probe (`drift_replay native.tsv SLOT CLASS TOP [FACTOR]`): the clone's
  drift scorer on a car's recorded native kinematics.
* Scenarios `drift_s1`, `outrun_stand`.

## Additions (2026-11-05, docs/notes/parity/tuning-physics-parity.md C)

* **Fields** (context only): drivetrain (mover +0x4C) `dt_tgt` +0x38, `dt_54`, `dt_68` the drag
  shift grade, `dt_6c`, `dt_78` / `dt_7c` / `dt_80` the random shift offsets, `dt_up2..5` the
  up-shift rpm +0x8C..+0x98, `dt_cc..dt_e0` the offsets' ranges, `dt_e4`, `dt_e8` the clutch
  factor, `dt_f0` the class. `dt_68`, `dt_78`, `dt_80`, `dt_cc`, `dt_d0`, `dt_dc`, `dt_e0`,
  `dt_e8` have clone accessors. A native path takes at most three pointer reads after the car
  (cave.py packs four offsets, the car's slot read being the first).
* the private tool `shifts.py OUT…`: per matched car, each gear change (frame, gears, frames not
  locked, shown rpm before, the grade).

## Additions (2026-11-05, docs/notes/parity/world-systems-parity.md)

* **Save patches** (the maintainer's tool `savepatch.py`): `[start] camera = N` (the player's camera
  setting, profile player 0 +0x10 → 0x83AA74: 0 bumper, 1 hood, 2 near, 3 far, 4 Super Far)
  and `[start] volumes = [master, speech, menu music, game music, effects, engine, tires]`
  rewrite a copy of the save (`OUT/scenario.sav`, checksums renewed) that both sides load.
* **Audio** (`[run] audio = true`): the original runs with `fastgpu.sh --audio-file
  OUT/native.wav` (+ `native.sync.tsv`) and `native.py`'s EventWatch logs every live AEMS
  instance's parameter block with the sim clock every ~40 ms (`native-events.tsv`); the
  clone writes its offline mix (`NFSU2_PARITY_PCM`, 48 kHz f32, `clone.f32`) and its events
  (`NFSU2_EVENT_LOG`, `clone-events.tsv`, time = frame / 60). The private tool `audiomix.py OUT
  [name:t0-t1,…]` prints RMS dBFS per window for both mixes (the WAV placed on the frames
  through the sim clock) and, per event (CAR by its p0 slot, FX_ROADNOISE by side), the
  median parameter block of each side.
* **Transplant** (`[run] transplant = F`): before frame F's update the clone's player takes
  the original's centre of mass, velocity and forward axis (spin zeroed; plan line `pose`):
  a response (a wall hit) compared from the same incidence.
* **Look back**: key `look_back` (the original's event 0x7E through 0x605C60 on the player
  object, re-sent while held; the clone's Controls row 9, B).
* Scenarios: `audio_freeroam` (idle, throttle, brake, look back; audio), its
  `_nomusic` / `_nospeech` variants, `audio_race` (STAGE_1_CIRCUIT_1 from the countdown,
  audio), `camera_bumper|hood|near|high` (the same drive per camera setting) and `crash_wall_tp`
  (crash_wall with the transplant at frame 190).

## Additions (2026-11-05, docs/notes/parity/world-systems-parity.md §7)

* **audiomix.py sync fix**: the sim clock 0x865188 starts again at the world's load, so the
  run's clocks are mapped through the last rising stretch of `native.sync.tsv` (before, the
  native windows fell in the boot / front end: The audio and camera parity's "3–5 dB louder" was this).
* Scenario `audio_freeroam_musiconly` (speech, effects, engine, tires 0 by the profile: the
  EA Trax alone; it lived where the audio and camera parity's game-music-0 run died).
* a private developer probe (`trax_dump OUTDIR [S]`) (the decoded songs, mono 11025 Hz) with
  internal evidence WAV T…` (which song and at what level a
  native WAV plays); a private developer probe (`lane_speed_at FILE…`) (lane speed bytes at points).
* Reading AI state the cave faulted on: internal evidence polls
  through /proc (peek.Session) after `native.py … --keep` (no pointer chase in the game).

## Additions (2026-11-05)
* `NFSU2_TRAFFIC_SEED=<n>` in the clone's environment varies its traffic draws (xor into the pool's
  seed); traffic statistics are compared as means over several seeds
  (internal evidence, `multirace.sh`), one seed being too noisy.
* The traffic field `mover` reads 4 for a clone car on the BeltMover.
* Scenario `freeroam_probe` (3 s) only brings the original to free roam for a `/proc` probe
  (`--keep`, then internal evidence).

## Additions (2026-11-05, docs/notes/parity/drag-stage4-parity.md)

* **Clone shifts fixed**: the clone flow put a scripted `shift_up` / `shift_down` only into the
  held-key set, so the clone's player never shifted on a scripted key (drag_s1 / the drag shift sweep clone
  players had no manual shifts). The press now also goes through the game's key handler.
* the private tool `dragshift.py SCEN OUT --deltas … [--nitro T --nitro-hold S]`: a drag driven
  like a player with real X keys on the session's Xvfb (Up, Left Shift, Left Alt), shifting
  in lockstep at chosen rpm under each gear's up-shift point; screenshots at each press and
  14 frames after; `--replan OUT` writes the key frames into native.json for `parity.py
  clone`. The private tool `shiftgrades.py OUT…`: each player up-shift's shown rpm, q, light,
  formula grade, native grade, message, and the clone's.
* `[start] garage_from = SAVE` (savepatch.py): another save's garage and current car slot in
  the scenario's save (the finished-career save's Skyline in the stage-4 career of stage4_start.sav).
* Fields `dt_up6`, `dt_up7`, `heat` (engine +0x8C); clone accessors for `dt_up2..7`, `dt_e4`,
  `dt_f0`. Scenarios `drag_s4_h15_finished`, `drag_s4_h17_finished`, `drag_s4_d9_finished`.

## Additions (2026-11-05, docs/notes/parity/drag-stage4-parity.md §8)

* **Lane taps by race coordinate**: `dragshift.py --replan` writes the lane taps as
  `taps: [[coord, 6, key], …]` (the player's +0x8FC in the row before the clone's press) and
  `parity.py` as plan lines `tap COORD FRAMES KEY`: the clone presses the key on the first frame
  its player's race coordinate reaches COORD (in order, one a frame) and lets go FRAMES later.
  The frame-keyed replay left the plan's lanes after the crest; this one follows a native drag
  to the line (w4: player within 7.3 m all race). `PARITY_TAPS=frame` keys them by frame as before.
* `PARITY_EXTRA=N`: the clone runs N frames past the original's recording; the flow prints the
  race's results rows, the reward, the race result and the sponsor offers when the race ended.
* `PARITY_SPINMAX`: the cave's lockstep spin limit (default 150 M pause loops).
* `dragshift.py --continue-keys K1,K2,…` (after the race; Return waits 4 s, other keys 1 s) with a
  screenshot and a career read after each (bank, stage-record reputation and race count, the
  race-result object, the event's visit entry); a `kinds` log line (car +0x24, car +0x14 → +4)
  at frames 1 and 600.
* Field `rcoord` now has the clone accessor `rcoord` (`Progress::along`; the rivals' too).

## Additions (2026-11-05, docs/notes/outrun/pace.md)

* Outrun starts: `parity.py` writes `tick N`, the original's tick 0x865188 at frame 0 (the
  `clock` column), and `start_outrun` sets the clone's world tick to it before the challenge
  (the AI nitrous gap counts from +0x7AC = 0; the clone flow challenged ~4 s after the load, the
  original 19–50 s). `PARITY_NO_TICK=1` keeps the old start.
* Encounter cars now record `ai_target` (the controller's target speed) and `ai_goal` (5 chase,
  6 lead) in the clone.
* Scenarios `outrun_brake_p14` (the player braking from the challenge: the rival's pace alone)
  and `outrun_pace_p14` (the racer-traffic parity's throttle-held outrun with the rival's goal / target / catch-up /
  gas / rpm recorded).

## Additions (the grid stance, docs/evidence/grid-stance)

* Fields: the four suspensions' compression, rate and force (`comp_*`, `crate_*`, `sforce_*`: mover +0x28 + 4i →
  +0x1C / +0x20 / +0x24), the tyre forces and ground normal (`fx_*`, `fy_*`, `gnorm_*`: wheel +0x124 / +0x128 /
  +0x150), the sideways slip (`yslip_*`, car +0x3B8 + 8i), `load_fr`, the rest class `rest` (mover +0x60; in the start
  scene the NIS mover's placement count), `heading16` (car +0x40C); the rigid-body rows, ω and L now have clone
  accessors. In the start scene the controllers' fields must be skipped (`circuit_s1_scene_stance.toml`).
* The clone's rows carry each value's shortest round-trip f32 text (the exact bits), integers as integers.
* `[run] stance = true` or `PARITY_STANCE=1` (career events): the plan's `stance` line puts the original player's
  exact frame-0 state (native.rec: centre of mass, rows, velocity, ω, L, compressions and rates, rest class) on the
  clone's player at the arm.
* the private tool `stance.py RUN [CLONE.tsv…]`: the player's frame-0 stance per wheel side by side, the speeds 0.5–4 s
  after the green, the one-frame pushes and the contact frames.

## Additions (the full transplant, docs/notes/parity/prop-hits.md §7b)

* `[run] transplant_full = true` (or `PARITY_TRANSPLANT_FULL=1`) with `transplant = F`: instead of the
  pose (centre of mass, velocity, forward axis; spin zeroed) the clone takes the original player's exact
  state before frame F's update from `native.rec` (plan `stance_at F` + 41 numbers): the stance's 30
  (centre of mass, body rows, v, ω, L, the suspensions' compression and rate, the rest class), then the
  gear (car +0x434), the engine's and the input shaft's speed, the clutch, the four wheel spins, the
  gearbox's target gear (+0x38), the shown rpm (car +0x440: the automatic choice reads it) and the
  shift state (+0x44). Test: `plans_parse`.
* The free-roam roll (`[start] speed`, native.py) writes the gearbox's target gear (+0x38) with the
  gear: before, it stayed at the launch's 1st and the first shift after the roll went to 1st
  (0x5AA970 shifts toward +0x38; **traced**). Runs made with the older roll carry that shift.
* Clone rows at the transplant frame are recorded before the transplant (the state the clone had);
  from F + 1 on both sides start from the same state.
* Limits of the full transplant before the tyre state (measured): under full braking from 158 km/h the
  original's front-left wheel stayed locked while the clone's spun up from the first frame, and in a
  held-throttle left turn the clone's front wheels slid in the first frame where the original's gripped.
  Those runs held the brake from a standstill (it latches reverse, so the roll went off in reverse
  gear) and their held left turn was sent on one frame only (fixed below); from the new starts and
  with the tyre state the clone's wheels follow the original's (docs/notes/parity/cornering-parity.md). The
  straight-line and wall numbers above were made without the tyre state.

## Additions (the tyre and steering state, docs/notes/parity/cornering-parity.md)

* With `transplant_full`, `parity.py` also writes the plan line `tyres_at F` (23 numbers,
  `Plan::tyres_at`, `put_tyres`) when the run recorded the new fields: per wheel the lock flag
  (+0x10C) and the net torque's running mean (+0x120), the steering's angle, lock, counter-steer latch
  and last input (PlayerSteering = input +0x220: +0x7C, +0x80, +0x10, +0x84), the handbrake ramp
  (mover +0x70), the wheel hop (car +0x46C..+0x478), the rest damping (rigid body +0x90), the engine's
  power factor (+0x80), the clutch's last slip (+0x58) and the shift offsets (+0x78..+0x80).
  `PARITY_NO_TYRES=1` leaves it out. The engine's reaction (+0x7C) is not carried: the original's
  frame ends after the force pass its post-step takes for the next sub-step, so +0x7C already holds
  that pass's reaction, and the clone, put on that state, takes the same pass first and builds it
  from 0 (carrying it counted it twice; docs/notes/parity/cornering-parity.md §1, §6).
* The stance's suspensions go in one sub-step behind (`put_suspensions`: compression − rate × the last
  sub-step), as the clone keeps them; with the original's own +0x1C the first sub-step read a rate of
  about 0 (all earlier stance and full-transplant runs).
* The free-roam place keeps the driver's flags (Stability Control, car +0x464; the manual gearbox).
* A held left turn is re-sent every frame (`native.py`: held by the press, not by the value 0x00).
* Fields `lock_*`, `ntq_*`, `torq_*`, `steer_drift`, `steer_avg`, `steer_rate`, `steer_lim`,
  `steer_last`, `hb_ramp`, `hop_t`, `hop_calm`, `rb_damp`; scenarios `corner_*`.
* Since the clone's sub-step ends with its force pass as the original's does (docs/formats.md, **Order
  of a frame and a sub-step**), a frame's recorded tyre forces, slips, loads, torques and the
  drivetrain's input shaft are taken at the same point in both games. The suspension compressions
  are still put one sub-step behind at a transplant (`put_suspensions`): the transplanted car has
  no stored force pass and takes one on the state it was put on.

## Additions (the contact log for every pair, docs/notes/parity/prop-hits.md §7d)

* `PARITY_CONTACT_ALL=1` now also logs every contact (not only those with a prop) with the player's
  rigid body before 0x5AF0F0 (record `contact`, column `rb`: the body's first 64 words; centre of mass
  +0x20, momentum +0x70, angular momentum +0x80, velocity +0xA0, ω +0xC0), and hooks 0x5B2620's call of
  0x5AF0F0 at 0x5B2794 to log the same body after the impulse (record `after`; momentum and angular
  momentum are updated there, velocity and ω later).
* `PARITY_CONTACT_EXACT=1`: every float of the contact log as its shortest round-trip text (replays near
  the query's 1e-5 squared-gap boundary need the exact bits); default 5 decimals.
* Measured: with the contact log installed the 200 km/h barrel scenario's native rows came out one frame
  shifted in one run; compare such runs only against their own clone side.

## Additions (the query's entry state, docs/notes/parity/prop-hits.md §7e)

* The contact log's query record (now 0x200 bytes, at most 30 000 records) also holds both bodies'
  z rows (+0x140 / +0x1C0) and the pair's state before the query: kept sets, counts and warm flag
  (+0xF0..+0x118) and the dirty flag (+0x220), printed as `z A … B …`, `entry A … B … warm n dirty n`
  and `d …` (the distance's exact float with `PARITY_CONTACT_EXACT=1`).
* `devprobes` `query_replay TRACK CAR native-contacts.tsv… [-v]` (maintainer only, like the developer probes): every logged query of a hull or
  track-polygon pair that the pair measured, run through the clone's query from its entry state on
  the logged transforms; counts per pair kind how many give the original's distance, closest points
  and kept sets to the last bit (`-v` lists the others with their bits).
