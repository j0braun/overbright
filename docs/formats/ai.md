# AI, traffic and outruns

Part of [docs/formats.md](../formats.md), the traced behaviour of `SPEED2.EXE`; the tags
(**verified**, **inferred**, **open**) are explained there.

## AI driver (verified except where noted)

The racers' low-level driver (`src/ai/driver.rs`): controller type 3
(0x4260B0) builds it with 0x425AB0 (0x460 bytes, vtable 0x784EB0, base
"CarDriver" 0x423330); the world update 0x609CF0 runs every controller's
think, then every driver's 0x421030 with the frame dt (floored at 1e-4
for the steering rate).

* **Outputs** (the block the mover reads in 0x5ABC00 and writes back
  after its pedal overlap check): +0x1E0 the front-wheel angle as a 16-bit
  angle, positive left, = trunc(degrees × 65536) / 360; +0x20C gas, +0x214
  brake, +0x210 handbrake; +0x1E4 the gear request (−1 automatic, 0
  reverse, 1 neutral, n that gear: 0x5AA970 requests it outright). The
  clone's `CarState::step_ai` holds the wheel angle for the frame (no
  player steering 0x416BB0, no reverse latch), puts the pedals through the
  same overlap check (none in URL events) and hands the gear request to
  the gearbox (`Drivetrain::request_gear`).
* **Helpers.** Running mean (0x579F30 / push 0x576980): count up to n,
  sum += v − old, mean = sum / count; fill (0x5769E0) sets every sample to
  v, the count 0 and the sum n·v. Windowed mean (0x579F90(W, 45) / push
  0x576AB0): room for trunc(45·W + 0.5) samples, entries older than W s
  dropped, an empty slot is value 0 at time 0. Tracker (0x4016B0(n1, n2,
  k), update 0x576BC0(v, dt, no rate, no area)): times and trapezoid areas
  (n1), rates (n2); I = Σareas·count / (Σdt·k), D = mean rate; reset
  0x57A020(v) (value 0, previous v, areas and rates filled with v) and
  0x576C50 (rates only). Curves 0x5764E0 (evenly spaced, clamped) and
  0x5767A0 (points, clamped, a vertical step gives the mean).
* **Adaptive PID** (0x409B50, output 0x409DA0 = Σ Kᵢ·clamp(termᵢ), terms
  clamped ±1e5): every 0.2 s period one gain in turn (trunc(t / 0.2) mod 3)
  steps by −q·rateᵢ·x·dt within its bounds when |termᵢ| ≥ 0.01 (0x4182C0 →
  0x411390, mode 0); q = mean(dx/dt) / mean(gain steps) clamped ±1000, or
  the sign of mean(dx/dt) (0 under 0.001) when the steps are ~0 (0x409EA0);
  the step and dx/dt histories are 0.2 s windows.
* **Mode** (verified; 0x421030 at 0x421116, 0x415F80 at 0x415F92): by
  the car's mover (car +0x28, set by the mode switch 0x5F46B0 at
  0x5F4907) and kind (car +0x24, 4 for traffic): +0x28 = 1 (full
  physics: the human, and a racer only within its physics hold, see Racer
  movers) mode 1 on the adaptive PID; 3 (MeloMvr) mode 4
  on the table for a traffic car, else mode 0 on the PID; 4 mode 0 on
  the PID; any other — 2, the TrafficMover — mode 3 on the table. The
  table timer (+0x248) and the stuck flag then make it 5 and 6. A table
  mode (0x42172C, also for +0x28 = 4 and mode 5) steers by δ =
  −(P[m]·P + D[m]·D + I[m]·I) (0x78395C, 0x783994, 0x783978: mode 3
  0.7, 0, 1.42; mode 4 0.85, 0.12, 0.79), takes the lock curve, moves the
  wheel at curve 0x7E9E34(|P|°) × curve 0x7E9E20(v) locks a second
  (doubled back towards centre) and multiplies the new angle by H +0x270 (0x421E8C)
  (H = car +0x20, the tuning block at record +0x110: record +0x380, the
  steering ratio, 1 for the traffic models). Front wheels off on mover 3:
  the minus-slip steering is held within ±5° (0.0873); the gain caps take
  the SUV values only on mover 1. Clone: `AiDriver::mode` from its
  `mover` / `traffic` fields, which the traffic sets each frame (2 on the
  TrafficMover, 3 after a hit); racers take theirs from `racer_mover` (Racer movers).
* **Look-ahead point** (0x415F80): modes 3 and 4 (verified): L =
  max(forward speed × 0x7839B0[m], 0x7839CC[m]) (mode 3: 0.55 s, 5 m;
  mode 4: 0.9 s, 9 m). The rest: L = curve 0x7E9D94 (22.9 … 55.9 m over
  0..160 m/s forward speed) × curve 0x7E9DF8 (1.3 → 1.1 over performance
  class 0..3; on mover 3 or 4 curve 0x7E9DE4, **not ported**: no such car
  in the clone). On the path (0x415630, ≤ 20 points): from the first point
  ahead along the velocity (0x404EF0: the first point whose 2D direction
  from the car has a positive dot with the velocity normalised by 0x43CE50,
  which gives (1, 0) for a standing car, and that is not on the car;
  thresholds 0x7837C4 / 0x7837C8 / 0x827A20 all 0; none: the last point;
  cached per world tick, +0x1B4 / +0x1B8; **verified**), distances summed with each corner's
  excess × k (k = 7 within 5 s of the green light, 1 after), the point at
  L by linear interpolation, past the end along the last segment. No path:
  ahead along the velocity (forward speed ≥ 5) or the heading, at least 5
  m, and the target speed dropped.
* **Steering** (front wheel down): errors e1 = heading − direction to the
  point, e2 = e1 blended with the direction of travel (weight 0 at 5 m/s,
  1 at 10), e3 = atan2(offset right of the path, 5 m); trackers (5, 5,
  30) take e1, e3, e2. P/I/D are the e1 and e2 trackers' weighted by their
  squares when they agree (e1 alone when they don't); I clamped ±6°
  (0x81E5AC), D ±1. Steering PID terms (−P, −I, −D); bounds car Kp
  0.15–0.5, Ki 0–0.5, Kd 0–0.08 (SUV Kp 0.4–0.7), rates 5e-5, 5e-5, 5e-6;
  x = rate of |error| (°/s) − curve 0x827A18 (error should shrink 0.5/s,
  ≤ 5°/s); under 10 m/s forward the gains restart from speed curves
  0x7E9D44 / 0x7E9D80 / 0x7E9D58. Front wheels off: the wheel angle held
  0.3 s, then minus the slip angle; gains capped (Kp 0.15, Ki 0, Kd 0.05;
  SUV 0.4 / 0 / 0.1). δ clamped ±45°; lock curve 0x7E9E0C (45 30 5.5 5 3 2 …
  °), 45° in drift races; the wheel moves at most 100 locks a second.
  Pinned (0x421778 …): the first of the car's contact records (car +0x3C,
  count +0x18, the i-th through 0x5A0830 in the pair list's order) pushed
  in the last 0.1 s (record +0x0C, ticks × 0.00025) and made over 0.6 s
  ago (+0x08) that is car against car (0x592C70: both bodies type 0) with
  the other car (0x592CA0) under 26.82 m/s (+0x430, 60 mph), or against
  an upright track polygon (0x5A0170: a type-3 body whose polygon normal
  z, +0x60 → +0x78, is under 0.1 in size) replaces the PID's output by
  lock-curve(signed speed)·min(made − 0.6, 1)°, negative when the push's
  point (+0x30) lies at or left of the way of travel (0x591FD0 in the
  frame 0x591DF0: the velocity's direction, reversed when the forward
  speed is negative, the heading when |v|² ≤ 1) — steering away from it.
  The clone keeps the records (`CarState::contacts`, `drive::ContactNote`).
* **Pedals:** no driven wheel down: full (capped) gas. Target under 2
  m/s: brake. Forward/reverse direction latches (−7/−2 and 7/2 m/s): going
  the wrong way brakes 0.9 (forward target: steering mirrored); reverse
  target: gear 0, steering mirrored. Speed control 0x41D8D0: target < 5:
  bang-bang (dead band 1 m/s); ≤ 30: table PID on |target| − |v| (gains
  0x7E8FCC … 0x7E9058 by mode, braking table when target + 2 ≤ v); > 30:
  the Simple adaptive PID (Kp −0.3…−0.2, Ki 0, Kd −0.6…0, term2 ±10,
  derivative target curve 0x81E7F4, no rate when the target jumps by more
  than 1, Kp −1 under 10 m/s). The change is limited to curve 0x7E9D10 ×
  dt (30 15 7.5 6 … /s) from last frame's gas − brake; a traction cap from
  the summed wheel slip beyond 4 m/s (0x410040: racing, after 4 s or
  sliding past 5°). Gas ≤ the throttle cap 0x783A28 {0.93, 0.97, 1, 1}.
  Countdown: gas 1, brake 1, neutral. Handbrake timer +0x240 above 20
  mph: handbrake alone; stuck +0x24C: brake and handbrake.
* **Angles** (0x43E6D0, `driver::angle16`, verified): the arcsine table
  0x43C490 (208 entries at 0x7EA628, a u16 angle and an f32 slope each:
  x × 65536 truncated picks a segment by halving 0x8000, 0xC000, … at most
  11 times, 16 entries a segment; angle − trunc((x − entry x) × slope ×
  −65536); 1 or more is 0x4000, negative x mirrored) of the smaller
  component over the length (|dy|/r when |dy| < |dx|, else 0x4000 minus
  that of |dx|/r; 0 when r² ≤ 1e-4), put in its quadrant (dx < 0: 0x8000 −
  a; dy < 0: 0x10000 − a; both a + 0x8000). Within 8 units of the exact
  angle (`table_angle_follows_atan2`).
* **Nitrous hold** (verified; ported, `AiDriver::nitrous_hold`). 0x59FA40
  (ECX = car +0x40, from 0x421F45..0x421F4E) is the nitrous stage of a
  running shot: 0 with no shot (+0x420 = car +0x460 is 0 or 0x7FFFFFFF),
  else 1 + the highest i with the tank fraction (0x5925C0) over 0x7A1798[i]
  = {0, 1.4, 1.7}; an empty shot is stopped (0x592510). The mover stores it
  at engine +0x84 (0x5ABD75), the grip set-up indexes 0x7A17AC with it
  (0x5A5106). In 0x421030, after the speed control and the countdown block
  (0x421ECE) and before the stuck override (0x422034): a nonzero stage sets
  the driver's latch +0x25C (0x421F61); while it is set, outside a drag race
  (0x89E7D8 / 0x890108) and with the target (+0x230) not under last frame's
  (+0x260, written at 0x421FBD), gas (+0x20C) is the throttle cap (+0x22C)
  and the brake 0 (0x5EB920, 0x5EB900(0)); otherwise the latch clears. The
  latch outlives the shot until the target falls. **Observed** under Wine
  (outrun_brake_p14 b1, b3, b6, b9): after the outrun's end the rival's
  target dropped to three quarters (43 m/s at 57 m/s) and it kept
  accelerating (57 → 61 m/s in 2 s) through the shot that started there;
  in a fresh run with no shot at the end (350Z, gas and brake recorded) the
  rival braked to its new target (32 → 24.5 m/s, brake 0.75 then 0).
  The clone had read 0x59FA40 as a crash level its cars never have, so the
  latch never set and its rivals braked to the target.
* **Throttle cap +0x22C.** The constructor 0x425AB0 writes +0x22C (0x425B44..0x425B67, verified):
  0x783A28[0x89E804] when its controller's +0x14 object has class 2 at +4,
  else 1.0 (0x784250, verified). It is not the only store to +0x22C: the
  sibling constructor 0x425E40 (vtable 0x784F40) zeroes it at 0x425E76, and
  0x424452 (FUN_4243B0) and 0x41E25E (FUN_41E060, from a table at 0x83AA64)
  store it too; which object class those belong to is not traced. The index
  0x89E804 has three direct writers, 0x5323F5 (a career event's +0x7D),
  0x53F775 (Quick Race drag: the difficulty) and 0x5400C5 (not traced;
  see Difficulty: settings and effects), and one read, in the
  constructor.
* **Car +0x4D8** (verified): the car init routine 0x604270 sets 1.0f; the step sets 1.4f
  (0x3FB33333) through the setter 0x5EBBF0 (its only caller) while the stuck
  flag +0x24C is set, when it also brakes and handbrakes (0x5EB900, 0x5EB910)
  and cuts the throttle (0x5EB920(0)). The setter copies the value to a
  mover's +0x6C when car +0x28 is 1; 0x5F47DE stores a new mover at car +0x34
  and copies +0x4D8 into its +0x6C (disassembly); the per-car snapshot routine
  0x5FB600 ("Car%d" section) saves it with 0x571780. What reads that +0x6C is
  not traced.
* **Slip trackers** (0x4057C0, from 0x421030; verified, observed; ported,
  `AiDriver::track_slip`): over the four wheels of the car's exported pair
  (car +0x3B4 / +0x3B8 + 8i, ECX = driver +0x220 → +0x40 → +0x374), the
  along-wheel values beyond ±4 m/s (0x7839F4) summed, negated when the
  forward speed +0x3EC is negative, into the tracker at driver +0x274; the
  |across| values beyond 4 m/s (0x7839F8) into +0x278 (0x576BC0, value at
  tracker +0x7C). The pair is the wheels' +0x100 / +0x104 on the physics
  mover (the tyre pass 0x5A8290: +0x100 the slip +0x118 while sliding, 0
  while gripping, 0x5A8600 / 0x5A86A1; +0x104 the contact's speed across
  the wheel in both branches, 0x5A8606 / 0x5A86AB) and the chassis' +0x90 +
  8i on the MeloMvr and the NIS mover (write-back 0x5A2B70). **Observed**
  (stage-4 drag, three rivals on the physics mover, 1895 frames each): both
  trackers equal the sums of the previous frame's exported pair on every
  frame (the driver runs before the mover). The clone had summed the
  wheels' own slip; past 4 m/s the two differ only for a wheel spinning on
  a car at rest (the export is 0 with no travel), and in the same capture
  the own slip and the export beyond 4 m/s never differed: no change to the
  rivals' driving was measurable (the replays' sim values are unchanged).
  The career playthrough's state first differs at step 1173 (free-roam
  traffic) with this read alone (reverting it moves the first difference to
  the drag lane changes at step 2468): the clone's traffic and belt cars keep
  a stale `exported_slip` after a mover switch (slip 0, export 6–14 m/s;
  120 604 wheel-frames in one playthrough, movers 4 / 1 / 2), where the
  original zeroes the pair when it builds a BeltMover (0x5A93B0 → 0x59F890)
  and at a placement (0x5F52F1 → 0x59F890) and the belt update writes it each
  frame. **Ported since** (traced): the TrafficMover's frame 0x408820 clears
  every wheel's pair (0x408BC5 / 0x408BC7); the BeltMover's 0x59EC90 writes
  along = its speed − the velocity along the body's forward row (+0x90),
  across = −the velocity along its left row (+0xA0), for every wheel
  (0x59F344..0x59F378, 0x59F402 / 0x59F408; `belt_slip_pair`). Wheel-frames
  with a mismatch between own slip and export beyond 4 m/s in one playthrough:
  120 604 → 17 760, all of them the belt's own pair (slip 0 there).

## AI route layer (src/formats/ai_routes.rs, src/ai/route.rs)

Everything above the low-level driver; verified except where noted.

* **Route file** `TRACKS/ROUTES<region>/Routes<track><F|B>.bin` (0x57F554,
  chunks at 0x5DEB90): 0x34121 route records (0x8C header: +0x0A id, +0x0C
  0 for race records, +0x10 name, +0x34 count, +0x38/+0x3C the first/last
  points' coordinates, +0x60 / +0x68 the min and max (x, y) corners —
  0x43D410's box test with a margin, e.g. at 0x417EFF —, +0x70 flags — bit 6 listed
  by 0x5D9440, bit 10 never branched into; +0x74/+0x75 (+0x7C/+0x7D
  backward) fade the lateral adjustments) then 0x38-byte control points:
  x, y, +0x08 the race coordinate (on 4001F the distance to the line, e.g.
  6273.4 at the start against the track's 6274 m), +0x0C flags (bits 0–10
  the record id, bit 11 a racer start point), +0x10 segment length /256,
  +0x12 z /8, +0x14/+0x16 half widths /256, +0x18 tangent /32767, +0x1C/+0x1E
  junctions, +0x20 bank, +0x21 grade (/128), +0x22 + lane lateral position
  (fraction /128 of the wider half width; lane 0 racing line forward, 1
  backward, 2–5 / 6–9 traffic), +0x2C + lane speed (m/s; the racing lines
  read 100 in every file checked). 0x34122: 32-byte junctions after 4 bytes
  (+0x04/+0x06 target point, +0x08/+0x0A target record, the +0x06/+0x0A
  pair for type 3; +0x0C flags: type nibble, bit 18 direction, bit 20 not
  for racers, bit 23 flip, bit 26 turn, bit 27 branch odds). Types in the
  shipped race files: 2, 3, 4, 5, 7, 10. 0x34123 (traffic intersections,
  Traffic) (in 122 of the 226 Routes files)
  and 0x34124 (race traffic teleporters, 0x108 bytes; in no shipped file)
  are read by the traffic AI (0x413800, 0x4111C0, 0x417980), not by racers.
* **Geometry** (0x5DD370 / 0x5DD4A0): P + L·l + F·d with F = (tangent,
  grade), L = (tangent turned right, bank); travelled backward a point uses
  the previous point's tangent reversed (0x401AA0) and its segment length
  (0x40D200), and lanes map 0→1, 2–5→6–9 (0x40D3A0). Along/lateral of a
  point: dots with F and L (0x40D160, 0x40D1B0). The z parts are
  + grade/128 and + bank/128 (0x5DD370: F = (tx, ty, grade), L = (ty, −tx,
  bank)); travelled backward − the previous point's grade (0x40D310 via
  0x5D8A00) and − the point's own bank (0x5DD4A0).
