# Car simulation fixes

Fourteen small fixes and checks of the car simulation, each traced in the original's code and held by a test.
They start with the positive-time-step captures of the original that the later fixes build on. The workflow
for such a fix is [parity-workflow.md](../../developing/parity-workflow.md); the index of physics notes is
[README.md](README.md).

## Fresh native positive-dt component boundaries

Captured 2026-11-03 in an owned Wine copy of the verified English target
(SPEED2.EXE SHA-256 `8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`).
This supplies fresh original-game evidence missing from
"Rest and bump-stop component refresh" (below) and the retained frame-entry x2 recorder.
**No clone physics or scheduling changes are included.**

### What was recorded

The recorder runs on the native game thread, copying actual state at:

| Tag | Native boundary |
|---|---|
| 1 | `0x5B04E0` entry, before normal component traversal |
| 2 | `0x5B051B`, after normal traversal, immediately before `0x5ABB10` |
| 3 | return from rest test `0x5A5E30` to `0x5ABB33`, retaining EAX |
| 4 / 5 | before / after each visited bump helper `0x59E1B0` at `0x5ABB58`; x87 return retained |
| 6 / 7 | before / after `0x5A0D90` at `0x5ABBE9`, retaining actual zero substep dt |
| 8 | after `0x5ABB10`, before the next force-pass tail dispatch at `0x5B0520` |

Every record includes actual `0x7FC800` substep dt, `0x865150` frame dt,
`0x865188` simulation ticks, the live component list, full suspension
objects and their pointed-to records (including exact maximum compression),
wheel objects and contact normals, rigid-body pose/momenta/cached velocity,
engine/clutch state, controls, registers and FXSAVE state. The previous
compression for a normal traversal is its tag-1 value; suspension +0x20
is **rate**, not previous compression. Tag 2 gives the newly measured value.

The native comparison is ordered **equality**, not an inferred threshold:
`0x5ABB4B..0x5ABB56` compares current compression to record +0x14, tests
AH against `0x44`, and skips on parity. Only ordered equality reaches the
bump helper. Exact captured maxima are front `0.1600000113248825` and
rear `0.16500002145767212`. The instruction extract is retained.

### Results

| Run | Records | Complete step groups | Positive normal posts | Rest-triggered zero refreshes | Bump-triggered zero refreshes |
|---|---:|---:|---:|---:|---:|
| baseline, no teleport | 1,572 | 334 | 334 | 118 | 0 |
| landing, +1 m / -2 m/s | 2,932 | 678 | 678 | 110 | 0 |
| hard landing, +0.6 m / -12 m/s | 2,560 | 556 | 556 | 112 | 17 |

All runs have **zero dropped records, no incomplete step groups, fixed
TABLE pointers verified against the live list/object vtables, exact
suspension-record pointers verified, and successful hook restoration**.
The ordinary landing reached only approximately 0.084 m front / 0.052 m
rear compression. It is retained as a negative control, not relabeled as
bump coverage.

The hard landing has 39 bump-call pairs. Native step 134 reaches front
full compression during the normal component post (records 756 → 757).
The rest return is zero. Front bump returns are 1.7469496727 and
1.7537295818; the zero-time component post follows, then the next force
pass. Step 135 reaches rear full compression. Wheels 3 and 2 leave full
compression at steps 137 and 138. Both fronts leave at step 151, whose
complete sequence is `[1,2,3,8]`: no bump and no zero-time post.

This exposes the cached-velocity refresh, not just a clutch phase:

| Hard-landing record | Boundary | Body vertical momentum | Cached vertical velocity |
|---|---|---:|---:|
| 757 | normal post | -6.75891447 | -6.40655327 |
| 763 | after impulses, before zero post | -3.26074600 | -6.40655327 |
| 764 | after zero post | -3.26074600 | -3.09075427 |
| 765 | next force input | -3.26074600 | -3.09075427 |

Step 139 visits two fully compressed front suspensions and both bump
returns are zero, but still executes the zero-time traversal. This is
fresh positive-dt evidence for the no-impulse refresh case. Its rest
return is zero, so these callbacks are not being confused with rest hold.

The same continuous capture contains a native throttle change 0 → 1
between adjacent complete steps 112/113, before the impact, and 1 → 0 at
340/341. Gear and target gear remain 2 through the compression episode;
clutch phase is Engaging (2), with engine power 0.7. The bump zero posts
in this episode do **not** change the clutch phase or power. Therefore
this is **not** evidence of a gear/phase transition exactly coincident
with the compression boundary. Rest-driven phase changes elsewhere in
the trace are distinguished from bump-driven updates by the captured
rest return and bump-call sequence.

### Instrumentation and provenance

the maintainer's private captures retains the assembly, linked recorder bytes,
disassembly, Python driver/decoder, three complete losslessly compressed
raw streams, metadata, summaries, selected hard-landing decoded boundaries,
and a SHA-256 manifest. The assembly saves/restores GPRs, EFLAGS and full
x87/SSE FXSAVE state; in particular the bump helper's x87 return is not
consumed by logging. Raw FXSAVE bytes remain in every record. Exact original
and replacement hook bytes are retained in each run's metadata.

Code occupies verified zero padding at `0x782B00`; no executable file
was changed. Installation and restoration occur while the game is paused
through its menu and SIGSTOPped. The five hook sites are checked against
the exact executable bytes before installation. The recorder redirects
calls/returns, but the original component/rest/bump implementations run.
The temporary hooks and telemetry padding are restored byte for byte.
A process-local allocation holds the records; stopping the owned process
releases it. The private executable's file hash is unchanged afterward.

The runs use a Peugeot 206 in Quick Race. Teleports change only the native
rigid body's position and momenta, with the original mass used to convert
requested vertical speed to momentum. Every before/after byte and keyboard
transition is retained in metadata. Suspension travel, maximum compression,
engine/clutch phases, gears and wheel spins are **not** fabricated or
written to produce an expected result. The native simulation supplies
those states. Instrumentation and SIGSTOP can affect wall-clock timing;
actual native dt is retained rather than substituted with a claimed fixed
rate. This is not an uninstrumented trajectory or performance comparison.

Owned runtime paths:

- Wine: a private copy, display `:174` (stopped).
- Worktree: an internal run folder, based on `ae2dbbb`.
- Runtime logs/full decoded JSON: an internal run folder.

The original source, original game directory and user saves were not
written. The borrowed scene work and parent physics source are unchanged.

### Reproduction and verification

Use an owned copy as required by `capture.py`, the retained an assistant
`wine/fastgpu.sh` method, and `fastnav.py OWNED race` to reach a fresh
Quick Race. Read the driver before running it; it installs temporary
in-memory instrumentation. Run the baseline, ordinary landing and hard
landing in that order. Each finishes at the native pause menu; send Escape
to resume before starting the next driver invocation:

