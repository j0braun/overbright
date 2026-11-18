# Frontend car reflection plane (2026-11-03)

Original executable SHA256:
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.
An ordinary main-menu RX8 capture used a private Wine copy, display 175, PID 972557,
DXVK/RADV 1280×960. Only Return keys dismissed startup screens; no code or
memory writes and no reward detour. Original assets/saves stayed untouched.
The owned game stopped after capture. The complete final frame is
9056810–9073579; the trace tail after that frame is truncated on shutdown.

The read-only `snapshot.py` retains global 0x7F45A8 = 0.15000000596046448,
platform angle 120, active FE node 0x51AE268/renderinfo 0x514E2D0, and camera
bytes/pointers for all views. Active base z is 0.2916250228881836.
Views 1 and 4 point at the same camera 0x8760E0. Raw active-node position and
basis are asserted against the snapshot by `reconstruct.py`.

Native 0x630EDB–0x630F17 negates the basis's world-z row and computes
reflected base z as `2 * [0x7F45A8] - base_z`. Car_Draw then adds its
independently established +0.01 mesh-copy bias. This is separate from
physical placement and from shadow geometry.

`constants.txt` retains the same-frame reflection car upload 9057128,
main car upload 9064743, viewport boundaries, and successful Present.
Registers c14–c16 are the affine WorldView rows. The camera is shared,
so `inverse(main_WorldView) * reflected_WorldView` cancels that common
view transform. Multiplication by the live main base plus the independently
verified mesh bias reconstructs the reflected world matrix. This does not
assume a guessed camera look-at matrix or identical projection matrices.

The reflected draw translation z is 0.0183751323; inferred plane is
0.1500000776. Its full matrix differs from the live-plane construction by
at most 1.27e-6. The old z=0 construction misses z by 0.300000155 m.
Earlier documentation inferred z=0 from WVP comparison, but the paired
main/reflection WVP depth rows differ; projected inversion does not cancel
these different projections. The affine WorldView evidence supersedes it.

The correction only changes the frontend car reflection caller to
`garage::reflected_car_world`, using the existing `TYRE_HEIGHT` constant.
Both geometry and lighting receive that reflected base, then the renderer
applies its shared +0.01 mesh bias. Main/cover poses, physics, shadows and
in-game reflection captures are unchanged. The writable native global is
observed at its shipped value here; runtime modifications to that global
are not an exposed clone setting. Full native image parity is not claimed.

Regenerate:

```
/usr/bin/python3 reconstruct.py constants.txt native-snapshot.json
```

`matrices.bin` packs 32 little-endian f32 values: the JSON base columns,
then reflected draw columns. Manifest hashes retain these inputs/results;
full trace/frame remain private in an internal run folder.

## Executed validation

- Native paired-matrix regression failed before correction with 0.300000155 m
  z error (`red.log`); corrected asset-backed library suite: 482 passed,
  9 ignored (`library-result.txt`).
- Release game build passed. Real 640×480 `--shot qrcars` and 320×240 reflection
  readback passed (`gpu-smoke.log`, `clone-qrcars.png`, `clone-reflection.png`).
  The reflected car is visible; the clone diagnostic uses a different car
  from native RX8, so these are rendering smoke evidence, not pixel parity.
- Extractor regeneration byte-matches `matrices.json`; its f32 serialization
  byte-matches `matrices.bin`. Parent independently reproduced the fixture.
