# Authored frontend part animation evaluation

Read-only native main-menu RX8 snapshot: owned display :175, private prefix
a private Wine copy, PID 1019985 on 2026-11-03. `screen.png`
identifies the screen; `snapshot.json` preserves instance/channel identities.
No native hooks or memory writes were used. The session was stopped afterward.
`captured-memory.json` retains address/length-labelled raw reads. `snapshot.py`
and `channels.py` are the exact private-session readers (their runtime paths
are intentionally retained). No original assets, profiles or executable changed.

## Established discrepancies

1. Native loader 0x434AC4 passes instance +0x64 to channel vtable +0x10.
   The captured vtable 0x7C1978 selects 0x6E2110, which reads descriptor
   unsigned-short +0x0A directly. For the 21 RX8 animations the durations
   are 27 (one), 28 (ten), 30 (six), and 31 (four) frames. The clone's
   keys-minus-one inference ended each animation one frame early. The parser
   now retains the channel header duration. Sampling still clamps at the
   final key (0x6E7801), including during the final duration interval.
2. Captured quaternion key blocks select 0x6E7300, which calls 0x6E4450.
   This selects the short quaternion arc, linearly interpolates and normalizes;
   the clone used spherical interpolation. Exact keys go through 0x6E7670
   and are copied without normalization. Both behaviors are now reproduced.

The wrapper evaluator is 0x6E6910; its lazily created key evaluator uses
0x6E3F20/40 and the constructor 0x6E1BE0 enables interpolation. The headless
oracle directly calls original 0x6E7790 with interpolation enabled and the
captured authored key blocks, retaining original callback addresses. It bypasses
lazy allocation/wrappers, not key selection or interpolation arithmetic.

`native-instructions.asm` includes bounded length, evaluator, callback and
normalization instructions. Original executable SHA256:
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.

## Reproduction

The scripts below are kept with the maintainer's private evidence.

```
NFSU2_EXE=/path/to/SPEED2.EXE NFSU2_SNAPSHOT=docs/evidence/fe-authored-animation NFSU2_PROBE_DIR=/private/probe /usr/bin/python3 native_sample.py
/usr/bin/python3 reconstruct.py docs/evidence/fe-authored-animation /path/to/CARS/RX8/PARTS_ANIMATIONS.bin /private/reconstructed
```

The first command requires LLVM llvm-mc, GNU i386pe ld, Wine and NumPy. It
creates its own prefix, executes original instructions in a private headless
probe and writes results.json; no GPU/game launch is involved. The second
independently parses the original ELF animation banks, checks all 42 captured
channels against authored keys and frame counts, and regenerates samples.bin
and asset-match.json. Capture pointer/length assertions precede comparison.

All 21 RX8 quaternion banks are sampled at frames 0, .25, 1, 4.25, 15.25,
29.75, 30, 30.5 and 31 under PC64 and PC24: 378 cases. `samples.bin` contains
13 little-endian floats per case: first/second keys, fraction, native result.
The regression tolerance is 3e-7; the previous slerp regression fails on an
actual authored pair (`red.log`). This is float-tolerance agreement, not a
bit-exact x87 port. Release library: 492 passed, 9 ignored. Duration boundary
and open/close reversal tests now use the native terminal-frame interval.

## Shipped duration consistency

`audit_durations.py /path/to/game` checks every shipped PARTS_ANIMATIONS.bin.
`duration-audit.json` records 29 files, 1170 channels and zero mismatched
quaternion/translation duration pairs, with each asset digest and channel
header. The parser's assignment from either channel therefore agrees for all
shipped data. This does not establish precedence for custom banks whose
paired duration headers disagree; that unsupported case remains unclaimed.

## Door selection and scope

Static 0x61FA0A–0x61FABE agrees with current ANIMSTYLE selection and scissor
wide-body substitution: only style 1 uses a fitted wide-body level >0 and
kind >0 to choose kind+2. No selection correction is justified here. The
snapshot validates all authored regular/scissor/suicide/wide key banks, but
was not a live fitted-door style-change capture. Existing rest/variant and
split-half behavior remains; no claim of complete animation/state or pixel
parity is made. No new GPU image test is needed for the isolated sampling
arithmetic, which has original-instruction fixtures and full library coverage.

## Matrix inverse cost assessment

`inverse_cost.rs` measures the unchanged B*P*B^-1*M formula with black-box
inputs over one million iterations, five release-optimized runs. Observed
22.4–23.7 ns/composition (`inverse-cost.log`), under concurrent CPU build load.
This is a CPU microbenchmark, not frame timing. Production caches the matrix
once per visible animated part per draw_uniforms_at invocation and reuses it
across materials. No optimization or operation-order change was made.
