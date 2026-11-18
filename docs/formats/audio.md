# Audio, speech and music

Part of [docs/formats.md](../formats.md), the traced behaviour of `SPEED2.EXE`; the tags
(**verified**, **inferred**, **open**) are explained there.

## Audio (verified except where noted)

Code: `src/audio/` (abk, gin, fe, collision, engine, mixer). All from SPEED2.EXE.

**ABKC banks** (verified; loader 0x7212E5, instance setup 0x721096).
+0x0A u16 module count, +0x14 file size, +0x18 BNKl offset, +0x1C first
module record, +0x30 call relocations (count, offsets; each patched to
node table 0x817FE0[id]), +0x34 pointer relocations (dword += bank base),
+0x38 events (count, then {record + 4, name block, type}; the name block
is {u32, name at +4}: "FX_TURBO_01", "PlayCommonSample"; the clone once read it
from +8). A module record is 0x3C +
(byte +0x24 + byte +0x27)·4 bytes: +0x28 code start, +0x2C code end (the
instance template starts there), +0x30 instance size, then byte +0x24
template offsets that get the bank base: the sound players' (node 27,
0x720D57) states. A player state's +4 points to a sound list: count, then
12-byte entries {u8 kind, …, u32 bank sample at +4}. Event names in the
install: FE_COMMON_MB "CommonSample" (19 sounds), FE_MB "FrontEndSample"
and "FrontEndSample_loop" (71 each).

**AEMS event graphs** (verified; `src/audio/aems/`, probe
the maintainer's probe `aems_probe`). A module's code (record +0x28) is x86 emitted by
the bank compiler: `push esi; mov esi,[esp+8]`, then only `push esi; call
id` (ESI = the node's block; the file holds the node id, the loader patches
it to 0x817FE0[id], 0x72132C), `mov [esi+d],eax`, `mov eax|ecx,[esi+d]`,
`add|sub|imul eax,[esi+d]`, `cmp eax,ecx; jl|jg +2; mov eax,ecx` (min /
max), `mov byte [esi+d],imm8`, `add esi,imm`, `add esp,imm`, ending `pop
esi; ret` (all 224 modules of the 176 installed banks; nothing else). The
clone decodes these forms into ops and evaluates them; the node functions
are ported. Record: +0x1C u16 live instances, +0x1E u16 the most
(0x7210A4), +0x20 u16 volume nodes, +0x22 u16 array nodes, +0x24 players,
+0x25 release node, +0x26 parameter node, +0x27 sub-event slots, +0x2C the
template, +0x30 instance size, +0x34 the {record, instance, owner} slot.
Instance (0x721096, on the game's 0x7358C0 → 0x735490): the template
copied; at +8 a tick-list node runs the code with ESI = instance +0x18
(0x725520); from +0x18 the listener nodes: the release node (0x14 bytes;
callback 0x72103A sets +0x10 = 1 when the game releases its handle,
0x735550 → 0x735030), +0x20 · 0x1C volume nodes (0x72102C), the parameter
node (0x721046: byte +0x10 dwords of the game's block copied to +0x14 on,
on creation (0x734DB0) and on every 0x735580 dispatch), the array nodes
(0x72106C). Every installed module has exactly the release and parameter
nodes, so node 0 (+0x18) reads the release flag and node 1 (+0x2C) the
first parameter, `mov eax,[esi+0x18+4k]` the others. The game's block is
the event object +4 on. Tick: 0x721DE6 runs the list 0x8B405C when the
integer of 0x8183F0 (+= 0x8B471C = 10 / 0x8B4054 per mixer pass) changes;
0x8B4054 = the configured period 0x8B44D0 clamped to 10..100 ms, −1 (the
only value 0x724413 stores) giving 20 ms: one tick per 20 ms. Nodes
(0x817FE0): 0 the release flag read once; 1 / 2 parameters 0 / 1; 3 a
flag read once; 4 the end (+0xC set: the players' voices stopped
(0x817FA0) and the instance freed); 6 the input in [+0, +4] else a
wrapping counter stepping by byte +0xC; 7 on +0xC: +8 = +0 + rand % +4
(0x724903, a sum-with-carry generator: **not ported**, xorshift); 8 a
shuffle without repeats; 10 1 once on entering [+0, +4], re-armed in
[+8, +0xC]; 11 a one-shot ms timer (float clock +0 += the period); 12 the
value of the first set flag; 13 any set; 14 a gated envelope (gate 1 from
0 restarts at +0x14, 3 jumps to the sustain segment u16 +0x12, segments
{f32 ms, f32 level} from +0x18, 0 after the last, 2 holds); 15 a curve (a
file table: byte +0 entry size 1 / 2 / 4, u16 +2 count, +4 / +8 the input
range, f32 +0xC the scale, entries from +0x10; linear between entries
unless the scale is 1; recomputed on a change); 16 a delay line in ticks;
17 select (1-based); 18 route (1-based, clearing the last); 19 / 20 min
/ max of n; 21 the product of n × f32; 22 the sum of n; 23 −, 24 ×, 25
÷, 26 % (0 for a zero divisor), 36 +; 27 the sound player; 28 an
oscillator (sine 0x8181D8 quarter table, square, saw, triangle; period ms,
amplitude); 29 a glide to +0x18 over +0x10 ms (× +0x14 / 4096, no
overshoot); 30 min(sum, +4); 31 max(+4 − +8, +0); 32 min(+8 × +4, +0); 33
/ 34 min / max of two; 35 +8 × +4 × f32 +0; 37 byte +0x19 read once; 38 /
39 (0x7200A2, 0x7201C2) a sub-event output and a named value (SWTN banks
only; ported, see "Sweeteners and the decel crackle"); 5 and 9
are unused. x87 `fistp` rounds to nearest (even).
Sound player (node 27, 0x720D57): +0x18 the state (0 stop, 1 play, 2
release), +0x14 the list index (clamped), +4 the list (count, then 12-byte
{kind, …, +4 BNKl slot}), +8 the voice, +0xC / +0xD the state and the one
before, +0x10 the playing kind (0xFF none), byte +0xE parameters of 12
bytes from +0x1C {kind, +4 last sent, +8 value}, byte +0xF position
outputs after them (elapsed / remaining ms; no car bank sets it). 0 → 1
starts entry [index] (0x720B5B: the block 0x721A6B {+0 volume 0x7F, +4
second volume 0x7F, +5 send 0, +8 azimuth 0, +0xA 0, +0xC pitch 0x1000,
+0xE 0x1000, …} set by 0x817F60[kind] from the parameters, then 0x724333),
unless the state was 2 after 1; 2 → 1 resends pitch and volume (0x720BE7);
→ 2 releases (0x72412A(v, 0): the voice gain 0; 0x723D9B(v, 0)); → 0 stops.
While playing (0x720C7C) changed parameters go to 0x817F30[kind]; a voice
gone resets the player and it returns 0, else 1. Parameter kinds: 0 the
pitch (0..0xFFFF, step = rate · 16 / out rate · pitch at 0x724D0E: 0x1000
= the recorded rate), 1 (0x800..0x2000, +0x6E), 2 the volume (0..0x7FFF;
live gain v · 1/32767 at 0x72085B, the start byte v >> 8), 3 the azimuth
(+0x1C, u16), 4 (±0x4000, +0x1E), 5 an effect send (byte), 8 the second
volume (byte / 127 multiplies the channel gain, 0x724F5D). The car banks
use kinds 0, 2 and 3. Not ported: the azimuth / pan law, kinds 1, 4–7.

**BNKl** (verified; 0x727669 / 0x727A46 / 0x72AAF8). u16 count at +6; sound
i's header at entry + u32 [bank + 0x14 + 4i] (0 = none), "PT\0\0" then
tags: 0xFC skipped, 0xFF ends, 0xFD/0xFE valueless, else a size byte (0xFF:
BE32 size follows) and a big-endian value, sign-extended for 1–3 bytes
(0x72A195). 0x84 rate (22050), 0x82 channels (1), 0xA0 codec (10), 0x85
samples, 0x88 data offset from the bank. Every sample in the install is
codec 10, mono.

**Codec 10, EA-XA** (verified; 0x728DF9 class 2, 0x734AFF / 0x734A81).
15-byte blocks of 28 samples: byte 0 high nibble picks (c1, c2) from
{0, 0.9375, 1.796875, 1.53125} / {0, 0, −0.8125, −0.859375}, low nibble
the shift; nibbles high first, s = c1·h1 + c2·h2 + nibble·2^(12−shift)
(0 for shift > 12; table 0x819688). A block starting 0xEE: BE16 h1, BE16
h2, 28 raw BE16 samples (61 bytes). History starts at 0.

**GIN "Gnsu20"** (verified; 0x466770). +8 f32 min rpm, +0xC max rpm,
+0x10 N, +0x14 M, +0x18 samples, +0x1C rate (24000–36000); N+1 u32 rpm →
position, M+1 u32 cycle starts, then 19-byte blocks of 32 samples
(0x466590): sample 0 = (b0 & 0xF0) + (i8)b1·256, sample 1 = (b2 & 0xF0) +
(i8)b3·256, b0 & 0xF the coefficients (same tables, 0x7ED92C/0x7ED93C), b2
& 0xF the shift (rows of 0x7ED950, same values), then 15 bytes of nibbles.
Reads (0x466C60) round to i16 with saturation and fail past the end.
rpm → position (0x466840) and cycle → position (0x466910) interpolate their
tables; position → cycle (0x466AF0) is the containing cycle plus the
fraction (the clone binary-searches, same result); cycle length (0x4669D0)
interpolates neighbouring half-spans. Player setup 0x467160: block =
rate·0.011, crossfade rate·0.0005; SetRPM 0x467230(rpm, ms): aim =
rpm → position over ms/11 + 1 blocks. Fill 0x466D30: ported. The engine
sound's three SetRPM calls (0x473FD3, 0x4741A3, 0x4742F5) all pass 60 ms
(0x42700000), as the clone does (verified).

**Engine choice** (verified). Sound index = 0x459240(car type index) (a
switch, default 0x12); level 3/2/1 with performance part 0x18/0x13/0x0F
fitted, else 0; engine entry = 0x7F2230[index][level]; entry (0x7F0DE0,
0x4C bytes) = EE bank, SPU bank, id, GIN name. Entries 41/42 reuse 14/12.
The car type index is the GLOBALB car type order (verified: with it the
stock recordings match the cars that have their own, e.g. 1 FOCUS →
FordFocus_Road, 3 240SX → Nissan_240SX, 8 HUMMER → Hummer, 27 SKYLINE →
Nissan_Skyline, 29 RX7 → Mazda_RX7; `globalb_order_picks_matching_recordings`).
File names match case-insensitively (GIN_Eclipse_5Zigen.gin).

**FE sounds** (verified). 0x50D150(event, control) plays table 0x837FE0
[event·27 + control] (filled by 0x50CF40; values in `audio/fe.rs`, −1 =
none). PlayFESound 0x458520: id < 23 → FE_COMMON's CommonSample (0x469760),
23–93 → FE_MB's FrontEndSample with id − 23 (73 → the _loop event), index
clamped to 0–0x4B, volume = master FE volume, pitch 0x1000, pan 0. Player
parameter setters 0x817F60: pitch +0xC, +0xE clamp 0x800–0x2000, volume
byte clamp(v, 0, 0x7FFF) >> 8; defaults 0x721A6B vol 0x7F, pan 0x40,
pitch 0x1000. Which control kind each clone screen counts as (icon menus
ICONSCROLLER, car lot CARLOT, parts BROWSER, paint PAINT, dialogs BUTTON):
inferred.

**Mixer channel law** (verified unless marked; the game's own mixer code,
not the library's). **Not ported**: the stereo and surround weights (the
law is not complete; see "Open" below), the writer of the flag rec(p)+0x4E,
and the distance's level, so the clone keeps its current linear volume. The
signed byte rec(p)+0x5F is written by 0x723C41 (see the angle-word bullet).

* Channel gains (0x724F5D, verified). For record p and each channel
  c < [0x8B46E8]: g = (float)s · v · K1 · W_p[c], where s = the signed byte
  at rec(p)+0x5F (movsx), v = the float at rec(p)+0x48, K1 = [0x78A94C] =
  0.007874015718698502 (float32 of 1/127) and W_p[c] = the float at
  block(p)+4c. The product s·v·K1 is stored as a float32 (0x724F9B) before
  it is multiplied by W_p[c] (0x724FA4), and g is stored as a float32
  (0x724FA7). rec(p) = [0x8B466C] + 0x90·p; block(p) = [0x8B4724] + 0x88·p.
  0x72905D stores g at V(p)+0x1C+4c and sets V(p)+1 := 1 (the snapshot below
  clears it; the name "ramp pending" is inferred), V(p) = [0x8B4A00] + 0x5C·p.
  Its only caller is 0x724FAC.
* Aux gains (0x729E24 and 0x729087, verified). 0x729E24 has no direct call
  site; its only reference is the store into [esi+0x74] at 0x724E22, where
  esi = block(eax) is set at 0x724D92 (the callback at block+0x74). For group g =
  word [obj+4] and each child vid of rec(g) (count byte +0x23, words at
  +4+2k) it calls 0x729087(vid, i, x·[rec(g)+0x48]), with x and i its
  arguments. 0x729087 stores the value at [V(vid)+0x38] + 4i and sets
  V(vid)+1 := 1. Its only caller is 0x729E5E.
* Weights (0x724FC5, verified; callers 0x7208F8, 0x72092E, 0x722387,
  0x72410E, 0x724EAA; it ends with a call to 0x724F5D). If rec(p)+0x4E is
  zero: W_p[c] = T_c[b] · K2 for c < [0x8B44DF], where b = the high byte of
  the word at rec(p)+0x1C (passed to 0x725878, whose only caller is
  0x725012), T_c = the table at [0x8B473C + 4c] and K2 = [0x784548] =
  0.003921568859368563 (float32 of 1/255). With [0x8B46EC] = 3, W_p[5] := 0
  (0x7258BE). If rec(p)+0x4E is not zero: W_p[c] = 0 for c < [0x8B46E8] and
  W_p[5] = 1.0 (0x724FEA–0x725007).
* Tables (0x72559B, verified; callers 0x721901 and 0x72470C, after the
  layout below). N_t = [0x8B44DF] tables of 256 bytes: one block of
  N_t·256 bytes (from 0x724792) at [0x8B4754], table c at +0x100·c, its
  pointer at [0x8B473C + 4c]. N_t = 1 writes 127 in every row
  (0x725610–0x725619). For N_t ≥ 2 each row b (0..255) is zeroed
  (0x725620–0x72563B) and then compared with the speaker angles (fcomp
  against [0x8B46D4] at 0x725644); the row angle is b·360/256 degrees
  (b·256 times 0x784248 = 360/65536, 0x725602–0x725605). Helper 0x72554C
  (calls at 0x72567B, 0x7256EB, 0x725784, 0x725812; π/180 at 0x7C6918, π/2 at
  0x784254) takes an output pointer and three angles a1, a2, a3 in degrees,
  computes T = (θ3 − θ2)/(θ1 − θ2) · π/2 with θi = ai · π/180, and stores
  0x72A0AE(T) and 0x72A032(T) as its two floats (0x72554C decompiled, REA).
  For N_t = 2 (verified): θ = 256 · b · 0x784248 with 0x784248 =
  360/65536 exactly, so θ = b · 360/256 degrees. The tests are θ ≤ 90 at
  0x725641 against [0x8B46D4] = 90, then θ < 270 at 0x72573C against
  [0x8B46D0 + 4·N_t] = [0x8B46D8] = 270 (the N = 2 speaker angles above).
  θ ≤ 90 gives a1 = 90, a2 = −90, a3 = θ (0x7256C4; a3 stored at 0x72565E);
  90 < θ < 270 gives a1 = 90, a2 = 270, a3 = θ (0x7257C4); θ ≥ 270 gives
  a1 = 90, a2 = −90, a3 = θ − 360 (0x72574F). So T = d · π/360 with
  d = θ + 90, 270 − θ or θ − 270, T_0[b] = round(255 · 0x72A0AE(T)) = round(255 ·
  sin T) and T_1[b] = round(255 · 0x72A032(T)) = round(255 · cos T). The round
  is fistp, the FPU rounding mode, taken as nearest-even (the global control
  word was not traced; all 52 fldcw loads in the binary use stack slots).
  Stores: T_0 at 0x725717, 0x7257B0 and 0x72583E; T_1 at 0x725734 (table
  [0x8B4738 + 4·[0x8B44DF]], which is [0x8B4740] for N_t = 2) and 0x725853.
  Rows b = 0, 32, …, 224 are (180,180), (236,98), (255,0), (236,98), (180,180),
  (98,236), (0,255), (98,236). Recomputed in Python with exact sin and cos for
  all 256 rows: the nearest value to a rounding boundary is 0.0088 of a byte
  (b = 51, cos), far above float32 error. 0x72A032 is the cosine series
  1 − x²/2! + … + x¹²/12! (constants 0x784260 = 0.5, 0x786DB0, 0x786DAC,
  0x786DA8, 0x786DA4, 0x7C6A24) and 0x72A0AE the sine series x − x³/3! + …
  (constants 0x786D94, 0x7C6A30, 0x786D8C, 0x786D88, 0x7C6A2C; the 1/13!
  constant at 0x7C6A28 is not read). Each first reduces by 2π = 0x786D9C
  (REA). Truncation error is below 10⁻⁸ on [0, π/2]. Not traced: the N_t ≥ 5
  branch (from 0x725664).
* Speaker angles (0x7258E6, verified; callers 0x7218FC and 0x724707, after
  [0x8B46E8] := [0x8B44E0] + [0x8B44DF] at 0x7218E9–0x7218F7 and
  0x7246F7–0x724702). N = 1: mode [0x8B46EC] = 0, [0x8B46D4] = 0.0. N = 2:
  mode 1, [0x8B46D4] = 90.0, [0x8B46D8] = 270.0. N = 4: mode 2, 45, 135,
  225, 315 at [0x8B46D4]..[0x8B46E0]. N = 6: mode 3, [0x8B46D4] = 0.0, then
  45, 135, 225, 315 at [0x8B46D8]..[0x8B46E4]. Other counts change neither.
* Ramp snapshot (0x728F75, verified). It sets V+1 := 0 and V+0x3C := 0.0,
  copies [V+0x38][i] into [V+0x34][i] for i < [0x8180B8] and V+0x1C+4c into
  V+4+4c for c < [0x8B47F9], then V[0] := 2. The aux ramp's endpoints are
  these arrays: g0 = [V+0x34][i] and g1 = [V+0x38][i] are loaded at
  0x728BE3–0x728BE6, compared at 0x728BED, and passed to 0x728957 at 0x728C11
  (not traced).
* Angle word and pan (verified unless marked). The pan setter
  0x724093 takes (voice handle, dword angle, pan). For each record p of the
  voice's list with count byte +0x23 = 1 it stores the pan, clamped to
  [−0x4000, 0x3FFF], at WORD rec(p)+0x1E, and writes WORD rec(p)+0x1C :=
  (WORD +0x50 + WORD +0x4C + the dword angle) mod 2^16, zeroed when
  [0x8B46EC] = 0, then calls 0x724FC5 (0x7240E8–0x72410E). It returns −10 when
  [0x8B4608] = 0. The slot-3 setter 0x7208D8 (pointer at 0x817F3C; 0x817F30
  and 0x817F40 hold 0x7207E9 and 0x7208FF, whose id roles are not traced); the
  setter writes the low word of its second argument to +0x1C under the same
  count-1 test and calls 0x724FC5. The weight routines read no +0x1E: in 0x724F5D–0x725480 the only
  read of +0x1C is at 0x72500D (in 0x724FC5). Three game calls reach 0x724093,
  each pushing exactly two arguments, so the callee's third slot is the
  caller's stack: 0x473F5A, 0x47412F and 0x474285. At 0x473F5A (the engine
  update 0x473E60, called at 0x47433F from the code at 0x474310, which does not
  set EDI) and that was wrong: the third slot holds a pushed zero (see the angle-slots
  bullet below), so the pan is 0 there if the callee pops four bytes (checked only
  for vtable 0x78AF30; the class of ESI is not identified). The dword angle is
  the result of `[vtable+0x3C](0)` on the object (one argument; the second pushed
  zero stays on the stack; class not identified; the other two callers do the
  same). The engine update also sets the signed byte
  rec(child)+0x5F through 0x723C41 (verified: for each record p of the voice's
  list and each child, the byte := its second argument, then a jump at 0x724CCB
  to 0x724F5D(child)). The engine update 0x473E60 calls 0x723C41 at 0x473FA7
  with ebx pushed; ebx holds clamp(DAT_0082A998 >> 8, 0, 0x7F) from
  0x473ECF–0x473EE6 (ebx not re-checked over 0x473F5F–0x473F7A). The row is the high byte of the angle
  word, and the rows are b · 360/256 degrees, so a 16-bit azimuth with 0x10000
  = 360 degrees would fit (inferred, not shown). Not established: any game path
  that writes a layer azimuth into +0x1C, the slot-3 value (its writer is not
  found), and the meaning of the pan argument. The azimuth and pan to L/R law is
  therefore not complete.
* Channel count (verified unless marked). The init path at 0x7249E5
  (the branch at 0x7249C7 of the
  function at 0x7249BF) stores N_t = 2 at 0x724A72 on its success path (the
  call 0x725DB5 at 0x724A02 returns ≥ 0), after [0x8B44A4] := 1 (0x724A3D).
  Then 0x724AF6–0x724B18 clamp N_t into [[0x8B44A4], [0x8B44A5]], and
  0x724B85–0x724BBC set [0x8B44E0] from bl (0x724B91, 0x724BA3; bl = 0 from xor ebx,ebx at
  0x724AC7, and ebx is not written later in 0x724AC7–0x724BC3)
  and, for AL = 6 with [0x8B44E0] = 0, set N_t := 5 (0x724BB5) and
  [0x8B44E0] := 1 (0x724BBC). From N_t = 2 the clamp leaves 2 when [0x8B44A5] ≥ 2 and gives
  [0x8B44A5] when that is smaller, so AL = 6 is not reached on this path
  (arithmetic). The total [0x8B46E8] = N_t + [0x8B44E0] is therefore 2 (bl = 0, and
  0x724BA3 leaves [0x8B44E0] = 0). The address 0x8B44A5 is the first argument
  (pushed last) of the call 0x725DB5 at 0x724A02 (the call's writes to
  [0x8B44A5] are not traced); other
  entries into 0x724AF6 are not traced.
* Open, all needed for stereo (after the audio pass: not complete, nothing
  ported). The angle word rec(p)+0x1C and the pan are traced only as far as the
  two bullets above. The writer of the flag rec(p)+0x4E is not identified (68
  listing lines use +0x4E; the writers for this record are not separated out).
  The weights of channels c ≥ N_t: 0x725878 does not write them, while 0x724F5D
  reads c < [0x8B46E8] = [0x8B44E0] + [0x8B44DF]; the initial value of
  [0x8B44E0] is not traced. The distance (+0xA8, see Impact sounds). The runtime
  volume scale ([0x8180B4] is 1.0 in the file and is stored 1.0 at 0x7246B5,
  fld1 then fstp, its only direct store; read at seven sites, 0x720897,
  0x722052, 0x722286, 0x723436, 0x723F6E, 0x724008 and 0x72773F, not traced;
  the name is inferred).
* Angle source of the engine objects (verified unless marked;
  still not complete, nothing ported). The object with vtable 0x78AF30 is
  registry entry "CARSFX_PlayerEngine": record 0x7ED458 = {name 0x788C54,
  0x7ED138, factory 0x47D6E0, type 0x10080}; the name string is at 0x788C54.
  The factory returns 0 unless the call to 0x43E910 returns at least 0x510;
  otherwise it allocates 0x110 bytes through 0x440820 and jumps to the ctor
  0x473D30 (vtable store at 0x473D38), or returns 0 if the allocation fails.
  The destructor, slot 0x00, is 0x47D720, which calls 0x473D90. Other entries
  of the table: CARSFX_AIEngine (name 0x788C68, factory 0x47D680, type
  0x10000), CARSFX_Hydraulics (name 0x788C40, factory 0x47DAE0), CARSFX_Rain
  (name 0x788C34, factory 0x47DFE0) and CARSFX_DriveBy (name 0x788C24). The
  DriveBy entry has 0x47E070 where the others have 0x7ED138, so its factory
  field is not confirmed; 0x47E070 is a factory-shaped function.
  The angle getter 0x45E3B0 (vtable slot 0x3C) reads WORD [S+2], where
  S = [this+0x58+idx*4] and idx is its argument; the one traced caller
  (0x46126A) passes 0. The level getter 0x45E390 (slot 0x38) reads DWORD
  [S] & 0x7FFF, so the angle is the high half of the dword that also holds
  the level. Both return 0 when S is null. Neither the ctor 0x473D30 nor the
  base ctor 0x473BF0 has a store to offset 0x58.
  The code at 0x46126A–0x46128D calls vtable slot 0x3C of esi with argument
  0 (the class of esi is not shown), clamps the result to 0..0x10000 (signed)
  and stores it at [[esi+0xB0]+0x40]. The enclosing method is not traced.
  FUN_00480420 (entry 0x480420, body 0x480420–0x480858 ending in ret; its
  only resolved caller is 0x483900; decompiler output, cross-checked against
  the disassembly) stores obj+0x58 + idx·4 := entry block + sub·4, at 0x480698
  when the entry has no 3D connections and at 0x4807B6 after the 3D loop. obj
  is the sound object found by ID through FUN_00468F70, with key
  (ID & 0xE0FFFFFF) | ([instance+0x50] << 11), where ID is the entry's
  vtable+0x10 value. idx = bits 16..19 and sub = bits 24..27 of an input-map
  word. The entry block is entry+0x4C = channel·0x40 + [instance+0x40], set
  only when (entry+0x08) & 0xFF000000 == 0xC0000000; channel is entry+0x48,
  the index of the entry's ID group from the loop over the entries at
  [instance+0x34] (count [instance+0x6C]), not a channel number.
  The 3D objects (list at [instance+0x24], count [instance+0x60]) are used
  through vtable slots 0x08 (its return value is compared with the lookup
  ID), 0x0C (its return value builds the connection key), 0x14 (called with
  the entry, obj and the key) and 0x18 (its value is stored at
  [entry+0x3C] + k·4, k the connection index). The debug strings "3dLookupID"
  and "SFXOBJOUT" are present. The methods behind the slots are not traced,
  and their roles are not established.
  **Missing**: the writer of the dwords in the entry blocks (the AEMS
  evaluation, not traced), so where the azimuth value is produced; the slot
  0x18 methods and how the 3D objects are created; any azimuth computed from
  positions; which entry block drives left and right; whether the object
  found by ID is a CARSFX_PlayerEngine instance (the listing has 579
  destination operands with offset 0x58, so FUN_00480420 is not shown to be
  the only writer of that table). The sound code (0x45xxxx–0x48xxxx) and the
  mixer/AEMS code (0x70xxxx–0x74xxxx) contain no trig instruction. All 41
  trig instructions (fsin, fcos, fptan, fpatan, fsincos) are at 0x5BFD50,
  0x63D917–0x63ED11, 0x652FD8–0x653E6D, 0x67431D, 0x6749A7–0x6749B4,
  0x689227–0x68977A, 0x6A1453–0x6A1CD8, 0x6E715B–0x6E7181, 0x75DBC6,
  0x75EEE4, 0x76AA15 and 0x76AA18; no caller of these sites is traced.
  REA returns no procedure at 0x76AA15, which follows a ret at 0x76AA14. The
  listener frame, the unit of the 16-bit azimuth (0x10000 = 360 degrees is
  inferred) and the sign convention are not established.
* Angle slots, pan inputs and the object label (verified unless
  marked as inferred or not traced; the angle source is still not complete,
  nothing ported). Vtable
  0x78AF30 holds 0x45E390 (slot 0x38) at 0x78AF68, 0x45E3B0 (slot 0x3C) at
  0x78AF6C and 0x474310 (slot 0x48) at 0x78AF78. Both accessors take the
  index as their argument and read S = [this+0x58+4·arg]. Slot 0x38 returns
  DWORD [S] & 0x7FFF; slot 0x3C returns WORD [S+2]; each returns 0 when S is
  null (0x45E390–0x45E3A8 and 0x45E3B0–0x45E3C5; every path ends in `ret
  0x4`). So the level and the angle are the two halves of one packed dword:
  bits 0–14 the level, bits 16–31 the azimuth word. The binder (FUN_00480420,
  body 0x480420–0x480858, `ret` at 0x480858) stores S = table + 4·sub at
  0x480698 and 0x4807B6 (`mov [ebp+eax*4+0x58], edx`; verified). Here table =
  [ebx+0x4C] (the entry, in the worker's naming; `mov edi,[ebx+0x4c]` at
  0x480680 and 0x4807A2), sub = bits 24–27 and the slot index = bits 16–19 of
  the dword read from [ecx]. The writer of the packed dword itself is not found
  (see the missing list below).
  Pan call inputs (stack layout verified; callee conditional). At 0x473F49–
  0x473F5A, 0x47411E–0x47412F and 0x474274–0x474285 the caller pushes two
  zeros, calls `[eax+0x3c]` with ecx = esi (for 0x473F49, eax = [esi] at
  0x473F44, so the callee is the vtable of esi, whose class is not identified;
  the other two sites are not checked), then pushes the angle and the handle
  ([esi+0xE0] at 0x473F52 and 0x474127, [esi+0xFC] at 0x47427D) and calls
  0x724093. If the callee is 0x45E3B0 (vtable 0x78AF30 slot 0x3C), its `ret 0x4`
  pops one zero, so the second zero is still on the stack at the setter's entry
  as its third dword. The setter reads it at [ebp+0x10] (0x7240A6) and
  range-checks it against 0xFFFFC000..0x3FFF (0x7240A9–0x7240BA). So the pan is
  0 at all three sites on that condition; the pop count of other callees is not
  verified. This corrects the earlier bullet (that slot is not EDI) and the earlier
  count of one push at 0x474285 (there are two, at 0x474274 and 0x474276). The
  setter's exits seen (0x7240A4, after `leave`; 0x724129; 0x72413D) are plain
  `ret`. The instruction after each call is not an `add esp`, so the three
  dwords are cleaned up elsewhere (not traced).
  Object label. The 0x7ED440 area is referenced only by direct addresses (all
  43 references to 0x7ED440–0x7ED4FF in the listing are literal; no indexed
  walk). 0x7ED440 is a plain dword initialised to -1 at 0x473C3C. The pushes at
  0x47F830–0x47F8D5 pass addresses to 0x479130 with ecx = 0x82BEBC (each push is
  followed by `mov ecx,0x82bebc; call 0x479130` through 0x47F8D0): 0x7ED454,
  0x7ED464 and 0x7ED474 (the type fields of records k = 0..2 in the main
  reading), and 0x7ED444, 0x7ED430, 0x7ED3DC, 0x7ED394, 0x7ED3A8, 0x7ED3B8,
  0x7ED3C8, 0x7ED410, 0x7ED420. The 0x47F8E4–0x47FB3C range is not checked.
  Verified dword positions: 0x7ED448 = 0x788C68 ('CARSFX_AIEngine'),
  0x7ED44C = 0x7ED138, 0x7ED450 = 0x47D680, 0x7ED454 = 0x10000; 0x7ED458 =
  0x788C54 ('CARSFX_PlayerEngine'), 0x7ED45C = 0x7ED138, 0x7ED460 = 0x47D6E0,
  0x7ED464 = 0x10080; 0x7ED468 = 0x788C40 ('CARSFX_Hydraulics'), 0x7ED470 =
  0x47DAE0; 0x7ED478 = 0x788C34 ('CARSFX_Rain'), 0x7ED480 = 0x47DFE0; 0x7ED488
  = 0x788C24 ('CARSFX_DriveBy'), 0x7ED490 = 0x47E070. Read as [name, list head
  0x7ED138, factory, type] from 0x7ED448 + 16k, the five records k = 0..4 are
  consistent (the same dword 0x7ED138 in each, read as a list head; no factory
  is −1). The area continues to at least 0x7ED4F8 (not decoded). Vtable 0x78AEE0
  slots 1 and 2 are 0x461040 (address of
  0x7ED444) and 0x461050 (dword 0x7ED448); vtable 0x78AF30 slots 1 and 2 are
  0x461470 (address of 0x7ED454) and 0x461480 (dword 0x7ED458); the four
  accessor bodies were checked. If slot 2 is a name getter (inferred; its
  callers are not traced), each vtable names its own factory's record. Factory
  0x47D680 calls 0x440820 with 0xC4 and tail-jumps to the ctor 0x473CE0, which
  stores 0x78AEE0 at 0x473CF8 (verified), so 0x78AEE0 is AIEngine on that
  reading. Factory 0x47D6E0 calls 0x440820 with 0x110 and tail-jumps to the
  ctor 0x473D30 (`jmp 0x473d30` at 0x47D718), which stores 0x78AF30 at 0x473D38
  (verified), so 0x78AF30 is PlayerEngine on that reading. The other reading,
  [factory, code, name, head] from 0x7ED450 + 16k, pairs 0x47D680 with
  PlayerEngine and gives the record at 0x7ED440 the factory −1; it is not used
  (0x7ED440 is also a plain global set to -1 at 0x473C3C). So the label
  CARSFX_PlayerEngine for 0x78AF30 is consistent with the bytes and the
  inferred getter; the registry walk is not found in this pass.
  **Still missing for the angle** (so no stereo law is ported): (a) the writer
  of the packed dwords in the 16-dword blocks whose pointer the binder reads
  from entry+0x4C (verified above); the code that sets entry+0x4C and writes
  the packed dwords is not traced, and it is presumably the azimuth source (the
  AEMS evaluation, not traced); (b) whether the instance found by 0x468F70 (it
  indexes [ecx+edx*4+0xA8] with edx = bits 16–23 of its argument) is a
  CARSFX_PlayerEngine (vtable 0x78AF30); (c) the azimuth unit, sign and
  listener frame (0x10000 = 360 degrees stays inferred). The stereo weights
  also need the writers of rec(p)+0x4C and rec(p)+0x4E, which were not traced
  in this pass.
* Derived, not applied: with N_t = 1 the weight is 127/255, so the traced
  law gives g = s·v/255 for a single channel. The init path (see Channel
  count) gives N_t = 2.

**Open.** The voice kinds 1, 4–7; the master FE volume value; the
engine's EE/SPU bank layers (the +0xB0 block, see "Engine volume") and the
_DCL decel recording's voice; the .csi event files (EVT_SYS, "MOIR") and
.fx; the scrape (FX_Scrape), the traffic horn and bottom-out, DriveBy,
Hydraulics and Rain (see "Road, wind, bottom-out, AI and traffic sounds,
pass": RoadNoise, WindNoise, WindWeather, BottomOut, the AI engines,
the traffic engines and whooshes are ported there); SparkChatter is the
decel crackle; no music or speech in this install.


**Car effect sounds** (verified unless marked; `src/audio/car/`,
probes the maintainer's probe `sfx_banks`, the maintainer's probe `sfx_players`).

* Registry (records {name, list head 0x7ED138, factory, type}; the type is
  the dword before the name, see Registry layout below): CARSFX_Shift
  0x7ED398 (factory 0x47C800 → ctor 0x471ED0, vtable 0x78ABA0), CARSFX_Turbo
  0x7ED3AC (0x47CA70 → 0x472480, vtable 0x78AC10), CARSFX_Nitrous 0x7ED3BC
  (0x47CBE0 → 0x4729D0, vtable 0x78AC58), CARSFX_Skids 0x7ED434 (0x47D5D0 →
  0x473A70, vtable 0x78AE48); controllers SFXCTL_Shifting 0x7ED2A8 (0x46EBD0 →
  0x45C0D0, vtable 0x789F28, type 3), SFXCTL_AIShifting 0x7ED2B8, SFXCTL_Skid
  0x7ED248 (0x46DF40 → 0x45BB00, vtable 0x789EE0). Slot +0x18 picks the bank,
  +0x10 attaches controllers by type (`>> 4 & 0xFFF`), +0x14 binds the sound
  context, +0x24 is the update. Bank paths 0x7ED160 on (sound\NOS\,
  sound\Shifting\, sound\Turbo\, sound\Skids\), names 0x7ED1A0 on; event
  names 0x7EDDA8 on (FX_NITROUS, FX_PURGE, FX_SHIFTING_01, FX_SKID,
  FX_TURBO_01, CAR_SWTN). The clone's event table read takes the name 4 bytes
  late ("ITROUS", "HIFTING_01"; the FE names lose "Play"): the name block's
  string starts at +4 (**noted, not changed**).
* Sound context levels (0x4593B0): engine level 3/2/1 by parts 0x18/0x13/0x0F
  (+0x94), then round(upgrade state float[cat]) (0x5995F0, x + 0.5 then ftol)
  clamped 0–3: turbo cat 4 (+0x98), nitrous cat 2 (+0x9C), tyres cat 8 (+0xA0),
  transmission cat 1 (+0xA4). The floats' writer is **open**; the clone takes
  the highest fitted package level of the category (**inferred**).
* Engine table 0x7F2230 (33 × 4 × 12 bytes, [sound index][level]): {engine
  entry, shift record, turbo index}. The shift object reads the shift record
  with the transmission level (0x45E6A3): record = 0x7F2860 + 0x60·n
  (`carfx::SHIFTS`, `SHIFT_BANKS`: +0 bank name GEAR_{SML,MED,LRG,TK}_{Base,
  Lev1..3}, +4 up volume, +8 up clunk delay s, +0x3C down volume, +0x40 down
  delay). +0xD4 = 0x7F3040 + 12·engine entry (name, two dwords; not used).
* **Nitrous** (0x484F20, vtable +0x24, runs when bound +0xA0): state +0xB8.
  0 → 1 when 0x59FA40 (car +0x40) is non-zero and 0x5ECDC0 is 0: FX_NITROUS
  (0x472AF0 → 0x457E70(0, level, angle, 0)) and the ramp +0xCC (0x459990)
  0 → 1.0 over 400 ms; also the GB_NOS speech (0x484510) when the tank fraction
  (0x5925C0) > 1.5 under further mode gates (not ported). 1 → 0 when the stage
  is 0 or 0x5ECDC0 non-zero: release (0x45F2A0: event +0x10 = 1) and ramp
  current (+0xE0) → 0 over 1000 ms. Purge: 0x5ECDC0 non-zero with none playing
  starts FX_PURGE (0x45F2D0); zero again releases it (0x45F3E0, event +0xC =
  1). 0x5ECDC0(car) = the stage when |car+0x30→+0x20C| ≤ 0.001 or car +0x430 ×
  2.237 < 5 mph, else 0 (+0x20C the input block's gas, **verified**
  2026-11-04; `carfx::purging(stage, speed, gas)`). Bank
  (0x465830): Nitrous_01 for nitrous level 2 or 3, else Nitrous_00. Each frame
  (+0x28, 0x45F4B0) the event's volume = slot 0x38 level (0..0x7FFF) and its
  angle = slot 0x3C: the mix map's (ported, see "Mix map").
  The blocks (0x457E70): FX_NITROUS {variant 0..2, volume, azimuth,
  stop flag 0..1}, the stop (0x45F2A0) sets the flag and dispatches; FX_PURGE
  (0x45F2D0) starts all zero and per frame (0x45F4B0) gets {level[1],
  azimuth, stop flag (0x45F3E0), 0x45E3D0 = 0x1000 unless paused}; an event
  whose instance ended (owner count ≤ 1, 0x735070) is freed. The graphs
  pick the voices and do the fades (the FX_NITROUS loop rises over ticks in
  the graph; the +0xCC ramp is not what the voices follow). Ported.
* **Shift** (SFXCTL_Shifting update 0x483130): the context's gear (+0x74 →
  +0xC8) above the last (+0xCC) starts an up-shift (0x46EC70) only over 3000
  rpm (0x7885C0, engine +0x70); below it a down-shift (0x46EE60) unless the
  new gear is 1. Both arm +0x98 with +0xA0 = now + the record's up (+8) /
  down (+0x40) delay; 0x482D50 sets the play flag +0x70 when the clock passes
  it, and CARSFX_Shift's update (0x47C890) then plays FX_SHIFTING_01
  (0x45E730) with variant 1 for the up-shift states 1–3 (+0x78), else 0.
  Volume (0x45E730): t = (round(rpm) − 1500)/6500 clamped (0 when ≤ 0), k =
  trunc(512 − 512·(0.1 + 0.9t)), level × ((fade[k] × record volume) >> 15) >>
  15, clamped 0..0x7FFF, level = slot 0x38 (0 up, 1 down; the mix map's,
  was 0x7FFF). fade = 0x7EF7F8 = trunc(32767·cos(π/2·k/511)) (all 512
  checked). The flags +0x71 / +0x72 play CAR_SWTN ("ShiftDisengage",
  0x4720D0 / 0x4722B0) on the engine's banks only at ≥ 7000 rpm: **open**
  (not ported). The event block (0x45E730 → 0x7358C0 at 0x45E990): {variant
  0..2, volume, 0x1000, azimuth, 0, (context +0xA8 = 2)}, the last event
  released first; the graph picks the sample (variant 1 → entry 1). The
  +0xA8 flag's meaning is **open** (the clone 0). The
  rpm-blip state machine of 0x482D50 (states 1–7, jump table 0x483114) is
  ported (see "Engine transients").
