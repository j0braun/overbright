# Stage-4 drags with the finished-career save's Skyline, original vs clone (2026-11-05)

Issue: the stage 4 drag did not look native, and a blue-zone shift got the short shift
message. **Verified** = traced in SPEED2.EXE, **observed** =
read under Wine, **inferred** / **open** as marked. Evidence: the maintainer's private captures.

## Verdict

* **"Short Shift!" under a blue light is native.** The tachometer's light turns blue
  max(500, w) rpm under the shift point, but an up-shift grades above 0 only from 0.95 w
  under it (w the gear's drag window). For the finished-career save's Skyline (class 0) that is a 160–367 rpm
  band per gear where the original itself shows the blue light and then "Short Shift!"
  (observed: 8939–9078 rpm in 1st–4th, screenshots below).
* **The clone's light read another rpm than the grade** (fixed). The light and the needle's
  blue tint were computed on the engine's rpm and the drivetrain's gear; the original computes
  both on the car's shown rpm (+0x440, the same rpm the grade reads) and on the
  tachometer's gear of the last update (HUD +0x80). While revving, the engine leads the
  shown rpm, so the clone's light went blue / green / red a few frames before the grade
  agreed: a shift on a fresh green could still grade "Good", one on a fresh red "Perfect".
* **The grading itself matches**: 28 native up-shifts (every band, gears 1–5, three
  tracks) give the grade the clone's formula gives at the same shown rpm (0 mismatches).
* **Harness fix**: the clone side of the parity harness never executed a scripted
  up-shift (the key went into the held-key set only); now the press goes through the game's
  key handler. Earlier drag comparisons (drag_s1) had no clone player shifts.
* **Physics differences seen** (the clone's Skyline launched ~3 % slower, its wheel hop
  started a frame late, its driven wheels spun up over a crest): traced and fixed in the drag-physics parity,
  §5; the launch now matches to 0.2 % through the first 3–4 s.
* **To the finish** (§8): with the lane taps keyed by race coordinate the clone follows a won
  native run to the line (player within 7.3 m all race) and wins in 30.07 against the original's
  30.15, with the same reward (300 reputation, 2200 bank) and the sponsor popup after it. The lane
  table's 80 m early start is the AI racers', not the player's (fixed); HEAD START's 1 s is whole
  ticks (fixed: the nitrous refill at frame 61 as natively). The clone's rivals still crash on
  4213's lane splits where the original's do not (open).

## 1. Shift grading and the light (verified, observed)

### The trace

* **Grade** 0x594F30(rpm, gear, flag) on the drivetrain (mover +0x4C), called with the flag
  by 0x5A1710 (drag only, 0x89E7D8 / 0x890108; target +0x38 ≥ gear +0x3C) for each gear
  request, result in +0x68. early = up(gear) − rpm (up = +0x84 + 4·gear), w = 0x7FBF60[class
  +0xF0][gear] × +0xE4 (1.0 in the career; 0x7A19F0 by difficulty in Quick Race):
  w ≤ 0 → 0; early < 0 → 1 if |early| < (1 − 0.95)·w else −1; else q = 1 − early / (0.95 w)
  (0x7FBF5C = 0.95): q ≤ 0 → 0, with the flag q < 0.5 → 0.5, else 0.85; without the flag q.
* **Message** (0x4C7090, 0x4C73B6..0x4C7541) once per gear change of +0x434: grade in (0, 1]:
  > 0.5 "Perfect Shift!" 0x0598B065, else "Good Shift!" 0x27D2DD45; going up from a forward
  gear with the race on: 0 "Short Shift!" 0x2303D76C, < 0 "Over Rev!" 0x27D75CD8.
* **Light and needle** (0x4C777D..0x4C7A5F): the rpm is [esp+0x10] = car +0x440 (or the
  override 0x7FBED8 when it is not 100; 0x4C711A..0x4C713E), the gear HUD +0x80 (the car's
  +0x434 stored at the end of the last update, 0x4C7A6C), rating r = 0x594F30(rpm, +0x80, 0).
  Off (0x4C784C → 0x4C7857: own needle colour, light hidden) when shifting (0x5ECD80: +0x48)
  or when r = 0 and rpm < 0x5ECD40(+0x80) − max(500, w) (0x795AE8 = 500); else r = −1 red
  (needle 0xFF800000, PulseRed), r > 0.5 green (0xFF00FF00, PulseGreen), else blue
  (PulseBlue; needle blended toward (0, 128, 192) by (rpm − (up − W)) / (0.5 W), W =
  max(500, w), 0x4C7A13).

### The finished-career save's Skyline in the career (class 0, +0xE4 = 1, up-shift 9400 in gears 1–5, observed)

| gear | w | light blue from | "Short" below | "Good" | "Perfect" (light green) | "Over Rev" (red) from |
|---|---|---|---|---|---|---|
| 1st | 360 | 8900 | 9058 | 9058–9229 | 9229–9418 | 9418 |
| 2nd | 175 | 8900 | 9234 | 9234–9317 | 9317–9409 | 9409 |
| 3rd | 160 | 8900 | 9248 | 9248–9324 | 9324–9408 | 9408 |
| 4th, 5th | 140 | 8900 | 9267 | 9267–9334 | 9334–9407 | 9407 |

(rpm = the shown rpm on the frame before the request; at q exactly 0.5 the light is still
blue and the grade Perfect.) Nitrous does not enter the grade or the light (no nitrous term
in 0x594F30 / 0x4C7090; the nitrous run below confirms the bands).

### Observed under Wine (the maintainer's tool `dragshift.py`, grades.tsv)

S4_H_DRAG_15 (track 4213) and S4_H_DRAG_17 (4404), the player shifting with the real Left
Shift key at chosen rpm, every request graded by the original (+0x68) and the formula:

| run | frame | gear | w | shown rpm | q | light | formula | original +0x68 | message | clone rpm / grade (same frame, replayed keys) |
|---|---|---|---|---|---|---|---|---|---|---|
| h15_b | 12 | 1 | 360 | 8938.9 | 0.000 | blue | 0 | 0 | Short | 8916 / 0 |
| h15_b | 314 | 2 | 175 | 9022.1 | 0.000 | blue | 0 | 0 | Short | 8974 / 0 |
| h15_b | 446 | 3 | 160 | 9078.0 | 0.000 | blue | 0 | 0 | Short | 9909 / -1 |
| h15_b | 514 | 4 | 140 | 9068.0 | 0.000 | blue | 0 | 0 | Short | 7438 / 0 |
| h15_c | 124 | 1 | 360 | 9150.9 | 0.272 | blue | 0.5 | 0.5 | Good | 9263 / 0.85 |
| h15_c | 245 | 2 | 175 | 9283.5 | 0.300 | blue | 0.5 | 0.5 | Good | 9608 / -1 |
| h15_c | 381 | 3 | 160 | 9302.6 | 0.359 | blue | 0.5 | 0.5 | Good | 9816 / -1 |
| h15_c | 416 | 4 | 140 | 9432.2 | -1.000 | red | -1 | -1 | Over Rev | 7945 / 0 |
| h15_d | 125 | 1 | 360 | 9340.8 | 0.827 | green | 0.85 | 0.85 | Perfect | 9035 / 0 |
| h15_d | 231 | 2 | 175 | 9346.5 | 0.678 | green | 0.85 | 0.85 | Perfect | 8451 / 0 |
| h15_d | 368 | 3 | 160 | 9388.7 | 0.926 | green | 0.85 | 0.85 | Perfect | 7932 / 0 |
| h15_d | 396 | 4 | 140 | 9475.9 | -1.000 | red | -1 | -1 | Over Rev | 7188 / 0 |
| h15_e | 126 | 1 | 360 | 9449.6 | -1.000 | red | -1 | -1 | Over Rev | 9232 / 0.85 |
| h15_e | 280 | 2 | 175 | 9426.3 | -1.000 | red | -1 | -1 | Over Rev | 9750 / -1 |
| h15_e | 386 | 3 | 160 | 9494.1 | -1.000 | red | -1 | -1 | Over Rev | 9738 / -1 |
| h15_f | 133 | 1 | 360 | 9808.5 | -1.000 | red | -1 | -1 | Over Rev | 9569 / -1 |
| h15_f | 300 | 2 | 175 | 9900.4 | -1.000 | red | -1 | -1 | Over Rev | 9572 / -1 |
| h15_f | 439 | 3 | 160 | 9855.8 | -1.000 | red | -1 | -1 | Over Rev | 9895 / -1 |
| h15_n | 118 | 1 | 360 | 9262.5 | 0.598 | green | 0.85 | 0.85 | Perfect | 9190 / 0.5 |
| h15_n | 223 | 2 | 175 | 9316.9 | 0.500 | green | 0.85 | 0.85 | Perfect | 8954 / 0 |
| h15_n | 351 | 3 | 160 | 9435.6 | -1.000 | red | -1 | -1 | Over Rev | 8010 / 0 |
| h17_a | 132 | 1 | 360 | 9254.8 | 0.575 | green | 0.85 | 0.85 | Perfect | 9199 / 0.5 |
| h17_a | 242 | 2 | 175 | 9351.4 | 0.708 | green | 0.85 | 0.85 | Perfect | 8975 / 0 |
| h17_a | 443 | 3 | 160 | 9347.7 | 0.656 | green | 0.85 | 0.85 | Perfect | 8790 / 0 |
| d9_a | 121 | 1 | 360 | 9408.9 | 1.000 | green | 1 | 1 | Perfect | 9391 / 0.85 |
| d9_a | 221 | 2 | 175 | 9386.2 | 0.917 | green | 0.85 | 0.85 | Perfect | 9278 / 0.5 |
| d9_a | 411 | 3 | 160 | 9390.5 | 0.937 | green | 0.85 | 0.85 | Perfect | 9162 / 0 |
| d9_a | 736 | 4 | 140 | 9380.9 | 0.856 | green | 0.85 | 0.85 | Perfect | 9072 / 0 |

28 up-shifts on three tracks (h15 = S4_H_DRAG_15, h17 = S4_H_DRAG_17, d9 = S4_DRAG_9; driver deltas b 450–330, c 250–110, d 120–45, e −5, f −400, n / n2 150–60 with nitrous, h17 150–60, d9 60–20 rpm under the point): **0 mismatches** between the original's grade and the formula, the message on every screenshot as the grade says. The clone column is the clone on the same key frames; its rpm differs from the physics (§3), not the grading.

Screenshots 14 frames after the press (`hud-*.png`): blue light + blue needle at 9000 →
"Short Shift!" (h15_b), blue at 9284 → "Good Shift!" (h15_c), green at 9347 → "Perfect
Shift!" (h15_d), red needle at 9426 → "Over Rev!" and the orange gear digit (clutch at
× 0.35, h15_e).

