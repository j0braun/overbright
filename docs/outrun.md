# Outrun: the encounter racers end to end (traced + live comparison)

One description of the career's random outruns. The career/manager/HUD
side is in `formats.md` "Outrun"; this file covers what the rival does and
indexes the detailed notes. **Verified** = traced in SPEED2.EXE (REA/Ghidra
or `speed2.dis`); **observed** = seen under Wine (2026-11-04, vanilla,
fastgpu/DXVK, the finished-career save, `docs/evidence/outrun-live/`);
**inferred** / **open** as marked.

## Life of an encounter car

1. **Set-up and placement** (formats.md "Outrun"): 0x53F010(3) builds three
   racers (ids 100–102, owner kind car +0x24 = 3). The manager (race status
   +0xC40) in TRYING_TO_ENGAGE (3) places a parked car 250–500 m ahead on the
   player's road (0x42A5E0 → 0x5F6D60).
2. **Mover on placement** (verified, observed): 0x5F6D60 calls
   0x5F46B0(0x5F4410(car)) (0x5F6DCE). For owner kind 3, no contact hold
   (controller vt+0x50 = 0x41E510: +0x734 running) and no drag/drift
   (0x401950) 0x5F4410 returns **3, the MeloMvr**. Under Wine every
   encounter car was mode 3 whenever it was not on the belt (car +0x28).
   *Clone before*: physics mover (1). *Now*: `Encounters::spawn` puts it
   on the MeloMvr and gives it its tank (the car's, 0x59F9DA; the clone's
   tank was filled lazily by the physics step only). The tank is **kept**
   across parking and placements (observed: one 350Z's tank went 24000 →
   21841 → 18431 → 12357 → 2948 → 915 ticks over five outruns; with 915
   left it fired no nitrous); 0x5F6C8C refills only on a size change.
3. **Before a challenge: goal 9** (AIGoalOffsetPursuit, vtable 0x784BF8,
   0x42A860 → 0x428630), action type 19 (observed). Its slot +0x4C
   (0x40DC00) caps the driver's speed: with no race coordinates (0x88F2BE
   clear) max(table(distance to the player) × player speed (+0x430), floor);
   table 0x81E378 → points 0x7E9BC8 {0 m 1.1, 20 m 1.0, 50 m 0.8, 100 m 0.5,
   150 m 0.1} (0x5767A0 interpolates, holds the ends); floor 30 m/s, for an
   AI car (owner class 2) 26.822 (60 mph) within 100 m and 17.881 (40 mph)
   beyond (verified; **observed**: target exactly 17.881 far and 26.822 near
   a standing player; rival 16.1 m/s on the belt = 0.899 × 17.88). *Clone
   before*: the race rivals' speed × 0.75 (~25 m/s, 57 mph), so the cars
   already cruised near race pace and the challenge changed little; *now*
   `outrun::pursuit_cap` is applied in state 3. The goal's offsets
   (verified, 0x42A860 per slot, slot +0x00 the mode): 0x41C100(player)
   gives a longitudinal a and lateral c; mode 1 with |a| ≥ 30 → mode 1,
   offset 0; mode 2/3 with |a| ≥ 20 → mode 1, offset 0; otherwise mode 1
   picks the side of c (c ≥ 0 → mode 3, +5; else mode 2, −5), mode 3 keeps
   +5 until c < −2 (→ mode 2), mode 2 keeps −5 until c > 2 (→ mode 3);
   then 0x428630(player car, 10, offset, 7 − age s) writes goal +0x18 / +0x1C
   / +0x20 and, for the current action of type 19, action +0x210 (target),
   +0x214 (10) and +0x218 (offset), then 0x428490. How action 19 turns
   +0x214/+0x218 into its path (its users around 0x4237xx–0x4240xx) is
   **open**, so the clone still drives the route stand-in (inferred).
