# Asset format: on-disk layout

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md)
apply.

This page says where packs live, how a pack's files are organised (the **logical tree**), how they are stored
(**folders and bundles**), and how the importer groups a base set. What each file contains is on the kind pages.

## 1. Folders on the player's machine

All folders follow `platform::dirs` (docs/playing/platform-dirs.md): the per-user folders by default, `data/` beside the
programs in a portable install, and any folder the player chose in `locations.txt`.

```text
<app>/                                   Overbright's own folder (dirs::app; portable: data/)
  base/                                  base sets                        locations.txt key: base   (new)
    active.txt                           "# Overbright base v1", then the id of the base set in use
    nfsu2-pc12-eu/                       a base set made by the importer (id: import.md §3)
      pack.toml                          kind = "base"                     manifest.md
      import.toml                        the import record                 import.md §6
      bundles/*.obz                      the logical tree, one bundle per load unit (§5); or the tree as plain files (§3)
    <community-set>/                     a community base set (no import.toml), folder or bundles
    .staging-<id>/                       an import in progress (§6); never mounted
  mods/                                  mod packs                         locations.txt key: packs
    order.txt                            the player's order and on/off     layering.md §2
    <id>/                                a pack as a folder (the authoring form)
    <id>-<version>.obpk                  a pack as one file (the sharing form)
<cache>/                                 the asset cache (dirs::cache; rebuildable, never authoritative)
  cooked/…                               cooked units                      cooked.md
```

- **`base/` is data, not cache.** Once the player's original folder is gone, the base set is the only copy of the
  content: cache pruning never touches it, and `NFSU2_NO_CACHE` does not affect it.
- `active.txt` names one base set. With one base set present and no `active.txt`, that set is used. The launcher
  writes the file atomically; the game reads it at start (`--base <id>` overrides it for tests and Studio runs).
- Today's registries (`packs/`, `texture-pack/imported/`, the HUD mod folder, the generated upscale art, `maps/`)
  are read as **legacy packs** without moving files ([layering.md](layering.md) §7).
- Overbright's other own files (options, profiles, logs, screenshots) are **not** assets and stay where
  docs/playing/platform-dirs.md puts them. Native profile saves keep the original's own format, so the original game can
  still load them (docs/formats/saves-options.md).

## 2. The logical tree

Every pack, base or mod, has the same logical tree. The first level is the **load unit** (what the engine loads
together); the second level is the **kind folder**. A pack holds only the folders it needs.

| Unit (first level) | Holds | Filled from (base sets made by the importer) |
|---|---|---|
| `global/` | data every mode shares: car and part tables, career data, shared textures and meshes, HUD textures, AI tuning, the city's dynamic textures, the track maps and their calibration (loaded by track number) | `GLOBAL/*`, `TRACKS/LOC4DYNTEX.BIN`, `TRACKS/TRACKMAP*.BIN` |
| `frontend/` | menu packages (also those stored in `GLOBAL/` files), shared fonts, menu and language textures, magazines, garage platforms and environment maps | `FRONTEND/*`, menu packages from `GLOBAL/*`, `LANGUAGES/LanguageTextures.bin`, shared fonts from `LANGUAGES/*` |
| `world/<REGION>/` | one track region: sections, scenery, visibility, lights, collision, road network and AI routes, the region's textures, meshes and animation | `TRACKS/<REGION>.BUN`, `TRACKS/STREAM<REGION>.BUN`, `TRACKS/ROUTES<REGION>/*` |
| `cars/<NAME>/` | one car (or one shared part group: `WHEELS`, `SPOILER`, `MIRRORS`, …): geometry, textures, vinyls, part animations | `CARS/<NAME>/*` |
| `cars/common/` | what all cars share | `CARS/TEXTURES.BIN` and other top-level `CARS/` files |
| `scenes/<NAME>/` | one in-engine scene (NIS): its meshes, textures, animation, event track | `NIS/Scene_<NAME>_*` |
| `text/<language>/` | strings, credits, legal texts, save-screen texts and language fonts of one language | `LANGUAGES/*`, `CREDITS/*`, `memcard/LOCALE_*` |
| `audio/` | sound banks, engine sounds, streams, speech, music, mix and effect settings (sub-units: [audio.md](audio.md)) | `SOUND/**`, `SDATA/sdat.viv` |
| `movies/` | full-motion videos, their table, the subtitle cues (language-neutral; kind defined on [text.md](text.md)) | `MOVIES/*`, `SUBTITLES/*`, the movie table in `GLOBAL/GLOBALB.BUN` |
| `tables/` | engine table overrides (mod packs and community base sets; an imported base set has none, its tables are built into the engine) | – |
| `races/` | custom races (mod packs) | – |
| `residual/` | original bytes no decoder reads yet (base sets made by the importer only) | any file |
| `.studio/` | authoring state: journal, autosave, panel state (folder form only; never in a bundle or `.obpk`) | – |

