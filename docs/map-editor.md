# Map editor and map mods

> **Preview.** The map editor ships as a preview: it works, but not every part is finished and its
> files may still change between versions. Keep a copy of map mods you care about.

The map editor is a desktop program, `overbright-editor`, next to the game and the launcher. With it you move,
delete and add props in the city and make races of your own on the game's road network. Everything it saves is
a **map mod** in Overbright's own data folder, which is applied on top of the original data when a world loads.
The game's files and your career save are never written.

If no map mod is enabled, none of this runs: worlds and races are the original's.

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
- **Status bar**: saved or unsaved (and the last autosave), the last message, the camera, the warning and
  error counts, and a test run's state and result.

## Menus

A menu item that cannot act now is greyed out and says why when hovered (Duplicate with no prop selected,
Reset camera in the map view). Every item does something; a test checks that each one has a taker.

| Menu | Items |
|---|---|
| File | **File ▸ New map mod…** (Ctrl+N: a name, its folder and the region), **File ▸ Open map mod…** (Ctrl+O), the five most recent mods, **File ▸ Save** (Ctrl+S), **File ▸ Mod details…** (name, author, description), **File ▸ Import…** (a mod folder, a `map.obmap` or a `.zip`), **File ▸ Export…** (a `.zip` or a folder; `map.obmap` only), **File ▸ Quit** (Ctrl+Q; asks first when there are unsaved changes) |
| Edit | **Edit ▸ Undo** and **Edit ▸ Redo**, named by the step they undo or redo (256 steps), **Edit ▸ Duplicate**, **Edit ▸ Delete**, **Edit ▸ Select all of this solid**, **Edit ▸ Drop to ground** |
| View | **View ▸ 3D view**, **View ▸ Map** (M switches), **View ▸ Frame selection** (F), **View ▸ Reset camera** (the region's overview), **View ▸ Hint line** |
| Race | **Race ▸ New race**, **Race ▸ Test race** (F5), **Race ▸ Drive here** (Shift+F5) |
| Help | **Help ▸ Controls**: every key, made from the editor's key table |

## Controls

**Help ▸ Controls** lists the same keys as this table; both are made from the editor's key table (a test checks
that this table matches it). Tab is not the editor's: it moves the keyboard focus between fields. Single keys
do nothing while a text field has the keyboard.

| Keys | Does |
|---|---|
| `Q` | Select tool: a click selects a prop or a race point, never adds |
| `W` | Move tool: drag an arrow to move the selected props |
| `E` | Rotate tool: drag the ring to turn the selected props |
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
| `LMB / Ctrl+LMB` | Select / add to the selection |
| `Shift + drag an arrow` | Copy the selection and move the copy |
| `Alt while dragging or placing` | No snapping |
| `Shift + click on the route` | Insert a checkpoint there |
| `[ / ]` | Previous / next checkpoint |
| `Ctrl+D` | Duplicate |
| `Del` | Delete |
| `End` | Drop to the ground |
| `Ctrl+Z, Ctrl+Y (Ctrl+Shift+Z)` | Undo, redo |
| `Ctrl+N, Ctrl+O` | New map mod, open a map mod |
| `Ctrl+S` | Save |
| `Ctrl+Q` | Quit |
| `Enter` | Confirm a dialog |
| `F5 / Shift+F5` | Test race / Drive here |
| `Space` | Play / pause the simulation (Simulate tab) |

While the right mouse button is held the tool keys fly the camera instead (W A S D, Q / E).

## Props

- **Select** a prop with a click in the view or in the tree; Ctrl+click adds to the selection. **Edit ▸ Select
  all of this solid** selects every placement of the same model.
- **Move** shows an arrow per axis, **Rotate** a ring around the vertical. The handles keep their size on screen,
  in the 3D view and in Top. Moves snap to 0.25 m and turns to 15° while **Snap** is on (right-click it for other
  steps; Alt held: no snapping). Shift while dragging an arrow moves a copy. A drag moves the world as you drag
  and is one undo step when released.
