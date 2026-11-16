# Nitrous in Street X and drift races (2026-11-04)

Player report: "for street x is it allowed to use nos? I still can trigger it".

**Answer (verified):** no. In the original, a car has no nitrous in a drift or
Street X race, even with a nitrous package installed. The key does nothing: there
is no shot, no boost, no flames and no sound. The gauge is hidden too.

## Trace (SPEED2.EXE)
- The key handler, JOY 0xA3 / 0xA4 → 0x602BE0 → 0x6034C6, has **no race-type
  test**. A press starts a shot only when 0x5EB570 → 0x59FAB0 sees a tank fraction
  (0x5925C0) above 0. Then 0x59FB00 runs, followed by SPRAY SHOW (0x56ED20) and the
  rumble. Everything else (the torque at 0x594310, the grip at 0x5A5106, flames and
  sound) reads the running shot's stage (0x59FA40), so with no shot none of it runs.
- The tank: 0x591CE0 fills car +0x454 (size) and +0x45C (left) from the tuned
  block +0x2A0 (capacity, s × 4000) and +0x2A4 (shots). It runs at the car set-up
  (0x59F9DA) and when the size no longer matches the block (0x5F6C30 → 0x5F6C8C).
- The race set-up: 0x5F6C30 → 0x639CF0 → 0x5B4FD0 picks the mode for 0x5B4DD0:
  - 3 for drift (0x89E7D9 / 0x890104 / 0x890114, not in a drag race);
  - 10 for Street X (0x89E7E3 / 0x89010C);
  - 2 for drag.

  0x5B4DD0 then runs the race tune:
  - mode 3: 0x5A7760, table 0x7FC2E4;
  - mode 10 or 5: 0x5A77D0, table 0x7FC33C;
  - mode 2: 0x5A7840, table 0x7FC394.

  0x5A7760 and 0x5A77D0 end with `mov [block+0x2A0], 0` (0x5A77BB, 0x5A782B).
  The drag tune does not.
- So the tank is 0 s. 0x59FAB0 returns 0 and no shot can start. The style refill
  0x592650 scales its amount by the size (0), so it never fills the tank either.
- The gauge rule (0x600231: no gauge in drift 0x401910 / Street X 0x401930) is
  separate, and the clone already had it (`hud::features`).

## Observed under Wine
Setup: copy `$OB_PRIVATE/wine/runs/<run>` on display :213, profile `career_end_won.sav`.
In Quick Race, a Peugeot 206 got the Customize → Nitrous Oxide → Street Package
(the "My Cars" entry). Readings came from `[[0x8900B4]+4]`, the player car. The
script and logs are in `evidence/streetx-nitrous/`.

| Race | block +0x2A0 | +0x2AC | size / left (ticks) | Alt held 2 s at about 13 m/s |
|---|---|---|---|---|
| Circuit | 2.0 | 0.5 | 8000 / 8000 | shot starts (+0x460 = clock), tank empties in 2 s, 15.9 → 26.3 m/s |
| Street X (0x89E7E3 = 1) | 0.0 | 0.5 | 0 / 0 | nothing (+0x460 stays 0), 14.9 → 19.9 m/s |
| Drift (0x89E7D9 = 1) | 0.0 | 0.5 | 0 / 0 | nothing, 14.9 → 19.9 m/s |

The career Peugeot (no nitrous part) and the stock 206 read 0 everywhere.

## Clone
`race::apply_tune` now clears `nitrous_capacity` for `RaceMode::Drift` and
`RaceMode::StreetX`, as 0x5A77BB / 0x5A782B do. A race places the car again
(`Drive::place` resets the state). The tank is then filled lazily from the tuned
handling and is empty, so `Tank::start` refuses: no torque, grip, flames or sound.
The HUD gauge rule is unchanged.

Tests: `race::tests::drift_and_street_x_tunes_empty_the_tank` and
`race_setup_has_no_nitrous_in_drift_and_street_x`.

Open:
- Mode 5 of 0x5A77D0 has no caller found through 0x5B4FD0.
- The 0x5F6C8C refill on a size mismatch is still filled once in the clone, as before.