Kind folders inside a unit: `textures/` ([textures.md](textures.md)), `meshes/` ([meshes.md](meshes.md)),
`anim/` ([animation.md](animation.md)), and each unit's own kinds on its page ([world.md](world.md),
[collision.md](collision.md), [roads-ai.md](roads-ai.md), [cars.md](cars.md), [career.md](career.md),
[ui.md](ui.md), [text.md](text.md), [audio.md](audio.md), [movies.md](movies.md),
[engine-tables.md](engine-tables.md), [residual.md](residual.md)).

A unit MAY have **sub-units** one level down where the original loads pieces of it separately (`frontend/platforms/<NAME>/`,
`frontend/envmaps/<NAME>/`, `audio/<sub-unit>/`); kind pages name them. A unit's root MAY hold the files of kinds that belong to the unit as a whole (`world/L4RA/region.toml`,
`visibility.toml`, `placements.toml`;
`scenes/IntroNis08/scene.toml`); everything else sits in a kind folder.

Unit names: `<REGION>` and `<NAME>` keep the original's spelling (`L4RA`, `PEUGOT206`, `IntroNis01`);
`<language>` is the original's language file name in lower case (`english`, `german`, …); names this specification
chooses are lower case, so they never collide with an original name (which are upper case or mixed case).

## 3. Folder form

A pack folder holds `pack.toml` (and optionally `migrations.toml`, [versioning.md](versioning.md) §4.3) at its root and the logical tree below it, file for file. This is the form Studio
edits and the form a pack author keeps in version control. Logical path = path relative to the folder, with `/`.
A reader MUST refuse symbolic links and hard-linked directories inside a pack folder, and any file whose name
breaks [conventions.md](conventions.md) §6.2.

A base set MAY also be stored in folder form (the importer's `--folder`, used by developer builds and by players
who prefer plain files): `import.toml` then lists every file with its digest instead of bundles, and verification on
mount (§5) works per file. The player's default is bundles, because many small files cost more to open and scan,
especially on Windows ([performance.md](performance.md) §3.5).

## 4. Bundles (`.obz`) and pack files (`.obpk`)

A **bundle** stores a part of a logical tree in one file: a ZIP archive with a restricted profile, so any ZIP tool
can open it and Overbright can memory-map it.

