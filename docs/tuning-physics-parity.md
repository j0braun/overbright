# Tuning, parts and launch physics vs the original (2026-11-04)

This page of the roadmap: launch physics first (the race-AI parity's open items A and C,
docs/race-ai-parity.md), then the drag rub, then the tuning/parts sweep.
**Verified** = traced in SPEED2.EXE, **observed** = read under Wine with
the parity harness (docs/parity-harness.md), **inferred** / **open** as
marked. Raw runs: `<work dir>/tuning-work/runs/<name>/`;
extracts in `docs/evidence/tuning-physics-parity/`.

## New harness fields (tools/parity/fields.toml)

Drivetrain and wheels, context only (tol −1), each with a clone accessor
in `src/bin/game/parity.rs`: mover car +0x34; mover +0x48 the engine
(`eng_w` +0x14 crank speed rad/s, `eng_pedal` +0x78, `eng_power` +0x80,
`eng_cut` +0x88), mover +0x4C the drivetrain (`clutch` +0x14, `shift`
+0x44, `in_w` +0x4C, and native-only +0x50/+0x58/+0x5C/+0x60/+0x70, engine
+0x7C), mover +0x38.. the wheels in the record's order FL FR RR RL (the
clone's `wheels` order too): +0x28 spin, +0x118 / +0x16C slip, +0x138
load, +0x164 the previous load (the hop's term), +0x160 sliding, +0x108
torque, +0x180 grip. Car +0x42C the exported forward speed (`xspeed`),
+0x464 Stability Control off (`stab`), +0x46C / +0x474 the wheel hop's
count and grip. Globals 0x865148 (frame counter) and 0x7FB724 (world
update counter). `PARITY_NATIVE_PATCH="ADDR:HEX,…"` writes bytes into the
running original before the race (experiments only).

## A. "Launches ~10 % quicker" — three causes

### A1. Car +0x440 is the shown rpm, not the engine (observed)

The race-AI parity compared car +0x440 with the clone's engine rpm. +0x440 is a smoothed
rpm (it falls at a fixed per-tick rate, 3.82 rpm per 1/1800 s tick on the
COROLLA, then closes in by halves); the engine itself (+0x14 × 9.5493)
drops as fast as the clone's. With `eng_w` both sides agree to the
0.1 rad/s through the clutch's first frames (evidence
`rpm-display-vs-engine.txt`). The "steady 5500 rpm/s fall" of the race-AI parity was this
display smoothing.

**The writer (verified, ported).** The mover's publish (0x5A5966..0x5A5A61,
once per mover call) writes physics view +0x400 = car +0x440: with the
engine rpm e (+0x14 × 9.5493) and the shown value s, a fall faster than
r = −table / crank inertia (engine data +0xC; table 0x7A1A34 = 2.5, or 15
while an up-shift past 1st is pending, +0x48 and target +0x38 > 2) is
held to s + r·dt (only when the call's dt > 0), then s = 0.45·s + 0.55·t
(0x7A20B0 / 0x7A20AC). The view's constructor starts it at 1000
(0x59F993). The tachometer (0x4C6933), the engine sound's context
(0x484C50) and the dyno (0x4018C0) read it. `drive::shown_rpm_step`,
`CarState::shown_rpm()`; the HUD, the engine sounds, the dyno and the
harness's `rpm` read it now. Replayed against the original's COROLLA
launch within 0.02 rpm over 50 frames (`shown_rpm_matches_native_launch`,
fixture `shown-rpm-corolla.tsv`); in circuit_s1b the clone's +0x440 runs
within 13 rpm of the original's until the jolt of item A4.

### A2. The green's gear change came one update early (fixed)

Observed (every rival, every run): at the first status-4 frame the
drivetrain is still in neutral with the clutch locked (+0x44 = 3, +0x14
= 1) though the driver's block already reads gear request −1 and brake
0; the shift to 1st (Disengaging) starts in the next update. The race
start's hold release writes the request after the movers ran (the AI
input's vt+0x30 is 0x410030, `+0x1E4 = −1`, **verified**; the mover reads
+0x1E4 through vt+0x20 = 0x405B20 and hands it to 0x5AA970, which out of
neutral asks 1st at once, **verified**). The clone applied the release in
the same update, so its clutch, gear and first wheel spin ran a frame
ahead. Now `AiControls::mover_held` (set by `AiDriver::green_release`)
makes that update's physics read the countdown block (brake 1, neutral);
the harness's `gas`/`brake` read the driver block. After: clutch,
gear and shift state match frame for frame (circuit_s1b–g).

