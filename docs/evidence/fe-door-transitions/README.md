# Native door request and reversal state

## Focused correction

0x433D10 explicitly pushes variant **0** before calling 0x433AD0. It reads
that REG animation instance's +0x60 speed and +0x64 duration, computing
(1 / 30) / cached_speed * duration. The clone previously used the active
record's variant and speed. These can differ: authored RX8/RX7 REG doors
last 30 frames, SCISSOR/WIDE 28; request speed and last REG draw speed can
also differ. A reversal therefore used the wrong mirrored start time.

Channels now remembers the last REG draw speed (native default 1 from
0x436516) and uses REG duration for reversals. The speed updates when REG
is evaluated, matching 0x43429D–0x4342A3; opening another variant does not
update it. Selection, retained open variant, rest matrices and part geometry
are unchanged. This models the cache within one Channels lifetime; the
original global animation instance/retained-style sharing across multiple
cars is not newly ported or claimed here.

## Original instruction oracle

`native_transition.py` executes original 0x433E20 request handling,
0x433D50 state changes and 0x433D10 reversal duration in a private headless
PE probe. It substitutes object lookup 0x433A30 and animation lookup
0x433AD0, returning controlled existing records and an instance with the
captured authored RX8 REG duration from ../fe-authored-animation/asset-match.json.
The record states, speeds and requests are controlled test inputs, not a
live transition trace. No allocator path is used. The animation lookup logs
its requested variant; every reversal requests 0 and every non-reversal
case makes no lookup. Original instruction ranges/digests and callback
addresses are recorded in native-results.json.

224 cases cover PC64/PC24, left/right channels, seven requested styles,
Open→Closing, Shut→Opening and both reversal directions, with cached REG
speeds 1/2 distinct from the record/request speeds .75/1.5. The Rust
regression primes REG through its real pose method, issues the request and
checks state, variant, retained variant and start time (3 microseconds tolerance).
Before correction case 0 gave start 9.166669 versus native 9.500002; all
cases now pass. This is not a bit-exact x87 implementation.

```
NFSU2_EXE=/path/to/SPEED2.EXE NFSU2_PROBE_DIR=/private/probe /usr/bin/python3 $OB_PRIVATE/docs/evidence/fe-door-transitions/native_transition.py
/usr/bin/python3 $OB_PRIVATE/docs/evidence/fe-door-transitions/reconstruct.py $OB_PRIVATE/docs/evidence/fe-door-transitions/native-results.json /private/transitions.bin
```

The native probe requires llvm-mc, GNU i386pe ld, Wine and NumPy, uses a
private Wine prefix and no GPU. Original executable SHA256:
8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297.
Reconstruction regenerates transitions.bin byte-identically. Each 52-byte
record is `<4I4f3IfI`: channel, variant, initial state, open flag; REG speed,
record speed, requested speed, start; ticks, output state, output variant;
output start; retained variant. Release library: **493 passed, 9 ignored**.

## Ordinary live style previews

Owned display :175, private a private Wine copy session:
PID 1041243. No hooks or game-memory writes. The original NAME profile
had Doors locked (earlier PID1036732). A copy of the existing the finished-career save
was added to the owned game/save directory; its source remained read-only,
SHA256 1e0acea4a742f242b955bb9f7628bd6bc943b5bfc72d07d97f1d29a7ec1db453
before and after. Existing NAME was preserved. Both native sessions stopped.

Navigation: load the finished-career save → Customize → RX7 → Change Interiors/Car Specialties
→ Doors. Stock, Scissor and Suicide were previewed using Down and Open/Close
Doors (`C`). No part purchase was needed. `live/` retains screenshots and
read-only snapshots: original record bytes, channel/state/variant, cached
animation speeds/frames, FE car/render/ride pointers and fitted slot6/160
part bytes. The reader retains all FE cars (including staging/fading nodes),
not an unsupported assertion that every node is the selected car. Reads
are sequential and are not an atomic render-frame snapshot.

Observed: stock uses variant0, scissor1 and suicide2 for both doors;
Opening settles to Open; stepping a style recreates records. REG instances
retain speed1 and duration30 while scissor instances evaluate duration28.
Two quick C presses were attempted; `scissor-reversed.json` actually shows
Closing, **not** a successful reversal. Its filename records the attempted
probe only. Thus the precise reversal finding rests on native instructions
and the controlled oracle, not that live attempt. No live wide-body preview
or rest-matrix pixel comparison was established. Existing authored bank and
static wide-style selection evidence remains in ../fe-authored-animation.
