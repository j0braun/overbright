# Photo mode (clone deviation, opt-in)

**Nothing here is in
SPEED2.EXE and nothing was traced for it**: the original has no photo mode.
Default **Off**: the native pause menus, frames, sounds and simulation are
unchanged (gates below).

## Turning it on
Options → Display → Enhancements → **Photo Mode in Pause Menu** Off / On
(`enh.photo_mode 1`; a QoL value, kept by the Graphics presets). With it on,
UI_pause in free roam (`screens::explore_pause_menu`) and in a race
(`race_pause_menu`) lists **Photo Mode** just before "Quit to Main Menu"
(label `options::PHOTO_MODE` 0xC1_04E0_91, not in the game's string table:
the icon strip falls back to `options::clone_label`; icon: the Replay
item's camera, 0x84D1D864). Not offered in the dyno's test-run pause. No
key binding (not needed with the menu item; open).

## What it does
* **The world stays paused** exactly as under UI_pause: the pause menu
  remains the mode (`Game::photo` is a layer over it), its updates run
  unchanged (no simulation step moves anything; particles and rain are not
  stepped under the pause; the 60 Hz cadence's interpolation sits at the
  paused state), only the menu's mouse is not fed and the stick does not
  step the menu. Each frame the draw swaps in the photo camera, the world's
  mode (free roam, or the paused race from `paused_race`, so traffic,
  encounter cars and rivals draw as when driving), the strip's Customize
  Display switches (car shadow `custom[2]`, light glow `custom[9]`, speed
  blur `custom[11]`), hides the HUD, the menu, the cursor and the mirror,
  and puts all of it back after the draw (`photo_begin` / `photo_end`).
  The player's car is drawn even behind the bumper / hood cameras.
* **Camera** (`nfsu2::photo::Rig`): an orbit about a point near the car
  (pan, yaw, pitch, distance 0.8–12 m, roll ±45°, vertical FOV 10–100°),
  starting from the game camera's place and FOV. Limits: the point within
  10 m of the car's centre, the eye within 15 m; a 0.3 m sphere swept
  (steps of half its radius, refined by halving) from the car's centre to
  the point and from the point to the eye against the world's collision
  triangles (`Collision::triangles_near`, walls and floors) stops before any
  face, so the eye never enters a wall, an overpass or the road. Faces the
  sphere touches at the start of a sweep (a car parked against a wall) do
  not hold it.
* **Depth of field** (the motion blur and DoF work's Cinematic pass, `blur_dof::set_photo_mode`,
  `set_photo_dof`): Aperture Off / f/1.4 … f/16 (Off by default; the
  Graphics row's Cinematic is not needed and not used in photo mode), Focus
  Auto (the car looked at, the motion blur and DoF work's `focus_car`, band widened by the f-number)
  or 1–100 m. Band `photo::focus_band` = focus × (1 ∓ N/24) (far side open
  at N ≥ 24); the largest circle of confusion × 2.8/N (`coc_scale`).
  the motion blur and DoF work's clone motion blur is off in photo mode (no motion while paused).
* **Grade** (`render::photo_grade`): the game's own post chain (tint, bleach
  bypass, overbright, streaks: the original grade) draws into an
  intermediate; one pass then applies Exposure (−3…+3 EV, gain 2^EV),
  Bleach 0–100 % (a luminance overlay mixed in) and Filter Original / Black
  & White / Sepia; alpha (the UI mask) is the pixel's own.
* **Pictures** (Enter / A / F12): the frame drawn again offscreen at the
  Picture Size (1x–4x the display in each direction, default 2x) with no help
  bar: the window's size is set to the capture's for one draw (so the
  Graphics page's SSAA / MSAA / Render Scale apply on top, within
  `qol::scene_size`'s budget), read back, written as PNG on a thread to
  `<data dir>/overbright/Screenshots/Overbright <UTC YYYYMMDD-HHMMSS>.png`
  (`(2)`, `(3)` … for several in a second); the strip shows "Saved …". The
  scale is held by `photo::capture_scale`: each side ≤ the GPU's largest
  texture, pixels × MSAA samples ≤ 64 × 3840 × 2160 (e.g. 4K at 8x MSAA:
  2x). The frame counter is put back (a capture is no game frame).
  **Tiling is not implemented** (open): sizes past the texture limit are
  reduced instead (4x of 4K = 15360×8640 fits the reference desktop GPU's 16384).

## Controls (help bar at the bottom, the front end's Conduit font)
| | keyboard / mouse | controller |
|---|---|---|
| move the point | W A S D, R / F up / down | left stick, triggers up / down |
| orbit | mouse drag, I J K L | right stick |
| zoom (distance) | wheel, [ ] (also PgUp/PgDn, + −) | L1 / R1 |
| roll / FOV | Q / E, Z / X | strip rows |
| setting row / value | ↑ ↓ / ← → | D-pad |
| take picture | Enter, F12 | A (Select) |
| hide help bar | H, . | Button 3 |
| reset camera | C | X (Customise) |
| back to the pause menu | Esc | B (Back) |

Strip rows: Field Of View, Roll, Focus, Aperture, Exposure, Filter, Bleach,
Car Shadow, Light Glow, Speed Blur (the game's own speed blur as the pause
shows it, On by default), Picture Size.

## Code
`src/photo.rs` (rig, limits, sweep, rows, focus band, capture size),
`src/render/photo_grade.rs` (the grade pass), `src/bin/game/photo.rs` (the
layer: open / close / keys / tick / draw swap / help bar / capture, and the
`--photo` check); hooks in `src/bin/game.rs` (`Game::photo`, `act`,
`key`, `update_world`, `pad_frame`, `draw` → `draw_frame`, the post chain's
target, `blur_and_dof`), `src/render/blur_dof.rs` (`set_photo_dof`,
`dof_wanted`, `photo_band`), `src/qol.rs` (`photo_mode`),
`src/frontend/options.rs` (row), `src/frontend/screens.rs` (menu item,
clone label fallback).

## Checks
* `cargo test`: `photo::tests` (camera held by walls / radius / ground and
  let off a wall it starts against, closest point, rig from the game camera
  and roll, capture size limits, focus band, rows' ends, held controls),
  `screens::photo_mode_tests` (native pause lists without the row, the item
  before Quit with it), `qol` round trip (41 deviations), `options` row order.
* `game --photo OUTDIR [explore|race]` (headless, offscreen 1280×960): drives
  900 steps, pauses, checks the item is listed, opens photo mode, moves the
  camera with held keys and the wheel, sets f/2.8, takes 2x (2560×1920),
  4x (5120×3840, Sepia, +0.5 EV, Bleach 40 %, f/1.4 at 3 m) and 1x (B&W, no
  shadow / glow / speed blur, roll 15°, FOV 35°) pictures, pushes the rig
  past every limit, closes, resumes, drives 300 steps.
  `NFSU2_PHOTO_CONTROL=1` does the same steps and draws without photo mode.
  With `NFSU2_STATE_HASH` the two hash files are **identical** for both
  scenes (1296 steps each; internal evidence): opening
  photo mode, moving, capturing and closing leaves the simulation as the
  plain pause does.
* Parity gates with the row Off (base = <base>, both fast-build
  binaries): see internal evidence.

* The picture is drawn in the window's format and read back as RGBA8
  whatever that format is (`render::read_back_rgba8`; docs/hdr.md "The SDR
  swap chain's format"). `photo_formats::a_picture_is_the_same_in_every_window_format`
  (game files) takes the same 1x picture on 8-bit BGRA, 8-bit RGBA, 10-bit
  and 16-bit float windows: RGBA exact, the others within 1/255 of BGRA.
  `NFSU2_PHOTO_FORMAT=rgba8|rgb10a2|rgba16float` runs the `--photo` check on
  such a window. A picture that cannot be made (the draw fails, a format the
  read-back cannot convert) is a "Not saved: …" line under the strip and a
  log warning; photo mode and the game go on.

## Evidence (the maintainer's private captures)
`pause-photo-item.jpg` (the item selected), `*-photo-help.jpg` (help bar,
f/2.8), `*-capture-*.jpg` (the pictures, downscaled), `explore-photo-held.jpg`
(rig pushed past the limits), `*-photo.txt` / `*-control.txt` (the check's
report), `state-hash.txt`, `parity-gates.txt`.

## Open
* Tiled capture beyond the GPU's texture limit (sizes are reduced instead).
* No key binding to open it from driving (the pause menu item only).
* Screen-space effects sized in pixels (glow, DOF circle of confusion, SSAO)
  follow the capture's size as they follow SSAA's (docs/post-scale.md); a
  4x picture's glow is not compared against the 1x frame's by numbers.
* HDR Output: the picture is the SDR frame (8-bit PNG).
* Judged by eye only (no native counterpart).