* **Registry layout** (verified): the records are {type, name,
  list head, factory}, 16 bytes, the type dword first (each class's slot 1
  returns the address of its record's type, slot 2 the name: e.g. 0x45C4B0
  → 0x7ED2C4 and 0x45C4C0 → [0x7ED2C8] for SFXCTL_Engine). Attach (slot
  +0x10) switches on (type >> 4) & 0xFFF. Controller types: 0 Physics
  (0x470A00) / AIPhysics, 1 Wheel (0x46E820), 2 Skid (0x46DF40), 3
  Shifting / AIShifting, 4 AccelTrans, 5 Engine (0x47BFD0, vtable
  0x789F70), 6 HybridMotor, 7 Tunnel, 8 3DCarPos, 9 3DObjPos, 11/12 the
  right / left wheel positions, 13/14 the left / right wind positions.
  Car objects: Shift 1, Turbo 2, Nitrous 3, SparkChatter 4 (FX_SPARKCHATTER,
  the scrape), Skids 5, RoadNoise 6 (ROADNOISE_00_MB, FX_ROADNOISE /
  _TRANS), WindNoise 7 (WIND_00_MB, FX_WIND), Hydraulics 8, Rain 9,
  WindWeather 10, BottomOut 11 (0x7ED568), DriveBy 12; traffic objects
  (0x7ED524 on, type high word 3): TrafficEngine 0 (0x47E5B0), TrafficWoosh
  1 (0x47E870), TrafficHorn 2 (0x47E6A0), TrafficBottomOut 4 (0x47F2B0),
  all with TRAFFIC_MB.abk (FX_TRAFFIC; one module, one player). Bank names
  0x7ED1F8 on: the four skid banks, the two stitch banks, ROADNOISE_00,
  ENV_COMMON, FX_MAIN_MEM, WIND_00, FX_Rain, TRAFFIC, FX_Misc,
  Hydraulics_00. None of RoadNoise, Wind, SparkChatter, BottomOut or the
  traffic objects is traced or ported.
* **Turbo** (verified unless marked; `carfx::Turbo`, ported).
  Bank (slot +0x18, 0x465AF0): [record +0] under sound\Turbo\ (0x7ED188);
  the record (bind 0x45EB30) is 0x7F3338 + 24 · n with n = dword +8 of
  the 0x7F2230 entry [sound index][turbo level], turbo level = 0x5995F0(car
  +0x300, 4) (round of the upgrade float, category 4) at +0xAC. Records
  (`carfx::TURBO_BANKS`): +0 the name (TURBO_{SML1,SML2,MED,BIG,TRUCK}_{0,1,2}
  _MB.abk: the 15 files of the install; the TURBO_00A.. names at 0x7ED1A8 are
  not used by this path), +4 the whine volume, +8 the variant-1 blow-off
  volume, +0xC the variant-2/3 one, +0x10 the full spool 40.0, +0x14 the
  decay 0.5 (all records). Attach (0x472680): type 3 → +0xB4 (shifting),
  5 → +0xB8 (SFXCTL_Engine), 8 → +0x1BC. The boost value is the engine
  controller's slot 0x38 (0x45C540: +0x130), written by 0x45C710 as the
  3-frame running mean (+0x90, 0x579F30(3) / 0x576980) of the sound state's
  +0x44 (outside shifts; [+0x6C]+0xC0 while shifting) plus +0x148 (0x45C7A0:
  a shift offset, 0 outside shifts; not ported). The sound state's +0x44 is
  0–100 on the AI path (0x46FF0C, clamped to 100); the player's writer is not
  traced (the clone: throttle × 100, **inferred**). Update 0x47CAD0: +0x184 =
  value × 0.01 when value > 20.0 ([0x7ED3A4]), else 0; then 0x4726E0 and
  0x472770; then +0x180: 0 → 1 when value > 20; 1 → 2 when value < 20,
  starting the blow-off (0x45EE70), zeroing the spool +0xF4 and running
  0x472770 again; 2 → 0 when 0x45EFF0 (no blow-off event, its state 1 =
  ended, or its ramp +0xD8 = 0). Spool 0x472770 (when the whine event +0xBC
  exists, else 0x45F0A0 starts it: variant 0, volume level × 0 >> 15): unless
  paused ([0x82B884]+0x13) +0xF4 += +0x184 when it is above 0.01 (0x78425C),
  else −= record +0x14; clamp to 0..+0x10 and to k · +0x10 with k = 0.4 +
  0.6 · clamp((trunc(rpm) − 1000) · 0.01, 0, 1) (rpm = the sound state's
  +0x48; 0.4 = 0x788810, 0.6 = 0x78880C). At exactly the cap with +0x100
  clear: +0x100 := 1 and the envelope +0x104 is reset (0x4598C0) and given
  (0, 0, 1000 ms) and (0, 0.6 · 512, 2000 ms) (0x4598F0: from, to, ms,
  shape 0); away from the full spool with +0x100 set it is cleared. +0xF8
  = spool / full; +0xE4 = trunc(+0xF8 · 32767) × record +4 >> 15, and while
  +0x100 is set × fade[trunc(envelope)] >> 15 (fade = 0x7EF7F8; the
  envelope stepped by 0x46AF70 with the tick's dt). Blow-off 0x45EE70 (only
  with +0xA0, [0x7FAAA8] = 1 and none playing): variant +0xEC = 1, or 2 +
  rand(2) (0x43C1C0) when +0xF8 > 0.75 (0x785510); volume +0xE8 =
  trunc(record [+8 for variant 1, else +0xC] × +0xF8); the event 0x458260
  (variant, 0, trunc(+0xF8 · 1024), 0, angle, trunc(rpm)); ramp +0xC4 :=
  (1.0, 1.0, 1 ms). 0x4726E0 while it plays: when the sound state's +0x54 >
  20, the ramp is 1.0 and the shift state +0x78 is 0, ramp (1.0 → 0, 150
  ms); when 0x45EFF0 holds, 0x45F050 releases it. Per frame (slot +0x28,
  0x45ECE0): blow-off volume = clamp(trunc(((+0xE8 × level[1 for variant 1,
  else 2]) >> 15) × ramp), 0, 0x7FFF); whine volume = clamp((+0xE4 ×
  level[0]) >> 15, 0, 0x7FFF), +0xC = clamp(trunc(+0xF8 · 1024), 0,
  0x2000), +0x10 the azimuth (slot 0x3C, 0..0xFFFF), +0x14 = clamp(sound
  state +0xB0, 0, 0x400), +0x18 = clamp(trunc(rpm), 0, 10000). The bank's
  module has two sound players, list indices 0 and 4 (both loops; MED_0: 2
  and 4); the list's 1–3 are one-shots. The event block (0x458260) is the event +4 on: {variant 0..5,
  volume 0..0x7FFF, spool 0..0x2000, azimuth, the sound state's +0xB0
  0..0x400, rpm 0..10000}; 0x45ECE0 rewrites the whine's p1..p5 and the
  blow-off's p1 each frame. The FX_TURBO_01 graph (run by the clone) reads
  p0, p1, p2, p3, p5: the variant picks the list entry (1 → 1, 2 / 3 → 3),
  pitch and volume come from curves of spool and rpm (MED_0 at volume
  32767, rpm 8000: pitch 0xD01..0x1016 over spool 0..1024; the second loop
  is silent at 1000 rpm), a release (0x735550) stops it at once.
  **Inferred**: the per-tick spool steps scaled to 60 ticks a second; the
  levels from the mix map. **Open**: whether the object exists
  for cars with no turbo part (the clone: always).
