# Active-outrun BeltMover applicability (2026-11-03)

Evidence only, captured against the original executable; no production changes.
The loaded-road BeltMover predicate applies to this actual race-type-7 rival.
It is not limited to ordinary circuit/sprint racers. This capture does **not**
establish unloaded-section behavior, goal-5 behavior, or complete belt trajectory
parity.

## Result and negative controls

`follow.bin.zlib` contains 828 fixed 16384-byte records, zero dropped records,
and 204 complete decision → predicate entry → predicate return → optional mode
request → decision exit groups. All predicate returns match the independent
finite-input decoder of native gates. All FXSAVE control words are 0x007f (PC24).
There are 203 groups in race 7 and one final group after native completion in
race 0. The active-race groups comprise 38 accepted, 156 distance-rejected,
and 9 clock-rejected predicates. The final free-roam group is accepted; do not
report all 39 accepted predicates as active-outrun observations.

The first group is challenge startup with the previous clock 83.12; the clock
then resets. Do not sort records by clock or describe this as an 83-second
outrun. The periodic sampler observes native completion at clock 45.04, race 0,
without a finish bypass. The final decision clock is 44.980003356933594.

At record 544, clock 30.000001907348633, nearest cached human distance
180.18370056152344 m, native 42B280 requests mover 4 from mover 3. Nine retained
pre-30-second far-distance predicates reject on the clock (including records
503, 507, 519, 523, 527, 535, 539). At record 549, clock 30.200000762939453,
distance 106.60648345947266 m, 42B2A2 requests mover 3 from mover 4. Across the
capture there are six requests each for modes 3 and 4. All requests occur with
action 6. Goal/action row counts are (6,6)=624, (6,1)=144, (6,0)=56 and final
free-roam (1,6)=4. There is no goal 5 observation.

All full-car+0x5CC pack pointers remain non-null. They change between 0x4A2AC30
and 0x4A2ABC0 at records 241 (13.25 s), 349 (19.34 s), and 646 (35.54 s).
The decision's cached loaded byte stays true. There are **zero** 4022D0 world
load-notice events. This proves loaded pack changes, not an unloaded transition.
The raw periodic metadata field named `loaded` is only the low byte read by the
original recorder script; it is not a boolean. The full record/decoder dword
`loaded_pack` is authoritative. The script and original metadata are preserved.

## Capture intervention and ownership

A fresh private copy/profile in `a private Wine copy`, display 174,
was navigated with the retained `fastnav.py ... freeroam` method. Source assets,
original source, and user saves were not changed. Session paths, PID, hooks,
original/patch bytes, all deliberate state writes and wall timestamps are in
`follow.json`. All installed hook/code/telemetry bytes were restored and verified;
the owned Wine/Xvfb session was stopped after capture.

This is an instrumented scenario, not a natural driving playthrough. The retained
encounter recipe writes global 0x860A58=0 (its broader meaning is not established
here), player proximity/momentum, selected opponent +0x250 and challenge request
race-status+0xC30. Native code then enters race 7. During the run only player
placement/exported velocity/body momenta are controlled, roughly every 0.2 wall
seconds: follow 60 m behind before clock 27, 180 m through 37, 60 m through 44,
then 240 m. Each write records original and replacement bytes. Clock, rival
mover, path, action, skill, wheels, loaded state and predicate/finish gates are
not patched. Following does not guarantee the requested separation between
samples: the native cached distance, not the requested gap, drives the audit.
The rival eventually wins naturally as separation crosses the native threshold.

## Hook and field boundaries

- 42B259 decision entry, ESI controller; cached loaded byte is native stack+1B.
- 42B26C call to 402180, entry and return; return AL is saved EAX at record+60.
- 5F46B0 Car_SetMoverMode entry, ECX car, stack return and requested mode retained.
- 4022D0 world notice entry, ECX controller; no matching events in this run.
- 42B2BC decision end before 41BF80.

`capture.py` records complete displaced bytes and hook targets. `recorder.s`
preserves flags, integer registers and FXSAVE/FXRSTOR, and rereads mover and
other pointees on every event. Raw ranges and decoded fields are in the source;
metadata `layout` lists each payload. Fixed payload ranges must not be taken as
proof of complete native object sizes. Controller/car backlinks, controller
vtable, header identity, register identities, null payloads, contiguous sequence
indices, same tick/world across each group, and predicate entry/return adjacency
are checked. Raw locator bytes are retained; decoded `locator_node` and
`locator_direction` are offset labels, not validation of native routing semantics.

Native predicate 402180 checks special track/drag/drift, signed driver path
count, controller timers, exported forward scalar, wheels down, action kind,
skill, nearest human distance, clock and finished racer status. Its skill call
402580→610520 reads descriptor+901 with the retained global override.
5F1170 reads cached human positions at world+80/+90; current player geometry is
not substituted. The observed finite values are decoded with PC24 distance
arithmetic. The 30-second multiplier is float32 .00025. NaN/general numerical
behavior is not claimed. `native-ranges.json` retains exact EXE bytes and hashes;
`native-disassembly.txt` retains their bounded disassembly. World notice behavior
is static evidence only here.

## Reproduction and verification

Offline, from the repository root:

```sh
python3 internal evidence: outrun/belt-applicability/decode.py internal evidence: outrun/belt-applicability/follow.json
python3 internal evidence: outrun/belt-applicability/test_decoder.py
```

The second command checks all captured groups and rejects ten rehashed
corruptions: truncation, sequence, target, controller backlink/vtable, precision,
predicate return, missing decision end, swapped valid mode request, and mode caller.
Each mode request must match its group's entry mode, predicate, caller and exit
mode. The decoder accepts only the observed one-human configuration; the native
multi-human cached-slot policy is not generalized. `SHA256SUMS` is relative to this
directory. Recorder rebuilding with GNU `as --32`, `ld -m elf_i386
-Ttext=0x782b00 -e decision`, and `objcopy -O binary --only-section=.text`
was byte-identical to `recorder.bin`.

For a new authorized private session, navigate fresh free roam first, then:

```sh
python3 capture.py $OB_PRIVATE/wine/runs/<run> /absolute/owned/output/follow.json --seconds 65
```

This depends on the retained private `peek.py`/fastgpu method, display ownership,
and exact executable hash. Never run it against a user/shared prefix. Hook
addresses and linked symbols must be rechecked if changing assembly. The capture
script restores hooks in `finally`; the operator must stop their owned session.

## Bounded next step

The clone currently has ordinary-racer belt machinery (`belt_wanted`, build,
step and leave paths in `src/bin/game.rs`) but `EncounterCar` has no belt state
and supplies mover_four=false / Input::Ai. A supported next slice is to map the
existing encounter driver's retained path/actions/timers onto the same loaded
race-7 predicate and explicit mode-3↔4 transition machinery, with the retained
203 active groups as a gate oracle and a real encounter integration regression.
Do not bypass the additional native gates merely because only distance/clock
reject in this run. Before implementing trajectories, audit build/leave state
and route ownership; supplied capture fields do not prove clone-produced inputs.
Goal 5 and unloaded notices remain separate capture requirements. No blanket
ordinary-racer port or full BeltMover parity is justified by this evidence.
