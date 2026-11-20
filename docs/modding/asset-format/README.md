# The Overbright asset format

Status: **draft 1 — design; the core library exists (`crates/ob-asset-format`), no program reads game data
through it yet.**

> **This is a design.** No released Overbright build reads or writes this format yet: today the game reads the
> original game folder directly, and its caches and mod files are the ones described in the other docs
> ([asset-cache.md](../../developing/asset-cache.md), [texture-packs.md](../texture-packs.md), [map-editor.md](../map-editor.md)).
> Programs and tools named here (`overbright-import`, `overbright-studio-cli`, Studio, the web build) are planned,
> and so are the source paths, checks and tests the pages name as the place a rule will live. Rules, ids and
> numbers may still change before the first implementation; numbers marked [guess] or [est] are estimates.
> The format's core library exists (`crates/ob-asset-format`: exact floats, names and digests, bundles, the
> manifest, `order.txt`, the layer stack and the merge engine, the rule registry); no program reads game data
> through it yet.

Overbright's own documented, versioned format for everything the engine loads. A one-time **import** turns the
player's own copy of the game into a **base set** in this format; from then on the game, the launcher, Studio, the
command-line tool and a later web build read only this format, through one asset layer. **Mod packs** are the same
format, layered on top. A base set made entirely by a community, with no data from the original game, is possible
in the same format.

## 1. Pages

Common rules (read first):

| Page | Contents |
|---|---|
| [conventions.md](conventions.md) | Conformance words, text and binary rules, floats, coordinates and units, names, hashes, ids, references, digests, unknown data, extensions, limits, errors, the page template |
| [layout.md](layout.md) | Folders on disk, the logical tree (units and kind folders), folder form, bundles (`.obz`) and pack files (`.obpk`), a worked example |
| [manifest.md](manifest.md) | `pack.toml` format 2 for base sets and mod packs |
| [layering.md](layering.md) | The layer stack, `order.txt`, merge operations, clashes, legacy packs |
| [residual.md](residual.md) | Original bytes kept until understood; the rebuild invariant |
| [cooked.md](cooked.md) | The cooked form (asset cache): units, keys, framing, what the web build loads |
| [versioning.md](versioning.md) | Version numbers, compatibility promises, migrations, keeping docs and code in step |
| [import.md](import.md) | The importer: editions, steps, verification, the import record, re-import, repair, progress protocol |
| [parity.md](parity.md) | Equality levels, the proofs, the gates, and the risks to the native guarantee |
| [performance.md](performance.md) | Runtime budgets (desktop, low end, web), how the cooked form meets them, the benchmark gate every kind passes |
| [rationale.md](rationale.md) | The design decisions and the evidence behind them (other projects' code and specs, standards, research) |

Kinds (one page per family; each follows conventions §12):

| Page | Kinds |
|---|---|
| [textures.md](textures.md) | texture sets, texture records, texture animation |
| [meshes.md](meshes.md) | solids (glTF 2.0 with `OB_` extensions) |
| [animation.md](animation.md) | animation banks, camera tracks, in-engine scenes, part animations |
| [world.md](world.md) | regions, stream sections, scenery sections and placements, visibility, lights, smackables, world animation, parameter maps and fog |
| [collision.md](collision.md) | collision per section, barrier and scenery groups |
| [roads-ai.md](roads-ai.md) | road network, AI routes, traffic, track markers, custom races |
| [cars.md](cars.md) | car types, handling, parts, performance packages, vinyls, region car lists |
| [career.md](career.md) | career data, events, stages, unlocks, tracks and track maps, magazines |
| [ui.md](ui.md) | menu packages (FEng), fonts, HUD |
| [text.md](text.md) | strings, languages, credits, subtitles, legal texts |
| [audio.md](audio.md) | sound banks, engine sounds, streams, speech, music, mix maps, effects |
| [movies.md](movies.md) | full-motion video |
| [engine-tables.md](engine-tables.md) | the engine's built-in tables and their overrides |

Schemas: [`schemas/`](schemas/) (JSON Schema 2020-12; shared definitions in `schemas/common.schema.json`).
Examples: [`examples/`](examples/) (every file validates against its schema).

## 2. The model in one picture

```text
 player's game folder ──(import, once; verified)──► base set (source form, <app>/base/<id>/)
                                                        │
 mod packs (<app>/mods/, folder or .obpk) ──────────────┤  layer stack: base + enabled packs (order.txt)
                                                        ▼
                                     asset layer (one VFS + merge per kind)
                                                        │
                       ┌────────────────────────────────┼──────────────────────┐
                       ▼                                ▼                      ▼
              cooked form (cache)                  Studio / CLI / MCP     web build (OPFS)
              rebuilt from source any time         (read; write packs)
                       │
                       ▼
                     game
```

## 3. Kind registry

Short names are used in references (`<kind>:<key>`, conventions §6.6). Item keys name one item exactly; scoped
references (by name or hash) resolve in the referencing unit by the engine's original lookup order (conventions
§6.6). Each page's section 2 has the full key rules.

