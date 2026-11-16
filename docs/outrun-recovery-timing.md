# Recovery request consumption after motion

Follow-up to `405e33f`. This corrects encounter recovery's controller/physics
boundary while retaining the clone's existing collision subdivision policy.

## Native order

| Address | Operation |
|---|---|
| `42427B -> 5EBF70` | AccidentRecovery action sets car `+96C = 1`. |
| `60A027 -> 5FBBF0` | World velocity snapshot. |
| `60A042 -> 5F0F10` | Iterate enabled collision bodies, calling virtual `+2C` with full world dt. |
| `7A2478 + 2C = 5A9F20` | Car collision body frame saves collision start through `593220`, then calls `6012C0`. |
| `6012DB` | Run current mover virtual `+04`. |
| `60130E` | Read pending recovery flag after mover returns. |
| `6013AF..601519` | Racer recovery resolves placement and calls `5FBEE0`. |
| `601524` | Clear the consumed request. |
| `60A06B -> 5FFA60` | Collision processing. |
| `60A086 -> 604750` | Publish complete world acceleration. |
| `60A257 -> 602660 -> 600720` | Human race route-trigger check, after acceleration publication. |

The collision body's saved start is separate from car `+520` world velocity.
`593220` saves the pre-motion pose and dt; `5A9F20` later updates bounds through
`5A9DF0`. The mover switch in `5F46B0` replaces car `+34` and changes existing
collision body `+5C`; it does not replace car `+3C`. The clone callback preserves
the existing sweep start instead of inventing a second snapshot at placement.
Full native placement/driver callback parity is still outside this change.

The last row corrects the earlier route-trigger timing suspicion recorded in
`outrun-placement-acceleration.md`: that helper's name/ownership alone did not
establish its caller phase. The clone's existing post-`step_race` trigger placement
already follows acceleration publication and is left in that position.

Exact instruction ranges and hashes are retained in
`evidence/outrun/recovery-timing`. This is static original-code evidence, not a
new live-game capture. The prior 36-case native prefix oracle remains the
acceleration/reset arithmetic evidence; no new synthetic callback oracle is
presented as execution of a complete original world update.

## Production boundary

The goal5/6 controller now transfers its action's reset request into an
`EncounterCar` latch corresponding to car `+96C`, rather than placing the car
inside `Encounters::think`. The latch survives action replacement until physics
consumes it; new spawning clears it.

`step_world_with_post_motion` invokes an optional callback after each car's
initial mover simulation and before broadphase/contact processing. Existing
`step_world` callers use a no-op callback. Contact re-simulation does not invoke
the callback again. No force, integration or contact-solver formula changes.

Encounter recovery uses that callback only in the **final subdivision** of the
outer update. Placement therefore sees completed outer motion, remains inside
the original acceleration token, and precedes the final contacts/publication.
Its driver, controls, route and controller/cache reset behavior is shared with
the existing reset implementation. It does not replace the car state, refill the
nitro tank or reinitialize drivetrain state. The callback updates the input used
by any remaining contact re-simulation to the reset controls.

**Remaining cadence limit:** the clone still processes intermediate contacts
between outer subdivisions, whereas the demonstrated native normal world runs
one full body-motion pass before its collision pass. Final-subdivision recovery
is a bounded correction within that existing architecture, not full native
motion/contact scheduling parity. Ordinary race-rival recovery also retains its
prior scheduling. Reconciling outer collision cadence must be a separate shared
physics change, coordinated with force/contact validation; this patch neither
caps FPS nor silently changes the collision rate.

## Regression scope

The actual-asset free-roam regression queues recovery on an active encounter,
checks consumption and controller recovery, and compares published acceleration
against the original outer token. Test-only thread-local observation is enabled
and cleared within that test: recovery occurs exactly once, after final
subdivision motion, and does not repeat next update. The fixture exercises
multiple subdivisions and verifies the mover changed velocity before placement.
At the callback boundary the test also checks nitro state is preserved across
the placement itself. Existing contact, controller lifecycle and world-clock
regressions remain relevant. The original pending-latch test failed before the
post-motion implementation was added.

The combined `encounter_recovery_placement_then_contact_consumes_once` regression
places the player at a real route recovery landing and starts the pending rival
20 m away. Production `step_explore` must produce a live car-pair contact record
and resolve the landing contact after one final-subdivision recovery. It checks
finite resulting transforms/velocities, latch consumption, controller resume,
the complete outer acceleration delta and no repeated recovery next update.
It deliberately does not require the final position to equal the landing: the
native retained pre-motion sweep can rewind the placed pose during contacts.
This guards the combined path; it is not a native collision-output oracle.
