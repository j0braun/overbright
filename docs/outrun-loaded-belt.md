# Loaded-road active-outrun BeltMover mapping

Pre-implementation mapping, based on `evidence/outrun/belt-applicability`.
Only loaded-road race7 selection is covered by the live oracle. Goal5 and
unloaded notice behavior remain unobserved. Existing Belt trajectory/road pose
and quaternion approximations do not become verified by matching the selector.

| Native input/boundary | Current encounter representation / intended mapping |
| --- | --- |
| 42B259 end of full think | Existing `ThinkSchedule::full_think`; selection belongs after action/driver update, never fallback |
| driver+10 path count | Controller's retained Steer.path passed to DriverInputs; empty output means no path. AiDriver borrows this path and does not retain another copy |
| driver+230 target | Controller.target_speed after cap/suspension; same DriverInputs target |
| action+204 | Actions.kind (JustDrive6, Pass0, Block4, Avoid1, GotHitB, RecoverD); GetUnstuck is separate state and maps9 |
| controller+730 | Add encounter-owned belt block countdown; contact gives2s, belt drop1s; existing ordinary implementation establishes mechanism |
| controller+734 | Add contact hold ticks; human_hit gives20000 ticks; countdown uses existing4000Hz logic, renews4000 with fewer than4 wheels |
| fullcar+42C | Explicit exported_forward_speed, not live velocity projection |
| fullcar+424 | Existing runtime wheels_down representation; component export lifetime remains an existing approximation |
| skill callback | Encounter's selected nonhuman AI row; raw oracle retains signed descriptor row and override |
| distance | Human world-sample position, three-dimensional; gate helper evaluates native PC24 Z²+Y² thenX² order |
| race clock | Active Outrun.clock, native gate30s; original capture retains integer ticks |
| finished flag | Active outrun participant is unfinished; manager ends race7 on completion; existing belt is retained through post-race parking |
| fullcar+5CC | Existing streamed_in visibility proxy; native streaming latency remains unverified |
| fallback5F4410 | Full physics during contact hold, otherwise Melo3; do not reset placement or drivetrain |
| full-think accepted already4 | Rebuild existing belt with retained path/target, preserving Belt history |
| 59EC90 motion | Shared existing belt evaluator and road placement once per world update, then Input::Belt in collision world |
| resolved contact | Existing drive off_belt emits belt_hit and switches current input; consume contact flags before next substep |
| recovery/spawn | Drop old belt when placement replaces its path; preserve existing recovery ordering |

Native 402450 writes +734 on human contact (othercar+18) or its forced flag,
and +730 for any mode4 contact. Native42C281 decrements +734, extends it if
not all wheels are down, and restores the selected mover on expiry. These
writers are statically traced; captured successful transitions all have both
timers zero. Runtime timer tests must be described separately from the live
203-group gate oracle. The clone's boolean contact notifications cannot claim
every native contact flag variant.

No automatic entry is added for unloaded roads. An already active belt can
continue its existing path there, but native notice-driven mode selection is
still open. Mode4 persists in the native final race0 group, so ending an outrun
must not invent an immediate belt exit. Post-race/free-roam controller parity
beyond that retained state remains open.

## Implemented boundary and remaining scope

The active race7 caller now supplies the retained driver path/target, actual
mode to scheduler/driver, exported scalar, action state and explicit timers to
the loaded decision. A missing steering update retains the driver's previous
path; a supplied empty path clears it. Recovery and spawn clear the old plan
and belt. The shared builder and stepper preserve ordinary-racer equations and
movement order; ordinary entry seed speed remains its existing value.

The gate's false branch42B28E..42B2AC calls5F4410 only when the old mover is4.
A non4 mover remains unchanged. Thus initial mode1 is preserved until an
independently represented mover change occurs. General encounter spawn/start
mover lifecycle42C0C0/hold remains the next required native slice. The runtime
test asserts initial1, later4, and rejected4→3 rather than hiding this distinction.