| Short name | Schema id (file kind) | Key | Page |
|---|---|---|---|
| `texture` | `ob.texture-set/1.0` (records), images in KTX 2.0 | `texture:0xHASH` (packs may write `texture:NAME`); resolved per view | [textures.md](textures.md) |
| `texture-anim` | `ob.texture-anims/1.0` | `texture-anim:0xHASH`, per view | [textures.md](textures.md) |
| – | `ob.texture-index/1.0` | one per unit (`<unit>/textures/index.toml`) | [textures.md](textures.md) |
| – | `ob.texture-overrides/1.0` | (scope, hash, renderer) | [textures.md](textures.md) |
| `solid` | glTF 2.0 + `OB_solid_list`, `OB_solid`, `OB_marker`, `OB_material` | item `<unit>/<list>/<NAME\|0xHASH>` (`world/L4RA/1427.0/XO_CONE_01`); scoped `NAME` / `0xHASH`; files marked `OB_solid_list.schema = "ob.mesh/1.0"`; lists a pack adds are named `<pack id>:<stem>` | [meshes.md](meshes.md) |
| – | `ob.mesh-patch/1.0` (packs) | entries keyed by solid item key | [meshes.md](meshes.md) |
| `anim`, `skeleton` | `ob.anim-banks/1.0` (+ `.eagl` passthrough) | bank `<unit>/<source>.<n>` (banks a pack adds: `<pack id>:<stem>`); item `<bank>/<NAME>`; scoped `NAME` / `0xHASH` | [animation.md](animation.md) |
| `part-pivots` | `ob.part-pivots/1.0` | `<CAR>` | [animation.md](animation.md) |
| `camera-track` | `ob.camera-tracks/1.0` | `<unit>/<source>/<group>/<index>` | [animation.md](animation.md) |
| `scene` | `ob.scene/1.0`, `ob.scene-events/1.0`, `ob.scene-table/1.0` | `scene:<NAME>`; events by (scene, index); table rows by (list, index) | [animation.md](animation.md) |
| `region` | `ob.region/1.0` | `region:L4RA` | [world.md](world.md) |
| `section` | `ob.section/1.0` | `section:L4RA/1427` (stream section, the load unit) | [world.md](world.md) |
| `scenery` | inside `ob.section` | `scenery:L4RA/1407` (scenery section) | [world.md](world.md) |
| `placement` | inside `ob.section` (base), `ob.placements/1.0` (packs) | `placement:L4RA/1407/12` (region, scenery section, index); added `placement:L4RA/docks-remix:a12` | [world.md](world.md) |
| – | `ob.visibility/1.0`, `ob.smackables/1.0`, `ob.world-anims/1.0`, `ob.fog/1.0` | `light:L4RA/1427/3`, `prop-kind:L4RA/<NAME>`, `scenery-group:…`, `scenery-link:…`, `fog-map:L4RA/0xHASH` | [world.md](world.md) |
| `collision` | `ob.collision/1.0`, `ob.collision-shared/1.0`; `ob.collision-delta/1.0` reserved | `collision:L4RA/1427`; `barrier:L4RA/<NAME>`, `volume:L4RA/<NAME>` | [collision.md](collision.md) |
| `road`, `route`, `marker` | `ob.road-net/1.0`, `ob.route-paths/1.0`, `ob.ai-routes/1.0`, `ob.track-markers/1.0` | `road:<R>/<trk>`, `route:<R>/<4001F\|FreeRoam\|…>`, `marker:<R>/<All\|FreeRoam>/<i>`, `route-trigger:<R>/<i>` | [roads-ai.md](roads-ai.md) |
| `ai-tuning` | `ob.ai-tuning/1.0` | `ai-tuning:global` | [roads-ai.md](roads-ai.md) |
| `race` | `ob.race/1.0` (packs) | `race:<pack>:<id>` | [roads-ai.md](roads-ai.md) |
| `car`, `preset` | `ob.car-type/1.0`, `ob.car-index/1.0`, `ob.car-presets/1.0` | `car:<NAME>` (record name, e.g. `car:PEUGOT`); type index = position in the index | [cars.md](cars.md) |
| `part`, `part-cost` | `ob.part-db/1.0`, `ob.part-costs/1.0` | `part:0x<GROUP>/0x<HASH>`; `part-cost:<n>` | [cars.md](cars.md) |
| `perf-package`, `perf-part`, `tuning-unlock` | `ob.performance/1.0` | `perf-package:<category>/<level>`, `perf-part:<id>`, `tuning-unlock:<part>/<param>` | [cars.md](cars.md) |
| `car-unit` | `ob.car-unit/1.0` | `car-unit:<NAME>` (folder name; `common`) | [cars.md](cars.md) |
| `event`, `stage`, `sponsor`, … | `ob.career/1.0` | `event:<NAME>`, `stage:<n>`, `<kind>:<NAME>` | [career.md](career.md) |
| `track`, `track-map` | `ob.tracks/1.0`, `ob.track-maps/1.0` | `track:<n>`, `track-map:<n>` | [career.md](career.md) |
| – | `ob.magazines/1.0` | `<kind>:<name or 0xHASH>` | [career.md](career.md) |
| `fe-package`, `fe-object` | `ob.fe-package/1.0` | `fe-package:<NAME>`; `fe-object:<NAME>/0xID` | [ui.md](ui.md) |
| `font` | `ob.font/1.0` | `font:<name>` (no case; the language's fonts first) | [ui.md](ui.md) |
| `language`, `string` | `ob.language/1.0`, `ob.strings/1.0` | `language:<language>`; `string:0xHASH` (in the active language) | [text.md](text.md) |
| – | `ob.credits/1.0`, `ob.legal-text/1.0`, `ob.memcard-text/1.0`, `ob.subtitles/1.0` | `credits:<language>/<FILE>`, `legal-text:<language>/<FILE>`, `memcard-text:<language>`, `subtitles:<NAME>` | [text.md](text.md) |
| `sound-bank`, `sound`, `sound-event` | `ob.sound-bank/1.0` | `<DIR>/<STEM>` (`IG_GLOBAL/ENV_COMMON_MB`), `…/<slot>`, `…/<EVENT>` (file names: no case) | [audio.md](audio.md) |
| `engine-recording` | `ob.engine-recording/1.0` | file stem (no case) | [audio.md](audio.md) |
| `speech-line`, `speech-event` | `ob.speech-bank/1.0` | `<bank>/<entry>/<record>`, `<bank>/0xNNNN` | [audio.md](audio.md) |
| `music-stream`, `music-event` | `ob.music-map/1.0` | `<map>/<index>` | [audio.md](audio.md) |
| `mix-map`, `reverb-preset` | `ob.mix-map/1.0`, `ob.reverb-preset/1.0` | file stem | [audio.md](audio.md) |
| `movie`, `movie-entry` | `ob.movie/1.0`, `ob.movie-table/1.0` | `movie:<stem>` (`Scene01_en`); table row name | [movies.md](movies.md) |
| `table` | `ob.engine-table/1.0` (packs, community sets) | `table:<name>`; one value `table:<name>@<path>` | [engine-tables.md](engine-tables.md) |
| – | `ob.residual-map/1.0`, `ob.import/1.0`, `ob.pack-migrations/1.0` | per original file; one per base set; per pack | [residual.md](residual.md), [import.md](import.md), [versioning.md](versioning.md) |

**Additional short names** the pages define for sub-items: `solid-list`, `anim-bank`, `camera-group`, `light`,
`flare`, `prop`, `emitter`, `anim-entity`, `anim-instance`, `horizon-fog`, `prop-kind`, `scenery-group`,
`scenery-link`, `collision-face`, `route-barrier`, `route-trigger`, `checkpoints`, `road-node`, `route-record`,
`trick`, `cash-pickup`, `car-unlock`, `fe-platform`, `fe-envmap`, `speech-bank`, `music-map` (keys on their pages).

**Not yet owned by a kind** (imported byte for byte into the residual until a page defines them; references to them
are bare hashes, conventions §6.6):

| Data | Where | Meanwhile |
|---|---|---|
| Light materials (0x135200) | `GLOBAL/GLOBALB.BUN` | solids and materials hold their hashes as bare `hash32` (meshes.md) |
| Particle effect descriptions (0x35021) | `GLOBALB.BUN`, `FRONTEND/PLATFORMS/Showroom.BIN`; a region's own records | a region's records are carried opaque in `world/<REGION>/effects.bin` (world.md); placed emitters (0x35020) in regions are decoded by world.md, those in `Showroom.BIN` are kept |
| Car collision volumes (0x80034020) | `CARS/*/GEOMETRY.BIN` | kept |
| Collision-audio stitch records (0x8003B500) | front-end and in-game bundles | kept |
| Front-end light setups (0x3B700) | `FRONTEND/FrontB.lzc` | kept |
| The second career container (a second 0x80034A10) | `GLOBAL/GlobalB.lzc` | its children 0x34A15, 0x34A1A, 0x34A1C are cars.md's (the same items when identical to the first container's); the rest kept |
| Chunk 0x34202 (1,096 bytes) | `GLOBAL/GlobalB.lzc` | kept (no reader found) |