- **Prop browser** (bottom tab): the region's placeable models with a search, the filters **Knock-over**,
  **Barriers**, **Signs**, **Street furniture**, **Trees** and **All in region**, and how many of each are
  placed. Drag a tile into the view (it lands on the ground under the pointer, facing the camera's heading), or
  choose one and press Ctrl+D with nothing selected to place it at the pointer.
- **Properties**: the model's name and where it comes from (an original placement, added by this mod, or added
  by another mod, which this mod cannot change), the position (typed values are one step on Enter), the heading
  and tilt with **Upright** (stand it straight, heading kept) and the ground under it, **Reset to original**,
  **Duplicate**, **Delete**, and what happens to its collision (see Limits). With several props selected, moves
  and turns apply to all, around their centre.

## Races

- **Race ▸ New race** (or **New race…** in the tree) makes a sprint on the selected race's base track, else on the race
  track whose roads come nearest to where the camera looks, and arms Start: click the road for the start line, then for the checkpoints (the Checkpoint tool is armed for you),
  finish last for a sprint. Points snap to the base track's roads while Snap is on (Alt: no snapping).
- With **Select**, a click on a race point selects it, a drag moves it, Del removes it, `[` / `]` step through the
  checkpoints. With Checkpoint armed, Shift+click on the route inserts a checkpoint into that leg. Grid adds a
  slot (8 at most; with a slot selected, a click moves it).
- **Properties** of a race: name, mode (circuit, sprint, street X, drift), laps, **Base track** (which race track
  lends the road network, barriers, AI lanes, traffic and opponents), **Direction** (forward or backward),
  length, status, **Delete race**; of a race point its position.
- **Validation** (bottom tab): every problem of every race, each with **Show** and, where one exists, a
  one-click fix (**Try base track N**, **Snap grid to road**, **Snap start to road**, **Snap finish to route**,
  **Set laps to N**, **Rename to …**). A race with an error (a leg with no road, fewer than 2 checkpoints) cannot
  be tested or simulated. Warnings from the last simulation are listed here too.

## Saving

**Save** (Ctrl+S) writes `map.obmap` through a temporary file, so a failed write leaves the old file in place.
Only Save writes `map.obmap`: Test race, Drive here and Simulate play a copy (below). Unsaved changes are also
written to `.editor/map.obmap.autosave` every minute; when you open a mod whose autosave is newer than the mod,
the editor offers to restore it. Quitting or opening another mod with unsaved changes asks first.

The editor's own files live in the mod's `.editor/` folder and are never exported:

| File | Holds |
|---|---|
| `editor.toml` | the view: region, camera, the widths of the tree and the properties and the height of the bottom tabs (written on save, when another mod is opened and when the window closes) |
| `map.obmap.autosave` | the unsaved changes |
| `tested.txt` | the tested marks |
| `test-result.txt` | the last test's result line |
| `run/map.obmap` | the run copy the game plays for Test race, Drive here and Simulate |

An older editor kept these files beside `map.obmap`; opening the mod moves them into `.editor/` once (a file
already in `.editor/` stays, and the old one is then left where it was).

## Testing a race

**Test race** (F5) writes the mod as it is in the editor, unsaved changes included, to its run copy
`<mod folder>/.editor/run/map.obmap` (through a temporary file, so the game never reads half a file) and starts
the game next to the editor on that copy:

```
game --map-file <mod folder>/.editor/run/map.obmap --map-race <id> --map-result <mod folder>/.editor/test-result.txt
```

`map.obmap` is not written and the mod stays unsaved; the status bar adds "(testing unsaved changes)" while it
has changes Save has not written. `--map-file <path>` reads the mod from that file; in the run the mod is known by
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
(`drove=yes`).

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

