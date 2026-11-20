# Overbright Studio

> **Preview.** Studio's window, `overbright-studio`, is the shell the map editor and the later workspaces move
> into. This version has the shell and Home; World and Races show where the map editor's tools will be. Until they
> move in, edit maps with `overbright-editor` ([Map editor](map-editor.md)). It is not in the release package yet.

```
overbright-studio [--mod <folder>] [--maps <dir>]
overbright-studio --shot out.png [--size WxH] [--scale F] [--mod <folder>] [--maps <dir>]
                  [--workspace home|world|races|mod|settings] [--input mouse|keyboard|xbox|playstation|deck]
                  [--focus rail|address|toolbar|left|view|right|bottom] [--drawer] [--find] [--address]
```

`--shot` draws the whole window off-screen into a PNG (no window opens) and prints `studio layout: fits …` or
`studio layout: clipped: …`. The mods are the map editor's (`--maps`, default the map mods folder).

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

## Home

The open mod's name; a card that says how it stands, with the next steps (Go to World, Make a race); and rows for
its changes, whether it is in your game, how it is saved and whether this window may change it (another program
that has the mod open keeps it; Studio then only looks). Beside them: your mods and something new to start. All of
it is read from the mod's history, as the program that writes it has it.

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

Every button is one press; a chord is never the only way. Help ▸ Controls lists the same table, made from the key
table in `crates/ob-studio-gui/src/studio/nav.rs`.

## Addresses

The address bar shows names ("Harbour edits › World"). Ctrl+L shows the address itself, selected, to copy or to
paste another: Enter goes there, Esc returns to the names. Ctrl+Shift+C copies it.

- An item: `ob://<mod id>/<kind>/<key>[#<field>][?guard=<value>]`, as the asset format names items, e.g.
  `ob://harbour-edits/placement/L4RA/1407/12#position`.
- A screen: `ob://<mod id>/@<screen>[?name=value…]`, e.g. `ob://harbour-edits/@world`; Studio's own screens use
  `studio` (`ob://studio/@home`). A screen's name starts with `@`, which no kind name can.

Back and forward (Alt+← / Alt+→, the mouse's side buttons, the ← → buttons, the pad's View button) walk the last
50 places of the window. An address Studio cannot follow (a mod you do not have, a screen this version lacks) is
not followed silently: the window says why.

## Checks

`cargo test -p ob-studio-gui studio` draws the window headless at every size of the resolution matrix (and at
display scales 1.5 and 2), in every workspace, for the mouse, the keyboard and a pad, and fails on any part
outside the window, two parts overlapping, a view or an address under its smallest size; it walks the window by
keyboard and by pad, and reads addresses back.
