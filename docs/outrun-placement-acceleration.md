# Placement acceleration clear versus world snapshot lifetime

This follow-up to `9cc2666` audits the acceleration fields only. It does not
replace the clone's road-node recovery placement or claim a complete native
teleport/mover implementation.

## Native call paths and definite clear

`5FBD70` (full teleport) calls `5F52D0` at `5FBD84`, then route/controller resets.
The route-trigger helper `5FBE10` reaches it at `5FBEA8`. The human reset path
`6034BA -> 6009B0` can use either `5FBEE0` (`600DBA`) or `5FBD70` (`600E01`).
Racer recovery consumed inside the car update follows `6013AF..601519 -> 5FBEE0`.
`5FBEE0` calls `5F52D0` at `5FBFDB`. Thus both placement helpers reach the same
body reset, irrespective of which route/position strategy supplied the pose.

`5F52D0` first saves the mover, selects mover 0, and at `5F52EC..5F52F1` calls
`59F890` with `ECX = car + 0x40`. The following stores are unconditional:

| Native stores | Body offset | Car field |
|---|---|---|
| `59F896..59F89C` | `30/34/38` | velocity `70/74/78` cleared |
| `59F8A4..59F8AA` | `40/44/48` | acceleration `80/84/88` cleared |
| `59F8C0..59F8CC` | `D0/D4/D8` | spin `110/114/118` cleared |
| `59F8D5..59F8E1` | `E0/E4/E8` | angular acceleration `120/124/128` cleared |

Placement subsequently installs its new velocity through `592170` (`5F536F`).
This makes preserving the *published* pre-reset acceleration incorrect at the
body reset boundary. The saved outer-world velocity `car+520` is a separate
field; the direct stores in `59F890` do not reach it. `5FBBF0` updates that saved
velocity only for positive dt, and `604750` uses it to publish acceleration only
for positive dt. A reset before that snapshot and a reset after it therefore
have different interval semantics.

The complete `5F52D0` also calls mover, driver and controller routines. This
investigation does **not** claim that every downstream callback preserves the
saved field, nor that the original full teleport has been executed in the
oracle. Establishing the full callback graph or a live placement capture remains
necessary before widening that retention claim.

## Bounded original-code probe

internal evidence executes unchanged
instruction prefixes `59F890..59F8AD`, `5FBBF0..5FBC66`, and `604750..60483A`.
Harness epilogues stop before unrelated body setup, world callbacks, and race
events respectively. Synthetic car storage has distinct acceleration, current
velocity, and saved-velocity sentinels. The harness explicitly selects PC24 and
runs in an owned headless Wine prefix with guaranteed wineserver cleanup.

The 36 cases cover both reset-before-snapshot and reset-after-snapshot, three
velocity transitions, positive .05 / 1/60 dt, signed zero, negative dt, and NaN.
The checker verifies all 12 output words per case: acceleration and saved
velocity immediately after the reset prefix and after the final publication.
It confirms unconditional acceleration clear, positive-dt snapshot/publication,
and zero/nonpositive/unordered preservation of the now-cleared vector. The
caller ordering in these synthetic cases is controlled by the harness; it is
not evidence that the live game takes both orderings at a particular call site.
Hashes, exact-entry disassembly, and result data are retained in
the maintainer's private captures.

## Bounded clone correction and remaining phase differences

`reset_encounter` previously edited pose/velocity while leaving
`world_acceleration` stale. It now explicitly clears that vector. The regression
first failed with sentinel `(11,22,33)` surviving recovery; it then verifies zero
immediately after actual recovery, zero-dt retention, and that an independently
captured interval token still publishes its original velocity delta.

Manual `Drive::place` / `reset_to_road`, race route-trigger placement, and ordinary
`reset_rival` already create fresh `CarState` values whose acceleration is zero.
No blanket acceleration-retention rule is added to those paths. Tank, drivetrain,
ground and controller reset fidelity are outside this small correction.

The later recovery-timing follow-up moves encounter request consumption out of
controller work and into the post-motion/pre-final-contact callback. See
`outrun-recovery-timing.md` for the bounded integration and remaining intermediate
collision-cadence difference.

**Correction to the earlier route-trigger timing inference:** the full world
caller publishes acceleration at `60A086`, then calls `602660` at `60A257`.
Its `600720 -> 5FBE10` route-trigger placement therefore happens *after*
publication. The clone's route-trigger check after `step_race` has the correct
relative acceleration phase; merely finding it within a car/racer helper did
not establish that helper's world-update position. No route-trigger phase move
is justified by this trace. Existing scene channel placement remains approximate.