4. **Belt** (BeltMover, mode 4): the full think's gate 0x402180 (from
   0x42B26C, only when car +0x5CC is set, "loaded"): ≥ 3 path points,
   timers +0x730/+0x734 idle, forward speed ≥ 5, a wheel down, action not
   8/9/10/11/13, skill ≠ −2, ≥ 120 m from the human (0x5F1170), and the race
   clock **race status +0x14** × 0.00025 ≥ 30 s (verified). +0x14 runs in free
   roam too and is reset by the challenge (observed: 328.5 s → 0.0), so
   before a challenge far cars are on the belt; in the race not before 30 s
   (observed twice: mode 4 at race clock 30.07 / 30.09 s). Once, mode 4 came
   at 14.2 s (loss1, +0x5CC not logged): consistent with the racer controller's
   world notice (vtable 0x785370 slot +0x10 = 0x4022D0, formats.md "Racer
   movers": unloaded → mode 4) — **inferred** for this run, +0x5CC was not
   logged then (it was set throughout loss2 / lead1).
   **Observed** (matched loss, 2026-11-04 round 2): mode 4 at race clock
   10.19 s, the very sample car +0x5CC cleared, 114 m away — the world
   notice. *Clone now*: the full think's gate also runs before a
   challenge (goal-9 cars, action type 19, the class-2 cadence; race
   clock = time since the last challenge, from the free-roam load before
   any: **inferred**) — far cars cruise at 0.899 × 17.881 = 16.075 m/s on
   the belt as in the original; and the world notice (`encounter_world_notice`)
   puts a streamed-out car on the belt, a mode-4 car's belt being rebuilt
   at each full think while unloaded (0x42B2A9). The clone's streaming
   keeps more sections than the original (the matched loss never
   unloaded there), so when the notice fires differs (**open**).
5. **The challenge** (0x607190): goal 6 for the race order's first (the
   rival), action 6 at once (observed); the race clock restarts; its first
   full think requests nitrous (observed: +0x7A8 stamped 0–0.24 s after the
   challenge in all three runs, shots 1.4–2.2 s, then again every ~11 s).
   Goal-5/6 speed: `SpeedTarget::update_outrun` (place 1 × 0.84, place 2 ×
   1.0, see internal notes). Goal-5 search, junction
   cache, scheduling: `outrun-think-scheduling.md`,
   `outrun-action-lifecycle.md`, `outrun-cache-capacity.md`,
   `outrun-controller-integration.md`.
6. **Nitrous on the MeloMvr** (verified): the chassis' torque reads its
   own stage Melov8r +0x198 (0x598B62: + curve(+0x180) × handling +0x2AC ×
   0x7A17A0[stage]), but +0x198 has no writer besides the constructors
   (0x5A8199, 0x5B01D2): a shot on mode 3 adds no torque. The shot's time is
   the clock less its start (0x592490); the clone now advances a running
   shot on the MeloMvr as well (before it never ran out there).
7. **Unchallenged timeout** (0x40BEE0, observed, `native-idle-timeouts.txt`):
   with the player standing, cars placed over 30 s ago and ≥ 250 m away
   (576, 739 m) were parked and a car re-placed 268–283 m away in the same
   frame; mode 3 ↔ 4 at ~120 m from the player before any challenge.
8. **The end**: 300 m (304.9 in feet) behind the leader, formats.md
   "Outrun"; observed losses at 20.96, 32.2 and 35.8 s with the player
   stopped.

## Live comparison (docs/evidence/outrun-live)

Same driving in both: the player put 10 m behind a free-roam encounter car
on its travel at its speed with the throttle held (`engage.py`; the clone's
`--flow outrun`), Enter Race when the career offers it (+0x1AE8 = 2), then
the handbrake held. The native rival (unknown car, a random road) and the
clone's (its own draw and road) differ, so speeds are compared as ranges.

| | native (loss1 / loss2 / lead1) | clone before | clone now |
|---|---|---|---|
| before, far | goal 9, mode 4, target 17.88, 16.1 m/s | 25–27 m/s, mode 1 | 17.9 m/s, mode 3 |
| goal at challenge | 9 → 6, action 6 | 9 → 6, action 6 | same |
| mode in race | 3; 4 from race clock 30 s | 1 | 3 (4 per the gate) |
| first nitrous | 0–0.24 s | 6.15 s (a 4 s-old load; no tank on mode 3 later) | 0 s |
| speed 1 / 3 / 5 s | 26–31 / 25–37 / 27–37 m/s | 33 / 37 / 40 | 23 / 32 / 39 |
| shot length | 2.2 / 1.4 / 0.7 s | 2.1 s | 2.1 s (before: never ran out on mode 3) |
| outcome | lost, 21.0 / 32.2 / 35.8 s | lost, 10.8 s | lost, 9.7 s |

### Round 2: like for like (same car, skill row, road, start, tank)

