# Reward-cover mesh height (2026-11-03)

Followup to `../cover-shadow`: do not compensate the old capture's xy offset.
The native base transform and the mesh draw transform are different objects.

## Native source and paired evidence

Executable SHA256 remains
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.
A private Wine copy, display 175,
DXVK/RADV 1280×960. Original files/saves were untouched. The same bounded
instrumented main-menu→reward entry described by `../cover-shadow` was used.
It is renderer evidence, not natural career-progression evidence.

The FE car list is at 0x8A30F0; node+8 is its renderinfo, +0x910 position,
+0x920 basis. Physics is embedded at node+0x20, with its record at +0x50;
record+0x10+0x30i holds wheel position/radius. Renderinfo+0x10/+0x20 is the
shadow box. Native 0x4A6670 builds the base pose from the platform-disc center,
upgraded wheel record, and hydraulic corner heights. The shared clone
`Turntable` was not changed.

Two sessions retain different live base positions; they do not establish why:

- Without apitrace (PID 951584), raw `fe-car-57be470.bin` has base position
  (0,5.400018692,0.291625023). Renderinfo is `renderinfo-57beff8.bin`.
- With apitrace (PID 954498; see `instrumented-entry.json`), the active node
  `fe-car-519f4f0.bin` exactly matches the position and basis in
  `native-snapshot.json`: (-0.011557341,5.420319557,0.291625023).
  The retained snapshot reports platform angle 120 degrees; live camera eye
  agrees with the selected photo record.

The untraced JSON/global snapshot was superseded by the paired run, so the
platform angles cannot be compared from retained evidence. These are not
controlled same-angle sessions: neither tracer causation nor an xy placement
correction is established. No xy offset is applied to the clone. The earlier
513e2f8 dump was an inactive node and is excluded from this fixture.

The paired trace has a complete final frame at calls 5022428–5032199. First main
car constants are 5026702, shadow constants 5029713. `paired-matrix.json` removes
the separately measured shadow eye-pull from its WVP and reconstructs the car
mesh matrix using the live camera. Comparing that with the same-session live
base transform yields translation delta
(+3.54e-8,+6.88e-6,+0.0099981664) metres. Error is bounded by text-dumped float
precision. `paired-matrix.bin` stores the two f32 column-major matrices for the
regression. `manifest.json` identifies raw trace/frame and node dumps.

This agrees with explicit native instructions: at 0x621326 the current base
matrix is copied into EBX, then 0x62132D–0x621339 adds float 0x7A66C8=0.01 to
its translation z. Ordinary body slots pass that matrix to 0x5C5940 at 0x6244A4;
animated parts copy it at 0x6235A1, accessories at 0x626391/0x62641F, and wheel
matrix products use it at 0x628421–0x62848B. The original matrix remains the
source for the shadow grid and initial body lighting context. This is a draw
adjustment, not a physical/turntable placement adjustment.

## Scope and validation

Only `CarRenderer::render_cover` uses the corrected mesh matrix. It raises
mesh geometry in world-space z by 0.01 before per-part wheel/animation
transforms. The supplied body-light context and shadow callback retain the
base transform. Other render entrypoints and garage placement are unchanged.
The native Car_Draw adjustment is not reward-gated and appears broader than
this patch. Other car/garage consumers need their own review and regression
evidence; this change does not claim complete coverage of the native bias.

The regression transforms several RX8-sized points through the live-base
input and compares them with the native draw matrix (3e-5m tolerance), so it
fails for the old base-only draw. The existing cover GPU diagnostic checks both
reward kinds' shadow toggle/restoration and saved output; kind 1/2 capture and
saved-view reload remain covered. Full native image parity is not claimed.

## Regenerating the fixture

`paired-constants.txt` retains the exact two shader-constant uploads plus the
main viewport and successful Present. The full trace/frame stay private;
their hashes remain in the manifest. Regenerate the expected JSON with:

```
/usr/bin/python3 reconstruct.py paired-constants.txt native-snapshot.json
```

`paired-matrix.bin` is little-endian f32: the 16 base matrix values followed
by the 16 reconstructed draw values, each in column-major order from the JSON.
The manifest hashes the retained inputs, script, and both fixture formats.

## Executed checks

- Before the correction, the native matrix regression failed with origin z
  error -0.00999817 m (`red.log`).
- Asset-backed release library suite: 474 passed, 7 ignored
  (`library-result.txt`). Release game build passed.
- Real 640×480 draw-path diagnostic: kinds 1/2 capture/save/reload passed;
  kind 3 shadow changed 686 pixels and kind 4 changed 4312, both restored
  exactly. Write-failure/retry and existing reward saves passed (`gpu-cover.log`).
- Regenerated JSON byte-matches the fixture, and its f32 serialization matches
  the binary fixture. The extractor first verifies the active raw node's
  position and basis against the snapshot.