### Clone changes

* `Drivetrain::drag_shift_light(h, gear)` / `drag_needle_blend(h, gear)` (src/handling/):
  the shown rpm (`choice_rpm`, as the grade) and the gear passed in; `Hud::drag_last_gear`
  (+0x80) is passed from `src/bin/game.rs`.
* Tests: `handling::tests::drag_light_reads_the_shown_rpm_and_blue_starts_before_the_grade`
  (the band where the light is blue and the shift Short; the light on the shown rpm and the
  HUD gear), `handling::tests::drag_grades_match_the_originals_skyline_shifts` (the 18 native
  grades of h15_b..f replayed).

## 2. Method

* **Save.** the player's finished-career save has finished the career: no stage-4 drag
  is open in it. `[start] garage_from` (the maintainer's tool `savepatch.py`) puts the finished-career save's whole garage
  (payload 0..0x993E: part lists, cars, the five career slots, ranked cars) and current slot
  (career +0x8740) into `stage4_start.sav` (the original's stage-4 career),
  so both games race the finished-career save's Skyline (all upgrades, class 0) in stage-4 events. Rivals are
  matched to that car by each game (0x532020).
* **Native driver**: the private tool `dragshift.py SCEN OUT --deltas <list of shift times> [--nitro T]`:
  the harness's start (boot, Resume, put in the trigger, accept, held at the countdown), then
  real X key presses on the private Xvfb (Up held from the countdown, Left Shift, Left Alt),
  not the cave's handler events (the injected throttle is overridden in a drag,
  parity-harness.md "Drag input"). The cave records in lockstep; after each record Python
  reads gear, shown rpm and the gear's up-shift rpm and presses Left Shift once the shown rpm
  reaches up − D(gear). A press is seen by the game's update 2 frames later. It stops when
  the race status leaves 4 or the player's gear drops (the AI takes the car at a total).
  Screenshots at each press and 14 frames after. `--replan OUT` writes the press frames
  into native.json's plan for `parity.py clone`.
* **Clone**: `parity.py clone` on the same frames (now with real shifts), grid and rival
  models forced to the original's.
* Scenarios `drag_s4_h15_finished`, `drag_s4_h17_finished`, `drag_s4_d9_finished`; new harness
  fields `dt_up6`, `dt_up7`, `heat` (engine +0x8C) and clone accessors for `dt_up2..7`,
  `dt_e4`, `dt_f0`; the private tool `shiftgrades.py OUT…` (the table above).
* Each run ends when the player hits traffic or a wall at full speed (no steering in the
  driver): 8–13 s; the finish, results, ET and rewards were therefore not reached (**open**).
* Wine: a private Wine copy, display :239, wined3d; raw runs kept with the maintainer's private evidence.

## 3. Per-tick comparisons (three stage-4 drags)

Player (SKYLINE) speed in m/s, original / clone, same key frames (`pertick-summary.txt`,
`launch-crest-frames.txt`; position error by second in each run's report):

| run (track) | 1 s | 2 s | 5 s | 8 s | first hit (s) | player position error at 2 / 4 / 6 s (m) |
|---|---|---|---|---|---|---|
| h15_e (4213) | 11.8 / 11.5 | 23.3 / 22.7 | 39.6 / 41.2 | 45.3 / 48.1 | 10.05 / 9.95 | 1.3 / 6.4 / 8.0 |
| h17_a (4404) | 10.9 / 10.6 | 21.9 / 21.7 | 45.3 / 41.3 | 57.3 / 52.1 | 8.13 / 8.52 | 0.8 / 3.1 / 10.5 |
| d9_a (4222) | 12.2 / 11.9 | 23.9 / 23.8 | 46.9 / 45.3 | 59.0 / 55.9 | 14.22 / none | 2.0 / 1.6 / 4.6 |
| h15 b, c, d, f, n | 8.9–12.2 / 8.7–11.9 | 16.1–23.7 / 15.8–23.4 | | | 9.6–13.3 / 9.7–13.2 | |

The clone is 2–3 % slower at 1–2 s in every run; from the first shifts on the gap follows
the grades (on the replayed frames the clone's rpm is lower, so its shifts grade lower and
get no shift bonus: e.g. h17 Perfect / Perfect / Perfect natively, Good / Short / Short in the
clone).

* **Countdown and launch.** The first frames match to < 1 % (rpm 9900 vs 9917 held at the
  limiter, the same clutch ramp 0.084 / 0.418 / 0.916, heat 0.0225 identical to 4 digits,
  1st engaged on frame 4 on both sides). From frame 4 the clone's driven wheels turn at
  once (0.10 rad/s) where the original's stay at 0 for that frame with twice the wheel
  torque (+0x108 3.28 vs 1.63); on frame 5 the original's rear slip is 0.70 m/s and starts
  the wheel hop (car +0x46C = 3, grip +0x474 = 0.195) while the clone's 0.48 does not, and
  its hop starts a frame later at slip 4.3 (count 4, grip 0.71). The clone is then 2–3 %
  slower through 1st gear (11.2–11.5 vs 11.7–11.8 m/s at 1 s). **Open**: the frame-4 wheel
  release and the drive torque's averaging (see docs/tuning-physics-parity.md "Rear-pair
  spin-up").
* **Over a crest** (h15, ~5.5 s and ~6.8 s): with all four wheels off the ground the
  original's wheels keep road speed (sliding flag +0x160 stays 0, slip 0.14, so 0x59D360
  takes the slip out) and its engine revs free of them (input shaft constant); the clone had
  one wheel down a frame longer, its rear slip jumped from 0.2 to 2.3 in one frame (the
  driven wheels set to the engine, `match_wheels`, is the **inferred** cause), the slip
  passed 0.5, the tyres slid and the wheels spun up to the engine, so it lands with more
  rpm / speed. The vertical motion before the crest already differs (vel.z −1.44 vs +0.09 at
  frame 330). **Open** (suspension over the crest).
* **Sliding flag**: the clone sets the wheel's sliding flag when the friction circle clamps
  (src/drive/, `state.sliding = true`); 0x59D610 does not write +0x160 (it scales +0x124 /
  +0x128 and sets +0x170 only; +0x160 is written by the tyre force 0x5A8577 / 0x5A8616 and the
  reset 0x59CAC6 / 0x59F44D). Only the tyre smoke reads the clone's flag (render side; left
  as is, **open** for the particles unit).
