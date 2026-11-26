# Outrun follow-ups

Ten short investigations of the outrun racers' controller, each traced in the original's code: speech, the
Think scheduling, goal 5 and 6 actions, the route cache, the belt mover, and the acceleration and recovery
fields. How the original runs outruns end to end is in [original/outrun.md](../../original/outrun.md); the
index of outrun notes is [README.md](README.md).

Each section below was a page of its own; addresses without `0x` are image addresses in `SPEED2.EXE`.

## Outrun speech entry selection

The missing win speech was an index-entry selection bug. `UG2_Speech.evt`
has one play step for event 0x12, not separate play steps for each voice.
Its descriptor at file +0x10C is 0x01040028: entry id 0x28, tuple component
4, mode 1. Five index entries share id 0x28. Their header +2 selectors
1–5 identify different audio bases. Previously the parser discarded both
the descriptor mode/component and the index selector; lookup always used
the first entry. Only voice mask 2 matched that entry's records.

### Native trace

Addresses refer to verified SPEED2.EXE SHA-256
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.

* 0x738340 executes the event; 0x7377B0 builds play candidates.
* 0x737030 resolves the packed descriptor. Mode 1 at 0x73709A reads the
  selected tuple component as a **word integer**, then calls 0x735C30.
  Native tuples include the event id before the components; Rust tuples
  omit that leading field. The descriptor's component is 1-based.
* 0x735C30 searches the sorted entry table at 0x8B5174/count 0x8B5180,
  comparing both id (+0) and selector (+2). Mode 0 instead uses 0x735BD0.
* 0x737230 / 0x736F30 filter candidate records against ordinary component
  bit masks. Entry selection and record bit-mask filtering are distinct.
* Candidate +4 retains the resolved entry index. 0x737B58 / 0x738860 use
  that identity for recent-line history. 0x737DD8 retains the selected
  entry for streaming; 0x738A20 computes the record offset within it.

| Voice mask | Component 4 selector | Index audio base |
|---|---:|---:|
| 1 | 3 | 0x23EC100 |
| 2 | 1 | 0x16CE100 |
| 4 | 2 | 0x1E45100 |
| 8 | 4 | 0x2A19400 |
| 0x200 | 5 | 0x3051F00 |

The existing native tuple mapping in `lines::outrun` remains intact.
The resolved id/selector now survives selection and decoding, and recent
line history uses distinct identities for the five entries. The clone's
bank-local ordinal is sufficient to distinguish these identities; it is
not numerically the native global sorted ordinal.

### Evidence and validation

internal evidence includes a
reproducer running unmodified native resolver/search machine code in a
minimal isolated PE under Wine, using the real index headers. All five
selectors resolve to their corresponding bases; absent selectors and an
absent id reject; a mode-0 lookup succeeds. The harness tests entry
resolution with synthetic tuples, not complete native speech playback.

The real-asset regression failed before the source change on native tuple
`[8, 1, 1, 3]` (no matching entry). It now exercises 25 voice/situation
combinations through record selection and PCM decoding. It also passes
all five win voices and Rachel's first-outrun win through `Streams::say`
and an offline mixer, requiring finite nonzero output. This establishes
audible PCM availability, not a human comparison of spoken content or
full-game output-device playback.

The broader EVT interpreter remains partial: mode 2 and preceding event
switch/control steps are not implemented. Neither shipped speech bank's
play descriptors uses mode 2; unsupported modes explicitly reject rather
than being interpreted as mode 0. This change does not claim full EVT
interpreter parity.

## AIRacer Think scheduling and fallback evidence

This investigation executes original scheduling blocks and captures a private native
career session. It does **not** change production scheduling. The bounded search
and cache are available, but scheduling alone would omit action-dependent behavior.

### Verified control flow

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

### Retained evidence and reproduction

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

### Smallest complete integration boundary

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
| Action manager | `Actions::think` (`42D0F0`, equivalent goal-5 chooser `42D6F0`), `Unstuck::update` | Native proposal logic/default constructors match (see "Goal-5 action lifetime and route import follow-up" (below)); encounter path does not use the complete action manager; map native missing/present lifetime, construction, completion, replacement and ready callback before using it as a scheduler gate. |
| Driver/nitro | `AiDriver::update`, existing race nitro policy | Separate retained driver physics from controller `420810` action/path updates; trace active fallback nitro callbacks and full-think action selection/tail before claiming equivalence. |

Next bounded task: trace full `42AE56..42B2C1` and `420810` against the existing
action and driver implementations, then capture action lifetime/ready/result and
controller tail events on the same thread. This closes the persistent-action
contract needed to dispatch search and speed correctly. The present trace proves
why merely throttling search or wrapping all controller work in a timer is wrong;
it does not justify those shortcuts or claim full native controller parity.

## Goal-5 action lifetime and route import follow-up

This follows "AIRacer Think scheduling and fallback evidence" (above). It is static tracing plus a 52-case
original-code helper oracle, not a complete action implementation or new live run.
Original executable digest and extracted range hashes are retained in
internal evidence; readable assembly is adjacent.

### Action lifecycle after a successful full-think route update

