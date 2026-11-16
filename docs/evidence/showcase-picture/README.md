# Kind 1/2 showcase picture save

The original `SPEED2.EXE` inspected read-only has SHA-256
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.
Raw instruction extracts in this directory were produced with GNU `objdump -d
-Mintel --start-address=... --stop-address=...`; no installer or original game
write was involved.

## Established native flow

* `0x4CEDC0`: Accept opens the save question; Yes (`0x11223344`) at
  `0x4CEE11` clears byte `0x7FA29C` and sets the UI update counter +0x9C to 1.
  No (`0x11223345`) takes the leave path. Back can retake before a save is armed.
* Tick (`0xC98356BA`) at `0x4CF0C7` waits while the dialog is present, then
  increments the counter while it is at most 4. Once greater than 4 it calls
  `0x510AE0` (returns 432), `0x510AD0` (returns 320), then `0x5CE520`, and
  arms the capture through `0x488B50`. This is five UI **update ticks**, not a
  timer in seconds or a frame-rate cap.
* `0x5CE520` stores those arguments at capture +0/+4 and allocates a 512×512
  capture resource. `0x5C3130` later reads +0/+4 for the framebuffer crop.
  Thus 320×432 is the logical capture extent, not offsets or identifiers.
* `0x4B59D0` checks capture readiness through `0x488B80`; only then does it
  restore `0x7FA29C`, obtain the actual capture buffer/resource, set memory-card
  operation 2, and open package 0xCB1. The clone previously mapped both answers
  directly to free roam and had no world-picture buffer.
* `0x53790D` checks byte `0x7FA29C` before the cursor drawing call. The pending
  clone save similarly hides its cursor (and its optional debug FPS overlay),
  while the authored showcase masthead/extra remain present.

## Crop and implementation

`0x5C3130` scales the logical width by framebuffer width and the float at
`0x7A3C28` (`0x3ACCCCCD`), and logical height by framebuffer height and the
float at `0x7A3C24` (`0x3B088889`). `0x75BED4` converts each to an integer
with truncation toward zero; the following signed integer half-size operation
truncates separately. The integer framebuffer centres are calculated separately.
Left/right have fixed offsets −10/+3, top/bottom −4/−6. At 640×480 this is
(150,20), 333×430. The offsets remain pixels at larger resolutions; scaling
that whole rectangle would be incorrect. Tests include 4:3, wide, ultrawide,
4K and odd dimensions. Out-of-bounds tiny targets are clamped safely.

Yes restores the same `ShowcasePreview` and camera picks with a five-update
countdown. Picture mode freezes driving. After the countdown, the next complete
world frame is rendered with its ordinary post passes and showcase UI to a
private target, read back, and copied back to the normal output for presentation.
This avoids assuming that a window surface supports texture readback. If no
world frame is ready, the request remains pending. No frontend car or replacement
cover image is generated. The crop feeds the existing `native_save::picture`
512-square container and existing cover numbering/atomic writer. Kind 3/4
reward covers keep their separate renderer and pixel source.

## Verification

`game --selftest-showcase` always selects a unique temporary data directory
before loading settings. It creates a diagnostic profile and never loads an
existing user save. `NFSU2_BENCH_SIZE=WxH` sets output size;
`NFSU2_SHOWCASE_EVIDENCE=/private/output` optionally saves the captured crop and
the decoded 512-square picture. The diagnostic checks No, retake, five ticks,
frozen car position, saved bytes against the presented real-world crop,
saved-view reload, a deliberate write failure and retry, and existing kind 3/4
GPU reward saves. `cargo test --release --test showcase_picture` checks crop
arithmetic, input locking, waiting without a rendered frame, RGBA/BGRA orientation
and native-container roundtrip.

The initial flow diagnostic failed with `showcase Yes must capture/save; only
No may directly leave`, before any GPU capture. Static evidence establishes the
native control and capture path, not pixel-perfect scene rendering. Existing
world lighting, camera behavior and authored-layer timing remain the renderer's
behavior; this change makes no new full visual-parity claim. Native capture
resampling and memory-card timing are represented by the already-existing
picture writer, not newly claimed byte-identical native output.

Final verification on RADV/reference desktop GPU: all three GPU diagnostic runs at 640×480,
2560×1080 and 3840×2160 exited successfully. Each verified both showcase kinds
and both existing reward kinds, save/reload, cancellation/retake and deliberate
write failure/retry. The `Is a directory` message in each log is the expected
failure injected under the test's private save directory. All 456 non-ignored
library tests passed (seven explicit GPU tests ignored), as did the three
`showcase_picture` tests. The two committed sample images were visually
inspected: actual world/car and authored cover layers are present. Wide-output
images retain the renderer's existing UI layout/borders; those are not new
claims of native framing parity at every aspect ratio.

Reproduce in a private cache/target, with read-only original game data:

```sh
export CARGO_TARGET_DIR=/tmp/nfsu2-magazine-target
export XDG_CACHE_HOME=/tmp/nfsu2-magazine-cache
export NFSU2_CACHE_DIR=/tmp/nfsu2-magazine-cache/assets
export NFSU2_ROOT='/path/to/original/game'
cargo test --release --test showcase_picture
cargo test --release --lib
cargo build --release --bin game
NFSU2_HEADLESS_ONLY=1 NFSU2_BENCH_SIZE=640x480 \
  NFSU2_SHOWCASE_EVIDENCE=/tmp/nfsu2-magazine-images \
  "$CARGO_TARGET_DIR/release/game" --selftest-showcase
```

Repeat the diagnostic with `2560x1080` and `3840x2160`. Its dedicated command
always creates a new temporary data directory; the original profile directory
is never selected. The evidence option writes only diagnostic PNGs to the
explicitly provided directory.