The editor reads the stream while it arrives and never waits for the game. The tab has **Play** / **Pause**
(Space), speed 0.25x to 8x, a scrub bar over everything recorded so far, the camera (**Free camera** or behind a
racer) and **Lines** (each racer's driven line). The cars are drawn as boxes in the 3D view and as arrows on the
map (the player's slot and the followed car lime, opponents light, traffic grey, street racers amber), with each
racer's place. Below are the results (place, car, time, best lap, laps) in finishing order: finished racers by
time, then the rest by their place on the road (no places before the green light); a racer without a finish when
the run ended is last, as "did not finish". The player's slot, which the AI drives here, is "Your slot (AI)".
Under the table the events (green light, laps, finishes, crashes, stuck, wrong way), each with **Show**. A racer
that stood still for 4 s after the green, drove against the race's direction for 2 s, or lost 6 m/s or more in
one tick becomes a warning in the Validation tab, named by the checkpoint it was heading for: "Simulation: AI got
stuck before checkpoint 3".

### Simulation stream

Plain text, one record per line; readers skip records they do not know.

| Record | Fields |
|---|---|
| `OBSIM 1 mode=race seed=7 race=<id> track=<n> laps=<n> checkpoints=<n>` | the header (version 1) |
| `C <id> <P\|R\|T\|X> <name>` | a car's first appearance: player slot, opponent, traffic, street racer |
| `F <tick> <time> <car>\|<car>…` | one update; each car `id kind x y z qx qy qz qw speed lap checkpoint place` |
| `E <tick> <time> <kind> <car> <checkpoint> <value>` | `start`, `checkpoint`, `lap` / `finish` (value: time), `crash` (m/s lost), `stuck`, `wrong-way` |
| `R <car> <place> <time> <best> <lap,lap…>` | a racer's result (`-`: none; place 0: did not finish) |
| `END <finish\|quit\|limit\|error …>` | the end |

Times are seconds on the race clock (negative before the green; the start scene sets it back once) or since the
start in free roam; positions in world metres, the rotation a unit quaternion, speed in m/s. Ids: the player's
slot 0, opponents 1.., traffic 100.., street racers 200...

## The game's free camera

Started with `--editor`, the game has a free camera in free roam (F8). The world is paused under it, W A S D /
R / F fly, the mouse looks around, Z / X change the field of view, H hides the hint strip, G puts the car at the
aim point, and Esc or F8 returns to driving. It does not edit anything. Without `--editor`, F8 does nothing.

## What a custom race is

A custom race runs on a **base track**: one of the game's race tracks in the region. The base track provides the
road network (`TRACKS/ROUTES<region>/Paths<track>.bin`: the roads around that track's own race, not the whole
region, so a custom race stays within its base track's area), its race barriers (the roads closed for it), the
AI racing lanes (the base track's `Routes<track><F|B>.bin`: the racing line of the base track's own race, not of
the custom route; see docs/tracing/ai-lanes.md), traffic, the opponents, and the race set-up of a career event on
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

## Limits

- Collision of the city's static scenery is baked per section and is not tied to the placements. A moved or
  deleted building keeps its collision where the original has it, and an added copy of one has none.
  Knock-over props (cones, signs, barrels, …) carry their own collision: moved ones are hit where they now stand,
  added copies of one are knock-over props too, and deleted ones are gone.
- Overlays (route, checkpoints, handles) are drawn over the world without depth.
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
the mods at the first world load of a run. To share a mod, use **File ▸ Export…** or the launcher's Export… (a
`.zip`). To import one, use **File ▸ Import…**, the launcher's Import a map mod… ("Add a mod…" takes a map mod
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
| `[race]` | `id` (a-z 0-9 - _, unique), `name`, `track` (base track number), `backward` (yes/no: the base track's backward barriers and lanes), `mode` (circuit, sprint, streetx, drift), `laps` (1–20; 1 for sprint and drift), `checkpoint` (x y, repeated, in order, at most 64), `grid` (x y z heading, repeated, at most 8, pole first), `start` / `finish` (x y z heading) |

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
(placements by number; the 3D view is framed on them) and `--sim <stream file>[@seconds[:car]]` (a saved
simulation at that time, following that car). A shot runs on a fixed clock, so the same arguments draw the same
picture. scripts/checks/editor_shots.sh runs it at every size of the resolution matrix (docs/checks.md).

## How it is wired

- `src/tools/map/` (the game library): the format (`format.rs`), the folders, archives and the editor's files'
  places (`store.rs`: `editor_dir`, `run_file`, the run copy's atomic write, `move_old_sidecars`), the custom
  race's route (`race.rs`), the game data the editor reads (`assets.rs`: `MapAssets`, the race tracks and their
  road networks), and the edits without a window (`edit.rs`). `tools::map::set_only` makes one mod the only one
  for a run: `Only::Folder` (`--map-mod`) or `Only::File` (`--map-file`). `sim/` holds the stream format and the
  problem detection, which the game writes the stream with and the editor reads it with.
- Edits as data: `ops.rs` has `Op`, one edit of a mod (move, delete, reset, add, duplicate or drop a prop; make,
  change or delete a race and its points; the mod's details; a `Group` of ops under one name), with props named
  by `PropKey` (`Original` with the mod file's region, section, solid, place and `nth`, or `Added` with the add's
  id), races by id, never by an index in a loaded world. Ops read and write JSON (one per line in an
  `ops.jsonl`; vectors as arrays, rotations as their three rows) and round-trip exactly. `session.rs` has
  `MapSession` (the mod, the original and edited world, the undo history, no window): `run(op)` makes an op as
  one undo step named by `Op::label` (the names Edit ▸ Undo shows), and a refused op or group changes nothing.
  An edit re-applies only the placements it changes. The shapes of `Op`, `PropKey`, `RacePoint` and `Change`,
  their JSON and the step names are fixed; new ops and refusal reasons may be added.
- `src/world/map_overlay.rs`: applies the edits to a loaded world. The game loads worlds through
  `tools::map::load_world` (`World::load`, then the mods for the region).
- `crates/ob-editor/`: the desktop editor, a crate of its own on the game library, so the game's dependency tree
  has no egui. `src/editor/` there is its library: `app.rs` (the window's panels, dialogs, `--shot`), `menu.rs`
  (menus, toolbar, the key table `KEYS`, `Command::ALL`), `tree*.rs`, `props.rs`, `browser.rs`, `races.rs`,
  `validate.rs`, `viewport.rs` and `worklight.rs`, `mapview.rs`, `sim/`, and `testplay.rs` (the test hand-off:
  it writes the run copy, starts the game — `game` beside the editor, `OB_GAME_EXE` names another — polls it,
  reads the result line and keeps the tested marks). `src/bin/overbright-editor.rs` is the window. Its tests run
  with the workspace's (`cargo test -p ob-editor` alone); `docs_tests.rs` checks this page against the key
  table and the menus.
- The game: `src/bin/game/shell/cli.rs` (`MapTest`: the flags, the end watch, the result line),
  `flow/race_setup.rs` (`start_map_test`, `map_race_event`), `Game::map_race_route`, which `drive_to_grid` asks
  before reading a track's own route, and `enh/sim_stream.rs` (the simulation runs).
- The launcher: the Mods page's Map mods section (`launcher/screens.rs`), `Env::start_editor` and
  `dirs::editor_exe` (docs/launcher.md).
- Tests: `src/tools/map/tests.rs` (round trip, refusals, store, the run copy and `.editor/`, world and route with
  the game's files), `src/tools/map/session_tests.rs` (every op through JSON, step names, all-or-nothing groups,
  replay), `crates/ob-editor/src/editor/` (every menu command taken, every key in the key table, the toolbar at
  every width, the hand-off with a stand-in game: unsaved changes tested from the run copy, `map.obmap` untouched,
  the tested mark), `shell/cli.rs` (the flags and the run's end), `flow/race_setup.rs` (a test run on a custom
  race and from a spot, with the game's files), and `launcher/tests.rs`.
