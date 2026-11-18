# Shop / dyno scene cars on the NIS mover, dyno camera (2026-11-06)

Vanilla SPEED2.EXE (sha256 8f6a9ae2…2a297) under Wine, private copy
a private Wine copy (WidescreenFix's ASI loader off), fastgpu on :170
with `--audio-file`, the finished-career save's profile (Lancer). Read-only polls; no writes.

* `scenecap.py SESSION OUT.json [secs] [wait]`: waits for a world car (0x890080
  +0x1C) on mode 5, then logs mode, rigid body (+0x20 CoM, +0x30 axes, +0xA0
  velocity), mover count +0x60, view 1's camera 0x8760E0 and the player's
  camera mover vtable. `native-*.tsv` are its slot-0 rows (one per sim tick):
  `shopenter` / `shopexit` (IND_BODYSHOP_1, Engage → Enter Shop → Escape →
  Yes), `dynoenter` (Garage → Performance Tuning / Dyno → Yes).
* `campoll.py`: the camera mover and view 1 through a Dyno Run
  (`native-dynorun-camera.txt`).
* `cmp.py NATIVE.tsv CLONE.log COM_FWD COM_UP`: native CoM → origin against the
  clone's `NFSU2_PROBE=1` lines (`--shot dynoenter` with `NFSU2_SHOT_SAVE`,
  `--shot shopenter` with `NFSU2_SHOP=IND_BODYSHOP_1 [NFSU2_SHOP_EXIT=1]`).
* `native-dynoenter-strip.jpg`, `native-shopenter-strip.jpg`: frames of the
  original's scenes (videos in the work dir).
* `sndpoll.py`: see `../ending-hold-audio/`.

Tests: `shop_and_dyno_scenes_drive_the_car_like_the_original`,
`dyno_camera_matches_the_dyno_run_under_wine` (game.rs).

`native-shopexit-stop.tsv`, ShopExit1's end on mode 1 with the body's
gravity +0x160 and the pedals (`../dyno-exit/carpoll.py`): the rest test's
class 2 (gravity 0, velocity 0) within three polls.