### A3. The harness's grid copy lifted the cars (harness, fixed)

`place_on_grid` turned each settled car flat onto the original's heading:
the rear loads fell from 1.9 to 0.3 kN and the rest test (class 2, no
gravity) froze them so through the countdown, so the clone launched on
light tyres. Now the car is moved as the race set-up places it
(`CarState::placed`, 0x5F4A00) keeping its settled pose relative to a
fresh placement; loads match (1.94 vs 1.77–2.18 kN native at rest).

### A4. The replay manager's DUMMY_SNAPSHOT resets every car's physics caches every 100 world updates (verified, ported)

Every native run shows, on the world update whose counter 0x7FB724 is a
multiple of 100, a one-update jolt on every car at once: vertical velocity
−0.03 m/s, rear loads +0.25 kN, driven wheels −30 % spin (or a spinning
pair grabbing outright, slip 8.5 → −0.05 m/s), the engine −90 rad/s
(evidence `snapshot-jolt.txt`; frames 17/117/217/… in circuit_s1g =
counter 400, 500, 600 …).

**Mechanism (verified).** The replay manager's per-frame 0x581060 calls
0x57D010 (0x58110B), which jumps to 0x57C190 when the manager records and
`0x7FB724 % 100 == 0` (0x57D048..0x57D060). 0x57C190 builds a snapshot
named DUMMY_SNAPSHOT on the stack and calls the serialise-all 0x57BFF0
after writing `[esp+0x1E4] = 0` and `byte [esp+0x48] = 1` with one push
outstanding: that is snapshot +0x1DC (the write cursor) = 0 and **+0x40
(take mode) = 1**, with a buffer size +0x1E4 of 0 (0x5714B0). So it is a
full *take* into an empty buffer: no bytes are stored (0x571780: the
cursor runs past the size), but every serialiser's take/restore branch
(`+0x40 | +0x41`) runs. Those branches reset caches: the mover 0x5A47C0
(aero block +0x50: +0x8 = 1, +0x10/+0x14/+0x20..+0x28 = 0, +0x30/+0x34 =
1), the rigid body 0x59B700 (0x59B778: +0x3C/+0x4C/+0x5C/+0x60..+0x68 =
0, +0x6C = 1, +0x150 = +0x154 = 1, +0x15C = 0, +0x160 = gravity 0x7FBEF0;
then vt+0x1C), the suspensions 0x59C990 (0x59CA22..0x59CB85), the tyres
0x59E030 (0x59E066..), the engine 0x594230 (+0x90 = 0), the view 0x59F530
(0x59F562, 0x59F692, 0x59F6FB, 0x59F807..) and the contact pairs
(0x5AE2E0/0x5AE230). Bisected under Wine with `PARITY_NATIVE_PATCH`: the
jolt goes with 0x57D059 NOPed (circuit_nosnap), with the world serialiser's
three calls skipped (exp_world3), the world list 0x5FF8A0 (exp_wlist), the
per-object loop 0x5FF9D6 (exp_wobj) and the mover call 0x5FB90F (exp_mover);
at launch speeds the rigid body's alone removes it (exp_rb2; evidence
`aistats.txt` and the run dirs). Not the per-player serialisers
(0x60A66A, circuit_nocars).

