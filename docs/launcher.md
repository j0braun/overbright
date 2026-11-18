# Launcher (`overbright`)

Design: the platform work launcher design (its work list: prerequisites, probe,
core, screens, start and watch, tests, packaging). Not native: SPEED2.EXE has no
launcher; the game starts and works without it (NFSU2_ROOT, game-root.txt,
`--set-root`, auto-detection all stay). Nothing from EA is shown or shipped:
its own dark look (a smoked slab, a rail under the page names, lime titles), set in
Sofia Sans Semi Condensed (SIL OFL 1.1, `src/launcher/fonts/`, OFL.txt beside it).

## The flow and the look (current)

This section describes the launcher as it is now; the dated parts below record how it got there, and
where they disagree this one holds.

**When the launcher appears** (`src/bin/overbright.rs`, decided before any window is mapped):
1. `--launcher` given (also the Steam launch option for "always show the launcher") → the main screen.
2. Shift or Ctrl held, or a pad's View (Back / Select; LB where Steam takes View) held at start
   (`platform_deck::held_at_start`; `OB_HELD_AT_START=0/1` answers for the start tests) → the main screen.
3. No usable game folder saved → the setup screens (Find your game).
4. The last start did not finish: the start marker (`start-marker.txt` in Overbright's folder, `dirs::start_marker`)
   is written by `run::start` before the game runs and removed once the start is past (`run::START_SECS`) or the
   game ended under the launcher's eyes; one left over means the launcher and the game went down together during
   a start → the recovery screen ("The last start didn't finish": Start with safe settings, Start as usual,
   Settings, Save support file…). Safe settings are the original graphics, a window, the HUD mod and texture
   packs off; they stay until changed in Settings / Mods.
5. "When Overbright starts" is "Show this window" (the default, `launcher.skip` off) → the main screen.
6. Otherwise the game starts at once and the launcher waits for it; a failed start opens the crash screen. A
   graphics check that fails shows up as the game's own `error.kind=gpu` line: the crash screen then shows
   "This computer has no Vulkan graphics driver" with Check again.

**Pages and screens** (`screens::Screen`, `--render <name>`): the top bar carries Overbright's mark and the
pages Game · Settings · Mods (LB / RB, Q / E, Ctrl+Tab, or a click; the rail's dot marks the page); the content
sits on one smoked slab; the footer has the version and the button legends of the last input (`legends`).
- `main`: the game's name, its state ("Ready · version 1.2 · English", "Checking…"), the folder, Play (focused),
  and three rows that open their pages: Graphics, Screen, Mods. When Play cannot work its fix takes Play's place
  (`main-novulkan`, `main-gone`); B / Esc asks before quitting (`main-quit`, Stay focused). The shaders run in
  their own block (`main-prepare`, `main-shaders`, `main-shaders-ready`).
- `settings`: Screen (hidden under gamescope), Resolution (read-only there), Graphics (the probe's choice tagged
  "suggested"), Intro movies, Title screen, When Overbright starts, Advanced settings ›, Back to defaults (Y / R; asks first,
  `settings-reset`); a description of the focused row at the right. Settings are saved when they change.
- `settings-adv`: Text size (100 / 125 / 150 %: above 100 % the page keeps the larger text and
  scrolls), Motion (Full / Reduced), Launcher sounds (off by default: a soft tick on a focus move and a press,
  synthesised, through cpal; never in tests or with `NFSU2_AUDIO_HOST=none`), Get shaders ready again,
  Storage › (below), Import saves, About Overbright (licence, notices, Make portable); This computer at the right.
- `dev` (the fourth page, Game · Settings · Mods · Dev; LB / RB, Q / E go round all four): Map editor (the
  "Open map editor (preview)" button, `screens::map_editor_button`: starts the desktop map editor on its own
  Open / New; greyed with a line saying why when `overbright-editor` is not beside the launcher, `dirs::editor_exe`),
  "Start the game with the editor tools" (this one start with `--editor`: F7 the texture editor, F8 the free
  camera; `Model::play_args`), Support file (Save…) and the Logs folder (Open), the F3 overlay's
  key; a description of the focused row at the right. Advanced keeps the rest.
- `storage` (`storage-portable`, `storage-custom`, `storage-missing`, `storage-copy-back`; Settings › Advanced ›
  Storage): Portable mode as a switch (on: asks, offers to copy what is kept now into `<program>/data`, writes
  `portable.txt`; off: asks, offers to copy back, removes `portable.txt`, the data folder stays; every copy
  question lists first what will be copied, what is already there and what differs, and never overwrites a
  file already there, docs/platform-dirs.md "Copying between places"; refused cleanly when the program
  folder cannot be written; applies from the next start: the footer says "Portable after restart" / "Portable ·
off after restart" and the page offers Restart now, focused, which starts the launcher again with the same
arguments (`Outcome::Restart`; the new one waits for the old one's lock)), then every folder with its path, Change… (the system
  folder dialog, Overbright's own browser with a pad or on the Deck), Open and Reset to default: game folder, HUD
  mod folder and the `platform::dirs::Location`s (docs/platform-dirs.md "Folders put elsewhere"). Change… asks
  "Copy and use it" (the plan listed first; copied on a worker by the same rules, checked, the old folder kept),
  "Use it, copy nothing" and, when files other than settings differ, "Copy, replace (backups kept)". A
  folder on a drive that is not there says so and keeps Reset. The page scrolls when the window is short.
- `mods` (`mods-empty`, `mods-off`, `mods-conflict`, `mods-order`, `mods-remove`, `mods-map-editor-open`,
  `mods-map-editor-failed`): Use mods; Texture packs
  (Overbright packs from `obpack`, then the imported TexMod / TexWizard packages, each with a switch; Remove is
  X / Space and asks first); a message when two packs that are on change the same textures, with Change the
  order (Move up / Move down) when both are in the same list; Add a mod…; HUD (HD HUD and maps, Add HUD mod…,
  Choose folder…, Forget folder); Map mods (`tools::map::store`: a switch each with its races and prop
  edits under the name; Open in map editor (the chosen mod; the editor's own Open / New is the Dev page's button), Import a
  map mod…, Export… as a `.zip`). The map editor (`overbright-editor`, `dirs::editor_exe` beside the launcher,
  `OB_EDITOR_EXE` names another) starts with `--mod <folder>` and the shown game folder as `NFSU2_ROOT`; one at a
  time: while it is open the section's controls are greyed and a line says the page waits; when it closes the
  map mods are read again (`App::poll_jobs`); a failed start or a bad exit is shown as an error line. The
  editor is not the game: no start marker, no recovery screen. Use mods off sets every pack, the HUD mod and the map mods
  off for the game and keeps what was on in `mods-paused.txt`; on gives it back. The page scrolls when the
  list is long.
- Setup: `first-run` / `first-none` / `first-several` (the copies found, or none with Choose the game folder…),
  `check` / `err-own` / `err-missing` (the folder and its verdict: the error with its fix as the focused button).
- `crash` (also `failure`), `recover`, `browse`, `import`, `prepare`, `about`, `licence`, `notices`.
Every error is an `model::Issue` (one per situation: title, cause and fix, its buttons), never matched by text.
"Save support file…" writes `diag::support` on a worker thread (the game program is asked for its
hardware check, up to 20 s; the window stays live with "Saving the support file…") and offers Open folder;
`overbright --support-file [folder]` does the same without a window. Text size, Motion and Launcher sounds are
the launcher's own (`launcher.txt` in Overbright's folder). A crash of native code (Windows) is written to
launcher.log (`diag::native_crash`). With a pad as the last input the page names carry the bumpers' glyphs and
the Settings note shows the held button's glyph; at the first start that found the game the main screen
says so, and Play keeps the folder. `--input pad|deck|ps|kbd|mouse` picks the legends of a render.

**Look**: `draw::THEME_A` (page #050605, ink #EEF0E8, secondary #C8CCC0, lime #C6F087 for titles and focus,
Play #F2F4EC), Sofia Sans Semi Condensed (italic for titles, Play, labels and buttons; upright for reading),
focus rings at least 3 px and never in the primary style. Sizes follow the mockups' unit (1 at 1366 × 768,
`Builder::ua`); the frame's vertical margins press together on short windows (`Builder::ac`).

**Checks** (`launcher::tests`): every screen and state laid out at 640×480 … 3840×2160 with 1280×800 and
1366×768 among them; `rendered_text_contrast` (WCAG 2 ≥ 4.5:1 and APCA Lc ≥ 60, reading text ≥ 75, measured
against the rendered pixels under each text); `smallest_glyphs` (at 1280×800 a capital ≥ 9 px and a line ≥ 12 px,
at 1920×1080 every text ≥ 18 px of em); the arrows reach every control; tests/launcher_render.rs compares the
renders at the eight sizes. Start tests: tests/launcher_start.sh (also: Save support file… on the failure
screen; under Wine the system folder dialog opened and cancelled), tests/launcher_fresh.sh (the pages:
Settings › Advanced for the game folder and the shaders, Settings for the display rows, kept at once),
tests/launcher_mouse.sh (scale 1 and 2; Back by its legend), tests/launcher_wayland.sh. The screen trace
lists each legend as `[button legend:<button> …]`.

## Part one (2026-11-12)

**Prerequisites, in the library**
- `launcher.skip` (bool, native off): a row of the option table
  (`qol/enhancements.rs`, `presets keep`, write rank 70, no deviation, no
  apply). options.txt dispatches `launcher.*` keys to it
  (`frontend/settings/options_txt.rs`), so the game, which rewrites
  options.txt from its model, keeps the line. Restore Native leaves it.
- `qol::setup`: `KEY_FILES` are the game's data files (SPEED2.EXE is not
  needed, docs/exe-tables.md), found in any case (`key_file_path`);
  `installed_languages(root)`; `saved_texture_pack[_file]` /
  `save_texture_pack` (`texture-pack-root.txt` beside game-root.txt, the same
  atomic write as game-root.txt); `check_lines(check, fingerprint)` + `Mark`:
  the check in plain words, shared by `--check-game` (game-side patch) and
  the launcher's check screen.
- `render::enh::assets::hd_ui`: the pack folder is `NFSU2_TEXTURE_PACK`,
  else the kept texture-pack folder, else `texture-pack` beside the exe;
  `is_complete(root)` for the launcher.

**The launcher core** (`src/bin/overbright.rs`, `src/launcher/`)
- winit window + softbuffer (CPU pixel buffer; no wgpu on its path, so it
  opens with no Vulkan driver: checked under Xvfb with
  `VK_ICD_FILENAMES=/nonexistent`), tiny-skia shapes, ab_glyph text.
- Widgets: label, status line (✓ ! i ✗ drawn as shapes), button (main /
  normal / disabled), cycle, list (scrolls, notes at the right), checkbox,
  text field (caret, typing, Backspace/Delete/Home/End, long text shows its
  end). Every text is measured and wrapped or shortened with "…" (paths in
  the middle) at layout time.
- Focus: Tab / Shift+Tab in build order; arrows move to the nearest control
  that way (nearest line, then nearest across); Left/Right step a cycle;
  Up/Down move in a list and leave it at its ends; Enter/Space use; Escape
  back. Mouse: every control works with the mouse alone (see "The mouse"
  below): hover, press, release inside to use (outside: nothing), cycle
  arrows, lists by the wheel, the scroll bar and double clicks, text fields
  by click.
  Gamepad (gilrs directly; the game's `pad::host` maps to game actions, not
  menu keys): d-pad / left stick move, A / Start use, B back, LB / RB =
  Shift+Tab / Tab.
- Scale: unit u = min(w/960, h/720), at least 0.75 × the display scale
  (body text ≥ 14 px at 100 %); content at most 1100 u wide, centred; a
  screen that does not fit is rebuilt smaller down to that floor, then the
  page scrolls. Label/control rows stack when narrow.
- `overbright --render <screen|state> --size WxH [--scale F] out.png`: a
  screen with fixed fake data (`model::FakeEnv`), no window, no files read;
  a state (`headless::STATES`, listed by `--list-screens`) is a screen with
  that data changed to what a player can meet: `no-game`, `check-ok`,
  `not-a-game-folder`, `files-missing`, `checking`,
  `first-start`, `probe-failed`, `no-vulkan` (the probe found no Vulkan
  driver: the line names the package to install), `long-values` (a long German path, many
  languages, a long pad name), `unofficial`, `portable` (About of a portable install), `start-failed`,
  `unreadable-folder`, `import-done`, `import-empty` and the Mods page's
  `mods-empty`, `mods-busy`, `mods-results`, `mods-remove`, `mods-hud-complete`,
  `mods-hud-incomplete`.
- Folders: `src/launcher/dirs.rs`, through `platform::dirs`
  (docs/platform-dirs.md): Overbright's folder (options.txt, game-root.txt,
  texture-pack-root.txt, `launcher.lock`), `logs/`, the profiles folder,
  the original's save folder (Windows only), the game exe beside the
  launcher (`OB_GAME_EXE` names another: the start tests' stub).

**Screens, first part**: `first-run` (found folders list, typed path,
Check), `check` (path + `check_lines`, Use this folder / Back), `main`,
`widgets` (all widgets). Settings are written through the game's own code
(`model::save_choices`: `Settings::load_text` → change → `write_file`),
only after a change.

## Part two (2026-11-12)

**`game --probe`** (`src/bin/game/shell/probe.rs`; the format in
`src/launcher/probe.rs`, one definition for both sides). Handled before the
command-line check and the game-folder search: no game files, no window, no
device, no log file. `key=value` lines, unknown keys ignored, exit 0 (or 1
with `error=` and `ok=0` without an adapter):
```
probe=1
version=Overbright 0.2.1 (commit unknown, linux-x86_64)
adapter.name=…            adapter.backend=Vulkan   adapter.type=DiscreteGpu
adapter.pci=1002:1234     adapter.driver=…         adapter.vram=<bytes, 0 unknown>
gpu.class=high            (qol::classify_gpu: the presets' tiers)
monitor=1920x1080@60.000  (the primary monitor; `none` without a display)
monitor.modes=1280x720,…  (its own video modes)   monitor.exclusive=1  (0 on Wayland)
modes=640x480,…           (display::mode_list of those: the old fixed list)
settings.file=0           (options.txt existed)
first_run.resolution=1280x960  first_run.fullscreen=0
first_run.detail=3        first_run.custom=1,1,2,…   (22 values)
ok=1
```
- The adapter is asked for with the game's own options
  (`Gpu::adapter_options`, now shared with `Gpu::new`; byte-identical) but
  without a window surface. **Inferred:** the same adapter as the window's
  on a one-GPU machine or where the high-performance GPU presents (here:
  the discrete GPU of a discrete + integrated pair); not checked on a
  multi-GPU laptop.
- The monitors come from a winit event loop that opens no window.
- `first_run.*` = `probe::first_run(card)` = `Settings::default()` +
  `first_run_display(card)`, what `create_window` does without options.txt
  (0x5CECA0's detail estimate, which also reads the CPU's speed on this
  machine). The estimate reads a table of the player's SPEED2.EXE
  (docs/exe-tables.md), so the lines are left out when no usable game
  folder is known (a fresh machine); the launcher runs the probe again
  with the chosen folder in `NFSU2_ROOT` (below).
- 0.03–0.3 s here (Vulkan instance + adapter).
- The launcher runs it on a thread (`probe::spawn`, 20 s limit) when its
  window opens, with the game folder in use as `NFSU2_ROOT`, and **again
  whenever the folder changes** (Use this folder; `window::Run::reprobe`;
  an older result still on its way is dropped); the GPU line shows
  "Checking…", then the adapter or why not. Before the game's first run
  (no options.txt) the Display / Resolution / Graphics rows are enabled
  once the probe gave the first-run values (the card alone is not enough:
  they need the folder), and show them; the first write is
  `Settings::default()` + the probe's detail estimate applied as
  `first_run_display` applies it (`estimated_detail`) + the choices,
  exactly the values the game's first start would have picked (tests
  `saving_respects_the_first_run`, `first_write_uses_the_probes_card`). The
  launcher itself never reads SPEED2.EXE: before, its first write called
  `first_run_display` in its own process, which read the table from the
  current folder and ended the launcher with a panic on a fresh machine.

**Screens, second part** (`src/launcher/screens.rs`)
- `main`: the Resolution row walks the list of the display path chosen,
  as the game's Options do (`display::modes_for` on the monitor: windowed
  sizes up to the desktop, the monitor's own modes in exclusive full
  screen, the desktop size alone on Wayland; switching Display moves the
  size with `fit_mode`). The monitor is the launcher window's until the
  probe reports the game's view (`monitor.modes`, `monitor.exclusive`);
  a probe from an older game without those keys reads as exclusive with
  the fixed list. Mods row with "Mods…" (the Mods page, below); Language,
  Graphics card and Controller as compact information rows (the first
  connected pad's name from gilrs, live); footer About / Import saves… /
  Quit / Play. Play and the whole footer stay in the window without
  scrolling at every size down to 640×480, also with the probe's note shown
  (test `play_is_reachable_without_scrolling`).
- `first-run`: the found folders say where they come from in words: "found
  on this computer", "inside a Wine prefix" (a `drive_c` in the path),
  "used last time" (game-root.txt), "set by NFSU2_ROOT", "the current
  folder"; a "Choose folder…" button, which opens the system's folder
  dialog (see "Choosing the game folder").
- `about`: the version line (`build_info::version_line`), GPL-3.0-or-later
  in plain words, "you need your own copy of the game", EA's disclaimer word
  for word ("Overbright is not affiliated with Electronic Arts. EA has not
  endorsed and does not support this product."); buttons Licence (GPL-3.0) and Third-party notices: `licence` /
  `notices` show `LICENSE` / `THIRD-PARTY-NOTICES.md` built in
  (`include_str!`), reflowed, scrolled with the arrows, Page Up/Down,
  Home/End and the wheel; Back at the top. The renders use a fixed sample
  notices text, so a new notice does not change them. Under the version
  line: "Source code: <url>" from `OB_SOURCE_URL` (the release build's
  `SOURCE_URL`, `model::source_url`: an http(s) address), else, as a
  warning line, "This is an unofficial build without a source link: ask
  whoever gave it to you for its source code." (an empty value and
  package.sh's placeholder `SOURCE_URL` count as none). `overbright
  --version` prints the version line and `source: <url>` or that sentence;
  `scripts/release/package.sh` fails a package whose launcher has no
  source link.
- `failure`: a title naming what went wrong, one sentence with the fix
  and one main button that does it, by the kind of error (see "Errors in
  plain words"); the log file's path; Open log folder (Explorer / `open` /
  `xdg-open`; when that fails the path is shown) and Details, which shows
  how it ended (exit code, panic, signal) and the game's last 8 output
  lines in a box; Settings (main), Quit. No "Copy log": a clipboard needs a
  new dependency.
- `browse`: the folder browser for the game folder, the texture-pack
  folder and the save import: the places, the folder shown as clickable
  parts, a list (".." first, then the subfolders sorted, hidden ones left
  out; notes "game folder", "saves") taking the room the window leaves,
  Enter or a double click opens the chosen row, Up one folder, a typed
  path (Enter), Cancel, Open, Use this folder; Windows' drives ("This PC")
  above the drive roots (see "The mouse"). It is the fallback of the
  system's folder dialog, the way with a gamepad, and the Mods page's
  pickers (they list package files with notes). On Windows also
  "Windows dialog…" (the system dialog from inside it).
- `import`: from (Windows: `%LOCALAPPDATA%\NFS Underground 2`, the
  original's folder (0x4A9C40); elsewhere a folder the player picks, the
  browser starting at the game folder's Wine prefix `drive_c/users`), to
  (Overbright's profiles folder), one line per profile ("will be copied
  (148 KB)" / "already in Overbright's folder; kept as it is") and the
  saved cover pictures counted; "Copy N saves" copies, then lists the files
  copied.

**Import saves** (`src/launcher/saves.rs`): a save is a folder holding a
file of its own name (any case): profiles `NAME/NAME` and saved covers
`NAME DVD n/…`, `NAME Magazine n/…`. Every file of such a folder is copied
(to `<file>.part`, then renamed) into Overbright's profiles folder. A name
Overbright already has (any case) is skipped, never overwritten; the source
is only read; Overbright's own folder as the source is refused. The player
may also pick one profile's folder. Not checked: whether the original keeps
anything else in its save folder (only profile and cover folders are
known; others are left out).

**Choosing the game folder** (`App::choose_folder`, `model::system_folder_dialog`)
- "Choose folder…" (first run), "Choose another folder…" (a refused
  folder), the save import's "Choose folder…" and the HUD mod folder's
  "Choose folder…" open the **system's folder dialog**:
  - Windows: `IFileOpenDialog` with `FOS_PICKFOLDERS`
    (`src/launcher/win_dialog.rs`, the COM interfaces declared by hand),
    on the window's thread (it runs its own message loop). Show's
    `ERROR_CANCELLED` is a cancel; any other failure means no dialog.
    Opened and cancelled by mouse under Wine, where it is Wine's own
    folder dialog; **not run** on Windows, no automated test opens it.
  - Linux: rfd 0.18 (MIT) with only its `xdg-portal` feature: the
    desktop's file chooser through `org.freedesktop.portal.FileChooser`
    (libdbus opened at run time, no build-time library), and zenity when
    the portal does not answer. It runs on a worker thread, so the
    launcher keeps drawing; the screen says "Choose the folder in the
    dialog that opened." meanwhile, and one dialog is open at a time. The
    dialog has no parent window (it may open beside the launcher, not on
    it).
  - rfd answers a cancel and "no dialog" alike. No dialog is assumed when
    there is no session bus and no zenity on `PATH`, or when the answer
    comes within 300 ms (faster than a person cancels).
  - A cancel changes nothing. No dialog: **Overbright's own folder
    browser** opens at the same place. It also opens instead of the
    dialog when the last input came from a gamepad (desktop dialogs are
    not made for one), inside gamescope (Steam Deck Game Mode:
    `SteamDeck=1`, `SteamGamepadUI`, `GAMESCOPE_WAYLAND_DISPLAY`, or
    `XDG_CURRENT_DESKTOP` naming gamescope; its portals have no
    FileChooser), and always with `OVERBRIGHT_OWN_PICKER=1`.
  - With no folder found, the first-run screen's default focus is
    "Choose folder…", not the path field (a pad or Game Mode may have no
    keyboard).
- The folder chosen is checked (`RealEnv::check`):
  - its long names: Windows' registry gives the install folder in 8.3
    form (`C:\PROGRA~2\EAGAME~1\…`); `model::long_path` shows and keeps
    the file system's spelling (`canonicalize`, its `\\?\` prefix taken off).
    The found folders get the same.
  - one level off is corrected (`model::game_dir_near`): a pick of
    GLOBAL, or of Overbright unpacked inside the game folder, takes the
    folder above; a pick of `EA GAMES` takes the one game folder below.
  - **Overbright's own folder** (it holds `overbright` / `overbright.exe`,
    or it is the folder the launcher runs from) and is no game folder is
    refused with its own title, "This is Overbright's folder".
  - a folder that cannot be read is refused as such.
- The check screen's first line is the verdict: "Ready to play", "Ready
  to play, with the notes below" (files that differ from the known
  release), or, for a refused folder, the title as the heading, one
  sentence with the fix and its button (Choose another folder…, or Check
  again when some files are missing). Two notes are left out as nothing to
  act on: the mod files found (Overbright ignores them) and what
  SPEED2.EXE is (Overbright does not use it).

**The state line is true** (`src/bin/overbright.rs`, `screens::game_state`)
- Before the window opens, the folder found (`setup::find`: NFSU2_ROOT,
  the kept folder, the current folder, the usual places) gets the same
  full check as the check screen (files, sizes, checksums and the
  fingerprint). A usable folder opens the main screen; a refused one opens
  the check screen with the reason; none, the first-run screen.
- The main screen's state line comes only from that check and the
  computer's check: "Ready to play" once both passed, else "Game folder
  checked · checking this computer…", "… · no graphics adapter answered
  (see below)", "… · this computer was not checked". Play is the main, focused action only when
  the game can start: while this computer is checked it is drawn as an
  ordinary button and not focused; with no graphics adapter, "Check
  again" beside the folder takes the focus and runs the check again
  (`screens::play_state`). The skip rule ("Start the game directly")
  starts the game only for a folder that passed the full check.

**Errors in plain words** (`run::ErrorKind`, `screens::failure_words`)
- The game prints a machine line, `error.kind=<kind>`, on its own line
  before its error text: `files-missing` (the game folder check at its
  start, `shell::startup`), `gpu` or `no-window` (its window or the
  graphics device could not be made, `shell::window`; by the error's
  words). The launcher reads it (the line is kept in the log, not in the
  lines shown), else infers `crash` from a panic line and `no-window` from
  "could not open a window"; `no-program` is the launcher's own (the game
  program could not be started); anything else is `other`.
- Each kind has a title, one sentence and one button; no file sizes,
  checksums, addresses or command lines (test `the_games_error_kind_line`):

  | kind | title | button |
  |---|---|---|
  | files-missing | The game files are not complete | Check the game folder (the check screen) |
  | gpu | No graphics driver the game can use | Try again |
  | no-window | The game window could not open | Play windowed (Display set to Windowed, saved, started) |
  | crash | The game stopped unexpectedly | Try again |
  | no-program | The game could not be started | Try again |
  | other | The game stopped (during its start) | Try again |

**Start and watch** (`src/launcher/run.rs`, `window.rs`,
`src/bin/overbright.rs`)
- `overbright [--launcher] [--screen <name>] [-- game args]`: arguments it
  does not know and all after `--` go to the game; the environment passes
  through.
- Play saves the changes, hides the window and starts `game` beside the
  launcher with the folder shown in `NFSU2_ROOT` (so the game plays the
  folder the player chose even when another one is set in the
  environment); its stdout and stderr are read on two threads. A non-zero
  exit, a panic line (`panicked at`, `[PANIC`) or the game's "could not
  open a window" line (it then exits with 0) is a failure, one within 10 s
  a failed start; the window comes back on the failure screen. A normal end
  closes the launcher. Closing the window while the game runs waits for it.
- Logs: `logs/launcher-<UTC time>.log` (newest 5 kept; the game's
  `game-*.log` are its own): the command, and the end ("exit code 101 …
  (a failed start)"). For a plain run the game writes its own log
  (`qol::log_file`), so the launcher's file adds the game's output only
  after a failure (its last lines); for runs with arguments, which leave no
  game log, all output (`[out]` / `[err]` lines). Home folders scrubbed to
  `~`.
- Back on the main screen (Escape, a pad's B) never quits at once: it asks
  ("Quit Overbright?", Stay focused).
- One launcher at a time: `launcher.lock` in Overbright's folder, held with
  `File::try_lock` until the process ends (also after a crash); a second
  start says "Overbright is already running." and exits.
- Skip: with `launcher.skip` set and a usable game folder, the launcher
  starts the game at once and shows nothing, unless `--launcher`, a held
  Shift or Ctrl (Windows: GetAsyncKeyState; X11 / XWayland: XQueryKeymap through
  x11-dl, loaded at run time), a held pad View / Back, or a start that did
  not finish (see "When the launcher appears" above). A failed start
  then opens the window on the failure screen. **Limit:** a pure Wayland
  session gives no keyboard state to a program without a focused window:
  Shift is seen only through XWayland; `--launcher` always works.
- The game starts and works without the launcher as before.
- Windows: `overbright.exe` is a window program (no console); a start from
  a terminal attaches to it.

**When the launcher does not open** (`src/launcher/start_log.rs`,
`src/bin/overbright.rs`)
- `launcher.log` in Overbright's folder, next to options.txt (Windows
  `%LOCALAPPDATA%\Overbright\launcher.log`, Linux
  `~/.local/share/overbright/launcher.log`; `NFSU2_DATA_DIR` and portable mode
  move it with the rest): written from the launcher's first line by every window run
  (the tool flags `--render`, `--version`, `--list-*`, `--add-*`, `--help`
  leave it alone). It starts with the version line, the system (on Windows
  the version and build number: Windows 11 is 10.0 build 22000 and later),
  the program's path, the arguments and Overbright's folder, then the
  start's steps (the game folder found, gamepads, the window's size and
  scale, the first picture shown, the hardware check) and every log line
  from Info up; each line is written at once, so a hard stop leaves the
  steps before it. The previous run's file is kept as `launcher.old.log`;
  a run that did not end with `end: the launcher closed normally` is noted
  in the next run's header. Home folders are shown as `~`. `RUST_LOG`
  still decides what reaches the terminal.
- A panic on any thread is written to the file (message, place, thread,
  call stack; a Windows release build's call stack shows addresses only)
  before the thread unwinds. One that ends the launcher (the main thread,
  or winit's event handlers, which hand it back to `main`) is caught in
  `main`: exit code 101 and the message below.
- Every failure that ends the launcher (an error or a panic) is printed to
  stderr and, on Windows when no one sees stderr (a start from Explorer or
  a shortcut), shown in a message box: one plain line of what happened,
  the log's path and where to report it. "Overbright is already running."
  is shown the same way; the lock is taken before the log is opened, so a
  second start leaves the first one's log alone.
- What a player attaches to a report: `launcher.log` (and
  `launcher.old.log` when the problem was the start before), and after a
  game start also the newest file in `logs/`.
- Not caught: a stop the process cannot report (a stack overflow, the
  system or a security program ending it); the log then ends at the last
  step reached and the next start notes it.

**The game's setup help** (`qol::setup::help` / `help_for`): plain words
for players, the shipped program names per system (`overbright` /
`./game --set-root "$HOME/Games/…"`, `overbright.exe` / `game.exe
--set-root "C:\Program Files (x86)\EA GAMES\…"`); no developer wording
(NFSU2_ROOT, "the clone").

## Tests
- `src/launcher/tests.rs`: every screen at 640×480, 800×1280, 1280×720,
  1920×1080, 2560×1080, 3840×2160 (and 2560×1440 at 200 %): every text
  inside its widget and off the marks/arrows, measured width = box, widgets
  inside the window and not overlapping, no two texts overlapping, every
  glyph in the font, body ≥ 14 px (documents: the same over their scrolled
  length); long paths / many languages / errors wrapped or shortened; focus
  order and arrow moves; keyboard, text and mouse changing the model; the
  first-run, browser, import, about/licence and failure flows; Play
  reachable without scrolling; the first-run trap of `save_choices` and its
  use of the probe's card (sandboxed data folder).
- `launcher::probe::tests`: a captured `game --probe` output
  (`tests/launcher/probe-sample.txt`, adapter name and ids replaced) parses;
  write → read round trip; errors. `launcher::saves::tests`: a copy into a
  temporary folder, the source unchanged. `launcher::run::tests`: a stub
  game (shell script) printing a panic and exiting 101 → a failed start,
  the tail, the log; log pruning; the lock refusing a second holder.
  `qol::setup::tests::help_is_worded_for_each_system`.
- `tests/launcher_render.rs`: the 66 renders (11 screens × 6 sizes) and the
  31 states at 1366 × 768 with 150 % scaling (`state-<name>-1366x768@150.png`)
  against `tests/launcher/*.png` (our own images; at most 0.05 % of pixels off by
  more than 2), and a self-check that a changed screen fails. Re-bless:
  `OB_LAUNCHER_BLESS=1 cargo test --release --test launcher_render`, look
  at the PNGs, `optipng -o2`.
- `tests/launcher_fresh.sh <dir with game and overbright> <X display|auto>
  <work dir>`: the first run on a fresh machine with the shipping
  binaries (empty HOME, no NFSU2_ROOT, `env -i`), steps a–i: `game
  --probe` without first_run lines; the first-run screen; a wrong folder
  refused; a SPEED2.EXE with a table byte flipped refused, a patched copy
  ("modified copy … game data matches") used; a folder with every name in
  another case (symlinks to the player's game) used; the probe run again
  each time, the display rows live with the values `game --probe` prints
  for that folder; no options.txt until Play, then the first-run values +
  the choice; Play starts the game (its log in the logs folder, `game
  start: done`), its window closed with WM_DELETE_WINDOW, "a normal end"
  and exit 0; a second start on the main screen with the saved folder.
  It reads the screens from the launcher's log (`RUST_LOG
  launcher::screen=debug`: each screen's texts, control ids, "(off)", boxes
  for clicks) and keeps screenshots. RADV on Xvfb has no DRI3: the game
  presents through Mesa's software path (`MESA_VK_WSI_DEBUG=sw`). Run by
  `scripts/checks/checks.sh` in the full set (`FRESH=1` in fast), with
  `auto` (its own Xvfb, stopped by PID).
- `tests/launcher_start.sh <overbright> <X display> <work dir>`: on a private
  X display (never the desktop: `env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE
  DISPLAY=…`), `launcher.skip` set, a stub game that prints a log and exits
  101: the failure screen opens (window found with xdotool, screenshot),
  the launcher's log holds the exit code and the tail, a second launcher is
  refused, Escape + Enter (Play) from the window fail again. Needs
  NFSU2_ROOT. `OB_WINE=1` runs it for `overbright.exe` under Wine with
  `cmd.exe /c stub.bat` as the game.
- `qol::tests::launcher_prerequisites`, `launcher_skip_row`; the option
  table tests count 71 rows.
- `launcher::tests::skip_boxes_round_trip_through_options_txt` (the two
  start boxes: toggled with the keyboard, saved, read back by the game's
  parser and the launcher, an older `enh.skip_title 2` shown as both).

**Windows build and Wine (2026-11-12)**: `overbright.exe` and `game.exe`
built in the release container (`scripts/release/container/build.sh
--windows --fast`), Wine 11 on a private prefix and a private Xvfb:
- all 60 `--render` images equal the Linux baselines (1 % tolerance, none
  off); `game.exe --probe` prints the adapter (Vulkan through winevulkan),
  the monitor and the first-run values, exit 0; `game.exe --check-game`
  exit 0; `tests/launcher_start.sh` with `OB_WINE=1` and `game.exe
  --crash-test` as the failing game: PASS (failure screen with the panic
  lines, the launcher's log, the second launcher refused, Play from the
  window).
- Found under Wine and handled: winit asserts while listing a monitor's
  modes when Wine reports modes without every field (the launcher window
  and `--probe` now fall back to the monitor's own size); Wine hides
  `XDG_*` from Windows programs (renamed `WINE_HOST_XDG_*`), so
  `XDG_DATA_HOME` does not isolate a Windows run under Wine — the test uses
  `NFSU2_DATA_DIR`.
- Wine-only quirks, not fixed: after a game run the window is mapped off
  the Xvfb screen (X position 7204, 1110; drawn and taking keys; the test
  moves it into view); with `cmd.exe /c stub.bat` as the game, Wine's
  console host ends the launcher with an X error (BadWindow on
  UnmapWindow).
- Not proven by Wine: real Windows window placement, DPI, the
  `IFileOpenDialog` button, GetAsyncKeyState's Shift at start, Explorer
  opening the log folder, `%LOCALAPPDATA%\NFS Underground 2` of a real
  install, file locking on NTFS, SmartScreen on the unsigned exe.

## Fresh-machine walk (2026-11-12, unit LC)

Walked with the shipping binaries on an empty HOME (tests/launcher_fresh.sh).
Found and fixed:
- The launcher's first write before the game's first run called
  `first_run_display` itself: it read SPEED2.EXE from the current folder
  and panicked (window gone) on Play or Quit after any change. Now it
  takes the probe's values; the rows wait for them.
- The probe ran once, before a folder was known, so its first-run values
  never came; the rows were enabled from the card alone. Now it runs
  again with the chosen folder.
- A SPEED2.EXE whose tables differ was shown with the error line but "Use
  this folder" stayed enabled (the game then refuses to start): an error
  line now makes the folder unusable; SPEED2.EXE is no longer also listed
  as "differs … may still run" next to its own verdict.
- Mod files in another case (DINPUT8.DLL) were not found; language names
  showed in the folder's case ("gERMAN"); the edition line said "the build
  the clone is traced on".
- A typed path kept from before had its caret at the start (Backspace did
  nothing): a field focused by default now has the caret at the end.
- Play did not tell the game which folder: with `NFSU2_ROOT` set to
  another one the game played that one.
- The game exits with 0 when its window cannot open: the launcher called
  it a normal end and closed without a word. It now reads the game's
  "could not open a window" line as a failed start. (The game's own exit
  code is a game-side follow-up.)
- The window has the exe's icon (`platform::icon`, `_NET_WM_ICON` checked
  in the walk).

## Look, hierarchy and first-run wording

Compared with the launchers and first-run screens of other projects that
run on the player's own game data (OpenRCT2, OpenMW's wizard, Ship of
Harkinian, OpenGOAL, devilutionX, fheroes2, CorsixTH, ScummVM; general
launchers such as Prism and Heroic), not native:
- **Every refusal carries its fix.** A folder the check refuses says why in
  one sentence by what is wrong (`screens::advice`: most files missing →
  "the game's files are not here … the one that holds the GLOBAL, FRONTEND
  and TRACKS folders"; a few missing → reinstall / copy again), and its main button is
  "Choose another folder…" (the browser beside the refused folder; Cancel
  comes back to the check). "Use this folder" stays, off (the fresh-machine
  walk reads it).
- **Play is the obvious thing.** The main screen's footer is split: About
  and Import saves… at the left, Quit and Play at the right; the main
  button is at least 150 units wide. The game folder is plain text (both
  ends of a long path kept, `Ctrl::Path`) before its "Change…" button, not
  a dim note after it.
- **Line length.** Paragraphs and status lines wrap at `layout::MEASURE`
  (760 units, about 80 characters), not at the 1100-unit column.
- **Contrast** (WCAG 2 ratios, tests `theme_contrast`, `backdrop_keeps_text_readable`): text, dim text, the
  empty field's hint (`PLACEHOLDER`, was the disabled grey at 3:1) and the
  status colours ≥ 4.5:1 on the page and on a control; a control's outline
  (`BORDER`) 3:1 against the page (was 1.6:1: fields and buttons were hard
  to see); disabled text 3.4:1 on its control; rules and the log box keep
  the faint line (`RULE`).
- A text field not being typed in shows both ends of a long path (the end
  only while typing, the caret's side).
- Wording: the game folder is named by what it holds ("the folder the game
  was installed to: it holds GLOBAL, FRONTEND and TRACKS"); the copy the
  player needs is "your own copy of Need for Speed Underground 2 for PC,
  any region, patched to 1.2" (no file name); "Check this folder"; the
  header shows the launcher's number alone beside its name (0.3.0, never
  "version 0.3.0" beside the game's 1.2); the GPU line says "Checking this
  computer…"; the Graphics row's Enhanced reads "More detail and light; the
  same art and colours"; the other installed languages "not yet"; the
  failure screen asks to send the log file named when asking for help.
- **First window size**: 960 × 720 logical, at most 92 % of the monitor's
  width and 85 % of its height (`window::initial_size`: a 1366 × 768
  laptop gets 960 × 652 under its task bar), the minimum size no larger.
- **Pad repeat**: a held d-pad direction or stick repeats after 400 ms,
  then every 110 ms (lists, the Resolution row), as a held arrow key does.
- Sizes and scalings (test `every_state_fits_every_size_and_scale`): every
  screen and every state at 1280 × 720, 1366 × 768, 1920 × 1080 and
  3840 × 2160 at 100 / 150 / 200 %, laid out clean (inside, no overlap,
  every glyph, ≥ 14 px at 100 %) and without scrolling whenever the window
  is at least the launcher's smallest (480 logical pixels high); below that
  (768p at 200 %) the page scrolls, nothing clipped.

## The visual pass: identity, the main screen, the backdrop

- **Identity, all our own**: the header is Overbright's icon (the amber
  ring, `platform::icon`, drawn from build_support/icon_art.rs), the name
  in bold and an amber bar under it; the one accent is the ring's amber
  (`theme::ACCENT` #FFA12E: the main button, the focus ring, the selection),
  with hover and pressed shades (`ACCENT_HOVER`, `ACCENT_PRESSED`,
  `SURFACE_PRESSED`: a button held with the mouse shows pressed).
- **Panels** (`Kind::Card`, `Builder::card_begin` / `card_end`): every
  page's content sits on a panel (`content_begin`, closed by the footer),
  so no text but the header's lies on the backdrop.
- **The main screen** (`screens::build_main`): the hero panel holds the
  game ("Need for Speed Underground 2", its state with a mark —
  "Ready to play", "… checking this computer…",
  what is wrong — the folder and Change… / Find game…) and **Play**, the
  largest control (a 220 × 72-unit button, `Builder::big_button`), right
  beside it, focused by default. Under it the **Start settings** panel
  (Display, Resolution, Graphics) and, beside it when the window is wide,
  the **Mods** panel (what is on, the packages' names, Mods…) and the start
  choice; the computer's facts (language, graphics card, controller) as a
  quiet line; About / Import saves… at the left and Quit at the right of
  the footer. The panels grow to the footer, so no half of the window is
  left empty at any size (`main_screen_hierarchy`).
- **The backdrop** (`backdrop.rs`): at once, Overbright's own (a soft amber
  glow from the top left on the dark page); once a usable game folder is
  known, the game's title picture (LS_SPLASH in `FRONTEND/FrontB.lzc`, read
  with the game's own readers on a worker thread: 7 ms here), shrunk to 160
  texels, blurred, darkened to at most 24 % over the page and faded in over
  300 ms (`window::poll_backdrop`; it does not delay the first frame or
  block input). It is read at each start and never written anywhere; no
  game folder, an unreadable file or no picture: Overbright's own stays.
  Contrast: the header's text keeps 4.5:1 over the brightest the backdrop
  can be (`backdrop::brightest`, test `backdrop_keeps_text_readable`; the
  version note uses `PAGE_DIM`). `overbright --render <screen|state> --size …
  --game <folder>` draws a render over that folder's picture (for a look
  on the developer's machine; the committed renders use the fallback, so
  nothing of the game is in the tree).
- **Arrows and the pad**: Up / Down only consider what is within 160 units
  sideways when anything is (a move keeps to its column instead of
  jumping across), test `every_control_is_reached_with_the_arrows`: every
  control of every screen and state is reached from the default focus
  with the arrows (the d-pad) alone.
- Not done (on purpose): a fade between screens (it would add redraws for
  no information); the start-to-first-frame time stays within the polish
  round's (interleaved on a private Xvfb, 15 runs each, main screen:
  35.2 ms before, 37.6 ms after).

## Mods

The main screen's **Mods** row says what is on ("None: the game's own textures", "2 texture packages on,
HUD mod on") and opens the Mods page (`screens::build_mods`): one sentence that Overbright includes no
texture art and what can be added (`screens::MODS_INTRO`: TexMod packages `.tpf`, TexWizard packs as a
folder or `.zip`, the HD HUD mod; docs/texture-packs.md has the full table of formats); the texture packages
(a list: name, textures, size, on / off; Add
package…, Turn on / off (Enter on the list does the same), Move up, Move down, Remove… with a confirmation
whose default is "Keep it"); the result line of each import; the HUD mod's folder, its check
(`hd_ui_pack::inspect`), "Use the HUD mod" (= `enh.hd_ui`, written with the other choices through the
game's settings code) and Choose folder… / Forget folder. The folder browser's `Packages` purpose lists
`.tpf` files, TexWizard pack folders and `.zip` files holding a TexWizard pack with the folders
(`model::read_packages`; a pack folder is listed once, as a package), Enter on one imports it; "Import all
here" takes `model::packages_here` (the folder itself when it is a package; in a game folder with TexWizard
or its `TexturePacks`, the packs TexWizard loads, most important first, `formats::texwizard::find_packs`). An import runs on a worker thread (`window::poll_import`, the progress in a
status line); closing the launcher meanwhile leaves only the temporary folder, removed the next time.
docs/texture-packs.md has what is kept where and how the game picks the packages up. Tests:
`launcher::tests::mods_page_flow`, `mods_page_when_empty`, `texture_mods::tests` (a package made in the
test, the real import path on the player's game files, broken and foreign packages, the list, removal),
`texture_pack::tests::launcher_texture_mods_reach_the_loader`; the renders `mods-*.png` and the states
`mods-empty`, `mods-busy`, `mods-results`, `mods-remove`, `mods-hud-complete`, `mods-hud-incomplete`.

**The HUD mod from its downloads** (`hud_mod`, docs/texture-packs.md "The HUD mod"): **Add HUD mod…**
opens the folder browser (purpose `HudMod`: folders, and `.zip` / `.7z` / `.rar` files with their kind and
size). Enter on a `.zip`, or "Use this folder", takes it (`App::hud_pick`): the part it is (HUD part:
`assets/userstart.end`; map part: `Binary/script.end`) joins `Mods::hud_parts`, the other part is
looked for beside it; with one part a line asks for the other (focus back on Add HUD mod…); a 7-Zip or RAR
file says to extract it first (its extracted folder beside it is taken when there); anything else: "is not
the HUD mod". With both parts the conversion runs on the window's worker (`window::poll_hud`, the progress
as a status line), its result line shows, the converted folder becomes the HUD mod folder
(`texture-pack-root.txt`), the existing check runs on it and **Use the HUD mod** is focused. "Choose
converted folder…" (was "Choose folder…") still takes a pack converted earlier. Test
`hud_mod_from_its_downloads`; states `mods-hud-waiting`, `mods-hud-converting`, `mods-hud-set-up`,
`mods-hud-not-the-mod`, `browse-hud`, `browse-many`.

**Without the window** (scripts, the Windows smoke, maintainers): `overbright --list-folder <dir>
[game|packages|hud]` prints the browser's rows; `--add-packages <file|folder>…` and `--add-hud-mod
<file|folder>…` do what the Mods page does, with the same code, into the player's own folders, and print
its lines (`ok:` / `warning:` / `error:`); exit 0 only when everything was set up.

**The screen trace keeps whole values**: a text drawn shortened (a long path with its middle left out,
a list row, a field not being typed in, a cycle's value or a button's label) carries its whole value
(`ui::TextBox::full`, `ui::text_of`); the screen trace (`launcher::screen` at debug level, what the start
tests and assistive output read) prints it after the drawn text as `⟨…⟩` (`window::trace_texts`). The
launcher has no tooltips. Test `a_long_path_is_drawn_short_and_traced_whole` (a 200-character game folder at
three sizes); tests/launcher_fresh.sh passes from a 149-character work folder (its step f looks for the
whole path).

**The backdrop switch**: `launcher::backdrop::FROM_THE_GAME` (on). Setting it to `false` is the one line
that keeps Overbright's own glow behind every page and never reads the game's picture.

**First run**: under "needs your own copy" the line "No mods are included either. Texture and HUD mods you
have can be added later under Mods; they belong to their authors." (`screens::NO_MODS`).

## The mouse

Every screen can be used with the mouse alone; the keyboard and the pad work as before (their tests are
unchanged). What was wrong before: the clicks were right (positions and boxes are both in the window's
physical pixels; checked on X11 at scale 1 and 2 and in a nested Wayland session at 1, 1.25, 1.5 and 2),
but the wheel and a touchpad only scrolled the page, never a list, so the folder browser showed its first
six rows and the rest could only be reached with the arrow keys; the row chosen when the browser opened
("..") went up on its first click; and (read from the code) a page scrolled by the wheel was scrolled
back to the focused control by the rebuild that followed (`build_fit` always called `keep_in_view`; now
only after a key, `UiState::mouse_scrolled`).

- **Lists** (`ui::handle`, `ui::list_bar`): the wheel scrolls the list under the pointer, 3 rows a notch
  (`ui::WHEEL_ROWS`); a touchpad's pixel deltas add up in `UiState::wheel_rows` to whole rows (a third of
  a row three times is one row: nothing is lost to rounding). Over anything else the wheel scrolls the
  page. A list with more rows than it shows has a scroll bar inside its right edge: the thumb is dragged,
  a click on the track pages. The row under the pointer has a shade (`SURFACE_HOVER`); the chosen row
  keeps its own (`SELECTED`) when the pointer leaves. The wheel and the bar move the rows, not the
  selection; a key that moves the selection brings it back into view (`UiState::list_sel`).
- **Choosing and opening**: one click chooses a row; a second click on the row the last click chose opens
  it (a double click, or a slower second click); a click on a row chosen by the keyboard only chooses it
  (so the ".." chosen when the browser opens does not go up on a single click). Open does the same for
  the chosen row.
- **The folder browser** (`screens::build`, `Screen::Browse`): "Places:" links (`Env::places`: Home,
  Desktop / Documents / Downloads when they exist, the game folder and the texture-pack folder when
  chosen; on Windows "This PC" and the drives `C:` …; elsewhere "File system" (`/`) and the drives
  mounted under `/media/<user>`, `/run/media/<user>`, `/mnt`); the folder shown as clickable parts
  (`model::path_parts`, the last nine, "…" for the one before them; on Windows "This PC" first, then the
  drive `C:\` or the share `\\server\share\`, Windows' `\\?\` long-path prefix kept in the folders
  but not shown); the list taking the room the window leaves (`App::rebuild`, `Builder::list_grow`);
  Up one folder (from a drive's root: the drives); the mouse's back button goes up a folder (elsewhere it
  is Back); the footer Cancel, Windows dialog… (Windows), Open, Use this folder.
  It opens when the system's folder dialog cannot (see "Choosing the game folder").
  For the game folder, "Use this folder" takes the highlighted row when it is marked "game folder"
  (what a player choosing the game means), else the folder shown above.
- **Text fields**: a click puts the caret at the nearest character (`Kind::TextField::stops`; a field
  showing its text shortened in the middle puts it at the end); a double click selects the whole text
  (drawn on `SELECTED`), which typing, Backspace or Delete replace (`Edit::Clear`).
- **Buttons, links, checkboxes**: act on release inside the control (a press dragged off does nothing);
  cycle arrows step on the press. The page's own scroll bar (a page taller than the window) is dragged
  and clicked too.
- **The pointer's shape** (`Ui::pointer_at`, winit's `set_cursor`): a hand over what can be clicked, the
  text beam over a text field, the arrow elsewhere (X11, Wayland, Windows).
- **Dropped files** (`App::drop_paths`, winit's `DroppedFile`, gathered per drop): a game folder (or a
  file in it, SPEED2.EXE) on the first-run or check screen or in the game folder's browser is checked as
  if chosen; `.tpf` files, package folders and `.zip` files holding a TexWizard pack on the main or Mods page or in
  the packages browser are imported (a dropped folder that is no package: the TexWizard packs in it); other
  `.zip` / `.7z` / `.rar` files there (and folders in the HUD mod's browser) go to the HUD mod.
  winit 0.30 delivers drops on X11 (XDND) and Windows (OLE drag and drop), not on Wayland (no drag and
  drop in its Wayland backend): there nothing else changes. Covered by the model test; a real drag in a
  window is not tested (no drag source on Xvfb).
- **Scaling**: winit gives the pointer, the wheel's pixel deltas and the window's size in physical
  pixels on X11, on Wayland (also at fractional scales, through wp_fractional_scale and wp_viewporter)
  and on Windows (the manifest is per-monitor DPI aware); the layout is built in the same pixels with
  the scale factor only choosing the unit, so no position is converted (`window::wheel_input`).
- **The screen trace** for scripts: each list's rows, its first row, the chosen row and its scroll bar
  on the line after it (`rows first=… shown=… sel=… len=… row_h=… pad=… bar=x,y,w,h thumb=y,h: a | b`);
  the folder shown as the whole value of its last path part (`⟨/data/player/Games⟩`); `launcher::input`
  at debug level logs the pointer (`cursor x,y scale s`), the wheel's deltas and dropped files.

Tests: `launcher::tests::mouse_only_from_the_first_start_to_play`, `mouse_only_mods_add_and_remove`,
`the_wheel_scrolls_the_list_under_the_pointer` (scale 1, 1.25, 1.5, 2), `the_list_scroll_bar_and_hover`,
`path_parts_places_and_back_button`, `path_parts_on_each_system` (drive letters, shares, long paths),
`text_field_by_mouse`, `dropped_files_do_what_choosing_them_does`, `wheel_deltas_from_winit`,
`page_scroll_bar_by_mouse`; the state `browse-many` (40 folders scrolled, a row under the pointer).
`tests/launcher_mouse.sh <overbright> <X display|auto> <work dir> [scale]`: the real window on a
private Xvfb, the mouse alone (xdotool): the browser by the wheel, double clicks, Open, the back button,
the Home place, the scroll bar dragged and clicked, a path part; a folder checked; with NFSU2_ROOT the
game folder chosen and a package added on the Mods page. Run by checks.sh with the launcher's fresh run
(`FRESH=1`, full) at scale 1 and 2; with `OB_WINE=1` under Wine (also "This PC", the drives, Up at a
drive's root), as the Wine smoke's step `l_mouse`. `tests/launcher_wayland.sh <overbright> <work dir> [scale…]`: the browser on
Wayland at scale 1, 1.25, 1.5 and 2 in a private headless mutter (its own D-Bus without activatable
services, its own runtime folder and socket; the pointer through mutter's RemoteDesktop API,
`tests/launcher_wayland.py`): the scale factor winit reports, a click on Browse…, a wheel notch (3 rows),
100 logical px of touchpad scrolling (whole rows at the scale's row height), a double click; with the
fresh run too (SKIP without mutter). winit 0.30 on X11 reports a wheel button sent by
XTEST (xdotool) on its press and on its release, so one xdotool notch is 6 rows there; a real wheel
comes as XInput2 scroll motion, once.

## The prepare step

`Screen::Prepare` and `launcher::prepare` (docs/prepare.md): the game run with `--prepare` as a
child, its `step=` lines shown as a bar (`Kind::Bar`, `Builder::bar`) and a line. Started by
itself once a session when `game --probe` says `prepared=0` for the folder in use and the main
screen is up (`App::offer_prepare`, called by the window, which owns the child:
`Run::poll_prepare`), behind the main screen (`App::start_prepare_background`): the hero's
state line says what it does (`game_state`, `State::background_words`) and **Play** stops it
first (`Run::play`). **Prepare again** in the main screen's footer opens `Screen::Prepare` with
the run; **Skip** there stops the child at any time; Done / failed end in **Continue**.
`OVERBRIGHT_NO_PREPARE` (set at all) keeps it from starting by itself (the mouse test sets it;
"Prepare again" still works). The launcher stops a run still going when its window closes.
States for `--render`: `prepare-running`, `prepare-done`, `prepare-failed`. Tests:
`launcher::prepare::tests`, `the_prepare_step_is_offered_once_skippable_and_can_be_asked_for`,
the fresh-machine walk's step p (started behind the main screen after the folder is used, its
line, not started again, Prepare again to Done, the stamp, `prepared=1` from the probe).

## Portable mode

A file `portable.txt` (or a folder `portable`) beside `overbright` and `game` makes the install portable: both
programs keep settings, saves, caches, logs, crash reports, screenshots and mods in `data/` beside them
(`platform::dirs`, docs/platform-dirs.md "Portable mode"), the launcher's `launcher.lock` too.

- The header shows `<version> · Portable` on every screen.
- About shows where Overbright keeps its files (`Model::folders`, `Env::folders`). Outside portable mode it
  has **Make portable** (`Env::make_portable`): it writes `portable.txt` beside the programs and looks again
  (`dirs::refresh_portable`), so the rest of the session uses `data/` for everything: `launcher.log` goes on in
  `data/launcher.log` (`start_log::follow`; the old file's last line names the new one), the lock is taken in
  `data/` too, the log folder of a game started next is looked up at each start (`GameStart::logs`), and the
  game folder, the HUD mod folder and the main page's choices are written into `data/` at once (options.txt's
  other values start native). Its line says what to copy over to take the other settings, saves and mods along
  (Overbright's folder and, on Linux, the profiles folder); nothing is moved. A folder that cannot be written
  (an install under Program Files) shows the error. Turning it off: delete `portable.txt`.
- The kept game folder (`game-root.txt`) and HUD mod folder (`texture-pack-root.txt`) are written relative to
  the program folder when they are on the same drive (`qol::setup::save_root` / `saved_root`), read back from it, and shown resolved.
- State `portable` (`--render portable`): the About page of a portable install. Tests
  `about_shows_and_turns_on_portable_mode`, `start_log::tests::the_log_follows_a_moved_folder`.

## Open (next units)
- Copy log (clipboard), a GPU class for the presets from the probe.
- The game's exit code when its window cannot open (0 today).

## Start choices: skip the intro movies, skip the title screen

Settings has the rows **Intro movies** (Play / Skip: EA, THX, the warning screen and the intro) and **Title
screen** (Show / Skip: "Press Enter" and its wait). Like the display rows they are
options.txt values, so they are shown only when the launcher can write them (`display_rows_on`: options.txt
exists or the probe gave the first-run values); before that the panel's line says when they come. They are
the game's own Enhancements rows Skip Intro Movies and Skip Title Screen (docs/start-and-menus.md):
`Choices::skip_movies` / `skip_title` read with `Enhancements::skips_movies` / `skips_title` and written with
`set_skips` through `save_choices` (options.txt's `enh.skip_movies` / `enh.skip_title`), so a change in either
place shows in the other. Both ticked, nothing plays between the start and the main menu's profile step.
Both are off by default (native). Render state `skip-movies` (the movies skipped, the title kept). Tests:
`skip_boxes_round_trip_through_options_txt`, the focus order on Settings (`display`, `resolution`, `graphics`,
`skip_movies`, `skip_title`, `skip`, `advanced`, `reset`).
