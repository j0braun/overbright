# Overbright Studio

> **Preview.** Studio's window, `overbright-studio`, is the shell the map editor and the later workspaces move
> into. This version has the shell, Home, the map editor as the World and Races workspaces, the Browser, Runs, the
> Changes and Data pages and the Mod page.
> `overbright-editor` ([Map editor](map-editor.md)) still opens the same editor in a window of its own. For starting
> Studio and what this preview leaves out, see [Getting started with Studio](studio-start.md).

```
overbright-studio [--mod <name|folder>] [--maps <dir>]
overbright-studio --shot out.png [--size WxH] [--scale F] [--mod <folder>] [--maps <dir>]
                  [--workspace home|world|races|browser|runs|mod|settings|changes|data] [--input mouse|keyboard|xbox|playstation|deck]
                  [--focus rail|address|toolbar|left|view|right|bottom] [--drawer] [--files] [--warning]
                  [--find] [--address] [--select N] [--row N] [--places] [--job WORDS]
                  [--right-tab inspector|history|help] [--new-mod NAME] [--tutorial N] [--race-tutorial N]
                  [--collision off|invisible|all]
                  [--search WORDS] [--kind models|textures|sounds|strings]
                  [--run FILE] [--compare FILE] [--car N] [--at FRAME]
overbright-studio --version
```

