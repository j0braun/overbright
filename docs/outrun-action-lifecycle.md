# Goal-5 action lifetime and route import follow-up

This follows `outrun-think-scheduling.md`. It is static tracing plus a 52-case
original-code helper oracle, not a complete action implementation or new live run.
Original executable digest and extracted range hashes are retained in
internal evidence: outrun/action-followup/metadata.json; readable assembly is adjacent.

## Action lifecycle after a successful full-think route update

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

Reproduce the oracle without a game/GPU session:

```
python3 internal evidence: outrun/native-action-lifecycle-probe.py SPEED2.EXE PRIVATE_OUTPUT_DIR
python3 internal evidence: outrun/check-action-lifecycle.py
```

The 16 state cases execute original `403120` and `401990`, covering every
predicate combination and byte `+4` value. They also verify all three callback
calls/order. The 36 election cases execute original `402360`, covering absent/
present old/proposed actions, priority below/equal/above, ordinary request 22 and
explicit requests 6/9. Chooser, destructor and predicate callbacks are synthetic;
this does not validate the real goal-5 chooser or action implementations.

## Chooser equivalence and remaining Rust integration gaps

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

`src/ai/actions.rs::Actions::think` already models that same native chooser, so
reuse is appropriate after accounting for the goal context and existing fidelity
limits. Encounter AI currently does not invoke it; it also lacks native persistent
action state, exact ready/completion callbacks and output retention. Existing
`Unstuck` and action geometry are reusable, but inferred collision sweeps and
unported calls documented in the module remain limitations. These gaps—not
separate chooser addresses—are the reasons a scheduling-only wrapper is unsafe.

## Controller output is not the physics driver tick

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

## Exact successful chase-search import

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

## Next implementation gates

The controller action dispatch and cache import can now be specified separately,
but production goal-5 integration still requires the exact goal-5 chooser/action
methods, output retention, world-update/list boundary, active-nitro policy, and
search-result import validation. The smallest faithful implementation is a persistent
encounter controller dispatching both full and fallback work with retained action
output; the ordinary race controller should remain unchanged while this is built.
The new oracle closes action state precedence and priority/request semantics only.

## Follow-up original import and goal-5 chooser oracles

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
