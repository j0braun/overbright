# Map editor and map mods

> **Preview.** The map editor ships as a preview: it works, but not every part is finished and its
> files may still change between versions. Keep a copy of map mods you care about.

The same editor runs in its own window (`overbright-editor`) and as the World and Races workspaces of Overbright
Studio ([Studio](studio.md)); its tools, keys and files are the same in both, except that in Studio Ctrl+D opens
Studio's Changes page (Duplicate stays in the Edit menu and the prop menu) and Ctrl+N opens Studio's New mod
dialog.

With the map editor you can:

- move, delete and add props in the city (signs, barriers, trees, street furniture);
- build races of your own on the game's roads, checked as you go;
- test a race in the game with one key, or simulate it without a window and watch the replay;
- share your work as a `.zip`.

The editor is a desktop program, `overbright-editor`, next to the game and the launcher. Everything it saves is
a **map mod** in Overbright's own data folder, applied on top of the original data when a world loads. The
game's files and your career save are never written.

If no map mod is on, none of this runs: worlds and races are the original's.

## Opening it

- From the launcher: **Mods** page, **Map mods**: choose a mod and press **Open in map editor**; or on the
  **Dev** page press **Open map editor (preview)** to make a new mod or open another one there. Only one editor
  runs at a time. While it is open the launcher's Map mods controls are greyed out; when it closes the launcher
  reads the mods again.
- From a terminal: `overbright-editor [--mod <folder>] [--region <code>] [--maps <dir>]`. The game folder is
  taken from `NFSU2_ROOT` or Overbright's saved game folder, as the game does; `--maps` names another map mods
  folder.

Each editor window edits one map mod on one region (the city, `L4RA`, for nearly everything). With no mod open
the editor shows its start screen: **New map mod…**, **Open map mod…**, **Import…**, the recent mods and the map
mods folder.

## The window

There are no modes: one tree, one toolbar, and the tools follow what is selected.

- **Toolbar**: the same buttons in the same order in every state. At the left **Select** (Q), **Move** (W),
  **Rotate** (E) and **Snap** (right-click it for the steps; with a race tool armed it snaps race points to the
  road); then the race tools in their own slot: **Checkpoint** (C), **Start** (S), **Finish** (X) and
  **Grid** (G); then **Undo**, **Redo** and **Save**; at the right **Simulate**, **Drive here** and
  **Test race** (F5). A button that cannot act is greyed out and says why when hovered (the race tools until
  a race exists). On narrower windows the labels get shorter (no key hints, "Drive" for Drive here); nothing
  moves or disappears.
- **Tools**: with **Select** a click selects a prop or a race point and never adds anything. **Move** and
  **Rotate** drag the selected props' handles. A race tool is armed by its button or key and adds to the
  selected race on every click on the road until **Esc** (back to Select) or another tool. **Race ▸ New race**
  arms Start, and the start line placed arms Checkpoint. Picking a prop disarms a race tool.
- **Left: the tree.** On top the **Mod card** (name, author, how many races and prop edits; a click opens
  **Mod details**), then **Races** (each race with its start grid, start line, checkpoints and finish line, an
  error badge when it cannot be raced and a **tested** badge when it finished a test as it is now, and **New
  race…**), then **Props** (the moved, deleted and added props; each with **Reset to original** and **Delete**).
  A click on a row selects it. Below 1400 points of window width the tree starts folded to a narrow rail that
  names its three parts; a click on the rail opens it, the button at the tree's top folds it again.
- **Centre: the view.** The 3D view, drawn by the game's own renderer, or the 2D road-network **Map** (M). The
  selection and its handles are drawn on top, and in both views the races: the selected race bright with its
  start and finish lines, grid and checkpoints, the others' routes dim. The buttons at the view's top right pick
  the look: **Auto** (street near the ground, whole map from above), **Street** (what the game draws on the
  street, with fog), **Whole map** (every section of the region, no fog), **Top** (the top-down overview, T) and
  **Haze**; and the light: **Work light** (the default) brightens the dark parts of the picture for editing, so
  the region at its night time stays readable from above; **Game look** shows the picture exactly as the game
  draws it. The work light is a pass of the editor alone, between the finished picture and the window: it
  measures the picture's average brightness, raises the darks by up to 32 times along a curve that keeps white
  white and each colour's hue, and lifts black slightly. The game, its renderer and its options are not changed.
- **Hint line** (under the view): the tool in force, what a click does with it, **Esc** back to Select while a
  race tool is armed, and how to fly. **hide** at its right; **View ▸ Hint line** shows it again.
- **Right: Properties** of the selection (below). Its button folds it to a rail.
- **Bottom tabs**: **Prop browser**, **Validation**, **Simulate** and **Log**. The button at the right of the
  tabs folds them to the tab strip. The side and bottom panels are resized by dragging their edge; their sizes
  are kept with the mod.
- **Menu bar**: the menus below, and the mod's name and region at the right with a dot while there are
  unsaved changes.
- **Status bar**: saved or unsaved (and why the last save failed, if it did), the last message, the camera, the warning and
  error counts, and a test run's state and result.

## Menus

A menu item that cannot act now is greyed out and says why when hovered (Duplicate with no prop selected,
Reset camera in the map view). Every item does something; a test checks that each one has a taker.

