# Getting started with Studio

Overbright Studio is the program for making mods: the map editor's new home, where its tools and the later
workspaces move in. In Overbright 0.5 it is a **preview**: it ships in the Linux and Windows downloads beside the
launcher, and the parts that are not ready yet are listed below. Studio never changes the game's files or your
career save.

## Starting it

- From the launcher: open the **Dev** page and press **Open Studio (preview)** under **Studio**. Studio opens on
  Home. While it is open the button is greyed out; when Studio closes, the launcher reads your map mods again.
- From a terminal: `overbright-studio` on Linux, `overbright-studio.exe` on Windows, from the folder you unpacked
  Overbright into. `--mod <folder>` opens that map mod.

If the launcher says Studio's program is not beside it, unpack the whole download again into one folder:
`overbright-studio` must sit next to `overbright`.

## What is in the preview

- **Home**: the open mod, how it stands and the next steps, your other mods and something new to start.
- **World and Races**: the map editor inside Studio, on the open mod. World brings the prop tools forward, Races
  the race tools. It is the same editor `overbright-editor` opens.
- **Changes** (Ctrl+D): every value your mod changes, beside the original, and which mod wins where two give the
  same value. *Reset* gives a value back.
- **Automatic saving** with a history to undo and redo: the status bar says when your work is saved.
- **The workspaces on the rail** at the left. A workspace keeps its place and its Ctrl+number.
- **Keyboard and pad**: everything works with a mouse, a keyboard or a game pad (Xbox, PlayStation, Steam Deck).

The window, its keys and its pad buttons are on the [Studio page](studio.md).

## The command-line tool

`overbright-studio-cli` makes the same edits from a shell or a script, with plain or JSON output. Run
`overbright-studio-cli help` for every command. The launcher's Dev page has a **Command-line tool** row whose
**Open folder** shows where it is. See [Studio command-line tool](studio-cli.md).

**Looks** (painting a game texture in your own paint program) works only through the command-line tool in this
preview: `overbright-studio-cli looks open`, `looks import` and `looks watch`. See [Studio Looks](studio-looks.md).

## Not there yet

These come in a later update:

- **The Mod page** (export and sharing). Until then, export a map mod as a `.zip` on the launcher's **Mods** page.
- **Looks in the window**: a workspace for textures. Until then, use the command-line tool (above).
- **A tutorial** that walks you through a first mod.

The map editor (`overbright-editor`, [Map editor](map-editor.md)) is still in the download and edits the same map
mods.

## Problems

Started from a terminal, Studio prints what went wrong there. Please report anything that looks wrong
([Troubleshooting](../troubleshooting.md) says what a report needs); it is a preview, and reports shape what comes
next.
