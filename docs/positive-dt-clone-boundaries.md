# Positive-dt boundary mapping and isolated bump replay

Source inspected: `ae2dbbb` plus evidence commit . This follow-up changes no production physics. Native capture details and controls are in `positive-dt-native-capture.md`.

## Boundary mapping

| Native boundary | Clone location in src/drive.rs | Finding |
| --- | --- | --- |
| 5B04E0 normal component traversal: body, wheel/suspension pairs, engine, clutch | substep, lines 1977–1998, 2129 onward, 2449–2477 | Clone measures suspension before its Euler integration; engine/clutch post runs after integration. Native normal-post has freshly changed compression before its rest/bump test. The implementations partition these operations differently. |
| 5ABB10 rest test after normal-post | lines 2480–2503 | Clone also computes rest after normal post, and applies wheel/momentum rest changes. |
| 5ABB41–74 full-compression wheel loop | lines 2000–2036 | Clone defers this block to the next substep's start. Native performs it before the captured next-force boundary in the same group. This alone does not establish a one-step force error: clone's next force accumulation also follows its deferred bump block. Frame-end and input-transition equivalence remains unverified. |
| 59E1B0 impulse calculation | lines 2024–2030 | Concrete mismatch: native reads unchanged cached linear/angular velocity for every wheel, changing momentum only. Clone changes linear velocity after each wheel, so later impulses depend on earlier impulses. |
| 5A0D90 zero-dt component traversal | lines 2032–2036 and 2514 onward | Clone refreshes spin and engine/clutch state explicitly. Native traverses all 11 component objects. The capture verifies traversal and velocity refresh, but does not establish every component's mathematical effect. No wholesale relocation is justified by these data. |

## Replaying actual native bump inputs

Run `python3 $OB_PRIVATE/docs/evidence/positive-dt-native/replay_bumps.py`. Its saved output is `bump-replay.json`. It verifies the raw capture hash, reads each actual bump-entry hub, cached body matrix, normal, mass, velocity and spin, and compares the reconstructed impulse with the corresponding native x87 return. The matrix transform and cross product are supported by retained `bump-formula-disassembly.txt` (441000, 43D1E0, 59E1B0).

All 39 pairs match within an absolute 1e-5 check; observed maximum error is **5.901805821828532e-8**. The small residual includes Python-double versus native x87/f32 intermediate rounding; this is not bit-exact emulation. Every pair also verifies cached linear and angular velocity are byte-equivalent as decoded floats across the native impulse call.

The second calculation holds the captured geometry and starting velocity fixed, but applies the clone's sequential linear-velocity update between wheels. At step 134:

| Wheel | Native return | Native formula | Clone sequential formula |
| --- | --- | --- | --- |
| 0 | 1.7469496727 | 1.7469496896 | 1.7469496896 |
| 1 | 1.7537295818 | 1.7537296409 | 1.3606659649 |

Thus the later-wheel difference survives identical native input state; it is not attributable to track geometry or an invented clone starting state. This isolates the algorithmic mismatch but **does not execute CarState::substep**. A full replay needs matching native-to-clone pose, component histories, world contacts and handling conversion. Those have not been established here, so no full trajectory-parity claim follows.

The actual native throttle changes occur before and after the impact. Actual gear and target gear remain 2 throughout compression entry, and clutch phase remains Engaging through the bump refresh. There is no captured compression/gear-transition coincidence to replay.

## Bounded next change

A justified candidate is to retain the pre-loop linear velocity for all bump closing-speed calculations, accumulate the resulting impulses, and refresh velocity once after the loop. Keep the existing scheduling until a separate boundary replay proves which operations can move. Add an actual Rust helper regression seeded from these native inputs before changing production behavior. This report makes no such change and does not claim full rest/component parity.
