# Photo mode

Pause the game, move a free camera around your car and save large pictures with depth of field and a
colour filter. Photo mode is optional and off until you turn it on. The original game has no photo mode,
so nothing here comes from `SPEED2.EXE`.

## Turning it on

**Options → Display → Enhancements → Gameplay & Accessibility → Photo Mode in Pause Menu**: On. The pause menu in free roam and in a
race then lists **Photo Mode** just before "Quit to Main Menu". It is not offered in the dyno's test-run
pause, and there is no key to open it while driving.

With the row Off the pause menus, pictures, sounds and simulation are the original's.

## Controls

The help bar at the bottom lists them too.

| | Keyboard / mouse | Controller |
|---|---|---|
| Move the point | W A S D, R / F up / down | left stick, triggers up / down |
| Orbit | mouse drag, I J K L | right stick |
| Zoom (distance) | wheel, [ ] (also PgUp/PgDn, + −) | L1 / R1 |
| Roll / field of view | Q / E, Z / X | the strip's rows |
| Setting row / value | ↑ ↓ / ← → | D-pad |
| Take a picture | Enter, F12 | A (Select) |
| Hide the help bar | H, . | Button 3 |
| Reset the camera | C | X (Customise) |
| Back to the pause menu | Esc | B (Back) |

The settings strip has these rows: Field Of View, Roll, Focus, Aperture, Exposure, Filter, Bleach, Car
Shadow, Light Glow, Speed Blur (the game's own speed blur as the pause shows it, On by default) and Picture
Size.

## What you can do

- **Camera**: it orbits a point near your car. You can pan, turn and tilt it, set the distance (0.8–12 m),
  roll it up to ±45° and set the vertical field of view (10–100°). It starts where the game camera was. The
  point stays within 10 m of the car's centre and the camera within 15 m, and the camera stops before walls,
  overpasses and the road. Your car is drawn even from the bumper and hood cameras.
- **Depth of field**: Aperture Off (the default) or f/1.4 to f/16. Focus Auto (on your car) or 1–100 m.
- **Look**: Exposure −3 to +3 EV, Bleach 0–100 % and the Filter Original, Black & White or Sepia, on top of
  the game's own colour grade. Car shadow, light glow and speed blur can be switched off for the picture.
- **Pictures**: saved at 1x to 4x the screen size in each direction (2x by default) as PNG files in
  `<data folder>/overbright/Screenshots/`, named `Overbright <UTC YYYYMMDD-HHMMSS>.png` (with `(2)`, `(3)` …
  for several in one second). The strip shows "Saved …". The help bar is not in the picture.
- The world stays paused, exactly as in the pause menu: photo mode does not move anything in the game.

### Limits

- A picture larger than your graphics card can draw at once is made smaller instead. Each side stays within
  the card's largest texture, and pixels × MSAA samples stay within 64 × 3840 × 2160 (for example 4K at 8x
  MSAA gives 2x). 4x of 4K (15360 × 8640) fits a card with a 16384 limit.
- With HDR Output on, the picture is the normal (SDR) image as an 8-bit PNG.
- If a picture cannot be made, a "Not saved: …" line appears under the strip and the game goes on.

## How it works

The rest of this page is for developers.

- **Pause**: the pause menu stays the mode (`Game::photo` is a layer over it) and its updates run as before.
  No simulation step moves anything, particles and rain are not stepped, and the 60 Hz cadence's
  interpolation sits at the paused state. Only the menu's mouse is not fed and the stick does not step the
  menu. Each frame the draw swaps in the photo camera; the world's mode (free roam, or the paused race from
  `paused_race`, so traffic, encounter cars and rivals draw as when driving); and the strip's Customize
  Display switches (car shadow `custom[2]`, light glow `custom[9]`, speed blur `custom[11]`). It hides the
  HUD, the menu, the cursor and the mirror, and puts everything back after the draw (`photo_begin` /
  `photo_end`).
- **Menu item**: `screens::explore_pause_menu` and `race_pause_menu`. The label `options::PHOTO_MODE`
  0xC1_04E0_91 is not in the game's string table, so the icon strip falls back to `options::clone_label`.
  The icon is the Replay item's camera, 0x84D1D864. The setting is `enh.photo_mode 1`, a quality-of-life
  value that the Graphics presets leave alone.
- **Camera** (`nfsu2::photo::Rig`): a 0.3 m sphere is swept (steps of half its radius, refined by halving)
  from the car's centre to the point and from the point to the eye against the world's collision triangles
  (`Collision::triangles_near`, walls and floors) and stops before any face. Faces the sphere touches at the
  start of a sweep (a car parked against a wall) do not hold it.
