# Mark This Spot (2026-11-05)

Not native: an opt-in report aid (QoL family). Code: `src/mark_spot.rs`
(the report), `src/bin/game/enh/mark.rs` (what the game sees), hooks in
`src/bin/game.rs` (`Game::key`, the `--shot` explore arm), the setting in
`crates/ob-settings/src/extras.rs` (`Extras::mark_spot`, options.txt `enh.mark_spot N`) and
the row in `src/frontend/options/`.

## Setting

Enhancements page, row **Mark Spot Key** (label id `0xC1_04E0_6D`, after
Crash Reports): Off (default, native: no key) / F9 / F10 / F11 / F12. The
key works anywhere (free roam, races, menus; not in photo mode, which takes
every key). Presets and Restore Native's graphics presets keep it (it lives
in `extras`, which `Preset::apply` carries over); Restore Native resets it.

## The report

`<data home>/overbright/marks/mark-<UTC time>.txt` (`-2`, `-3` … for a
second mark in the same second), written atomically; the HUD shows
"SPOT MARKED: <file>" for 3 s and the log a warning line with the path.
Home paths read `~`. It holds:

* a `note:` line for the player's description;
* mode (explore / race / menu), region, the race event's name, the car type;
* the car's position (world, Z up), heading (degrees from +x, as
  NFSU2_SHOT_AT takes it), speed;
* the camera: eye, a point 10 m ahead, the vertical FOV, the game camera's
  own FOV (`Camera::game_fov`, 0x5C7ED0's half-angle projection) and up;
* the streaming section drawn at the car (`World::drawn_sections_at`);
* the nearest scenery placement (solid name, position, distance);
* the world clock (0x7FB718, `Drive::clock`) and the game time;
* the window size; the settings hash (XXH3-64 of options.txt's text) and
  options.txt itself; the game files' fingerprint (docs/developing/game-fingerprint.md);
* the current profile's save as it is in memory, written next to the report
  as `mark-<time>.sav` (the car's parts, paint, career);
* the commands that rebuild the view headless:

```
NFSU2_SHOT_SAVE="$HOME/.local/share/overbright/marks/mark-….sav" \
NFSU2_SHOT_AT=x,y,z,heading NFSU2_SHOT_HOLD=1 \
NFSU2_SHOT_LOOK=ex,ey,ez,tx,ty,tz,fov,game_fov,ux,uy,uz \
NFSU2_RAIN_CLOCK=<clock − 3 s> NFSU2_SHOT_SIZE=WxH \
NFSU2_SHOT_ENH='enh.…;custom.N V;…' [NFS_DETAIL=d] \
game --shot explore mark-….png
```

and the bench line for performance with the same settings
(`NFSU2_BENCH_CAR`, `NFSU2_BENCH_DETAIL`, `NFSU2_BENCH_ENH`; the bench
drives its own route, not this spot), and for a race the event's
`NFSU2_SHOT_EVENT=<name> game --shot race`.

## Shot hooks added

* `NFSU2_SHOT_HOLD=1` (explore arm): no gas while the shot settles, and the
  car moved back onto NFSU2_SHOT_AT's x, y afterwards (it rolls ~0.2–0.3 m
  while it settles), so the marked pose comes back.
* `NFSU2_SHOT_LOOK` takes `,game_fov,ux,uy,uz` after the fov (old 6/7-value
  forms unchanged).
* `NFSU2_SHOT_MARK=1`: the explore shot writes a report of its own spot (the
  check below).
* `NFSU2_RAIN_CLOCK` is the clock a load starts at; the explore shot runs
  3 s (90 × 1/30 s) before it draws, hence `mark_spot::SHOT_SETTLE_S`.

## Check (evidence: docs/evidence/q1/mark-spot/)

A shot marked itself at (−1650, 600) heading 45 (`NFSU2_SHOT_MARK=1
NFSU2_SHOT_HOLD=1 NFSU2_SHOT_AT=-1650,600,0,45`); the report's command was
run as written: the two 1280×960 frames differ in 33 pixels by more than
3 % (AE, fuzz 3 %), mean absolute difference 0.034 / 255; the car pose and
camera in the second run's own report are identical.

## Limits (open)

* The car's motion (speed, steering, suspension travel), traffic, rivals
  and particles are not restored; the shot's world settles for 3 s first.
* In a race the shot command rebuilds free roam at the spot, not the race;
  the race event is named for `--shot race`.
* The shot's screen passes follow the World Detail level (NFS_DETAIL),
  not the individual custom rows (an existing shot rule).
* The game camera's FOV and up are restored; a replay or NIS camera is
  recorded like any other (its own motion is not).