Each becomes a kind page when its reader exists.

**`OB_` extension registry.** Extensions Overbright itself defines, with whether each is always required and safe
to copy. TOML-side extensions: none in draft 1. glTF-side: `OB_solid_list`, `OB_solid`, `OB_marker`, `OB_material`
([meshes.md](meshes.md); their schemas are in `schemas/gltf/`), which are part of the mesh profile itself rather than
optional extensions.

## 4. Status of this draft

Every page is **draft 1**: a design, complete for every kind of the reference install and checked for consistency
across pages, but not implemented. Each kind page lists its open points (section 10). What is still open across
pages is listed in [parity.md](parity.md) §4 and §5.

`scripts/docs/asset_format_check.py` checks this folder: every schema is valid JSON Schema 2020-12, every TOML
example validates against its schema, the glTF example's `OB_` extensions validate against their schemas and its
buffers and accessors fit, and every link between the pages resolves (it needs the Python package `jsonschema`;
not yet checked with the Khronos glTF validator). Every example is made up: none holds data from the original game.

Parts of the pages are generated from `crates/ob-asset-format` and must not be edited by hand: every rule table
(between `<!-- generated:rules -->` and `<!-- /generated -->`), the manifest's field tables, and
`schemas/pack.schema.json`, `schemas/pack-migrations.schema.json`, `schemas/placements.schema.json` and
`schemas/race.schema.json`. `cargo run -p ob-asset-format --example
asset_format_docs -- --write` regenerates them; the crate's test `docs_in_step` (part of `cargo test`) and
`scripts/checks/asset_format.sh` fail when the pages and the code differ ([versioning.md](versioning.md) §6).

