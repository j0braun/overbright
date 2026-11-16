# Rest and bump-stop component refresh

This corrects the reason the native engine/clutch zero-time post was
previously omitted. It does not claim a complete port of every component
update or full collision parity.

## Native path and correction

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

## Why the old collision test failed

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

## Runtime evidence and tests

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

## Remaining limits

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