* **Rivals.** Launch and first shifts follow the original (e.g. COROLLA 2→3 at frame 148 vs
  153, both graded 0.85), the AI shift offsets are each game's own draws (as the drag shift sweep). One rival
  (RX7, h15_e) came out class 1 natively and class 2 in the clone: the performance match
  (0x532020) of a rival to an upgraded Skyline is **open**. Rival launch speeds at 1 s:
  0.1–2.1 m/s lower in the clone (aistats).
* **Heat / blow-up.** Heat +0x8C rises 0.45/s over the redline and falls 0.1/s, identical
  per frame in the matched stretches; no run blew the engine (the over-rev runs shift).
* **Traffic, lane changes, collisions.** Both players hit traffic or the wall at a similar
  time (h15_e 10.05 vs 9.95 s, h15_f 13.33 vs 13.15 s; status 9 natively). Lane AI and
  traffic placement were not compared (**open**).
* **HUD, camera, audio** beyond the shift message / light: not compared this round (**open**).

## 4. Nitrous

Runs h15_n (Left Alt tapped 0.1 s at 4 s) and h15_n2 (held from 3.0 to 9.0 s). Natively the
held shot accelerates the car visibly (32.8 → 49.1 m/s from 3.0 to 4.25 s, with a Perfect
3→4 shift inside) and the N2O gauge drains over the hold (`nitro-gauge-h15_n2.jpg`: full at
0.4 s, a little lower at 1.6–1.8 s, about half at 4.2–4.5 s; the HUD was not drawn on two
shots). The grades with the shot running follow the same
bands (h15_n2: Perfect at q 0.65 / 0.84 / 0.71, Over Rev past the point; no nitrous term in
0x594F30 or 0x4C7090, verified). The clone's shot (key held on the same frames) starts and
ends with the key (tank 25600 ticks = 6.4 s for this car; `left` 5884 after the 6.0 s hold).
The original's tank was not read (car +0x41C, the harness's `nos_left`, holds no tank value
for this car: it reads float bit patterns), so the shot's length and its boost per frame
are **open**; the speed comparison is confounded by the different shift grades.

## 5. Follow-ups (2026-11-05)

New tools: the maintainer's tool `subtrap.py` + `subtrap.s` stop the original's drive mover before
(A, 0x5B73D8) and after (B, 0x5B73E3) each integrator call 0x5AA490 of one car (a code cave at
0x782F00, control block 0x8BBE00; only the car whose mover + 8 is in EBX) and read its body,
engine, drivetrain, wheels and suspension there; `dragshift.py --substeps N` arms it for the
first N frames, `--lanes` taps Left / Right around cars in the player's lane, `--after S`
takes screenshots after the race. The clone prints the same points with
`NFSU2_SUBSTEP_DUMP=<car name>` (`CarState::debug_substep`, not native, off by default).
Evidence: the maintainer's private captures (launch-substeps-n1.txt, speeds-after.txt).
a private Wine copy, displays :247 / :248, raw runs kept with the maintainer's private evidence (n1: sub-steps + lanes; n2: lanes + nitrous 3.0–7.0 s).

### 5.1 The launch (verified, observed; fixed)

Three causes, each traced and checked against the sub-step capture (n1, frames 1–14):

* **The rest test reads the car's gear of the last frame.** Car_RestClassify 0x5992C0
  holds a car (class 2: velocity, momentum and, with a wheel down, wheel spins zeroed by
  0x5A5E30) when gas ≤ 0.3 **or car +0x434 = 1 (neutral)** or no wheel down (or +0x428 < 3:
  the count of attached wheels, 4 on a whole car, 0x5920C0 / 0x59F823). Car +0x434 is the
  drivetrain's gear (+0x3C) copied by Car_SpeedAndWheelSpin (0x5A5924), which 0x5B72E0
  calls once, after the sub-step loop (0x5B74E1). So in the frame in which the clutch puts
  1st in (Changing → Engaging) the car is still held: natively frame 4 ends with the drive
  torque sent (+0x70 0.96) but speed, input shaft and wheel spins exactly 0. The clone
  tested its live gear and moved off one frame early. Fix: `CarState::frame_gear`, the
  drivetrain gear at the mover frame's start, for `rest_class`.
* **The snapshot take frees locked wheels.** The front wheels lock (0x59D410: brakes over
  r·road + 0.1|ω|, and over 1) as the car brakes to a stop on the line; at a standstill the
  lock test is skipped (slip and side speed 0, 0x5A8474), so the flag stays. The wheel's
  serialiser Wheel_Serialize 0x59C990 zeroes, in its take / restore branch, +0x108,
  **+0x10C (lock, 0x59CA6A)**, +0x124 / +0x128, **+0x160 (sliding, 0x59CAC6)** and +0x164; the
  replay's DUMMY_SNAPSHOT (every 100 world updates, also in the countdown) runs it. Observed:
  the original's four wheels unlocked at the green; the clone's front pair locked (from the
  roll to the line), so its AWD Skyline lost the front drive (0x594A40 adds no torque to a
  locked wheel) and launched slower. Fix: `CarState::snapshot_take` clears `locked` and
  `sliding`.
