# NIS target application and frame order

This extends `race-start-wheelspin.md` with the consumer of the target stored by
0x5A40B0. Evidence is original executable disassembly, not a live trajectory
comparison. The source digest, exact function extents and constants are retained
in internal evidence: nis-mover-step/metadata.json; raw instructions are alongside it.
No production scene or physics code changes accompany this investigation.

## Target application before integration

0x5AB3D0 is a distinct frame path. At0x5AB4BD it reads mover+60, passes
`placement_count > 3`, target heading from+90, target position at+70 and its frame-dt
argument to0x5A44B0 with ECX=mover+24's chassis. It then clears pending byte+94.
The helper call is unconditional in this frame function; the pending byte is
not an early-return gate here.

0x5A44B0 computes `(target_position - body_position) / dt` and wrapped
heading-error/dt times the native degrees-to-radians constant. It derives
current heading through original0x43E6D0's unsigned16-bit result; replacing
that quantized angle with an arbitrary floating atan2 is not established.

For placement_count<=3, the branch at0x5A4663 retains body cached vertical velocity
(+A8) and cached spin x/y, supplies the newly computed planar
velocity and yaw rate, and calls0x59B9C0 with current body position/orientation
and a final zero argument. It therefore does not simply overwrite the body's
position with the target at this boundary. Full setter semantics remain to
be checked in an execution oracle.

For placement_count>3, the other branch subtracts cached planar velocity, multiplies by
body+164, sets the correction's vertical component to zero, and calls0x59BC70
with that vector and point(0,0,-0.2). It then adds a yaw-momentum correction:
(desired yaw rate - cached yaw rate)/selected-inverse-inertia[+28]. The
selection is body+170 when+158 is zero and+1B0 otherwise. Global86B390/+394
are added to angular momentum x/y; their values must be retained in any oracle,
not silently assumed zero. This path modifies momentum before integration.

These are static field/operation observations. In particular, the point passed
to0x59BC70 must not be casually relabeled world or local coordinates until that
callee is traced. The runtime placement-counter lifetime must also be observed across scene transitions.

## Scene integration and tyres

After target application,0x5AB4DF computes
`substeps = trunc(frame_dt / float[7FC040]) + 1`, with the stored constant
0.014285714365541935. It divides frame_dt by that count, stores each substep dt
at7FC800, calls0x5AA490 on the component list, and calls0x5BB0E0 on the body
orientation after each traversal. Finally it invokes0x5A2B70 with the original
frame dt. This is an internal simulation subdivision, not a render-FPS limit.
The detailed contents and dispatch order of0x5AA490 remain a prerequisite
before porting this frame path.

NIS chassis0x5AB550 computes tyre slip/skid diagnostics, but immediately before
transforming/applying each wheel force it explicitly zeroes that force's x/y
(0x5AB806/809); vertical force remains. It also clears two per-wheel fields at
0x5AB80B/80E. Thus simply adding authored torque to the clone's ordinary tyre
forces is not established as equivalent. After its four-wheel loop it calls
0x5A46B0, the already documented authored body-torque consumer.

## Consequence and next verification

The current `scene_cars` resets pose every scene update and uses a channel
finite difference for velocity. Its recently fixed angular-momentum consistency
prevents spurious rollover, but does not implement the native target application
or this mode-specific integration path. Do not remove that fix or inject torque
into pose replacement and claim native NISMover parity.

Next execute original0x5A44B0 with retained body/chassis state, both counter-selected branches,
nonzero vertical velocity/spin and wrapped headings; compare complete modified
body ranges, including the callees' cached-state effects. Then establish
0x5AA490 traversal and capture a moving scene with confirmed identity/slot/mode.
Only those results can support replacing the current placement approximation.

## Original-byte target-helper execution

`probe-target.py` now executes unmodified0x5A44B0 and its original callees in a
private PE32 under Wine, with original image sections mapped at their addresses.
The original startup is not invoked and no helper is stubbed. The executable
image and generated PE remain private runtime artifacts, not repository fixtures.
Only synthetic input/output body snapshots are retained. Each test resets all
768 body bytes and explicitly sets chassis+14, pose, mass, both inertia tables,
cached velocity/spin, momenta, target and global86B390/+394. This isolates helper
semantics; it does not emulate a running scene or normal engine initialization.