| Menu | Items |
|---|---|
| File | **File ▸ New map mod…** (Ctrl+N: a name, its folder and the region), **File ▸ Open map mod…** (Ctrl+O), the five most recent mods, **File ▸ Save** (Ctrl+S), **File ▸ Mod details…** (name, author, description), **File ▸ Import…** (a mod folder, a `map.obmap` or a `.zip`), **File ▸ Export…** (a `.zip` or a folder: the map and the mod's textures, `looks.toml` and `textures/`, never `work/` or `.editor/`; textures from the original game's pixels are warned about first; it proposes `<folder>.zip` in the `exports` folder of Overbright's data folder, made on the first export, and the status line says where the file went), **File ▸ Export as mod pack…** (a `.obpk` file for the launcher's Mods page: the placements, races and textures as the asset format's own files; it proposes `<folder>-1.0.0.obpk` in the `exports` folder), **File ▸ Quit** (Ctrl+Q; saves first, and asks only when that fails) |
| Edit | **Edit ▸ Undo** and **Edit ▸ Redo**, named by the step they undo or redo (256 steps), **Edit ▸ Duplicate**, **Edit ▸ Delete**, **Edit ▸ Select all of this solid**, **Edit ▸ Drop to ground**, **Edit ▸ Rotate 90°**, **Edit ▸ Face camera**, **Edit ▸ Draw wall**, **Edit ▸ Draw ground** (see Removed and drawn collision) |
| View | **View ▸ 3D view**, **View ▸ Map** (M switches), **View ▸ Frame selection** (F), **View ▸ Reset camera** (the region's overview), **View ▸ Collision ▸ Off**, **View ▸ Collision ▸ Invisible barriers** and **View ▸ Collision ▸ All collision** (see Collision), **View ▸ Hint line** |
| Race | **Race ▸ New race**, **Race ▸ Test race** (F5), **Race ▸ Drive here** (Shift+F5), **Race ▸ Restart race** (Ctrl+F5) |
| Help | **Help ▸ Controls**: every key, made from the editor's key table |

## Controls

**Help ▸ Controls** lists the same keys as this table; both are made from the editor's key table (a test checks
that this table matches it). Tab is not the editor's: it moves the keyboard focus between fields. Single keys
do nothing while a text field has the keyboard.

| Keys | Does |
|---|---|
| `Q` | Select tool: a click selects a prop or a race point, never adds |
| `W` | Move tool: drag an arrow to move the selected props |
| `E` | Rotate tool: drag the ring to turn the selected props (Alt: turn freely) |
| `C` | Checkpoint tool: a click on the road adds a checkpoint |
| `S` | Start tool: a click on the road puts the start line |
| `X` | Finish tool: a click on the road puts the finish line |
| `G` | Grid tool: a click on the road adds a grid slot |
| `Esc` | Back to Select (a race tool armed); else clear the selection |
| `RMB + W A S D` | Fly |
| `RMB + Q / E  (F / R)` | Fly down / up |
| `RMB + wheel` | Fly speed |
| `Shift / Ctrl while flying` | Fast / slow |
| `Wheel` | Dolly; in the overview and the map: zoom |
| `F` | Frame the selection |
| `T` | Top-down overview on / off |
| `Home` | Fit the overview or the map to the region |
| `M` | 3D view / Map |
| `L` | Collision layer: off / invisible barriers / all collision |
| `LMB / Ctrl+LMB` | Select / add to the selection |
| `Shift + drag an arrow` | Copy the selection and move the copy |
| `Alt while dragging or placing` | No snapping |
| `Shift + click on the route` | Insert a checkpoint there |
| `[ / ]` | Previous / next checkpoint |
| `Ctrl+D` | Duplicate |
| `Del` | Delete |
| `End` | Drop to the ground |
| `RMB on a prop / Shift+F10` | The prop menu: Rotate 90°, Rotate…, Face camera |
| `Ctrl+Z, Ctrl+Y (Ctrl+Shift+Z)` | Undo, redo |
| `Ctrl+N, Ctrl+O` | New map mod, open a map mod |
| `Ctrl+S` | Save |
| `Ctrl+Q` | Quit |
| `Enter` | Confirm a dialog |
| `F5 / Shift+F5` | Test race / Drive here |
| `Ctrl+F5` | Restart race (the open test game) |
| `Space` | Play / pause the simulation (its timeline under the view) |
| `Left / Right, Shift+Left / Right` | The simulation a recorded frame back / on, a second with Shift |
| `Up / Down` | Show the line above / below in What went wrong (Simulate tab) |

While the right mouse button is held the tool keys fly the camera instead (W A S D, Q / E).

## Props

- **Select** a prop with a click in the view or in the tree; Ctrl+click adds to the selection. **Edit ▸ Select
  all of this solid** selects every placement of the same model.
- **Move** shows an arrow per axis, **Rotate** (E) a ring around the vertical. The handles keep their size on
  screen, in the 3D view and in Top. Moves snap to 0.25 m and turns to 15° while **Snap** is on (right-click it
  for other steps; Alt held: no snapping, the ring turns freely). Shift while dragging an arrow moves a copy. A
  drag moves the world as you drag and is one undo step when released. With a prop selected the hint line names
  the Rotate key and the prop menu.
- **The prop menu**: right-click a prop in the view (a click, not a flight: the camera must not move between
  press and release), or press Shift+F10 with props selected. It selects the prop when it is not selected yet and
  offers **Rotate 90°** (a quarter turn to the left, seen from above), **Rotate…** (turn by a typed angle, or by
  −90°, 180°, −15°, +15°), **Face camera**, **Drop to ground**, **Duplicate** and **Delete**, each one undo step.
  Several props turn about their centre. **Edit ▸ Rotate 90°** and **Edit ▸ Face camera** do the same.
- **A prop's front** is its local −y side: the side the game's signs show their face on (their readable side
  looks along their local −y axis). A small arrow marked "front" at the foot of each selected prop shows it;
  **Face camera** turns the front towards the camera, and a prop dropped from the browser lands with its front
  towards the camera. The **Heading** in Properties is where the local x axis points, so the front looks 90°
  clockwise of it (a heading of 90° faces the front along +x).
- **Prop browser** (bottom tab): the region's placeable models as tiles with a picture of each, a search, the
  filters **Knock-over**, **Barriers**, **Signs**, **Street furniture**, **Trees** and **All in region**, and how
  many of each are placed. A search looks in every filter: it switches to **All in region** (clearing it brings
  the filter back) and each filter's button counts its matches. A long name is cut on its tile; the tooltip and
  the line under the tiles name it in full with its filters, kind and count. The tiles are buttons: Tab reaches
  them and Enter chooses one. Drag a tile into the view (it lands on the ground under the pointer, its front
  towards the camera), or choose one and press Ctrl+D with nothing selected to place it at the pointer.
- **The browser's pictures** are drawn by the game's own renderer from your game files, the first time a region's
  browser shows (a dozen a frame; the tile shows a symbol until its picture is ready and the line under the tiles
  says how many are still being drawn), each seen from its front, a little to the side and above. They are kept
  in the asset cache (`prop-thumbs/` in the cache folder, about 10 KB each, named by a hash of the model and its
  textures) and read from there after that; nothing of the game's art ships with the editor. Deleting the folder
  only makes them be drawn again.
- **Properties**: the model's name and where it comes from (an original placement, added by this mod, or added
  by another mod, which this mod cannot change), the position (typed values are one step on Enter), the heading
  and tilt with **Upright** (stand it straight, heading kept) and the ground under it, **Reset to original**,
  **Duplicate**, **Delete**, and what happens to its collision (see Limits). With several props selected, moves
  and turns apply to all, around their centre.

