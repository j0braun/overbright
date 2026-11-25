# Overbright documentation

Start with the part that fits what you want to do. Each folder has its own index page.

## Playing

Getting the game running, the settings, and what differs from the original. See [playing/](playing/README.md).

- [Launcher](launcher.md): find your game, check it, choose mods and start.
- [Troubleshooting and bug reports](troubleshooting.md): what to do when the game does not start or stops,
  and how to send a support file.
- [Differences from the original game](playing/native-deviations.md): every place Overbright plays or looks
  different, and the switch that turns it off.
- [Optional graphics settings](playing/settings/README.md): one page per feature on the Enhancements page.
- [Controllers](playing/controller-support.md), [screen resolutions](playing/resolutions.md),
  [Windows](playing/platform-windows.md), [where your files go](playing/platform-dirs.md).

## Modding

Texture packs, the texture editor, the map editor, Overbright Studio and the asset format (a draft). See
[modding/](modding/README.md).

- [Texture packs](modding/texture-packs.md): add HD textures and the HUD mod.
- [TexMod packs](modding/texmod-import.md): use `.tpf` packages made for the original.
- [Texture editor](modding/texture-editor.md) and [map editor](modding/map-editor.md).
- Overbright Studio (preview), the map editor's new home: [getting started](modding/studio-start.md),
  [its window](modding/studio.md), [its command-line tool](modding/studio-cli.md) and
  [Looks](modding/studio-looks.md) (painting a game texture in your own paint program).
- [Asset format (draft)](modding/asset-format/README.md): the format for base sets and mod packs; Studio and
  the map editor's run files use its core library already.

## Building and testing

How to build Overbright, test a change and make a release. See [developing/](developing/README.md).

- [BUILDING.md](../BUILDING.md) and [ARCHITECTURE.md](../ARCHITECTURE.md) at the top of the repository.
- [Developer guide](developing/developer-guide.md): tools, diagnostic switches, performance notes.
- [Testing](developing/testing.md) and [the checks](developing/checks.md) every change runs.
- [Proving a simulation fix against the original](developing/parity-workflow.md).
- [How releases are built](developing/building-releases.md).
- [Writing the docs](STYLE.md).

## How the original works

What is known about `SPEED2.EXE` and its files, with addresses. See [original/](original/README.md).

- [The address map](formats.md): the traced behaviour, split into [chapters](formats/).
- [Names for addresses](symbols.tsv) and the [provenance tags](provenance.tsv) of measured values.
- [Uncapped frame rates](original/uncapped-fps.md), [random numbers](original/rng.md),
  [save points](original/save-points.md), [outruns](original/outrun.md) and more.
- [Coverage maps](original/coverage/README.md): how much of the original's traffic and racer AI
  Overbright reproduces, function by function.

## Notes

Investigation write-ups: comparisons with the original, measurements and how each fix was found. They are
grouped by topic. See [notes/](notes/README.md).

- [Parity](notes/parity/README.md): side-by-side checks against the original (races, driving, world systems).
- [Physics](notes/physics/README.md), [outruns](notes/outrun/README.md), [graphics](notes/graphics/README.md),
  [audio](notes/audio/README.md), [performance](notes/performance/README.md),
  [menus and shops](notes/front-end/README.md).
- Data behind the notes: [evidence/](evidence/) and the port lists in [native-fixes/](native-fixes/).
- Pages that moved in the folder sort: [notes/MOVED.tsv](notes/MOVED.tsv) (old path, new path).
