# This page findings: what is wrong at which resolution (2026-11-11)

**Status after the resolution fixes (2026-11-12):** F1 fixed for Wide (cause: near-plane clipping of the road under the chase
camera at the ~161° vertical angle; Wide now keeps the 4:3 vertical angle below 4:3; Stretched keeps the
original's rule and is listed as a known XFAIL); F2 closed by the 640 × 480 minimum window (320×240 left the
matrix). The stale mode list and the row naming an unoffered size: fixed (lists per display path, rebuilt on a
monitor change). Details: docs/resolutions.md "Results / the resolution fixes".

Source: `scripts/checks/resolutions.sh` over the whole matrix (26 sizes x {stretched, wide, wide_hudfull on the
HUD screens} x 15 screens, plus the 8 `mouse_*` shots x {wide, stretched} at every size: 1274 cells) on the
merge base binary + this work's dump. Per-cell lines in `results.tsv.gz`, the table in `table.md`, the failures by
check in `causes.txt`, contact sheets `sheet_*.jpg` (five of the 24, this folder; the rest stay in the run directory). Sorted by how many cells a cause breaks.
"Original" = SPEED2.EXE at its own modes (640x480 ... 1600x1200, 4:3, and 1280x1024): it has no aspect handling
(canvas stretched 0x5C5030, Vert- 0x5C7ED0; docs/ultrawide.md). Nothing was run under Wine for this work.

## F1 - Portrait (9:16, narrower than 4:3): the 3D view leaves a flat band at the bottom (8 cells; check `flat`)

* Screens: free-roam HUD, race HUD, pause (the world behind) at 1080x1920 under every option (stretched, wide,
  wide_hudfull). A band of the clear / fog colour (purple at night, olive in the race) fills the bottom ~15 %
  of the screen; the HUD's tach and minimap sit on it. The race-start scene and photo mode are not affected.
* Where: `display::view_aspect` (src/render/native/display/ui_frame.rs:266) gives Wide its Hor+ only wider
  than 4:3; narrower it keeps the native Vert- rule (1/aspect, 1), so `Camera::projection_between`
  (src/render/camera.rs:101-103) multiplies the vertical angle by (4/3)/aspect: 2.37x at 9:16, the free-roam
  camera's 1.248 rad x 1.265 becomes ~161 deg vertical. Why exactly the bottom stays empty (near plane, the
  road's culling or LOD under the camera) is **open**; the angle is the trigger (5:4, 1.07x, is fine).
* Original: no portrait mode (narrowest native mode 1280x1024) - not applicable; not checked under Wine.
* Proposed fix (Wide only; Stretched stays native): below 4:3 keep the 4:3 vertical angle as well (cap:
  `view_aspect` -> (0.75, 1), the horizontal angle narrows), or draw the 3D view in the centred 4:3 box with
  bars above and below under ~1.0.

## F2 - 320x240 (a tiny window, below the native minimum): the race HUD's text and gauges minify to mush (3 cells; check `resind`)

* Screen: race HUD at 320x240, all three options (mean 10.8, 7.3 % of pixels off against 640x480 downscaled;
  menus and the free-roam HUD stay inside the limit, mean <= 6.8, 2.3 %). At half the canvas scale the lap /
  position text and the tach numbers are unreadable; the 3D view is fine.
* Where: every UI texture is sampled bilinear as fe.efx does (src/render/native/ui/renderer.rs:110-117) at any
  scale; below scale 1 that skips texels (whether the FE textures have mips was not checked: **inferred**).
  The Resolution row never offers < 640x480 (`mode_list`, src/render/native/display/present.rs:101), but a
  windowed window can be resized to any size (src/bin/game/shell/window.rs:295, no minimum size).
* Original: cannot run below 640x480 - not applicable.
* Proposed fix: a minimum window size of 640x480 (`with_min_inner_size`), or (Wide) area-filtered / mipmapped
  UI sampling when the canvas scale is < 1.

## Checked and passing in every cell

* UI canvas rectangle, scale and reaches equal the option's rule at all 26 sizes (incl. 1366x768, 2256x1504,
  1080x1920, 320x240); Stretched is the whole target everywhere.
* No textured quad the original shows whole is cut off; none visible outside its canvas; every textured quad
  and every glyph at the frame's scale (Wide: uniform).
* Mouse: the game's own hit test finds every recorded widget at the centre of the pixels drawn for it (main,
  options, settings hub, car select, parts, world map, pause, dialog, results); the 8 `mouse_*` shots pass at
  every size in Wide and Stretched.
* FOV: every game camera's m00 / m11 follow the rule (Wide Hor+ wider than 4:3, Vert- at 4:3 and narrower;
  Stretched Vert-); photo mode's free camera keeps its vertical angle.
* Bars: movies and loading screens pillar- / letterboxed in black (Wide), full (Stretched); the 32:9 FE scene
  cap black; the race start's cinematic bar across the width; no stray side bars.
* Resolution independence within each aspect (4:3, 16:10, 16:9 incl. 1366x768, 3:2): every size matches its
  smallest size downscaled (largest: mean 6.2, 2.2 % edge pixels: finer 3D detail).

## Not decidable by the script

* Portrait layout (menus a 4:3 box in the middle third, HUD top / bottom elements moved by the vertical
  reach): correct by the rule; whether it is what a portrait player wants is a design call.
* Photo mode under Stretched: its free camera is Hor+ whatever the option (src/photo.rs:233, `game_fov =
  None`); photo mode has no native counterpart.
* Mouse groups: a group whose children go to both sides (UI_PC_Help_Bar's group objects) is recorded with its
  first quad's place (src/frontend/mouse.rs `record`), so its rectangle's centre is off what was drawn on wide
  screens (a note, 3 groups per FE screen); no screen hit-tests those groups today.
* The 21:9 sizes are three different aspects (2.370, 2.389, 2.400); 5:4, 32:9 and 9:16 have one size each: no
  downscale compare for them; their contact sheets were looked at (no defect beyond F1).
* Real-window display modes (docs/resolutions.md "Display-mode handling"): stale mode list after a monitor
  change and a Resolution row naming a size the borderless fallback does not output are **wrong** by
  reading; Wayland fractional scale, exclusive switches and monitor changes are **untested** (need a real
  display; nothing was opened on a desktop).