## Races

- **Race ▸ New race** (or **New race…** in the tree) makes a sprint on the selected race's base track, else on the race
  track whose roads come nearest to where the camera looks, and arms Start: click the road for the start line, then for the checkpoints (the Checkpoint tool is armed for you),
  finish last for a sprint. The race tools work the same in the 3D view, in **Top** and on the **Map**: a click
  places the point, a drag moves it, each one undo step. Points snap to the base track's nearest road while Snap
  is on (Alt: no snapping); in Top and on the Map a point takes its height from the road under it, never from
  the camera. The selected race's base-track roads are drawn faint green over the 3D view (near the camera), in
  Top and on the map: a race may only use those roads. On the Map, a drag that does not start on a race point
  pans as before.
- With **Select**, a click on a race point selects it, a drag moves it, Del removes it, `[` / `]` step through the
  checkpoints. With Checkpoint armed, Shift+click on the route inserts a checkpoint into that leg. Grid adds a
  slot (8 at most; with a slot selected, a click moves it).
- **Properties** of a race: name, mode (circuit, sprint, street X, drift), laps, **Base track** (which race track
  lends the road network, barriers, AI lanes, traffic and opponents), **Direction** (forward or backward),
  length, status, **Delete race**; of a race point its position.
- **Validation** (bottom tab): every problem of every race, each with **Show** and, where one exists, a
  one-click fix (**Try base track N**, **Snap grid to road**, **Snap start to road**, **Snap finish to route**,
  **Turn start along route**, **Set laps to N**, **Rename to …**). A start line facing more than 45° off the
  route's direction there is a warning (the cars would start across the road or against the race). A race with
  an error (a leg with no road, fewer than 2 checkpoints, a start line or grid slot with no floor under it) cannot
  be tested or simulated. Warnings from the last simulation are listed here too.
- **Heights of race points**: a start or finish line or a grid slot takes its height from the road when it is
  placed, moved or written by any program: the highest floor at or below it + 1 m, else the highest floor at its
  x/y (one placed under the road comes up onto it), as the game puts its own start-grid markers. Where there is
  no floor at all (off the map) the height stays as given, and Validation reports "has no floor under it" (an
  error for the start and the grid, where the cars would fall through the world; a warning for the finish) or
  "is N m above the floor" (a warning).
- **Checks of drawn and removed collision** (all warnings): a drawn wall that crosses a race's route (Show goes to
  the crossing); a drawn wall standing more than 2 m above the floor, or with no floor, under one end (cars drive
  under it); a removed wall the region's collision does not have (removing it does nothing); a drawn wall or ground
  partly outside the region's collision area (the game leaves that part out). `overbright-studio-cli checks run`
  runs them too, on the region's world with the mod's drawn collision applied, so a race start on drawn ground has
  a floor.

## Saving

Every step (each edit, undo and redo) is written to the mod's history, `.editor/history.journal`, before it
counts, so a crash or a killed editor loses at most the step being made. `map.obmap` itself is saved at quiet
points: 2 seconds after the last step, and at the latest 30 seconds after the first unsaved one while editing goes
on. **Save** (Ctrl+S) saves at once. Each save writes `map.obmap` through a temporary file and a rename, so a
failed write leaves the old file in place. Test race, Drive here and Simulate play a copy (below).

When you open a mod whose history holds steps `map.obmap` does not have yet (the editor stopped before it saved
them), the editor asks **Restore unsaved changes?**: **Restore** keeps them, **Discard them** goes back to the file
as one step that Undo takes back. Undo and redo reach back over earlier sessions: the history keeps the last 256
steps. Quitting or opening another mod saves first; only when that save fails does the editor ask, and the steps
then stay in the history for the next opening.

One program writes a mod at a time. A mod already open in another editor (or another program of Overbright
Studio) opens read-only, with a strip over the view saying which program has it; edits are not kept, the view
follows the other program's steps, and **Try again** opens the mod for writing once the other program has let go.
When another program writes `map.obmap` while the mod is open, its change becomes one step that Undo takes back.

An older editor wrote unsaved changes to `.editor/map.obmap.autosave`. When that file is newer than the mod,
opening the mod offers it once through the same prompt; the editor removes it after the answer and never writes
it again.

The editor's own files live in the mod's `.editor/` folder. They are never exported:

| File | Holds |
|---|---|
| `editor.toml` | the view: region, camera, the widths of the tree and the properties and the height of the bottom tabs (written on save, when another mod is opened and when the window closes) |
| `history.journal` | every step, with its author and the last saves (the history) |
| `saved-by.json` | the build that last wrote the mod's files and each file's hash then, written on every save, so a later build can repair what an older one wrote wrong (checked by `every_save_records_the_build_that_wrote_it`) |
| `studio.lock`, `studio-owner.toml` | the write lock while a program has the mod open, and which program it is (removed when it closes the mod; one left by a program that died is ignored) |
| `inbox/` | changes other programs send to the program that has the mod open |
| `map.obmap.autosave` | an older editor's unsaved changes (offered once, then removed) |
| `tested.txt` | the tested marks |
| `test-result.txt` | the last test's result line |
| `run/map.obmap` | the run copy the game plays for Test race, Drive here and Simulate |

An older editor kept these files beside `map.obmap`; opening the mod moves them into `.editor/` once (a file
already in `.editor/` stays, and the old one is then left where it was).

## Testing a race

There are three ways to try a mod. They differ in who drives and where it shows:

| Button | Who drives | What it does |
|---|---|---|
| **Simulate** | the AI | The game races the selected race with the AI in every car, without a window. You watch it in the **Simulate** tab and see where cars got stuck. With no race selected, it shows the traffic around the camera. |
| **Test race** (F5) | you | You race the selected race in the game window. Finishing it marks the race tested. |
| **Drive here** (Shift+F5) | you | You drive freely in the game window, with this mod loaded, from the ground the camera looks at. |

Each button says this when hovered, and the **Simulate** tab says it at its top.
**Help ▸ How to test your race** puts it on one page with a good order: Simulate to find the spots where the AI
has trouble, Test race to drive it yourself, Drive here to look around. While Test race or Drive here has the game window open, a
strip over the view says so; `Esc` in the game ends the run, and you can keep editing.

**Test race** (F5) writes the mod as it is in the editor, unsaved changes included, to its run copy
`<mod folder>/.editor/run/map.obmap` (through a temporary file, so the game never reads half a file) and starts
the game next to the editor on that copy:

