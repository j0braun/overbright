# Studio command-line tool

`overbright-studio-cli` does what the map editor does, from a shell or a script: every edit is a command with
plain or JSON output and a fixed exit code, a file of edits can be checked with a dry run and then applied as one
step, and the map checks run from the command line. It is a console program on Linux and Windows, so a shell
waits for it and reads its output.

```
overbright-studio-cli help                                   # every command, flag and exit code
overbright-studio-cli help race point add                    # one command's flags
overbright-studio-cli pack create --name "Docks remix"       # a new mod in the map mods folder
overbright-studio-cli race set --mod maps/docks --race loop --laps 3 --dry-run
overbright-studio-cli ops apply --mod maps/docks --file edits.jsonl --label "Night version"
overbright-studio-cli checks run --mod maps/docks --json
```

## Commands

A command's words are its stable name with spaces (`race point add` is `race.point.add`). The list, each
command's flags, their types and which are needed come from Studio's command registry, the same one the editor
and its schema export (`crates/ob-studio/schema/studio.json`) use, so `help` is always complete.

| Command | Kind | What it does |
|---|---|---|
| `props move` | edit | Move a prop (`--key`, `--position`, and `--heading` or `--rotation`) |
| `props delete` | edit | Hide an original prop or remove an added one |
| `props reset` | edit | Put an original prop back as the original has it |
| `props add` | edit | Add a prop of one of the region's objects (`--solid`, `--position`, optional `--heading` or `--rotation`, `--id`, `--ground`) |
| `props duplicate` | edit | Copy a prop to a new place |
| `props drop` | edit | Stand a prop on the floor under it |
| `collision remove` | edit | Take out a wall of the original's static collision (`--target '{"original": [[x, y, z], ...]}'`, its corners as the collision view's Inspector shows them) or a drawn wall or ground (`--target '{"drawn": "wall-1"}'`) |
| `collision wall` | edit | Draw a wall from `--from` to `--to` (each `[x, y, z]` on the road), `--height` metres tall, with surface id `--surface` (optional `--id`) |
| `collision ground` | edit | Draw ground over `--outline` (3 to 32 points `[[x, y, z], ...]`), with surface id `--surface` (optional `--id`) |
| `race create` | edit | Make a race on a base track (`--track`, optional `--kind`, default sprint, and `--id`) |
| `race delete` | edit | Delete a race |
| `race set` | edit | Change a race's `--name`, `--id`, `--track`, `--kind`, `--laps` or `--backward`; several at once are one step |
| `race point add` | edit | Add a checkpoint or grid slot, or place the start or finish line (`--part`, `--position`, optional `--index`, `--heading`) |
| `race point move` | edit | Move a race point to `--position`, or a checkpoint one place `--later` (or `--later=false`: earlier) in the route |
| `race point delete` | edit | Delete a race point (`--part`, optional `--index`) |
| `race put` | edit | Put a whole race in place (`--to`, the race as JSON) |
| `pack create` | run | Make a new map mod with nothing changed yet (`--name`; optional `--folder`, else made from the name, and `--maps`, the mods folder, default the map mods folder; `--region` is the region it is edited on; `--template sprint`, `circuit` or `sample` starts it with a sprint along the docks, a three-lap circuit downtown or the sample race pack, as Home's templates do) |
| `pack details` | edit | Change the mod's `--name`, `--author` and `--description` (those left out stay) |
| `pack put` | edit | Put the whole map in place, from a map file (`--file`) |
| `pack export` | run | Export the mod as one file to share (`--to` a `.zip`, or a new folder), without its work files; textures made from the original game's pixels need `--derived-ok` ([Studio](studio.md#the-mod-page)) |
| `modpack export` | run | Export the mod as a mod pack file, `<id>-<version>.obpk` (`--to` a `.obpk` or a folder; optional `--version`, default 1.0.0, `--licence` as an SPDX expression, else the pack stays on this computer; `--derived-ok` as for `pack export`). Its placements, races and textures are the asset format's own files ([layering](asset-format/layering.md#7-todays-mod-registries-as-legacy-packs)) and the provenance is the launcher export's |
| `modpack convert` | run | Convert a plain map mod or an Overbright texture pack (format 1: `world/`, `car/`, `any/`; `--from` a folder or `.zip`) to a mod pack file (`--to`; optional `--id`, `--version`, `--licence`, `--derived-ok`). The provenance says `converted`; what the pack format has no place for is listed (`left out`), a folder that is neither kind is refused. A converted texture pack stays on this computer. Needs no `--mod` |
| `pack repair` | edit | Repair a `map.obmap` that no longer reads, where a safe repair exists (a sprint or drift race with more than one lap runs its one lap), as one step |
| `edit group` | edit | Several edits as one step (`--label`, `--ops`: the edits as a JSON list) |
| `ops apply` | edit | Apply an operations file as one step (`--file`, optional `--label`) |
| `looks open` | run | Open a texture in your own paint program: writes its work file (`--texture world:0x1A2B3C4D`; the game's pixels, or `--blank` for an empty canvas) ([Studio Looks](studio-looks.md)) |
| `looks import` | edit | Take a texture's work file in now, as one step (`--texture`) |
| `looks watch` | run | Watch the work files and take each save in as one step (optional `--seconds`; default until Ctrl+C) |
| `looks list` | read | The mod's changed textures and work files, with where their pixels started |
| `props solids` | read | The region's placeable solids, as the prop browser lists them (`--filter` words of the name, `--category` knock-over, barriers, signs, street, trees or all; default all), with where they are placed; needs the game |
| `race tracks` | read | The region's base tracks a race can be on, as the race panel lists them (number and name); needs the game |
| `looks find` | read | Find the region's world textures by name words or hash: `looks find road` or `--text 0x1A2B3C4D`; gives each texture's id for `looks open`, name and size; needs the game |
| `collision show` | read | What the World view's collision layer shows in `--mode` `off`, `invisible` (invisible walls and the race and career barriers) or `all` (default; also the walls behind scenery and the floors), counted as the window builds it; with `--mod` the mod's removed and drawn collision applies; needs the game |
| `runs list` | read | The mod's kept runs (simulations and recordings), newest first, as the Runs workspace lists them |
| `runs show` | read | Open a run (`--run`): its race, seed, how it ended, the results and the problems it found |
| `runs telemetry` | read | One car's telemetry in a run (`--run`, optional `--car`, default the player's slot 0): speed over time, distance from the racing line when the mod has the run's race, its events |
| `runs compare` | read | Compare a run with another (`--run`, `--with`, optional `--car`): the tick its state hashes part and when the car is over a metre from where the first has it |
| `runs rename` | run | Rename a run (`--run`, `--to`); never over another run |
| `runs delete` | run | Delete a run file (`--run`); there is no dry run and no undo |
| `data kinds` | read | The kinds the Data grid shows (placement, race) with their columns and row counts |
| `data show` | read | One kind's table as the Data grid shows it (`--kind`; optional `--search` words, `--filter column=text`, `--sort column`, `--desc`), each value with where it comes from (original, yours, another mod) |
| `data set` | edit | Write one cell as the grid does (`--kind`, `--row` the row's key as `data show` gives it, `--field` the column, `--value` as typed into the cell): one undo step, with a request, a diff and `--dry-run` like any edit |
| `browser search` | read | Search the region's models and textures as the Browser workspace does: words of the name, a hash or `section:N` (after the command or `--text`), `--kind` all, models or textures, `--category` for models, `--textures` any, in-use, unused, animated or large; needs the game |
| `browser used` | read | Where a model or texture is used: a model's placements, sections and textures, a texture's models and sections; needs the game |
| `game find` | read | The game folder the tool uses and where it came from: `--game`, `NFSU2_ROOT` or the launcher's choice (exit 4 when none, or it does not exist) |
| `checks run` | read | Run the map checks on the mod |
| `history show` | read | The mod's kept steps, with their authors |
| `schema export` | read | The kinds' and commands' schemas |

Flags every command takes:

| Flag | Meaning |
|---|---|
| `--mod DIR` | The mod's folder (the one holding `map.obmap`). |
| `--game DIR` | The game folder. Without it the tool looks where the editor and the game look: `NFSU2_ROOT` when it is set, else the folder chosen in the launcher. Prop edits and the checks read it. |
| `--region NAME` | The region the mod is edited on (default `L4RA`). |
| `--dry-run` | Show what an edit would change and change nothing. On a command that is not an edit it is a usage error. |
| `--json` | Print one JSON document on stdout. |
| `--wait SECONDS` | How long to wait for Studio's answer when Studio has the mod open (default 10). |
| `--no-inbox` | When another program has the mod open, fail at once instead of asking it. |
| `--help` | The command's help. |

`overbright-studio-cli --version` prints the version line, as the game, the launcher and the editor do.

Values: `--flag value` or `--flag=value`. Numbers as usual; a position as `x,y,z` or a JSON list; a rotation as
nine numbers (its three rows) or a JSON list of rows; a prop's `--heading` in degrees as the Properties panel shows
it (around +z from +x, positive to the left seen from above): a moved prop keeps its tilt (as Heading in
Properties does), an added one stands upright; `--heading` and `--rotation` together are a usage error; a yes/no flag alone means yes (`--ground`), `--ground=false`
no. A prop's `--key` is `added:<id>` for a prop the mod added, or the key as JSON for an original one
(`{"original": {"region": "L4RA", "section": 1407, "solid": "…", "at": [x, y, z]}}`, as `history show` and the
editor's operations files write it). An unknown command or flag, a missing argument or a value of the wrong type is
a usage error (exit 2), never ignored.

## Output

Without `--json` the tool prints a line or a short list for people; errors go to stderr. With `--json` it prints
one JSON document on stdout and nothing on stderr. Every document has `"format": 1` (the document's version; new
fields may be added, existing ones keep their meaning), `command`, `ok` and `exit` (the exit code). A failure adds
`error` with `kind` (the word in the table below), `message` and, for a refused edit, `reason`
(`no_race`, `no_placement`, `id_taken`, `bad_value`, …) and, when the tool can say more about a bad value,
`detail` (`a sprint race runs one lap`, `rotation rows must be unit axes`). A mod whose `map.obmap` does not read
fails every command with the line and the reason; when a safe repair exists the error also holds `repair` (the
command and the `fixes` it makes).

An edit's document holds:

- `request`: the edit as JSON, exactly what the map editor sends for the same change (`{"op": "set_laps",
  "race": "sprint", "laps": 3}`). It can be pasted into an operations file.
- `label`: the step's name, as the editor's Edit ▸ Undo shows it; `change`: what the edit did (the race or point
  it made or moved, the props it added).
- `outcome`: `made` (with `step`, the history step, and `via`: `file` or `inbox`), `unchanged` (the mod already
  was so; nothing is written) or `dry_run`.
- `diff`: `items`, each changed item of the mod (its details, one prop edit, one race) as `added`, `removed` or
  `changed` with its value before and after, and `lines`, the file's changed lines with two lines around each
  (` ` same, `-` removed, `+` added, `@@` lines skipped).

`checks run` gives `errors`, `warnings` and `checks`: each with `level` (`error`, `warning`, `info`), `passed`,
`text`, the `race` it is about, the `point` and the place (`at`, x and y) the editor's Show goes to, and the `fix`
the editor offers. `history show` gives `step` (the current one), `steps` and `undone` (each with `step`,
`author`, `label`, `time_ms`), `versions` and `unsaved`.

`pack create` gives `name`, `folder`, `dir` (the new mod's folder, for `--mod`), `region` and `template`. It never writes over
a mod: a folder in use (`error.reason` `folder_used`) or a name another mod has (`name_used`) fails with exit 1;
no name (`no_name`), a name over 60 characters (`long_name`), a folder name with other characters than a-z, 0-9,
`-` and `_` (`bad_folder`) or a bad `--region` (`bad_region`) is a usage error (exit 2).

`props solids` gives `region`, `count` and `solids` (each `name`, `knock_over`, `placed`, `categories`); `race tracks`
gives `region`, `count` and `tracks` (`number`, `name`); `looks find` gives `region`, `text`, `count` and `textures`
(`texture`, `name`, `width`, `height`); `game find` gives `game` and `source` (`--game`, `NFSU2_ROOT` or `launcher`).
The first three read the same lists the window shows (the prop browser, the race panel's tracks, Looks' textures), so
the same words give the same answer. On a near miss (`--filter barier`, `looks find raod`, `--category barier`,
`--region L4RB`, a command or `--template` with a typo) the answer says "Did you mean": plain text on the line after,
`did_you_mean` in the JSON for the lists, and in the message of a usage error. The list commands need no `--mod`.

`collision show` gives `region`, `mode`, `label`, `faces_read`, `counts` (each `what` the mode lists and its `count`) and
`all` (`invisible`, `barriers`, `visible`, `floors`, `floors_not_ground`: every kind whatever the mode).

`runs list` gives `count` and `runs` (`run`, `what`: the race's id or `world`, `seed`, `end`, `bytes`, `time_ms`). `runs show` gives
`what`, `seed`, `end`, `span`, `frames`, `cars`, `standings` (`car`, `name`, `place`, `time`, `best`, `laps`, `dnf`) and
`problems`. `runs telemetry` gives `speed` and, when the mod has the run's race, `lateral` (each `samples`, `min`, `max`,
`mean`), the car's `events` and the whole `series` (time, value). `runs compare` gives `same`, `hash_tick`, `time` and
`apart`. A run that is not there fails with `error.reason` `no_run`, a car it does not have with `no_car`.

`data kinds` gives `kinds` (each `kind`, `rows`, `columns`). `data show` gives `kind`, `total`, `count`, `columns` (`field`,
`label`, `type`, `editable`) and `rows`: each `key` and `cells` by column, each cell `text`, `from` (`original`, `yours` or
`mod <name>`), `yours` and `original` (the original's value when it differs). A cell the format cannot write yet, or a
row another mod owns, is refused by `data set` with `error.reason` `bad_value` and the grid's words in `error.detail`.

`browser search` gives `region`, `text`, `count`, `found` (`models` and `textures` the words find at their widest) and `hits`
(a model's `name`, `hash`, `placed`, `shown`, `sections`, `triangles`, `textures`, `used`; a texture's `name`, `texture`
(its Looks id), `width`, `height`, `animated`, `models`, `materials`, `used`). `browser used` gives `found`: those hits with
a model's `placements`, `as_lod`, `section_list` and `texture_list`, or a texture's `model_list` and `section_list`.

`pack export` gives `to`, `files`, `bytes`, `derived` (the textures from the original game's pixels) and
`build`; refused for derived textures, its `error.reason` is `derived` and `derived` lists them.

`modpack export` gives `to`, `id`, `files`, `bytes` and `build`, and refuses derived textures like `pack export`
(`error.reason` is `derived`). `modpack convert` gives the same plus `kind` (`map mod` or `texture pack`) and `skipped`.

`looks open` gives `texture`, `name`, `work_file`, `origin` (`game` or `blank`) and `derived`. `looks import`
gives `outcome` (`made` with its `step`, `unchanged`, or `sent` with the inbox `request` when Studio or the map
editor has the mod open: that program makes the step and, when it next saves the mod, writes `looks.toml` and the
texture under `textures/`, as `looks import` does itself when nothing else has the mod open). `looks watch` gives `saves`, one per save taken in, with its `outcome` (or `refused` and the `reason`).
`looks list` gives `textures` (each with `texture`, `name`, `origin`, `derived`, `size`, `image`, `pack_file`)
and `work`.

## Same as the window

The window's commands and the command line's are the same functions: the Runs list, Data grid, Browser index and
collision layer the window draws are read from the code the commands call (`ob_studio::runs`, `data`, `browser`,
`collision_view`). A test (`crates/ob-studio-gui/src/studio/parity_tests.rs`) lists the window's workspaces, menu
actions, menu commands and key table and fails when one has neither a counterpart here nor a reason to be
window-only. What stays in the window only: the camera, layout and what is drawn (View, Frame, Reset camera), focus,
areas, places and dialogs, the Test and Drive here buttons that start the game on the open mod (the tool has no
game window), Undo and Redo (a command is one step of the same history, `history show`; an edit is taken back by
the edit that reverses it), Save (a command's step is written at once), opening a mod (a command names `--mod`) and
Studio's settings.

## Exit codes

| Code | Kind | Meaning |
|---|---|---|
| 0 | `ok` | Done. |
| 1 | `failed` | The command failed: an edit refused, a conflict, a file that does not read. |
| 2 | `usage` | The command line is wrong: an unknown command or flag, a missing or bad value, an operations file line that is not an edit. |
| 3 | `checks` | The checks found an error. |
| 4 | `no_game` | The game's data is missing or does not load (give `--game`, set `NFSU2_ROOT`, or choose the game folder in the launcher). |
| 5 | `crashed` | The game, run as a child, crashed (for the commands that run it; none yet). |
| 6 | `timed_out` | The program that has the mod open did not answer within `--wait`. |
| 7 | `busy` | Another program has the mod open and `--no-inbox` was given. |
| 130 | `cancelled` | Cancelled (Ctrl+C). |

## Edits and the history

An edit runs on the mod as the map editor would run it, and the result becomes one step of the mod's history
(`.editor/history.journal`) under the author `overbright-studio (command line)`; the mod's file is written at once.
Studio's history shows the step, and Edit ▸ Undo takes it back like any other.

- **Dry run** (`--dry-run`): the edit is made on a copy and shown; nothing is written, not even the editor folder.
  It reads the mod as its writer has it now, unsaved steps included.
- **Operations file** (`ops apply --file F`): one edit per line, each the JSON of an edit (the `request` the tool
  prints, or a line of the editor's `ops.jsonl`); empty lines and lines starting with `#` are skipped. All of its
  edits are one step, named by `--label` (else after the file); if one is refused, none is made. A line that is
  not an edit is a usage error naming the line.
- **Only files that read**: every edit is checked against the map file's rules before it becomes a step (a sprint
  or drift race runs one lap, a rotation's rows are unit axes, …), and a file is written only when it reads back,
  so no command leaves a mod that no longer opens. A dry run makes the same checks. A mod an older tool left
  unreadable this way is repaired with `pack repair` (one undoable step; `--dry-run` shows it first).
- **Studio has the mod open**: only one program writes a mod at a time. The tool then puts its change into the
  mod's inbox (`.editor/inbox/`); Studio makes it as one step at its next look and answers, and the tool prints
  that answer. A change made against the mod as the tool read it is merged item by item: if Studio changed the same
  item meanwhile, the answer is a conflict (exit 1) and nothing is made. When no answer comes within `--wait`, the
  tool exits 6; the request stays in the inbox and is made when Studio next looks. With `--no-inbox` the tool
  changes nothing and exits 7.

## Checks

`checks run` runs the map checks the editor's Validation tab shows: per race, what keeps it from being raced
(errors), what is likely a mistake (warnings) and what is fine. They read the base tracks' road networks from the
game folder (exit 4 when it is not known or does not read; a mod without races needs no game), and the region's
world for the floor under each start line, finish line and grid slot ("has no floor under it", "is N m above the
floor"; [Map editor](map-editor.md)), and the mod's drawn and removed collision, on the region's world with that
collision applied (so a start on drawn ground has a floor). The exit code is 3 when a check finds an error, so a script or a build can
stop on it.

`race point add`, `race point move` and `race put` give a start or finish line or a grid slot the road's height
when the game is known (a point at z = 0 under the road comes up onto it); without the game the height stays as
given and `checks run` with the game reports it.

## For developers

The tool is `crates/ob-studio/src/bin/overbright-studio-cli.rs` over `ob_studio::cli` (no window: it links Studio's
operation layer only). `crates/ob-studio/tests/cli.rs` runs it on mods written in the test and compares each JSON
document with its kept copy in `crates/ob-studio/tests/cli/` (`OB_STUDIO_BLESS=1` rewrites them; review the diff);
it also checks that every registered command is on the command line and documented here (and the window's side: `parity_tests.rs` above), and that every exit code
is in the table above. Every edit the map editor makes goes through its JSON and back before it runs
(`ob_studio::session::through_json`), so a request JSON cannot carry fails the editor's tests.
