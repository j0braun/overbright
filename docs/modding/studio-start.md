# Getting started with Studio

Overbright Studio is the program for making mods: the map editor's new home, where its tools and the later
workspaces move in. In Overbright 0.5 it is a **preview**: it ships in the Linux and Windows downloads beside the
launcher, and the parts that are not ready yet are listed below. Studio never changes the game's files or your
career save.

## Starting it

- From the launcher: open the **Dev** page and press **Open Studio (preview)** under **Studio**. Studio opens on
  Home. While it is open the button is greyed out; when Studio closes, the launcher reads your map mods again.
  Under the button the launcher shows Studio's state: its version, whether it is open and on which mod (also when
  you started Studio some other way, once it has a mod open), and the last mod you opened. On the **Mods** page a
  map mod Studio has open says "being edited in Studio". The launcher only looks: it never starts Studio by itself.
- From a terminal: `overbright-studio` on Linux, `overbright-studio.exe` on Windows, from the folder you unpacked
  Overbright into. `--mod <name>` opens that map mod: its folder name in your map mods folder, or the path of the
  mod's folder (the one holding `map.obmap`). A mod outside the map mods folder opens with the mods beside it on
  Home. A name or path that is neither, or a mod that does not read, stops Studio with a message saying so.

If the launcher says Studio's program is not beside it, unpack the whole download again into one folder:
`overbright-studio` must sit next to `overbright`.

## What is in the preview

- **Home**: the open mod, how it stands and the next steps, your other mods and something new to start:
  **Empty map mod** (or File ▸ New mod…, Ctrl+N) asks for a name and makes a new mod, ready to edit;
  **Sprint race** and **Circuit race** start it with a race, and **Sample race pack** opens your own copy of three
  finished races.
  New to modding? **Start here** on Home has two tutorials. *Your first mod* (also F1) takes five steps: make a
  mod, go to a street, place a prop, drive it in the game and share it. *Make your first race* takes six: make a
  mod, choose a base track, place the start, add race points, test the race and share it.
- **World and Races**: the map editor inside Studio, on the open mod. World brings the prop tools forward, Races
  the race tools. It is the same editor `overbright-editor` opens.
- **Changes** (Ctrl+D): every value your mod changes, beside the original, and which mod wins where two give the
  same value, textures you painted included. *Reset* gives a value back.
- **Mod** (Ctrl+0): the mod's name, author and description, whether it is on in your game, what a shared copy
  holds, whether it is ready to share, and **Export** to one `.zip`.
- **Automatic saving** with a history to undo and redo: the status bar says when your work is saved.
- **The workspaces on the rail** at the left. A workspace keeps its place and its Ctrl+number.
- **Keyboard and pad**: everything works with a mouse, a keyboard or a game pad (Xbox, PlayStation, Steam Deck).

The window, its keys and its pad buttons are on the [Studio page](studio.md). A step-by-step path from install to a
shared mod is [From install to a shared mod](getting-started.md); sharing is [Share your mod as a pack](share-a-pack.md).

## The command-line tool

`overbright-studio-cli` makes the same edits from a shell or a script, with plain or JSON output. Run
`overbright-studio-cli help` for every command. The launcher's Dev page has a **Command-line tool** row whose
**Open folder** shows where it is. See [Studio command-line tool](studio-cli.md).

**Looks** (painting a game texture in your own paint program) works only through the command-line tool in this
preview: `overbright-studio-cli looks open`, `looks import` and `looks watch`. See [Studio Looks](studio-looks.md).

## Not there yet

These come in a later update:

- **Looks in the window**: a workspace for painting textures. Until then, use the command-line tool (above).
- **Settings**: the rail's Settings page is a placeholder for now.

The map editor (`overbright-editor`, [Map editor](map-editor.md)) is still in the download and edits the same map
mods.

## How it is checked

`cargo test -p ob-studio-gui studio` draws the window at every size of the resolution matrix, runs both tutorials by
keyboard and by pad, and walks Home, Changes and the Mod page; the details are on the [Studio page](studio.md#checks).

## Problems

Started from a terminal, Studio prints what went wrong there. Please report anything that looks wrong
([Troubleshooting](../troubleshooting.md) says what a report needs); it is a preview, and reports shape what comes
next.
