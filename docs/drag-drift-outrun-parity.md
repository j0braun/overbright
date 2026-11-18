# Drag, drift and outrun parity (2026-11-04)

**Verified** = traced in SPEED2.EXE, **observed** = read under Wine,
**inferred** / **open** as marked. Evidence: the maintainer's private captures.

## 0. Outrun variety (the same rival car appearing often)

### Verdict

The clone repeated more than the original: **every career free-roam load
drew the same three encounter cars** until something else (a race's
line-up) moved the random numbers. Fixed to native. The original itself
keeps one set-up's three cars for the whole free-roam stay (every outrun
and every re-placement uses the same three slots), so an opt-in
Enhancements row **Outrun Variety** (Off = native) now redraws a slot's car
once it has been out and is parked again.

| free-roam loads in a row (garage jump and back), a community save (EVO, stage 4) | line-ups |
|---|---|
| original under Wine (observed) | 9 loads: 7 of 8 changed the line-up; 16 different models in 27 slots (RX8 IS300 PEUGOT / SENTRA CORSA IS300 / PEUGOT106 240SX CELICA / same again / GTO A3 PEUGOT / A3 WRX COROLLA / GOLF IS300 MIATA / 350Z WRX PEUGOT / SENTRA SUPRA SUPRA) |
| clone before (`Encounters::new` on a copy of the random state) | 6 of 6 the same: ECLIPSE, 3000GT, CORSA |
| clone now | 10 of 10 different (20 models in 30 slots), e.g. COROLLA ECLIPSE IS300 / IS300 240SX TIBURON / MUSTANGGT COROLLA 350Z / CORSA CELICA 240SX / … |

### The pick (verified)

* 0x540790 (career free-roam set-up, every free-roam load: from the
  garage, a shop, a race) → 0x53F010(3): 8 names (0x4FE970), then per
  racer i < 3 into the record 0x85EF80 + 0x8F0·i: 0x4FEAF0(record, 100 + i,
  2 + i, 1, 0, 0, −1.0), stock parts 0x6371A0, a random paint 0x6371F0,
  the level-2 neon 0x6371B0(0x9A, 2), the skill row 0x4FED00(0x89E7F4);
  then 0x532020(0) matches each to the player's performance and dresses it.
* 0x4FEAF0: debug table 0x7FAA78.. (−1, none); the pool 0x7F6DC0 (26 car
  types; 0x570A40 always true) or the SUVs 0x7F6DDC when the player's car
  record +0x88A is set; shuffled (each place swapped with place
  `Rand_Int(26)`, 0x43C1C0 on 0x7EA624); then, with a margin from 0.06
  doubling each pass, the **first** car whose scaled rating (0x5B69A0(1,
  −1) with ECX = record +0x10, after 0x610280 put the candidate in) is at
  most the player's (0x5B69A0(1, −1) with ECX = racer info 0 +0x20,
  0x5ED910(0) + 0x10 + 0x10; param 7 < 0) plus the margin
  (0x4FEC50..0x4FECCC). Param 4 = 1: the player's own type allowed.
  **No "not the same as the last" rule**, no exclusion between the three
  slots (duplicates possible), no per-encounter redraw: the records are
  written only here.
