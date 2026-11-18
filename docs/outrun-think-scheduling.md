# AIRacer Think scheduling and fallback evidence

This investigation executes original scheduling blocks and captures a private native
career session. It does **not** change production scheduling. The bounded search
and cache are available, but scheduling alone would omit action-dependent behavior.

## Verified control flow

`42AC53..42AE55` decrements controller `+6F0` by the supplied dt on every call.
Owner class means `[owner+14]+4`, not the car's performance class.

| Owner class | Full-think gate |
| --- | --- |
| 2 | Signed world-update counter `7FB724` remainder modulo 8 selects this owner in `[world+14, world+18)` through the car array at `world+1C`; modulo 4 in drag when mover `owner+28 != 4`. Signed wrapping tick delta from `+778`, scaled by f32 `0.00025`, must be at least f32 `0.2`. |
| 1 | Owner kind `+24 == 2` bypasses the gate; otherwise signed counter remainder modulo 8 must equal signed owner ID `+0A`, with the same tick threshold. |
| Other | Decremented `+6F0 <= 0`; unordered/NaN takes fallback. |

The x87 comparison does not round the scaled tick delta to f32 first. The checker
uses exact rational arithmetic for that comparison. Native signed remainder is
preserved, including the absence of a lower-bound check in the class-2 slot test.

Fallback `42AD16` with no action (`+6F8 == 0`) enters full think regardless of
these gates. Full think `42AE2F` stamps `+778` and adds f32 `0.1` to the already
decremented `+6F0`; it does not reset the timer to 0.1.

With an action, fallback first handles class-2 nitro (`59FA40`, `40D850` and
controller callbacks). Suspended `+710` skips the following work. Otherwise it
calls goal virtual `+34` at `42AD7E`: for goal 5, `4204D0 -> 41C5E0`, the speed
policy. Mover 4 then exits. Other movers call action virtual `+10` at `42AD96`.
False exits; true calls route update `428720` at `42ADA4`. Failure goes to
`42AE74` (recovery `402530(1)`); success calls controller `+B0`, then `420810`.
The oracle substitutes observable callbacks and exit markers for these downstream
operations; it establishes branch dispatch, not their behavior.

Full think invokes goal `+44` at `42AF40` (`420910`, goal-5 search), then
`41D0E0` pruning and `428720`. Default goal-5 action construction is `4269E0`,
vtable `785684`, type 6. Its `+10` is `404800` (false). Observed action tables
`7857B4` (type 11) and `785858` (type 1) also have `404800` at `+10`.

`7FB724` is a world-update counter: `57A17B..57A19D` increments it alongside
simulation ticks `865188`; `576D00..576D39` resets counter 0 / ticks 4000;
`5FF7D2` writes 100000 during teardown. This is not evidence that a rendered
frame is the correct clone clock boundary.

## Retained evidence and reproduction

the maintainer's private captures contains raw binary outputs, inputs and
metadata hashes. Run (the scripts are kept with the maintainer's private evidence):

```
python3 check-think.py
```

The isolated PE oracle (`native-think-gate-probe.py EXE PRIVATE_OUTPUT_DIR`)
executes 578 cases using original bytes `42AC53..42AE55` and inactive-nitro
`59FA40..59FAA5`, with synthetic goals/actions/route callbacks. It covers class
1/2/other, drag/mover differences, missing/present action, ready/suspended branches,
799/800/801 ticks, signed wrap/remainders, nonzero racer-list offsets and NaNs.
Metadata names all substituted endpoints. It skips the Think prologue and timer
helpers before `42AC53`, active-nitro work, full-think action selection and tail.

The live trace has 1,346 records / 510 entries: 67 goal-5 entries, 67 goal-5 speed
callbacks, 4 search callbacks and 4 route callbacks. Two goal-5 entries naturally
had no action, at world counters 9455 and 9556, elapsed 774 and 135 ticks; each
passed fallback into full think. One deliberately cleared action at counter 9698
was **goal 6**, not goal 5. Class 2 was observed live; class 1/drag are covered
only by the isolated oracle. Racer-list begin/end were 1/4, while this owner's
ID was 100 and its relative racer slot was 0.

Capture used a private Wine copy, display 157, game PID 888452,
supervisor 888205, a private copied prefix/game and fresh career. The script
forced a challenge and player lead placement/momentum, then deliberately cleared
one action pointer. These are diagnostic interventions, not natural driving.
Patches were restored and the private session stopped. No shared session was
attached. `capture-think.py` and `think-hooks.s` retain the exact procedure.
Only default action `785684`'s ready callback was hooked; other tables' false
callbacks were established statically, not directly counted.

Each 256-byte record is 64 little-endian u32 words. Fields 0..23 are event,
world counter, ticks, controller, owner, owner class, owner kind, mover, last-think
tick, remaining f32 bits, action, action vtable, action type, action state, goal,
goal type, suspended, input-dt bits, racer begin, racer end, total car count,
signed owner ID, nitro timer, world pointer. Words 24..39 are first 16 world car
pointers; 40 is route point count, 41 frame-dt bits, 42 sub-dt bits. Events:
1 entry, 2 full-think entry, 3 fallback entry, 4 goal search, 5 goal speed,
6 route update, 7 default action ready. Full/fallback hooks precede timer writes.

## Smallest complete integration boundary

The production insertion site is `src/bin/game.rs::Encounters::think`, not a
standalone timer around `outrun_search::search`. It currently updates route, speed,
line path, unstuck and driver each encounter frame; it has no equivalent persistent
native current-action pointer/state. Ordinary races use separate `racer_think`.
Preserve that ordinary path while bringing the encounter controller to parity.

| Dependency | Existing component | Missing integration/proof |
| --- | --- | --- |
| World clock/list | Encounter clock and car vector | Establish native world-update boundary and active racer-list membership/order; don't use rendered frame or owner ID. |
| Scheduler | This oracle | Persistent `+778/+6F0`, class/kind/mover and action presence; dispatch both full and fallback. |
| Goal-5 search | `src/ai/outrun_search.rs::search` (`41B820 -> 41B230`) | `420910` invocation/state import, including successful/failed result and route/cache lifecycle in exact order. |
| Cache/route | `junction_cache.rs`, `RacerRoute::update` and pruning (`41D0E0`, `428720`) | Stop unconditional encounter route rebuilding on fallback whose action-ready result is false; route failure/recovery must remain observable. |
| Speed | `SpeedTarget::update_outrun` (`41C5E0`) | Preserve callback on eligible fallback and full-think paths; suspension/mover ordering matters. |
| Action manager | `Actions::think` (`42D0F0`, equivalent goal-5 chooser `42D6F0`), `Unstuck::update` | Native proposal logic/default constructors match (see `outrun-action-lifecycle.md`); encounter path does not use the complete action manager; map native missing/present lifetime, construction, completion, replacement and ready callback before using it as a scheduler gate. |
| Driver/nitro | `AiDriver::update`, existing race nitro policy | Separate retained driver physics from controller `420810` action/path updates; trace active fallback nitro callbacks and full-think action selection/tail before claiming equivalence. |

Next bounded task: trace full `42AE56..42B2C1` and `420810` against the existing
action and driver implementations, then capture action lifetime/ready/result and
controller tail events on the same thread. This closes the persistent-action
contract needed to dispatch search and speed correctly. The present trace proves
why merely throttling search or wrapping all controller work in a timer is wrong;
it does not justify those shortcuts or claim full native controller parity.
