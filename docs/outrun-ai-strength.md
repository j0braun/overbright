# Outrun opponents' strength (2026-11-04)

Issue: the outrun racers were too easy and crashed easily (checked with the
finished-career save). Measured on that save
(`<the play setup's options file>`, career car Skyline GT-R) in the
original under Wine (fastgpu, a private Wine copy, display :194,
vanilla, no widescreen fix) and in the clone (`--flow outrun`,
`NFSU2_FLOW_SAVE`). Evidence: the maintainer's private captures.
**Verified** = traced in SPEED2.EXE; **observed** = read under Wine;
**inferred** / **open** as marked.

## Verdict

Not native before this change. Two clone faults made the rivals slow and made
them miss junction turns; both are fixed. A third (rivals stopping for good
after some lead changes) is fixed by the menus follow-up (§3).

| | original | clone before | clone now |
|---|---|---|---|
| route class for the encounter lines (0x88E994) | **0** (observed) | 3 (stock COROLLA/CORSA/PEUGOT) | 0 |
| record 1095 line bytes, cp 18 (lane 0 / 1) | 64 / 74 | 45 / 52 | 64 / 74 |
| leader's target on the n1 straight (× 0.84) | 62.16 m/s | 43.68 | 62.2 |
| n1 matched (same pose, car, row, road) | lost 43.8 s, rival max 54.4 | **won** 30.4 s, max 42.7 | lost 23.8–35.5 s, max 51.5–56.1 (4 traffic draws) |
| rival top speed, bot runs n1/n2/n3/n6 | 54.4 / 51.0 / 42.7 / 58.1 | 44.0 / 35.8 / 44.0 / 41.9 | 56.1 / 46.8 / 49.7 / 54.4 |
| lead runs: crashes per km (≥ 10 m/s lost in 0.5 s, after 2 s) | 2.07 (8 runs, 17.4 km) | 2.14 (12 runs) | 2.51 (12 runs) |
| median recovery to 80 % speed | 3.75 s (max 19.1) | 3.52 s | 3.76 s |
| controller suspended (+0x710) | never (8 runs, 48 k rows) | 0 of 12 runs | 4 of 12 runs before the menus follow-up, 0 of 12 after (§3) |

The crash rate in the player-ahead test (the player put 40 m ahead every 5 s, so most
contacts are the rival meeting the player or traffic while chasing) is about
the same in both games; the original's rivals crash too. What made the clone
"easy" was the speed (30 % below the original on straights), the zigzag path
at junctions (walls), and the open suspension below.

## 1. The racing-line class (fixed, verified + observed)

0x5DDFD0 (route manager, ECX = 0x88A320) walks the racer infos (0x89E810,
0x940 each, count 0x89E814) and takes, of the AI ones (+0x04 = 2), the least
0x5B5340 class of their performance (info +0x20), into +0x4674 (0x88E994);
0x5E8790 then computes the racing-line bytes (CP +0x2C / +0x2D) with that
class's top speed, braking and acceleration curve. The infos hold the
performance 0x5B7580 matched to the player, so the class is the upgraded
cars'. Observed: 0x88E994 = 0 with COROLLA (row 1), CORSA (row 0), PEUGOT
(row 1) whose stock class is 3; record 1095's bytes at cp 10/14/18/21/22/24/26
were [56,63] [60,72] [64,74] [67,74] [68,74] [70,73] [72,71] — exactly the
clone's `racing_speeds` for class 0 (`native-record1095-lane-bytes.txt`,
`clone-record1095-by-class.txt`). The clone took the stock types' class
(`Encounters::new`), so the leader's target (route speed × 0.84, 0x41C5E0
race-state-7 branch) was 43.7 m/s where the original's was 62.2. Now the
routes are made once the cars are loaded, from their upgraded class
(`encounter_class`, `src/bin/game.rs`).

## 2. Junction-gap points on the driver path (fixed, verified)

0x415D50 adds a path point (at most 20) for each FuturePoint as 0x4122C0(point,
its +0x54, the lateral): the point's vtable +0x38 (0x40D380 → 0x5DD4A0 backward,
0x5DD370 forward) at the point's offset (+0x58) with lane 0's offset
(0x401C40), its adjust +0x04 and the lateral. A junction gap's inserted points
(0x418DA0, byte +0x74) go the same way; +0x74 is only a flag, read by the
smoothing 0x41A180, which sets their adjust so that the line passes the Hermite
curve. The clone used an inserted point's stored curve place without the
lateral, so a car off its line (laterals of 5–10 m were seen) got a path
alternating every 8 m between the line and its lateral, steered to and fro
(±19°) and ran straight on at turns (L1: into the wall at (−511, −1561) at
29 m/s). Now `RacerRoute::line_path` builds every point with the lateral.
This path is shared with race rivals (same function).

## 3. Rival stops for good after a lead change (fixed, verified by trace)

Clone (before): in 4 of 12 lead runs the chaser (goal 5) was suspended and
braked to a standstill until the race ended (L3 w40/w45, L5 w45/w55; e.g. L3
w45 from (−220.8, −1577.1)). At the first full think after that change
the goal-5 search (0x41E540 → 0x41B820 → 0x41B230) returned as origin the
first point of a record travelled backward (record index 720 cp 0, also 712 /
347 cp 0 in the other runs) whose segment held both cars; the rebuilt list
could not go on past the record's start, the car's own entry lay past its
only point, and the clone suspended it (`route.past_end()` → +0x710).