**ZIP profile** (APPNOTE.TXT 6.3.10, https://pkware.cachefly.net/webdocs/casestudies/APPNOTE.TXT):

| Rule | Value |
|---|---|
| Archive | single disk, no 64-bit ZIP extensions, no encryption, no data descriptors (bit 3 clear), no archive comment longer than 0 |
| Methods | 0 (stored) or 8 (deflate). **Payload** entries outside `residual/` (`.bin`, `.ktx2`, `.png`, `.eagl`, `.ivf` and other binary data the engine uses in place) MUST be stored in base-set bundles; under `residual/`, stage-0 files the engine reads in place MUST be stored and everything else SHOULD be deflated; **text** entries (`.toml`, `.gltf`) SHOULD be deflated (the glTF JSON of the reference install is about 120 MB stored and 14 MB deflated) |
| Names | logical paths, UTF-8 flag (bit 11) set, no directory entries, no duplicates, no case-only duplicates |
| Order | the **central directory** lists entries sorted by logical path (byte order), for binary search; the entries' **data** lies in the unit's load order (the order the kind pages' loaders read them), so a load reads the bundle front to back ([performance.md](performance.md) §3.4). Both orders are deterministic. In a `.obpk`, `pack.toml`'s data comes first; in the central directory it sorts like any other path |
| Times | every entry 1980-01-01 00:00 (DOS time 0x0021). With stored entries the same tree always gives the same bytes; deflated entries depend on the deflate implementation and level, so a pack's identity is its content digest ([manifest.md](manifest.md) §4), never the bundle's bytes (https://reproducible-builds.org/docs/archives/) |
| Alignment | each stored entry's data starts at a multiple of **64** bytes from the file's start (a cache line; the alignment USDZ uses for the same purpose), padded through the local header's extra field in the layout Android's zipalign uses (header id `0xD935`, a `u16` alignment value, then zero bytes; listed in APPNOTE 6.3.10 §4.6.1), so tools that rewrite ZIP files and know that field keep the alignment. Bundles the importer or Studio writes MUST be aligned; a third-party `.obpk` with misaligned stored entries is accepted and those entries are copied instead of mapped |
| Extra fields | only the `0xD935` alignment field, written identically in the local and the central header, so each entry's data offset follows from the central directory alone; readers ignore unknown extra fields from other tools |
| Limits | a bundle < 4 GiB and ≤ 65,535 entries; a stored entry ≤ 1 GiB. A pack larger than that is stored as several bundles (§4, last paragraph); a single-file `.obpk` is therefore < 4 GiB |

- **Reading**: the central directory is read once; a stored entry is a slice of the mapped file (zero copy; the
  existing reader `crates/ob-formats/src/formats/zip.rs` already maps the file); a deflated entry is inflated on use. Readers MUST
  check every entry's CRC-32 when it is inflated and when the bundle is verified (§5); stored entries read from a
  verified bundle are not re-checked on each read.
- A reader MUST refuse an entry name that breaks [conventions.md](conventions.md) §6.2, overlapping entries, an
  entry whose data runs past the central directory, and every profile breach above (rule `LAY.1`).
- **`.obpk`** is a bundle that holds a whole mod pack, `pack.toml` first: the one file a player downloads and the
  launcher opens. Entries in a `.obpk` MAY be deflated. Authors' tools that write plain `.zip` files are accepted
  by the importer of packs (`overbright-studio-cli pack import`), which rewrites them to the profile.
- A pack MAY be stored as several bundles (a folder with `pack.toml` and `bundles/*.obz`), as base sets are. The
  logical trees of a pack's bundles MUST NOT overlap (`LAY.2`).

## 5. How a base set is grouped and checked

The importer writes one bundle per load unit, so one load opens one file:

| Bundle | Logical subtree |
|---|---|
| `global.obz` | `global/**` |
| `frontend.obz` | `frontend/**` |
| `world-<REGION>.obz` | `world/<REGION>/**` |
| `cars-<NAME>.obz` | `cars/<NAME>/**` (also `cars-common.obz`) |
| `scenes.obz` | `scenes/**` |
| `text-<language>.obz` | `text/<language>/**` |
| `audio-<sub-unit>.obz` | `audio/<sub-unit>/**` ([audio.md](audio.md)) |
| `movies.obz` | `movies/**` |
| (each bundle above) | `residual/<ORIGINAL/PATH>.*` of the original files that unit loads: file maps and small kept bytes deflated; stage-0 kept files **stored and 256-aligned**, every one of them ([residual.md](residual.md) §2, §3.1.1). Originals that several units read go in `global.obz`. |

- `import.toml` lists every bundle with its size and `xxh3-128` digest ([import.md](import.md) §6).
- **Verification on mount**: a reader keeps, per bundle, the size, modification time and (Unix) inode it last
  verified (in the cache folder, like the asset cache's source keys, `crates/ob-core/src/cache/format.rs`). When they match, the
  bundle is trusted; otherwise it is hashed whole in the background (XXH3 runs at about 34 GB/s per core, so the disk decides: ≈ 0.1 s per 100 MB cold on an NVMe SSD with four readers, [performance.md](performance.md) §3.6) and compared
  with `import.toml`. The background verifier pauses itself while the game loads or streams
  ([import.md](import.md) §5). A mismatch makes the base set **damaged** (`LAY.3`, [import.md](import.md) §8).
- A community base set without `import.toml` lists its bundles and digests in `pack.toml` `[bundles]`
  ([manifest.md](manifest.md)).

## 6. Writing safely

Every writer (importer, Studio, CLI) writes a file to a temporary name in the same folder and renames it into
place, as the asset cache and the map editor already do (`crates/ob-core/src/cache/temp.rs`, `crates/ob-map/src/tools/map/store.rs`). The
importer builds a whole base set in `base/.staging-<id>/` and renames the folder into place last, so a base set is
either complete or absent ([import.md](import.md) §5). A reader never mounts a `.staging-*` folder.

## 7. Worked example

A base set imported from the European PC 1.2 release, and two mod packs. Paths inside bundles are logical paths;
each file kind is defined on the page named at the right. Shortened with `…`.

```text
<app>/base/
  active.txt                                     "# Overbright base v1" / "nfsu2-pc12-eu"
  nfsu2-pc12-eu/
    pack.toml                                    kind = "base"                                  manifest.md
    import.toml                                  edition, files, bundles, units, checks         import.md
    bundles/
      global.obz
        global/cars/index.toml                   car types in table order                       cars.md
        global/cars/types/PEUGOT.toml            one car type record
        global/parts/db.toml, db.lists.bin       the part database
        global/career/career.toml                events, stages, sponsors, …                    career.md
        global/maps/track-maps.toml              minimap and World Map calibration per track
        global/ai/tuning.toml, tuning.bin        AI tuning                                      roads-ai.md
        global/textures/index.toml               texture sets: GlobalB, GLOBALA, InGame*, HUD, TRACKMAP<n>   textures.md
        global/textures/GlobalB.toml             one set's records
        global/textures/GlobalB/66994F4A.ktx2    one texture (DEFAULTALPHA), original blocks
        global/meshes/InGameCommon.bun.0.gltf    shared solids                                  meshes.md
        global/anim/InGameCommon.bun.cameras.toml  typed cameras                                animation.md
        residual/GLOBAL/GlobalB.lzc.map.toml     the fate of every byte of that file            residual.md
        residual/GLOBAL/GlobalB.lzc.kept.bin     bytes no decoder reads yet
      frontend.obz
        frontend/ui/packages/UI_Main.toml        one menu package                               ui.md
        frontend/ui/fonts/<name>.toml            a shared font
        frontend/textures/FrontB.toml, FrontB/…  menu textures                                   textures.md
        frontend/platforms/CarLot01/meshes/CarLot01.BIN.0.gltf   a garage platform (sub-unit)   meshes.md, ui.md
        frontend/magazines/magazines.toml        photo-shoot cameras, cover setups              career.md
      world-L4RA.obz
        world/L4RA/region.toml                   stream sections, detail limit                  world.md
        world/L4RA/visibility.toml               drivable outlines, visible-section lists
        world/L4RA/sections/1427.toml, 1427.bin  stream section 1427: scenery sections 1427 and 1407, in order
        world/L4RA/smackables.toml, anims.toml, fog.toml
        world/L4RA/textures/A21.toml, A21/636C197D.ktx2   the region's texture sets         textures.md
        world/L4RA/meshes/1427.0.gltf, 1427.0.bin         solid list 0 of stream section 1427   meshes.md
        world/L4RA/anim/L4RA.BUN.banks.toml, L4RA.BUN.0.eagl                                   animation.md
        world/L4RA/collision/1427.toml, 1427.bin, shared.toml                                  collision.md
        world/L4RA/routes/paths.toml, roads/4001.toml, ai/4001F.toml, markers/All.toml         roads-ai.md
        residual/TRACKS/STREAML4RA.BUN.map.toml, .kept.bin   at stage 0: the whole original file, stored     residual.md
      cars-PEUGOT.obz
        cars/PEUGOT/unit.toml                    what the original folder held                  cars.md
        cars/PEUGOT/meshes/GEOMETRY.BIN.0.gltf   car solids                                     meshes.md
        cars/PEUGOT/textures/TEXTURES.toml, VINYLS.toml, …                                     textures.md
        cars/PEUGOT/anim/PARTS_ANIMATIONS.bin.banks.toml                                       animation.md
      scenes.obz
        scenes/IntroNis08/scene.toml, events.toml, meshes/bundle.0.gltf, anim/bundle.banks.toml   animation.md
      text-english.obz
        text/english/language.toml, strings.toml, charmap.bin                                  text.md
        text/english/credits/UK_ENGLISH.toml, legal/agree.eng.toml, memcard.toml
      audio-engine.obz
        audio/engine/banks/ENGINE/CAR_00_ENG_MB_EE.toml, .bin   an engine sound bank (passthrough)   audio.md
        audio/engine/recordings/GIN_Eclipse_5Zigen.toml, .bin
      audio-speech.obz, audio-music.obz, audio-ingame.obz, audio-frontend.obz, audio-common.obz
      movies.obz
        movies/table.toml                        the movie table                                 movies.md
        movies/Scene01_en/movie.toml, video.ivf, audio.bin
        movies/subtitles/Scene01.toml            subtitle cues                                   text.md
<app>/mods/
  order.txt                                      "# Overbright mods v1" / "docks-remix" / "-old-hud"
  docks-remix/                                   folder form
    pack.toml                                    kind = "mod", contents = ["world", "races"]     manifest.md
    world/L4RA/placements.toml                   moves, disables, additions (ob.placements)     world.md, layering.md
    world/_any/textures/images/0x5FB8BCD1.png    a texture override for every region            textures.md
    races/docks-sprint.toml                      a custom race                                   roads-ai.md
    .studio/                                     journal, autosave (never exported)
  old-hud-1.0.0.obpk                             a pack as one file (switched off in order.txt)
```

The full tree of an imported base set is listed by `overbright-studio-cli base ls` ([import.md](import.md) §9).

## 8. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| LAY.1 | A bundle breaks the ZIP profile (§4): 64-bit ZIP extensions, encryption, data descriptor, unknown method, misaligned stored entry, bad name, duplicate, overlap. |
| LAY.2 | Two bundles of one pack hold the same logical path, or a bundle holds files outside its subtree (§5). |
| LAY.3 | A base-set bundle's digest does not match `import.toml` / `[bundles]`, or a listed bundle is missing. |
| LAY.4 | A symbolic link or a name outside conventions §6.2 in a pack folder. |
| LAY.5 | `active.txt` names a base set that does not exist or is damaged (the launcher asks; the game refuses to start). |
<!-- /generated -->
