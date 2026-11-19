# Map editor and map mods

> **Preview.** The map editor ships as a preview: it works, but not every part is finished and its
> files may still change between versions. Keep a copy of map mods you care about. In this version the window's
> Races mode has no race tools yet: races are not placed or edited in the window.

The map editor is a desktop program, `overbright-editor`, next to the game and the launcher. With it you move,
delete and add props in the city and make races of your own on the game's road network. Everything it saves is
a **map mod** in Overbright's own data folder, which is applied on top of the original data when a world loads.
The game's files are never written.

If no map mod is enabled, none of this runs: worlds and races are the original's. The game itself keeps only an
opt-in free camera (see below).

## Opening it

- From the launcher: **Mods** page, **Map mods**: choose a mod and press **Open in map editor**; or on the **Dev**
  page press **Open map editor (preview)** to make a new mod or open another one there. Only one editor runs at a time. While it is open the
  launcher's Map mods controls are greyed out, and when you close it the launcher reads the mods again.
- From a terminal: `overbright-editor --mod <folder> [--region L4RA]`. The game folder is taken from
  `NFSU2_ROOT` or Overbright's saved game folder, like the game does.

Each editor window edits one map mod on one region (the city, `L4RA`, for nearly everything).

## The window

- **Toolbar**: the mode (**Props** / **Races**, Tab), the tools of the mode, snapping, the view (3D / Map, M),
  and on the right **Drive here** and **Test race**.
- **Left**: the mod's tree. *Props* has the moved, deleted and added props. *Races* lists each race with its
  start grid, start line, checkpoints and finish line, an error badge when it cannot be raced, and a **tested**
  badge when it finished a test as it is now.
- **Centre**: the 3D view, drawn by the game's own renderer, or the 2D road-network map. The selection, the
  route, the checkpoints, the grid and the move handles are drawn on top.
- **Right**: the properties of the selection (position, heading, reset to original; race name, mode, laps,
  base track; a checkpoint's position).
- **Bottom**: the prop browser, the validation list (each entry has Show), and the log.
- **Status bar**: saved / unsaved, camera, issue counts, and the test run's state and result.

## Controls

| Input | Does |
|---|---|
| Right mouse button held + W A S D, Q / E | Fly; down / up (Shift faster, Ctrl slower) |
| Right mouse button held + wheel | Fly speed |
| Wheel | Move forward / back |
| F | Frame the selection |
| Left click | Select; Ctrl + click adds to the selection |
| W / E (Props) | Move / rotate tool; drag a handle (snaps 0.25 m and 15°, Alt held: no snap) |
| End | Drop the selection to the ground |
| Del | Delete |
| Ctrl+D | Duplicate |
| Tab | Props / Races |
| C / G / S / X (Races) | Checkpoint / grid slot / start line / finish line tool: click the road |
| M | 3D view / road map |
| Ctrl+Z, Ctrl+Y (Ctrl+Shift+Z) | Undo, redo |
| Ctrl+S | Save |
| F5 | Test race |
| Shift+F5 | Drive here |

## Saving

**Ctrl+S** writes `map.obmap` through a temporary file, so a failed write leaves the old file in place. Unsaved
changes are also written to `map.obmap.autosave` every minute and before every test. When you open a mod whose
autosave is newer than the mod, the editor offers to restore it. The editor's own state (camera, panel sizes)
is kept in `editor.toml` next to the mod, and the tested marks in `tested.txt`. Neither file is exported.

## Testing a race

**Test race** (F5) saves the mod and starts the game next to the editor:

```
game --map-mod <folder> --map-race <id> --map-result <mod folder>/test-result.txt
```

With these flags the game:

- applies only that mod for this run (whatever `enabled.txt` lists; nothing is written to it);
- skips the boot screens and menus and loads straight onto the race's start grid. The race uses its base
  track's career event of the same mode for its set-up (opponents, traffic) and runs as a race outside the
  career: no progress and no reward;
- drives the car of the career save that is loaded (with its parts), or the Quick Race car select's first car
  when there is no save. The save is then let go, so nothing in the run writes it;
- ends after the results have been shown for 8 seconds (or at once on Enter), on Esc (the pause menu), or when
  the window is closed. It then writes one line to the result file:
  `finished=yes place=1 of=4 time=83.412 laps=2`, or `finished=no laps=2`.

The editor keeps working while the game runs; only a second test waits. When the game has ended, the status bar
shows the result, and a finished race gets its **tested** badge until it changes. A race with an error in the
validation list (a leg with no road, fewer than 2 checkpoints) cannot be tested. A race the game cannot start
(an unknown id, no route, no career event of its mode on its base track) makes the game exit with the reason in
its log and no result line, which the editor reports.

**Drive here** (Shift+F5) does the same with `--map-at x,y,z,heading` (heading in degrees around +z from +x) in
place of `--map-race`: free roam with only this mod from the camera's spot on the ground. Esc ends the run
(`drove=yes`).

None of these flags change anything when they are not given. A flag without its value, a malformed spot,
`--map-race` together with `--map-at`, or `--map-mod` without one of them is a usage error (exit code 2).

## Simulating a race

**Simulate** (the toolbar: the chosen race in Races mode, else the world around the camera; or the buttons of
the **Simulate** tab at the bottom) saves the mod and runs the game next to the editor
with no window and no sound, the AI driving every car:

```
game --map-mod <folder> --map-race <id> --simulate --stream - --sim-seed <n>
game --map-mod <folder> --map-at x,y,z,heading --simulate-world --stream - --sim-seed <n>
```

- `--simulate` races the custom race as **Test race** would set it up, but the player's slot is driven from the
  grid by the AI racer controller that otherwise drives the player's car past the finish line (the human's
  skill row). `--simulate-world` runs free roam from the spot (traffic, street racers); the player's car stays
  parked.