* **Start / road node search** (controller vt+0x30 = 0x42CDC0(arg) →
  0x413590(mode, arg) → 0x5E2E50; **verified**, ported as
  `RacerRoute::find`): 0x42CDC0 clears the node (+0x0C), the FuturePoint
  list (0x411BE0) and the 16 junction decisions (+0x14…, bits 0–1),
  then 0x413590 searches the race records (vt+0x70 / vt+0x74): each
  offers its nearest control point (3D, z from +0x12; radius 999999,
  0x783498) whose +0x0C has the mask's flag — mask 4 (bit 11) for an AI
  racer with the "Win road race" goal, 0x400 (bit 27) for a human racer
  (car +0x14 → +4 = 1), 0x200 (bit 25) for another goal kind; of these
  the nearest wins whose segment holds the car (0x40CD00: not past either
  half width; 0x40CE90: not outside the segment's length). The first
  pass takes only records whose coordinate range (+0x38 / +0x3C,
  0x5D44D0: inclusive, wrapping when +0x3C ≤ +0x38) holds the car's race
  coordinate (racer +0x8FC); the second all records, and with arg 1 also
  points off their segment. The racers' placement (0x5EF229–0x5EF24D)
  calls vt+0x30(0), then vt+0x30(1) when that found nothing, then
  0x40F750. Direction: for racer types 1 / 2 with an old node, the one
  the new point's tangent shares with the old node's way (+0.1 × the
  car's unit velocity, 0x43D090); then in a race (0x88F2BE) 0x40F750
  sets it from the record: forward unless +0x70 bit 1 (the race's
  direction on that record). Without an old node, outside a race, the
  car's velocity (speed > 1) or forward vector decides. **Clone**: the
  old-node blend is not used (in a race the record decides anyway);
  the walk's start passes no race coordinate.
* **Walk** (0x422220 / 0x41EC20): a point per control point (at a junction that is
  no turn a new point under 3 m on is walked past, but in a drag race the list's last
  point is dropped instead, docs/drag-stage4-parity.md 8.6); at a point's
  junctions, a type 3 against the direction ends the walk; a junction whose
  bit 18 differs from the direction is taken when it is type 2 (goal slot
  11, 0x40DDE0) or when, through 0x41CFA0 → goal slot 10 (0x41BCF0 →
  0x413CC0, **verified**; "Win road race" goal vtable 0x784768, slot 10 shared with "Win drag race" 0x7847D0) the racer
  takes it: type 2 (0x5DCBF0(0)), or type 2 / 8 for a human (car +0x14
  → +4 = 1), yes; a target record with +0x70 bit 10, types 5 / 7 / 10 or
  bit 20 no; in the hold (track +0xE4 −1 → 0x5EF000: racer numbers
  100–105, car +0x0A from the set-up's byte +1 at 0x604442, wait
  12 − 0.5·(n − 100) s, 0x802278 / 0x80227C; else race time ≤ +0xE4; the
  track table reads −1 on 128 tracks, 0 on 3) no; then a type 4 under
  0.5 (0.333 with bit 27; drag: 0x40DA10) and any other type by the
  skill row (controller vt+0x78 = 0x402580 → 0x610520, clamped 0–9) of
  the AI tuning (0x81CB60 + 0x68·row): r ≤ +0x524 takes a type 1,
  +0x524 < r ≤ +0x528 a type 0, +0x528 < r ≤ +0x52C none, above a type 0
  (0x413E33–0x413EC9). Every row reads +0x524 = 0 and +0x52C = 1; +0x528
  is 0, 0, 0.05, 0.2, 0.35, 0.5, 0.7, 0.7, 0.85, 0.9 — so the junctions
  the race load retypes to 0 (the target lane 20 m further on in 10 s)
  are taken by the better rows, type 1 never, and a human (row −2 → 0)
  takes neither. The random numbers are the world's (g_World +0xF8,
  0x5EA510 with ECX = 0x890080 at 0x413DCB and 0x413E57, verified; the
  clone used a default-seeded generator per racer until 2026-11-01, so
  every racer drew the same numbers, see Difficulty). The decision is
  kept. **Clone**: the walk's race clock was never set, so no branch was
  ever taken; `racer_think` now hands it the clock and the row's odds.
  The racer number (car +0x0A) read 100, 101, 102 for the three Quick
  Race rivals in world order (Wine), so the clone gives rival i
  the hold of number 100 + i (12, 11.5, 11 … s; `route::start_hold`;
  **inferred** for career races). Taking a junction XORs the direction
  with bit 23. A new point's s is the last's plus its distance along the
  last point's direction (0x40AE30); after a junction points closer than
  3 m are walked past (offline), a record's last point within 5 m is
  dropped (0x418C50: the walk's cursor still moves on to it, so the next
  step starts from the dropped point — the clone once re-stepped from the
  list's last point and stalled on reversed 4601 / 4606), and the gap is filled with points every 8 m along the
  last point's direction (0x418DA0: trunc(d/8) − 1 points, flagged +0x74)
  — after every new point that is not a turn entry, junction or not
  (0x4224A7 / 0x4224B6 → 0x4226E1; read under Wine, <work dir>: the grid's list
  had them inside record 476 and 655; the clone had them across junctions
  only until then).
  After the extend (0x422220) and the speeds (0x40A8C0), before the turns
  (0x422780; called at 0x4281AD), 0x41A180 bends them: from the point last
  before the extend (count − 1) backed past two unflagged points, each run
  of flagged points between unflagged i0 and i1 goes onto a per-axis
  Hermite curve (0x574840: a = t0 + t1 − 2Δ, b = 3Δ − t1 − 2t0) from P0 /
  P1, the lane points at s0 / s1 (0x4122C0), t = (s − s0)/(s1 − s0). The
  tangents (0x43CEB0) are scaled to 0.75·(s1 − s0) ((1, 0) when zero):
  the start P0 minus the line at i0 − 1 (i0's control-point direction,
  vt+0x20, with none before), the end the line at the next unflagged point
  after i1 minus P1 (i1's direction, vt+0x1C, with none). Only the lateral
  moves: 0x4123C0 gives the curve point's lateral off the control point
  less (lane offset + adjust), added to +0x04; lanes 2–5 (+0x08…) bend the
  same way, each on its own lane's places (ported since for racers as well:
  `FuturePoint::lane_adjust`, read by `lane_lateral`, `nearest_lane` and
  `lane_path`; native values of lanes 2–5 differ where the two walks differ,
  docs/drag-stage4-parity.md 8.6). The clone ports this as `RacerRoute::smooth`: run after the
  extend and the speeds, from the update's first new point backed past two
  plain ones, and at the route's start (0x418DA0 now only inserts the
  flagged points; the bend is the separate pass). The port that bent each
  point as it was inserted left a tournament rival stalled in
  `game --selftest` (S4_URL_3); with the pass, the rival races through.
  The selftest then hit an index past the end in the narrow phase's pair
  cache (`src/narrow.rs`: a cached vertex of a hull whose vertex count had
  changed); a cached index past either hull's end is now a miss.
  **Road node follow / re-search** (AIRoute_Update 0x428720, **verified**):
  with a list, the car is projected on it (0x419B20 → s, lateral) and its
  point found (0x40AB10). When s lies from the first point's s − 50
  (0x785B80) up to (not including) the last's, and the car's edge distance
  at that point (0x40ADA0, compared at 0x42882D) is at most twice the car's
  half extent (0x60C1B0 × 2; the clone takes 4.4 m) or the controller's
  vt+0x8C says so (the AI racer controller 0x785370: 0x404800, false;
  the traffic controller 0x785428: 0x570A70, true), the node follows the
  point (vt+0x18(0x401ED0 = the point's control point), 0x4120C0, node
  direction from the point's class) and the flag at [esp+0x11] is set
  (0x42886F). Otherwise (0x428973) the point index is −1 and the node
  searches again, vt+0x30(1) (0x42CDC0 above), every such frame; the
  list is then rebuilt from the node (0x428030 with index −1). With no
  list the node's own control point is tested the same way (lateral and
  along its segment, 0x428893–0x42895E). The clone's former restart took
  the walk's first direction (`self.backward`) and the nearest flagged
  point: after leaving the road it turned 4107's walk onto record 136
  forward against the race and the car drove back (and so on 4011, 4081,
  4103, 4124…); the restart now is the search above. The list keeps 20
  points behind the car (0x428030: ctl slot 10 always true) and extends
  to max(125 m, 3 s × speed) past the car's point with more than nine
  points, 40 at most. Checked: holding the branches, a walk from 4001's
  pole comes round after the track's length.
* **Speeds** (0x40A8C0): new points take their control point's lane bytes,
  then forward from the first new one v ≤ √(v_prev² + 2·a(v_prev)·Δs) with
  the car's acceleration curve (perf object +0x55C, `handling::acceleration`)
  and backward v ≤ √(v_next² + 2·decel·Δs) (0x40A6E0; traffic lanes ±0.2 m/s
  per m). Decel by class 12, 10.6, 9.3, 8 (SUV 10, 9, 8, 6, 0x7A4748/68),
  grip 3.7, 3.4, 3.1, 2.8 g (SUV 3, 3, 3, 2, 0x7A4758/78). Turns (bit-26
  junctions, 0x41EE20, five at most): v_t = √(g·9.8/max(|κ|, g·9.8/60²))
  from the circle through the line 75 m before, at and 75 m after the turn
  (0x5D4780 / 0x5D46B0; 60 near the list's end); the point before is held to
  √(v_t² + 2·decel·Δs), the one after to √(v_t² + 2·a(v_t)·Δs), then both
  ways again (0x422780). Corner points (0x41F8E0(turn, d, mode), d = ∓75
  in mode 1, 0x783C74; ∓10 in mode 2, 0x783C78, which only fills a local
  array): at s_t + d·0.5 (0x784260; s_t the entry's +0x130), on the
  point at or before that s behind, at or after ahead; 0x4122C0 adds
  extra = ±8 (0x783C9C) − the point's +0x04 adjust (side by the entry's
  +0x151), clamped to vt+0x10(0) − 5 on the right / vt+0xC(0) + 5 on the
  left (0x783C98; 0x40B1F0 / 0x40B2A0: the +0x16 / −(+0x14) half widths
  less lane offset + adjust); stored behind at +0xDC/+0xE0, ahead at
  +0xEC/+0xF0, with the plain lane line at s_t ± 75 at +0xD4/+0xD8 and
  +0xF4/+0xF8. The circle runs through +0xDC, the apex +0xE4 (written by
  0x41EE20) and +0xEC; the speed is 60 outright when the entry's +0x148 >
  count − 2. Then the four lane floats +0x138…+0x144 become 6.0.
  **Corner apex** (0x41EE20, decompiled and disassembled; a turn is taken
  only while +0xD0 < 5, else 0x41A090). The turn record at +0xD0 (0x80 bytes each) gets +0x148 =
  list count − 2 and +0x130 = s_t = the shorter of two 0x40AE30 lengths
  (0x41F175–0x41F1A6); +0x151 = side: 1 when [esp+0x28] > [esp+0x18]
  (0x41F3A7–0x41F3BB), which the decompile reads as the left edges (vt+0xC),
  else 0 (right edges, vt+0x10); +0xE4, +0xE8 and +0x104 take the apex
  values from the stack (0x41F870–0x41F88F). Lanes 2–5 give four clearances
  √(room²/2) − 1.2 against a minimum that starts at 3.535534; a lane point
  sits at s = clearance + 3.535534 on the edge ∓ clearance (0x4122C0). The
  list is trimmed while its last s > min + d − 3 (d the shorter of two
  0x40AE30 lengths) and grown at s + 3 by 0x418A50 while more than 6 m short
  of min + d (40 points at most). The new entry gets s = min + 3 and is
  walked to the apex (0x41EC20); on the right side, 0x40B0B0 picks the
  highest lane 2–5 whose flag is set, and that lane's point sets the walk's
  s (vt+0x2C), else the lane-0 point does. **Clone gap**: the clone takes
  the lane line at s_t and skips the trim and insert, so the apex is not
  ported. Still open: the two comparands' vectors (not traced), the x/y
  pairing of the apex stores, and lateral travel backward (the clone swaps
  the half widths).
  Only bit-26 junctions add turns, and a walk on 4001, 4013, 4101 … crosses
  none: the corners come from the lane bytes the race load rewrites (next).
  **Open**: the files set bit 26 on many junctions (4001F: 96 of 455, of
  types 7, 2 and 4), so which a walk meets wants checking again.
* **Free-roam post-load** (0x5E9E90 with g_RouteCoordRace 0x88F2BE clear,
  from 0x57A660 after 0x578070 closed the paths file's barriers
  `BARRIERS_<track>`, `PLAYER_BARRIERS_<track>`, `BARRIERS_CAREER<stage>`
  through 0x5D5020, which sets barrier +0x10 and +0x12; 0x5D4FF0 clears
  only +0x10; `RouteFile::finalize_free_roam(_paths)`). **Verified**: the
  clone's result equals the original's memory after the intro (career
  stage 1, L4RA) in every junction (key, targets, flags; 3289, live
  0x883DB4 = 3245), every record's +0x70 and every point's +0x08, +0x0C
  and +0x1C/+0x1E (test `free_roam_matches_the_original`). Junction
  layout in memory: +0x00 f32 sort key, +0x04/+0x06 target / own point,
  +0x08/+0x0A target / own record (type 3: +0x06/+0x0A both), +0x0C
  flags: low nibble type "k = 0" (and k = 2), bits 4–7 type k = 1 — both
  copied from the file's bits 8–11 / 12–15 first —, bit 18 direction, 23
  flips, 26 turn. 0x5D43D0 and 0x5D4390 count only indices below
  0x883DB4. Steps:
  1. 0x5D98D0: live = table length (0x883DB8), keys 0, types from byte 1;
     records lose bit 1.
  2. 0x5E41B0: 0x5D9660(1) (bit 6 set; 7, 9, 11 clear); per record bit 1
     clear, point coordinates (+0x08) and +0x38/+0x3C zeroed, bit 6 clear
     when a short at +0x44 is 4000 (none in L4RA). 0x5D9440 lists bit 6.
  3. 0x5E27E0: a junction on an unlisted record → 0x77, dropped; into an
     unlisted record: each type k of 0/1/4/5/6/8/9/10 → 7; 2 → 7 and cut
     back (0x5E1600) for k = 0/2 without the record's bit 7, else both
     types 3; dropped when all of k = 0..2 read 7. Keys: the own point's
     race-road distance (0x5D9E40 +0x24; 0 in free roam), a dropped one
     10 · 0x88F2B0 + index (0x88F2B0 is 1.0 in free roam: **verified**,
     0x57A6CF calls 0x5D9A70 on the route data 0x88F298 when 0x89E7B0 = 0,
     which zeroes +0x18…+0x433 and stores +0x18 = 1.0, +0x1C = 1.0,
     +0x20 = 128.0). The bit-9 branches cannot run (bit 9 clear).
  4. 0x5E2BE0: the whole table heap-sorted (0x5D4100, Numerical Recipes'
     hpsort, not stable; comparator 0x5D42C0 key a − key b), the control
     points' slots (records id < 1500, indices below the length)
     renumbered; live −= dropped.
  5. 0x5DE9A0: on listed records with +0x0C 0, a point's first race link
     (type 2 either nibble, or low 3) with bit 18 clear goes to the last
     valid slot, the first with it set to the one before.
  6. 0x5D9720: records lose bit 12; listed ones whose box overlaps a
     closed barrier's box (0x43D4C0 strict) get it.
  7. `while 0x5E2140()`: per live junction and type k not dead (0x5DCBA0:
     7, or 8 for k = 0): 0x5E1CB0 "barred" (a closed barrier, 0x5D2E70 /
     0x5D4D60 segment test, on the step into the point or — on a bit-12
     record — on a step within 200 m along 0x5DD560 before a junction
     off to another record; k = 1 ignores barriers with +0x12) from the
     own point (bit 18 inverted) and from the target (bit 23 applied):
     types 0/1/4/6/9 barred only from the own point → 2, from the target
     → 7 (both: also cut back); a link 2 barred from the target → 7 and
     cut back. 0x5E1600 walks the own record from the own point
     (forward for bit 18 set): junctions the same way into unlisted
     records → 7 until one into a listed record becomes the link (2;
     0x5E14C0 picks between turns, bit 26 — with all coordinates 0 it
     keeps the first) or already is one; junctions the other way whose
     reverse (0x5D8770) is live and listed → 7, links recursing (depth
     ≤ 100).
  8. 0x5D9600: records lose bit 8, points bits 11, 18, 25–28.
     0x5DE5D0 → 0x5DE250 from each live type's target walks the record
     marking points (k 0: bit 18 / 11, 1: 26 / 25, 2: 28 / 27 by
     direction ^ record bit 1) and the record (bit 8) in stretches up to
     each point with live junctions, stopping at a marked point, the end
     (last stretch unmarked) or when the last junction checked there is a
     link / type 3 against the direction. 0x5DE8E0: records without bit 8
     lose bits 6, 7, 9; relisted.
  9. 0x5D8870: junctions whose own or target record is unlisted dropped
     (key as in 3); sorted again; live −= dropped. Live types 5 / 10 set
     bit 2 (bit 18 = own record's bit 1) or bit 3 on the record they lead
     to.
  Then 0x5DDFD0 / 0x5E8790 (speeds, below). The encounter racers' copy
  of RoutesFreeRoam goes through the same post-load before its speeds
  (`Encounters::new`, career stage's barriers).
* **No pruning in races** (**verified**, static and in memory). 0x5E9E90
  runs the block above only when g_RouteCoordRace (0x88F2BE) is clear or
  0x88A330 (route manager 0x88A320 +0x10) is 1. In a race 0x57A6F2 builds
  the race road first (0x5E6A50 → Race_BuildRoute 0x5E5BC0 sets +0x26 = 1),
  so the race branch (0x5E41B0's race half, 0x5E23E0, 0x5E1E90, 0x5E2760,
  the bit-9 paths of 0x5E27E0) would need 0x88A330 = 1, and nothing sets
  it: its only references are reads (0x57F500, where a non-zero value
  skips loading the route file at all, and 0x5E9F2F), and the only store
  through the manager's +0x10 is the constructor 0x5E3FA0 (from the static
  initialiser 0x77E760) clearing it. Observed under Wine in a Quick Race
  circuit on 4014 forward: 0x88A330 = 0, the junction table's live count
  0x883DB4 = its length 0x883DB8 = 444, every record's +0x70 and every
  control point but its lane speeds (+0x2C…) byte-equal to
  `Routes4014F.bin`, and 14 junctions differing only in their type nibble
  (the junction-type adjustment below). So a race's network is its file
  plus the racing speeds and that adjustment; the race branch is dead
  code and is not ported. The race's traffic reads the same route
  manager, so the clone hands it the race's processed file.
* **Race road lap** (Race_BuildRoute 0x5E5BC0, **verified**): it clears
  the block like 0x5D9A70 (+0x18 = 1.0, +0x1C = 1.0, +0x20 = 128.0), then
  +0x24 (0x88F2BC) = road record byte +3 ≠ 0 (the road loops), +0x25 =
  the direction argument, +0x28 = the record's u16 +0; at 0x5E5EFC
  0x5E48C0's length goes to +0x38/+0x40 and to +0x18 (0x88F2B0, the lap
  length), half of it (× 0.5, 0x784260) to +0x1C (0x88F2B4), and +0x26
  (0x88F2BE) = 1. Observed on 4014: 3001.135 and 1500.5675.
* **Racing-line speeds at race load** (0x5E9E90 → 0x5DDFD0 → 0x5E8790(50);
  `AiRoutes::racing_speeds`). The 100s in the files are overwritten:
  - *Inputs.* Speed mode 0x88E9A8 (route manager 0x88A320 +0x4688) from
    the track record's +0x94 flags: bit 0 or 2 → 2, bit 1 → 0, bit 3 → 1,
    bits 11/12 → 4, bit 13 → 5 for track numbers 0x125D–0x1261, else 6
    (0x5E9EF9: 5 + (n ≠ clamp(n, 0x125D, 0x1261))), else 3 (circuits 4011–4015:
    0x71 → 2). Class +0x4674 = the lowest performance class (0x5B5340) among
    the AI racers (type 2), else among the humans, else 3 (+0x4674 has no
    other writer but the constructor 0x5E3FA0). 0x5B5340 sets the class up
    (0x5B4FD0) only when its byte (info +0x24) is 0xFF. In a career race
    the AI racers' bytes still read 0 when the track load (0x57A73B →
    0x5E9E90) runs, so the class is 0 (**observed** under Wine, the infos
    polled every 5 ms through the load: a stage-4 URL circuit, track 4714,
    and a stage-3 Street X, 4603: bytes 0, then +0x4674 = 0 and 0x876D2C =
    3.7 / 3.515 (class 0, modes 6 / 4), then the cars' own class 2 / 3
    about 0.1 s later; the URL rivals' path points carry the class-0 lane
    bytes). The writer of that 0 is **open**. A Quick Race keeps the cars'
    own class (4014: class 3, the lane bytes read under Wine). Clone:
    `field_class`. Table +0x4670
    (0x88E990, freed after): per class GLOBALB chunk 0x34A33 (0x10C rows,
    kept at 0x86B338): +0 top speed (100.6, 97.9, 95.2, 91.2 m/s), +0x0C 64
    accelerations over 0..top (0x5A78F0, read by 0x5764E0). +0x4678 = 1
    (speeds), +0x4679 = 0 (no lane-position optimisation). 0x5D4C40 copies
    row +0x4688 of 0x7A4668 (8 floats) to 0x876D10: +0x18 braking ×
    {12, 10.6, 9.3, 8}[class] (SUV {10, 9, 8, 6} when an AI racer drives an
    SUV) → 0x876D28, +0x1C grip × {3.7, 3.4, 3.1, 2.8} (SUV {3, 3, 3, 2}) →
    0x876D2C; the rows' +0x18 are all 1, +0x1C 1, 0.7, 1, 1, 0.95, 1.25, 1.
  - *Line points.* 0x5DDFD0 sets lanes 0 and 1 of every listed record
    (+0x70 bit 6, not bit 9; lane flag +0x74 / +0x7C clear) to the top
    speed (0x5D9350). 0x5E8790 builds a 400-byte entry per control point of
    those records, records by id (0x5D4A30; entry +0 the CP, +4/+6 the
    lanes' sample counts, lane blocks at +0x10 and +0xD0, three 64-byte
    samples each): a sample holds its entry (+0), lane (+4), links ahead
    (+0x20) and behind (+0x24) to another sample, the near links +0x28 /
    +0x2C (0x5E5AB0's results), its point +0x30 and κ +0x08; at first one
    sample a lane linked to the neighbouring CPs' first samples (lane 0
    with the points, lane 1 against). The join pass (0x5E8B05–0x5E8EC1,
    **verified**), junctions in table order, not 7/8, not bit 26, two
    different records without bits 9/10, source record +0x0A point +0x06
    on lane bit 18, target 0x5D86E0 on bit 18 ^ bit 23, both points
    carrying the lane flag (+0x0C bit 18 lane 0, bit 11 lane 1):
    (a) when the partner junction (0x5D8770) is a race link (2) and the
    target lane has a sample behind, that link and the old point's links
    ahead (every sample of its entry on that lane: +0x20, +0x28) are
    cleared; (b) a race link (2) clears the links behind (+0x24, +0x2C) of
    every sample of the source's old point ahead, then becomes the
    source's first sample's link ahead; the target takes the source
    behind in its first sample, or — when that has one — in a new sample
    copied from its last (0x5D4CF0), the source linking to that one;
    (c) any other type adds a sample to the source (a copy of its last)
    linking ahead to the target's first sample, which takes it behind,
    or — when the target has a sample behind — to a new copied sample on
    the target that links back. The sample's point (+0x30, 0x5E3EB0) is
    the lane's lateral position off its control point (0x5DD370 /
    0x5DD4A0). So a race link cuts the lane where it leaves: on 4014
    record 428's first two points keep the class top speed (91), cut off
    by the links at its third point. Clone: `racing_speeds` links samples
    (node, index) the same way; test `race_4014_speeds_match_the_original`
    reproduces all 1641 points' lane bytes and all 444 junction flags of
    the Wine memory dump (`$OB_PRIVATE/wine/compare/race71/mem`).
  - *Curvature limit.* With 0x876D24 = 900 (0x7A4790), 0x5E5AB0 finds per
    sample the neighbours ahead (0x5E5930 along +0x20) and behind
    (0x5E59F0 along +0x24): the first of ≤ 10 on the far side of the point's
    direction of travel (0x40CBC0) at 30 m or more, else the last tested.
    0x5D4AE0: κ = 1 / radius of the circle through behind, here, ahead
    (0x5D4780, 2D, radius ≥ 0.001; 0 in line; needs both links). 0x5D8F10:
    v = min over samples √(g·9.8 / max(|κ|, g·9.8 / top²)), written as
    trunc(v) to CP +0x2C + lane.
  - *Braking and acceleration.* With 0x876D24 = 1 the neighbours are the
    adjacent points. Up to 10 rounds until nothing changes: across the
    junctions, then in entry order lane 1 brakes (0x5D8FD0 → 0x5D4B90: v ≤
    √(v_ahead² + 2·braking·d)) and lane 0 speeds up (0x5D9150 → 0x5D9080:
    v ≤ √(v_behind² + 2·a(v_behind)·d), a the class's curve), then in reverse
    lane 0 brakes and lane 1 speeds up; d is the 3D distance between the
    line points, speeds whole m/s throughout.
  - Result on 4013 (class 3): the hairpins at (444, −51) and (41, −6) read
    34–36 and 44 m/s, the straights up to 100.
  - **Partner junction** (0x5D8770, verified except as marked): for a junction J, the target
    point is 0x5D86E0's (type 3: +0x06/+0x0A, else +0x04/+0x08). If that is
    J's own source point the answer is J; otherwise it is the first junction
    K of the target point (0x5D43D0) with bit 18 ≠ (bit 18 ^ bit 23 of J)
    whose target (the same type rule on K, 0x5D8811–0x5D8847) is J's source point
    (0x5D8841–0x5D8852; K is returned at 0x5D885C, else 0 at 0x5D8865). The join pass of 0x5E8790 calls
    it (0x5E8CC2); verified (0x5E8CDA–0x5E8D32): when K is a race link (type 2; `sete` at 0x5E8CDA) and the lane J would fill already has a link (its +0x34 pointer, cleared with +0x3C), the old record's lane is unlinked first: the dwords at each sample base −8 and base are zeroed (samples 0x40 apart from record +0x38 + lane·0xC0; count at +4 + 2·lane), then it links (0x5E8D34–0x5E8DF8, only partly read: it re-tests type 2 and clears the +0x30 slot's old record the same way, with bases at +0x3C, before the link stores). The other callers are 0x5DCCD0, 0x5DE5D0
    and 0x5E1600. Ported (the join pass above).
  - **Race distance after t seconds** (0x5DDDF0, verified): from the lane's
    first sample (this + 0x38 + lane·0xC0) it walks the samples, adding the
    3D distance over the mean of the two lane speeds (cp +0x2C + lane) each
    step and 3 s per type-2 turn junction (bit 26) whose bit 18 equals the
    lane index; that junction moves the walk to lane bit 23 ^ bit 18. At t it
    returns the sample's cp +0x08 (the race coordinate), or *this + 0x08 when
    the chain runs out. Its only caller is 0x5E8790 (0x5E9557, 0x5E9571), and both calls pass t = 10.0 (the float at 0x7A4798, loaded at 0x5E9545 and
    0x5E955C, pushed at 0x5E9552 and 0x5E956C).
  - **Junction-type adjustment** (0x5E8790, after the braking rounds; verified
    except as marked): when 0x88F2BE ≠ 0 and the track row whose +0x8A equals
    0x88F2C0 has +0x94 bit 1 clear (0x5E92E1–0x5E932F), each junction of type
    0, 1 or 4 (0x5E9369–0x5E937A; the type is the low nibble of +0x0C) gets
    d = R(0x5E9571) − R(0x5E9557), the two
    0x5DDDF0 results, each at t = 10.0 (the float at 0x7A4798, pushed at 0x5E9552
    and 0x5E956C). The lanes passed are verified (0x5E9499–0x5E94A3: the first call takes bit 18 of the junction's flags, the second bit 18 ^ bit 23; no write to edx in between, 0x5E94B6–0x5E9545). The record test before them (0x5E9421–0x5E94D5) needs each point's lane flag, bit 18 on lane 0 and bit 11 on lane 1, set in its own +0x0C. The wrap
    applies when 0x88F2BC ≠ 0: d < −T adds the lap 0x88F2B0 and d > T subtracts
    it, T the float at 0x88F2B4. d > 20 (0x7A479C = 20.0) makes the junction
    type 0 and d < −20 type 1 (0x5E95B1–0x5E95EA), otherwise nothing changes.
    No speed-mode gate exists: the block (0x5E92E1–0x5E933F) is gated by 0x88F2BE ≠ 0, the row's +0x94 bit 1 clear and [0x883DB4] against ebx (entered by falling through at 0x5E92DB when bl = 0, or by the jump at 0x5E9095 when [esp+0x2c] ≥ 10); the only compare against 1 in
    0x5E8790 is the junction-type test at 0x5E9377. No instruction stores to 0x88F2BE or 0x88F2BC by absolute address or immediate (they lie at +0x26 and +0x24 of the block at 0x88F298, which the code also loads as ecx at 0x40352E and 0x4094EB),
    so their writer is not traced.
  - **Lane optimisation** (+0x4679): dormant. The only store to +0x4679 is at
    0x5DE0D3, which clears it (+0x4678 = 1 is set at 0x5DE0DA), so the
    50-round pass (0x5E3EB0, 0x5E5AB0, 0x5D4AE0, 0x5E3E50, 0x5D91F0) never
    runs on the shipped racing path; the clone does not port it.
  - Each round starts with a pass over the junctions (0x5E90A0–0x5E924A:
    not 7/8, not bit 26, two different records without bits 9/10, no lane
    flag test): the source's lane (bit 18) brakes and the target's (bit 18
    ^ 23) speeds up, 0x5D8FD0 / 0x5D9150 with flag 1 — which only repeats
    the same near-link test (the loop passes the near link, not the walked
    one), so it acts like flag 0. Ported (`across`).
  - **Race distance after t** and **junction-type adjustment** are ported
    (`racing_speeds` with a `RaceLap`: the race's road length and loop
    flag, on a track without +0x94 bit 1); 0x5DDDF0 walks the near links
    (+0x28) from the lane's first sample and a turn link's jump lands on
    the target lane's first sample. Free roam builds no race road, so no
    adjustment there.
* **Speed at s** (0x411DD0): between the point at or before s and the next
  on another control point, by the first one's control point distance.
* **Path** (0x42B340 → 0x41D530, and 0x42B340 → 0x420D40 at 0x42B555 in one branch; 0x42B340 itself calls 0x41D160 at 0x42B4CC, 0x428DF0 at 0x42B4FC, 0x40FBE0 and 0x401D40; AIActionJustDrive, vtable 0x785684 (ctor 0x4235A0 passing 6 to base ctor 0x4234A0; slot 1 0x40E3E0 passes "AIActionJustDrive" to 0x578CF0; slot 2 0x42B570 calls this path at 0x42B579): the racing-line
  points of the next 20 points, offset from the car's own lateral back to
  the line at 0.05 m per m (0.2 in Street X; see Race types → Street X,
  where the clone ports it for Street X only);
  the avoidance re-plan (time to collision <
  0.75 s, new paths ≥ 3 s) is not modelled. In a drag race racers drive a
  traffic lane of the same FuturePoints instead (see Race types → Drag,
  Lanes).
* **GetUnstuck** (AIActionGetUnstuck, type 9, vtable 0x785784;
  `driver::Unstuck`). The goal's chooser (0x42D0F0 = "Win road race" slot
  +0x38, run by 0x402360 when the controller asks for type 22) creates it
  (0x426230 → ctor 0x423C30: priority 500, timeout 5 s 0x783628, minimum
  run vt+0x98 0x4083E0, 1.3 s by default 0x783B48) when no type 9 runs,
  the current action's priority is ≤ 500, the last one started over 6 s ago
  (vt+0x94 0x4083A0: controller +0x708 against 0x783B50) and the stuck test
  0x40E7C0 returns 500 (its argument is controller +0x54, 0x42627F; the
  chooser stamps +0x708 only after a 500, 0x42D187): −1 first while
  the racer is in its start hold (0x5EF000(car), ECX the race status:
  outside drag and track 1099, racer number car +0x0A in 100…105 and
  the race clock since status +0x10 at most 12 + 0.5·(100 − n) s,
  0x802278 / 0x80227C; **read** under Wine, Quick Race 4014: the rivals
  in world order are 100, 101, 102), and −1 when 0x60BF00 is false (ECX =
  0x89E970[the word at planner + 0x30·(+0x308 slot) + 0x34], the car's own
  road entry's world object, whose +0x1C is the car (read: table entries
  1…16 are the world list's cars in order): car +0x424 > 2, more than two
  wheels down; true with no car) — **ported** 2026-11-01 (
  `Unstuck::update`'s `held`, `route::start_hold`; rival i gets number
  100 + i, **inferred** outside Quick Race); then more than 2 s since car +0x9C0 (0x7836F8), and
  either the car is over 20 m off its route (0x401C10, 0x7836F4), or (racer
  under 25 m/s 0x7836EC, type 4 under 20 0x7836F0) 0x40DF50(car, 1) = 1,
  or a contact (car +0x3C list) on the front or a side (0x402A40 classes
  0, 2, 3) is 0.4 s old (0x7836E4), touched within 0.25 s (0x784D28) and
  the car's +0x430 speed ≤ 5 (0x7836E8). The chooser stamps +0x708 (the
  6 s) before the constructor, and the manager (0x429FB2: `cmp [new
  +0x200], ebp; jg`) keeps the proposal only when its 500 is above the
  running action's priority, so it is deleted under an AccidentRecovery
  (priority 500, `Unstuck::update`'s `running`): a rolled racer is not
  backed up, its recovery runs its 5 s to the reset (rivals_keep_the_race_route,
  Sprint 4107: two racers on their sides restarted the recovery each time
  a GetUnstuck took over after ~4.5 s and never reset). Car +0x9C0 is the tick the car
  was last placed (0x5F52D0 from the placement calls 0x5FBD70 /
  0x5FBEE0: the grid and AI resets; 0 from the car set-up 0x60430D /
  0x6046B4); the clone's rivals are placed as the countdown starts, so
  it reads the race clock + the countdown. Its begin (0x420940): target speed
  +0x1F0 −15 m/s for controller type 3 (0x7836D8), else −6 (0x7836DC),
  along a path from the racer's route (0x404CD0, 0x41D530); the driver
  reverses with the steering mirrored. It ends (0x403120 → 0x401990) on the
  timeout (slot 8 0x403250), or after the minimum run on slot 7 (0x40E9F0:
  a rear contact, class 1, held 0.4 s at ≤ 5 m/s; or 0x40DF50 = 2) or slot
  6 (0x414440: more than 2 s since car +0x9C0, facing the facing
  point below (cos ≥ 0.9, 0x7836E0) and no front contact held 0.4 s at
  under 5 m/s).
  * 0x40DF50(car, check) (`driver::leaving_road`), from the car's route
    point (car +0x2C → +0xC, travelled forward when +0x10 is set) with
    half widths +0x14 left / +0x16 right: under 1 m/s (+0x430) the point
    2 m ahead along the heading (+0x40C), by the point's own tangent, past
    an edge and further out than the car → yes. Moving: the car and the
    point 0.1 s ahead along its velocity (+0x70), right of the way of
    travel (0x40CC40; the half widths swap backward); time to the right
    edge = room / lateral rate when that rate > 0.001 and both rooms > 0,
    else 99999 (the leftward branch repeats the same test, so moving left
    is never timed); no when that time > 1 s and neither side is past its
    edge and getting worse. A yes with forward speed (+0x42C) > 0 returns
    2 if the forward vector (+0x90) is more than 70° off the point's
    tangent (cos ≤ 0.342, 0x783668), else 0; otherwise 1, unless `check`
    and the controller's point (+0x30 vt+0x80) lies within 60° of the
    velocity (the heading under 1 m/s; 0x4036E0 with 0.5) → 0. The start
    (0x40E8D3) wants 1, slot 7 (0x40EA73) 2, both with check 1.
  The contacts are the car's contact records (car +0x3C through
  0x5A0830, track and car alike; `CarState::contacts`): "0.4 s old" is
  record +0x08 (made), "touched within 0.25 s" +0x0C (last push; only the
  start reads it — slots 6 and 7 read +0x08 alone, a live record being
  pushed within 0.3 s anyway); speed is car +0x430 (|v|), ≤ 5 m/s as
  under 5 or within 0.001 (0x4010C0) for the start and slot 7, strictly
  under for slot 6; slot 6 also wants the minimum run (+0x218 / +0x210 /
  +0x214). 0x402A40: the record's point (+0x30) in the frame of travel
  (0x591FD0, see the pinned rule); within 95% of the car's half width
  (0x5EC260's box) it is front (0) or rear (1) by its sign along the way
  of travel, else a side (2, 3). Off route (0x401C10 on 0x40CC40): l = the
  offset right of the way of travel off the route point; l − right half
  width when l > 0, else −l − left (+0x14/+0x16 /256, swapped backward,
  0x401BC0) — the distance past the road's edge (`driver::past_edge`).
  The facing point (0x4143F0, `RacerRoute::facing_point`): 0x4032A0
  takes the FuturePoint list entry (pointers +0x324, count +0x320) 3 past
  the car's (the slot +0x308 block, racer + 8 + 0x30·slot, its +0x28
  index) — 1 in a drift or Street X race (0x89E7D9 / 0x890104 / 0x89E7E3 /
  0x89010C) — clamped to the last; 0x4122C0 places it at its own distance
  (+0x54, so along = its +0x58 offset past its control point +0x5C/+0x5E)
  on the racer's lane (+0x30C: lateral = the lane's offset 0x401C40 + the
  point's lane adjustment, +0x04 for lane 0, +0x08… for 2–5), 2D.
  0x40DF50's controller point is the driver's (car +0x30 = controller
  +0x08, 0x5EBB8E) vt+0x80 (0x405BC0): driver +0x224, the look-ahead point
  0x415F80 writes — last frame's, the controllers thinking before the
  drivers. vt+0x98 (0x4083E0) gives 1.6 s (0x783B4C) when the controller's
  vt+0x2C (0x40D4F0: the distance past the road's edge at the route point,
  0x401B70 forward / 0x401BC0 backward, 0 without one) is above 0 and its
  running mean +0x7A4 (5 samples, 0x7E957C) is full and under 0.2 m/s
  (0x7E9574). The samples (0x42C2F6, in a coordinate race 0x88F2BE while
  0x89E7B0 is neither 0 nor 7 — 7 is set at 0x6071AE) are pushed whenever
  the timer +0x7A0 (1.5 s from the constructor, 0x7E9578; 0x577320 counts
  it down) runs out: racer status (car +0x1C) +0x18 × the road length
  0x88F2B0. Status +0x18 is the progress rate (0x5F7E84): each frame
  0.333 × itself + 0.667 × (Δ progress +0x14) / dt (0x7A5694), laps a
  second — so the mean is metres of race a second (`Laps::rate`; the
  clone treats every race as a coordinate race, **inferred**). 0x40DF50's
  third caller is GetUnstuck's own vt+0x38 (0x423E20, after its lane path
  0x420D40, not in drift or Street X, once +0x218 is set): when 0x40DF50
  (car, 0) = 1 (reversing out past an edge) and the car's lane place is
  ahead of its slot's (0x60C000 > 0) the target speed becomes 0x40DEE0
  (car, 0.2) — |v| − 0.2 × a braking rate from the car's specs (+0x20 →
  +0x264/+0x268/+0xDC/+0xFC/+0x110) — and the rest of that update
  (0x424083 …) is skipped; not ported.
* **Target speed** (0x41C5E0 offline): skill = the entry's difficulty byte;
  every +0xAD4 (5 s) the offset is redrawn as rand(2j) − j with j the row's
  +0x584 (40 … 20 m) using the racer's random numbers; v = speed_at(s_car +
  offset) (30 when 0) × the row's +0x538 (0.74 … 1.00) × the catch-up; the
  goal's cap (1e9) never binds. Rows: GLOBALB chunk 0x34B00 (0xAE0 bytes),
  0x68 per skill.