```
game --map-file <mod folder>/.editor/run/map.obmap --map-race <id> --map-result <mod folder>/.editor/test-result.txt
```

The test game stays open and takes your edits. Studio starts it with `--studio-link <mod folder>/.editor/link`: two
small files in that folder (`command.txt` from Studio, `game.txt` from the game), each written through a temporary file;
nothing listens on a network, and it is the same on Windows and Linux. Without the flag the game never looks at the
folder. About ten times a second, between two frames (never inside a simulation step, so play is the same at any
frame rate), the game reads `command.txt`; a new command reads the mod's run copy again and starts the run over with it
(race points, props), or only starts over (**Race ▸ Restart race**, Ctrl+F5). A moved or added prop shows after that
restart, not yet while you drag; the game loads the region again, behind its loading screen. F5 or Shift+F5 while the game is
open send the new race or spot instead of refusing. Free roam goes on from where the car is after an edit. A run that
cannot start (the race was deleted, no road at the spot) stays as it was and the status bar says why. The game writes its own state about twice
a second; the status bar shows "Game connected", "Game applying changes" or "Game not connected".

Test race does not save `map.obmap` itself; the status bar adds "(testing unsaved changes)" while the mod
has steps not yet saved. `--map-file <path>` reads the mod from that file; in the run the mod is known by
its folder's name (for a run copy, the mod's folder; for any other file, the folder holding it, or `map-file`
when that name is not a folder name). `--map-mod <folder>` takes the saved mod from the maps folder instead. With
these flags the game:

- applies only that mod for this run (whatever `enabled.txt` lists; nothing is written to it);
- skips the boot screens and menus and loads straight onto the race's start grid. The race uses its base
  track's career event of the same mode for its set-up (opponents, traffic) and runs outside the career: no
  progress and no reward;
- drives the car of the career save that is loaded (with its parts), or the Quick Race car select's first car
  when there is no save. The save is then let go, so nothing in the run writes it;
- ends after the results have been shown for 8 seconds (or at once on Enter), on Esc (the pause menu), or when
  the window is closed, and writes one line to the result file: `finished=yes place=1 of=4 time=83.412 laps=2`,
  or `finished=no laps=2`.

The editor keeps working while the game runs; only a second test waits. When the game has ended, the status bar
shows the result, and a finished race gets its **tested** badge until it changes. The mark is the race's content
(its base track, direction, mode, laps, checkpoints, grid, start and finish, not its name), so it holds when the
tested race is saved unchanged and goes with any change that makes it drive differently. A race the game cannot
start (an unknown id, no route, no career event of its mode on its base track) makes the game exit with the
reason in its log and no result line, which the editor reports.

**Drive here** (Shift+F5) does the same with `--map-at x,y,z,heading` (heading in degrees around +z from +x) in
place of `--map-race`: free roam with only this mod, from the camera's spot on the ground. Esc ends the run
(`drove=yes`). A spot with no floor of the game's collision under it at any height (past a road's edge) ends the
start with "no ground at x, y" in the log, as a race that cannot be raced does; `--simulate-world` ends its stream
with that reason, which the Simulate tab shows.

None of these flags change anything when they are not given. A flag without its value, a malformed spot,
`--map-race` together with `--map-at`, `--map-mod` together with `--map-file`, or either without `--map-race` or
`--map-at` is a usage error (exit code 2). A `--map-file` the game cannot read as a map mod ends the start with
the reason and exit code 2.

## Simulating a race

**Simulate** (the toolbar: the selected race while a race or race point is selected, else the world around the
camera; or **Simulate race** / **Simulate world** in the **Simulate** tab) writes the run copy as Test race does
and runs the game next to the editor with no window and no sound, the AI driving every car:

```
game --map-file <run copy> --map-race <id> --simulate --stream - --sim-seed <n>
game --map-file <run copy> --map-at x,y,z,heading --simulate-world --stream - --sim-seed <n>
```

(`--map-mod <folder>` in place of `--map-file` simulates the saved mod.)

- `--simulate` races the custom race as Test race would set it up, but the player's slot is driven from the
  grid by the AI racer controller. `--simulate-world` runs free roam from the spot (traffic, street racers); the
  player's car stays parked.
- No window, no graphics device and no audio device are opened; nothing is saved.
- Each update (the 60 Hz tick) is written as one line to `--stream <file>` (`-` or no flag: standard output),
  with the race's events (below). The same seed gives the same stream, byte for byte.
