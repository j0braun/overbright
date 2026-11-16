# Native acceleration input: writer, phase, and bounded replacement

Investigation follow-up to reviewed controller commit . The original
evidence was committed separately; the retained-state implementation is described
at the end of this document. The original executable SHA256 and byte-range hashes are in
the maintainer's private captures. The native helper uses copied original
instructions with synthetic car data, in an isolated headless Wine prefix. It
is not a new live-game capture.

## Finding

**Native car `+80` is a finite difference of velocity across the world update.**
It is not the force-pass acceleration (`force / mass`) and should not be replaced
by the clone's local `dv` in the physics force pass. That local value excludes
later collision impulses and covers a smaller integration interval.

The normal update chain is statically explicit:

| Address | Operation |
|---|---|
| `5813ED..581417` | The same f32 dt stored to `865150` is passed to `609CF0`. |
| `609F1E` | AI racers' controller virtual `+0C` (including `42ABD0`). |
| `609FD2` | Every driver's virtual `+04`. |
| `60A027` | Every car's `5FBBF0(dt)` snapshots pre-step position/velocity/spin. |
| `60A06B` | `5FFA60(dt)` runs world physics/contact processing. |
| `60A086` | Every car's `604750(dt)` computes acceleration and race events. |

`5FBBF0` copies velocity `+70/+74/+78` to `+520/+524/+528`, and spin
`+110/+114/+118` to `+530/+534/+538`, only when dt > 0. `604750` computes
velocity difference divided by that same dt into `+80/+84/+88`, and the angular
equivalent into `+120/+124/+128`, also only when dt > 0. Zero, negative, and
unordered dt preserve the prior vectors. The next controller update therefore
reads the previous completed world's acceleration. This is separate from the
current controller call's dt and from per-mover `7FC800` substep lengths.

This confirms the *kind* of estimate used in `90d98d8`; the remaining differences
are the exact snapshot boundary, persistent component state, projection and
rounding. Existing `formats.md` already documented the angular half at the drift
steering section; this trace verifies the linear half and its caller order.

## Arithmetic and original-code oracle

`604776..604834` first computes an x87 reciprocal of dt. X's velocity difference
stays on the x87 stack; Y and Z differences are stored as f32 before multiplying
by that reciprocal. Each final component is stored as f32. Thus simply doing
three f32 divisions is not necessarily bit-identical. `5EC3B0` multiplies stored
acceleration by the **full 3D forward vector** `+90/+94/+98`, adding Z+Y then X
on the x87 stack. At `90d98d8`, `Actions::own_accel` instead truncates to XY, dots with
the normalized XY heading, then divides by dt. That also loses slope/roll input.

`native-acceleration-probe.py` executes the unchanged writer prefix and complete
`5EC3B0` for 480 cases. It replaces the continuation at `60483A` with a harness
epilogue before unrelated race-event callbacks. The cases cover positive .05,
1/60, .03125 and .001 seconds, signed zero, negative dt, NaN, changing velocities,
and X/Y/Z/mixed forward vectors. x87 precision is set explicitly to 24, 53 and
64 significant bits; all exceptions are masked and rounding is nearest.
**These oracle modes alone do not identify the live control word.** The later
retained FXSAVE check below establishes PC24 for its normal-world session.

`check-acceleration.py` verifies all outputs and the retained binary hash:

- 480 cases match the instruction-order model; 240 preserve the old vector.
- 41 positive-dt cases differ from three ordinary f32 component divisions.
- 122 positive-dt projections differ from the previous XY dot-then-divide shape;
  two of these differences occur even with zero forward Z.

The checker uses Python f64 for the 53/64-bit intermediate models. It matches
these cases; this is not a proof that f64 universally emulates x87 extended
precision. Exceptional inputs beyond the explicit dt cases, overflow/underflow,
FPU exception handling and directed rounding are not established here.

## Clone state and implementation boundary

`CarState` exposes velocity, spin, orientation and the previous steering yaw
sample (`last_yaw_rate` / `last_steer_dt`). At `90d98d8` it had **no stored equivalent
of car `+80`**, and `frame_dt` belongs to individual physics/mover calls.
`Actions::last_velocity` and `Encounters::road_prev` are controller-owned samples.
They should not become the authoritative native acceleration state.

The justified implementation boundary is to introduce a retained world acceleration
vector (and pre-world velocity snapshot) on `CarState`, sample after controllers
and drivers and before the outer world-physics loop, then finish once after the
whole loop and its contact/placement handling. The value must survive skipped
full thinks and zero-dt updates. The game callers currently subdivide their
updates before calling `step_world`; updating this value at every `step_world`
call would sample the wrong interval. Outer free-roam, race, held-scene and
stationary-world paths must use the same explicit begin/end boundary. Tests must
cover variable dt, multiple substeps, a contact impulse, a controller-side
placement between think and physics, and zero dt.

Only then should `Actions::own_accel` read the stored vector projected on the
full 3D forward direction. Other-car acceleration uses `60BE40 -> 5EC340`, a
separate speed-dependent projection, and needs its own exact normalization/
near-zero audit; changing own acceleration does not prove that helper's parity.
The reviewed finite-difference patch need not be replaced with force/mass: that
would move it farther from the demonstrated native operation.

## Smallest remaining live capture

Use one **owned private** active-outrun session and a non-mutating recorder at:

1. `5FBBF0` entry and `5FBC66` (after snapshot);
2. `604750` entry and `60483A` (after acceleration write);
3. `5EC3B0` entry at the next AI read (optionally capture its returned x87 value
   at an identified caller, without consuming/changing the live x87 stack).

