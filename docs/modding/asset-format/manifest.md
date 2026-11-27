# Asset format: the manifest (`pack.toml`, format 2)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md)
apply. Schema: [`schemas/pack.schema.json`](schemas/pack.schema.json).

Every pack, base set or mod pack, has one `pack.toml` at the root of its logical tree. Format 2 is a superset of
today's texture-pack manifest (format 1, `src/obpack/manifest.rs`): a format-1 file is read as described in §6.

Prior art taken: the base game as "just another pack" (Factorio's `base` mod,
https://lua-api.factorio.com/latest/auxiliary/mod-structure.html; OpenMW's root content file); a base set as a
declared set of required items with a completeness check (OpenTTD base sets,
https://github.com/OpenTTD/OpenTTD/blob/master/docs/obg_format.txt; the OpenMW game template's lesson that
engine-hard-coded asset names make replacement sets painful,
https://openmw.readthedocs.io/en/latest/reference/modding/openmw-game-template.html); an engine version range per
pack (Minecraft `min_format`/`max_format`, https://minecraft.wiki/w/Pack.mcmeta; VCMI `compatibility`).

## 1. Example (mod pack)

```toml
format = 2
kind = "mod"
id = "docks-remix"
name = "Docks remix"
version = "1.2.0"
authors = ["A. Modder"]
licence = "CC-BY-4.0"
overbright = ">=0.5"
description = "New cones and a sprint through the docks."
homepage = "https://example.org/docks-remix"
origin = "own-work"
share = "public"
contents = ["world", "races", "textures"]
affects_play = true

[game]
bases = ["nfsu2-pc12-*"]

[dependencies]
cone-pack = ">=1.0"

[optional]
hd-hud = "*"

[conflicts]
old-docks = "*"

[[tested]]
race = "docks-sprint"
race_digest = "xxh3-128:5c1f0e6a9d3b4f2c8e7a1b0d9c6f3e21"
car = "PEUGOT"
time_s = 83.41
overbright = "0.5.0"
```

## 2. Example (base set made by the importer)

```toml
format = 2
kind = "base"
id = "nfsu2-pc12-eu"
name = "Need for Speed Underground 2 (PC 1.2, European), imported"
version = "1.0.0"
licence = "LicenseRef-player-copy"
overbright = ">=0.5"
origin = "imported"
share = "local-only"

[base]
edition = "pc12-eu"
region = "eu"
languages = ["english", "french", "german", "italian", "spanish", "dutch", "swedish", "danish"]
profile = "nfsu2/1"
import = "import.toml"
```

## 3. Fields

<!-- generated:fields pack -->
| Field | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `format` | u32 | yes in format 2 | – | 2 or 3. A writer declares the lowest format that reads its file (3 only when `items` is used; versioning.md §1). A reader refuses a higher format (`MAN.1`). A file without `format` is read as a format-1 manifest by the legacy rules of §6, not as a format-2 file missing a field. |
| `kind` | enum `base`, `mod` | yes | – | What the pack is ([layering.md](layering.md) §1). |
| `id` | packId | yes | – | Unique id (conventions §6.1). For a folder pack it SHOULD equal the folder name; for `<id>-<version>.obpk` it MUST equal the file name's id part (`MAN.2`). |
| `name` | string ≤ 80 | yes | – | Display name. |
| `version` | semver | yes | – | The pack's own version (https://semver.org/spec/v2.0.0.html). |
| `authors` | array<string ≤ 80>, ≤ 32 | no | `[]` | Format 1's `author` is read as one entry. |
| `licence` | string ≤ 200 | yes for `share = "public"` | – | An SPDX licence expression (https://spdx.org/licenses/); `LicenseRef-…` for others. `license` is accepted as a spelling. |
| `overbright` | versionReq | yes | – | The Overbright versions that can load it; a reader outside the range refuses with "made for Overbright <range>" (`MAN.3`). |
| `description` | string ≤ 2000 | no | `""` | Plain text, shown as text, never as markup. |
| `homepage` | string (https URL) ≤ 300 | no | – | A link shown in the launcher; never fetched by Overbright. |
| `origin` | enum | yes | – | `own-work`; `from-game-files` (computed from the player's game data: upscaled textures, copied records); `port:<game>` (from another game the player owns); `imported` (a base set made by the importer). |
| `share` | enum `public`, `local-only` | yes | – | `local-only` is forced for `from-game-files`, `port:*` and `imported` (`MAN.4`); Studio's exporter refuses to mark them public. |
| `contents` | array<enum> | no | computed | The kind families the pack holds (`textures`, `meshes`, `world`, `collision`, `routes`, `races`, `cars`, `career`, `ui`, `text`, `audio`, `movies`, `tables`, `anim`); shown as chips in the launcher. Writers compute it; a reader warns when it differs from the files. |
| `affects_play` | bool | no | computed | `true` when any file can change simulation, AI or career (world placements with collision, routes, races, cars, career, tables). Saves and replays record such packs ([layering.md](layering.md); the save sidecar, [layering.md](layering.md) §8). Writers compute it; a reader recomputes and uses the stricter value. |
| `priority` | i32 | no | 0 | Format 1 only: where a new pack is first placed among unlisted packs. Ignored once the pack is in `order.txt`. |
| `extensions_used` | array<string> | no | `[]` | Extensions the pack's files use (conventions §9). |
| `extensions_required` | array<string> | no | `[]` | Subset of `extensions_used` a reader MUST know (`CONV.14`). |
| `extensions_safe_to_copy` | array<string> | no | `[]` | Subset of `extensions_used` whose data a tool that does not know them may keep when it rewrites an item (conventions §9); an extension not listed is not safe to copy. |
| `items` | logicalPath | no | – | Format 3. The pack's per-item fingerprints: the file (normally `items.toml`) listing every other file of the pack with its `xxh3-128` digest, so a changed, missing or added item is named exactly (§4.2). Absent in older packs, which load and try out as before; only in format 3 (`MAN.10`). |
| `[game]` | table | no | – | What the pack was made against. |
| `game.bases` | array<string> | no | `["*"]` | Base set ids it supports, `*` wildcards allowed (`nfsu2-pc12-*`). The game refuses a pack whose list has no match for the base set in use, or that names a base set when no imported copy is in use, and says which it was made for; a pack that is on but refused is not mounted at all. Any reference or guard that does not match refuses it too (conventions §11: the whole pack, with the stale items listed). |
| `game.fingerprint` | array<sha256> | no | – | Digests that identify the copy of the game the pack was made for (`sha256:<hex>`, the import record's `source.identity`; Overbright's exporter writes it beside `bases`). A non-empty list the base set in use is not on is refused like a base set mismatch. Format-1 files may list digests of key original files, which are read and shown. |
| `[dependencies]` | table packId → versionReq | no | – | Required packs, enabled and below this one ([layering.md](layering.md) §2, `ORD.2`). |
| `[optional]` | table packId → versionReq | no | – | If present and enabled, kept below this one; never required. |
| `[conflicts]` | table packId → versionReq | no | – | Never enabled together (`ORD.3`). |
| `[[tested]]` | array of tables | no | – | One entry per tested race, written by Studio after a finished test run: `race` (the race id), `race_digest` (the race digest of [roads-ai.md](roads-ai.md) §3.7, followed by the content digests of the pack's play-affecting files other than `races/` files, in path order, all hashed together, so editing one race never clears another's mark), `car` (name), `time_s` (f32), `overbright` (semver). An entry whose `race_digest` no longer matches is stale and is removed on the next save; other races keep their marks. |
| `[base]` | table | iff `kind = "base"` | – | Base-set fields (below). Forbidden in mod packs (`MAN.5`). |
| `[bundles]` | table file name → digest | no | – | Bundles of a pack stored as several `.obz` files ([layout.md](layout.md) §4–5) and their `xxh3-128` digests. Imported base sets list them in `import.toml` instead. |
| `[extras]` | table | no | – | Tools' data (conventions §9). |
<!-- /generated -->

`[base]` fields:

<!-- generated:fields pack.base -->
| Field | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `edition` | string `[a-z0-9-]{1,32}` | yes | – | The importer's edition key ([import.md](import.md) §3), `custom` for an unknown or modified install, or the community set's own name. |
| `region` | enum `na`, `eu` | yes | – | The original's region behaviour the set follows (crates/ob-core/src/region.rs: NA = region 0, every European release = 6). |
| `languages` | array<string> | yes | – | The language keys a player can choose: the `text/<language>/` units with role `player` that hold a string table ([text.md](text.md) §2). |
| `profile` | string `<name>/<major>` | yes | – | The **required-items profile** the set satisfies (§5). |
| `import` | logicalPath | for imported sets | – | The import record, normally `import.toml`. |
| `marker` | string ≤ 80 | no | – | A short line the launcher shows for community sets ("Community city set 0.3"). |
| `storage` | enum `owned`, `linked` | no | `owned` | Whether stage-0 files are copies (`owned`) or references to the game folder (`linked`, developer builds only; [import.md](import.md) §4.2). |
<!-- /generated -->

## 4. Content digest

A pack's **content digest** is computed over its files sorted by logical path, excluding `.studio/`, as conventions
§7 defines list digests (domain tag `ob-pack-content/1\0`, then per file its path and its digest); `pack.toml` takes
part as the canonical JSON form of its TOML document (RFC 8785, JSON Canonicalization Scheme, after the standard
TOML-to-JSON mapping) with its `tested` array removed, so marking a race as tested does not change the pack's
identity and every tool computes the same bytes. Two
digests exist: the `xxh3-128` one for local caches and checks, and the `sha256` one (per-file SHA-256 inside the same
construction) for anything compared between machines. It identifies a pack's content independent of how it
is stored (folder, one `.obpk`, several bundles). Mod sets, saves' sidecars and a later online lobby compare
packs by `(id, version, sha256 content digest)`; a published `.obpk` is also identified by the `sha256` and size of
the file, computed after alignment.

### 4.1 Provenance (planned, `provenance.toml`)

A mod pack MAY carry `provenance.toml` at its root (`ob.provenance/1.0`; its schema is not part of draft 1): per
logical path or item, `origin = "own"`, `"derived"` (started from the player's own game files) or `"imported"`, and
a credit line. It travels inside the `.obpk`, and the merge and export checks read it: items marked `derived` or
`imported` keep the pack `local-only` (`MAN.4`) unless the author removes them. Overbright itself never hosts or
shares packs; authors are responsible for what they share, and the editor warns when an item started from the
original (even a repaint traced over it may still count as derived).

### 4.2 Per-item fingerprints (format 3)

A format-3 mod pack lists, in the file its `items` field names (normally `items.toml`), every other file of the pack
except `pack.toml`, itself and `.studio/` with its `xxh3-128` digest (conventions §7):

```text
schema = "ob.items/1.0"

[items]
"map.obmap" = "xxh3-128:0123456789abcdef0123456789abcdef"
"races/docks-sprint.toml" = "xxh3-128:fedcba9876543210fedcba9876543210"
```

Paths are sorted; the file is deterministic, so the same files always give the same bytes. A reader compares the
pack's files with the list and reports each item as **changed** (other bytes), **missing** or **not listed**
("Look inside" shows them; nothing is refused or repaired because of a difference: the fingerprints say what changed,
the content digest of §4 still identifies the pack). The items file is at most 4 MiB; a damaged one is refused
(`MAN.10`).

Compatibility ([versioning.md](versioning.md) P.2, P.4): packs without `items` (every format-2 pack) are read exactly
as before and report no fingerprints; they load and try out unchanged, and a reader never writes the field into them.
Only new packs written by Overbright 0.7 or newer carry it, and they declare `format = 3`; a format-2 file that
holds `items` is refused (`MAN.10`), and an older Overbright refuses a format-3 pack with `MAN.1` ("newer than this
Overbright reads") instead of misreading it.

## 5. Required-items profile

The engine cannot run without certain items: the menu packages it opens by name, its fonts, the car and part
tables, at least one region, the HUD textures, the sound banks it starts. Today these names are spread through
the code. The format makes them explicit:

- **`profile = "nfsu2/1"`**: the list of every item key the engine requests by a fixed name, generated from the
  code (each such request goes through one registry, `assets::required`, and a test enumerates it). It is
  published with the format as `profiles/nfsu2-1.toml` [to be generated].
- Each item of the profile has one of two tiers:
  - **required**: the engine cannot start or cannot run a mode without it (menu packages, fonts, the car and part
    tables, the regions, the HUD textures). A base set MUST provide every required item (`MAN.6`).
  - **expected**: the engine has a neutral stand-in that the kind page defines (a missing movie is skipped, as the
    original skips a missing movie; a missing sound plays as silence; a texture no set holds draws the
    `DEFAULTALPHA` texture, which is itself **required**, [textures.md](textures.md)). A base set SHOULD provide them; `base check` lists the missing ones as warnings.
- An imported base set of a known edition provides every item of both tiers; the importer checks it on every
  import (`IMP.5`). A community set is checked by `overbright-studio-cli base check`, so a set can start without,
  for example, a sound bank compiler existing yet.
- New engine features that request new fixed names raise the profile's version only for required items.
- This is what lets a community base set exist: same ids, same roles, the engine unchanged.

## 6. Format 1 (today's texture packs)

A `pack.toml` without `format` or with `format = 1` is read as a legacy texture pack ([layering.md](layering.md)
§7): `name`, `author` (→ `authors`), `licence`/`license`, `priority`, `version` (any text; not semver), `description`,
`compress` and `source` keep their meaning (`src/obpack/manifest.rs:23-40`); `kind = "mod"`,
`origin = "own-work"` unless `source` names an import (`texmod`, `texwizard`, `upscale`: then `from-game-files`),
`share` follows `origin`, `overbright = "*"`, `id` = folder name. Fields that format 1 ignores remain ignored.

## 7. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| MAN.1 | `format` higher than this reader knows, or not an integer. |
| MAN.2 | `id` invalid, or not matching the `.obpk` file name. |
| MAN.3 | This Overbright version is outside `overbright`. |
| MAN.4 | `share = "public"` with an origin that forces `local-only`; `public` without `licence`. |
| MAN.5 | `[base]` in a mod pack, or missing in a base set. |
| MAN.6 | A base set lacks an item of its profile (the first ten missing keys are listed). |
| MAN.7 | A dependency, optional or conflict entry names the pack itself, or a version requirement does not parse. |
| MAN.8 | `pack.toml` larger than 64 KiB (as today, `src/obpack/manifest.rs` `MAX_BYTES`). |
| MAN.9 | `extensions_required` not a subset of `extensions_used`. |
| MAN.10 | `items` in a format-2 file; the items file missing, over 4 MiB, not TOML, or listing a path or digest that does not parse (format 3). |
<!-- /generated -->
