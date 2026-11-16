# Cornering and braking against the original, from a shared start

The clone's player is put on the original player's exact state (the full transplant,
`[run] transplant_full = true`, docs/parity-harness.md) and both games run the same keys on the
same frames. This page lists the per-wheel and steering state the transplant now carries, the
proof that the first frames after it agree, and the cases measured from it.

**Verified** = traced in SPEED2.EXE; **observed** = read under Wine; **inferred** / **open** as
marked. Raw runs stay in the work directory; the summaries are in `docs/evidence/cornering-parity/`.

## 1. The state the next frame reads (traced)

Per wheel (physics mover +0x38 + 4i, FL FR RR RL):

| wheel | read by | carried before | now |
|---|---|---|---|
| +0x28 spin | everything | yes | yes |
| +0x10C lock flag | 0x5A8290 (no brake torque into a locked wheel, the slide branch), 0x594A40 (no drive torque), 0x59D360 (a locked wheel's spin rate is 0, 0x59D3CD); set by the lock test 0x59D410 | no | yes |
| +0x120 net torque mean | 0x59D360: `(T − r·Fx + last) · 0.5` (0x784260) | no | yes |
| +0x108 torque, +0x118 slip, +0x124 / +0x128 forces | zeroed each sub-step by the wheel set-up 0x59D040 (0x59D081..0x59D0A2) | — | not needed |
| +0x160 slide flag | written by the tyre pass before 0x5ABE00 and 0x59D360 read it | — | not needed |
| +0x178 / +0x17C brake inputs, +0x180 / +0x184 grips | set every frame from the pedals (0x5A4D90) | — | not needed |

The rest (traced in the clone's port, carried now): the player steering (input car +0x30, the
PlayerSteering object at +0x220, from 0x41E36C): the wheel angle +0x7C (car input +0x1E0), the
lock last allowed +0x80, the counter-steer latch +0x10 (a byte, 0x405F9C), last frame's shaped
input +0x84 (0x41E042); the handbrake's rear-grip ramp (mover +0x70); the wheel hop (car +0x46C
count, +0x470 timer, +0x474 grip, +0x478 calm); the rigid body's rest damping (+0x90); the
engine's reaction on the crank (engine +0x7C, the next engine step adds it); the clutch's last
slip (+0x58); the gear change's random offsets (+0x78, +0x7C, +0x80: the next shift point).

Not carried:
* The steering's two running means (PlayerSteering +0x14 and +0x48, 0x576AB0: count +6, next +7,
  sum +8, mean +0xC, samples +0x10, window +0x28, oldest +0x2C, times +0x30). They keep their
  samples; the scenarios hold their keys long enough before the transplant. Recorded as
  `steer_avg` / `steer_rate`: at frame 52 the original's input mean read 0.94–0.97 (its keys act
  from frame 18, after the teleport hold) against the clone's 1.0; two original runs with 0.94
  and 0.97 there are otherwise identical, so the mean does not matter at that point.
* The crank's reaction (engine +0x7C). The original's state at a frame's end is the one after the
  force pass its post-step takes for the next sub-step (docs/formats.md, **Order of a frame and a
  sub-step**), so +0x7C already holds that pass's reaction; the clone takes the same pass again
  from the transplanted state and builds the same reaction from 0. Carrying it counted it twice:
  in a disengaging shift (hand90-2, frame 52 → 53) the engine slowed by 1.17 instead of 1.35 rad/s.
  The engine's power factor (+0x80), which the clutch's tidy-up sets before that pass, is carried
  now (it was not; the earlier trial carried it together with the doubled reaction).
* The drivetrain's random generator (the clone draws the shift offsets from its own).

## 2. Three harness errors found on the way (fixed)

* **The suspensions went in a sub-step ahead.** The original measures the suspension at the end
  of each sub-step (0x5A87A0 from 0x5B04E0), so its +0x1C / +0x20 are the pose as it stands. The
  clone measures at the start of the next sub-step and keeps the measurement before; putting
  the original's +0x1C there made the clone's first sub-step see a rate of about 0. The clone
  now takes `x − rate · last_sub` (`put_suspensions`): the first frame's vertical speed agrees
  (braking, frame +1: 0.035528 against 0.035519 m/s; before 0.0309 against 0.0335 in a run of
  the same kind). Every earlier stance and transplant run had this (docs/prop-hits.md §7b: the
  wall and barrel numbers there were made before the fix).
