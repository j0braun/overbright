# Modding

Pages for modders: texture packs, the editors, Studio and the asset format (a draft). Overbright ships no replacement
art; every pack is one you add yourself.

## Texture mods

| Page | What it covers |
|---|---|
| [Texture packs](texture-packs.md) | Where packs go, the pack kinds Overbright reads, the HUD mod, the order packs apply in. |
| [TexMod (.tpf) packs](texmod-import.md) | Using `.tpf` packages made for the original: the format and the import rules. |
| [Reload Packs When Changed](pack-hot-reload.md) | See changes to a pack in the running game without a restart. |
| [Texture pack cache](texture-pack-cache.md) | How packs are compressed and cached so loads stay fast. |
| [Upscaled world texture pack](texture-pack-generator.md) | Making an upscaled pack from your own game files, on your machine. |

## Tools

| Page | What it covers |
|---|---|
| [Texture editor](texture-editor.md) | A texture browser in the game for making your own packs. |
| [Map editor and map mods](map-editor.md) | Moving, adding and removing scenery and races; map mods and how they load. |
| [Getting started with Studio](studio-start.md) | What Studio is in this preview, how to start it from the launcher, and what comes later. |
| [Overbright Studio](studio.md) | Studio's window (preview): the rail, the docks, Home, World and Races, Changes, addresses, and how it works by keyboard and by pad. |
| [Studio command-line tool](studio-cli.md) | The map editor's edits, a file of edits with a dry run and the map checks from a shell or a script, with JSON output and fixed exit codes. |
| [Studio Looks](studio-looks.md) | Paint a game texture in your own paint program: work files, each save one step of the mod's history, live in the test game. |

## Asset format (draft)

[asset-format/](asset-format/README.md): the format for base sets and mod packs. It is mostly a design: its core
library (`crates/ob-asset-format`) exists and tests the examples and schemas, Studio and its command-line tool
use it for a mod's changes and checks, and the map editor's run files ([runs.md](asset-format/runs.md)) are one
of its kinds; no program reads game data through it yet.