The48 cases span both branches, both body+158 selections, identity and90-degree
body orientations, dt0.016/0.1, three target offsets/headings including359-degree
wrap, nonzero vertical velocity and x/y spin. Target z is deliberately100 while
body z is3. Every case preserves body position/orientation, vertical momentum and
vertical cached velocity. The direct path updates planar velocity, yaw rate,
linear momentum and angular momentum using the base inverse inertia regardless
of+158. The impulse path updates momentum but leaves all cached velocity/spin
components unchanged; its yaw correction uses the selected inverse inertia.
The rotated direct setter transforms the preserved x/y spin into correspondingly
rotated angular momentum, as expected from its actual matrix callees.

`check-target.py` verifies the raw snapshot digest, complete changed-word sets,
case coverage, unchanged pose/vertical data and direct-path arithmetic/inertia.
Its arithmetic assertions allow2e-6 relative/2e-5 absolute for Python versus x87
rounding; snapshot preservation and identity checks are exact. It does not claim
a complete independent formula comparison of the capped impulse branch. Two
separate executions reproduced all output bytes identically. All48 cases passed
the offline audit. The private wineserver is stopped by the probe's finally block.

Reproduce with the verified executable and a private output directory:

```
python3 internal evidence: nis-mover-step/probe-target.py /path/SPEED2.EXE /private/probe
python3 internal evidence: nis-mover-step/check-target.py
```

This closes the target helper's basic setter/cache ambiguity. Full moving-scene
parity still requires its counter lifecycle,0x5AA490 traversal, actual contact state,
event timing and frame handoff. No production movement change is included.

### Capped impulse follow-up

The retained fixture now contains64 cases: the preceding48 plus a large target
offset(20,-30) with heading181. These exercise both1000-unit linear-impulse and
10-unit angular-impulse caps. The checker now independently reconstructs the
impulse branch too: planar velocity error times mass, linear cap, cross product
of(0,0,-0.2) with the capped impulse, angular cap, additions to existing momenta,
global x/y additions and selected-inverse-inertia yaw correction. All64 original
outputs pass the same2e-6 relative/2e-5 absolute arithmetic tolerance; exact
preserved-state and changed-word assertions remain unchanged. This supersedes
the earlier audit's limited impulse-arithmetic coverage. The earlier repeated
48-case run is historical evidence; the expanded64-case run was executed once.


### Caller correction: the branch selector is a placement count

Original0x4309D0 resolves the scene car through the20-byte slot table827B98.
Its slot+0C (827BA4 +20*slot) is tested for zero before the initial5FBD70
placement, passed as argument3 to5A40B0 at430AB5..430AC5, and incremented
at430AD8..430AE5 after placement. Thus mover+60 is a placement counter,
not a scene-type enum. Counter0..3 selects the direct helper branch; counter>3
selects impulses and the target-history smoothing branch. The caller checks
car mover mode5 separately at430A96. Earlier wording calling+60 a mode was
incorrect and is superseded here. The Rust helper takes the already-evaluated
boolean and its tested arithmetic is unaffected. Actual counter reset/scene
transition lifetime and integration frequency remain to be verified.

### Static placement-counter lifetime

internal evidence: nis-mover-step/placement-lifetime.asm retains the reset and transfer
sites from the same digest-verified executable; its companion JSON records exact
instruction ranges and the assembly digest. This establishes these operations:

- 0x77ADD0 initializes eight 20-byte slots: pointer, previous x/y and placement
  count are zero; slot+10 is -1.
- 0x4391B0 destroys any occupied slot object, then clears previous x/y and
  placement count for every slot, including already-empty slots. It also sets
  slot+10 to -1. The count reset is at 0x439223.
- The setup sequence at 0x43B12C calls that cleanup, then 0x4328B0,
  0x43ACF0 and 0x435FC0. After 0x437910, the loop at 0x43B172 again clears
  previous x/y and placement counts across all eight slots, preserving the
  slot pointers. Thus initialization work before this second reset cannot be
  counted as later target-placement updates.
- Cleanup is also called at 0x43B20A and 0x43B412 before unlinking and
  destroying the selected scene object. Slot swaps at 0x4308C4..0x43095B
  transfer the entire five-word record, including the count; it is not reset
  merely because the record moves to another slot.

These are static lifetime boundaries, not an observed timeline for every scene
transition. The next moving-scene capture must retain slot identity, count and
the post-setup reset alongside target/frame calls. Incrementing a render-frame
counter or resetting on every target call would not implement these semantics.

### Executed target-setter state and history

`probe-target-state.py` now executes original 0x5A40B0 and its unmodified
callees in the same private mapped-image harness. No original startup or helper
stubs are used. The 96 synthetic cases cover counts 0/3/4/5, frame dt .016/.1,
headings 0/364/65535 angle units, stationary/aligned moving cars, and changed or
unchanged requested targets. Complete mover, chassis, body and car windows are
retained before and after each call. Two independent executions produced
byte-identical snapshots. Runtime original-image/PE files remain private.

