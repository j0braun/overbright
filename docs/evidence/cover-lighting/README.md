# Reward cover lighting investigation (2026-11-03)

Original `SPEED2.EXE` SHA256:
`8f6a9ae2c4cfa01181d2fd755d99c6f4b7103f60273908ea1de1e719dfe2a297`.
Original assets and saves were read-only. Investigation worktree starts at
`086193d`; all caches, test profiles and captures are private.

## Established selection

* `0x523E42` looks up the selected environment record (`0x510750`). If found,
  `0x523E55` passes its +4 light hash to `0x4A7990`, before the cube is loaded.
* `0x4A7990` uses `0x48A2D0` and stores the resulting setup pointer in render
  info +0x904 for both FE car objects, and FE car +0x6C.
* `0x48A2D0` looks through the live table at `0x82DADC` / count `0x82DAE0`,
  stride 0x74, returning static DEFAULT (`0x7F3E10`) for a missing hash.
* The chunk loader `0x48A280` is registered indirectly: its pointer sits at
  `0x7FA8DC` in `0x570D80`'s dispatch table `[0x7FA858,0x7FA8E8)`.
  It installs chunk 0x3B700. The corresponding unload handler is `0x48A2B0`.
* FrontB.lzc contains ten setups. The native process snapshot also has ten,
  including exact matches for every magazine environment's three lights.
  `light-tables.json` records the asset interpretation; `native-light-table.json`
  records the live fields and SHA256 of the 1160-byte table.

Snapshot provenance: reflection worker's owned Wine prefix
a private Wine copy, PID 888452, display :157.
The worker read `peek.Session.u32(0x82DADC/0x82DAE0)` and then
`read(pointer,count*0x74)` while `fastnav.py freeroam` navigated a fresh profile.
The exact screen was not recorded. This proves the native table contents in
that session, **not** a captured cover-time draw or its full visual appearance.
The selection itself is established by the native instruction chain above.

| Environment record | Light hash | Intensities |
| --- | --- | --- |
| C2082D5F (Showroom) | 89D8D95D | 0.4 / 0.5 / 0.5 |
| 64C95FAE (ElaborareGT) | F6044827 | 2 / 0.457 / 0.924 |
| FDF6041D (Showroom) | 965628BB | 2 / 2 / 1.217 |

Directions and colours differ too. Selecting by cube filename would conflate
the two Showroom rigs. The previous clone used fixed IN_GAME lights
(0.5 / 0.35 / 0.1) whenever a cube loaded, discarding the environment's light hash.

## Shadows and remaining limits

The existing original cover comparison shows SHADOWFE, absent from clone
`draw_cover`. Existing native FE shadow recovery establishes a flat grid at
z=0.152 (no inner raise), RGB 0x808080, alpha 254 and eye pull 0.104 for the FE
camera. The WorldNoDepth effect has depth testing, no depth writes and no fog.
Those parameters alone do not establish its position in the cover's FEng/car
passes or how it blends with the authored background. No speculative shadow
pass is added by this correction. Blend order relative to car transparent
materials remains open.

No-environment native selection skips the rig store entirely; its retained
value depends on previous FE state. The clone's existing DEFAULT behavior for
that case remains an approximation. This investigation does not resolve that
state history, synchronized random choices, paint/reflection parity, or the
full native FEng composition order. Kinds 1/2 retain their in-world lighting.

## Regression commands

Use a private target and XDG homes, with `NFSU2_ROOT` pointing to read-only
original data:

```sh
cargo test --release --bin game cover_uses_selected_environment_light_hash -- --ignored --nocapture
cargo build --release --bin game
NFSU2_HEADLESS_ONLY=1 NFSU2_BENCH_SIZE=640x480 \
  NFSU2_SHOWCASE_EVIDENCE=/private/cover-images \
  "$CARGO_TARGET_DIR/release/game" --selftest-showcase
```

The asset-backed regression failed before the correction: environment
C2082D5F/light89D8D95D received white theta200/phi90/intensity0.5 instead of
its authored theta149/phi100/tinted/intensity0.4 light. It exercises actual
`Game::cover_view` selection, not just the light-table parser.

A related kind-1/2 question remains: native car texture setup
`0x6387DE..0x638840` selects SHADOWFE if either frontend mode is active or
reward flag `0x838578` is nonzero. `UI_ShowCase_Preview` sets that flag through
`0x510C50/0x510C60`. Clone in-world shadows always select SHADOWIG. The timing
of native render-info texture setup relative to entering showcase is not
established here, so this is a conditional-path discrepancy to investigate,
not proof of which texture the existing world car uses in a saved photo.
Showcase camera pull 0.023 is already implemented. Grid/alpha still depend
on frontend versus world state separately; selecting SHADOWFE alone would
not justify applying frontend geometry or alpha in the world.


## Verification result

The final asset-backed regression passed all eight selections: all three
native rigs with cube present and absent, an unknown light hash and an absent
environment record. Release game build succeeded. Existing build warnings
were unchanged.

The real headless GPU diagnostic at 640x480 exited 0 and passed both showcase
kinds plus both reward kinds, cancellation/retake, five-update capture wait,
save/reload and injected write failure/retry. The `Is a directory` line is the
intentional failure in the private test profile. Kinds 3/4 rendered their
actual car and layers through `draw_cover`; their saved pixels matched the
existing picture-container roundtrip. Both committed reward crops were
visually inspected. These are clone diagnostic images, not native screenshots
or claims of full native visual equivalence. The cover target is fixed at
640x480 independently of window size, so this change adds no resolution rule.

Raw build logs and full test output remain in the private sibling
`cover-lighting-runtime/` directory. The prior magazine worktree was not edited;
its privately owned target cache was reused with no concurrent builds.
