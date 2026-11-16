# Retained tyre forward speed during landing

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

## Native observation and force replay

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

## Regression scope

`native_landing_sliding_uses_cached_forward_speed` restores six actual rear
wheel inputs from steps 129/131/133 and compares the shared production sliding
function with observed native final forces. Their zero drive torque, spin,
low longitudinal velocity and inactive circle limiting isolate this consumer.
Old live speed must differ by more than one force unit for every fixture.
The 1e-4 bound is unchanged from the larger force diagnostic. A separate test
checks the production cache/read methods across velocity changes and refresh.
These six observations do not cover the high-speed launch-grip ramp; that
consumer's shared scalar selection is supported by its native reader trace.

Reproduce fixture extraction and focused tests:

```
python3 $OB_PRIVATE/docs/evidence/cached-forward-speed/export.py
cargo test --release --lib native_cached_speed -- --nocapture
```

`manifest.json` records source hash and exact event indices. Expected force
bytes are copied from the next matched native normal-pre boundary, never
calculated by the clone. The fixture is independent of the cached-wheel-count
commit; the production change does not alter drivetrain or handling parsing.

## Validation

All 497 release library tests passed (9 existing ignored), including both
new regressions. High-refresh driving passed at 60–500 Hz (existing reverse
case ignored); all 3 tuning tests passed. Maximum native sliding force error
is 7.6293945e-6. The initial full-library attempt lacked NFSU2_ROOT and failed
one asset-dependent map-pack test; the complete run with the correct isolated
environment passed. Fixture regeneration and git diff whitespace checks pass.
Log hashes and exact terminal counts are retained in an internal validation record.