- The run ends at the race's end (every racer through), after `--sim-seconds` (default 900 for a race, 120 in
  free roam), or when standard input reads `quit` (the editor's **Stop**).
- `--stream`, `--sim-seed` and `--sim-seconds` without `--simulate` / `--simulate-world`, `--simulate` without
  `--map-race`, or `--simulate-world` without `--map-at` are usage errors.

The editor reads the stream while it arrives and never waits for the game. A run shown has a **timeline** under
the 3D view / Map (it reads only the recorded run, so it serves a saved one too): a track over everything recorded
so far that a click or drag moves along (always to a recorded frame), **<** / **>** a recorded frame back / on
(Left / Right; Shift: a second), **Play** / **Pause** (Space), the speed (0.25x to 4x), the time, and the camera:
**Free camera** (fly it yourself), **Follow** (behind and above the car chosen beside it) or **Chase** (the game's
own chase camera, its Far view, riding with that car: the car's camera settings from the game files, the car's
moves from the recording; it does not push off walls as in the game, and without game data it follows). The time
shown after the same real time is the same at any display rate, and a camera's view at a time does not depend on
how the playback got there. On the track a marker stands at each thing **What went wrong** found in the run
shown (a colour per kind, named at the right of the row: stuck and start trouble amber, crashes red, wrong way
blue, missed checkpoints light, off the roads grey); hovering one names it, a click shows it as its What went
wrong line does. Over the view's top right the **Positions** board lists each racer's place at the time shown,
its car and its lap or finish time; a click follows that car. The Simulate tab keeps **Lines** (each racer's
driven line). In the 3D view the cars are the game's own cars: each car's
model with its paint, body parts, vinyls, rims and lights as the run built it (the stream's look records, below),
drawn by the game's car renderer with the world's lighting, its wheels steering, rolling and riding the
suspension, every car's headlight pool on the road, and its head and brake light flares as the game's glow pass
builds them (the brake lights lit while the run had them on); they move smoothly between the game's 60 Hz updates at
any playback speed. Traffic is drawn the same way. A car shows as a coloured box while its model loads (on its
own thread; the view keeps drawing) or when it cannot be built (no game data, a stream written before the looks);
**Car models** (on by default, before the colour key) switches every car to boxes. On the map the cars are
arrows (the player's slot and the followed car lime, opponents light, traffic grey, street racers amber), with
each racer's place; a key in the Simulate tab names the colours. Below are the results (place, car, time, best lap,
laps done) in finishing order: finished racers by
time, then the rest by their place on the road (no places before the green light); a racer without a finish when
the run ended is last, as "did not finish"; a racer still on the road shows "racing". The player's slot, which
the AI drives here, is "Your slot (AI)", marked "AI-driven, not your time", with a note over the table: its place
and time are the AI's, and Test race gives your own. The tab says how the run ended (every racer through, the
time limit, or stopped) and how many problems it added to Validation, with **Show Validation**. The same seed
runs the same race again; another seed changes the AI's and the traffic's choices.
The Simulate tab shows one of three lists (the row above it switches): **Results** (above), **What
went wrong** and **Events** (green light, laps, finishes, crashes, stuck, wrong way, each with **Show**). A racer
that stood still for 4 s after the green, drove against the race's direction for 2 s, or lost 6 m/s or more in
one tick becomes a warning in the Validation tab, named by the checkpoint it was heading for: "Simulation: AI got
stuck before checkpoint 3".

**Runs** (next to **Seed**, 1 to 10) simulates the race that many times, one seed after the other (Seed, Seed + 1,
…); the tab says which run is going. **What went wrong** opens by itself when the runs end with a problem. It
reads only what the runs recorded (no second simulation) and lists one line per problem and place, worst first,
with each time it happened under it:

- **Cars in trouble at the start**: stuck or crashed within 6 s of the green and 40 m of their grid spot, the
  sign of a start line or grid facing the wrong way;
- **Stuck** before a checkpoint, with how long the car stood (or that it never moved again);
- **Crashed into prop** *solid (id)*: a hard hit within 6 m of one of the mod's added or moved props;
  **into another car** (within 5.5 m, named); otherwise **into a wall or the scenery**;
- **Drove the wrong way** before a checkpoint;
- **Missed checkpoint** *n*: a racer passed over it in one jump, or did not finish and never reached it;
- **Left the base track's roads**: a racer more than 25 m from the base track's roads (as Validation measures a
  checkpoint) for 2 s or more, at its farthest spot;
- **Finish spread** (information): first to last finish, and who did not finish.

With one run a line counts the cars ("Stuck before checkpoint 3: 2 cars"); with several it counts the runs
("Stuck before checkpoint 3: 3 of 5 runs, 4 times") and each occurrence names its seed. A click on a line, or on
Enter, Tab or Up / Down onto it, shows it: that run's replay (another seed's run is swapped in), paused at the
time, following the car, the 3D camera and the Map on the spot. Space on a focused line does not start the
playback. Eight occurrences per line are listed, the rest counted.

