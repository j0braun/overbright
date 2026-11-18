# Frontend reflection lighting and local eye distance

This reuses the ordinary native main-menu RX8 capture from
`../fe-reflection-plane` (PID 972557, same-frame main/reflection views).
No new native session or GPU capture was needed. Original executable SHA256:
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.

The actual production `CarLighting::frontend` calculation uses the retained
base matrix, camera eye/forward, and FrontB's SHOWROOM rig (hash 89D8D95D).
Both body uploads already match: HarmonicCoeff maximum error 1.8e-7 and
LocalEyePos error below 5e-7. An initial scan of all ten asset rigs uniquely
matched SHOWROOM; the next closest harmonic maximum error was 0.05488.
The native raw node retains its rig pointer 0x84FC308, but this capture did
not dump that pointer's target. The numeric match and existing native
main-menu scene-selection path support the rig comparison; no cross-session
pointer-address equivalence is assumed.

For the wheels/brakes, the same-camera affine WorldView cancellation from
the preceding investigation reconstructs each part's actual world matrix.
The probe then calls production `CarLighting::in_axes`, exactly the method
used by `CarRenderer` for these parts. Eight representative rim/brake
uploads per view cover all four wheels; repeated materials sharing those
matrices are omitted. Their HarmonicCoeff already matches within 1.8e-7.

The reflected right-front rim's LocalEyePos differed by 0.05666733 before
this fix. Native emits the raw local vector while it lies within the
frontend's 2.78–8 distance range. The clone unnecessarily inflated its
length using `sqrt(x*x+y*y+z*z+1)` before clamping.

`0x48A6A0` calls `0x5BB1A0`; the latter transposes the axes and explicitly
clears the homogeneous row/column including +0x3C. Therefore the vector's
w is **zero**, not one, when 0x48A7E8 sums its four squared components.
`native-transpose.asm` and `native-eye-length.asm` retain these instructions.
The bounded correction is `local.length()`. It preserves the established
body/part context selection, harmonic calculation, base versus copied mesh
matrices, and reflection order. In-game min/max are both 2.78, so their
nonzero normalized eye output is unchanged.

After correction all 16 captured rim/brake contexts pass: maximum eye error
2.86e-6, harmonic error 1.8e-7. The two body contexts remain matching.
An additional unit test covers zero, below/at/inside/at/above the frontend
clamp interval and the fixed in-game clamp. This is numeric lighting parity
for the retained closed-car capture, not a full image-pixel comparison or
new open-part animation capture.

## Reproduce

```
/usr/bin/python3 reconstruct.py /tmp/lighting-regenerated
NFSU2_ROOT=/path/to/game CARGO_TARGET_DIR=../magazine-target cargo run --release -p devprobes --bin fe_reflection_lighting_probe   # maintainer only (private developer probes)
```

The script regenerates `body-constants.bin`, `camera.bin`, and
`wheel-contexts.bin` from the exact retained uploads plus the prior raw
snapshot, asserting raw base pose and shared camera bytes. Each wheel record
is little endian: u32 call number, u32 view (0 reflection, 1 main), 16 f32
column-major world-matrix values, then 96 f32 shader constants. The manifest
hashes these fixtures and their retained dependencies. The private full
frame/trace remain at an internal run folder with their hashes
in the preceding evidence manifest.

Executed full asset-backed release library suite: **483 passed, 9 ignored**.
`red.log` records the native-fixture failure; `green.log` records all 18
passing contexts. All three regenerated binary fixtures byte-match.