Native609E30..609E4D copies full-car+60 into cached human world+80. Full-car+60
is the body's COM position (also the height produced by5F4F75); CarState.position
is explicitly the car-space origin. The runtime adapter transforms handling COM
by the existing quaternion matrix on both cars. A rotated, nonzero-COM geometry
regression straddles120m: the origins would accept, the COM points correctly
reject. Native raw axes versus normalized runtime quaternions remains open.
The human COM is snapshotted once at Encounters::think entry before any mover;
Drive is immutably borrowed throughout dispatch. The live capture's cached+80
matches current player+60 in203/204 predicate records; record451 differs because
the diagnostic follows the rival by writes that may interrupt a world update.
The oracle always retains cached+80, including451. There is one human; the
second cached slot is a distant sentinel. No multi-human equivalence is claimed.

Contact timers are owned by the encounter: +730 uses8000 ticks on a belt hit,
4000 on drop; +734 uses20000 on human hit and extends4000 when expiry finds
fewer than4 wheels.577320 subtracts trunc(float32(dt*4000)) per update. This
keeps native timer quantization with uncapped updates; it does not add a frame
cap. Ordinary racers retain their existing seconds-based +730 timer. Shared
stepper returns its drop notification so it does not impose that approximation
on the encounter. Contact notifications run for all active encounters, including
before first belt entry and after race completion, because stale human contacts
must inhibit entry. That contact-driven hold expiry can select3; general
initial spawn mode selection is still open. Other native forced-contact flag
variants are not represented by the clone's existing boolean notifications.

Existing belt is kept through the post-outrun parking interval, matching the
final captured race0 group. Free-roam route/control behavior beyond this retained
mode is not newly verified. No unloaded-road entry is introduced. An existing
belt continues/rebuilds its retained plan there, but notice-driven transitions
are unverified. Nitrous strategy rejects existing mode4 before its timer query,
as native416F70 does; full fake-engine/nitrous effects on belt are not claimed.
Existing road placement, catch-up production, force/mover ordering and spline
approximations remain. The203-row oracle supplies native inputs and establishes
selector/distance arithmetic, not production of identical routes/frames/speeds.


The independent review found and corrected a timer guard error before delivery:
42C0CA loads car+5CC and42C0D7 sets BL from its non-null test.42C2BC and
42C2D8 require this loaded flag. Unloaded hold expiry therefore preserves the
current mode (tested for both1 and4); loaded ongoing hold evicts retained4 to1;
loaded expiry with four grounded wheels restores3 even from retained4. Initial
draft omitted those guards. Static source contradiction and regression checks
are retained; no tolerance or timeout was widened. General unloaded notices
remain unimplemented. The finalized production helper was not reverted for a
retrospective red run.


## Validation and reproduction

All runs use the read-only verified assets and private XDG/TMP paths via
an internal run folder (retained below as `run.sh.txt`).
From this worktree, invoke that wrapper with (the scripts are kept with the maintainer's private evidence):

```sh
cargo test --lib --bin game
cargo test --test track_l4ra --test high_refresh --test tuning_physics
cargo test --bin game intro_skip_preserves_ai_grid_handoff -- --ignored --nocapture
cargo run --bin game -- --selftest
python3 test_decoder.py
python3 generate-belt-gates.py
```

Final results:512 library passed/12 ignored;20 game passed/12 ignored;
26 track;1 high-refresh passed/1 existing ignored;3 tuning; explicit intro-skip
passed at60/240Hz with zero authored target/progress/handoff errors. Full selftest
passed (cash6650, reputation0, stage1, wins3). Native203-case selector and exact
PC24 distance replay pass; the hardened204-group decoder and10 corruption
checks pass; compact fixture regeneration is byte-identical.

the maintainer's private captures retains logs, exact source hashes and static
byte ranges. The initial asset-backed regression failed with actual mover1
versus expected4 before production changes. Expanded final regression also
covers absent vs empty path, pre30/near fallback,4→3→4, actual240Hz world motion,
postrace retained4, actual60Hz player contact, loaded/unloaded hold behavior,
expiry and rotated COM geometry. These runtime checks validate the integration
and regressions; they do not compare an entire cloned trajectory to native.
No clock thresholds, assertions, tolerances or frame caps were relaxed.