* **Skids** (verified unless marked; ported in
  `src/audio/car/skids.rs`). CARSFX_Skids
  (vtable 0x78AE48): attach 0x473B40 (type 1 Wheel → +0x13C, 2 Skid →
  +0x138, 8 → +0x140), bind 0x460840 (events +0xC4 = 2 when the car's
  [ctx+0x2C]→+4 is 1, 1 for 2–4, else 0), bank 0x465B90: 0x7ED1F8[2 · drift
  + (class = 2)] under sound\Skids\ (0x7ED180), drift = 0x89E7D9 or
  0x890104, class = 0x458F00(sound index) (`[1,1,0,0,0,0,0,1,2,0,1,0,2,2,1,
  1,1,0,0,2,2,0,0,0,0,0,1,0,1,1,1,1]` for 1..32, else 0; class 2 =
  SKID_PAV2 / SKIDS_DRIFT2). Update 0x460A70, per event i < +0xC4: the
  surface (+0xC4 = 2 or 4: Wheel +0xB0 / +0xB4 by i; else +0xB0 unless +0xB4
  differs and matches +0xB8, …) 14, 15 or 22 gives the three layer volumes
  (+0xC8, +0xCC, +0xD0 + 12i) [0x7ED240] = 11579, else 0x7FFF; zero when the side's
  wheels are up (Wheel +0xE8 / +0xE9); the slip parameters +0xF8/+0xFC/+0x100
  + 12i = SFXCTL_Skid +0x1A0/+0x1A4/+0x1A8 (i = 0) or +0x194/+0x198/+0x19C
  (i = 1), their means with one event. Per frame 0x460C70: event i's +4/+8/
  +0xC = level[3,5,4 (i = 0) or 0,2,1] × the layer volume >> 15, +0x10/+0x14
  (0..0x400) and +0x18 (0..0x3FF) the slip parameters, +0x1C the azimuth,
  +0x30 (0..180) and +0x34 (0..1000) from 0x59FA20, +0x3C/+0x40 from
  0x5ED4A0 / 0x5ED540 (0..0x400), +0x44 = 1 on surface 8 or 9. SFXCTL_Skid
  (0x46DFE0, called from its slot +0x14 0x47BDF0): the tuning record +0x6C =
  0x7F2F20 + 0x48 · k, k = drift (0/1), or 2 + drift for sound indices 9, 13,
  14, 20, 21 (0x45BD81 table); slip = rec +0 × Wheel +0x90 (+0x98 for the
  other side) clamped to [rec +8, rec +4], a 3-frame running mean, then
  trunc((mean − rec +8) · 1024 / (rec +4 − rec +8)) → +0x194 / +0x1A0; the
  others use the wheel's +0xA0..+0xAC against rec +0x28 and the car's speed
  (+0x430 > 0.5). Its outputs (slot 0x14): +4/+8/+0xC the side means, +0x10
  = clamp(max · 32, 0, 0x7FFF). Records 0–3 (rec +0, +4, +8): (1.3, 11, 0.4),
  (0.64, 12.3, 1.9), (0.85, 11, 0.64), (0.64, 12.3, 1.9). The skid banks hold
  five loops each (SKID_PAV 32 kHz, the others 36 kHz) and 7 / 5 sound
  players: which loop sounds for which parameter is the AEMS graph's (run
  since then).
  Verified unless marked: **SFXCTL_Wheel** update 0x46EA50 (slot
  +0x14 0x47BE80): +0x90 / +0x94 = the sums of the car's slip pair (+0x3B4
  + 8i longitudinal, +0x3B8 + 8i lateral) over wheels 1, 2 on the ground
  (+0x390 + i), +0x98 / +0x9C over wheels 0, 3 (order FL, FR, RR, RL);
  +0xA0 + 4i = |car +0x3D4 + 4i| (traced: the side wheels' spin,
  see "Skids, the wheel gates" below); 0x45BFB0: +0xB0 = car +0x394
  (wheel 0's surface) when it equals +0x3A0, +0xB4 = +0x398 when it equals
  +0x39C, else kept (race type 8 with no ghosts forces 25: not ported);
  0x46E8B0: +0xE8 = 0 when wheels 0 and 3 are both up, +0xE9 for 1 and 2.
  The same function first sets +0xD0 = car +0x60 + 1.5 (0x7ED268) × car
  +0x90 − 0.8 (0x7ED264) × car +0xA0 and +0xC0 = the same + 0.8 × +0xA0:
  the points SFXCTL_3DRightWheelPos / LeftWheelPos (+0xE0 / +0xE4) track.
  **SFXCTL_Skid** 0x46DFE0 (the means are 5-frame: 0x579F30(5); the start
  0x45BDF0 resets +0xA4, +0x144, +0x16C to 0 but never +0xCC): records
  0x7F2F20 + 0x48k, 18 dwords (`skids::RECORDS`). Layer 1: A = W90 > 0 ?
  r0·W90 : 0 clamped to [r8, r4], mean, +0x194 = T((mean − r8)·1024/(r4 −
  r8)); +0x1A0 from W98. Wheel gates: with |car +0x430| > 0.5 and WA4 or
  WA8 > r28 a timer runs; past r2C, I1 = T((max − r28)·1024/(1 − r28)),
  else 0 (WA0 / WAC → I2). n = the normalised velocity in the car frame,
  lat = |n.y|, back = |n.x| when n.x ≤ 0. ang = 0x59FA20 (heading − atan2
  of the velocity, 0 under 1 m/s), G = clamp((|ang| − r40)/(r44 − r40)).
  Layer 2: +0x1BC = min(old + r30, max(old − r34, T(I1·lat·r38))), +0x198
  = T((+0x1BC + (1 − r38)·G·1024)·k1), k1 = 1 when ang > 0 else r3C (+0x1A4
  with I2 and the opposite k). Layer 3: L = W90 < 0 ? |W94·rC| : 0 clamped
  to [r14, r10], q = T((L − r14)·1024/(r10 − r14)), m = T(mean q); with
  max(input +0x210 handbrake, +0x214 brake) > 0.15 +0x1C4 = T(I1·back·r20)
  else 0 and m = 0; slew (r18 up, r1C down); +0x19C = T((1 − r20)·m) +
  T(+0x1C4·r20) (+0x1A8 left). All six clamped to 0..1024. Right side
  (wheels 1, 2) +0x194..+0x19C, left +0x1A0..+0x1A8.
  **CARSFX_Skids**: creator 0x4608D0 makes count events of 0x48 bytes with
  p7 = clamp(0x82BC2C (drift), 0, 3), p8–p10 = 0x1000, the rest 0
  (0x7358C0 on 0x82C500); count = 2 when the context's vfunc +0x2C → +4 is
  1, 1 for 2–4 (meaning **open**; the clone 2 for the player, **inferred**).
  0x460A70: event 0 the left side (surface +0xB0, ground +0xE8,
  intensities +0x1A0..), event 1 the right; the layer volume 11579 on
  surfaces 14, 15, 22, else 0x7FFF, 0 off the ground. 0x460C70: p0–p2 =
  (level × volume) >> 15 (levels 3, 5, 4 / 0, 2, 1; the mix map's, was
  0x7FFF), p3, p4 (0..0x400), p5 (0..0x3FF) the intensities, p6 the
  azimuth, p7 drift, p8–p10 0x1000, p11 = T(|ang|·57.3) (0..180), p12 =
  T(speed·2.237) (0..1000), p13 0, p14 = T(0x5ED4A0: front lateral slip /
  speed, only against the steering), p15 = T(0x5ED540: rear), p16 = surface
  8 or 9. **Inferred** in the clone: the wheel's surface byte as car
  +0x394 + 4i's id, the handbrake from the wheels' +0x17C inputs.
* **Engine volume** (verified as read; not changed): the player
  engine update 0x473E60 sets the GIN voice's volume byte (0x72412A) to
  clamp(((level[0] × [[this+0xC0]+0x80]) >> 15) >> 8, 0, 0x7F), the master
  byte (0x723C41) to clamp([0x82A998] >> 8, 0, 0x7F) and the aux send
  (0x723BC1) to clamp(([0x82B078] × level[0]) >> 23, 0, 0x7F). The object
  at +0xC0 and its +0x80 (a 0..0x7FFF volume) are not identified, nor the
  mix map's level. So the clone's engine gain 0.5 + 0.5 · throttle is **not
  native** (no traced replacement yet).
  Verified (full notes in docs/engine-volume-chain.md; **not
  ported**): +0xC0 is SFXCTL_HybridMotor (type 6, vtable 0x789FE4, update
  0x47C3D0; 0x473BCC belongs to CARSFX_Skids). Its +0x80 is the volume:
  with the context's hybrid flag (+0x78: engine entry 0x7F0DE0 +0x10 ≠ 0;
  most entries) the player plays two granular voices (0x474030): the accel
  recording at +0x80 = T(v · rec80 +0x28 · G · FC · 0.01) and the decel
  (_DCL, record 0x7F1AA8 + 0x34·rec80 +0x14) at +0x84 = T((1 − v/100) ·
  rec84 +0x08 · G · FC), v the throttle % (Engine +0x130), G = Engine +0xE0
  / 32767 (0x7FFF outside shifts), FC a ±0.2-per-update smoothed gain whose
  target depends on Shifting / AccelTrans states and context fields not
  identified (+0x4C, +0x54, [[ctx+0x40]+0xD8], 0x5ECD80), then × the rev
  limiter ramp +0x118. Non-hybrid (0x473E60, entries 9 and 38): +0x80 =
  T(rec +0x28 · FC · G). SetRPM gets max(HM +0xF8, the GIN's minimum rpm),
  +0xF8 = rec +0x20 + (clamp(rpm, 1000, 10000) − 1000)/9000 · (rec +0x1C −
  rec +0x20). Not ported because the FC targets and the second (decel)
  voice need the open state fields; the clone kept 0.5 + 0.5 · throttle before (see "Engine volume chain" below: now ported).

* **Engine volume chain** (verified unless marked; ported in
  `src/audio/engine_chain/`, replacing the clone's 0.5 + 0.5 · throttle).
  Registry (records {type, name, list head, factory} from 0x7ED230):
  SFXCTL_Physics 0x7ED2F4 (factory 0x470A00 → ctor 0x45D210, vtable
  0x78A008, update slot +0x14 0x484C50), SFXCTL_AccelTrans 0x7ED2D4
  (0x46FFA0 → 0x45C9C0, vtable 0x789FB8, update 0x4831A0, attach 0x46FFE0:
  type 3 → +0x70, 5 → +0x6C), SFXCTL_HybridMotor 0x7ED2E4 (0x4700F0),
  SFXCTL_MasterVol 0x7ED334, SFXCTL_3DCarPos 0x7ED368, CARSFX_SparkChatter
  0x7ED3C8 (0x47CC40), CARSFX_RoadNoise 0x7ED3DC (0x47CD00), CARSFX_WindNoise
  0x7ED410 (0x47D430), CARSFX_WindWeather 0x7ED420 (0x47D500),
  CARSFX_BottomOut 0x7ED568 (0x47E9D0), CARSFX_TrafficEngine 0x7ED524
  (0x47E5B0), CARSFX_TrafficHorn 0x7ED544 (0x47E6A0), CARSFX_TrafficWoosh
  0x7ED554 (0x47E870), CARSFX_TrafficBottomOut 0x7ED578 (0x47F2B0).
  - **The sound context** (vtable 0x78BAA8, init 0x479660; +0x74 the racer
    record SetCar 0x4593B0 is given; slot +0x2C 0x4796F0 returns it). Its
    fields come from SFXCTL_Physics 0x484C50 (this +0x44 the car sound
    object, +0x68 the context): +0x44 = the throttle % (car +0x30 →
    +0x20C × 100) moved at most 30 (0x784478) per update toward the input;
    +0x48 = (car +0x440 rpm − idle) / (max − idle) × 9000 + 1000 (idle /
    max = the tuning's +0x1F0 / +0x1F8 via 0x5B5360(1), 0x45D370; 9000 =
    0x78C194; the 0x7FBED8 = 100.0 test always takes the car's rpm); +0x4C
    = the input above 30 % (+0xBC; the context's +0x34 clock is stored at
    the edges, +0xC4 / +0xC0); +0x50 = the gear (car +0x434); +0x54 = the
    input %. **ctx +0x40 is the Physics controller**: its +0xD8 (the
    "[[ctx+0x40]+0xD8]" input) is set when the game state (0x827E88:
    0x430560 / 0x430420 / 0x430350 with 0x890118 = 2..4, or 0x82A3A4 > 0)
    takes the car out of the player's hands; then 0x46FA30 runs a scripted
    rev instead. All CAR_AUDIO_PATTERN banks, gates and binding are ported
    (2026-11-04, `docs/nis-audio-patterns.md`; DynoExit has no clone route).
    Ordinary driving keeps +0xD8 clear.
  - **AccelTrans** (0x4831A0): +0xD0 = ctx +0x4C, +0xD1 the last; on the
    rising edge 0x45CD30 (speed ≤ 15 mph, ctx +0x54 − Engine's
    [+0x74]+0x90 ≥ 30, Physics +0xD8 clear, state 0, Shifting idle or
    0x45C090, gear 2 outside race type 8 at ≤ 1500 ctx rpm) starts the
    launch 0x45CC30 (state 2, ramps from the tuning record +0xD4), else
    0x470030 (the same throttle jump, state 0, shifting idle, ctx rpm ≤
    0x5ECD40(gear) − 250 and ≥ 3000, 2 s since the last, a racer or gear >
    2) the tip-in 0x45CB70 (state 1); on the falling edge 0x45CDC0 (state 0,
    ctx rpm ≥ 6000, gear > 2) sets +0xD8. 0x47C290 steps the ramps
    (interpolators {elapsed, ms, from, to, shape, value, done} at +0x74
    (rpm), +0x90, +0xAC (the throttle: +0xC0 is "AccelTrans.+0xC0"), stepped
    by 0x46B100: shape 0 linear, 1 hold, 2 t², 3 1 − (t − 1)², 6 a curve via
    0x468160) and ends the states when the rpm ramp is done (2 → 3 → 0;
    off throttle → 4). Ported with the SFXCTL_Shifting blip
    machine 0x482D50 (see "Engine transients"). 0x5ECD80(car) = car +0x28 = 1 and
    [[car+0x34]+0x4C]+0x48 ≠ 0: the human car changing gear (**verified**
    under Wine; the drivetrain's `pending`); 0x470540 / 0x4701A0 then
    take the Shifting table even in state 0 (rec84 lerp / fixed gains).
  - **SFXCTL_Engine** (0x47C010) outside shifts / launches: +0x128 = ctx
    +0x48, +0x130 = the 3-frame mean of ctx +0x44, +0xE0 = 0x7FFF. Rev
    limiter 0x46F610 (skipped when the car sound object's +0x14 is 2):
    with Shifting mode 0 and +0x128 > 9800 (0x788600) it sets +0xE4 and
    ramps +0x118 from its value to 0.15 over trunc(450 (0x7885EC) · value ·
    s) ms and +0xFC to 0.85 over trunc((1 − value) · 120 · s) ms, s =
    0x7F0050's map of the gear (1 → 1.0, 5 → 5.0, clamped: static init
    0x77C380 sets x1 = 5.0); below 9800 (or a shift mode other than 0) it
    ramps +0x118 back to 1.0 over trunc((1 − v) · 50) ms and +0xFC to 0 over
    trunc(50 · v) ms. Ported.
  - **HybridMotor** as in docs/engine-volume-chain.md, ported with the
    states above: D0 the 4-tap mean (0x579F30, mean of the taps filled) of
    the per-update ctx rpm change; on throttle the rec80 lerp by t1, off it
    the rec84 lerp by t2 (hybrid), ±0.2 per update; the trapezoid; +0x80 /
    +0x84 to the accel / decel GIN voices (bytes clamp(((x · level0) >>
    15) >> 8, 0, 127)), both at max(+0xF8, the accel GIN's minimum rpm).
    Tables `hybrid::ENGINES` (43 × 0x4C at 0x7F0DE0) and `DECELS` (37 ×
    0x34 at 0x7F1AA8) are read from the exe. At steady rpm the accel GIN
    gets B = rec80 +0x3C (0.15 for entry 0): its level rises with the rpm
    rise rate; the steady sound is the abk layer (+0x7C, the +0xB0 block),
    which **is** created on PC and is ported (second pass): 0x47D920
    starts event "CAR" (id record 0x7EDE18 = {name 0x788300 "CAR", hash};
    slot 0x82C538, bound by 0x735840 at 0x458110 and rebound / retried at
    0x47DA6A) of the engine record's EE bank (rec80 +0x00; 0x465C80 takes
    +0x04, the SPU bank, only when [0x850078] = 4), block 0x6C bytes: +4
    the id (rec80 +8, 0..60), +0x34 = ([0x850078] ≠ 4), the rest 0. The
    per-frame block is `HybridMotor::abk_block`. Probed graph roles (CAR_00):
    p3 (throttle · 10.24) switches between the on-throttle loops (samples
    5, 6) and the off-throttle ones (3, 2); p4 / p5 attenuate them; p25
    (+0x68 = HM +0xB8 × Engine +0xFC) plays sample 8, the rev-limiter
    layer, silent below the limiter; p13 + 0x7FFF is the second volume
    (kind 8); p21 goes out as kind 6 (not mixed). The CAR modules use no
    node 38 / 39: those appear only in the SWTN_CAR_xx shift-sweetener
    banks (CAR_SWTN, still not ported). The tunnel / master globals
    0x82B070 / 0x82B880 / 0x82A998 / 0x82B078 are written by 0x4717xx
    (SFXCTL_Tunnel / MasterVol: lerps of float targets by +0xCC, × 32767);
    the clone takes 0x7FFF (**inferred**: outside tunnels).
* **Pan law** (ported in `audio::mixer::pan_weights`): every voice
  goes to the two channels with the N_t = 2 table weights at its azimuth
  row (see "Mixer channel law"); at azimuth 0 each channel gets 180 / 255;
  a mono device gets 127 / 255 (N_t = 1). The azimuths and the distance
  attenuation come from the mix map (see "Mix map";
  channel 0 = the right speaker, **inferred**).
* **Skids, the wheel gates**: the wheel gates' car field. SFXCTL_Wheel reads car
  (+0x44 → +0x30) +0x3D4, +0x3D8, +0x3DC, +0x3E0 (0x46EA82–0x46EA9B).
  +0x3D4 is only zeroed (0x5A93E7) and copied by the replay snapshot
  (0x59F6E1); +0x3D8 is an integer (0x5A8EA1, from 0x59E440), a denormal
  read as a float; +0x3DC = min(max(|wheel 0 +0x118|, |wheel 3 +0x118|) ×
  1/6, 1) and +0x3E0 = the same for wheels 1, 2 (0x5A5A6A–0x5A5B11 in
  0x5A5550; the movers' +0x118 is the longitudinal slip r·ω − v, 6.0 at
  0x7A1A2C). So I1 (+0x3D8 / +0x3DC) is the left wheels' spin and I2
  (+0x3D4 / +0x3E0) the right's. Ported. Event count: the racer record
  (ctx +0x74, slot +0x2C) → +4 is the owner kind (2 AI racer, 3 traffic;
  1 taken as the human player, **inferred**), so the player has 2 events.
* **Skids: the car's slip pair** (verified in the decompiled code; observed
  under Wine on all four cars of the first drag and circuit). Car +0x3B4 +
  8i / +0x3B8 + 8i (FL, FR, RR, RL) are written by the physics mover's
  export 0x5A5550 from each wheel's +0x100 / +0x104 (physics view +0x374 +
  8i; the global (0, 0) at 0x86B390 instead while mover +0x84 is set, the
  hold 0x599210), and by the MeloMvr / NIS write-back 0x5A2B70 from the
  chassis' +0x90 + 8i. The tyre pass 0x5A8290 (all four wheels, from the
  rolling constraint 0x5ABE00) sets +0x118 = radius × spin − the hub's
  speed along the wheel, then +0x104 = the speed across it and +0x100 =
  +0x118 in its sliding branch (|+0x118| over 0.5 m/s 0x7FC704, the travel
  angle over the tyre record's +0, or +0x10C locked; only with a nonzero
  slip length and travel), 0 in its gripping branch (0x5A86A1). So car
  +0x3B4 is 0 while a tyre grips (read 0.00 where the wheel turned 0.33
  m/s fast) and the slip while it slides; ported as
  `WheelState::exported_slip`. Its readers: SFXCTL_Wheel's sums (0x46EA50),
  the burnout score (0x5F7420: `lea ecx,[esi+0x3B4]` at 0x5F7755, the
  driven wheels' mean), the Burnout replay camera's weight (0x441D30: car
  +0x3B4 + +0x3BC + +0x3C4 + +0x3CC at 0x441D82) and the AI's nitrous test
  (0x402660: car +0x40 → +0x374 + 8i at 0x40267D, the driven wheels' mean
  |slip|) — all three read the export in the clone too
  (`sim::drive::style::driven_wheel_slip`, `sim::replay::camera::exported_slip_sum`,
  `CarState::ai_driven_slip`); the wheel gates' +0x3DC / +0x3E0 read the
  wheels' +0x118 instead. +0x104 is the hub's speed across the wheel in
  both branches (0x5A8606, 0x5A86AB). **Open**: the AI's slip tracker
  0x4057C0 reads the car's pair (+0x3B4 / +0x3B8 + 8i from 0x4057E4) where
  the clone reads the wheels' own slip; for a racer on the MeloMvr the pair
  comes from the chassis' +0x90 + 8i (0x5A2B70), whose lateral half the
  clone does not model.

* **Engine transients** (verified unless marked; ported in
  `src/audio/car/transients/`, driven by `hybrid::HybridMotor::update`).
  - **Interpolator** 0x459990(from, to, ms, shape) / 0x46B100(dt):
    {elapsed, duration s (ms · 0.001; ≤ 0 → 0.01 s), from, to, shape,
    value, done}; done when elapsed > duration (strict). Shapes (table
    0x46B21C): 1 hold, 2 t², 3 1 − (t − 1)², 6 the curve
    0x468160(2, 0x7FFF − clamp(trunc(t · 32767 (0x789CF4)), 0, 0x7FFF), 0)
    × 1/32767 (0x78A6C8), else linear. 0x468160(0, p): the fade table
    0x7EF7F8 at p >> 6 plus ((next − it) · (((p & 0x1E) << 9) | 0x3FF))
    >> 15 when it is not 0 (only bits 1–4 of p, as coded); type 2 squares
    it (>> 15). A tracked ramp (0x47C1A0, 0x47BF20, 0x47BEA0, 0x47AAA0)
    writes `to` first and, once done, the value = the target.
  - **SFXCTL_Physics fields** (0x484C50): +0x70 = the context rpm, +0x84
    the slewed throttle (ctx +0x44; slot 0x24 0x45D300 returns it), +0x8C
    the input %, +0x90 last update's, +0xC8 the gear (car +0x434), +0xCC
    last update's, +0xBC on throttle (input > 30). Engine +0x74 is the
    Physics controller (attach 0x46F3E0: type 0 → +0x74, 3 → +0x6C, 4 →
    +0x70, 8 → +0x78).
  - **AccelTrans** (vtable 0x789FB8; bind 0x45CA80: the launch record
    +0xD4 = 0x7F3248 + 20 · engine record +0x18 for the player (owner
    kind 1), else 0x7F3284 or 0x7F32C0 at random; `transients::LAUNCHES`,
    `LAUNCH_INDEX`). Records {f32 top of the second ramp, rpm added,
    launch ms, settle ms, lift ms}. Update 0x4831A0: +0xD1 = +0xD0, +0xD0
    = ctx +0x4C, +0xD8 = 0; on the rising edge the launch test 0x45CD30
    (mph ≤ 15, input − Physics +0x90 ≥ 30, Physics +0xD8 clear, state 0,
    Shifting state 0 (0x45C090), ctx gear 2 outside race type 8 (the
    dyno), ctx rpm ≤ 1500) starts 0x45CC30: state 2, rpm ramp ctx rpm →
    ctx rpm + rec +4 over rec +8 ms shape 6, ramp +0x90 0 → rec +0 over
    rec +8 ms, throttle ramp held at ctx +0x44; else the tip-in test
    0x470030 (the same jump, state 0, Shifting 0, ctx rpm ≤
    0x5ECD40(gear) − 250 (drivetrain +0x84 + 4 · gear via 0x5947C0, a real
    rpm against the context's 1000..10000 one, as coded) and ≥ 3000, 2 s
    (clock +0x5C − +0xCC) since the last, owner kind 2 or gear > 2)
    starts 0x45CB70: state 1, rpm ramp ctx rpm + 1000 → ctx rpm over 0.5
    s shape 6, +0x90 0.8 → 0 over 0.2 s, the throttle ramp at 100 for
    0.01 s. Off throttle after on: 0x45CDC0 (state 0, Shifting 0, ctx rpm
    ≥ 6000, gear > 2) sets +0xD8 (no sound reader found). 0x47C290 steps
    +0x90, then 0x47C1A0 (states 1 / 3 / 4 track ctx rpm, 2 plain) and
    0x47C220 (states 1–4 track ctx +0x44, re-armed when done); then with a
    state set and off throttle (state ≠ 4): rpm ramp Engine rpm → ctx rpm
    over rec +0x10 ms shape 6, state 4, +0x90 → 0 likewise; states 1 / 3
    / 4 end when the rpm ramp is done, 2 then goes to 3 with ramps to ctx
    rpm / 0 over rec +0xC ms. The +0x90 ramp's value (+0xA4) has no reader
    in the sound code (searched).
  - **Shifting** (vtable 0x789F28; bind 0x45C330: +0x74 the shift
    record, +0x64 the transmission level ctx +0xA4). Update 0x483130:
    clock, flags +0x70 / +0x71 / +0x72 cleared; Physics gear above the
    last → 0x46EC70, below → 0x46EE60; then 0x482D50. The 0x60-byte record
    (0x7F2860, `transients::SHIFT_RECS`): +8 up clunk delay s, +0xC / +0x10
    up rpm drop / ms, +0x14 / +0x18 engage rpm add / ms, +0x1C / +0x20 the
    volume bump ms / size (f32), +0x24 / +0x28 / +0x2C the volume wobble
    amplitude / period / decay ms, +0x30 / +0x34 / +0x38 the rpm wobble's,
    +0x40 down delay, +0x44 / +0x48 down rpm drop / ms, +0x4C / +0x50 the
    blip's rpm add / ms, +0x54 / +0x58 its fall / ms, +0x5C the settle ms
    per rpm.
    0x46EC70 (up): wobble and state cleared; if Physics rpm ≥ 3000:
    state 1, +0xA8 = Engine +0x12C and Engine +0x128 ↔ +0x12C swapped (the
    rpm goes back to the last update's), +0xA4 the clock, throttle ramp
    +0xAC Physics throttle → 0 over 0.1 s, rpm ramp +0xC8 +0xA8 → +0xA8 −
    rec +0xC over rec +0x10 ms, bump ramp +0xE4 zero (0.001 s), ramp +0x140
    +0xA8 → +0xA8 − 1200 over rec +0x10 + rec +0x18 ms shape 6, clunk armed
    (+0x98, +0x9C = 1 up, +0xA0 = clock + rec +8), +0x71 = 1.
    0x46EE60 (down, not into gear 1 = neutral): the same start in state 4
    with the throttle over 0.05 s and the rpm ramp +0xA8 → clamp(+0xA8 −
    rec +0x44, 1000, 10000) over rec +0x48 ms; clunk at clock + rec +0x40.
    0x482D50: the clunk flag +0x70 when the clock passes +0xA0; +0x7C = 0;
    with a state: 0x47BF20 (ramp +0x140 tracks Physics rpm; the rpm ramp
    tracks it in states 2 / 3 / 7), 0x47BEA0 (the throttle likewise with
    the Physics throttle), the bump ramp, 0x46F000; then (jump table
    0x483114) 1 → 2 when the rpm ramp is done: rpm ramp clamp(rec +0x14 +
    Physics rpm, 1000, 10000) → Physics rpm over rec +0x18 ms, throttle
    ramp Physics throttle × 0x7885A0[clamp(gear − 2, 0, 3)] (all 1.0) →
    it over 0x7885B0[…] ms (all 0), bump rec +0x20 → 0 over rec +0x1C ms,
    0x46F080, 0x46F000, +0x72 = 1; 2 → 3 when done; 3 ends (0x45C430)
    once both wobble decays are done or the wobble is off; 4 → 5: rpm ramp
    Engine rpm → clamp(Physics rpm + rec +0x4C, 1000, 10000) over rec +0x50
    ms (× 0.7, 0x7844D8, below gear 3) shape 6, throttle Engine +0x130 →
    100 (0x7ED29C) over 215 ms (0x7ED2A0); 5 → 6: Engine rpm → Engine rpm
    − rec +0x54 over rec +0x58 ms (× 0.7 below gear 3 at level ≤ 1),
    throttle 100 → 0 over 215 ms; 6 → 7: Engine rpm → Physics rpm over
    trunc(clamp(|Engine rpm − Physics rpm| · rec +0x5C, 0, 800)) ms shape
    6, throttle 0 → Physics throttle over 60 ms; 7 ends when done.
    0x46F080 (below gear 5): wobble on (+0x13C), rpm decay +0x104 from k ·
    rec +0x30 (k 0.85 in gear 3, 0.7 in gear 4, else 1) → 0 over rec +0x38
    ms, volume decay +0x120 rec +0x24 → 0 over rec +0x2C ms. 0x46F000:
    steps both; the rpm one done ends the shift; else +0x90 =
    trunc(+0x118), +0x94 = rec +0x34, +0x80 = trunc(+0x134), +0x84 = rec
    +0x28. 0x45C430 clears the wobble and the state.
  - **SFXCTL_Engine with the transients**: the idle flag 0x46F9D0 (+0x13C:
    owner kind 1, racer +0x900 clear (**inferred** clear), AccelTrans state
    0, Engine rpm ≤ 2500); the wobble slot 0x3C 0x45C7A0 (in a shift: each
    of the rpm (+0x90 / +0x94 → +0x144, phase +0x14C), throttle (+0x88 /
    +0x8C → +0x148, never set) and volume (+0x80 / +0x84 → +0x140, phase
    +0x150) oscillators with a non-zero amplitude steps its u16 phase by
    trunc(dt / (clamp(period, 1, 10000) · 0.001) · −65535) subtracted,
    mod 65535, and outputs Sin16 (0x43C2B0) × the amplitude; outside
    shifts all 0 and the phases 0x4097); the rpm slot 0x24 0x46F7D0 (the
    Shifting rpm ramp's value in a shift, else AccelTrans's in a transient,
    else ctx rpm; with the idle flag outside shifts ctx rpm clamped to
    [rpm − 60 (0x7885CC), rpm + 999 (0x78AAB0)]; +0x12C = +0x128, +0x128 =
    that + 2 · +0x144 + +0x1C8 (no writer in the class: 0)); the throttle
    0x45C710 (reset to the Shifting's or AccelTrans's throttle ramp, else
    ctx +0x44 pushed); the volume 0x45C6C0 (+0xE0 = 0x7FFF, in a shift
    0x7FFF − trunc(bump · −32767 (0x789FB0)), then + trunc(+0x140)); the
    rev limiter only starts outside shifts. HybridMotor's D0 filter
    (0x45D0A0) and gain targets (0x470540 / 0x4701A0) by these states are
    as in docs/engine-volume-chain.md (re-checked: in a shift 2 rec80
    snapped, 3 / 5 rec80, 7 rec80 snapped on throttle, 1 / 4 / 6 rec84;
    AccelTrans 1 rec80 snapped, 2–4 rec80). **Inferred**: the controllers
    run Physics, Shifting, AccelTrans, Engine, HybridMotor in that order.
    The first update sees gear 1 and input 0 as the last (the object is
    built with +0xC8 = 1, +0x8C = 0; **verified** under Wine). Not ported: 0x46F580
    (an envelope armed by HM slot 6 through Engine +0x152; Engine +0x134),
    Physics +0xD8's scripted rev 0x46FA30 outside the now-ported
    ShopEnter1/ShopExit1 patterns 3/4 with retained car-context state across Explore (see `docs/evidence/nis-audio-pattern` and `docs/evidence/nis-audio-pattern4`).
  - **CARSFX_Shift** plays FX_SHIFTING_01 on the Shifting's +0x70 (the
    clone's shift sound now comes from this machine); 0x45E730 reads ctx
    +0x48 (the context rpm) with `_ftol` (truncation; the clone rounded
    the car's rpm before). CARSFX_Turbo's rpm is ctx +0x48 and its boost
    Engine +0x130 (slot 0x38), shifting = Shifting state ≠ 0.
* **Sweeteners and the decel crackle** (verified unless marked;
  `carfx::PlayerSfx::swtn_update`). CARSFX_Shift's bank slot (0x465990)
  loads a second bank when the record +0xD4 (0x7F3040 + 12 · ctx +0x7C,
  record 0 past 0x2B; `carfx::SWTN`) names one: sound\Engine\ (0x7ED164)
  + SWTN_CAR_xx_MB.abk (entries 41 / 42 use 14 / 12). Each holds two
  events, CAR_SWTN and CAR_Sputter, and an entry "CAR_SputOutput" naming
  CAR_Sputter's node-38 block. The game binds the slots at 0x458100
  (0x82C598 "Sputter_Message", 0x82C538 "CAR", 0x82C680 "CAR_SWTN",
  0x82C568 "CAR_Sputter", 0x82C648 "CAR_SputOutput"; names from
  EVT_SYS/UG2_ENGINES_AEMS2.csi, records 0x7EDE10..0x7EDE30).
  - CAR_SWTN (0x4720D0 on +0x71, rpm ctx +0x48; 0x4722B0 on +0x72, rpm
    Shifting +0xA8; both only when 0x7FAAA8 (1) and rpm ≥ 7000 (0x78ABF8)):
    block {variant 0 / 1, clamp(rec80 +8, 0, 60), clamp(trunc(rpm), 0,
    10000), clamp(((trunc(ramp) · record +4) >> 15) · level >> 15, 0,
    0x8013), the azimuth 0..0x10000, 0, 0}; the ramp 0x7F095C (0x4599E0:
    {26000, 32767, 4000, 7000, in, out}) gives 32767 at ≥ 7000; level =
    slot 0x38 index 3 / 2.
  - CARSFX_SparkChatter (registry 0x7ED3C8, factory 0x47CC40 when
    0x43E910 ≥ 0x500, ctor 0x472C10, vtable 0x78ACB0; attach 0x472C80:
    type 3 → +0xB0, 8 → +0xFC; bind 0x45F6E0: +0x9C ctx for owner kinds 1
    / 2, +0xBC the 0x7F3040 record) is the engine's decel crackle, not a
    scrape: its start (slot 0x1C, 0x485130) adds the listener 0x47CCD0 to
    CAR_SputOutput (0x735900) and starts CAR_Sputter with {clamp(rec80
    +8, 0, 60), the object, 0 …} (11 parameters); per frame (0x45F800):
    p2 = clamp(trunc(ctx +0x48), 0, 10000), p3 = clamp((level[0] · record
    +8) >> 15, 0, 0x8013), p4 the azimuth, p5 = p6 = 0, p7 =
    clamp(trunc(ctx +0x44 · 10.24 (0x78A1C8)), 0, 0x400), p10 = Shifting
    state ≠ 0, dispatched (0x735580); 0x45F780 toggles p8 from ctx +0xF0
    in gears 1–2 (its writer not traced: 0 in the clone); stop 0x472CF0.
    The graph pops (samples 0–9, random pitch) off the throttle at high
    rpm and is silent with p7 = 1024 (probed: `carfx` test
    `sputter_pops_on_a_lift`). The listener 0x47CCD0 takes CAR_SputOutput
    messages whose third parameter is the object and hooks 0x472CD0 (the
    pops' output to the game, e.g. exhaust flames: **inferred**; no
    sound).
  - **AEMS node 38** (0x7200A2): block +0 / +4 a named output bound by
    the bank's event entry (type 1 → 0x7353F0 → 0x7351B0 name lookup), +8
    the open message, byte +0xC ranged, byte +0xD n, n {min, max} when
    ranged, then {start, stop, n parameters}. Stop: release (0x735550),
    +8 = 0. Start with none open: clamp, open (0x735490: an object with
    count 1, every listener of the output called, 0x734DB0). Neither, with
    one open: clamp and send (0x735080). Returns the open message's count
    (0x735070) or 0. The instance's end (0x71FFF6) releases the open ones.
    **Node 39** (0x7201C2): on a change of +0x14 (last +0x10), the value
    clamp(+0x14, +8, +0xC) goes to the named value (0x7356D0); returns 0
    (no installed bank uses it). Both ported (`aems::Instance::messages`
    keeps what they send; nothing in the clone listens).
* **Mix map** (verified unless marked; ported in
  `src/audio/mixmap/`, test `audio::mixmap::tests`). Replaces the earlier
  note (load only).
  - **Load.** 0x4869A0 picks 0x7EDE4C[k]: k = 2 (DRG) with the drag flags
    0x89E7D8 / 0x890108, 3 (URL) with 0x89E7E1, 1 (DFT) with the drift
    flags 0x89E7D9 / 0x890104, else 0 (MAPOUTPUT); 0x486920 allocates the
    mix object (vtable 0x78A600: 0x477DD0, 0x4678C0 pools, 0x477C70 parse,
    0x478FE0 update) and loads the file asynchronously (0x57B4B0, callback
    0x486500 → 0x485FA0). 0x477C70 → 0x467610 sizes the pools (names
    0x78A6A8.. "AUD: NFSMixCtl / NFSMixDataProc / NFS3DMixCtl / SubChannel /
    Master Channel / MixMapProc / NFSEvtMixCtl Pool"). 0x485FA0 takes the
    nine object groups' instance counts ([0x82B884] +0xA8 + 4g → +0x18)
    and makes one copy (MixMapProc, vtable 0x78A6BC) per instance per
    section (0x4837C0 → 0x477D60, 0x477F80 controls, 0x477EB0 3D,
    0x477DF0 event controls, 0x480270 subs, 0x480320 masters), then binds
    (0x483900 → 0x478120 / 0x480420, 0x485E90 → 0x4809E0 / 0x483960 /
    0x485340 / 0x480860 → 0x478240 "Connect Mix Channel").
  - **File.** +4 section count (9), +8 the offset of their offsets. Section
    s = object group s (0 global: music / speech / FE …, 1 the player's
    car, 2 AI racers, 3 traffic, 5 collisions, 6 wooshes, 7 planes /
    environment, 8 trains); its header: +4 controls, +8 3D controls, +0xC
    sub channels, +0x10 master channels, +0x14 the masters' output maps,
    +0x18 event controls (offsets from the section, −1 none); each block a
    count and 12 bytes, then entries. Control: {id, range, input} (12
    bytes); 3D control: {id (byte 3 & 0xF = the variant count), variants of
    6 dwords}; event control: 7 dwords; channel: {id (bits 16–23 = the
    input count), w1, (masters) the object id, inputs}; map: {count (low
    nibble), words}.
  - **Ids.** Bits 28–31 the kind (0 control, 2 master, 3 sub, 4 sound
    object field, 6 controller field, 8 / 9 3D control, A / B event
    control), 16–23 the section / group, 11–15 the instance, 4–10 the
    object class (registry type >> 4 & 0x7F: 0x468F70 → 0x464CD0 matches
    object +0x4C), 0–3 the field (object +4 + 4·f) or 0–7 the node.
  - **Per frame** (0x478FE0 → 0x467E70 per copy): (1) data procs
    (0x477C30): curve((id >> 24) & 0x1F, clamp(input, 0, 0x7FFF))
    (0x468160); (2) controls (0x477BD0): lin_to_db(0x7FFF − ((0x7FFF −
    dp) · K >> 15)) + B, then × input >> 15 (a 3D input converted with
    0x467FF0); B / K from the range word (0x477B60: bit 15 clear: B = w &
    0x7FFF, K = 0x7FFF − lin(−B); set: B = 0, K = 0x7FFF − lin(w as i16));
    the input via 0x480910 (default 0x7EDE80 = 0x7FFF); (3) event controls
    (0x468D40, below); (4) 3D controls (0x468580, below); (5) sub channels
    (inline): 0 + Σ inputs clamped to [w1 low as i16, (w1 >> 16) &
    0x7FFF]; (6) masters (0x467460): −dBUP + Σ inputs clamped to [−dBUP −
    10000, dBUP] (dBUP = w1 >> 16; 0x479030), −10000 when [0x82B884]+0x14
    is set; then 0x477AA0 for each map word {mB offset i16, slot bits
    16–19, block dword 24–27, 3D connection 20–23}: mB = master + offset
    (+ the 3D connection's mB), level = 0 below −10000, 0x7FFF above 0,
    else 0x467FF0(mB); the dword = level | azimuth << 16 (the 3D
    connection's +0x14, else [0x82BC7C]). Masters with the same object id
    share one 16-dword block; the binder 0x480420 points the object's
    +0x58 + 4·slot at it (the slot-0x38 / 0x3C reads 0x45E390 / 0x45E3B0).
  - **Inputs** (0x478240): kind 2 / 3 a master / sub channel's +0x10, 0 a
    control's +0x1C (slot 0x18 0x5A8750), A / B an event control's +0x18
    (slot 0x28 0x4688F0). The own section ("Local") reads the own copy;
    another section ("Ext") adds one input per copy of it. The 8 / 9 words
    of a channel are its 3D connections, in order. Data procs read their
    own id (0x4809E0: 2 a channel of copy 0, 0 a control, 8 a 3D control's
    mB, 4 / 6 a field).
  - **mB ↔ level.** 0x467FF0: 0 → 0x7FFF; q = trunc(mB / −602) (magic
    0xE4C8B7B5), r = mB % 602, table 0x7EE048..0x7EE7EC [601 + r] >> q,
    below 50 → 0. 0x468050: 0 → −10000; e = leading zeros below bit 14
    (16 → −10000), (−1 − e) · 602 + table 0x7EE7F0 [the mantissa as 9
    bits], at least −10000. Both tables are copied from the exe.
  - **Curves** 0x468160(t, x, db), jump table 0x468350: 0 the fade table
    0x7EF7F8 at x >> 6 blended by bits 1–4; 2 its square; 4 the mirrored
    fade (rising); 6 its square; 8 0x7FFF − x; odd t = the even one on
    0x7FFF − x (levels) or negated (mB); > 9 → 0; the mB forms of 2 / 6
    double the base mB and give −10000 below −9600.
  - **3D controls** (0x4683E0 init: mB −10000, the first variant; the
    variant follows the view mode 0x82BB48 by byte 3 & 0xF, 0x4684B0).
    Variant {w0: bits 12–15 the distance (0 the position object's f1, 1
    its f0, else −1.0), 8–11 the azimuth (0 f3, 1 f2, else kept at
    +0x14); w1: curve types in bits 28 / 16 / 24 / 20 for the four
    directions; w2..w5 {lo, hi} metres (15 bits each)}. Quadrant q =
    azimuth >> 14 picks (w2, w3), (w3, w4), (w4, w5) or (w5, w2); beyond
    both his → −10000; else the distance is clamped into each pair,
    normalised (ix in extended precision, iy through a float),
    × 32767 (_ftol) and 0x468380 crossfades curve(t, ix) and (angle in
    the quadrant ≠ 0) curve(t, iy) — both with the first direction's type
    t, as coded — by 2 · angle: lin_to_db.
  - **Position objects** (SFXCTL_3DCarPos 0x47C7A0 → 0x471D60, vtable
    0x78AB68; update 0x471DC0 → slot 0x24 0x45E100 → 0x45DF00): the
    nearer listener (0x82BC14 = 0 with one player); f0 = trunc(distance to
    the player's car, 0x82BEDC), f1 = to the camera (0x82BF10), f2 = the
    azimuth from the car along its heading (0x82C6C4), f3 from the camera
    along its look (0x82BECC); no camera → −1, −1, 0, 0. 0x45DE60: n =
    normalised offset (0x43CE50; zero → (1, 0)), a = 0x4000 −
    asin16(n · dir) (0x43C490, table 0x7EA628), 0xFFFF − a when n.x·dir.y
    − n.y·dir.x < 0. The listener (0x46D7E0): camera = [[car +0x18]
    +0x238] +0x1C: position +0x40, look +0x50; the car +0x60 / +0x90;
    0x82BB48 = 4 in scripted states, 3 for camera kinds 6 / 7, 5 in game
    flow 5, else 1 / 2 for the sound context's view id (+0xA8 =
    0x502500(the player's camera setting 0x83AA74): settings 0..4 → 2, 4,
    1, 0, 5) 2 / 4, else 0. The doppler ratio (0x45E1F0, 343 m/s) is not
    ported.
  - **Event controls** (vtable 0x78A6EC; 0x468900): {id (byte 3 & 0xF the
    mode), V (mB; u16 > 10000 negative), the scale's id (+0x10), the
    trigger's id (+0xC), attack, hold, release (bits 0–11 × 16.67 ms
    (0x78A728), 12–15 the curve)}. 0x468D40(dt): idle while the timer and
    the trigger are 0; the timer += dt · 1000; mode 0 (0x468960) attack
    then release, 1 (0x468A70) attack / hold (envelope 0) / release, 2
    (0x468BD0) attack, sustain while triggered, release on the trigger's
    drop (re-trigger latches); the envelope = V ≥ 0 ? 0x7FFF − curve :
    curve; out = trunc((32767 − env) / 32768 · V) (latched forms with the
    held value), then × scale / 32767 (0x78A6C8).
  - **Fields the clone gives** (`mixmap::Scene`): SFXCTL_Physics (0x470A60,
    slot 6): f0..f3 = |mph| / 30, 60, 100, 140 · 32767, f4 = (10000 − ctx
    rpm) / 9000 · 32767, f5 = fewer than four wheels down, f6 the view
    slew (0x7D00 for view ids 0 / 1 / 5, 0xFA0 for 4, else 0, ±0x3FFF a
    frame), f7 / f8 / f9 0 (cinematic cameras, race progress: not used);
    SFXCTL_Skid f3 (0x47BDF0: clamp(max of the side means << 5)); the
    group-1 class-3 fields (0x45C210: the option volumes × master, all
    32767 at the defaults, f6 = 0); SFXCTL_MasterVol (0x471970: 1 −
    volume · master · fade, 0 at full volume); position objects for the
    car (classes 8, 0xA **inferred** as the car), its right / left wheel
    pairs (0xB / 0xC: SFXCTL_3DRightWheelPos / LeftWheelPos, verified in round 2 of the sound comparison: SFXCTL_Wheel's two points 1.5 m ahead and 0.8 m to each side, 0x46E8B0), AI cars
    (section 2 class 8) and traffic (section 3 class 0). Every other
    field of an existing object is 0 (as for the nitrous, bottom-out and
    traffic fields are given, see "Ducking inputs" below).
  - **Player slots** (MAPOUTPUT, defaults, 5 m behind the car, idle):
    engine 23064; shift 23091 / 23091 / 18342 ×4; turbo 4879 ×3; nitrous
    9471 / 5027; SparkChatter 18342; skids 7742 / 8202 / 10937 per side;
    the sound objects now read these instead of 0x7FFF (`carfx::CarInput
    ::mix`, the engine's level[0]), and their azimuths (slot 0 except the
    purge 1, the skids 3 / 1) go into the event blocks and the engine's
    pan (0x724093).
  - **Pan side** (**inferred**): with the azimuth 0x4000 on the listener's
    right (the cross product above in the right-handed z-up world), the
    90° speaker, channel 0, is the right one; `mixer` now sends it to the
    device's channel 1.

**Road, wind, bottom-out, AI and traffic sounds** (verified
unless marked; ported in `src/audio/world/env/`, `traffic.rs`,
`stitch.rs`; render probe the maintainer's probe `car_audio_render`; self-test "road
/ wind voices"). Vtable slots of the sound objects: +0x10 attach, +0x14
bind, +0x18 bank, +0x1C start, +0x20 stop, +0x24 update(dt), +0x28 per
frame, +0x38 / +0x3C the mix map's level / azimuth (0x45E390 / 0x45E3B0),
+0x44 / +0x48 (engines) start / per frame. All the levels and azimuths
below are the mix map's slots ("Mix map"), object classes 6
RoadNoise, 7 WindNoise, 10 WindWeather, 11 BottomOut (section 1), 0 the
AI engine (section 2), 0 TrafficEngine, 1 TrafficWoosh (section 3).

* **Ramps and graphs.** 0x4599E0 (record {y0, y1, x0, x1, in, out}): out =
  y0 + (y1 − y0) · min(f, 1) for f = (in − x0) / (x1 − x0) > 0, else y0.
  Graph_Eval 0x5767A0: piecewise linear through (x, y) points, clamped.
* **RoadNoise** (vtable 0x78ACF8; attach 0x472E10: SFXCTL_Wheel → +0xB0;
  bind 0x463500; bank 0x4658F0: `ROADNOISE_00_MB.abk`, events
  FX_ROADNOISE (7 sounds) and FX_ROADNOISE_TRANS). Start 0x472E50: two
  FX_ROADNOISE events +0xB8 / +0xBC (slot 0x82C540, id record 0x7EDD50)
  with {4, 0 …}; +0xB4 = the owner kind (context slot +0x2C → +4).
  Update 0x47CD90 → 0x4735E0 (owner kind 1 only; otherwise +0x100 = +0xFC
  = +0x108 = 0, +0x104 = trunc(0)): mph = car +0x430 × 2.237; B =
  trunc(Graph(0x82C69C, mph)) × 0x7FFF >> 15 with the points 0x7F09B0
  (0, 0), (60, 28000), (100, 32500), (150, 24500), (175, 18000) (static
  init 0x77C460); r(v, k) = min(|v| · 0.01, k) for the Wheel slip sums
  (+0x90/+0x94 right, +0x98/+0x9C left); +0xFC (right) = 1.1 · (1 + r(R,
  0.15)) · B, +0x100 (left) = (1 + r(L, 0.15)) · (1 + min((WAC + WA0) ·
  0.5 · 0.1, 0.1)) · B, each at most 32000 (0x78ADB4) (the right one's
  "spin" term is the constant 0x78882C = 0.1, as coded); O = ramp
  0x7F09D8 {1500, 4500, 0, 100}(mph) (0x788818 / 0x78881C, static init
  0x77C480); +0x108 (left) = O · (1 + r(L, 0.2)) · (1 + min((WAC + WA0) ·
  0.075, 0.15)), +0x104 (right) = O · (1 + r(R, 0.2)) · (1 + min((WA8 +
  WA4) · 0.075, 0.15)), each at most 6000 (0x789FE0); all truncated.
  Then 0x45FAA0: +0xE0 / +0xE4 = the last surfaces, +0xE8 / +0xE9 =
  changed, +0xD8 / +0xDC = Wheel +0xB0 (wheels 0, 3) / +0xB4 (1, 2).
  Per frame 0x47CDB0, left side first: on a change the class 0x45F940
  (surfaces 1, 3 → 5; 2, 13 → 4; 7, 26 → 7; 8, 10, 15 → 0; 11, 12, 17 →
  2; 14 → 3; 16 → 1; 25 → 6; else −1) → +0xF8 (left) / +0xF4 (right);
  −1 releases the side's surface event (+0xC4 left, +0xC0 right), else
  0x45FB00 replaces it: FX_ROADNOISE {class (0..15), 0, 0x1000, 0 …}
  ("Left Road Noise" / "Right Road Noise"). With the side's wheels down
  (Wheel +0xE8 / +0xE9) a transition 0x4731C0: variant 1 when the last
  surface was 11 or 16, else 0x45F9D0(surface) (11, 16 → 0; 14 → 2; else
  none). Then the blocks (dispatched with 0x735580): surface events p1 =
  clamp(level × ((T[class] × side volume) >> 15) >> 15) with T 0x7ECF4C =
  {15000, 24000, 32000, 24000, 24000, 5000, 19000, 32500}, 0 with the
  side's wheels up, p2 = clamp(+0x108 left / +0x104 right, 0, 0x1FFF) (0
  under the pause flags [0x82B884] +0x14, or +0x13 with +0x18 + 1 <
  +0x1C), p3 the azimuth, p5 = 1 on surfaces 8 and 9; the base events p1
  = level × ((+0x100 × T[4]) >> 15) >> 15 (both with the left volume, as
  coded), p2 as their side, p3. Slots: left surface 1, right 0; +0xBC 5,
  +0xB8 4; transitions 3 (left) / 2 (right); thumps 7 / 6.
  Transition 0x4731C0(variant, side): base {variants 0, 1: 12000; 2, 3:
  0x5FB4; else 0x7FFF} × trunc(ramp 0x7F0978 {20000, 32767, 12, 60}(mph))
  >> 15 → +0xF0 / +0xEC; FX_ROADNOISE_TRANS (slot 0x82C6B4, record
  0x7EDD58; releasing +0xC8 / +0xCC) {variant (0..15), clamp(level × that
  >> 15), clamp(trunc(ramp 0x7F0994 {3600, 4400, 12, 60}(mph)), 0,
  0x1FFF), azimuth, 1, 0, 0}; +4 := 1; and with no thump running a
  stitch object ("AUD:RoadTransitions") of collision record 0x4E +
  (counter & 3) (counters 0x82CB44 right, 0x82CB48 left), updated each
  frame (0x47BB30) with the thump slot's level and azimuth and dropped
  when its layers end.
* **WindNoise** (vtable 0x78ADB8; attach 0x45FDF0: types 13 / 14 →
  +0x1A8 / +0x1AC; bind 0x45FE30; bank 0x465F60 `WIND_00_MB.abk`). Start
  0x45FE80: FX_WIND (slot 0x82C530, record 0x7EDD70) {0, 0, 0x1000, 0 …}.
  Update 0x45FFB0: v = car +0x430, below 2 → 2, above 40 → 39.95; t = v /
  40; +0x19C = trunc(1040 t), +0x190 = trunc(32767 t); the wind position
  objects (+0x120 / +0x130 → the attached controllers' slot +0x2C) at the
  car's position + R · (its heading +0x90/+0x94 rotated by ±(1280 +
  19200 t), 16-bit angle, Cos16 / Sin16), R = 65 (1 − t), at least 1.01 ×
  context +0x70 (its writer not traced). No installed mix map reads classes
  13 / 14 (the field lists of all nine sections), so the positions are not
  modelled. Per frame 0x473970: p1 / p4 = clamp(0x7FFF ([0x7ED3EC]) ×
  ((level[0 / 1] × +0x190) >> 15) >> 15), p2 = 0x1000 (0 paused), p3 /
  p5 = the azimuths [0 / 1], p6 = clamp(+0x19C, 0, 0x410).
* **WindWeather** (vtable 0x78AE00; bank 0x4664C0 WIND_00). Update
  0x473A10 (outside game flow state 1): start (slot +0x44, 0x460680) when
  none: FX_WIND_Weather (slot 0x82C4F8, record 0x7EDD78) {0, 0, 0, 0};
  +0xB4 = ramp 0x7F09F4 {1.0, 0.25, 0, 70}(mph) (x1 0x78885C, static init
  0x77C4D0). Per frame 0x460770: p0 = clamp(trunc(level[0] × +0xB4)), p2
  = 4000, p3 = 0. **Inferred**: created for the player like the other
  0x1xxxx objects (the creation list is not traced).
* **BottomOut** (vtable 0x78B4E8; update 0x47ED80(dt), per frame
  0x47F100). Wheel pairs A (0, 1), B (2, 3), C (1, 2), D (0, 3) (car
  +0x390..+0x393) with "was down" flags +0xBC / +0xC4 / +0xCC / +0xD4 and
  timers +0xC0 / +0xC8 / +0xD0 / +0xD8: a pair not down clears its flag
  and sets its timer to the car object's clock (+0x44 → +0x34) on the
  first frame, then adds dt each frame. Tipped +0xDC: set when (car +0xB4
  + +0xB0) · 0 + +0xB8 (the up vector's z) < 0.9 (0x7ED564) and
  0x5EC200 < 1 (a wheel up); cleared when all four are down. A or B
  landing (down, flag clear, timer > 0.05 (0x78888C)) untipped plays the
  knock 0x47EA30(v, 0); tipped, any pair landing plays 0x47EA30(v, 1). v =
  Σ ramp 0x7F0B5C {0.4, 1, 0.4, 1}(|car +0x34 → +0x28 + 4i → +0x1C / tuning
  +0x184 + 0x20i|) × 31.75 (0x78BC78) when car +0x28 = 1, else 127. The
  knock (0x47EA30): the old one (+0xB4) released; v clamped to 0..127;
  collision record 0x4E + (counter & 3) − 4 · trunc(v · −1/64) (counter
  0x82CBF4; 0x78B06C) or, flag 1, 0x56 + (counter & 3) − 4 · trunc(v ·
  −1/128) (0x82CBF8; 0x78453C); a stitch object started with a zero
  block; +4 := 1. A second knock (+0xB8, record 0x2E + … by
  Rand_Float(127)) when 0x459AC0(car position) +0x58 is 6 — the nearest
  camera director's event kind, the crash / jump camera (ported, see
  "Second bottom-out knock" below); not ported: 0x47EC60 (the context's
  +0xD8 re-trigger, +8). Per frame
  0x47F100: +4 = +8 = 0, the knocks updated with slot 0 (+0xB4, +0xB0)
  and 2 (+0xB8) and dropped when they end.
* **Stitch objects** (0x45B1F0, vtable 0x789EC0, "AUD:RoadTransitions",
  "AUD:TrafficWsh_Stch"): record = 0x43BE60 on the manager's +0xD0 list
  (0x46CD60(1, i) the whoosh list); start 0x46CDD0 and update 0x47BB30
  copy a block {+0, volume, pitch offset p, azimuth, +0x10} and scale the
  volume by the record's gain table at +6 (0x7F34A0 list 0, 0x7F36E8 list
  1: all 26028); a layer (0x45B260 / 0x45B280) starts by its bank byte
  +1: 0 "AEMS_StichCollision" (slot 0x82C5E0, record 0x7EDDF8), 1
  "AEMS_StichWoosh" (0x82C5D0, 0x7EDE00), volume (+2 − rand(+0xA)) ×
  block volume >> 15, pitch (+4 − rand) × (0x1000 − p) / 4096, azimuth +8
  + the block's, delay +6; update 0x46CF70 resends volume (layer × block
  >> 15, 0..0x7FFF), azimuth (0..0x10000) and pitch (0..0x2000), and drops
  ended layers. All whoosh-list layers use bank 1.
* **Doppler** (0x45E1F0, run by 0x471DC0 after the position update; the
  ctor 0x45DD60 sets +0x6C = 1, the filter window 4 (0x5768B0) and
  +0xA0 = +0xA4 = −1): d = field 0 (+0x6C = 1; field 1 for 0) as a float;
  ratio = 343 / ((d − +0xA4) / +0x60 + 343) when both ≥ 0, else 1;
  Average_Add → +0xB4. 0x45E290(k): clamp((+0xB4 − 1) · 0x7F3760[k] + 1,
  0.8, 1.2) (k: 0 horn 0.35, 1 traffic engine 0.33, 2 AI engine 0.6, 3
  0.3); 0.0 while [0x82B884] +0x13. **Inferred**: +0x60 the frame time.
* **AI engine** (CARSFX_AIEngine, vtable 0x78AEE0, ctor 0x473CE0: +0xC0 =
  ((0x82CB4C + 100) % 600) − 300 per object; attach 0x4610B0: type 5 →
  +0xB8, 8 → +0xB4; bank 0x465EA0: the engine record's +4, the SPU bank
  `CAR_xx_ENG_MB_SPU.abk`). Start 0x461300: "CAR" (slot 0x82C538, record
  0x7EDE18), 0x6C bytes {clamp(rec +8, 0, 60), 0 …} (no +0x34 flag).
  Per frame 0x4610F0: Engine slot 0x34 first; p1 = clamp(trunc((rpm −
  1000) · 1/9000 · (rec +0x1C − rec +0x20) + rec +0x20), 0, 10000) (rpm =
  the Engine's slot 0x24 result; 0x784550, 0x78A290); p4 = p5 =
  clamp(((Engine +0xE0 × level[0]) >> 15) − 0x7FFF, −0x7FFF, 0); p16 =
  clamp(trunc(0x45E290(2) × (+0xC0 + 16000) − 16000), ±0x3FFF); p3 =
  clamp(trunc(Engine slot 0x38 × 10.24), 0, 0x400); p17 = clamp(Engine
  +0x134, 0, 0x400); p15 = the azimuth (0..0x10000); p21 = clamp(Engine
  +0x138, 0, 25000). **Inferred**: the AI's Engine controller outputs as
  the context's rpm / throttle with no transients (+0xE0 = 0x7FFF, +0x134
  = 0, +0x138 = 24000), the stock engine entry.
* **Traffic engine** (vtable 0x78B3A8; attach 0x476680: the position
  object → +0xB4; TRAFFIC_MB.abk, FX_TRAFFIC, 2 sounds). Start 0x4621C0:
  variant = 0x82CB38 toggled 0 / 1, FX_TRAFFIC (slot 0x82C668, record
  0x7EDD80) {variant, 0, 0, 0, 0}; the position object is given the car's
  +0x60 / +0x90. Per frame 0x462240: +0x10 (field 3) = trunc([[0x82B884]
  +0x38] +0x10 × +0 × 32767) or 32767 when that is null (**inferred**
  null); with car +0x550: p1 = level[0], p3 = the azimuth, p4 =
  clamp(trunc(min(mph / 140 · 1024, 1024)), 0, 0x3FF) (0 at or below 0),
  p2 = clamp(trunc(0x45E290(1) · 16383 − 16383), −0x3FFF, 0x3FFF);
  without it p1 = p3 = 0.
* **Traffic whoosh** (vtable 0x78B490; 0x475E10, 0x47E8D0). Start, with
  none running and car +0x550: a human player's car (racer +4 = 1, +0x900
  clear) within 5 m (3D, 0x4755F0); rel = 0x462670 = |(view car +0x70,
  +0x74) − (the traffic car's)| (the view car: 0x459A50, the nearer of
  two players) > 15 (0x788870); 3 s (0x787ADC) since +0xB8. Record i =
  (0x82CB68 % 6) + 9 − 6 · trunc(clamp(clamp((rel − 15) / 45, 0, 1), 0,
  0.99) · 127 · −3/128) of the whoosh list (so 9..26); block {i, 0 …};
  +0xB8 = the clock; +0xC (field 2) := 1. Per frame 0x47E8D0: +0xC = 0;
  the whoosh updated with slot 2; released when rel ≤ 15, when its layers
  end or while paused.
* **Scrape** (`src/audio/car/scrape.rs`, verified; FX_Scrape, the second
  event of `FX_Misc_MB.abk`, slot 0x82C5D8, record 0x7EDDF0, block ctor
  0x458010 {0..0x7FFF, 0..9000, 0..0xFFFF, 0..16, 0..0x7F}).
  * Record +0x40 is the push's third argument: Contact_ApplyImpulse
    (0x5AF0F0) passes the length of the contact's relative velocity less
    its normal part (0x43D0C0 at 0x5AF30E, |·| at 0x5AF601) — the sliding
    speed that also ramps the friction in below 1 m/s (0x7FBF30). The
    clone computes it in `drive::slide_speed` for track and car pairs and
    keeps it in the record. The same value feeds 0x5A9630's contact effect
    6 (0x800F70..0x800F74), which the clone fed with the closing speed
    before: now the sliding speed (`particles::post_contact`).
  * Arming: each push within 100 m of the listener runs 0x4814E0 then
    0x4792D0 → 0x469DE0: the record's sound object, neither scraping
    (+0x134) nor done (+0x135), asks slot +0x18: 0x459890 (track 0x78BEE0
    and car–car 0x78BE90) is record +0x40 ≥ curve +0xA4's x0, the curve
    0x82BD40 = {0, 1, 0, 40} (static init 0x469F89 from 0x7884D4..0x7884E0,
    set by the first update 0x46A2B9), so every push passes; props
    (0x78BB08) answer 0x404800 = false. A pass sets +0x22.
  * Start (0x481F10 → 0x46A460): +0x134 = 1, a pool voice (+0xBC, none:
    done); class +0x120 (0x459680) 1 — two cars — returns here, so a
    car–car contact never starts FX_Scrape; class 0 sets the debug name
    "SND_WORLDScrape" (0x7EC844), +0xE0 = +0x54, **+0xD4 = +0xD8 = +0xDC =
    0** (0x46A51A..0x46A526, ebx = 0), then 0x4596E0 builds the block
    {+0xD4, +0xD8, +0xDC, +0xD0 (0, constructor 0x46A0D2), +0xE0}.
  * Per frame (0x479970): +0x54 = trunc(127 · ramp(record +0x40: 0..40 →
    0..1)) (slot +8 0x479AD0, 0x78B104), +0xE0 = +0x54; in the fade +0xD4
    = trunc(+0x6C · +0xD4); 0x459770 resends +0xD4, +0xD8, +0xDC, +0xE0.
  * Stop: 0x481F10 marks the object done (0x46A1A0) when the record is
    gone or its last push is 0.3 s old (0x4595D0: 0x865188 − record +0xC
    in seconds ≥ 0x7FBEE0); scraping, that starts the fade ramp +0x58
    {0, 0.1 s, 1 → 0, linear} (0x46B100); its end stops the event
    (0x46A3A0 → slot +0x38 0x46A400) and clears +0x134.
  * Result: p0 (+0xD4) is 0 at the start and only ever scaled, and
    FX_Scrape's graph takes p0 as its volume (the clone's `aems` run of
    the bank: p0 0x7FFF fades the voice in, p0 0 keeps it at 0), so the
    shipped scrape plays at volume 0. The clone runs it the same way
    (`Scrapes`, silent; the self-test counts its starts).
  **Inferred**: no 10-object cap or voice pool, +0x54 fresh at the start.
* **Second bottom-out knock** (ported in `roadwind`): 0x459AC0(pos)
  is the camera director nearest the position (0x8900B4[0], or the nearer
  of two players' cars), and its +0x58 is the running event kind; 6 is the
  crash / jump camera (see "Player camera movers"). So while that camera
  runs a knock also starts a second one (+0xB8): stitch record 0x2E +
  (0x82CBF0 & 3) − 4 · trunc(Rand_Float(127) · −1/64) (0x47EB9C..0x47EBE5;
  Rand_Float 0x43C1F0 = (seed mod 0x7FFFFFFF) · 2^−31 · n on 0x43C1C0's
  seed), updated with slot 2 (0x47F100). Not "world type 6".
* **Not ported** (traced so far). The sound-object factory table
  0x7ED4C4.. ({id, name, list, creator}): CARSFX_TrafficHorn 0x30020
  (creator 0x47E6A0), CARSFX_TrafficWoosh 0x30010 (0x47E870),
  CARSFX_BottomOut 0x100B0 (0x47E9D0), CARSFX_TrafficBottomOut 0x30040
  (0x47F2B0 → 0x47E980: BottomOut's constructor 0x475F90 with vtable
  0x78BC30, which keeps BottomOut's update 0x47ED80 and per frame
  0x47F100 — the same knocks for each traffic car), SFXObj_Plane /
  _Train / _Fountain / _Reverb, CARSFX_TrafficStereo 0x30030. The
  traffic horn is ported since sound105 (see "Sound effects audit");
  TrafficBottomOut is not (the traffic cars' +0x28 for the knock volume
  is open).
* **Collision impacts and the mix map.** The collision objects'
  vtables override slots 0x38 / 0x3C (0x46A400 the scrape stop, 0x479900
  the stitch start), so they never read the mix map's level / azimuth
  slots: an impact's level is its own 3D chain (0x479AD0 → +0x54). Section
  5 only takes their fields (0x40050000/1/5/6) as inputs. The clone's
  impacts stay as they were.
* **Ducking inputs** (`mixmap::Scene::sfx`). The sound-object fields the
  nine sections read (kind 4): section 1: Shift f6, Nitrous f0, SparkChatter
  f1, Rain f2, BottomOut f0 / f1, DriveBy f5; section 3: TrafficEngine f3,
  TrafficWoosh f2, TrafficHorn f1; section 5: collision f0, f1, f5, f6;
  section 6: f6, f7. Given now: Nitrous f0 = +4 (1 from 0x472AF0, outside
  game flow states 2 / 3 / 7, to 0x45F2A0), BottomOut f0 (1 on a knock's
  frame; **inferred**: the mix reads it before 0x47F100 clears it),
  TrafficWoosh f2, TrafficEngine f3. The tunnel (SFXCTL_Tunnel, class 7
  fields 0–2) needs tunnel geometry the clone lacks: 0.
* **Against the original under Wine** (recordings of
  a private Wine copy, 48 kHz stereo, free roam after Start
  New Career: Rachel's 350Z at the airport, raining, chase camera; the
  clone's the maintainer's probe `car_audio_render`, engine entry 0, no rain). RMS
  (L / R dBFS): idle −29.3 / −27.9 (original) vs −35.6 / −35.6 (clone);
  full throttle −18.7 / −18.4 vs −27.4; skid −17.7 / −17.9 vs −25.7.
  The original's idle energy sits at 100–1000 Hz (peaks near 128 / 191
  Hz, with rain and FX_WIND_Weather in it), the clone's below 100 Hz for
  a different engine; the absolute offset also includes the clone's
  mixer headroom (not native). So these numbers compare loudness trends
  only. **Channel mapping: not established.** Turning the chase camera
  with the mouse (6_camera_turn.wav) keeps the car centred and the
  balance within 1 dB; no traffic car passed during 5_wait_traffic.wav.
  The clone keeps channel 0 (the 90° speaker) on the device's right
  (**inferred**), unchanged.

## Sound effects audit (2026-10-30)

The checklist is docs/sound-audit.md. **Verified** (traced) unless marked.

* **Inventory sources.** The sound-object registry 0x7ED138..0x7ED5D8
  (records {name, list head, factory, id}; the controllers' list head is
  0x7ED230): the global objects SFXObj_Music 0x0, _Speech 0x10, _Ambience
  0x20, _Movies 0x30, _NISStream 0x50, _FEHUD 0x60, SFX_Common 0x40;
  the car objects 0x100x0 (Shift 1, Turbo 2, Nitrous 3, SparkChatter 4,
  Skids 5, RoadNoise 6, WindNoise 7, Hydraulics 8, Rain 9, WindWeather
  0xA, BottomOut 0xB, DriveBy 0xC), PlayerEngine 0x10000,
  AIEngine 0x20000, the traffic 0x300x0
  (Engine, Woosh, Horn, Stereo, BottomOut), SFXObj_Collision 0x50000,
  _Woosh 0x60000, _Plane 0x70000, _Train 0x80000, _Reverb 0x100E0. The
  event names (0x7EDD50..0x7EDE30, {name, id}, bound by 0x457BC1..
  0x458101): FX_ROADNOISE, FX_ROADNOISE_TRANS, ENV_STATIC, FX_MAIN_MEM,
  FX_WIND, FX_WIND_Weather, FX_TRAFFIC, PlayCommonSample,
  PlayFrontEndSample, PlayFrontEndSample_loop, FEDriveOn, FX_NITROUS,
  FX_PURGE, FX_SHIFTING_01, FX_SPARKCHATTER, FX_SKID, FX_Hydraulic,
  FX_Hydr_Bounce, FX_Weather, FX_Camera, FX_Scrape, AEMS_StichCollision,
  AEMS_StichWoosh, FX_TURBO_01, Sputter_Message, CAR, CAR_SWTN,
  CAR_Sputter, CAR_SputOutput. Their users: FX_MAIN_MEM 0x462906 /
  0x462CA5 (the plane / train objects), ENV_STATIC also 0x462A02,
  0x462D20, 0x46312D, 0x463699, 0x463A02 and the traffic horn 0x475AA2,
  FX_Weather 0x4617F6 (Rain), FX_Hydr_Bounce 0x47DED7, FX_Camera 0x45E52E,
  FEDriveOn none but its binding.
* **PlayFESound** 0x458520(id) / **StopFESound** 0x458560(id): with
  0x7FAAA8 set, id < 23 → the manager's +0x78 player (FE_COMMON) slot
  +0xC / +0x10 with id, id < 0x5E → +0x74 (FE_MB) with id − 23. SFXObj_FEHUD
  (factory 0x47E550, ctor 0x475560, vtable 0x78B360) puts itself in both
  players (0x4755D0: +0x74 → +0xE4, +0x78 → +0x24). 72 call sites: 0x50D150
  and 14 other readers of the event table 0x837FE0 (row = event · 27:
  0x838067 RIGHT, 0x838069 RIGHT/TOGGLE, 0x838082 UP, 0x83809D DOWN,
  0x8380B8 SELECT, 0x8380EE BACK, 0x838119 FOCUS/SMS_POPUP, 0x838149
  INSTALL/PAINT, 0x83813F / 0x838150, 0x8381AB LTRIGGER, 0x8381C6
  RTRIGGER, 0x8381E1 WRONG) and direct ids: 0x12 (0x4C85A4), 0x49
  (0x4D8C15), 0x56 (0x4EB263, 0x4EB32D, main menu messages 0x1A2826E1
  and the one before 0x4EB325, which also set 0x83AA79 to 0 / 1), 0x4A
  (0x55B911), 0x4B (0x55DEAA), 0x54 (0x5212AB, kind 0x36), 0x58 / 0x5C /
  0x5D (0x4A652E..0x4A6614), the Quick Race options' 8, 0x2E, 0x2F, 0x30,
  0, 1, 2, 3, 5, 9 (0x4FA641..0x4FA952) and the parts browsers' 4 / 5.
  The HUD's countdown (0x4AB5B0), wrong way (0x4AAFC0) and race over
  (0x4C7F00) call no sound.
* **GPS found** (0x4C8410, from the minimap element 0x4DA640 when
  0x8385B9 and Career_IsFreeRoam): a local flag is set when the GPS state
  (0x501100) is 3; with the element's +0x35 clear and the flag set,
  PlayFESound 0x12; +0x35 = the flag. Clone: `Game::gps_sound` (the
  element's run condition taken as the clone's minimap GPS gate, Explore
  in a career: **inferred** for 0x8385B9).
* **FE car turn** (0x4D82D0 at 0x4D8BE6): car angle +0x274 ≠ +0x170 →
  +0x170 = +0x274 and, unless +0x16C, PlayFESound 0x49, +0x16C = 1, +0x16D
  = 0; equal → unless +0x16D, StopFESound 0x49, +0x16D = 1, +0x16C = 0.
  Id 0x49 is FE_MB's PlayFrontEndSample_loop sound 50 (bank slot 48: 44.1
  kHz, 112500 samples, loop 0..112499 — the maintainer's probe `fe_loop_probe`).
  Clone: `Game::car_turn_sound` on `garage::View::at`'s car angle; the
  loop also stops when the FE car goes (**inferred**).
* **Drive-by whooshes** (`audio::driveby`). SFXCTL_Tunnel (factory
  0x471000, ctor 0x45D950, vtable 0x78A064; attach 0x471040 takes a type
  0xA0 3DTunnelPos into +0x6C, which 0x45DAF0 points at +0x80): per frame
  0x483250 calls 0x471080 and then finds the car's own zone; zones are
  route triggers (0x88F298) of kinds 5 / 6 / 7 / 0x14 (masks 0x20 / 0x40 /
  0x80 / 0x100000, 0x5D9C80 — kinds ascending, record order, restarting
  after a refused hit; 39 / 79 / 42 / 0 records, the same chunk in every
  Paths*.bin); a hit counts when its +0x14 is 0 or ≥ the car's z. 0x471080:
  cooldowns +0x94 / +0xA4 −= dt (floored at 0); the point ahead = car
  +0x60 + normalised +0x90 · +0x42C · 0.3 (0x7886EC); +0x74 = 5 / 6 / 7
  by its zone, else 2. Outside (+0x70 clear) with a zone ahead: when
  +0x94 < 0.01 (0x78425C) or the zone ≠ +0x98, and the ramp 0x7F0B40
  ({0, 1, 18, 45} from 0x788868 / 0x788864 by 0x77C5B0) of speed +0x430
  > 0.01: +0x7C = 1, +0x94 = 3 (0x7886F0), +0x98 = zone, +0x80/+0x84 =
  the point, +0x88 = car z + 10 (0x784560), +0x90 = the ramp. Inside a
  kind 5 zone (+0x78 = 5) with +0x74 = 2: likewise on +0xA4 / +0xA8 (= 0)
  into +0x9C and +0xA0. Then +0x78 = 5 / 6 / 7 / 0x14 (the last kind
  tried), +0x70 = found; entering picks a reverb (0x45DB90 with 5, 6 for
  6–7, else 3; leaving with +0xD7 restores by 0x7ED5D8[+0xB4]): ported
  in sound7 (`reverb::ReverbCtl`, see "Sound round 7"). CARSFX_DriveBy (factory 0x47E070, ctor 0x474930: blocks +0xBC
  / +0xD0 {0, 0x7FFF, 0x1000, 0, 0} and +0xEC / +0x100 the same with +0 =
  4 set at the start; attach 0x4619F0: type 0x70 → +0xB0; bind 0x463500)
  update 0x474A20: on +0x7C, slot +0xB4 or else +0xB8 (both busy: none),
  whoosh stitch record 3 · (15 − trunc(+0x90 · −0.0234375 (0x78B070))) +
  n, n = counter mod 3 (0x82CB54 when +0x74 = 5, else 0x82CB58; counter =
  n + 1), started with block slot 0 (0x45B1F0 on [0x82B884]+0xD0 list 1,
  0x46CDD0), +0x114 = it; on +0x9C, slots +0xE4 / +0xE8, record 3 · (18 −
  trunc(+0xA0 · −0.015625 (0x78B06C))) + n (0x82CB50), block slot 4. As
  the ramp is 0..1 the records are 45–47 and 54–56. Per frame 0x47E0D0:
  a stitch whose +0x1C is clear is released; else level = slot 0x38(block
  slot), azimuth = slot 0x3C (the entry ones only for +0x114), pitch word
  0x1000 (0 while paused), 0x47BB30. The mix map gives class 0xC levels
  (section 1: slots 0–3 11558, slot 4 7724 in the test scene; the 3D
  tunnel position's distance does not change them). **Inferred**: the
  tunnel update before DriveBy's in a frame.
* **Race start scene line** (`audio::speech::nis::race_start`,
  `Game::race_start_scene`). The NIS load 0x435BE0(hash, variant), with
  Crowds' scene loaded (0x827C38) and 0x7FAAA8, calls NisAudio_Request
  0x484510(2, hash, variant, 0x4307B0, 0) (0x435CE8). Kind 2 with no
  special name (GB_NOS 0x78C18C, CollisionStream 0x78C17C, BLOWNENGINE
  0x78C154, THUNDERSTRM 0x78C148) stops a queued NIS stream (0x46C9A0 /
  0x46C9D0) and adjusts the variant (0x48473E): a tournament (0x863470)
  with +0x0D = 1 → + 4; else split-screen (manager +0x84 = 1) or game mode
  8 / 0x200 → + 8; else + 4 when the player's sound context (0x4690B0(0))
  +0x8C is 0x15, 0xE or 9. Track bit by the jump table 0x484B98: 0 or >
  13 → 0x2000, n → 1 << (n − 1). The scene's bit: IntroNis%d hashes
  0x82BA78 (0x45B160: "IntroNis0%d" below 10, 32 entries) → 0x7ED068[i]
  = 1 << i (29 entries; 30–32 read past the table). NIS_Select_Start
  (0x45A6D0 at 0x484A60) = {scene bit, track bit, 4 when [0x8900D8] +0x1C
  is 3, else 1}, then the stream is queued (0x46CA60(2, 0x475530, 0)).
  `nisaudio` event 0x4000 (entry 0x4023, constraints (0x1FFFFFFF, 0),
  (0x3FFFF, 1), (7, 2)) holds 2 records per scene / track: third key 0
  (value 1) or 2 (value 4) — the maintainer's probe `nis_start_probe`. Not modelled:
  the tournament and split-screen adjustments. **Under Wine** (Quick Race
  circuit, Resort Loop, Peugeot 206, Crowds on; memory reads every 0.25
  s): flow +0 / +8 /
  +0x18 / +0x1C went 0/0/0/0 → 1/1/2/1 (LOADING) → 3/0/2/1 (PRERACE) →
  4/0/2/1 (race), so the third key is 1 there; the clone passes 1
  always (**inferred** for other race kinds; who writes +0x1C = 3 is
  open — its only reader besides this is 0x57831F → 0x5FC910). The line
  starts with the scene (**inferred**: the queued stream's start time).

### Round 2

**Verified** (traced in the disassembly; REA's Ghidra provider did not
start, so `speed2.dis`) unless marked.

* **Who plays a menu sound.** The event table is read by
  FE_PlayEventSound 0x50D150(obj, event, control) (80 call sites, each
  in a screen's own code with the screen's +0x30 as the control) and
  FE_PlaySoundId 0x50D180(id) (44), besides the direct readers listed
  above. FEWidget_Handle 0x51DCE0 (only from FEWidgetList_Dispatch
  0x52CCA0, per owner): with +0x2C set, a pad message gets a default
  event (0x0C407210 / 0x406415E3 → 8, Up 0x72619778 → 6, Down
  0x911C0A4B → 7, Back 0x911AB364 → 10, Left 0x9120409E → 4, Right
  0xB5971BF1 → 5, PAD_START 0xB5AF2461 → 9, else 0); the widget's
  vtable +0xC(message, …, default) returns the event actually played,
  table[event][+0x30]. So a menu sound follows the control's own
  answer, not the key. The list widgets play Up only off the first
  node (0x55CB12, 0x55D6AB: `current == first` skips the sound); the
  list at +0x138 of 0x4CC150's screen likewise: up (UP, control 7) only
  off the first node, down (DOWN, 7) only off the last, each with its
  step 0x5437D0(∓1).
* **IconScrollerMenu** (handler 0x543D50; the event local starts at 0):
  Left / Right are handled only with +0x79 set, Up / Down only with it
  clear (the strips move on Left / Right, so theirs is set); each also
  needs a non-empty list and +0x7B. Left: LEFT unless the current item
  is the first (FEList_AtFirst 0x5276E0), then FEList_Prev 0x527760;
  Right: RIGHT unless the last (0x527710), then FEList_Next 0x527740
  (both wrap only with the list's +0x2C). Up plays UP even on the first
  item (it then does not move); Down plays DOWN unless on the last.
  Accept: SELECT, or WRONG (0x13) when the item's +0x22 (locked) is set;
  Back: BACK (and +0x7B = 0). Clone: `IconMenu::sound_event` (the strips
  do not wrap, so Left / Right at the ends are silent; Up / Down silent;
  the SideScroll text list keeps Up / Down: **open**, its handler's
  sound).
* **Setup purchase** (Shop_PurchaseSetup 0x547580, from the unique-part
  screens 0x562D49 / 0x563007): it ends with PlayFESound(table
  [INSTALL][PAINT] = 0x838149, id 38) after the car is updated
  (0x5477A9), once per purchase. Clone: `purchase_setup`.
* **Transmission question** (0x4E8D30's message switch): Auto
  (0x1A2826E1, 0x4EB25B) and Manual (0x5F5E3886 → 0x4EB325) both play
  PlayFESound 0x56 (FE_MB 63) before setting 0x83AA79. Clone: the
  dialog answer in `menu_key`'s Dialog arm (`fe_id`).
* **Traffic horn** (`audio::traffic`, CARSFX_TrafficHorn): ctor 0x475820
  (+0xC6 = 1, its only writer; +0xC0 = +0xC8 = 0; 0x82BC50 = 0); update
  0x47E6E0 with the context's +0x3C: Interp_Step 0x46B100 on the fade
  +0xD0; with no voice (+0xB4) — car +0x550, clock > +0xC0 + 3 and >
  +0xC8 + 3 (0x787ADC), +0xC6, TrafficHorn_HumanFast 0x475690(15.0)
  (the first racer with +4 = 1 and +0x900 clear: car +0x42C > 15), and
  TrafficHorn_ViewTest 0x475B30: the player's car (0x459A50 → [0x8900B4]
  +4 with fewer than two humans, 0x8900BC) within 20 m (0x78886C, 3D) of
  the traffic car +0x60; n = normalise(traffic +0x70, +0x74, 0), d =
  normalise(player − traffic, z 0), f = normalise(player +0x90 forward);
  n · d ≥ cos 2.268928 (130°; 0x43C350 is a cosine: the series at
  0x43C2B0 starts at 1.0, 0x784250) and f · n ≤ cos 1.919862 (110°); in
  a drag (0x89E7D8 or 0x890108) n · d ≥ cos 3.577925 alone. Then
  Rand_Float(1) < 0.3 or clock > 0x82BC50 + 5 starts it (variant 5 +
  Rand_Int(3), +0xB8 = 1.2 + Rand_Float(2.8), +0xC5 = 1,
  CARSFX_TrafficHorn_Start 0x4759C0: ENV_STATIC {clamp(variant, 0, 50),
  0, 0, 0, 0}, 0x82BC50 = +0xBC = clock, fade 1 → 1 over 0.001 s); else
  +0xC8 = clock. With a voice: clock past +0xBC + +0xB8 (or +0xC5
  clear), +0xC4 clear, no fade yet → SFXObj_FadeOut 0x462480 (1 → 0,
  0.15 s); fade done → SFXObj_StopHandle 0x4624E0 (+0xC0 = clock). Per
  frame SFXObj_EnvStatic_Frame 0x462550 (vtable slot 10 → 0x475920):
  p2 = clamp(trunc(doppler(0) · 4096), 0, 0x1FA0), p1 = clamp(trunc(slot
  0x38(1) · fade value +0xE4), 0, 0x7FFF), p3 = clamp(slot 0x3C(1), 0,
  0xFFFF) (mix map class 2). **Inferred**: the context's +0x3C (set as
  a context starts, 0x464D30 / 0x469B40) on for every traffic car the
  clone keeps, as for the engines; +0xC4 clear (its writers not
  traced); the objects' clock. **Open**: the horn's field 1 for the mix
  map's section 3 (not fed).
* **Interactive score**: see Round 3 below (ported in part).
* **Still open**: FEDriveOn, SFXObj_Ambience, the screen-specific ids of
  F6, F7, F11–F14, F21 (see Round 3 for the rest).

### Round 3

**Verified** (REA / Ghidra decompile, the disassembly for the vtable
targets Ghidra has no function at) unless marked.

* **Showcase world start / stop (F5).** EngageState_Run 0x529590 and
  EngageState_Stop 0x5295C0 each play PlayFESound(table [BACK][0] =
  0x8380EE, id 5) when the cell is not −1; their only callers are the
  showcase states (0x4B5AAF, 0x4B5C4C, 0x4B5D73, 0x4B5E71): Control Car
  and Show Off starting, Back ending them. Clone: `showcase_key` on
  `ShowcaseKey::Drive` / `Stop`.
* **FX_Camera is the showcase's camera shutter.** The flag the
  audio manager's 0x45E450 reads and clears ([0x82B884] +0x08) has one
  writer, 0x458880 (`mov byte [ecx+8], 1`), called only by 0x501250,
  which Take Picture runs (0x4CF592, just before 0x499E60 streams the
  masthead); 0x501250 also calls 0x5DBD80(1) on each view 1–2 with
  +0x08 set (not traced further). 0x45E450 then stops the last event
  (+0x9C, its vtable +0x04), and starts FX_Camera (slot 0x82C72C,
  record 0x7EDDE8 → "FX_Camera" of FX_Misc_MB) with an all-zero block
  (+0x04..+0x1C), keeping it in +0x9C. Clone: `Scrapes::shutter`
  (`audio::scrape`, which owns the FX_Misc bank; the bank runs in the
  showcase too), from `ShowcaseKey::Stream`.
* **Part fitted (F10).** FE_PlayInstallSound 0x521290(part) follows
  0x55C240's fit at six sites: part +0x04 (the slot) 0x10 (exhaust tips)
  → table [INSTALL][EXHAUST] (0x838150, id 49); 0x36 (rear window
  decal) → id 0x54 (84), then [INSTALL][0] (0x83813F, id 50); any other
  → id 50. Callers: the custom gauges overlay's purchase (handler
  0x563D00 at 0x563EE7, after the needle / face / numbers picks),
  the body shop's carbon toggle (0x5670AC) and select (0x5671A9, both
  in 0x5664A0's handler), the UI_DecalMain screen's handler 0x568D40
  (vtable 0x79BA1C; 0x568E25, 0x56901B) and the parts list screen
  0x553320's handler 0x56A310 (vtable 0x79CAD8; 0x56A391). Clone:
  `install_sound` on the body shop's toggle and select and the gauge
  purchase. **Open**: which clone screens the last two handlers are
  (the clone's decals go through the setup purchase, 0x547580).
* **UI_VirtualKeyboard (F15, was "Quick Race mode options").** The
  handler at 0x4FA5B0 is slot 2 of vtable 0x78F8E4, which the
  constructor 0x4E3300 installs; its factory 0x4EFD40 is the descriptor
  0x7F8554 {"UI_VirtualKeyboard.fng"}. Sounds: Up 0, Down 1, Left 2,
  Right 3, Back 5, Select on a key 0x2E, on the special keys 0x2F / 0x30
  (by 0x498BC0's index), PAD_START 0x2F, BUTTON1 0x30, 0x5073EF13 → 8
  (unless 0x4FF970() is 8), 0xD9FEEC59 → 9. **Open**: who opens it (the
  name 0x799D54 is only compared in the screen base constructor at
  0x551ED3; its hash 0x4677789D appears nowhere in the code).
* **Online messenger (F17).** MenuScreen_BaseNotify 0x55B5D0 on
  Pad_Button1 (0xC519BFC0), logged in (0x8500D0 > 0xB), not in the
  garage, no dialog up and the messenger not shown: id 0x4A (74), then
  UI_OLEAMessenger.fng. Online only: n/a.
* **Mouse hot spot (F18).** 0x55DE10 (the menu mouse walk, see the PC
  controls) plays id 0x4B (75) when the hit node's mask +0x1C has bit 2
  and posts its message (0x5587A0). The clone has no menu mouse.
* **Garage hydraulics (F19, C15's front-end half).** 0x4A6030 runs the
  garage car's four corner heights from parts slot 0xA1's mode (part
  +0x05 >> 5, 1–3; table 0x7F6864, 0x114 bytes per mode): with the FE
  sound object up ([0x82B884] +0x74), each corner's change of state
  (+0x1A4 + 4i: rising, settled, back at rest) plays one of 0x58–0x5D
  (88–93) by the corner's index parity and the mode (mode 1 even
  corners 0x59 / 0x5A, mode 2 odd corners 0x5B / 0x5D, back at rest 0x58
  or 0x5C by i / 2); the exact branch conditions are not re-traced. The
  clone ports it (2026-10-31, hydro): see "Hydraulics".
* **Interactive score (partial).** 0x482180 (each frame, unless
  state 5): in the front end (audio +0x90 = 1) EA Trax when the menu
  music level (audio +0x38 → +8) is non-zero and the menu mask has a
  song (+0x84), else the score; in game EA Trax when the game music
  (+0xC) and master (+0) are on, the race mask has a song (+0x8C) and
  it is neither a drift (0x401910) nor a drag race (0x4018F0), else the
  score; EA Trax returning −1 (0x47B080: no song) also runs the score
  0x47B260. The volume: +0xCC's (EA Trax) or +0xD0's (the score) vtable
  +0x38(side) → 0x46C800(v >> 8). 0x47B260 takes the context +0xB4
  from 0x46B330 and starts g_InteractiveMusicEvents 0x7ECC10[+0xB4]'s
  first event (0x7399E0 / 0x4580E0); a context change ends the piece
  (front end: 0x739320 at once; in game: +0xD0 +0xC = 1 fades it, and
  at volume 0 the new context's event starts), and with nothing playing
  (0x82AFD4 = −1) the context's event starts again. 0x46B330: in the
  front end +0xB4 = 0x1C, in a career (0x850078 = 1) with a shop record
  (0x86344C when 0x863444 is set) 0x16–0x1D by its name hash and type
  (0x17 body shop, 0x18 / 0x1A car lot by CC_MEGALOW, 0x19 other shop
  kinds, 0x1B, 0x16; 0x1D none matched); in game +0xB4 = field 0 (int)
  of the "Ambience" parameter map (object 0x82C710, built by 0x60B790
  from the name; +0xC the bound map) at the first player's car object
  +0x60 / +0x64 (0x5FA550 → 0x5FA4F0 row, 0x5F0B90(0) its dword), and
  +0x60 the "Volumes" map's (0x82BEA0) field 0 (−3 dB steps, rows 0,
  −3, −6, −9, −12). Bayview's Ambience map: 20 rows, values 0–16 and
  20. The MPF: each score event (27, 29, … 77) starts at a type-1 node
  whose next is a type-4 node; their streams are entries 0, 11–16,
  23–28, 40–42, none of them a song's (`score_events_and_streams`).
  Clone: see "Round 4" below (the earlier note "the fade cuts, the
  level is the side's music" is replaced).
* **Rain**: not ported; `EnvInput::rain` is the hook for the
  weather port (CARSFX_Rain 0x47DFE0, FX_Rain_MB).

### Sound round 4

* **EA Trax playlist (verified).** The music object (0xD4 bytes,
  0x459B80, [audio 0x82B884 +0xA0]) holds per side a song mask, a pool
  of the songs not played yet and the playlist position of the last
  sequential pick: front end +0x84 / +0x88 / +0xA8, game +0x8C / +0x90 /
  +0xAC; +0x9C the play mode (1 Random). 0x479220 builds the masks from
  the profile playlist 0x83ACB8 (27 × {song index, state}; states 1 / 3
  → front end, 2 / 3 → game) and 0x46BAA0 stores them with the pools
  full and both positions −1. It runs on the profile's reset (0x515BCE)
  and load (0x5346AA), each jukebox state change (0x53DEC0 → 0x53E09D),
  the play-mode toggles (0x524246, 0x565E38), the jukebox's exit
  (0x56B1D0 on 0x911AB364) and 0x551071 — nothing else touches the
  pools, so going between the front end and the game keeps each side's
  place. 0x46B6F0 (the front end's pool also for g_RaceType 8, the
  dyno): a mask of 0 → −1 (no song: the score); a spent pool refills
  from the mask and the position goes back to −1; Sequential: the first
  playlist position after the last whose song is in the pool;
  Random: n = Rand_Int(count of songs left, bits 0..26) (0x43C1C0 on the
  game's one random state 0x7EA624; the 0x43BDF0 call before it reads
  the performance counter and discards it), the n-th song left by song
  index; the position is not moved. The pick leaves the pool. In the
  front end 0x47B080 also needs +0xA5 = 1 and +0xA4 = 0 (0x47AC50's
  states). Only 0x515AA0 (table order) and the profile load write the
  playlist's index fields; the jukebox lists and edits by position.
  The title forces song 0 first whatever the seed (0x4A90F0 → 0x47AC50
  with −2: front pool 1; docs/menu-music.md). Clone (`music::Playlist`,
  `Streams::music_frame`): ported; the clone draws from its own
  `GameRng` stream (same seed), so after the first pick the order is
  not the original's (**inferred**: other code draws from the shared
  state in between). Test `playlist_sides_and_modes`; capture:
  `NFSU2_SOAK=menu:1000,drive:300,menu:600 NFSU2_AUDIO_WAV=-
  NFSU2_AUDIO_DEBUG=1 game --flow soak` logs "trax pick Some(0) (front
  end, random)", in the race Some(26) then Some(19) (game), back in the
  menu Some(4) then Some(2) — the front end goes on from its own pool
  (before: it wrapped to song 0 after every race). Offline the songs
  last longer than their length (the decoder thread is slower than the
  offline mixer).
* **Music levels (verified).** 0x482180 sets the stream volume each
  frame from the music object's +0xCC (EA Trax: the SFXObj_Music
  object, 0x4750B0 at its vtable slot 7) or +0xD0 (the score:
  SFXObj_Ambience, id 0x20, factory 0x47E410 → 0x4750D0, vtable
  0x78B158, slot 7 0x475100): vtable +0x38(slot) = 0x45E390, the mix
  map's packed level, slot 0 in the front end, 1 in game → +0xBC,
  0x46C800(+0xBC >> 8) (0..127). Section 0 of the mix map (all three
  maps): SFXCTL_MasterVol (0x471970, class 0) fields f0..f5 = (1 − v ·
  master · c) · 32767 for v = menu music (+8), game music (+0xC), speech
  (+4), effects (+0x10), tires (+0x18), engine (+0x14), c =
  curve0(0x7FFF − master · 32767) / 32767; controls 0..5 take them
  through curve 8, so control k = lin_to_db(volume). EA Trax: master 0
  (slot 0, base −1100 mB, control 0) and master 1 (slot 1, base −900,
  control 1 and the sub-channel ducks); the score: master 4 (both
  slots, base −200 in MAPOUTPUT / DFT, −600 in DRG, map offset −800 mB),
  control 3 — **the effects volume** — plus event controls 1 (trigger
  SFXObj_Ambience f2, −10000 mB, mode 2, attack and release 11 frames
  curve 2: the context-change fade), 2 / 5 (speech ducks) and 13.
  0x482180 also routes to the score when the side's music level or (in
  game) the master is 0, so with the menu music off the front end plays
  the score at the effects level.
* **Score in game (0x47B260, verified).** A context change sets
  SFXObj_Ambience +0xC (f2) = 1, +0xC8 = 1 and +0xB0 = the new context;
  while +0xC8 is set the volume is level >> 8 and at level 0 the piece
  stops and the +0xB0 context's event starts (+0xC = 0, +0x10 = 1,
  +0xC8 = 0). Steady: with nothing playing the context's event starts;
  volume (trunc(+0x7C · 32767) · level) >> 23, +0x7C the ramp at +0x68
  (0x459990, 300 ms, shape 0) from 0x468140(+0x64) to 0x468140(+0x60)
  when +0x60 (the "Volumes" map's field 0 at the car, when bound)
  changes. The front end: a context change stops the piece at once.
  Only the first event of each 0x7ECC10 pair is ever read (no reference
  to +4). Front-end contexts (0x46B330): outside a career 0x1C; in a
  career by the shop record (+0x40 scene name, +0x38 hash): CARLOT01
  0x17, PERFSHOP01 0x18 at CC_MEGALOW else 0x1A, PARTSSHOP01 0x17 at
  SH_BODYSHOP / SH_BODYSHOP_01 else 0x19, PAINTSHOP01 0x1B, AUDIOSHOP01
  0x16, CRIB01 or none 0x1D, another 0x1C. Clone (`Streams::score_frame`,
  `music::front_end_context`, `mixmap::master_vol_fields`; the mix map's
  section 0 now runs in the front end too): ported in game (songs and
  score at the mix map's levels; the Volumes ramp **inferred** to step
  once a frame). **Open** (resolved in Sound round 5): the front end's songs and FE sounds through
  the mix map (SFXObj_FEHUD master 9's slot per FE id is not traced), so
  in the front end the songs keep the menu music level at full scale and
  the score the effects level at full scale. Check: `NFSU2_AUDIO_WAV=…
  NFSU2_AUDIO_DEBUG=1 game --shot draghud` logs "score context 0x1 event
  47 stream 16 gain 0.197" (DRG: −600 − 800 mB). The fade is
  not exercised by a test or a shot (none changes the context in a race).

### Sound round 5

* **Levels checked under Wine (verified).** The original (wined3d,
  a private Wine copy, audio into ALSA's null device: a private
  `ALSA_CONFIG_PATH`, so 0x7FAAA8 = 1 and the sound objects exist) read
  with fresh options (0x83AA28: master 1, speech 1, menu music **0.7**,
  game music 1, effects 1, engine 1, tires 1): main menu — the music
  object's +0xBC 6447 (>> 8 = 25), EA Trax slots [6447, 11598],
  SFXObj_Ambience [10337, 10337], SFXObj_FEHUD (class 6) slots 0..6
  [4235, 6157, 6522, 6157, 6522, 4486, 3085]; Quick Race (Resort Loop)
  under way — +0xBC 11585 (>> 8 = 45, the song's stream volume 45/127),
  Ambience 10325. The clone's section 0 gives exactly these in the front
  end (test `mixmap::global_levels`) and 11598 in game (13 higher, 0.01
  dB: the same stream volume 45). So round 4's in-game levels stand, and
  the front end follows the same map: its songs at slot 0 (the menu
  music level through master 0: 25/127 at the default 0.7, not 0.7) and
  its score at slot 0. Clone: `Game::stream_sounds` uses the map's levels
  in the front end too.
* **FE sounds (verified).** SFXObj_FEHUD's two players: FE_COMMON (mgr
  +0x78, vtable 0x789D28, the object at +0x24) +0xC 0x469760(id): unless
  0x82B924 is set, FX event PlayCommonSample (slot 0x82C5A8) with the
  block {id clamped 0..0x4B, FEHUD vtable +0x38(0) clamped 0..0x7FFF,
  0x1000, 0}; +8 0x458D50(record) the same at slot 0 × record +8 >> 15.
  FE_MB (mgr +0x74, vtable 0x78A744, the object at +0xE4) +0xC 0x469520:
  PlayFrontEndSample (0x82C660) with {id, slot 1, 0x1000, 0}. Both free
  the wrapper at once (0x45FA60(1) releases the handle); the graphs play
  out (PlayCommonSample layers two detuned side voices at a quarter of
  the level, volume bytes 4 / 4 / 16 at 4235). Id 50 (0x49, the FE car
  turn) is PlayFrontEndSample_loop (0x82C590), started once with {50, 0,
  0, 0}, +0xEC = −1; StopFESound (+0x10, 0x469490) only sets +0xEC = 0
  and +0xE8 = the time; the update (+0x14, 0x4793D0) stops it once
  0x43E5C0's elapsed passes 100 (0x784258; **inferred** ms), else sets
  {level slot 6, 3000 + 500 · clamp(m, 0, 1)}, m the mean of the last 5
  |FE car +0x248| (the right-stick turn rate, stored at 0x7EC858 by
  0x4D8C31; average 0x82C6E8 via 0x576980 / 0x576920; ramp 0x7EFFFC),
  and slot 5 on its +4 one-shot. The FEHUD slots include the effects and
  master volumes (SFXCTL_MasterVol control 3), so the clone divides them
  by its effects bus (`Game::fe_event_levels`). Clone: `fe::FeEvents`
  (test `fe_events_play_at_the_hud_levels`); the clone's FE car has no
  stick turn, so m = 0 (the native value with the stick idle). Check:
  `NFSU2_SOAK=menu:30 NFSU2_AUDIO_WAV=x.wav NFSU2_AUDIO_DEBUG=1 game
  --flow soak` logs "FE sound 3 (PlayCommonSample 3) level 4235".
* **Rain (CARSFX_Rain, verified unless marked).** Id 0x10090 (section
  1 class 9), ctor 0x4746D0, vtable 0x78AFD0. Update +0x24 0x474770(dt):
  nothing in flow state 1; with a weather object (0x48B1E0(1)) +0xB0 =
  weather +0x1F0; above 0.001 with no event (+0xD0) 0x461750 starts
  FX_Weather (`FX_Rain_MB`, 0x82C508, block +4..+0x18 = 0, +0xCC = 0);
  otherwise a playing event not stopping is stopped by 0x461840 (+0xC8 =
  10 s, +0xCC = 1, block +0x18 = 1). Thunder: +0xB8 += dt; with rain >
  0.25 (0x78B020), 0x8A329C = 0x88EC68 = 0 (**inferred** 0: writers not
  traced) and +0xB8 > +0xBC: at +0xC4 = 0, +0xB8 = 0 and, unless the NIS
  stream object is busy (0x46C9A0), +0xC0 cleared, vtable +0x44 0x4852A0,
  +0xBC = Rand_Int(20) + 25, +0xC4 = 1; else +0xC4 counts to 32 and wraps
  to 0. Stopping: +0xC8 −= dt, at 0 vtable +0x20 0x474720 releases it.
  Params +0x28 0x4618C0: {+4 0, +8 0, +0xC slot 0, +0x10 7000, +0x14 0}
  (slot 1 and +4 = 1 when the sound context +0x9C has +0xAC set and +0xA8
  2 or 4: **open**, not modelled). Mix field 2 = 1 while playing and not
  stopping (+0x34 0x461890). 0x4852A0: unless +0xC0, flow state 2 / 3 /
  7 or g_RaceType 8, NisAudio_Request(4, "THUNDERSTRM", 1 when rain <
  0.5 (0x7864E0) else 2, callback 0x461880) → 0x484510 sets
  SoundFX_Select = (4, variant) (0x45A770), stream kind 4, unless GB_NOS
  plays (0x82BB29). SoundFX_Select's event is `nisaudio` 0x400E
  (constraints (0x1F, 0), (0x3F, 1)): records [0, 2] GB_NOS (1, 4), [1,
  3] × 6 CollisionStream (2, 8), [2, 0] × 9 THUNDERSTRM (4, 1), [3, 4] × 4
  BLOWNENGINE (8, 0x10), [4, 5] × 3 (no caller). None fits (4, 2), so in
  rain of 0.5 or more no thunder line plays (**inferred**: the library
  plays nothing for no fit). Clone: `roadwind::Rain`,
  `EnvSfx::rain_update`, the line by `Streams::play_nis` (its first
  record: **inferred**, the library's pick among the nine is not
  traced), +0xC0 cleared when the line ends (**inferred** for the
  callback). Tests `soundfx_lines`; check: `NFSU2_RAIN_CLOCK=150
  NFSU2_SOAK=career,drive:90 NFSU2_AUDIO_WAV=x.wav NFSU2_AUDIO_DEBUG=1
  game --flow soak` logs "rain FX_Weather starts (rain 0.692, level
  1831)" and "thunder variant 2 (rain 0.69) fits no line".
* **Scene lines (verified).** Every scene's NIS load (0x435BE0)
  calls NisAudio_Request(2, scene hash, variant) (gated on 0x827C38 and
  0x7FAAA8; the clone takes both as set). After the variant's track
  (0x48473E, `speech::nis::track`) a scene that is no IntroNis goes on at
  0x4848F0: the ending / dyno / shop tables (0x82BAF8.., 0x82BB18..)
  give the first value (EndingNis n → 1 << (n − 1), over 4 → 4), the
  second the track bit, 0x4000 for the enter scenes (0x82BB20 / 24),
  0x8000 for the exits (0x82BB18 / 1C); NIS_Select_End = {first, second,
  1} (0x45A720 at 0x484AC5), stream kind 6. Clone: `Game::scene_line`
  for the ending scene (`play_race_ending`) and ShopEnter1 / ShopExit1
  (`start_shop_scene`). Dyno scenes are not in the clone.
* **Not reached**: W7 planes / trains / fountains (factories 0x47F2F0 /
  0x47F350 → ctors 0x476090 / 0x4761A0, referenced only by the registry:
  what creates them in the world is not traced), W8 reverb (SFXObj_Reverb
  0x47F3B0 → 0x4762C0; no reverb DSP in the clone), the screen-specific
  FE rows F1 / F6 / F7 / F10 / F12–F14 / F21 / F22 (they now play through
  the event graphs at the native levels, but which control reacts per
  screen is unchanged).

### Sound round 6

**Verified** (the disassembly and REA / Ghidra) unless marked.

* **World sound states.** The sound-state registry list 0x82BEC4
  (records {id, name, list, factory}, registered at 0x47FB80..):
  CSTATE_Main 0, Fountain 0x40000 (0x47F600), Collision 0x50000,
  DriveBy 0x60000, Plane 0x70000 (0x4836C0 → 0x47F580, vtable 0x78BCC4),
  Train 0x80000 (0x483700 → 0x47F5C0, 0x78BCF4); the object registry
  (records {id, name, list, factory}: 0x7ED588 SFXObj_Plane 0x47F2F0,
  0x7ED598 SFXCTL_3DEnviroObjPos 0x4627D0, 0x7ED5A8 SFXObj_Train 0x47F350,
  0x7ED5B8 SFXCTL_3DTrainObjPos 0x462FE0, 0x7ED65C SFXCTL_3DFountainPos
  0x4637C0, 0x7ED66C SFXObj_Fountain 0x47F520). The managers are made at
  0x485600.. with (class, the most) through vtable +4 and set up through
  +8: class 4 fountains (0x78A550: 5 states only while 0x89E7A0 is in
  4000..4199, 0x476F00), 7 planes (0x78A4E0: 3 states, 0x476E00), 8
  trains (0x78A518: 6 states, 0x476EB0). A state (0x476990 makes it from
  the registry by the class's high word) attaches (0x464D30: +0x40 the
  table entry, +0x3C = 1, each object's slot 0x2C) and releases
  (0x464D60). Updates (vtable +0x18, with 0x8900BC > 0): 0x464410
  (planes, 2000 m), 0x464600 (trains, 400 m; an entry 0x439F40 cannot
  find gets state 2 and is skipped), 0x4647E0 (fountains, 150 m, only on
  tracks 4000..4199; the point's group 0 / 1 into state +0x44): per entry
  in order, d = Vec3_Distance to the nearest player's car (0x459A50); a
  state on the entry releases at d ≥ R or (planes, trains) when the
  animation waits; otherwise below R a free state (+0x30) attaches, or,
  none free, the first busy state whose own entry is farther from its
  nearest player than d is released and attached. Then 0x463BD0 runs the
  objects. Tables: 0x7ED690 (6 planes; drag on tracks 4400..4499
  0x7ED718, the same), 0x7ED6D8 (drag elsewhere, 5), 0x7ED760 (URL, 3),
  0x7ED798 (24 trains), 0x7F0D70 (7 fountain points) and 0x7F0B80 (31).
  0x439F40(number, hash): [0x827B94] (the world animation controller
  list made by 0x435D60 for 0x827B88) entry whose instance record +0x00
  is the number (0x432170), its entity controller whose hash (vtable +0xC)
  is `hash`, the node's world translation +0x40 (world matrix +0x10,
  0x60BB20 = local +0x50 × parent); waiting = the controller's player in
  its start delay (0x431610: flag 0x80, 30 · speed · +0x14 < 30 · +0x68,
  frame below the end) or pause (0x431660: 0x100, +0x18 / +0x6C). L4RA
  has all six free-roam planes (instances 0x22..0x2C, the entity hashes
  0xA1672F2D..33) and, of the trains, only instance 0x6A's three
  entities: the other entries' numbers name L4RA instances that hold
  other entities (e.g. {0x71, 0x863F23CC}, while 0x863F23CC is under
  0x70) or none (0x72, 0x76, 0x7E, 0x84); 0x10 is L4RB's. The objects:
  SFXObj_Plane (vtable 0x78B530): position 0x462AE0 (+0xC0, the move
  +0xD0), start 0x462810 (controller +0x2C → +0xC0, +0x28(0, 20000);
  FX_MAIN_MEM {0 …} in a URL race unless 0x457BA0 (game mode 4 with race
  type 8), else ENV_STATIC {1 …}), frame 0x462B70 (a move below 0.3 m,
  0x78758C, stops both; else restart if idle; FX_MAIN_MEM {0, slot 1,
  doppler(5) · 4096 ≤ 0x4000, azimuth 1, ±min(field 0, 1000) · 2.6 —
  negative for an azimuth in (0x4000, 0xC000) — within ±1000};
  ENV_STATIC {1, slot 0, doppler(5) · 4096 ≤ 0x1FA0, azimuth 0}), stop
  0x462A40. SFXObj_Train (0x78B578): start 0x463020 (ENV_STATIC {0 …}),
  frame 0x4631E0 {0, slot 0, 0x1000, azimuth 0}, stop 0x463180.
  SFXObj_Fountain (0x78B650): start 0x463920 (ENV_STATIC {4 …}), frame
  0x463820 (group 0: slot 0, 0x1450; group 1: slot 1, 0xFA0), stop
  0x463A60. The position controllers share 3DCarPos's update 0x471DC0
  (fields 0x45E100 → 0x45DF00, 2D; doppler 0x45E1F0) over the pointed
  position; 0x45DE50 returns field 0 (+8, the distance). doppler(5) =
  depth 0.3 (0x7F3774). ENV_COMMON's ENV_STATIC: variant 0 → sound 1, 1 →
  2, 4 → 3. Mix map: sections 4 / 7 / 8, class 0, field 0x6004/7/80000
  (the distance), one copy per state (0x4860FC). Clone: `audio::enviro`
  (`World::anim_points`, `LoopPiece::waiting`, `Scene::enviro`). Levels
  (MAPOUTPUT, the maintainer's probe `enviro_probe`): fountain slot 0 2886 at 5 m,
  0 from 50 m; plane slot 0 / 1 3663 / 9748 near, 555 / 1476 at 400 m, 0
  at 800 m; train 4225 at 5 m, 543 at 200 m. **Inferred**: the 0.3 m move
  per 60 Hz frame; the objects after the managers each frame. Check:
  `NFSU2_SOAK=career,drive:40 NFSU2_AUDIO_WAV=x.wav NFSU2_AUDIO_DEBUG=1
  game --flow soak` logs "plane 3 starts at 534 m …";
  `NFSU2_SHOT_AT=-150,-610,16,0 … game --shot marker x.png` "fountain 0
  starts at 6 m …".
* **Reverb.** SFXObj_Reverb (0x47F3B0 → 0x4762C0, vtable 0x78B5C0;
  attach 0x4763A0 takes the type-0x70 SFXCTL_Tunnel into +0xB0) loads the
  12 presets of 0x7ED608 into 0x82AF60 (0x82BC54 = 1 once the 12th is in,
  0x4632B0). The graph's DSP, the preset levels and the zones: see "Sound
  round 7" (the level table starts at 0x7F3778, not 0x7F3788 as first
  read here).
* **SoundFX streams (S4).** NisAudio_Request 0x484510 by name: GB_NOS →
  SoundFX_Select (1, 4) unless 0x82BB28, the race flow's state is 2, 3
  or 7, or 0x82BB2B, then 0x82BB29 = 1; CollisionStream → 0x82BB2B = 1, (2,
  8); BLOWNENGINE → 0x82BB2C = 1, (8, 0x10); THUNDERSTRM unless 0x82BB29;
  every request first clears 0x82BB2B unless the view mode 0x82BB48 is
  3; each is queued as stream kind 4 (0x46CA60). Callers: GB_NOS from
  CARSFX_Nitrous's update 0x484F20 on its start (state 0, a shot running
  and no purge) when the nitrous fraction (0x5925C0) is above 1.5
  (0x785110), the car a human racer (0x431DD0), the flow in state 4, 0x8A329C
  = 0x88EC68 = 0 and no NIS stream (+0x44); 0x82BB29 clears when the
  shot ends (0x484FB9). BLOWNENGINE from SFXCTL_Physics 0x484C50
  (0x484E82..0x484F11): once per controller (+0x6C), flow state not 2 /
  3 / 7, the player's context (+0x14 = 1), the engine blown (0x5ECE10),
  no NIS stream busy (0x46C9A0) and the stream ready (0x46C790).
  CollisionStream from Car_Crash once its crash camera starts
  (0x6022F0(6, 0, 1, 1) at 0x5A0105 → 0x486130). Clone: `soundfx_request`
  (`Game::car_sounds`, `PlayerSfx::nos_edge`, the crash hook in the race
  update). **Inferred**: the flow states (3 before the start, 4 racing and
  in free roam, 7 finished), 0x82BB28 clear, a request replacing the
  playing line.
* **FE.** FEDriveOn (F22): its event slot 0x82C520 is referenced
  only by the binding (0x457BF7) and its destructor (0x77C750) — never
  played (n/a). UI_VirtualKeyboard (F15): its name 0x799D54 is referenced
  only by the descriptor 0x7F8554 and the base ctor's compare 0x551ED4,
  the descriptor and the hash 0x4677789D nowhere (byte search of the
  exe) — no opener (n/a). The "help bar" sites of F6 are the generic
  helpers: 0x51DD91 in FEWidget_Handle, 0x51DE5F in 0x51DE40(event,
  control) (24 callers) and 0x51DE8A in 0x51DE70(_, event) (column 0, 23
  callers), so F6 is part of F1. UI_GenericParts_Browser's handler
  0x5664A0 (the clone's Parts browser): Accept (0x0C407210 /
  0x406415E3) on a locked item (+0x58 → +0x22) plays WRONG column 0
  (0x8381E1) and returns (clone: `menu_key`). The F10 handlers: UI_DecalMain
  0x568D40 is slot 4 of vtable 0x79BA14 (written at 0x532658 /
  0x532670), the parts list 0x56A310 slot 32 of 0x79CA60 (0x552EEC /
  0x552F8E). **Open**: the remaining screens' handlers (F1, F7, F10,
  F12–F14, F21: 171 call sites of 0x50D150 / 0x50D180 / 0x51DE40 /
  0x51DE70).

### Sound round 7: the reverb

**Verified** (the disassembly and REA / Ghidra) unless marked. Clone:
`audio::reverb` (graph and controller), `audio::mixer` (bus, chunks),
`Game::car_sounds` / `engine_sound`.

* **The .fx file** (big-endian; 0x726DB3 / 0x7268BF): `"FX" 0B 00`, u32
  size, then per channel config c = 1..6 a module count (dword 1 + c) and
  a link count (dword 7 + c). The config used is the largest c ≤ N (the
  output channel count 0x8B47F9) with modules; every preset has c = 1 and
  2, so stereo uses c = 2. Its records follow the earlier configs'
  (record sizes 0x818A08 = {4, 8, 4, 4, 8, 8, 4, 8, 4, 4} by kind, link
  records 12 bytes): module {u8 kind, u8 channel, words p2, p4, p6}, link
  {u32 from, u32 to, u8 from port, u8 to port}. 0x72A26D(from, to, pf, pt)
  sets from +0x10 / +0x14 (by pf) = to and to +8 / +0xC (by pt) = from
  (an input already set refuses the link), to +0x1A = pf. The input module
  is the config's first kind 0 (+0x4C counts the records before it). A
  module's channel byte puts it in output slot 0x72653C(c, N, ch) =
  0x8188DD[(N + 6c) · 6 + ch] (c = N = 2: ch 1 → slot 1, 2 → slot 2); a
  kind 9 holds two channels in its nibbles. Slot s + 1 feeds output
  channel s (with c = N the copy table 0x8183F8 is all 0xFF).
* **Modules** (0x7268BF's switch, 16.8 parameters; sr = the mix rate
  0x8B44C6, 44100 from 0x724A69, clamped to the driver's range at
  0x724B24): every module pulls its inputs through vtable +0 (module, n,
  scratch, out, port) and returns n; float samples in the 16-bit range.
  - 0 input (0x72A164 binds the bus buffer; 0x72A138 copies n and advances).
  - 1 one-pole low-pass (0x72AABE): a = 2·p4 / sr, g = p6 / 256, input
    coefficient g·a, feedback g·(1 − a); 0x72D8B0: y = y₁·b + a·x.
  - 2 / 3 / 4 9-tap FIR (0x72D616 / 0x72D697): f = 2·p / sr (fractions of
    Nyquist): 2 low-pass at 2·p2/sr; 3 high-pass at min(2·p2/sr, 0.8); 4
    band-pass f₁ = min(2·p4/sr, 0.8), f₂ = 2·p6/sr. Taps c4 (centre) = f
    (3: f₁, 4: f₂ − f₁), c[4 − k] = sin(kπ·f) / (kπ) for k = 1..4 (4:
    2 sin(kπ(f₂ − f₁)/2)/(kπ) · cos(kπ(f₂ + f₁)/2)), × 0.54 − 0.46·cos(kπ/4)
    (k = 0 outermost … 4 centre); type 3 negates them and adds 1 to c4;
    scaled by 1 / |2Σc0..c3 + c4| (type 3: the alternating sum, i.e. unit
    gain at Nyquist; types 2 and 4 at DC, so the band-pass divides by its
    small DC gain). y = (x + x₋₈)c0 + (x₋₃ + x₋₅)c3 + (x₋₆ + x₋₂)c2 + (x₋₁ +
    x₋₇)c1 + x₋₄c4.
  - 5 resonator (0x72A817 / 0x72A6F8): f = p2, bandwidth p4, g = p6 / 256;
    r = 1 − p4·π / sr, c = 2r / (1 + r²) · cos(2π·f / sr), A = (1 − r²) ·
    √(1 − c²) · g; y = (x + 1e−20)·A + 2rc·y₁ − r²·y₂; passes x through
    when sr·0.63662 ≤ p4, f ≤ 0 or sr / 2 ≤ f.
  - 6 gain (0x72A6E4): p2 / 256.
  - 7 allpass (0x72A602 / 0x72A481): g = (p2 >> 1) / 127; delay
    0x72D567(sr, p4) = 16 · the smallest prime ≥ max(3, sr·(p4 / 16) /
    1000) samples (integer divisions; 0x7339E9 tries divisors 2 ..
    isqrt + 1, 0x7339D1), a 16-aligned zeroed buffer; v = g·d + x + 1e−20,
    y = d − g·v, d ← v (four samples a pass; the length is a multiple of
    16).
  - 8 sum (0x72A3D9): in1 into out, in2 into its cache, out += cache.
  - 9 splitter (0x72A2F3): +0x26 set → pull in1, copy to out and the cache,
    clear +0x26; else copy the cache and set +0x24; a set +0x24 at entry
    clears itself and sets +0x26. So of two reads per chunk the first
    computes.
  The series: cos 0x72A032 (1 − x²/2 + … + x¹²/12!, 2π subtracted while
  x > 2π), sin 0x72A0AE (to x¹³, x¹¹ stored as a float), √(1 + x)
  0x72D581 (binomial to x⁷: 1/2, −1/4, −1/2, −0.625, −0.7, −0.75,
  −0.7857 chained).
* **The hook** 0x72655A (0x8B4800, called at 0x7294CC after each mix
  chunk of ≤ 256 frames, 0x7295A8): idle 0x8B47C8 += 1; below 3000, for
  each bus (one, 0x8180B8) and FX: input → the bus buffer (0x8B47C4[bus],
  0x400 bytes), slot 1 then slot 2 pulled into 0x8B48D4 and added × the
  FX's channel gain (+0x68, 1.0; +0x80 = 1 would ramp it over 16 samples
  at 0x728957, but no code writes +0x68 / +0x80 after the load) into the
  output channel buffers 0x8B48F0[c] (0x72C360: dst += src·g); then the
  bus is cleared. A voice whose send (+0x38[bus]) is nonzero mixes its
  16-sample block × the send into the bus (0x729453..0x729499) and sets
  the idle count to 0; the load sets it to 3000, so a new graph waits for
  a send and a graph stops 3000 chunks after the last one.
* **Sends.** 0x723BC1(voice, bus, byte) stores byte / 127 and the driver's
  +0x74 (0x729E24) makes the voice's send byte / 127 × its volume (+0x48).
  The dry channel gains are (+0x5F second volume / 127) · +0x48 · pan
  (0x724F5D; 0x723C41 sets +0x5F). Callers: the player engine's GIN voices
  (0x473E60 → 0x473F0E, 0x474030 → 0x4740E3 / 0x474244): send byte =
  clamp(([0x82B078] · level) >> 23, 0, 127), second volume clamp([0x82A998]
  >> 8, 0, 127), level = the object's vtable +0x38(0); the interactive
  music (0x46C470 from 0x73A1A1 / 0x73A26A / 0x73BF6C on the stream
  handles 0x82B620[]); the event graphs' parameter kind 5 (0x720936 via
  0x817F44; the car banks set none). No music event sends, and the ABK
  engine graphs send p14 (wet B) in their voices' start blocks: Sound
  round 10.
* **Levels** (0x7F3778, 24 bytes a preset: {wet A, dry A, wet B, dry B}
  mB, fade-out ms, fade-in ms): 0–2 −1800, 3–4 −500, 5 0, 6 −300 (fade-out
  120), 7 −1800, 8–9 −1600, 10 −1200 (fade-in 120), 11 −900 (fade-in 80),
  dry 0 for all, else fades 140 / 60. (Sound round 6 read the table from
  0x7F3788, one field pair late.) 0x45DB90(preset, mB) (only with the
  tunnel object's +0xD7, set at 0x4832E3): offset 0x82A99C = mB, asked
  0x82AF2C = preset, +0xD4 = 1, the interpolator +0xB8 from 0 to 1 over
  the current preset's fade-out (≤ 0 → 10 ms), targets {0, 1, 0, 1}.
  0x4714A0 (per frame): +0xD6 = 0, step the fade; fading out and done and
  +0xAC clear (+0xAC = the fade not done at the last frame's end, 0x4718E3,
  so the switch comes a frame after the fade ends): +0xD6 = +0xD5 = 1, the
  preset 0x82B060 = the asked one, the fade 0 → 1 over its fade-in,
  current ← targets, targets = 0x467FF0(clamp(wet + trunc(offset), −10000,
  0)) / 32767 for the wets and 0x467FF0(dry) / 32767; fading in and done:
  current ← targets. Outside fades current moves to the target by at most
  0.25 a frame; during one the output is current + (target − current) ·
  fade. Outputs trunc(· 32767): 0x82B078 wet A, 0x82A998 dry A, 0x82B070
  wet B, 0x82B880 dry B; on alternate frames (0x82AF28) both wets + 1
  (capped 0x7FFF). 0x4713F0(mB) outside a fade: new offset, wet targets
  recomputed. 0x4633F0 reloads the graph (0x721515, 0x7219EC, 0x721A3C
  with 0x82AF60[0x82B060]) when +0xD6 is set, in game (mode ≠ 1, 6).
  Start values (0x45D990): preset 5, targets / current {0, 1, 0, 1},
  outputs {0, 0x7F, 0, 0}, +0xB4 = −1.
* **Choice.** SFXObj_Reverb's start (0x4632C0): front end preset 0 with
  0x89E7E5 or 0x836474 set, else 3 (no voice sends there in the clone);
  in game preset 8 then 0x45DB90(8, 0). SFXCTL_Tunnel_Update: entering a
  zone (+0x70 set from clear) → 0x45DB90(kind 5 → 5, 6–7 → 6, else 3, 0);
  leaving → 0x45DB90(region preset, 0). Then 0x47C6D0: +0xB0 = +0xB4; with
  the "Reverb" parameter map (0x82C5E8, name 0x78C2B0, bound) +0xB4 =
  field 0 and the offset = field 2 (ints) at the car's +0x60 / +0x64; out
  of 0..11 → 0 in the front end, else 9; a changed region outside a zone
  → 0x45DB90(0x7ED5D8[+0xB4] (identity), offset), else 0x4713F0(offset).
  L4RA's map: 32 rows, presets 0 (row 0), 8, 9, 10, 11, offsets 0, +500 …
  +1200 and −500 mB.
* **Clone.** `reverb::Graph` parses the stereo config and runs the modules
  per chunk (module-owned buffers instead of the shared scratch pair; a
  missing input reads zeros where the original reads its scratch: no
  preset has one); the mixer renders in chunks of ≤ 256 frames, sums the
  engine voices × their send into the bus, runs the graph and adds slot 1
  to the library's channel 0 (the right speaker) and slot 2 to channel 1;
  the graph runs at the mix rate with the same formulas (bit-for-bit
  delays and coefficients at 44100; at 48000 the delays are its primes).
  Setup in f64 with the original's float stores; per-sample arithmetic in
  f32 (the x87 keeps intermediates wider: differences of an ulp,
  **inferred** inaudible). `ReverbCtl` ports 0x45DB90 / 0x4713F0 /
  0x4714A0 / 0x47C6D0 and the zone entry / exit with the drive-by
  tunnel's zones (`driveby::Tunnel`). Not ported: the music streams'
  sends, the voice-stop declick into the bus (0x72886C on 0x8B4924), the
  16-sample send ramps (0x728B48). Measured (the maintainer's probe `reverb_ir`, 8 s
  impulse at 44100): T60 Simple_Tunnel 4.8 / 4.0 s (channels 0 / 1),
  Simple_Tunnel_Sml 0.6 / 0.5, Garage 0.7 / 0.45, Hills 1.1, Hills_Close
  0.6 / 0.7, City_Open 0.86, City_Dense 1.1 / 1.3, Alley 0.5 / 0.4; the
  Car_Show presets are filters with no tail; cost 0.08–0.3 % of one core
  per preset. In game (`NFSU2_AUDIO_WAV`, explore shot, 3 s with the
  throttle held): open road (region 9, City_Open, wet A 5185) send byte
  11, wet peak ≤ 1186; in the tunnel at (−205, 83, 20) (zone kind 5,
  Simple_Tunnel, wet A 32767) send byte 56–86, wet peak 16257, capture
  peak 20499 / 8577 and RMS 3998 / 2333 against the open road. **Open**:
  a capture of the original's output for comparison (not made).

### Sound round 7 (fesound)

**Verified** (the disassembly and REA / Ghidra) unless marked. Clone:
`audio::fe_rules` (the rules), `Game::native_menu_sound` (which screens
use them), `Game::dialog_ready_sound`, test `tests/fe_sounds.rs` (the
screen / key / state → id table).

* **Call-site map.** The 239 calls of PlayFESound 0x458520,
  FE_PlayEventSound 0x50D150(_, event, control), FE_PlaySoundId
  0x50D180(id), 0x51DE40(event, control) and 0x51DE70(_, event) (column
  0) — 171 of the four helpers, 68 direct — by the function holding them
  and the screen it serves (the vtable that holds the function, its
  constructor, the descriptor naming the package; "via" = only called from
  that function):
  0x497B00 UI_EngageMessageDialog_Handler: UI_EngageMessageDialog (0x497B41, 0x497B51)
  0x498F00: UI_EngageEventDialog (0x498FC8, 0x498FF2, 0x499059, 0x4990F7, 0x499152)
  0x499690 UI_EngageShopDialog_Handler: UI_EngageShopDialog (0x4996C3, 0x499727, 0x49977A, 0x4997BF, 0x499805, 0x49987A, 0x4998C6)
  0x499C00 UI_EngageRaceDialog_Handler: UI_EngageRaceDialog (0x499C4E, 0x499CF4, 0x499D9C)
  0x499FA0 Showcase_Leave: — via 0x4CE770 (0x49A029)
  0x49B980: — (0x49B99C)
  0x4A2E20: UI_PC_LAN (0x4A2ED6, 0x4A2EFF)
  0x4A6030 Garage_HydraulicsUpdate: — via 0x4D82D0 (0x4A652E, 0x4A6541, 0x4A6614)
  0x4AB9F0: — via 0x4C8AC0 (0x4ABA01, 0x4ABA16, 0x4ABA25)
  0x4AFFA0: UI_CareerCarLot (0x4B006B)
  0x4B04C0: UI_CareerCarSelect (0x4B0532, 0x4B0594)
  0x4B0850 MapWidget_StepSelection: UI_InGame_WorldMap (0x4B0984)
  0x4B1060: UI_InGame_WorldMap (0x4B10A8)
  0x4B1A50: — via 0x4B1AE0 (0x4B1ABC)
  0x4B1E60: UI_Status_Master (0x4B1E95, 0x4B1EDD)
  0x4B1F20: UI_Status_Master (0x4B1F56, 0x4B1F8D)
  0x4B2110: UI_SMS_Mailbox (0x4B2141, 0x4B2183)
  0x4B51D0: UI_VirtualKeyboard (0x4B524E)
  0x4B59D0: — via 0x4CE770 (0x4B5A91)
  0x4B5C70 Showcase_ControlCar: — via 0x4CE770 (0x4B5C83)
  0x4B5EA0 Showcase_CycleCameras: — via 0x4CE770 (0x4B5EAD)
  0x4B99D0: UI_OLForgotAccountName (0x4B9AB2)
  0x4C1D40 FEGarage_SetupPartView: — via 0x4C2100 (0x4C1DB1)
  0x4C5580 UG_LS_Splash_Handler: MW_LS_Splash, UG_LS_Splash (0x4C55B5)
  0x4C8410 HUD_MinimapGPSText: — via 0x4DA640 (0x4C85A4)
  0x4C9170 HUD_QueuePrompt: — via 0x4CA020, 0x4CA670 (0x4C91FA)
  0x4CBDA0: UI_InGame_WorldMap (0x4CBDF2)
  0x4CC140: UI_CareerWorldMap (0x4CC1AB)
  0x4CCB40: UI_SMS_Mailbox (0x4CCB8F, 0x4CCBE2)
  0x4CCC10 UI_SMS_Mailbox_Handler: UI_SMS_Mailbox (0x4CCC85, 0x4CCD0C, 0x4CCE09, 0x4CCEBE)
  0x4CE590: UI_VirtualKeyboard (0x4CE5C0)
  0x4CE5D0: UI_VirtualKeyboard (0x4CE5F8)
  0x4CE610: UI_VirtualKeyboard (0x4CE637)
  0x4CE770 UI_MagazinePhoto_Ctor: — via 0x4E3500 (0x4CEB54, 0x4CEC36, 0x4CEEA5, 0x4CEF20)
  0x4CF460 Showcase_TakePicture: — via 0x4CE770 (0x4CF46D)
  0x4CF750: — via 0x4E3710 (0x4CFAD5)
  0x4D0470: UI_OL_Disconnect (0x4D04A3, 0x4D04C2, 0x4D04E1)
  0x4D4950: UI_OLEAMessenger (0x4D497C, 0x4D49B2)
  0x4D82D0 FEGarage_UpdateCarTurn: GarageMain (0x4D836B, 0x4D83ED, 0x4D8658, 0x4D866C, 0x4D86DC, 0x4D8C15)
  0x4D90D0: MU_QuickRaceCarSelect (0x4D9164)
  0x4DA9A0 HUD_EngagePrompt_Update: — via 0x4EC5A0 (0x4DAABD, 0x4DAB42, 0x4DAB54)
  0x4DFC50: UI_CareerCarSelect (0x4DFD88, 0x4E00F6)
  0x4E08E0: UI_InGame_WorldMap (0x4E0957, 0x4E09C8, 0x4E09E8)
  0x4E0E50 UI_WorldMap_NewStage: UI_CareerWorldMap (0x4E0ECA)
  0x4E0F60 UI_CareerWorldMap_Handler: UI_CareerWorldMap (0x4E0FD8, 0x4E10A9)
  0x4E3390: — via 0x4EFDA0, 0x4EFDC0 (0x4E3452, 0x4E3485)
  0x4E6770: UI_OLEAMessenger (0x4E6910)
  0x4E8530: — via 0x4F2CA0 (0x4E870D)
  0x4EB220: MU_QuickRaceCarSelect (0x4EB263, 0x4EB32D)
  0x4ED9E0: UI_CareerCarLot (0x4EDCDB, 0x4EDD18)
  0x4EFA10 UI_QRTrackSelect_Handler: UI_OL_ViewTrack, UI_QRTrackSelect (0x4EFA7E, 0x4EFD08)
  0x4F0370: UI_OLSelectPersona (0x4F0573)
  0x4F3DE0: UI_PostRaceStats (0x4F3E49)
  0x4F9340: UI_Options_PC_Controller (0x4F93BE, 0x4F9495, 0x4F9519, 0x4F95D4, 0x4F96B0)
  0x4FA5B0 UI_VirtualKeyboard_Notify: UI_VirtualKeyboard (0x4FA641, 0x4FA675, 0x4FA6B0, 0x4FA6DE, 0x4FA73D, 0x4FA757, 0x4FA771, 0x4FA784, 0x4FA7B0, 0x4FA7C3, 0x4FA7D6, 0x4FA85A, 0x4FA88A, 0x4FA952)
  0x4FB680: UI_PC_LAN_ServerSelect (0x4FB79C, 0x4FB7EE)
  0x4FBBC0: — via 0x4FD6C0 (0x4FBD0C)
  0x4FBE20: UI_PostRaceStats (0x4FBE85)
  0x4FC1F0: UI_OLCarSelect, UI_QRCarSelect (0x4FC338, 0x4FC3AF, 0x4FC3C5, 0x4FC400, 0x4FC43E, 0x4FC47A, 0x4FC49B, 0x4FC4B1, 0x4FC64C, 0x4FC72A, 0x4FC753, 0x4FC786, 0x4FC7AF, 0x4FC7CC, 0x4FC7FA, 0x4FCBB2, 0x4FCC5B, 0x4FCC99, 0x4FD022)
  0x51DCE0 FEWidget_Handle: — via 0x52CC80 (0x51DD91)
  0x521290 FE_PlayInstallSound: UI_CustomHUDOverlay, UI_GenericParts_Browser, UI_IcePartsOverlay … (0x5212AB)
  0x5241F0: EA_Trax_Jukebox (0x524228)
  0x529590 EngageState_Run: — via 0x4B5D80 (0x5295A7)
  0x5295C0 EngageState_Stop: — via 0x4B59D0 (0x5295D6)
  0x529710 SMS_OpenScreens: — via 0x529760 (0x52974B)
  0x52D660: UI_OLCreateUser, UI_OLEALogin, UI_OLUseExisting … (0x52D697)
  0x52DA00: UI_OLCreateUser, UI_OLFilters, UI_PC_Customize_Options … (0x52DABB)
  0x52E480: — via 0x538140 (0x52E4DE)
  0x531500: — (0x531562, 0x5315B7)
  0x53A320: UI_GenericParts_Browser (0x53A32D)
  0x53A3E0: UI_GenericParts_Browser (0x53A3ED)
  0x53DEC0: EA_Trax_Jukebox (0x53DFEC)
  0x543D50 IconScrollerMenu_Handler: LS_LangSelect, UI_CareerCrib, UI_ChooseCustomizeCategory … (0x543FC8)
  0x547580 Shop_PurchaseSetup: UI_ChoosePaintCategory (0x5477A9)
  0x54C060: — (0x54C223, 0x54C265, 0x54C2A7, 0x54C2CD, 0x54C2FF, 0x54C335, 0x54C3AE, 0x54C3D0)
  0x54FA00: UI_GenericParts_Browser (0x54FA46, 0x54FA74)
  0x550F20: EA_Trax_Jukebox (0x550F62, 0x550F8A)
  0x556030 UI_PerformanceTunin: UI_BuyPerformanceParts, UI_OLAgeVerif, UI_OLCreateUser … (0x55648B)
  0x556940: UI_ICEMAIN (0x55694E)
  0x559250 UI_VinylBrowser_AskAddVinyl: UI_GenericParts_Browser (0x5593DE)
  0x559AC0: UI_ChooseSpinner, UI_Rims_Browser (0x559C2D)
  0x55B5D0 MenuScreen_BaseNotify: UI_ChoosePerformancePackage, UI_ChooseUniquePart (0x55B911, 0x55BC2C)
  0x55CAD0: UI_NeonPartsOverlay (0x55CB28, 0x55CB53)
  0x55D420: — via 0x5632A0 (0x55D4B7, 0x55D558, 0x55D5A0)
  0x55D670: UI_CustomHUDOverlay (0x55D6C1, 0x55D6EC)
  0x55DE10 FE_MouseClickWalk: UI_PC_Help_Bar (0x55DEAA)
  0x55EBB0: UI_GenericParts_Browser (0x55EC15, 0x55EC46)
  0x55EDE0 UI_PartsBrowser_EnterCategory: UI_GenericParts_Browser (0x55EDEC)
  0x55EF90 UI_PartsBrowser_LeaveCategory: UI_GenericParts_Browser (0x55EF9E)
  0x55F750: UI_Rims_Browser (0x55F7AA, 0x55F7D8)
  0x55FCF0: UI_ChooseSpinner, UI_Rims_Browser (0x55FCFF)
  0x5605C0: UI_ICEMAIN (0x5605CC)
  0x560930: UI_CustomHUDOverlay, UI_IcePartsOverlay (0x560981, 0x5609AC)
  0x5614C0: UI_ChooseSpinner (0x5614CE)
  0x5632A0: UI_ChooseVinylLayer (0x56356D, 0x563725, 0x563A06, 0x563AA6) — corrected in round 8: these four are UI_Paint's handler 0x563420 (vtable 0x79D8F4), which Ghidra folds into 0x5632A0
  0x5650D0: UI_PerformanceTuning_Drivetrain, UI_PerformanceTuning_Sliders (0x56511C, 0x56512D, 0x565142, 0x565155, 0x565186, 0x56519B, 0x5651F8)
  0x565DA0 Jukebox_UpdatePlayMode: EA_Trax_Jukebox (0x565DDA, 0x565E1A)
  0x5664A0 UI_GenericParts_Browser_Message: UI_GenericParts_Browser (0x5664F0, 0x566643, 0x56666A, 0x566763, 0x56685A, 0x566A1E, 0x566C05, 0x567325)
  0x567A60: UI_Rims_Browser (0x567D4F, 0x567E56)
  0x568750 UI_ICEMAIN_Message: UI_ICEMAIN (0x568783, 0x568932)
  0x569EF0: UI_BuyPerformanceParts (0x56A14F, 0x56A20C)
  0x56B1D0 UI_Jukebox_Handler: EA_Trax_Jukebox (0x56B22A, 0x56B269, 0x56B31A)
  0x56B3C0 UI_DecalMain_Message: UI_DecalMain (0x56B627, 0x56B7EF)
  0x579CC0: — via 0x57B8C0 (0x579CE6)
  0x582BE0: — via 0x583B60 (0x582CAC)
  0x584F50: — via 0x586630 (0x58500F)
  0x58B3A0: — via 0x58C6A0 (0x58B726)
  0x58F750: PC_OL_GameRoom, UI_OLCarSelect, UI_OLGameRoom … (0x58FC12)
  0x61F830 CarPart_RequestAnim: — via 0x499F10, 0x4B59D0 (0x61F866, 0x61F87B, 0x61F973, 0x61F9C6, 0x61FA64, 0x61FA77, 0x61FAFC)
  The 0x58xxxx functions (0x579CC0 … 0x58F750) are the online lobby's;
  0x61F830 CarPart_RequestAnim plays the garage hood / trunk / door
  animations' ids 0x4C–0x53 (76–83, FE_MB 53–60; the clone has no such
  animations).
* **Event table rows.** 0x50CF40 fills 26 rows of 27 (0x837FE0, 0xAF
  dwords + a word of −1) and then rows 1, 2 and 4–19 only, so events 0,
  3 (SUMMON_CUSTOMIZE) and 20–25 play nothing; 0x51DE70(_, 0x18) in the
  track select's step (0x4E3390) reads row 24: silent.
* **List screens** (base handler 0x556030, vtable 0x795CB0 +0x38; the
  list ctor 0x552790 sets +0x148 "active" and +0x149 "sounds"):
  UI_Options 0x49C690 (all keys to the base while +0x14D is clear),
  UI_PauseOptions 0x4A8010, UI_PC_Customize_Options 0x49C540,
  UI_Wheel_Options 0x49CBB0, UI_QRModeOptions 0x4EF620,
  UI_PerformanceBrandSelect 0x55C940, UI_BuyPerformanceParts 0x569EF0
  (with its own Up / Down, not ported) and the online screens pass the
  message to it. Up / Down step (0x5390F0 or 0x539380 with +0x144) and
  play table [UP] / [DOWN] column 7 on every press while +0x148 is set;
  Left / Right with an enabled current row (+0x54, +0x28): the row's step
  (vtable +4), then 0x50D460 maps the message to its default event and the
  row's +0x38 filters it, played on the row's +0x3C column; Accept
  0x0C407210 / Back 0x911AB364: the row's +4, then (no row, or an enabled
  one) SELECT / BACK on the screen's +0x30; 0x406415E3 SELECT with an
  enabled row. Row classes in the option builders (0x4B3110..0x4B8B4D,
  0x4B6510): sliders (+0x38 0x50D6F0, +0x3C 0x40CB20 = column 1): Left
  silent when the value before the step (+0x80 = the model's +0x2C, step
  0x4FF480) was the minimum (+0x7C), Right when it was the maximum
  (+0x78); value rows (0x50D500 = the default event, 0x408CD0 = column 2):
  always; buttons (0x50D800: only 0x406415E3 → SELECT, column 4 0x40CBB0):
  silent on Left / Right. UI_QRModeOptions' rows are all value rows;
  UI_PerformanceBrandSelect's (option vtable 0x79B6B0) too. The triggers,
  Button1 and Start reach no sound there.