**What changes (observed).** `tools/parity/snapdiff.py --attach` stops the
original's game thread right before the snapshot (EB FE over the jmp at
0x57D060) and right after it (EB FE at 0x581110), both at the same loop
counter, and diffs the player car's objects (evidence
`snapshot-take-diff.txt`, loops 1700 and 1800 in a race): rigid body
+0x110..+0x128 (the force / torque sums) zeroed with +0x118 = −10.339
(m·g), +0x130..+0x148 = the old sums, +0x150 = 1; the suspensions'
+0x24 and +0x30..+0x98 (forces, mount points, preload +0x88) zeroed; the
tyres' (mover +0x38..) rotation rows, contact points, hub velocity, +0x104,
+0x108 torque, +0x124..+0x138 (forces and load), +0x164 (the load the hop
reads), +0x16C slip, +0x170 circle, +0x180 grip, +0x184 zeroed — the spin
+0x28 is kept; engine +0x90 (catch-up) 0; car +0x3A4..+0x3B0 = 1,
+0x418..+0x420 and +0x510..+0x538 zeroed.

**Where the resets bite (traced).** The native mover integrates at the
start of a sub-step the sums its force pass left at the end of the last
one: the rigid body's derivative reads the force / torque sums +0x110 /
+0x120 (0x59C2F4.., 0x59C3BE..), the wheel's derivative 0x59D360 reads the
tyre's drive torque +0x108, its force +0x124 and the running mean +0x120,
the rate also the sliding flag +0x160 and the slip +0x118 (the gripping
correction). The rigid body's take branch is its tidy-up (vtable 0x7A20CC
+0x1C = 0x59C600): the sums become (0, 0, −m·g) / 0. The force pass
(0x5B0530) feeds the tyres' load through 0x59D170 from each suspension's
force +0x24 and contact flag +0x80, both zeroed by the take.

**Ported: the rigid body's part.** `Race::world_updates` stands for
0x7FB724: 0 when the race's world is built (0x576D00 from the world
constructor 0x5FF54D; the race set-up; **inferred** for a restart, which
builds a new race in the clone), +1 per world update, the start scene and
countdown included; on a multiple of 100 every racer takes the snapshot
before the update (`CarState::snapshot_take`) and its next integration
reads gravity alone. The counter at the green varies natively with the
load (381–497 in seven runs), so the harness copies it (`world_n` in the
plan). Pinned: `the_dummy_snapshot_drops_the_car_for_a_sub_step` (−0.034
m/s; Wine −0.031..−0.036); in circuit_s1g the clone's CORSA drops 0.032
against the original's 0.033 at frame 17.