`42AE56` looks up the owner's race record with `5DED70`, conditionally applies it
with `5FBE10`, and updates controller `+760` through `40FBE0`. Suspension `+710`
jumps to the braking/cleanup exit `42B02F`. Missing goal is initialized through
controller virtual `+88`; missing action is constructed through goal virtual
`+3C` (`4269E0` for goal 5). An additional owner `+30` initialization runs when
its byte `+10` is false, before the search callback. These paths precede the
search/prune/route ordering already captured.

Route failure at `42AF53` dispatches goal `+1C`, outrun state 7, parking, or
suspension/braking. It is not equivalent to ignoring an empty route. A successful
route still checks own planner position against the final route point, then
`40ABB0` / controller `+77C` stop conditions. Active nitro has a separate full
think condition through controller `+A0`, whereas fallback uses `40D850`.

With debug/global `81CB88 == 0`, the ordinary action lifecycle is:

1. `42B1CF -> 403120` refreshes action `+10` to zero, then always calls virtual
   `+20`, `+1C`, `+18` in that order. True returns set state 3, 2, 1 respectively;
   later true results overwrite earlier ones. There is no short-circuit.
2. `401990` says done for null action, state 1, state 2, or state 3 with byte
   `+4 != 0`. In this particular manager block an action was ensured earlier.
3. If done, invoke old action `+0C`, destroy it, clear current action, and ask
   controller `+AC` for type `0x16` (new candidate output). Assign that result.
4. Otherwise call `402360`. Request field `controller+6F4 == 0x16` asks chooser
   for a proposal, accepting it only when current action exists and candidate
   signed priority `+200` is strictly greater than current priority. Rejected
   candidates are destroyed. An explicit request type bypasses that comparison,
   and resets `+6F4` to `0x16` after calling the chooser. If a proposal is
   accepted, invoke old action `+0C`, destroy it and assign the proposal.
5. `42B24C` invokes controller `+B0` with current action, then `420810`.

The `81CB88 != 0` branch instead forces/updates type 6 and has separate allocation
and replacement behavior; it must not be silently folded into the ordinary path.