* **The per-frame readers see the end state's slip.** The sub-step capture shows the
  original's force pass written after each integration (at B the clutch torque +0x70 is
  12 × the clutch the integration just reached, +0x118 = r·ω − v of the end state): the
  integrator evaluates the forces for the next sub-step after integrating. The clone
  evaluates them at a sub-step's start, which integrates the same, but its wheel slip at
  a frame's end was one sub-step old (frame 5: 0.47 against the original's 0.70), so the
  wheel hop (0x5B6310, from the frame's start, slip × load / 3 over 0.5) began a frame
  late. Fix: `CarState::end_slip` recomputes each wheel's slip (and side speed) from the
  end state after the sub-steps. The hop's load term (+0x164) is still the last sub-step's
  (2.39 against 2.58 at frame 5), so the hop's count can differ by one (3 natively, 4 in
  the clone at the n1 launch; the grip 0.1952 / 0.1954 matches): **open**.
* Result (speeds-after.txt): at 1 / 2 / 3 s 12.23 / 23.63 / 33.85 m/s natively against
  12.23 / 23.62 / 33.73 (n1); h17_a matches within 0.3 % to 8 s; the remaining splits come
  from shift grades on a knife edge (d9_a: the clone 40 rpm higher at the 1→2 press,
  9449 against 9408, past the 9418 over-rev bound, so its 2nd gear clutch runs at × 0.35).
