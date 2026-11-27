# Share your mod as a pack

Overbright never hosts or shares mods. You make one file and give it to people yourself. You are responsible for
what you share.

## What you can share

A map mod (props, races and the textures it paints) leaves Studio as one file, in one of two forms.

| Form | Made by | Read by |
|---|---|---|
| `.zip` | Studio's Mod page, `overbright-studio-cli pack export`, the map editor, the launcher's **Export…** | The launcher's **Import a map mod…**, the map editor's Import, Studio |
| `.obpk` (a mod pack) | The launcher's **Export a map mod…** on the Mods page | The launcher's **Import…** under **Mod packs** |

Both hold the same files: `map.obmap`, the mod's texture list and `textures/…`, and `provenance.toml`. Your
editor's work files (`work/` and every name starting with a dot) are never in them.

## Before you export

1. Open the mod in Studio and press Ctrl+0.
2. Fill in **Name**, **Author** and **Description**. Enter opens a field, Enter keeps it, Esc leaves it.
3. Read **Ready to share**. A red line stops the export: a map check that fails, a missing name, or a texture that
   is not in the size the game reads. A mod with races also needs the game's race tracks.
4. A warning about **Made from the original game** means some texture is built from the original game's pixels.
   Do not share those unless you may. Studio asks before it exports them.

## Export a `.zip` from Studio

1. On the Mod page, check the **Export to** name. Studio proposes `<folder>.zip` in the `exports` folder of
   Overbright's data folder ([Where Overbright keeps its files](../playing/platform-dirs.md)).
2. Press **Export**. The note under the button says where the file went.

From a shell: `overbright-studio-cli pack export --mod <dir> --to <file.zip>` ([command-line tool](studio-cli.md)).

## Export a `.obpk` from the launcher

1. Open the launcher's **Mods** page, then the **Mod packs** part.
2. Press **Export a map mod…** and choose the mod.
3. The file is written as `<id>-<version>.obpk`. A pack made this way carries the base set it was made for. A
   player whose base set differs sees which one it needs, and the pack is not mounted.

## Add a shared mod

Send the file to a friend. They add it in the launcher, on the **Mods** page:

- A `.zip`: **Import a map mod…**, then turn the mod on with its switch.
- A `.obpk`: **Import…** under **Mod packs**. A pack file also shows in the map mod import's file list.

Studio never turns a mod on by itself. Packs apply in the order the launcher shows, the first line the strongest.
**Move up** and **Move down** change it. If two mods give the same value, Studio's Changes page shows who wins
([Overbright Studio](studio.md#changes)).

## Credits

If your mod uses art or other assets that someone else made, list them in a file named `credits.toml` in the mod's
folder, one table per asset:

```toml
[[credit]]
name = "Brick wall texture"
by = "Someone"
licence = "CC-BY-4.0"
source = "https://example.org/bricks"
```

`name` and `by` are needed; `licence` and `source` are optional. The file travels with the mod in a `.zip`, and an
export or conversion to a `.obpk` writes the credits into the pack's `provenance.toml`, so they stay with it. A texture
pack that is converted adds its own author as the credit for its images by itself. The launcher's **Look inside** and
the edit report show the credits, next to the authors in the pack's manifest.

## What a pack changes

A pack's **Look inside** on the Mods page, and `overbright-studio-cli modpack report --pack <file or id>`, say in words
what it changes compared with the game: its props (each moved, removed or added), races, collision edits and the
textures it replaces, its credits, which game copy it was made for, and what it overrules among the packs that are
on. A pack made by Overbright 0.7 or newer also lists a fingerprint for each item, so the report says which items were
changed, are missing or were added since it was made ([manifest.md](asset-format/manifest.md) §4.2); older packs have
none and are read as before. Nothing is switched on or written by it, so you can read the report of a pack file before you add it.

A pack that changes races, props or collision carries the **affects play** tag (its manifest's `affects_play`, which
the exporter computes). Saves made while it is on remember that, in a small file of Overbright's own beside the
profiles' folder; if you load such a save later with the pack off or changed, you are told, and the save itself is
never changed. A pack that only changes how things look is tagged **looks only** and saves do not depend on it.

When a pack is made for another install than yours, each edit that names a prop this install does not have is left
out on its own, with the reason in the log, and never put on the nearest other prop.

## If a pack is refused

The launcher shows a reason under the pack's name and does not mount it. The usual reasons: it was made for
another Overbright version, it needs another base set, or a pack it depends on is missing or off. The rules are
on [layering](asset-format/layering.md).

## For developers

The pack file format is [layout.md](asset-format/layout.md) and the manifest is
[manifest.md](asset-format/manifest.md).
