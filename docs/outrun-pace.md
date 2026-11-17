# Outrun rival pace and the drag rivals' nitrous (2026-11-05)

Issue: the outrun racers were too easy. The racer-traffic parity (world-systems-parity.md §12) had measured the clone's
rival at 26–41 m/s and 21–40 s to pull 300 m ahead against the original's 35–47 m/s and 9–14 s,
but over different starts (eight native outruns against twelve traffic seeds of one start) and
with the player's throttle held, so the player's own crashes decided most of the outrun's length.
This work compares like for like: the clone starts from each native outrun's frame-0 pose (both
cars, the rival's model and tank, now also the world tick) and the player is scripted the same in
both games. **Verified** = traced in SPEED2.EXE (Ghidra / `speed2.dis`); **observed** = read under
Wine (copy `a private Wine copy`, display :318, wined3d, the game's own frame cap, one instance at a time);
**inferred** / **open** as marked. Evidence: the maintainer's private captures; raw runs in
`<work dir>/` (`nat/`, `runs/`).

## Verdict

The rival's speed target was already native; its **car** was not. On the MeloMvr (the mover every
encounter racer drives on, outrun.md) the clone gave the car twice the original's air drag at full
gas, so above ~30 m/s it accelerated 3–15 % slower and topped out lower. With the drag fixed the
rival's speed at 1 / 3 / 5 s from the challenge matches the original's within 0.1–0.3 m/s in every
like-for-like start that did not meet traffic differently (table below), and its pace near traffic
moved to the original's (41 vs 39 m/s, before 33). Three more native faults were found on the way:
the AI's nitrous rpm band was inverted (all AI racers), a drag rival's shot never ended before the
tank was empty, and the MeloMvr ignored the race catch-up k. The harness also started the clone's
outruns with the world tick ~20 s short of the original's, which delayed the rival's first nitrous
by 5–7 s.

## 1. The speed target (verified; no change)

0x41C5E0 (goal slot +0x34) in race state 7 (0x89E7B0): the route's speed at the driver's
distance (0x411DD0; 30 m/s when it reads 0) × 0.84 for place 1 (0x78357C[stage], all seven 0.84) or
1.0 otherwise (0x783598), no distance jitter and no skill factor (only the jitter's RNG draw),
× the catch-up +0x770 (`piVar2[0x1DC]`), then the least of that, the action's +0x1F0 (unless +0x1FC)
and the goal's slot +0x4C — for goals 5 and 6 (vtables 0x784B58 / 0x784BA8) 0x40DDD0 = 1e9
(0x7847C8), i.e. no cap. **Observed**: `ai_catch` (+0x770) = 1.0 in every outrun frame (the catch-up
does not run in career outruns), goal 6 while leading, the driver's target (+0x230) 53–62 m/s on
the starts' straights; the clone's `ai_target` (now recorded for encounter cars) reads the same
values to 0.1–2 m/s on the same road (`b1-accel-*.txt`). The skill row enters only the nitrous rows
(+0x568…+0x574) and the unused jitter.

## 2. The MeloMvr's aero and catch-up (verified; fixed)

* The MeloMvr frame 0x5AADA0 fills its aero object (built at 0x5B6D10: [0] car +0x260, [1] body,
  +0x08 / +0x0C 1, +0x10 / +0x14 0, +0x30 1) every frame: **+0x10 = the AI driver's gas** (+0x20C,
  0x5AAFD2) and **+0x14 = car +0x424 × 0.25** (0x5AAFE1..0x5AAFF2), then 0x5A3390. The drag of
  0x59E630 (shared with the physics mover) is (2 − t)·c·(|v| ÷ k)·v (0x59E677..0x59E6AC, 0x7FBED0 = 2)
  at the centre of mass or, with +0x14 ≥ 0.5, 0.1·(1 − t) m below it; the downforce's front value
  1 m ahead and rear 1 m behind with a wheel down (40 / 41 % of the sum only with none). The clone's
  `melo.rs` had t = 0 (2·c, twice the drag at full gas) and always 40 / 41 %. Now `aero_drag` and
  the downforce follow the physics mover's code.
* 0x5A3390 (arg car +0x468 = max(controller +0x770, 1); 0x7FC0A0 = 1, no writers, so the arg is
  used): k into aero +0x08, body +0x208 (top speed × k), the grip +0x140 (× 0x7A1998 = 1.0 in its
  special case) and the chassis' **+0x13C**, which the spin rates 0x598AD0 read: the torque curves at
  rpm ÷ k (0x598B26), the drive × (1 + 3 (k − 1)) and the driven wheels' lever ÷ 1.25 (1 + 2.5 (k − 1))
  (0x598C6E..0x598CB7, constants 0x7A1990 = 3, 0x7A1994 = 2.5, 0x7A1964 = 1.25). The clone held k = 1.
  Outrun rivals have k = 1 (observed), so this changes race rivals on the MeloMvr with Catch Up on.