* **UI_PerformanceTuning_Sliders / _Drivetrain** (0x5650D0; ctor
  0x55AC20 clears +0x149 at 0x55ACED, so the base is silent): Down id 1,
  Up 0, Left 9, Right 8 (direct, every press), Back 0x51DE70(10) (id 5),
  0xC519BFC0 (Button1) id 11, 0xC519BFC3 0x51DE70(2) (SUMMON_HELP, id 10);
  Accept the row's +0x24, silent.
* **Engage dialogs.** UI_EngageRaceDialog 0x499C00: Right focuses
  Cancel_Button (0x63857DE0) and plays RIGHT (column +0x30) unless it is
  focused already; Left likewise Accept_Button (0xD72F002A) / LEFT;
  Accept and Back silent; the package's 0x35F8620B (the "ready" its
  script sends) 0x50D180(15). UI_EngageShopDialog 0x499690: Left / Right
  play on every press; Accept on Cancel BACK; Accept on Accept BACK when
  0x501690() and 0x85B940 (the challenge retry, then 0x5150D0), else
  0x499640 silent; Back BACK in the retry (0x501640), else silent; ready
  0x51DE70(10) (id 5). UI_EngageEventDialog 0x498F00 (no challenge
  running, 0x863444 / 0x863450): Left / Right only when they move the
  focus; Accept on Cancel BACK (0x511280, back), on Accept_Button SELECT
  (0x541130), on 0xB8A7C6CE SELECT when 0x495270(); Back silent; ready
  moves the focus only. UI_EngageMessageDialog 0x497B00: ready
  0x50D180(15), 0x406415E3 (the clone's Accept) 0x51DE70(10), Back
  silent. UI_SMS_Mailbox 0x4CCC10: ready 0x50D180(15); Left /
  PAD_LTRIGGER (0x4B2110(−1)) and Right / PAD_RTRIGGER (0x4CCDD1) play
  [UP][0] (id 0) when they change the mailbox (not at the first / last);
  Up / Down (0x4CCB40) [UP][0] / [DOWN][0] when they move (Down also
  needs the next node's +0x14); Accept silent (state 2); Back BACK
  (+0x30); Button1 on a row SELECT (+0x30) with the delete question
  (0x5587A0 0xF0A6E6B5). The clone's mailbox now also steps on the
  triggers. The ready sounds: `Game::dialog_ready_sound`, at the
  package's own 0x35F8620B time (`frontend::host_time`).
* **UI_CareerWorldMap** 0x4E0F60: Up / Down 0x4CC140(0 / 1) (UP / DOWN
  column 7 only off the first / last event), Left / Right 0x4E0E50(0 /
  1) (LEFT / RIGHT column 7 only when the stage list moves: +0xB0 vs the
  ends +0x9C / +0xA0), Accept SELECT (+0x30) with the race question
  unless 0x51F7F0() is 1, Back BACK (both not in the opening frame,
  +0x4C ≠ 0x8651A8).
* **UI_QRTrackSelect** 0x4EFA10: Accept 0x50D180(4), Back 0x50D180(5)
  (unless 0xDE6EFF34 is already in state, 0x537D50); Left / Right
  0x4E3270(±1) → 0x4E3390: row 24, silent.
* **Generic dialogs** (GenericDialog_SMALL / MED …): their descriptors
  (0x7F835C, 0x7F8340) have no factory, and no handler above serves
  them: **open** which code (FEWidget_Handle 0x51DCE0 on the global widget
  list 0x8637B0, dispatched through 0x52CDD0 in the table 0x79BCE0, is
  the candidate) plays their keys; the clone keeps the BUTTON column.
* **Still on the control-kind rule** (open per screen, mapped above):
  the car lot / car select (0x4ED9E0, 0x4DFC50, 0x4AFFA0, 0x4B04C0), the
  QR car select (0x4FC1F0) and GarageMain's turn (0x4D82D0, CARSEL
  column 23), the in-game map (0x4E08E0, 0x4B0850), UI_Status_Master
  (0x4B1F20, 0x4B1E60), UI_PostRaceStats (0x4FBE20, 0x4F3DE0), the
  controller page (0x4F9340), the jukebox (0x56B1D0, 0x565DA0,
  0x53DEC0, 0x550F20, 0x5241F0), the shops' browsers and overlays (F10,
  F12–F14, F21: 0x5664A0, 0x567A60, 0x568750, 0x56B3C0, 0x5632A0,
  0x55CAD0, 0x55D670, 0x560930, 0x569EF0), the showcase (0x4CE770 and
  its states), the MC screens, the rewards and results (no call sites:
  their sounds, if any, come through the widget path).