The native rival was a 350Z (car +0xC → "350Z"), skill row 0 (owner
+0x901, read by controller vt+0x78 = 0x402580 → 0x610520). The clone's
`--flow outrun` takes `NFSU2_ENCOUNTER_CAR=350Z:0` (check aid: every
encounter racer that type and row; performance still matched to the
player by 0x5B7580), `NFSU2_FLOW_POSE` (both cars' position and velocity at
the native challenge, plus the rival's tank ticks) and, for
`NFSU2_FLOW_OUTRUN=bot`, `NFSU2_FLOW_PATH` (the road path the native bot
drove, from `devprobes/src/bin/outrun_path` = `outrun::road_path`, 0x415130 walks)
with the same keyboard bot (`bot.py` / `OutrunBot`) and the same lead
intervention (`NFSU2_FLOW_LEAD_AT=3`, `LEAD_M=40`: the player put on the
rival's driver path ≥ 40 m ahead at the rival's speed; native and clone
put it within 1 m of each other, (−331.95, 274.49) vs (−332.88, 274.42)).

| matched lead swap (`*-matched-leadswap.txt`) | native | clone |
|---|---|---|
| rival speed 0 / 1 / 2 / 3 s | 19.0 / 25.5 / 31.5 / 36.1 | 19.1 / 25.0 / 30.8 / 35.1 |
| rival speed 4.3 / 5.4 / 6.6 s | 41.2 / 45.3 / 49.4 | ~40.0 / ~43.6 / ~47.2 (−3 … −4.5 %) |
| nitrous (tank 915 ticks) | none | none |
| goal 6 → 5 (player leads) | 5.56 s, 20.8 m apart | 6.95 s, 13.1 m apart |
| goal 5 actions | 0, 1, 11 (contact, mode 1) | 1 (contact) |
| goal 5 → 6 (rival retakes) | 8.06 s | 10.25 s |
| outcome | lost, 16.0 s | lost, 21.4 s |

| matched loss, player braking (`*-matched-loss.txt`) | native | clone |
|---|---|---|
| nitrous | 0.22 s (0.54 s shot) | 0.77 s (0.5 s), again 11.5 s |
| rival speed 1.8 / 2.8 / 3.8 / 4.8 s | ~27.0 / 32.4 / 33.9 / 31.3 | 27.8 / 32.5 / 31.4 / 29.6 |
| slowest in the bend | 27.9 (target 27.95) | 22.0 (target 22.8) |
| mode 4 | 10.19 s (unloaded) | none (stayed loaded) |
| outcome | lost, 15.3 s | lost, 12.3 s |

Findings: on the same car and road the rival's acceleration on the
MeloMvr is within 5 % for the first 6 s; the remaining divergences are
the route's racing speed in that bend (clone target 18 % lower), the
lead swap's distance (the leader's road node: the clone's stand-in, the
route walk's current point, is nearer the car than the controller's
+0x0C node — **inferred** from the 20.8 vs 13.1 m), and the clone's wider
streaming. A **win was not obtainable** with the keyboard bot: on the
bot's own run (`native-bot-chase.txt`) the rival reached 64 m/s while the
player's Evo, braking for bends, held ~44; with a 40 m head start the
rival retook the maintainer within 2.5 s in both games.

Remaining **open**: a win; action 19's offset geometry; the leader's road
node for review rule; the route's bend speeds against the native ones;
streaming radius vs the world notice; the other encounter cars at a
challenge (native: they become racers too — goal 5, action 6, a nitrous
shot — until their 2 s hold parks them; ported: goal 5 on the
leader every manager update (traced); that they stand behind the
player and the challenged car in the race order is **inferred** from
the observation under Wine, the clone's order holds those two only).

## Pace like for like (2026-11-05)

`outrun-pace.md`: the MeloMvr's drag took no throttle term (2·c instead of (2 − gas)·c, 0x5AAFD2 →
0x59E630), so the clone's rivals accelerated 3–15 % slower above ~30 m/s; fixed, together with the
MeloMvr's catch-up k (0x5A3390 → chassis +0x13C) and the inverted AI nitrous band (0x402590). From
nine native starts with the player braking the rival's speed at 1 / 3 / 5 s now matches the
original's within 0.3 m/s; the speed target (0x41C5E0 race state 7) was already native.

## Strength on the player's save (2026-11-04)

`outrun-ai-strength.md`: the encounter racing lines now use the upgraded
racers' class (0x5DDFD0; the original's 0x88E994 = 0, the clone used the
stock class 3: targets 30 % low), junction-gap path points keep the lateral
(0x415D50 → 0x4122C0), and the open goal-5 suspension at record ends.

## Stages 4 and 5: what a win gives (2026-11-05)

