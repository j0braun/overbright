# Showcase shadow lifecycle investigation (in progress)

Kind1 and kind2 now have complete observed preview/save/exit lifecycles. Both retain SHADOWIG; no production texture-selection correction is justified by these observations. All native sessions use a private copied Wine prefix/display176 and the verified executable. Entry hooks preserve general registers, flags and FXSAVE state; session cleanup verifies original bytes and stops only the owned session.

## Retained runs

- `pilot-world`: offset-validation pilot. Record word8 incorrectly sampled TextureInfo+14 (inside its name), **not its hash**. The exact old recorder/controller is in `pilot-recorder`. Do not use this run for hash classification.
- `kind1-navigation`: startup-only run ended on stdin EOF; zero records, restored hooks.
- `kind1-navigation-live`: corrected TextureInfo hash+24. 16,950 events, no drops, restored hooks. Diagnostic clone-generated native-format stage2 save loaded by native executable, internal profile the finished-career save/LANCEREVO8. One explicit write resets milestone B0DEDBEF completion byte at862946 from1 to0 before world entry. Save/UI prompts and all commands retained. This is instrumented progression, not naturally earned milestone evidence.

The corrected run maps full player car+554 to render-info058868B0. It records setup of LANCEREVO8_SHADOWIG (0C38F02B) during game state5. It does **not** reach photo preview, shutter, or save. Its6172C7/617351 draw hooks observed traffic, not that player RI. Therefore setup selection is not yet proof of the texture submitted in the player photo.

## Next observer revision

Current recorder additionally observes622D5A (second render-info+33C read) and622DF9 (its strip submission). Tags5/6/7 distinguish this path from original tags2/3/4; tag1 remains setup638840. Both submit hooks retain the native return value. `player-draw.asm` retains original instructions.

`session.py place` is explicit diagnostic relocation: adjust physics body position by requested full-car position delta, write full-car centre of mass (+60; +50 is body origin), zero body linear/angular momentum atomically while stopped. Every changed byte is logged. It is not a native teleport implementation and must not establish reset/placement semantics. Real trigger engagement and native photo UI will follow if the native marker is available.

No shared session, original save, executable, or asset is modified. The copied profile originated in review/photo-saves-placed/s2a.sav; it is not a validated naturally progressed native save. Xvfb1280x960 holds native640x480 content, as visible in retained screenshots.

## Kind1 completed lifecycle

`kind1-preview` records native trigger engagement, native ShowCase menu, Take Picture, Accept, Save Yes, and return to driving. Access is instrumented: completion byte862946 was cleared and the native car was relocated to the verified stage2 trigger(-606,-766,17.671473), with all writes retained. Neither shadow flag, render-info, texture pointer, nor package flow was forced. The native engage dialog and photo UI performed their own state transitions.

Player car057AA1B8+554 points to render-info057D68B0 in every retained world/preview/shutter/exit snapshot. Address reuse is real: earlier main-menu setup event0 at that same address selected RX7_SHADOWFE. World setup event19693 selects LANCEREVO8_SHADOWIG(0C38F02B), pointer0913DA80, while game state5 loads the world. There is **no later setup event** for that RI. Actual reader622D5A and successful strip submissions622DF9 retain that exact texture through reward flag1 and after it clears to0 on exit. `check-lifecycle.py` checks20,696 player observations across the selected phases, all successful returns, and one world generation. Record indices count observer events, not rendered frames.

The saved native `the finished-career save Magazine 1` file is retained byte-for-byte as `native-saved-magazine.bin`, with BGRA512x512 decoded to PNG (opaque display). It shows the completed turbo cover. The post-save snapshot has showcase byte0 and reward flag0. This directly supports retaining the clone's IG texture choice for this kind1 case; the6387DE flag branch alone would have suggested an incorrect FE replacement if resource lifetime were ignored.

Raw58,471 records, zero drops, all hooks/padding restored. RawSHA256 d9943bf8eff60e66ea51c568b3b78a12aa9563359f5e11a28f1deafb4023c49e. Scope remains one Lancer, one kind1 milestone, this diagnostic save and rendering setup. This kind1 capture does not establish naturally earned progression. Kind2 is covered separately below.

## Kind2 completed lifecycle and conclusion

`kind2-preview` follows the same native path at stage2 DVD1: explicit completion-byte8628FE reset1→0 and instrumented relocation to(-301,-554,19.668434), then native engage, preview, picture, Save Yes, and driving. Player0627D620+554 maps to RI06270698. Its only setup, event13814 during state5, selects IG0C38F02B at09915400. The reader and actual successful native submissions retain that exact resource under reward flag2 and after exit clears it. No setup runs at preview entry or save. Cover hash9E101F9E matches the selected DVD. The original saved `the finished-career save DVD 1` file and decoded PNG are retained.

Raw49,955 events, zero drops, restoration verified; rawSHA2566b274cef815c0072d4d811ac8b4c41d6c79ce50c95d22b3a2e26334d22a3b87d. The combined checker validates20,696 kind1 and11,695 kind2 observations across the required phases (not every trace row is a phase/player match). Both captures retain successful native strip returns and post-exit snapshots.

**Decision:** retain current world IG choice for kind1/2. The setup branch6387DE..638840 is conditional on state/flag **when resources are built**; changing flag838578 later does not rebuild the already loaded player's render-info in these captures. A proposed unconditional FE switch for world photos would contradict the observed actual submissions. Kind3/4 FE shadows are outside this experiment and should remain unchanged. No clone production files were changed.

Limits: one car(LANCEREVO8), one milestone of each kind, one native rendering setup, instrumented trigger access from a clone-generated native-format profile. This does not prove all cars, car changes during preview, alternate settings, or natural career progression. It does directly resolve the resource lifetime/submission question for two complete native UI lifecycles, including the saved artifacts and return to driving.

Recheck with `python3 internal evidence: showcase-shadow-lifecycle/check-lifecycle.py. `analyze.py CAPTURE_DIRECTORY` verifies raw/compressed hashes and groups by most recent setup event. `manifest.json` hashes all retained evidence and source files except itself. The exact recorder/controller used by each completed run is copied inside that run directory.