* **CARSFX_TrafficBottomOut.** Its knock volume: 0x47EF40 tests the
  car's mover (car +0x28, 0x5F46B0) — 1 (physics) sums the wheel
  compressions, any other gives 127 (push 0x42FE0000 at 0x47EFB3). A
  traffic car runs the TrafficMover (2), or the MeloMvr (3) after a
  contact, so its knocks are always 127 (record 0x4E + n + 4, or 0x56 +
  n + 4 tipped). Clone: `roadwind::bottom_out` shared by the player's
  BottomOut and `traffic::OtherCars` (class 4 slots 0 / 2 for level and
  azimuth; the second knock by the race's camera event kind, **inferred**
  for the traffic car's nearest director). Test
  `traffic_bottom_out_knocks_on_landing`.
* **SFXObj_Ambience.** Its vtable 0x78B158 has no update: slot 0
  the deleting destructor 0x46EC10, 1 / 2 its name and id (0x461FA0 /
  0x461FB0), 3 0x45BAC0 (−1), 7 0x475100 (it becomes the music object's
  +0xD0), 8–13 empty (0x4022C0 `ret` / 0x4031C0 `ret 4`), 14 / 15 the
  shared mix-map level / azimuth 0x45E390 / 0x45E3B0. So it only carries
  the score's level (slot 1) and fade (field 2), already ported.
  0x47E490 (named its update in round 4) is another class's deleting
  destructor (0x475150).

### Sound round 8

**Verified** (the disassembly, objdump at the exact addresses, REA /
Ghidra, the packages' `PkgR` / object responses read by the clone's FNG
parser) unless marked. Clone: `audio::fe_rules` (round 8 part),
`Game::native_menu_sound`, `Game::menu_notify_sound`, test
`round8_screen_key_state_table` in `tests/fe_sounds.rs`.

* **FEng's sound notification (the "FEng default").** FEngine's pad
  pass 0x54D4C0 hands each pad message (tables 0x7F7D20 / 0x7F7D50 /
  0x7F7D78; Accept as 0x0C407210 to the focused object, else 0x406415E3
  to the package) to the focused object when it has a response for it
  (object +0x3C), else to the package when its `PkgR` (+0x5C) has one; each
  delivery also queues the message to target −5 (0x51AA20). The queue
  pump 0x541FD0 passes target −5 to the game interface's slot 7
  (0x52CDD0, vtable 0x79BCE0 → 0x52CCA0 over the package list 0x8637B0,
  node +0x14 the screen, +0x18 the package), i.e. FEWidget_Handle
  0x51DCE0 on that package's screen: with the screen's byte +0x2C set it
  maps the message to a default event (Accept 8, Up 6, Down 7, Back 10,
  Left 4, Right 5, Start 9, else 0), calls the screen's vtable +0xC
  (message, …, default) and plays table[result][+0x30]. FE_MoveFocus
  0x52B3B0 notifies 0x55D1E635 / 0xABC08912 the same way (default 0).
  No package has a response entry of type 3 or a script event to −5, so
  pad deliveries are the only source. The base constructor 0x551DF0 sets
  +0x2C = 1 and +0x30 = 0; the list (0x552790) and icon scroller
  (0x552340) constructors and the car lots (0x4F9930, at 0x4F9A60 /
  0x4F9BDF) clear +0x2C; UI_QRTrackSelect's (0x4E2FDE) too. 63 screen
  vtables have +0xC = 0x570A80 (returns 0, silent). The filters that
  answer: 0x49CB40 (engage race / shop / event dialogs: 0x50D1A0(msg,
  default, 1) — events below 6 as given, Up / Down 0, Back with the flag,
  the rest as given), 0x4E2280 (UI_QRCarSelect: LTrigger 6, RTrigger 7,
  Accept 0, split screen by player, else 0x50D1A0), 0x4CFAC0 (UI_Options,
  whose constructor 0x4CF750 sets +0x2C = 1: 0x50D180(11) when +0x14E —
  set by the Player builder 0x4B7CD0 at 0x4B817D — outside mode 8;
  returns 0), 0x510EB0 (Left / Right 0: UI_MagazineView 0x553BF0,
  UI_PerformanceDyno_Results), 0x4A4750 → 0x50D230 (the directions 0:
  UI_PostRaceReward), 0x4A3F00 (UI_PostRaceResults: Back 0, Button1 8,
  else 0x50D230), 0x49DFC0 → 0x50D1D0 (Left / Right 0: UI_SponsorPopup,
  MC_List; the customize / wheel options have it but +0x2C = 0), 0x49CA50
  → 0x50D1F0 (Back only: Credits), 0x5102F0 (UI_PerformanceTuning_Graph:
  the directions and Back as given, Button1 10, 0xC519BFC3 2, else 0),
  0x5103B0 (the dyno chart: 0 while +0x4D, else as 0x510EB0), 0x4A33D0
  (MC_Main, by its mode +0x8C: 0x50D1A0 / 0x50D1D0 / 0x50D210 accept only
  / 0x50D1F0 back only, Accept 8, all gated on +0x64), 0x49DE90
  (UI_OLISPConnect). The rest of the list (online, LAN, crib, pause,
  tuning-category screens) is in the scan, not mapped to the clone.
* **Round 7 corrections from it.** The engage dialogs keep +0x2C = 1
  (their constructors store the vtables at 0x4CE73F, 0x4B532F and
  0x4B5291 right after 0x551DF0 and never write +0x2C):
  their packages answer Back (race / shop also PAD_ACCEPT) and the buttons
  0x0C407210 (Cancel_Button also Left, Accept_Button Right), so Accept
  plays SELECT (with a button focused, or none on the race / shop dialog)
  and Back BACK besides the handlers' sounds; Left / Right reach only the
  button that answers them, where the handler's own id plays too (one
  sound). UI_Options' Player category adds id 11 to every answered key
  (every row answers Left / Right: verified in round 9). UI_QRTrackSelect stays
  as round 7 (+0x2C = 0).
