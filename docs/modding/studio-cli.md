# Studio command-line tool

`overbright-studio-cli` does what the map editor does, from a shell or a script: every edit is a command with
plain or JSON output and a fixed exit code, a file of edits can be checked with a dry run and then applied as one
step, and the map checks run from the command line. It is a console program on Linux and Windows, so a shell
waits for it and reads its output.

```
overbright-studio-cli help                                   # every command, flag and exit code
overbright-studio-cli help race point add                    # one command's flags
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
| `props move` | edit | Move a prop (`--key`, `--position`, `--rotation`) |
| `props delete` | edit | Hide an original prop or remove an added one |
| `props reset` | edit | Put an original prop back as the original has it |
| `props add` | edit | Add a prop of one of the region's objects (`--solid`, `--position`, optional `--rotation`, `--id`, `--ground`) |
| `props duplicate` | edit | Copy a prop to a new place |
| `props drop` | edit | Stand a prop on the floor under it |
| `race create` | edit | Make a race on a base track (`--track`, optional `--kind`, default sprint, and `--id`) |
| `race delete` | edit | Delete a race |
| `race set` | edit | Change a race's `--name`, `--id`, `--track`, `--kind`, `--laps` or `--backward`; several at once are one step |
| `race point add` | edit | Add a checkpoint or grid slot, or place the start or finish line (`--part`, `--position`, optional `--index`, `--heading`) |
| `race point move` | edit | Move a race point to `--position`, or a checkpoint one place `--later` (or `--later=false`: earlier) in the route |
| `race point delete` | edit | Delete a race point (`--part`, optional `--index`) |
| `race put` | edit | Put a whole race in place (`--to`, the race as JSON) |
| `pack details` | edit | Change the mod's `--name`, `--author` and `--description` (those left out stay) |
| `pack put` | edit | Put the whole map in place, from a map file (`--file`) |
| `pack repair` | edit | Repair a `map.obmap` that no longer reads, where a safe repair exists (a sprint or drift race with more than one lap runs its one lap), as one step |
| `edit group` | edit | Several edits as one step (`--label`, `--ops`: the edits as a JSON list) |
| `ops apply` | edit | Apply an operations file as one step (`--file`, optional `--label`) |
| `looks open` | run | Open a texture in your own paint program: writes its work file (`--texture world:0x1A2B3C4D`; the game's pixels, or `--blank` for an empty canvas) ([Studio Looks](studio-looks.md)) |
| `looks import` | edit | Take a texture's work file in now, as one step (`--texture`) |
| `looks watch` | run | Watch the work files and take each save in as one step (optional `--seconds`; default until Ctrl+C) |
| `looks list` | read | The mod's changed textures and work files, with where their pixels started |
| `checks run` | read | Run the map checks on the mod |
| `history show` | read | The mod's kept steps, with their authors |
| `schema export` | read | The kinds' and commands' schemas |

Flags every command takes:

| Flag | Meaning |
|---|---|
| `--mod DIR` | The mod's folder (the one holding `map.obmap`). |
| `--game DIR` | The game folder; by default the one the launcher set. Prop edits and the checks read it. |
| `--region NAME` | The region the mod is edited on (default `L4RA`). |
| `--dry-run` | Show what an edit would change and change nothing. |
| `--json` | Print one JSON document on stdout. |
| `--wait SECONDS` | How long to wait for Studio's answer when Studio has the mod open (default 10). |
| `--no-inbox` | When another program has the mod open, fail at once instead of asking it. |
| `--help` | The command's help. |

`overbright-studio-cli --version` prints the version line, as the game, the launcher and the editor do.

Values: `--flag value` or `--flag=value`. Numbers as usual; a position as `x,y,z` or a JSON list; a rotation as
nine numbers (its three rows) or a JSON list of rows; a yes/no flag alone means yes (`--ground`), `--ground=false`
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

`looks open` gives `texture`, `name`, `work_file`, `origin` (`game` or `blank`) and `derived`. `looks import`
gives `outcome` (`made` with its `step`, `unchanged`, or `sent` with the inbox `request` when Studio has the mod
open). `looks watch` gives `saves`, one per save taken in, with its `outcome` (or `refused` and the `reason`).
`looks list` gives `textures` (each with `texture`, `name`, `origin`, `derived`, `size`, `image`, `pack_file`)
and `work`.

## Exit codes

| Code | Kind | Meaning |
|---|---|---|
| 0 | `ok` | Done. |
| 1 | `failed` | The command failed: an edit refused, a conflict, a file that does not read. |
| 2 | `usage` | The command line is wrong: an unknown command or flag, a missing or bad value, an operations file line that is not an edit. |
| 3 | `checks` | The checks found an error. |
| 4 | `no_game` | The game's data is missing or does not load (give `--game`). |
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
game folder (exit 4 when it is not known or does not read; a mod without races needs no game). The exit code is 3
when a check finds an error, so a script or a build can stop on it.

## For developers

The tool is `crates/ob-studio/src/bin/overbright-studio-cli.rs` over `ob_studio::cli` (no window: it links Studio's
operation layer only). `crates/ob-studio/tests/cli.rs` runs it on mods written in the test and compares each JSON
document with its kept copy in `crates/ob-studio/tests/cli/` (`OB_STUDIO_BLESS=1` rewrites them; review the diff);
it also checks that every registered command is on the command line and documented here, and that every exit code
is in the table above. Every edit the map editor makes goes through its JSON and back before it runs
(`ob_studio::session::through_json`), so a request JSON cannot carry fails the editor's tests.
