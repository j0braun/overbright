# Overbright Studio

> **Preview.** Studio's window, `overbright-studio`, is the shell the map editor and the later workspaces move
> into. This version has the shell, Home, the map editor as the World and Races workspaces, the Changes page and the
> Mod page.
> `overbright-editor` ([Map editor](map-editor.md)) still opens the same editor in a window of its own. For starting
> Studio and what this preview leaves out, see [Getting started with Studio](studio-start.md).

```
overbright-studio [--mod <name|folder>] [--maps <dir>]
overbright-studio --shot out.png [--size WxH] [--scale F] [--mod <folder>] [--maps <dir>]
                  [--workspace home|world|races|mod|settings|changes] [--input mouse|keyboard|xbox|playstation|deck]
                  [--focus rail|address|toolbar|left|view|right|bottom] [--drawer] [--files] [--warning]
                  [--find] [--address] [--select N] [--row N] [--places] [--job WORDS]
                  [--right-tab inspector|history|help] [--new-mod NAME] [--tutorial N] [--race-tutorial N]
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
job, `--right-tab` picks the right dock's tab. The game folder is
`NFSU2_ROOT`, else the one the launcher kept.

## The window

| Part | What it is |
|---|---|
| Menu bar | File, Edit, View, Go, Test, Mod, Help; back and forward (← →); the address bar; Find; the open mod's name with its lamp. Under 1100 points wide, Test and Mod sit in Go and File. |
| Rail | Home at the top, then the workspaces that make things (MAKE) in the order they shipped, then Mod (SHARE) and Settings at the bottom. With names and Ctrl+number at 850 or more points high, icons only below (the name shows on hover). A workspace never moves and its Ctrl+number is never given to another. |
| Left panel | The workspace's own (World: the Mod tree). 270 points (230 under 1400 wide), dragged between 180 and 40 % of the width; folds to a 28-point strip under 1400 wide. |
| Right dock | Inspector, History, Help. 320 points (300 under 1400 wide), 260 to 40 %; under 1180 wide a 28-point strip that opens a 340-point drawer over the view (Esc or B closes it). |
| Bottom dock | Problems, Log, Jobs, then the workspace's tabs. 20 % of the height (120 to 240); folded to its tab strip under 700 points high. It spans the view's width only. |
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
tabs (Prop browser, Validation, Simulate) after Problems, Log and Jobs, the hint line under the view, and the 3D
view or the map drawn in the window by the game's own renderer. World brings the prop tools forward, Races the race
tools; the editor's menu items join Studio's menus (Edit, View, Test: New race, Test race, Drive here). Problems
lists the mod's warnings and errors; Log is the editor's log.

The editor's keys work as in its own window (Q W E tools, Ctrl+Z / Ctrl+Y, F5 Test race, Shift+F5 Drive here,
RMB + WASD to fly). While the view, the tree or the bottom dock has focus, Enter, Esc, Tab and the arrows are the
editor's; F6 and the pad's triggers still move between areas. The pad's L4 / R4 (Deck) undo and redo, the Menu
button tests the race. Studio keeps a few keys for itself: Ctrl+D opens Changes (the editor's Duplicate is in Edit
and the prop menu) and Ctrl+N opens Studio's New mod dialog.

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

## Changes

Ctrl+D (or `ob://<mod id>/@changes`) opens the Changes page over World; Esc goes back to where it was opened. It
lists every value the open mod gives, grouped by item, each with where the value in your game comes from, drawn
as a mark: a ring for the original game's, a lime dot for yours, a pink diamond for another enabled mod's that is
above yours in your game (the mods apply in their enabled order; a mod that is off is shown as if turned on, on
top). Beside each value is the original. A value another mod beats also shows yours and why it is not used, and
**Show in Clashes** in place of Reset.

- **This mod** (`@changes`): your game without the mod → with it. *Show* picks **Everything this mod changes**,
  **Only what loses** or **All values** (every value of the items the mod changes).
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

The command-line tool exports the same way: `overbright-studio-cli pack export --mod <dir> --to <file.zip>`
(`--derived-ok` for textures from the original game's pixels; [Studio command-line tool](studio-cli.md)).

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
| D-pad | Tab, arrows | Move focus; ◀ ▶ change a value |
| Menu · Options · Menu | F5 | Test |
| View · Create · View | Alt+← (Alt+→ forward) | Last place |
| Sticks | Right mouse button + WASD | Move and look in the 3D view |
| Deck L4 R4 | Ctrl+Z, Ctrl+Y | Undo, redo |
| – | Ctrl+D | Changes |
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
open, Last place and Next place when there is one, Address, Copy address, Changes, New mod, Tutorial, Undo, Redo) and the places you were, narrowed by what is typed. ↑ ↓ (the D-pad)
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
the four review sizes (1920 × 1080, 1280 × 800, 1024 × 768, 2560 × 1080) with the same check and the editor's
toolbar check, and walks into and out of World by keyboard and pad. `studio::guide_tests` runs both tutorials by
keyboard and by pad on a stand-in city and makes the templates and the sample from Home; `studio::mod_page_tests`
keeps a detail as one step, exports a mod with derived textures and imports it again; `studio::changes_tests`
resets values and textures as single undoable steps and walks the rows by keyboard and pad.