- **Depth of field** uses the Cinematic depth-of-field pass (`blur_dof::set_photo_mode`, `set_photo_dof`);
  the Graphics row's Cinematic setting is not needed and not used. Focus Auto is `focus_car`, with the band
  widened by the f-number. Band `photo::focus_band` = focus × (1 ∓ N/24) (far side open at N ≥ 24); the
  largest circle of confusion × 2.8/N (`coc_scale`). Motion blur is off in photo mode (nothing moves).
- **Grade** (`render::photo_grade`): the game's own post chain (tint, bleach bypass, overbright, streaks:
  the original grade) draws into an intermediate target. One pass then applies Exposure (gain 2^EV), Bleach
  (a luminance overlay mixed in) and the Filter; alpha (the UI mask) is the pixel's own.
- **Pictures**: the frame is drawn again offscreen at the Picture Size. The window's size is set to the
  capture's for one draw, so the Graphics page's SSAA, MSAA and Render Scale apply on top within
  `qol::scene_size`'s budget. It is read back and written as PNG on a thread. `photo::capture_scale` holds
  the size limits. The frame counter is put back afterwards (a capture is not a game frame). The picture is
  drawn in the window's format and read back as RGBA8 whatever that format is (`render::read_back_rgba8`;
  [hdr.md](hdr.md) "The SDR swap chain's format"). A failed picture is also a log warning.

### Code

`src/photo.rs` (rig, limits, sweep, rows, focus band, capture size), `src/render/enh/post/photo_grade.rs`
(the grade pass), `src/bin/game/enh/photo/` (the layer: open, close, keys, tick, draw swap, help bar,
capture, and the `--photo` check). Hooks in `src/bin/game.rs` (`Game::photo`, `act`, `key`, `update_world`,
`pad_frame`, `draw` → `draw_frame`, the post chain's target, `blur_and_dof`),
`src/render/enh/post/blur_dof.rs` (`set_photo_dof`, `dof_wanted`, `photo_band`), `src/qol/`
(`photo_mode`), `src/frontend/options/` (the row), `src/frontend/screens/` (the menu item and the label
fallback).

### Tests

- `cargo test`: `photo::tests` (camera held by walls, radius and ground, and let off a wall it starts
  against; closest point; rig from the game camera and roll; capture size limits; focus band; rows' ends;
  held controls), `screens::photo_mode_tests` (the pause menu lists no item with the row off, and the item
  before Quit with it on), the `qol` round trip (41 deviations), the `options` row order.
- `game --photo OUTDIR [explore|race]` (no window, offscreen 1280 × 960): drives 900 steps, pauses, checks
  the item is listed, opens photo mode, moves the camera with held keys and the wheel, sets f/2.8, takes 2x
  (2560 × 1920), 4x (5120 × 3840, Sepia, +0.5 EV, Bleach 40 %, f/1.4 at 3 m) and 1x (B&W, no shadow, glow or
  speed blur, roll 15°, FOV 35°) pictures, pushes the rig past every limit, closes, resumes and drives 300
  steps. `NFSU2_PHOTO_CONTROL=1` runs the same steps without photo mode. With `NFSU2_STATE_HASH` the two
  runs' state hashes are identical in both scenes (1296 steps each): opening photo mode, moving, capturing
  and closing leaves the simulation as the plain pause does.
- `photo_formats::a_picture_is_the_same_in_every_window_format` (needs the game files) takes the same 1x
  picture on 8-bit BGRA, 8-bit RGBA, 10-bit and 16-bit float windows: RGBA exact, the others within 1/255 of
  BGRA. `NFSU2_PHOTO_FORMAT=rgba8|rgb10a2|rgba16float` runs the `--photo` check on such a window.
- With the row Off, the comparison checks against the build before photo mode found no difference.

## Open

- Tiled capture beyond the graphics card's texture limit (sizes are reduced instead).
- No key to open photo mode while driving (only the pause menu item).
- Screen-space effects sized in pixels (glow, the depth of field's circle of confusion, SSAO) follow the
  capture's size, as they follow SSAA's ([docs/notes/graphics/ssaa.md "Post effects under SSAA and Render Scale"](../notes/graphics/ssaa.md#post-effects-under-ssaa-and-render-scale)); a 4x picture's glow was not
  compared with the 1x frame's by numbers.
- Judged by eye only: the original has nothing to compare with.