For pictures of these screens without a window, `overbright-editor --shot` also takes `--hover
simulate|drive-here|test-race` (the button's tooltip), `--dialog how-to-test|controls` (a help page) and `--run
test-race|drive-here` (starts the run in the first frame; only with `OB_GAME_EXE` naming a stand-in game, so a
shot never opens the real one).

### Simulation stream

The asset format's run file, `ob.run` (docs/modding/asset-format/runs.md, which has every field): plain text, one
record per line; readers skip records they do not know.

| Record | Fields |
|---|---|
| `OBSIM 1 schema=ob.run/1.0 mode=race seed=7 race=<id> track=<n> laps=<n> checkpoints=<n>` | the header (version 1; `schema=` names the run file's minor version) |
| `C <id> <P\|R\|T\|X> <name>` | a car's first appearance: player slot, opponent, traffic, street racer |
| `L <id> <model> <camber> <slot:hash,…\|->` | a car's look: its model (`CARS/<model>`), the front wheels' x-turn (radians) and its fitted visual parts (slot and part hash in hex, the performance packages' visual parts included); before the car's first update and again when it changes (a traffic slot taking another car) |
| `F <tick> <time> <car>\|<car>…` | one update; each car `id kind x y z qx qy qz qw speed lap checkpoint place`, then `steer0 steer1 roll0..3 lift0..3 lights` |
| `E <tick> <time> <kind> <car> <checkpoint> <value>` | `start`, `checkpoint`, `lap` / `finish` (value: time), `crash` (m/s lost), `stuck`, `wrong-way` |
| `H <tick> <hash>` | the state hash over every tick so far (16 hex digits), every 60 ticks and after the last tick |
| `R <car> <place> <time> <best> <lap,lap…>` | a racer's result (`-`: none; place 0: did not finish) |
| `END <finish\|quit\|limit\|error …>` | the end |

Times are seconds on the race clock (negative before the green; the start scene sets it back once) or since the
start in free roam; positions in world metres, the rotation a unit quaternion, speed in m/s. Ids: the player's
slot 0, opponents 1.., traffic 100.., street racers 200... A car's wheels and lights: the front wheels' steer
(radians), each wheel's roll about its axle (radians, 0..2π, table order) and lift (the hub's offset, m), and the
lights (1 headlights, which every car has on in game; 2 brake lights, the racers' flag held 0.5 s as the game
holds it). The state hash covers each car's exact position, rotation, velocity and spin, not the rounded values
written: a second run of the seed, or a replay, that parts from this one shows it at the first `H` that differs.
The same seed still gives the same stream byte for byte, hashes included. A reader skips records and trailing fields
it does not know, so a stream from before the looks and wheels still plays (as boxes), and the version stays 1.
The editor rebuilds each look with the game's own car loader (`Car::load_with` on the model and parts) and draws
its flares with the game's own flare sprites and glow pass. Not yet drawn from the stream: the spinners' own turn.

## The game's free camera

Started with `--editor`, the game has a free camera in free roam (F8). The world is paused under it, W A S D /
R / F fly, the mouse looks around, Z / X change the field of view, H hides the hint strip, G puts the car at the
aim point, and Esc or F8 returns to driving. It does not edit anything. Without `--editor`, F8 does nothing.

## What a custom race is

A custom race runs on a **base track**: one of the game's race tracks in the region. The base track provides the
road network (`TRACKS/ROUTES<region>/Paths<track>.bin`: the roads around that track's own race, not the whole
region, so a custom race stays within its base track's area), its race barriers (the roads closed for it), the
AI racing lanes (the base track's `Routes<track><F|B>.bin`: the racing line of the base track's own race, not of
the custom route; see docs/original/tracing/ai-lanes.md), traffic, the opponents, and the race set-up of a career event on
that track with the same race mode. The mod provides the checkpoints, the start grid (8 slots behind the start
line when none are placed), the start and finish lines, the mode and the laps. The route is built through the
checkpoints on the road network exactly as the game builds a track's own route (`RoadNet::route`, 0x5E5BC0). A
leg with no road through the open network (a closed barrier in the way), a checkpoint away from the base track's
roads, or a leg its roads do not join is reported in Validation; the summary line of a race that builds gives the
size of that area. A circuit closes back to its first checkpoint. A sprint ends at its finish line, or else at its
last checkpoint. Checkpoints taken from a track's own route build that track's road (test
`a_custom_race_on_native_checkpoints_is_the_native_road`).

Custom races are raced through Test race. They do not appear in the career or the Quick Race lists, and their
results do not count toward the career.

## Collision

The Collision submenu of the View menu draws the game's collision over the 3D view, the top overview and the
Map: the original's, with the mod's moved scenery, removed walls and drawn walls and ground (see "Removed and
drawn collision"). It is drawn again whenever an edit, undo or redo changes the collision.

- **View ▸ Collision ▸ Off** (the default) draws nothing.
- **View ▸ Collision ▸ Invisible barriers** draws the walls a player cannot see: in red the invisible walls,
  in amber the race and career barriers.
- **View ▸ Collision ▸ All collision** adds, in grey, the walls that stand behind scenery and, in the 3D view,
  the floors near the camera: road floors in the road colour, floors the wheels do not count as ground in lime.

The L key steps through the three.

A key at the view's bottom right names the colours and counts each kind in the region. In the 3D view a wall is
a see-through panel with its outline; in the top overview and on the Map it is a line along its foot.

What is drawn:

- The faces come from the region's collision packs as the game loads them with the mod
  (docs/modding/asset-format/collision.md); a face stored in several packs is drawn once.
- **Race and career barriers** are the walls of a barrier group other than 0 (`BARRIERS_<n>`,
  `PLAYER_BARRIERS_<n>`, `BARRIERS_CAREER<n>`). The game switches them on per race or per career stage; the
  editor draws them all, whichever are on.
- **Invisible walls** are the walls of group 0 (always there) with no drawn scenery standing at them. The editor
  decides this itself: a wall is cut into 2 m pieces, and a piece counts as hidden by scenery when a steep
  triangle of the original's placements (free roam, no panorama, no race-only pieces) comes within 1 m of it,
  2 m above its foot. A piece with none is drawn as invisible. The game has no such flag; this is the editor's
  reading, so a wall behind a thin or far-off model can show as invisible.
- A wall the game makes reach from −2048 m to 2048 m (flag 0x04) is drawn from the floors beside it to 4 m
  above them.
- In the 3D view walls are drawn up to a few hundred metres from the camera (farther from higher up) and floors
  up to 120 m.

## Removed and drawn collision

With the collision layer shown, a click on a wall in the 3D view selects it (outlined in lime; a click inside a
drawn ground selects that), and the Properties panel names it and its size.

- **Remove wall** (Del) takes the selected wall out of the game's collision: an invisible wall, a wall behind
  scenery, or a drawn wall or ground (named **Delete** for those). It is one undo step ("Remove wall",
  "Remove wall-1"); the scenery standing there stays. Race and career barriers cannot be removed (the game
  switches them per race). Only static collision is touched: knock-over props keep their own.
- **Edit ▸ Draw wall**: two clicks on the ground make a wall between them, of the Properties panel's height
  (0.5 to 50 m, default 2 m), standing on the ground's height at each end. A drawn wall is a closed box 0.5 m
  thick reaching 0.5 m below the ground, so a car meets it from either side at any frame rate. The tool stays
  armed for the next wall; Esc stops it.
- **Edit ▸ Draw ground**: clicks on the ground make an outline (3 to 32 corners, not crossing itself), ended
  with Enter or a click on its first corner: a floor over it, at its corners' heights. Drawn ground goes first
  in its cells and competes by height with the other floors there (flag 0x01), so it carries a car that drives
  onto it without taking the ground from a road above or below it.
- Picking and drawing work in the 3D view, in Top and on the Map alike: in Top and on the Map the point is the
  mouse's x/y at the road's or floor's height there (as for race points), and a click picks a wall by its foot.
  Drawn points snap to existing collision corners and to the ends of your own drawn walls and outlines within
  10 points on the screen; a ring shows the corner a click takes, and Alt places the point where the mouse is.
  Enter, Esc and Del work as in the 3D view.
- Each drawn wall and ground has a surface id (grip and sound): by default that of the floor under its first
  point; the Properties panel sets another.
- With Show off, drawing switches the layer to Invisible barriers so the new wall shows.

In the file these are `[collision.remove]`, `[collision.wall]` and `[collision.ground]` sections (see File
format); the game's load and the editor's world write them as they write moved scenery's walls (below), so Test
race, Drive here and Simulate collide with them as the editor shows. A removed face the region does not have is
counted in the log ("removed collision face(s) … are not in its collision") and does nothing. Without such
sections nothing changes.

## Collision of edited scenery

The game bakes the collision of its static scenery into each stream section's collision pack; nothing in a pack
names the placement a face came from. A map mod's scenery gets the collision the original gives the same solid:

- **Knock-over props** (cones, road barriers, fences, barrels, …) are knock-over props wherever they stand: an
  added copy knocks over like the original (choice: format rule; the original's layout is in docs/formats/), a moved one is hit where it now stands, a deleted one is gone.
- **Static solids** (street lights, walls, …): a solid's collision is the walls standing inside the box of one
  of its original placements, taken into the solid's own space. Walls of a barrier group (race barriers), walls
  reaching ±2048 m (invisible walls) and floors are never taken. Where the solid's placements differ, the shape
  most of them share is used. An added placement gets that shape at its place; a moved original takes its own
  walls to its new place and leaves none behind; a deleted one leaves none.
- New walls are written into every pack whose grid covers them (as the original lists a wall on a section edge
  in each section's pack), after each cell's own faces, with cells and sub-cell masks by the rule the shipped
  packs follow (L4RA: the same faces in 14,547 of 14,550 cells; the rest touch only a cell's edge).
- Without a map mod, or with one that edits only knock-over props and solids without collision, the packs are
  the original's, unchanged.
- The game's log says how many faces the mods took out and wrote ("static collision: … out, … in").
- The editor's world follows each edit, undo and redo at once: a moved solid stops cars and shows in the collision
  layer where it now stands, with the same faces in the same order as the game's load of the mod gives; taking
  the edits back leaves the original's collision, bit for bit. Test race, Drive here and Simulate load the run
  copy and so get the same collision.

Code: `src/world/prop_collision.rs` (the shape, the cells, the editor's re-apply), `TrackGround::edit_pack` in
`src/ground.rs`; tests in `src/world/prop_collision/tests.rs` (a car at 70 m/s into an added, a moved and an
editor-moved street light at 30, 60, 120 and 200 steps a second; the editor's world against a load, step by step)
and `src/bin/game/enh/sim_stream.rs` (a simulated sprint whose racers stop at a row of moved street lights).
Open: the original's own pack build is not traced (the cell rule is read from the shipped packs).

## Limits

- Static scenery takes its collision with it (see "Collision of edited scenery"), but only its walls: a solid's
  floors (a ramp, a raised platform) stay where the original has them, and a solid the original gives no
  collision (benches, most signs) has none when added.
- Overlays (route, checkpoints, handles, collision) are drawn over the world without depth.
- Drawn ground is flat between its corners.
- Shops, event markers, traffic lanes and AI lanes cannot be edited yet. Drag races are not offered (their lanes
  are fixed to the drag tracks).
- Copy and paste in text fields work only inside the editor (no system clipboard); screen readers are not
  supported yet.
- Tested on Linux; on Windows only under Wine.

## Where map mods live

| | |
|---|---|
| Mods | `<Overbright data>/maps/<folder>/map.obmap` |
| Mods in use | `<Overbright data>/maps/enabled.txt`, one folder name a line, applied in that order |
| Editor files | `<Overbright data>/maps/<folder>/.editor/` (see Saving; never exported) |
| Recent mods | `editor-recent.txt` in Overbright's settings folder |

`<Overbright data>` is the folder from the README's "Where your files go" (`data/` in portable mode). If a mod
cannot be read, it is skipped with the reason in the log, and the others still apply. The game reads the list and
the mods at the first world load of a run. An enabled mod's painted textures (`<folder>/textures/world/<HASH>.png`,
`…/car/…`, written by [Studio Looks](studio-looks.md)) load with it, in the list's order (the last mod the
strongest), before every texture pack. To share a mod, use **File ▸ Export…** or the launcher's Export… (a
`.zip` with the map, `looks.toml`, `textures/` and `provenance.toml`; every export shares the same copy,
`tools::map::share`). To import one, use **File ▸ Import…**, the launcher's Import a map mod… ("Add a mod…" takes a map mod
too), or copy the folder into `maps/` and add its name to `enabled.txt`. An import is validated before anything is
written and never goes over an existing folder.

## File format (`map.obmap`, version 1)

UTF-8 text, at most 8 MiB. The first line that is not blank or a `#` comment is `overbright-map 1`. Then
`key = value` lines: first the header keys, then sections, each starting with a `[name]` line. Values run to
the end of the line; numbers are decimal (written in their shortest exact form, so a file read and written
again is the same); angles are degrees around +z from +x; coordinates are the game's world coordinates (Z up),
within ±100 000.

Header: `name`, `author`, `description` (optional, at most 200 characters each).

| Section | Keys |
|---|---|
| `[prop.move]` | `region`, `section`, `solid`, `at`, `nth` — the original placement; `position` (x y z), `rotation` (9 numbers: the local x, y, z axes as rows) |
| `[prop.delete]` | `region`, `section`, `solid`, `at`, `nth` |
| `[prop.add]` | `id`, `region`, `solid`, `position`, `rotation` |
| `[collision.remove]` | `region`, `corner` (x y z, 3 or 4 times: an original face's corners as its pack stores them) |
| `[collision.wall]` | `id` (a-z 0-9 - _, unique among the drawn walls and ground), `region`, `from`, `to` (x y z: the wall's ends on the ground, 0.5 to 500 m apart), `height` (0.5 to 50 m), `surface` (0 to 255) |
| `[collision.ground]` | `id`, `region`, `point` (x y z, 3 to 32 times, an outline that does not cross itself, at most 500 m across), `surface` |
| `[race]` | `id` (a-z 0-9 - _, unique), `name`, `track` (base track number), `backward` (yes/no: the base track's backward barriers and lanes), `mode` (circuit, sprint, streetx, drift), `laps` (1–20; 1 for sprint and drift), `checkpoint` (x y, repeated, in order, at most 64), `grid` (x y z heading, repeated, at most 8, pole first), `start` / `finish` (x y z heading) |

Every heading is in degrees around +z from +x and is the way the cars face: a grid slot's car, the cars behind
a start line, the cars crossing a finish line. The game turns a line's heading into its own line markers'
convention (a quarter turn less) when it races the mod.

Overbright 0.5.0 placed a custom race's start scene a quarter turn off the start line's heading, so the cars
stood across the road. A race whose start was turned by hand to make up for it now starts across the road
again; its Validation (and `overbright-studio-cli checks run`) says the start line faces about 90° right of the
route's direction, and **Turn start along route** puts it back (in the file: the start's heading plus 90).

An original placement is named by where the original data has it: its region (`L4RA`), streaming section, solid
name and position; the placement matched is the nearest of that section and solid within 5 cm. Where the original
has several placements of one section and solid at exactly the same place, `nth` says which (0 for the first in
the original's order, the default; written only when larger). An added placement's `id` (a-z 0-9 - _, unique
among the mod's adds) names it within the mod; it defaults to `a<n>` for the mod's n-th add and is written only
when it differs, so files without these keys read and write byte for byte as before. Each original placement may
be changed by one edit per mod. Rotation rows must be unit axes (a mirrored placement, as the race barriers have,
is allowed); added placements take their lower LODs, view flags and section from the nearest placement of the
same solid (or the nearest placement for the section).

A file is refused as a whole, with its line, when the first line is wrong, its version is newer than the build
reads, a section or key is unknown, a key is missing or given twice, a number does not parse or is out of
range, a rotation is not one, a race id or an add id repeats, or a placement is changed twice. A race with
fewer than 2 checkpoints is kept (a race being made) but cannot be raced. An edit naming a placement or solid
the region does not have is skipped and counted in the log (the tree marks it: "this edit does nothing").

## Shots without a window

`overbright-editor --shot out.png` draws the whole window off-screen into a PNG and exits; no window opens. It
takes `--size WxH` (default 1600x900), `--scale F` (the display scale), `--mod`, `--region`, `--maps`, `--mode
props|races` (which side of the selection is in front), `--tool <name>` (a toolbar tool), `--view
3d|map|street|whole|top`, `--light work|game`, `--tab browser|validation|simulate|log`, `--select N,M`
(placements by number; the 3D view is framed on them) and `--sim <stream file>[,<stream file>…][@seconds[:car[:chase]]]` (a saved
simulation at that time, following that car, with `:chase` on the game's chase camera; several files are several runs, What went wrong over all of them, the
last one shown), `--collision off|invisible|all` (the collision layer) and `--camera x,y,z[,yaw,pitch]`
(the 3D camera, metres and degrees). A shot runs on a fixed clock, so the same arguments draw the same
picture. scripts/checks/editor_shots.sh runs it at every size of the resolution matrix (docs/developing/checks.md).

## How it works (for developers)

- `crates/ob-map/` (the map data, below the game library, which re-exports it at `tools::map::…`): the format
  (`format.rs`), the folders, archives and the editor's files' places (`store.rs`: `editor_dir`, `run_file`, the
  run copy's atomic write, `move_old_sidecars`), the custom race's route (`race.rs`, on the original's race tracks
  and routes, `frontend/career/`), the edits as data (`ops.rs`) and the simulation stream (`sim/`). No world,
  window or GPU.
- The collision layer: `crates/ob-map/src/tools/map/collision.rs` sorts the faces into walls and floors, tests
  each wall piece against the scenery and keeps a 64 m index for drawing; `crates/ob-studio-gui/src/editor/collision.rs`
  builds it the first time it is shown for a region and draws it, with a reference image test
  (`crates/ob-studio-gui/tests/reference/collision-layer.png`, `OB_BLESS=1` writes it again).
- The simulation's cars: the game writes each car's look and wheels into the stream
  (`src/bin/game/enh/sim_stream.rs`; a car keeps the visual parts it was built with, `Car::look_parts`);
  `crates/ob-studio-gui/src/editor/sim/record.rs` keeps the looks by time and blends each car between the two
  updates around the time shown (`Recording::cars_at`), and `editor/sim/cars.rs` builds each look once with the
  game's car loader (on up to four threads; a shot waits for them), gives each car its own car renderer and
  draws them right after the world, through `nfsu2::studio_api::{car, render::car}`.
- `src/tools/map/` (the game library): the game data the editor reads (`assets.rs`: `MapAssets`, the race tracks
  and their road networks), and the edits on a loaded world (`edit.rs`). `tools::map::set_only` makes one mod the only one
  for a run: `Only::Folder` (`--map-mod`) or `Only::File` (`--map-file`). `sim/` holds the stream format and the
  problem detection, which the game writes the stream with and the editor reads it with.
- Edits as data: `ops.rs` (in `crates/ob-map/`) has `Op`, one edit of a mod (move, delete, reset, add, duplicate or drop a prop; make,
  change or delete a race and its points; the mod's details; a `Group` of ops under one name), with props named
  by `PropKey` (`Original` with the mod file's region, section, solid, place and `nth`, or `Added` with the add's
  id), races by id, never by an index in a loaded world. Ops read and write JSON (one per line in an
  `ops.jsonl`; vectors as arrays, rotations as their three rows) and round-trip exactly. `crates/ob-studio/src/session.rs` (Studio without a window) has
  `MapSession` (the mod, the original and edited world, the undo history, no window): `run(op)` makes an op as
  one undo step named by `Op::label` (the names Edit ▸ Undo shows), and a refused op or group changes nothing.
  An edit re-applies only the placements it changes. The shapes of `Op`, `PropKey`, `RacePoint` and `Change`,
  their JSON and the step names are fixed; new ops and refusal reasons may be added. `Op` is matched in full by the
  session and its tests, so a new op is handled there at once.
- `src/world/map_overlay.rs`: applies the edits to a loaded world. The game loads worlds through
  `tools::map::load_world` (`World::load`, then the mods for the region).
- `crates/ob-studio-gui/`: the desktop editor, a crate of its own on the game library, so the game's dependency tree
  has no egui. `src/editor/` there is its library: `app.rs` (the window's panels, dialogs, `--shot`), `menu.rs`
  (menus, toolbar, the key table `KEYS`, `Command::ALL`), `tree*.rs`, `props.rs`, `browser.rs`, `thumbs.rs` (the
  browser's pictures), `races.rs`, `validate.rs`, `viewport.rs` and `worklight.rs`, `mapview.rs`, `sim/`, and `testplay.rs` (the test hand-off:
  it writes the run copy, starts the game — `game` beside the editor, `OB_GAME_EXE` names another — polls it,
  reads the result line and keeps the tested marks). `src/bin/overbright-editor.rs` is the window. Its tests run
  with the workspace's (`cargo test -p ob-studio-gui` alone); `docs_tests.rs` checks this page against the key
  table and the menus.
- The game: `src/bin/game/shell/cli.rs` (`MapTest`: the flags, the end watch, the result line),
  `flow/race_setup.rs` (`start_map_test`, `map_race_event`), `Game::map_race_route`, which `drive_to_grid` asks
  before reading a track's own route, and `enh/sim_stream.rs` (the simulation runs).
- The launcher: the Mods page's Map mods section (`launcher/screens.rs`), `Env::start_editor` and
  `dirs::editor_exe` (docs/launcher.md).
- Tests: `src/tools/map/tests.rs` (round trip, refusals, store, the run copy and `.editor/`, world and route with
  the game's files), `crates/ob-studio/src/session_tests.rs` (every op through JSON, step names, all-or-nothing groups,
  replay), `crates/ob-studio/tests/map_overlay.rs` (the live re-apply after every op is a full apply), `crates/ob-studio-gui/src/editor/` (every menu command taken, every key in the key table, the toolbar at
  every width, the hand-off with a stand-in game: unsaved changes tested from the run copy, `map.obmap` untouched,
  the tested mark), `shell/cli.rs` (the flags and the run's end), `flow/race_setup.rs` (a test run on a custom
  race and from a spot, with the game's files), and `launcher/tests.rs`.