* The 2× drive torque the first stage-4 pass saw on frame 4 is the record's timing, not a physics difference:
  the record shows the forces for the next sub-step (the clone's first sub-step of frame 5
  sends exactly the original's recorded 0.9628).

### 5.2 Over a crest (traced; airborne coupling now matches)

0x5AA840 sets the driven wheels to the engine (0x5A12E0) only with a wheel down by the
physics view's count +0x3E4, which Car_SpeedAndWheelSpin publishes once a frame
(0x5A57ED..0x5A581D; the clone's `aero_down`, as before); the tyre force 0x5A8290 runs for
all four wheels (0x5ABE30 loop) and sets +0x160 from |slip| > 0.5 / the slip angle / the lock,
and 0x59D360 removes a non-sliding wheel's slip within the sub-step (0x59D3D4..0x59D406).
With the launch fixed the crest of h15_b now runs alike: all four wheels off from frame 334
on both sides, the engine revving free (801 / 799 rad/s at 340) and the input shaft constant
(696 / 697). The landing still differs (**open**): the clone's front pair touches down two
frames earlier (342 against 344) with the rear slip at 0.21 (the original's 0.14 in the air),
so the clone's mean driven slip passes 0.1 with a wheel down and 0x5A12E0 sets the wheels
to the free-revving engine; the original's stays under 0.1. The car's pitch in the air
(vel.z −4.99 against −4.72 at 340) is the difference, from the suspension at the crest.

### 5.3 The sliding flag (verified; fixed)

Tyre_FrictionCircle 0x59D610 scales +0x124 / +0x128 and writes +0x170 only; +0x160 comes
from the tyre force (0x5A8577 / 0x5A8616) and the serialiser. The clone no longer sets
`sliding` when the circle clamps (src/drive/); the tyre smoke reads the native flag.

### 5.4 Rivals (verified, observed; class match fixed)

Race_SetupOpponents 0x5260C0 for a skill source 0 / 3 entrant: the rating is the player's
car's preset range lerped by skill / 10 and the target score −1 (0x526309), the spread lerp
only when 0x89E7D8 and 0x890108 are both 0 (0x5262FA, 0x52631D..0x526326): in a drag race
0x5B6AE0 gets −1 and keeps the nearest preset, without the second fit. Observed: the finished-career save's
Skyline range 273.8–591.0 at skill 4 → 400.5; the original's stage-4 rivals rated 397.5
(COROLLA), 398.4 (PEUGOT), 402.4 (RX7), 403.3 (IS300), 399.6 (CELICA), 400.7 (ECLIPSE), …
(racer info +0x20 performance, rating +0x28): every one the nearest preset. The clone ran
the second fit (364–388, all class 2). Fix: `fit_performance(.., drag)`; the clone's rivals
now come out at the same ratings and classes (RX7 402.4 class 1, IS300 403.3 class 1).
Rival launches after the fixes (h15_e): within 0.02 m/s of the original at 0.1–0.4 s
(COROLLA 0.184 / 0.181, 0.804 / 0.816, 2.309 / 2.313 m/s at frames 6 / 12 / 24); their wheel
hop begins on the same frame but its count is 3 in the clone against 4 natively for two of
them (the hop's load term, 5.1). Pre-green revving: the rivals' engine at the green
8491 / 8414 rpm (COROLLA), 7969 / 7969, 8928 / 8936 — the countdown revving matches to 1 %.
The AI shift offsets stay each game's own random draws.

### 5.5 Nitrous (verified, observed)

The tank is the physics view's +0x41C = **car +0x45C** (ticks, 4000 a second), the running
shot's start tick view +0x420 = car +0x460 (Nitrous_SecondsLeft 0x592490 and
Engine_GetNitrousStage 0x59FA40 run on ECX = mover +0x58 = car + 0x40, 0x5ABD72); the
harness read car +0x41C (fields.toml `nos_left` now 0x45C, `nos_start` 0x460). n2 (Left Alt
held 3.0–7.0 s): natively 24000 at the green, full 25600 from frame 61, the shot from frame
182 (start tick 53248) to 421, 12775 left; the clone 24000, 25600 from frame 60, 12772 left
at 422 — the shot's length matches to 3 ticks, the refill and the end one frame apart
(the refill at ~1 s is HEAD START, now at frame 61 in both: 8.5). Speeds with the shot: 46.70 / 46.53 m/s at
4 s, 51.64 / 50.95 at 5 s (a shift grade in between), 55.49 / 56.30 at 7.5 s.

### 5.6 Finish, lanes, traffic, camera, audio (open)

* **Finish.** `--lanes` changes lanes when a world-list car is in the player's lane ahead;
  in both stage-4 runs the original's world list held only the four racers through the race
  (+0x10 = 4: no traffic there), so no lane change fired, and the player was totaled at
  58 m/s (n1 9.8 s, n2 11.4 s; `after-*.png`: "Totaled!"). What it hit is not identified
  (no traffic car in the list; the steering swung from −0.8 to −3.3 just before, n2 frames
  640–672). The finish, ET, quarter times, results, rewards and reputation stay **open**.
* **Traffic.** The clone's same race had 12 traffic cars (clone.tsv.traffic) and its player
  crashed at 9.0 s (n2); the original's world list had none. Whether the original places
  drag traffic in another list, or none in these events, is **open** (docs/formats.md "Drag
  placement" describes a fill into the world list for a Quick Race drag).
* Lane AI, camera and audio were not compared this round (**open**).

## 6. 2026-11-05: drag traffic, the lane table's coordinate, the finish, the hop's load term

a private Wine copy, display :310, wined3d; raw runs kept with the maintainer's private evidence
(p1 probe + crash shots, p3 lane plan, p4 the finish). Evidence: the maintainer's private captures.
New harness options: `dragshift.py --probe` (the traffic pool and world list every 30 frames),
`--shots-from F`, `--seconds S`, `--taps X,Y,key;…`, `--lane-map FILE` (lane planning, below); parity
fields `rcoord` (car +0x8FC), `dlane` / `dreq` (controller +0x30C / +0x7B4, **not** the lane: +0x30C read
2.99999 as a float all race, so the lane record is elsewhere for the DragCarController); clone knob
`NFSU2_DRAG_LANE_LOG=1` (the player's lane, the lanes of its point's record, where each ends, its
lateral off each, per frame).

### 6.1 Drag traffic (verified, observed; fixed)

* **Observed** (p1, `--probe`, `traffic-probe-p1.txt`): S4_H_DRAG_15 sets level 1 (0x89E7A8 = event
  +0x36), the race road is built (0x88F2BE = 1), the car table holds 16 entries (0x89E814: 1 human, 3 AI
  racers, **12 traffic**, +4 = 3 — the pool 0x4FE9F0 made them: track 4213's byte +0xEC + 1 + 2·1 = 12),
  but the world list holds the 4 racers (+0x10 = +0x18 = 4) from the accept to the end.
* **Verified**: the set-up's fill 0x6065C0 links a traffic entry's car into the world list only when
  0x42F7A0 returns it (0x6066FF..0x606713); the placer 0x42F910 walks the world list's traffic
  (+0x18 .. +0x10, 0x42FA70..0x42FA86) and returns at once while +0x10 − +0x18 = 0 (0x42F95B); no other
  caller links race traffic (0x42F7A0 has one caller). So a pool car the fill cannot place never
  appears. 0x42E4A0's points must carry flag bit 25 or 26 (0x417C60): **no race-record point of
  Routes4213B has either** (nor 4211, 4212, 4221, 4222, 4223 either way; 4201–4203, 4401–4404 have
  them), so those six drag tracks (the 500 m start-gap ones) run without traffic.
* **Clone**: it found no point either, but placed the 12 cars after the green from its placer. Now
  `traffic_fill_apply` drops the pool cars the fill could not place (`t.cars.retain`), so its placer never sees
  them; test `drag_fill_finds_no_point_on_4213_backward`; the maintainer's probe `drag_traffic_pool` lists every career
  drag's pool. p1 clone: no traffic, and the player now meets the same obstacle as the original.

### 6.2 What the player hit (observed)

Not traffic: a freight hopper standing in the player's lane (`crash-p1.jpg`, frames 672–714). The drag
lanes end at obstacles: on 4213 backward lane 3 (record lane 7) ends at race coordinate ~815 (records
283 / 282 lack it), lane 2 at 674 / 968 / 1284 / 1548, lane 3 at 1112 / 1422 / 1675, lanes 4 and 5 in
between; the player has to change lanes before each (both games crash at the same place without:
p1 native 115.1 / −1952.5 at frame 730, clone 114.8 / −1952.1). The drag-physics parity's "Totaled!" runs hit these.

### 6.3 The lane table reads the race coordinate (verified, observed; fixed)

0x412500 hands 0x40B110 the car's **+0x8FC** (0x4125E9) and the flag "controller type 2"
(0x41263D..0x41264E: car +0x14 → +4 = 2, the player's DragCarController); 0x40B110 starts a row 80 m
(0x783CCC) early only with that flag. Observed: the player's +0x8FC on the grid 378.05 (p2, p3), the
clone's race coordinate (`Progress::along`) 377.9; the clone read the table at the list distance
(−20.6 on the grid), so its blocks fell ~400 m early (S4_H_DRAG_15: 3 → 2 refused at list distance
1219, where the original allows it), and it started every row 80 m early for the AI racers too. Now
`lane_allowed` takes `RacerRoute::race_coord` (the player's DragSteer route gets `race.me.along`, the
rivals' had it) and the 80 m only for the player (**corrected** in 8.3: the 80 m is the AI racers', owner kind 2). Tests `lane_table_reads_the_race_coordinate`,
`drag_table_blocks_changes` (AI rows).
**Checked**: the clone replaying p3's native lane taps (14 taps, frames 688–1636) drives the same lanes
for 27 s (position error under 6 m, `p3` report) and crashes where the original did (frame 1640, race
coordinate 1670, lane 3's obstacle at 1675, after the planner's 3 → 4 tap was reversed).

### 6.4 The finish (observed; clone open)

`--lane-map lanemap-4213B.json` (from the clone's lane log): each frame the player's lane is the
nearest lane centre at its place, and when that lane ends within 150 m of +0x8FC the driver taps
towards the neighbour that runs farthest, repeating that side until the car is in it. p4 (the finished-career save's
Skyline, grid lane 3, shifts 150/90/70/60/60 rpm early, 14 taps): **"Winner! Your time: 29.74"**;
Event Results 1 NAME Skyline 29.74, 2 Nigel Golf 31.00, 3 Mataio 240SX 31.75, 4 Harlan G35 35.15
(`results-p4.jpg`). p3 (totaled at 1670): 1 Harlan Celica 34.62, 2 Nigel MX-5 36.36, 3 NAME Skyline 0.00,
4 Mataio A3 0.00 — a totaled player is placed by progress, its time 0.00.
The clone side of p4 (same key frames) follows to 0.5 % for 7 s, then loses the shift grade at the
crest (below) and drives off the plan's lanes; a clone-only run with its own taps (`planrun.sh`)
finishes first at race clock 30.1 s. The clone's ET, results, rewards and reputation against p4 stay
**open** (they need the crest first). Rewards and reputation were not read natively (the recording ends
at the finish; the results screen's Continue was not pressed).

### 6.5 The crest at 4.5–7.5 s (observed; open)

p4 / its clone replay: at frame 270 (a bump before the crest, all wheels down) the original loses 2.07
m/s in one frame, the clone 1.21; over the crest (276–288) the original's rear wheels unload with the
engine's torque on them (slip 1.9 → 7.1, sliding set) while the clone's keep road speed (0.03–0.18).
The clone is 2.3 % faster after it, revs higher in the air, and at the 4 → 5 press in the second, long
jump (frames 344–444, 1.7 s airborne) its shown rpm is 9648 against 9377: Over Rev (clutch × 0.35)
instead of Perfect, so its wheels lose the engine in the air where the original's are driven up to 242
rad/s and give 5–6 m/s at the landing (frames 444–450). The bump response at frame 270 is the first
difference; a sub-step capture there (`subtrap.py`, armed from frame ~265) is the next step.

### 6.6 The hop's load term (verified, observed; fixed, partly)

The n1 sub-step capture shows +0x164 written after each integration (0x5AA490's force pass), so at the
frame's end it is the end state's (frame 4: 3.1758 / 3.1903 / 2.5855 / 2.5812 after the fourth
integration). `CarState::end_loads` measures the suspensions at the end pose (the next sub-step's start
load exactly: compression at the end pose, rate over the last sub-step, the anti-roll bar's live order,
the preload at the end speed, camber) and `wheel_hop` reads it (`hop_loads`), only when its start test
can fire. Test `hop_reads_the_end_state_load_term` (n1's native terms give count 3 and grip 0.195188).
Hop counts against the original on four the drag-physics parity runs (16 cars): 8 equal before, 10 after (n1 4 / 4,
`hop-counts.txt`). The rest sit on a 0.2 % knife edge between the two rear wheels, from the held car's
pose: at rest the original's loads are symmetric (3.7637 front, 2.4589 rear) and the clone's are not
(3.8432 / 3.8345 / 2.3849 / 2.3942); that pose difference (the car rolled to the line and held, 0x5992C0)
is **open**.

### 6.7 Merge follow-up: a rival through the wall at Sprint 4122's hairpin (verified; fixed)

The hop fix changed the launches enough that rival Liz (PEUGOT, Sprint 4122 backward,
`rivals_keep_the_race_route`) reached the hairpin at (−1510, 730) on the BeltMover (mode 4, far from
the human) after a GetUnstuck. The world's edge there is a group-0 wall (faces (26, 1577..1579),
flags 0x16, one-sided, normals into the road, ±2048 m high; no ground south of y ≈ 727,
the maintainer's probe `ground_column`); it stops a car on the physics mover (checked in the clone with
INTRO_POSE drives into it). The belt car's wall contacts were resolved but the clone kept it on the
belt (a deliberate deviation for an older barrier problem), so the belt placed it past the wall each
frame and it fell into the void. Natively a track contact of a loaded mode-4 car reaches 0x402450
(0x593990 passes the track partner; 0x5F46B0 takes no body out of the sweep): +0x730 = 2 s and
0x5F4410's mover at once. Ported (`off_belt` on track contacts); the old barrier trouble does not come
back (every rival of the test's four races finishes, back ≤ 0.003 laps).
The −44 m/s "reversing" after her later recovery was not a driven reverse: (1) the AccidentRecovery
started while she fell was dropped after 4.5 s because the clone could not place her on her route
list (no own road entry), so its 5 s reset never fired — natively 0x6148F0 always keeps the car's own
node, so a running AccidentRecovery now goes on with no entry (inferred for the other actions); its
reset then puts her on the route's node, heading its way, at speed max(0, speed) × road·forward
(0 here), as 0x5FBEE0 does; (2) the BeltMover was later built from the falling car's forward speed
(~80 m/s). A reset's placement 0x5F52D0 switches the mover to mode 0 and back (0x5F52E7, 0x5F53F4),
so a belt car gets a fresh, pathless belt that stands; the clone kept the old one. Ported (`reset_rival`
rebuilds a pathless belt, `Belt::speed` 0 for it). Tests: `rivals_keep_the_race_route` passes,
`a_belt_with_no_path_stands`. Not checked under Wine at that spot (an AI racer on mode 4 at that
hairpin is not reproducible with the harness); the evidence is the disassembly and the collision data.

## 7. 2026-11-05: the crest's "bump", the held pose, the finish, free-roam gear, racers and traffic

a private Wine copy, display :331, wined3d; raw runs kept with the maintainer's private evidence
(c1: p4's plan with the sub-step trap armed at frame 255, `dragshift.py --substeps 40 --substeps-from 255`;
f1 / f2: p4's plan to the finish, `--after 16 --continue 4`; g1 / g2: the free-roam resume probe,
`p11-work/tools/probe_gear.py`). Evidence: the maintainer's private captures.

### 7.1 The "bump" at frame ~269 is a knocked fence (verified, observed; fixed)

The one-frame speed loss before the crest is not the road: the car hits a **ZCS_FenceH** prop (0.2 t,
inertia diag (0.312, 1.319, 1.009), yawed 66.9°) standing in its lane. The sub-step capture
(`fence-substeps-c1.txt`) shows frame 268's four sub-steps, then the contact moving the car back to
its third sub-step's end and running the rest of the frame again (two sub-steps of 0.004373 s) with the
impulse taken: momentum (50.858, 22.729, −0.005) → (47.738, 21.084, 0.016), i.e. ΔP (−3.121, −1.645,
0.021), J = ΔP × 1.85 / 1.271 = (4.54, 2.39, 0.03), |J| 5.13, spin change (0.139, −0.167, −0.039).
The clone (NFSU2_SUBSTEP_DUMP's new `SUBSTEP PROP` / `SUBSTEP TRACK` lines) met the same fence at the same
place and normal (0.927, 0.374) but took |J| 3.36: its prop term of k (1/m + n·((I⁻¹(r × n)) × r)) was
14.35 against the original's 8.84. 0x5AF0F0 gets the prop's I⁻¹ from vtable +0x60 = **0x5B1C50**, which
returns rigid body +0x1B0 (+0x170 without +0x158), and applies it with 0x43C9C0 to the world r × n
**unturned** — the body-space tensor, as for the car (0x593A50); the clone had turned it (R·I⁻¹·Rᵀ).
With the unturned tensor the clone's j is 5.118 (native normal part 5.109); the native J at the
clone's contact arm and car inertia gives the spin change (0.133, −0.154, −0.036), the original's
(0.139, −0.167, −0.039), so the contact point agrees. Fix: `Prop::body_inverse_inertia` in the car and track
impulses (src/smackable/; the rigid body's own updates still turn it). Test
`smackable::tests::car_push_takes_the_props_unturned_inverse_inertia`. Speeds after it (k4 = p4's plan):
41.84 / 41.74 m/s at 270 (clone / original), before 42.49.

* The friction part differs a little: the original's J leans 4.7° off the fence normal (μ·j·min(1, slip)
  with the car's 0.3 and a tangential slip of ~0.3 m/s), the clone's 16.7° (its closest-point normal is
  1.1° off the fence face, slip 1.8 m/s). 0x5941D0 picks the pair's μ: a car (+0x11 = 0) against a type-3
  body (the track) takes the car's side, else the larger (verified); same as the clone. The 1° normal is
  the closest-point pair's (0x594010: difference of the pair's points within 1 mm), **open**.
* **The rest of the crest is chaotic** (observed): the knocked fence falls onto the car again at
  frames 274–275 (clone: a second `SUBSTEP PROP` with normal z ≈ 1, vel.z −0.66 → −1.64). Two native runs
  with the same inputs to frame 268 (speeds equal to 0.08 %) differ from there: p4's rear load spikes
  to 392 at 274 and its rear wheels slide and spin up with the engine in the air (slip 1 → 7); c1's do
  not (slip 0.16–0.29, load 0 in the air, as the clone now). The airborne coupling is the drag-physics parity's (5.2) and the
  4 → 5 over-rev of p4's clone replay follows from that second touch, not from a drivetrain difference.

### 7.2 The held pose and the hop count (harness; fixed)

The rear-load asymmetry behind the hop-count ties (6.6) is the parity flow's, not the physics': when
the original draws the player another grid slot than the clone's own draw, `place_on_grid` carried the
clone car's settled pose from its slot to the original's (7.15 m away in c1), roll included. Natively the
held loads were symmetric (c1 3.8317 / 2.3907 / 2.3907; p4 3.8316 / 2.3903 / 2.3908); the clone's
3.8254 / 2.3820 / 2.4003, hop 4 against 3, 2.5 % slower by 2.5 s. Now the parity flow hands the original's
player place to the slot draw (`Game::parity_grid_at`, a check aid: the racer that drew that slot gives it
up), so the clone's car settles there itself: loads 3.8384 / 2.3900 / 2.3900, hop 3, speeds within 0.2 %
to 2.5 s (`hold-pose-c1.txt`). Native frame 0 loads equal the clone's to 4 digits where the slots already
agreed (k4). The later splits again come from shift grades on a knife edge (c1: 3 → 4 at 9324.8 natively =
Perfect's lower bound 9324, the clone 9312.6 = Good).

### 7.3 The finish (observed; ET not comparable)

The same driver (p4's shift deltas and lane map) gives different native finishes: p4 won in 29.74,
f1 was totaled at 24.8 s, f2 came 2nd in 30.69 behind Mataio's 206 (30.63) (`finish-runs.txt`,
`results-f2.jpg`). The rivals are each boot's own draw (G35 / Golf / 240SX, Mustang / Miata / Celica,
206 / Eclipse / 350Z). f2's Continue led to Post Race ("Return to Explore Mode", `postrace-f2.jpg`) with no
reward screen (a loss). The clone's replay of f2's key frames follows to 6 s (player within 0.8 m) and
then leaves the plan's lanes (the lane taps are by frame; the crest's chaos shifts the timing) and hits the
lane-end obstacle at ~14 s. A winning native run's rewards / reputation after Continue were not
captured (**open**: needs a won run; the clone's formula is `career::reputation_points`, 1 / 2 / 5 %
margin → 100 / 200 / 300).

### 7.4 Free-roam gear at rest (observed; no change)

The sparks-and-fire work's "clone stands in neutral where the original's stands in first": the car set-up 0x59F981 stores
neutral (car +0x434 = 1) and the resumed car stays so on the loading screen's mover (vtable 0x7A5BB8,
clock 6967 ticks); on the physics mover (0x7A25E8) 0x5AA970 asks for first at once — the first sample in
free roam (clock 8978, the "Fort Union" banner, `freeroam-first-sample.jpg`) already has gear / target 2
(`freeroam-resume-gear.txt`). The clone does the same (neutral at its first frame, first gear by frame 10,
The audio and camera parity `freeroam_stand`). The native recordings start seconds into free roam, which is why they show first
from frame 0. Whether the original draws the hand-over's first frames (the 0.5 s samples cannot tell) is
**open**; the pop on the N → 1 change follows whichever.

### 7.5 Racers rear-ending traffic (open)

Not reached this work. What was traced: AvoidAvoidable's speed rule skips a traffic object (0x6148D0:
controller +4 = 3) and objects 0x60BF80 passes (its car's 0x5EBFB0: car +0x974 stamped within +0x978 s,
"recently hit"); the clone tests `e.car.racer`, which leaves non-racer, non-traffic cars (none in a race)
the same. The slow test's room check (99999 cost) and 0x42F505's bVar4 / bVar5 tests stay **open**
(formats.md, AvoidAvoidable).

## 8. 2026-11-05: the replay to the finish, a won native run, the lane table's 80 m, HEAD START

a private Wine copy, display :343, wined3d; raw runs kept with the maintainer's private evidence
(w1–w3: nitrous at 2 / 8 s, totaled at the crests; w4: won; w5–w7: boots hung; f2e / w4b: clone replays). Evidence:
the maintainer's private captures (finish-w4.txt, the post-race screens).

### 8.1 Lane taps by race coordinate (harness; done)

The clone's replay pressed the native lane taps at the native frames; after the crest the clone runs a few
metres off the original's timing, so a tap changed lanes at another place and the clone met a lane-end
obstacle (7.3). Now `dragshift.py --replan` writes each lane tap as `taps: [coordinate, 6, key]`, the
coordinate the player's +0x8FC in the row before the clone's press (row f + 1), and `parity.py` writes
`tap COORD FRAMES KEY` plan lines: the clone presses the key on the first frame its player's race
coordinate (`Progress::along`) reaches COORD and lets go FRAMES later (`Taps`, parity.rs;
`PARITY_TAPS=frame` keeps the old frame keys). New: the clone accessor `rcoord` (fields.toml `rcoord`
now records the clone's race coordinate), `PARITY_EXTRA=N` (the clone runs N frames past the original's
recording and prints its results, reward, race result and the sponsor offers), `PARITY_SPINMAX` (the
cave's lockstep spin limit), `dragshift.py --continue-keys` and its career reads (`career`: bank
0x861E74, stage 0x862838, stage record 0x862018 + 0x104 · stage +0xE0 reputation / +0xE4 races, the
race-result object 0x85BA64, the event's visit entry), and the `kinds` probe. Test
`parity::tests::taps_follow_the_race_coordinate`.

**Checked** (f2, the crest-avoidance pass's run, 14 taps; w4, 14 taps): every clone tap fires within 1 m of the original's
coordinate and the clone's player stays in the plan's lanes to the line. w4: player position error
≤ 7.3 m through the race (0.8 m at 30 s); f2: the clone follows to the finish but 0.3 s behind (the crest's
landing, 6.5, and a fence the original's rival had knocked down, 8.4).

### 8.2 A won native run (observed) against the clone

w4 (rivals Sentra / 240SX / WRX STi, no nitrous): **"Winner! Your time: 30.15"**; Return → a LOADING
screen → **Event Results** 1 NAME Skyline 30.15, 2 Nigel WRX STi 31.95, 3 Mataio Sentra 33.09, 4 Harlan
240SX 0.00 (totaled at 11 % of the road) → **Race Reward** "Reputation Points Won: 300", "Bank Won: 2200"
→ **"SELECT A SPONSOR!"** → **Sponsorships** (KENWOOD first: signing bonus 5200, URL victory 5000,
sponsor race purse 2250) → "Want to sign on with KENWOOD?" (No selected). Memory: at the results the
stage-4 record already counts the win (its first win slot set) while the bank (42150), the reputation
(0.0, 0 races) and the visit state (2) are unchanged — as formats.md's stage-1 sprint (paid on leaving
UI_PostRace).
The settlement after leaving UI_PostRace was not captured: w4's key presses ended on the sign question,
and three further boots (w5–w7) hung on the loading screen before free roam (**open**).

The clone's replay of w4 (same key frames, taps by coordinate, the original's rival models):
**the player wins in 30.07** (native 30.15; −0.08 s), splits within 0.19 s at every 100–200 m; reward
**300 / 2200** (margin 1 − 30.07 / 32.33 = 7.0 %; the original's 1 − 30.15 / 31.95 = 5.6 %; both ≥ 5 %
→ 300, `career::reputation_points`), the race result won / pays, and after the reward the sponsor popup
with KENWOOD, SPARCO, NOS, ENKEI, BREMBO on offer (KENWOOD first, as the original's screen). The
rivals' rows differ (8.4).

### 8.3 The lane table's 80 m is the AI racers' (verified, observed; fixed)

0x412500 hands 0x40B110 the flag `car +0x14 → +4 == 2` (0x41263D..0x41264E); 0x40B110 subtracts 80 m
(0x783CCC) from a flagged row's start when the flag is set (0x40B17C..0x40B184). **Observed** (`kinds`
probe, w3 / w4): the player's +0x14 → +4 is 1 and every rival's 2 (traffic 3) — the owning kind. The drag-traffic pass had
read 2 as the player's DragCarController, so the clone started the player's blocks 80 m early and the
rivals' on time; now the 80 m is the AI racers' (`table_allows(…, ai_racer)`, the rival lane choice and
AvoidAvoidable's lane test pass it; tests `drag_table_blocks_changes`, `lane_table_reads_the_race_coordinate`
updated). The run test of 0x412500 (0x412552: skipped when the car's +0x24 is 2): **observed** the
player's +0x24 is 1 at the green and **2 at frame 600**, the rivals' 3, so the player's lane changes
skip it, as the clone had (comment corrected).

### 8.4 Rivals (observed; open)

In both replays two of the clone's three rivals stop before the line, where the original's finish (f2: all
three; w4: two, the 240SX totaled natively at 11 %): f2 at race coordinate ~1405 the clone's ECLIPSE
scrapes a wall in its lane 4 from ~1342 (gas on, −1 m/s a frame, steering −4°) and PEUGOT runs into it;
w4 the clone's WRX STi, moved from lane 2 to lane 3 at 884 (lane 2 ending), stays 8.4 → 6.3 m off lane 3's
centre over 76 m and hits lane 2's end obstacle at 962. The original's rivals also run 4–9 m/s faster
from ~12 s in f2 (their nitrous comes in several shots, 24000 → 20137 → 14401 → 1465; the clone's in one,
and 0x42ABD0 can end an AI shot when 0x40D850 fails at 0x42AD24..0x42AD55 for any racer, which the clone
does only outside drag). A rival that knocked a ZCS_FenceH at the second jump natively (f2, frame 1730)
leaves it standing in the clone, whose player then takes the 6 m/s hit (frame 1778). **Open**: the AI's
lane convergence (the lane path's offset steps, 0x40FC80 / 0x60BFB0, against the native rivals'), the
drag AI's nitrous end, the shift timing that follows from both.

### 8.5 HEAD START and the nitrous refill at ~1 s (verified, observed; fixed)

The refill at ~1 s (5.5) is the style points' HEAD START (0x56F730: leading, race time > 1 s (0x79DB70)
and < 1.5 s, faster than 5 mph), which fills the tank through the style refill (0x601737). The race time
is the race status's +0x14 × 0.00025 (formats.md, "Place"), whole ticks: at exactly 4000 (frame 60, the
recorded race_clock in both games) it is not over 1 s; the clone's summed f32 clock read just over 1.0 there and refilled a frame early. Now the
style detectors get the clock in whole ticks: the refill comes at frame 61 in both (f2: 24000 → 25600).

### 8.6 Lane splits: the lane room ported, the stops not yet

A fresh native run of S4_H_DRAG_15 (dragshift.py with lane keeping; rivals PEUGOT, 3000GT, RX7) recorded
the rivals' walk points (controller +0x370: count +0x374, points +0x378 + 4k, each point's +0x54, record
+0x5C, rooms +0x18.. / +0x2C.., lane adjustments +0x04..) and the slip trackers. Evidence:
`docs/evidence/native-ai/` (`drag-room.txt`, `finishes.txt`).

* **The lane room (verified, observed; ported).** 0x41D530 clamps each lane path point's offset with
  0x40FC80 to the point's room for the lane less a margin (formats/career-2.md, "Lane room"); the clone's
  `RacerRoute::lane_path` had no clamp. The room is the point's own (0x41FD50's backward pass steps by
  Δs × 99999 and never binds) and matches the native points to 0.01 m where the point's lane adjustment
  is 0 (test `lane_room_matches_the_originals_walk_points`). In a drag race 0 is always inside the room,
  so the clamp binds only where a road edge comes within the margin of a lane path.
* **The stops before the two ports below (observed).** Replays of w4, f2 and d1 with the base: four of
  nine rival runs stopped before the line (w4 WRX at 1271.6, f2 ECLIPSE at 1669.4 and 350Z at 1667.7, d1 RX7
  at 1409.7); natively all of them drive on (the 240SX's natively totaled run at 569 is another case).
* **The drag walk joins records as the original (verified, observed; ported).** 0x422220, at a junction
  that is no turn: outside drag races a new point under 3 m on is walked past (the next one taken); in a drag
  race (0x89E7D8 / 0x890108) the list's last point is dropped instead, its segment added to the distance,
  until the new point lies 3 m on (a list of one starts again from the new point). So on 4213 backward record
  278's last point gives way to record 277's first, which the clone had dropped (it kept the old record's end
  and took record 277's second point). `RacerRoute::drag` (`start_in`, the race set-up's rivals and the
  player's DragSteer). **Observed** (d2's walk points, 216 points at frames 900 … 1600): 199 of them are in the
  clone's walks started at the rivals' poses (the rest lie behind the start), and the 8 m points' lane 2–5
  adjustments match within 0.10 m where the two walks have the same point before (record 278 point 1 + 8 m:
  lane 2 −3.40 against −3.50) (test `drag_walks_join_records_and_bend_lanes_as_the_original`).
* **Lane adjustments (verified, observed; ported).** 0x41A180 bends lanes 2–5 (FuturePoint +0x08..+0x14) as it
  bends the racing line (+0x04): each lane's own Hermite between its places at two control points, the
  tangents from the place before and to the next plain point (0x4122C0 on the lane). 0x419B20 / 0x60BFB0
  take the lateral off the adjusted lane, 0x41D530 lays the path on it and 0x40B840 / 0x40B780 take the
  room off it (`RacerRoute::smooth`, `lane_place`; `lane_lateral`, `nearest_lane`, `lane_path`).
* **AvoidAvoidable's path (verified; ported).** Its update 0x42F330 calls 0x42BF00 (or with no object
  0x420D40) every update; 0x42BF00 with no lane to move to rebuilds the action's path on the controller's
  lane record from where the car is (0x42BFA9 → 0x420D40). The clone returned the path driven before, so a
  rival whose record had just moved to the next lane went on along the old one. Now `DragLane::record_path`
  (test `avoid_without_a_lane_drives_the_records_path_from_here`).
* **The stops (observed; fixed).** Replays of d3 (a fresh native run: TIBURON, IMPREZAWRX, TIBURON), w4 and f2
  (`docs/evidence/native-ai/finishes.txt`): base 5 of 9 rival runs stopped before the line (d3 IMPREZAWRX at
  1667.4 and TIBURON at 1669.5, w4 WRX at 1271.6, f2 ECLIPSE at 1669.4 and 350Z at 1667.7); now none, as in
  the original (the 240SX natively totaled at 569 in w4 is another case: the clone's drives on).
* **The rivals' nitrous (observed).** Each shot starts when 0x40D850 passes with no shot running and ends when
  it fails (§9); a shot needs at least the row's +0x570 seconds left (0.5–2 s), all four wheels down, the
  driven wheels' exported slip under 2 m/s, the row's gear (+0x588 + 1) and the rpm inside 0x402590's band.
  What is left at the line follows from when those hold: natively 2190 / 0 / 0 (d3), 1465 / 200 / 0 (f2),
  0 / 0 (w4) ticks; the clone 0 / 0 / 5956, 3198 / 0 / 0, 0 / 0 / 0. An earlier run's 10800 and 4867 left
  natively against the clone's empty tanks (d2) came with the clone's rivals 4–6 m/s faster on the old walk;
  on the native walk the paces agree within 1–3 m/s (d3 at 1100 / 1400 / 1700 m). No rule differs; the shots'
  timing follows each run's gear changes.

## 9. 2026-11-05: the rivals' nitrous (verified, observed; fixed)

`outrun-pace.md` §3–§4: 0x40D850's rpm band was inverted (0x402590 returns 1 *outside* the band; a
shot may run only inside it) and the clone never ended a drag rival's shot before the tank was empty
(0x42AD24..0x42AD55: any AI racer, 0x408430 always true in a drag race). Now the tank goes out in
several shots as the original's, and in both the drag-finish pass replays all three rivals reach the line (f2:
PEUGOT 30.74 s, native 30.63; the drag-finish pass's clone: two of three stopped). Still **open**: w4's WRX (native
31.95, clone 33.76), the 240SX that crashes natively at 11 %, and the lane convergence.

## Open

* The bank and reputation after leaving UI_PostRace on this stage-4 win (8.2; the reward screen's 300 / 2200
  match, the settlement itself is formats.md's stage-1 sprint observation).
* Rivals (8.4, 8.6): none stops on 4213's lane splits any more (lane room, lane adjustments, the drag walk's
  record join and AvoidAvoidable's path ported); the rivals' rows of the results are not compared run for run
  (their shots' timing follows each run's gear changes, 8.6). The drag AI's nitrous end is fixed (§9).
* The fence contact's friction direction (7.1, the closest-point normal 0x594010); the crest after the
  fence's second touch is chaotic natively (7.1).
* Whether the free-roam hand-over's first frames are drawn (7.4).
* Racers and traffic (7.5); camera and audio.