Reproduce the oracle without a game/GPU session (the scripts are kept with the maintainer's private evidence):

```
python3 native-action-lifecycle-probe.py SPEED2.EXE PRIVATE_OUTPUT_DIR
python3 check-action-lifecycle.py
```

The 16 state cases execute original `403120` and `401990`, covering every
predicate combination and byte `+4` value. They also verify all three callback
calls/order. The 36 election cases execute original `402360`, covering absent/
present old/proposed actions, priority below/equal/above, ordinary request 22 and
explicit requests 6/9. Chooser, destructor and predicate callbacks are synthetic;
this does not validate the real goal-5 chooser or action implementations.

### Chooser equivalence and remaining Rust integration gaps

The native goal-5 chooser `42D6F0` and ordinary/goal-6 chooser `42D0F0`
have identical instruction streams after normalizing internal branch addresses,
except the default call: ordinary directly calls `4269E0`; goal 5 calls goal
virtual `+3C`, which points to `4269E0`. Ordinary/leader goal `+3C` points to
`428250`, a direct jump to `4269E0`. Both therefore use the same default action
implementation. Both pass `2.0` to the avoidance constructor. No new chase-specific
action classes or constructors are justified by these addresses.

This supersedes the earlier inference that differing entrypoint addresses implied
different constructor behavior. Reproduce with `compare-choosers.py SPEED2.EXE`;
`search-import-followup/chooser-comparison.json` records the complete instruction
comparison and thunk/vtable checks. The proposal order is GetUnstuck (`40E7C0`),
IJustGotHit (`403320`), avoidance (`42BA20`), recovery (`403380`), Block
(`414130`), Pass (`42B5B0`), then the default.

`src/ai/actions/::Actions::think` already models that same native chooser, so
reuse is appropriate after accounting for the goal context and existing fidelity
limits. Encounter AI currently does not invoke it; it also lacks native persistent
action state, exact ready/completion callbacks and output retention. Existing
`Unstuck` and action geometry are reusable, but inferred collision sweeps and
unported calls documented in the module remain limitations. These gaps—not
separate chooser addresses—are the reasons a scheduling-only wrapper is unsafe.

### Controller output is not the physics driver tick

`420810` transfers current action's path at `+20` to driver virtual `+3C`, runs
controller `+80` and `40D980`, then goal `+34` speed processing. It conditionally
starts/continues nitro from action byte `+1FF` (`59FA40`, controller `+A4`,
`59FB00`), handles the fresh action timestamp `+1E0 == 865188` and driver mode,
then calls `41D160(0,0)` on the action path. Finally it updates controller
`+714/+718` when the owner's performance descriptor byte changes.

Thus `AiDriver::update` should not simply stop when full think is skipped: native
physics/driver updates consume retained controller output. Conversely, rebuilding
an action path every skipped frame defeats the native action-ready gate. The
fallback path can call goal speed once before readiness and again inside
`420810` on a successful ready route; the previous live goal-5 trace observed
only false-ready actions, so its one speed call per entry is not a universal rule.

### Exact successful chase-search import

`420910` runs only for race state 7 and owner class 2, passing goal target to
`41E540`. The latter chooses velocity or forward direction using owner `+430`
against constant `784250`, obtains target road coordinates from `5EC0C0` when
available (including packed elevation), otherwise target world position, and
calls `41B820` with budget 400/mode 1. Failure retries with negated direction;
if both fail it returns without modifying this route/cache state.

On success `41E669..41E6FC`:

- Copy search origin pointer to controller `+0C`; canonicalize result direction
  byte to controller `+10`.
- Clear bits 0–1 of every one of the 16 cache words. Keep other bits.
- Clear the existing FuturePoint list with `411BE0` on controller `+6E8`.
- Iterate the result's signed-byte decision count. Each result u16 supplies
  junction low 13 bits and accepted bit 13. Find the first invalid slot, retain
  old word mask `0xFFFE0007`, then OR junction shifted 4, accepted shifted 3,
  and flags `7`. There is no deduplication and no ordinary overflow helper.

Unlike ordinary `41CFA0`, this loop uses index -1 if no slot is available.
Do not invent slot-zero eviction here. The traversal guards both decision appends with count < 15 at
`41B552` and `41B598`; `outrun_search::walk` already has the same bound.
Thus valid search results cannot fill all 16 cleared slots. A direct import oracle
should still test 0/1/15 decisions, flags, repeated IDs, and double-failure
preservation before wiring this path. Existing `JunctionCache::clear` matches the initial bit clear, but its
ordinary `insert` has a different mask/flags/overflow policy and cannot serve
as the import routine. Search bit 2 also needs the goal `+48` clearing lifecycle
(`40DB50 -> 404970`) before integration.

### Next implementation gates

The controller action dispatch and cache import can now be specified separately,
but production goal-5 integration still requires the exact goal-5 chooser/action
methods, output retention, world-update/list boundary, active-nitro policy, and
search-result import validation. The smallest faithful implementation is a persistent
encounter controller dispatching both full and fallback work with retained action
output; the ordinary race controller should remain unchanged while this is built.
The new oracle closes action state precedence and priority/request semantics only.

### Follow-up original import and goal-5 chooser oracles

`search-import-followup/` now retains two additional original-code fixtures:

- 36 executions of successful import `41E669..41E704`: 0/1/15 decisions,
  direction byte 0/1/2/255, ordinary/repeated/max junction IDs, alternating
  decisions and varied preexisting high bits. The list-clear callback `411BE0`
  is a synthetic counter, so actual FuturePoint destruction remains outside this
  fixture. Origin/direction writes and every cache word are captured. This begins
  at the success block; double-failure preservation is static evidence only.
- 141 executions of complete chooser `42D6F0..42DA66`: all 64 combinations of
  six successful/unsuccessful proposals, then current types 22/9/11/1/13/4/0
  crossed with signed priority boundaries -1/100/101/200/201/300/301/400/401/500/501.
  Geometry probes and constructors are explicitly synthetic; metadata lists
  original-address substitutions. Controller permission callbacks return true,
  recovery has one wheel down, block mode is enabled and its tuning threshold
  is fixed at 200. The fixture does not prove those geometry or permission
  implementations. The default constructor returns type 6; constructor stubs
  return type identifiers so the native selection can be checked independently.

The chooser oracle confirms ordered types 9, 11, 1, 13, 4, 0 with respective
maximum current priorities 500, 400, 300, 500, tuning-dependent (fixture 200),
and 100. The current type is excluded from its own proposal. First successful
eligible proposal wins; GetUnstuck stamps controller `+708`. The checker also
compares the actual set of queried geometry probes, not only the selected type.
The first compile rejected LLVM's `push offset` relocation; replacing it with
`mov register, offset; push register` fixed the harness before any native run.

Both probes were rerun with byte-identical output. Run
`check-search-import.py` to check both retained fixtures. `JunctionCache::import_search`
now implements the proven successful-import primitive, with a Rust regression
against all 36 native snapshots. It intentionally does not call ordinary insert,
deduplicate entries or consume the ordinary overflow counter. The traversal's
fifteen-decision invariant is asserted before mutation. It is not yet called by
encounter AI: origin/list replacement, goal lifecycle, action geometry and retained
controller output remain integration work. Ordinary goal behavior is unchanged.

## Persistent goal 5/6 integration review

This work follows the original scheduling, import, and chooser oracles. It does
not establish complete native parity for the existing action geometry or movers.

The clock and controller wiring below was integrated in 92c3081. The earlier
0834b53 snapshot commit supplied evidence only; implementation and native
behavior limits remain distinguished below. Parent AI verification passed
57 tests with one ignored; asset-backed parent checks are recorded separately.

### Clock and suspension review

The scheduler belongs to the loaded `Drive`, alongside the existing world clock,
not the encounter manager. `Drive::tick_clock` advances it once at the existing
world-update boundary: free roam with/without traffic, races, held start scenes,
and stationary world views. `restart_clock` and fresh loads reset it to tick
4000/update 0. Encounter activation does not reset or advance it. Physics
substeps do not advance it. Paused paths which do not simulate the world do not
advance it. A replay snapshot holds it with the world clock ("WorldTime",
0x57BBB0) and a replayed frame advances it as a live one does. The live 509-boundary evidence establishes
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
by the menus follow-up (docs/notes/outrun/ai-strength.md §3): 0x428720 itself returns false then
(0x428C88..0x428CAD), so a full think goes to cleanup in race 7 and only the
ready fallback suspends. A walk that merely reaches
a dead end ahead is not any of these failures.

### Active-outrun context evidence

the maintainer's private captures preserves two atomic stopped-process snapshots
from owned private session in a private Wine copy, game PID 936476,
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

### Review scope and remaining limits

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

### Action input provenance

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

## Outrun cache capacity: original-byte findings

This follow-up corrects an earlier interpretation: **neither 0x40F6F0 nor
0x41A090 directly clears the cache or resets/rebuilds the route.** The
sixteen-slot full path overwrites slot zero. Describing it as a clear-all
route/cache reset, as earlier investigation notes did, was not supported
by the instructions. The initial investigation was evidence-only. The follow-up below now implements
ordered storage after executing the outstanding helper paths; it builds on
6053e59's reviewed expiry.

### Counter lifecycle and full-cache instructions

The original executable digest is
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.
`0x827AB8` falls within the virtual, zero-filled tail of `.data`, beyond
its raw file size. Its PE initial value is zero. Reading a raw file offset
past that section would incorrectly interpret bytes from another section
as an initializer. The probe's PE reader explicitly zero-fills this tail.

An exact four-byte-address scan of the entire executable finds only file
offsets `0x1D07E` and `0x1D089`, the operands of the load at `0x41D07D` and
store at `0x41D088`. The retained disassembly likewise has only these two
explicit references. There is no explicit-address reset site. This is
static evidence of an image-initialized global with only the overflow
increment located, **not** a live observation across race/region teardown
or proof that an indirect block write can never affect it.

At a cacheable miss, `0x41D050–0x41D05F` searches slots 0–15 for the first
entry with valid bit 0 clear. If none exists:

1. Load old global count, increment/store with 32-bit wrap.
2. Signed-compare the **old** value to ten. If old < 10, run the two
   helper calls described below; otherwise skip them.
3. `0x41D0AA` sets index zero. `0x41D0AC–0x41D0D1` overwrites that slot's
   junction/acceptance bits, clears search bit 2, and sets flags 0–1.
   The mask retains unrelated high bits; no other slot is written.

There is no FIFO/LRU rotation and the global is not capped at ten. At
signed overflow it eventually becomes negative and the helper gate opens
again. Cache hits do not evaluate the predicate or increment this global;
reusing an invalid middle slot does not increment it either.

### Exact helper/callback graph and effects

The full-cache procedures and key dispatch/accessor instructions below
are retained in internal evidence, reproducible with
`route-evidence.py`. Deeper handling/geometry callees are named as
dependencies, not asserted to be fully reconstructed by this extraction.

```text
41CFA0 full miss, old 827AB8 < 10
  controller virtual +24 -> 41BE90 -> reads controller+6E8 (walk)
  41A090(walk, owner, 1)
    5B5340(owner[+10]+10): reads byte +4 of that descriptor
      if byte == FF: 5B4FD0(1,1,1) -> 5B4DD0 -> handling-data helpers
    5EC0E0(owner) -> owner[+2C] virtual +1C
      captured controller vtable 785370 -> 402170 -> reads *controller[+6E0]
    for each active FuturePoint:
      4122C0(point, stack-output, point.s, 0, nonnegative lane)
        point virtual +08: captured 40D080 or 40D3A0 (lane mapping)
        401C40 (lane offset)
        point virtual +38: captured 40D0F0 -> 5DD370,
                            or 40D380 -> 5DD4A0 (world point output)
      5D43D0(point, 0), 5D43D0(point, 1): junction lookup
  40F6F0(controller)
    for each valid cache slot, only if global 88F2BE != 0:
      5D5CE0(88F298, junction[+00], owner[+8FC]): coordinate difference
      discard floating return
```

`40F6F0` and its sole helper `5D5CE0` contain no memory stores apart from
stack argument construction; they do not mutate cache/route/counter.
`41A090` writes its stack locals and the stack geometry output. It has no
direct stores to the walk, its FuturePoint pointer array, or the cache.
It cannot honestly be labeled a route-reset routine on this evidence.

The controller and two point-vtable bindings above come from retained
`goal5-branch-capture` snapshots: controller vtable `785370`, FuturePoint
vtables `784A48` and `784AA0`. Those snapshots establish these concrete
bindings, not all possible runtime subclasses.

**Condition left outstanding by the initial investigation:** `5B5340` can enter the lazy path when
`byte [dword [owner+10]+14] == FF`. `5B4DD0` then writes descriptor data
(e.g. +08, with downstream calls including `5AC500`, `5997F0`, `59B0F0`,
`5B4790`, `5B5AC0`, `5764E0`, and `5A77D0`). This is not a cache reset, but
skipping the full-cache diagnostic call unconditionally would skip real
lazy initialization in that state. We have not established that this
state is unreachable for an active goal-6 owner at overflow, or that the
clone always performs its equivalent earlier. Nonempty walk callback
behavior also has not been executed by this new oracle.

### Native byte-execution oracle

`native-cache-probe.py` constructs a private PE32 and executes **unmodified
41CFA0, 41A090, 40F6F0 and listed dependencies** under Wine. The explicit
synthetic state uses an empty walk and an already initialized owner
+0x10 descriptor. The eligibility callback is original `41BD20`. The
predicate callback alone is synthetic: it increments an observation
counter and always returns false. Thus this is a cache-storage oracle,
not a native acceptance/RNG or live controller oracle.

The 36 returned states are retained as `native-cache.bin` and
`native-cache.json`; metadata records executable/function/result hashes.
They cover filling all sixteen slots, thirteen consecutive overflows,
a retained slot-1 hit, invalidating/reusing slot 7, global=0/1 traversal
modes, and forced old counters 9, 10, -1, INT_MAX and INT_MIN. The old-count
condition is established by original disassembly; the fixture confirms
resulting counter/slot state, not an independently measured helper call
count.

Observed examples:

- Fill junctions 0–15: slots encode `junction << 4 | 3`, counter remains 0.
- Insert junctions 16–28: slot 0 changes each time, slots 1–15 survive,
  counter becomes 13.
- Revisit junction 1: no predicate call; counter remains 13.
- Invalidate slot 7 and insert 29: slot 7 reused, counter remains 13.
- Forced old counter -1 wraps to 0; INT_MAX wraps to INT_MIN.

The first assembly attempt failed before execution due to unsupported
immediate relocations; register pushes corrected it. The first executable
attempt faulted at a missing synthetic controller +1C vtable binding;
adding the original `402170` binding corrected the fixture. Neither is a
native game discrepancy. Successful runs use a private prefix with display
drivers disabled. No original game assets/saves were modified, no game or
GPU session was launched, and no Wine process remained after completion.

### Initial implementation gate (resolved for the clone path below)

Do not implement an invented clear-all/reset policy. The native slot policy
is now established, but this patch deliberately remains **evidence-only**
until the specific lazy-descriptor/nonempty-walk condition above is
resolved. A next native capture should record `827AB8`, owner descriptor
+14, walk count/pointers, cache words and callback vtables immediately
before/after the low-counter helper path, then observe the counter over
controller replacement and race/region teardown. An initialized-descriptor
and nonempty-walk byte harness with those actual callback implementations
would independently test helper side effects without inventing a reset.

Then migrate storage to ordered sixteen slots while retaining invalid-slot
identity, predicate eligibility and search flags. Merely truncating the
current unordered HashMap cannot reproduce first-invalid-slot reuse or
slot-zero overwrite. Goal-5 search wiring and think cadence remain outside
this work; no chase or full-controller completion is claimed.

Validation: two successful original-byte probe runs completed all 36 calls
and reproduced the JSON, raw bytes and metadata byte for byte;
the check script `check-native-cache.py` (kept with the maintainer's private evidence) passed its raw digest,
slot selection, predicate-call count and counter checks; all three Python
tools parsed successfully. No Rust production code changed, so no Rust
build/test was run in this evidence-only follow-up.

### Helper follow-up and bounded storage (2026-11-03)

`cache-helper-followup/` retains two deliberately different experiments:

- **Private live game-thread diagnostic:** `capture-cache-helpers.py` installs
  `cache-helper-hooks.s` at the route-update call of a selected actual goal 6
  controller. An explicitly forced challenge starts the outrun; this is not
  natural overflow observation. The hook calls original 41A090/40F6F0 with the
  existing initialized descriptor, then sets its class byte to FF and calls
  the same helpers again. Its 27 active points used vtable 784A48. Before,
  after initialized, and after lazy snapshots are identical across controller
  0x760 bytes, performance 0x800, walk 0x200, and forty 128-byte point slots.
  Class 1 was restored by the lazy path. Snapshots and hook hashes are retained.
  Only the selected private process was modified in memory; the call-site
  patch was restored and that Wine/Xvfb session stopped afterward.
- **Original-byte PE oracle:** `native-helper-probe.py` expands the earlier
  storage harness with two synthetic active points and original forward
  40D080/40D0F0 and backward40D3A0/40D380 callbacks, plus their complete executed
  geometry dependencies. Its 39 calls preserve 4096 bytes containing walk,
  points, and route records. It also exercises unrelated high bits, search
  bit 2, middle-slot invalidation/reuse, repeated full-cache overwrites, and
  controlled false/true predicate results. The true path binds original
  accepted-junction callback 410580. The predicate alone remains synthetic;
  these results do not validate native RNG or acceptance choices.

The check script `check-cache-helpers.py` (kept with the maintainer's private evidence) checks raw digests,
all 39 cache/counter/predicate-call results, and both helper snapshots. An
initial extension attempt with true acceptance faulted at the previously
unused null +58 fixture binding; adding original 410580 fixed the harness.
That was a fixture omission, not a discrepancy in the game.

The descriptor is the already-documented performance object: owner+10
points to the ride, and ride+10 is Perf_Class(5B5340). Native lazy 5B4DD0 builds
handling/upgrades/tuning, acceleration data and class. The clone performs
that work eagerly: `car::car_type` always calls `handling::setup_performance`
after parsing upgraded/tuned records, and `Car::set_race` runs it again on
race changes before driver/route use. The helper return/geometry outputs
are discarded. There is therefore no equivalent uninitialized descriptor
to repair in this clone route path. This conclusion is scoped to current
car loading; a future lazy handling cache would need to preserve that
prerequisite. The live experiment does not proveFF is unreachable in every
native controller state, nor that no unobserved global is touched by setup.

`ai::junction_cache::JunctionCache` now stores sixteen packed words. Existing
goal-specific eligibility and goal 6 expiry decisions stay at their callers;
only their common storage changes. Lookup tests valid bit 0, insertion uses
the first invalid slot or overwrites slot 0, and applies the exact 41D0BE mask
`FFFE0003`. Expiry clears only bit 0 and exempts search bit 2. Route reset uses
42CEB0's `FFFFFFFC` mask, preserving other bits. Search imports remain
unwired. The image-lifetime overflow counter is wrapping AtomicI32, never
reset by a controller or route. Tests inject separate counters, so their
forced signed-overflow values cannot contaminate other tests.

No route reset, LRU/FIFO eviction, think-cadence change, or goal 5 integration
is introduced. Teardown/region counter lifetime remains supported by static
explicit-reference evidence, not a live observation across those transitions.

Validation of this implementation: the new eviction/RNG regression failed on the previous HashMap, then passed after replacement. All 440 non-ignored library tests passed (three ignored), both without and with verified original assets using private cache/data paths. Existing 36-call and new 39-call evidence checkers pass. No rendering or scheduling claim follows from these tests.

## Loaded-road active-outrun BeltMover mapping

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

### Implemented boundary and remaining scope

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


### Validation and reproduction

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

## Native acceleration input: writer, phase, and bounded replacement

Investigation follow-up to reviewed controller commit . The original
evidence was committed separately; the retained-state implementation is described
at the end of this document. The original executable SHA256 and byte-range hashes are in
the maintainer's private captures. The native helper uses copied original
instructions with synthetic car data, in an isolated headless Wine prefix. It
is not a new live-game capture.

### Finding

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

### Arithmetic and original-code oracle

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

### Clone state and implementation boundary

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

### Smallest remaining live capture

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


### Implemented retained state and observed precision policy

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
"Recovery request consumption after motion" (below) correction queues it until after complete outer
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
see "Native other-car acceleration projection" (below) for the original-code oracle and limits.

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

### Scene boundary follow-up

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

The subsequent reset audit in "Placement acceleration clear versus world snapshot lifetime" (below) establishes an
unconditional native acceleration clear in `59F890`. Encounter recovery now
clears its previously stale vector too. Full callback snapshot retention and
native placement timing remain explicitly open there; this does not change the
normal-world snapshot contract.

## Native other-car acceleration projection

The road-entry scalar `60BE40 -> 5EC340` uses retained world acceleration,
separately from own-car `5EC3B0`'s forward-axis projection. Binding is explicit:
`5FBA36..5FBA4E` and `6044D3..6044F3` pass car `+60`, `+70`, `+80`, `+90` to
`60BD00`. That setter assigns road-entry `+10` to velocity `+70` and `+14` to
acceleration `+80`. Retained disassembly includes both callers and the setter.

`60BE40` computes sqrt((vx²+vy²)+vz²), stores the speed as f32 and passes it with
both vector pointers to `5EC340`. Its comparison uses constant `784358`, bits
`3DCCCCCD` (0.1f32). **Strictly below** that threshold it returns the full 3D
acceleration magnitude. At or above it, it calls `43CF10` to normalize the full
velocity vector, then sums Y-product + Z-product + X-product. Negative direct
speed also takes the magnitude branch; NaN direct speed takes normalization.
These direct-helper cases are evidence of the helper, not ordinary finite car
entry states. There is no division by controller dt and no heading projection.

`43CF10` recomputes the 3D length, multiplies each component by its reciprocal,
and stores f32 components. A zero vector normalizes to +X; that fallback is
unreachable through the finite ordinary entry's projection branch because zero
speed takes the magnitude branch first. PC53/64 can retain a length with more
precision than the f32 speed argument. The PC24 implementation can reuse its
already rounded length; a generic normalize-or-zero or XY normalization would
not preserve these semantics.

### Evidence and validation

internal evidence executes complete unchanged
original `60BE40`, `5EC340`, and `43CF10` instruction blocks in an owned headless
Wine helper with synthetic vectors. It uses a private prefix and shuts down its
wineserver on success/failure. It does not attach to the live game. Executable
SHA256 and block/result hashes are in `other-acceleration-followup/metadata.json`.

`check-other-acceleration.py` checks all 351 original results, including the
separately returned normalized vector: 105 entry and 12 direct-helper cases for
each PC24/53/64 mode. Cases include zero/signed-zero velocity, the exact threshold
and adjacent f32 values, vertical motion, braking, mixed vectors and deterministic
random finite vectors. Coverage counts are asserted. Rust checks all 105 PC24
entry results bit-for-bit using the derived fixture. The game regression checks
that missing/stale controller velocity history and different controller dt no
longer change the road-entry value; own-forward output remains separate.

Runtime `CarState::velocity_acceleration` uses explicit PC24 arithmetic based on
the retained normal-world FXSAVE evidence described in
"Native acceleration input: writer, phase, and bounded replacement" (above). This is not a universal live-mode CW claim.
The Python f64 model matches these PC53/64 fixtures but is not a general x87
extended-precision emulator. Rust f32 extended-exponent underflow/overflow,
nonfinite vector exception behavior and directed rounding remain outside the
validated finite domain.

The bounded game change replaces `road_car`'s XY finite difference from legacy
controller history with the retained vector helper. History storage/call
arguments remain for now to avoid unrelated lifecycle changes. No change is
made to own-action forward projection, controller scheduling or action choice.

## Placement acceleration clear versus world snapshot lifetime

This follow-up to `9cc2666` audits the acceleration fields only. It does not
replace the clone's road-node recovery placement or claim a complete native
teleport/mover implementation.

### Native call paths and definite clear

`5FBD70` (full teleport) calls `5F52D0` at `5FBD84`, then route/controller resets.
The route-trigger helper `5FBE10` reaches it at `5FBEA8`. The human reset path
`6034BA -> 6009B0` can use either `5FBEE0` (`600DBA`) or `5FBD70` (`600E01`).
Racer recovery consumed inside the car update follows `6013AF..601519 -> 5FBEE0`.
`5FBEE0` calls `5F52D0` at `5FBFDB`. Thus both placement helpers reach the same
body reset, irrespective of which route/position strategy supplied the pose.

`5F52D0` first saves the mover, selects mover 0, and at `5F52EC..5F52F1` calls
`59F890` with `ECX = car + 0x40`. The following stores are unconditional:

| Native stores | Body offset | Car field |
|---|---|---|
| `59F896..59F89C` | `30/34/38` | velocity `70/74/78` cleared |
| `59F8A4..59F8AA` | `40/44/48` | acceleration `80/84/88` cleared |
| `59F8C0..59F8CC` | `D0/D4/D8` | spin `110/114/118` cleared |
| `59F8D5..59F8E1` | `E0/E4/E8` | angular acceleration `120/124/128` cleared |

Placement subsequently installs its new velocity through `592170` (`5F536F`).
This makes preserving the *published* pre-reset acceleration incorrect at the
body reset boundary. The saved outer-world velocity `car+520` is a separate
field; the direct stores in `59F890` do not reach it. `5FBBF0` updates that saved
velocity only for positive dt, and `604750` uses it to publish acceleration only
for positive dt. A reset before that snapshot and a reset after it therefore
have different interval semantics.

The complete `5F52D0` also calls mover, driver and controller routines. This
investigation does **not** claim that every downstream callback preserves the
saved field, nor that the original full teleport has been executed in the
oracle. Establishing the full callback graph or a live placement capture remains
necessary before widening that retention claim.

### Bounded original-code probe

internal evidence executes unchanged
instruction prefixes `59F890..59F8AD`, `5FBBF0..5FBC66`, and `604750..60483A`.
Harness epilogues stop before unrelated body setup, world callbacks, and race
events respectively. Synthetic car storage has distinct acceleration, current
velocity, and saved-velocity sentinels. The harness explicitly selects PC24 and
runs in an owned headless Wine prefix with guaranteed wineserver cleanup.

The 36 cases cover both reset-before-snapshot and reset-after-snapshot, three
velocity transitions, positive .05 / 1/60 dt, signed zero, negative dt, and NaN.
The checker verifies all 12 output words per case: acceleration and saved
velocity immediately after the reset prefix and after the final publication.
It confirms unconditional acceleration clear, positive-dt snapshot/publication,
and zero/nonpositive/unordered preservation of the now-cleared vector. The
caller ordering in these synthetic cases is controlled by the harness; it is
not evidence that the live game takes both orderings at a particular call site.
Hashes, exact-entry disassembly, and result data are retained in
the maintainer's private captures.

### Bounded clone correction and remaining phase differences

`reset_encounter` previously edited pose/velocity while leaving
`world_acceleration` stale. It now explicitly clears that vector. The regression
first failed with sentinel `(11,22,33)` surviving recovery; it then verifies zero
immediately after actual recovery, zero-dt retention, and that an independently
captured interval token still publishes its original velocity delta.

Manual `Drive::place` / `reset_to_road`, race route-trigger placement, and ordinary
`reset_rival` already create fresh `CarState` values whose acceleration is zero.
No blanket acceleration-retention rule is added to those paths. Tank, drivetrain,
ground and controller reset fidelity are outside this small correction.

The later recovery-timing follow-up moves encounter request consumption out of
controller work and into the post-motion/pre-final-contact callback. See
"Recovery request consumption after motion" (below) for the bounded integration and remaining intermediate
collision-cadence difference.

**Correction to the earlier route-trigger timing inference:** the full world
caller publishes acceleration at `60A086`, then calls `602660` at `60A257`.
Its `600720 -> 5FBE10` route-trigger placement therefore happens *after*
publication. The clone's route-trigger check after `step_race` has the correct
relative acceleration phase; merely finding it within a car/racer helper did
not establish that helper's world-update position. No route-trigger phase move
is justified by this trace. Existing scene channel placement remains approximate.

## Recovery request consumption after motion

Follow-up to `405e33f`. This corrects encounter recovery's controller/physics
boundary while retaining the clone's existing collision subdivision policy.

### Native order

| Address | Operation |
|---|---|
| `42427B -> 5EBF70` | AccidentRecovery action sets car `+96C = 1`. |
| `60A027 -> 5FBBF0` | World velocity snapshot. |
| `60A042 -> 5F0F10` | Iterate enabled collision bodies, calling virtual `+2C` with full world dt. |
| `7A2478 + 2C = 5A9F20` | Car collision body frame saves collision start through `593220`, then calls `6012C0`. |
| `6012DB` | Run current mover virtual `+04`. |
| `60130E` | Read pending recovery flag after mover returns. |
| `6013AF..601519` | Racer recovery resolves placement and calls `5FBEE0`. |
| `601524` | Clear the consumed request. |
| `60A06B -> 5FFA60` | Collision processing. |
| `60A086 -> 604750` | Publish complete world acceleration. |
| `60A257 -> 602660 -> 600720` | Human race route-trigger check, after acceleration publication. |

The collision body's saved start is separate from car `+520` world velocity.
`593220` saves the pre-motion pose and dt; `5A9F20` later updates bounds through
`5A9DF0`. The mover switch in `5F46B0` replaces car `+34` and changes existing
collision body `+5C`; it does not replace car `+3C`. The clone callback preserves
the existing sweep start instead of inventing a second snapshot at placement.
Full native placement/driver callback parity is still outside this change.

The last row corrects the earlier route-trigger timing suspicion recorded in
"Placement acceleration clear versus world snapshot lifetime" (above): that helper's name/ownership alone did not
establish its caller phase. The clone's existing post-`step_race` trigger placement
already follows acceleration publication and is left in that position.

Exact instruction ranges and hashes are retained in
the maintainer's private captures. This is static original-code evidence, not a
new live-game capture. The prior 36-case native prefix oracle remains the
acceleration/reset arithmetic evidence; no new synthetic callback oracle is
presented as execution of a complete original world update.

### Production boundary

The goal5/6 controller now transfers its action's reset request into an
`EncounterCar` latch corresponding to car `+96C`, rather than placing the car
inside `Encounters::think`. The latch survives action replacement until physics
consumes it; new spawning clears it.

`step_world_with_post_motion` invokes an optional callback after each car's
initial mover simulation and before broadphase/contact processing. Existing
`step_world` callers use a no-op callback. Contact re-simulation does not invoke
the callback again. No force, integration or contact-solver formula changes.

Encounter recovery uses that callback only in the **final subdivision** of the
outer update. Placement therefore sees completed outer motion, remains inside
the original acceleration token, and precedes the final contacts/publication.
Its driver, controls, route and controller/cache reset behavior is shared with
the existing reset implementation. It does not replace the car state, refill the
nitro tank or reinitialize drivetrain state. The callback updates the input used
by any remaining contact re-simulation to the reset controls.

**Remaining cadence limit:** the clone still processes intermediate contacts
between outer subdivisions, whereas the demonstrated native normal world runs
one full body-motion pass before its collision pass. Final-subdivision recovery
is a bounded correction within that existing architecture, not full native
motion/contact scheduling parity. Ordinary race-rival recovery also retains its
prior scheduling. Reconciling outer collision cadence must be a separate shared
physics change, coordinated with force/contact validation; this patch neither
caps FPS nor silently changes the collision rate.

### Regression scope

The actual-asset free-roam regression queues recovery on an active encounter,
checks consumption and controller recovery, and compares published acceleration
against the original outer token. Test-only thread-local observation is enabled
and cleared within that test: recovery occurs exactly once, after final
subdivision motion, and does not repeat next update. The fixture exercises
multiple subdivisions and verifies the mover changed velocity before placement.
At the callback boundary the test also checks nitro state is preserved across
the placement itself. Existing contact, controller lifecycle and world-clock
regressions remain relevant. The original pending-latch test failed before the
post-motion implementation was added.

The combined `encounter_recovery_placement_then_contact_consumes_once` regression
places the player at a real route recovery landing and starts the pending rival
20 m away. Production `step_explore` must produce a live car-pair contact record
and resolve the landing contact after one final-subdivision recovery. It checks
finite resulting transforms/velocities, latch consumption, controller resume,
the complete outer acceleration delta and no repeated recovery next update.
It deliberately does not require the final position to equal the landing: the
native retained pre-motion sweep can rewind the placed pose during contacts.
This guards the combined path; it is not a native collision-output oracle.
