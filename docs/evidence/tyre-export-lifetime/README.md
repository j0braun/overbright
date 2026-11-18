# Native exported forward-speed lifetime: contact capture

Source EXE SHA256:
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.
This is native live memory, not generated clone expectations. Private game,
prefix and display174 only; metadata retains session/PIDs. Original files and
user saves were not edited. The owned prefix was started fresh, navigated to
Quick Race / Circuit / default track / Peugeot206 / Auto, and stopped after
capture. The reference navigator needed one manual Return at profile entry;
final transmission selection was manual. No memory position/velocity/input
state was fabricated. Up and Left were actual keys; their event indices are
retained. BackSpace did **not** cause reset: no event6–9 was observed.

## What each hook sees

All hooks replace exactly one original CALL, preserve GPRs/flags and FXSAVE
state, run its original callee and preserve stack cleanup/return behavior.
The installer asserts the original bytes against the hashed EXE, pauses at the
menu and SIGSTOPs for patch/restoration, then verifies all restored bytes.
The source ELF `.text` zero padding at782B00 holds the recorder; telemetry is
8BBE00; VirtualAlloc supplies128MiB for16384 slots of8192 bytes. Captured code
and script hashes, patches and original bytes are in contact-reset.json.

| Event | Original call | Boundary |
|---|---|---|
|1|5B72EB→43BDF0|ordinary physics mover entry, before its setup/integration|
|2/3|5B74E1→5A5550|immediately before/after mover export|
|4/5|5B2794→5AF0F0|contact response after rewind and pre-response component refresh; then immediately after response|
|6/7|5F52F1→59F890|placement clear, after switching mover0; not observed|
|8/9|5F536F→592170|placement velocity installation; not observed|

The record includes car, mover and body identities, world counter, native
ticks, saved registers and full FXSAVE (control word007F in every record).
It copies full-car0..A00, ordinary body's0..220, mover0..200, and the contact
pair0..100 when relevant. Nonphysics modes leave body/mover payload absent
(zero) explicitly; the header still retains the current mover pointer. No
cached pointer is substituted across reset. Body is captured only in mode1.
The contact's +0C/+10 identity must include this car's collision body.

## Audited result

7276 records /59,604,992 raw bytes, zero dropped, every hook restored.
2248 complete mover-entry→export-pre→export-post triples;266 complete contact
response-pre→response-post→mover-entry→export-pre→export-post sequences.
All sequence indices are consecutive; each sequence retains its world/tick.
Each mover retains the scalar from entry through export-pre. Each export-post
is bit-exact PC24 `(vz*az + vy*ay) + vx*ax` using the newly exported vectors.
No tolerances are involved in these assertions.

All266 responses retain full-car+42C.257 change authoritative body momentum.
All266 subsequent mover entries differ from the current body projection;
those entries retain the prior exported scalar, including through export-pre.
Example world1299/tick67829: scalar18.134641647338867 survives response and
re-step entry; export finally publishes14.150827407836914. The preceding
export has different momentum from response-pre because rewind/refresh have
already run: event4 must not be described as contact entry or outer-frame entry.

This observes a player car in one circuit/contact scenario. It does not prove
all contact types, NIS/Melo/traffic transitions, placement or a full native
force trajectory. The separate static writer audit lists these remaining
prerequisites. It does not justify changing all `speed()` readers together.

## Reproduce/audit

Build recorder with GNU `as --32`, `ld -m elf_i386 -Ttext=0x782b00
-e mover_entry`, then `objcopy -O binary --only-section=.text`; verify hook
symbol addresses against capture.py before installing any changed recorder.
After the owned private fastgpu session reaches green (the scripts are kept with the maintainer's private evidence):

```
python3 capture.py WINE_PREFIX contact-reset --seconds 16
python3 decode.py contact-reset.json
python3 test_decoder.py
```

Stop only that owned session using its fastgpu.sh stop command even on failure.
The retained decoder checks size/hashes/no drops, pointers, modes, PC24,
projections and exact sequence adjacency. Its test also rejects truncation,
target/body pointer corruption, phase corruption, wrong CW and dropped records.

## Follow-up: natural R-key reset

`key-reset.*` adds3823 records with a separate retained recorder/script variant
(`capture-reset.py`, `recorder-reset.s/bin`). It used a fresh owned session,
same Quick Race setup, actual Up for6s then lowercase `r`, then Up for4s.
No reset flag or placement state was written externally. The additional hook
at6034BA records the call to6009B0 **after** the event handler's eligibility
branches; its saved native frame records action0xA1 and pressed byte1. This
establishes that R reached the reset action and passed its gates in this run;
it does not prove every reset eligibility branch. Human racer pointer is
full-car+18, not the different pointer stored at full-car+20.

The variant also snapshots32 bytes at the saved native EBP at header768.
For events6–9 this is the5F52D0 frame: return address at+4, position pointer
at+8, requested speed at+C, heading at+10. For event10 it is the input-handler
frame, whose action and pressed fields have different meanings. Preserve that
distinction when decoding. The retained variant still fits the same zero
padding and preserves the original calls, register and FXSAVE state.

One exact consecutive sequence is retained, all world5026/tick176449:

|Record|Event|Scalar|Placement argument|Mode|
|---|---|---|---|---|
|2196|reset dispatch|20.95913314819336|not a placement frame|1|
|2197|reset pre|20.95913314819336|8|0|
|2198|reset post|0|8|0|
|2199|velocity pre|8|8|0|
|2200|velocity post|8|8|0|
|2201|mover entry|8|—|1|
|2202|export pre|8|—|1|
|2203|export post|8.056135177612305|—|1|

Native return address5FBFE0 identifies the road-placement path5FBEE0→5F52D0.
Installed velocity is exactly(-8.099270820617676,-0.10871096700429916,0),
bit-exact PC24 axis×(8+0.1). The scalar stays8 into the next ordinary mover.
Body/mover payloads are explicitly absent during mode0; no stale physics
pointer is substituted. The first subsequent ordinary mover has the original
physics mover/body addresses again. The reset was an input-handler placement
**before** that mover; this is not a capture of6012C0's post-mover pending-AI
recovery callback, though their shared5F52D0 writer is statically traced.

This run also has1266 complete mover triples and10 contact/re-step sequences.
Zero dropped, hooks restored, own Wine/Xvfb stopped. Both captures and negative
checks pass the same decoder. `capture.py` and its original recorder remain
unchanged, so their metadata hashes still identify exactly what produced the
first capture. The first failed BackSpace attempt remains retained separately.