**Observed effect** (`outrun_brake_p14` b1, MUSTANGGT, the player braking): native / clone before /
clone now, m/s² over 0.5 s in 4th gear at 41–47 m/s: 3.90 / 3.48 / 3.90, 3.77 / 3.37 / 3.77,
3.69 / 3.19 / 3.69, 3.89 / 3.11 / 3.89 (`b1-accel-old.txt`, `b1-accel-new.txt`): equal to the
second decimal until the crest at 6.5 s.

## 3. The AI nitrous band (verified; fixed — every AI racer)

0x40D850 (controller slot +0xA0 for drag, inside 0x416F70 otherwise) wants 0x402590 **false**.
0x402590 clamps the car's rpm (+0x440; 0x7FBED8 = 100 selects it) into [A, A + 0.95 (B − A)]
(max with A at 0x40261C, min with the top at 0x40262F) and returns 1 when the rpm differs from the
clamp (0x40264A): so a shot may run only **inside** the band. A / B are 0x5946F0 / 0x594680(gear, 0)
on the physics mover (car +0x28 = 1) and the chassis' down / up rpm +0x1C0 / +0x19C on the MeloMvr
(3, 0x4025C8); any other mover passes. formats.md and the clone had "outside" and skipped the
MeloMvr's band. With it inverted an AI shot could start only above 95 % of the band and every
up-shift failed the test.

## 4. A drag rival's shot ends when 0x40D850 fails (verified; fixed)

