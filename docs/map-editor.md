# Map editor and map mods

The map editor is an opt-in modding tool: it changes the city's props and makes races of your own on the
game's road network. Everything it saves is a **map mod** in Overbright's own data folder, applied on top of
the original data when a world loads. The game's files are never written.

With no map mod enabled and without `--editor`, nothing here runs: worlds and races are the original's.

## Turning it on

Start the game with `--editor` (`game --editor`). In free roam, **F8** opens the editor; the world pauses
under it (as under the pause menu) and is drawn from the editor's free camera. Esc (or F8) saves the mod
when it changed and goes back to driving. Without `--editor`, F8 does nothing.

## Controls

Both pages:

| Key | Does |
|---|---|
| W A S D, R / F | Fly forward / left / back / right, up / down (Shift: faster) |
| Mouse drag, I / K, J / L (Props page), numpad 8 2 4 6 | Look around |
| Tab | Props page / Races page |
| Ctrl+S | Save the mod |
| F5 | Save, then load the world again with the mod (added props are drawn after this) |
| G | Put the car at the aim point (crosshair), facing the camera's way, and close |
| H | Hide / show the panel |
| Esc, F8 | Save and close |

Props page (the crosshair aims):

| Key | Does |
|---|---|
| Space, Enter | Pick the prop under the crosshair (small props only: not roads, terrain or building shells) |
| Arrows, PgUp / PgDn | Move it 0.25 m along the ground / up and down (Shift: 2 m) |
| Q / E | Turn it 15° (Shift: 90°) |
| Del, Backspace | Delete it |
| C | Add a copy of the picked prop's solid at the aim point (drawn after F5) |
| Z | Undo this mod's change of the picked prop (exact after F5) |

Races page:

| Key | Does |
|---|---|
| N | New race (a sprint) whose start is the aim point, on the nearest race track's road network |
| [ / ] | Previous / next race of the mod |
| P | What Enter edits: checkpoints, start grid, start line, finish line |
| , / . | Previous / next checkpoint or grid slot |
| J | Fly the camera to the selected point |
| Enter | Add a checkpoint or grid slot after the selected one at the aim point, or set the start / finish line there (heading: the camera's) |
| Del | Remove the selected checkpoint, grid slot or line |
| Arrows | Move the selected point (0.5 m, Shift 4 m) |
| M | Race mode: circuit, sprint, Street X, drift |
| + / - | Laps (circuits and Street X) |
| T | Save and race it |
| Ctrl+X | Delete the race |

The race page draws the checkpoints (numbered), the numbered grid slots, the start and finish lines and the
road the race will run (dots), and says whether a route exists.

## What a custom race is

A custom race runs on a **base track**: one of the game's race tracks of the region. The base track gives the
road network (`TRACKS/ROUTES<region>/Paths<track>.bin` covers the whole region), its race barriers (the
roads closed for it), the AI racing lanes, traffic, the opponents and the race set-up of a career event on
that track of the same race mode. The mod gives the checkpoints, the start grid (8 slots behind the start line
when none are placed), the start and finish lines, the mode and the laps. The route is built through the
checkpoints on the road network exactly as the game builds a track's own route (`RoadNet::route`, 0x5E5BC0);
a checkpoint leg with no road through the open network (a closed barrier in the way) is shown and the race
cannot be started. A circuit closes back to its first checkpoint; a sprint ends at its finish line, else at
its last checkpoint. Checkpoints taken from a track's own route build that track's road (test
`a_custom_race_on_native_checkpoints_is_the_native_road`).

Custom races are raced from the editor (T). They do not appear in the career or the Quick Race lists, and
their results do not count toward the career.

## Limits

- Collision of the city's static scenery is baked per section and not tied to the placements: a moved or
  deleted building keeps its collision where the original has it, and an added copy of one has none.
  Knock-over props (cones, signs, barrels, …) carry their own: moved ones are hit where they now stand,
  added copies of one are knock-over props too, deleted ones are gone.
- Added props are drawn once the world is loaded again (F5, or the next load of the region).
- Shops, event markers, traffic lanes and AI lanes are not editable yet.
- Drag races are not offered (their lanes are fixed to the drag tracks).

## Where map mods live

| | |
|---|---|
| Mods | `<Overbright data>/maps/<folder>/map.obmap` |
| Mods in use | `<Overbright data>/maps/enabled.txt`, one folder name a line, applied in that order |

`<Overbright data>` is the folder of the README's "Where your files go" (`data/` in portable mode). The editor
edits the last mod listed in `enabled.txt`, or makes `my-map` and lists it. A mod that cannot be read is
skipped with its reason in the log; the others still apply. The list and the mods are read at the first world load of a run (a change made outside the game applies after a restart); the editor's saves refresh them. To share a mod, copy its folder or zip it; to
import one, copy the folder into `maps/` and add its name to `enabled.txt`. `nfsu2::tools::map::store` has
`import` (a folder, a `map.obmap` or a `.zip` holding it; validated before anything is written, never over an
existing folder) and `export` (a folder or a `.zip`) for the launcher.

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
  (`race.rs`), the editor's edits without the window (`edit.rs`).
- `src/world/map_overlay.rs`: the edits applied to a loaded world. The game loads worlds through
  `tools::map::load_world` (`World::load`, then the enabled mods for the region); the asset cache keeps the
  original world, and the world's loaded state is taken again after the mods so a kept world's reset keeps them.
  The renderer builds its instances from the placements after that, so added placements need no renderer
  change; the editor's moves and deletes reach it through `World::take_changed` and `update_placement`.
- `src/bin/game/enh/map_editor/`: the editor in the game (keys, camera, panel through `tools::overlay`), and
  `Game::map_race_route`, which `drive_to_grid` asks before reading a track's own route.
- Tests: `src/tools/map/tests.rs` (round trip, refusals, store, world and route with the game's files) and
  `src/bin/game/tests/map_editor.rs` (the editor unreachable without `--editor`, its edits and save, a
  custom race on its own route).