* **UI_CareerCarLot** (constructor 0x4F9930: +0x30 = 12 CARLOT, +0x2C =
  0, +0x1BC = 1; handler 0x4ED9E0): Left / Right → 0x4AFFA0: when the
  current car (+0x58) is not the first / last list node, LEFT / RIGHT on
  +0x30 (2 / 3) and the step (the list does not wrap); Up / Down are not
  handled; Accept (0x0C407210 / 0x406415E3, with +0x1BC) asks (states 2 /
  3 / 4 / 5) and plays SELECT; Back asks (0xD76E0B5A, or "Rachel needs her
  car back") silently; the leave question's Yes 0x51123592 plays BACK
  (0x50D150(10, +0x30)). **UI_CareerCarSelect** (constructor at 0x4F9AFD,
  +0x30 = 12, +0x2C = 0; handler 0x4DFC50 → 0x4B04C0): Left / Right as
  above; Accept opens its dialogs silently; Back BACK; the "drive instead"
  Yes (0xD05FC3A3) BACK. The clone's lot no longer wraps or steps on Up /
  Down.
* **UI_QRCarSelect** (handler 0x4FC1F0, vtable 0x797714, filter
  0x4E2280, +0x2C = 1, +0x30 = 0): the handler plays 0x50D180(4) on Accept
  and 0x50D180(5) on Back once ready (+0x1A7, the package's 0x35F8620B);
  Left / Right (0x4E2590) and the triggers (0x4EED10) play nothing
  themselves; the package answers Left, Right, both triggers, Back and
  Start, so the notification plays LEFT 2 / RIGHT 3 / UP 0 (LTrigger) /
  DOWN 1 (RTrigger) / BACK 5 / START 6 on every press. Button1 plays
  SELECT on the paths that open a screen or the "limit" message (not
  ported: the clone's Customize from here is not built).
* **GarageMain's swing sound** (0x4D82D0): starting the swing (state 3
  with +0x162 / +0x161, or state 1 when the loader's car is in with
  +0x2CE0) plays 0x51DE40(5, 0x17) when +0x148 is 0, else 0x51DE40(4,
  0x17): RIGHT 69 / LEFT 70 on CARSEL. +0x148 = 1 for a Left step: set by
  UI_QRCarSelect's step 0x4E2590 (0x4E2645… / 0x4E274C) and the crib's
  select (0x4A7900 / 0x4A7930 / 0x4A7960 from its tick, 0.3 s after the
  key); the constructor (0x4EACF3) sets 0 and UI_CareerCarLot never
  writes it, so the career lot swings and sounds by the last direction
  set. Clone: `Game::garage_left`, played at the clone's car change (the
  game's after the 0.3 s and the load: **inferred** timing); the swing's
  turn now follows +0x148 too.
* **UI_InGame_WorldMap** (handler 0x4E08E0; filter 0x570A80): with the
  GPS off (0x4AE930) Left / Right → 0x4B0850(3 / 2), whose 0x51DE40(4 /
  5, 0) plays when the selection (+0x14) changes; Up / Down → 0x4B1060(0
  / 1): with a legend row (+0x5C) 0x50D180(2) / 0x50D180(3) and the step,
  without one 0x4B0E70 silently; LTrigger / RTrigger → 0x4CBDA0(3 / 2):
  0x50D180(2) / (3) and the zoom; Accept 0x0C407210 (0x4B0EF0) and Button1
  (0x4B10C0) SELECT; Back BACK.
* **UI_Status_Master** (handler 0x4B1F20, filter 0x570A80): ready
  0x50D180(15) (`dialog_ready_sound`); Accept / 0x406415E3 / Back SELECT /
  BACK on +0x30 unless +0x100 is this tick; Left / Right → 0x4B1E60:
  LEFT / RIGHT on column 7 unless on the first / last page.
* **UI_PostRaceStats** (handler 0x4FBE20 → 0x4F3DE0; the list
  constructor clears +0x2C): Left / Right clamp +0x184 to 0 … racers − 1
  (0x890118 +0x24) and play 0x51DE70(_, 0x50D260(msg, 4, 5)) when it
  moved; Back 0x51DE70(_, 10).
* **Generic dialogs** (closes round 7's open item): ShowDialog 0x558030
  copies the 0x338-byte request and pushes the package with the factory
  0x5542B0 (0x390 bytes, constructor 0x5541D0, vtable 0x79C894: handler
  0x54C060, filter 0x570A80, +0x30 = 0; the request at +0x58, so +0x380
  = the count (+0x328), +0x384 = the back flag (+0x32C), +0x370 the back
  message). 0x54C060: Accept 0x0C407210 from a button (0xB8A7C6CC / CD /
  CE, with a count) SELECT and the button's message; Up / Down UP / DOWN
  with three buttons; Left / Right LEFT / RIGHT on every press with two;
  Back BACK and the back message only with the flag. The requests:
  0x558690 (two buttons, count 2, flag = its 8th argument: 0x5587A0 and
  0x558760 pass 1, 0x558830 / 0x558870 0) and 0x5584A0 (one button, flag
  = its 6th: 0x558580 1, 0x558620 / 0x5585F0 0). Clone: `Dialog::back`
  (false on 0x558620's "Deactivating GPS"; the clone's other dialogs keep
  Back, **inferred** per call site) and `Dialog::answer_sound`.
* **UI_Paint** (constructor 0x56AEC0: vtable 0x79D8F4, +0x30 = 10 PAINT;
  handler 0x563420): with +0x188 clear (the slot strip) and +0x189 the
  key goes to IconScrollerMenu 0x543D50 (LEFT / RIGHT off the ends,
  SELECT / WRONG, BACK on column 10); 0x406415E3 on a locked slot also
  0x51DE70(_, 0x13). In a slot (+0x188): Up / Down UP / DOWN on +0x30 (39
  / 40) then 0x55D2E0, Left / Right LEFT / RIGHT on PAINT_TYPE (41 / 42)
  every press; 0x406415E3 SELECT with swatches (+0x158 not empty); the
  triggers 0x55D0E0 and LEFT / RIGHT on PAINT_TYPE when the finish
  (+0x174) is 0x02DAAB07 / 0x03437A52 / 0x03797533; Back 0x55CEA0
  silently.
* **UI_GenericParts_Browser** (handler 0x5664A0, filter 0x570A80): Left /
  Right 0x54FA00(∓1) on the strip (+0x4C): LEFT / RIGHT on +0x30 unless at
  the first / last (0x4AEB90 / 0x4AEBC0); Up / Down 0x55EBB0(∓1) on the
  part list (+0xB0) likewise UP / DOWN; Accept on a locked item WRONG,
  with an empty list nothing, else SELECT through every branch (0x55EDE0
  id 4, 0x53A320 / 0x53A3E0 id 4, the questions 0x51DE40(8, +0x30));
  Back in a category (+0x1D9) 0x55EF90(1) id 5, outside one BACK when
  0x83898C (0x552CD0's mode, Customize) or +0x1D7 / +0x1D5, else the
  leave question silently. The clone's other shop browsers on the same
  struct (rims, spinners, ICE, neon, decals) take this rule
  (corrected in round 9: the rims, ICE, neon and decal handlers read key
  by key), and no browser steps the strip from inside a category
  (round 9: 0x54FA00 returns while +0x1D9 is set).
* **UI_Options_PC_Controller** (handler 0x4F9340, filter 0x570A80) with a
  row (+0x64): Up 0x4ED380(−1) and UP on column 7 unless the row is the
  first (+0x54) or +0x5C (then the row is dropped silently); Down
  0x4ED380(1), DOWN column 7; Left / Right 0x4CB9E0 and LEFT / RIGHT
  column 7; Accept sends 0x406415E3 to the row (binding) and Back leaves,
  silently; without a row Down picks the first and Left / Right change
  the device silently (the clone always has a row).
* **EA_Trax_Jukebox** (handler 0x56B1D0, constructor 0x553C80: +0x108 =
  0; filter 0x570A80): Up / Down 0x550E70 (silent; the reorder mode's
  0x550F20 plays UP / DOWN); Left / Right 0x53DEC0: [LEFT][2] / [RIGHT][2]
  (0x83804E / 0x838069) every press; LTrigger / RTrigger 0x5241F0:
  [LTRIGGER][0] / [RTRIGGER][0] (0x8381AB / 0x8381C6; round 4's F11 named
  0x565DA0, which is the mouse 0x9803F6E2's); 0x406415E3 the preview
  (0x480C10) and SELECT; Back BACK; Button1 0x531640 and table
  [SELECT][+0x30].
* **UI_ShowCase_Preview** menu (0x4CEA70): Up / Down UP / DOWN when
  +0x54 moves within 0 … 3 (0x4CEB54 / 0x4CEC36); Accept SELECT (Control
  Car 0x4B5C70, Cycle Cameras 0x4B5EA0, Take Picture 0x4CF460, 0x4CEEA5);
  Back BACK (0x4CEF20). Corrected in round 9: 0x4CEEA5 / 0x4CEF20 are the
  picture state's, the menu has no Back and Show Off no SELECT.
* **The handlers without call sites** sound by the notification alone
  (`fe_rules::quiet_screen`, `cover`): UI_PostRaceReward Accept / Back;
  UI_PostRaceResults Accept and Button1 SELECT, Back nothing;
  UI_SponsorPopup, UI_PerformanceDyno_Results Accept / Back; MC_List
  Accept / Back / Up / Down; Credits Back; UI_PerformanceTuning_Graph the
  directions, Back and Button1 (BACK); UI_MagazineView Accept / Back.
  UI_ChooseCustomHUD (0x5644E0, filter 0x570A80) is silent. UG_LS_Splash
  (0x4C5580): Start / 0x406415E3 once ready (0x836495) 0x51DE70(_, 9),
  id 6.
* **Still open:** MC_Main and its messages (filter 0x4A33D0 by mode
  +0x8C and +0x64, set from the constructor's arguments 0x4E8BAD /
  0x4E8BB3), the dyno chart (+0x4D, set at 0x557AFA on a condition not
  traced), UI_OLISPConnect and the LAN screens (online stubs): these keep
  the control-kind rule.

### Sound round 9

**Verified** (the disassembly, REA / Ghidra, the packages' `PkgR` and
object responses: the maintainer's probe `fng_pkg_responses`, the maintainer's probe `fng_responses`)
unless marked. Clone: `audio::fe_rules` (round 9 part), `audio::stitch::
CommonStitches`, `Game::native_menu_sound`, `Game::garage_request`, tests
`round9_screen_key_state_table` (tests/fe_sounds.rs) and
`car_part_records_and_common_slots` (stitch.rs).

* **MC_Main** (constructor 0x4E8B70, vtable 0x797A5C: handler 0x4FB8F0
  with no sound calls, filter 0x4A33D0; base 0x552AA0 → 0x551DF0 leaves
  +0x2C = 1, +0x30 = 0). The filter: +0x64 clear → 0; 0x0C407210 → 8;
  else by the mode +0x8C (table 0x4A344C): 1 0x50D1A0(msg, default, 1), 2
  0x50D1D0 (Left / Right 0), 3 0x50D210 (PAD_ACCEPT only), 4 0x50D1F0 (no
  writer). Writers: 0x4A35C0(a, b, c) (three buttons → 2, two → 1, one →
  3, +0x64 = 1), 0x4A3890 (two, focus 0xB8A7C6CD, mode 1), 0x4A39C0 (one,
  mode kept), 0x4A3AE0 (+0x64 = 0), 0x4A3C80 (both 0). MC_Main_SetState
  0x4E8D30: states 1 / 2 and 5 (0x4A3C10) → 0x4A3890; 3 (the name) hides
  the slots (0x4A3460) and keeps +0x64 / the mode; 6 clears +0x64;
  0x7000000 → 0x4A39C0. The memory-card library's message 0x4C6050(msg,
  n, buttons) → 0x4A35C0 by n (1–3). The package answers PAD_ACCEPT,
  Back, Left, Right; buttons CC "Yes" (Right), CD "No" (Left), CF / D0 /
  D1 (Up / Down) and all 0x0C407210. Rule (`fe_rules::mc_main`): Accept
  SELECT; two buttons Left / Right / Back LEFT / RIGHT / BACK, Up / Down
  nothing; three Up / Down / Back; one Accept only. The clone: McMain's
  Ask / Confirm two buttons, its name editor too (**inferred**: that the
  PC editor lets the pad messages through), McMessage by its buttons.
* **UI_PerformanceDyno_Chart** (constructor 0x557A30, vtable 0x79AC68:
  handler 0x54A810, filter 0x5103B0): +0x4D = 1 at 0x557AFA when the
  push's argument (+0x10) is 0 (a new run: 0x5F6C30(1), 0x5B7EE0(1)), 0
  at 0x557B52 otherwise (0x537990(…, 1) from the results); the handler
  clears it when the progress (0x56D450) reaches 1 (0x54A87F, then
  0x523340). Filter: 0 while +0x4D, else Left / Right 0 and the default.
  Pushes (all three, **verified**): the mode select's accept
  (0x55A9F7 in 0x55A750, the first mode only: 0x83A63C was 0xFF) and the
  dyno menu's Dyno Run (0x53C854) push it with 0 (0x5377B0's third
  argument): a pull every time, whatever is tunable; only the results'
  Back (0x53D5C9 / 0x53D623, 0x537990 / 0x5377B0 with 1) shows the chart
  without one. The original's career at stage 3: its "no pull on the first chart" (206,
  nothing tunable) was read after the pull had ended: its state log began
  ~39 s after the Yes (tu2.jpg already shows the sweep at 1,000 rpm with
  no curve, a pull in progress; the stage's pulls took 22 s); stage 4
  saw the pull twice. The clone pulls on both and not on the results' Back.
  The package answers PAD_ACCEPT and 0xC519BFC0 (default 0); Back only the
  "Event Handler" object (**inferred** never focused). So: silent during
  the run, then Accept SELECT (`fe_rules::dyno_chart`).
* **IconScrollerMenu** 0x543D50 (column +0x30): Left / Right need items,
  +0x79 (horizontal) and +0x7B (active): LEFT / RIGHT unless at the first
  / last (0x5276E0 / 0x527710), then the step; Up / Down need +0x79 clear
  (UP on every press, DOWN unless last); Back BACK (10) and leave;
  0x0C407210 WRONG (0x13) on a locked item, else SELECT with +0x7B and
  the item's +0x20 clear; PAD_ACCEPT not handled.
* **UI_Rims_Browser** (0x567A60; constructor 0x5678F0 → 0x552340):
  every message but Back-in-sizes first goes to 0x543D50. +0x214 (the
  sizes list; 0x55FB10 sets it and clears the strip's +0x7B, 0x55FCF0
  clears it, plays id 5 and sets +0x7B): Up / Down 0x55F750 (UP at
  0x55F7AA / DOWN at 0x55F7D8 unless at the ends; nothing outside the
  sizes); Left / Right only preview the rim (0x4C2100) outside the sizes;
  PAD_ACCEPT outside: 0x55FB10 + 0x51DE40(8); in the sizes the buy
  questions + 0x51DE40(8); Back in the sizes 0x55FCF0(1) (id 5), outside
  0x543D50's BACK.
* **UI_ICEMAIN** (0x568750): 0x0C407210 on a locked item WRONG
  (0x8381E1) and return; +0x169 (in the parts overlay) drops Left / Right
  and skips 0x543D50; Back with +0x169 (0x838998 ≠ 2) 0x5605C0 (id 5);
  0x0C407210 on the strip: 0x543D50's SELECT, then the category (slot 9
  with parts: the question and 0x51DE40(8); else 0x556940, which plays
  id 4 itself: two id-4 sounds), in the overlay forwarded to it.
  UI_IcePartsOverlay (0x568D40): Up / Down 0x560930 (UP / DOWN unless at
  the ends); Accept the buy questions silently; Back forwarded to
  UI_ICEMAIN; Left / Right the camera.
* **UI_CustomNeonMain** (0x56A890): 0x543D50 unless +0x159 (the list);
  no sound of its own (Back in the list 0x562B20, Accept 0x557140:
  silent). UI_NeonPartsOverlay: Up / Down 0x55CAD0 (UP / DOWN unless at
  the ends), the rest forwarded or silent.
* **UI_DecalMain** (0x56B3C0): 0x543D50 unless +0x174 (the overlay);
  0x9FBBD1D4 (the overlay closed) plays id 5 (0x56B7EF); the 0x51DE70(8)
  at 0x56B627 needs +0x174 on a path that returns when it is set
  (unreachable). The overlay (0x563F00…): Up / Down 0x55D670, Accept the
  fit (FE_PlayInstallSound), Back forwarded (**inferred**: it ends in
  0x9FBBD1D4). Rule: `fe_rules::strip_shop` / `strip_shop_extra`.
* **The strip from inside a category** (round 8's open item):
  UI_GenericParts_Browser's Left / Right (0x5664A0 → 0x54FA00(∓1)) return
  at once while +0x1D9 (set by 0x55EDE0 at 0x55EE60, cleared by 0x55EF90
  at 0x55EFDD) is set, and Up / Down (0x55EBB0) need it; the rims dim
  their strip (+0x7B), ICE / neon / decals skip 0x543D50 in their lists.
  No browser moves the strip from inside: nothing to port.
* **UI_ShowCase_Preview** (correcting round 8): the menu state 0x4CEA70
  handles Up / Down / PAD_ACCEPT / the mouse only — no Back. Accept: Control
  Car 0x4B5C70 SELECT (0x4B5C83), then 0x499F10 and EngageState_Run
  (jmp at 0x4B5D73, id 5); Show Off 0x4B5D80 EngageState_Run only
  (0x4B5E71); Cycle Cameras SELECT (0x4B5EAD); Take Picture SELECT
  (0x4CF46D). The picture state 0x4CEDC0: PAD_ACCEPT SELECT (0x4CEEA5)
  and the save question; Back with +0x9C and +0x99 clear BACK (0x4CEF20)
  and the menu; Left / Right / Up / Down 0x4B57B0 (nothing).
* **Car-part sounds** (CarPart_RequestAnim 0x61F830(car, channel, open,
  speed), before 0x433E20, on every call): with [0x82B884]+0x90 = 1 (the
  FE load 0x486B30; the game's 0x486C10 sets 5, then 2 drag, 3 drift, 4)
  0x50D180: hood open 0x51 / shut 0x4D; trunk 0x53 / 0x52; channel 2
  with the door part (+0x810) attribute ANIMSTYLE (0x7A8758) = 1 0x4C /
  0x4F, else 0x50 / 0x4E; channel 3 none. In game 0x481500(n) on the
  FE_COMMON player ([0x82B884]+0x78): hood 3 / 2, trunk 5 / 4, doors 1 /
  0. 0x481500: the first free of slots +0x28 / +0x2C (none: nothing)
  gets a stitch (0x45B1F0, collision list record n, 0x43BE60 on +0xD0
  list +4; pool test 0x43E910 < 0x45C) started with a zeroed block
  (0x46CDD0). The player's update (vtable 0x789D28 +0x14, 0x4815B0)
  loops twice over +0x28 only: released when +0x1C is clear, else
  0x47BB30 with volume = its +0x24 (SFXObj_FEHUD, 0x4755D0) vtable
  +0x38(4) — FEHUD's mix-map slot 4 — pitch and azimuth 0. So +0x2C is
  never updated or freed: a second sound during the first stays silent
  and holds the slot. The showcase runs in the world (the in-game path):
  Show Off's toggles (0x4B5AD0) and the shuts of 0x499F10 (Control Car,
  the leave 0x499FA0). Clone: `CommonStitches`, `Game::part_stitch`; the
  FE garage's opens / shuts (`garage_open_parts`) play the FE ids at the
  change (ANIMSTYLE not read: **inferred** 0x50 / 0x4E; repeated opens
  of an open part, which 0x4C1860 passes on, are not modelled).
* **The crib's swing** (UI_CareerCarSelect 0x4DFC50; round 8's inferred
  timing): each Left / Right (0x4B04C0) plays LEFT / RIGHT and stores
  +0xB8 = 0x8651A8; the tick (0x4DFFEC) waits until (now − +0xB8) ·
  0.00025 ≥ 0.3 (0x784268, 0x78758C), then by the car (+0x54 +0x930 /
  +0x931): 0x4A7930 (+0x161 = 1, +0x148 by the last key +0x1A8),
  0x4A7960 (0x0E550F66 → GarageMain +0x162 = 1 at 0x4D8DE6), or 0x4A7900
  and the load 0x4D8EC0; +0xB8 = 0. GarageMain's state 3 with +0x161 /
  +0x162 (or state 1 once the loaded car is in) starts the swing and
  plays 0x51DE40(5 / 4, 0x17). So the platform swings once, 0.3 s after
  the last key, in its direction. Clone: `Game::garage_crib` (the car and
  the swing wait; the clone's load is at once). The lots (UI_CareerCarLot,
  UI_QRCarSelect) wait the same 0.3 s (traced since: see "Fast switching"
  under the garage).
* **Dialogs' Back** (ShowDialog 0x558030, request +0x328 count, +0x32C
  the back flag, +0x318 the back message): one button 0x558440 →
  0x5583A0 (count 1, flag 1, back message 0xB4EDEB6D; 38 call sites),
  0x558580 (flag 1), 0x5585F0 / 0x558620 (0); two buttons 0x5587A0 /
  0x558760 (1), 0x558830 / 0x558870 (0). The clone's dialog texts found
  pushed before a wrapper call (a scan of the disassembly):
  all flag 1 except "Deactivating GPS" (0x12C23B2C, 0x558620 at 0x4B1101,
  already `back = false`) and the LAN stub's 0x43003B5B (0x5585F0); the
  shop notices (0xD1AC60BE, 0xDA15D7E5, 0xD104AD36, 0x89D4E03F …) are
  0x558440's (flag 1). Dialog texts not pushed next to a call (built
  from tables) keep Back (**inferred**).
* **UI_Options' Player rows**: the nine rows (option_name_1…9,
  0xB2980BA8…B0, the focused objects) all answer 0x0C407210, Left and
  Right; the package PAD_ACCEPT, Back, Up, Down, Button1, 0xC519BFC3. So
  the filter 0x4CFAC0's id 11 plays for Left / Right on every row.