**Ported: the tyres' part (captured sub-step by sub-step).**
`tools/parity/substep.py` stops the original before (0x5B73D8) and after
(0x5B73E3) every call of the physics mover's integrator 0x5AA490 across a
snapshot (evidence `substep-capture.txt`, a COROLLA's spinning rear wheel,
loop 400, 16 frames after the green). The take zeroes the tyre's drive
torque +0x108, force +0x124, load +0x164, slip copy +0x16C, circle +0x170,
grip +0x180 / +0x184 **and its sliding flag +0x160**, the suspension's
force +0x24 and contact flag +0x80, and engine +0x90; the frame set-up
rewrites +0x180, +0x184 and +0x90 before the first integration. That
integration's wheel derivative 0x59D360 then halves the running mean
+0x120 (no torque, no force) and, the wheel no longer sliding, takes the
whole slip (+0x118, kept) out: 47.27 → 1.25 rad/s in one sub-step; the
next three sub-steps spin it back to 23.4, the clutch drags the engine
601.7 → 510.6 rad/s. The clone does the same in the first sub-step after
`snapshot_take` (`CarState::substep`: net torque halved, the gripping
correction for every wheel, a locked wheel's rate zeroed first). After
(circuit_s1g frame 17): CORSA front wheel 22.10 vs the original's 21.57
rad/s, engine 455.3 vs 456.6, input shaft 288.7 vs 287.7, vertical speed
−0.0105 vs −0.0096. Pinned:
`the_dummy_snapshot_lets_spinning_wheels_grip_for_a_sub_step` (the spin
ends the frame at 0.47 of a plain frame's; Wine 0.54–0.55 later in the
spin-up).

**Launch re-measured** (aistats, `aistats-snapshot-full*.txt`; the older
runs' counter phase taken from their jolt frames with `PARITY_WORLD_N`):
circuit_s1f MIATA 4.4 / 8.8 / 17.6 m/s at 1 / 2 / 5 s against the
original's 4.3 / 8.9 / 17.6 (was 10.5 at 2 s), 240SX 8.3 vs 8.0 at 2 s;
circuit_s1e COROLLA 4.4 / 11.1 / 21.6 vs 4.3 / 11.0 / 21.5; circuit_s1g
CORSA 5.6 / 10.7 / 20.8 vs 5.5 / 10.7 / 21.0. The rivals that differ more
(circuit_s1f MIATA#2, s1e MIATA) hit other cars.

**Counter on a restart (open).** The clone's restart builds a new race,
so its counter restarts at 0 as a load does natively; whether the
original's Restart rebuilds the world (0x5FF54D) was not traced.

What it does to a launch depends on where the 100-update boundary falls:
in circuit_s1f it hit the MIATAs at 0.77 s with the rear wheels spinning;
they gripped, the engine fell to 2500 rpm and the cars were 1.6 m/s
slower at 2 s than the clone (aistats v2s 8.9 vs 10.5). Without it
(circuit_nosnap) the clone matches: 240SX 6.0 / 12.7 / 21.2 vs 6.1 /
12.8 / 21.2 m/s at 1 / 2 / 5 s, COROLLA 4.4 / 10.7 / 20.9 vs 4.5 / 10.9 /
21.0 (evidence `aistats.txt`). So most of the race-AI parity's "10 % quicker" is this
jolt, not the drivetrain. The clone does not reproduce it (**open**:
port once the state it changes is known; it is native behaviour, every
1.67 s of play, also outside races while the replay manager records).

### Remaining small differences (open)

* The wheel hop's grip (car +0x474): 0.924 vs 0.939 on the COROLLA; it
  comes from the first frame's slip × load / 3 (0x5B6649), which follows
  how the rear pair spins up.
* Rear-pair spin-up: the clone's driven wheels part (34 / 21 rad/s at
  0.23 s) where the original's stay within 1 %; both split torque by each
  wheel's own spin share (0x594A40, **verified** the same). Seed and
  damping of the split are **open**.
* (Car +0x440 is ported now, see A1.)

## C. Drag up-shift timing (ported)

Re-measured (drag_p4, evidence `drag-shifts.txt`): the rivals' shifts
(frames with the drivetrain not locked) are 30 (1st), 16, then 20 / 30 /
86 natively, the clone's 30, 16, 16; the original's rivals shift at other
rpm than the clone's (COROLLA 2→3 at 692 rad/s against 769).

**Three causes (verified in SPEED2.EXE, observed under Wine).**

1. *The AI's shift offsets.* The up-shift rpm (0x594680) is the gear's
   point +0x84 (redline − 100, 0x5AC7E0) plus a random offset (+0x78, or
   +0x80 for 2nd gear), redrawn at every gear change by 0x594720 as
   lo + random(hi − lo) from the ranges +0xCC..+0xE0 (0x5EA510 on the
   world's random state 0x890080 +0xF8). The drivetrain's constructor
   gives −100..0 for all three (0x5AFAC3..0x5AFB01: 0x7FC294 = −100,
   0x7A18C4..0x7A18CC). The racer set-up 0x5B6130 (0x5B619E..0x5B626A),
   for an AI racer's controller (car +0x14 → +4 = 2) with owner kind 2–4
   in a drag race (0x89E7D8 / 0x890108), overwrites them from the AI
   tuning row of the controller's skill (vt+0x78, only for rows 0..=9):
   up +0x558..+0x55C, down +0x550..+0x554, 2nd gear +0x560..+0x564 at
   0x81CB60 + 0x68·row (rows 0–9: −900..300 down to −110..15; down
   0..0.01). Outside the career (0x850078 ≠ 1) it also sets the clutch
   factor +0xE8 = 0x7A19FC[difficulty] = 0.4 / 0.8 / 1.0. Observed (drag_s1
   re-run, evidence `drag-shifts-p4c.txt`): the CORSA (row 3) −500, 140,
   0, 0.01, −500, 140; the FOCUS and PEUGOT106 (row 2) −650, 200, …; the
   player −100, 0 ×3; offsets drawn per gear change e.g. +38 / −405,
   −297 / −34, −175 / −473 (CORSA).
2. *The shown rpm.* The mover hands 0x5AA970 the car's shown rpm +0x440
   (0x5ABD83..0x5ABDC0; the override 0x7FBED8 is 100 = off), and both the
   automatic choice and the drag grading 0x5A1710 → 0x594F30 read that
   argument; the clone read the engine's.
3. *The grade and the slow clutch.* An offset over 5 % of the shift
   window past +0x84 grades −1 (0x594F30), and the clutch then runs at
   × 0.35 (0x7A18DC) for a target of 3 or more (0x5949C0): the 86-frame
   shifts. A grade of 0.85 / 0.5 makes the clutch × 1.85 / × 1.5 (16 / 20
   frames). The clone's grading was already this; with −100..0 offsets it
   never over-revved.

**Ported.** `Handling::shift_ranges` / `clutch_scale`,
`handling::drag_ai_shift_setup` (called for each rival in
`line_up_rivals`), `AiTuning::shift_ranges`, the reroll from the ranges,
`Drivetrain::shown_rpm` (set by `CarState::step` and `CarState::shift`
from car +0x440) for the choice and the grade, × +0xE8 in the clutch rate.
Each rival's drivetrain draws its offsets from its own fixed sequence
(`Drivetrain::seeded`; natively one shared random state, not matched).
After (drag_c1, same cars and skill rows): clone CORSA 30 / 16 / 16, FOCUS
30 / 20 / 20, PEUGOT106 30 / 86 against native 30 / 86 / 16 / 20,
30 / 20 / 86 / 86, 30 / 86 / 20 / 86: every shift duration now follows its
grade as natively (−1 → 86 from 2nd gear, 0.85 → 16, 0.5 → 20); which
shift gets which grade follows the random draws.

**Drag target 1000 and the drag green release (enabled).** With the above,
the drag rivals now take the controller's cap 1000 (0x784550) as target
(+0x230, observed in every drag run) and the green release as in the other
races. `--flow firstdrag` with seeds 0, 4, 7, 20261103 × 60 / 144 Hz /
uncapped × first + Restart: 24 runs, 0 failures, rivals on their wheels,
3 rival drop-outs (traffic hits; 8 in first-drag.md's last count);
firstrace, postrace and the playthrough pass. aistats (drag_c1): CORSA
5.9 / 10.9 m/s at 1 / 2 s vs 5.8 / 10.9 native, FOCUS 5.5 / 12.6 vs
5.4 / 12.4, PEUGOT106 4.7 / 10.7 vs 4.7 / 10.7; target 1000 on both sides.

**The contact-pair take (traced, nothing to port).** The pair's
serialiser 0x5AE230 runs for a take (+0x40) 0x5985A0 with the two bodies'
ids (vt+0x7C), which returns at once while they equal the pair's own
(+0x224 / +0x228; a live pair's always do), then 0x5983A0 and the contact
record's 0x592B50, which only copy fields into the snapshot (0x571780 /
0x571650; the DUMMY_SNAPSHOT buffer is empty). 0x5A9B20 runs only for a
restore (+0x41). So the take changes no contact state; consistent with
`snapshot-take-diff.txt`, where no contact field moved.

## B. The drag rub: first_drag Restart seed 4

**Symptom.** With the item-A4 snapshot port `--flow firstdrag` failed (Restart,
seed 4, every rate): rival 2 (MIATA) rolled over at 29.75 s (125°). The
handover suspected the snapshot at 29.350 s sinking the car. **Not so**:
with the snapshots switched off from 29 s on (a temporary hook) it tipped
the same way. The snapshot only changes the launches, and with them where
the rubbing rivals go.

**What happens (clone, logged per sub-step; evidence
`drag-rub-barrels.txt`).** The rub pushes rival 2 into a stack of
ZCS_Barrel_A props by the wall (0.9 t, 2 m tall, centre of mass 0.95 m up)
at 37 m/s. The barrels stop it in about 0.2 s; no car pair is involved.
The barrel then stays overlapped for whole frames (`stuck`), and each
stuck contact runs the push-out (0x5B4300). The clone took that push's
direction from the car's **origin** to the barrel's centre. That line
points steeply up (the barrel's centre is 1.3 m above the origin), so
every push moved the car down by up to the 0.15 m vertical cap. The car
sank 0.17 m, all four suspensions went to full compression (bump stops),
and the springs threw it up at 4 m/s with 2.7 rad/s of roll. It landed on
its side.

**Native (verified).** 0x5B4300 gets each body's position at the moment
from its vtable +0x34 (0x5B4326 for A, 0x5B433C for B). For the car
collision body (vtable 0x7A2478) that is 0x593290, which interpolates the
position from vt+0x08 = 0x593B50: owner (+0x130) +0x60, the **centre of
mass** (car +0x60; docs/formats.md, car +0x50 = the origin), or [+0x68]+0x20.
The direction is B − A (0x5B43C4..0x5B43D8, or 0x5B446F.. below 55.56 m/s),
normalised (0x43CF10). A moves back by it and B on, each only when its
vt+0x64 allows (0x5AEE90 for a car). Ported in both push-outs:
`smackable::car_contact` (car with prop) and `drive::resolve_cars` (car
with car) now use `CarState::com_at`. No threshold was changed.

**After.** firstdrag passes at 60 Hz, 144 Hz and uncapped, first run and
Restart: rival 2's roll peaks at 7.7°. The Restart's rival 1 now takes
the same barrels and peaks at 36.1° (the flow's limit is 45°), then lands.
Whether the original's rivals hit these barrels in this race was not
checked under Wine (**open**). The plan's native capture of a staged
two-car rub across a snapshot (step 2) and the contact-pair take branches
(0x5AE2E0 / 0x5AE230, the 0.3 s record 0x592BE0) were not done: the fault
was not in the snapshot.

## Tuning sweep (the drag shift sweep item 3)

**Method.** The physics runs on a tuning block, a copy of the car type
record from +0x110 (0x2B0 bytes) with the performance parts, uniques and
dyno tuning applied (0x5B4DD0 → 0x5AC500, 0x5997F0, 0x5B4790); the
upgrade state follows it (block + 0x2B0: category levels 0–3, unique flags
+0x28, fitted part bytes +0x50 + id) and the set-up object sits at
block − 0x30 (+0x08 the class area, +0x0C 0x59A360's acceleration area,
+0x14..+0x20 the launch figures of 0x5B06B0). The engine's data pointer
([[car +0x34] +0x48] +0x18) is block + 0x1F0 (record +0x300: idle, redline,
max rpm), so `tools/parity/recdump.py` reads all three from a running
original (free roam), and the harness's native.json already held each
car's block (car +0x20). `devprobes/src/bin/block_sweep` rebuilds the clone's
record from the same fitted parts and uniques (`car::tuned_record`) and
compares word for word (race-tuned runs on the parsed handling after
`race::apply_tune`); the game's own load with the dyno tuning is dumped
with `NFSU2_DUMP_TUNED_RECORDS=DIR` (check aid, `<car>-tuned.bin` and its
sliders).

**Coverage (observed).** 149 cars: every car of 29 earlier harness runs
(stock rivals, early fits, the outrun's 36–39-part cars, drag and drift
tunes) and the players of six saves from stock to maxed (career start
350Z, a LANCEREVO8 with all nine uniques, PEUGOTs of stages 2–6). All 149
blocks match; for the five free-roam players the class area, the
acceleration area, 0-60, 0-100 and the quarter mile (time, speed) match
too (evidence `tuning-sweep.txt`). So the parts' effect on power/torque
(curve points), gearing, mass, brakes, tyres, suspension, aero and nitrous
is native at every level seen, and the top speed and launch figures the
set-up derives from them follow.

**Fixed: the dyno tuning dropped the uniques' bonuses.** The players'
tuned loads matched but one: the community LANCEREVO8 (uniques 0–8, every
slider neutral) had tyre grip / sliding 0.05 lower (1.85 / 1.6 front,
1.9 / 1.65 rear against the original's 1.9 / 1.65, 1.95 / 1.7). 0x5B4790
takes the uniques off the block before tuning (0x599DA0 at 0x5B47BD) and
puts them back after (0x5997F0 at 0x5B4BDF); so do the bases (0x5A7DB0)
and the screens' writer (0x5B3FB0) (**verified**). The clone tuned the
block with the uniques on, so the tyre pair's write clamped the unique's
+0.05 off. Ported: `performance::remove_uniques` (0x599DA0: mass ÷
(1 − k), the nitrous, brake and tyre terms and the torque / turbo curve
terms subtracted; unique 1 is left) around `tuning::apply` and
`tuning::nudge`. After: all six players match.

**Open.** Non-neutral dyno sliders were not read from the original (no
native save with them; the earlier per-slider ports stand); set-up +0x10
(written outside 0x5B4DD0) is not identified; free-roam encounter racers
run on another mover (no block); the slider views (`slider_view`,
`slider_range`) still work on the block with the uniques on (display
only); dynamic checks (braking distance, cornering grip) against the
original were not run — their inputs (the block) match; and the visual
rating / reputation formula for part combinations was not compared with
the original (`PartDb::visual_rating` is the earlier port).

## Tests

* `tests/tuning_physics.rs::upgraded_blocks_match_the_originals` (five
  native blocks in `tests/fixtures/native_blocks`).
* `tests/tuning_physics.rs::neutral_tuning_keeps_the_uniques_bonus` (fails
  without `remove_uniques`).

* `handling::tests::drag_up_shift_is_chosen_and_graded_at_the_shown_rpm`
  (fails reading the engine's rpm).
* `handling::tests::drag_ai_set_up_takes_the_ranges_and_the_clutch_factor`.
* `tests/ai_route.rs::drag_ai_shift_ranges_by_skill_row` (the AI tuning's
  rows against the Wine reads; skips without NFSU2_ROOT).

* `tuning_physics::stuck_cars_are_pushed_apart_between_their_centres_of_mass`
  (fails with the origins).
* `tuning_physics::the_dummy_snapshot_lets_spinning_wheels_grip_for_a_sub_step`
* `tuning_physics::the_dummy_snapshot_drops_the_car_for_a_sub_step`
  (the snapshot's rigid-body reset against the Wine drop).
* `drive::tests::shown_rpm_matches_native_launch` (car +0x440 against
  the original's frames).
* `first_race::tests::rivals_hold_the_line_and_launch_at_full_throttle`
  now also checks that the green's update leaves every rival in neutral
  with the clutch locked and that the shift to 1st is under way one
  update later (fails without `mover_held`).

## 2026-11-06: re-check on <base>, the standing look point

**State found.** Part 4 (the DUMMY_SNAPSHOT port, the drag rub push-out
from the centres of mass), C (the drag AI shift offsets, the × 0.35 slow
clutch, target 1000), the tuning sweep and the test isolation (docs/test-determinism.md)
were already merged on main; the roadmap's
"NOT merged" note was stale. The snapshot-tuning pass re-ran the stored native captures against
the base (the stage-4 drag to outrun-pace changes included) with `parity.py clone`.

**Launches on <base> (observed, evidence `p15-launch-aistats.txt`).**
Most rivals matched within 0.1–0.2 m/s at 1 / 2 s, but one rival in a
third of the races left in another wheel-hop phase: the hop's first count
(car +0x46C, 0x5B6649: trunc((1.5 − g)·6 + wheel & 1)) came out 3 where
the original's was 4 (6 of 18 rivals over six runs), so the pair grip
pattern of the hop (+0x180 × 0.35 / 0.65) ran mirrored and that car fell
behind (circuit_s1f MIATA#1 7.5 m/s at 2 s against 8.9 natively). The
count's parity is the wheel with the largest min(slip, slip · load/3)
(+0x118, +0x164; the native loop takes the first strictly larger, as the
clone), so the left / right rear loads at the clutch bite decide it, and
those followed a roll the clone's rivals got from steering: they held
2.8–3.9° at the green where the original's held 1.0–2.9° (circuit_s1f
MIATA 3.11° vs 1.05°), until the car rolled (frame 13).

**Cause (verified).** The look-ahead point (0x415F80 → 0x415630) starts
from the first path point ahead along the velocity (0x404EF0, called with
the car's speed +0x42C, position +0x60 and velocity +0x70). 0x404EF0
normalises the velocity with 0x43CE50, which returns **(1, 0)** for a
zero vector (0x43CE99), and takes the first point with dot > 0 and a
nonzero distance (thresholds 0x7837C4 / 0x7837C8 = 0, 0x827A20 =
0x81CB84² = 0 in the image). The clone's `flat_unit` returned zero, so a
standing car found no point ahead and looked from the path's **last**
point. Observed under Wine (circuit_s1f): the original's look point stays
(−4.17, 369.0) from the green on, through the roll-off; the clone's was
(−4.56, 371.44) until it moved, then (−4.39, 368.99).

**Fixed.** `driver::flat_unit` gives (1, 0) for a zero vector (only
`path_point` uses it). After (`p15-launch-aistats.txt`, `p15-standing-look.txt`):
every circuit rival's steering at the green is within 0.1° of the
original's (0.96–2.75°), circuit_s1f MIATA#1 4.3 / 8.8 / 17.5 m/s at
1 / 2 / 5 s vs 4.3 / 8.9 / 17.6 (was 4.3 / 7.5 / 17.2), s1e COROLLA
4.3 / 11.0 / 21.5 vs 4.3 / 11.0 / 21.5, every circuit rival within
0.1 m/s at 2 s. Hop phase: 5 of 18 still differ (the drag rivals, whose
start steering is the lane DragSteer's, and circuit_s1e MIATA).
Test: `ai::driver::tests::a_standing_car_looks_from_the_first_point_ahead`
(fails with the zero fallback).

**Open.** The remaining hop-phase flips (drag_c1 FOCUS / PEUGOT106,
f2e ECLIPSE, w4b IMPREZAWRX, s1e MIATA): their start steering is within
0.06° and their rear-load left/right swing at the clutch bite still
differs (the clone's swings sign frame to frame where the original's
moves smoothly; the player's AWD Skyline, sub-step capture n1, matches),
not traced further. The drag rivals' 1→2 shift grade (w4b WRX 17.9 vs
20.7 m/s at 2 s: the clone over-revs, −1, slow clutch; the original's
grades 0.85) follows the per-rival random offsets, which do not share the
original's random state (C above). The replays' row phase: the clone's
+0x108 / +0x138 rows are the last sub-step's start values, the original's
its end (native torque per clutch unit 81 every frame, the clone's
40 / 66 / 73 over the first three; spins agree), a reporting difference.

**Flows.** `--flow firstdrag` with seeds 0, 4, 7, 20261103 × 60 /
144 Hz / uncapped × first + Restart (24 runs) failed once on <base>, and
the same with the look fix: "first drag 144 Hz seed 0: the player did not
finish" — no rival tipped; the flow's player bot hit traffic at 17 s and,
shifting only up, crawled on in top gear (5.5 → 34 m/s over 24 s) past
the 75 s. That run alone (seed 0, 144 Hz only) finishes in 58.95 s like
the 60 Hz one, so a flow run still depends on the run before it (state
kept across `fresh_first_drag`; **open**, not traced). The bot now also
shifts down under 40 % of the redline (flow aid, `first_drag.rs`; no game
change): seed 0 at 60 / 144 Hz finishes 50.6 / 52.0 s, Restart 41.2 /
59.6 s.