### 4.1 Decisions this draft is built on

- **Players always get an owned copy.** The launcher's import copies the player's game once into Overbright's own
  bundles; afterwards the game folder is not needed ([import.md](import.md) §4.2). Play never waits for the import:
  until the copy is complete, the game reads the folder as it does today.
- **Linked base sets are for developers only.** A set that refers to files in the game folder instead of copying
  them exists only in developer builds, CI and test sandboxes; a release build refuses to mount one
  ([import.md](import.md) §4.2, `IMP.7`).
- **The import stops when the disk is short.** The free-space check names the space needed and where; there is no
  fallback that leaves the player's data in the game folder ([import.md](import.md) §10, `IMP.2`).
- **Graduation on demand, every kind as the goal.** The first import keeps every original file byte for byte
  (stage 0). A kind graduates to its documented, decoded files when a tool or a base set needs it; until then it
  stays in the original's own encoding inside Overbright's own bundles. The stated goal is that every kind
  graduates, and every kind must have graduated before engine-level work that depends on it (bakers, new regions,
  community base sets) ([import.md](import.md) §4.1, [parity.md](parity.md) §3).
- **One stale reference refuses the whole mod.** A pack is applied completely or not at all; a pack whose reference
  or guard no longer matches the base set is refused with the count and the first stale item, and the editor
  offers to repair it ([conventions.md](conventions.md) §11, [layering.md](layering.md)).

