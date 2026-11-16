# Persistent goal 5/6 integration review

This work follows the original scheduling, import, and chooser oracles. It does
not establish complete native parity for the existing action geometry or movers.

The clock and controller wiring below was integrated in 92c3081. The earlier
0834b53 snapshot commit supplied evidence only; implementation and native
behavior limits remain distinguished below. Parent AI verification passed
57 tests with one ignored; asset-backed parent checks are recorded separately.

## Clock and suspension review

The scheduler belongs to the loaded `Drive`, alongside the existing world clock,
not the encounter manager. `Drive::tick_clock` advances it once at the existing
world-update boundary: free roam with/without traffic, races, held start scenes,
and stationary world views. `restart_clock` and fresh loads reset it to tick
4000/update 0. Encounter activation does not reset or advance it. Physics
substeps do not advance it. Paused paths which do not simulate the world do not
advance it. Existing replay clock behavior remains open; this change does not
claim a native replay reconstruction. The live 509-boundary evidence establishes
rounding and increment order for ordinary world updates, not every menu branch.

`402530` clears the driver inputs and sets controller byte `+710`. `40D550`
clears it (at `40D590`), resets placement remaining time using signed world ID,
and retains last full-think tick `+778`. `402070` goal reassignment does not
clear suspension. Hence a role change alone must not manufacture recovery.
For goal 5/6, goal virtual `+1C` returns true (`40CA10`/`40CA50`). In race 7,
full-route failure at `42AF53` therefore goes to cleanup without suspension or
speed callback, retaining the previous driver output. Ready fallback route
failure at `42AE74` instead suspends. A successful route with own distance beyond
the last FuturePoint was thought to suspend (`42B002..42B02D`); corrected
by the menus follow-up (docs/outrun-ai-strength.md §3): 0x428720 itself returns false then
(0x428C88..0x428CAD), so a full think goes to cleanup in race 7 and only the
ready fallback suspends. A walk that merely reaches
a dead end ahead is not any of these failures.

## Active-outrun context evidence

`evidence/outrun/nitro-context` preserves two atomic stopped-process snapshots
from owned private session `a private Wine copy`, game PID 936476,
display 157. `capture-nitro-context.py` records the diagnostic challenge, player
placement and momentum writes; these are controlled setups, not natural driving.
The session was stopped after capture. `check-nitro-context.py` verifies all
retained file hashes and the raw racer fields used below.

Both samples have race type 7, global lap count zero, racer lap byte zero and
car `+8F8` zero. Thus the native signed last-lap comparison against laps minus
one is false, and the normal cooldown row `+568` applies. The challenged racer
changes from goal 6 to 5. World IDs are 0..3, while racer IDs are 0/100/101/102.
The latter confirm the input to `5EF000` start hold; that helper bypasses drag
and track 1099, not outruns. Its clock is signed tick delta from race status
`+10`, not accumulated frame seconds.

Capture decoding caveats: slot 3's controller was not an AIRacer layout, so its
JSON `goal` and `remaining` fields are invalid and excluded from conclusions.
The JSON `race_frame` value is merely a raw float reinterpretation at status+4;
it is not established as a frame counter. The raw bytes and original JSON are
preserved unchanged. Racer IDs/lap fields come directly from car/status snapshots.

## Review scope and remaining limits

The draft wires scheduler, successful bounded search import, persistent actions,
retained output, full/fallback speed dispatch, action-specific nitrous requests,
and recovery-reset consumption. Ordinary goal9 remains on its previous path.
The shared action code's existing approximations remain (road entry extents,
collision sweeps, acceleration estimate, and road-node recovery placement).
Encounter mode-4 BeltMover transitions are still absent. Native reset switches
movers and resets driver/controller through 5FBD70/5F52D0; the bounded clone
reset restores road pose/velocity and controller state, retaining tank and
drivetrain rather than replacing the whole CarState. Exact mover suspension
reinitialization is not asserted. These limits preclude a claim of complete
native controller parity.

Validation: 578 scheduling cases, 509 live clock boundaries, 52 action-lifetime
cases, 36 import cases, 141 chooser cases, chooser implementation equivalence,
and seven original action readiness vtables. Rust AI tests include native search
import/walk/goal exit, suspension reset, signed nitrous tick thresholds, and
ordinary action tests. Asset-backed runtime tests cover full/fallback callback
counts, retained path/action, full versus ready route failures, role change,
reset recovery and world clock lifetime across free roam/races. Final command
results are recorded with the implementation review commit.

## Action input provenance

`Scene.rate = 0.05` is the path's lane-change/spatial planning rate from
`42B340` (ordinary non-Street-X constant `7E8E64`), not the full-think period.
It stays 0.05 when full thinks are selected by the world modulo/tick gate.
`5EC3B0` reads car acceleration `+80` dotted with forward `+90`; it does not
compute a difference between action calls. The existing clone estimates this
from world-update velocity differences. The new controller updates that velocity
history on skipped full thinks too and uses the previous world update duration
(the interval that produced the measured velocity change), not the current
frame duration. Placement resets that history. This remains an approximation to
the native physics acceleration, especially across collisions; no
bitwise acceleration parity is claimed.

Nitrous draft uses signed tick deltas for `+7A8/+7AC/+7B0`. `416F70` updates
flat-road state only when the strategy callback reaches those checks; early
cooldown/maximum-run exits preserve it. Race7 bypasses the reserve-under-one-second
refusal, uses a two-second maximum and speed ratio 1.0. `408430` permits stopping
strictly after the skill row's minimum. `40E390` clears action `+1FF` when called,
queries strategy only with no running shot, and otherwise leaves it false.
JustDrive always calls it; Pass calls it with an ahead target and absolute
closing <=5; Avoid calls it only in its far branch. An update without that call
retains `+1FF`, while replacement constructs a clear flag. `420810` consumes
that retained request after its speed callback. The existing tank's `level` implements `59FA40` stage classification and
stops empty shots; no new tank model is introduced.

The reused `Actions` implementation still represents action ages with its f32
`Scene.clock` instead of distinct native tick fields. This integration makes the
controller scheduling and nitrous tick comparisons explicit; it does not claim
bitwise equivalence for those pre-existing action age/geometry calculations.
The reset's preserved tank regression establishes that the clone does not refill
or discard the tank; it does not substitute for a full native mover-reset oracle.