* **Reverb leftovers** (W8; ported in Sound round 10; this copy had no reverb port, so the code to
  add to `audio::mixer` in main is described here):
  - *Send ramps* (0x728B48, a voice's 16-sample block): pull 16 samples
    (voice +0x40 vtable +0); none → current sends / gains = targets
    (+0x34 ← +0x38, +0x4 ← +0x1C) and stop. Else for each bus with a
    graph (0x8B4804[b]) and each output channel: current ≠ target →
    0x728957: dst[i] += src[i] · (cur + (i + 1) · (tgt − cur) / 17), i =
    0..15 (0x7C69E4 = 1/17; the block ends 1/17 short of the target),
    then current = target; else, target ≠ 0, dst += src · tgt
    (0x8B4A04). Mixer: keep `send_cur` per voice; per 16-frame block
    ramp from it to `send` as above, then `send_cur = send` (the same
    for the dry gains).
  - *Voice-stop declick* (0x728FD2 on a voice's stop): acc_bus[b] +=
    current send[b] · the last sample (+0x3C, the block's 16th output),
    acc_ch[c] += current gain[c] · it. The next chunk (0x7292BC): for each
    bus with a graph and acc ≠ 0, 0x72886C(acc, bus buffer): bus[i] += acc
    · (16 − i) / 17 for i = 0..15 (0x7C6A20 … 0x7C6A08), acc = 0; the
    same into the channel buffers (0x8B490C[c]). Mixer: on a voice's end
    add its last wet / dry sample × gains into accumulators; at the next
    chunk start fade them out over 16 samples into the bus and the dry
    mix.
  - *Music sends*: 0x46C470(stream, bus, byte) → 0x723BC1 on the stream
    voice ([0x82B620 + 4·stream]); called by the music library's command
    processor 0x73BB60 (command +9, 1-based switch 0x73C55C): command 7
    (0x73BF57) sets bus = (int8) +0xC byte 0 to send byte = (int8) byte
    1; command 16 (0x73BF2A → 0x739EE0) fades it (7-bit target, high word
    the time) and the fade steps send it (0x73A1A1, 0x73A26A). **Open**:
    who queues commands 7 / 16 (the game side or the MPF event scripts),
    hence which music events send.

### Sound round 10 (mixfx): the mixer's ramps, stop fades and sends

**Verified** (the disassembly, REA / Ghidra; `pfdata/MusicSFx.mpf` read
from `SDATA/sdat.viv`) unless marked. Clone: `audio::mixer` (`mix_gain`,
`fade_out`, `declick`, `Mix::set_send`), `audio::carfx::BankVoices`
(the start block's send, kind 5); tests `gain_ramps_over_sixteen_samples`,
`set_gain_ramps_at_the_next_chunk`, `stops_fade_out_over_sixteen_samples`,
`stopping_a_tone_has_no_step` (mixer.rs), `engine_abk_graphs_send_wet_b`
(carfx.rs).

* **The chunk** (SndMix_Render 0x729574: chunks of min(256, n) frames,
  0x7295A8, each through 0x7292BC): first the stop accumulators (below)
  fade out into the bus (only with a graph, 0x8B4804[0]) and into the
  cleared channel buffers 0x8B48F0[c]; then each playing voice (state 2,
  0x729378): with its dirty flag (+1, set by the target setters 0x72905D,
  channel gain +0x1C[c], and 0x729087, send +0x38[b]) one 16-sample block
  through 0x728B48 and the flag cleared; then the chunk's remaining
  samples pulled (voice +0x40 vtable +0) and mixed at the targets
  (0x8B4A04: dst += src · g, skipped at g = 0), the bus only with a graph;
  a nonzero send there resets the FX idle count (0x729499; the ramp block
  does not). +0x3C = the last sample pulled (0x7293E2).
* **The ramp block** 0x728B48: pull 16 samples; none → current = target
  for every send and channel gain, return 0. Else per bus with a graph and
  per channel: current ≠ target → 0x728957 (dst[i] += src[i] · (cur + (i
  + 1) · (tgt − cur) · 1/17), i = 0..15, the step 0x7C69E4 = 1/17; 2·d as
  d + d, 3..16 from the float constants 0x787ADC … 0x79C68C), then
  current = target; else dst += src · target. A bus without a graph keeps
  its current send. A new voice starts at its targets (0x728F75: current
  = target, +0x3C = 0, state 2). So a gain change ramps over the next
  chunk's first 16 samples and ends that block 1/17 short of the target.
* **The stop fade**: 0x728FD2(voice) adds current send[b] · +0x3C to the
  bus accumulators [0x8B4924][b] and current gain[c] · +0x3C to
  0x8B490C[c] (no graph test), then frees the voice's sources. Callers:
  0x7292AC (a pull giving < 0 in the chunk loop, 0x7293C9) and 0x72A000
  (the stop API, per voice of a list, then 0x725378). At the next chunk's
  start 0x72886C(acc, buffer): buffer[i] += acc · (16 − i)/17 (constants
  0x7C6A20 … 0x7C69E4, each the float nearest k/17), acc = 0, for each
  nonzero accumulator (0x7292E7, 0x729334). The bus accumulators are
  zeroed once at 0x726D1B and cleared only by the fade, so a stop while no
  graph is loaded waits for the next graph.
* **Music sends: none.** The command processor 0x73BB60 runs the actions
  of the pending event instances ([0x8B7BB0]+0xDC[], 0x739930 →
  0x739870: 16-byte actions from instance +0x10, count +0xE & 0x3F; +0
  the track mask, +4 the delay, +9 the command, +0xC its argument). The
  instances are copies (0x739540) of the `.mpf`'s event records (0x7399E0
  at 0x739AA0) or events built in code with 0x73B9D0: 0x738EF0 (command
  4), 0x73CB80 (2 and 4), 0x73D1B0 (6, the volume fade 0x739E80). The
  only `.mpf` (`PFData/MusicSFX.mpf`, loaded by 0x73A7C0 from 0x483FB8)
  has 79 events whose 105 actions are commands 4 (79) and 1 (26); none is
  7 or 16, so 0x46C470 is never reached and the music streams never send
  to the reverb (their send stays at the stream voice's start value,
  **inferred** 0). Nothing to port.
* **The engine ABK graphs send wet B.** Run in the clone's AEMS
  interpreter (`engine_abk_graphs_send_wet_b`), every
  `SOUND/ENGINE/CAR_nn_ENG_MB_EE.abk` "CAR" event starts its voices with
  the start block's send byte (+5, kind 5) = p14 >> 8 (p14 = ([0x82B070]
  wet B · level0) >> 15, 0x47436D) and sets no send live. So those voices
  send at p14 >> 8 / 127 × their volume (0x723BC1 → 0x729E24: the byte /
  127 × +0x48; the second volume not included). The live kind 5
  (0x720936) stores clamp(v, 0, 0x7FFF) >> 8 as bus 0's byte. The clone
  now gives the bank voices that send (`Sink::send`); before, only the
  GIN engine voices sent.
* **Clone.** Each voice keeps current gains (library channels 0 / 1, the
  mono output) and send; per chunk a target that moved ramps over the
  first 16 samples as above (the dirty flag is the comparison: a set that
  leaves the target unchanged ramps nothing natively either). Stops
  (`Mix::stop`, a one-shot's end, an engine voice replaced, a track's
  stop or end) add the last sample × the current gains (and send) to the
  accumulators, faded out at the next chunk. Tracks (speech, music) ramp
  and fade as voices (**inferred**: that their stream voices take the
  same path; the stream library's voice not traced). Deviations: a chunk
  under 16 frames ramps over its length (the library always mixes a
  16-sample block); a one-shot's end fades at the chunk after its last
  sample (the library's source returning < 0 one chunk later is
  **inferred**, the pull not traced); the x87 extended precision of the
  ramp products is not reproduced (≤ 1 float ulp). The movie soundtrack
  keeps no gain state (its level never changes).
* **Cost** (`mixer::cost::render_cost`, ignored by default: 24 loops with
  gains moving every game frame, 10 s at 44100 in 512-frame callbacks,
  the reference desktop): 85 ms before, 55–59 ms after (voices mixed per chunk, not
  per frame).
* **Captures** (`NFSU2_AUDIO_WAV`; shots now also write `<file>.stops`:
  each stop's frame and the step a cut would leave): explore 4, race 3,
  drift 30, drag 8 stops, the largest cut step 0.0021 of full scale; at
  every stop the output's largest sample step within 40 frames is below
  the largest in the 300 frames before it (no click). The game's stops
  come at low levels; `stopping_a_tone_has_no_step` stops a 440 Hz tone
  at its peak and finds no step above the tone's own slope plus 1/17 of
  its amplitude.

### Sound round 11 (sfx): the full inventory and the car stereos

**Verified** from the disassembly unless marked. Checklist and the bank
table: docs/sound-audit.md (round 11).

* **Inventory.** The banks the exe can load: the csi list 0x7ED148..
  0x7ED15C (UG2_MAIN, UG2_FE, UG2_ENGINES_AEMS2, UG2_TURBO, UG2_ENVIRO,
  UG2_STITCH; `MW_MAIN_AEMS.csi` / `MW_SIREN_AEMS.csi` are not in it and
  no code names them: never loaded), the bank names 0x7ED190..0x7ED22C.
  `Sparkchatter_00/01.abk` (0x7ED198 / 0x7ED19C) and the
  `TURBO_0x[A-D]_MB.ABK` names are not referenced by code; FX_SPARKCHATTER
  (slot 0x82C5C8) has no start site. The 41 start sites of 0x7358C0
  (pairs with the retry after 0x735840) map onto 27 of the 29 bound
  events; FX_SPARKCHATTER and FEDriveOn have none. Two were not in the
  audit: 0x46368D (the car stereo, below) and 0x46AEBD (the water drum:
  FX_ROADNOISE_TRANS {3, 0, 0, 0, 1}, which ROADNOISE_00_MB plays as
  sample 14, 1.99 s — the maintainer's probe `aems_probe`; at level 0 until something
  sets p1, **open**).
* **Context masks** (0x476B50 and its neighbours, through 0x476990(0,
  mask) → context slot 1 0x483740: +0x24 = mask, 0x47F660 makes one
  object per set bit by sub-id (0x476790 → 0x476890: registry id bits
  4–15 = the bit, bits 16–23 = the manager's class, classes 2 / 3 falling
  back to class-1 entries), 0x47F710 makes the controllers they ask for):
  the player car 0x5FFF, traffic 0x1F (Engine 0, Woosh 1, Horn 2, Stereo
  3, BottomOut 4), the AI racers 0x802D (bits 0, 2, 3, 5 and 15 =
  CARSFX_Stereo 0x200F0). The AI contexts are made for each car-table
  entry with +4 = 2 or +0x900 set and attached (+0x28) at once, before
  slot 1 makes their objects.
* **Traffic attach** (0x463E30, the traffic manager's update): over its
  16 car slots (+0x20..+0x5C), a car within 70 m (0x78A414, Vec3_Distance
  of the cars' +0x60 to the nearest player's car 0x459A50) with no context
  gets a free one (manager slot +0x30) and is attached (context slot 10,
  0x469B20: +0x30 = the car, +0x44 = the car type (car +0x14 → +0x10) in
  0x23..0x2B except 0x24 / 0x28, +0x3C = 1, each object's slot 11); a car
  at 70 m or more with one is released (slot 11 of the context, 0x469B90:
  each object's slot 12); then 0x463BD0 runs the objects.
* **CARSFX_TrafficStereo** (registry 0x7ED64C {0x30030, name 0x788924,
  0x7ED63C, factory 0x47F4E0}; ctor 0x47F410, vtable 0x78BC80) and
  **CARSFX_Stereo** (0x7ED63C {0x200F0, 0x78893C, 0x7ED138, 0x47F480};
  ctor 0x4764B0, vtable 0x78B608) share slots 7–12. Attach (slot 11,
  0x476580): with a car whose +0x14 → +4 is 3 (traffic): a type (car
  +0x10 → +0, the type index) of 0x1F / 0x22 (TAXI / TAXI02) sets +0xA0 =
  0, any other +0xA0 = (Rand_Int(3) = 0) (0x43C1C0); then +0xB8 = 8 +
  Rand_Int(2). Slot 7 (0x4765F0): +0xB8 = 8 + Rand_Int(2); with a car
  whose kind is 2, 0x82CB6C = (0x82CB6C + 1) mod 6 and +0xA0 = (it = 0).
  Slot 9 (0x4635B0): with +0xA0 and a car (context +0x30): playing (+0xB0)
  and slot 0x38(0) = 0 → stop (0x45FA60(1)), +0xB0 = 0; not playing and
  slot 0x38(0) ≠ 0 → ENV_STATIC (0x82C510) {clamp(+0xB8, 0, 50), 0, 0,
  0, 0} (node 0x457E20, "AUD: AI STEREO") → +0xB0. Slot 10 (0x4636E0):
  p1 = clamp(slot 0x38(0), 0, 0x7FFF), p3 = clamp(slot 0x3C(0), 0,
  0xFFFF), p2 = 0x1000, dispatched (0x735580). Slot 12 (0x463540) frees
  it. ENV_COMMON_MB's ENV_STATIC plays sound 7 (1.86 s loop) for variant
  8 and 8 (2.58 s loop) for 9. The mix map binds traffic class 3 slot 0
  in section 3 and AI class 15 slot 0 in section 2 (`mixmap::tests::
  stereo_slots`; 1060 and 3770 in the still test scene).
* **Clone** (`traffic::Stereo`): each traffic car slot attaches within 70
  m of the player's car, rolls as 0x476580 (taxis by the car name), plays
  ENV_STATIC 8 / 9 at the mix map's traffic class 3 slot 0, pitch 0x1000,
  and releases beyond 70 m or when the slot's car changes. **Inferred**:
  the clone's pool slot as the context's car, slot 9 before slot 10 in a
  frame. The clone still runs the traffic's other objects for every car
  with a lane, not only those within 70 m (the mix map's levels decide
  what is heard). **Open**: CARSFX_Stereo on the AI racers — 0x476580
  needs kind 3, and slot 7 is reached through the car context's slot 16
  (0x469CD0 → 0x464BB0, objects' slot 7), whose caller is not found; not
  ported. The stereo voices' reverb send is the graph's start-block
  send byte, through the clone's generic event-bank voices (as the
  horns', `carfx::CarFxSounds`).

### Sound round 12 (2026-10-31): traffic sound range, racer stereos, the water drum, hydraulics, speech

**Verified** from the disassembly (objdump, REA/Ghidra) unless marked. Not
seen under Wine: with no sound device the original runs with 0x7FAAA8 = 0
(read with peek.py in free roam), and every sound manager, car add
(0x4690E0) and object is gated off, so the sound contexts cannot be read
there.

* **Traffic contexts (the 70 m sound slots).** The traffic manager
  (vtable 0x78A3E0, 13 slots; audio object +0xB4, allocated at 0x48576F
  with its 16 car slots +0x20..+0x5C and count +0x60 cleared) is set up
  with 0x463AD0(3, 6): class +0xC = 3, the context cap +0x14 = 6.
  * Contexts: at the world's start (World_Start 0x60A690 → 0x486DB0 at
    0x60A773, slot 2 of the in-game managers +0xAC..) slot 2 0x476AF0
    counts the car table's (0x89E7A0, count 0x89E814) traffic entries
    (+4 = 3), caps them at 6 and makes that many contexts with mask 0x1F
    (0x476990 refuses past +0x14). Context vtable 0x78BA78 (ctor
    0x4795D0): slot 4 0x464BB0, slot 5 (update) 0x464C10, slot 6
    0x464BF0, slot 10 attach 0x469B20, slot 11 release 0x469B90.
  * Cars: a car's creation adds it (0x604723 → 0x4690E0: owner kind 3 →
    the manager's slot 4 0x463F10, the next of the 16 slots), the track's
    unload (0x5FF3C0 / 0x5FF700 → 0x5F45EF → 0x469160 → slot 10
    0x463EE0) clears its slot. So a track's traffic cars keep their
    slots in creation order for the track's life.
  * Each frame (slot 6, 0x463E30): for each of the 16 slots with a car,
    d = Vec3_Distance (0x43CE10) of the car's +0x60 to the nearest
    player's car (0x459A50); the context attached to that car (list +0x10
    by +4, +0x3C set and +0x30 = the car) is released (slot 11) when d ≥
    70 (0x78A414); with none and d < 70 the manager's slot 12 (0x464350)
    returns the first context of the list with +0x3C clear, or none — the
    car then simply has no sound. No priority, no stealing, no distance
    ordering. Then 0x463BD0 runs every context's update (slot 5).
  * Attach (0x469B20): +0x30 = the car, +0x44 = its type in 0x23..0x2B
    but 0x24 / 0x28, +0x40 = the car, +0x3C = 1, each object's slot 11.
    Release (0x469B90): +0x40 = 0, +0x3C = 0, each object's slot 12,
    +0x30 = 0.
  * The objects (mask 0x1F) act only while attached: TrafficEngine
    (vtable 0x78B3A8) starts FX_TRAFFIC at the attach (slot 11 0x4621C0:
    0x82CB38 toggles the variant, 0x462080) and stops it at the release
    (slot 12 0x475790, also clearing its position object); its frame
    (0x462240) tests context +0x3C. TrafficWoosh (0x78B490): its start
    test 0x475E10 needs +0x3C; release 0x462650 frees the whoosh.
    TrafficHorn (0x78B428): 0x47E6E0 needs +0x3C, its clock is the
    context's +0x34; release 0x4624E0 stops the horn, clears +0xC4 /
    +0xC5 and sets +0xC0 = the clock. TrafficBottomOut (0x78BC30): its
    update 0x47ED80 needs the car +0x30; release 0x462700 frees the three
    knock stitches. TrafficStereo (0x78BC80): attach 0x476580, release
    0x463540 (round 11). Objects travel with their context, so a horn's
    last-stop time and the like belong to the context, not the car.
  * The mix map's copies per section are the groups' instance counts
    (0x485FA0), so section 3 has one copy per context.
  * **Clone** (`traffic::OtherCars`, `traffic::assign`): min(pool cars, 6)
    contexts, attached in pool order within 70 m of the player's car and
    released at 70 m; engines, whooshes, horns, knocks and stereos run only
    on attached contexts; the mix map's section 3 copies are the contexts
    at their cars (game.rs `mix_sounds`). Before, every traffic car with
    a lane ran all five objects. Tests `traffic::tests::
    contexts_go_to_the_first_cars_in_range`, selftest (six of ten cars).
    **Inferred**: a context lets go when the clone's pool car loses its
    lane (the native car keeps its slot and is released only by
    distance), the doppler history starting again at an attach, the mix
    map's copy for a free context placed at the player's car (its objects
    are silent).
* **Racer stereos.** World_Start (0x60A690) calls 0x458850 at
    0x60A7B3, after the cars (0x5F4260): every manager's slot 5
    (0x463BB0: +0x1C = 1, then each context's slot 4). The AI and player
    contexts (vtables 0x78BF30 / 0x78BF88) have slot 4 = 0x469CD0: with a
    car (+0x30), 0x464BB0 runs each object's slot 7 and each controller's
    +0x20, then +0x3C = 0. CARSFX_Stereo's slot 7 is 0x4765F0: +0xB8 = 8 +
    Rand_Int(2); a racer (car +0x14 → +4 = 2) sets 0x82CB6C = (0x82CB6C +
    1) mod 6 and +0xA0 = (it = 0). 0x82CB6C lies past .data's raw data
    (0 at boot) and has no other writer, so across the session the 6th,
    12th, … racer gets a stereo. Slots 9 / 10 are TrafficStereo's (the
    ENV_STATIC loop on the mix map's AI class 15 slot 0). This replaces
    round 11's "no caller found". **Clone**: `traffic::Stereo::
    racer_roll` as each racer's objects are made (the session's counter
    in `OtherCars`), played on section 2 class 15 slot 0. **Inferred**:
    the roll in the clone's racer order (the car table's order is not
    modelled).
* **Offline check of the traffic contexts and the splash** (selftest):
    six of ten traffic cars in range hold a context; twelve racers give
    two stereos; a splash's three requests run and end with the sample
    after 2.05 s of rendered mix (the maintainer's probe `aems_probe` with
    `NFSU2_PROBE_VOICE_TICKS`: FX_ROADNOISE_TRANS's instance ends when its
    voice does).
* **Water drum splash.** The props' sound object (vtable 0x78BB08,
    ctor 0x47A960: +0x14C..+0x154 = 0; base vtable 0x78A760) overrides
    slot 8 (the start, base 0x46A540) with 0x46AE00 and slot 9 (the
    frame, base 0x481F10) with 0x481FD0. Material +0x140 and the prop
    +0x144 come from the first update (0x46A710: +0x144 = 0x592D90's prop,
    +0x140 = its info +0xD8). The prop's +0x505 is set once by 0x5ADFA0
    (the prop starts moving: +0x506 = 1, +0x19 = 1, 0x86B378 + 1, the
    particle groups of its list +0x4FC started (0x48BC50 on 0x8357E0) and
    +0x505 = 1 the first time; the only other store, 0 at 0x5B18A9, is not
    traced).
    So a WATER_DRUM (material 5) that has been knocked loose, at the start
    of an impact sound, with +0x50C clear, sets +0x50C and starts the three
    requests {3, 0, 0, 0, 1} (round 11). The frame 0x481FD0 (after
    0x481F10) for each request: if its instance has ended (0x735070 gives
    1) release it and return; else p1 = clamp(+0xC0 · [0x7ECF90] >> 15, 0,
    0x7FFF) ([0x7ECF90] = 17000), times the ramp 0x4599E0 (0x82BCD0, input
    +0x11C) when +0x128 has bit 0x10; p3 = clamp(+0xC8, 0, 0xFFFF); p2 =
    2000, 4000, 7000 for requests 0, 1, 2 (clamped to 0x1FFF); dispatched
    (0x735580). +0xC0 / +0xC8 are the object's impact block {+0xBC..}
    level and azimuth, zeroed by the start 0x46A540 (0x46A63E..) and fed to
    the stitch player each frame (slot 16 0x481FB0 → 0x47BB30). +0xC0 is
    written every frame by the object's mix-map client SFXObj_Collision
    (0x461CE0 at 0x461D85: section 5's level of the kind's slot at the
    object's point; **verified**, and read under Wine: 0 on the
    start frame, then 12999 next to a barrel hit, falling as the car
    leaves — docs/prop-hits.md). **Open**: the writer of +0x128 bit 0x10.
    The splash starts with the object (0x46AE00 is vtable 0x78BB08 slot
    8, the start the queue's flush calls at 0x4818A6), so the five-object
    cap drops it with the object.
    **Clone** (`smackable::prop_sound` sets `Impact::splash`,
    `Game::mix_levels` starts it with the object after the cap,
    `collision::splash_level`, `EnvSfx::splash` / `splash_start`): the
    drum's first sounding impact after it was knocked loose splashes once;
    three FX_ROADNOISE_TRANS variant-3 voices at pitch 2000 / 4000 / 7000
    and level V · 17000 >> 15, V the object's copy's level each frame,
    azimuth 0 (mono mixer), the ramp not applied. The clone once took V as the
    impact's volume byte scaled to 0x7FFF (16999 for a hard hit against
    the original's 6743 at 1 m: +8 dB) and the splash bypassed the cap.
* **Hydraulics (C15 / F19) exist in PC gameplay.** The specialties shop
    sells HYDRAULICS (parts slot 0xA1, `car.rs` slot names), the Controls
    screen has Hydraulics Pressure (action 22) and Hydraulics Bounce
    (action 19), the physics has the hydraulics (0x5A51C0; car +0x408
    bounce bytes, the mover's wheel states +0x74 + 4i read by 0x5ED480),
    the style tricks count Bounce / Hydros (0x56F9C0), the garage moves the
    car's corners (0x4A6030), and the race-start scenes' event tracks have
    CAR_HYDRAULIC_* events. CARSFX_Hydraulics (registry 0x7ED464 id
    0x10080, factory 0x47DAE0 → ctor 0x4745B0, vtable 0x78AF88): attach
    (slot 4, 0x474610) keeps the controller of sub-id 8 at +0xF4; update
    (slot 9, 0x47DD40), while +0xA0: per wheel i, a finished FX_Hydraulic
    instance (+0xB8 + 4i) whose state +0xCC + 4i is below 2 is released
    (0x474660, +0xDC + 4i = the state); the state = 0x5ED480(i) and +0xF8
    + 4i = car +0x408 + i; the bounce FX_Hydr_Bounce {0, 0, 0x6F54, 0}
    (+0xC8, slot 0x82C550) is released once ended and started again for
    a wheel with +0xF8 + 4i set; the frame (slot 10, 0x4615E0) sends each
    wheel's p5 (+0x14) and the bounce's p3 (+0xC) = clamp(slot 0x38(0), 0,
    0x7FFF). **Not ported** (open): the clone has no hydraulics (no
    physics 0x5A51C0, no actions 19 / 22, no garage motion 0x4A6030), so
    there is nothing to drive the sounds; the rest of 0x47DD40 (the
    FX_Hydraulic starts per state) and +0xA0's writer are not traced.
* **Rachel's spoken hints.** Already ported (menus107, race2:
    **Rachel's speech**, **Rachel's hints**): every poster of the hint
    slots (0x5243E0 front-end lines and the car lot / crib, 0x524250,
    0x5242D0, 0x528330, 0x525400, 0x525520, 0x5255E0 / 0x525620, 0x551080),
    the counting and firing (0x54BD40, 0x53E0C0), the neighbourhood lines
    (0x525660), the D-Day script's lines, the showcase line, the outrun
    lines and the pick (0x738860) are wired in the clone (game.rs `say`,
    `Hints::tick`, `zone_changed`, `fe_line`, `milestone`, `race_over`,
    `stage_up`, `sponsor_popup`, `unique_taken`, `outrun_start` / `_end`,
    `showcase_line`, `dday_a`). The kind-2 front-end line, 0x5243E0(2),
    is UI_GenericParts_Browser's INIT in the body shop (0x566EE7..0x566EEE,
    with 0x838990 = 0, before the save snapshot 0x52A470(0)); it sets +0x28,
    so the DVD lines (kind 0xD) can come. Checked this round in the clone's
    offline capture (`NFSU2_AUDIO_DEBUG=1 NFSU2_AUDIO_WAV=… game --flow
    newcareer`, which now logs each line): the career scene's Career
    (0x8000, 0x20), then the script's (0x800, 0x20) with SMS_INSTRUCTION,
    (2, 0x20) and (0x10000000, 0x20) — each fits entry 0x27, one record —
    in the script's order, 43 s of output with no gap. Still **inferred**: the library's event choice
    after 0x45AD40 → 0x722ECE / 0x722F98 (the request +0xC; every tuple the
    game sets fits one event, so the clone's first-fit rule picks the same),
    the NIS / speech stream arbitration, 0x46C790 / 0x45A550's meaning in
    "free". Not heard in the original (no speech object without a sound
    device).

### Sound comparison against the original's recorded audio (soundcmp, 2026-11-01)

First comparison of the clone's sound with the original's actual output
(`fastgpu.sh --audio-file`, Wine dsound → PulseAudio null sink; the clone's
offline capture `NFSU2_AUDIO_WAV` with `--flow soak`, new phase
`keys:secs` with `NFSU2_SOAK_KEYS`, volumes `NFSU2_SOAK_VOL`). The report and
tables are kept with the maintainer's private evidence.

* **Output scale (verified).** SndMix_Render 0x729574 mixes each chunk into
  16-bit channel buffers and SndMix_Interleave 0x72CD6A only interleaves
  them into the device buffer (0x725C8E): full scale, no headroom. The
  clone's mixer had a 0.5 master (−6 dB); measured under Wine the FE clicks
  (PlayCommonSample {id, 4235, 0x1000, 0}, music muted) were 5–7 dB louder
  than the clone's. Now 1.0 (`mixer::MASTER`); the clicks match within
  ~1 dB in RMS and peak, and the spectra band for band.
* **Engine entry of the car driven (verified under Wine).** The CAR event's
  parameter 0 is the engine entry: 27 for Rachel's 350Z (350Z_Jason, rpm
  1250..6596 — the parameter 1 readings 1252 idle, ≤ 6583). The clone took
  the profile's parts (none during D-Day) → entry 25. `Game::driven_perf`
  now gives the ride's parts (the D-Day preset, a Quick Race car's), for
  the engine and for `car_sfx_input` (shift, nitrous, turbo levels).
* **SFXCTL_MasterVol f8..f13 (verified, 0x471A84..0x471B72):** trunc(v ·
  master · 32767) for menu music, game music, speech, effects, tires,
  engine (+0x24..+0x38). Section 0's event controls take f10 / f11 as their
  scales; the clone gave 0, so those ducks never acted. f7 (+0x20) and
  f14 (+0x3C: f1, or 0x7FFF in drag / some race states) remain **open**.
* **Speech ducks (verified).** SFXObj_Speech (vtable 0x78B1A0, update
  0x4751A0): both fields 0 when the speech system's +0x5D is set, when the
  NIS stream object has +0x4C = 1, or when the speech stream is not busy
  (0x45A550); else f0 = 1 with g_GameState 3, otherwise f1 = 1 (the other
  field kept). MAPOUTPUT section 0's event controls on f1 (0x40000011:
  −900, −300, −929, −1320 mB) and f0 (0x40000010: −689, −810 mB), scaled by
  f10, duck the world. Under Wine the player engine's level 0 went 18236 →
  6477 (−9.0 dB) while Rachel spoke and back ~0.6 s after; the clone now
  gives exactly 6477 / 18236. **Inferred**: busy = the clone's speech
  track playing (`Streams::speech_playing`); +0x5D not modelled.
* **Speech stream volume (verified, 0x46CB48):** each frame the speech
  stream's volume = trunc(kind 3's level / 32768 · 100) / 127, the level
  from 0x45AC20 = the speech object's mix slot (0 when audio +0x90 = 1,
  the front end; else 1): masters 2 / 3 of section 0, −450 / −300 mB plus
  the speech volume's control 2. The clone played speech at the option
  volume (≈ 0 dB): ~9 dB louder than measured. Now `Streams::
  set_speech_gains`; measured −21 / −18.5 dBFS RMS against −24 for the
  original's (different) line. The NIS stream's kind-7 level (0x46CBAE) is
  **open** (the NIS line keeps the option volume).
* **Matches** (no change): which FE sounds fire per key (ids 3 Right, 2
  Left, 4 Accept, 5 Back, none for Up / Down on the main menu, none on
  opening the pause, 5 on leaving it); the pause silences everything,
  music included; no countdown sounds; the engine's rpm map (idle 1250 /
  1252, top ≈ 6550–6580) and its levels and spectra in reverse,
  acceleration and on the limiter (within ~1.5 dB and a few dB per band).
* **Open**: the idle bed under the engine is ~4–5 dB quieter and has less
  250–1000 Hz energy and stereo decorrelation than the original's (the
  airport's ENV_STATIC plane loops and the reverb are the candidates, not
  traced); the race start is ~2–3 dB louder in the clone (different
  scenes, music on; not separated); the first FE sound of a `--flow soak`
  run plays at level 32767 because the mix map has not run a frame yet
  (the flows only).

#### Sound comparison, round 2 (2026-11-01)

Both games recorded under the same conditions (the original by
`fastgpu.sh --audio-file` with `sndwatch.py` / `parmwatch.py` and memory
reads; the clone by `--flow soak` with the new check aids below), compared
by event parameter blocks first and by the audio where the blocks agree.
The recordings and scripts (ovol.py, tprobe.py, vtscan.py, crun.sh) are
kept with the maintainer's private evidence. New check aids in the clone: `NFSU2_EVENT_LOG=path` (every AEMS
event start, parameter block and end with the capture time,
`audio::aems::log`; the counterpart of parmwatch), `NFSU2_MIX_TRACE=
"s.copy.class,…"` (a mix-map master channel's inputs once a second),
examples `mixmap_dump` and `mix_traffic` (a section's controls; the
traffic engine's level for given position fields).

1. **Idle bed in free roam (fixed).** Round 1 muted the game music; the
   ~20 dB missing in the clone's wind beds came from that. SFXCTL_MasterVol
   **f14** (+0x3C, 0x471B75..0x471BBB, copied to 0x7ED344) is 0x7FFF in a
   drag or a drift (0x89E7D8 / 0x890108 / 0x89E7D9 / 0x890104) or when
   the EA Trax controller 0x82B9C8 has no in-game song (+0x98, the count of
   playlist entries in state 2 / 3 that 0x46BAA0 is given), else f1 (the
   game-music field). Read under Wine: 0 / 16384 / 32767 at game music 1 /
   0.5 / 0. Section 0's controls 6..8 (curve 9 of f14, boosts 2040 / 839 /
   1500 mB) feed the player's wind (class 7) and weather wind (class 10),
   and the player's volume fields (0x45C210, group 1 class 3: f0 speech, f1
   menu music, f2 = [0x7ED344], f3 effects, f4 engine, f5 tires, f6 =
   0x7FFF − f2) feed control 8 (f2) and others; the clone gave f14 = 0 and
   f2 = 32767 / f6 = 0 always. With music off at idle: FX_WIND_Weather p0
   2154 and FX_WIND p1 / p4 53 in both now (205 / 3 before); with music
   on 205 / 4 in the original, as the clone had. Also **SFXCTL_3DRight /
   LeftWheelPos** (0x100B0 / 0x100C0) track SFXCTL_Wheel's two points 1.5 m
   ahead, 0.8 m to each side (0x46E8B0; read live: azimuth 5112 / 60423 =
   ±28°), not the side wheels' means (±87° in the clone): the wind,
   road-noise and skid azimuths now match exactly. Result at idle, music
   off, 8 s windows: original −29.3 dBFS, L/R correlation 0.42, bands
   −12.8 −7.7 −2.9 −9.1 −10.9 −15.1 −23.0 dB; clone −29.2 / −29.0, 0.22 /
   0.43, −12.3 −8.8 −2.1 −11.0 −12.8 −18.1 −26.7 (250–500 Hz ~2 dB low in
   that one window). **Round 3**: not a defect. The bed is not
   steady (wind, planes, traffic), and one 8 s window says little: over
   the original's a1 capture (8 s windows from
   100 to 1700 s, Welch PSD of the mono mix, octave bands 31 Hz…8 kHz as
   dB of the total) the 50 quiet idle windows
   (−28…−31.5 dBFS, 4–8 kHz under −33 dB) give −15.9 −7.1 −2.7 **−10.6**
   −12.5 −16.7 −26.6 −39.2 (SD 0.5–1 dB below 1 kHz, 2–4 dB above); the
   clone's 240 s idle at the airport, music off (`--flow soak` with
   NFSU2_SOAK=profile,career,keys:240, NFSU2_SOAK_VOL=game_music 0),
   22 windows: −16.5 −7.1 −3.1 **−9.6** −11.7 −16.9 −27.1 −39.8 (SD 1–2
   dB). The 250–500 Hz band is within 1 dB (the clone's a little higher),
   inside the window-to-window spread, so no loop, send, EQ or road-noise
   change is made. Caveat: the original's capture was a career save at
   its own spot, the clone's the D-Day airport. The planes (ENV_STATIC p1 0..836 at the airport) are not
   the bed.
2. **Traffic.** The traffic engine's level for the original's own
   position fields (SFXCTL_3DTrafficPos, vtable 0x78B3EC, f0..f3 read live
   at 0.1 s) through the clone's mix map matches the original's FX_TRAFFIC
   p1 to the unit one frame later (88/88, 74/74, 1729/1729 …, mean −0.7 dB
   over 37 samples, the difference the 0.1 s sampling lag): distance
   (camera f1) and azimuth (camera f3) laws verified, including the
   crossfade using the first direction's curve type for both (0x468380 does
   not read its second type argument). The level caps at 13690 in both.
   Horns, stereos and whooshes were not triggered (player at rest);
   **open**.
3. **Nitrous (the finished-career save's Lancer).** FX_NITROUS's block {0, 9471, azimuth,
   stop flag} is the same; the instance lasts 4.07 s for a 3 s press in
   both (stop flag at the key-up, ~1 s tail). The audio level and pitch
   were not separated from the collisions both runs hit; **open**.
4. **Race finish and results:** not recorded this round (**open**).
5. **Shops / garage / markers:** leaving the garage in the original plays
   only PlayCommonSample 4 / 2 (Accept / Left) at 4235 and the menu
   music; the NIS line and door were not captured (the save prompts took
   the window); the clone goes from "Return to Explore Mode" → Yes straight
   to free roam, where the original asks "overwrite previously saved
   game?" / "save the profile?" first (autosave on in the finished-career save; a menu
   difference, not sound). Markers 14 / 15 / 16 and shop music **open**.
6. **Skids and collisions:** the resting FX_SKID blocks match (7741 10936
   8201, azimuths now 5112 / 60423); scripted skids were not comparable
   (different traffic and walls); **open**.
7. **NIS stream and the speech flag (fixed / traced).** 0x46CB00 sets the
   NIS stream's volume each frame like the speech stream's: trunc(level /
   32768 · 100) / 127 with 0x45AC20(kind): kind 3 the speech object, else
   SFXObj_NISStream ([[0x82BA30]+8]+0x50, set by 0x4752D0; section 0 class
   5) at the slot the kind picks (table 0x45AD18: 1 → 2, 2 / 7 / 9 → 0, 4
   → 4, 5 / 8 → 3, 6 → 1; 0 when the slot is unbound). The kinds are
   NisAudio_Request's first argument, queued as is: 1 GB_NOS, 2 a race
   start, 4 THUNDERSTRM, 5 CollisionStream, 6 a shop / ending scene, 8
   BLOWNENGINE, 9 the career start; 7 is queued by 0x46CD20 (**inferred**:
   a crowd line). The clone's NIS line now takes that level (it kept the
   option volume) and the SoundFX requests their kinds (the old note said
   kind 4 for all). The speech system's **+0x5D** (the speech stream,
   [0x82BA30]+4) is set when a queued request waited 1 s without starting
   and was deleted (0x47B8E0) and cleared at the next queue (0x46CAD1):
   while set the ducks are off. The clone starts a line at once and never
   expires one, so it never sets it (equivalent). MasterVol f7 (+0x20) = 1
   with race type 8, race status state 4 and 0x827EC8 / 0x827EB0 /
   0x827E98 all 0 (events 4 and 12 of section 0: −240 / +300 mB); f15
   (+0x40) = 1 while a movie plays with [0x8383A4] in 30..34 (event 13,
   −10000 mB): both left 0 (**open**: those states).
8. **Option volumes (verified).** At base, master 0.5, master 0.2,
   engine 0.5, master 0.5 + engine 0.3, effects 0.5, effects 0, tires 0.5
   and game music 0.3 (game music 0 otherwise), the player's CAR block (p4
   / p5 / p14), FX_WIND, FX_WIND_Weather, FX_ROADNOISE and FX_SKID are
   identical in both, e.g. CAR p4 −25811 / −30314 / −32338 / −29285 /
   −32031, FX_WIND_Weather 2154 / 759 / 132 / 1078 / 0 / 1569. The speech
   and music levels were not re-measured this round.
9. **Panning and distance.** Distance and azimuth: see 2. The pan law and
   its side: with the engine, tires, speech and music at 0, a traffic car
   at azimuth ≈ 49 600 (−90°) is 15–20 dB louder on the left in the
   original's WAV and 12–19 dB in the clone's; at ≈ 12 700 (+70°) the
   original's right is 11–13 dB louder (law: 14.6 dB). So 0x4000 is the
   listener's right (the clone's mapping, previously inferred, now
   measured).


#### "No in-game sound" on the player's machine (engine, 2026-11-01)

* **Symptom (measured on the player's live session, development builds 0843 and
  0901).** The game's PipeWire stream (`alsa_playback.game`, f32 stereo
  44.1 kHz, 512-frame buffers, unmuted at 100 %) carried exact zeros
  (`pw-record` of the stream and of the sink monitor: 0 for 25 s), while
  the `cpal_alsa_out` worker ran at ~30–100 % CPU with its voluntary
  context switches frozen (6 311, then 6 314 an hour later) and its
  involuntary ones climbing (14 937 → 20 927 in 5 s): it never slept.
  Not a mix-map, duck or option fault: every offline capture of the same
  save (the finished-career save, the player's own copy and the fixture; 48 / 44.1 kHz; 60 /
  1000 fps; frames drawn with the preload; a real-time render thread)
  played the engine, the world and the music.
* **Root cause (cpal 0.17.3 + PipeWire's ALSA plug-in, read in both
  sources).** pipewire-alsa 1.6.10 (`pcm_pipewire.c`): once the PCM is in
  XRUN, `snd_pcm_pipewire_poll_revents` adds POLLERR to the revents
  (lines 173–174) and `update_active` keeps the poll eventfd signalled
  (lines 98–100, 121–122), so every poll returns at once with POLLERR
  until the application calls prepare. cpal 0.17.3
  (`src/host/alsa/mod.rs`): `poll_descriptors_and_prepare_buffer` turns
  POLLERR into `Err("alsa::poll() returned POLLERR")` (lines 938–942) and
  `output_stream_worker` maps any Err to `PollDescriptorsFlow::Continue`
  (lines 845–851): poll again, never prepare, never recover. Only an xrun
  that cpal meets inside `avail()` (EPIPE → XRun → prepare) or `writei`
  (EPIPE → try_recover) recovers; one that happens while the worker
  sleeps in poll leaves it spinning for good. Also in 0.17.3: a writei
  error other than EPIPE (ESTRPIPE after a suspend, EBADFD) is retried in
  a `continue` loop without recovery (lines 1050–1052). cpal 0.18.0's
  changelog: "ALSA: Polling errors trigger underrun recovery instead of
  looping."
* **The trigger: an xrun while the worker waits in poll.** Reproduced in
  a private PipeWire 1.6.10 + WirePlumber with a null sink
  (`scripts/audio_sandbox.sh`, the maintainer's probe `audio_xrun_repro`, watchdog
  off): one `pw-metadata -n settings 0 clock.force-quantum 4096` (a
  graph quantum longer than the stream's buffer) put the worker at 100 %
  CPU (50 ticks per 0.5 s), 0 voluntary switches, no callback for the
  rest of the run, also after the quantum went back. The graph's quantum
  changes on a desktop whenever a client with another latency comes or
  goes (a browser, a call, a notification), and a non-real-time worker
  woken late under load (a load's rayon pool on every core) is the same
  case. Under PipeWire a late *callback* (the mix held 600 ms or 1.5 s)
  recovers even on 0.17.3: the xrun is met in writei. (The first round's
  repro, where a 600 ms hold stopped the callbacks, ran through the
  pulse-alsa plug-in, whose xrun path was not read: **open**.)
* **Why underruns happen at all (measured, not the callback).** In the
  device soaks below the callback's longest wait for the mix lock was
  3.5 ms and its longest render 3.7 ms (20 / 8 ms once each, in the
  first 5 s of the main menu), through career loads (the
  the finished-career save load), race starts and quits, free roam and the menus: the game
  thread never holds the lock for long, so a lock-free ring buffer would
  not change anything. The gaps between callbacks follow the graph's
  quantum (85 ms at 4096, 171 ms at 8192 frames) and CPU starvation; they
  are xruns cpal 0.18 recovers from.
* **Fix (not native; the original's DirectSound has no such state):**
  * cpal 0.18.2 (`Cargo.toml`): its ALSA worker prepares on POLLERR and
    on a missed xrun (`poll_descriptors_and_prepare_buffer`); on Linux
    the `pulseaudio` feature (a pure-Rust client, no libpulse; PipeWire's
    pulse server too) is cpal's default host where a pulse socket
    exists, and `audio::mixer::hosts` falls back to ALSA when it does
    not open (e.g. no cookie: the client sends an empty one, which
    pipewire-pulse refuses). NFSU2_AUDIO_HOST=alsa|pulseaudio picks one
    (a check aid). Windows (WASAPI) and macOS (CoreAudio) are unchanged
    (`cargo check --target x86_64-pc-windows-msvc` / `aarch64-apple-darwin`
    pass, built `--no-default-features`: mimalloc's C build needs a
    cross compiler).
  * The stream is opened on a thread of its own (`open_async`): PipeWire's
    ALSA plug-in waits for the format without a timeout (no session
    manager: forever) and the pulse client ~2 s per try. `Mixer::new`
    waits 2 s, then the game starts silent and the stream comes in when
    it opens; the watchdog never waits on it (a reopen given up after
    5 s, then tried again).
  * `Mixer::keep_alive` stays as the last resort: once a frame it reopens
    the stream when its callback has been silent ≥ 400 ms (or three
    buffers and 0.1 s), at most once a second, the old stream dropped on
    a thread of its own. With cpal 0.17.3 it caught this spin (the
    callback stops while the worker spins; dropping the spinning stream
    returned, its thread gone), 0.4–0.7 s of silence, but a reopened
    0.17 stream span again while the quantum stayed long (3 restarts in
    the run). With 0.18 it is not needed for an xrun: 0 restarts.
  * The device is asked for a 1024-frame buffer (~23 ms) where its range
    has it, else its own.
* **Checks (private sound server only; bwrap with its own PID namespace,
  no /dev/snd, an empty /run/user/<uid>):**
  * forced quantum 4096 (1.5 s) and 8192 (1 s), watchdog off: cpal 0.17.3
    silent for good; 0.18.2 ALSA and PulseAudio hosts play on, the
    longest gap one quantum (86 / 170 ms), 0 restarts;
  * the mix held 150 ms every 0.7 s, or 1.5 s once: both hosts play on,
    0 restarts;
  * the sound server killed and restarted: one reopen, ~400 ms silent
    (the PulseAudio case reopened on ALSA, pipewire-pulse still down);
  * a pulse socket that accepts and never answers, and PipeWire without
    WirePlumber: the game runs on silent, the stream opens ~0.5 s after
    the session manager comes up;
  * two device soaks at once against the null sink (`--flow soak`
    NFSU2_SOAK_WINDOW, the frames drawn), chaos every 60 s (quantum 4096
    for 2 s, 16 busy loops for 2 s, quantum 8192 for 1 s, 2048 for 3 s):
    A, the PulseAudio host, a fresh career ×4 (menus, the D-Day intro,
    free roam 240 s, quit, a race and its ending), 26.7 min; B, the ALSA
    host, the finished-career save load and races, free roam, quits ×5, 30.8 min. No
    restart. The longest gap between callbacks 194 ms (A) and 183 ms (B),
    every gap over 100 ms during a forced quantum. B's 940 underruns all
    fell in the quantum windows (4096: 757, 8192: 183), none in play; at
    8192 the ALSA plug-in's buffer leaves 128 ms of zeros in each 171 ms
    for that second. The sink's recording: the longest run of zeros
    inside sound 405 ms (A 3 ≥ 200 ms, B 4, all at a forced 8192 or 4096
    quantum, the sink refilling); no silence that lasted.
* **Device check aids:** the maintainer's probe `audio_xrun_repro` (the busiest thread's CPU
  and context switches, the callback's silence; NFSU2_REPRO_HOLD,
  NFSU2_REPRO_NO_WATCHDOG); `scripts/audio_sandbox.sh [--pulse]
  [--no-wireplumber] CMD` (the private PipeWire); `--flow` with
  NFSU2_FLOW_DEVICE=1 NFSU2_DEVICE_CHECK_OK=1 (the flow on the default
  device, the watchdog run, `device audio:` lines every 5 s: callbacks,
  lock wait, render and gap maxima, `Mixer::take_stats`; in the sandbox
  only).
* **Levels of the finished-career save fixture (native):** its options are master 0.5,
  engine 0.65, tires 0.65, so the player's level 0 is 3735 against
  18236 (CAR p4 −31491 at idle, −27202 with the defaults): ~14 dB
  quieter, as the original's option curves give (round 2, item 8).
* **Check aids:** `--flow engine` (fresh career and the finished-career save in free roam, a
  Quick Race: the CAR event, its levels and the output's RMS; run by
  `tests/engine_audible.rs`); `--flow soak` with NFSU2_SOAK_FPS (frame
  rate), NFSU2_SOAK_WINDOW (an offscreen window: frames drawn, the
  preload run), NFSU2_AUDIO_RATE (the capture's rate), NFSU2_AUDIO_REALTIME
  (a render thread at real time with lock-wait / late-block stats);
  `--bench` takes NFSU2_FLOW_DATA / NFSU2_FLOW_LOAD and prints the audio
  RMS with NFSU2_BENCH_AUDIO. Note: `--flow` uses NFSU2_FLOW_DATA, not
  XDG_DATA_HOME, for the saves (a load from elsewhere fails silently into
  a fresh career).

## Speech and music archive (SDATA/sdat.viv; 2026-10-29)

Ported in `src/formats/viv.rs`, `src/audio/{schl,microtalk,ealayer3,
mp3_tables,speech,music,streams}.rs`; tests `tests/speech_music.rs`, the
selftest's "streams" step, probe the maintainer's probe `speech_probe`.

* **The archive** (**verified** unless marked): a "BIG4" (header kind 3 of
  0x6FA8D0; 0xC0FB → 1, "BIGF" / "Shp" → 2; header size u32 BE at +0xC,
  entries from +0x10: offset, size (BE, width = the 4th character, at
  most 4), name) with 10 files: speech/UG2_Speech.{csi,evt,idx,big},
  speech/nisaudio.{csi,evt,idx,big}, pfdata/MusicSFx.{mus,mpf}. The audio
  start-up 0x486510 looks for `sdata/sdat.viv` in the install folder, else
  under the CD path (0x458890), makes the big-file device (0x6F6750: one
  archive slot of 0x120 bytes, 12 open files of 0x20), puts its name at
  the front of the search path (0x6F6E7A with 1: the vtable +0x30 call
  leaves its pushed 1 as the second argument; 0x6F6DB2 splits the path
  at ';') and mounts the archive (0x6F67D7: 0x40 bytes, the header size
  0x6FA93B, the whole header kept, "BF Header", realcore big_vfs.cpp).
  Opening (0x6F69E6): "bf:" and one leading '/' or '\' dropped,
  "archive|name" picks an archive, else the archives in mount order; the
  entries in order (0x6FAA2E), names compared without case (0x6FA8A0 →
  0x75DED8), separators not folded. The game's own file calls (0x57CA20 →
  0x6F8710) go through the default device (0x6F78DC: no ':' and no
  leading slash), so the archive is searched before loose files
  (**inferred**: the default device's walk was not decompiled).
* **Streams** ("SCHl" / "SCCl" / "SCDl" / "SCEl" chunks, size LE incl.
  the head; "PT" tags as the banks'): the codec class of 0x728DF9 —
  10 EA-XA (the NIS bank, stereo 32 kHz; blocks: +8 samples, +0xC a u32
  per channel, the history carried across blocks), 4 / 0x16 MicroTalk
  (the speech bank, mono 22050 Hz), 0x17 EALayer3 (the music, stereo
  32 kHz). Decoder setup: 0x72CB4B (MicroTalk 0x7328E8 / 0x732B10 /
  0x732BE1), 0x72428B (EALayer3 0x728744 / 0x728469 / 0x72820C).
* **MicroTalk** (0x7343F3 frame, 0x733A9E excitation, 0x733BC8 LPC,
  0x733C54 synthesis, 0x73432C header, 0x734798 output; tables 0x819208
  coefficients, 0x819308 codebooks, 0x819508 commands; state 0xD68
  bytes): see `audio::microtalk`. In a stream block each channel's data
  starts with a byte: non-zero = a header follows (the first block),
  then per frame a marker byte (0xEE: after the frame a BE offset, count
  and that many BE samples overwrite it; state +0xD64, 0x73478B with the
  version > 1). With a lag over 216 the first subframe reads below the
  codebook into the state before it (history, coefficients, gains).
* **EALayer3** (`audio::ealayer3`; tables 0x7D0C30 Huffman trees in the
  mpg123 form, 0x7D0CC0 linbits, 0x7CF620.. alias coefficients, 0x7D1600
  the synthesis window as −32768 · D[32j + k] for k ≤ 16, each row
  twice): per EA frame a byte 0 / 0xEE, then MSB-first the MPEG header
  byte (version, rate, mode, extension), the granule index, scfsi with
  granule 1, per channel part2_3 (12) and 47 more side-info bits, the
  main data, padding; 0xEE frames end with {offset, count, count ×
  channels BE samples} played after the granule (**inferred** from the
  block sample counts, 1152 + 47 = 1199); a 0xEE frame with header 0 is
  that PCM part alone; a 0 byte with header 0 ends the block. Blocks
  hold 2304 samples (4 granules); song 0 decodes to 198.2 s. Output
  (**measured** under Wine): on the MMX path (0x8B4A2C set) the
  granule's 576 subband floats become 16-bit values at 0x72FD95 (×
  16384.0, `fistp`, low 16 bits; the largest seen 22389, no wrap) and the
  fixed-point synthesis 0x730C66 (pmaddwd with 0x7D11E0, >> 13,
  `packssdw`) gives 16-bit PCM that **saturates**. Against the clone's
  decode of the same 16.3 s of song 0: scale 0.99997, residual 7 LSB rms,
  3420 samples at the limits where the clone's float runs past full
  scale. The clone rounds to nearest and saturates (`ealayer3::pcm16`);
  the bit-exact fixed-point path is **open**.
* **Speech banks** (`audio::speech`; loads 0x482A00 NIS, the code at 0x484352 the
  speech bank by language table 0x7ECFA8; completion 0x45AF90 →
  0x45A7C0): `.csi` "MOIR" switch names → ids (bank word 0x1A4B NIS,
  0x721E speech; NIS: SoundFX_Select, NIS_Select_End, NIS_Select_Crowd,
  NIS_Select_Start, as the exe's table 0x7ED01C); `.idx` +0x54 the entry
  count, entries {flags, size, offset, base in the .big}; an entry
  (0x738A20): u16 id, +4 key count (bit 7: played bits), +5 records, +7
  unit − 1 (× 256 bytes), +8 u16 end, records {BE16 offset, keys}; the
  played-stream callback 0x485B10 → 0x484060 adds the entry's base and
  starts the stream (0x7227A3) on `speech/nisaudio.big` (speech object
  +0x38). `.evt`: per event a play step after the dword 0x46439: 3, the
  entry, the constraint count, {mask, component} pairs. Selection: a
  record fits when each key's bit is set in its switch component &
  mask (**inferred**, see the module). NIS events: 0x4000 → 0x4023
  (race starts, NIS_Select_Start set by 0x45A6D0 with the IntroNis flag
  and the camera track's bit), 0x4011 → 0x4024 (NIS_Select_End: endings
  EndingNis0n → 1 << (n − 1), n ≥ 3 → 4; the dynos 0x100, the shops
  0x200; then 0x4000 entering / 0x8000 leaving / the camera track's bit;
  then 1 — tables 0x82BA78 / 0x82BAF8 built by 0x45B160 from "IntroNis%d",
  "EndingNis0%d", DynoExit, ShopExit1, DynoEnter, ShopEnter1), 0x4014 →
  0x4025 (4 × 30.6 s), 0x400E → 0x400E (23 short mono lines). Volume
  (0x484060): the stream's level byte / 127 (0x723E9D).
* **In-game speech** (UG2_Speech, MicroTalk, 857 lines in 14 entries,
  10 events on the switches Career, Career_Magazine, Career_Dvd, Outrun,
  Time, FE_Screen, FE_OptionScreen, Intro_Area, Intro_Events,
  FE_Dynoshop; the exe's name table 0x7ED0DC). Who speaks, when and which
  line: **Rachel's speech** below (2026-10-30; ported).

* **EA Trax** (`audio::music`): the `.mpf` ("xDFP" v4): 162 nodes, 79
  events, 43 streams in bank 0 (layout set up by 0x73A7C0: slot +0x38
  nodes, +0x40 events, +0x4C banks, +0x50 stream table). Song i →
  event i (0x7399E0 on the event table, **verified**) → node 3i (type 1)
  → node 3i + 2 (type 4) whose stream id is **1-based**: song i plays
  table entry id − 1 (song 0, Riders on the Storm: id 39 → entry 38,
  373.6 s; **verified** under Wine by the recorded output's waveform,
  normalised correlation 0.98 while the title showed its caption — the
  old 3i + 2 → entry 39 rule played another song). Events 27–78 are
  the interactive score (table 0x7ECC10 per context +0xB4: 0x1C the
  front end, career shops 0x16–0x1D by the shop visited, 0x46B330; in
  game the "Ambience" parameter map), played in part by the clone
  (Sound effects audit, Round 3).
* **Music manager** (0x482350 each frame → 0x482180 → 0x47B080 /
  0x47B260; player states 0x47AC50 on +0xC0, **traced**): in the front
  end (audio +0x90 = 1) with the menu music level and a menu-mask song
  → EA Trax (0x47B080), else the interactive piece (0x47B260); in game
  EA Trax when game music and master are on, the race mask has a song
  and it is not a drag (0x89E7D8 / 0x890108, 0x4018F0) or drift race
  (0x89E7D9 / 0x890104, 0x401910) — those get the interactive score.
  A new song starts only when none plays (0x82AFD4 = −1, the stream
  ran out or was stopped) — songs never loop, the end of one starts the
  next of the playlist (0x46B6F0: the jukebox order 0x83ACB8 with the
  mask, sequential from +0xA8, or random among the not yet played with
  +0x9C; the remaining mask +0x88 / +0x90 refills when empty) — and in
  game not while PC_Loading is up (0x4A9330) nor with no flow. States:
  1 start (front end, 0x480C30), 2 stop on leaving the game (0x481440 →
  0x47AF40), 5 pause (0x739030(…, 1)): the flow's PRERACE / PAUSE /
  9 (0x481210 with 3 / 6 / 9), PC_Loading over a running flow, any
  game state but 3 / 6, a movie in game (0x8383AC); 6 resume; 7 stop
  on the flow's LOADING / CAMERA_FLYBY (0x481210 with 1 / 2); 0xB stop
  for a movie in the front end, back to 1 after it (a new song). The
  playing song's caption: Chyron_ShowSong. **Clone**
  (`streams::MusicContext`, `Game::music_context`, `Streams::
  music_frame`, `Mixer::pause_track`): front end / game / paused /
  stopped as above; a track load stops the song (`begin_loading`), the
  countdown and pause menus hold it, in-engine scenes stop it in game
  (the front end's pass, 0x47B080 with audio +0x90 = 1, has no scene
  test), a drag or drift race has no music (its interactive score is not
  ported). A scene goes with its track (`Game::drop_drive`). The dyno
  (L1, internal notes): audio +0x90 = 5, the in-game rules with
  the front end's pool (0x46B6F0 for g_RaceType 8); its song plays through
  DynoEnter and is held by the Quit question (flow message 7). **User bug
  fixed** (2026-10-30, "silent after a while, also in the menus"): Quit
  to Main Menu from the pause left a running scene behind (Esc during
  the career's start scene, a race's start scene or a shop scene), and
  the scene test stopped every front-end song until the next city start;
  checked with `--flow soak` (below). Likewise Return to Explore Mode
  from a race's pause kept the race as paused, and every menu after it
  (the garage, the shops, the SMS screens) held the song (`Paused`),
  silent for good once the speech ended: a city start now ends any race
  (`Game::explore`). A decoder thread that dies marks its buffer done
  (`streams::Decoding`), so no track can wait for samples for good. The
  playlist is the clone's own
  (`music::next_song`: table order after the last song, either side, no
  shuffle; 0x46B6F0's per-side positions and random mode are **not
  ported**), every song has a stream (song 0..26 → events 0..26 →
  streams, all 27 decode in full).
* **Audio check aids** (game.rs): `NFSU2_AUDIO_WAV=file` runs the real
  mixer offline on game time for `--shot`, `--selftest` and `--flow`
  (`-` for no file); `NFSU2_AUDIO_METER=s` with it prints every s seconds
  of game time the output's peak and RMS with the mode, music context,
  song, volumes and the mix (voices, tracks, levels), dropping the
  samples so a run can last hours; `NFSU2_AUDIO_STALL=1` a device that
  never plays; `NFSU2_AUDIO_DEBUG=1` the live game's line every 2 s.
  `--flow newcareer` (`NFSU2_FLOW_LOAD=profile`, `NFSU2_FLOW_SMS=esc`)
  plays the career menu's first option as a player keys it and logs the
  D-Day script, inbox and car; `--flow soak` runs `NFSU2_SOAK` phases
  (`menu:s`, `career`, `career_esc`, `drive:s`, `race`, `race_explore`,
  `garage:s`, `quit`, `profile`); `--flow dday` the selftest's D-Day
  save paths alone. Each flow gets a fresh save folder, or
  `NFSU2_FLOW_DATA=dir` (a copy of the profiles; never the player's own). The selftest covers the player's paths:
  `selftest_dday_paths` (a D-Day save's career menu, Resume with the
  script over, Start New Career with the instruction in the viewer, Esc
  and Quit in the start scene with the menu's songs after) and
  `selftest_music_after_race_pause`.
  **Open**: the interactive score, the fades (0x459990 over 300 ms
  between interactive pieces), the shuffle option's source.
* **Volumes** (0x45C210, the options +0x24.. × master × 32767 as mix map
  fields): the clone scales effects (one-shots, loops), engine (the
  granular voices), speech (speech tracks), menu / game music (music
  tracks) by the option × master (`mixer::Levels`); the mix map's own
  ducking of these fields is not applied (**open**).

## Rachel's speech (2026-10-30)

**Verified** in the binary (objdump; REA's Ghidra provider did not start:
no Java) unless marked. Ported in `audio::speech` (`Picker`, `lines`,
`Bank::fit`), `audio::streams` (`say`, `stop_speech`, `speech_free`),
`frontend::dday` (`Hints`, `Script::step`) and game.rs (`say`,
`speech_free`); tests `dday::tests`, `speech::tests`. Under Wine with the
audio driver off the speech object 0x82BA30 is 0 (peek), so every speech
call is skipped and Rachel is always "free": the lines themselves cannot
be checked there; the hint slots and clocks were (below).

* **The line objects.** No hint object is kept: each caller builds a
  two- or four-dword object on its stack, {vtable, +4, +8, +0xC}, whose
  vtable is a one-entry table (0x79AD04 → 0x510F10, 0x79AD08 → 0x510F30,
  0x79AD0C → 0x510F50, 0x79AD10 → 0x510F70, 0x79AD14 → 0x510FA0, 0x79AD18
  → 0x510FC0, 0x79AD1C → 0x510FE0, 0x79B870 → 0x510ED0), and calls it on
  the speech object +0x2C when 0x82BA30, 0x7FAAA8 and +0x2C are all set.
  The methods set one switch's tuple (0x735880 on its block, adding the
  switch with 0x735800 the first time) and, when the speech stream object
  (0x82BA30 +4) has no request pending (+0x44 = 0), queue stream kind 3
  (0x46CA60(3, 0x45B480, 0)); 0x45B480 → 0x45AD40(3) starts the request
  in place of what the stream plays and frees it (+0x44 = 0). Tuples
  (`speech::lines`): 0x510ED0 Career (1, +4, +8); 0x510F10 Time (4, +4);
  0x510F30 FE_Screen (2, 1, +4, +8); 0x510F50 FE_Dynoshop (2, 0x20,
  +4, +8) (0x46D580 registers 0x7ED124 = FE_Dynoshop; FE_OptionScreen,
  0x7ED10C, is only registered by 0x4587A0 and never set); 0x510F70 Intro_Events (0x10, +4, +8 or +0xC by
  Rand_Int(2)); 0x510FA0 Intro_Area (0x20, +4, +8) (0x46D480); 0x510FC0
  Career_Magazine (1, 0x200, +4, +8); 0x510FE0 Career_Dvd (1, 0x400, +4,
  +8); 0x46D2D0 Outrun (x, +4, +8) with x 1 → 3, 2 → 1, 4 → 2, 8 → 4,
  0x200 → 5 (0x510EF0 calls it with 8: the tuple is (8, +4, +8, m(+8)),
  verified in the disassembly 0x46D2D0–0x46D336).
* **Rachel's outrun lines** (2026-10-31, race2; **verified** in the
  disassembly; ported: `Hints::outrun_start` / `outrun_end`,
  `lines::outrun`, game.rs `rachel_outrun`). Both run on the hint object
  (career +0x887C) with the line object {vtable 0x79AD00 → 0x510EF0, a,
  b}, each after Speech_Stop 0x45B490 and only when 0x82BA30, 0x7FAAA8
  and its +0x2C are set; the random draws (0x43C1C0) are made either way.
  Start (0x607190 → 0x532E40 → 0x511000): with +1 set, pick =
  Rand_Int(5) kept at +0x1A, (a, b) = (1, {1, 2, 4, 8, 0x200}[pick])
  (table 0x7993C8); else (0x20, 0x200). End (0x541330 → 0x5110C0(won),
  after the bank, 0x529BE0 and the magazine triggers): with +1 set, r =
  Rand_Int(2), a = won ? (r ? 16 : 8) : (r ? 2 : 4), b = the table at
  +0x1A; else a = 0x40 won / 0x80 lost, b = 0x200, and +1 is set (so +1
  is also "an outrun line was said", besides 0x524340's DDAY_EVENT_A).
  Not heard under Wine (no speech object with the audio driver off).
* **Which event and record** (**verified** in the disassembly and ported,
  docs/native-fixes/N-AUD2.tsv: `speech::Library`, `Streams::say` / `play_nis`). A
  setter stores its switch's tuple (0x735880); the switch's listener
  0x738630, installed per switch by 0x7386A0 from the `.evt`'s switch
  table (switch i → event i), posts the request {event key, the event's
  +0xB values} (0x736920): it passes when Rand(100) ≤ event +9
  (0x73698A) and waits in the channel's queue (0x736860: lifetime +2,
  priority +4; **not modelled**, the clone evaluates at once). 0x738340
  (from 0x7385B0) skips an event at its channel's count limit (+8 & 0xF;
  none of the requested events has one), orders the alternatives
  (0x737470: count +6, alternative k at event + 4 · u16[+0xC + 2k],
  weight (b & 0x1F) · 4^(b >> 5) (table 0x819A94 = 1, 4, …, 0x4000), one
  Rand(total) per non-zero weight, zero weights last in file order) and
  tries them in that order: Rand(100) < alternative +1, the switch masks
  0x7382A0 (+8 >> 4 of them) and the key fit 0x738DD0 (+7 keys; the game
  callback 0x50D510 answers 0), then 0x7377B0: per step (alternative +2
  >> 2 steps at alternative + 4 · byte[+8 + j]; +2 & 3 a channel mode)
  0x737030 resolves the entry and 0x737230 lists its records: 0x7371C0
  (each constrained key's union against the request value), per record
  0x736F30 (constraint j against key j: a bit of its mask and of request
  value (component − 1); components 0 / 0xFE any, 0xFF the state table)
  and 0x737140 (enable bits); a record within the entry's last +6 plays
  (0x735D70, the ring after the key table, written by 0x735D30 from the
  play step 0x737EEB) is only kept as the fallback played longest ago,
  used when nothing else fits. The first alternative with candidates
  plays; a request that fits nothing plays nothing (the retry hook
  0x8B5158 is never set). The shipped banks use one alternative per
  event (weight 0x39 = 100, chance 100, one step, no keys, no switch
  masks, no channel modes, no state components; all ten / four events
  chance 100), so the gates pass and draw; the rings: FE_Screen's entry
  0x2D keeps 1, the five outrun entries 0x28 keep 7. Each setter's
  switch event is the one the clone's old first-fit search happened to
  find for every setter tuple (1376 tuples compared), so the change is
  the draws before the pick, the rings, and the NIS bank's lines now
  picked at random among their fitting records (THUNDERSTRM 9,
  CollisionStream 6, BLOWNENGINE 4) instead of the first.
* **The pick** (0x737B20 → 0x738860, **verified**): a candidate slot
  holds the entry (+4), its first fitting record (+8) and their count
  (+9); the record is first + pick(count, entry). The generator 0x738750
  (five words 0x819AB4…0x819AC4 summed with carries, the counter
  0x819AC8) is seeded by 0x738940 from the library start 0x736180 at
  each speech-library init 0x486220 (0x4866EA with 2, 0x486A6D with 1 —
  the latter on every world entry, a draw read under Wine at 0x486A31): the first
  init's seed is the game's draw Rand_Int(0xFFFFFFFF) on 0x7EA624
  (0x4866F0 / 0x486A2C, through 0x43C140), each later one the last seed
  + 1 (0x82BA34, .bss 0 — not a static 9), taken as |seed| (
  `Library::init`; the game's call is listed for review, until then
  the clone keeps the seed 10). 0x738860 avoids the last min(n / 2, 10)
  picks of the same entry (history 0x8B5688, 32 × {index, key}, ring
  0x8B5708; 0x738800), at most 32 draws; with key −1 it makes one draw
  and touches no history (0x73698A, 0x7374E3, 0x738489 — the request
  gate, the weighted order and the alternatives' chance — draw so, all
  on the one generator, for both banks; 0x737917, the shuffle branch of
  0x737B20, is not reached by the shipped events).
* **Free** (0x525A00, the same test in 0x54BD40 and 0x5112B0): true
  without a speech object; else 0x46D100(0) (**verified** in the
  disassembly): with the speech object's +0x5D set, free outright; else
  0x46C790 on the speech stream's channel — 0x722ECE count 0, or count 1
  with that stream (0x722DB5) gone (state 3) or playing (state 2) with
  under 100 ms left; queued and not started (state 0 / 1) or more than
  one queued: not free — and not free while the NIS stream object
  ([0x82BA30] +8) has +0x4C = 1 (the backup stream: a kind-7 request
  queued by 0x46CD20 from 0x475330 after a kind-2 line, cleared by
  0x46CA10). 0x46D100(1) is 0x45A550 (the channel's current stream in
  state 2) with the same +0x4C test; SFXObj_Speech 0x4751A0 reads it the
  same way. A NIS line itself does not enter either. The clone
  (`Streams::speech_free` / `speech_playing`): a line still
  decoding is "queued", the remaining ms from the track position; +0x5D
  taken as clear. The backup line (**verified**: 0x475330,
  0x4754C0, 0x46CD20, 0x46CA10, 0x479200, 0x45AD40(7); `streams::Backup`):
  when the race start's line (kind 2) starts — NIS audio ready (0x475530
  sets 0x82BC48) and the scene ready to play (0x4791D0 sets +0xB0) —
  +0xB1 is cleared and, while NisAudio's last request kind (+4, written
  at 0x4845F0) is still 2, 0x46CD20 sets NIS_Select_Start = (NisAudio
  +0xC, +0x10, 4) — the scene's and track's values the kind-2 request
  stored at 0x484A56, with 4 as the third — and queues kind 7 (ready
  callback 0x462020 sets 0x82BC4A); +0x4C = 1. Skipping the scene
  (NIS_SkipToCut 0x43B460 → 0x479200: 0x82BC49) with the backup ready
  starts it in place of the line ("Button THrough NIS Playing"). Cleanup
  (0x46CA10, "Cleaningup Backup Stream ..."): with the race status object
  0x890118 in state 3 and its clock +0x14 · 0.00025 ≤ 15 s, once the
  speech stream (0x45A550 on [[0x82BA30]+4]+0x34 +4, Rachel's) is not
  playing; after 15 s in any state; each once per kind-2 start (+0xB1);
  and by 0x4754C0 ("NIS IS OVER", a virtual slot, caller **open**) when
  the speech stream is not playing. The game's calls are wired:
  the start (`race_start_line_started` after the race start's
  `play_nis`), the skip (`nis_button_through` in place of a plain stop)
  and the cleanups each audio update (`backup_update` with the race
  status's state and +0x14 below); 0x4754C0's caller is still **open**, so
  `nis_over` stays unwired. **The race status's clock +0x14** (**verified**
  + **measured** on two runs): written only by the race
  status update 0x6076E0 while the state is 4: +0x14 = 0x865188 − +0x10,
  rounded by 0x577410(0.01) (0x577370: whole 0.01 s, half up, in f32 at
  the x87's 24-bit precision; `standings::round_centi`), so a multiple of
  40 ticks; +0x10 is the green's tick rounded the same way (0x5FC910(4),
  also +0x04 = the exact tick, +0x08 = 0). Before the green it keeps the
  reset's 0 (the scene, state 2, and the countdown, state 3); after the
  race leaves state 4 it is frozen. A skipped scene only shortens the
  scene (199 → 67 frames), the zero is the green either way, so the 15 s
  rule counts from the green. 60 000 ticks × 0.00025 is 15.000001 in
  f32, so 59 960 is the last value inside the window. Clone:
  `Race::status_clock_update` (the world clock `Drive::clock` as the
  tick; frozen once every racer finished, the clone's end of state 4,
  **inferred**), `Race::status`. Still **inferred**: that
  0x46CA10's channel +0x48 stops only a backup already playing.
* **The hint object** (career +0x887C = 0x8635BC, 0x190 bytes; built by
  0x531810, saved with the career block): +1 DDAY_EVENT_A over, +4 the
  complaint count, +8 kind 5's line said, +9 / +0xA the car lot's / the
  crib's once flags, +0xB / +0xC UI_SponsorPopup's once flag / its first
  line said, +0xD + n the DVD rating flags, +0x18 a DVD line pending,
  +0x19 a rating for kind 0xF (written only as career +0x8895 by
  0x5331D0, after its cover loop: ftol of the highest +0x3C among the
  stage's DVD covers (+0x24 = 2 / 4, +0x25 = career +0x7AF8) above the
  visual rating 0x528420, else 0; **verified**), +0x1C / +0x20 the
  neighbourhood line's zone and time, +0x28 a part fitted in a shop, +0x2C
  the covers spoken of (count +0xA4), +0xA5 the stage the next kind-12
  line names, +0xA6 the showcase line said, +0xA8 eight slots {ticks,
  kind, value}, +0x108 / +0x10C the sim clock 0x865188 last / summed,
  +0x110 / +0x114 the driving time in ticks last / summed, +0x118 /
  +0x11C the front-end clock 0x8651A8 last / summed, +0x120 + 4·kind the
  front-end lines' last times (kinds 0–0x1B). The clone persists it
  (`Hints::{to,from}_bytes`, native save `HINTS`; the untraced bytes are
  kept as loaded). **Observed** (Wine, a new career, 2026-10-30): after
  the airport scene the slots held (60000, 2) and (160000, 3); about 24 s
  of driving later kind 2 had fired (0x86347F = 1, slot cleared to zeros)
  and kind 3 had 64095 left, +0x114 = 95905 = the ticks driven, +0x10C =
  +0x11C = 99902.
* **Posting** 0x5111F0(ticks, kind, value): the first slot whose kind is
  0; with none free the post is dropped. Posters: 0x5243E0(4) (the car
  lot's INIT; once +9; FE_Screen (0x1000000, 0x20) at once, kind 4 in
  4000 ticks); 0x5243E0(0x11) (the crib's INIT; once +0xA; FE_Screen
  (0x100000, 0x10), kind 6 in 4000, kind 10 in 0x1D4C0; also sets
  0x8641F8); 0x524250 (kinds 2 / 3 unless queued); 0x5242D0(event, won)
  (a stage-0 race: kind 1 in 4000 unless one is queued; DDAY_EVENT_A
  (0xDD60E402): +4 = 3, +1 = 1; then with event +0x34 ≠ 0 and won: kind 5
  in 4000 while +8 is clear and +0x37 = 1, kind 7 in 4000 when the stage
  record's def +0x28 is the event and the stage is not 5); 0x528330(k)
  (the stage end: unless +0xA5 is set, kind 12 in 0xEA60 = 15 s; +0xA5 =
  the new stage); 0x525400 (kinds 0xE / 0xD with the cover as the value);
  0x525520 (UI_SponsorPopup's INIT 0x4C13DF, once +0xB: the first time
  Career (0x8000000, 0x20) and kind 0xB in 4000, else Career (0x10,
  0x20)); 0x5255E0 / 0x525620 (kinds 8 / 9 in ftol(0x7993BC (1.0) × 4000 +
  0.5), from UI_ChooseUniquePart's messages 0x561730 on Accept
  0xD05FC3A3 (0x561876 / 0x561840): 8 after the part is fitted (0x55C240,
  0x528A00), 9 with +0x151 after the performance unique (0x521440); not
  on the Back question's Yes; `Hints::unique_taken`); 0x551080 (kind 0xF, below).
* **Counting** (0x551080 from 0x5513D7, each free-roam frame while the
  script does not run): +0x118 / +0x11C and +0x108 / +0x10C always; with
  the driving time above 0, +0x110 = ftol(t × 4000 + 0.5) (a reading of 0
  or 0x7FFFFFFF is taken as none: step 0) and its step goes to 0x54BD40:
  every slot with a kind counts down; the best is slot 0 at first, then
  any live slot strictly below the best; it fires (0x53E0C0(kind, value),
  then the slot zeroed) when it has a kind, is at or below 0 and Rachel
  is free — else it waits. Then 0x525660 (career stage 0x862838 ≥ 2, 0x55117F; **verified**,
  `Hints::zone_changed` + `Profile::neighbourhood_line`): the player's
  zone +0x9B8 (0 → nothing) is kept in +0x1C (the first seen with
  +0x20 = +0x114); on a change, when Rachel is free (0x5112B0) and
  +0x114 − +0x20 ≥ ftol(0x7993C0 (300.0) × 4000 + 0.5) ticks, 0x500970
  on career +0x7148 counts the open events (+0x80, 64 slots) with +0x7F
  set and draws Rand_Int(count), then takes that many-th filled slot
  counting every open event, +0x7F or not; its marker (+0x28, 0x4FFC50),
  the kind-0x11 trigger holding the marker's point (+4; 0x5D9C00) and
  that trigger's `NEIGHBOURHOOD_<name>` hash (below): when it is not
  the zone just entered and is one of the thirteen, Intro_Events (0x10,
  bit, 8 or 0x10 by Rand_Int(2) in 0x510F70; only with a speech object)
  and +0x20 = +0x114. Bits by name (0x52573C): JACKSON_HEIGHTS 0x200000,
  PIGEON_PARK 0x100000, BEACON_HILL_EAST / _WEST 1, HOTELS 0x1000000,
  FINANCE 0x800000, FORT_UNION 0x8000, EL_NORTE 0x10, SOUTH_MARKET
  0x40000, STADIUM 0x80, AIRPORT 0x40, COAL_HARBOUR_EAST 0x8000000,
  COAL_HARBOUR_WEST 0x10000000. The zone (0x5EB050, the player update,
  career free roam 0x500B60, not replaying): +0x9B0 = 0x5D9C00(car
  +0x60, 0x11) while it is 0 or every 8th frame (0x7FB724 & 7); when it
  differs from +0x9B4 and is not 0, +0x9B8 = the hash of
  "NEIGHBOURHOOD_%s" (0x50B9C0) over the name 0x5D3DF0 finds for its
  +0x30 among the thirteen (0x800C38, bin hashes; 0 when none); +0x9B4
  = +0x9B0. **Inferred** in the clone: the open list's order for the
  slots', the 8-frame phase. Then, with +0x28, +0xC, no +0x18 and +0x19 = n whose
  flag is clear: kind 0xF (n) in 4000, flag n set, +0x19 = 0; +0x28 = 0.
* **Firing** 0x53E0C0 (jump table 0x53E618; Career = (1, a, b)):
  1 Time (4, 1 / 4 / 8) by the count 0 / 1 / 2 (then +1; the third also
  +0x873E, the markers and HUD; at 3 nothing); 2 Intro_Events (0x10,
  0x400000, 8) and the reveal; 3 Intro_Events (0x10, 0x8000, 1); 4 Career
  (0x2000, 0x20) and SMS_LOC_GO_TO_GARAGE; 5 Career (1, 0x20) once (+8);
  6 Career (0x1000000, 0x20) and SMS_LOC_EVENTS_OUT_THERE; 7 (0x40,
  0x20); 8 (0x80, 0x20); 9 (0x100, 0x20); 10 (0x800000, 0x10); 11
  (0x20000, 0x20); 12 Intro_Area (0x20, {1, 2, 4, 4}[+0xA5 − 2], 0x20)
  for +0xA5 in 2..5, then +0xA5 = 0; 13 Career_Dvd (0x511260(value), 1),
  +0x18 = 0 (table 0x7FA490, 10 covers, else 1); 14 Career_Magazine
  (0x511240(value), 1) (table 0x7FA3F0, 20 covers, else 0x10); 15
  Career_Dvd (0x7FA4DC[value], 4).
* **The D-Day script's lines** (0x5342A0, table 0x53448C): state 3 waits
  for Rachel (0x525A00) and goes to 5 with 0x525910 Career (0x800, 0x20)
  and SMS_INSTRUCTION; 4 → 5 after 2 s when free; 5 → 6 after 2 s, free,
  read and closed, 0x525960 Career (2, 0x20); 6 → 7 after 2 s and free,
  the markers, the GPS and 0x5259B0 Career (0x10000000, 0x20); 7 → 8 at
  once (its 0x525A00 call's result is not used); 8 → 9 (0x534210). The
  career scene: 0x484AF4 starts CareerNis1's NIS line on the NIS stream
  object (+8) and then calls 0x5258C0, Career (0x8000, 0x20) on the speech
  object (+4) — a separate stream object, so both play (**inferred**: the
  stream manager's arbitration between the two objects was not traced).
  0x525860 (once +0xA6, Career (0x10000, 0x20)) is called by the
  showcase's INIT (0x4CF170 on 0x35F8620B, call at 0x4CF452, after it
  enters the menu state 0x4CEA70; `Hints::showcase_line`).
* **Front-end lines** (0x5243E0(kind), career only: 0x850078 = 1;
  ported in menus107 as `Hints::fe_line`, see **Shops, round 3**): 0x8641F8 (set by kind 0x11;
  recomputed from 0x863444 / 0x86344C +0x50) turns kinds 1, 2 and 0x12
  into 0x11; kind 1 with 0x86344C +0x38 = 0x7FC8D0D0 becomes 0. Then a
  kind whose last time +0x120 + 4·kind is set and less than 3600 s
  (0x7993C4 × 4000) of +0x11C ago is skipped; else the speech stream is
  stopped (0x45B490), the line set, and the time kept (also when no
  speech object). Lines (FE_Screen (a, b) / FE_Dynoshop (a, b)):
  0 (0x400000, 0x20) the first time, (1, 0x20) later (0x401820 on its
  time); 1 (0x800000, 0x20), later (1, 0x20); 2 (8, 0x20); 3 (0x10,
  0x20); 5 (0x40, 0x20); 6 (0x80, 0x20); 7 (0x100, 0x20); 8 (0x400,
  0x20); 9 (0x800, 0x20); 10 (0x2000, 0x20); 11 (0x4000, 0x20); 12
  (0x8000, 0x20); 13 (0x10000, 0x20); 14 (0x20000, 0x20); 15 (0x40000,
  0x20); 16 (0x80000, 0x20); 17 none; 18 (0x2000000, 0x20); 19–27
  FE_Dynoshop (1, 2, 4, 8, 0x10, 0x20, 0x40, 0x80, 0x100; b 0x10).
  Callers: 0x55AA3C (UI_PerformanceTuning, 0x1B), 0x55C68B
  (Garage_PreviewPart, 1), 0x562D18 / 0x563521 (UI_ChooseUniquePart, 0xF
  / 8), 0x566EEE (a purchase, 2: also +0x28), 0x568995 (0x12), 0x56AB6A
  (0xE, on INIT), 0x56B648 (rims browser, 5). Corrected and ported: see
  the later note on 0x5243E0's callers (0x563521 is UI_Paint's, 0x55C68B
  UI_ChoosePerformanceCategory's, 0x56B648 UI_DecalMain's; plus 0x53B780
  → 0x525050 and 0x55D9CC → 0x525110; all twelve ported).
