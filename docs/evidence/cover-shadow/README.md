# Reward-cover FE shadow (2026-11-03)

This is an instrumented **native renderer** capture, not evidence that natural
career reward progression completed. It ran in a copy of a private Wine
prefix; the original game, assets and saves were read-only. Private display :175, DXVK/RADV, 1280×960. The selected FOTF DVD
is kind 4; kinds 3 and 4 use the same reward constructor and cover rendering.

Natural stage-3 DVD navigation reached the reward SMS and paid the reward, but
remained in game state 6 with the reward package requested. That bounded attempt
was stopped. A fresh main-menu session used the existing Career action's native
package transition (0x537990), with its package immediate at 0x4AED92 temporarily
changed from 0x78F590 to 0x78F958 (`UI_MagazineReward.fng`). Selected cover
0x837FD8 was 0x6298F309 and reward flag 0x838578 was 2. The code immediate was
restored after entry. `instrumented-entry.json` records exact process writes.
The unchanged reward constructor selected camera 0x05582E08, environment
0xFDF6041D, and the four hashes in `native-cover-state.json`.

The native main-menu car was RX8. With the cursor hidden, only shadow enable
0x870CC0 changed 2→0→2. On versus off changes 23,796 pixels, bbox
[297,541,714,669]; restored is byte-identical to on. Both images and comparison
are included. Cursor/shadow were restored and the owned Wine session stopped.
No downloaded executable or installer was run other than the original game and
existing diagnostic tools. No original file was patched.

## Draw evidence

`manifest.json` identifies the original executable, complete extracted frame,
raw trace/text SHA256, and vertex blob digests. The 632 MiB trace stays in the
private runtime. Stopping Wine leaves an incomplete trailing frame, but extracted
calls 60573166–60582937 end in a successful Present and are complete.

The frame's relevant sequence is:

1. Background 512² FE quad, call 60573364 (clip corners -0.5375,0.920833 to
   1.0625,-1.2125), then UI text. The background is therefore already present
   before the main car draw; it is not one of the later three layer quads.
2. 320×240 reflected-car view, then main 1280×960 opaque car draws.
3. Three shadow strips, calls 60580460/60580485/60580510: six triangles each,
   eight vertices, stride 24. Raw z=0.152, D3DCOLOR=0xFE808080, UV thirds.
4. Later blended car draws, then the frame's postpasses.
5. Three 512² FE layer quads at 60581751/60581806/60581861 (same quad bounds),
   then remaining reward UI/cursor.

`native-shadow-draws.txt` contains the full explicit state around those strips:
RGB writes only (7), SRCALPHA/INVSRCALPHA, alpha test off, depth LessEqual,
depth writes off, cull none, fog off. The native queue (0x5CEB40/0x5CF430)
is non-late then late, order 0..7, effect 0..22, first-use texture groups within
each bucket, submission order within a texture group. Car effect 5 precedes
shadow effect 13 at order 2. Blend alone is not an ordering key.

## Geometry and limits

`cover_shadow_probe` compares all 24 native strip vertices with the existing
stock RX8 grid: z, UV and color are exact; relative xy shape differs by at most
4.77e-7 metres. There is a constant raw xy offset of approximately
(-0.0115574,+0.0203195) metres. These are vertex inputs **before** eye-pull.

`matrix-decomposition.json` separates that offset from bias. The norm of the
shadow WVP's final row xyz gives scale 0.9890214426. Using the selected camera
eye (-0.060200006,-4,1.2882), the eye-scale translation is
(-0.00066091,-0.04391423,+0.01414258). Removing this bias from the shadow WVP
and multiplying the inverse reconstructed VP by car WVP registers 10..13 gives
first car world translation approximately (-0.01156,5.42033,0.30162). The clone's
turntable is (0,5.4,0.291625). The xy difference matches the raw shadow offset
within trace float precision; native reconstructed car z is also about 1 cm
higher. The flat shadow grid itself stays at z=0.152 in both. This is an inverse reconstruction, not a direct memory matrix read.
The source of that native car-position difference remains open. This patch
keeps shadow and car together using the clone's existing transform.

The patch adds the missing pass and native queue split to reward covers only.
It does not claim complete native composition parity: inherited camera/car
position, postprocessing, fitted-part submission order, neon, and other car
lighting differences are outside this correction. Kind-1/2 SHADOWFE selection
timing is unchanged. Existing world/garage shadow order and write masks are
unchanged. Authored background and foreground artwork are preserved.

## Reproduction

Use a private target/cache and the read-only verified game root:

```
CARGO_TARGET_DIR=../magazine-target cargo test --release --lib cover_
CARGO_TARGET_DIR=../magazine-target cargo build --release --bin game -p devprobes --bin cover_shadow_probe
NFSU2_ROOT=".../Need for Speed Underground 2" ../magazine-target/release/cover_shadow_probe --gpu
NFSU2_ROOT=".../Need for Speed Underground 2" XDG_CACHE_HOME=../cover-shadow-runtime/cache NFSU2_SHOWCASE_EVIDENCE=../cover-shadow-runtime/clone NFSU2_BENCH_SIZE=640x480 ../magazine-target/release/game --selftest-showcase
```

The showcase diagnostic uses its own unique temporary profile. Its kind-3/4
checks toggle the actual shadow setting, redraw the real cover, require a pixel
change and exact restoration, and verify saved-file pixels. The GPU probe checks
that shadow RGB changes while every destination alpha byte stays at sentinel 77.

## Results

- Asset-backed library suite: 467 passed, 7 ignored. An initial run without
  NFSU2_ROOT failed only the existing map-pack test requiring real assets;
  rerun with the verified root passed.
- Native relative grid: maximum error 4.7683716e-7 m across all strip vertices.
- GPU alpha fixture: 624 changed RGB pixels, all 4,096 alpha bytes unchanged.
- Real cover path: kind 3 changes 661 pixels; kind 4 changes 4,207 pixels;
  both restore exactly. Saved-file pixels pass for both kinds, and existing
  kind-1/2 capture/save/reload plus write-failure/retry checks pass.

Clone on/off crops and focused test logs are included. The differing native RX8
and clone diagnostic PEUGOT preclude full-image pixel-equivalence claims.

### Placement followup

The later same-session capture in `../cover-placement` separates the base pose
from the mesh draw. The old xy difference appears in the traced session's live
base pose, while the untraced session matches the clone. The platform angle
was not retained for both sessions, so the cause is unresolved and no xy
compensation is justified. The 1 cm z difference comes from the native mesh-matrix copy at
0x62132D, not the turntable. Reward-cover mesh draws now apply that adjustment;
shadow geometry and base placement stay unchanged. See the paired live-state
and draw-matrix regression there for the stronger evidence.