`--shot` draws the whole window off-screen into a PNG (no window opens) and prints `studio layout: fits …` or
`studio layout: clipped: …`. The mods are the map editor's (`--maps`, default the map mods folder); `--mod` takes a
mod's folder name there or the path of any mod's folder ([Getting started](studio-start.md#starting-it)). `--select N`
selects placement N in World (on Changes: focuses value row N); `--new-mod NAME` shows the New mod dialog with NAME
typed. `--files` folds the Mod page's Files open, `--warning` shows its "Made from the original game" question.
`--tutorial N` shows *Your first mod* at step N (1 to 5; 6 is the last card), `--race-tutorial N` *Make your first
race* at step N (1 to 6; 7 is the last card). `--row N` focuses the focused
area's N-th control, `--places` opens the places under ← (after World and Mod), `--job WORDS` shows a running
job, `--right-tab` picks the right dock's tab. `--collision` sets World's collision layer. On Browser, `--search` types
words into the field, `--kind` picks the chip and `--select N` focuses the N-th result. On Runs, `--run` opens a run
file of the mod's runs folder, `--compare` compares it with another, `--car` picks the telemetry car and `--at` seeks
to a frame. The game folder is
`NFSU2_ROOT`, else the one the launcher kept.

## The window

| Part | What it is |
|---|---|
| Menu bar | File, Edit, View, Go, Test, Mod, Help; back and forward (← →); the address bar; Find; the open mod's name with its lamp. Under 1100 points wide, Test and Mod sit in Go and File. |
| Rail | Home at the top, then the workspaces that make things (MAKE) in the order they shipped, then Mod (SHARE) and Settings at the bottom. With names and Ctrl+number at 850 or more points high, icons only below (the name shows on hover). A workspace never moves and its Ctrl+number is never given to another. |
| Left panel | The workspace's own (World: the Mod tree). 270 points (230 under 1400 wide), dragged between 180 and 40 % of the width; folds to a 28-point strip under 1400 wide. |
| Right dock | Inspector, History, Help. 320 points (300 under 1400 wide), 260 to 40 %; under 1180 wide a 28-point strip that opens a 340-point drawer over the view (Esc or B closes it). |
| Bottom dock | Problems, Log, Jobs, then the workspace's tabs. 20 % of the height (120 to 240); folded to its tab strip under 700 points high (Runs: open at any height, 36 % of it, for its graphs). It spans the view's width only. |
| View | A tool workspace's 3D view, at least 480 × 270: when the docks would squeeze it, the right dock becomes a drawer first, then the left panel folds. On Home, Mod and Settings: the page, on a slab at most 1320 points wide. |
| Legend row | The keys or buttons that matter in the focused area, right-aligned, as the launcher shows them. Hidden while the mouse is used; it comes back with the first key that moves focus or any pad button. |
| Status bar | Left: whether the work is saved ("Saved automatically" once the files have every step, "Saving…" for the moment before, "Saving is behind", a disk problem in red) and the last step to undo. Right: what the checks found ("Problems 0 · 1 warning": the map editor's checks in World and Races, else the Mod page's ready list with the map checks; a click shows them), the busy line of a running job (a test in the game, a simulation with its run "2 of 5"; the Jobs tab and Home list them), and whether the mod is on in your game. The counts and the busy line drop first on narrow windows. |

The window is at least 960 × 600 points; below that it shows one line, "Studio needs at least 960 × 600". The UI
scale grows with the window: 1.25 from 1000 lines, 1.5 from 1400, 2 from 2000 (times the system's own scale),
never so far that the window falls under its smallest size. The Steam Deck's 1280 × 800 is the smallest size every
walkthrough is checked at. The docks' inner edges are dragged to size them (in the ranges above). Panel sizes and
folds are kept per workspace in the mod's editor folder
(`.editor/studio-layout.toml`, never exported); View ▸ Reset layout forgets them.

Colours, faces, lamps and the shared words are the launcher's (`crates/ob-look`, [STYLE](../STYLE.md)). Where a
value comes from is marked by a shape as well as a colour: a grey ring is the original's, a lime disc is yours, a
pink diamond is another mod's that wins in your game.

## World and Races

Both are the map editor ([Map editor](map-editor.md)) on the open mod, the same editor `overbright-editor` shows:
its toolbar in the toolbar row, the Mod tree in the left panel, the selection's properties in the Inspector, its
tabs (Prop browser, Race, Validation, Simulate) after Problems, Log and Jobs, the hint line under the view, and the 3D
view or the map drawn in the window by the game's own renderer. The two show different parts of it:

- **World** is the props: the left panel lists the Props branch, the toolbar has Select, Move, Rotate, Snap and Drive
  here (no race tools, Test race or Simulate), the Inspector is the selected prop's, the bottom tabs are Prop
  browser and Validation, the collision layer can be switched on, and the race route is not drawn.
- **Races** is the races: the left panel lists the mod's races (with New race), the toolbar adds the race tools
  (Checkpoint, Start, Finish, Grid), Test race and Simulate, the Inspector is the selected race's or race point's,
  the bottom tabs are Race (the race as a strip of its stops and legs), Validation and Simulate, the route is drawn over the props, which are dimmed, and the
  collision layer is off.

In the game a race's barriers (`BARRIERS_<track>`, `PLAYER_BARRIERS_<track>`) are on only during a race on that base
track, so the collision layer follows the workspace: World draws the permanent walls and free roam's own barriers
(`BARRIERS_4000`, the career stages'), the races' barriers only with View > Collision > Show race barriers; Races
draws the selected race's barriers over the permanent walls, dimmed.

Each keeps its own selection, camera, view and layers while the other shows. The editor's menu items join Studio's menus (Edit, View, Test: New race, Test race, Drive here). Problems
lists the mod's warnings and errors; Log is the editor's log.

The editor's keys work as in its own window (Q W E tools, Ctrl+Z / Ctrl+Y, F5 Test race, Shift+F5 Drive here,
RMB + WASD to fly). While the view, the tree or the bottom dock has focus, Enter, Esc, Tab and the arrows are the
editor's; F6 and the pad's triggers still move between areas. The pad's L4 / R4 (Deck) undo and redo, the Menu
button tests the race. Studio keeps a few keys for itself: Ctrl+D opens Changes (the editor's Duplicate is in Edit
and the prop menu), Ctrl+G opens Data and Ctrl+N opens Studio's New mod dialog.

**How World's and Races' output is checked.** `studio::world_tests` draws both with the map editor at the six matrix sizes for the mouse, keys and pad (clip and overlap check, the editor's toolbar check, its view inside Studio's), and walks into and out of World by keyboard and pad; Test race runs the race in the game.

## Home

The open mod's name; a card that says how it stands, with the next steps (Go to World, Make a race); and rows for
its changes, whether it is in your game, how it is saved and whether this window may change it (another program
that has the mod open keeps it; Studio then only looks). Beside them: your mods and something new to start. All of
it is read from the mod's history, as the program that writes it has it. Under the rows, **What each place does**
lists World, Races, Changes and Mod with their Ctrl+number and one line each; a row goes there. Over your mods,
**Start here** has the two tutorials, *Your first mod* and *Make your first race*. Rows that do not fit the window
are left out (the Mod menu lists all your mods).

Under **Start something new** are **Empty map mod** and two templates, **Sprint race** (one sprint along the
docks: a start, nine checkpoints and a finish) and **Circuit race** (three laps downtown). A template opens the
New mod dialog, which says what the mod starts with; Create makes the mod with that race in it, ready to change.
Under **Sample** is the **Sample race pack**: three races (a sprint along the docks, a circuit downtown and a hill
sprint past the observatory) with start gates of water barrels, crash barrels at the finishes, curve signs and
road barriers at the sharp corners. It opens your own copy, made the first time (folder `sample-race-pack`);
what you change in it stays yours. `overbright-studio-cli pack create --template sprint|circuit|sample` makes the
same mods.

**Empty map mod** under "Start something new", **New mod…** on the card when no mod is open, **File ▸ New mod…**
and Ctrl+N open the New mod dialog: a name, and the folder made from it (or typed: a-z, 0-9, `-` and `_`). It
says as you type where the mod goes, or why it cannot be made: no name, a name over 60 characters, a name another
of your mods has, a folder in use or a bad folder name; nothing is ever written over a mod. Create makes it the
way the map editor's New map mod… and `overbright-studio-cli pack create` do, opens it and goes to Home, which
says "Your new mod is ready". Tab and the arrows (the D-pad) move between Name, Folder, Create and Cancel; Enter
(A) creates, or cancels on Cancel; Esc (B) cancels. On Home itself ↑ ↓ (the D-pad) move between the card's
buttons, the places, Start here, your mods, Empty map mod, the templates and the sample, and Enter (A) takes the one
with the lime ring.

### The tutorials

*Your first mod* walks through five real steps: **Make a mod** (the New mod dialog, with "My first mod" typed in),
**Go to a street** (World, the camera taken down to street level in the city), **Place a prop** (drag one from the
Prop browser, or let Studio place a cone where you look: the editor's own add, one step to undo), **Drive it**
(Drive here: the game starts at that spot with the mod) and **Share it** (the Mod page and its Export). Each step
ticks by itself when the mod shows it done, however it was done; only the mod the tutorial made counts, so a mod
you had before is never changed by it. The card with the step is Home's card while Home is open and sits over the
view's lower right corner elsewhere; its button does the step and **End tutorial** closes it. Start it from Home's
Start here, Help ▸ Tutorial or F1; F1 also does the current step. With a pad, the step's button is on Home's card
(LB to Home, A). The last card's **Go to Races** goes on to making a race.

*Make your first race* walks through six: **Make a mod** ("My first race"), **Choose a base track** (Go to the
docks: Races, the camera over the dock road in Coal Harbor, and the editor's New race, which takes the base track
whose roads are nearest), **Place the start** (the Start tool's click on the road: the line sits on the road at
the road's height, and the cars line up behind it facing down the road), **Add race points** (click the road with
the Checkpoint and Finish tools, or let Studio add four checkpoints and the finish along the base track's roads
ahead of the start, each one step to undo; the step ticks when the race has no error left), **Test the race**
(Test race, F5) and **Share it**. While a race is selected, its base track's roads are drawn faint green over the
view and the map, so you can see where race points may go. The last card's **Open the sample** opens the sample
race pack. Start it from Home's Start here or Help ▸ Make your first race. While either tutorial runs, F1 does
its current step.

**How Home's output is checked.** `studio::tests` draws it at the six matrix sizes for the mouse, keys and pad with the clip and overlap check, `studio::inside_tests` tabs round every control, and `studio::guide_tests` walks it by keyboard and pad from Start here to an exported pack.

## Changes

Ctrl+D (or `ob://<mod id>/@changes`) opens the Changes page over World; Esc goes back to where it was opened. It
lists every value the open mod gives, grouped by item, each with where the value in your game comes from, drawn
as a mark: a ring for the original game's, a lime dot for yours, a pink diamond for another enabled mod's that is
above yours in your game (the mods apply in their enabled order; a mod that is off is shown as if turned on, on
top). Beside each value is the original. A value another mod beats also shows yours and why it is not used, and
**Show in Clashes** in place of Reset.

- **This mod** (`@changes`): your game without the mod → with it. *Show* picks **Everything this mod changes**,
  **Only what loses**, **Only edited** (what differs from the original: an item the mod only restates is left out)
  or **All values** (every value of the items the mod changes). The inspector's **Touches** line says what the
  focused item touches: its region and stream section, the neighbouring sections that draw that section (from the
  city's section lists, when World has them loaded) and whether it **affects play** (props and races do; textures
  are looks only). **Return to original** gives the whole object back as one history step (a moved prop stands
  where it did, a hidden one is shown, an added prop or a race is gone); Ctrl+Z takes it back.
- **History step** (`@changes?step=<n>`): what one step of the mod's history changed, before → after.
- **Compare versions** (`@changes?version=<name>`): a kept version → now.
- **Clashes** (`@changes?mode=clashes`): the values another mod gives too, as the stack in your game's order with
  the winner last.

A mode is only shown when it has something to show. *Reset* (in This mod and Clashes) gives one value back: a moved
prop's position goes back to its original place; a heading (whose original is in the game's files) or a hidden prop
takes the edit out. For an added prop or a race the button is *Remove*. ↑ ↓ (the D-pad) move between rows; Shift+↓ or Enter (A) picks several, and Reset in the
row's menu (Shift+F10, X) takes them back in one step. Every Reset is a step of the mod's history: Ctrl+Z takes it
back. The selected row's details show the step that last changed it, with *Show in History*; *Show in World* and
*Show in Races* go to the item's address. The rows are the asset format's layering
answer (`ob_studio::changes`): each prop edit is a placement item, each race a race item, merged through the
enabled mods with the pack of every field.

**Looks rows.** Each texture the mod paints ([Studio Looks](studio-looks.md)) has a row of its own: the texture's
name, its thumbnail beside the original's (the game's own texture shows once World has loaded a region that uses
it; until then its ring), and, when another enabled mod paints the same texture and is above yours, that mod's
paint with yours under it as not used; the texture is then on Clashes too. *Reset* takes the texture out of the
mod's texture list, so the game draws its own again (one step; Ctrl+Z brings the paint back, and the pack files
follow). *Show in History* opens the step that last painted it. `ob://<mod id>/texture/world:0x1A2B3C4D` opens
Changes on that texture's row. A history step or a version compare lists the textures it painted.

## Browser

Ctrl+4 (MAKE ▸ Browser, `ob://<mod id>/@browser`) searches the game's assets of the region the open mod is edited
on: its models (the solids the region draws) and its textures, by name across both kinds. Type words of a name (all
must be in it), a hash (`0x1A2B3C4D`) or `section:1407` (what is placed or seen in that streaming section). The chips
under the field pick the kind (All, Models, Textures, Sounds, Strings, each with how many the words find) and that kind's filter:
models by category (the prop browser's), textures by use (In use, Unused, Animated, Large).

Every result says where it is used, worked out once from the loaded region and kept while it stays the same: a
model, how many placements stand it, in which sections, how many are hidden by a mod, and which textures its
materials draw; a texture, how many materials in how many models draw it, and in which sections they are seen. A
name in those lists is a link: it shows that result. The viewer beside the list (under it on a narrow window) shows
a model's picture, drawn by the prop browser's renderer and kept in the same cache, or the texture's pixels as
Looks reads them.

Browser only reads. Enter (A) or the button takes a result to where it is worked on: a model to World, selecting
its first placement (and choosing it in the prop browser, so Ctrl+D places another; a model with no placement says
so); a texture to its row in Looks on Changes when the mod paints it, else the `looks open` command that starts it.
Export glTF (Ctrl+E, X on the pad) writes the model as a glTF 2.0 file for an outside 3D program (Blender, any glTF
viewer): `<name>.gltf`, `<name>.bin` and its textures as PNG, in the mod's `.editor/reference/` folder, which the
mod's shared copy leaves out. It is a read-only reference: the pictures and shapes are your own game files, so the
file says `origin = "from-game-files"`, `share = "local-only"` (its `asset.extras` and a `LOCAL-ONLY.txt` beside it)
and Studio never writes into the game's folder. The game's +Z up is turned to glTF's +Y up by a root node. Bring back
(Ctrl+I) takes the exported file, edited in your 3D program, back as a mod model: its materials are mapped onto the
model's parts by name (the names the export gave them), the model keeps the game's texture list, flags and
markers, and anything that cannot be mapped is refused with the reason in the verdict line (a material that is
not a part, a part with no triangles, no texture coordinates, over 65,535 vertices, a file for another model).
The mod model is kept in `.editor/models/` with a record (`<name>.model.toml`): made from your game files, it is
local-only and a shared copy leaves it out. The file is read by a worker process of its own with a time and
memory limit, so a file that crashes the reader cannot take Studio down. `browser export` and `browser import`
do the same from the command line (`--own` there says a model is entirely your own work).
The model viewer also gives the mesh in numbers: vertices, triangles, whether it is lit, its size in metres (Z up)
and its parts with their triangles (`browser mesh` prints the same lines). Export area (the button at the top right,
Ctrl+Shift+E, X on the pad while a chip line has the focus) writes the area of 60 metres around the World camera as
one glTF reference, `area-<region>.gltf`: each model once as a mesh, each placement a node with its place, shown
placements only, at most 12,000 of them (a smaller radius is the way out, `browser area --radius`). It is marked and
kept like a model's export: from the game files, local-only, in `.editor/reference/`, never into the game's folder.

The Sounds chip lists the game's sound banks by event (`FX_TURBO_01`, with its bank, length and rate); Strings lists
a language file's lines by their hash, with the language chips under the kind chips (English unless another is
chosen), and finds a line by words, by `0x` hash or by its label. Both are read from the game folder the first time
the chip is chosen and kept; they are searched by the same field. Enter (A) on a sound plays it on the audio output:
the output is opened by that first click and never before, and nothing plays by itself. Export WAV (Ctrl+E, X)
writes the sound as a 16-bit mono WAV into `.editor/reference/`, marked as from the game files like a glTF; only the
banks' own codec is decoded, the others are listed and say so. `browser sounds`, `browser strings` and `browser wav`
do the same from the command line, and `browser area` the area export.
↑ ↓ and Tab move over the chips and results, ← → change a chip (in the results they move in the search field), Esc
(B) goes back. Cars and regions the mod is not edited on are not indexed yet.

**How the Browser's output is checked.** `studio::browser_tests` draws the models, sounds and strings kinds at the six matrix sizes for the mouse, keys and pad (every part inside its slab, none over the viewer), reads the index against the world, checks that the command line prints the viewer's numbers, and checks that a missing game folder is said instead of listing nothing.

## Runs

Ctrl+5 (MAKE ▸ Runs, `ob://<mod id>/@runs`) is where a mod's recorded runs are read. It is the map editor's own
view, timeline and cameras (the same playback as the Simulate tab; there is no second player) with the list of the
mod's run files in the left panel and a Telemetry tab in the bottom dock. Both are open at any window size the view
leaves room for.

- The list. Every run file in the mod's editor folder (`.editor/runs/*.obrun`, never exported) is a row: its name,
  the race (or "free roam"), the seed, how it ended and how big it is. A simulation that ends (Simulate in the
  toolbar) is kept there by itself as `sim-<race>-seed<n>`; a run file put in that folder shows up too. Enter (A)
  opens the row in the view, paused at the start: scrub, play, step, change the speed and ride with a car as for any
  run, at any display rate. Under the row the cursor is on: Compare, Rename and Delete (C, F2, Delete). Rename
  never replaces another run; Delete asks first (Enter deletes, Esc keeps). The pad's D-pad moves over the rows, A
  opens, X's menu has the rest.
- Telemetry. For the car chosen (the one followed, else the first racer): its speed over the run's time, and how
  far it is from the racing line, left of its way positive (the race's start, checkpoints and finish in the open
  mod; a circuit closes the loop), with a tick for each event of the car and a line at the time shown. A click or a
  drag on a graph moves the playback. The values are the run file's, frame by frame. A run recorded with the drive
  channels (Drive to record, below, or `--run-channels drive`) also has the car's gear, throttle, brake and steering
  input as graphs, and the gear in the readout; a channel the file does not carry is not drawn (an older run has
  none of them), and the lateral graph says so when the run's race is not in the open mod.
- Compare. Compare on a second run draws its racers as ghost outlines in the view (and arrows in the Map) on the
  same race clock, a row of its events under the timeline's track, its graphs in the Telemetry tab, and where it
  parts from the first: the first state hash that differs (the tick itself when both runs carry the hash of every
  tick, as a recording does; within the 60 ticks before it otherwise), or its car a metre from the first's at the
  same time. Compare again on the same row stops it.
- Drive to record. The button above the list (also Test ▸ Drive to record) starts the game on the race chosen in
  Races, or from the camera's spot when none is, in its window as Test race does, and the run you drive is kept
  as a run (`drive-<race>`, `-2`, `-3` when taken). The game writes the run file as you drive (one frame per
  update, whatever the display rate, with the drive channels and the state hash of every tick), so it can be
  opened in the list while the game still runs; it ends when the race does, when the game window is closed, or
  when Studio starts the run over (an edit that reaches the game does that: the recording stops where it was and
  keeps what it has). Recording reads the game and never changes it: the run is the same with it on or off. A
  test that is already open stays as it is; close its window first. A command line has no game window, so there
  is no `runs` command for it (`game … --record <file>` is the game's own flag, docs/modding/asset-format/runs.md).

Runs only reads the files (and keeps, renames or deletes them). Telemetry to a port and a race from a driven line
are not in it yet.

**How Runs' output is checked.** `studio::runs_tests` draws the page at the six matrix sizes for the mouse, keys and pad, opens and compares runs in the editor's playback, checks the telemetry against the run file and keeps the list honest after a rename or delete; a run recorded in the game is the same with recording on or off (the frame-rate and parity checks).

## Data

Ctrl+G (or `ob://<mod id>/@data`) opens the Data page over World; Esc goes back. It shows one kind of record at a
time as a table: **Props** (the props the mod moves, hides or adds) or **Races**. The columns are the kind's
schema (`ob_studio::data`: the same fields the file check and the inspector use); the rows are the Changes rows, so
each cell carries the same mark (ring: the original game's, lime dot: yours, pink diamond: another mod's) and a
value of yours shows "was" and the original beside it. A mod's value is an overlay on the original's: the table
holds only what the mod gives.

- **Sort.** Click a header, or ↑ to the header, ← → to a column and Enter (A): up, then down, then off.
- **Search** narrows the rows to those with the words in any cell.
- **Edit.** ↑ ↓ ← → (the D-pad) move between cells. Enter (A) edits a cell, Enter keeps it, Esc (B) leaves it. A
  yes / no cell flips at once. Each edit is one undo step of the mod's history, named as the editor names it
  ("Laps 4"), and Ctrl+Z takes it back; the same op runs from the editor, the command line and agents. A value the
  format refuses leaves the mod as it was and says why.
- **Select.** Shift+↑ ↓ ← → (or Shift and a click) grows the selection from the focused cell, Ctrl+A selects every shown
  cell, Esc ends it. On the pad, View starts and ends selecting: the D-pad then grows the selection.
- **Fill.** With several cells selected, Enter (A) edits the focused one and Enter keeps the value in every selected
  cell, as one undo step ("Fill 4 cells"). A cell that cannot be written (a read-only column, another mod's row)
  or a value it refuses leaves the whole fill undone and says which cell and why.
- **Copy and paste.** Ctrl+C puts the selection on the clipboard as tab-separated text. Ctrl+V pastes tab-separated
  or comma-separated text: one value goes into every selected cell, a block starts at the focused cell and has to fit
  the table. Every cell is checked by the same rules as a single edit, a block is all or none, and it is one undo
  step ("Paste 6 cells").
- **Table file.** Export table (Ctrl+E) writes the shown table as CSV to `.editor/reference/<kind>.csv` in the mod's
  folder (never the game's folder): a `row` column with each row's key, then the columns by field name. Edit the file
  in a spreadsheet and Import table (Ctrl+I) brings it back: only the cells that differ become edits, each checked as a
  single edit is, all as one undo step ("Import table 3 cells"). A file that changes nothing changes nothing. The
  command line takes any path: `data export --to`, `data import --from`.
- **Long tables.** Only the rows in view are drawn, so tables of tens of thousands of rows scroll smoothly; the wheel,
  the arrows and the scroll bar at the right move through them.
- **In Changes** on the focused row opens that row's entry on the Changes page.

Races can be edited at any time (Name, Race type, Base track, Laps, Backward); a race's checkpoints are shown by
their count and a prop's Object is shown only: those columns cannot be written here yet. A prop's Position, Heading
and Hidden are edited while World has the mod open (they need the city loaded); without it the page says so.

**How Data's output is checked.** `studio::data_tests` draws the Props and Races tables at the six matrix sizes for the mouse, keys and pad, and each edit is checked in the mod's file and as one undo step.

## The Mod page

Ctrl+0 (SHARE ▸ Mod, `ob://<mod id>/@mod`) shows the open mod as you share it:

- **Name, Author, Description.** Enter (A) opens the field, Enter keeps it as one step of the mod's history
  ("Mod details"; Ctrl+Z takes it back), Esc (B) leaves it unchanged. While World or Races has the mod open, the
  step goes through the map editor, the mod's one writer.
- **In your game**: on, or "Off · Turn on in the launcher". Studio never turns a mod on itself.
- **Files**: what a shared copy holds, with sizes; Enter folds the list open. Work files (`work/`) and the editor's
  files (`.editor/` and every other name starting with a dot) are never shared.
- **Clashes**: the values another mod in your game gives too, each with who wins (read-only). Enter opens
  Changes on its Clashes view.
- **Ready to share**: the map checks pass (a mod with races needs the game's race tracks, from the game folder the
  launcher set), the mod has a name, and each texture of the mod's texture list is in the pack data as the picture
  the list names, in a size the game reads (else an error that stops the export; when all are, "N textures in
  place"); textures made from the original game's pixels are listed as a warning. The
  tested mark comes later.
- **Export to** and **Export**: one `.zip` (or, for a name without `.zip`, a new folder). It proposes
  `<folder>.zip` in the `exports` folder of Overbright's data folder ([Where Overbright keeps its files](../playing/platform-dirs.md)),
  which the first export makes; the note under it says where the file went. The `.zip` holds
  `<folder>/map.obmap`, as the map editor's and the launcher's exports do (all three write the same shared copy),
  so their Imports read it and bring every file back, plus the mod's other files (`looks.toml`, `textures/…`) and
  `provenance.toml`: the build that wrote it and the
  textures whose pixels came from the original game. A mod with such textures asks first ("Made from the original
  game"): Enter exports anyway, Esc does not. While the ready list has an error, Export says "Not ready to share
  yet: see the list" and writes nothing. You are responsible for what you share; Overbright never hosts or
  shares mods.

**File ▸ Export as mod pack…** writes the mod as one `<id>-<version>.obpk` instead: the file the launcher's Mods page
takes (it holds the placements, races and textures as their own files, with the same provenance). It asks for the
same yes for textures from the original game's pixels. The command line has `modpack export` (and `modpack convert`
for an older map mod or texture pack: [Studio command-line tool](studio-cli.md)).

The command-line tool exports the same way: `overbright-studio-cli pack export --mod <dir> --to <file.zip>`
(`--derived-ok` for textures from the original game's pixels; [Studio command-line tool](studio-cli.md)).

**How the Mod page's output is checked.** `studio::mod_page_tests` draws it at the six matrix sizes for the mouse, keys and pad, keeps a detail as one step, and exports a pack and imports it again as the same mod.

## Keyboard and pad

Studio has seven areas, always in this order: rail, address bar, toolbar, left panel, view, right dock, bottom
dock. F6 and Shift+F6 (the pad's RT and LT) step through the areas that are shown: a folded panel is skipped, a
drawer counts as its dock and opens when it gets focus. The focused area has a lime line along its top. Esc (B)
goes one level out: menu, field, row, area, and on a page the back link; it never leaves the workspace.

Inside an area the arrows (the D-pad) move between its controls in reading order: the rail's workspaces, ← → the
address and Find, the toolbar's main action, a dock's tabs and rows, a page's rows. The focused one wears the
launcher's focus, the selection fill and a lime ring; ← → (◀ ▶) change a dock's tab, Enter (A) takes the control as
a click does. Tab and Shift+Tab move the same way and go on past an area's last control into the next area (an
area with nothing to focus is passed by), so Tab alone reaches every control of the window. In World and Races the
map editor's view, tree and bottom tabs keep their own keys.

The pad's buttons do the same as in the launcher and the game. The keyboard uses Studio's own keys, because
letters are the tools' keys and typing here (the launcher keeps Q E, Space and R).

| Pad (Xbox · PlayStation · Deck) | Keys | Does |
|---|---|---|
| A · ✕ · A | Enter | Select |
| B · ○ · B | Esc | Back, one level out |
| X · □ · X | Shift+F10, Menu key | The focused thing's menu: undo, redo, test here, reset (on Changes), copy address |
| Y · △ · Y | Ctrl+K | Find: names, commands, places, pasted addresses |
| LB RB · L1 R1 · L1 R1 | Ctrl+PgUp, Ctrl+PgDn; Ctrl+1 … Ctrl+0; Ctrl+, (Settings) | Workspace |
| LT RT · L2 R2 · L2 R2 | Shift+F6, F6 | Area |
| D-pad | Tab, arrows; ← → | Move focus; ◀ ▶ change a value |
| Menu · Options · Menu | F5 | Test |
| View · Create · View | Alt+←, Alt+→ | Last place, and forward again |
| Sticks | Right mouse button + WASD | Move and look in the 3D view |
| Deck L4 R4 | Ctrl+Z, Ctrl+Y | Undo, redo |
| – | Ctrl+L, Ctrl+Shift+C | Show the address to edit or paste; copy it |
| – | Ctrl+D | Changes |
| – | Ctrl+G | Data |
| – | Ctrl+N | New mod |
| – (Home's card) | F1 | The tutorial's step; starts the tutorial |

Every button is one press; a chord is never the only way. Help ▸ Controls lists the same table, made from the key
table in `crates/ob-studio-gui/src/studio/nav.rs`.

## Addresses

The address bar shows names ("Harbour edits › World"). Ctrl+L shows the address itself, selected, to copy or to
paste another: Enter goes there, Esc returns to the names. Ctrl+Shift+C copies it.

- An item: `ob://<mod id>/<kind>/<key>[#<field>][?guard=<value>]`, as the asset format names items, e.g.
  `ob://harbour-edits/placement/L4RA/1407/12#position`. In World the selected prop is the address
  (`placement/<region>/<section>/<index>`; the names read "Harbour edits › World › L4RA › <solid>"), in Races the
  selected race (`race/<mod id>:<race id>`); following one selects it.
- A screen: `ob://<mod id>/@<screen>[?name=value…]`, e.g. `ob://harbour-edits/@world`; Studio's own screens use
  `studio` (`ob://studio/@home`). A screen's name starts with `@`, which no kind name can.

Back and forward (Alt+← / Alt+→, the mouse's side buttons 4 and 5, the ← → buttons, the pad's View button) walk the
last 50 places of the window; a right-click on ← lists them by name, newest first, and goes back to the one taken.

Find (Ctrl+K, the pad's Y) lists Go to (the workspaces), your mods (taking one opens it; Home's Open a mod opens Find on them), Commands with their keys (Test while World or Races is
open, Last place and Next place when there is one, Address, Copy address, Changes, New mod, Tutorial, Undo, Redo), the places you were and, in World and Races, the game's neighbourhoods of the region (Places: Enter flies the view to one; the one under the camera is named in the address bar and the status bar), narrowed by what is typed. ↑ ↓ (the D-pad)
move in the list; Enter (A) or a click runs the command or goes to the workspace or place; a pasted `ob://` address
goes straight there. An address Studio cannot follow (a mod you do not have, a screen this version lacks) is
not followed silently: the window says why.

## Checks

`cargo test -p ob-studio-gui studio` draws the window headless at every size of the resolution matrix (and at
display scales 1.5 and 2), in every workspace, for the mouse, the keyboard and a pad, and fails on any part
outside the window, two parts overlapping, a view or an address under its smallest size; it walks the window by
keyboard and by pad, and reads addresses back. `studio::inside_tests` tabs round every control of World and Home at
1920 × 1080, 1280 × 800 and 1024 × 768, reaches every row of each dock by the arrows and the D-pad, runs a command
and a place from Find, goes back and forward by the mouse's buttons 4 and 5, drags the docks' edges (the sizes are
kept with the mod) and reads the busy line and the counts in the status bar. `studio::world_tests` draws World and Races with the map editor at
the six matrix sizes with the same check and the editor's
toolbar check, and walks into and out of World by keyboard and pad. `studio::guide_tests` runs both tutorials by
keyboard and by pad on a stand-in city and makes the templates and the sample from Home; `studio::mod_page_tests`
keeps a detail as one step, exports a mod with derived textures and imports it again; `studio::changes_tests`
resets values and textures as single undoable steps and walks the rows by keyboard and pad.
`studio::docs_tests` reads this page against the code: the Keyboard and pad table is the key table both ways, and
every key the page names is in Studio's or the map editor's key table. A key added to the code needs a row here.
