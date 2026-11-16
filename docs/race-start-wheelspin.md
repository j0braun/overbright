# Race-start wheel events (2026-11-02)

This is an incremental port. Wheel event parsing and angular-speed rules
below are traced in SPEED2.EXE (SHA-256
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`).
(2026-11-06: superseded for race starts and endings by the full mode-5
mover, `nis-mover.md`; this file keeps the wheel-event rules.) Smoke appearance and rev audio are **not verified**.

## Native path

* Scene car slots remain the existing grid-slot mapping (0x432860).
  `NIS_UpdateCars` 0x439240 updates every bound slot until the cut, with
  0x4309D0 forwarding its pose to mode-5 mover 0x5A40B0. REA/Ghidra
  decompiled 0x439240; direct disassembly confirms the remaining addresses.
* 0x430B20 maps `CAR_BRAKE_LOCK` to type 4,
  `CAR_BURNOUT_ADDITIVE` to 5, `CAR_ENGINE_TORQUE` to 6,
  `SEQ_CAR_NITRO` to 7, `SEQ_CAR_BURNOUT` to 8,
  `CAR_AUDIO_PATTERN` to 13. The event parser is 0x432D00.
* 0x432F47..0x432F71 parses a burnout sequence's car and duration and
  calls 0x430EA0. This expands sixteen records from 0x7EA2B0..0x7EA454.
  Seven set additive wheel speed; nine set engine torque controls.
  The sequence does **not** set brake lock. Its wheel records are:

  | Event time | Additive turns/s |
  | --- | ---: |
  | start | 5 |
  | start + 3 | 10 |
  | start + duration - 20 | 10 |
  | start + duration - 15 | 8 |
  | start + duration - 10 | 6 |
  | start + duration - 5 | 3 |
  | start + duration | 0 |

  Loader 0x432BA7..0x432BB3 sorts 0x1C-byte records with time comparator
  0x430DB0 through 0x5D4100; the clone likewise sorts expanded events.

* Event dispatcher 0x4392E0 resolves car number minus one through
  0x827BA8 + 20*slot. Types 4/5 require mover mode 5. 0x598EE0 writes
  mover's chassis (+0x24) +0x250 (front brake multiplier); 0x598EF0
  writes chassis +0x254 (burnout additive). Constructor 0x5B7117 sets
  chassis vtable 0x7A25B8, brake multiplier 1 and additive 0.
* Chassis post-step 0x5AB890 first calls 0x5AB1A0, then replaces wheel
  speeds. Forward velocity is the rigid body's velocity dotted with its
  forward axis; magnitude below 0.1 becomes zero. Each wheel starts at
  forward speed/radius. Rear split below 0.2 selects the fronts, above
  0.8 the rears; intermediate splits select all wheels, but only at
  positive forward speed. Selected wheels add
  `360 * additive * 0.01745329238474369` radians/s. Front wheels then
  multiply by brake lock. The named “lock” is therefore a multiplier,
  not a boolean or an extra force. Thresholds/constants are at
  0x784554, 0x7850EC, 0x7A19E0, 0x7A19E4 and 0x784244.
* NIS tyre pass 0x5AB550 computes longitudinal/lateral hub slip including
  body yaw and front steer; slip magnitude greater than 6 (0x7A199C)
  sets its skid flag. 0x5A2B70 writes slip and skid state back to the car
  and wheel objects. Its call to 0x59DBC0 reaches 0x5E6E80; this path
  has not been established as the smoke emitter path.
* Revving is separate: type 13 dispatch 0x439547 resolves car sound
  object +0x968 and calls vtable +0x48 with command 7, pattern number,
  and truncated parameter. This is not the ordinary driving burnout
  scoring path. The sound handler/pattern table remains open.

The shipped intro tracks contain eight burnout commands across seven
scenes: 11 (Car1 at10 duration60, Car3 at112 duration58), 13 (Car1 at10
duration152), 19 (Car3 at15 duration120), 24 (Car4 at0 duration30),
25 (Car4 at15 duration120), 26 (Car4 at0 duration30), and 27 (Car3 at0
duration30). Thus AI cars can receive the same scripted burnout as the
human, depending on their scene slot. Not every intro has a burnout.

## Clone change and remaining differences

`shop_scene::wheels` parses direct brake/additive commands and the
all sixteen records of burnout sequences, including the nine body-torque
commands. Torque controls are retained but not applied by the current
pose-driven scene placement. Controls are evaluated by
absolute scene time, including seeks. Rotation integrates separately
across event boundaries, avoiding a refresh-dependent event duration.
The literal native degree conversion is retained; floating point
integration may differ by invisible rounding.

Race-start placement now keeps the car state and pre-step wheel phase
instead of constructing a new CarState at every moving scene key. Wheel
speed comes from the native formula; its base forward speed still comes
from the scene channel derivative (**approximation**). The shared shop/
ending scene update applies the same wheel controls. Normal gameplay
frame limiting and physics scheduling are unchanged.

The NIS slip/skid calculation feeds the existing tyre effect layer.
That layer's emitter choice and emission rate remain **inferred**, as
documented in `particles::tyre_group`; smoke fidelity is not established.
Native NISMover target steering, rigid-body forces, application of torque events,
hydraulic events and sound patterns still need a full port. Before the
cut, the existing held physics update also remains; its wheel phase is
replaced with the scene integration rather than counted twice.

## Validation

The win-burnout worker captured the shared native mode-5 chassis under
Wine in the isolated review prefix. Artifact:
`review/win-burnouts/native-finish-rwd.json` in the private workspace.
It uses a 240SX win, instrumented finish/lap fields and candidate flags
to isolate EndingNis01; native eligibility and wheel dispatch were left
intact. Of 59 mode-5 samples, 45 had additive 2.5 and front brake
multiplier approximately 0.2. One has front angular speed 4.8839164 and
rear 40.127541: `rear - front / brake = 15.70796`, the traced
`360 * 2.5 * degree_to_radian`. Later samples have brake 1, additive 0,
and four zero wheel speeds. The maximum residual of that relationship
across all 45 active samples is 0.000002265 rad/s. This confirms the shared additive/brake
mechanics; it does not visually verify an intro or its smoke.

Four focused tests cover duration/fade timing, axle selection and the
stationary AWD exception, direct controls/seek/slot isolation, and
rotation at 30/60/144/240 Hz. They passed both standalone rustc and
`cargo test --release --lib shop_scene::wheels`.

The ignored real-asset game regression
`race_start_wheel_phase_survives_scene_placement` exercises 120 frames
of IntroNis13, asserting that moving placements retain accumulated
phase and the scripted Car1 burnout reaches its driven wheels.
It passed (1 test, 120 scene frames). Run it with `NFSU2_INTRO_NIS=13`,
`NFSU2_ROOT`, and isolated XDG homes.

The initial wheel implementation's complete release suite subsequently
finished successfully: 554 passed, 0 failed, 11 ignored. This is not a
claim of complete native parity or validation of later changes.

`cargo build --release --bins --examples` passed (all 155 examples).
Application selftest passed. All nine required shots (`main`, `career`,
`explore`, `intro`, `qrcars`, `race`, `qrstart`, `drift`, `drag`) and
IntroNis13 frames 60/61 rendered; their logs contain no panic, validation
or error messages. Artifacts are in the isolated worker runtime's
`shots/` directory. The frame-60 image was inspected; a still image does
not establish wheel animation or native smoke fidelity.


## Body-torque follow-up (2026-11-02)

The name `CAR_ENGINE_TORQUE` does **not** mean a drivetrain torque or an
RPM command. Parser 0x432ECD reads car number and two floats. Dispatch
0x439485..0x4394BE calls 0x5A4090 only for mode 5; that writes chassis
+0x258 and +0x25C, initially both zero (0x5B712D). NIS tyre-force pass
0x5AB550 calls 0x5A46B0 after its wheel loop. With body forward F (+0x30)
and left L (+0x40), the latter adds the following to the body's torque
accumulator (+0x120/+0x124):

```
torque.x += first * F.x + second * L.x
torque.y += first * F.y + second * L.y
// torque.z (+0x128) is unchanged, even when the body is tilted.
```

The intervening global vector +0x86B390/+0x86B394 was (0,0) in the native
capture. REA/Ghidra output and its executable digest are preserved under
`evidence/nis-torque/`. This maps the consumer; **active torque response
has not yet been matched under Wine**. No body-torque integration is
claimed or added by this change.

The nine torque records in 0x7EA2B0..0x7EA454 are, relative to sequence
start: (0, .1, -2), (3, 0, -2), (8, 0, 2), (12, 0, -1),
(17, 0, 1), (22, 0, -.5), (27, 0, .5), (32, 0, 1), plus
(duration-10, 0, 0). The last is relative to the end; the others remain
start-relative even for a shipped 30-frame burnout. Thus sorting can put
another nonzero command after the reset; it must not be silently clamped
to the duration. Native short-sequence cutoff/dispatch still needs live
validation. The parser now retains both direct torque commands and all
sequence records, with separate typed controls so torque cannot become
wheel speed. `Scene::wheel_events` owns these parsed records; the current
render/physics path consumes brake/additive only.

### Why movement remains open

**Superseded 2026-11-04**: the race-start scene cars now run the ported
NIS mover (target setter, frame, suspension, burnout spin, body torque);
see `nis-mover.md` for the chain and the Wine replay. The text below is
the earlier state.

0x5A40B0 only teleports when the horizontal target error exceeds 1.1 m
or the wrapped heading error exceeds 15 degrees. Nearby, a changed target
sets the mover's pending flag, updates target history and adjusts steering;
low-speed steering seeks heading-error * 25, with a 65 degrees/second
rate limit and a +/-45-degree clamp. Its high-speed (>3.2 m/s) branch uses
velocity direction; full NIS force integration remains separate. This
invalidates any claim that assigning the exact scene pose each frame is
the complete native movement solver.

A fresh private Wine Quick Race capture retained 77 consistent human
mode-5 samples, with zero additive/torque and a nonzero body-to-target
lag of only 0.00018–0.00043 m (see `native-target-samples.json`); that
small stationary residual alone does not validate the movement solver. The built-in forced-scene globals
0x865024/0x865028 requested IntroNis13, but actual scene identity and the
human's bound scene slot were not established. This confirms the observed default controls, **not** an active-burnout/
torque capture or a quantitative validation of target following.
An earlier capture's last record crossed a mover teardown and is excluded;
the retained run checks mode and mover identity after reading. A restart
attempt captured no scene samples. The selector was reset and the private
Wine session stopped. No native physics or event code was patched.

The current clone resets body orientation to its placed pose each scene
frame. Injecting torque into that path would erase the response on the
next placement and would mix NIS torque with ordinary driving forces.
Consequently this bounded change preserves the missing authored commands
without pretending to finish rocking, target following, rev audio or smoke.

Regression: the new 16-record assertion failed against the old parser
(actual 7, expected 16), then passed. Tests also cover signed control
changes, end-relative reset, short-sequence ordering, direct-command
arity, seeks/slot isolation and unchanged stationary wheel speed. The
full sequence is also compared against a 448-byte native table extract
(`burnout-records.bin`, VA 0x7EA2B0, same executable digest).


`SEQ_CAR_NITRO` can interrupt those controls (IntroNis13 uses it at
frames 45 and 100). Parser 0x432F33 calls 0x430DC0, which copies four
records from 0x7EA240..0x7EA294. Two are torque: at start (.5 * intensity,
.5), then zero at start+10. Only the first coefficient is scaled. The two
intervening type-1 effect records at +2/+5 are outside this torque parser.
These torque overrides are now retained too; a regression failed with the
stale burnout control before this addition, then passed. This does not
implement the nitro visual effect or the body's response.


Final follow-up validation: all eight focused parser/wheel tests pass,
including comparison with the native table extract. The release library
suite passes with `NFSU2_ROOT` set (439 passed, zero failed, three ignored).
The release game build and application selftest pass. Nine standard shots
and an IntroNis13 shot render successfully with no panic/validation/error
messages. Logs and images are in the isolated worker runtime's
`torque-*.log` and `torque-shots/`. The full binary/integration suite was
not rerun for this parser-only follow-up; its earlier 554-test result
above belongs to the prior wheel implementation.