`check-target-state.py` independently reconstructs all 1792 output bytes per
case, including preserved state. It verifies:

- The placement count is stored even when the target is unchanged. Such a call
  retains history, pending state and steering rather than recomputing them.
- Changed targets at count <=3 copy the incoming position directly. Count >3
  computes 0.25 times the previous incoming target plus 0.75 times the new one,
  then stores the new incoming target as history. It does not average against
  the previously smoothed target or the body position. The old-history product
  is stored as float before the final product/addition.
- Changed targets set pending byte +94 and target heading +90. Body and car
  windows remain untouched in these cases. Steering at chassis+130 follows the
  dt-dependent rate bound for stationary cars; the aligned moving fixture
  instead selects zero through the native slip-angle branch.

All cases intentionally remain inside the teleport gates. Static instructions
at 0x5A4161..0x5A4197 select 0x5A43A8 when planar body/target distance exceeds
float[7A19CC] (1.100000023841858) or absolute wrapped heading error exceeds
float[7A19D0] (15). That branch calls the full car-placement routine 0x5FBD70
and reacquires the mover before restoring target/history/count. Its side
effects are not captured by this isolated fixture. Arbitrary moving slip angles,
epsilon-boundary target comparisons, tilted bodies and full scene integration
also remain open. The complete native setter must include these branches;
this evidence does not justify wiring a partial setter into live playback.