* **The free-roam place dropped Stability Control.** The harness's free-roam start replaced the
  car's state with a fresh placed one, clearing car +0x464 (the original reads 1 in free roam,
  field `stab`), so the clone's rear wheels missed the launch factor 0x59D4A0 under 50 mph. The
  place keeps the driver's flags now.
* **A held left turn was sent once.** `native.py` decided "held" by the event's value, and the
  steering's left is the value 0x00, which read as a released key; the original steered for a
  frame and then went straight. Held now by the press (`tools/parity/selftest.py` checks it).
  Earlier runs that held `left` steered only on their first frame.

## 3. The proof: no input change after the transplant

Full braking from 200 km/h (brake held since frame 30, ABS active, no lock), transplant before
frame 60 (`docs/evidence/cornering-parity/proof-brake-t60.txt`), 60 frames:

| quantity | with the tyre state | without (`PARITY_NO_TYRES=1`) |
|---|---|---|
| wheel spins | ≤ 0.008 rad/s | 1.45 rad/s from frame +1 |
| longitudinal slips | ≤ 0.002 m/s | 0.062 m/s from frame +1 |
| net torque means | ≤ 0.0094 kN·m | 0.31 kN·m from frame +1 |
| apart at 0.5 / 1 / 2 / 5 s | 0.003 / 0.003 / 0.003 / 0.76 m | 0.16 / 0.33 / 0.62 / 1.08 m |
| \|v\| at 2 / 5 s (original 62.06 / 19.03 km/h) | 62.06 / 21.39 | 61.09 / 19.25 |

A held left turn at 79 km/h with the clutch locked, transplant before frame 100 (40 frames,
`proof-turn90-t100.txt`): spins within 0.018 rad/s, slips 0.003 m/s, yaw rate 0.0016 rad/s, the
tyre forces within 0.6 % (front left 2.757 against 2.745 kN at +1).

