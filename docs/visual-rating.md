# Visual rating (2026-11-05)

The car's visual rating (the stars, 0–10) and what it drives, compared
with the original. **Verified** = traced in SPEED2.EXE (disassembly /
REA), **observed** = read from the running original under Wine,
**inferred** / **open** as marked. Code: `PartDb::visual_rating`
(src/formats/parts.rs), `car::ride_visual_rating` (src/car/; the game's
`Game::visual_rating_car` now calls it). Tools: the maintainer's tool `rating.py`
(+ `rating.s`, the probe cave), the maintainer's tool `fixture.py`,
the maintainer's probe `visual_rating_sweep`. Evidence: the maintainer's private captures.

## The formula (verified)

* **0x528420** (career rating; ECX the career 0x85AD40): the current car
  (0x503680 on the car list 0x83AD90 with career +0x8740; none → 0.0,
  0x5284BE), its ride built on the stack (constructor 0x40C910, the car's
  vt+0xC fill = the record's parts with the derived slots, docs/formats.md
  0x503880), then 0x5234D0 on it.
* **0x5234D0(ride)**: the override 0x7FA290 (−1.0, never written) is
  returned when it is not −1 (never). A widebody is fitted when slot 6's
  part has a level (byte +5 & 0xE0). Points = the sum over slots 0..0xA9
  of 0x5142E0(car type, slot, part), skipping slots 0x4E–0x5C (the vinyl
  colours), 0x1E, and with a widebody slots 1, 2 and 0x0B (bumpers,
  skirt); an empty slot (0x610300: ride +0x590 + 4·slot = 0) adds
  nothing. Rating = linear interpolation in the (rating, points) table at
  0x7FA238: (0, 0), (1, 500) … (10, 5000), i.e. points / 500; at 5000
  points or more 10 (0x7FA288 / 0x7FA28C).
* **0x5142E0**: 0x501860(category 0x501D70(slot, part), level = byte +5 >> 5);
  the paint slot 0x3F with the car type's stock paint (+0x84C of the 0x890
  record at [0x8A1CCC]) scores 0; −1 scores 0.
* **0x501860**: category 1..0x56 only; level 0 → 0; level 4 (uniques) →
  400 for categories 0x0B–0x0D, 0x22, 0x4A, 0x4B, 1000 for 0x16, 100 for
  0x3E, else 0 (jump table 0x5018C4 / 0x5018D4); levels 1–3 the signed word
  at +4 / +0x10 / +0x1C of the category's 0x28-byte record (0x501750; the
  career chunk 0x34A1C at [0x83845C]); others 0.
* **Rounding (fixed).** The interpolation runs on the x87 stack at 24-bit
  precision (Direct3D's default control word): `k + (points − 500k) / 500`
  with the division and the add each rounded to f32. The clone divided
  once, `points / 500`: 269 of the 5 000 totals then differ by one ulp —
  the sweep found them (1 680 points: the original 3.3600001, the clone
  3.36; also 1 180, 1 320, 1 430, 1 445, 1 695, 3 985 points; 2 110 rides
  of three saves). Now `parts::rating_from_points`. No visible effect (the
  stars floor the value, the DVD thresholds are whole ratings), but the
  bits now match, and the cover pay's ftol sees the original's value.
* Otherwise the clone's port already had all of this (`PartDb::visual_rating`,
  `CostTable::points`, `PartDb::category`). It clamps every negative point
  value to 0 where the original zeroes only −1: the same, the table holds
  no other negative value (`visual_rating_sweep costs`).

## Under Wine (observed)

the private tool `rating.py OUT SAVE[:PROFILE]... --sweep` boots each save in
the private Wine copy, drives it to free roam and installs a probe cave
(`rating.s`, at 0x782B00 in place of the main loop's frame-time call
0x5812EC, game thread): it builds the career ride as 0x528420 does and
rates it with 0x5234D0 and with 0x528420 itself, then rates batches of
rides written into a VirtualAlloc'd buffer (ride +0 the car type, +0x590 +
4·slot the part pointers into the loaded part table, found from the ride's
own pointers). The sweep: the career ride with each slot's part replaced
by every part `PartDb::parts_for` lists for the car, each fitted slot
emptied, a widebody with six parts of each bumper / skirt slot, every
slot's last part. `visual_rating_sweep compare` rates the same parts.

* 14 saves (the finished-career save SKYLINE, a community save RX7 and LANCEREVO8; fixtures
  garage 240SX, the community-career body saves (RX7 / ESCALADE / LANCEREVO8) and its after-buys save,
  the finished-career save, the stage4_*, stage5_* and career_end_won saves, stage1_win_streetx; career_start has no car:
  0x503680 gives none, rating 0; mycar's save stopped at a stage movie):
  **293 165 rides, all bit-identical** after the rounding port
  (`ratings-sweep.txt`). 0x528420 and 0x5234D0 on its ride agreed for
  every save.
* The clone's own path from the save (`Profile::from_native` → the ride
  with its derived slots → `car::ride_visual_rating`) gives the
  original's 0x528420 bits for every save (`ratings-saves.txt`). The native
  ride differs from the clone's rating ride in slots 0x0D (ENGINE) and
  0x1B (brakes) only, for three saves: 0x6399E0 sets them from the
  performance packages (docs/formats.md "Brakes from the performance
  parts"; the clone fits them for drawing, `car::perf_visual_parts`, not
  in the rating ride). Their categories score 0 at every level (each part
  swapped there left the rating unchanged), so the rating is not affected.

## Tests

* `tests/visual_rating.rs::native_rides_rate_as_the_original` — fixture
  `tests/fixtures/visual_rating/rides.txt` (the maintainer's tool `fixture.py`: 11
  saves' career rides and 1 884 swept rides, one per slot and native
  rating, the 1-ulp cases included), bit for bit.
* `tests/visual_rating.rs::native_saves_rate_as_the_original` — eight
  fixture saves through the clone's save path against the original's
  0x528420 bits.
* `formats::parts::tests::rating_rounds_as_the_original` (fails with
  `points / 500`).
* `tests/frontend.rs::visual_rating_from_parts` uses `rating_from_points`.

## Open

* Windows: the 24-bit x87 mode is Direct3D's default and was observed under
  Wine's d3d9; not checked on Windows (**inferred** the same).
* The rating ride's slots 0x0D / 0x1B (above) are not filled from the
  performance parts in the clone (no effect on the rating).
* Take Picture's flash (0x501250 → 0x5DBD80(1), see nis-mover.md
  "SCREEN_FLASH") is not ported.

## What the rating drives (verified: every caller)

0x528420 has two callers, 0x5234D0 three more; nothing else reads the
rating (so sponsors and their terms do not depend on it):
* 0x5331D0, after a part is fitted: the stage's DVD covers whose
  required rating (milestone +0x3C) is ≤ the rating are offered, the others
  withdrawn (fcomp / `test ah, 0x41; jp`: offered on less-or-equal) —
  clone `Profile::offer_covers` (`rating >= m.rating`), the same;
* 0x541220, a photo shoot's pay: milestone +0x26 + ftol(stage def +4 ×
  rating + 0.005) — clone `Profile::cover_reward` (f32 multiply and add: the
  x87's 24-bit arithmetic, see Rounding);
* 0x53D880 (UI_Status_Career's stars, +0x4C) and 0x55B1B0 (a shop's
  current +0x4C / preview +0x50 after each fit) — clone `rating_stars`,
  `shop_rating_quads`.