**Traced** (Ghidra): the end 0x541330 (game mode 1): a win adds the stage definition's +0x02
(i16) to the bank (career +0x7134), a loss takes it off (at least 0); a win counts in the
stage record's +0xDC; then 0x529BE0 (SMS_OUTRUN_VICTORY, condition 5, on a win /
SMS_OUTRUN_DEFEAT, condition 6, on a loss — each only when the inbox does not hold it yet,
0x514CB0), 0x533AA0 (on a win, every condition-2 record of the stage whose count equals
+0xDC: its message by 0x5338D0 with the generic labels, kind 1 SMS_VISUAL_GENERIC_n + the
slot's label, kind 2 SMS_PERF_GENERIC_n, n = counter 0x86420C % 5 or % 3 + 1, which then
steps; the world challenge is on offer from then on), 0x5110C0 (Rachel's line), 0x533320
(markers / open events), 0x4F92A0 (a HUD object freed). Nothing else: no cash besides the
bank, no part, no DVD offer (the DVD covers come from 0x5331D0 at the city start). The
unique parts are the world challenges' rewards (STAGE_4_VISUAL_3 at 4 outrun wins,
VISUAL_4 at 6, PERF_2 at 9; STAGE_5_PERF_3 at 6, VISUAL_5 at 11).

Per stage (stage definitions 0x34A18, read by the clone): bank 100 / 100 / 100 / 100 / 250 /
400 for stages 0–5, the cap on a stage's outrun wins (+0x40, 0x513D30) 3 / 5 / 10 / 15 / 20 /
20 (the player's a community save holds exactly 3, 5, 10, 15; the finished-career save's stage-4 record 20); none past 5.
**Opponent pick and difficulty have no per-stage table**: 0x53F010 draws each encounter car
against the player's car rating (0x4FEAF0, −1 → 0x5B69A0) from the pool, and its skill row by
0x4FED00 from the career difficulty (0x89E7F4 = 0x83AC30, copied at every city start). Under
Wine (Medium, 1): the challenged rivals were an A3 with row 4 (stage 4, a community save) and a CORSA
with row 3 (stage 5) — within Medium's rows; this is consistent, not a distribution check.

**Observed under Wine** (`docs/evidence/outrun-s45/`; Wine copy `a private Wine copy`, wined3d;
`outrun_win.py`: the sandbox save's +0xDC set, engaged as outrun.py does, the player put 15 m
ahead until its racer +0x24 = 0 (it leads), then the rival lifted 400 m: status 7, the player
wins at once):

| | stage 4 (user's a community save copy, +0xDC 3) | stage 5 (`stage5_start`, +0xDC 5) |
|---|---|---|
| bank | 1,000,007,600 → +250 | 148,300 → 148,700 (+400) |
| +0xDC | 3 → 4 | 5 → 6 |
| new messages | 1: STAGE_4_VISUAL_3's, body SMS_VISUAL_GENERIC_1_BODY, argument 0xBB034EA6 (Spoilers), subject SMS_VISUAL_GENERIC_1_SUBJECT ("Unique Spoilers"); SMS_OUTRUN_VICTORY already in the inbox | 3: SMS_OUTRUN_INTRO (at the engage, this save's first), SMS_OUTRUN_VICTORY, STAGE_5_PERF_3's with SMS_PERF_GENERIC_1_BODY / _SUBJECT ("Unique Performance Upgrade") |
| counter 0x86420C | 0 → 1 | 0 → 1 |
| then | the mailbox opened at once over free roam (the trigger message is urgent) | same |

The clone gives the same for both saves (`tests/playthrough_late.rs`
`outrun_wins_at_stages_4_and_5_give_what_the_original_gave`); no code change was needed.

## Detailed notes
`outrun-think-scheduling.md` (42ABD0 gates), `outrun-action-lifecycle.md`
(goal-5 actions), `outrun-cache-capacity.md` (16-slot junction cache),
`outrun-controller-integration.md` (persistent controller),
`outrun-loaded-belt.md` (race-7 belt gate), `outrun-recovery-*.md`
(recovery, outrun speed branch), `outrun-*-acceleration.md` (acceleration
inputs), `outrun-speech-selection.md` (Rachel's lines).

## Aside: the false WRONG WAY in `wrong_way_circuits_and_sprints`

Not in the AI/route code this work touched. On <base> (fixed-60 cadence)
the false frame is Circuit 4005 variant **0** at race clock 53.50 s (variant
1 at 167.4 s no longer fails): the player's car on the autopilot runs
correctly (heading 0.336 rad, velocity (46.8, 16.2) m/s, on the
autopilot's road node record 79 point 3), but the race's progress locator
(`race.me.locator`) has snapped to another road 8.3 m away, point
(69.14, 317.71), direction (−0.353, −0.935) — ~129° off the car's travel —
so `wrong_way` fires. **Resolved (2026-11-04)**: not a crossing road
but route segment 12's own spline loop (three nodes 10 m apart, phantom
points from 50–80 m neighbours, 0x5E48C0 → 0x5D50A0); the original's
locator gives the same way there under Wine — native behaviour, see
internal notes item 1.
