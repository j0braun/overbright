# Asset format: layering (base set, packs, order, merge, clashes)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md)
apply.

Prior art taken: USD's layer stack with "strongest opinion wins" and per-field opinions, its `over` (change what
exists) versus `def` (define something new) and "inactive" instead of delete
(https://openusd.org/release/glossary.html#usdglossary-layerstack); OpenMW's and Godot's explicit,
deterministic mount order instead of directory order; Factorio's dependency rules (`!` incompatible, `?`
optional) (https://wiki.factorio.com/Tutorial:Mod_structure#dependencies). Avoided: Bethesda-style whole-record
overrides, whose untouched copies of base values silently revert other mods ("dirty edits",
https://tes5edit.github.io/docs/7-mod-cleaning-and-error-checking.html).

## 1. The layer stack

```text
  top     order.txt line 1         (strongest)
          order.txt line 2
          …
          dependencies, always below their dependents
  bottom  the base set             (weakest; exactly one)
```

- **"Use mods" off** (the launcher's switch): the stack is the base set alone, and `order.txt` is not read. With a
  base set imported from the player's own copy, this is the original game, proven by the parity gates
  ([parity.md](parity.md)).
- A test or Studio run passes an explicit stack (`--base <id> --mods a,b`); it then ignores `active.txt` and
  `order.txt`.
- Readers build the merged view per kind, **after decoding and before the cache** (the order the map overlay
  already uses, `src/world/map_overlay.rs`). The cache key of merged data is defined in [cooked.md](cooked.md) §3:
  for every path looked up, the ordered layers and digests that contributed or "not found", validated against the
  effective stack; the stack order itself is not part of the key.

## 2. `order.txt`

```text
# Overbright mods v1
docks-remix
hd-hud
-old-hud
```

- First line exactly `# Overbright mods v1` (`ORD.1` otherwise). Then one pack id per line, strongest first. A
  leading `-` keeps the pack in its place but switched off. `#` lines are comments; blank lines are ignored.
- A pack present in `mods/` but not listed is **off** and shown as new. Nothing is enabled behind the player's back.
- An id listed twice, or a listed id with no pack, is reported and skipped (the file is not refused: the player's
  order survives a removed pack).
- Writers keep the player's comments and the order of untouched lines.
- Dependencies: the launcher keeps each pack's `[dependencies]` below it and enabled, moving them on enable
  and saying so; a stack in which a dependency is above its dependent, missing or off is refused with one line
  ("docks-remix needs hd-hud below it") (`ORD.2`). `[conflicts]` pairs are never both on (`ORD.3`). A pack whose
  dependency is refused for any reason is refused too, and the message names the dependency that caused it
  (`ORD.4`).

**What the game does with it** (implemented, `crates/ob-asset-format/src/modpack.rs`, `src/mod_packs/`). At start the
game reads `mods/order.txt` (no file, or no pack on: nothing else is read, the game is the original's), checks each pack
that is on (the `overbright` range `MAN.3`, `game.bases` and `game.fingerprint` against the base set in use, `residual/`
and `import.toml` `MRG.6`, dependencies and conflicts `ORD.2`–`ORD.4`) and mounts the rest, the first line the strongest.
A pack that is refused is not mounted at all and the log and the launcher say why ("made for the base set nfsu2-pc12-eu;
the one in use is …"). A pack made from a Studio map mod holds the kinds as their own files: `world/<REGION>/placements.toml`
(`ob.placements`), `races/<id>.toml` (`ob.race`) and image-only texture entries (`world/_any/textures/images/0x<HASH>.png`,
`cars/_any/textures/images/…`); the static collision edits, which have no kind file yet, stay in a `map.obmap` that holds
nothing else. An older pack with its whole map in `map.obmap` and `textures/world|car/…` still reads, and a converted texture
pack's `global/textures/images/` folders are read by renderer (§7). The game reads them above the plain map mods and
reports clashes between packs (§4) in its log and in the launcher's Mods page. A `.obpk` is unpacked once into the cache,
keyed by its digest.

## 3. Merge rules

Every kind page defines its items, their keys, and which of the operations below it allows. The rules here are the
same for every kind.

### 3.1 Operations on an item

A pack file entry for an item carries an optional `op`:

| `op` | The key below this layer | Effect | Absent fields |
|---|---|---|---|
| `"merge"` (default) | MUST exist (`MRG.1`) | Fields present replace the lower layer's fields | keep the lower layer's values |
| `"add"` | MUST NOT exist (`MRG.2`) | Defines a new item | take the schema's defaults; required fields MUST be present (`CONV.5`) |
| `"replace"` | MUST exist (`MRG.1`) | The whole item is this entry | take the schema's defaults, **not** the lower values |

and an optional `disabled = true | false`: a disabled item stays in the merged data with its key and its position
but the engine treats it as absent in the way its kind page defines (a placement is not drawn, though static
collision baked into the section stays until collision deltas exist, [collision.md](collision.md); a knock-over
prop loses its own; a sound plays as silence of its original length so the event player's state and random draws
do not change). Each kind page says whether `disabled` is allowed at all (car types refuse it: the engine's tables
are indexed by their position). Base items are never deleted; a pack disables them. A higher layer MAY set
`disabled = false` again.

Why `merge` must find its item: a pack made against another base set (another edition, a community set) would
otherwise quietly create half-defined items. It is refused with the item's key instead.

**Pack-scoped keys.** An item whose key contains the pack's own id (an added placement `L4RA/docks-remix:a12`, a
custom race `race:docks-remix:sprint`) can only be defined by that pack, so an entry that carries the pack's own
local id defaults to `op = "add"`; `merge` / `replace` of it are allowed only from a *higher* pack that lists it in
`[dependencies]` (`ORD.2`). Kinds whose added items are new **names** (strings, fonts, presets, recordings) follow
§3.1 as written; the new name's hash must not collide (`CONV.15`). Added lists and banks are named `<pack id>:<stem>`
so they never share a key with a base list or another pack's.

**Several files of one kind.** Where a kind allows more than one override file in one folder (career.md does),
they apply in byte order of their file names, so the result never depends on the file system's listing order.

### 3.2 Fields

- **Scalars and fixed arrays** (`vec3`, `mat3`, `color`): replaced whole.
- **Sub-tables**: merged field by field, recursively, with the same rules.
- **Lists**: replaced whole by default. A field table MAY declare a list **keyed** (by a field of its elements);
  its elements are then merged like items: an element with `op = "add"` is appended, one with a known key is merged
  or replaced, `disabled` hides it. The page states the list's key and where added elements go. Where the
  original's elements have no key of their own, a **base** element's key is its **`index`** in the base set's list
  (never shifting: base elements are never removed, only disabled), and an element a pack adds is keyed
  `<pack id>:<n>` (its position among that pack's own additions), addressable only by that pack and by packs that
  depend on it. Every element of an item a pack adds (or replaces whole) is that pack's addition. `op = "add"` without a key appends. An index into the *merged* list is never a key: it would shift
  when a lower pack is switched off or reordered.
- **Opaque fields** (`u_*`, `bytes`, external arrays) are replaced whole.
- **`revert`** (reserved): an entry field `revert = ["field", …]` that sets the listed fields back to the **base
  set's** value, whatever lower packs did (the role of CSS's `revert-layer`, https://www.w3.org/TR/css-cascade-5/),
  so a higher pack can undo a lower pack's change without pinning a number that would be wrong on another base set.
  A kind adopts it in a minor version by adding the field to its entry schema, only for fields packs may merge and
  only on items that have a base value; in draft 1 no kind has adopted it yet, so packs write the value explicitly.
- Writers SHOULD drop a field whose value equals the lower layers' merged value (Studio does this on save), so a
  pack holds only real changes. Readers MUST NOT treat an equal value differently from an absent one, except for
  clash reporting (§4).

### 3.3 Order of items

Where items have an order ([conventions.md](conventions.md) §6.5), the merged order is: the base set's items in
their original order, each kept in place whatever is merged into it or disabled; then items added by packs, the
lowest layer's first, each layer's in the order of its files and entries. Adding never shifts an original item's
position, so original keys and the original's order-dependent behaviour stay as they are.

### 3.4 Keys of added items

Items a pack adds are keyed by the pack: the kind's key with the pack id as its namespace (e.g. a placement
`docks-remix:a12`, [world.md](world.md)), or a new name for named kinds. New names follow
[conventions.md](conventions.md) §6.4 (no hash collisions with other names of the kind in the stack).

### 3.5 Unit wildcards

Some overrides apply wherever an item appears (a texture hash used in every region). A kind MAY allow an override
file in a wildcard unit: `world/_any/…` stands for every `world/<REGION>/`, `scenes/_any/…` for every
`scenes/<NAME>/`, `cars/_any/…` for every
`cars/<NAME>/`. Wildcard entries are weaker than unit-specific entries **of the same pack** and follow the stack
order otherwise. The kind page says whether it allows wildcards ([textures.md](textures.md) does).

## 4. Clashes

A **clash** is two mod packs giving a value for the same item field (or both adding the same key, or one
disabling what another changes). The stronger layer wins; the clash is never an error but is always shown:

- per item and field, with both packs' values, in the launcher's Mods page and Studio's Pack panel;
- `overbright-studio-cli pack clashes --json` lists them for scripts.

The merged view also says, per item, which pack each field's value comes from (an element field of a keyed list
included, `grid[docks-remix:1].heading_deg`; a field no pack gave has the base set's value), so Studio's inspector
can show where a value comes from; an element's packs are those of its fields
(`crates/ob-asset-format/src/layering.rs`, `Item::from`).

Base set versus pack is not a clash. A pack's own wildcard entry versus its own specific entry is not a clash.

## 5. What a mod pack may not do

- Change the base set's files, or contain data copied or derived from the player's game data, unless its manifest
  says `origin = "from-game-files"` and `share = "local-only"` ([manifest.md](manifest.md) §3). The pack check
  (`MRG.5`) refuses, in a pack not so marked, any file — or any payload array or byte range inside one — whose bytes
  equal an original file, a residual node, or a file, array or payload of any imported base set on this machine,
  and any file Studio's journal records as copied or derived from base-set content (an exported texture, a
  converted recording). It catches copies and what Studio derived; it cannot catch content transformed outside
  Overbright, which is why the manifest's `origin` is the author's own declaration and Studio never offers an
  un-marked export of base-set content.
- Contain `residual/` or `import.toml` (`MRG.6`). Only an imported base set has them.
- Execute code. Packs are data only; there is no script kind.

## 6. Checks at merge time

<!-- generated:rules -->
| Rule | Text |
|---|---|
| ORD.1 | `order.txt` without its first line, or of an unknown version. |
| ORD.2 | A dependency missing, off, or above its dependent; or a pack that merges or replaces an item another pack added without listing that pack in `[dependencies]`. |
| ORD.3 | Two packs listed in each other's `[conflicts]` are both on. |
| ORD.4 | A dependency of this pack was refused (named), so this pack is refused too. |
| MRG.1 | `merge` or `replace` of an item that no lower layer has. |
| MRG.2 | `add` of an item a lower layer already has (use `merge`). |
| MRG.3 | An operation the kind does not allow (e.g. `add` of a car before add-on cars exist). |
| MRG.4 | A keyed-list element with an unknown key and no `add`. |
| MRG.5 | In a pack not marked `from-game-files` / `local-only`: a file, array or byte range equal to an original file, a residual node or a base-set file or payload, or a file the journal records as derived from base-set content. |
| MRG.6 | A mod pack with `residual/` or `import.toml`. |
| CONV.15 | (from [conventions.md](conventions.md) §11) A new name colliding by hash with another name of the same kind. |
<!-- /generated -->

A refused pack is not mounted at all ([conventions.md](conventions.md) §11).

## 7. Today's mod registries as legacy packs

Until they are converted (by the player, through the launcher, once), today's registries are mounted as virtual
packs without moving or rewriting a file, in the order the launcher shows them today:

| Today | Mounted as | Notes |
|---|---|---|
| Overbright texture packs (`packs/<id>/`, `pack.toml` format 1, `packs/order.txt`) | mod pack `<id>`, texture overrides in `global/textures` with the renderer filters that reproduce today's domains exactly ([textures.md](textures.md) §4) | format-1 manifest fields map 1:1 ([manifest.md](manifest.md) §6) |
| Imported TexMod / TexWizard packages | mod pack per package | `origin` from the import record |
| The HUD mod | mod pack `hud-mod`, `origin = "from-game-files"`, `share = "local-only"` | its textures become texture overrides; its map calibration rows become merges of the track-map records ([career.md](career.md)) |
| Upscale / HD UI art (`texture-pack/`) | mod pack `upscale-art`, `origin = "from-game-files"`, `share = "local-only"` | |
| Map mods (`maps/<folder>/map.obmap`, `maps/enabled.txt`) | mod pack per folder: placements as `world/<REGION>/placements.toml` entries (`ob.placements`), `[race]` sections as `races/` | placement references (stream section, solid name, position within 5 cm; `nth` only in the map editor's next format-1 revision) resolved as in [world.md](world.md) §4.4 |

**Conversion in the tools** (`overbright-studio-cli modpack convert`, `src/mod_packs/convert.rs`): a plain map mod (folder or
`.zip`) is written as above; an Overbright texture pack of format 1 becomes `global/textures/overrides.toml` with one entry per
image (`draw` as the table says, `compress = "none"` from the manifest) and its images under `global/textures/images/<world|car|any>/`;
the game reads those folders by renderer (its own domain first, then `any/`). The provenance carries `converted = true` and
`converted_from`. A converted texture pack is `origin = "from-game-files"`, `share = "local-only"`: where its pixels came from is
not known. Files the pack has no place for (any non-image file) are listed and left out; a name that is no pack id, two
images for one texture in one folder, or a folder with neither kind of content are refused. A placement written from a map mod
carries the map mod's reference (`extras.legacy`: stream section, solid, position, `nth`) and no `key`: resolving the
`<scenery>/<index>` key needs the base set's placement table, so a placement named only by that key is refused by the game
until the resolver exists (the pack is not mounted and the log says which entry).

The launcher offers "Convert to the new format" per pack; conversion writes a format-2 folder next to the old one,
keeps the old one as a backup, and is lossless (a test reads both and compares the merged data).

**One load path** (`src/mod_packs/stack.rs`): the game reads every mod kind's textures through one function,
`mod_packs::texture_layers`, which returns the layers strongest first, each with its id and kind: the packs switched on
in `order.txt` and the plain map mods (the packs above the map mods), the Overbright texture packs, the texture packages
the launcher keeps, the World Texture Pack row's imported and generated folders; the HUD mod is the layer the loader
falls back to when no other is in use. Map data comes through `mod_packs::mount_all` (the plain map mods weakest, then
the packs). Each kind keeps its own list and its own order among its layers, so a player's existing mods read exactly as
before; with none in use the stack is empty and nothing is read or written. A converted pack (§7 above) is a layer like
any other.

## 8. Saves and the layer stack

Profile saves are not assets: they keep the original game's own format, and the original game can load them
(docs/formats/saves-options.md). So a pack that affects play ([manifest.md](manifest.md) §3, `affects_play`) never
puts its content into a profile save:

- References to pack content (a pack's car, part, race or event) go into a **sidecar** file beside the profile; the
  profile itself keeps a value valid for the original (a stock part, the last stock car).
- The sidecar records the stack the save was made with (pack ids, versions, content digests) and the rename
  migrations already applied ([versioning.md](versioning.md) §4.3).
- A save loaded without a pack it needs shows "needs <pack>" and deletes nothing. With "Use mods" off the sidecar is
  not read.

How it is done (`mod_packs::sidecar`): the sidecar is `<data home>/NFS Underground 2 Mods/<name>.toml` (lower case
name), beside the profiles' folder and never inside it (the memory-card list counts the profiles' folder's files, and
the original reads that folder). Each profile save writes it while a pack is on, with each mounted pack's id, version,
`affects_play` and, for a pack file, its digest; a save made with no pack on removes it, and a player with no pack
never gets the file or the folder. A profile load reads it: an `affects_play` pack that was on at the last save and
is off, not installed, or another version or content now is reported in the log ("this save was made with the mod pack
X 1.0.0, which is off now (nothing was changed in the save)"); a looks-only pack is not. `affects_play` is the
manifest's value or, when the pack's map data holds placements, races or collision edits, true (the stricter value).
A trial run's copy (Try it once) has its own sidecar folder in its data home, so the player's own is not touched.
