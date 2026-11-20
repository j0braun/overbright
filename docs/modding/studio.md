# Overbright Studio

> **Preview.** Studio's window, `overbright-studio`, is the shell the map editor and the later workspaces move
> into. This version has the shell, Home, the map editor as the World and Races workspaces, and the Changes page.
> `overbright-editor` ([Map editor](map-editor.md)) still opens the same editor in a window of its own. For starting
> Studio and what this preview leaves out, see [Getting started with Studio](studio-start.md).

```
overbright-studio [--mod <folder>] [--maps <dir>]
overbright-studio --shot out.png [--size WxH] [--scale F] [--mod <folder>] [--maps <dir>]
                  [--workspace home|world|races|mod|settings|changes] [--input mouse|keyboard|xbox|playstation|deck]
                  [--focus rail|address|toolbar|left|view|right|bottom] [--drawer] [--find] [--address]
                  [--select N]
```

`--shot` draws the whole window off-screen into a PNG (no window opens) and prints `studio layout: fits …` or
`studio layout: clipped: …`. The mods are the map editor's (`--maps`, default the map mods folder). `--select N`
selects placement N in World. The game folder is `NFSU2_ROOT`, else the one the launcher kept.

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
| Status bar | Left: whether the work is saved ("Saved automatically", "Saving is behind", a disk problem in red) and the last step to undo. Right: problems, and whether the mod is on in your game. |

The window is at least 960 × 600 points; below that it shows one line, "Studio needs at least 960 × 600". The UI
scale grows with the window: 1.25 from 1000 lines, 1.5 from 1400, 2 from 2000 (times the system's own scale),
never so far that the window falls under its smallest size. The Steam Deck's 1280 × 800 is the smallest size every
walkthrough is checked at. Panel sizes and folds are kept per workspace in the mod's editor folder
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
button tests the race.

## Home

The open mod's name; a card that says how it stands, with the next steps (Go to World, Make a race); and rows for
its changes, whether it is in your game, how it is saved and whether this window may change it (another program
that has the mod open keeps it; Studio then only looks). Beside them: your mods and something new to start. All of
it is read from the mod's history, as the program that writes it has it.

## Changes

Ctrl+D (or `ob://<mod id>/@changes`) opens the Changes page over World; Esc goes back to where it was opened. It
lists every value the open mod gives, grouped by item, each with where the value in your game comes from, drawn
as a mark: a ring for the original game's, a lime dot for yours, a pink diamond for another enabled mod's that is
above yours in your game (the mods apply in their enabled order; a mod that is off is shown as if turned on, on
top). Beside each value is the original. A value another mod beats also shows yours and why it is not used.

- **This mod** (`@changes`): your game without the mod → with it. *Show* keeps everything you change, only what
  loses, or every value of the items you change.
- **History step** (`@changes?step=<n>`): what one step of the mod's history changed, before → after.
- **Compare versions** (`@changes?version=<name>`): a kept version → now.
- **Clashes** (`@changes?mode=clashes`): the values another mod gives too, as the stack in your game's order with
  the winner last.

A mode is only shown when it has something to show. *Reset* gives one value back: a moved prop's position goes
back to its original place; a heading (whose original is in the game's files), a hidden prop, an added prop or a
race takes the edit out. ↑ ↓ (the D-pad) move between rows; Shift+↓ or Enter (A) picks several, and Reset in the
row's menu (Shift+F10, X) takes them back in one step. Every Reset is a step of the mod's history: Ctrl+Z takes it
back. *Show in World* and *Show in Races* go to the item's address. The rows are the asset format's layering
answer (`ob_studio::changes`): each prop edit is a placement item, each race a race item, merged through the
enabled mods with the pack of every field.

## Keyboard and pad

Studio has seven areas, always in this order: rail, address bar, toolbar, left panel, view, right dock, bottom
dock. F6 and Shift+F6 (the pad's RT and LT) step through the areas that are shown: a folded panel is skipped, a
drawer counts as its dock and opens when it gets focus. The focused area has a lime line along its top. Esc (B)
goes one level out: menu, field, row, area, and on a page the back link; it never leaves the workspace.

The pad's buttons do the same as in the launcher and the game. The keyboard uses Studio's own keys, because
letters are the tools' keys and typing here (the launcher keeps Q E, Space and R).

| Pad (Xbox · PlayStation · Deck) | Keys | Does |
|---|---|---|
| A · ✕ · A | Enter | Select |
| B · ○ · B | Esc | Back, one level out |
| X · □ · X | Shift+F10, Menu key | The focused thing's menu |
| Y · △ · Y | Ctrl+K | Find: names, commands, places, pasted addresses |
| LB RB · L1 R1 · L1 R1 | Ctrl+PgUp, Ctrl+PgDn; Ctrl+1 … Ctrl+0 | Workspace |
| LT RT · L2 R2 · L2 R2 | Shift+F6, F6 | Area |
| D-pad | Tab, arrows | Move focus; ◀ ▶ change a value |
| Menu · Options · Menu | F5 | Test |
| View · Create · View | Alt+← (Alt+→ forward) | Last place |
| Sticks | Right mouse button + WASD | Move and look in the 3D view |
| Deck L4 R4 | Ctrl+Z, Ctrl+Y | Undo, redo |
| – | Ctrl+D | Changes |

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

Back and forward (Alt+← / Alt+→, the mouse's side buttons, the ← → buttons, the pad's View button) walk the last
50 places of the window. An address Studio cannot follow (a mod you do not have, a screen this version lacks) is
not followed silently: the window says why.

## Checks

`cargo test -p ob-studio-gui studio` draws the window headless at every size of the resolution matrix (and at
display scales 1.5 and 2), in every workspace, for the mouse, the keyboard and a pad, and fails on any part
outside the window, two parts overlapping, a view or an address under its smallest size; it walks the window by
keyboard and by pad, and reads addresses back. `studio::world_tests` draws World and Races with the map editor at
the four review sizes (1920 × 1080, 1280 × 800, 1024 × 768, 2560 × 1080) with the same check and the editor's
toolbar check, and walks into and out of World by keyboard and pad.
