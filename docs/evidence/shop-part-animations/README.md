# Shop part-animation previews: native vs clone (2026-11-04)

Native: SPEED2.EXE (sha256 8f6a9ae2…2a297) under Wine, fastgpu.sh on :188,
a private Wine copy, the finished-career save
(sha256 1e0acea4…3b9453) copied into the prefix's AppData profile dir;
Main Menu → Profile Manager → Load the finished-career save → Customize → the tuned 350Z →
Change Interiors / Car Specialties. Keys: arrows, Return, Escape, `c`
(Open/Close). Frames with `peek.py frames` (15–30 fps). Raw captures stay in
a private Wine copy.

Clone: the same save and keys,
`NFSU2_SHOT_SAVE=<the finished-career save> NFSU2_SHOT_KEYS="Right,Right,Accept,Right,Right,Right,Accept,…" NFSU2_SHOT_KEYS_GAP_MS=2500 [NFSU2_SHOT_MS=2500] game --shot custcars out.png`
(helper: work/cs.sh). Left = native, right = clone in every pair.

| file | shows |
|---|---|
| native-trunk-audio-entry-spoiler.jpg | entering Trunk Audio: hatch and spoiler rise together |
| trunk-audio-strip-native-vs-clone.jpg | mode 3 strip, spoiler on the open hatch |
| layout-list-native-vs-clone.jpg | layout list on TUNED: no components, Visual Rating 4.22, Install Part |
| layout-back-native-vs-clone.jpg | Back to the strip: rating gone, base camera, car angle kept |
| component-list-native-vs-clone.jpg | Trunk Slot 1 list: components lit, rating 4.58 |
| native-window-tint-rating-open-close.jpg | the specialties list shows the rating from its opening, not after Back |
| native-trunk-audio-exit-strip.jpg | Back from Trunk Audio: hatch shuts, Custom Gauges highlighted |
| doors-stock/scissor/suicide-native-vs-clone.jpg | Open/Close Doors per style, mirrors on the doors |
| native-doors-step-shut-sheet.jpg, native-doors-leave-list-strip.jpg | a step / Back shuts open doors in the next frame |
| split-hoods-native-vs-clone.jpg | COBRA, COPPERHEAD open (clone below); the left half is black natively (open) |