* **Catch-up** (0x4172D0, refreshed every +0xAD8 = 0.3 s): with the option
  (0x89E7F8) off 1. Its writers are the race-type defaults (0x4FEF80,
  0x4FEFF0, 0x4FF070, 0x4FF0E0, 0x4FF160 by 0x83A63C, from the test-run
  set-up 0x526980) and the Quick Race set-ups (0x526730 / 0x526840; the
  mode switch at 0x53F225 … and 0x601E34 copy the mode record's option,
  byte +0x15 of 0x83AAB4 + 4 + type × 20, +0x119 for type 12); the career
  set-up 0x5322F0 does not write it, so a career race runs with the
  option on as the boot leaves it (**observed** under Wine: career
  S3_CIRCUIT_12 from a stage-3 save, Medium, the human parked at the
  grid: controller +0x770 of the three rivals 0.651–0.677 from ~20 s on,
  0.795–0.83 with the human driving in the field; see
  docs/race-ai-parity.md "Rival pace in a career race"). On: after the row's +0x580 seconds (3 … 19), 0x4084A0's
  bonus — curves k = alt + 2·(laps ≥ 4) and k + 4 (11 points at +0x58·k;
  street X +0x2C0/+0x318) at the gap (m, + AI ahead), lerped by skill from
  k at 0 to k + 4 at 9 (verified at 0x4084A0: the call at 0x4085DA is 0x576C70(0, k, 9.0, k + 4), so k is the 0 end; street X 0x2C0 at 0 and 0x318 at 9; x = max(0, the controller's vtable +0x78 result), taken as the skill, the slot's provider not resolved) — plus clamp(−0.005·(v_ai −
  v_human)/(0.01·gap² + 1), ±0.05); the place tables (+0x424/+0x464) are 0.
  1 + bonus, faded by the curve at +0x504 ((0,1)(0.33,1)(0.67,1)(1,0)) over
  the race; the controller keeps it (+0x770) and the car gets max(it, 1) as
  grip (+0x468). **The whole refresh** (verified from the disassembly
  0x4172D0–0x41774B, 2026-10-31, aidrive117; Ghidra has no procedure there):
  it is the racer controller's slot +0x80 (vtable 0x785370 +0x80), called
  by the controller's think 0x420810 at 0x42082A every frame. It runs (else
  the multiplier is 1; 0.75, 0x783B74 at 0x417337, every frame when race
  type 0x89E7B0 is 0 — free roam, so career free roam's encounter racers
  aim at 0.75 × their route speed, 0x41C8A8; grip max(0.75, 1) = 1; clone:
  `Encounters::think` outside an outrun; the free-roam value is not read
  under Wine; type 7 with the racer in first keeps 1) when controller
  +0x744 is set (1 from 0x42070B; 0x40D800 clears it and fixes the grip,
  0x40D830 sets it and grip 1), car +0x18 is clear, outside drag and
  drift. Every +0xAD8 s it clears the picked human (+0x748); when 0x81CB98
  is 0, 0x89E7C0 > 0, the current action's +0x1FE is set (1 from the
  action base 0x4234A0) and the race time (0x890118 +0x14 ticks) has reached
  the row's +0x580, it picks the human (0x8900B4, or by place of two) and
  B = 1 + s·out1 + out2 from 0x4084A0, s = 0x89E7FC[the human's index] when
  0x401930 (street X: 0x89E7E3 or 0x89010C) is false or tuning byte +0x421
  is set (else 1). 0x89E7FC / 0x89E800 are 1.0 in the career set-ups
  (0x5267C2, 0x5268A8, 0x526A28) and tuning +0x94C + 4·(4·difficulty
  0x89E7F4 + the human car's class 0x5B5340) in the Quick Race set-up
  (0x53F306 / 0x53F331); the shipped table is all 1.0. **Line-speed term**
  (0x41757B–0x417676), with the option 0x89E7F8 on, a human picked and
  (outside street X or tuning byte +0x422 set): v = the human's player
  +0xA0 mean (0x5EB210: +0x0C of a 10-sample running mean 0x579F30(10),
  pushed once a second by the player update at 0x602842 while the flow
  state is 4 or 5 and the car has a road node, 0x40F690: car +0x430 over
  the node's control point byte +0x2C — the racing-line speed — or, with
  that 0, 1 when moving at 0.01 m/s or more, else 0; the timer +0xA4 is
  1 s, 0x7A551C); f = the 2-point curve 0x413390(pair, 2, 0, 9.0) at the
  skill (0x5764E0, clamped) with pair = tuning +0x4CC + 8·place when v < 1,
  else +0x49C + 8·place (place = the human's status +0x0A, 1 for the
  leader); then B × max(1 + (v − 1)·f, row +0x53C, street X +0x544)
  (0x401050 = max). **Fade** (0x41767C–0x41771A, outside street X or byte
  +0x423): (m − 1)·curve +0x504(status +0x14 / max(laps, 1)) + 1. The
  shipped bytes +0x420…+0x423 are 0, so a street-X race takes none of
  these. **Checked under Wine** (Quick Race circuit, Resort Loop, Medium,
  catch-up on, traffic minimum, the human parked at the start): rivals of
  skill 3 / 3 / 4, 1035 / 1062 / 1130 m ahead, read +0x770 = 0.6267 /
  0.6267 / 0.6514; the formula gives 0.6267 and 0.6514 (gap curves 0 and
  4: −0.2167 / −0.2056; human mean 0, place 4 → pair (1.27, 1.0), f 1.18 /
  1.15, so the row floor 0.80 / 0.82 wins). Clone: `catch_up_multiplier`
  (route.rs) with `CatchUp::line` / `scale`, the human's mean in
  `Race::human_line` (game.rs `step_race`; the road node is the human-mask
  search 0x413590, see Racer +0x8FC for what is still open there).
* **Racer movers** (verified 2026-10-31, aidrive117: the racer
  controller's update block 0x42C0C7–0x42C2DE, its contact handler
  0x402450 (slot +0x38) and world notice 0x4022D0 (slot +0x10), 0x5F4410;
  **seen under Wine**: in a Quick Race circuit the rivals read car +0x28
  = 3 after the green and 4 once out of the loaded world, the human 1;
  the round-12 log, "AI racers' mover", has all three on 3 from the frame
  the 5 s hold ran out). The AI racers are not on the physics mover for the race:
  before the start (0x5EEF00 false: not 0x89E7B8 > 0 with race status ≥ 3,
  nor status 4) and
  outside drag the controller's timer +0x734 is set to 5 s (0x783518) each
  frame; a done racer (0x5EE260: status +0x10 and a race type) is put on
  mode 1; while the timer runs (0x577320 by dt) on mode 1; when it runs
  out, a car with fewer than four wheels down (car +0x424) gets one more
  second (0xFA0 ticks), else, with the world under it loaded (car +0x5CC),
  it takes 0x5F4410's mode — for an AI racer (racer +4 ≠ 1, +0x900 clear)
  1 when the timer runs (slot +0x50 = 0x41E510), in drag (0x89E7D8 /
  0x890108) or when 0x401950 holds (drift 0x89E7D9, 0x890114 or 0x890104,
  not 0x890108), else 3 (car kind +0x24 = 3 or 5), 0 for kind 0; 1 when
  0x8650AC is set. The contact handler (every resolved contact, 0x5F4970):
  the other car's +0x18 (the local human) or its flag → timer 5 s
  (0x78349C) and mode 1, else 0x5F4410's mode; mode 4 when +0x5CC is null;
  a mode-4 car also gets +0x730 = 0x783510 s. The world notice: unloaded
  → mode 4 when the mover's slot +0x10 agrees; loaded → no mover change
  (0x4022F8 only touches 0x5ED5D0 under +0x77D). Mode 4 is 0x5A9040
  (vtable 0x7A214C, update 0x59EC90): it runs the car along its path at a
  distance s advanced each frame, heading from the path, the body set on
  the road (0x5F5020) where the world is loaded, wheels posed, and car
  +0x42C = the speed — no tyres, no collision response. Under Wine a
  rival stayed on mode 4 at 210 m with the world under it loaded again.
  Clone: `racer_mover` (game.rs, the hold in 1/4000 s ticks as 0x577320
  counts it, run before the think; see "AI racers' mover" in "Driving vs
  the original, round 12"): modes 1 and 3 (the MeloMvr, `nfsu2::melo`,
  with the car's rating as the traffic's); a contact with the human's car
  switches back inside the collision step (`CarState::human_hit`); the
  driver's mode follows (`AiDriver::mover`: 3 → mode 0; 4 on the belt).
  Mode 4 is ported (2026-11-01): see BeltMover below.
* **BeltMover** (mode 4; ctor 0x5A9040, vtable 0x7A214C, name "BeltMover"
  0x59E940, update 0x59EC90; verified in the decompiled code unless marked;
  `nfsu2::ai::belt`, game.rs `belt_wanted`, `build_belt`, `step_belt`,
  `streamed_in`). **When**:
  * Scheduling caveat: missing actions also enter full think, and skipped
    full-think calls still dispatch action-dependent fallback work, including
    goal speed callbacks. See [the native scheduling oracle and live trace](../outrun-think-scheduling.md).
  * The racer controller's full think (0x42ABD0 from 0x42AE2F: on the
    racer's turn, world counter 0x7FB724 mod 8 (mod 4 in drag off mode 4)
    = its place among the AI cars of the world list, once 0.2 s (0x784554)
    have passed since its last, +0x778) ends at 0x42B259: with the world
    under the car loaded (car +0x5CC, read at 0x42ABFB) **0x402180** true
    → mode 4 (0x5F46B0(4)) or, already on it, a rebuild (0x5918B0); false
    → a mode-4 car back to 0x5F4410's mode. Not loaded: a mode-4 car is
    rebuilt. The global 0x7E8D70 (1 in the image) gates the loaded branch.
  * 0x402180 is false for: track 0x89E7A0 = 1099 with 0x86B320 set and
    0x86B328 = 4 (not ported, no shipped track); drag (0x89E7D8 / 0x890108)
    or drift (0x89E7D9 / 0x890104); a driver path of ≤ 2 points (driver
    +0x10); the timers +0x730 or +0x734 running (0x401820); car +0x42C
    under 5 m/s (0x783504); car +0x424 < 1; no action (+0x6F8) or its
    type +0x204 in {8, 9, 0xA, 0xB, 0xD}; controller slot +0x78 = −2 (the
    human); 0x5F1170 (the least 3D distance from car +0x60 to world +0x80
    + 16·i, the positions 0x609CF0 copies of the first +0x14 cars, unused
    slots 9999999) under 120 m (0x78350C); race status +0x14 / 4000 under
    30 s (0x783508); a finished racer (car +0x1C, 0x5EE260).
  * Streamed out: the world notice (0x5F6CE0 → slot +0x10, 0x4022D0) and
    the contact handler 0x402450 put a car with car +0x5CC null on mode 4
    whatever 0x402180 says. Car +0x5CC is the pack of the car's drivable
    section (tracker car +0x5C4, 0x5E68E0 → 0x5DFEC0, 0x591390 on the pack
    table 0x88F768 + 8 + 4·index, index 0x5D3C20). **Observed** under Wine
    (Quick Race 4014, human parked on the grid): the
    23 packs held were exactly the entries of the visible-section list
    (0x34153) of the human's section 716 that are detail copies (number mod
    100 in 20..39: 721, 723, 724, 729–739, 1328, 1334, 1336, 1338, 1521,
    1528, 1531, 1830, 1833), and car +0x5CC matched "the car's section +
    20 is in that list" in 487 of 487 samples of the rivals over 90 s.
    **Inferred**: the list follows the human car (not the camera) and
    changes with no lag; the streamer's own choice is not traced. The
    clone unloads nothing; it only applies this rule (`streamed_in`).
  * Back: at the think's start (0x42AC04) a loaded mode-4 car with +0x730
    running goes to 0x5F4410's mode (+0x730 then counts down); 0x402450
    gives +0x730 = 2 s (0x783510) to a mode-4 car on any contact, then
    0x5F4410's mode (mode 1 and +0x734 = 5 s for the human's car).
    Track contacts count too (**verified**: 0x593990 passes a track
    partner of a loaded car and 0x5F46B0 takes no body out of the sweep, so
    0x5B2620 → 0x5F4970 → 0x402450 runs; the switch is at once,
    0x4024B8..0x4024DA): the clone now leaves the belt on a wall contact as
    on a car one (it had kept the belt, which dragged a rival through the
    wall at Sprint 4122 backward's hairpin into the void). A reset's
    placement (0x5F52D0) switches the mover to 0 (0x5F52E7) and back to the
    kept mode (0x5F53F4): a fresh belt with no path, which stands
    (`Belt::speed` 0) until the think's end builds one.
  * **Observed** (Wine, same race, 3 rivals, rows forced 4): mode 3 from
    5 s, mode 4 at race clock 30.0 s 395–434 m away (loaded); unloaded from
    ~430–530 m; mode 3 again at 64–126 m when they came round (94–99 s),
    mode 4 again past 120 m. On mode 4 the speed read 0.88–0.90 × the
    driver's target (+0x230).
  **Build** (0x5918B0): the driver's path (driver +0x10: count byte,
  points 16 bytes from +0x20) resampled to equal spacing when it has more
  than two points (0x405120, in place: cumulative lengths 0x576780, the
  index at i·L/(n − 1) 0x5767A0, the point between 0x405020; L at driver
  +0x1D4), copied as the knots of a Catmull-Rom spline (0x57A310(2, 0,
  0x14): type 2 ends, the first and last steps repeated (0x570250); basis
  0x865230 built by 0x56FFB0; 0x570650 takes knots trunc(u) − 1 … + 2);
  then 0x5916A0: t = 0; scale +0x10 = (n − 1) / L (knots per metre); u0
  (+0x1C) the nearest u to car +0x60 (0x570740, then 0x5774D0: Newton
  steps of at most one knot, halved on a sign change, to 0.005 m, at most
  998); v0 (+0x20) = car +0x42C × scale; a (+0x24) = (0.899 × the
  driver's slot +0x74 (+0x230) × scale − v0) / slot +0x78 (+0x234, the
  action's +0x1F4 counted down; 0.5 s when not above 0), held under
  0x4038D0(car, v0 / scale) × scale and above −20 × scale; the lateral
  blend +0x18 = (car − spline point) · (tangent × z), its rate half the
  car's lateral speed, blend time 0.5 s; the heading blend +0x28, 0.4 s
  (from the ctor: the car's speed along its heading). **0x4038D0** (mode
  4): v' = v / car +0x468; a = the acceleration curve (+0x55C, 0x5764E0)
  at v'; drag = (1 − car +0x4DC) × H +0x260 (record +0x370) × v'² / H
  +0x110 (mass); a = drag when [car +0x10] +0x20 (the gearing top speed,
  0x5B5AC0's +0x10; see the chase shake's S) is above 0 and ≤ v' and drag < a; it
  returns a − drag − 9.8 × the unit tangent's z at u0 (0x591630; off mode
  4 the car's velocity + forward instead).
  **Frame** (0x59EC90): when the spline has ≥ 2 knots, not both |a| and
  |v0| / scale under 0.01, and n > u0: with the world loaded,
  0x59EA30(0.5) (g' = 9.8 + (v0 / scale) × (H +0x264 + +0x268) × 0.5 /
  mass; 0.05 … 0.45 s ahead, free flight from the last point the path
  held the car, against the path's height; the largest gap) over 1.0
  sets controller +0x730 = 1 s (0x591550); t += dt; speed car +0x42C =
  (a t + v0) / scale; u(t) = u0 + v0 t + a t² / 2; the heading (+0x40C)
  from the blended velocity 0.4 s ahead (spline derivative × u's rate);
  car +0x60 = the point at t moved along tangent × z by the blended
  offset (target 0); loaded: laid on the road (0x5F5020), else axes from
  the path; the velocity the move over dt; each wheel posed rolling at
  speed / radius (0x59CBA0); within 200 m of a human 0x59DBC0 per wheel
  (not ported). Otherwise 0x5918B0 again and the car stopped (+0x70,
  +0x42C, +0x430 = 0). The blends (0x574E40): while value ≠ target
  (0x574BA0 restarts each frame), a cubic Hermite from (value, rate) to
  (target, 0) over the blend time, stepped by dt / time. **Clone**: the
  modes and rules above, the stream rule `streamed_in`; belt cars are left
  in the world step as kinematic bodies (`Input::Belt`: their own frame
  does nothing, they take part in the collision step), and a resolved
  contact with a car or a prop (0x5F4970 → 0x402450) hands the car to the
  physics mover for the rest of the frame and then sets +0x730 = 2 s
  (0x783510, `BELT_CONTACT_BLOCK`) and 0x5F4410's mode where its world is
  loaded (mode 4 kept otherwise); a human contact also restarts the 5 s
  hold (ported 2026-11-01; `drive::off_belt`, `CarState::belt_hit`).
  **Track contacts** (2026-11-01). Traced: the car body's partner
  filter (vt+0x54 of vtable 0x7A2478, **0x593990**) passes a track
  partner (null, or a body with +0x11 ≠ 1) when 0x81CB88 is set, or when
  body +0x17 is 0 and [body +0x60] +8 is set; the mover switch 0x5F46B0
  (case 4: 0x5A9040, body +0x5C = 0) takes nothing out of the sweep. Read
  live (Quick Race 4014): a loaded mode-4 rival has +0x17 = 0 and [+0x60]
  +8 set (null exactly while unloaded, as car +0x5CC), so natively a
  loaded belt car does get track pairs and 0x5F4970 → 0x402450 hands it
  back on any of them. **Gap sweep (verified)**: the same filter refuses every
  partner — track, car (+0x11 = 0, also needing +0x190) or prop — while
  [body +0x60] +8 is null, and the sweep 0x5B23C0 asks the body's own
  vt+0x54(0) before sweeping it at all: an unloaded car meets nothing.
  The clone: `CarState::streamed` (set each frame for racers, encounter
  cars and traffic from the stream rule) gates car pairs, track contacts
  and prop contacts in `step_world`. **Observed**: in 150 s (480 loaded mode-4 samples
  of three rivals, the pair list 0x86B490 walked to its sentinel) no
  belt car had a contact record — the belt line keeps clear of walls
  there. The clone (round 3 diagnostics): its belt cars meet race
  barriers and walls on their path lines (Sprint 4122: ~1100 track
  contacts over the four route-test races without the hand-back; with it
  ~12000, a car repeatedly put back on the belt into the same barrier at
  (−280, 295) or (−190, 180), and two rivals did not finish), and
  probing the wheels' ground with the native no-car query (0x5F4A00 →
  0x5E72F0(point, 0, 0) from each wheel's point, not 3 m above) did not
  change that. So the hand-back stays out (**open**): why the clone's
  belt paths cross race barriers / walls where the original's do not is
  not traced. **Measured** (first lap, s, rows and cars forced, human parked;
  original / clone over six seeds × 3 cars, median (mean)): row 1 109.4 /
  110.0 (110.8); row 4 98.5 / 98.9 (98.6); row 6 90.6 (91.6 in a second
  run, 2026-11-01) / 92.1 (93.0); row 8 88.0 / 88.6 (89.6). Before the
  port: 106.6 (106.8), 98.1 (97.8), 95.0 (93.2), 91.8 (90.6), mean
  (median). The means carry the start-pack contacts and the seed-6 wedge
  (see Difficulty → "Open too").
* **Racer actions** (2026-10-31, aidrive117 traced the chooser; actions118
  traced the actions and ported JustDrive's hand-back, Pass and Block,
  `src/ai/actions.rs`). The goal "Win road race" (vtable **0x784768**,
  name 0x7847B8 via slot 2 0x40CA40; 0x7847D0 is "Win drag race", name
  0x784820 via 0x40CA90, chooser 0x42D470 — both share slot 10
  0x41BCF0) chooses in 0x42D0F0 (slot +0x38, 0x42EFE0 jumps there),
  called with the running action's type and priority. In order:
  GetUnstuck (0x423C30, type 9, see GetUnstuck); IJustGotHit (0x423CC0,
  vtable 0x7857B4, type 0xB, priority 400 from 0x403320: 0x402AE0(…, −1,
  1) names the car that hit) when the current type is not 0xB and its
  priority ≤ 400; AvoidAvoidable (0x4243B0, vtable 0x785858, type 1,
  priority 300 from 0x42BA20: 0x428CE0 finds an object on the car's own
  path within 3 s) when the type is not 1 and the priority < 301;
  AccidentRecovery (0x4241C0, vtable 0x7857E8, type 0xD, priority 500)
  unless the type is 13, the priority > 500, car +0x424 > 1 or 0x403380
  (car, 0.707: the body's up axis z ≥ 0.707); then, outside Street X
  (0x89E7E3 / 0x89010C) and type 4, with the priority ≤ the i32 at AI
  tuning (0x81CB60) +0x934 + 8·(car +0x14 → +0x902) and 0x414130: Block
  (0x4238C0, vtable 0x785728, type 4, no time limit); else, when the type
  is not 0, the priority ≤ 100, 0x42B5B0 finds a car and the goal's
  vt+0x9C(car, −1) (below): the chooser writes the
  plan to controller +0x30C…+0x318 and proposes Pass (0x423610, vtable
  0x7856D8, type 0, priority 100, 5 s); else JustDrive (0x4269E0 /
  0x4235A0, vtable 0x785684, type 6, priority 0, no end).
  * **vt+0x9C** is the controller's (`edi` = planner → car +0x2C,
    vtable 0x785370 +0x9C = 0x408280; the goal's vtable has 20 slots):
    (object, time unused). Once 1 s (0x783B44) has passed since +0x79C
    the refused object +0x798 is forgotten; the refused object is refused
    again; else a draw 0x5EA510(1.0) on the world's random numbers
    (0x890080 +0xF8 → 0x43C170) under the AI tuning's +0x530 + 0x68·row
    (row = vt+0x78, 0x402580 → 0x610520: car +0x901; −2 → 0) takes it;
    a refusal stores the object and the time. Asked for Pass and for
    AvoidAvoidable (0x42D283, before its constructor). Ported
    (`Actions::willing`, the world generator `CarField::rng`).
  * **Manager** (0x429F56…0x42A02D, after the road entries 0x428720):
    0x403120 sets the action's status +0x10: 3 when slot 8 says the time
    limit (+0x0C, kept when ≥ 0, 0x40C960) has run out (0x4031D0 /
    0x403250: (now − +0x08) × 0.00025 > +0x0C), 2 when slot 7 (given up),
    1 when slot 6 (done), later ones overriding; 0x401990: any of them
    ends it, and the chooser is then asked with type 0x16 and priority −1.
    The proposal replaces the action only when its priority (+0x200) is
    above the asked one; else it is deleted. The old action's slot 3
    (0x403560) calls 0x5ECF30(0, 0) on the car. Base ctor 0x4234A0(type,
    priority, limit): +0x204 type, +0x200 priority.
  * **JustDrive** (slot 2 0x42B570 → 0x42B340, then 0x40E390 nitrous,
    +0x1FC/+0x1FD/+0x1FE = 1, +0x1F8 = 0): the plain plan is {lane 0,
    offset 0, rate R} (R 0.2 in Street X, 0x783680, else 0.05). The least
    time to an entry ahead (index < own) is −gap / +0x04 (0x7842A0 = −1)
    over those with |+0x04| > 0.1 (0x784358) and a positive time. When it
    is above 0.75 s (0x78367C) and the plan is not plain (lane −1 or ≠ 0,
    |offset| > 0.001, or |rate − R| > 0.001), the plain plan is tried
    (0x428DF0 over 3 s, 0x783670) and taken with its path when clear;
    otherwise a plan on lane 0 is **kept** (a plan on another lane goes
    back to plain) and the path built on it (0x420D40). So a pass's or a
    block's offset stays until the road ahead is clear. Ported.
  * **Pass proposal** 0x42B5B0: not in the start hold (0x5EF000 with
    0x883DAC +0xF6 ≠ 1; the byte's value is **not traced**, taken as ≠ 1),
    only 5 s (0x7843E8) past the start (0x890118 +4); the entries are
    walked from count − 1 **down to 1 (entry 0 is never looked at)**; one
    qualifies with a bumper gap g = 0x60C000 − entry +0x0C − own +0x0C > 0,
    a racer (0x60BEB0: object +0x1C → +0x1C set) and either 0x403A40 or
    g < 10 with +0x04 < 1. For each, the controller's plan is copied to
    the out record (ECX of 0x429890, asm 0x42B75D–0x42B793) and 0x429890
    run on it; a success names that car. The last success names the
    target, but the record holds the **last qualifying** car's plan, even
    a failed one (native, ported as is). Priority 100.
  * **0x403A40**(target, car, gap, closing, a5, flag): a = the car's
    acceleration along its heading (0x5EC3B0: car +0x80 · +0x90) less the
    target's (0x60BE40 → 0x5EC340 on the object's +0x14 / +0x10 and |v|;
    plus it when the car's forward · the target's +0x18 direction ≤ 0);
    true when gap < −3·(1.5·a + closing) (0x7E8E0C = 3, 0x784260 = 0.5).
    a5 < 0 makes it call 0x4038D0 and drop the result (dead). The clone
    takes the accelerations from the velocity change over the frame
    (**inferred** for the body's values).
  * **0x429890** (plan beside a target; ECX the plan {lane, offset,
    rate}): need = own +0x08 + target +0x08 + 3; side = 0x41D2D0; unless
    |need| ≤ |target offset − plan offset| (already clear), offset =
    target − need (side 2), target + need (side 3), or for side 1 the
    side the car is on (target > own → minus, < own → plus, equal → kept);
    with lane ≠ −1 the plan takes it and rate 0.05 (0x7E8E64); then
    0x428DF0(path, controller, 3.0) is the result. The target offset is
    the lane record's +0x04 that 0x40FC80 moves the path towards (ported:
    `line_path`'s `target`).
  * **0x41D2D0** (side): ahead = (target s − own s) − (own +0x0C + 5 +
    target +0x0C) > 0. Unless the car's controller class (car +0x14 → +4)
    is 3 with the target under 5 m (racers are class 2, so always): if
    |own offset| > 1 or |target| > 1 or the gap ≤ 50 or |0x419E10(lane,
    own point, −1)| < 0.12: side 1 when |own − target| < 2 and ahead,
    else 2 when own ≤ target, else 3; otherwise 2 when 0x419E10 ≤ 0, else
    3. Then the room at the target's point (lane 0: FuturePoint +0x18 /
    +0x2C; lanes 2–5 +0x14 / +0x28 + 4·lane) with need = both +0x08 + 5:
    sides 1/2 with no room on the minus side go to 3 when there is room on
    the plus side and ahead; sides 1/3 with no room on the plus side go to
    2 when there is room on the minus side and ahead. 0x419E10 takes the
    racing line at the car's point, the list's middle point and its last
    (0x4122C0 each); the value made of them (0x419F39 on) is **inferred**
    (the clone: the cross product of the two unit chords, `bend`).
  * **Pass update** (slot 2 0x42B940): with the target's entry
    (0x60C130 on +0x218) at a centre gap > 0 (0x7D7D3C), 0x429890 on a
    copy of the plan (0x40FBE0), taken when it succeeds; else the path on
    the kept plan (0x420D40). 0x40E390 (nitrous) when the target's
    |+0x04| ≤ 5 (0x7843E8; **not ported**, the racers' nitrous outside
    drag races is open). Done (slot 6 0x42B7F0): when entry 0 is not the
    target, 0x42B5B0 again, and a different car found is taken with its
    plan (+0x21C restarts, 0x41BC60); then, 3 s (0x783688) after +0x21C,
    done when the target has no entry or its centre gap < −(own +0x0C +
    target +0x0C + 10) (0x783684). Given up (slot 7 0x40E4A0): no entry,
    or 3 s after +0x21C with the centre gap > 0, 0x403A40 false and the
    gap > 10 (0x78368C). Time limit 5 s.
  * **Block proposal** 0x414130: car +0x430 / max(0x5EC080, 0.1) > 0.8
    (0x5EC080: controller 0x4024F0, the node's racing-line speed byte
    +0x2C) and not in the start hold; the first entry behind (own + 1 …)
    that is a racer (0x60BEB0) at |v| ≥ 10 decides (others are skipped):
    none when it is off the road at its point (offset ≤ 0 needs offset >
    room +0x18, > 0 needs offset < +0x2C), when the gap (0x60C000, plus
    both +0x0C when the tuning byte +0x938 + 8·row is set) is ≥ 0, when
    its point is before the list's first (0x401F70: list +0x31C first
    point +0x54), when the gap ≤ −50 or +0x04 ≤ −5; it is blocked when
    +0x04 > 0.001 and |gap / +0x04| < 2 (30 for the human, 0x614890), or
    when it is the human within 35 m. Its priority is the i32 at +0x934 +
    8·row. Shipped rows: 200, 200, 400 with the byte 1 in each (three
    rows: the row index stays under 3 because +0x94C is the catch-up
    table). The row is racer info +0x902, 0x6373F0's sixth argument:
    the career line-up 0x5260C0 passes the entry's byte +0x0B (1 in all
    679 career entries, the maintainer's probe `difficulty_data`), the Quick Race
    and free-roam line-ups 0x53EF75 / 0x53F0D2 pass 1, the human 0
    (**verified**; read 1 for every rival under Wine). It does not depend
    on the difficulty; the clone used the difficulty (always 0) until
    2026-11-01 and now keeps each rival's row (`Rival::block_row`).
  * **Block update** (slot 2 0x423A10): 0x5ECF30({255, 0, 0, 0}, 1) on
    the car (**open**); with the target's entry the plan offset is its
    offset (0x60BFB0 on the plan's lane) and the rate 0.15 (0x7E9BF4)
    when the centre gap < 10, else the curve 0x81E7DC (built by 0x77A660
    → 0x576780 from the pairs at 0x7E9BF0: (2, 0.15), (10, 0.05);
    0x5767A0 linear, clamped) of gap / +0x04; no entry: offset 0, rate
    0.15. Then the path (0x420D40), +0x1F0 = controller vt+0x7C, +0x1F4 =
    0, +0x1F8 = 2.0, +0x1FC/+0x1FD/+0x1FE = 1. Done (slot 6 0x41CC40):
    0x414130 finds no one (another follower is taken over). Given up
    (slot 7 0x40E5D0): the target's centre gap + both +0x0C > 0
    (alongside or ahead). Ported. The light call is a debug tint:
    0x5ECF30(colour, on) sets car +0x97C and +0x980…+0x98C ({255, 0, 0,
    0}, red; AvoidAvoidable's "far" uses {0, 255, 0, 0}), read only by the
    car's state dump (0x5FBB97) and cleared by 0x604657 and by every
    action's end (slot 3 0x403560: 0x5ECF30(0, 0)). +0x1F8 goes with the
    goal's speed (slot +0x34 0x41C5E0 at 0x41C8E9) to the driver's slot
    +0x38 (0x4054A0: +0x230 target, +0x234 ← +0x1F4, +0x238 ← +0x1F8,
    +0x23C ← +0x1FD); +0x238 has no reader besides the dumps 0x405A80 /
    0x415DD0. +0x234 is counted down by the frame time to 0 (0x421E96)
    and returned by the driver's slot +0x78 (0x410000; callers not
    traced). +0x23C off zeroes throttle and brake (0x421F1A). The speed
    the driver gets is the least of the line speed and +0x1F0 unless
    +0x1FC (0x41C8B1), and of the goal's slot +0x4C.
  * **0x428DF0** (clear test): the path on the plan (0x420D40; a path
    that cannot be built is not clear), then 0x428CE0(path, controller,
    3.0, &t) (paths of 1…20 points only): over the entries not ignored by
    the goal's vt+0x84 (0x41BEA0: none) and kept by 0x403820 (not the
    car's own, centre gap > −(own +0x0C + entry +0x0C) less 0.5 m for the
    human, 0x783790), 0x4275E0 sweeps the car along the path and keeps the
    least time; clear when none or the least time ≥ 3. **0x4275E0**
    (verified): the other car's path is its driver's (0x60BE20: car
    +0x30 → +0x10, a racer's driven path); a traffic car's (0x6148D0) not
    going backwards (controller vt+0x2C > −0.25) is replaced by 0x425A40 →
    0x420B80: from its place +0x60 with its velocity +0x70 and
    acceleration +0x80, v += a·0.1, p += v·0.1 (0x43D090, 0x3DCCCCCD) per
    point until v turns against the first, the length 0x60BF20 (the
    controller's reach 0x404910) is run or 20 points, each appended by
    0x404CD0 (a point under 1 m from the last, or within 20 m one turning
    back, refused: a standing car's path is one point); with |a| > 0.75 |v|
    instead 0x420A60: five points length / 5 apart along the velocity (the
    heading under 1 m/s) — a static obstacle line. Boxes per path point
    (0x4040A0: half the shape's extents, own body +0xE0 / +0xF0 by
    0x5EC260, the other's 0x5AACE0; box 0 along the car's heading, box i
    along p[i−1] → p[i]); a traffic car's boxes × 1.3 when the car is not
    traffic (car +0x24 ≠ 4) and the traffic moves at 2 m/s or more. Both
    cars start at their first path point ahead (0x404EF0: along the
    velocity, farther than the AI tuning's +0x24 = 0.02 m, else the last);
    each step lasts until the first of the two reaches its point at its
    speed (own +0x430, the other |v|, at least 0.001, grown per step by
    0x5EC3B0 / 0x60BE40), that car then aims at its next point (the other's
    when |t_own − dt| > 0.001); 0x4044C0 tests the two aimed-at boxes at
    the relative place and velocity (each car's direction to its point ×
    its speed): a 2D separating-axis interval per axis (0x4043F0: (−p − r)
    / v … (r − p) / v; |v| < 0.0001: always or never), intersected by
    0x404240; the entry time when > 0, 0 when already overlapping, else
    99999; a contact when 0 ≤ t ≤ the step. The sweep ends after `look`
    seconds or either path's end (a traffic car's prediction covers about
    2 s). Ported (`actions::swept`); **not ported**: a car going backwards
    (0x425910 / 0x425980: a straight line from 0x420A60), the human's path
    (taken as a traffic car's prediction, **inferred**).
  * **Road entries** (0x6148F0; entry 0x30 bytes at planner +0x08,
    count +0x04, own index +0x308): over the object list, at most 15
    besides the car (16 counting it), those with a 3D distance under the
    reach (0x404910: max(125 m, 0x7E8E60, car +0x430 × planner vt+0x60;
    that slot is **open**)) or that 0x403A40 finds reached (gap = the
    distance, closing = ±|Δv|, minus when the car's forward · the other's
    +0x18 direction < 0), and within 3 × the reach in race coordinates
    (0x5D5CE0 on car +0x8FC). +0x00 / +0x04 = the other's velocity less
    the car's, turned by 0x4000 − the heading of 0x40ADD0's direction
    (0x5BAE70): across and along; +0x08 / +0x0C = k·w + (1 − k)·l and
    (1 − k)·w + k·l, w and l half the shape's extents (0x5AACE0), k = cos²
    (other's heading − car +0x40C); +0x10 s and +0x28 point (0x419C10);
    +0x14 / +0x18…+0x24 the offsets on lanes 0, 2…5 (0x4123C0); +0x2C the
    object id. Sorted by (Δs² + Δoffset²) signed by Δs, largest first.
    Clone (`entries`): this with the list's direction for 0x40ADD0's and
    the handling dimensions for the extents (**inferred**), without the
    race-coordinate window and the object filter (0x5EC070, car +0x550).
  * **AccidentRecovery** (ctor 0x4241C0(500, −1): +0x210 = now, +0x214
    = 0; proposed at 0x42D2D2 when the type is not 0xD, the priority ≤ 500,
    car +0x424 < 2 wheels down and 0x403380(car, 0.707) false: the up
    axis's z (0x43CF10 on +0xB0) ≤ 0.707): update 0x424210 drives the
    path on the kept plan with +0x1F0 = 0, +0x1FC = 0, +0x1FD = 0 (no
    pedals), +0x1F4 = +0x1F8 = 0; 5 s (0x783720) after +0x210 it asks the
    reset once (0x5EBF70: car +0x96C = 1; +0x214 = 1). Done (slot 6
    0x4033D0) once +0x214 is set, or with four wheels down and upright;
    slots 7 / 8 never. The reset of a class-2 car (car frame 0x6012C0 at
    0x6013AF): kept speed max(+0x42C, 0) × max(direction · forward, 0)
    (the race road's +0x900 / +0x904 on race coordinates), at most 8 m/s
    (0x8021CC); Car_PlaceOnRoad 0x5FBEE0 on the controller's road node
    (+0x2C → +0x0C) and record (0x40F690), facing its way (reversed by
    the record's +0x70 bit 1 on race coordinates), lateral 0 (a drag
    lane's from 0x403DB0 / 0x5E2DB0 in drag races, not ported). Ported
    (`Actions::recover`, `reset_rival`; the node is the walk's current
    FuturePoint, inferred).
  * **IJustGotHit** (ctor 0x423CC0(400, −1, hitter): +0x218 the hitter;
    proposal 0x403320: not in the start hold, 0x402AE0(planner, −1, 1)).
    0x402AE0 walks the body's contact records (car +0x3C, count +0x18;
    0x5A0830; record +0x60 ≥ −1, so all), takes the other body
    (0x592CA0) when not ignored (controller vt+0x84, 0x41BEA0: none), with
    a road entry (0x60C100) and a racer (flag 1, 0x60BEB0): the human over
    a rival; among alike, one ahead (gap > 0) over one behind, else the
    lesser gap. 0x40D5C0 runs after it (only for an object +0x24 of 3 or
    4: not racers). Update (slot 2 0x423E20): +0x1F8 = 0, +0x1FC = 0,
    +0x1FD = 1; with the hitter's entry, x = 0x60C020 (its offset less the
    car's on the plan's lane): x < 0 → offset = max(plan offset, its
    offset + 5 (0x783714) + both +0x08), else min(plan offset, its offset −
    both − 5); rate 0.25 (0x783718); but when the centre gap < 2.8
    (0x78370C) − both +0x0C (behind) the plan offset stays at rate 0.05.
    No entry: offset 0, rate 0.25. The path (0x420D40). Speed, outside
    Street X and drift (0x89E7E3 / 0x89010C / 0x89E7D9 / 0x890104) with
    the entry: 0x40DF50(car, 0) = 1 (the car's road-edge state: off its
    node's widths +0x14 / +0x16 or leaving within 1 s, 0x783660; **open**,
    the clone takes it as never) with the hitter ahead → 0x40DEE0(car,
    0.2): +0x1F0 = speed − 0.2·(braking: (|v|·(+0x264 + +0x268)/2 + 9.8·m)
    ·(+0xDC + +0xFC)/2 / m), +0x1F4 = 0.2. Else, the hitter the human
    (0x614890) and the car not class 3: with the gap > both +0x0C − 2.8
    (ahead), 0x402940(…, 0, 0.2): +0x1F0 = max(|v_human| − 0, 0), +0x1F4 =
    bumper gap / closing (99999 under 0.01, 0x78425C; at least 0.1);
    behind, +0x1F0 = the cap (controller vt+0x7C, 0x5A9F60 → 1000), +0x1F4
    = 0; beside, +0x1FC = 1. A rival hitter: the cap (class 3 with the
    hitter ahead facing away: car +0x430 and +0x1FD = 0). Otherwise +0x1FC
    = 1. Then without +0x1FC a +0x1F0 under 20 (0x783710) becomes 20 once
    the race has run 10 s (0x402000 on 0x890118, 0x784560). Done (slot 6
    0x41CC90): 0x402AE0 finds no one (another hitter is taken over).
    Ported (`Actions::got_hit`, contacts from `CarField::partners`).
  * **AvoidAvoidable** (proposal 0x42BA20: 0x428CE0 on the path driven,
    car +0x30 → +0x10, ECX 0x827A00, 3 s: the first object and its time;
    priority 300; then vt+0x9C; ctor 0x4243B0(300, −1, object, 2.0, t):
    +0x218 object, +0x21C t, +0x224 = 2, +0x22C = t + 2 (0x783728), +0x228
    = now, +0x220 = a traffic object's controller +0x7A0, else 0). Update
    0x42F330: +0x1FD = +0x1FE = 1; with the entry and outside the start
    hold, the goal's slot +0x20 (1 for "Win road race", 0x40CA60) picks
    0x428FD0 (2: 0x42BF00) with side = entry +0x00 > 0, slow = +0x42C <
    11.18 (0x783734). 0x428FD0 (ECX the lane record {lane, offset, rate,
    stamp +0x0C}): true when 0x428DF0 finds the kept plan clear, or the
    stamp is under 0.5 s old; the room at the car's point (lane 0: +0x18
    / +0x2C; lanes 2…5 +0x14 / +0x28 + 4·lane), step = max(room·0.1, 1);
    for rates 0.05, 0.10, 0.15 (0x7E8E64, +0.05, ≤ 0.151) offsets kept ∓
    k·step, k = 1, 2…, the minus side first when side, each tried while
    inside the room (0x420D40 + 0x428CE0: clear when no hit or the time ≥
    3 s); the first clear one is copied (0x40FBE0) and true. The stamp is
    set **only for the 999 rate** (verified: 0x42925D / 0x42932F
    take the stamp 0 when |rate − 999| > 0.001, now otherwise, as the fallback does;
    observed: in six native circuit runs the record's stamp +0x0C
    (controller +0x36C) read non-zero in exactly the 3088 AvoidAvoidable
    samples at rate 999 and zero in all 17348 others), so the
    0.5 s hold keeps a fallback plan, not a stepped one. Failing: with no object from the first
    test, or the car a racer and the object one (0x60BEB0), false; else
    w = both +0x08, lo = obj − w − 5, hi = obj + w + 5, widened to the kept
    offset on its side (0x401070 min / 0x401050 max); costs (1, 0) when
    obj < kept (plus side cheaper), else (0, 1); with a contact record
    (0x592C70) and the |lo − obj|, |hi − obj| apart by > 2 those; with
    slow, 99999 for a side whose obj ∓ w leaves a point's room between
    the car's +0x28 and the object's; the cheaper finite side's plan at
    rate 999 (0x7E8E68) is copied with its path (0x401D40) and the stamp
    now (neither side finite: nothing changes); false. Then
    unless it was true, or the object is traffic (0x6148D0: object +0x1C →
    +0x14 → +4 = 3) or ghosted (0x60BF80: object +0x1C → 0x5EBFB0, the
    car's ghost timer +0x974 / +0x978; the racer-traffic parity, verified): +0x21C ≈ 0 (0x4010C0, 0.001) → near
    when the gap > 0; else the centre gap in +0x21C s ((a·t/2 + closing)·t
    + Δs, a = 0x5EC3B0, Δs from 0x60C060) > 3 (0x783730) → near, else
    far. Near outside Street X / drift: +0x1FC = 0, +0x1F0 = max(+0x430 −
    0.25·min(closing, 0), 20) (0x783738, 0x78372C), +0x1F4 = +0x21C,
    +0x1F8 = 2, tint off. Far: +0x1F0 = 2 × the cap, +0x1FC = 0, +0x1F4 =
    +0x1F8 = 0, 0x40E390 (nitrous), tint green. Otherwise +0x1FC = 1 (in
    a drag race +0x1F0 = the cap), +0x1F4 = 0, +0x1F8 = 2, tint off.
    Goal slot +0x20 = 2 only: 0x420ED0 for lanes outside 0…1 / 2…9. Done
    (slot 6 0x42BA80): no target or entry; else 0x428CE0 on the path
    driven: a hit is taken (0x41BC60, +0x21C its time; in drag races only
    when 0x4140E0 passes it, **open**) and the action goes on; without
    one it ends once now − +0x228 ≥ +0x22C — for a traffic object only
    when it is 10 m behind (0x785BAC, gap < −10 at 0x42BBFF), and with
    +0x220 set by its road section's test (0x409320 / 0x409360 /
    0x5D5CE0: the record of the refcounted object +0x7A0 points to, its
    point +0x94 against the object's race coordinate car +0x8FC); in drag
    races it ends. Slots 7 / 8 never. +0x7A0 of the race traffic read 0 in
    every sample of six native circuit runs (43212 traffic
    samples), so the 10 m rule is the one that runs there. Ported
    (`Actions::avoid`, `avoid_plan`, with the slow room check
    `RacerRoute::rooms_between` since the racer-traffic parity; **not ported**: +0x220's test
    (never set in the races seen), the nitrous ask; the contact kind is
    any car contact, **inferred**).
  * **0x419E10** (the bend in 0x41D2D0) stays **open**: the clone's
    `bend` is inferred. 0x4275E0 is traced (see 0x428DF0).
  * **Checked under Wine** (2026-10-31, a private Wine copy with wined3d: DXVK
    wedged the Xvfb; Quick Race circuit with the defaults, the player left
    on the grid, 3 rivals; controllers found by their vtable 0x785370,
    action +0x6F8 → +0x200 / +0x204 / +0x218, lane record at controller
    +0x54 +0x30C; 1266 rival samples over 180 s): Pass 293, JustDrive
    973; every Pass ran with rate 0.05 and offsets ±4.0…6.2 m (17
    samples at 0, the start of a pass), passes re-taken on the same car
    for up to ~35 s; JustDrive held the plain record (0, 0, 0.05) in 971
    of 973 samples, the other two a pass's kept offset (5.46, 6.19) —
    the hand-back above. AvoidAvoidable (priority 300) runs with rate
    999.0 (0x7E8E68) and a −6.5 m offset; GetUnstuck shows priority 500.
    Car +0x14 → +0x902 read 1 for every rival (the Block row index). The
    clone at a comparable set-up (`--bench racing`, circuit, 3 rivals,
    `NFSU2_DEBUG_ACTIONS=1` logs each rival's action twice a second):
    Pass with rate 0.05 and offsets ±2.9…6.0 m, re-taken after the 5 s
    limit, Block twice (rate 0.15), JustDrive plain otherwise. Not seen
    under Wine (no follower close enough): Block; the road-entry count
    (controller +0x54 +4) read 0 between thinks, so the entries were not
    compared.
  * **Checked under Wine, round 2** (2026-10-31, a private Wine copy, wined3d;
    Quick Race circuit, Peugeot 206, 3 rivals; controllers by vtable
    0x785370, action +0x6F8 → +0x200 / +0x204 / +0x218 / +0x1F0 /
    +0x1FC…, lane record +0x54 +0x30C; ~0.9 s samples): IJustGotHit
    between rivals right after the start: type 0xB, priority 400, +0x1F0
    = 1000 (the cap, a rival hitter), +0x1FC = 0, +0x1FD = 1, plan rate
    0.25 with offsets +6.52 then −8.84 as the hitter changed sides, and
    for the rival behind its hitter the kept offset 0 at rate 0.05 — as
    0x423E20 reads. AvoidAvoidable (type 1, priority 300): with the
    player's car put stopped 15 m ahead of a rival, the object was the
    player's and the kept plan (0, 0.05) stayed with +0x1FC = 1 (the plan
    clear: 0x428FD0 true) until it ended 2.7 s on; on a later object (a
    non-racer) the plan stepped to −2.11 at 0.05, then the fallback −6.55
    at rate 999. Pass (type 0, 100) seen again. Not reached: Block (the
    teleported player's car did not hold behind a rival at speed),
    AccidentRecovery. The clone (`--bench racing`, NFSU2_DEBUG_ACTIONS=1):
    IJustGotHit with rates 0.25 / 0.05 and offsets −9.7…+5 (one +24 m
    off the hitter on the far side, the native has no room clamp either),
    AvoidAvoidable mostly on the player's car standing on the grid with
    the kept plan, stepped offsets ±8.4…8.8 at 0.05…0.15 and the 999
    fallback not reached (no traffic in the bench race).
  * Drag racers run "Win drag race" (0x42D470: GetUnstuck, 0x424560 /
    0x403410 / 0x403450 the drag-race hit actions, AvoidAvoidable — in a
    drag race with no willingness draw, its update's lane search 0x42BF00
    over lanes ±1…3 via 0x428E70, ported as `Actions::drag_avoid`, see
    docs/first-drag.md); the
    other choosers 0x42D6F0 (vtable 0x784B58, Chase car: the same list as
    0x42D0F0) and 0x42DA70 (0x784BF8, Offset pursuit: no Block / Pass)
    are not racers'. The clone runs the actions for every racer outside
    drag races (drift and Street X included; Block not in Street X).
* **Upgrades from skill** (0x5260C0 → 0x5B6AE0, sources 0 and 3 offline):
  rating = the player's car's first/last preset rating lerped by skill/10
  (0x59A460 reads the player's car), the opponent car's nearest preset (0x34A31
  chunks, one per car type: u32 count, 16-byte presets of f32 rating and a
  68-bit part mask MSB-first from +5) fitted, then rating += 150·(target −
  handling score) and fitted again; target = 0.03 + 0.8936·skill/10 (0x34A32
  +0x70/+0x74). Handling score 0x5B54A0: Σ of nine normalised tuning
  measures × (0.14, 0.10, 0.15, 0.15, 0.10, 0.15, 0.15, 0.08, 0.08),
  clamped 0.03–1, normalisers 0x34A32 +0x30…+0x6C. Presets checked: car 0
  starts at 196.7, the Corolla at 144.1, the Focus ends at 613.9 (the
  global bounds).

## Traffic (src/ai/traffic.rs; game.rs `Traffic`, `traffic_think`, `traffic_settle`)

Explore mode's and races' traffic. Verified except where noted.

* **Models**: car types whose record +0x844 is 2 (`CarType::traffic`; the
  stock-car test needs 0): 4DR_SEDAN(02), AMBULANCE, BUS, COUPE, FIRETRUCK,
  HATCHBACK(02), MINIVAN, PANELVAN, PARCELVAN, PICKUP, SUV, TAXI(02) …
  Record bytes +0x854 + zone (`traffic_caps`, five zones; the zone is
  zone +0x3C of the type-0x10 zone 0x5D9C00 finds) cap how many traffic
  cars take the model there: 5 for most, 3 for PANELVAN / PARCELVAN, 0 for
  AMBULANCE, BUS and FIRETRUCK; +0x859 keeps a model streamed. The model
  pass 0x412BE0 (from 0x57F9A6, verified) gives each traffic car one:
  while fewer than 7 models are in use, the r-th (0x43C1C0 over the count
  still under their caps) model under its cap, its use count rising; then
  the first in use still under its cap, else the first in use, else the
  first. Clone: 0x412BE0 over the types with a CARS folder, the caps
  zone of the traffic zone holding the player at the free-roam start, one
  load per model.
* **Model life cycle** (verified, 2026-10-28): a car's model is fixed for
  the whole track load; **no native code moves a placed car to another
  model as the player crosses zones**. The track loader 0x57F2E0 (from
  0x581590, every track load) in state 6: 0x430240 on 0x827E88 gives
  the point — the first car of the list 0x890080 (+0x1C → +0x60/+0x64,
  the player in free roam), or, when 0x89E7B0 ≠ 0 and 0x864FA8 = 0, the
  marker 0x86501C (0x5D8630) else "ForwardStartLine" /
  "BackwardStartLine" by 0x89E7A4 (0x5D8670) — then 0x5D9C00 (kind 0x10)
  → zone +0x3C → 0x412BE0 (slot +0x908 per traffic slot, entry +0x910
  per model), then 0x412DB0 loads every entry with +0x910 > 0 (+0x8F8
  handle, +0x900 load time, count +0x00). State 7 (0x57FA1C → 0x420030):
  for each car of 0x890080, its slot (car +0x14) takes the record of the
  entry whose type is slot +0x908 (0x413440), +0x908 = +0x10, the car is
  rebuilt on it (0x601120), and the entry with the car's type and skin
  byte (record +7) gets the render model bound (0x61CA60) and +0x908 /
  +0x90C users ++ (+0x904 time). Users drop in the car dtor (0x5F4618 →
  0x41AC40 → 0x412910 by type and skin byte). The streamer 0x412950 (from
  the placer 0x42F910 every 5 s) only loads / frees model data: zone at
  the first human racer's car (0x8900B4 → +4 → +0x60) when 0x8654A4 = 6, else 0; it flags an
  entry loaded over 6 s (0.00025 × ticks) whose users +0x90C exceed
  max(4, cap) for freeing (+0x914, cleared when record +0x859 + zone is
  set), frees flagged entries with no current users (+0x908 = 0), and,
  while fewer than 7 and fewer than the entry count (+0x908 of the
  manager) are loaded, loads the r-th unloaded entry (0x5EA4F0) if its
  cap > 0, with a fresh skin byte. Its loads are not bound to any car: the
  only stores of slot +0x908 are 0x412BE0, 0x420030 and 0x420140
  (teardown, back to the slot's own type), and the only user of the
  manager list besides these is 0x412720 (save/restore). The placement
  (0x42F800 → 0x42E9F0 → 0x42C950; callees 0x42AA20, 0x5E2DB0, 0x5ECF30,
  0x5F46B0, 0x5FBEE0) does not touch the model; 0x604270 (from 0x42C7BA)
  rebuilds a car on its own slot record. So the zone dependence is per
  load: the clone drops free roam's pool when a race loads its track
  (`drive_to_grid`) and builds it again, with the zone at the player, on
  the next free roam. **Open**: whether leaving a shop / garage / pause
  menu is a track load (the clone keeps the pool there); in a race the
  point is the start-line marker (0x89E7B0 / 0x864FA8 meanings
  **inferred**), the clone uses the grid slot.
* **Skin per model** (verified selection; verified that it does not
  reach the texture): each
  manager entry carries a skin byte (+0x17, the car record's +7), drawn by
  0x61BC30 for a type with +0x844 ≠ 1: 0 when record byte +0xD9 (count)
  is 0, else record byte +0x880 + rand(count) (0x43C140 with a 0x43C1C0 /
  0x5EA4F0 seed); at 0x412840 (manager reset, from 0x4179F0 and
  0x41AAF0) and on each streamer load. All cars of one model share it.
  Counts: TAXI/TAXI02/AMBULANCE/FIRETRUCK 1, PARCELVAN 2, BUS 3,
  HATCHBACK 5, PANELVAN 6, the rest 8; CARS/TEXTURES.BIN holds
  `<CAR>_SKIN1`…`n` (128² DXT1, each a different body colour; the
  traffic dirs' own TEXTURES.BIN are empty). The byte is the type's skin
  table entry (verified, 0x61BC30): `table[rand(count)]` with the table at
  record +0x880 (bytes 1…count for every traffic type). What it does
  (verified, 2026-10-28): it only picks the entry — 0x420030 binds a car
  to the entry with its type and skin byte (0x4200C5), 0x412910 drops
  users by both — and the save/replay serialiser 0x5FB7C3 restores it
  through 0x6380D0 (record +7 = byte, then 0x636850 rebuilds the remap).
  **Nothing turns it into a texture**: every traffic car draws
  `<CAR>_SKIN1`:
  * All 15 traffic GEOMETRY.BINs name `<CAR>_SKIN1` directly (4–5
    references each; no `GLOBAL_SKIN1`, `<CAR>_SKIN2`, `CARSKIN` or
    0xA7366AE6) and light the body with the `TRAFFIC` light material.
  * The remap (0x636850 → 0x631E30 on render info +0x40): entry 0 is
    {from +0x270 = `<type +0x20>_SKIN1` (format 0x7A8A8C), to +0x2D0 =
    struct +0x290}; +0x290 is ride +0x584 (`DUMMY_SKIN<n>`, set by
    0x61C290 only when car type +0x88C ≠ 0: 1 for the 31 racer types, 0
    for all 15 traffic types) else `<CAR>_SKIN1`. So traffic maps SKIN1 →
    SKIN1, and the compositor 0x61D7A0 returns at once (+0x584 = 0). No
    other code in the car range (0x600000…0x63FFFF) writes the entry's
    +0x63C / +0x640. `%s_SKIN2…4` (0x7A8A40/34/28, 0x63207B…0x6320C0) only add
    those hashes to the car's texture load list (SKIN5…8 never), and no
    `%s_SKIN%d` string or hash arithmetic forms them. The skins 2…n look
    like unused data.
  * GLOBALB chunk 0x34601 (loaded at 0x636C44 into 0x8A1CD4, count
    0x8A1CD8 = size / 0x40, records {type, skin, colour name, …}; the
    results-screen colour names "Silver", "Metal", …) is searched by
    (type +0x840, skin) in the car draws, and every result is dropped:
    0x6166A3 (both exits of the loop reach `xor eax,eax` at 0x6166DB, eax
    then overwrites the slot at 0x6166E3), 0x6210EE (eax zeroed at
    0x621121) and 0x6383DA (the match is in edi, overwritten at 0x638415;
    the stack copy +0x20 is not stored on a first-record match and is
    next written at 0x638A06 with no read between). 0x6101B0's one caller
    (0x60165E) is a results record.
  * Per-model body paint (verified, 0x6371F0 from 0x412892 at manager
    reset): ride slot 0x3F (+0x68C) = the rand(count)-th part
    (0x43C1C0, iterating 0x61B990) of the 0x631A30(type, 0x3F, any hash,
    any level) parts of the paint part type — for every traffic type all
    270 paint-type parts (body, rim, caliper colours; the clone's
    `PartDb::find_after` order, the maintainer's probe `traffic_paints`). Its
    attribute 0x6BA02C05 becomes render info +0x384 (0x638D9E → 0x48F6A0),
    which only replaces `CARSKIN` / 0xA7366AE6 (0x6174A4) — light
    materials no traffic geometry uses — so it does not show. The clone
    does not model it.
  * Render info +0x3F8 = rand(8) (0x6383A5) has no reader.
  The clone (`CarType::composited_skin`, record +0x88C) composites only
  types with a DUMMY_SKIN slot, so a traffic car draws the shipped
  `<CAR>_SKIN1`. The solid draw 0x5C5940 (verified, 2026-10-28) does not
  swap it: for each strip (0x3C bytes from solid +0x14, count +0x10) the
  texture is the solid's own table entry (solid = draw argument +0x0C;
  `[solid +0x2C] + 4 + 8·strip byte +0x1C`) and the light material
  `[solid +0x3C] + 4 + 8·strip byte +0x20` (0x48AC50 for 0xFF); no car,
  skin byte or instance field is read. Its other inputs only pick the
  shader (the effect +0x3C by the globals 0x870CC4 / 0x870CD4 /
  0x8707CD, texture byte +0x56, and texture hashes 0x06D57415 /
  0xBD3D01A8 → 0x86EB80) before 0x5CEB40. So a per-car SKIN1 swap could
  only come from a table patched per instance, which is the remap
  (0x636850 → 0x631E30) above: SKIN1 → SKIN1 for traffic. No
  per-instance colour constant was found in the traffic draw.
* **Traffic wheels** (verified, 2026-10-29): the world's car pass
  0x6316E0 (the in-game draw of every car in the world list 0x890080,
  at most 99) draws a car whose kind (car +0x14 → +4) is 3 — traffic —
  with the **front-end draw 0x6162F0** (0x6317F5…), every other car with
  0x620AF0. So traffic never reads the in-game slot k / record +0x20
  (render info +0x404): its wheels sit as in the front end, outer face at
  record +0x1C (render info +0x400, 0x61759A), the tyre running inward,
  the left ones turned round — what the clone draws. 0x6162F0 draws the
  wheels only when the ride's byte +0x855 is set (0x61775D; cleared only
  by 0x61BE41 for slot 0x1D when 0x50F1D0 holds) and the LOD index
  (esp+0xB8, the model column of render info +0x17A8 + 0x20·slot +
  0x10·group + 4·LOD) is at most 2 (0x61776B), each wheel i when render
  info +0x2D78 + 0x20·i is set and, with a car object, its byte +0x3F4 +
  i (0x6177A8…0x617832). No traffic model is drawn without wheels by
  this path at LOD A–C. The clone (`traffic_close`, `TRAFFIC_MODEL=<m>`
  now forces every pool car to that model, `TRAFFIC_VIEW=side` looks
  from the left side): all 15 traffic models (4DR_SEDAN(02), AMBULANCE,
  BUS, COUPE, FIRETRUCK, HATCHBACK(02), MINIVAN, PANELVAN, PARCELVAN,
  PICKUP, SUV, TAXI(02)) draw four `<MODEL>_TIRE_FRONT_A` in their arches
  (contact sheet $OB_PRIVATE/wine/compare/traffic67/contact_before.png),
  with no non-finite or lifted wheel pose over 20 s of free roam. The
  reported van without wheels was not reproduced (the shots drew no
  traffic before <work dir>; the window always did). 2026-10-30:
  PANELVAN and TAXI draw with four wheels from 6 m (LOD A) and 35 m
  (LOD B, `TRAFFIC_VIEW=far`); the wheels' LOD C model is the `_B` one
  (the part's model column), and past LOD C (size < 10, ~110 m) the
  wheels are skipped (0x61776B) in the original too. TAXI shows in
  `traffic_close` again. **Open**: the clone
  drew every car at LOD A; now see **Car LOD**. The original under Wine
  was not shot next to a van.
* **Car LOD** (**verified** 2026-10-29 unless marked; `car::screen_size`,
  `car::pick_lod`, `car::lod_gate`, `CarPart::lods`, `CarRenderer::
  set_lod`, `car_lod` in game.rs): the world's car pass 0x6316E0 draws
  kind-3 cars (traffic) with 0x6162F0 and the rest (the player, racers)
  with 0x620AF0, passing the start LODs 0x8A1C10 / 0x8A1C14 (Car Geometry
  Detail). Each draw: size = 0x48ACB0(view, car position, 3.0): d = p −
  eye (camera +0x40), 0 when d · forward (+0x50) < −3; else k / (|d| − 3)
  · 3 when |d| − 3 > 1.5 (0x784260), else 2k, ftol; k = view +0x0C =
  256 / tan(fov / 2) (0x5C7ED0; 355.5 read from the running original's
  view 1 at 1280×960). Size < 4 → the car is not drawn (0x620CCA,
  0x616354). LOD = start, stepped while size ≤ table[LOD] — in game
  0x7A66E4 {40, 30, 20, 0}, front end / traffic 0x7A66F4 {40, 20, 10, 0}
  — clamped to 3 (**corrected** 2026-10-29, gps93: the step is strict,
  size < table[LOD] — fcomp then `test ah, 5; jp` stops on equal, at
  0x616733 and 0x621172 alike — the clone stepped on ≤); a second index (esp+0xF4 / +0x32C, the wheels') steps
  alike from 0x8A1C14. In game a showcase (0x499DB0) forces 0. The model
  drawn per slot and group is render info +0x17A8 + 0x20·slot + 0x10·group
  + 4·LOD (a null model draws nothing, 0x6230C6), the LODs the part
  database lists as `<name>_A`…`_D`; slot gates (switch 0x630DA8 /
  0x630D68 on the slot): 1, 2, 9, 10, 11 below 3, slot 9's group 1 only
  at 0, 0x13–0x16 and 0x1A only at 0; the wheels only at LOD ≤ 2
  (0x62751A in game, 0x61776B front end), their model's LOD clamped to 2
  (0x627650). The clone loads every LOD's solids (also from the shared
  part files) and adds each LOD's model as its own part (`CarPart::lods`);
  the renderer draws the parts of the car's LOD. **Airborne** (
  **verified**; `car::airborne_lod`, `car::underside_visible`; library
  `airborne_pick` / `note_underside` in the main view's draws and the
  traffic's, the shop fixes; the mirror view's draw is **open**): after the pick, LOD > 2
  with no wheel down (car +0x424 = 0) and render info +0x2EB8 set → 2
  (0x6211CB…0x6211F8; 0x616776 in the front-end draw). +0x2EB8 is the
  visible byte of record 10, CULLABLE_CAR_PART_UNDERNEATH (0x803E08: mode
  1, normals (∓0.07, 0, 1), offsets 0.05), written by 0x60C330 after the
  pick (0x621321), so the pick reads the previous draw's: with the eye in
  car space (0x43C7C0 inverse, 0x43C9C0) and the part at the mean of the
  four wheel points (0x638A4C…0x638B8B), visible while n·(eye − p) − 0.05
  < 0 for both planes; outside the game (0x8654A4 ≠ 6) always set; the
  normals unnormalised (0x8A1BD8 is never written). **Not ported**: split
  screen (g_ScreenLayout 3 and not the front end: size × 0.7, start 1),
  the debug overrides 0x802728 / 0x80272C, slots 0x11 / 0x12's gate on
  esp+0xD8, the LOD checks of the other draw blocks (0x621925 ≤ 2,
  0x6244E5 > 1, 0x625A86 < 3, 0x627096 > 1 — not traced
  which features they gate; 0x624CAB ≤ 2 gates the rear plate block
  0x624CBD…0x625A79 unless the draw's argument +0x2C is set, which the
  world pass 0x6316E0 passes as 0: `lod_gate` slot 0x21); brakes, spoiler (+0x1928), mirrors
  (+0x1BA8), exhaust and plate take their slot's model at the LOD.
  **Measured** (2026-10-29, gps93, player report "traffic LOD too
  aggressive"): a copy of SPEED2.EXE with a logger jumped to from
  0x6167CB (code cave 0x782B00, ring at 0x8BB800 in .data's page slack;
  the copy only, deleted afterwards) recorded each traffic draw's final
  LOD, size (esp+0x58), car position ([ebp+0x10]) and eye ([view +0x40]
  +0x40) in the career intro's free roam at High car detail (0x8A1C10 =
  0): 790 draws at 95–270 m, every size equal to ftol(355.5 / (d − 3) ·
  3) and every LOD equal to the strict front-end table (size 11 → C at
  ~96 m, size 9 → D from ~110 m). View 1's +0x0C was 355.5 and the chase
  camera's +0xC4 13018 (71.5°): 0x5C7ED0 builds +0x0C from the camera's
  unwidened angle, while the projection's x scale takes angle × 1.1
  (0x7A27E4) and its y scale angle × (h/w) × 1.1 / 0.8696 — so +0xC4 is
  the horizontal angle and k is the clone's `256 / tan(game_fov / 2)`
  (`FIELD_OF_VIEW` 13020). So the original also draws traffic at LOD B
  past ~29.7 m (size < 40), C past ~56.3 m (< 20), D past ~109.7 m (< 10).
  **Measured near** (2026-10-30, a private Wine copy under wined3d): a
  copy of SPEED2.EXE with two hooks into a cave at 0x782B00 (jmp from
  0x6167CB in 0x6162F0 and from 0x62121F in 0x620AF0, each replaying the
  overwritten `cmp; jne`; a 64-entry ring of {LOD, size, kind, car
  [ebp+0x10] x y z, eye x y} at 0x8BB800, counter 0x8BB7FC, both in
  .data's page slack; read with peek) in a Quick Race circuit (Peugeot
  206, chase camera). Traffic (0x6162F0) at horizontal distances 25–28 m:
  size 48–43 → A; 41–43 m: 27–26 → B; 54–56 m: 21–20 → B; 66–68 m: 16 → C;
  82–89 m: 13–12 → C; 115–121 m: 9–8 → D. The racers (0x620AF0, in-game
  table) at 33–37 m: size 35–31 → B. Every size is ftol(3k / (d − 3))
  with k = 355.5 (d horizontal, so a few % low), every LOD the strict
  table's, start 0 (0x8A1C10 read 0). So traffic at LOD B from ~30 m and
  C from ~56 m is the original's choice; the clone's `car_lod` gives the
  same. The front end's main-menu car also goes through 0x620AF0 (size
  410 at 9.4 m, LOD 0 by the showcase 0x499DB0). Each car is logged twice
  per frame, the second draw at LOD 2 (the player at 6 m, size 307):
  **inferred** to be the road-reflection view (view 4, whose draw passes
  another start), not traced. Shot aid: `TRAFFIC_VIEW=far TRAFFIC_DIST=<m>
  TRAFFIC_MODEL=<m> game --shot traffic_close` (camera behind-left at that
  distance with the chase lens).
* **Traffic zones** (verified): route triggers of kind 0x10 (chunk
  0x3414A, see Triggers; L4RA's PathsFreeRoam.bin has four).
  0x5D9C00 on 0x88F298 finds the one holding a point (box, then polygon).
  +0x2C + 4·level is a row of the AI tuning's spacing table (levels 1–3:
  +0x30/+0x34/+0x38; L4RA reads 0–2, 3–5, 6–8), +0x3C the caps zone (2,
  0, 1, 3). The table (GLOBALB 0x34B00 +0xA8C + 8·row, min/max m): 240–300,
  110–145, 80–95, 90–110, 70–100, 50–65, 50–70, 35–45, 20–25.
* **Lanes**: `RoutesFreeRoam.bin` (TRACKS/ROUTES<region>), lanes 2–5 /
  6–9 (0x40D3A0). A lane exists on a record when its header byte
  +0x82 + lane is non-zero (0x410CA0, via the route manager 0x88A334 and
  the point's record id); in L4RA's file 477 of 855 records carry traffic
  lanes, and the bytes agree with the points' lateral sentinel (−1, speed
  6) on 99.8% of point-lanes. Used lanes read 15–20 m/s. A dead end parks
  the car.
* **Junctions** (verified): the walk (0x41EC20) runs with the traffic
  goal "Be ambient traffic car" (vtable 0x784900, type 8, controller
  +0x6FC; ctor 0x40CB30, inline at 0x42AB73 / 0x42C884). At a point's junctions of the
  direction (bit 18 = travelled backward; bit 23 flips), in order: low
  nibble 3 ends the walk; the traffic type is the **high nibble** (bits
  4–7; racers read the low one): 2 is always taken (goal slot +0x2C,
  0x40CB70 → 0x5DCBF0(1)), 4 with odds ½ (slot +0x28, 0x413BB0, a draw
  0x5EA510 on the traffic manager 0x890080's numbers), or never when it
  has bit 26 unless the target record has the car's lane in its arriving
  direction (lane, or lane + 4 when bits 23 and 18 differ) and that lane
  is the highest (5 down to 2) the current point's record has (0x40B0B0);
  5, 7, 10 and the rest never. L4RA's free-roam file: high nibble 2 ×
  ~900, 4 × ~280, 7 × ~1900, 5/10 × ~235. Where the record lacks the
  car's lane the path takes the nearest it has, one up then one down, up
  to three apart (0x415B70). Clone: these rules; the draws come from
  each car's own numbers, and the racers' 30 m spacing after a bit-26
  turn is not applied (**inferred**).
* **Lane pick** (0x410CA0, verified): up to ten tries of lane
  2 + rand(400000)/100000 on the side (2–5 forward, 6–9 backward), then the
  first that exists, else none. In a drag race it first tries the lane of
  a random human racer.
* **Driver** (verified): the traffic controller (ctor 0x426580, vtable
  0x785428) builds the racers' CarDriver (0x425AB0, 0x460 bytes), so
  the steering and the pedals are the racers' code (AI driver section),
  in the mode the car's mover picks;
  the TrafficMover reads its wheel angle +0x1E0 and pedals. The driver
  runs in mode 3 on the TrafficMover (car +0x28 = 2) and mode 4 on the
  MeloMvr (AI driver, Mode): table steering at a look-ahead of
  max(0.55 s × speed, 5 m), not the racers' PID at 23–30 m (fixed
  2026-10-24: on the racers' mode the clone's traffic weaved about its
  lanes, median 2.0 m off the lane's centre, 67% of car-frames over 1.5
  m; on mode 3, 0.2 m and 0.4%, `traffic_lane_keeping`). The ctor's
  throttle cap +0x22C (0x425B44) is 0x783A28[0x89E804] only for a
  racer's controller (class 2) and 1.0 (0x784250) for any other, so 1
  for traffic. Clone: the same `AiDriver` with cap 1
  (`TRAFFIC_THROTTLE_CAP`), its `mover` (2, or 3 after a hit) and
  `traffic` set each frame in `traffic_think`.
* **Actions** (verified): the traffic goal (vtable 0x784900) proposes
  (slot +0x38, 0x426D70) "Obey traffic intersections" (type 0x11, vtable
  0x785990, update 0x425150) at priority 600 while the walk has an
  intersection ahead (0x4149E0), else "Avoid Cars as a traffic car" (type
  2, 0x785950, update 0x424CB0; ctor 0x424B40) at 300 when 0x4145B0 finds
  one, else its default (slot +0x3C, 0x426EC0) "Drive Traffic Lane" (type
  7, 0x785820, update 0x4242D0). The controller (0x429F56) takes a
  proposal of higher priority, or any once the current is done (+0x10 1
  or 2, 0x401990; Obey is done with no intersection ahead, 0x41CE30). A
  new action of type 8, 1 or 2 switches a car in mode 2 over loaded world
  collision to mode 3 (0x42A013; **verified** in the asm 0x429FF8–0x42A028:
  the new action's type +0x204 against 8, 1 and 2, car +0x5CC non-zero,
  car +0x28 = 2, then 0x5F46B0(3)). For traffic that is "Avoid Cars": a car
  on the TrafficMover that starts avoiding goes to the MeloMvr and stays
  there (nothing returns it, see Mover). **Observed** under Wine
  (2026-10-31, career free roam, the player standing at the airport,
  `a private Wine copy`, 1,255 placed car-samples): every mode-3 car's action
  was type 2 (or type 7 after the avoidance ended), every mode-2 car's
  type 7; 26–39 % of the placed samples were mode 3, most switching 1–4 s
  after being placed (cars catching the one placed before them). Ported
  2026-10-31 (`TrafficLane::take_avoid` → `Melo::new` in `traffic_think`;
  until then the clone kept avoiding cars on the TrafficMover). 0x4145B0
  proposes the avoidance when 0x402ED0 with look 99999 (0x47C34F80, so
  any object in its side window is taken at time 0) finds one whose time
  is at most 5 s and that 0x4140E0 keeps (a car crossing at the
  intersection is dropped); its exemption for an object over 20 m/s
  needs the controller's slot +0x68 and 0x60BEB0, never true for traffic.
  **0x424CB0** (verified, round 4, 2026-10-31): only while the tracked
  object (+0x218, a road-entry node: its car is node +0x1C) is among the
  entries (0x60C130), is a **human's car** (0x614890: the node's car's
  controller kind, car +0x14 → +4, is 1) and closes at under 40 m/s
  (entry +0x04 above −0x783750) does it compute the tracked gap (0x60C000
  − entry +0x0C − own +0x0C, at least 0) and try lane changes: 0x4146C0
  on the lane, then −1, +1, −2, +2 when the entry's +0x00 is positive,
  else +1, −1, +2, −2; the first allowed is set (0x405410); none marks
  the car blocked. 0x4146C0 takes a candidate in 2–5 that 0x412500
  allows from the lane (verified: the target 0 or 2–5; the walk's
  current record has it, 0x40AE90 — bytes +0x74 / +0x75 + 8·i for lanes
  0–1, +0x82 + i for 2–9 — and, for a car whose +0x24 is not 2, the
  target's reach (walk +0xAC + 4·lane) is at least the last walk point's
  +0x54 (0x40AA90) or at least 25 m past the current lane's (0x40AF30);
  in a drag race each lane between must also pass 0x40AF30(−0.1) and
  0x40B110. The reach floats' writer is **not traced**: the clone tests
  the record only) and where 0x402ED0 (lateral 0, the action's look)
  finds nothing, or an object 0x4140E0 drops, or one reaching in over
  5 s (0x783740), or one that is not the tracked object (0x60BEA0:
  pointer equality) and reaches later than +0x21C. Then 0x402ED0 on the
  car's lane with its own lateral and the look (+0x220): a found object
  becomes the tracked one (a new one: look 30 m, 0x41F00000) and +0x21C
  its time; standing (under 0.5 m/s) the look grows to max(look, 30, the
  found distance) and 0x40F4A0 asks for a lane (**not ported**; the call
  is traced in Lane request). Speed (+0x1F0): nothing found → the cap
  (controller slot +0x7C), or the slow rule at distance 0 when blocked;
  found → the slow rule when its time is under 1.5 s or the tracked gap
  is within the look, else the cap. The slow rule: 0 within 15 m
  (0x78374C) of the found distance, else 6 m/s (0x40C00000), +0x1F8 = 2.
  So **for a tracked traffic car or racer the gap stays 0** and the car
  holds 6 m/s behind whatever 0x402ED0 finds — within 30 m, or closing at
  all (a time under 99999), and never changes lanes. **Observed** under
  Wine (circuit 4014, level 3, `a private Wine copy`, the tracked node's car
  logged): the queue's cars hold +0x1F0 = 6 tracking traffic cars 29–124
  m off with times of 5.6 to 58,131 s. Then the path (0x420D40), +0x1F4 =
  0, +0x1FC = 0, +0x1FD = +0x1FE = 1. **Done** (vtable slot +0x18,
  0x41CD10, verified in the asm; Drive Traffic Lane's 0x403170 is never
  done): when the tracked object is gone from the entries; else not
  while it is ahead (0x60C000 ≥ 0) and its +0x04 is at most 5 m/s
  (0x7843E8); else done unless 0x402ED0 with the look finds an object
  reaching in 5 s or less that 0x4140E0 keeps, which it then tracks (a
  new one: look 30 m, its time). Clone: all of this
  (`TrafficLane::avoid_update`, `avoid_done`, `car_ahead` = 0x402ED0 on
  `entries` = 0x6148F0, see Road entries; a lane change sets the walk's
  lanes through 0x415B70 and the path eases over at the lateral rate).
  Until round 4 the clone ran the lane changes and the tracked gap for
  any tracked car, so its queue ran at the lane speeds (9–20 m/s) with
  gaps over 30 m; now it holds 6 m/s as the original (below, Race
  traffic under Wine). 0x42F330 (the
  "stopping behind" logic below) is "Avoid Avoidable" (type 1, 0x785858),
  not a traffic action.
* **Think** (verified): the traffic controller's update (slot +0x0C,
  0x429D90) runs for a traffic car (car +0x14 → +4 = 3) only on world
  updates where the update counter 0x7FB724 (++ in 0x57A170) modulo twice
  the traffic list's size is odd and half of it is the car's slot — one
  car every other update, each every 2n updates; other controllers think
  every 0.2 s (+0x6F0, 0x783B7C). 0x7FB724 is the frame loop's counter:
  0x57A0F0 (called once per simulated frame from the loop, 0x5814FA)
  adds 1 to it, while it adds round(dt × 60 + carry) to the 60 Hz tick
  0x7FB720 (0x787590, carry 0x865154) and round(dt × 4000 + ½) to the
  sim tick 0x865188, dt (0x865150) being the frame's time. A think: 0x413800 (leave the
  intersections), the walk (0x41D0E0 …), dead end → parked, intersection
  registration (0x417890), action choice, then 0x422150: the action's
  update (path), the goal's speed (0x41C380) and the driver's path (slot
  +0x3C), the action's path then cleared. Clone: one count per frame
  (`think_slot`). A placed car gets no think call of its own at placement
  (**verified**). The think 0x429D90 is reached through slot +0x0C of the
  controller vtable 0x785428 (its one reference is the vtable word at
  0x785434; no direct call or jump to it). The traffic world update 0x609CF0
  has one direct caller, 0x5811D0, which passes ecx = [0x890080]
  (**verified**). The update calls that slot in three loops over its car
  list (indirect sites 0x609F1E, 0x609F5E and 0x609F9D; each passes the
  update's argument, which the think subtracts from its timer as dt)
  (**verified**). Before the loops it runs the placement pass 0x42F910 when
  its +0x118 is not 0 (**verified**). The think's gate for a traffic car
  (A = [C+4], with C the think's object and [[A+0x14]+4] = 3) passes only if
  0x430340 returns nonzero, [C+0x77D] is 0, the counter 0x7FB724 mod 2N is
  odd, and A is list[begin + (residue >> 1)] with residue >> 1 < N, where
  N = end - begin of the list at 0x890080 (**verified**). The think timer
  +0x6F0 is decremented for every car but is not part of the traffic gate
  (**verified**). So a placed car's first think is its own turn in that
  cycle, up to 2N - 1 counter steps after placement. Whether that turn can
  fall in the placement update is **not verified**: it depends on whether the
  pass has added the car to the 0x890080 list before the loops (the pass
  reads that list and calls 0x5F0E40 with it; that call was not identified).
  In bounded windows (each up to the next listed function start), the
  placement chain 0x42F910, 0x42F800, 0x42E9F0, 0x42E790, 0x42E620 and
  0x42C950 has no call through slot +0x0C and no register-indirect call
  (**verified**). 0x42C950's calls through slots +0x18 (0x413540) and +0x20
  (0x40D4A0) are verified; its other indirect calls in those windows were not
  classified. The helpers 0x413540, 0x40D4A0, 0x42AA20, 0x428490, 0x5FBEE0
  and 0x5F46B0 have no +0x0C or register-indirect call in their first 160
  lines (**verified**, bounded). The 503 [reg+0x0C] sites in the binary were
  not enumerated, so that 0x609CF0 is the only route to the think is **not
  verified**. The clone's `traffic_think` (game.rs, lines 1309 and 1326) sets
  `think[i] = true` for a placed car and lets a plan-less car think, so it
  steers on its placement frame unconditionally. The binary's gate is not
  unconditional (**verified** mismatch). Left unchanged: a fix needs the car's
  motion before its first think (walk and lane, Path below), which the clone
  does not model. The driver steers every frame on the last think's path and
  speed.
* **Stopped for good** (controller +0x710; **verified** in the decompile
  and the asm, round 3, 2026-10-31): the think 0x429D90, after the NIS /
  +0x77D test and before the turn gate, calls **0x408DE0** on every world
  update: while the car's collision body has live contact records (body
  +0x3C → byte +0x18, each 0x5A0830(i)), a record whose summed pushes
  (+0x60, 0x5A9689) exceed 30 (0x783B8C), or the car under 1 m/s (+0x430,
  0x784250) while its driver's target is over 1 m/s (car +0x30 is the
  CarDriver, vtable 0x784EB0, slot +0x74 = 0x40FFF0: +0x230), stops it:
  0x4023E0 aims the driver 1 m ahead on the heading (slot +0x40), the
  driver's speed slot +0x38 gets (0, 0, 0, 1) — target 0 — and +0x710 = 1.
  On its turn the think then only leaves its intersections (0x413800) and
  repeats that (0x429E50 → 0x429F2B). The same stop follows a path the
  road walk cannot build (0x428720 false, 0x429E6B) and **0x417750**: the
  walk's last point has a junction of low nibble 3 in the walk's direction
  (bit 18 against the walk point's slot +4) and (its +0x54 − the car's road
  entry +0x18) / max(+0x430, 0.1) is under 1 s. Then **0x40ABB0** (the
  walk at its end, at most one point past the car, the point's record
  0x88A334[+0x5C] +0x0C = 1 unless walk +0xC8) parks the car (+0x77C).
  The placement does not clear the car's contact records (verified:
  0x5F52D0 → 0x59F890 resets the car, records end only 0.3 s after their
  last push through 0x592BE0 → 0x5B2280, and a pair with a record is never
  deleted, 0x592E70), so a re-placed car's first stuck test can read its
  old pushes, natively as in the clone.
  Only the placement clears +0x710: 0x5FBEE0 calls the controller's slot
  +0x34, **0x42A050** (+0x710 = +0x77C = +0x798 = 0, a new Drive Traffic
  Lane action, the path). A stopped car is not parked, so it stays placed
  while within 250 m of the player and holds the placer through the jam
  test (0x4112A0: under 1 m/s, not parked). **Observed** (Wine, Quick Race
  circuit 4014, levels 1 and 3, `a private Wine copy`, memory log of +0x710):
  a mode-2 car stops at (−246, 307), the end of a traffic lane at the race
  road's closure (z 0.6), with no contact records (+0x710 1, target 0,
  action 7), and stays; a mode-3 car avoiding at 8.4 m/s stops on a hard
  push (records 1). Clone: `TrafficCar::stopped`, `STUCK_IMPULSE`,
  `STALL_SPEED`, `STALL_TARGET` (the contact notes carry each record's
  +0x60, `ContactNote::impulse`), `TrafficLane::ends_ahead`
  (`END_STOP_TIME`; the car's place on the walk is its distance along the
  first walk point, **inferred** for road entry +0x18), cleared by
  `place_traffic`; the replay snapshot holds it. **0x428720's failure** (round 4,
  verified in the decompile and the asm 0x429E70–0x429E9E): it returns
  false when the walk is empty (planner +0x320 = 0: count 0, own index
  0xFF), when the controller's road node (+0x0C) is null (0x411BE0
  clears the walk), or when the car's own entry s (+0x10) is past the
  walk's last point's +0x54; the think then aims 1 m ahead (0x4023E0),
  sets target 0 (slot +0x38) and +0x710 = 1 — stopped for good, not
  parked. Parking at the walk's end is 0x40ABB0's only (after 0x417750):
  the walk point at or after the car's s (0x40AB10) is the last or the
  one before it, and that point's record has +0x0C = 1 (or walk +0xC8,
  writer **not traced**). **No route record has +0x0C = 1**: all 17,226
  records of the 226 shipped Routes*.bin read 0, and so did all 474
  records in memory under Wine (0x88A334, Quick Race sprint). So a walk
  that ends without a type-3 junction runs the car on to its last point,
  where it stops for good. Clone (round 4): `TrafficLane::update`
  returning None (the car past its last point) stops the car (+0x710)
  instead of parking it; `parks_at_end` keeps 0x40ABB0's test (never
  true with the shipped data). Until round 4 the clone parked it.
* **Path** (verified unless noted): Drive Traffic Lane's update 0x4242D0
  (lane class 0–1 → 1, 2–9 → 2 against the goal's slot +0x20: equal
  0x420D40, else 0x420ED0; +0x1F0 = controller slot +0x7C, +0x1F8 = 2,
  +0x1FC = 0, +0x1FD = +0x1FE = 1) calls 0x420D40 on the controller's
  lane record +0x30C {lane, lateral offset, rate}; the traffic controller's lane setter (slot +0x20,
  0x40D4A0) writes {lane, 0, 0.151 (0x7837D8)}. 0x420D40 (rate ≠ 999:
  margin 1 m; 5 m in some race modes) rebuilds the path (0x41D160(0, 0)
  unless 0x41BFE0 / car +0x30 +0x1D9 keep its front) through 0x41D530: a
  point per walk point after the car's (≤ 20, 0x415D50), the lateral
  offset starting at the car's own off the lane (0x60BFB0) and moving
  towards the record's offset by rate × the point's segment (0x40FC80),
  then held within the point's lane room less the margin (0x40FC80: low
  = room +0x18 (lane 0) / +0x14 + 4·lane (2–5), high = +0x2C / +0x28 +
  4·lane, 0 for other lanes; margin = 1 m + the walk's +0x10 +
  0x30·(+0x308), the car's own entry +0x08 (see Road entries), 0; outside the race modes that
  average crossed bounds, low = min(low + margin, 0) and high = max(high
  − margin, 0)). Only 0x41FD50 writes the rooms, from 0x428030, which
  for a traffic car runs only in the placement (0x42C950 has no direct call to 0x42A190 or 0x42A360 in the disassembly; that chain is not traced): the last point's from its slots +0x0C / +0x10 (forward
  0x40B2A0 / 0x40B1F0: −half width +0x14, +0x16, less the lane's
  position 0x401C40 and the point's adjustment; backward 0x40B840 /
  0x40B780 the half widths swapped), then a pass back to the first that
  also limits each by the next (decoded as a loop, see Room pass; not
  applied). Points the walk adds later keep the ctor's 0 (0x40AD20), so
  there the offset is held to 0 — the lane's centre. Where 0x415B70
  changes the lane the offset carries over by the lanes' difference.
  Clone: `TrafficLane::update` (`room`, `LANE_MARGIN`; the steps the
  placement's walk built carry room).
  **Kept front** (2026-11-04): with the driver's +0x1D9 set (observed 1 for every
  traffic car) 0x420D40 keeps the path's front between thinks and plans afresh only on
  0x41BFE0; the clone now does too (docs/world-systems-parity.md, Traffic).
  **Where the path starts** (verified, 2026-11-01, decompile and asm):
  0x41D530 with an empty path (path count byte 0, as 0x41D160(0, 0)
  leaves it) starts at walk index entry +0x28 + 1 of the car's own road
  entry (planner +0x308), at most the walk's last. Entry +0x28 is EAX of
  0x419C10 (0x614D0F, stored at 0x614D18; 0x419C10 keeps 0x419B20's
  return in EAX, asm 0x419C3A–0x419C56): **0x4198C0** with flags 0 — the
  walk point whose place on the lane (0x4122C0 at the point's +0x54,
  lateral 0) has the least 2D distance from the car plus 10 × |the
  record point's z (i16 +0x12 / 8) − the car's z|. So the car's point is
  its *nearest* walk point, not one it has driven past. Until 2026-11-01
  the clone moved its walk on only once the car was past the point's
  segment along the point's direction; a car on the MeloMvr that had
  swung off its lane (after a hit or an avoidance) and had the next point
  inside its turning circle then orbited that point for good — the player's
  "traffic drives in a circle". **Observed**: the clone's
  `traffic_circling_soak` near the Industrial graphics shop (IND_GRAPHICS
  at (456.6, −1553.3)) found 35 circling cars in 8 × 240 s, e.g. a
  MINIVAN on the MeloMvr at (667, −1294), walk step (691, 30, backward,
  lane 4), turning 640–690° every 10 s at 6 m/s for over 200 s, and a
  TAXI02 at (285, −1575) on (703, 24); the city-wide soak 8 more (12
  spots). The original under Wine (`a private Wine copy`, career free roam with
  the player teleported to (690, −1330) and (300, −1560), 300 s each, all
  16 traffic cars sampled at ~17 Hz, 1,703 MeloMvr car-samples at the
  first spot): no car turned over 300° in 10 s. Clone: `TrafficLane::update`
  takes the nearest point (the points before it dropped; the walk keeps
  no points behind the car, so the nearest never goes back, **inferred**;
  0x4198C0's skip of far points through 0x75BED4 only speeds up its
  search, **inferred**). After the fix: 0 circling in 24 spots × 300 s
  and four Quick Races at level 3 (circuits 4014 and 4001, sprint 4123,
  drag 4201; `traffic_circling_soak`; the
  detector counts apart cars stopped for good, whose turns are a hit's
  spin with target 0); the lane-keeping measure (`traffic_lane_keeping`)
  reads a median of 0.34 m off the lane's centre, p90 0.72 m, 0.8 % of
  car-frames over 1.5 m (the path now starts one point further on in the
  second half of each segment).
* **Speed** (verified unless noted): placed at the lane's speed byte
  (0x42E9F0 hands 0x42C950 control point +0x2C + lane, which goes to
  0x5FBEE0 → 0x5F52D0 as the car's speed; the 55 mph argument goes to
  controller +0x79C). The traffic goal's speed (slot +0x34 → 0x41C380,
  the racers' is 0x41C5E0) hands the driver (slot +0x38, 0x4054A0: +0x230
  target, +0x234, +0x238, +0x23C pedals allowed) the least of min(+0x79C,
  55 mph) (controller slot +0x7C, 0x41E7A0), the walk's lane speed at the
  car (0x411DD0, linear between walk points; traffic lanes limited to
  ±0.2 m/s per metre, 0x40A8C0) and at max(1.3 s × speed, 8 m) ahead,
  0 when parked; then min with the action's +0x1F0 unless its +0x1FC
  (clear in both traffic actions) and with the goal's slot +0x4C
  (55 mph). Before all that the 55 mph cap meets a crest cap (verified):
  0x411FA0 from the car's walk point, over consecutive points while the
  first's +0x54 is within max(1.3 s × speed, 20 m) (0x783564, 0x783568),
  the least of next − this of slot +0x24 (forward 0x40D0B0: grade byte
  +0x21 / 128; backward 0x40D300: minus the previous point's, 0x5D8A00,
  its own for the first) and where (this point's +0x54); under −0.1
  (0x78356C) the cap becomes (cap − 10)·(1 − t) + 10 with t = table
  0x7E9BC0 (0x40C840: {10 m, 1}, {40 m, 0} at 0x7E8DB0, linear, clamped)
  at that place — slowing for a crest, not a bend. Clone: `crest_cap`
  (the walk's +0x54 measured from the car's point). The clone passes +0x230 only (+0x234 /
  +0x238 are not read by its driver). In races the
  teleporter
  path (below) caps a placed car at min(entry +0xB0 + 8·level mph ×
  0.447, lane byte).
* **Race teleporters** (route chunk 0x34124, 0x108-byte entries at
  0x883DC4, count 0x883DC8, loaded by 0x5DEC27; RoutesFreeRoam has none):
  0x42E790 places a traffic car for a racer: a coin flip for the side,
  0x409570 picks an entry (occupancy byte +0x100 < 6, a record at +0x00,
  per-side windows +0x10/+0xE0 × speed, at least 20 m and 120 m) whose
  window holds the racer's race coordinate (racer +0x8FC; ±60 m when
  0x4094A0 is false), then 0x42C950 sets it up. 0x89E7A8 indexes the
  entries' per-level fields: the traffic level 0x89E7A8, which also gates
  the placer and indexes the oncoming share. **No shipped file has the
  chunk** (all 226 Routes*.bin checked; 0x34123 is in 122), so the count
  stays 0, 0x409570's loop (count ≤ 0 at 0x4095AA) finds nothing and
  0x42E790 fails; 0x42F800 then goes on to the ordinary placement. In
  the retail data the teleporter path never places a car; the clone has
  none (native behaviour, nothing to port).
* **Mover** (verified unless noted): the mode switch 0x5F46B0 builds a
  car's mover per mode (car +0x28; 0 the 0x7A5BB8 one, 1 the physics
  mover 0x5B5CD0, 2 TrafficMover 0x408740, 3 0x5B6D10, 4 0x5A9040, 5
  0x5B6FD0; with car +0x5C0 clear any mode but 0 / 4 becomes 4, or 0 for
  a car whose +0x24 is 1). 0x5F4410 gives a traffic car (+0x24 = 4) mode
  2 and the placement sets it (0x42C930). TrafficMover (0x24 bytes:
  +0x0C x, +0x10 y from car +0x60/+0x64, +0x14 heading = u16 +0x40C ×
  2π/65536, +0x18 speed from +0x430) moves the car as a kinematic bicycle
  each frame (0x408820): d = speed·dt; with the front-wheel angle δ (the
  controller's u16 +0x1E0, wrapped to ±π) at least wheelbase × 0.01
  (wheelbase = car type wheel 0 x − wheel 3 x) it turns about the point
  wheelbase / δ to the left of the rear axle (pos + forward × wheel 3 x)
  by d / radius, else it goes straight; then speed += (accel(v) · gas −
  brake(v) · brake) · dt, both tables at the old speed (0x5764E0, linear
  over 4 points on 0..24.5865 m/s: accel 0x7EA03C 3.576, 2.682, 2.012,
  1.341; brake 0x7EA050 6.705, 13.41, 20.12, 26.82 m/s²), held within
  0..24.5865 (55 mph). It writes x/y, keeps z, sets the body on the road
  (0x5F5020 → 0x5F4A00, verified: each wheel block's car-space position
  (car +0x20 +0x10 + 0x30·i) turned by the heading (0x5BAE70) finds its
  ground (0x5E72F0); with all four found the body's axes are forward =
  norm((w1 − w2) + (w0 − w3)), left = norm((w3 − w2) + (w0 − w1)), up =
  forward × left, kept as they come, and z = mean ground + (Σ(H +0x20 −
  H +0x18, + 0x30·i)/4 + H +0x08) / |up.z| with H = car +0x20 (0x5F4CE0:
  `fld [H+esi+0x20]; fsub [H+esi+0x18]`, the record's wheel radius
  +0x130 less its z +0x128, i.e. radius − z; 0x5F4F75: H +0x08 = the
  centre of mass's z, record +0x118, or with car +0x34's slot +0x0C
  object that object's +0x1F0 +8 = the rigid body's centre of mass
  +0x1F8), the ×0.25 at 0x784D28 — the height of the centre of mass;
  else level on the heading at the old z. Clone: all of this (the
  centre of mass's height turned into the car-space origin's; x/y kept as
  the origin's (verified: 0x5F4A00 stores only the z), the axes squared up, the clone's
  `road_below` for 0x5E72F0), the velocity along the
  move at the speed, the yaw rate from the heading change and each wheel
  rolling at speed / radius. **Hit** (verified, 2026-10-28): the world
  collision step 0x5FFA60 gathers the frame's contacts from the pair list
  0x86B490 (0x5B5060; track, car and prop pairs alike) and resolves each,
  at most 0x802080 (20), through 0x5B2620 (the call at 0x5FFBB4), which
  first calls slot +0x48 of both bodies with the other (0x5B2637,
  0x5B2643). A car's collision body (car +0x3C, built by 0x5AEC30 from
  0x5FB966 / 0x601202: vtable 0x7A2478) has 0x593950 there: the other's
  car (+0x130), or 0 when the other is not a car body (its byte +0x11
  set) or is null, goes to 0x5F4970 on its own car, which calls the
  controller's slot +0x38 (car +0x2C; with no controller, modes other than
  1 and 3 go to 0x5F46B0). The traffic AI's (vtable 0x785428,
  "TrafficController" serializer 0x429CF0) is 0x408ED0: mode 3 when car
  +0x5CC is set, whatever it touched (the argument is not read; else it
  stays on the TrafficMover). So any resolved contact — a wall, a prop or
  a car — hands a traffic car to the MeloMvr. The racers' base handler 0x402450
  instead goes to mode 1 when the other car has +0x18 (or the flag), else
  0x5F4410's mode, mode 4 without car +0x5CC.
  **Car +0x5CC** (verified) is the +0x08 of the car's world-position
  tracker at +0x5C4: the world collision pack of the track section under
  the car. 0x5E68E0(pos, r), when the car has left the tracker's box
  (+0x0C/+0x10 ± +0x18 + r), finds the section (0x5DFEC0 on 0x883E70) and
  links the tracker into that section's loaded pack (0x591390, 0x5E68A0:
  +0x08 = pack, its list +0x68), null while the section is not loaded;
  the pack's section id is its i16 +0x30. Each frame 0x5F6CE0 sets car
  +0x5C0 = (+0x5CC ≠ 0) and, when it changes, aligns the car (0x5F5020) or
  frees its wheels (0x592050/0x592090), and tells the mover (slot +0x10)
  and the controller (car +0x2C, slot +0x10). So a traffic car goes to
  the MeloMvr only where the world under it is loaded.
  **MeloMvr** (0x5B6D10, vtable 0x7A2540, "MeloMvr"; verified unless
  noted; `src/melo.rs`, `Input::Melo`): +0x20 a rigid body (0x5A7F40 on
  the tuning block H = car +0x20, the physics' RigidBody 0x7A20CC), +0x24
  the chassis "Melov8r" (0x5B0040, vtable 0x7A2568), +0x28 an aero block
  (H +0x260, the drag / downforce), +0x38..+0x44 four physics Wheel
  objects (0x5A8170, vtable 0x7A20F0) that are not in the state list —
  only the body and the chassis are integrated. The chassis replaces the
  Suspension / Engine / DriveTrain objects: +0x20 the wheel spins, +0x30
  compressions, +0x50 loads, +0x60 contact, +0x70 + 8·i the tyre's
  car-space (x, y) force, +0x90 + 8·i the slips, +0xB4 + 4·i lock flags,
  +0x120 rpm, +0x124 gear (ctor 2), +0x128 gas, +0x12C brake, +0x130 the
  wheel angle, +0x134 the gear request, +0x13C / +0x140 the sub-step scale
  and grip (1), +0x144 the skill grip 0.25 + 0.025·clamp(0x5B69A0(1, −1)
  on car +0x10, the rating on the skills' 0–10 scale, 0, 10), +0x19C /
  +0x1C0 up / down-shift rpm by gear (0x5AC7E0, the first up-shift × 1,
  1.2, 1.1 by layout, 0x7FC2B8 + 0x144·layout), +0x158 / +0x16C / +0x180
  the torque (H +0x200, 9 points idle … max), closed-throttle (H +0x248,
  3) and nitrous (0x7FBF38) tables. 0x5A2A70 starts it from the car: the
  wheel spins (car +0x2F0 …), gear (+0x3F4), rpm (+0x400). Its frame
  (0x5AADA0): the wheels-down mean of the surfaces' +0x50 drag into aero
  +0x34 (1 with none), body +0x150 / +0x154 = 1 (the rest hold), gravity
  9.8 and aero +0x30 = 1 — in crash mode (car +0x38 = 1) 0, 11 and a
  quarter per wheel down; the ground inertia with a wheel down (leaving
  it L.z ÷ record +0x270); 0x5A29B0: the centre of mass, the ride
  offsets (car +0x3B8 …, which 0x5A2FE0 ramps towards controller targets,
  0 for traffic (**inferred**: +0x1FC has 84 references, some storing 1, so its 0 from the traffic actions is untraced; +0x200 to +0x208 untraced) and the preload 0x598A00 (as the
  physics': weight + downforce × aero × forward speed × record +0x37C,
  split by the centre of mass between the axles); the controls (+0x1E0,
  +0x20C, max(+0x214, ½ +0x210), slot +0x20); the aero object's +0x10 =
  the driver's gas (+0x20C) and +0x14 = car +0x424 × 0.25 (0x5AAFD2..
  0x5AAFF2: so the drag's (2 − t) term and the downforce arms of
  0x59E630 run as on the physics mover; **The outrun-pace pass**: the clone took t = 0, i.e.
  twice the drag at full gas, and 40 / 41 % of the downforce always);
  0x5A3390 (arg car +0x468, the catch-up; 0x7FC0A0 = 1 so the arg is
  used): k into aero +0x08 (|v| ÷ k), body +0x208 (top speed × k), the grip
  +0x140 (× 0x7A1998 = 1 when +0x42C × 2.237 > 30 and car +0x464 = 1) and
  the chassis' +0x13C, which 0x598AD0 reads: the torque curves at
  rpm ÷ k, the drive × (1 + 3 (k − 1)) and the driven wheels' lever ÷ 1.25
  (1 + 2.5 (k − 1)) (0x598C6E..0x598CB7; **verified** the outrun-pace pass, ported); grip × (1 − 0.025·0x5ED5E0, the road's wetness at
  the car via 0x61E390 (**corrected 2026-10-30, <work dir>**: 0x8A1F34 is not a
  weather object but the data pointer (+0x0C) of the "Rain" parameter map
  object 0x8A1F28, filled by the map loader; no instruction names it, yet
  under Wine it rains. So the grip drops by up to 2.5 % while it rains at
  the car, see **Rain**; `melo.rs` `grip`)); then ⌊dt · 70⌋ + 1 sub-steps (0x7FC040), each the
  Euler step 0x5AA490 (derivatives of the body and the chassis, then
  0x5AB0D0 for the next: slot +0x1C of each — the chassis' 0x5AB1A0 —
  the rest test 0x5A5E30, the bump stops 0x5A3E90, and 0x5AB140) and
  0x5BB0E0's orthonormalisation; the write-back 0x5A2B70. The force pass
  0x5AB0D0 has no caller but the sim vtable 0x7A2534 slot +4 (the
  post-step), so a sub-step integrates the sums the previous pass left,
  the first of a frame those of the previous frame's last pass (verified,
  the clone's `Melo::pending`; the very first
  sub-step on a new mover runs a pass first: **inferred**, the new rigid
  body's sums are not traced). The pass sees the rows before 0x5BB0E0.
  **Suspension** (0x5A3710 over 0x598D60): wheel i at the centre of mass
  + R·(block − centre of mass − (0, 0, ride)) finds its ground (0x5E72F0,
  the normal into +0xE0 + 0x10·i); x = (ground − z) / (n·up) (−100 under
  0.001) + radius; within max extension … max compression (record
  +0x280 / +0x2A0 +0x18 / +0x14) with contact, the excess over the top
  into +0xC4 + 4·i; below, no contact. Load = max(0, (max(x, 0)·k0 + 1)
  ·k1·0.9·x + (bump +8 or, with the rate ≤ 0, rebound +0xC)·0.9·rate +
  preload), rate over the sub-step (≥ 0.0001 s); no anti-roll, no
  camber term. **Gearbox** (0x5AB1A0, after it): brake over 0.95 with
  |ω| < 30 locks a wheel (spin 0), under 0.2 frees it; rpm = 9.549304 ×
  the mean front (× H +0x1B8) or rear (× +0x1BC) spin, by the split
  +0x1C0 (under 0.2 front, over 0.8 rear, else both's mean) × ratio
  +0x1D0 + 4·gear; request −1: at least gear 2, up while past +0x19C +
  4·gear (to the top), down while under +0x1C0 + 4·gear (above 2);
  request 0: gear 0, reverse; held within idle +0x1F0 … max +0x1F8.
  **Bump stop** (0x5A3E90): past the top, 0.2 × mass × the hub's closing
  speed along the ground normal (if > 0) pushes along it at the hub; the
  totals go to car +0x330 + 4·i. **Tyres** (slot +0x14, 0x5A38F0): at a
  wheel in contact the hub's speed in the plane, v + ω.z × arm only,
  splits along / across the heading (0x43C570, plus the u16 wheel angle
  for the fronts): lateral, and longitudinal less radius × spin, into
  +0x90; slip over 6 m/s marks a skid; gain 1, or with the slip over 0.05
  × grip the skill grip × grip (and × (1 − 0.6t) over the timer +0x1E4,
  which nothing in a traffic car's path sets); force −gain × load × slip ×
  1.25 each way, turned back by the wheel angle into car space, (x, y,
  load) (zeroed (x, y) with +0x138 set: cleared by the ctor, no writer
  traced) into the world; its horizontal part held within 4 (2 online,
  3.2 on track flag 0x2000 of record +0x94: neither in the clone) × load ×
  grip and × (2.5·(scale − 1) + 1) past scale 1; then the load's slope
  part off — all of load / up.z × up.xy when locked or the brake is over
  0.4 under 0.1 m/s, else |left.x|, |left.y| of it while under the cap;
  applied at the wheel's arm through 0x59BB10's caps; +0x70 + 8·i keeps
  the car-space (x, y), at most 50. Then aero (0x59E630, the physics'
  drag and downforce × the aero factor), 0x5A2FE0 (hop forces from the
  controller's +0x1EC … +0x1F8, 0 for traffic: none (**inferred**: no store to the controller's +0x1EC in 0x40–0x43, the one match at 0x42B739 being a stack slot; the gate in 0x5A2FE0 is untraced) and
  0x5A3420 (not in crash mode, +0x138 clear: under two wheels down
  −g(|ω|)·m·ω, g 0x86B498 = {0, 0}, {0.1, 0}, {2, 20} from 0x7FC8E0; with
  none down two righting torques of strength (cos − 1)·100·m ≤ 0, which
  it zeroes: no effect). **Force pass** (verified): the integrator
  0x5AA490 is the generic Euler step — 0x5A0D30 asks every state object
  for its derivatives (slot +0x18) and adds dt × each; the chassis' slot
  +0x18 (0x598AD0, vtable 0x7A2580; called nowhere else) is the four
  wheel spins' rate. Per wheel i (0, 1 front): with flag +0xB4 + 4·i set
  0; else r = rpm / +0x13C, engine E = T(r)·thr − B(r)·(1 − thr), plus
  N(r)·car +0x2AC·k[+0x198] with nitrous (+0x198 > 0; k = 1.7, 1.0,
  1.4, 1.7 at 0x7A17A0; never for traffic); E = 0 when thr < 0.05 and
  the rpm equals +0x1F0. Drive D = ratio·gear·E·share·0.5 (ratio H +0x1B8
  + 4·(i ≥ 2), gear +0x1D0 + 4·+0x124, share 1 − +0x1C0 for i < 2 else
  +0x1C0); brake Bk = −0.5·+0x12C·clamp(ω_i, −20, 20) (2.5 × −0.2); k1 =
  1, k2 = 1.25, or with +0x13C > 1, d = +0x13C − 1: k1 = 3d + 1, k2 =
  1.25·(2.5d + 1); R = 0x5767A0 on the graph 0x86B578 (built at 0x77DF40
  from 0x7FC8C8: {0, 0.05}, {2, 0.322}, {20, 0.5}) at the body's speed
  |+0xA0|, ÷ (k1·k2) when the share is positive; ω̇_i = (k1·D + Bk −
  R·(+0x70 + 8·i)) / 0.008 (0x7A1984) — the reaction is the tyre's
  car-space x force from 0x5A38F0. The body's slots are the physics
  mover's (the clone's: limiter, angular damping, rest hold, Euler step).
  Clone: all of this (0x5A5E30 / 0x5992C0 since 2026-10-29, see **Rest
  test**), except the bump stops' re-run of the slot +0x1C pass. Modes 1
  and 3 set car +0x3C +0x5C to 1, modes 0, 2, 4 to 0.
  **The way back** (verified): nothing in mode 3 returns a traffic car to
  mode 2 by itself. Mode 2 comes back with the placement (0x42C930), the
  controller's slot +0x10 (0x408C40) when world collision appears under
  the car (true: 0x4022D0, then 0x5F4410's mode 2; false: mode 4 when the
  mover's slot +0x10 answers 1, 0x570A70 for both movers), the parked
  cars' return (0x409A30 each placer frame: +0x77D cars whose body
  answers slot +0x54) and the broadcast slot +0x14 (0x408CA0). Clone: a
  placed car takes the TrafficMover; it goes through the collision step
  with the player and is put back on its mover unless the step resolved a
  contact of it (`CarState::resolved`, counted in `step_world` for track,
  car and prop contacts; until 2026-10-28 the clone tested for a live
  car-pair record instead, so a wall or prop never switched it and a
  stale record could: see Car field below), after which the MeloMvr (above, `Melo::new`
  with the model's rating) drives it until it is placed again —
  the clone's world stays loaded, so as in the game that is the only way
  back. 0x430A9C switches a car with +0x5CC to mode 5 (0x5B6FD0). The
  traffic action AIActionDriveTrafficLane (vtable 0x785820, name 0x4242C0)
  reads the controller's lane +0x30C: 0–1 → 1, 2–9 → 2.
* **Placer** (0x42F910, per frame; verified unless noted): runs only
  while the traffic level 0x89E7A8 is non-zero, the traffic list
  (0x890080 +0x18..+0x10) is not empty, the game state (*0x890118) is 4
  and there are human racers. Every 5 s it re-streams models (0x412950).
  A countdown (+0x00) of 1 s (0x783BA0; 0.3 s, 0x783BA4, in a drag race)
  starts a pass: from the slot after the last one looked at (+0x04),
  round-robin, each car that 0x417BB0 says needs placing (verified: its
  controller's slot +0x68, 0x41E790 → the goal's slot +0x10, 0x40C9C0, is
  goal type == 2, false for traffic's type 8, so the branch is 0x410DE0:
  0x409890 in a coordinate race, else 0x409840 on car +0x60; 0x5F0FC0
  takes the least 2D distance to the points at manager +0x80 + 0x10·i,
  which 0x609CF0 fills each frame from the first +0x14 cars of the list
  (+0x60/+0x64/+0x68, the rest 9999999): two of them, four or six with
  0x89CF69 / 0x8669F4 / 0x866A14 and +0x14 ≥ 3 / ≥ 5. The first +0x14
  cars are the human racers (**verified** in memory, 2026-10-31, career
  free roam: world [0x890080] +0x10 = 20 cars, +0x14 = 1, +0x18 = 4 —
  list[0] the player, list[1..3] the encounter racers, list[4..19] the
  16 traffic cars; 0x609CF0's three think loops run +0x18..+0x10,
  +0x14..+0x18 and 0..+0x14), so the clone's measure to the player is
  native) — parked (car
  +0x77C) or, outside coordinate races, more than 250 m (0x78429C) from
  the players (0x409840 → 0x5F0FC0; in coordinate races 0x88F2BE more than
  250 m of race coordinate, 0x409890) — gets a try (0x42F800); a failed
  try parks it. A pass handles 1 car, 16 in a drag race (0x4018F0 on the
  race parameters 0x89E7A0: +0x38 = 0x89E7D8, or its online twin
  0x890108), so free roam fills its pool one car a second. The pass steps
  one slot at a time (**verified**, 0x42FA70–0x42FAC5: ebx + 1, wrapping at
  the traffic list's size, until the tries reach the pass's count or ebx
  is back at +0x04, then stored in +0x04). The clone stepped from the
  moving cursor until 2026-10-31 (slots 1, 2, 3, … apart, so some cars were
  never looked at and a far stopped car stayed placed); fixed.
* **Spacing** (0x418240, verified unless noted): 0x42F910 draws, per human racer,
  the traffic zone's spacing row at the level: min + rand(max − min)
  (0x5EA510); 99999 outside every zone or when 0x4112A0 finds a jam (the
  racer under 30 m/s and, in the traffic list 0x890080 +0x1C (from +0x18
  to +0x10), a car under 1 m/s that is not parked (class 3 with +0x77C)
  within 250 m (0x7E9720) by |0x5D5CE0(racer +0x8FC, car +0x8FC)| (verified: 0x4112A0, the jam test, makes this call), the
  road distance of their race coordinates (0x5D5CE0, verified: a − b, wrapped by the lap +0x18 once it is past ± the half +0x1C on a closed race road, byte +0x24). In free roam every car's +0x8FC
  reads 0 (**verified** in memory, 2026-10-31: the player, the encounter
  racers and all 16 traffic cars over 300 s), so there the distance is 0
  and any placed traffic car under 1 m/s anywhere is a jam; the clone
  takes that in free roam (since 2026-10-31; before, 250 m in 2D), in a
  race the placer's distance below), and 0x42F800 then does not
  place. It is 0x42E9F0's third argument, the clearance radius and the
  least step between tries. Free roam, level 3: 50–70, 35–45 or 20–25 m.
* **Placement** (0x42F800 → 0x42E9F0; verified unless noted): for a
  random human racer (0x5EA4F0 on 0x8900BC), first 0x42E790
  (teleporters, below; in any mode) with probability 1.0, 0.35 when the
  racer's +0x42C is under 20 m/s (0x783BCC/D0 / D4) — it always fails in
  the retail data; then, unless the spacing is 99999, 0x42E620 in a drag
  race, else 0x42E9F0 when 0x4035A0 allows: traffic goes ahead of the
  racer at 24.5865 m/s (55 mph) or more (unless its +0x18 +0x248 flag),
  else ahead or behind on a coin flip; it walks the road from the racer's
  road node: the control point at the racer's controller +0x0C
  (0x5EC0C0) and its record (0x5EC0B0 → 0x40F690: the record of that
  point's id), no placement when either is missing or the record's id
  (+0x0A) differs; what keeps the human's +0x0C is **open**. **Observed**
  (2026-10-31, memory, career free roam's start, the player at (1695.3,
  −883.6)): the node is record id 515's point 6 at (1709, −876), flags
  0x19FC1A03, a record with lanes 0–1 only (no traffic lanes) — the start
  of the nearest segment of any record (13.137 m; record 692's segment 7
  is 13.154 m, its point 8 the nearest point at 15.6 m). The clone takes
  that rule (`road_node`, **inferred** from this one reading; until
  2026-10-31 it took the nearest point of a record with traffic lanes,
  record 285, 87 m away, from which nearly every try placed a car).
  "Ahead" is the walk byte itself (1: 0x401CC0 steps to the next index;
  0x42EB04 flips it in a coordinate race on a node record with +0x70
  bit 1): the record's own order, not the player's heading (**verified**:
  0x42E9F0 reads no heading; the clone took the player's heading before). The 250.0 is **verified** as argument 4 of the first
  road walk: 0x42EB67 loads it (0x7E9720) and pushes it, and 0x42EB7A
  calls the walk 0x415130 with count 1 as argument 2. In the head of
  0x415130 (0x415130 to 0x4151B5) it is stored at the walk's local
  [esp+0x18]; only when argument 5's low byte is zero is a record term
  (the word at +0x10 of argument 3, times 256, times 0x784278) added to
  it. The rest of the walk is not read, so calling 250 m the placer's
  first search radius is **inferred**. The start is the entry at
  (cursor + 1) mod divisor, the divisor being 0x417D80's result (not
  read), of a local list (cursor at [edi+0xC], 0x42EB34–0x42EB45; the
  list's build is not read), so this path has no distance comparison and
  does not confirm the nearest-control-point rule. Other readers of
  0x7E9720: the drag placer 0x42E620 (0x42E6A7, passed to 0x417C60 at
  0x42E6C1); the refusal 0x409890 (0x4098AC, returns 1 when
  |call 0x5F12B0 result| is under 250, after checking that the dword at
  the pointer in 0x890118 is 4; called at 0x4181CD beside the 0x409840
  call at 0x4181FA); the jam test 0x4112A0 (0x411334, |call 0x5D5CE0
  result| compared with 250; the label is inferred). The 2D refusal
  0x409840 reads 0x78429C (250.0), not 0x7E9720. The later walks at
  0x42ED06 take [esp+0x14] as argument 4. **In a coordinate race**
  (verified, asm 0x42EBBB..0x42EC19 and 0x42ED4C..0x42EDA9):
  each try's point is measured, for a point 0x5DCC90 measures by race
  coordinate, by the signed 0x5D5CE0(point +0x08, racer +0x8FC) (no
  FABS: a point behind never fails the 500 m test), else by its 2D
  distance to the racer; the gate is 0x418110 (race coordinate for such
  a point: 250 m of the player, 0x411060's one-sided spacing to traffic);
  and a car that is not parked (+0x77C clear) whose own +0x8FC lies ahead
  of the racer's is not placed ahead farther than that measure, nor one
  behind placed farther behind: the placement fails and 0x42F910 parks
  the car (`TrafficLane::spawn_race`, `reach_measure`). 0x42F910 draws
  the spacing once a pass before walking the slots; 0x42F800 draws which
  human (0x42F812) and the teleporter odds (0x42F852) for each car it
  tries (`placer_try_draws`, docs/rng.md). 0x417BB0 → 0x410DE0 → 0x409890
  keeps a car within 250 m of race coordinate alone, and 0x5F12B0 /
  0x5F0FC0 measure from the world's slots +0x80/+0xE0, which in a
  one-player race hold the player alone (world +0x14 = 1, slot 1 unset:
  **observed**). **Starts** (0x417D80, verified,
  <work dir>): the walk's start is not always the racer's own road. 0x42E9F0
  asks 0x417D80 for up to 3 starts {record, point, backward byte}: the
  first the racer's node; then, over the route manager's records
  (0x88D21C, count 0x88D218) other than the racer's, whose box (+0x60 /
  +0x68) holds the racer's point within max(+0x14, +0x16) + 30 m
  (0x43D410), the first point that runs parallel (|tangent · tangent| >
  0.85, 0x783BE8), carries flag bit 25 when its byte (the first's
  backward byte XOR the tangents opposing) is set, else bit 26, and lies
  in the racer point's frame within ±max(segment +0x10, 20 m (0x783BDC))
  along and 2–30 m (0x783BE0 / 0x783BE4) beyond the left half width
  +0x14 or the right +0x16 across; a failed point moves the scan on
  max(1, (int)(0.02 (0x7851C0) × its distance to the racer's point))
  points; one start per record. 0x42E9F0 then takes start (placer +0x0C
  + 1) mod count and stores it back, so successive tries rotate between
  the racer's road and the roads beside it (e.g. a divided road's other
  carriageway: in L4RA's free roam 2408 of 7403 traffic points have
  one). The walk's byte is its backward flag (0x415130 compares it with
  junction bit 18 and steps by 0x5D8C50 forward / 0x5D8C00 backward).
  Clone: `starts` / `TrafficLane::spawn_from` with `Traffic::start`
  (the records in file order: **inferred** that 0x88D21C holds them in
  that order). Then up to 100 tries
  (0x42EBAE; the cap exit 0x42ED35 bumps 0x827ACC). The 500 m test is
  **verified**: 0x42EC0E compares the point's distance with 500.0 at
  0x783BA8 (fcomp; refused when greater). The distance is the 2D
  sqrt(dx²+dy²) of the point from the point 0x401890 returns, or the
  float 0x5D5CE0 leaves in st(0) when 0x5DCC90 returns true (the "road
  distance" and "race coordinate" labels are inferred). Beyond 500 m,
  0x42ED29 returns 0, so the placement call ends with no car and no
  further tries. The car
  drives against the walk with odds 1 − p, p = float at the track record
  (0x883DAC = 0x5D3E50 of the race's track) +0x108 + 4·level
  (`Track::traffic_with`; ½ at every level for track 4000); 0x410CA0
  picks a lane on that side, else the other; the point needs flag bit 25
  (against) or 26 (with) and 0x418110 to pass: outside coordinate races
  it refuses a point that 0x409840 finds within 250 m of the players
  (0x5F0FC0; the clone: 2D distance to the player) or that 0x410FC0 finds
  within the spacing of another car (the world list 0x890080; parked
  traffic and hidden cars, 0x5EC070, skipped; 2D; a second radius for
  racers, +0x14 → +4 = 2, the same spacing here); in
  coordinate races (0x88F2BE) with the track's byte +0xF4 + 0x89E7A4
  clear it also refuses a point whose 0x5D5CE0 value (verified: 0x418110 makes this call) is under the track
  float +0xF8 + 4·0x89E7A4 (0x418110: the point's race coordinate less
  the start line's, race status +0xBF8 by +0xBF6 × 0x78425C × the road
  length 0x88F2B0; skipped while 0x402000 on the race status is under
  0x783BAC; not ported); then, for a point with a race coordinate
  (0x5DCC90), 0x409890 / 0x411060 measure race coordinate, else
  0x409840 / 0x410FC0 2D. 0x88F2BE (0x88F298 +0x26, set by 0x5E6A50 → 0x5E5BC0) is read as the
  race road's "built" byte (**verified**): 0x5E6A50 stores 0x5E5BC0's result
  there once 0x5D53A0 has returned a road record (on 0 it returns 0 and
  writes nothing), and 0x57A6F2 calls it only while 0x89E7B0 (a global with
  26 direct writes, 0x4BC5B1–0x60B630, one of them 0x601E1C in 0x601D30, which
  sets it from the type index through 0x5F0190 when [0x864FA8] is non-zero;
  its race-type meaning is **inferred**) is non-zero (else 0x5D9A70 is called).
  So a race is a coordinate race when its road builds (**inferred** that every
  race's build succeeds: 0x5D53A0 returns 0 when no 0x110-byte record matches
  its 16-bit key (**verified**), but the keys and 0x5E5BC0's failure returns
  are not traced). Clone: in a race the
  placer's distances are the larger of the 2D distance and the race-road
  difference of the two places (`RoadRoute::locate_once`, 0x5D5CE0;
  **inferred** for points off the race road). Between tries the walk
  moves on max(the point's segment, the spacing) metres (0x415130 at
  0x42ED06; a walk that fails or lands on the same point ends the
  placement — the doubling at 0x42ED13 never gets a second walk, since
  the loop's guard then holds the same point). The lane's side
  (**verified**, the calls at 0x42EC3A and 0x42EC51): 0x410CA0's flag is (walk byte = 0) ≠
  the draw, set meaning lanes 2–5, the other side when that finds none;
  the point needs bit 25 with the draw, else 26 (the clone had the side
  the other way round until 2026-10-31).
  **Placer search** (0x415130 with count 1 and argument 6 = 0, as both
  0x42E9F0 walks call it; **verified** from the decompile and the asm
  0x4151D4–0x415582; ported 2026-10-31 as `placer_walk`): not a car's
  drive but a search. The metres left start at the distance, plus the
  first point's segment for a backward walk, and each point visited takes
  off its own segment (+0x10); where they go below 0 that point is the
  result ({record 0x5D89A0, point, walk byte}). At a point's junctions of
  the walk's direction (bit 18 ≠ the walk byte; 0x5D4390 / 0x5D43D0, the
  indices below 0x883DB4): low nibble 3 sets the metres to −1 (this
  point is the result); on a point with flag bit 25 a high nibble 7 is
  passed by and a 2 followed (the target 0x5D86E0, the walk byte flipped
  by bit 23, the metres + the point's segment − the target's offset
  along the point's tangent, i16 +0x18 / +0x1A × 1/32767 at 0x81E7FC);
  every other junction (all of them on a point without bit 25) branches:
  the point after this one (0x401CC0, null at the record's end) goes to
  the front of a 20-entry ring of {point, metres, walk byte} (0x40C720;
  head +0xF0, tail +0xF4, full +0xF8, a full ring drops it) and the
  junction's target to the front or, with odds ½ (0x5EA510 ≥ 0.5;
  in a coordinate race when the junction's low nibble is not 0), the back
  (0x40C6B0), and the walk takes the ring's front (0x4132E0). A point
  stepped or turned onto must have, for the walk's starting direction,
  one of bits 11, 25, 27 (forward, 0x5D8C00) or 18, 26, 28 (backward,
  0x5D8C50), else that path ends and the ring's front is taken; popped
  entries are not tested. No result once the ring is empty, after more
  pops than 0x883DB4 (the junction count: 3245 in free roam), or after
  0x88D218 × 1024 steps. **Observed** (2026-10-31, `a private Wine copy`,
  career free roam, the player standing at the airport, 129 passes): 25–40
  % of the tries placed a car (19 of 48, 20 of 81), the cars at (1637–
  1657, −605 … −618), (1638–1657, −570), (1629–1652, −498 … −535) and
  (1503–1551, −1097 … −1139), 264–392 m off, at 20.0 m/s; 4.7–5.5 placed
  cars on average (1–12). The clone before (record 285's node, the drive's
  junction rules for the walk): 147 of 148 tries placed, 15 placed on
  average; after: 108 of 296 (36 %), 7.9 on average, at the same points
  (`traffic_probe`, which now runs a career profile).
  **Racing-line search** (0x415130 with argument 6 = 1, as both of the
  encounter placer's calls pass it: 0x42A69A / 0x42A73C push 1 before the
  walk byte, the metres, the point and the count 1; **verified** from the
  decompile and the asm; ported 2026-10-31 as `racer_search`, which
  0x42A5E0's port now uses in place of the drive's junction rules): the
  same search, but the pass-by / follow test reads the point's flag bit
  11 (+0x0C & 0x800) for bit 25 and the junction's low nibble for the high
  one (7 passed by, 2 followed, anything else a branch; 3 ends it as
  before). The 50 m steps (0x783D0C) start from the found point with its
  walk byte (ebx, the result's +8), the walked total + 50 each, while
  under 500 m (0x783D10). 0x42C950 places it
  at the lane's centre (0x5FBEE0 → 0x5DD370), moving at the lane's speed
  byte (Speed above); the clone holds it to the mover's 55 mph.
* **Traffic level** (0x89E7A8 = race parameters +0x08, verified): career
  free roam (0x540790, from 0x58191C in game mode 1 with no event)
  sets track 4000 and level 3; a career race takes its event's byte
  +0x36 (0x5322F0, signed; `Event::traffic`: 79 events at 0, 147 at 1,
  28 at 2 — drift races 2, circuits / sprints / Street X / drag mostly
  1); a Quick Race takes its mode record's level, 0x83AAC8 + 20 × type,
  for the type its options set (**verified**). The setter 0x4B2C00 is
  called with types 0, 1, 2, 3, 4, 8, 9 and 10 only (eight direct
  pushes; the calls are at 0x4B2BEC, 0x4B2C7C, 0x4B2C9C, 0x4B2CBC,
  0x4B2CE6, 0x4B2CFC, 0x4B2D2C, 0x4B2D4C); it stores the type into
  DAT_0083AAB4 at 0x4B2C15 unless the game mode (0x850078) is 8 or 0x200.
  Each push sits in one of eight handlers, each the vtable + 4 entry of one
  per-type menu class that the builder 0x4B2D60 stores into its object
  (vtables 0x790FD4, 0x790FDC, 0x790FE4, 0x790FEC, 0x791004, 0x79100C,
  0x790FF4, 0x790FFC for types 0, 1, 2, 3, 10, 4, 8, 9): 0x4B2BE0, 0x4B2C70,
  0x4B2C90, 0x4B2CB0, 0x4B2D20, 0x4B2D40, 0x4B2CD0, 0x4B2CF0. The builder's
  only caller is 0x4E2A50 (**verified**; it tests the game mode 0x850078
  against 8, 0x200 and 4; see the Outrun / Free Run bullet). The screen that 0x4E2A50 builds takes its name from its descriptor, so it is
  UI_QRModeSelect.fng (0x78F398) when the descriptor is record 0x7F8570
  (**inferred**, see below). The name flow is **verified**: 0x4E2A50 has no FNG
  file-name literal of its own: every helper that takes an FNG name is
  passed the name at object +0x4, copied from its descriptor argument
  (0x551DF0 reads the name from [0] and the hash from [2]). Its other
  literals are the messages Hide (0x78FC44) and Show (0x78FFD0), plus
  EA_MESSENGER_GROUP in the mode 8 path. 0x4E2A50's only direct caller is
  0x4EF280, and 0x4EF280 has no direct caller: its address is stored once
  in the binary, as the factory field (+4) of the descriptor record at
  0x7F8570, {name 0x78F398, factory 0x4EF280, hash 0xC6923ECD, flags
  0x405}. That this record is the descriptor the factory receives is
  **inferred**: the dispatcher that calls factories through the table is not
  traced, and no code in the disassembly references 0x7F85xx by address. On
  the path where the singleton at 0x836340 is still null, 0x4EF280 also
  pushes GarageMain.fng (0x78FCD8) and GarageMain_2P.fng (0x7906DC); those
  names are not in 0x4E2A50's body. The mode 8 and 0x200 paths overwrite the
  hash at +0x28 with
  0x36BEC45F and keep the name. 0x502FF0 (returns 0x78F398 at 0x50301A when
  its argument is 0 and [ECX+0x156A8] is 4) is not called here. Not traced:
  which mode selects that record. The other UI_QRModeSelect.fng reads, through
  the global 0x7F445C (static value 0x78F398) in 0x4E2900 and 0x4EF1B0, build the 0x18-byte singleton
  at 0x836340, not this screen.
  The race dispatch 0x53FC30 switches on that index with no case for 12
  (0, 1, 7 → 0x53F160; 2 → 0x53F6A0; 3 → 0x53F840; 4 → 0x53FA50; 5, 10 →
  0x53F380; 8 → 0x526840; 9 → 0x53F590; 11 → 0x526730). 0x526730,
  0x526840 and 0x53F590 read the level as [0x83AAC8 + 20 × type] when
  the index is not 12 (**verified**). The eight literal reads of the
  type-12 byte 0x83ABB8 (0x52679F, 0x526885, 0x53F1C9, 0x53F3E9,
  0x53F5E4, 0x53F712, 0x53F9E1, 0x53FAC2) are loads only; the three
  decompiled routines use them only as the index-12 fallback
  (**verified**). So the static 2 at 0x83ABB8 is not a single-player
  Quick Race's level: no single-player Quick Race sets the index to 12.
  The index's other direct store is 0x601D59, which stores the 8-bit value
  it reads through 0x581DA0, zero-extended to a dword (**verified**:
  0x581DA0 reads bits from the object at ECX, position +0x404; 0x601D30 reads
  the type from it as 8 bits, then other race fields, and is called from
  0x58BFF0, whose one resolved caller is 0x58EA36; whether that bit stream is
  the online session is **inferred**).
  The level's only writer found is 0x5029C0, called once,
  from the Mode Options handler 0x4B3AD0 (the call at 0x4B3B0A, which
  cycles 0–3; **verified**): it writes [0x83AAB4 + 0x104] for index 12
  and [0x83AAC8 + 20 × type] otherwise. The accessors 0x502930 (record
  +0), 0x502960 (record +1), 0x502910 (record +8, the track; 0x83ABD0
  for 12), 0x502750 (the level getter) and 0x5028D0 (zeroes 0x83ABC8,
  writes 0xFF at 0x83ABCC) are decompiled (**verified**); none of the
  stores within four instructions after each `mov ecx, 0x83AAB4` is
  through ecx, and the eax-based ones are fields of the 0x502910 pointer
  (record +8 on), not the level (+4). The save block supplies them
  (**verified**): 0x534780 and 0x5347D0 call the block loader 0x5344E0 with
  ECX = G + 0x10 (G = 0x83A9D0: passed to 0x534780 by 0x4A9931, to 0x5347D0
  by 0x4FBA7C and 0x4FD628) when [G + 0x156A8] is not 4 (0x534780's state-4
  branch reads a slot at G + 0x10 + [G + 0x20358] × 0xAB4C through 0x516850,
  not traced to 0x83AAB4). 0x4A9850 calls 0x534780 only when the header type
  is 0x10D and its size test passes. 0x5344E0 copies 95 dwords from 0x61
  dwords past the cursor C0 that 0x529240 returns (0x516850 and 0x529240 read
  the first sections from the block start, so C0 is not the block's dword
  0x61; the copy runs when C0 + 0xC0 dwords lies in the buffer) to 0x83AAB4 …
  0x83AC2F: the type index, the records and the index-12 level (0x83ABB8).
  So the static 2 there and the text bytes of types 3–11 are overwritten at a
  load. **New save** (the constructor's state is **verified**, see below; which new-save branch runs is **open**, see Path A and Path B): the static level bytes never reach a game run. G (0x83A9D0) is a
  static object. Its constructor 0x53ED30 is called once, from 0x77D390 with
  ECX = G (a push of 0x781FE0 and a call to 0x75C5ED follow; their purpose is
  not traced). It builds the two profile slots, 0xAB4C apart from G + 0x10,
  with the default builder 0x5344B0 (its only reference is a push at 0x53ED4D).
  With ECX = the slot base X, 0x5344B0 calls 0x5116E0 at X + 0x24, 0x52A230 at
  X + 0xD4 = 0x83AAB4 and 0x52A5E0 at X + 0x3B0. 0x52A230 (its only caller is
  0x5344C1) runs 12 times, for types 0 to 11, and sets at 0x83AAC4 + 20 × i:
  byte +0 = 1, byte +1 = 3, and the level byte at +4 (0x83AAC8 + 20 × i) = 1.
  It then calls 0x525CB0 with ECX = 0x83ABB4 (lea at 0x52A274), which writes
  0x83ABD0 … 0x83AC2F (the tail of the block; no level byte among them) and
  calls 0x515750 with ECX unchanged. 0x515750 sets the index-12 level byte
  0x83ABB8 = 1, so the static 2 is overwritten. Every level byte for types 0
  to 12 is therefore 1 on the constructor's path (path B below differs; the level getter 0x502750 and the Free Run dispatch
  0x53F590 read the low byte; 0x53F590 reads 0x83AB7C for type 9).
  Later code on this path writes outside the block. The constructor's remaining
  stores are at G + 0x156E8 and up, and its calls run on G + 0x156B0 (0x5030E0,
  stores at offsets 0 to 0xBB only), G + 0x1599C (0x5117C0) and G + 0x1677C
  (0x531D30, which constructs vectors from there up); 0x52A5E0 runs at
  X + 0x3B0 and builds its vectors from there up. Not traced: the callees of
  0x5117C0 (0x401650, 0x5F73E0), of 0x531D30 (0x503A00, 0x516B30, 0x525D00,
  0x5118E0) and the element constructors called through 0x75C461 by 0x52A5E0.
  The new-career path 0x532840 runs on the career object 0x85AD40: ECX is
  0x85AD40 at its four callers (mov ecx,0x85ad40 at 0x4F38EC, 0x5344F6,
  0x534752 and 0x60384A). Its own stores are at 0x85AD40 and above, and its
  callees write only above that object: 0x531810 at 0x85AD40 + 0x887C, 0x500700
  at 0x85AD40 + 0x72D8 + 0x104 × i (EBP set at 0x532973), 0x516480 at 0x83AD90
  (= X + 0x3B0), and 0x5760A0 stores nothing. The profile reset 0x515AA0 (called from 0x53473C on X and from 0x53474B on the second slot, ECX = G + 0xAB5C = X + 0xAB4C) writes the block through 0x515800 (Path B); its callees 0x479220 and 0x4FF9D0 are not traced, and 0x43DBD0 (called at 0x515AE9) is not traced. The
  slot select 0x512360 zeroes 0x83A9E0 … 0x83A9FF, then calls 0x43DBD0 with
  that address (the callee is not traced). The loader 0x5344E0 (called with
  ECX = X from 0x534822 and 0x5347BB) first calls 0x532840 with 1 on the career
  object (0x5344FE), then copies 95 dwords into 0x83AAB4 … 0x83AC2F when the
  buffer is long enough. The builder routines 0x52A230 and 0x525CB0 each have one direct caller (0x5344C1, 0x52A27A); 0x515750 has two references, the call at 0x525CD8 and the jump at 0x51595F (the tail of 0x515800, see Path B).
  **New-save branches** (**verified**): the new-save front-end handler 0x4F3780 (its only reference is the call at 0x4FB9B2 in 0x4FB8F0; the label MC_Main handler is not confirmed) takes its category 0x5000000 branch with the magic *(msg+0x10) == 0xB8A7C6CC: FUN_0052A470(1), then, if DAT_00836514 & 0x20000 is set, FUN_00532840(1) on 0x85AD40, then 0x4E8D30(3) as in path B (**path A**: whether the block is written was not checked for FUN_0052A470 or FUN_00532840, so the levels are taken to stay as they were: the constructor's 1s in a fresh session, the loaded values after a load); if it is clear, FUN_00534700 (**path B**, below). 0x4F3780 stores DAT_00836514 with its category bits cleared (mask 0xF0FFFFFF, at 0x4F37B4, after a call to 0x4A3AE0). Bit 0x20000 has no confirmed name (path A is taken when it is set). The code at 0x4AF0A6 pushes mode 0x21961, which includes 0x20000; the call at 0x4AF0AD passes it to 0x4A3100, which stores its third stack argument to DAT_00836514 at 0x4A3116 (**verified**). The routine start 0x4AF090 is not an instruction boundary in the listing, and the 'Start New Career' label is not confirmed.
  **Path B, the reset 0x534700** (stores **verified**): 0x534700 calls 0x515AA0 with ECX = G + 0x10 (slot 0, X; lea at 0x534739) and with ECX = G + 0xAB5C (slot 1; lea at 0x534745), then 0x532840(1) on the career object. 0x515AA0 keeps the slot in ESI (mov esi,ecx at 0x515AA7) and sets ECX = X + 0xD4, the block base B (0x83AAB4 for slot 0; lea at 0x515AF1), before its call to 0x515800 (0x515B10). 0x515800 (mov ebp,ecx at 0x515805) then: (a) writes 13 records from B + 0x10 at a stride of 20 (k = 0 to 12: byte +0 = 1, +1 = 3, +2 to +5 = 1, +6 = 0; loop 0x515810 to 0x51583E), which sets the level bytes of types 0 to 12 (B + 0x14 + 20k) to 1; (b) sets [B+0x50] = 2 (0x51585C: the level of type 3, 0x83AB04) and [B+0xDC] = 0 (0x515884: the level of type 10, 0x83AB90), and writes fields that are not levels; (c) after its loop, lea ecx,[ebp+0x100] (0x83ABB4 for slot 0, at 0x515954) and jmp 0x515750 (0x51595F). 0x515750 with that ECX stores 1 at ECX+4 (index 12, 0x83ABB8, the same value as (a)) and no other level byte (its other stores are at ECX+0 to +3 and +5, which are not level bytes, and at ECX+0x14 and up, above 0x83ABB8). So path B leaves each slot's levels for types 0 to 12 at 1, 1, 1, 2, 1, 1, 1, 1, 1, 1, 0, 1, 1 (**verified** from the stores). The outer loop runs for eax = 0 and 1 (exit test at 0x51594C). The branch at 0x515964 sets ESI, not EBP, and returns to 0x515904; every exit of the loop body reaches 0x51594C, so (c) is always reached (**verified**). The loop's other stores are dwords at B+4 and B+8. Not traced: its two virtual calls (0x5158E7 and 0x515924), which run after (a) and (b) on objects at 0x83AD90 and 0x8458DC. A level byte written by a callee would change the list above, so the list holds only if they write no level byte.
  **Callers of 0x534700** (call sites verified): 0x4F390A (in 0x4F3780: path B when bit 0x20000 is clear); 0x4C6385 (in FUN_004C6330, called only when FUN_0043DC30 returns 0; FUN_004C6330's only reference is the call at 0x4D9F37, in the routine the worker labels FUN_004D9DB0, not re-checked); and the jmp at 0x534855 from 0x534850, whose only reference is the call at 0x57EDC2 (inside 0x57ED20, which is called at 0x580E34).
  **Open** (what is missing): (1) which branch a new save made in a session that has not loaded one takes. Path A needs bit 0x20000 set at the category 0x5000000 Yes. The flow that sets that category is not traced (the state value 0x5000000 is pushed at 0x4E8E42 for a call to 0x4A3890 with 0xD36620FE, 0xF770 and 0x61C; 0x4A3890 is not decoded), and the flows through 0x4C6330 and 0x534850 (FUN_0043DC30's return value; the callers of 0x4D9DB0 and 0x57ED20) are not traced. So the fresh-session claim is **verified for path A only**; path B sets types 3 and 10 to 2 and 0. (2) A new save made after a load: path A keeps the loaded levels and path B resets them to the values above. Which applies depends on the same open flows, and whether a load sets or clears bit 0x20000 is not traced (the reader at 0x4A9A8F was not followed). The other single-player dispatch targets (0x53F160, 0x53F380, 0x53F6A0, 0x53F840, 0x53FA50) read index 12 (0x83ABB8), which is 1 after path B (**verified** from its stores) and after path A from the constructor's state; the dispatch targets' reads were not re-checked, and after a load path A keeps the loaded value. The clone's `QUICK_RACE_LEVEL` = 1 matches path A from the constructor's state (**verified**), but not path B (types 3 and 10), and it does not model the loaded levels (the clone-side claim that it has no per-type level was not checked here). No clone change: the branch is not settled.
  The race-type defaults 0x4FEF80 …
  (test runs) clear it and online setup (0x53FD40, which also sets
  0x89E7E4) leaves it 0. There is no other traffic density option. Every
  race mode with a non-zero level gets the pool (0x4FE9F0 from 0x53250A
  / 0x526966) and the placer (0x42F910 checks only the level, the list,
  game state 4 and human racers); a drag race places through 0x42E620
  (ported, see Drag placement). Clone: explore
  is career free roam, level 3 (Free Run `QUICK_RACE_LEVEL`); a race
  loads its own traffic (`Race::traffic`) from its routes file
  Routes<track><F|B>.bin, whose records carry the same traffic lanes
  as free roam's (L4RA's: 15–218 of 16–225 records), and its
  Paths<track>.bin zones (the same four), with track record +0xEC /
  +0x108 at the level and direction; the cars share the race's
  collision step with the racers, the avoidance sees the rivals as it
  sees the player, and a replay's snapshots hold the traffic's state.
* **Pool** (verified): 0x4FE9F0 (career free roam 0x540985, races
  0x53250A …; skipped while 0x89E7E4, set by the online setup 0x53FD40 and
  the test start 0x53FEC0) makes min(16, track record byte +0xEC +
  0x89E7A4 + 2·level) traffic cars (`Track::traffic`), none at level 0,
  each first on the streaming placeholder (0x40BDE0); track 4000 at level
  3: 16.
  StreamingTrafficCarManager (0x81E810, +0x900 list of 0x920-byte
  entries) holds one entry per car type with +0x844 = 2, in ascending
  record order (0x41AAF0; verified). A car's model is a zone rule with a
  random draw, not a cycle (verified; 0x412BE0 has one caller, the state-6
  branch of 0x57F2E0). Only car slots whose type (+0x10) has +0x844 = 2
  get a model, written to +0x908 (verified; where +0x10 is set is not
  traced). The zone is the record +0x3C that the 0x5D9C00 lookup (kind
  0x10) returns, 0 when none (verified; which point is passed, via
  0x430240, is not traced). A model's cap is its car type's signed byte
  +0x854 + zone (0 = never). If no cap is above 0, the loop is skipped and
  +0x908 keeps its prior value (verified; that initial value is not
  traced); the clone takes entry 0 for every car. While fewer than 7
  distinct models are in use, a car takes a random model among those still
  under cap (0x43C1C0: state % count, then one step of the generator at
  0x7EA624); after that, the first model in use still under cap, else the
  first in use, else the first entry (this last step is unreachable, from
  the code). Not track or level alone. The clone: `assign_models`
  (src/ai/traffic.rs), called from `Traffic::load` (src/bin/game.rs), with
  the caps of the traffic zone that `TrafficZone::at` finds for the player
  (the binary's point in free roam, 0x430240; see Model life cycle). The start state: the image holds
  0x12345678 at 0x7EA624 (verified), but 0x5817AF also writes the state
  (function not traced), so the state at the call is not verified. Where
  the caps run out before 7 models are in use and a car is left, 0x412BE0
  divides by zero (div at 0x43C1CA, by 0; from the code, not run); the
  clone takes entry 0 there. The player and traffic share one collision
  step (`step_cars`) as in races.
* **Intersections** (route chunk 0x34123, 0x114 bytes, 0x883DBC; verified
  unless noted). These are not lights: L4RA's free roam has 38, all kind
  1. An entry: +0x00 kind, +0x04 the two roads' record ids, +0x08 per
  road the nearest registered car's id, +0x0A per road its control point
  at the crossing, +0x10 the road holding the right of way (−1), +0x14
  sixteen 16-byte slots (car id − 0x32: used, road, distance, time;
  0x410750 "TrafficIntersection"/"IncomingTrafficCar"). A control point's
  flag bits 19–24 name its intersection (0x3F none; 0x5D8960, valid when
  both roads exist with record flag bit 6, 0x410BF0); the walk's first
  such point (walk +0xC4, 0x418BC7) is the one ahead.
  The line: 12 m before the crossing road's near edge — the other road's
  point moved out by its half widths (−left, +right) projected on the
  travel direction, the smaller (0x410800) — less the car's place on the
  walk (0x412170: the argument is the entry at controller +0x08 +
  0x30·(byte +0x308), passed by 0x417890 (call 0x4178FC, controller
  [esi+0x6e4]) and 0x425150 (call 0x425421, controller esi); it subtracts
  that entry's +0x10 (s) and +0x0C). The +0x0C is stored by the builder
  0x6148F0 at 0x614FB1 (record base edi−0x18, before the +0x30 step) as
  0.5·((1−T²)·D1 + T²·D2), D1 = out2.y − out1.y and D2 = out2.x − out1.x:
  the out vectors of 0x5AACE0 (call 0x614ECB, shape [[node+0x1C]+0x3C]+0x0C),
  T the float from 0x43C370. It is 0 when [node+0x1C] or [[node+0x1C]+0x3C]
  is null (zero path 0x614ED2 via 0x614EAF and 0x614EB6), and 0 for a type-4
  shape (0x5AACE0 writes the same vector to both out vectors, so D1 = D2 = 0,
  0x614F14–0x614F57). For other types it is undefined: 0x5AACE0 writes
  nothing and the builder does not zero the vectors on that path. Types 1
  and 3 go to 0x5A1A00 and 0x592920, not traced. **Open**: the shape type at
  +0x10 (types 1 and 3), and whether [node+0x1C] is the walking car's own
  record with the walk's entry array built by this builder (not traced).
  Each think the car leaves every slot (0x413800, the road's nearest found
  again, 0x408FB0) and registers at the intersection ahead (0x417890 →
  0x410B50): distance d, time max(d / speed, 0) (0 when d ≤ 0, 99999
  standing). Each placer frame (0x4111C0 → 0x409110) a road keeps the
  right of way only while one of its cars is within 12 m.
  Obey traffic intersections (0x425150): +0x1F0 = the cap (slot +0x7C);
  a car ahead within 30 m (0x402ED0, not a car crossing on the other road
  at the same intersection, 0x4140E0 → 0x410A50) gives 0 within 15 m, else
  at most 6 m/s. 0x402ED0 (verified as far as noted): over the
  controller's 0x30-byte road entries from the car's own (+0x308) back
  to the first, those at or beyond its s (+0x10) whose lateral off the
  car's lane (0x60BFB0) is within their +0x08 + 0.5 + the car's +0x08 of
  the car's; the first such gives gap = its s − its +0x0C − the car's s
  − the car's +0x0C (≥ 0), time = gap / |+0x04| when +0x04 < −0.01, also
  0x402CC0's distance and time; found with time 0 when either distance
  is under the look (or under 20 m with the car under 0.1 m/s), else
  only with a time under 99999. The entries (planner = controller +0x54:
  +0x08, 0x30-byte stride, the car's own at +0x308) hold +0x04 the
  relative velocity along the road, +0x08 / +0x0C the half extents
  across / along, +0x10 s and +0x2C the node id (through 0x89E970); see
  Road entries (round 4: traced, and ported as `TrafficLane::entries` /
  `car_ahead`). Then 0x4140E0 on the object found (a dropped one is not
  passed over), and the action slows when the time is under 1.5 s or the
  distance under 30 m: within 15 m 0, else at most 6 m/s.
  Before the car has stopped (+0x214), kind 3 and kind 2's road 1 must
  stop; otherwise the car's road may go (0x4109A0: kinds 1–2; the road
  holding it may, the other not; free, road 0 may and road 1 when road
  0's nearest car is 3 s or more away) and takes the right of way unless
  the other road has it (0x410A10). Going: +0x1F0 = min(cap, +0x1F0).
  Waiting: within 30 m of the line at most 6 m/s; past it (d < 0) 0, and
  stopped below 0.5 m/s. Then the path (0x420D40), +0x1F4 = 0, +0x1F8 = 1,
  +0x1FC = 0, +0x1FD = 1, +0x1FE = 0. The 0x5ECF30 colours are debug.
  Clone: all of this (`Crossings`, `TrafficLane::think`).
* **Stopping behind** (Avoid Avoidable, a racers' action; partly traced):
  its +0x218 is not an intersection but an object id: 0x60C130 finds it
  among the controller's 0x30-byte entries of things on the road ahead
  (+0x08, count +0x04; matched through the object table 0x89E970 by entry
  +0x2C; entry +0x04 the relative velocity along the road (see Road entries), +0x10 its s, 0x60C000 the gap to
  the car's entry +0x308). 0x42F330 compares a projected travel ((a·t/2 +
  v)·t + gap, t = +0x21C) with 3 m: within it +0x1F0 = 2 × the cap,
  +0x1F4 = +0x1F8 = 0, 0x40E390 and car +0x97C cleared; beyond it +0x1F0
  = max(20 m/s, speed − ¼·min(entry +0x04, 0)), +0x1F4 = t, +0x1F8 = 2
  (skipped in some race modes). Traffic stops behind cars only through
  the intersection action above and its avoidance (type 2, Actions).
* **Mode names**: the strings at 0x7E8EAC (Mellow, Physics, Cop,
  Traffic, Traffic in mellow, Out of control, J-Stop mode) are seven to
  0x5F46B0's six modes and nothing in the executable holds the table's
  address or any entry's, so they map onto nothing the code uses.
* **Room pass** (0x41FD50, verified from the decompile and the asm at 41FD50–41FFD0): ECX is the route object (node count +0x04, node pointers from +0x08). E is the entry stack pointer (the return address); the stack word E+4 is the argument, then the loop counter, then the flag byte. The nearest node is i = 0x4198C0(the point from 0x401890, 0, 0, 0, 0) (its search body is not traced). x (E−0x1C) = 0x419E10(route, 0, i, −1.0): with a negative band argument 0x419E10 skips its search and returns −(u × v), the (x, z) cross of the unit vectors u = P_i − P_mid and v = P_mid − P_last, where P_k is the track point at node k's +0x54 (0x4122C0), mid = i + (count − i)/2 and last = count − 1; it returns 0 when count − i < 2. An empty route exits at 41FD5B before x is set. For i = −1 on a non-empty route, 0x419B20 runs, then 0x43BD50 (int3 then ret; Ghidra marks it no-return), so the x = 0.0 store at 41FDC6 is reached only by falling through 0x43BD50. The byte at E+4 is 1 iff x ≤ −0.12 (0x7854E0), and bl is 1 iff x ≥ +0.12 (0x785368). The loop runs over the node pairs from the last back (A = node k, B = node k−1): S (E−0x28) = 99999.0 (0x784360) × (A +0x54 − B +0x54), and a flagged side takes 3.0 (0x783CA8) × S, into E−0x10 for the byte and into the working value for bl, in its bound. The first clamp (41FECE–41FEEA) tests against 1.0 (0x784250) and 99999.0 (0x784360); the second (41FEF8–41FF16) against −1.0 (0x7842A0) and 99999.0; the rest of the bound arithmetic is not traced.
* **Road entries** (verified unless noted; the builder is 0x6148F0, entered by the call at 0x428C71 in 0x428720 (its only resolved caller), whose tail 0x614FC1–0x615030 copies and sorts; the query is 0x402ED0; round 4, 2026-10-31, traced in full except where marked): the builder runs on the **planner at controller +0x54** (the think passes ECX = controller + 0x54 to 0x428720 at 0x429E65; [planner] = the car, +0x04 the entry count, +0x08 + 0x30k the entries, +0x308 the car's own index, +0x30C the lane, +0x31C the walk, its count +0x320). The think's end **0x41BF80** sets the count to 0 and the own index to 0xFF (through controller +0x6E4 = the planner), so the entries live only during a think: a memory read sees count 0 and the last think's stale entries. It walks the node list from 0x8A2CA4 to the sentinel (0x502FD0 returns its ECX, 0x8A2CA4; each node: +0x00 next, +0x08 id, +0x0C → position, +0x10 → velocity, +0x18 → forward, +0x1C the car; **observed**: the player, the racers and the traffic, one node each, table 0x89E970[id] = the node) and keeps a node whose car is null, or 0x5EC070 is false and the car's byte +0x550 is set, or the car is its own; at most 15 entries, 16 once its own is taken. A node is kept when its distance from the car is at most **0x404910**'s reach — max(125 m (0x7E8E60), controller slot +0x60 × the car's speed; the traffic controller's slot +0x60 is 0x408C70 = 4 s, 0x783B80) — or 0x403A40 holds (a stopping test on 0x4038D0, **not ported**), and |0x5D5CE0(node car +0x8FC or 0, car +0x8FC)| ≤ 3 × the reach. Each entry (0x30 bytes): **+0x00, +0x04** the node's velocity less the car's (+0x70), both turned by 0x5BAE70 through 0x4000 − the angle of the walk's current point's direction (0x40ADD0 → 0x43E6D0): +0x00 the part to the right, +0x04 the part along the road (negative while closing; 0x402ED0 reads it as such); **+0x08, +0x0C** the node car's half extents across and along the car's heading: cos²Δ·half y + sin²Δ·half x and sin²Δ·half y + cos²Δ·half x (0x43C370 is Cos16: it adds 0x4000 and jumps to Sin16 0x43C2B0), Δ = the node's forward angle less the car's +0x40C, the halves from 0x5AACE0 on the car's collision shape (type 1, the hull: the bounds of its vertices, 0x5A1A00; type 3, a box: centre +0x30 ± half +0x20, 0x592920; type 4 writes its centre to both and gives 0); **observed** under Wine (stale entries): 0.91 / 2.30 and 0.92 / 2.02 for cars in line, 2.04 / 1.18 for one across; **+0x10** s and **+0x14, +0x18…+0x24** the laterals off lanes 0 and 2–5 from **0x419B20**: the walk point nearest the position (**0x4198C0**: least 2D distance to the point's place on the lane (0x4122C0) + 10 × |point z − z|), s = that point's +0x54 − +0x58 + the position along it (vt+0x2C; +0x58 is **not traced**, the clone takes 0), the lateral its vt+0x34 less the lane's offset (**0x4123C0**, 0x401C40) less the walk point's own offset for that lane (+0x04 / +0x08 + 4·(lane − 2): 0x41A180's Hermite offsets on 0x418DA0's 8 m points; the traffic-mover parity, docs/world-systems-parity.md §8); +0x28 0x419C10's result; +0x2C the node id. The list is sorted in place (bubble sort, descending) by Δs² + Δlateral² signed by Δs, so entries ahead come first and the car's own index (+0x308: the entry whose node's car is the car) counts them. **0x402ED0** (lane, lateral, look): over the entries below its own index, nearest first, the first with own +0x10 ≤ entry +0x10 and |0x60BFB0(lane) − lateral| ≤ entry +0x08 + 0.5 + own +0x08 whose time is under 99999 or 0 (one in the window with neither is passed over); gap = max(0, entry +0x10 − entry +0x0C − own +0x10 − own +0x0C); time = the lesser of gap / |+0x04| (when +0x04 < −0.01, else 99999) and **0x402CC0**'s; 0 when the gap or 0x402CC0's distance is within the look, or when the car is under 0.1 m/s and either is within 20 m; it hands back the lesser of the two distances. **0x402CC0** (node, car): the node's position in the car's travel frame (**0x591FD0** on **0x591DF0**: the velocity's direction while |v|² > 1, reversed when car +0x3EC < 0, else the heading +0x3CC; x to the right, y ahead): with y ≥ 0 and |x| ≤ cos²Δ·node half y + sin²Δ·node half x + the car's half width, its distance is max(0, y − node half x − the car's half length), else 99999; the car's box is its body's +0xE0 / +0xF0 (0x5EC260; the clone takes its hull's, **inferred**). Over 0.1 m/s, with the relative velocity's part along the car's velocity pointing back and over 0.001 m/s, its time is that distance over it. Clone (round 4): `TrafficLane::entries` (the player, the racers and the placed traffic within the reach — only traffic cars with +0x550 set, `TrafficCar::active`, see docs/race-ai-parity.md "Belt cars in "Avoid Cars"" —, each with its hull's half box, `Other::half_box`; each placed at its nearest walk point), `car_ahead`, `box_ahead`; not ported: 0x403A40, the 15-entry cap and the race-coordinate test (**inferred**: the clone's pools stay under the cap; free roam reads +0x8FC as 0). Until round 4 the clone took the cars ahead along its first point within 1.5 m to either side, centre to centre.
* **Racer +0x8FC** (corrected): the disassembly has 56 accesses to displacement 0x8FC. The only store with a real object base is 0x60CC63 (`mov [ecx+0x8FC],edx`, after the `cmp` at 0x60CC5B). FUN_0060C950 (called from 0x620AF0 at 0x621089; this is the controller, [this] is the car) copies the controller's +0x314, or +0x318 when the car's byte +0x944 is set or DAT_008A1BF4 is non-zero, into the controller's +0x8FC, and sets its stamp +0x900 to −1 on a change. 0x72AEC9 and 0x72E01D are not car fields: each is a float store down a pointer walk with a 0x900-byte row stride (verified), and only shares the displacement, so whether the walk can reach a racer's +0x8FC is **Open**. They look like a decoder's buffer writes (**Inferred**: the streamer 0x72AD58 is called from 0x72802D, 0x7282EF and 0x7285BD, and the butterfly 0x72DFD0 from 0x72E0BA, by REA's complete caller lists; the decoder identity and these routines' bodies were not traced). No store to the car's own +0x8FC is found, so its free-roam source is **open**. Stores with displacement +0x314 / +0x318 (base not traced) are at 0x41C2D9–0x41C351, 0x420629–0x42062F, 0x42B898–0x42B89E, 0x42D408–0x42D40E and 0x42DA08.
* **Lane request** (0x40F4A0, traced; **not ported**: the flag and the record below are not traced, so no port is made): its fourth argument picks lanes 2–5 (zero) or 6–9 (else); it scans those whose record byte +0x82 + i is set, skipping any inside the fifth–sixth arguments' range (each +4 unless 0xFF; none skipped when either is 0xFF), and returns the one whose lateral offset (0x401C40) is nearest the point's reference, −4 when that argument is 1, −1 when none or when the car argument is null. For the 6–9 flag the side is the cross of (point − record) with the vector from 0x401AA0 (decompiled), not with the record's shorts +0x18 and +0x1A as on the 2–5 side. Its caller 0x424CB0 (the call at 0x424F48; the asm 424E7F–424F5A confirms the call and its conditions; the decompile was not re-checked) asks only when a car is found (0x402ED0 returns a nonzero time) and the car's +0x430 is under 0.5 (its standing branch), and passes ebx (the car), ebx +0x60, the result of 0x401ED0 on [esp+0x2C] (the 0x38-byte record at 0x88A334[u16 +0x5C] + 0x8C + 0x38·(u16 +0x5E), 0 when either index is 0xFFFF (formula not re-checked); the car's current step is **Inferred** for it; 0x401ED0 also gets the same flag and 0xFF, 0xFF (asm 424F32–424F3D), and its use of the flag is **not traced**), 1 when the byte at +0x10 of the car's +0x2C object ([esp+0x24]) is 0 (else 0), 0xFF and 0xFF. The byte is **not traced**, so the flag is **Inferred** and nothing is ported from it. A result other than −1 goes to 0x405410 on [esp+0x10] (&param_1[0xc3], the controller's +0x30C lane record; the setter's fields are verified from its asm, 405410–405450: [ecx] = lane, [ecx+4] = 0, [ecx+8] = the float at 0x7837D8 (0.151, loaded by fld, so the value is stored, not the address); its [ecx+0xC] is always written from the global 0x865188, since |0.151 − 999.0| > 0.001 (0x7E8E68, 0x78435C), and the global's value is **not traced**, not ported).
* **Car field** (clone fix, 2026-10-28): the game keeps a collision pair
  per pair of bodies (0x5A0350); the clone's `CarField` keyed its pairs
  by index into the frame's car slice, and in free roam and races that
  slice drops and adds traffic cars as they are parked and placed, so a
  pair's contact record and distance cache moved to other cars. With the
  hit test on live records (Hit above) that switched cars to the MeloMvr
  hundreds of metres from anything. `CarField::bind` now names the slice's
  cars (player, racers, traffic slot) each frame and carries the pairs to
  their cars' new indices.
* **Probe** (`traffic_probe` in src/bin/game.rs, ignored; `cargo test
  --release --bin game traffic_probe -- --ignored --nocapture`, with
  `PROBE_DRIVE=1 PROBE_BACK=1` a crude pursuit drives the player along
  record 285 at ~18 m/s, `PROBE_VERBOSE=1` logs events): per car
  spawns / despawns with model and distance, hits, wall and car contacts,
  long stops, junction turns, lane changes, and the lane offset, heading
  error and speed against the lane's. 2026-10-28, 120 s: standing, 116
  spawns / 100 despawns (every placement is ≥ 250 m out and so at once
  eligible again, as 0x417BB0 / 0x418110 have it), lane offset median
  0.21 m, p99 0.47, heading error median 0.8°, speed / lane speed 1.02, no
  hits; driving, before the fixes 3 hits of which one with nothing within
  40 m (stale pair record), after 2 hits, both the player's car. A
  player crash leaves a stopped traffic car near, and the jam rule
  (0x4112A0) then stops all placement and parks each car the pass picks
  (as native). Pool at the free-roam start: TAXI 5, TAXI02 3,
  4DR_SEDAN02 3, PARCELVAN 2, PICKUP, MINIVAN, SUV — the 0x412BE0 rule
  (after 7 models the first in use under its cap takes the rest).
  2026-10-31 (traffic audit): the probe runs a career profile, as the
  Wine measurement did. Without one (`profile` None, so not career) the
  player's own barrier list lacks `BARRIERS_CAREER<stage>` (0x5F54B0 adds
  it only in game mode 1) while 0x578070 switched it on for everyone, and
  the probe's traffic ran into the airport's stage-0 wall-off at y ≈ −921
  (group 120) and piled up there, which the original's career traffic
  drives through (memory: mode-2 cars at 20 m/s from −884 to −955). That
  follows the trace for a non-career free roam too (Quick Race Free Run:
  nobody's list switches the group off), not checked under Wine.
  **Observed under Wine** besides the above (same run): mode-2 cars hold
  20.1 m/s on 20 m/s lanes (1,061 of 1,307 mode-2 samples, the rest 20.0
  just placed or slowing). **Resolved 2026-10-31** (traffic round 2): the
  20.1 is the placement's, not the controller's. 0x5F52D0 (from 0x5FBEE0)
  sets car +0x42C to the lane speed but the velocity to the heading ×
  (lane speed + 0.1) (0x43CF90 on +0x90, then 0x592170, which also sets
  +0x430 = |v|, asm 0x592170–0x5921AE), and 0x42C950's 0x5F46B0(2) builds
  the TrafficMover (0x408740) with speed = +0x430. The speed control
  (0x41D8D0, mode 3 tables PA/IA/DA at 0x7E8FCC / 0x7E8FE8 / 0x7E9004,
  pedal rate table 0x7E9D10: 10 entries over 0–160, all as ported) then
  holds it: v < target + 2 keeps the pedal in 0…cap and the error is
  negative, so gas and brake stay 0. Memory, 2026-10-31 (career free roam,
  ~20 frames a second, 846 frames): every car placed in that run started
  at 20.1000 (or 20.0000, two placements; round 3, **observed** over 30
  placements in a race: every car placed from mode 4 — the world under its
  old place not loaded — started at the lane speed exactly (20.0000, or
  less where its first frame braked), every one from modes 0, 2 or 3 at
  lane + 0.1. 0x5F52D0 switches the car to mode 0, sets the velocity to
  lane + 0.1 and switches back to the old mode (0x5F46B0) before 0x42C950's
  mode 2; mode 4's mover (0x5A9040, which reads +0x430) is the
  **inferred** source, its destructor not traced. The clone's world is
  always loaded and has no mode 4, so nothing to port) with gas and brake 0 from its first mode-2 frame, and every
  traffic car's wheel flags (car +0x390…+0x393) read 1 in every sample,
  parked ones too: the TrafficMover sets them each frame (0x408BAC:
  0x592050(i, 1) with ECX = car + 0x40). The clone started the car with
  its wheels up (a new `CarState`), so its first driver frame took
  0x421030's no-driven-wheel-down branch (gas = the throttle cap, 1) and
  the car settled 0.4 m/s high at 60 Hz (0.8 at 20 Hz). `place_traffic`
  now sets the wheels down and the speed to the lane's + 0.1; the probe
  (`PROBE_HZ` sets its frame rate) holds 20.100 at 60 and 20 Hz. No
  change to the speed controller (src/ai/driver.rs).
  **Pool, a second load** (2026-10-31, career free roam): COUPE 2,
  4DR_SEDAN02 5, PARCELVAN, 4DR_SEDAN, HATCHBACK, TAXI 5, TAXI02 (car
  type +0x0C → name): seven models drawn, then TAXI filled to its cap 5
  and 4DR_SEDAN02 to 5 — the first in use under its cap in the car-type
  record order (GLOBALB 0x34600: TAXI 31, 4DR_SEDAN02 32, AMBULANCE 33,
  TAXI02 34, PANELVAN 35, COUPE 36, …; caps +0x854: 5 5 5 5 2 for the
  cars, PANELVAN 3 3 3 5 1, PARCELVAN 3 3 3 3 1, 0 for AMBULANCE,
  FIRETRUCK, BUS), as 0x412BE0 and `assign_models` have it. Two loads
  are too few for the draw's distribution (random, 0x43C1C0); both are
  consistent with the rule. The pool (one load): 4DR_SEDAN02 5, SUV 3,
  PANELVAN 3, PARCELVAN 2, HATCHBACK, MINIVAN, PICKUP — a random draw
  (0x412BE0), so not compared car for car.
* **Race traffic under Wine** (2026-10-31, traffic round 2; Quick Race
  Circuit on its first track, 4014 forward, 3 opponents, the player
  standing on the grid; the mode record's level byte 0x83AAC8 written in
  memory at the Game Options (the screen still read Minimum, the race
  took the byte: 0x526730); ~20 frames a second, the traffic list
  0x890080 → +0x2C… sampled each game frame; `race_traffic_probe` in
  src/bin/game.rs is the clone's twin, 1/20 s frames). Pool 12 at every
  level (clone 12). Level 1, 75–275 s after the load: 2 placed, no new
  placement — one car stopped in mode 2 at (−245, 301) (z 0.17, target
  0) and one in mode 3 beside the player at 16 m (a hit), so the jam
  rule (0x4112A0) held the placer. Level 3, from ~20 s after the load:
  11–12 placed for 20 s (mostly in mode 3, the MeloMvr, at 6–14 m/s,
  150–310 m off), then a queue: 7–8 cars at 144–233 m, slowing to 6 m/s
  and from ~60 s all at 0 behind a stopped mode-2 car at 233 m, no
  placement after (88 s + 75 s more: 7 placed, still). Level 2: 5–10
  placed, 28 placements in the first 31 game seconds, median 174–298 m.
  The clone (level 1/2/3, 120 s): 11 placed at the green, then mean
  5.8 / 6.8 / 8.0, 31 / 46 / 37 placements, median 352 / 312 / 191 m;
  at level 3 it forms the same queue at 140–200 m on the MeloMvr, but its
  cars there run 8–20 m/s and only now and then stop, so it keeps
  placing. **Round 3** (2026-10-31, `a private Wine copy`, two level-3 runs and
  the level-1 run above re-read): the permanent stop is +0x710 (Stopped
  for good): from ~37 s a mode-2 car at (−247, 309), 230 m from the grid,
  stops at the dead end (no contact records, target 0, action 7) — the
  same place as level 1's stopped car (−245, 301) — so the jam test holds
  the placer, each pass parks the far car it tries (11 → 10 → 6 → 5
  placed by 86 s), and the cars behind the dead end queue on the MeloMvr
  ("Avoid Cars", action 2) at +0x1F0 = 6.0 (the avoidance's slow rule:
  the tracked car beyond 15 m) and then 0 within 15 m: 5 placed at 191–230
  m, all standing, in both runs. The 6.0 is the avoidance's 0x40C00000,
  not the lane sentinel. The clone (now, 1/20 s frames, 200 s): level 3 —
  the dead-end car stops at (−244, 300) at ~45 s, 7 placed at 198–224 m,
  all standing from ~75 s, no placement after (before: 8–20 m/s, placing
  on); level 1 — 2 placed, both stopped at the dead end from ~45 s (the
  original: 2 placed, one stopped there, one hit beside the player);
  level 2 — 3 placed, all standing at 214–222 m from ~65 s. Its queue's
  cars ran 8–14 m/s at their lane speeds before the stop, not at 6
  (**resolved** in round 4, below). **Round 4** (2026-10-31,
  `a private Wine copy`, wined3d, two more level-3 circuit runs with the
  racers and the tracked node's car logged, `probe5.py`): **at the
  green 12 placed** in both runs (12 of 12; 188–834 m, most in mode 4,
  the world under them not loaded); the clone now places 12 too (11
  before: the fill's car at a walk end was parked, see Stopped for
  good). The queue: from ~25 s the cars behind the dead end hold +0x1F0
  = 6 (5.6–6.7 m/s) at 141–199 m tracking traffic cars 29–124 m ahead
  (times 5.6 s to 58,131 s), since 0x424CB0 computes the tracked gap only
  for a human's car (see Actions); one run then 4 standing at 188–228 m
  from ~70 s, the other 8 placed at 40 s. The clone now (1/20 s frames):
  12 at the green, the queue at 6.0 m/s (targets 6) from ~30 s at
  140–200 m, all standing from ~100 s at 167–230 m, 7 placed. **Sprint, level 1** (Wine, Quick
  Race sprint, first track, the player standing on the grid at (550,
  −742), one run): 9 placed at 10 s (196–2773 m, two stopped for good
  in mode 4 far off), 4 at 20 s, then 2 from ~30 s — both on the MeloMvr
  avoiding the standing player (action 2) and standing at 10 and 22 m
  (target 0, not +0x710), which holds the placer through the jam test for
  the rest of the run (130 s). The clone (track 4123): 12 placed at the
  green, 4 at 20 s, then 1 from ~28 s — a car stopped for good on a hard
  push (records' +0x60 48) 235 m off; no placement after. Both end with
  the placer held from ~30 s; the original's last cars reach the player,
  the clone's do not (**open**: one run each). **Sprint, levels 2 and 3** (round 4,
  Wine, one run each, the player standing on the grid): level 3 — 12
  placed at the green (559–2461 m, all mode 4), 8 at 10 s, 4 at 20 s,
  then 2 from ~30 s (one stopped for good 38 m off, one standing behind
  the player at 13 m, action 2, target 0), the placer held to the end
  (130 s); level 2 (the log joined ~85 s after the load) — 4 placed, two
  standing behind the player at 15 and 29 m (action 2) and two stopped
  for good at 45 and 188 m, no placement in 90 s. The clone (track
  4123): level 3 — 12 at the green, 8 at 20 s, 1 from ~30 s (stopped for
  good at 230 m), held; level 2 — 12, 4 at 20 s, 2 from ~70 s (both
  stopped for good, 32 and 50 m), held. Both hold the placer from
  30–70 s; the original's last cars stand behind the player more often
  (**open**: one run each). **Drag**: round 2's measurement stands (fill,
  frozen re-placements at 601 m); the round-4 attempt took Quick Race
  type 10 (Street X: no traffic cars at level 2 in the world list) and
  was not repeated. **Time of day**: no traffic code read here takes the
  clock (**open**: not searched for).
* **Free Run and other zones** (round 4, 2026-10-31; **observed**).
  **Quick Race Free Run** (type 9, its level byte 0x83AB7C = 1, so
  0x89E7A8 = 1; pool 10; the start at the airport, (1695, −884); Wine,
  wined3d, `fr1.txt`): 10 placed at the load, all 1.2–4.8 km off in mode
  4; 3 near by 15 s (109–393 m); by 30 s two cars **stopped for good at
  the stage-0 airport wall** (group 120) at (1594, −920) and (1605, −927)
  and one standing behind them (action 2, target 0) — the jam test then
  holds the placer for the rest of the run (90 s). So in Free Run the
  wall stands for the traffic, as the trace says (nobody's barrier list
  switches it off). The clone (`traffic_probe` with `PROBE_FREE_RUN=1`):
  pool 10, two cars stopped for good at the wall at (1609, −928) and
  (1594, −920) by 18 s, mean 2.3 placed. **Career free roam by zone**
  (level 3, pool 16, the player standing; fast GPU session
  `a private Wine copy`, teleported by writing the car's position copies;
  zones from PathsFreeRoam's kind-0x10 triggers, spacing rows +0x30 +
  4·(level − 1)): at the airport (zone caps 1, row 8) 6–11 placed over
  60 s, then 1 from ~75 s (a car stopped for good 113 m off holds the
  placer); in zone caps 0 (row 7, at (−437, 1197)) 9–12 by 15–30 s, then
  six cars standing behind the player at 16–51 m (action 2), held; in
  zone caps 2 (row 6, at (−2126, 2338)) 12–16 placed throughout 4 min.
  The clone (`PROBE_AT`, 90 s): airport mean 7.9 (max 13), caps 0 mean
  6.4, caps 2 mean 12.1 — the same order. **Open**: zone caps 3 (the
  teleport to (20.7, −1650.8) failed under Wine; the clone places no
  car there in 60 s, not checked); time of day (no traffic code was
  found reading the clock; not searched for).
* **Still open**: the player's road node rule (nearest segment start, inferred from one reading; see Placement); the mode-4 mover's part in the 20.0000 m/s placements (see Probe); 0x403A40's second chance for far objects, the 15-entry cap, the race-coordinate test and walk point +0x58 in the road entries, and the car's own box in 0x402CC0 (body +0xE0 / +0xF0, taken as the hull's) (see Road entries); walk +0xC8 in 0x40ABB0 (see Stopped for good); the free-roam source of the car's race coordinate (see Racer +0x8FC; it reads 0 in free roam); the controller +0x0C of a human racer (the placer's road node, 0x5EC0C0; its writers are not traced); the MeloMvr's rest test (0x5ABB2E → 0x5A5E30 with its wheels; the drive model's is ported) and modes 4–5's movers; the body of 0x5EC070; the byte at +0x10 of the car's +0x2C object (the lane request's flag); the lane request's port (its flag and record are not traced); the global at 0x865188 that the lane setter's +0x0C stamp takes (its value not traced); zone caps 3 and time of day (see Free Run and other zones); a round-4 drag measurement; and the drag placement's port, whose inputs are 0x417C60's road-table search (0x88D218/0x88D21C, 0x5D4490, 0x5D8CA0, 0x409520) and 0x418110's first test (0x5D3E50, 0x402000, and their globals).
* **Unplaced pool cars** (2026-11-05; **verified**, **observed** under Wine): 0x6065C0 links a traffic entry's car into the world list only when 0x42F7A0 returns it (0x6066FF..0x606713), and the placer 0x42F910 walks the world list's traffic (+0x18 .. +0x10) and returns while there is none (0x42F95B), so a pool car the set-up's fill cannot place never appears in the race. The drag fill needs race-record points with flag bit 25 or 26 (0x417C60); tracks 4211, 4212, 4213, 4221, 4222 and 4223 (either way) have none, so their drags run without traffic: S4_H_DRAG_15 (level 1) made its 12 pool cars (car table +4 = 3) and its world list held the 4 racers all race. Clone: `traffic_fill` drops the cars it could not place (docs/drag-stage4-parity.md §6.1).
* **Drag placement** (0x42E620, **verified** 2026-10-30 in the code and disassembly; ported, `TrafficLane::spawn_drag`, `drag_point`, `reach_point`, `drag_lane` in src/ai/traffic.rs): ECX/EBP is the placer, arg 1 the traffic car, arg 2 the human racer's car, arg 3 the spacing. It returns 0 unless the race road is built (0x88F2BE); the 0x5D5CE0 call on the two cars' race coordinates is unused. The flag is a draw 0x5EA510(1.0) under 1 − track float +0x108 + 4·level (0x883DAC, 0x89E7A8). **0x417C60**(car +0x60, car +0x8FC, 250 (0x7E9720), flag): over the race records 0x88D21C (count 0x88D218), each with flag' = flag, flipped in a coordinate race when the record's +0x70 has bit 1; the record must have a lane on that side (0x5D4490, ECX = record: any byte of +0x84..+0x87 for flag' set, +0x88..+0x8B clear, i.e. lanes 2–5 / 6–9); **0x5D8CA0**(pos, coord, 250, 0) (ECX = record) gives its point i (records +0x8C + 0x38·i, count +0x34) with the least measure ≥ 0 that is ahead (wrap(point +0x08 − coord) > 0, the lap wrap 0x88F2B0/0x88F2B4 when 0x88F2BC), the measure being **0x5D8A90**: max(|point xy − pos|, |wrap(point +0x08 − coord)| − 50 (0x785B80)) − 250; after each point the scan skips −2 − _ftol(measure × −0.02) (0x7852AC) more points when that is positive; the point must carry flag bit 25 (flag set) or 26 (the drawn flag, not flag'), pass **0x409520**(point, record, flag') (not the record's last point for flag' set, not its first for clear) and have the least gap wrap(+0x08 − coord) > 0 of all records. Back in 0x42E620 the flag flips when the point's record (0x5D89A0 with ECX = the point, at 0x42E6D9) has +0x70 bit 1 (coordinate race); **0x410CA0**(point, flag) gives the lane: in a drag race (0x89E7D8 / 0x890108) with human racers, with odds ½ (0x5EA510) a random human racer's (0x5EA4F0(count × 100000)) controller lane (its vt+0x1C, 2–9) moved to this side (−2 or −6, + 2 for flag set, + 6 clear) if the record has it; else up to ten random picks of the side's four (0x5EA4F0(400000) / 100000), then the first that exists; −1 none; the other side is tried when it fails. **0x418110**(&{record, point}, spacing, car) gates it: (a) when the track's byte +0xF4 + 0x89E7A4 is 0 and the race road is built and (no race status or the race clock 0x402000 under 0x783BAC = 0, the countdown), a point whose wrap(+0x08 − start line) is under the track float +0xF8 + 4·0x89E7A4 is refused (start line = race status +0xBF8 by +0xBF6 × 0.01 × 0x88F2B0); (b) a point with a race coordinate (0x5DCC90): **0x409890** (race status 4 only) refuses |0x5F12B0(+0x08)| < 250, where **0x5F12B0** is the least |Δ race coordinate| to the coordinates the world update 0x609CF0 copies each frame from the first +0x14 cars of the world list (+0xE0 + 4i: two, four with 0x89CF69 / 0x8669F4 / 0x866A14 and +0x14 ≥ 3); and **0x411060**(coord, spacing, car) refuses when a listed car (not the placed one, not hidden 0x5EC070, not a parked racer/traffic car +0x2C → +0x77C) has d = wrap(coord − its +0x8FC) with |d| < spacing for controller type 3 (car +0x14 → +4: an AI racer) or 0 < d < spacing for any other; else 2D (0x409840, 0x410FC0). Then 0x42C950(car, point, 0, lane, 0x41C4B127 = 24.5865, speed byte (point + lane + 0x2C) × 65536 × 1/65536, 0) places it and 0x42E620 returns 1. **Keeping a car** (0x417BB0, verified): a traffic car (controller vt+0x68 false) needs placing when parked (+0x2C → +0x77C) or when 0x410DE0 says it is not near a player: in race status 4 only, in a coordinate race 0x409890 on its +0x8FC (race coordinate alone), else 0x409840 on +0x60 (2D). **Pass** (0x42F910): 0.3 s apart in a drag race (0x4098D0 → 0x783BA4), each pass trying up to 16 cars (a try counts whether or not it places; a failed try parks the car) or until the cursor comes round. **Clone**: drag races now load their traffic (`Race::traffic`, Routes<track><F|B>.bin, the event's / Quick Race's level) and `traffic_think` takes this placement with the player's race coordinate (`Progress::along`, which matches the control points' +0x08: 305.2 against 305.18 at the Bayview Bridge drag's grid), its DragSteer lane, the race clock and the track's start gap (`Track::start_gap`, +0xF4 / +0xF8); the traffic cars' and rivals' race coordinates are their places located on the race road (**inferred**: +0x8FC of a traffic car is not traced); 0x5F12B0 is taken as the player's coordinate alone (**inferred**: which cars are the first +0x14 of the world list is open). The traffic waits through the countdown (the clone does not run it then; **observed**: in the original its cars start from about 0 m/s at the green). **Fill before the race** (2026-10-31, race2; **verified** in the disassembly unless marked, **observed** under Wine, `a private Wine copy`, Quick Race drag, Bayview Bridge, Traffic Level Minimum, 3 opponents): the race set-up's car placement 0x6065C0, after the racers, walks the car table (0x89E810, 0x940-byte entries, count 0x89E814; the traffic entries have +4 = 3 and, observed, +1 = 50, 51, … 59 in table order) and hands each traffic entry to **0x42F7A0**, which appends the car it returns to the world list (0x890080): in a drag race (0x89E7D8 / 0x890108) **0x42E4A0** first, then 0x42E0B0 (teleporters: returns 0 when 0x883DC8 is 0), then outside drag races 0x42E2C0 (not read). 0x42E4A0 (returns 0 unless the race road is built, 0x88F2BE): for k = 0..99, D = track float +0x100 + 4·direction (31.0 on Bayview Bridge, read in memory at 0x883DAC +0x100), the flag drawn (0x5EA510 under 1 − track +0x108 + 4·level), the point 0x417C60(**no position** (null), race coordinate 0, reach (entry +1 − 50 + k) × D, flag) — with a null position 0x5D8CA0's measure is wrap(point +0x08 − 0) − reach, so the point is the least race coordinate at or past the reach; the flag flips by the record's +0x70 bit 1; the lane 0x410CA0 (either side); then 0x418110(&{record, point}, D, no car) and the car is created there by **0x42C670**(entry, point, lane, speed byte (point + lane + 0x2C) × 65536 / 65536, 0): 0x604270 with the point's +0x08 as the car's race coordinate, the heading from the lane, velocity = speed along the heading (0x43CF90), TrafficMover 0x5F46B0(2), controller type 4. During the set-up the race status is 0, so 0x418110's start gap applies (before the green) and its 250 m tests (0x409890 / 0x409840, race status 4 only) do not. **0x418110's two paths** (verified): **0x5DCC90**(point) is true when the race road is built, the point's record has +0x70 bit 9 clear and the point has flag bit 29: then 0x409890 / 0x411060 by race coordinate (0x411060 refuses 0 < d < spacing for any non-AI-racer, so a car on the very point, d = 0, does not refuse it); otherwise 2D: **0x409840** (race status 4: 0x5F0FC0 < 250 m, 0x78429C) and **0x410FC0**(xy, spacing, spacing, car): any listed car (world list, not the placed one, not hidden, not parked) within the spacing in 2D (the second spacing for controller type 2). Bayview's drag routes (ROUTESL4RB/Routes4201F…4203F) have record 1 (flags 0x7FE341: bits 1 and 9) whose points carry bits 25 and 26 and record 100 (0x7FE141) whose points carry neither, so every drag placement lands on record 1 and is tested in 2D. Its points lie every ~49.5 m (403.60, 452.48, 502.47, 551.68, 600.99, 649.87, 699.72, …); the start line (race status +0xBF8 by +0xBF6 = 13.637 % of the 2258.26 m road = 308.0) plus the start gap 100 (+0xF8) refuses up to 408, so the first car takes 452.48 and each next car the next point (the one before is within 31 m in 2D): under Wine ten cars at 452.5, 502.3, 551.7, 601.0, 650.0, 699.6, 748.9, 798.6, 847.4, 896.2 in one load and 452 … 945 with 650 skipped (a point whose bits do not match the drawn side) in another, all at race status 0, standing (+0x42C = 0) until the start scene, where they roll on from ~19 m/s and stop by its end (~12 m on); they keep their places through the countdown. **Why they roll** (2026-10-31, race5; **verified** in the disassembly, **observed** under Wine with a memory log, `race5-tmp/probe.log`): the scene's start (0x43B930 / 0x43B9A0, NIS manager 0x827E88 with +0x62 set) pauses the traffic: +0x61 = 0 and **0x4099B0** on the placer, which for each traffic car (world list +0x18 … +0x10) not yet paused calls 0x593100 on its body (+0x3C), switches its mover to mode 0 (0x5F46B0(0): a 12-byte mover, vtable 0x7A5BB8, body +0x5C = 0), makes it inactive (**0x5ED5D0**(0): car +0x550 = 0, so World_DrawCars skips it at 0x6317A2 and the minimap's AI test fails) and sets the controller's +0x77D. The traffic think (controller vt+0xC, 0x429D90, vtable 0x785428) returns at once while NIS +0x61 is 0 (0x430340) or +0x77D is set. The set-up gave each car its lane speed (0x42C670 → 0x592170), and in mode 0 the body coasts on it to a stop: memory, three cars from race state 0 to 3: 15.2 m/s at the scene's start, 0 by its middle, 12.2 m from the fill place (872.4, 118.8) → (862.5, 126.0). At race state 3 (the scene's end) 0x430310 resumes them (+0x61 = 1, **0x409A30**: 0x593110(body, 0), mover 0x5F4410(car), active, +0x77D = 0) and the race-state notice (0x5FC910 → controller vt+0x14 = 0x408CA0, states 3 and 4: active, mover 2) — so they stand, visible, through the countdown and set off at the green (0.7 m/s at the first read past it, 20 m/s ~70 s of emulator time later). So during every race start scene the race's traffic is hidden. After the green the cars the placer re-places beyond 250 m stand at 601 m at 20.1 m/s **parked** (+0x77C = +0x77D = 1, inactive), i.e. hidden, not frozen in view. **Clone**: `Traffic::paused` hides the race's traffic (body, shadow, glows, headlight pools, sound) while the start scene's camera runs (`run.frame < cut`, the clone's state 2); **the coast** (race7, **observed** under Wine with a memory log of car +0x28 / +0x34's vtable / the input car +0x30, `race7-tmp/probe7.log`, and **verified** in the disassembly unless marked) is not a mode-0 motion: mode 0 (vtable 0x7A5BB8: 0x5EA1E0 dtor, 0x5EB930 = the wheel matrices only, 0x5923F0 → 0x5921C0) moves nothing, and the car body's slots with +0x5C = 0 (0x593B50 etc.: the body has no rigid body, +0x68) only read the car. The log shows mode 4 at the load (car +0x5C0 = 0: the world under the car not loaded, 0x5F46B0 at 0x5F472E), then **mode 2, the TrafficMover (vtable 0x78456C)**, through the whole scene though +0x77D is set: when the world under the car loads, 0x5F6CE0 (car +0x5C0 0 → 1) calls the controller's slot +0x10, **0x408C40** → 0x5F46B0(0x5F4410(car)) = 2, whatever the pause. Its pedals are the car's input (+0x30; TrafficMover +4): gas +0x20C = 0, brake **+0x214 = 1**, which **0x5F8B00** (from 0x5FBBF0; the per-car object of vtable 0x7A63E0, whose ctor 0x5FCBC4 also sets it) holds at 1 on every call while race status ≠ 4. So the car brakes on 0x408820 from its fill speed (+0x430, 20 m/s logged) by the braking table (0x7EA050, dv/dt = −(6.705 + 0.818 v)): 20 → 0 over 12.07 m (race5's log: 12.2 m; race7's: 14.28, 9.61, 6.88, 4.15, 1.92, 0.09 m/s at 960-tick steps, the table to 0.1 m/s²). At race state 3 the think resumes (gas 1, brake still 1: 3.58 − 6.71 < 0 at rest), so they stand until the green. **Clone**: `traffic_coast` runs each race traffic car's TrafficMover with gas 0 and brake 1 through the scene (hidden until the cut, as `Traffic::paused`) and gas 1 / brake 1 in the countdown; the steering is the controller's last (straight since the fill; **inferred** for the countdown's resumed think). The placer object (0x4179F0: +0 the countdown, 0.3 s in a drag race; +4 the cursor; +0x10 the restream tick) is untouched by the fill (read in memory: +0x10 = the load's tick 4000). **After the green** (observed, same run, the player standing on the grid at 305.2): 0x42F910's passes re-place the cars beyond 250 m of race coordinate from the player (0x417BB0 → 0x410DE0 → 0x409890) at the first allowed point, 600.99 (2D ≥ 250 m from the player, 0x409840), every 0.3 s, so several cars stand on that point at 20 m/s without moving ("frozen" — re-placed each pass, not failed); the two within 250 m drive off. **Clone**: `TrafficLane::fill_drag`, `traffic_fill_drag` (on the race's first frame), `drag_gate` (both paths, `coordinate_point` = 0x5DCC90), `Track::drag_fill` (+0x100). **Inferred**: the human racer's lane for 0x410CA0 at set-up time (the clone passes the DragSteer lane when known). **Fill outside drag races** (2026-10-31, race3; **verified** in the disassembly, ported: `TrafficLane::fill_race`, `traffic_fill`): 0x42F7A0 → 0x42E0B0 → **0x42E2C0** (ECX = the placer, world +0x118). The placer's +0x14 is set by **0x409900**(array +0x18, 1) from its ctor 0x4179F0: the running sums of +0x34 over the race records 0x88D21C that have a lane on the set side (0x5D4490(1), no flip), stored per record in the array, the total in +0x14. With +0x14 ≠ 0, for k = 0..99: flag = 0x5EA510(1) < 1 − track +0x108 + 4·level; **0x411110**(&pick, flag): r = 0x5EA4F0(+0x14); over the records (flag flipped by +0x70 bit 1 when the race road is built) that have a lane on that side, sum += +0x34 until sum > r, then pick = {record, record +0x8C + 0x38·(sum − r)}; with no such record the pick is left as it was (0x42E2C0's locals start null once, before the loop, so an earlier try's point is retried). The point must have +0x0C & 0x7FF = record +0x0A, bits 25 and 26 both set, and index ≤ +0x34 − 2; then **0x418110**(&{record, point, …, its x, y, z = +0x12 × 8192 / 65536}, track +0x100 + 4·direction, no car) as in the drag fill; the flag flips by the record's +0x70 bit 1 (race road built); **0x417C30** = 0x410CA0(point, flag), or with the flag negated when that is −1 (outside drag races: ten random picks, then the first); 0x42C670 creates the car. The clone runs it on the race's first frame for every pool car of a non-drag race with traffic, with the same gate as the drag fill (start gap before the green, 2D or race-coordinate spacing). **Inferred**: the race records are listed when the placer is built (0x5FF600, in the world set-up); the cars' behaviour before the green outside drag races (the clone runs no traffic until then; not checked under Wine). Horns. Traffic in races is ported (Traffic level).


* **Still open**: the free-roam source of the car's race coordinate (see Racer +0x8FC); the controller +0x0C of a human racer (the placer's road node, 0x5EC0C0; its writers are not traced); the MeloMvr's rest test (0x5AB0D0 → 0x5A5E30 without wheels; the drive model's, 0x5ABB2E with its wheels, is ported) and modes 4–5's movers; the bodies of 0x5EC070, 0x403A40, 0x404910 and 0x402CC0; the byte at +0x10 of the car's +0x2C object (the lane request's flag); the lane request's port (its flag and record are not traced); the global at 0x865188 that the lane setter's +0x0C stamp takes (its value not traced); and the drag placement's port, whose inputs are 0x417C60's road-table search (0x88D218/0x88D21C, 0x5D4490, 0x5D8CA0, 0x409520) and 0x418110's first test (0x5D3E50, 0x402000, and their globals).
* **Drag placement** (0x42E620, **verified** 2026-10-30 in the code and disassembly; ported, `TrafficLane::spawn_drag`, `drag_point`, `reach_point`, `drag_lane` in src/ai/traffic.rs): ECX/EBP is the placer, arg 1 the traffic car, arg 2 the human racer's car, arg 3 the spacing. It returns 0 unless the race road is built (0x88F2BE); the 0x5D5CE0 call on the two cars' race coordinates is unused. The flag is a draw 0x5EA510(1.0) under 1 − track float +0x108 + 4·level (0x883DAC, 0x89E7A8). **0x417C60**(car +0x60, car +0x8FC, 250 (0x7E9720), flag): over the race records 0x88D21C (count 0x88D218), each with flag' = flag, flipped in a coordinate race when the record's +0x70 has bit 1; the record must have a lane on that side (0x5D4490, ECX = record: any byte of +0x84..+0x87 for flag' set, +0x88..+0x8B clear, i.e. lanes 2–5 / 6–9); **0x5D8CA0**(pos, coord, 250, 0) (ECX = record) gives its point i (records +0x8C + 0x38·i, count +0x34) with the least measure ≥ 0 that is ahead (wrap(point +0x08 − coord) > 0, the lap wrap 0x88F2B0/0x88F2B4 when 0x88F2BC), the measure being **0x5D8A90**: max(|point xy − pos|, |wrap(point +0x08 − coord)| − 50 (0x785B80)) − 250; after each point the scan skips −2 − _ftol(measure × −0.02) (0x7852AC) more points when that is positive; the point must carry flag bit 25 (flag set) or 26 (the drawn flag, not flag'), pass **0x409520**(point, record, flag') (not the record's last point for flag' set, not its first for clear) and have the least gap wrap(+0x08 − coord) > 0 of all records. Back in 0x42E620 the flag flips when the point's record (0x5D89A0 with ECX = the point, at 0x42E6D9) has +0x70 bit 1 (coordinate race); **0x410CA0**(point, flag) gives the lane: in a drag race (0x89E7D8 / 0x890108) with human racers, with odds ½ (0x5EA510) a random human racer's (0x5EA4F0(count × 100000)) controller lane (its vt+0x1C, 2–9) moved to this side (−2 or −6, + 2 for flag set, + 6 clear) if the record has it; else up to ten random picks of the side's four (0x5EA4F0(400000) / 100000), then the first that exists; −1 none; the other side is tried when it fails. **0x418110**(&{record, point}, spacing, car) gates it: (a) when the track's byte +0xF4 + 0x89E7A4 is 0 and the race road is built and (no race status or the race clock 0x402000 under 0x783BAC = 0, the countdown), a point whose wrap(+0x08 − start line) is under the track float +0xF8 + 4·0x89E7A4 is refused (start line = race status +0xBF8 by +0xBF6 × 0.01 × 0x88F2B0); (b) a point with a race coordinate (0x5DCC90): **0x409890** (race status 4 only) refuses |0x5F12B0(+0x08)| < 250, where **0x5F12B0** is the least |Δ race coordinate| to the coordinates the world update 0x609CF0 copies each frame from the first +0x14 cars of the world list (+0xE0 + 4i: two, four with 0x89CF69 / 0x8669F4 / 0x866A14 and +0x14 ≥ 3); and **0x411060**(coord, spacing, car) refuses when a listed car (not the placed one, not hidden 0x5EC070, not a parked racer/traffic car +0x2C → +0x77C) has d = wrap(coord − its +0x8FC) with |d| < spacing for controller type 3 (car +0x14 → +4: an AI racer) or 0 < d < spacing for any other; else 2D (0x409840, 0x410FC0). Then 0x42C950(car, point, 0, lane, 0x41C4B127 = 24.5865, speed byte (point + lane + 0x2C) × 65536 × 1/65536, 0) places it and 0x42E620 returns 1. **Keeping a car** (0x417BB0, verified): a traffic car (controller vt+0x68 false) needs placing when parked (+0x2C → +0x77C) or when 0x410DE0 says it is not near a player: in race status 4 only, in a coordinate race 0x409890 on its +0x8FC (race coordinate alone), else 0x409840 on +0x60 (2D). **Pass** (0x42F910): 0.3 s apart in a drag race (0x4098D0 → 0x783BA4), each pass trying up to 16 cars (a try counts whether or not it places; a failed try parks the car) or until the cursor comes round. **Clone**: drag races now load their traffic (`Race::traffic`, Routes<track><F|B>.bin, the event's / Quick Race's level) and `traffic_think` takes this placement with the player's race coordinate (`Progress::along`, which matches the control points' +0x08: 305.2 against 305.18 at the Bayview Bridge drag's grid), its DragSteer lane, the race clock and the track's start gap (`Track::start_gap`, +0xF4 / +0xF8); the traffic cars' and rivals' race coordinates are their places located on the race road (**inferred**: +0x8FC of a traffic car is not traced); 0x5F12B0 is taken as the player's coordinate alone (**inferred**: which cars are the first +0x14 of the world list is open). The traffic waits through the countdown (the clone does not run it then; **observed**: in the original its cars start from about 0 m/s at the green). **Open**: a fill before the race — **observed** under Wine (Quick Race drag, Bayview Bridge, Traffic Level Minimum, 3 opponents): at race status 0, before the start scene, 10 traffic cars (world list 4–13; the list +0x10 = 14, +0x18 = 4 racers, +0x14 = 1) already stand on the road at race coordinates 452.5, 502.3, 551.7, 601.0, 650.0, 699.6, 748.9, 798.6, 847.4 and 896.2 (about every 49.5 m, from 147 m past the grid at 305.2) at 20 m/s; 0x42E620 / 0x42C950 have no caller besides 0x42F800 / the placers, so another routine places them (not found; 0x409770 and 0x6012C0 re-place a car on its controller's point, not this). The clone has no such fill: its traffic appears from the first pass after the green, ≥ 250 m ahead. The static byte 0x83ABB8 (type 12, value 2) is not a single-player Quick Race's level (see Traffic level). Horns. Traffic in races is ported (Traffic level).

## AI car looks (src/ai_looks.rs; game.rs `line_up_rivals`, `Encounters::new`)

Traced in the disassembly (Ghidra was unavailable: its JDK is missing),
**verified** unless marked. A ride's parts are dwords at ride +0x590 +
4·slot (0xAA slots); the clone keeps them as `installed` (slot → hash over
the stock car), hash 0 for a slot the game empties (the car loader then
draws nothing there).

* **Part searches.** 0x61B990(car, slot, hash, prev, level) walks the
  slot's two groups (0x6101F0) through 0x61B680: parts of the slot's type
  after `prev`, with the hash when non-zero, of the level (part +5 >> 5)
  unless −1, skipping the EXCLUDE_UG1 / UG2 / SUV parts. 0x631A30 counts
  them; 0x631CC0(slot, level) takes a random one (Rand_Int(count), then
  that many + 1 steps; 0 when there is none, no draw).
* **Who dresses whom.**
  * Career free roam's encounter racers (0x53F010, from 0x540790): names
    (0x4FE970(8)) first, then per racer 0x4FEAF0 (car), 0x6371A0 (stock
    parts, 0x637050(0)), 0x6371F0 (a random paint: slot 0x3F of any level;
    vinyl slots 0x40–0x43 emptied), 0x6371B0(0x9A, 2) (a random level-2
    underglow neon), 0x61C290(2 + i), the skill row (0x4FED00), 0x6373F0,
    0x601610; after them all 0x532020(0).
  * Quick Race (0x53EEA0): names, then per racer 0x4FEAF0 (which ends with
    0x610280 and 0x637050(0): stock) …, then 0x532020(0).
  * 0x532020(0): finds the best-rated human racer (0x5B69A0 on +0x10),
    then for each AI racer (racer info +0x04 = 2, 0x5ED910: list 0x89E7A0
    +0x70, 0x940 bytes each, ride at +0x10) matches its performance to
    that human's (debug floats 0x7FAA90… when set, else 0x5B7580) and
    calls 0x6399E0(1, 1). A non-zero parameter copies the human's car
    instead (0x413440, 0x61C290, 0x632ED0; not used by these callers).
  * A career event (0x5260C0): a RANDOM car (0x4FEAF0, stock) is dressed,
    0x6399E0(1, byte = 1); a preset (0x637280) keeps its parts,
    0x6399E0(1, 0).
  * 0x6399E0(refit, dress): with `refit` it refits the performance by the
    fraction (cat 8 + cat 5 + cat 0 of 0x599610) / 3 clamped to 0–1
    (0x5A6890; not ported), then with `dress` runs 0x6374C0(1); then the
    brakes and engine from the performance parts (see **Brakes from the
    performance parts**).
* **0x6374C0** (the levels `L[c]` = 0x5995F0(c), the upgrade state's float
  per category rounded; `mean` = trunc(Σ L[0..9] × 0.1)):
  * Table 0x802C50 (0x802DD8 = 14 rows of 0x1C: +0x00 slot, +0x04 a flag,
    +0x0C four rows of four signed chances): per row in order, the level
    row is L[9] for slots 1, 7 (and 0x0D), L[6] for 0x11, L[3] for 0x1B
    (jump table 0x637820 by slot − 1 through 0x637830), else `mean`;
    Rand_Int(100) against the row's running sums gives level k; k < 4
    takes 0x631CC0(slot, k) (0 empties the slot), k = 4 the first level-0
    part when the flag is set, else 0. Rows (`ai_looks::SLOT_CHANCES`):
    slots 9, 7, 0x0C, 0x10, 0x20, 0x0F, 0x0E, 0x9F, 0x99 (flag 0) and 1
    (flag 0) {60,30,10,0} {20,60,20,0} {0,20,60,20} {0,10,20,70}; 0x3F
    {0,70,30,0} {0,70,20,10} {0,20,60,20} {0,10,20,70}; 0x49 {70,20,10,0}
    {50,30,20,0} {20,10,60,10} {0,10,20,70}; 0x4A as 0x3F (flag 0); 0x9A
    {90,10,0,0} {40,50,10,0} {30,20,50,0} {30,10,30,30}. Every row sums
    to 100, so k = 4 never happens.
  * The body kit (slot 1): with one, `%s_KIT%.2d_REAR_BUMPER` and
    `%s_KIT%.2d_SKIRT` (0x7A8D04 / 0x7A8CF0; the car type's name and the
    kit's kind, part +5 & 0x1F) by hash in slots 2 and 0x0B, both set only
    when both are found; without one the level-0 kit, rear bumper and
    skirts.
  * Rims: a level from {70,20,10,0} {50,40,10,0} {10,20,50,20}
    {0,10,20,70} by L[8] (bytes built at 0x63776B), then 0x632DF0(ride,
    k): slot 0x1D parts of level k whose attribute 0xCE7D8DB5 (0x4FE130,
    as a signed byte) is car type +0xDC and 0xEB0101E2 (0x4FE120) lies in
    +0xDD…+0xDE, up to 128, Rand_Int pick; found, slots 0x1D and 0x1E take
    it.
  * Then 0x632ED0 (`level` = trunc of the same mean): Rand_Int(100) <
    {0,0,5,10}[level] takes a whole-car vinyl (0x61C4C0: a random slot
    0x40 part of any level with kind 0x18, 0x1A or 0x1B, up to 512) into
    slot 0x40, emptying 0x41–0x43 (none found: nothing). Otherwise three
    layers: slot 0x40 kind 0x17 at a level from {95,0,5,0} {90,0,10,0}
    {80,0,20,0} {65,0,35,0}; 0x41 kinds 0–0x0D, 0x10–0x14 from
    {80,10,10,0} {70,15,10,5} {55,25,15,5} {40,20,20,20}; 0x42 kind 0x19
    from {95,0,0,5} {90,0,0,10} {80,0,0,20} {65,0,0,35} (all drawn, then
    set; none found empties); then each layer's colour slots 0x4D + 4·layer
    + n take the colour set Rand_Int(5) of 0x8A43C8 (`VINYL_COLOUR_SETS`,
    hashes the static constructor 0x77F520 makes) in a shuffled order
    (0x610580 for layer 0, the same inline for 1 and 2: {0,1,2,3}, each
    place swapped with Rand_Int(4); the shuffle comes before the set's
    draw). Then the paint again (row 9, 0x802D4C) by `level`; and a paint
    listed in 0x8A4418 (5 records of 0x28: paint hash, count 5, hashes;
    static constructor 0x77F2D0; `PAINT_MATCHES`) gives slots 0x45 and
    0x47 one of its matches each (two Rand_Int(5), then the parts by
    hash).
* **Presets** (0x637280): a different car type first resets the ride
  (0x610280, 0x637050(0)); per slot 0 empties it, 1 keeps the stock part,
  another value is found by hash at any level (not found: 0), slots 0x19
  and 0x61–0x68 are never written; slot 8 then always takes the car's
  first part of any level; then the performance level (+0x48, 0x5A64B0)
  or flags (+0x2F4). Rachel's DDAY_PLAYER_CAR has 0 only in slots 8,
  0x23–0x2E (NO COMPONENTS, no models) and 0xA2 (nozzles), so emptying
  them changes nothing visible (the maintainer's probe `preset_slots`).
* **Clone**: `ai_looks::Ride` ports 0x631CC0, 0x6371F0, 0x6371B0,
  0x61C4C0, 0x632DF0, 0x6374C0 and 0x632ED0 with the shared random
  numbers (`opponents::GameRng`); `Preset::installed` gives 0 for a
  preset's 0; the car loader draws nothing for hash 0. Encounter racers,
  Quick Race and career RANDOM opponents are dressed, a career preset
  opponent takes the preset (the maintainer's probe `ai_looks` prints rides).
  The levels L[c] are the upgrade state's floats rounded, from the
  racer's own fitted parts after its performance is set (see **AI
  performance**; **verified**). **Inferred**: the random sequence is not
  the game's (the clone's state has other draws between; 0x5A6890 and
  0x5B7580 draw nothing). Slot 8 of a preset keeps the stock part (the
  first of level 0, inferred to be the first of any level).
  **Open**: not yet compared with the original under Wine (the plan: a
  Quick Race grid, reading each AI racer's parts at racer info +0x5A0 —
  part record pointers, hash at +0, level at +5 >> 5 — through the
  0x89E7A0 list; the private prefix copy would not start the game in
  this session).
* **AI performance** (`performance::{state_float, fit_category, refit,
  ai_refit}`, `ai::route::match_to_human`; game.rs `line_up_rivals`,
  `Encounters::new`, `human_perf`; the maintainer's probe `ai_perf`). **Verified**
  in the disassembly unless marked:
  * The upgrade state (ride +0x2F0 = 0x4017E0; the performance object is
    ride +0x10 = racer info +0x20, its state at +0x2E0, rating at +8):
    floats +0x00 + 4c are 3 × the category's level (0x5A6590 stores it;
    0x5AC500 zeroes the ten, then its per-category calls 0x5A6B60 …
    0x5A74B0 write them with the gain counts 0x5A6590 defaults to: 3 for
    categories 1 and 5, 2 for 9, else 1). 0x599610 reads float × 1/3,
    0x5995F0 float + 0.5 truncated.
  * 0x5A64B0(level, c): category c off (0x5A6460, also for level 0), then
    levels 1 … level, each package's parts fitted unforced (0x5A6330(part,
    1, 0)); c = −1 every category, level by level.
  * The refit 0x5A6890(target, categories, n; ECX the upgrade state): top
    = min(3, ftol(3·target + 0.5) + 1). Per category: off (0x5A6460);
    state A and best B copies; best = its level; done when |best − target|
    < 0.05. For level l = 1 … top while not done, three passes from the
    base state: pick (0x5A6220) the unfitted part of the category's level-l
    package with the largest mean gain (0x591140: n = gains not −1 by bits,
    the mean of the first n), or with `smallest` the smallest (ties keep
    the earlier); fit it (0x5A6330(·, 1, 0)); a level closer to the target
    than best becomes best (B = A), done within 0.05; pick again. Pass 0
    always largest, pass 1 always smallest, pass 2 alternating, smallest
    first. A resets to the base after each pass. Not done and l < top: the
    base gets the whole level (0x5A64B0(l, c)). Then the state = B; the
    mean of the categories' levels is returned. A part that will not go on
    would be picked forever; the clone ends the pass there (**inferred**
    guard, never met for categories 2 and 9).
  * 0x6399E0(1, dress): target = clamp((m5 + m8) + m0) × 0.33333334 to
    0–1, m = 0x599610 (tyres, suspension, weight), then 0x5A6890(target,
    {9}, 1): the aerodynamics refitted (`ai_refit`); then 0x6374C0 when
    dressing. 0x639CF0(kind == 2, …) runs 0x6399E0(that, 0) at an AI
    racer's creation (0x5F6C4F, 0x601195, 0x6064CE: cmp owner kind, 2;
    sete), so an AI car's category 9 stays the refit (the same, no draws)
    and a human's comes from its body parts (`fit_aero`).
  * 0x5B6AE0(rating, scaled, score; ECX the performance object): with
    `scaled` the rating maps 0–10 to 0x86B3F8 … 0x86B3FC (0x401790); the
    car type's presets (0x86B0C8 count / 0x86B0CC pointer per type, from
    GLOBALB 0x34A31 chunks in order, 0x599450) give the nearer of the two
    around it (`Presets::nearest`); every part comes off (+0x330 = state
    +0x50), the preset's bits go on, 0x5B4DD0 sets the car up (mode 3, 2
    or 10 by the online / race flags, else 0). With score ≥ 0, once more
    at rating + 150 × (score − 0x5B54A0 of the fitted car).
  * 0x5B7580(human, nitrous, offline; ECX the AI's performance object):
    0x5B6AE0(human +8, 0, offline ? human's 0x5B54A0 : −1), then with
    `nitrous` 0x5A64B0(ftol(human +0x2E8 + 0.5), 2) — the human's nitrous
    level — and 0x5B4FD0(1, 1, 1).
  * Quick Race (0x53EEA0) and career free roam's encounter racers
    (0x53F010) call 0x532020(0): the best-rated human (0x5B69A0; one
    player: the player), then each AI racer (kind +4 = 2; racer i at
    [0x89E810] + 0x940·i, count 0x89E814) 0x5B7580(human, 1, offline)
    — the debug floats 0x7FAA90 … 0x7FAAA4 are −1, so their 0x610350
    branch never runs — its class (0x5B5340) to info +0x24, then
    0x6399E0(1, 1).
  * A career event (0x5260C0) by the entry's skill source: 1 0x5B7580
    with the player (as above); 2 nothing; 0 and 3 0x5B6AE0(the player's
    car's preset range lerped by min(skill, 10) / 10, 0, offline ? spread
    lerp : −1) (see **Upgrades from skill**), the class from 0x59A4A0,
    then 0x5A6890(the player's nitrous level 0x599610(2), {2}, 1): the
    nitrous refitted towards the player's. Every entry then 0x6399E0(1,
    dress) (0x5263F5).
  * The clone: the human (`HumanPerf`, game.rs `human_perf`) is the
    career ride's stored state with its uniques — the rating of its record
    as free roam sets it up, its handling score, its nitrous level. Rachel's
    D-Day car (no car bought yet) is taken without the aero its body parts
    give: the preset (0x637280) does not run 0x639CF0, only the human
    racer's creation does, after 0x532020. A bought car is taken with that
    aero (the shops refresh it through 0x639CF0; that the refresh is
    stored is **inferred**).
  * **Checked under Wine** (memory read through the racer list, info +0x20
    performance object, +0x300 floats, +0x350 fitted bytes; peek loop,
    the maintainer's probe `ai_perf`): a Quick Race (stock Peugeot 206, rating
    196.696 in both): the Corsa and 106 rivals fitted part 0x30 only
    (float 7 = 0.3), the 206 rival none — as the clone. A new career's
    free roam (Rachel's 350Z: the same fitted parts and floats as the
    clone's, rating 477.923, handling score 0.65848 read and computed
    alike): the Golf, TT and 3000GT encounter racers' fitted parts and all
    ten floats (nitrous 2.0, the category-9 refit 2.235 / 1.98 / 2.235)
    match the clone exactly when the human's score is the preset's own
    (0.6085, category 9 at level 2); with the body aero (0.6585) the
    3000GT takes preset 26 instead of 25. The perf block the clone builds
    for the 3000GT's final parts equals the game's (+0x30 … +0x2DC) and
    scores the same 0.71152.
  * **Open**: whether 0x5B54A0's tuning block holds the dyno tuning (the
    clone leaves it out); the order of a preset entry's own performance
    (0x637280's +0x48 level) against the skill source's fit (the clone
    fits the preset first, **inferred**); the career event path (sources
    0 / 1 / 3) was not read under Wine.
* **FE car in the car lot** (0x4DF9C0's list modes): the selected entry's
  ride comes from 0x511E00 — built once from the entry's car record
  (0x503510(record +0x920), then the record's vtable +0x0C) — and is
  copied into the FE car 0x8371B0 (0x413440 at 0x4DF8AC / 0x4DFBC0) and
  handed to the garage (0x4D8C70). So the crib's Car Select and a
  trade's list show a slot's car with its parts; the clone showed every
  lot car stock and now takes the slot's parts and performance parts
  (`Profile::slot_ride`; `--shot cribcars` with `NFSU2_VINYL=1` shows a
  slot car's vinyl). The main menu, the crib and the shops already showed
  the profile's car with its parts.

## Outrun (career free roam; src/outrun.rs, game.rs `Encounters`)

Verified unless marked. The game's help text says it: tuner cars on the
road, "marked on your Mini-map as orange triangles", are challenged "by
pressing the Enter Race / Shop key when prompted" (0x9FD2C234); the
outrun is won by gaining the maintainer and opening "a distance of over 300
meters" (0xA227EAD5). Quick Race's Outrun (race type 8) is the split-screen
/ online mode (see Outrun / Free Run); the career's runs as race state
0x89E7B0 = 7 inside free roam.

* **Encounter racers**: the career free-roam set-up 0x540790 sets
  0x89E7C4 = 3 (0x54083F) and calls 0x53F010(3) (0x540976), which, unless
  0x89E7E4, draws 8 driver names (0x4FE970: random(0x35) from 0x7F6CD0, up
  to 100 tries each) and for i < 3 fills the record 0x85EF80 + 0x8F0·i:
  0x4FEAF0(record, 100 + i, 2 + i, 1, 0, 0, −1.0) — the debug table
  0x7FAA78 (−1 none) first, else the pool (SUVs 0x7F6DDC when the player's
  car record +0x88A is set, param 6 being 0), the player's car type allowed
  (param 4), against the player's rating (param 7 < 0 → 0x5B69A0 on the
  player's), the shuffle and margin of the Opponent line-up; then 0x6371A0,
  0x6371F0, 0x6371B0(0x9A, 2), 0x61C290(2 + i), 0x6373F0(100 + i, 2,
  record, name, row, 1) with the row of 0x4FED00(0x89E7F4): a random
  fraction (0x43C1F0(1.0)) against ten cumulative floats at the AI tuning
  (0x81CB60, chunk 0x34B00) +0x9F4 + 0x28·difficulty, the first it is at
  most (`AiTuning::random_row`); and 0x601610 builds the car, parked. The
  manager's slots (+0x1B10 + 0x10·i: +0x00 stamp, +0x04 mode, +0x08 held
  flag, +0x0C car) take the world list's AI cars (0x890080 +0x14 … +0x18,
  0x412F90). A parked car is one at the parking height (0x5ED440: |z −
  0x8025B8| < 0.001); 0x5F6E30 parks, 0x5F6D60 brings one back.
* **The career's side** (career +0x4230 = 0x85EF70): 0x513DB0, from the
  career update (0x55140A, career +0x713C 0 or 9), in game (0x8654A4 = 6)
  with race state 0: +0x1AE4 −= dt (0x865150); with +0x1AE8 = 0 and 0x513D30,
  +0x1AE8 = 1 and +0x1AE0 = 7. 0x513D30: past stage 5 yes; stage 0 needs
  0x86347D and 0x86347E clear (so during the D-Day intro, in Rachel's
  car, until her third complaint; see **Stage 0's races**); then no world
  challenge running (career +0xC08, 0x85B948), the stage record's outrun wins (+0xDC, 0x8620F4 + 0x104·stage)
  under its definition's +0x40 (signed; −1 no limit), and +0x1AE4 < 0.
  +0x1AE4 is written only by 0x500CA0 (0, with +0x1AE8 = 0) and that
  decrement. 0x500D20(car): 0x4DB890(HUD element +0x58, car ≠ 0) and
  +0x1AE8 = 2 with a car, 3 without; 0x500D80: +0x1AE8 = 2; 0x500CB0:
  +0x1AE8 = 1 and 0x513E00's first test; 0x513E00: not (game mode 1, stage
  0 and 0x86347E) and 0x85B948 = 0.
* **The manager** (race status +0xC40; 0x42EDF0, called at 0x608C97 from
  0x6076E0 while race state 0x89E7B0 is 0 or 7; jump table 0x42EF8C; state
  names eRANDOM_ENCOUNTER_MANAGER_STATE_* at 0x7E9B78): 0 NONE → 0x412F90
  (+0x04 = 1, the loaded opponents dropped, +0x08 = 0, the slots refilled
  and cleared, 0x500D70: +0x1AE8 = 0) → 1. 1 POLL_CAREER: 0x41ACE0 (0x500CB0;
  +0x04 = +0x1AE0; the three career records copied to +0x10 + 0x900·i by
  0x413440) → 3. 2 LOADING_OPPONENTS (0x40C370 starts a load, 0x63BC80, of
  the next opponent record; 0x40C3C0 waits 10 s from it; 0x40C410 after the
  third) is not entered from 1. 3
  TRYING_TO_ENGAGE: with 0x81CB94 set 0x42CD60, else 0x40BEA0, 0x42A5E0,
  0x413030, 0x42A860, 0x40BEE0. 4 RACING: in race state 7 the goals — the
  race order's first, an AI of controller type 2–4, goal type 6 on the
  second (0x42A9E0 → 0x428510), every other such racer AIGoalChaseCar on
  the first (0x4285A0) — then 0x40BF60. 5 DEACTIVATE_OPPONENT: 0x40BF60, then while
  0x40BFB0 counts cars out nothing; at 0 → 0x500CA0 and NONE.
  - 0x42A5E0 (placement): none while the newest slot stamp is 5 s old or
    less (0x783D38); for each parked car: from the player's road node
    (0x5EC0C0: controller +0x0C; 0x5EC0B0) walk 0x415130 250 m (0x783D08)
    in the player's direction (controller +0x10); then while the walked
    distance is under 500 m (0x783D10): a point 250 m or more from the
    player (0x5F0FC0, 0x78429C), with flag bit 11 (+0x0C & 0x800) and
    0x410FC0(point, 30, 300, car) clear takes the car (0x5F6D60, 0x5FBEE0
    on the point, facing the walk's way, at its racing line's speed byte —
    +0x2C forward / +0x2D backward, 100 in RoutesFreeRoam — × 65536 ×
    1/65536, and 0x5F52D0 moves it at that + 0.1 m/s; controller +0x10 =
    the walk's way; 0x428630(player car, 10, 0, 7)); the slot stamped, mode
    1, held flag clear; one car a call. Else 50 m more (0x783D0C). A failed
    walk skips the car.
  - 0x40C030 (the engage test, from 0x413030 per player): three passes over
    the three slots; each visit clears the slot's flag (keeping the old
    one), sets the car's +0x554 → +0x208C to 0 (0x60F540, not traced
    further) and, with no car chosen yet, tests an active car: ahead of the
    player along its own direction (the velocity's x, y from 1 m/s,
    0x784250, else the heading; normalised), 0 … 30 m in the plane (held:
    −10 … 40), height within 5 m (≤, 0x40C21D), its speed (+0x430 ×
    2.237) at least 30 mph, within −10 … +10 mph of the player's (held:
    ±15), and the angle between the directions, 0x4000 − asin16(dot)
    (0x43C490) × 360/65536, under 10° (held: 20°); the first to pass is
    chosen and its flag set (the later passes clear it again, so every
    flag ends clear). 0x413030: the chosen car, when 0x513E00 allows, gets
    0x60F540(car, 1) and 0x500D20(car); else 0x500D20(0); player +0x250 =
    the car either way.
  - 0x42A860 (TRYING_TO_ENGAGE, each slot car): 0x41C100(player car)
    gives a lateral and a longitudinal offset; mode 1 (≥ 30 m off) or 2 / 3
    (≥ 20 m off back to 1) pick a speed offset of ±5 and 0x428630(player
    car, 10, offset, 7 − age): the AIGoalOffsetPursuit (goal type 9,
    0x40DBA0, vtable 0x784BF8) on the player.
  - 0x40BEE0: an active car placed over 30 s ago and 250 m or more from
    the player (0x5F0FC0) loses its flag and is parked. 0x40BF60: a done car
    (+0x998, 0x5EC070) past its hold (+0x974 + +0x978, 0x5EBFB0) is parked.
    0x40BFB0: active cars done or within 250 m count, the others are
    parked.
* **The key**: the player's event handler 0x602BE0 (jump table 0x603598,
  index 0x6035EC) sends JOY_EVENT_REQUEST_ENGAGE_INFO (0xBF, name pair at
  0x7FB150) to 0x602DA9: with an event in reach (player +0x9AC, 0x527900)
  it is engaged; otherwise (0x602E19), in game mode 1 with 0x85EF50 +0x1C
  clear (0x5012C0), race state 0, player +0x250 set and 0x500D80, it calls
  0x4DB890(0) (the icon goes), sets race status +0xC30 and calls 0x838590 →
  0x5010E0. "Enter Race / Shop" is the Controls row the help text names
  (DIACTION action 15; its event binding not traced).
* **The race**: 0x6076E0 sees +0xC30 and calls 0x607190 (in game mode 1
  first 0x532E40 → 0x529B60, 0x511000): race state 7, +0xBE8 = 0, laps
  0x89E7BC = 0, 0x89E7C0 = the humans (0x890080 +0x14); every AI car of the
  world list becomes a racer (0x5FC2B0), the clocks start, the manager
  races (0x40BE80: state 4, +0x04 = 7 only when 0x89E7C0 = 2 and
  0x89E7C4 = 0), the race order puts the AI first, and every AI racer but
  player +0x250 is done: status 7, a place, +0x998 = 1 with a 2 s hold.
  Each frame 0x6016B0 → 0x5FC520 (state 7, offline) sets a racing racer's
  +0x24: 0 for the race order's first; else its 3D distance to the first
  (0x43CE10), unless (its +0x42C > 10 m/s or the first's ≤ 10, 0x7A56B0)
  and the first's controller has a road node (+0x2C → +0x0C) within 30 m
  (0x7A56B8) of it: then, the first faster than 10 m/s and its velocity
  along the node's direction (0x401B20, by the controller's +0x10) above 0,
  the first's place along it less its own (0x40CBC0, 2D); the first at 10
  m/s or less and the node less its position dotted with its velocity
  under 0, minus that distance. The order (0x5EEC90 with 0x5EEC10: in state
  7 unplaced racers first, by +0x24 ascending) makes the smaller first,
  and 0x6076E0's state-7 block takes the first's +0x24 off the others
  (0x607DBB); one more than 300 m (0x7A56B4) behind — 300 + 4.9 m
  (0x7A5F50) with the units in feet (0x83AA1A = 0) — gets status 7 (an AI
  racer the 2 s hold; the new controller 0x4260B0 of that block is only
  taken online, split-screen or outside state 7); with fewer than two
  racers left, the first gets status 6 (0x606CC0(0, 1, …, 6); an AI winner
  the same hold).
  0x5EEC90 numbers the places (+0x0A).
* **The end**: 0x5EA540 (the finish tick +0x50) holds 0 in race state 7
  outside mode 4, so 0x60B5CD calls 0x60AAB0 at once, which in state 7
  (offline, not mode 4) calls 0x5F84A0: race state 0, +0xBE8 = 1, laps 1,
  in game mode 1 0x541330(player status = 6), player +0x250 = 0 for the
  humans, the racers past the humans dropped (0x5F7960), then 0x42EFB0
  (NONE when 0x89E7C0 = 2 and 0x89E7C4 = 0, else DEACTIVATE_OPPONENT),
  whose first call 0x42CD60 gives every AI racer out (world list from
  +0x14, 0x5ED440) whose goal (+0x6FC) is not of type 1 the goal "Win road
  race" (0x42AA20(1) → 0x40CA20, type 1, vtable 0x784768; 0x402070 ends the
  old goal and its action). Goal 1's vtable is goal 6's (0x784BA8) but for
  slot +0x24 (0x50D510: no target car) and slot +0x28 (0x41BCF0 → 0x413CC0,
  the ordinary racer junctions instead of 0x413F50's). With race type 0 the
  catch-up (0x4172D0) is 0.75 and 0x41C5E0 takes the offline racer's speed,
  no cap (slot +0x4C 0x40DDD0 = 1e9). The cars drive on with it until
  0x40BF60 / 0x40BFB0 park them. **Observed** (outrun_brake_p14 b1, b2, b3,
  b6, b9): goal 1 on the end's frame (action 0), action 6 from the next
  think, `ai_catch` 0.75, target 43 / 31 / 39 / 40 / 39 m/s; parked (goal 0)
  about 2 s later. Clone: `EncounterGoal::RoadRace` for every car out from
  the end (`Encounters::think` while `ended`); it had kept goals 5 / 6 at
  the outrun's pace (test `outrun_cars_take_goal_1_after_the_end`). 0x541330 (career): a win adds the stage
  definition's +0x02 to the bank (career +0x7134) and counts in the stage
  record's +0xDC; a loss takes it off, at least 0; then 0x529BE0, 0x533AA0
  (the magazine triggers, condition 2 reads +0xDC), 0x5110C0, 0x533320,
  0x4F92A0.
* **HUD**: 0x5F33C0 in race state 7 sets high 0x40 (the getaway meter) and
  no leader board (0x800000 only outside state 7). The meter (0x4F87E0:
  +0x28 Getaway_Distance_Bar, +0x2C Distance_Number, +0x30
  Out_Run_Message, +0x34 Distance_Text, +0x38 the 0x8CA76279 group;
  Backing and Lines only listed) updates in 0x4DABD0: hidden, 0x8CA76279
  INACTIVE and (outside state 7) +0x44 = 0; shown, 0x4C9780 counts +0x44
  to 4: at 3 Out_Run_Message gets "Outrun Challenge Accepted^Lose Them!"
  (0x783202B0) when the player's place is 1, else "…^Catch Them!"
  (0x18B97380), shown with ZoomIn; at 4 the group runs ACTIVE. The value
  (0x4C9450): from −limit (0x5EEEB0: 300, or 304.9 in feet), each other
  racer's leader-board row, trunc((its +0x24 − the player's) × 4000 + 0.5)
  (0x5F82E0 in state 7) × −0.00025, taken when positive and larger, or
  negative and larger while the value is ≤ 0. At most 0: Distance_Number
  AHEAD, the bar 0xFF30CF30, and "You have the Lead!" (0xBFD1936F, ZoomIn)
  when the last value (+0x24) was over 0 and the race status state is 4;
  else BEHIND, 0xFF950000 and "You Lost the Lead!" (0x9B2AF3C7) after a
  value under 0. Distance_Text "Meters" (0x6BB2D6AF) or "Feet"
  (0x001578C3); Distance_Number "%$d" of minus the value (× 3.28,
  0x790AE4, in feet); the bar 0x4C67E0(bar, |value| / limit, 1, 99, 1) —
  the texture's width W (256; opaque from about x = 100): the clamped map
  0x4A9D60 from (W − 99)/W to 1/W gives u1 = 1 − f and width (1 − f)·W,
  the left edge kept. The laps element in state 7 (0x4AA202 → 0x4AA352)
  hides NumberLaps, LapNumber, the first LapCounterBacking, Lap_Time_Title,
  Lap_Time_Text and LapText; the position (0x4EBCBB) counts the racers less
  those done ("/2"). Race over (0x4C7F00): the backing runs 0x36819D93 in
  state 7 instead of RACEOVER; status 6 "You Won the Outrun!" (0xDC089C64),
  "Your time:" and the time, in a career (not 0x4AE8D0, 0x457BA0) "Bank
  Reward:" (0x5D10F6AF) and 0x500C60(stage) ("%s\n%s %s\n%s %$d"); status 7
  "You Lost the Outrun!" (0x5D1621B2) and the time. 0x5FFF60 keeps the race
  over up for 3 s from the finish tick in race state 0. The encounter icon:
  0x4C9170(9, 0) on the free-roam events element (+0x60, 8 slots of 0x14:
  state, type…) — 0x4C8B80 shows type 9's icon 0x33B5791A (0x4ABE50, also
  type 10) on 0x3B1B624A (BLINK), 0x0A729B1B (PULSE) and 0x50910822 with
  0x6FFF0767 (PULSE); 0x4DA9A0 blanks 0x50910822 (0xEA570722) and hides the
  rest each frame first; 0x4DB890(0) → 0x4DAB80 removes it.
* **Clone**: `src/outrun.rs` (the tests above, unit-tested) and game.rs
  `Encounters` (career free roam, not Free Run): three cars drawn as
  0x53F010 does, with the names drawn (not shown) and the looks of **AI car
  looks** (from a copy of the clone's shared random state, whose sequence
  is not the game's here), the career's side, the manager's states, placement, engage test,
  parking, the key (Enter Race / Shop), the race, the end and the bank,
  the meter, race-over, laps / position and icon above (`hud.rs`); the
  selftest's `selftest_outrun` loses and wins one, and `--shot outrun` /
  `--shot encounter` show the meter and the icon. Stand-ins
  (**inferred**): the player's road node is the nearest control point of
  RoutesFreeRoam (as the traffic placer's), with 0x40A390's flag rule
  (flip when the travel — velocity above 10 mph, else heading — is over
  135° off, dot < −0.707); an encounter racer's road node is its route
  walk's current point (`RacerRoute::node`); the road search is 0x415130 with
  its racing-line argument (`racer_search`, see Placement → Racing-line
  search; ported 2026-10-31); 0x5F0FC0 is the 2D distance to the player's car;
  the racers drive with the race rivals' AI (route, speed target, driver,
  GetUnstuck) on RoutesFreeRoam, with its racing speeds for their best
  class; the first leader is the challenged car (the game's first frame
  measures against the race order's first, the first encounter racer);
  the race status state counts as 4 (racing) for review messages; no
  countdown shows. Stage 0 offers none (its intro flags are not modelled).
* **Observed under Wine** (race9, vanilla, wined3d; the clone-written
  stage-2 save <work dir> as the profile, Resume
  Career → the garage → Return to Explore Mode, the SMS inbox closed;
  memory through the peek loop, `race9-tmp/enc9.py`, `chase9.py`): the
  manager (race status +0xC40, embedded: +0x00 the state, +0x04 the mode
  = career +0x1AE0 = 7) sat in POLL_CAREER while career +0x1AE4 counted
  down from 30 s, then went to TRYING_TO_ENGAGE with the three slots in
  mode 1 and their cars placed and driving at 36–46 mph (one held parked).
  With the player's car put 12 m behind slot 0's car on its heading at
  its velocity (rigid body, see Race8 checks), career +0x1AE8 went to 2
  (the prompt) and the Enter key engaged it: 0x89E7B0 = 7, manager state
  4. **Start** (`compare/race116/a private Wine copy.jpg`): "Outrun
  Challenge Accepted" / "Catch Them!" with a large "GO!" below it (Time
  0.30), the getaway meter at the top left reading "-69" Feet (red),
  "2nd /2", Time, the minimap, no leader board; 1.4 s on the message has
  gone (-79 Feet, Time 1.75). With the player put 30 m ahead: "1st /2",
  the meter "51" Feet in green. **End** (`a private Wine copy.jpg`): moved
  320 m ahead in a straight line (off the road), the run ended at once —
  0x89E7B0 = 0, the manager back to 3, slot 0 parked — as a loss: "You
  Lost the Outrun!" / "Your time: 15.80" with no HUD at all (no tach,
  minimap or meter) over a free camera behind the car. (Off the road the
  player's own +0x24 is its 3D distance to the first, so the player, not
  the rival, fell 300 m behind.) The clone's `--shot outrun` shows the
  same message, meter, position and Time, without the "GO!" (traced and
  ported in race10, see **Outrun "GO!"** below) and printed its time ".36"
  where the original prints "0.30": **fixed** (0x576D40: under a minute
  with no whole seconds the template "0.00" / "0.--" at 0x7A0630 /
  0x7A0628 takes the hundredths, `hud::time_text`; signed, it moves
  right whole like a two-digit field). A win was not captured.
* **Outrun "GO!"** (2026-10-31, race10; **verified** in the code and
  disassembly; ported, `hud.rs` countdown block): the start 0x607190
  (race state 0x89E7B0 = 7) sets race status +0x10 and **+4** (the green
  tick, as 0x5FC910(4) does) to now and +8 to 0. 0x6000A0's tail
  (0x60052C) gives the HUD high 0x20000 — the countdown element 0x4F7340 —
  whenever the flow state *0x8900D8 ≠ 7 and ≠ 6, player +0x234 = 0 and
  0x89E7B0 ≠ 0 (or 0x457BA0), so in an outrun. 0x4AB5B0 then asks
  **0x5EEA50**: with +4 set it returns 4 while (now − +4) / 4000 ≤ +0xC,
  else 0; with +4 unset and +8 set 3, 2, 1 by thirds of 3·+0xC, then 4.
  On 4 it shows "GO!" (0x535) with **0x5EEB10**'s fraction (now − +4) /
  4000 / +0xC (1 past the end) as the scale 2f + 1 and the alpha
  sin(90°·f). +0xC is **0.9 s** (0x5FC740 at 0x5FC7DA from 0x7A5690 =
  0x3F666666; free roam plays no start scene to change it). So an outrun
  shows "GO!" for 0.9 s from the challenge (the Wine capture: present at
  Time 0.30, gone by 1.75). The clone's earlier rule (no countdown in an
  outrun) was an inference and is removed.
* **Outrun end under Wine, round 10** (2026-10-31, race10, vanilla,
  wined3d, `a private Wine copy`, the stage-2 save s2a.sav; scripts
  `race10-tmp/win.py`, `tprival.py`, `chase9.py`, `pin2.py`). **Win**
  (`compare/race116/a private Wine copy.png`): with the player in the maintainer
  ("1st /2", the meter 149 Feet in green) and standing against a kerb
  while the challenged car drove off the other way, the run ended once
  it was ~356 m off: "You Won the Outrun!" / "Your time: 56.09" / "Bank
  Reward: 100" (stage 2's bank, no '$') over the chase view with no HUD
  at all; 3 s later the free-roam HUD was back (Bank, minimap, the zone
  name "City Center", a new SMS). The bank (career +0x7134, 0x861E74 in
  memory) went 1,250 → 950 over three losses and back to 1,050 with the
  win: −100 per loss, +100 per win, as 0x541330. The clone's `--shot
  outrun_won` (the player leading 1 s, then the car put 320 m back) shows
  the same three lines at the same place and size
  (`race10-tmp/sbs-outrun.jpg`), and `--shot outrun` now the "GO!" under
  the challenge at 0.36 s, as the original's at 0.30. Three engaged
  outruns ended as losses when the teleports left the player behind ("You Lost
  the Lead!" in green over the meter at -98 Feet, then "You Lost the
  Outrun!" / "Your time: 7.77" with no HUD over the chase view, the car
  on the road). **Observed**: after a rigid-body teleport of the
  player's car the camera shows a low side view of the car (the
  "Your time: 12.05" loss, `race10-tmp/w/v4.png`) — the same view as round
  9's Street X finish, which was also taken right after a teleport; so
  that side view is the camera's answer to a jump of the car (**inferred**;
  the director 0x5FB060 changes no camera at a finish, see After the
  finish), not a finish camera.
* **Open**: the encounter goals (AIGoalOffsetPursuit 0x40DBA0 with
  0x42A860's offsets before the race; 0x4285A0 AIGoalChaseCar and 0x428510
  goal type 6 in it; the action selection reading controller +0x6FC);
  0x60F540's car
  +0x554 → +0x208C; the record set-up 0x61C290 and 0x6373F0; 0x532E40 (0x529B60,
  0x511000 (Rachel's line, ported) at the start and 0x529BE0, 0x533320,
  0x4F92A0 at the end (0x5110C0, Rachel's line, ported); 0x85EF50 +0x1C and 0x838590 → 0x5010E0 in the key; the stage 0
  flags 0x86347D / 0x86347E; the minimap's AI racer markers
  (AIRACERINDICATOR_n: the constructor 0x4F73E0 lists them, what places
  them is not traced); the wrong-way test's route triggers in state 7
  (0x5DED20, see the HUD's Wrong way); 0x410FC0's radii by car kind (the
  clone: 30 m to traffic, 300 m to the other encounter racers).

* **Live comparison, 2026-11-04**: see [outrun.md](../outrun.md). Traced
  and ported: encounter cars come back on the MeloMvr (0x5F6D60 →
  0x5F4410 = 3 for owner kind 3) with their tank; goal 9's speed cap
  0x40DC00 (40 mph far, 60 mph within 100 m, or the pursuit table × the
  player's speed); a nitrous shot's clock runs on the MeloMvr (its torque
  stage +0x198 is never set there).