Reproduce with `probe-target-state.py ORIGINAL_EXE PRIVATE_OUTPUT_DIRECTORY`,
then run `python3 internal evidence: nis-mover-step/check-target-state.py against the
retained fixtures. No playable-package behavior changes accompany this evidence.

### Teleport placement also resets controller state

The old `scene_cars` comment claimed 0x5FBD70 did not touch the controller.
That inference was wrong. `teleport-controller.asm` and its digest/table JSON
retain the original instructions establishing these dependencies:

1. 0x5FBD70 calls 0x5F52D0. That function saves car+28's mover type,
   switches to type 0 through 0x5F46B0, places/reset fields and driver state,
   then restores the saved mover type through 0x5F46B0. A synthetic setter
   fixture cannot represent this by changing just a body pose.
2. After placement and locator update, 0x5FBD98..0x5FBDA7 invokes car+2C's
   vtable +30 with argument 1 and then +34 without arguments.
3. The four retained controller tables have +30 = 0x42CDC0. Its retained
   prefix preserves prior-node direction inputs, then clears controller+0C,
   the FuturePoint list through 0x411BE0 and bits 0–1 in all 16 junction
   slots before calling the new-node search 0x413590.
4. Tables 7852B0/785370/785518 have +34 = 0x40D550; 785428 instead uses
   0x42A050. The former stores signed owner+08 times float .1 times float
   -0.04545454680919647 into controller+6F0, calls driver vtable +5C,
   clears junction flags and bytes +710/+77C, then updates +74C through
   0x5D42E0. It does not clear the last-think tick at +778. The alternate
   +34 prefix also clears junction flags and the FuturePoint list.

This corrects the lifecycle requirement; it is static evidence, not a complete
teleport replay. The clone currently preserves the grid route because its
continuous NIS approximation replaces pose every frame. Applying these resets
every such frame would confuse continuous movement with native teleport events.
The full NIS port must distinguish those events and retain controller/mover
replacement semantics. Constructor side effects, full alternate reset behavior
and live transition timing remain to be captured.

### Static component integration and force dispatch

internal evidence: nis-mover-step/component-traversal.asm retains five complete
instruction ranges freshly extracted from the digest-verified executable.
The companion JSON retains their boundaries, assembly digest and PE-mapped
vtable entries. This resolves the dispatch sequence left open above; it does
not establish the live component membership or reproduce a whole scene frame.

For the composite at mover+08, constructor 5B702C selects vtable 7A258C:
slot+04 is 5A4420 and slot+08 is 5AB140. The NIS chassis at mover+24 receives
vtable 7A25B8 at 5B7117; its force slot+14 is 5AB550, not the ordinary chassis
force method. In particular, composite-relative offsets must not be mistaken
for mover-relative offsets when following this path.

The complete substep order is:

1. 5AA490 calls 5A0D30, which walks the linked list at composite+0C.
   Each component's vtable+18 writes its derivatives into one contiguous
   scratch array; vtable+04 supplies its state count. All derivatives are
   collected before any component state is integrated.
2. A second list traversal obtains each count (+04) and state pointer (+0C),
   invokes the component's +20 hook, then writes each state value as
   `state + dt * previously_collected_derivative`. The x87 result is stored
   to a 32-bit float after each addition; Rust arithmetic rounding equivalence
   is not established by this static trace.
3. After every component has integrated, composite+04 invokes 5A4420.
   When global 89E7B0 equals 8, a four-entry update through 56D4B0 precedes
   the list traversal. Then each component receives its vtable+1C post call.
4. 5A4420 tail-dispatches composite+08 to 5AB140. That routine calls
   chassis+14 (5AB550), then 59E630 on mover+28 with argument zero,
   then 5A2FE0 on the mover. It conditionally calls 5A3420 according to
   full-car presence and 5ED2A0/5ED310 results, and finally tail-calls
   5ECF80 when the full-car pointer is non-null.
5. The frame caller then orthonormalizes the body orientation and proceeds
   to its next substep, as documented above.

The constructor explicitly appends the body to the composite list at
5B707E..5B7095 and calls initialization before a final 5A4420 at 5B72BB.
Do not copy the ordinary mover's eleven-component captured list into this
mode: a live NIS list and the constructors' other registration effects still
need verification. Likewise, the downstream 5A2FE0/5A3420/5ECF80 side effects,
initial force state, ground history and frame writeback remain prerequisites
for replacing the current pose-channel approximation. The next execution
oracle should retain the derivative buffer and component membership together
with pre-integration, post-integration and post-force state, rather than
compare only final positions.

### NIS chassis membership and wheel-spin overwrite

The retained constructor has a second explicit list insertion, not just the
body insertion noted above. At 5B71A6..5B71D3 it takes mover+24's chassis,
uses chassis+04 as its link, appends it through the mover+14 sentinel and
mover+18 tail, and adds its reported state count to mover+1C. Thus the
constructor explicitly registers body followed by chassis. The four wheel
objects allocated later are stored at mover+38..44 without another insertion
in this constructor. A live list snapshot remains required before claiming
membership cannot change later.

`chassis-spin-dispatch.asm` and its digest/range JSON resolve the chassis
vtable helpers already recorded in `component-traversal.json`:

- Slot+04, 40CBB0, returns four state values.
- Slot+0C, 5A80D0, returns chassis+20, the wheel-spin array.
- Slot+18, 40D080, returns its argument without writing the derivative buffer.
  It does **not** write four zeros. The integration scratch values therefore
  must be observed before replacing this callback with an assumed zero-rate
  model; this static trace does not establish their runtime contents.
- Slot+1C, 5AB890, first calls the shared 5AB1A0 chassis post routine, then
  overwrites each of the four spins from body cached velocity projected on
  the body's forward column, divided by the corresponding wheel radius.
  Its drive-layout branches add the authored burnout contribution from
  chassis+254 to selected wheels; chassis+250 multiplies the front two.

This overwrite happens in the post traversal, before 5AB140 dispatches NIS
forces. Ordinary Melo wheel-spin derivative 598AD0 is not the NIS derivative
callback. Reusing the Melo engine/gearbox torque integration for scene wheel
spins would therefore need separate evidence. The authored burnout and front
brake values are inputs to this post overwrite, not proof that ordinary wheel
torque integration runs in scene mode. The next original-byte post oracle
should cover all drive layouts, both travel directions, nonzero authored
burnout/front-brake values and complete spin/state preservation.

### Precision audit after the live physics capture

All 2,580 FXSAVE records in `evidence/full-phase-inputs` have control word
0x007F at record offset 80: single-precision x87 with nearest rounding.
That observation is for the controlled ordinary-race capture, not a live NIS
frame. The earlier target-helper harness explicitly uses FINIT without FLDCW,
so its retained results must not be treated as proof of gameplay precision.

A repeat of the same 64 original target-helper cases explicitly loaded 0x007F
immediately after FINIT. `target-pc24-native.bin` retains its synthetic before/
after snapshots; `target-pc24-comparison.json` records the hash, reproduction
change and per-case changed offsets versus the original fixture. Outputs differ
in 26 cases, with maximum changed-float difference 0.0001220703125. This does
not invalidate the established branch/cache behavior, but it prevents a claim
of bitwise runtime arithmetic from the earlier higher-precision fixture.
The existing unwired Rust helper remains unchanged pending a precision-specific
comparison and confirmation of the live NIS control word. The private probe
completed and stopped its owned wineserver; no original image is retained here.

### Direct Rust comparison of both precision fixtures

`cargo run --release -p devprobes --bin nis_target_precision_probe` now compares the
actual `apply_target` helper against both retained original-code fixtures,
reporting component bits rather than relying on the older tolerance test.
The FINIT-default fixture differs in 8/64 cases (12 components, maximum
absolute error 0.0000009536743). The PC24 fixture differs in 22/64 cases
(34 components, maximum absolute error 0.00012207031). The complete output
is retained in internal evidence: nis-mover-step/rust-precision-comparison.txt.

This is a diagnostic, not a passing parity gate. Its successful process exit
means the comparison ran, not that arithmetic matches. Neither fixture
establishes the live NIS precision mode. The remaining work is to capture
that mode and match the corresponding original operation/rounding order
before wiring the helper into scene playback. No production behavior or
existing test tolerances changed in this audit.

### Angular impulse store boundary

The original 59BD43..59BD7B retains the scaled angular impulse in x87
registers through addition to existing momentum. The old helper rounded
the impulse before adding. Preserving that boundary removes all bit
differences against the 64 FINIT-default snapshots. The PC24 diagnostic
still differs (26 cases, 46 components); this remains a higher-precision
helper, not an implementation of the observed ordinary-race PC24 mode.
No scene playback is wired to this helper. `impulse-cap-rounding.asm`
retains the original instructions, and `rust-moment-rounding.txt` retains
the comparison. Live NIS precision and its matching arithmetic remain open.

### Live-precision target implementation

The successful fresh-career capture in `evidence/nis-mover-step/live-precision`
observed 404 NISMover entries, all PC24/nearest (0x007F). The target helper
therefore now rounds each operation as f32, including reciprocal, position
delta, yaw, capped impulse and angular momentum addition. The live precision
rounds arithmetic even when values remain in x87 registers.

The actual Rust helper now matches all 64 PC24 original-code snapshots
bit-for-bit (zero differing components). Its exact regression now targets
that fixture. The FINIT-default diagnostic differs in 26 cases, as expected
for a different precision mode; its historical fixture remains retained.
This supersedes the higher-precision helper described above. Extended-exponent
extremes and full scene integration remain unverified; the helper is still
unwired from scene playback.

### Live-list state layout and next capture boundary

The captured rigid-body vtable7A20CC resolves count+4 to5A80C0 (32 floats),
state+0C to5A80D0 (body+20), derivative+18 to59C1F0, and pre-integration
hook+20 to5A80E0. Alongside the verified four-float NIS chassis, the
composite scratch is therefore body[0..32], chassis[32..36]. The body
integrates bytes+20 through+9F; cached velocity+ A0 is outside that range.

At5AA4AA, immediately after derivative collection, scratch begins atESP+18
and the chassis slice atESP+98. This is the precise next live capture
boundary: retain36 derivative floats plus both pre-state arrays anddt, then
the post-integration/pre-component-post values. A no-op chassis derivative
does not justify inventing zeroes; its scratch words must be observed.
The retained59C1F0 assembly also shows damping/gating dependencies beyond
the small entry snapshot (+150/+154,+200/+204/+208), so that snapshot alone
is insufficient for a full derivative replay. Evidence: body-state-dispatch
assembly/JSON. No production behavior changed by this state-layout audit.

### Live derivative scratch is not zero

`evidence/nis-mover-step/live-derivatives` retains the final paired sample
from1212 NIS integrations in a fresh career intro. All36 updates match
PC24 explicit Euler bitwise. Crucially, chassis derivative words34/35
are BF7FD078/BD1C418A while pre-state is zero, yielding BC35E994/B9DE3B14
before post callbacks. Slots32/33 are zero. The no-op derivative callback
leaves live scratch data; replacing it with zeroes is contradicted by this
capture. Scratch provenance and how5AB1A0 uses the intermediate wheel state
before5AB890 overwrites spin need tracing. Only the last paired sample is
retained, so this does not establish all-frame scratch behavior.

### Integrated wheel state is consumed before overwrite

The no-op chassis derivatives cannot be dismissed solely because5AB890
later overwrites spin. Its first call,5AB1A0, runs suspension post5A3710,
then checks integrated+20..2C spin magnitudes against30 for brake-lock
flags and derives drivetrain RPM from those wheel values. Axle selection
uses native rear-split cutoffs0.2/0.8, axle ratio+1B8/+1BC, half and
9.549304 conversion; gear selection writes+124 and clamped RPM writes+120.
Only afterward does5AB890 replace spin using body velocity/radius and
burnout. This proves a state dependency, not that the final captured
small residual crosses a clamp or changes a gear. Original instructions
and constants are retained in chassis-preoverwrite.asm/json. A replay
must preserve this order and establish scratch provenance or its actual
observable effects before substituting deterministic zero derivatives.