```
python3 capture.py OWNED baseline --seconds 1.2 --no-teleport
# Escape, then:
python3 capture.py OWNED landing --seconds 2.5 --drop 1 --vertical-speed -2
# Escape, then:
python3 capture.py OWNED hard-landing --seconds 2 --drop .6 --vertical-speed -12
```

Exact recorded initial states and writes, not these relative-drop commands
alone, define the preserved trial. New playback need not be bit-identical.
All native sessions must be coordinated with other GPU work and stopped
through the owned-prefix supervisor afterward.

Offline audit (no game, Wine or clone simulation required) (the scripts are kept with the maintainer's private evidence):

```
python3 test_decoder.py
```

This audits all three complete streams, verifies raw/compressed hashes,
record indices, actual pointer/list consistency, normal/zero/force group
completeness and actual substep dt. Summaries reproduce identically. It
also deliberately alters a target pointer while recomputing transport
hashes, and truncates a group while updating counts/hashes: both are
rejected for the relevant semantic failure. Input transitions are accepted
only across adjacent complete step groups. `git diff --check` passes.
No Rust build/test is claimed or required by this evidence-only change.

### Remaining work

This provides a non-circular original-game oracle for compression entry,
exit, impulse and zero-time cached-state refresh. It does not by itself
validate any proposed clone reordering. A regression must initialize the
relevant clone state from the actual captured boundary, then compare
normal post, bump/zero post and next-force inputs. Moving only an early
`post(0)` call is still not established as the complete component port.
Coincident gear-transition coverage, broader collision paths and ordinary
uninstrumented trajectory comparisons remain open. No FPS cap, tolerance
relaxation or contact-test weakening is introduced.

## Positive-dt boundary mapping and isolated bump replay

Source inspected: `ae2dbbb` plus evidence commit . This follow-up changes no production physics. Native capture details and controls are in "Fresh native positive-dt component boundaries" (above).

### Boundary mapping

| Native boundary | Clone location in src/drive/ | Finding |
| --- | --- | --- |
| 5B04E0 normal component traversal: body, wheel/suspension pairs, engine, clutch | substep, lines 1977–1998, 2129 onward, 2449–2477 | Clone measures suspension before its Euler integration; engine/clutch post runs after integration. Native normal-post has freshly changed compression before its rest/bump test. The implementations partition these operations differently. |
| 5ABB10 rest test after normal-post | lines 2480–2503 | Clone also computes rest after normal post, and applies wheel/momentum rest changes. |
| 5ABB41–74 full-compression wheel loop | lines 2000–2036 | Clone defers this block to the next substep's start. Native performs it before the captured next-force boundary in the same group. This alone does not establish a one-step force error: clone's next force accumulation also follows its deferred bump block. Frame-end and input-transition equivalence remains unverified. |
| 59E1B0 impulse calculation | lines 2024–2030 | Concrete mismatch: native reads unchanged cached linear/angular velocity for every wheel, changing momentum only. Clone changes linear velocity after each wheel, so later impulses depend on earlier impulses. |
| 5A0D90 zero-dt component traversal | lines 2032–2036 and 2514 onward | Clone refreshes spin and engine/clutch state explicitly. Native traverses all 11 component objects. The capture verifies traversal and velocity refresh, but does not establish every component's mathematical effect. No wholesale relocation is justified by these data. |

### Replaying actual native bump inputs

The script `replay_bumps.py` (kept with the maintainer's private evidence) replays them. Its saved output is `bump-replay.json`. It verifies the raw capture hash, reads each actual bump-entry hub, cached body matrix, normal, mass, velocity and spin, and compares the reconstructed impulse with the corresponding native x87 return. The matrix transform and cross product are supported by retained `bump-formula-disassembly.txt` (441000, 43D1E0, 59E1B0).

All 39 pairs match within an absolute 1e-5 check; observed maximum error is **5.901805821828532e-8**. The small residual includes Python-double versus native x87/f32 intermediate rounding; this is not bit-exact emulation. Every pair also verifies cached linear and angular velocity are byte-equivalent as decoded floats across the native impulse call.

The second calculation holds the captured geometry and starting velocity fixed, but applies the clone's sequential linear-velocity update between wheels. At step 134:

| Wheel | Native return | Native formula | Clone sequential formula |
| --- | --- | --- | --- |
| 0 | 1.7469496727 | 1.7469496896 | 1.7469496896 |
| 1 | 1.7537295818 | 1.7537296409 | 1.3606659649 |

Thus the later-wheel difference survives identical native input state; it is not attributable to track geometry or an invented clone starting state. This isolates the algorithmic mismatch but **does not execute CarState::substep**. A full replay needs matching native-to-clone pose, component histories, world contacts and handling conversion. Those have not been established here, so no full trajectory-parity claim follows.

The actual native throttle changes occur before and after the impact. Actual gear and target gear remain 2 throughout compression entry, and clutch phase remains Engaging through the bump refresh. There is no captured compression/gear-transition coincidence to replay.

### Bounded next change

A justified candidate is to retain the pre-loop linear velocity for all bump closing-speed calculations, accumulate the resulting impulses, and refresh velocity once after the loop. Keep the existing scheduling until a separate boundary replay proves which operations can move. Add an actual Rust helper regression seeded from these native inputs before changing production behavior. This report makes no such change and does not claim full rest/component parity.

## Expanded native boundary capture and suspension-load diagnostic

This continues "Positive-dt boundary mapping and isolated bump replay" (above). It does not relocate production component passes or claim a full `CarState::substep` replay.

### What was missing, and what was captured

The first capture retained component objects but omitted the engine-record pointee used immediately by `5AA770`, the full drivetrain, mover history and source handling record. internal evidence audits the earlier capture's actual address coverage; its output is `audit.json`.

A fresh private native session on display 174 retained those dependencies. The expanded recorder keeps all original boundaries and adds full car (0x600-byte window), mover (0x200-byte window), drivetrain (0x100 bytes), live engine/gearbox records, four wheel records, and the car's source record. The windows describe bytes captured, not asserted C++ object sizes. There are 27 retained regions and 10816 used bytes in each 16384-byte record. Pointer checks verify the live owner and pointees at every boundary.

`expanded-landing` has 2530 records, 550 complete groups, 112 rest-triggered zero refreshes and 15 bump-triggered zero refreshes, with zero dropped records. The initial control is a +0.6 m position perturbation and -12 m/s vertical velocity expressed through actual native mass/momentum; the metadata records every write and throttle key transition. Compression, handling, clutch and wheel values were not injected. The source executable SHA256 is unchanged, all hooks restored, and the owned Wine/Xvfb session is stopped.

Reproduction uses the retained launcher with the owned prefix, followed by (the scripts are kept with the maintainer's private evidence):

```
python3 fastnav.py WINE_PREFIX race
python3 capture.py WINE_PREFIX /PRIVATE/OUTPUT/expanded-landing --seconds 2 --drop .6 --vertical-speed -12
python3 decode.py /PRIVATE/OUTPUT/expanded-landing
```

The capture pauses the game before disabling logging, then validates complete groups. Actual native dt and ticks are retained; reproduction is not promised bit-identical. Runtime navigation/capture logs are in the sibling `positive-dt-boundary-replay-runtime` directory. `test_decode.py` reproduces the summary and rejects a rehashed changed live engine pointer and a rehashed truncated group.

### A measured zero-time suspension effect

`5A87A0` preserves compression rate when dt is zero (`5A8877..5A8897`), but still recomputes spring/damper/preload/anti-roll load (`5A8AA0..5A8B22`). In the normal component traversal, each left suspension reads its right neighbour's previous compression. After all four suspensions update, the repeated zero-time traversal reads current neighbours on both sides.

At captured step 135:

| Suspension | Normal-post / zero-pre load | Zero-post load |
| --- | --- | --- |
| 0 | 38.5386962891 | 38.3348274231 |
| 1 | 23.4488868713 | 23.4488868713 |
| 2 | 43.5734596252 | 42.9173660278 |
| 3 | 43.8518676758 | 43.8518676758 |

Compression, rate and preload are unchanged between those zero boundaries. The load differences follow the neighbour term, rather than elapsed time or a gear transition. These are raw suspension loads at +0x24, **before** the tyre's camber multiplier; `WheelState.load` normally stores the later tyre load, so the two are not silently equated.

### Actual CarState component diagnostic

`WheelState::suspension_load` extracts the existing production arithmetic without changing operation order or pass scheduling. The ordinary tyre loop calls it before applying camber. The `native_zero_refresh_recomputes_suspension_loads` test constructs a CarState and seeds every wheel compression/rate read by that component from the native snapshots. Handling comes from the captured native car record; its two suspension records are byte-checked against the live suspension-record pointees. Preload comes from native suspension +0x88.

The test executes the shared production component calculation in native wheel order for both traversals, comparing all 60 wheel results from the 15 bump groups to native first-pass and zero-pass loads at absolute error below 1e-5. Its fixture is generated only from raw input/output snapshots (the scripts are kept with the maintainer's private evidence):

```
python3 fixture.py
cargo test --release --lib native_zero_refresh_recomputes_suspension_loads
```

This is an explicit boundary projection: pose, ground queries, aero, tyre camber/forces, engine/clutch, rest decisions and body integration are excluded because this component does not read them. Default values in the constructed CarState are not treated as captured native history. The diagnostic manually invokes the second calculation; production still lacks that second suspension traversal. Passing this test therefore proves the component calculation and the observed missing traversal's effect, not that production scheduling is corrected.

### Remaining boundary work

Native order remains normal component post → rest → full-compression impulses → optional zero component post → next-force input. Event 8 at 5B0520 is immediately before tail dispatch to 5B0530, not a render-frame marker. The clone measures suspension at its next substep start, performs its deferred bump block, then computes suspension loads/forces and integrates. Seeding a whole clone substep at native normal-pre would incorrectly start another integration from an already integrated native pose.

The capture's normal-post body has authoritative momentum and refreshed velocity. Between bump calls the momentum changes while cached velocity stays fixed; zero-post refreshes velocity. CarState has separate angular momentum/spin but stores only linear velocity. The bounded bump helper holds cached velocity while summing impulses, then applies the sum once. Native accumulates each impulse into existing momentum. That rounding/order distinction remains documented; no tolerance was widened and no complete state/precision equivalence is claimed.

For a full next-force replay, the missing ground-query context at car+4 (pack/section/recent floor history), aero-component frame setup, collision-pair history and exact native-to-clone phase projection still need mapping. The full-car snapshot retains pointers to some of these objects, not their pointees. A future capture should follow those specific pointers after tracing their layout. It should not replace absent state with CarState::new defaults, infer counter timing from rendered frames, or relocate passes on the strength of the bounded impulse/load diagnostics alone.

Validation: the targeted native-load diagnostic passed; the full library run passed 459 tests with 7 ignored. `validation.txt` retains commands/environment, and `SHA256.json` covers the raw capture, recorder, decoder and fixtures. The decoder negative controls and fixture byte-for-byte regeneration also passed.

## Bump-stop velocity cache correction

The native boundary capture and formula replay in "Positive-dt boundary mapping and isolated bump replay" (above) identified a later-wheel impulse error. `CarState::substep` now uses the shared `bump_stop_impulses` helper: all fully compressed wheels read the same cached linear/angular velocity; their impulses accumulate before velocity refresh. The first hard-landing group's second wheel previously used the first wheel's already-updated linear velocity.

The helper retains each impulse's 1000 linear and 10 angular cap, wheel iteration order, the existing rest gate, and the refresh when a full-compression visit returns zero impulse. Component pass scheduling is unchanged. In particular, the clone still schedules this block at the following substep's start; this patch does not resolve the larger boundary-equivalence question.

### Native regression

`bump_stop_loop_matches_native_hard_landing` exercises the actual production helper across all 17 recorded bump groups / 39 calls. Its CSV contains native pre/post linear and angular momenta, actual cached velocities/spin/mass/normals, and hubs transformed by the captured body matrix. Generate it with (the scripts are kept with the maintainer's private evidence):

```
python3 replay_bumps.py --fixture
```

Expected group momentum changes are taken directly from the native raw snapshots. The test checks both linear and angular changes with absolute vector error below 1e-5; that threshold was not widened. A second regression checks separating/zero-impulse visits, absent visits, and per-wheel caps. The initial two-wheel native regression failed with the extracted old sequential loop: linear impulse Z was 3.1053867 instead of 3.4981685.

This is a bounded impulse-loop regression, not a full trajectory replay. The clone stores linear velocity rather than the original body's separate authoritative momentum and cached-velocity fields. The helper sums impulse deltas, then applies each total once, while native adds each impulse to the existing momentum. Consequently f32 accumulation order is not bit-identical. The retained native group regression bounds this difference for the recorded landing; it is not an all-state precision proof. No full burnout or rest/component parity claim follows.

Validation: `cargo test --release --lib --test tuning_physics --test high_refresh -j2` with the original asset root and isolated target/XDG paths passed: 458 library tests (7 ignored), 3 tuning tests, and 1 high-refresh test (1 ignored). Library coverage includes the contact regression and both new bump tests. The base binary still marks reverse high-refresh coverage ignored; this patch neither changes that status nor claims to validate later parent changes. Native decoder audit and 39-call Python replay also pass, and the CSV regenerates byte-identically. Logs are retained in the sibling `positive-dt-native-capture-runtime` directory: `bump-red.log`, `bump-final-tests.log`. No Wine or GPU run was needed for this offline patch.

## Rest and bump-stop component refresh

This corrects the reason the native engine/clutch zero-time post was
previously omitted. It does not claim a complete port of every component
update or full collision parity.

### Native path and correction

`0x5B04E0` updates the component list, then calls `0x5ABB10`.
`0x5ABB10` calls the rest test `0x5A5E30`. A nonzero result requests
`0x5A0D90` with global step length `0x7FC800` set to zero. Otherwise,
full-compression suspensions call `0x59E1B0`; `0x5ABB63` requests the
refresh even when that helper gives zero impulse for opening velocity.
`0x5A0D90` calls each list member's vtable +0x1C, including engine
`0x5AA770` and clutch `0x5A1410`. A zero elapsed time does not prevent
state transitions: Disengaging with clutch 0 becomes Changing, then a
pending change becomes Engaging on the next update.

The clone now calls its existing engine/clutch post with dt 0 after rest
holding and after bump-stop handling. It uses the wheel speeds after the
hold/impulse. Full compression also requests that refresh when the hub
moves away from the surface. No frame rate, integration timestep,
collision threshold, restitution or separation behavior is changed.

### Why the old collision test failed

Reproducing `cars_bump_and_part` with the refresh restored gave two cars
at rest and `narrow::distance == 0`, as reported in round15. Exporting the
actual hull vertices showed **no penetration**: they have a 1.67 mm gap
along the head-on axis and a 1.94 mm gap along another separating axis.
The previous baseline also stopped both cars, with a 2.79 mm query result.

Native `0x5B3672..0x5B36ED` computes the initial support points' squared
distance and compares it to float `0x7A1934` = 9.999999747e-6. Near supports
can therefore take the zero-distance exit even when the full hulls are
separated. The clone retains that rule. A fresh Wine harness executing
those unmodified bytes confirms 1/2/3 mm take the near-zero path and 4/10 mm
continue. Its continuations are explicit harness stubs; this validates
the decision, not a complete native collision simulation.

The regression's strict `query > 0` check was an invalid overlap oracle.
It now accepts a strictly positive independent separating-plane gap as
well. This cannot accept intersecting convex hulls on that plane and
introduces no tolerance. It also detects the hit from actual impulses,
rather than a speed drop that might merely be engine braking.

### Runtime evidence and tests

`docs/evidence/rest-components/` preserves the threshold harness, hashes,
final clone hulls, fresh REA clutch output, and a 64-record excerpt of
an native 206 `x2` telemetry with original-source/excerpt hashes.
The latter is recovered original Wine evidence (instrumented capture,
initial teleport), not fresh the merged branch game playback. Its stationary frames
show the previously documented 0/2 clutch cycle. The metadata and decoder
retain actual frame lengths; there is no synthetic fixed-rate claim.

Two asset-backed regressions failed before their respective corrections:

* Held car, dt 0: expected Engaging but got Changing; position and engine
  heat must remain unchanged.
* Full-compression bump stop moving upward, dt 0: expected Engaging but got
  Changing; the body's upward velocity must remain unchanged.

The former proves the second discrete update, and the latter proves that
an impulse is not required to request it. Final validation results are
recorded in internal evidence.

### Remaining limits

The existing clone schedules bump stops at the next substep's beginning,
whereas native schedules them after end-of-step component updates.
Suspension/wheel state evaluation remains partly deferred; this patch
restores the omitted engine/clutch portion, not an entire native list
refresh. Contact resolution still refreshes suspension without all the
native zero-time engine/clutch/rest callbacks. Those ordering differences
need separate regression and native comparisons. The recovered 206 trace
has not been replayed end to end against this exact commit, and the fresh
harness is not proof of native car-to-car trajectories. Uncapped gameplay
and the separate wheel guard work are preserved.

The head-on regression now waits for a positive `Touching::Car` contact
impulse on both cars. A ground push can no longer satisfy its collision
gate; its existing strict separating-plane assertion is unchanged.
This strengthens the collision oracle only. The dt=0 bump regression
still cannot establish positive-dt ordering around force gathering and
integration. Full-compression entry/exit with changing throttle or gear
and positive dt remains open; no scheduling correction is claimed here.

## Suspension loads after the existing zero-time component post

This is a bounded production correction to the load consumed by the next force pass. **The full normal/rest/bump/zero traversal is still not implemented with native ordering.** Rest classification, deferred suspension measurement, complete zero-time component updates and full trajectory replay remain separate work.

### Consumer and refresh gate

The native component list places each wheel before its suspension. That does not make the tyre-force pass consume the earlier suspension load: after the complete normal/rest/bump/zero traversal, `5B0530` reads each suspension's +0x24 at `5B0590..5B05A9`, calls `59D170`, and stores the camber-adjusted value at wheel +0x164 (`59D228`). The tyre pass uses that field at `5A84EA` and `5A8513`. The retained disassembly records this dependency.

Across every one of the expanded capture's 550 groups, a zero-time traversal occurs exactly when the observed rest result is nonzero or the moving-car branch visits a fully compressed suspension. A visited bump requests the traversal even if its impulse is zero. Counts are 112 rest, 15 bump and 423 normal-only controls. The fixture records actual next-force-entry load from event 8, rather than predicting it from a final suspension formula.

The exporter verifies that compression, rate, preload and on-ground state are unchanged between normal-post and next-force-entry for all 2200 wheel cases. Thirteen left-wheel loads change because the zero traversal reads the right neighbour's newly updated compression.

### Minimal production plan

1. Keep current measurement, rest, engine/clutch, bump impulse and integration scheduling.
2. Retain the bump helper's visit flag, not whether it returned a positive impulse.
3. At the existing suspension-load calculation, choose the current neighbour compression if an existing zero-post site was requested: nonzero `CarState.rest`, or a visited bump. Otherwise preserve the normal pass's sequential neighbour state.
4. Leave rate/preload/contact and camber calculations in their current locations. The selector plus existing raw load calculation supplies the force input implied by repeating the native load calculation with unchanged compression/rate/preload.
5. Validate against all 550 captured groups through the shared production selector, with the old selector failing the native next-force loads first.

This folds the repeated load calculation into its final result; it does not pretend to execute all native components twice. The current `CarState.rest` is the class computed by the existing clone rest test; this patch does not prove that classification or its timing matches native in every situation. The selector follows the existing zero-post sites rather than inventing a new rest decision.

### Remaining replay prerequisites traced statically

* Ground query: `59D274..59D285` follows wheel +0x20 to the physics view (full-car +0x40), then reads view +0x4 (full-car +0x44), then `5E7E80` forwards to `5E72F0`. The context's +0x8 is the selected pack (`5E7311`); +0x2BC and +0x2C0 hold recent polygon count/list (`5E7362..5E7384`). Those polygon pointees carry plane/flags and were not retained by the expanded capture. The subsequent `evidence/full-phase-inputs` capture retains them and verifies their source-face pointers; that does not by itself establish full phase replay. A repeat query cannot generally be replaced by a fresh default `GroundQuery`.
* Aero: the physics mover's aero pointer is **+0x50**, as `5ABDDB..5ABDF9` writes throttle (+0x10) and frame wheels-down share (+0x14). `5991CA..5991E2` writes catch-up (+0x08) and draft factor (+0x0C). Earlier exploratory +0x24 suggestions were wrong for this mover; +0x24 belongs to a different layout. The aero pointee must be captured for an authoritative full force replay.
* Momentum/cache: native bump calls accumulate authoritative momentum while cached velocities stay fixed until zero-post. The clone helper preserves cached inputs, but sums deltas then adds them once and has no independently stored linear momentum field. Its floating-point sum order remains different.
* Native normal-pre is already after integration. A whole `CarState::substep` cannot be seeded there and treated as the corresponding native component update. A full refactor must preserve the phase of wheel histories and input/gear transitions, not just match one load result.

No FPS limit, timestep cap or tolerance change is part of this correction. Ground-query repetition, externally requested native reset paths and the remainder of the component traversal are not claimed equivalent.

### Validation

The old shared production selector failed at native step 135 wheel 0: 38.538696 versus next-force load 38.334827. The corrected selector passes all 550 groups / 2200 wheel cases with unchanged 1e-5 absolute error. The exporter also verifies the captured Handling and live suspension-record bytes for every group. Fixture regeneration is byte-identical.

The release library run passed 460 tests (7 ignored); track/contact passed 26, including held-car and zero-impulse bump refreshes; high-refresh driving passed 1 with the existing reverse case ignored on this base. Commands and environment are retained in internal evidence. No full-substep equivalence follows from these projected component tests.

## Retained tyre forward speed during landing

The production tyre force pass now uses a forward-speed snapshot throughout
one `simulate` call. Previously it recomputed speed from the changing body
velocity in every substep. The native tyre consumers read physics-view +0x3EC
(full-car +0x42C), retained across those inner steps. No existing `CarState`
field represented this scalar; `speed()` remains the live projection for its
other consumers.

This is a bounded cache correction. Runtime samples at `simulate` entry,
including collision re-steps, beside the existing cached contact count.
Native 5B74E1 instead publishes through 5A5550 after its inner integration
loop: 5A564C..5A5665 computes the exported forward speed. Matching the full
native export lifetime, all recovery/contact writers, and runtime orientation
representation remains open. This change does not claim that lifetime parity.

### Native observation and force replay

The retained the maintainer's private capture is a controlled original
landing (+0.6 m height, -12 m/s vertical velocity), not a natural jump claim.
All 559 force entries span 250 world/tick groups. Exported speed never changes
within one such group. The observed PC24/nearest control word is retained in
that source. Exact EXE bytes for writer and consumers are alongside the fixture.

Three supported landing boundaries distinguish the old consumer:

| Step | Live body forward speed (m/s) | Native exported speed (m/s) |
|---|---:|---:|
|129|0.486394|-0.603459|
|131|0.648816|0.094461|
|133|0.611459|-0.497222|

Native 5A85AE compares **signed** exported speed against one mph. These
observations take its low-speed sliding branch, dividing forces by sliding
mu; the old live projection skips it. Rear force differences also affect the
front rolling constraint. 59D4A0 reads the same scalar for rear launch grip.

The preserved expanded diagnostic in an internal run folder initially
passed 115 supported boundaries. The separate cached-wheel-count correction
raised that to 125. Supplying captured native forward speed raises it to all
128, with unchanged wheel/body thresholds 1e-4 and engine thresholds 1e-5.
181 other pairs explicitly fail prerequisite coverage because the live brake
record was not captured; they are not counted as passes. The full force replay
supplies native body matrices, contact caches, histories and scalar inputs.
It does not establish that runtime produces those same inputs, or prove full
substep integration/post-pass parity. Its larger shared force-input refactor
remains separate from this commit.

### Regression scope

`native_landing_sliding_uses_cached_forward_speed` restores six actual rear
wheel inputs from steps 129/131/133 and compares the shared production sliding
function with observed native final forces. Their zero drive torque, spin,
low longitudinal velocity and inactive circle limiting isolate this consumer.
Old live speed must differ by more than one force unit for every fixture.
The 1e-4 bound is unchanged from the larger force diagnostic. A separate test
checks the production cache/read methods across velocity changes and refresh.
These six observations do not cover the high-speed launch-grip ramp; that
consumer's shared scalar selection is supported by its native reader trace.

Reproduce fixture extraction and focused tests (the scripts are kept with the maintainer's private evidence):

```
python3 export.py
cargo test --release --lib native_cached_speed -- --nocapture
```

`manifest.json` records source hash and exact event indices. Expected force
bytes are copied from the next matched native normal-pre boundary, never
calculated by the clone. The fixture is independent of the cached-wheel-count
commit; the production change does not alter drivetrain or handling parsing.

### Validation

All 497 release library tests passed (9 existing ignored), including both
new regressions. High-refresh driving passed at 60–500 Hz (existing reverse
case ignored); all 3 tuning tests passed. Maximum native sliding force error
is 7.6293945e-6. The initial full-library attempt lacked NFSU2_ROOT and failed
one asset-dependent map-pack test; the complete run with the correct isolated
environment passed. Fixture regeneration and git diff whitespace checks pass.
Log hashes and exact terminal counts are retained in an internal validation record.

## Explicit exported forward-speed lifecycle

Ordinary physics now consumes the previous exported forward-speed scalar
throughout its substeps and contact re-steps, then publishes at mover exit.
Previously every `simulate` sampled current velocity, so a contact impulse
changed the tyre input before the native export boundary. In a real two-car
world regression the old re-step consumed−1.4191252 instead of the preceding
13.964559 export. Both that case and the post-motion callback regression fail
before this change and pass afterward.

Native evidence and its exact scope are in
[evidence/tyre-export-lifetime/README.md](../../evidence/tyre-export-lifetime/README.md).
Two untouched live captures establish3514 mover/export triples,276 contact
sequences, and one natural R/actionA1 human road placement. The new compact
fixtures copy3514 native exported vectors/scalars and one placement argument,
axis, velocity and scalar directly from those captures. Production arithmetic
matches all these raw values bit-for-bit, with no tolerance changes.

### Production boundaries and APIs

`CarState::publish_forward_speed` uses native PC24 `(vz*az + vy*ay) + vx*ax`.
Ordinary `simulate` calls it after its substeps and before returning to the
post-motion recovery callback; contact re-steps also publish when they finish.
Rewind, contact impulses, suspension/component refresh and bump/rest processing
do not republish. The force-loop consumer therefore retains the prior export.
The test observer's “entry” is immediately before that force loop, after frame
setup; it is **not** the native recorder's earlier mover-entry hook.

`set_exported_forward_speed` handles explicit scalar writers, and
`set_placement_speed` separately writes the argument and installs axis×(argument
+0.1). The native reset capture observes scalar20.959133→0→8→8, velocityaxis×8.1,
then the next mover retaining8 until its own export. Parameter8 versus velocity8.1
is intentional. Existing recovery route/pose choice and live `speed()` readers
are unchanged; this does not port the entire native teleport implementation.
The public getter exposes the retained scalar for writer/consumer regressions.

`speed()` still projects current velocity. `aero_down` and its current sampling
lifetime are unchanged. No cache-valid flag, lazy fallback, timing cap, control
threshold or test timeout was introduced. Ordinary race hold follows normal
physics export and never forcibly zeros a moving scalar.

### Direct writer audit

- **Constructors / generic `Drive::place`:** stationary state has scalar0.
  Nonzero callers must explicitly publish their intended initial condition.
- **Human, route-trigger, rival and encounter recovery:** use the native
  placement argument/velocity split. Their preexisting pose/controller resets
  and recovery parameter computation remain separate approximations.
- **Traffic placement:** uses lane speed as scalar and lane speed+.1 as velocity;
  the mover still initializes from the latter. No double+.1 is added.
- **Encounter spawn:** already installed lane-direction×(lane speed+.1); now
  also publishes the original lane argument. Existing route-direction versus
  quaternion-axis differences remain explicit.
- **Traffic / belt motion (`place_on_road`):** publishes the mover speed, which
  can differ from the translated pose's velocity projection. Ground/pose and
  belt movement retain existing approximations. The belt rebuild failure stops
  velocity and explicitly publishes that synthetic0. `Input::Belt` in physics
  is a genuine no-op and does not invent another publication.
- **Melo:** publishes at its existing5A2B70 write-back. `leave_melo` transfers
  drivetrain state and retains the just-published scalar for ordinary physics.
- **Intro, shop and ending poses:** converge through `scene_wheels`, which now
  publishes after their existing channel-derived motion. This is explicit
  integration with the existing NIS approximation, not full NISMover parity.
  Enter cut/cut seek still yields zero velocity/spin and publishes zero.
- **Dyno:** the deliberate post-settle reset clears the scalar (56D3D2).
  `Game::dyno_pose` only tilts the display proxy from the separately simulated
  `DynoPull`; it does not inject a new driving velocity. Exit scene motion
  passes through the scene publisher before ordinary driving resumes.
- **Synthetic motion:** camera/shot probes, benchmark/playthrough staged
  encounters, tests that deliberately install moving cars, track collision
  fixtures and the weight probe now explicitly seed a projection. They remain
  synthetic initial conditions, not native placement (+.1 is not added).
- **Older hit replay:** its log has no retained scalar; it explicitly seeds
  the reconstructed runtime projection. This remains an approximation and
  cannot serve as a native contact-entry cache oracle.
- **Retained native force fixtures:** supplied native scalars remain supplied;
  they are not overwritten by a runtime projection. Read-only AI driver tests
  that never run physics continue to read live speed and need no migration.
  Particle velocities and independent NIS primitive bodies are not CarState
  exports and were not modified.

Early-return audit: ordinary `simulate` has no rest or dt early return; only
Belt (no-op) and Melo (separate writer) return early. Native5B73C3 can skip its
integration loop but still reaches5B74E1 export. The scalar publication location
follows that static writer; the positive-dt captures do not establish zero-dt
component/clutch equivalence. Invalid dt behavior is outside this change.

### Validation

Private runtime: an internal run folder supplies
NFSU2_ROOT pointing to read-only verified assets, private XDG/TMP/cache, owned
`positive-dt-force-target` and2 build jobs. Source baselineb9935e1 includes the
cached-count, Enter fix, particle/HUD and StreetX test-driver work. Evidence
commits were applied separately before implementation.

```
cargo test --lib --test track_l4ra --test high_refresh --test tuning_physics --bin game -- --nocapture
cargo test --bin game intro_skip_preserves_ai_grid_handoff -- --ignored --nocapture
NFSU2_INTRO_NIS=13 cargo test --bin game race_start_wheel_phase_survives_scene_placement -- --ignored --nocapture
cargo run --bin game -- --selftest
cargo check -p devprobes --bin hit_replay -p devprobes --bin weight_probe   # maintainer only (private developer probes)
```

Results: library511 passed/12 existing ignored; game19 passed/12 ignored;
track26 passed; high-refresh1 passed/1 existing ignored; tuning3 passed.
Both explicit ignored scene tests pass. Enter's movingIntro03 and stationary
Intro25 paired runs at60/240Hz retain zero target/cut/green/progress gaps.
Full selftest passes (cash6650, stage1, wins3). Native cached-count drivetrain,
landing tyre-force, nonunit-axis camber, hub and suspension regressions remain
passing. Landing force maximum error stays7.6293945e−6 under its unchanged1e−4
bound. Six new library lifecycle tests cover actual mover callback, actual
contact re-step, actual placement consumption, explicit scripted/traffic input,
actual Melo handoff, and all raw projection/placement fixture rows. A real game
traffic-pose test distinguishes mover parameter from translated velocity;
existing recovery and scene integration tests now assert scalar publication.

The broader preserved force replay's128 supported /181 missing-brake cases are
not full-world validation and were not transplanted into this commit. This
change does not close native raw-matrix versus unit-quaternion fidelity,
complete force/component ordering, aero/count publication lifetime, or all
NIS/traffic/placement behavior. Positive-dt scalar lifetime is the verified slice.

## Drivetrain reaction uses the exported contact count

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

### Observed phase and mismatch

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

### Regression and explicit limits

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

### Expanded replay context

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

### Reproduction

The scripts below are kept with the maintainer's private evidence.

```sh
python3 export.py
cargo test --release --lib native_airborne_drivetrain -- --nocapture
```

The exporter hashes the original retained full-phase stream, keeps source
record identifiers, and copies actual native inputs/outputs. It does not
calculate expected engine or clutch states. `manifest.json` and
`lifetime-audit.json` retain provenance and the contact-count audit.

### Final validation

The release library suite passed 496 tests (9 existing ignored). The focused
native regression passed all 10 cases. High-refresh driving passed from 60 to
500 Hz (the existing reverse case remains ignored); all 3 tuning tests passed.
Fixture regeneration is deterministic; `git diff --check` passed. Full logs
remain in the internal run logs; an internal validation record retains
their hashes and exact result lines, including the broader diagnostic's
three unresolved failures.

## Scene momentum handoff correction

The drag diagnostic on track4201, seed20261103, reproduces all three rivals rolling before green at60 and240Hz. At60Hz frame358, rival0 has zero scripted spin but momentum(-4.262683,-6.339113,-0.04888056). At frame359, after camera cut stops placement, physics derives spin(-2.9257383,-4.2093377,-0.018266737), without a contact impulse. Scene placement reset pose/spin but retained accumulated physics momentum.

`CarState::set_scripted_spin` sets momentum using the same rotated inertia tensor inverted by `spin_from_momentum`. Scene placement uses it after setting orientation. Scripted yaw is preserved; this does not zero arbitrary driving momentum, change forces or cap FPS.

Root validation: countdown rollover assertion passes at60 and240Hz, with three rivals and green reached. Unit test verifies rotated anisotropic inertia roundtrip in both ground/air modes and stationary momentum reset. Existing IntroNis13 wheel phase regression passes. Logs in workspace review:drag-rollover-momentum-60hz.log,drag-rollover-regression-final.log,drag-rollover-final-240hz.log,scene-momentum-unit.log,rollover-wheel-regression.log.

Scope: corrected internal state consistency in the existing scripted-placement approximation. Full native NISMover trajectories remain open; this does not prove every drag collision or track matches native. Player stays on grid during reproduction.

### Expanded countdown regression

The selectable diagnostic now accepts ROLLOVER_TRACK from its enumerated drag-mode list. All10 tracks(4201,4401,4402,4212,4213,4221,4222,4223,4404,4211) passed at60 and240Hz:20 asset-backed runs with three opponents, green reached, and no pregreen rollover. Seed20261103, player stationary. Track4201 also passed500Hz. Reproducer and per-run logs are the maintainer's private captures in the private workspace. This expands countdown coverage, not native race trajectory or every collision coverage.

## Race intro skip places every car at the authored cut

Pressing Enter during a moving race intro advanced its clock to the cut but
left its cars at their previous scene positions. The next update passed the
`run.frame < cut` placement guard, so the held countdown retained those old
positions. This reproduces the reported AI cars starting behind the line.

The fix invokes the existing slot-based `scene_cars` placement when seeking,
before changing the clock. Both sample times are the cut: this is a seek,
not simulated travel over the skipped interval. It therefore generates no
artificial velocity/spin and retains `set_scripted_spin`'s angular-momentum
consistency. Existing wheel phases survive; progress is placed on the road.
The controller route approximation is preserved.

### Native basis

The verified executable has SHA256
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.
Unmodified bytes for the relevant ranges are disassembled in the evidence
folder. `43B460` seeks to StartCountdown through `43AF80`; that function
resets car binding/first-placement fields, calls `439240` at `43AFDF`, then
stores the new clock. `439240` evaluates each bound car channel. The reset
first-placement flag causes `430A75..430A81` to call `5FBD70` at the requested
pose. Merely changing the clock omits this placement.

The existing original-Wine observation and trace are recorded in
`formats.md`, “Race start scene cars” (grid145/finish151). This task rechecked
the executable bytes and the clone integration; it did not make a fresh native
runtime capture. Full NISMover trajectories and native teleport/controller
reset lifecycle remain open. The fix retains existing ground-derived Z and
model-origin/centre-of-mass conversion; it does not claim complete scene parity.

### Paired reproduction

The asset-backed game integration uses the first career drag event,
STAGE_1_DRAG_1, track 4201 variant 2, private stock PEUGOT profiles, and the
normal `Game::key` path. For each scene/rate, event and seed are identical
between full intro and Enter at 0/10/50/95 percent of the pre-cut duration.

The initial moving IntroNis03 reproduction at 60 Hz, skipped at 10 percent,
left AI slots 2/3/1 behind the complete-intro cut poses by 9.537/18.211/13.397 m;
at green the gaps were 9.167/17.048/10.155 m. IntroNis25 was a stationary control
and did not show that large displacement. Red and green log hashes are retained.

The final regression selects moving IntroNis03 naturally with seed 4 and
stationary IntroNis25 with seed 0, asserting both selections. It runs at 60 and
240 Hz with four skip times. It checks immediate authored target XY for the
player and every rival, zero seek motion/angular momentum, preserved route
node/driver/control/RNG state, and matched AI cut/green poses and progress.
Authored channels specify centre-of-mass positions: raw model-origin offsets
of roughly 0.19–0.245 m are not target-placement errors. The regression converts
the state back to its physical centre before checking the authored target.

Run in private XDG directories with NFSU2_ROOT set to the original installation
and NFSU2_INTRO_NIS unset:

```
cargo test --release --bin game intro_skip_preserves_ai_grid_handoff -- --ignored --nocapture
```

This is a CPU integration test using real assets; no rendered/native visual
match is claimed. Other intro assets and the exact user save are not covered.

### Final validation

The explicit paired regression passed all 20 runs: 64 immediate player/AI
centre targets and 48 cut/green/progress/target comparisons had zero error.
No tolerance was relaxed. Route/controller input state remained unchanged on
the key, and all sampled racers remained upright through green.

The broader game-test binary had 17 passes, 10 ignored and one failure:
`wrong_way_street_x_on_keys` did not finish track 4608 variant 0 within its bot
budget, with zero wrong-way/text errors. A fresh isolated unmodified 03f3c60
worktree reproduces exactly that failure. This test never sends Enter; the
failure predates this patch. It remains open and was not weakened or relabelled
as passing. The integration branch must still run its normal package checks.

## Native scene target arithmetic port

The specification is original0x5A44B0 and its64 retained native input/output
snapshots, documented in `nis-mover-step-investigation.md`. This step ports that
helper into Rust without changing the live scene loop. The existing scene loop
still needs native component ordering, contact state and mover lifecycle; using
this helper alone cannot establish or replace that work.

`drive::nis::TargetBody` retains separate momentum and cached velocity/spin,
plus the base and selected inverse inertia. Those distinctions are required by
the original helper and cannot be represented by treating CarState.velocity as
both values. Target application has two native branches: direct velocity/yaw
assignment with base-tensor momentum reconstruction, and capped impulse/moment
application that deliberately leaves the velocity caches unchanged. Both retain
position, orientation and vertical cached velocity. The direct setter rebuilds
all linear momentum from cached velocity times mass; it can change vertical
momentum if that cache was stale. The impulse path retains vertical momentum. The native
angle16 conversion is reused; no fixed frame interval or render cap is added.

Verification sequence:

1. A no-op implementation must fail against the original snapshots.
2. Implement both branches with original constants and float storage boundaries.
3. Compare all64 cases, all affected momentum/cache components and preserved
   pose, using the existing original-oracle arithmetic bound (2e-6 relative,
   2e-5 absolute). Do not loosen it to accommodate differences.
4. Run the library regression suite and retain exact results.

The fixture covers identity/90-degree orientation, both selected inertia states,
two positive frame intervals, wraparound headings, nonzero vertical motion/spin
and both impulse caps. It is not broad coverage of arbitrary tilted tensors or
near-boundary angle-table rounding. No fullscene or all-input bit-exact claim
follows from passing these cases. Runtime wiring stays open until the full
mode-5 frame path is established.

Validation: the no-op failed on native case0 (cached x velocity1 versus
1.2500285). Both branches then passed all64 original snapshots without changing
the tolerance. A subsequent review preserved native dx extended-precision
multiplication separately from stored dy, and the final complete library run
passed468 tests with7 ignored. Logs (red, green, final) are retained in the
private workspace. The first attempted test compile
had an incorrect relative fixture path; it was corrected before the genuine
no-op failure was recorded. No game binary/package behavior changes follow from
this unwired helper.

## Drift self-test relocation premise

The integrated `--selftest` failed at its final drift ranking assertion:
player NAMERay scored 21610, ahead of AI scores 11807/9670/8736. Sorting the
highest score first was correct; the test assumed its artificially relocated
player would have fewer points without enforcing a non-drifting trajectory.

### Controlled comparison

A temporary diagnostic logged player score, pose,
velocity, spin, momentum and wheel state before updates and each relocation.
The same diagnostic executable was run with native camber and with only the
old unit-axis camber shortcut selected. No other source changed between those
runs. The temporary selector and trace code are **not** in the final patch.

| Observation | Native double-cross camber | Old shortcut |
|---|---:|---:|
| Before first relocation, t=90.0323 s | score 0, velocity 0, slip 0 | score 0, velocity 0, slip 0 |
| After third relocation, vertical velocity | 10.674041 m/s | 10.6740265 m/s |
| First observed positive points-list score | 5462 after 33 relocations | 56 after 27 relocations |
| At that observation, speed / slip | 41.62645 m/s / 23.57791° | 8.01327 m/s / −59.8855° |
| Final player score | 21610 | 56 |
| Full self-test | fails expected low-score rank | passes |

At native-camber relocation 39 the player's speed reached 76.19703 m/s.
The test moved XY by 50 m every 0.25 s, retaining old height, orientation, velocity,
angular momentum and wheel/contact histories. Even the previously passing
formula produced an airborne, rotating car and unintended drift points.
The corrected formula changes the subsequent unstable trajectory; the old
low-score outcome was not a sound precondition for a ranking test.

Production `Race::drift_frame` reads actual velocity, slip and yaw.
`DriftManager::angle_points` considers speed and slip (>5°), among its
other gates. The diagnostic supplies physically moving/slipping input, so
this failure does not demonstrate incorrect score ordering. Nor does this
investigation establish general native force parity or normal-driving
behavior under every condition.

### Test-only correction

The scripted moves now obtain road height and route heading and call the
existing `Drive::place`. This reconstructs the grounded pose/state while
preserving barrier groups, transmission mode, hold and stability settings.
The test asserts zero velocity, spin and angular momentum immediately after
placement, and **zero player drift points** in the results. The original
relative-ranking assertion and rival-scoring check remain intact. No
production scoring, physics, camber formula or tolerances are changed.

### Retained diagnostic evidence

Private workspace directory:
an internal run folder.

SHA256 values:

- `native-camber-trace.log`: `f4fef8057180266d5ffbcf044c193ca577394633853812b9459e63f19b33cb64`
- `old-camber-trace.log`: `bc3e6e84eb7c5552ab9f10c73dd1e45d1eb1d8cbc4e781fae3141da69a892a80`
- `trace-diagnostic.patch`: `60149b16553dc2156ed2c4bf4bba4e86e748d3bcff5dcbf465585ec6be4f6755`
- `game-trace-binary`: `3e2b51e23df1adb760fcf911bd28b8b54a93aadf1ea069f6b47ab7a47d559e16`

The retained private `run.sh` isolates XDG/TMP/cache and supplies the read-only
game-assets path. Diagnostic commands were `NFSU2_DIAG_DRIFT=1 game --selftest`
and the same with `NFSU2_DIAG_OLD_CAMBER=1`. Those variables only operate in
the retained diagnostic executable, not the final game.

### Final validation

`cargo build --release --bin game -j 2` and the complete `game --selftest`
passed with the final patch and native camber unchanged. Drift results were
Brian 11807, Kristian 9670, Eddie 8736, NAMERay 0. The AI scores are identical
to the failing native-camber diagnostic; only the test player's relocation
changed. All later self-test checks completed (`cash $6650, reputation 0,
stage 1, wins 3`). `git diff --check` passed.

- `fixed-selftest.log` SHA256: `739b4404d9c2c66982970db9f1d6bccf5c41564b0948a6516e212d9c6d497d34`
- Validated final game SHA256: `042932b98689470e664a245fb5d2fafefffb1f7ef196f671d536d655d5c5e796`

There are no remaining diagnostic selectors or trace hooks in source.

## Street X keyboard test recovery

This changes only the test driver in `wrong_way_street_x_on_keys`. Production
physics, AI, contacts, scoring and race completion are unchanged.

The test drives every Street X track in both directions for two laps using
arrow keys, a ten-metre pursuit target, and a five-metre weave. It requires
completion within 300 simulated seconds, no false wrong-way flag while moving
along the road, and no incorrectly visible Wrong Way HUD text. All these
requirements remain unchanged, as do the rivals and weaving inputs.

### Established failure

A clean `03f3c60` baseline reproduces the track 4608 variant 0 timeout. This
predates and is independent of the Enter intro placement fix; the test never
sends Enter. With both native-backed cache consumers enabled, rival contacts
around 84.5–87 s turn the player away from its road target. It then reaches a
track wall at roughly 88 s. The bot keeps pressing forward throttle and steering
toward a target more than 100 degrees behind it. It has no reverse/recovery path.
The car remains upright, and 202 one-second samples are below 1 m/s. At the end,
it has 1.746 laps, with no false wrong-way or HUD-text events.

A same-binary diagnostic changed only the two cached-input selectors:

| Tyre speed | Drivetrain contact count | 4608 variant 0 |
|---|---|---|
|cached|cached|times out against wall|
|live|cached|finishes|
|cached|live|finishes|
|live|live|finishes|

Every run chose IntroNis12 for that case. These comparisons show sensitivity
to contact trajectories; they do not establish a defect in either native-backed
consumer or prove native collision parity. The actual AI-controller Street X
regression already passes; the failure is the simpler keyboard test driver
being unable to escape a contact. No production cache change was reverted.
Temporary selector hooks and per-second diagnostics were removed from source.
The retained comparison patch/log hashes permit review of that experiment.

### Test-only change and observed recovery

After one second below 1 m/s with a recent wall contact, the test bot holds the
normal reverse/brake key for two seconds and reverses its steering direction,
then resumes its existing pursuit. It never teleports, changes momentum,
injects lap progress or extends the race budget.

The affected run requests exactly one recovery at 89.271 s. At 90.271 s the actual
transmission is in reverse, the car moves at 3.51 m/s and has no contact; at
91.272 s it is still reversing at 5.70 m/s. It returns to first gear and completes
the two laps around 108 s. Every Street X track/direction finishes with zero
false wrong-way flags and zero incorrect text appearances. These timing and
contact results are clone diagnostics, not a claim of native collision parity.

Run with original assets and private XDG/TMP directories:

```
cargo test --release --bin game wrong_way_street_x_on_keys -- --nocapture
cargo test --release --bin game
```

Set NFSU2_ROOT explicitly: these existing asset-backed tests return early when
it is absent. `docs/evidence/streetx-bot-recovery` retains diagnostic excerpts,
source/log hashes and final validation counts. No GPU or native Wine session
was used for this test-driver correction.

Final asset-backed game test suite: 18 passed, 0 failed, 9 existing ignored.
The original 300-second budget, two-lap completion, and wrong-way/text
assertions are unchanged. `git diff --check` passes.