## 5. Prior art: what this format takes, and what it avoids

| Prior art | Taken | Avoided |
|---|---|---|
| glTF 2.0 (https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html) | The spec style (normative words, property tables, JSON Schema); buffers → views → typed arrays; `_`-prefixed custom attributes; `extensionsUsed` / `extensionsRequired`; `extras`; glTF itself for meshes | Ignoring unknown properties (here they are refused; versioning.md §2.1) |
| KTX 2.0 (https://registry.khronos.org/KTX/specs/2.0/ktxspec.v2.html) | The GPU texture container, original BC blocks unchanged | – |
| USD layering (https://openusd.org/release/glossary.html) | Layer stack, strongest opinion per field, `over` vs `def`, deactivate instead of delete | Composition arcs, time samples, the C++ dependency |
| Bethesda plugins (https://en.uesp.net/wiki/Skyrim_Mod:Mod_File_Format) | A conflict view per field; a no-op override lint | Load-order-relative numeric ids; whole-record overrides ("dirty edits") |
| OpenMW (https://openmw.readthedocs.io/en/latest/reference/modding/openmw-game-template.html) | The base game as the root content; a complete free template game | Engine-hard-coded asset names (here: the required-items profile, manifest.md §5) |
| Godot import system (https://docs.godotengine.org/en/stable/tutorials/assets_pipeline/import_process.html) | Cache keyed by the source's hash and the importer's version; stable ids separate from paths | Silent same-path replacement by packs |
| Unreal cooking and DDC (https://dev.epicgames.com/documentation/en-us/unreal-engine/cooking-content-in-unreal-engine) | Editable source vs per-target cooked data vs disposable derived cache | Priority by file-name suffix |
| Quake PAK, Source VPK (https://github.com/id-Software/Quake/blob/master/WinQuake/common.c) | A simple ordered search list | Archive-type-dependent priority |
| Factorio (https://lua-api.factorio.com/latest/auxiliary/migrations.html) | The base game as a pack; dependency kinds; one-shot rename migrations recorded in saves | – |
| Minecraft packs (https://minecraft.wiki/w/Pack.mcmeta) | Supported version ranges per pack; namespaced ids | A format number that changes every release |
| Ship of Harkinian (https://github.com/HarbourMasters/Shipwright/blob/develop/docs/VERSIONING.md) | Extract the player's own original once into a ZIP-based archive stamped with versions; mods are more archives of the same format; regenerate after a major update | – |
| OpenGOAL (https://github.com/open-goal/jak-project) | An edition database with per-file hashes and an order-independent identity; re-derive after updates | Needing a kept copy of the original for that (here: the residual's rebuild invariant) |
| OpenTTD base sets (https://github.com/OpenTTD/OpenTTD/blob/master/docs/obg_format.txt) | A base set as a declared set of files with hashes; original and free sets side by side | – |
| OpenRCT2 objects (https://github.com/OpenRCT2/objects) | Provenance of every item back to the original; the same id twice is an error | – |
| Freedoom (https://github.com/freedoom/freedoom) | A community replacement set keeps the original's names, sizes and roles, plus a marker | – |
| VCMI (https://github.com/vcmi/vcmi) | Sparse field edits addressed by id; list operations; compatibility ids for renames | – |
