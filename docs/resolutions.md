# Screen resolutions and aspect ratios (this work 2026-11-11, the resolution fixes 2026-11-12)

Requirement: every common screen resolution supported. This page built the matrix and the
findings ([evidence/resolutions/findings.md](evidence/resolutions/findings.md)); the resolution fixes fixed them (the portrait
band, a 640 × 480 minimum window, the Resolution row's list per display path and monitor) and made the matrix a
failing step of the FULL checks.

## What "supported" means

At every size and aspect ratio in common use, with every display option the settings offer, these are
correct (not only "a window opens"):

* the menus (the 640 × 480 FEng canvas), the in-game HUD, movies, loading screens, dialogs;
* the 3D world view (its field of view follows the option's rule), photo mode;
* mouse hit-testing (a click on a drawn widget acts on that widget).

The native rule still holds: the original's look stays available and byte-identical
(Aspect Ratio **Stretched (Original)**: the 640 × 480 canvas stretched over the target, 0x5C5030 /
0x5BF760, and the native Vert− projection, 0x5C7ED0; docs/native-deviations.md, docs/ultrawide.md,
docs/formats.md "Display modes and aspect"). Wide / tall layouts are the clone's default on top.

## The matrix

| aspect | sizes |
|---|---|
| 4:3 | 640×480 (the smallest window), 800×600, 1024×768, 1280×960, 1600×1200 |
| 5:4 | 1280×1024 |
| 16:10 | 1280×800, 1440×900, 1680×1050, 1920×1200, 2560×1600 |
| 16:9 | 1280×720, 1366×768 (non-integer scale; within 1 % of 16:9), 1600×900, 1920×1080, 2560×1440, 3840×2160 |
| 3:2 | 2160×1440, 2256×1504 |
| 21:9 | 2560×1080, 3440×1440, 3840×1600 |
| 32:9 | 3840×1080, 5120×1440 |
| 9:16 (portrait) | 1080×1920 |

Options per size (the display options the settings offer):

| option | settings | rule |
|---|---|---|
| `stretched` | Aspect Ratio = Stretched (Original) — the original's behaviour | canvas = the whole target, x and y scaled apart; 3D Vert− (horizontal angle fixed, `view_aspect` = (1/aspect, 1)) |
| `wide` | Aspect Ratio = Wide — the clone's default | canvas = the largest centred 4:3 box (whole pixels); elements keep their 4:3 size, side elements move out by the reach (to the centred 16:9 safe area, `side_reach`), fills reach the edges; 3D: the vertical angle of 4:3 at **every** aspect (`view_aspect` = (0.75, (4/3)/aspect)): Hor+ wider than 4:3, native at 4:3, narrower (5:4, portrait) the 4:3 picture with its sides cropped (was the native Vert−); front-end 3D scenes capped at 2.4:1 (`scene_bars`) |
| `wide_hudfull` | Wide + Enhancements HUD Layout = Full Width (HUD screens only) | as `wide`, the HUD's reach to the screen edges |

Screens (the existing `--shot` targets; 17 screens):

| screen | target | kind |
|---|---|---|
| main menu | `main` | front end over the garage |
| an options page with sliders | `options_audio` | front end |
| the settings hub | `options_enhancements` | front end |
| car select | `qrcars` | front end |
| shop / parts browser | `parts` | front end |
| world map | `worldmap` | menu over the world |
| HUD in a race | `lapko` (`race` at 5 s is still the start scene) | 3D + HUD |
| HUD in free roam | `explore` | 3D + HUD |
| pause menu | `pause` | menu over the world |
| loading screen | `loading` | 4:3 picture, pillarboxed |
| a dialog | `dialog` | front end |
| a movie frame | `movie` | 4:3 picture, pillarboxed |
| photo mode | `photo` (`NFSU2_PHOTO_SIZE`) | 3D (free camera) + help bar |
| post-race | `results` | front end |
| race-start scene | `qrstart` | 3D with cinematic bars |
| HDR calibration (`hdr_calibration`) | `settings_display` with `NFSU2_HDR_FAKE=pq`, `enh.hdr_output 1` | front end + the HDR test pattern |
| HDR calibration from the pause (`hdr_calibration_pause`) | `pause_settings_display`, the same HDR environment | menu over the world + the HDR test pattern |

Mouse: the existing `--shot mouse_*` targets (`mouse_main`, `mouse_options`, `mouse_settings`, `mouse_parts`,
`mouse_map`, `mouse_carlot`, `mouse_lists`, `mouse_controls`; each puts the pointer on a widget and asserts the
screen acted on it) at every size × {wide, stretched} with `RES_MOUSE=1`.

## The check (`scripts/checks/resolutions.sh`, `resolutions.py`)

```
scripts/checks/resolutions.sh OUTDIR [game]     # RES_SET=all (default) | family; RES_MOUSE=1; JOBS=4
```

Every cell (size, option, screen) is one headless shot with `NFSU2_SHOT_SIZE`, `NFSU2_ASPECT`,
`NFSU2_SHOT_ENH` and `NFSU2_UI_GEOM=<cell>/geom.json`. The dump (dev harness only:
`src/render/native/ui/geom_dump.rs`, hooks in `ui/batch.rs`, `render/camera.rs`, `frontend/mouse.rs`, one
call in the shot) holds the UI frame, every quad's canvas rectangle, place and the pixel box of its real
vertices with its scissor, the mouse's rectangles of that layout with the game's own hit test
(`mouse::inside`) at the centre of what was drawn for each, and every camera projection of the frame
(aspect, angle, m00, m11). Per cell, a line `PASS|FAIL size option screen (target): reasons`:

| check | FAIL when |
|---|---|
| `run` | rc ≠ 0, no PNG, a panic / validation line |
| `canvas`, `reach` | the canvas rectangle, scale or reaches differ from the option's rule, computed independently in Python (`expected_frame`) |
| `cut` | a textured quad the original shows whole (canvas rect inside 0..640 × 0..480) loses more than 1.5 px to its scissor or the target |
| `outside` | a textured (non-fill) quad is visible outside the canvas as its place moves it (natively off screen). Fills (fades, dims, letterbox bars, backings: flat or ≥ 4× magnified) reach the edges by design and are not counted |
| `scale`, `text` | a textured quad's pixel size is not its canvas size × the frame scale (Wide: both axes the same scale; Stretched: x and y apart). `text`: the same for glyphs (font textures) |
| `mouse` | the game's hit test misses a widget at the centre of the pixels drawn for it |
| `fov` | no projection at the target's aspect, or m00 / m11 off the rule (Hor+ / Vert−); a free camera (photo) must keep its vertical angle |
| `flat` | a 3D screen has a uniform non-black band (≥ 98 % of an edge strip within ±3 of its median, brighter than 20): the clear colour, nothing of the scene drawn there |
| `bars` | movie / loading: the pillar / letterbox not black or the picture black; a 32:9 front-end scene: its cap bars not black; a 3D screen: a black bar at a side edge; the race start: the cinematic bar not across the top |
| `flat` known | `KNOWN` in resolutions.py: Stretched at 1080×1920 (free roam, race, pause, the pause's HDR calibration) keeps the original's Vert− and its band (Results); reported `XFAIL`, not counted, as long as `flat` is the cell's only failure. `resind` known: the pause's HDR calibration at 1600×1200, whose help line quotes the HDR pass's cost at the shot's own size (text, not layout) |
| `hdr` | the HDR calibration screens: no pattern logged (`hdr pattern: px …` in the shot's log) or no `<shot>.hdr-preview.png`; the pattern box outside the menu's canvas or under 8 px; a drawn quad (row text, arrow, scroll bar, help text…) partly under it (only quads that hold it whole, the panel and full-screen fills, may); the preview shows no black frame or no peak box at the logged box |
| `resind` | the image differs from the same aspect's smallest matrix size by more than resolution: downscaled (box) to it, mean abs diff > 10 or > 3 % of pixels off by > 48 (calibrated: finer 3D detail at a higher size gives a mean up to ~6 with 1–2 % of edge pixels) |

Contact sheets per aspect and option: `OUTDIR/sheet_<aspect>_<option>.jpg`; the per-family table
`table.md`; failures grouped by check `causes.txt`.

In the FULL checks set (`checks.sh full`) it is a **failing step** with `RES_SET=family` (8 sizes:
640×480 = the minimum, 1280×1024, 1680×1050, 1366×768, 2256×1504, 3440×1440, 5120×1440, the portrait
1080×1920; 264 cells, ~6 min at 4 jobs): any `FAIL` cell fails the set, its first `FAIL` lines go into
checks.txt with the `RESOLUTIONS:` line (`RES_SET=off` leaves it out; `RES_REPORT_ONLY=1` makes
resolutions.sh exit 0 whatever the verdicts). The whole matrix stays a manual run (`RES_SET=all RES_MOUSE=1`).

**Self-test** (`resolutions.py selftest OUTDIR`, run by resolutions.sh after every check): takes passing cells of
the run, breaks one thing at a time and requires the check meant for it to fail — the canvas moved 10 px, a reach
+7 px, a quad's scissor emptied (`cut`), a quad moved into the pillar (`outside`), a quad and a glyph stretched
1.5× in x (`scale`, `text`), a widget's hit test missed (`mouse`), m11 × 1.1 (`fov`), the earlier Vert− rule in
portrait (`fov`), a flat band over the bottom 15 % (`flat`), a grey pillar on a movie and a black side bar on
the 3D view (`bars`), a fifth of the frame moved (`resind` thresholds), rc 101 + a panic line (`run`), no dump
(`dump`), the HDR pattern moved onto a row's text or removed, and a logged box off the drawn pattern (`hdr`). A check that does not catch its breakage fails the step (`RESOLUTIONS SELFTEST: n/15`).

## Display-mode handling (window code, read 2026-11-11)

From `src/bin/game/shell/window.rs` (`create_window`, `apply_display_mode`, `window_event`),
`src/render/native/display/present.rs` (`mode_list`) and `src/bin/game/render/frame/mod.rs`. Read, not
run: no window on a user desktop.

| item | what the code does | verdict |
|---|---|---|
| modes offered | Screen Mode row: windowed or full screen. Full screen = **borderless** on the window's monitor when the Resolution is the desktop size; else **exclusive** with the monitor's mode of that size at its highest refresh rate (`max_by_key(refresh, bit depth)`); Wayland has no exclusive mode: borderless always | works (by reading); untested on a real display |
| mode list | The resolution fixes (`display::modes_on`, src/render/native/display/monitor.rs): per path. **Windowed**: the native six, the fixed extras 1920×1080 / 2560×1440 / 3840×2160 and the monitor's video modes and desktop size, those larger than the desktop left out. **Full screen**: the monitor's video modes and its desktop size (exclusive / borderless); **Wayland** (no exclusive mode): the desktop size alone. Every list ≥ 640×480. Before a monitor is known (headless runs) the old list (`mode_list(&[])`). The Screen Mode row moves the size to one the new path offers (`fit_mode`: the size itself, else the largest listed that fits in it, else the smallest) | works (unit tests `lists_per_path`, `fitting`); untested on a real multi-monitor desktop |
| a monitor the system describes oddly | The monitor is read through `platform::monitor` (src/platform/monitor.rs) by the game window, `game --probe` and the launcher, never through winit's monitor calls directly: winit 0.30's Windows mode list asserts that every mode `EnumDisplaySettingsExW` lists carries its size, depth and refresh fields (`has_flag(mode.dmFields, REQUIRED_FIELDS)`; Wine on a virtual X display lists one that does not) and its name / size unwrap `GetMonitorInfoW`. On Windows the modes come from `EnumDisplaySettingsExW` directly (`platform::win::monitor_modes`): a mode without a size is left out, and with any incomplete mode winit's list is never asked for: no exclusive full screen (borderless at the desktop size, one warning line). Elsewhere every winit call is caught (`guarded`: the release profile unwinds; the panic hook and the crash report stay quiet) and answered with a fallback: no modes → the desktop size; no size → the window's own size / 960×720 for the launcher | works: under Wine on Xvfb (the Wine smoke's `w_window`, the launcher's start); `platform::monitor::tests` |
| a size the monitor lacks | full screen: borderless at the **desktop** size (the surface follows the real size via `Resized`, so the game renders at the desktop size; the Resolution row still shows the asked size). Windowed: `request_inner_size` — the WM may clamp a window larger than the screen; the surface follows `Resized` | works, but the row then names a size that is not the output (**wrong**, cosmetic); untested |
| sizes below 640×480 | The resolution fixes: the window's minimum inner size is 640×480 (`display::MIN_WINDOW`, `with_min_inner_size`: enforced by the window manager / Windows); an options.txt size below it is raised to it with one log line (`clamp_min_size`). Checked on a private Xvfb: options.txt `resolution 320x240` → a 640×480 window and one line `options.txt resolution: 320x240 is below the smallest size 640x480; using 640x480`. A WM that ignores the hint can still shrink the window; every pass then follows `Resized` as before | works (Xvfb); WM enforcement untested |
| DPI scaling | the window is created at the Resolution in **physical** pixels; `ScaleFactorChanged` (another monitor, a changed scale) keeps the physical size in windowed mode (`inner_size_writer`); the surface, the UI frame (`UiFrame::new(config size)`), the mouse (`CursorMoved` is physical) all use the same physical size. X11: `WINIT_X11_SCALE_FACTOR` (mouse_xtest.sh tests 1.5). Wayland fractional scale: winit reports physical = logical × scale (rounded); not exercised | X11 works (mouse_xtest 1.5); Wayland fractional **untested (needs a real display)** |
| resize while running | `Resized` → `WindowState::resize`: settle, `config` size, `qol::set_output_size`, surface reconfigured, depth recreated; the post chain and the UI read `config` size every frame (`out_size`), the projection's aspect is `out_size` w/h, the mouse's frame is `config` size | works (by reading); the 1366×768 / odd sizes render correctly headless |
| monitor change | The resolution fixes: on `Moved`, `ScaleFactorChanged` and `Resized` the game asks for the window's current monitor (`refresh_monitor` → `display::set_monitor`); when it differs (another monitor, or the same one with other modes / desktop size) the lists are rebuilt with a log line (`monitor …: desktop WxH, N modes; Resolution list rebuilt`) and the Resolution moves to one the active path offers (`fit_mode`), which `apply_display_mode` applies on the next frame. winit has no "monitors changed" event: an unplugged monitor is noticed through the move / resize the system gives the window | works by reading + unit tests; **untested (needs a real display)** |
| first run | opens a 1280×960 window (`DEFAULT_MODE`); following the desktop size is the agreed target, not done (docs/native-deviations.md) | open (known) |

## Tester checklist (needs a real display; not run on a user desktop)

1. Windowed, drag the window's corner small: it stops at 640×480 (X11 with a WM, Wayland, Windows).
2. Options › Display, Screen Mode Windowed: the Resolution row offers no size larger than the monitor.
   Switch to Full Screen: on X11 / Windows the row walks the monitor's own modes (an exclusive switch to a
   smaller one, e.g. 1280×720, changes the monitor's mode, and back on leaving); on Wayland only the desktop size.
3. Two monitors of different sizes: drag the windowed game to the other one; the log shows
   `Resolution list rebuilt`, the row's list is the new monitor's; a size the new monitor cannot show moves to
   the nearest it can. Full screen on monitor 2 opens on monitor 2.
4. Unplug the monitor the game is on (or change its resolution in the system settings): the game keeps
   running, the row's list follows.
5. Wayland with fractional scaling (125 %, 150 %): the window is the Resolution in physical pixels, the menus
   are sharp and the mouse hits what is drawn (click a few menu rows and the help bar).
6. A portrait monitor (1080×1920), Aspect Ratio Wide: the 3D view fills the screen without an empty band, the
   menus are a 4:3 box in the middle, the HUD at the top / bottom edges.

## Results

### The resolution fixes (2026-11-12)

F1 cause, proven with a shot each way (`explore` at 1080×1920 Wide on this work's binary, a throw-away override
of the camera's near plane): with the native Vert− at 9:16 the vertical half-angle is 80.4° (m11 0.169); the
band's top edge (81 % down the image, NDC y −0.62) is a ray 75° below the view axis that meets the road about
0.5 m in front of the eye along that axis — the near plane (0.5 m). With the near plane at 0.05 m the band is
road. So it is near-plane clipping of the road under the chase camera at an extreme vertical angle, not
culling, scissor or depth. Fix: Wide keeps the 4:3 vertical angle below 4:3 too (`view_aspect`), so no such ray
exists; HUD, menus and mouse are unchanged (they never depended on the 3D angle). Stretched is the original's
rule and keeps the band at 9:16 (listed `KNOWN`, XFAIL: SPEED2.EXE has no mode narrower than 5:4).
F2: 320×240 left the matrix; the window cannot be made smaller than 640×480.

Picture: internal evidence — 1080×1920
free roam: Wide before (band), the same with the near plane at 0.05 m (road), Wide after (4:3 vertical angle).

Whole matrix after the resolution fixes (`RES_SET=all RES_MOUSE=1`, 25 sizes, 1225 cells; 320×240's 49 cells left the matrix):
**1225 pass (3 of them XFAIL, the Stretched portrait band), 0 fail**; self-test 15/15. Before: 1263 / 1274
(11 fail). Family set (checks.sh full): 264 cells, 264 pass (3 known). Shown to fail on a broken layout: the
This page binary (the old Vert− below 4:3) on 1080×1920 + 1280×1024 + 640×480 gives `RESOLUTIONS: 99 cells, 73 pass
(3 known), 26 fail; by check: fov 26, flat 5` and exit 1. Evidence: `res2-table.md`, `res2-results.tsv.gz`,
`res2-sheet_9x16_wide.jpg`, `res2-sheet_5x4_wide.jpg`.

pass/total per aspect family and screen (bold: some fail)

| screen | 4:3 | 5:4 | 16:10 | 16:9 | 3:2 | 21:9 | 32:9 | 9:16 |
|---|---|---|---|---|---|---|---|---|
| main | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| options_sliders | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| settings_hub | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| car_select | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| parts_browser | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| world_map | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| race_hud | 15/15 | 3/3 | 15/15 | 18/18 | 6/6 | 9/9 | 6/6 | 3/3 |
| freeroam_hud | 15/15 | 3/3 | 15/15 | 18/18 | 6/6 | 9/9 | 6/6 | 3/3 |
| pause | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| loading | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| dialog | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| movie | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| photo | 15/15 | 3/3 | 15/15 | 18/18 | 6/6 | 9/9 | 6/6 | 3/3 |
| post_race | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| race_start | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_main | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_options | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_settings | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_parts | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_map | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_carlot | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_lists | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_controls | 10/10 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |

### This page (2026-11-11)


Whole matrix, 2026-11-11 (merge base binary + the dump; 1274 cells, `RES_MOUSE=1`, 24 min of shots at
5 jobs on a loaded machine): **1263 pass, 11 fail** (`flat` 8, `resind` 3). Evidence and the findings with
their causes: [evidence/resolutions/](evidence/resolutions/findings.md) (`table.md`, `causes.txt`,
`results.tsv.gz`, five contact sheets).

pass/total per aspect family and screen (bold: some fail)

| screen | 4:3 | 5:4 | 16:10 | 16:9 | 3:2 | 21:9 | 32:9 | 9:16 |
|---|---|---|---|---|---|---|---|---|
| main | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| options_sliders | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| settings_hub | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| car_select | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| parts_browser | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| world_map | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| race_hud | **15/18** | 3/3 | 15/15 | 18/18 | 6/6 | 9/9 | 6/6 | **0/3** |
| freeroam_hud | 18/18 | 3/3 | 15/15 | 18/18 | 6/6 | 9/9 | 6/6 | **0/3** |
| pause | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | **0/2** |
| loading | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| dialog | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| movie | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| photo | 18/18 | 3/3 | 15/15 | 18/18 | 6/6 | 9/9 | 6/6 | 3/3 |
| post_race | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| race_start | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_main | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_options | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_settings | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_parts | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_map | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_carlot | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_lists | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |
| mouse_controls | 12/12 | 2/2 | 10/10 | 12/12 | 4/4 | 6/6 | 4/4 | 2/2 |

The failures: F1 portrait 1080×1920 (free roam, race, pause; every option): a flat band at the bottom of the
3D view (Vert− at 9:16 makes the vertical angle 2.37× the 4:3 one); F2 320×240 race HUD: text and gauges
minified past legibility. Everything the maintainer saw at 1366×768 (menu panel centred at its proportions, 3D full
width, cinematic bars on the start scene) holds at every other size too.