* So the variety depends on the player's rating: a community save's EVO reads 585.26
  under Wine (racer info 0 +0x20 → +0x08), scaled 9.39 (bounds
  144.12–613.90); the clone computes the same 585.3 / 9.39. Every pool car
  is rated ≤ 4.03, so all 26 qualify and the first of the shuffle is
  taken (clone, 30 000 draws: 859–1554 per car; the swap shuffle is
  biased, as the original's). A stock stage-1 Peugeot (1.12) allows only
  the 7 pool cars rated ≤ 1.18 (PEUGOT, FOCUS, COROLLA, 240SX, MIATA,
  PEUGOT106, CORSA; **inferred** from the rule, matching the stage-2
  slots PEUGOT / CORSA / FOCUS in internal notes and the outrun-strength study's
  the finished-career save draws COROLLA / CORSA / PEUGOT), so early careers repeat more,
  natively.

### Under Wine (observed)

a private Wine copy, display :226, DXVK, vanilla (no widescreen fix), the
save `a community save-latest`: garage → Return to Explore Mode → read the three
records' +0x10 (car type, names from the car table 0x8A1CCC + 0x890·n) →
pause, Jump to Garage, repeat (`native-loads.txt`). Player car 28 (EVO),
stage 0x862838 = 4, difficulty 0x83AC30 = 1. Within a stay the records do
not change (each read again in the garage after free roam).
One load (the first of the second loop, `native-loads.txt`) kept the
previous line-up although the random state had moved; the cause is
**open**. One line-up held SUPRA twice (no exclusion between slots,
as traced). The session hung on a loading screen after 9 loads (Wine),
so the loop stopped there.

### Clone fault and fix

`Game::explore` drew the line-up on a copy of the game's random state and
left the state as it was ("the draws take a copy … and leave it as it
was"), and free roam itself draws nothing from it, so a garage jump, a shop
visit or the map reload drew the identical three cars (and only a race's
line-up moved it). Now the copy is taken over after the set-up
(`src/bin/game.rs`, `explore`): the set-up's draws move the state on, as
0x4FEAF0's `Rand_Int` calls do on 0x7EA624. `EncountersAhead` still
predicts from the state before. The draw itself was already the native
rule; it is now `EncounterDraw::pick` / `EncounterDraw::dress` (one racer),
shared by the set-up and Outrun Variety. Test:
`encounter_variety::free_roam_loads_in_a_row_draw_new_encounter_cars`
(asset; fails on the old code: 4 loads, one line-up).

### Outrun Variety (Enhancements, clone deviation, opt-in)

Display → Enhancements → Outrun Variety, Off (native) / On;
`enh.outrun_variety 1` in options.txt; kept by the graphics presets.
On: a racer that has been on the road and is parked again (0x40BEE0 far
park, 0x40BF60 after an outrun, the manager's reset) gets a new car for
its slot: `EncounterDraw::pick` against the same player rating and pool
(so the same class / performance band as native, never stronger or
weaker), a car type not out in the other slots (up to 8 redraws), a
random paint and neon, a skill row from the same difficulty row, then
the native match to the player's performance and dress (vinyls, body
parts by those levels). It draws on the racers' own random state, not the
game's; it loads on a thread and is swapped in only while the slot is
still parked and no outrun runs; the slot's stamps are kept, so placement
pacing (one car per 5 s) is native. The racing-line class (0x5DDFD0) stays
the set-up's. The new car's renderer is built on its first draw (a small
hitch possible; opt-in). Tests:
`encounter_variety::outrun_variety_redraws_a_parked_racer` (asset),
`qol::tests::outrun_variety_round_trip_and_native_default`,
`options::tests::outrun_variety_row_toggles_and_restores`.

### Open

* The skill-row byte of the record was not identified under Wine (+0x901
  read 00 / 00 / 01 in every load, so likely not the row); the clone's row
  draw (0x4FED00) is per the trace.
* Why one native free-roam load kept the previous line-up (above) was not
  traced. A fresh stage-1 save was not run under Wine; its 7-car pool is
  from the rule (see above).

### The firstrace flow after the RNG fix (2026-11-04)

With the set-up's draws taken over, `--flow firstrace` failed: "first race
uncapped: rival 1 turned 20.1°, rival 2 22.5°" (limit 16°).
* **Which draw.** The state hash (`NFSU2_STATE_HASH`, hashdiff.py) differs
  from step 0 (the game's random state after the free-roam set-up), so every
  later race of the flow draws another line-up. The failing race (the third
  fresh first race) drew COROLLA / 240SX / 240SX with the player in slot 1
  (old code: FOCUS / 240SX / MIATA, player in slot 0) and other per-car
  seeds. The same race at 60 Hz (`NFSU2_FIRSTRACE_HZ=60,144,60`) gives the
  same 20.1° / 22.5°: a seed matter, not the display rate.
* **Native order** (verified): the free-roam set-up 0x540790 → 0x53F010
  draws from 0x7EA624 (0x4FE970 names, 0x4FEAF0 shuffles, 0x6371F0,
  0x4FED00) before a race's start draws its line-up (0x5260C0 → 0x4FEAF0,
  0x4FEDE0 grid swap) from the same state, so consuming the encounter draws
  first is the native order. The clone's sequence is not the original's
  (other draws between), so no seed can be matched one to one.
* **Not new.** A sweep of 12 fresh first races + restarts each (60 Hz)
  gives 2 of 72 rival runs over 16° with the old code (max 17.8°) and 2 of
  72 with the new (max 22.5°): the same rate; the old code fails its own
  flow on other seeds. The turns are three rivals in a pack 2.9–4 m apart,
  all in AvoidAvoidable from 2.3 s, steering ramping to ±16–20° at 5.4–6.1 s
  (`NFSU2_FIRSTRACE_LOG=1`, now with the action and the line-up). Whether the
  original's pack does the same could not be checked per seed; a harness run
  of the stage-0 race needs a native save with it open (none in the
  fixtures) — **open**.
* firstdrag (6 runs, 0 failures), postrace and the playthrough (171 races,
  ending seen) passed with the change on the old base.
* **Rebased on main (2026-11-05: the test isolation, the tuning-physics parity's snapshot port and
  centre-of-mass push-out, d9ae78c + working tree)**: `--flow firstrace`
  passes with the RNG change (worst 13.8°: 9.4 / 8.2 / 13.6, 11.0 / 8.2 /
  7.2, 13.0 / 8.3 / 11.0, 7.0 / 5.7 / 11.1, 8.1 / 6.3 / 12.1, 7.6 / 8.0 /
  13.8 across the six runs). The pack-swerve seeds may still exist (the
  threshold stays seed-sensitive, see above); the Outrun Variety test holds
  `test_isolation::exclusive()` (it sets the global).
* The drift and outrun harness runs below were made on the old base
  (before the tuning-physics parity's snapshot port); their physics-side differences are to be
  re-measured on main.

## 2. Drift (harness `drift_s1`, STAGE_1_DRIFT_1, track 4311)

New harness fields (the maintainer's `fields.toml`, `[run] drift = true`): the
DRIFT trick's state / points / banked / multiplier (racer +0x1D4 → [+0]
+0x14 / +0x24 / +0x2C / +0x1C), the style total (+0x9C), the drift
manager's level (+0x1D8 → +0x1C), its off-road and proximity factors
(+0x18 / +0x3C), the yaw rate (car +0x118). State values: 0 running,
1 pending, 2 idle, 3 banked, 4 lost (0x56EAC0: a continuous trick in state
2 goes to 0x56D510(1); **verified**). The cave now follows up to 5
pointers per field and treats pointers past 0x7FFF0000 as null (a parked
encounter car's racer read 0xAAAAAAAA and faulted the game).

60 s from the green, same cars and grid (PEUGOT player; MIATA, PEUGOT,
MIATA), throttle with handbrake flicks:

| racer | original: total at 10/20/30/40/50/60 s, banks, top level | clone |
|---|---|---|
| player | 0 throughout, 0 banks, level 2 | 0/0/88/992/992/992, 2 banks, level 3 |
| MIATA #1 | 308/308/675/1095/1095/1095, 5 banks, 2 | 868/868/1036/1507/1619/1619, 4, 3 |
| PEUGOT #2 | 737/4551/7751/12106/12106/13813, 6 banks, 4 | 744/4815/8528/15049/21155/21155, 6, 4 |
| MIATA #3 | 463/725/1339/1962/1962/1962, 9 banks, 2 | 0/450/1095/1233/1233/1233, 3, 2 |

The cars part within seconds (physics and AI paths; position error 2–30 m
by 10–20 s), so the totals compare the scoring's scale, not a frame match:
the same order (rival #2 far ahead), the same top levels for the rivals.
**Scoring replay** (the maintainer's probe `drift_replay`): the clone's `DriftManager` +
`StyleManager` fed rival #2's *native* kinematics (speed, velocity, forward
axis, yaw rate) reproduce the original's DRIFT trick frame by frame with
the AI factor 1.25 (0x79DAF0[1]) and class 3, one frame later than the
record (the record is the state before the update): pending at 5.2° slip,
running past the threshold, the first bank 728 vs native 737, the level
1 → 2 on the same frame. From 15.2 s the original's points grew up to 2× faster than that first
replay's: the off-road factor. A second run (`drift_s1b`, with
`drift_surf` / `drift_near`) shows the drift manager's +0x18 between 1 and
4 for every racer (431–1031 frames each) and the proximity factor +0x3C
always 1 (no traffic). With the off-road wheel count taken from +0x18
(count × min(1, speed / (0.25 top)), 0x56CA90) the replay reproduces every
bank of two rivals: rival 1 321 / 1238 / 4247 / 5800 / 8068 / 10771 / 14077
vs replay 329 / 1262 / 4281 / 5854 / 8148 / 10858 / 14168, rival 3 281 /
577 / 4008 / 4445 / 6539 / 8327 vs 287 / 584 / 3995 / 4420 / 6524 / 8352,
with the same levels on the same frames (one frame earlier). So the
clone's scoring rules (rate, multiplier levels, thresholds, banking, the
AI factor) match the original's within 0.3–2.6 % per bank. **Open**: that
small excess (the first banks 2–3 % high: the rate's rounding or the
slip's source, 0x59FA20, not compared), whether the clone's surface flags
put the same wheels off the road (the totals of a run still differ because
the cars' paths differ), AI drift lines, and the time-out rules (not
exercised: no wrong way).

## 3. Outrun over a whole run (harness `outrun_stand`, the finished-career save)

the finished-career save (Skyline GT-R), the first encounter car out challenged
from 10 m behind (the harness's outrun start), then the throttle released
and the brake held (the player stops and rolls backwards), so the rival
drives away until the 300 m rule ends it. Both cars from the original's
frame-0 pose; the clone's encounter car forced to the original's model
(`parity.py` now sets `NFSU2_ENCOUNTER_CAR=<model>:-` from `pose.txt`;
the skill row stays the clone's draw). New field `behind` (racer +0x24,
0x5FC520 less the first's at 0x607DBB). Rival 240SX:

| t (s) | 0 | 1 | 2 | 3 | 4 | 5 | 6 | end |
|---|---|---|---|---|---|---|---|---|
| rival speed, original (m/s) | 18.26 | 23.91 | 29.67 | 34.75 | 38.53 | 42.02 | 45.73 | |
| rival speed, clone | 18.26 | 24.15 | 29.68 | 34.45 | 37.97 | 41.00 | 44.21 | |
| player behind, original (m) | 10.6 | 22.7 | 53.6 | 98.8 | 156.1 | 218.6 | 284.3 | ≈ 6.24 s (300 m, extrapolated; over by the first read after the record: rival status 6, race type 0 0.07 s later) |
| player behind, clone | 10.7 | 21.8 | 51.6 | 94.4 | 148.7 | 209.6 | 273.4 | 6.42 s (299.6 m at the last racing frame) |

* Win / loss: the same rule ends both (racer +0x24 past 300 m → status 7,
  the first status 6, race type 7 → 0), the same order of events; the
  clone ends 0.18 s later because its rival is slower (below).
* Gap over time: within 4 % throughout; the clone's rival matches the
  original's speed to 2.5 s (within 0.3 m/s) and then falls behind by up to
  1.5 m/s (3.3 %) at 6 s, in the same gears with the same 3 → 4 shift
  (3.5 s) and rpm 2–3 % lower: a drivetrain matter for the physics unit
, not the outrun AI. The player rolling back reads −25 m/s in the
  clone's signed speed and +25 in the original's |v| (+0x430), the same
  motion.
* Rewards: the loss's bank change (0x541330: the stage's +0x02 off the
  bank, at least 0) was not read: the harness's `cash` field (0x863E74) is
  not the bank in a race (open). **Open.**
* Harness fixes made for this run: recording past the outrun's end
  faulted the game (the racers are dropped, 0x5F7960, and car +0x1C goes
  stale: 0xAAAAAAAA / 0x00FFFFxx were read), so an outrun records the two
  racers only, for `seconds`, and `[run] tail` then polls race type,
  clock, the rival's +0x24 and status from Python until race type 0
  (`tail.tsv`); `[run] skip` leaves fields out; the cave treats pointers
  past 0x7FFF0000 as null.
* **Open**: a chasing player (closed-loop bot under the harness's lockstep,
  as the outrun-strength study's lead runs) for catch-up over a long outrun; the rival's AI fields
  (target, action) are not exported for encounter cars by the clone's
  harness view.

## Drag (item 1)

Not done: the lane AI, shifts, heat / blow, the drag target 1000 and the
green release wait for the tuning-physics parity's launch physics (docs/race-ai-parity.md, Open A
/ C).