Each record needs car pointer and signed world/racer IDs, owner class/kind and
mover mode, world counter `7FB724`, tick `865188`, **actual argument dt bits**,
`865150` and `7FC800`, velocity `+70`, saved velocity `+520`, acceleration `+80`,
forward `+90`, spin `+110`, saved spin `+530`, angular acceleration `+120`, and
x87 control word (`FNSTCW` or debugger FP register state). Capture the fields
atomically at each hook, preserve registers/flags/x87 state, and record overflow
or dropped records. Add a reset/placement marker and one collision frame so the
pre/post boundary can be checked where controller samples may diverge.

The retained nitro-context raw car snapshots contain many of these car fields,
but lack the actual dt, phase and control word. They cannot establish this
arithmetic/phase equality by themselves. A generic rendered-frame number, a
nominal FPS-derived dt, or two unsynchronized memory reads would not close it.


## Implemented retained state and observed precision policy

`CarState.world_acceleration` now retains the completed world velocity delta.
`begin_world_update(dt)` returns a snapshot token; `finish_world_update(token)`
publishes the vector once after the outer interval. Keeping the token independent
of `CarState` means a physics-consumed reset can replace the state without losing
its pre-step snapshot. Zero/negative/NaN dt preserve the prior acceleration.
`step_world` and `step_with_props` are deliberately low-level: neither publishes
per-substep acceleration. Public complete-frame `step`/`step_ai` and the game's
outer free-roam/race/held/scene paths bracket their complete intervals.

The ordinary racer snapshot follows controller mover-choice callbacks and
precedes BeltMover motion. Its recovery request is consumed by the car-physics
path (`6013AF`), so that velocity change belongs inside the interval. Encounter
recovery was initially called during controller work; the later
`outrun-recovery-timing.md` correction queues it until after complete outer
motion, before final contacts/publication, inside `step_explore`'s snapshot. Traffic snapshots originate inside `traffic_think`, after placement,
controller and driver work, immediately before its TrafficMover motion; they
finish in `traffic_settle` after contact/mover handoff. Standalone pre-green coast
and scene pose updates likewise publish their complete motion. Existing scene
mover and placement approximations remain; this does not establish their full
native trajectory fidelity.

`RoadCar.forward_accel` supplies the **own-action** full3D projection from this
retained value. `Actions` no longer rebuilds own acceleration from its last call.
The later other-car follow-up replaces `RoadCar.accel` / `60BE40 -> 5EC340`'s
legacy XY velocity-history approximation with its separate native projection;
see `outrun-other-acceleration.md` for the original-code oracle and limits.

The retained controlled-landing capture the maintainer's private capture
contains 2,580 records, all with FXSAVE control word `0x007F` at byte80 of each
65536-byte record. This means PC24, nearest rounding, exceptions masked. The
recorder saved FPU state before its own copying work and restored it afterward.
`check-live-acceleration-cw.py` verifies the decompressed SHA256 and every control
word; source hashes, phase counts and scope are retained in `live-cw.json`.
The session was an owned Quick Race Peugeot206 controlled landing. It does not
establish every frontend, replay or alternate gameplay mode's precision setting.

The runtime implementation therefore uses explicit f32 subtract, reciprocal,
multiply and Z+Y+X projection order, matching the 160 original-code **PC24** cases.
The 320 PC53/64 cases remain evidence of different possible arithmetic, not the
chosen runtime behavior. Rust f32 matches the observed significand rounding over
the tested finite range; x87 extended-exponent overflow/underflow extremes are
still not emulated or claimed. No arbitrary epsilon or force/mass replacement
was introduced.

Regressions cover the 160 PC24 writer/projection cases, variable outer dt,
substep accumulation, an impulse after the snapshot, zero/negative/NaN retention,
full3D slope projection, actual controller-side reset before sampling, and a
real two-car collision through `step_explore`. Existing encounter lifecycle and
free-roam/race clock regressions also run with read-only assets/private XDG.

## Scene boundary follow-up

The companion `SceneCar::frame` now brackets its channel motion just like the
player scene updater. The real Intro13 regression first failed with the old zero
acceleration, then checks moving, variable-dt, zero-dt and negative-dt companion
updates. Race-start scene placement now reuses the samples returned by
`step_race`, captured after controller/mover-choice callbacks. Previously its
second acceleration publication used an earlier pre-controller snapshot and
could include a controller-side velocity change. The regression also checks that
returned tokens reproduce each racer's physics publication.

This is a boundary correction within the existing scene approximation: scene
velocity is still a channel-position derivative, with inferred held physics
before scene placement, not the original mode-5 mover implementation. It does
not establish native scene acceleration trajectories. No controller or driver
runs in the companion channel updater itself.

The game velocity-writer audit found race route-trigger placement after
`step_race`, manual reset-to-road, encounter spawning, and diagnostic track-pose
placement outside the sampled physics interval. These existing placements
replace or initialize state; they are not turned into artificial acceleration
impulses here. Full native placement/reset retention remains a separate open
boundary. Traffic placement is sampled inside `traffic_think` after controller
work; BeltMover motion and physics-consumed rival recovery remain inside their
existing samples. Particle velocities and benchmark fixture assignments are not
car world updates.

The subsequent reset audit in `outrun-placement-acceleration.md` establishes an
unconditional native acceleration clear in `59F890`. Encounter recovery now
clears its previously stale vector too. Full callback snapshot retention and
native placement timing remain explicitly open there; this does not change the
normal-world snapshot contract.
