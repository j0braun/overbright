# Studio Looks

> **Preview.** This is Looks' first part: the work files, the history and the live test game, on the command line
> (`overbright-studio-cli looks …`, [Studio command-line tool](studio-cli.md)). Picking a texture in the 3D view,
> the before/after slider, where a texture is used, and the sharing warnings come in Studio's window later.

Looks lets you paint the game's textures in your own paint program. Overbright ships no texture art: the textures
you start from are your own game's, read from your game files, and what you paint stays in your mod's folder.
Looks never writes into the game's folder.

## Painting a texture

```
overbright-studio-cli looks open --mod maps/docks --texture world:0x1A2B3C4D
overbright-studio-cli looks watch --mod maps/docks
```

- **`looks open`** writes the texture's *work file*, `work/looks/world/<NAME>.png` in the mod's folder, and prints
  its path: open it in your paint program. It starts from the game's own texture, or from the mod's image when
  the mod already changes it. With `--blank` ("Repaint from blank") it starts from an empty canvas of the
  texture's size instead. A work file that exists is never overwritten: your paint is kept (and `--blank` is
  refused until you remove the file).
- **`looks watch`** watches the work files. Each save is taken in once the file has stopped changing (a program
  still writing it keeps changing it) and reads as a PNG the game takes (each side 1 to 4096): it becomes **one
  step** of the mod's history, named "Paint <NAME>", which undo takes back like any other step. A file that does
  not read yet is left until it changes again. `looks import --texture …` takes one work file in at once.
  When Studio or the map editor has the mod open, the step goes to it through the mod's inbox; it makes the step
  and writes `looks.toml` and the texture under `textures/` when it next saves the mod (about 2 s later, or on
  Ctrl+S). Undo there takes the texture away again.
- **`looks list`** shows the mod's changed textures and its work files.

A texture is named by its domain (`world` or `car`) and the game's hash of its name: `world:0x1A2B3C4D`. The
texture editor in the game (F7, [Texture editor](texture-editor.md)) shows both.

### Where the pixels came from

Each changed texture keeps where its pixels started: `game` (from the game's own texture: the result is
*derived* from the original's pixels) or `blank` (your own). Derived textures are made from the original game's art: they are
fine in your game, and Studio will warn before you share them. You are responsible for what you share; Overbright
never hosts or shares mods.

## Live in the test game

A test run started with `--looks-live <mod>/textures` shows each step in the running game within a second: the
game watches the mod's texture folder as [Reload Packs When Changed](pack-hot-reload.md) does, for that run only,
whatever the setting says, and takes those textures before every other pack. Only a test run (`--map-mod` or
`--map-file`) takes the flag; a normal start never reads the mod's textures this way, and nothing is written to
your options.

Measured headless on the reference desktop (`crates/ob-studio/tests/looks_live.rs`, a stand-in paint program, the
watcher ticked by the game's frames at 60 fps): about 0.2 to 0.4 s from the save to the frame that draws it.

## The mod's files

| Path in the mod's folder | What it is |
|---|---|
| `looks.toml` | The changed textures: per texture its id, name, `origin` (`game` or `blank`), image (by content) and size. A document of the mod's history; two programs changing two different textures both keep their change. |
| `textures/world/<HASH>.png`, `textures/car/<HASH>.png` | The pack data the game reads, written from `looks.toml`: what the list says and nothing else. |
| `work/looks/…` | Your work files. Never part of the mod you share. |
| `.editor/looks/<content>.png` | Every image a step named, by its content, so undo gets the image back. Never exported. |
| `.editor/looks-work.toml` | Which work file is which texture and where it started. |
