# Drivetrain reaction uses the exported contact count

When suspension contacts disappear within a world update, the drivetrain must
read the car's exported contact count, not the newly measured suspension count.
Native `594A40` scales crank reaction by `max(count / 4, 0.25)` using
`[clutch+34]+3E4`. `5AA840` reads the same location for its locked-clutch gate.
The captured clutch pointer is the physics view (`full car +40`), so this is
`full car +424`. `5ABC00` also copies that count /4 into aero+14. Original
bytes and hashes are retained in internal evidence.

The three production drivetrain consumers now share
`CarState::drivetrain_wheels_down()`, which uses the existing `aero_down` cache:
initial distribution, additional coupling distribution, and final settling.
Current suspension contacts still control their existing force/contact paths.
No component pass, render cap, timestep, or collision response is relocated.

## Observed phase and mismatch

All 559 retained force-entry records agree between exported count and aero+14,
and clutch+34 always points at the retained physics view. These original-game records use the retained controlled landing (+0.6 m
height, −12 m/s vertical momentum), not an asserted ordinary jump trajectory.
The ten airborne
steps 118–127 all occur in world 2972, tick 122057. Their suspension contacts are
zero while full car+424 remains 4. Body Z falls from 8.13985 to 7.69692 m while exported
car Z stays 7.5397067 m. `5A5550` exports the body and wheel state; its
`5A57ED..5A581D` copies wheel+11C flags into the exported count. The capture
therefore directly distinguishes current suspension inputs from the exported
car input consumed by the drivetrain.

At step 118 the former current-contact input produces crank reaction
−0.051740825 and engine speed 83.7072 rad/s, versus native −0.16419762 and
81.88971 rad/s. Using the existing cached count reproduces the native values.

## Regression and explicit limits

`native_airborne_drivetrain_uses_exported_contact_count` restores ten retained
engine/clutch states and handling inputs into the actual production drivetrain,
with a `CarState` containing zero current contacts and cached count 4. It uses
the shared production selector, distribution, settling and integration paths.
The old selector (`wheels_down()`) fails at step 118 with the same reaction
mismatch. The final selector matches all ten engine speeds and clutch
engagements bit-for-bit; maximum crank reaction residual is one f32 ULP
(1.4901161e−8), within the regression's explicit 2e−8 bound. Wheel torques are
also compared with captured outputs at 1e−6. These are tighter than the
expanded force diagnostic's prior 1e−5/1e−4 bounds, not relaxed tolerances.

All ten captures are **Engaging**: coupling returns None and settling returns
no matched-wheel speed. They directly exercise initial torque distribution
and first-gear reaction relief. The other two consumer changes are supported
by the shared native reader trace, not a claim of captured locked-clutch
coverage. The `#[cfg(test)]` handling constructor restores private clutch-slip
and engine histories, rejects unsupported timed bonuses, and excludes
post-update timer/RNG state from its oracle. The fixture asserts zero brake,
handbrake and wheel-hop inputs rather than supplying guessed live values.

The existing `aero_down` cache is refreshed by `simulate`, including collision
re-steps. This patch does **not** establish that its entire lifetime equals
the native outer car export in every path. It corrects consumers within that
existing cache boundary. Constructor state, rest/collision re-steps, general
native matrix fidelity and full component ordering remain separate work.

## Expanded replay context

The private an internal run folder worktree preserves a larger,
uncommitted production force-input boundary experiment. It supplies captured
native matrices, wheel axes, local steering rows, levers, loads and velocities
to the production force consumer; these are **not runtime-generated frames**.
Its 309 consecutive same-world/tick pairs include 181 unsupported cases with
nonzero brakes whose live brake record was not captured. All 61 zero-gravity
rest cases are among those unsupported cases.

Of the 128 supported pairs, 115 passed before this correction and 125 pass after
it at unchanged thresholds. The ten airborne engine failures are resolved.
Landing steps 129, 131, 133 still disagree in tyre/body forces. Those failures
are retained, not accepted as expected outputs or hidden in the ten-case
regression. The broad force-boundary refactor is not part of this patch.
The next concrete slice is the first landing discrepancy at step 129: trace
friction-circle inputs/limiting against the retained native wheel fields.

## Reproduction

```sh
python3 $OB_PRIVATE/docs/evidence/cached-wheel-count/export.py
cargo test --release --lib native_airborne_drivetrain -- --nocapture
```

The exporter hashes the original retained full-phase stream, keeps source
record identifiers, and copies actual native inputs/outputs. It does not
calculate expected engine or clutch states. `manifest.json` and
`lifetime-audit.json` retain provenance and the contact-count audit.

## Final validation

The release library suite passed 496 tests (9 existing ignored). The focused
native regression passed all 10 cases. High-refresh driving passed from 60 to
500 Hz (the existing reverse case remains ignored); all 3 tuning tests passed.
Fixture regeneration is deterministic; `git diff --check` passed. Full logs
remain in the internal run logs; an internal validation record retains
their hashes and exact result lines, including the broader diagnostic's
three unresolved failures.
