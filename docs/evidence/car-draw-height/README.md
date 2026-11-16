# Shared native car mesh height

Executable SHA256: `8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.
Read-only static review extends the paired live-state evidence in
`../cover-placement`; this task did not run a new original-game capture.

`native-consumers.asm` retains bounded `objdump -d -Mintel SPEED2.EXE`
excerpts. The frontend caller at 0x631239, racer caller at 0x6316AE,
and vehicle caller at 0x631891 all call 0x620AF0. The alternate traffic
branch at 0x6318F5 calls 0x6162F0. Both routines copy the base matrix and
add the same float at 0x7A66C8 (0.01) to copied translation z:
0x621326–0x621339 and 0x616845–0x61685D respectively. Neither addition
is gated on frontend state, reward kind, or LOD; allocation/culling may
return before it, and later branches may omit particular draws.

Initial body lighting precedes the copy (0x621003/0x621031; traffic
0x616601/0x61662F). Body slots
pass the copy at 0x6244A4, animated parts copy it at 0x6235A1, and wheel
matrix multiplication consumes it at 0x628421 onward. Shadows retain
another matrix. Thus this changes rendered meshes, not physics, wheel
contacts, base lighting, shadow geometry or the shared turntable.

The clone applies the bias once inside `draw_uniforms_at`, shared by direct,
cover, sampled, reflection, queued and fading LOD draws. Body lighting remains
the supplied base context; wheels obtain their existing per-part context from
the adjusted part matrix. Reflections receive the bias after their caller's
base-world construction, matching native order. The separate question of the
FE reflection plane (native uses writable 0x7F45A8) is not changed here.

## Regression scope

The asset/GPU test `native_mesh_height_all_parts_lods_and_batch` loads RX8 and
TAXI and checks actual draw uniforms for all four LODs, translated/tilted/
already-mirrored worlds, moving wheels and one isolated synthetic opened part.
Expected geometry is each unadjusted part followed by world-space +0.01 z.
It also checks body lighting against the original context, wheel lighting
against adjusted axes, and queued uniforms byte-for-byte against direct
uniform assembly. The prior paired original-game matrix regression remains.
This validates uniform assembly, not a new native image-pixel comparison.

Run with the read-only original data root:

```
NFSU2_ROOT=/path/to/game CARGO_TARGET_DIR=../magazine-target cargo test --release --lib native_mesh_height_all_parts_lods_and_batch -- --ignored --nocapture
```

Executed: red failed at RX8 body z (0.29 versus 0.30); green passed 981
part draws and byte-identical batches. Full release library result is retained
in `library-result.txt`; GPU-device test output is in `green.log`.

## Animated-part followup

The original fixture above assumed that an opened part also received a final
world-space +0.01 displacement. The later [animated-part instruction oracle](../fe-open-part/README.md)
corrected that assumption: native carries the copied displacement through
the part animation using B · P · B⁻¹ · M. The GPU fixture now checks this
order for animated parts; the ordinary-body and wheel expectations remain.