- No window, no graphics device and no audio device are opened; nothing is saved.
- Each update (the 60 Hz tick) is written as one line to `--stream <file>` (`-` or no flag: standard output),
  with the race's events; see "Simulation stream" below. The same seed gives the same stream, byte for byte.
- The run ends at the race's end (every racer through), after `--sim-seconds` (default 900 for a race, 120 in
  free roam), or when standard input reads `quit` (the editor's **Stop**).
- `--stream`, `--sim-seed` and `--sim-seconds` without `--simulate` / `--simulate-world`, `--simulate` without
  `--map-race`, or `--simulate-world` without `--map-at` are usage errors.

The editor reads the stream while it arrives and never waits for the game. The tab has play / pause (Space),
speed 0.25x to 8x, a scrub bar over everything recorded so far, **Follow** (the 3D camera behind a racer) and
**Lines** (each racer's driven line). The cars are drawn as boxes in the 3D view and as arrows on the map
(the player's slot and the followed car lime, opponents light, traffic grey, street racers amber), with each
racer's place. Below are the results (place, time, best lap, laps) and the events (green light, laps, finishes,
crashes, stuck, wrong way), each with **Show**. A racer that stood still for 4 s after the green, drove against
the race's direction for 2 s, or lost 6 m/s or more in one tick becomes a warning in the Validation list,
named by the checkpoint it was heading for: "Simulation: AI got stuck before checkpoint 3".

`overbright-editor --shot out.png --tab simulate --sim <stream file>[@seconds[:car]]` draws a saved stream at
that time, following that car (a check aid).

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
slot 0, opponents 1.., traffic 100.., street racers 200... The stream format and the problem detection live in
the library, `src/tools/map/sim/` (`format.rs`, `detect.rs`), which the game writes the stream with and the
editor reads it with (`editor::sim` re-exports them); the game's side is `src/bin/game/enh/sim_stream.rs`.

## The game's free camera

Started with `--editor`, the game has a free camera in free roam (F8). The world is paused under it, W A S D /
R / F fly, the mouse looks around, Z / X change the field of view, H hides the hint strip, G puts the car at the
aim point, and Esc or F8 returns to driving. It does not edit anything. Without `--editor`, F8 does nothing.

## What a custom race is

A custom race runs on a **base track**: one of the game's race tracks in the region. The base track provides the
road network (`TRACKS/ROUTES<region>/Paths<track>.bin`: the roads around that track's own race, not the whole
region, so a custom race stays within its base track's area), its race barriers (the roads closed for it), the
AI racing lanes (the base track's `Routes<track><F|B>.bin`: the racing line and race links of the base track's
own race, not of the custom route; see docs/tracing/ai-lanes.md), traffic, the opponents, and the race set-up
of a career event on that track with the same race mode. The mod provides the checkpoints, the start grid (8 slots behind the start line when
none are placed), the start and finish lines, the mode and the laps. The route is built through the checkpoints
on the road network exactly as the game builds a track's own route (`RoadNet::route`, 0x5E5BC0). If a checkpoint
leg has no road through the open network (a closed barrier in the way), the editor shows it and the race cannot
be tested. A circuit closes back to its first checkpoint. A sprint ends at its finish line, or else at its last
checkpoint. Checkpoints taken from a track's own route build that track's road (test
`a_custom_race_on_native_checkpoints_is_the_native_road`).

Custom races are raced through Test race. They do not appear in the career or the Quick Race lists, and their
results do not count toward the career.

## Limits

- Collision of the city's static scenery is baked per section and is not tied to the placements. A moved or
  deleted building keeps its collision where the original has it, and an added copy of one has none.
  Knock-over props (cones, signs, barrels, …) carry their own collision: moved ones are hit where they now stand,
  added copies of one are knock-over props too, and deleted ones are gone.
- Overlays (route, checkpoints) are drawn over the world without depth.
- Shops, event markers, traffic lanes and AI lanes cannot be edited yet.
- Drag races are not offered (their lanes are fixed to the drag tracks).

## Where map mods live

| | |
|---|---|
| Mods | `<Overbright data>/maps/<folder>/map.obmap` |
| Mods in use | `<Overbright data>/maps/enabled.txt`, one folder name a line, applied in that order |
| Editor files | `map.obmap.autosave`, `editor.toml`, `tested.txt`, `test-result.txt` in the mod's folder (never exported) |

`<Overbright data>` is the folder from the README's "Where your files go" (`data/` in portable mode). If a mod
cannot be read, it is skipped with the reason in the log, and the others still apply. The game reads the list and
the mods at the first world load of a run. To share a mod, use the editor's File ▸ Export or the launcher's
Export… (a `.zip`). To import one, use the launcher's Import a map mod… ("Add a mod…" takes a map mod too), or
copy the folder into `maps/` and add its name to `enabled.txt`. `nfsu2::tools::map::store` has `import` (a
folder, a `map.obmap`, or a `.zip` holding it; validated before anything is written, never over an existing
folder) and `export` (a folder or a `.zip`, `map.obmap` only).

## File format (`map.obmap`, version 1)

UTF-8 text, at most 8 MiB. The first line that is not blank or a `#` comment is `overbright-map 1`. Then
`key = value` lines: first the header keys, then sections, each starting with a `[name]` line. Values run to
the end of the line; numbers are decimal (written in their shortest exact form, so a file read and written
again is the same); angles are degrees around +z from +x; coordinates are the game's world coordinates (Z up),
within ±100 000.

Header: `name`, `author`, `description` (optional, at most 200 characters each).

| Section | Keys |
|---|---|
| `[prop.move]` | `region`, `section`, `solid`, `at` — the original placement; `position` (x y z), `rotation` (9 numbers: the local x, y, z axes as rows) |
| `[prop.delete]` | `region`, `section`, `solid`, `at` |
| `[prop.add]` | `region`, `solid`, `position`, `rotation` |
| `[race]` | `id` (a-z 0-9 - _, unique), `name`, `track` (base track number), `backward` (yes/no: the base track's backward barriers and lanes), `mode` (circuit, sprint, streetx, drift), `laps` (1–20; 1 for sprint and drift), `checkpoint` (x y, repeated, in order, at most 64), `grid` (x y z heading, repeated, at most 8, pole first), `start` / `finish` (x y z heading) |

An original placement is named by where the original data has it: its region (`L4RA`), streaming section, solid
name and position; the placement matched is the nearest of that section and solid within 5 cm. Each original
placement may be changed by one edit per mod. Rotation rows must be unit axes (a mirrored placement, as the
race barriers have, is allowed); added placements take their lower LODs, view flags and section from the
nearest placement of the same solid (or the nearest placement for the section).

A file is refused as a whole, with its line, when the first line is wrong, its version is newer than the build
reads, a section or key is unknown, a key is missing or given twice, a number does not parse or is out of
range, a rotation is not one, a race id repeats, or a placement is changed twice. A race with fewer than 2
checkpoints is kept (a race being made) but cannot be raced. An edit naming a placement or solid the region
does not have is skipped and counted in the log.

## How it is wired

- `src/tools/map/`: the format (`format.rs`), the folders and archives (`store.rs`), the custom race's route
  (`race.rs`), and the edits without a window (`edit.rs`). `tools::map::set_only` makes one mod the only one for
  a run (`--map-mod`).
- `src/world/map_overlay.rs`: applies the edits to a loaded world. The game loads worlds through
  `tools::map::load_world` (`World::load`, then the mods for the region). The asset cache keeps the original
  world.
- `crates/ob-editor/`: the desktop editor, a crate of its own on the game library (`nfsu2`), so the game's
  dependency tree has no egui. `src/editor/` there is its library (state, undo, panels, viewport); the
  library's modules it uses are imported in its `lib.rs` under their own names, so its `crate::world`,
  `crate::render`, … paths name the game library's. `testplay.rs` handles the test hand-off: it saves, starts
  the game (`game` beside the editor; `OB_GAME_EXE` names another), polls it, reads the result line and keeps
  the tested marks. `src/bin/overbright-editor.rs` is the window. Its tests run with the workspace's
  (`cargo test`; `cargo test -p ob-editor` alone), the portable-mode test of the editor binary included
  (`crates/ob-editor/tests/portable_mode.rs`).
- The game: `shell/cli.rs` (`MapTest`: the flags, the end watch, the result line), `flow/race_setup.rs`
  (`start_map_test`, `map_race_event`), and `Game::map_race_route`, which `drive_to_grid` asks before reading a
  track's own route.
- The launcher: the Mods page's Map mods section (`launcher/screens.rs`), `Env::start_editor` and
  `dirs::editor_exe` (docs/launcher.md).
- Tests: `src/tools/map/tests.rs` (round trip, refusals, store, world and route with the game's files),
  `tools::map` `one_chosen_mod_replaces_the_enabled_list`, `crates/ob-editor/src/editor/testplay.rs` (arguments, result lines,
  tested marks, the whole hand-off with a stand-in game), `shell/cli.rs` (the flags and the run's end),
  `flow/race_setup.rs` (a test run on a custom race and from a spot, with the game's files), and
  `launcher/tests.rs` (`mods_page_starts_the_map_editor`, `the_map_editor_starts_with_the_mod_and_the_game_folder`).