0x42ABD0's non-full think (0x42AD16): for an AI racer (car +0x14 → +4 = 2) with a shot running
(0x59FA40) and 0x40D850 false, slot +0xB4 (0x408430: true in a drag race, 0x89E7D8 / 0x890108, else
after the row's +0x570 s) lets 0x592510 end it and slot +0xA8 (0x41E500) stamp +0x7AC
(0x42AD24..0x42AD5E). The clone's drag rivals' shots ran until the tank was empty (`drag_nitrous_end`
now). In a drag race the request is 0x40D850 itself, so the next shot starts when it passes again.

**Observed** (the drag-finish pass's stage-4 drag replays f2 / w4, `drag-replays.txt`, native / the drag-finish pass clone / now):
the tank goes out in several shots as natively (w4 Sentra 24000 → 19692 → 9756 → 0 natively,
24000 → 18735 → 11740 → 0 now; the drag-finish pass: one shot to 0 by 15 s); all three rivals now reach the line in
both replays (the drag-finish pass: two of three stopped — the ECLIPSE scraping a wall from ~1342 and the WRX on the
old lane's end obstacle; their positions follow from the speed profile, the lane convergence itself
was **not** traced). f2: the PEUGOT finishes 30.74 s (native 30.63, the drag-finish pass clone: did not finish), the
player 30.52 (native 30.69). w4: the clone's rivals 32.37 / 32.38 / 33.76 (native 31.95 WRX, 33.09
Sentra, 240SX totaled at 11 %: the native WRX is 1.8 s faster, the clone's 240SX does not crash —
**open**).

## 5. Harness: the world tick at the challenge (fixed)

The world load sets 0x865188 to 4000 in both games (0x576D00); the controller ctor 0x4260B0 sets
+0x7A8 / +0x7AC / +0x7B0 to 0, so the first shot's gap (row +0x568; ~10 s for b1's rival, from the clone's first shot at tick 40862) counts from the
load. The original's challenges came 19–50 s after it (`clock` 76395–198950), the clone flow's after
~4 s, so the clone's rival fired its first shot at 6.6–9 s instead of 0.5–2.3 s. `parity.py` now
writes `tick N` (the original's frame-0 tick) for an outrun and `start_outrun` puts the clone's world
tick there before the challenge (`PARITY_NO_TICK` keeps the old start). First tank drop now: 0.5–2.4 s
in both, run for run (table). In play the clone's tick runs from the load as the original's: no
game change. (Nitrous on the MeloMvr adds no torque, outrun.md §6, so this changed the tank, not
the pace.)

## 6. Like-for-like comparison (observed)

Nine native outruns with the player braking from the challenge (`outrun_brake_p14` b1–b4, b6–b10,
the finished-career save's save, rival goal / target / catch-up / gear / rpm recorded) and the racer-traffic parity's six with the throttle
held (`outrun_traffic_p12` o2–o9); the clone from each one's frame-0 pose, 2 (before) / 3 (now)
traffic seeds. Full table: `outrun-table.md`; here the mean of the seeds.

| start (player) | rival | rival speed 1 / 3 / 5 s: native | before | now |
|---|---|---|---|---|
| b1 (brake) | MUSTANGGT | 25.8 / 37.1 / 45.1 | 25.6 / 36.6 / 43.8 | 25.7 / 37.1 / 45.1 |
| b2 (brake) | COROLLA | 25.0 / 35.1 / 36.9 | 24.8 / 34.7 / 36.7 | 25.1 / 35.3 / 37.3 |
| b3 (brake) | CELICA | 25.0 / 35.8 / 43.3 | 24.7 / 35.2 / 42.0 | 24.8 / 35.7 / 43.2 |
| b6 (brake) | COROLLA | 22.7 / 33.7 / 41.8 | 22.8 / 33.5 / 40.9 | 23.0 / 34.0 / 42.0 |
| b9 (brake) | ECLIPSE | 24.1 / 35.1 / 43.3 | 23.9 / 34.6 / 42.2 | 24.1 / 35.1 / 43.3 |
| o2 (throttle) | ECLIPSE | 30.9 / 39.4 / 45.7 | 30.6 / 38.8 / 44.2 | 30.9 / 39.5 / 45.8 |
| o3 (throttle) | TIBURON | 25.0 / 35.8 / 43.3 | 24.7 / 35.2 / 41.6 | 24.9 / 35.7 / 42.9 |
| o4 (throttle) | SUPRA | 34.9 / 43.0 / 49.3 | 34.7 / 42.1 / 47.6 | 35.0 / 43.1 / 49.4 |
| o8 (throttle) | COROLLA | 32.0 / 40.5 / 45.8 | 31.9 / 39.2 / 40.3 | 32.1 / 40.3 / 43.3 |

| | native | before | now |
|---|---|---|---|
| player braking: rival escaped (300 m) within 40 s | 9 / 9, mean 11.7 s | 18 / 18, 11.7 s | 27 / 27, 11.1 s |
| … same start, no crash in either (b1, b2, b3, b6): time to 300 m | 7.8 / 9.6 / 8.2 / 8.4 | 8.1 / 9.2 / 9.2 / 8.5 | 7.9 / 9.1 / 9.0 / 8.3 |
| player throttle held: rival escaped within 40 s | 6 / 6, mean 14.7 s | 10 / 12, 15.5 s | 18 / 18, 18.0 s |
| first nitrous shot ends (tank drop), s | 0.5–2.3 | 6.6–9.1 | 0.5–2.4 |
| rival speed within 30 m of traffic, p50 (throttle runs, `traffic-table.md`) | 38.8 | 32.5 | 41.3 |
| rival − traffic speed within 15 m, p50 | 26.5 | 21.9 | 29.7 |
| frames with traffic in the rival's lane 0–12 m ahead | 5.8 | 22.2 | 10.4 |

After 5–8 s single runs part: the route at a junction (goal 6's links 0x41BD00 draw from the world
generator, 0.75 / 0.1 by the distance to the second car — b9: the clone's target fell from 5 s as it
took the turn north at (−500, −1900) and it crashed at 7.9 s where the original went straight; b3: a
car in the rival's lane at (−515, −1901) in some seeds), traffic (each game's own draw), and crashes
on both sides (natively the rival stopped in b4 and b7; the racer-traffic parity's "native 9–14 s" counted those). The
throttle runs' time to 300 m is decided by the player's own crashes (the throttle-held player in the
original met traffic within 2–5 s in o2, o4, o9), not by the rival.

## Tests

`melo::tests::melo_drag_takes_the_gas_and_the_catch_up` (the (2 − t), arm and k terms),
`handling::tests::nitrous_band_is_the_clamp_test_of_0x402590`, `tests/ai_driver.rs`
`melo_nitrous_band_uses_the_chassis_shift_rpm` (asset), game.rs
`drag_rival_shot_ends_when_the_nitrous_test_fails`. `wrong_way_street_x_on_keys` (asset) now leaves
out the second after the bot's car really ran backwards: with §3 a rival knocked the bot round on
4607 variant 1 and its U-turn kept the native flag (100° with `wrong_way`'s slack) for 4 frames past
the test's 72.5° (the locator's road there is 33° off the test's polyline direction).

Race rivals: §3 changes every AI racer's nitrous (they now fire inside the band, i.e. mostly between
shifts, instead of only above 95 % of it), and §2's k changes race rivals on the MeloMvr with Catch
Up; neither was compared race by race against the original in this work (**open**).

## Open

* Win rate against a scripted player that can win: neither a braking nor a throttle-held player
  outruns any rival in either game; a road-following bot was not run natively in this work (the outrun study's
  keyboard bot could not win either, outrun.md).
* The MeloMvr's landing after a crest: b1 native 6 → 5, clone 6 → 4 at the same frame (wheel spin
  at touch-down; the gearbox 0x5AB1A0 itself matches the port).
* (Done since: after the outrun ends every car out takes goal 1, "Win road race", at three quarters of
  the offline racer's speed, 0x42EFB0 → 0x42CD60, and a rival whose nitrous shot runs holds the
  throttle, 0x421F45; formats/ai.md, Outrun "The end" and AI driver "Nitrous hold".)
* Drag (§4): the native WRX's 1.8 s, the 240SX's crash, the rivals' lane convergence
  (drag-stage4-parity.md 8.6).