What is left at frame +1 is not float noise in the strict sense: 0.1–0.6 % in the tyre forces of
a wheel at its friction circle, and a lateral force that flips between 0 and its first table step
where the slip angle sits at the table arctangent's 1/256 threshold (rear wheels in the braking
run, frames +3 on). The 0.1–0.4 % in the braked rear wheels' load follows a compression 10–12 µm
apart after one frame (the damper takes it over a 4 ms sub-step) — below the float32 step of the
world position there (0.03 mm at x ≈ 370); **inferred** to be rounding, not a rule. The recorded
`comp_*` and loads are a sub-step apart between the games (the original's are measured at the
end of the last sub-step, the clone's at its start): compare forces, spins and slips, not those.

## 4. The cases (keys replayed, 3 original runs each)

Free roam on the straight highway stretch south-east of the barrels (heading 120.6°,
scenarios `tools/parity/scenarios/corner_*.toml`), the player rolled at frame 30; the clone
transplanted before frame 32 (braking) or 52 (the rest, so the steering means had filled).
Wheels FL FR RR RL; "first" = the first quantity beyond noise (tolerances in
`<work dir>/cmp.py`: 1 mm, 1 mm/s, 0.01 rad/s, 1 N).

| case | from | first difference | apart at 1 / 2 / 5 s (3 runs) | contact |
|---|---|---|---|---|
| full braking | 199.5 km/h | +1 frame, front Fx −2.98 / −3.06 kN (3 %), the sliding fronts | 0.005 / 0.017 / 0.24–0.50 m | none |
| left lock, gas, rolled at 60 | 56 km/h | +1, FL Fx 0.92 / 0.93 (a clutch locking) | 0.004–0.018 / 0.007–0.039 / 0.07–0.17 m | wall at +4.6 s |
| … at 90 | 85 km/h | +1, FL Fx 0.875 / 0.890 | 0.003–0.007 / 0.007–0.009 / 0.05–0.67 m | wall at +1.55 s |
| … at 130 (sliding) | 124 km/h | +1, FL Fx 1.497 / 1.503 | 0.005–0.017 / 0.007–0.16 / 0.18–1.28 m | wall at +1.3 s |
| lift-off mid-corner (gas off at 1.5 s) | 85–89 km/h | +1, FL Fx 0.98 / 1.00 | 0.005–0.010 / 0.010–0.018 / 0.005–0.064 m | wall at +1.6–1.7 s |
| handbrake turn (left + handbrake 1.0–1.6 s) | 91 km/h | +1, front Fx −0.87 / −0.61 in a disengaging shift | 0.06–0.07 / 0.07–0.13 / 0.14–0.35 m | wall at +1.9 s |
| launch out of a slow corner (gas at 1 s) | 16 km/h | +1, FL Fx 0.27 / 0.32 | 0.006–0.018 / 0.010–0.026 / 0.06–0.11 m | wall at +3.4 s |

Yaw rate agreed to 0.002 rad/s or better through the first second in every case. Without the
tyre state (the same native runs, `control.txt`) the cars were 0.19 / 0.19 / 0.35 / 0.40 / 0.30 m
apart at 1 s in braking / turn 60 / 90 / 130 / lift-off; the handbrake and launch cases were the same
either way (their first difference is the drivetrain's).

**The original repeats itself.** Two runs that reached the transplant frame in the same state
(brake 1 and 2; turn90 1 and 3; hand90 2 and 3) are identical bit for bit in every recorded
physics field to the end, except the drivetrain's random shift offsets and the car's random
state (each run's own draws) and, in turn90, the steering's input mean (above). Runs whose start
differed (the teleport hold is not repeatable to the millimetre) are listed with their spread
in `cases.txt`.

**Verdicts.** The tyre model (slip, lock test, sliding and gripping forces, friction circle,
rolling constraint, wheel spin) runs as the original's from the same state: no rule to port was
found here. What parts the runs first is the drivetrain inside a gear change (all frame-52
transplants sit in one: the clutch engaging, or disengaging in the handbrake case), then the
wall contacts. The three drivetrain differences this left are explained and ported in section 6.

## 5. Car against car (original side only)

Staged against a TRAFFIC car (it keeps its lane and does not pursue): `[start] kind =
"traffic_approach"` in `native.py` picks the nearest traffic car driving (world list slots from
+0x18, owner kind +0x24 = 4, over 8 m/s), puts the player `back` m behind it on its travel and
`side` m to its left, and the `[start] speed` roll sets the player going at frame 30 (its speed
+ `closing`, or `speed`). Scenarios `carcar_rear30` (10 m behind, +30 km/h, throttle held) and
`carcar_side150` (25 m behind, 2.2 m left, 150 km/h, right from 1.15 to 1.45 s). Contact = the
first frame the traffic car's |v| changes by over 0.5 m/s; yaw rate = the heading's change per
second (`carcar-original.txt`):

| run | traffic car | closing at contact | player |v| at +0.5 / 1 / 2 s | traffic |v| at +0.5 / 1 / 2 s |
|---|---|---|---|---|
| rear 1 | MINIVAN | 34.5 km/h | 52.0 / 45.0 / 44.5 | 52.5 / 46.8 / 42.8 |
| rear 2 | COUPE | 52.7 | 63.7 / 66.9 / 70.6 | 64.8 / 64.7 / 59.1 |
| rear 3 | MINIVAN | 54.6 | 61.8 / 55.7 / 14.0 | 58.9 / 52.4 / 14.9 |
| side 1 | MINIVAN | 118.1 | 146.2 / 148.3 / 150.7 | 23.3 / 4.8 / 12.3 |
| side 2 | PANELVAN | 99.3 | 68.8 / 32.4 / 12.6 | 68.4 / 31.4 / 12.4 |
| side 3 | HATCHBACK02 | 109.2 | 93.1 / 92.9 / 79.4 | 94.1 / 84.2 / 60.0 |

The original's pick is a different traffic car each run (its traffic is its own draw) and the
traffic car's speed changes before the contact (rear runs: picked at 37.5 / 54.0 / 54.2 km/h,
43.6 / 36.2 / 34.3 km/h at the contact, so the closing speed came out 35–55 km/h), so
these three runs per case are three different encounters, not repeats. **Open**: the clone's side
needs the original's traffic car (model, lane, pose, speed) placed on its own traffic at the
transplant frame; the clone's traffic is its own pool, and no plan line places one yet.

## 6. The drivetrain: the force pass at the sub-step's end (ported)

The original's mover takes its force pass at the end of every sub-step, after the integration and
the tidy-ups (0x5B04E0 → 0x5B0530), and runs the frame's pedal, brake, steering and grip set-up
before its sub-step loop (0x5B72E0); the clone's sub-step had the pass at its start. Traced in
docs/formats.md (**Order of a frame and a sub-step**). What it explained (the same native runs,
clone side again; raw table `docs/evidence/cornering-parity/drivetrain-order.txt`):

* **The lock-up frame** (turn60-1, frame 53): the original's input shaft equals the engine
  (251.46) and its front wheels are equal (48.7855) because the force pass after the lock (0x5AA840
  → 0x5A12E0: locked, gas over 0.2, the driven slip over 0.1) has already set them to the engine;
  the clone's frame ended a pass earlier (250.47; 48.62 / 48.57) and matched them in the next
  frame's first sub-step. Not a different rule, a sampling phase; with the port the clone's frame
  ends there too: 251.458 = 251.458, both wheels 48.7851.
* **The disengaging shift** (hand90-2): the transplant's doubled reaction (section 1). Without it
  the engine reads 436.6425 against the original's 436.644 after the frame (before 437.388).
* **A plain locked frame** (120 km/h, full throttle, no shift; the barrel runs' transplant): the
  doubled reaction too. Frame +1 engine 589.789 against 589.632 rad/s before, 589.787 after; the
  wheel spins, vertical speed and roll rate agree as well (evidence file).
* **The unequal front wheels at a downshift against the wall** (bench_wall_hold, three new native
  runs): the wheels' own grip, not the differential (docs/formats.md, **Left/right split**): the
  wheel hop's grip (+0x474 = 0.914, count 3) leaves FL with 1 − 0.086·0.65 and FR with
  1 − 0.086·0.35 of its grip. The old runs predate `tyres_at` (the hop was not carried); with it
  the clone's fronts pull unequally as well: −6.428 / −6.602 (base), −6.453 / −6.628 (after the
  port), the original −6.454 / −6.634, yaw rate −0.00140 in both five frames later.

Distance apart at 1 / 2 / 5 s (m) and the first quantity beyond noise (cmp.py tolerances; `pos`
is left out, the native tsv's printed digits): the base, and with the harness fix and the port
(the harness fix alone is in the evidence file):

| case (3 runs) | base | port | first difference after the port |
|---|---|---|---|
| full braking | 0.005 / 0.017 / 0.24–0.50 | 0.003 / 0.011–0.012 / 0.003–0.008 | +1, front Fx −2.9776 / −2.9764 (0.04 %) |
| turn 60 | 0.004–0.018 / 0.007–0.039 / 0.07–0.17 | 0.001–0.004 / 0.001–0.006 / 0.01–0.10 | +1, FL Fx 0.918 / 0.920 |
| turn 90 (wall at 1.55 s) | 0.003–0.007 / 0.007–0.012 / 0.06–0.31 | 0.003–0.005 / 0.02–0.06 / 0.86–3.4 | +1–2, 0.2 % |
| turn 130 (wall at 1.3 s) | 0.005–0.017 / 0.08–0.15 / 0.75–1.01 | 0.007–0.010 / 0.005–0.08 / 0.05–1.19 | +1, FR Fy 0.05 % |
| lift-off | 0.005–0.010 / 0.007–0.017 / 0.009–0.033 | 0.002–0.004 / 0.000–0.006 / 0.011–0.105 | +1, FL Fx 1–3 % |
| handbrake turn | 0.06–0.07 / 0.07–0.13 / 0.15–0.29 | 0.007–0.010 / 0.024–0.037 / 0.14–0.28 | +9 (the keys' frame), FL Fx 0.7 % |
| launch | 0.006–0.018 / 0.010–0.026 / 0.06–0.18 | 0.006–0.012 / 0.007–0.018 / 0.012–0.144 | +1, FL Fx 0.274 / 0.287 |
| wall hold (hit at 1.07 s) | 0.017–0.023 / 0.004 / 0.03–0.11 | 0.001–0.004 / 0.007–0.009 / 0.15 | +10, the downshift: FR Fx 0.1 % |

Up to the first wall contact every case is as close or closer; after a contact the runs part
within a few frames either way (the contacts' push-outs, docs/prop-hits.md §7c–7d: the wall hold
agrees through the hit at frame 116 and parts at frame 120, where the original's yaw rate turns
−0.0032 and the clone's +0.0021 in one run, −0.0029 / −0.0032 in the others); which way the car
pivots off the wall is decided there, not in the drivetrain (open, collision code).
Left at +1: the launch's 5 % on the front wheels' Fx in an engaging clutch (open) and the 0.7 %
at the handbrake turn's key frame (the steering's running means are not carried, section 1).

## Tools

* `tools/parity/fields.toml`: `lock_*`, `ntq_*`, `torq_*`, `steer_drift`, `steer_avg`,
  `steer_rate`, `steer_lim`, `steer_last`, `hb_ramp`, `hop_t`, `hop_calm`, `rb_damp`.
* `parity.py` writes the plan line `tyres_at F` (23 numbers, `Plan::tyres_at`, `put_tyres`) with
  `transplant_full` when the run recorded those fields; `PARITY_NO_TYRES=1` leaves it out (the
  control).
