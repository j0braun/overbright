# Outrun speech entry selection

The missing win speech was an index-entry selection bug. `UG2_Speech.evt`
has one play step for event 0x12, not separate play steps for each voice.
Its descriptor at file +0x10C is 0x01040028: entry id 0x28, tuple component
4, mode 1. Five index entries share id 0x28. Their header +2 selectors
1–5 identify different audio bases. Previously the parser discarded both
the descriptor mode/component and the index selector; lookup always used
the first entry. Only voice mask 2 matched that entry's records.

## Native trace

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

## Evidence and validation

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