Native (traced, SPEED2.EXE):
* 0x41B230 on success copies the walk state and then writes the **origin**
  point and direction into the result (`*param_1 = param_2`, `+4 = param_5`);
  0x40C610 / 0x40A2D0 carry them to 0x41E540, which sets the road node (ctl
  +0x0C / +0x10), clears the decision flags and empties the list
  (`*(ctl+0x6E8)` → 0x411BE0: count 0, 40 entries freed). The clone's search
  and import match this (same origin, same containment, 0x40CE10 / 0x5D8A00:
  a record's first point uses its own length behind it).
* 0x42ABD0 full think: 0x42AF40 goal vt+0x44 (the search), 0x41D0E0, then
  0x428720 (ECX = ctl+0x54). Without a list 0x428720 tests the node's own
  segment (0x428893..0x42895E: lateral past the half width, 0x401B70, at most
  2 × 0x60C1B0; along clamped to [−back, length], 0x401090 / 0x40CCC0) and
  keeps it, else searches again (vt+0x30(1), 0x42CDC0); 0x428030 builds the
  list (0x426810 when empty). Its result (0x428C57..0x428CDB) is **false**
  when the list is empty and also when the car's own entry (+0x308 → +0x18)
  lies past the last point's s (+0x54).
* 0x42AF53: false → goal vt+0x1C (true for goals 5/6, 0x40CA10 / 0x40CA50)
  and race 7 (0x89E7B0) → 0x42AFD1 cleanup, **no suspension**, the driver's
  output kept. The suspension at 0x42B002..0x42B048 compares the same two
  values after a true return, so it cannot fire after 0x428720 (nothing runs
  between). Only the ready fallback (0x42ADA4 → 0x42ADAB → 0x42AE74,
  0x402530(1)) suspends on a false 0x428720.
* So at that spot the original's full think fails quietly and the next ready
  fallback's 0x428720 (no search: own entry ≥ last → 0x42CDC0 re-search →
  a new list) finds a node; the clone suspended at once.

Fix: `encounter_racer_think` treats "past the end" as 0x428720 returning
false (suspends only in the ready fallback); `RacerRoute::update` after a
search runs 0x428720's no-list node test (`route::node_holds`) instead of
skipping it. Clone lead runs after: 0 of 12 suspended; the 8 unaffected runs
are identical to before (internal evidence).

Not done: the spot was not reproduced in the original under Wine (the
earlier teleport of the rival blew up its velocity; driving the player there
with a matching chase was not attempted in this work), so "the original fails
the full think and recovers in the fallback" rests on the trace, not on a
memory read. The list builder for an empty list (0x426810) was not traced;
the clone extends with its 0x422220 port (**inferred** equivalent here).

## Other observations

* Opponent pick: the original drew COROLLA (row 1), CORSA (row 0), PEUGOT
  (row 1); career difficulty 0x83AC30 = 0, 0x89E7F4 = 0, throttle level
  0x89E804 = 3, stage 0x862838 = 6 (observed). The clone's draws differ (its
  random state is not the original's, see `Encounters::new`); the matched
  runs force car and row with `NFSU2_ENCOUNTER_CAR`.
* Acceleration from the challenge is within a few percent in matched runs
  (n1: native 16.5 → 38.1 m/s in 4 s, clone 23.1 → 39.8 from a later start).
* Nitrous: both fire a shot within 0.4 s of the challenge (as `outrun.md`).
* The clone's lead scenario starts from the pose the native script wrote
  ~0.3 s after the challenge, so the first second differs (L1 / L3 clone runs
  hit a wall or a car at 0–1.1 s that the original did not); those first 2 s
  are left out of the crash counts.

## Method

Original: `run_native.sh NAME bot` (`scen2.py`: engage the first moving
encounter car as `engage.py`, write the pose 12 numbers + tank and the road
path, drive the keyboard bot; `LEAD_AT=3 LEAD_EVERY=5 END_AT=60` repeats the
documented lead intervention every 5 s and brakes from 60 s), logged by
`orlog.py`. Clone: `run_clone.sh NAME CAR:ROW` (`--flow outrun` with
`NFSU2_FLOW_SAVE`, `NFSU2_FLOW_POSE`, `NFSU2_FLOW_PATH`,
`NFSU2_FLOW_LEAD_EVERY`, `NFSU2_FLOW_END_AT`, four traffic draws through
`NFSU2_FLOW_WAIT` 40/45/50/55). `stats.py` / `agg.py` count crashes, recovery,
speeds; `cmp.py` puts both rivals side by side. Raw logs stay in
`<work dir>/`.

Tests: `tests/ai_route.rs::encounter_racing_speeds_match_the_original_for_class_0`
(asset), `game.rs tests::encounter_class_comes_from_the_upgraded_cars` (asset),
`route.rs tests::inserted_points_keep_the_lateral_on_the_path` (fixture; fails
on the old path); the menus follow-up: `tests/ai_route.rs::chase_search_at_a_record_start_fails_the_update_and_recovers`
(asset) and the menus follow-up block of `game.rs tests::encounter_controller_full_fallback_and_reset_lifecycle`
(asset; fails with the old suspension).
