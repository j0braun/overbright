# Outrun cache capacity: original-byte findings (2026-11-03)

This follow-up corrects an earlier interpretation: **neither 0x40F6F0 nor
0x41A090 directly clears the cache or resets/rebuilds the route.** The
sixteen-slot full path overwrites slot zero. Describing it as a clear-all
route/cache reset, as earlier investigation notes did, was not supported
by the instructions. The initial investigation was evidence-only. The follow-up below now implements
ordered storage after executing the outstanding helper paths; it builds on
6053e59's reviewed expiry.

## Counter lifecycle and full-cache instructions

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

## Exact helper/callback graph and effects

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

## Native byte-execution oracle

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

## Initial implementation gate (resolved for the clone path below)

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
`python3 $OB_PRIVATE/docs/evidence/outrun/check-native-cache.py` passed its raw digest,
slot selection, predicate-call count and counter checks; all three Python
tools parsed successfully. No Rust production code changed, so no Rust
build/test was run in this evidence-only follow-up.

## Helper follow-up and bounded storage (2026-11-03)

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

`python3 $OB_PRIVATE/docs/evidence/outrun/check-cache-helpers.py` checks raw digests,
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
