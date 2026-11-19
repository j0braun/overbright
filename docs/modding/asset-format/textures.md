# Asset format: textures (texture sets, texture records, palettes, texture animation, overrides)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md), [layering.md](layering.md) and [residual.md](residual.md) apply and are not repeated.
Schemas: [`schemas/texture-index.schema.json`](schemas/texture-index.schema.json),
[`texture-set.schema.json`](schemas/texture-set.schema.json),
[`texture-anims.schema.json`](schemas/texture-anims.schema.json),
[`texture-overrides.schema.json`](schemas/texture-overrides.schema.json). Examples:
[`examples/textures/`](examples/textures/).

This page defines how the game's textures are stored (a TOML record per texture and a KTX 2.0 image), how a
`texture:` reference finds its texture (**views** and **search paths**, §2.3), how mod packs change textures
(**overrides**, §3.5), texture animation, and how today's Overbright texture packs are read as legacy packs.

Prior art taken: KTX 2.0 as the GPU image container, original BC blocks copied unchanged
(https://registry.khronos.org/KTX/specs/2.0/ktxspec.v2.html); Quake's and Source's ordered search list for name
lookups (first match wins); USD's per-field opinions for overrides (layering.md); today's Overbright packs
(docs/modding/texture-packs.md): keys by texture hash or name, PNG compiled to BC7, "pixels only, the record stays native".
Avoided: one global name space for all textures (the original's own lookup is history-dependent, §2.3), and
replacing whole original files.

## 1. Scope

| Original | Chunk ids | Here |
|---|---|---|
| Texture pack (TPK) | `0xB3300000` with header `0xB3310000` (`0x33310001` info, `0x33310002` hash table, `0x33310004` entries, `0x33310005` format records) and data `0xB3320000` (`0x33320001`, `0x33320002`) | one **texture set** (§3.2) |
| Texture pack with per-texture compressed blocks | `0x33310003` instead of (or, in the version-4 packs, beside) `0x33310004`/`0x33310005`; each block JDLZ or HUFF | one texture set; the decoded blocks give its records and images; each block is a compressed node of the residual map (§5.6) |
| Texture animation (flip-books) | `0xB0300100` (`0x30300101` header, `0x30300102` records, `0x30300103` frames) | one **texture-animation file** (§3.4) |

Where the original files go (the importer's mapping; set ids in §2.1):

| Original files | Unit and sets |
|---|---|
| `TRACKS/STREAM<REGION>.BUN` (every stream section with a pack, the shared `Y` sections included) | `world/<REGION>/textures/`, one set per pack, grouped by stream section |
| `GLOBAL/GlobalB.lzc` and `GLOBAL/GLOBALB.BUN` | `global/textures/GlobalB`, `GlobalB_2` — the two files hold the same 211 textures (records and pixels identical; measured), stored once |
| `GLOBAL/InGameCommon.lzc` and `.bun` | `global/textures/InGameCommon`, `InGameCommon_2` (the same 113 textures in both; measured) |
| `GLOBAL/GLOBALA.BUN`, `InGameRace.bun`, `InGameFreeRoam.bun`, `InGameDrag.bun`, `InGameDrift.bun`, `DYNTEX.BIN`, `GlobalTexturesNA.bin`, `GlobalTexturesROW.bin`, `HUD_CustomTextures_00.bin` … `_10.bin`, `HUD_CustomTextures_ALL.bin` | `global/textures/<file stem>` |
| `TRACKS/LOC4DYNTEX.BIN` (the city's dynamic textures: sky, the free-roam map) | `global/textures/LOC4DYNTEX` (shared by every region; [layout.md](layout.md) §2) |
| `TRACKS/TRACKMAP<n>.BIN` (129 files, one texture each, loaded by track number) | `global/textures/TRACKMAP<n>` (keyed by track number, not by region; [layout.md](layout.md) §2) |
| `FRONTEND/FrontB.lzc` | `frontend/textures/FrontB`, `FrontB_2` |
| `FRONTEND/MAGAZINES_STREAMING_HIGH.bin`, `_LOW.bin` | `frontend/textures/<file stem>` (the magazine *data* is [career.md](career.md)'s) |
| `FRONTEND/PLATFORMS/<X>.BIN` (the shops' models' pack and their environment-map pack) | sub-unit `frontend/platforms/<X>/textures/<X>`, `<X>_2` (the models: [meshes.md](meshes.md); the sub-unit: [ui.md](ui.md)) |
| `FRONTEND/ENVMAPS/<X>.bin` | sub-unit `frontend/envmaps/<X>/textures/<X>` |
| `LANGUAGES/LanguageTextures.bin` (one file for every language) | `frontend/textures/LanguageTextures` (one set for every language; [layout.md](layout.md) §2) |
| `CARS/<NAME>/TEXTURES.BIN` | `cars/<NAME>/textures/TEXTURES` (`cars/WHEELS/textures/TEXTURES` for the rims) |
| `CARS/<NAME>/VINYLS.BIN` | `cars/<NAME>/textures/VINYLS`: pixel storage only; which part names which vinyl is [cars.md](cars.md) §3.8 |
| `CARS/TEXTURES.BIN` | `cars/common/textures/TEXTURES` |
| `NIS/Scene_<NAME>_BundleB.bun` | `scenes/<NAME>/textures/<pack path stem>` |

**Not on this page:** which texture a solid's draw range uses ([meshes.md](meshes.md): its `textures` list holds
`texture:` references), particle and glow descriptions ([world.md](world.md)), menu packages' resource names
([ui.md](ui.md)), vinyl and decal definitions and paints ([cars.md](cars.md)), magazine records
([career.md](career.md)), the HUD map calibration ([career.md](career.md)). Render targets the engine creates at run
time and registers under a name (rear-view mirror, shadow and reflection targets: the table at 0x82D530 filled by
0x488910, **verified**) are not data and have no items here; their names are reserved (TEX.17).

## 2. Files and keys

### 2.1 Folders and files

Every unit that has textures holds a `textures/` folder:

```text
<unit>/textures/
  index.toml                    the unit's sets and animation files, in search order (§3.1)
  <SET>.toml                    one texture set (§3.2)
  <SET>/<HASH>.ktx2             its images, one KTX 2.0 file per texture (§3.3)
  <ID>.anims.toml               texture animations (§3.4)
  overrides.toml                mod packs only: overrides for this view scope (§3.5)
  images/**                     mod packs only: override images (PNG, DDS, KTX 2.0)
```

- **Set ids** (`[A-Za-z0-9][A-Za-z0-9_-]{0,62}`, also the file stem) are chosen by the writer; they are not keys and
  no reference names them. The importer uses: the stream section's name for a region's packs (`A21`, `Y0`); the
  original file's stem elsewhere (`GLOBALA`, `TEXTURES`, `VINYLS`; a shop file's stem inside its sub-unit:
  `frontend/platforms/CarLot01/textures/CarLot01`, `frontend/envmaps/Showroom/textures/Showroom`); for scenes the stem of the pack's own path (`SharedTextures`). The
  second and further packs of one group get `_2`, `_3`, … (`A25_2`, `GlobalB_2`).
- **Image file names** are the texture's hash as 8 upper-case hex digits (`3BE35F00.ktx2`): names are not unique
  and contain characters a file name should not (`ARC_CONC&BLUEBLDG_A_RJ`, `SGN_ETHNIC_TACO'S_A_RJ`).
- A **group** is what the engine loads together: one original file, or one stream section of a region. Sets keep
  the original's order inside their unit (the index).

### 2.2 Keys

| Item | Key | Reference form |
|---|---|---|
| texture | its 32-bit hash: `bin_hash` of its full name (conventions §6.4) | `texture:0x3BE35F00`; `texture:NAME` is accepted in mod packs and means `texture:` + `bin_hash(NAME)` (a key of exactly `0x` + 8 upper-case hex digits is always a hash) |
| texture animation | its base hash (the hash of frame 0's texture in every record of the reference install) | `texture-anim:0x636C197D` |
| texture set | (unit, set id) | – (not referenced) |
| override entry | (view scope, hash, renderer) | – |

A hash is unique inside a set (TEX.4). The same hash may appear in several sets — 2,166 hashes do in the reference
install (§2.3) — so a texture key alone does not name one record: **references are resolved in a view** (§2.3).

### 2.3 Views, search paths and how a reference resolves

**What the original does (verified).** All loaded texture packs sit on one list. A pack is appended when it is
attached (constructor 0x48FC40 links it at the tail of the list at 0x830AF0). `GetTextureInfo(hash, return_default,
include_unloaded)` (0x4901D0) first rejects hashes no loaded texture can have (an 8,192-entry count table indexed by
`hash & 0x1FFF`, 0x833784, raised as textures attach, 0x48FD84), then walks the list from the front and binary-searches
each pack's sorted hash table (0x48FE00 → 0x488BC0, 8-byte entries); on a hit it **moves that pack to the front of
the list** (0x490271–0x490292) and marks the texture used (+0x53 = 1). Without a hit it searches the engine's
named run-time textures (0x82D530/0x82D5C0, ≤ 36 entries), then returns the default texture when asked to: the
texture with hash `0x66994F4A` (`DEFAULTALPHA`, GLOBALB: 32 × 32 DXT3, all texels transparent black, blend 2),
which the pack loader records when it attaches it (0x48FDE0); with no default loaded it calls 0x43BD50 [open: an
error exit]. So a hash held by two loaded packs resolves to whichever of the two was hit most recently: the
original's answer depends on lookup history.

**Why a fixed order is exact for the base set (measured).** In the reference install 2,166 hashes occur in more than
one pack. 2,106 are identical everywhere (record apart from its offsets, and pixels), 27 have the same pixels and a
different record, 33 differ. Inside one region every duplicate is identical. The 60 that are not identical occur
only between groups that today's loaders never put in one view: the region files `GlobalTexturesNA`/`ROW` (one is
loaded by region, 0x57FC5E), the magazines' `HIGH`/`LOW` sets, the per-car HUD files against
`HUD_CustomTextures_ALL`, the shops' environment faces (`XPOS` … `ZNEG` in every shop and `ENVMAPS` file), the shop
files against each other, against the regions' shared stream sections and against `InGameCommon` (same pixels, other render
state: `FE_WHITE`, `SHD_SHADOWTILE_A`, `OBJ_TELEPOLE01`, `SFX_LIGHT_BEAMA`, …), `FrontB` against the in-game HUD
files (`3RDPERSON_TURBORED_1`, `3RDPERSON_REDLINE_*`, `SFX_SMOKE_B_CS`), `LOC4DYNTEX` against `FrontB`
(`TRACK4000_MAP`), and one fence texture across regions. The region and magazine pairs are exclusive by
construction; the others are believed exclusive in the original too [to verify: whether a region stays loaded under
a shop, the HUD files, the shop and environment files, `FrontB` against the in-game files, `TRACK4000_MAP`]. Where
no view holds two different textures under one hash, the original's move-to-front search and a fixed first-match
order give the same texture.

**The rule (normative).** A reference is resolved in a **view**: the situation in which the referencing item is
drawn. Each view has a **search path**, an ordered list of loaded sets; the first set in the path that holds the
hash supplies the texture. Overrides from mod packs apply before the search path (§3.5). A hash found nowhere is a
**miss**: the engine does what the original does where it asked for the default — it draws `DEFAULTALPHA` (with its
own blend state), which is therefore a required item of every base set ([manifest.md](manifest.md) §5).

| View | What it draws | Renderer | Search path, in order |
|---|---|---|---|
| `world/<REGION>` | everything of a loaded region except cars: scenery, sky, roads, particles, glows, rain, reflections | world | the region's sets whose stream sections are loaded, in index order; then `global`: `GlobalB`, `GlobalB_2`, `GLOBALA`, `InGameCommon`, `InGameCommon_2`, `LOC4DYNTEX`, and `GlobalTexturesNA` (region 0) or `GlobalTexturesROW` (else) (src/world/parse.rs:35, :236, :414) |
| `cars/<NAME>` | the car `NAME` wherever it is drawn (region, shop, scene), its shadow included | car | `cars/<NAME>` `TEXTURES`; `cars/common` `TEXTURES`; `cars/WHEELS` `TEXTURES`; then `global` `GlobalB`, `GlobalB_2` (src/car/load.rs:188, :425). Vinyl images are looked up only in `cars/<NAME>` `VINYLS` (src/car/skin.rs:138) |
| `scenes/<NAME>` | an in-engine scene's own models (characters) | world | the scene's sets in index order (src/character.rs:509); then the `world/<REGION>` path of the loaded region [to verify] |
| `frontend` (menus) | menus, loading screens, magazines | ui | `FrontB`, `FrontB_2`, `GlobalB`, `GlobalB_2`, `GLOBALA`, `InGameCommon`, `InGameCommon_2`, `InGameRace`, `InGameFreeRoam`, `InGameDrag`, `InGameDrift`, `HUD_CustomTextures_ALL`, `LanguageTextures`; `TRACKMAP<n>` when the screen asks for track *n* (src/frontend/assets.rs:62, :172); the magazine set of the detail level for the magazine screens (crates/ob-formats/src/formats/magazines.rs:98) |
| `frontend` (in-game screens) | the HUD, pause and result screens | ui | as the menus, except that a texture an in-game file holds in another form than `FrontB` resolves to the in-game file's copy (src/frontend/assets.rs:153) [to verify: the original holds `FrontB` unloaded in game] |
| `frontend` (shops) | a shop's or garage's models and environment map | world | the shop's sets (`frontend/platforms/<X>/textures/<X>`, its faces from `<X>_2` there or from `frontend/envmaps/<X>/textures/<X>`); `GlobalB`'s glow and neon textures; `FrontB` for the names the shop asks for (src/garage.rs:570, :502) |

- Sets a mod pack **adds** to a unit join that unit's part of every path, after the base set's sets (layering.md
  §3.3). Their hashes must be new in that view (TEX.16), so an added set never shadows anything; changing what an
  existing hash shows is an override's job.
- Views are named by the unit whose content is drawn; that name is also the **view scope** where overrides for it
  live (§3.5). `global` is not a view but a scope that reaches every view.
- A texture reference in the base set that resolves to nothing is **native** (six solid references in L4RA name
  textures no file holds; docs/formats/world.md "Open") and is not an error; in a mod pack it is `CONV.9` unless the
  same reference exists in the base set.
- `texture-anim:` references follow the same views; an animation applies to every reference to its base hash in the
  views that see its file (§3.4).

### 2.4 Example tree

```text
global/textures/index.toml, GlobalB.toml, GlobalB/66994F4A.ktx2, …, TRACKMAP4000.toml, …
frontend/textures/index.toml, FrontB.toml, FrontB_2.toml, …
frontend/platforms/CarLot01/textures/index.toml, CarLot01.toml, CarLot01_2.toml   frontend/envmaps/Showroom/textures/…
world/L4RA/textures/index.toml, A21.toml, A21/636C197D.ktx2, …, A25.toml, A25_2.toml, Y0.toml, A21.anims.toml
cars/PEUGOT/textures/index.toml, TEXTURES.toml, TEXTURES/…, VINYLS.toml, VINYLS/…
cars/common/textures/…   cars/WHEELS/textures/…   scenes/IntroNis01/textures/…
```

A mod pack:

```text
global/textures/overrides.toml, images/roads/ROAD_MAIN.png, images/plate.dds, …   every view (filtered by `draw`)
world/_any/textures/images/0x5FB8BCD1.png                                          every region
world/L4RA/textures/overrides.toml, images/water0.ktx2                             region L4RA only
cars/PEUGOT/textures/images/PEUGOT_SKIN1.png                                       the car PEUGOT only
```

## 3. Source form

### 3.1 The texture index (`ob.texture-index/1.0`, `<unit>/textures/index.toml`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.texture-index/1.0` |
| `set` | array<table> | – | no | `[]` | The unit's texture sets in search order (§2.3). |
| `set.id` | string | – | yes | – | The set's file `<id>.toml` and image folder `<id>/`. Unique in the unit (TEX.1). |
| `set.group` | string | – | yes | – | The load group (§2.1). |
| `set.section` | ref `section` | – | see Meaning | – | The stream section whose load brings the set in ([world.md](world.md): `section:L4RA/121`). In a `world/<REGION>` index: required for the base set's entries; optional for a mod pack's (absent: the entry belongs to the view whenever the region is loaded). Refused in every other index, `world/_any` and `global` included (TEX.1). |
| `set.op` | enum | – | in mod packs | – | `add` only (TEX.3). |
| `anims` | array<table> | – | no | `[]` | Texture-animation files `<id>.anims.toml`, same fields as `set` (`section` by the same rule), in the order their records apply. |

Where an index may stand: in every unit that holds sets (base set and mod packs), and, in mod packs, also in the
wildcard scopes `world/_any`, `cars/_any` and `scenes/_any` (§3.5); a wildcard index lists only `anims` entries,
never `set` entries (sets belong to a real unit; TEX.1). An animation file listed in a scope that is not a stream
unit (`global`, a wildcard, `cars/<NAME>`, `scenes/<NAME>`, `frontend`) applies whenever a view of that scope is
drawn (§3.4).

Limits: 4,096 sets and 4,096 animation files per unit. Example
([examples/textures/base/world/L4RA/textures/index.toml](examples/textures/base/world/L4RA/textures/index.toml)):

```toml
schema = "ob.texture-index/1.0"

[[set]]
id = "A25"
group = "A25"
section = "section:L4RA/125"

[[set]]
id = "A25_2"                 # the second pack of stream section A25
group = "A25"
section = "section:L4RA/125"

[[anims]]
id = "A21"                   # A21.anims.toml
group = "A21"
section = "section:L4RA/121"
```

### 3.2 A texture set (`ob.texture-set/1.0`, `<unit>/textures/<SET>.toml`)

One original texture pack, or a set an author makes. Records are kept in the original's order, which is ascending
hash in every pack of the reference install (order is data: the engine's model is a list).

**Set fields.**

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.texture-set/1.0` |
| `tpk` | table | – | in importer-made sets | absent | The original pack's header, for the encoder (§5.3); authored sets leave it out. |
| `tpk.version` | u32 | – | no | 5 | Info +0x00: 5 in 915 packs, 4 in six vinyl packs. |
| `tpk.name` | string | – | yes | – | Info +0x04 (≤ 27 characters): the pack's category (`TRACK`, `GLOBAL`, `VINYL`, …). |
| `tpk.path` | string | – | yes | – | Info +0x20 (≤ 63 characters, printable ASCII): the pack's source path in the original's tools (`Location4\TempTextures.tpk`). |
| `tpk.layout` | enum | – | no | `plain` | `plain` (entries `0x33310004`/`0x33310005`, one data block), `blocks` (per-texture compressed blocks `0x33310003`), `blocks_and_table` (both; the version-4 vinyl packs, §5.5). |
| `texture` | array<table> | – | yes | – | The records, below. At most 16,384. |

**Texture record.** Fields the engine reads are named; fields it does not read and whose meaning is not traced are
`u_0xNN` (conventions §8). Offsets are into the original's 0x7C-byte entry (§5.2).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `hash` | hash32 | – | yes | – | +0x24, the key. Unique in the set (TEX.4). |
| `name` | name | – | no | none | +0x0C: the name as the original stores it, ≤ 23 characters (a `name`, conventions §3.2: printable ASCII 0x20–0x7E, no leading or trailing space; it may be cut: `3RDPERSON_TURBODIAL_CUS`). The engine keeps this text (`Texture::name`; conventions §6.3). |
| `full_name` | name | – | no | none | The complete name (24–63 characters) when known; `bin_hash(full_name)` MUST equal `hash` and `name` MUST be its first 23 characters (TEX.9). For tools only. |
| `image` | string | – | yes | – | The image, a logical path relative to this file's folder: KTX 2.0 (§3.3; what the importer writes), or PNG or DDS in authored sets (§3.6). One image per record (TEX.5). |
| `width_px`, `height_px` | u16 | px | yes | – | +0x44 / +0x46. Equal to the image's level-0 size (TEX.7). Menus compute texture coordinates from them (docs/formats/career-2.md). |
| `alpha_usage` | u8 | – | no | 0 | +0x55: 1 = alpha test, texels with alpha ≤ 95 discarded (0x5BCC40, **verified**); 2 = no blending, the alpha channel is a reflection mask (roads, lane paint, water; docs/formats/world.md "Texture packs"); other values: no test. Values 0/1/2 occur (41,393 / 727 / 19,881). |
| `blend` | u8 | – | no | 0 | +0x56, frame-buffer blending (0x5BCC40, **verified**): 0 none; 2 SRCALPHA/ONE; 5 DESTALPHA/INVDESTALPHA; 6 DESTALPHA/ONE; 1 and any other value SRCALPHA/INVSRCALPHA. Every blended mode turns depth writes off. Kept as the byte (3 occurs three times). |
| `rendering_order` | u8 | – | no | 0 | +0x51, the draw-order bucket (glows use 5). |
| `late_buckets` | u8 | – | no | 0 | +0x54: non-zero puts the texture's draws in the second set of buckets, flushed after the first (0x5CF430, **verified**). |
| `scroll` | table | – | no | no scrolling | Texture-coordinate scrolling (0x5BCC40 → 0x5B7D50/0x5B7D90 → 0x48B450, **verified**), integers with their scales so that every value comes back exactly: |
| `scroll.kind` | u8 | – | no | 0 | +0x52: 0 none; 1 `x = t·speed`, offset `x − trunc(x)`; 2 the same with `t` cut to whole steps; 3 a fixed offset (`−fixed/1024`). `t` is seconds. |
| `scroll.step_256th_s` | i16 | 1/256 s | no | 256 | +0x58. |
| `scroll.speed_1024th` | [i16, i16] | 1/1024 per s | no | [0, 0] | +0x5A, +0x5C (u, v). |
| `scroll.fixed_1024th` | [i16, i16] | 1/1024 | no | [0, 0] | +0x5E, +0x60; the engine adds `−value/1024`. |
| `u_0x28` | u32 | – | no | `0x001A93CF` | +0x28, **open** (59,445 textures hold the default; tools of the modding community call it a class hash [guess]). |
| `u_0x2c` | u32 | – | no | 0 | +0x2C, **open**: the pack loader looks it up in the same pack and keeps the result at run time in +0x6C, itself when 0 (0x48FD86–0x48FDC0, **verified**); 0 in every texture of the reference install. |
| `u_0x4f` | u8 | – | no | 0 | +0x4F, **open** (0–3; an addressing mode [guess]). |
| `u_0x50` | u8 | – | no | 0 | +0x50, **open** (1 in two textures). |
| `u_0x57` | u8 | – | no | 1 | +0x57, **open** (0 in 19,660 textures; in plain packs almost only where `alpha_usage` is 2). |
| `u_0x62`, `u_0x64` | i16 | – | no | 256 | +0x62, +0x64, **open** (256 in every texture; scales in 1/256 [guess]). |
| `tpk` | table | – | no | derived | Encoder data that the rules of §5 do not derive: |
| `tpk.compression` | u8 | – | no | from the image | +0x4A when it is not the image format's usual byte (§3.3): `0x80` / `0x81` (vinyl images, P8). |
| `tpk.data_offset`, `tpk.palette_offset` | u32 | byte | no | the layout rule (§5.4) | +0x30 / +0x34 when they differ from the rule (2 plain packs and every block-compressed pack). |
| `extras`, `ext` | table | – | no | – | conventions §9. |

Limits: 16,384 records per set; sides 1–8,192 texels; `name` 23 characters. Example (excerpt of
[examples/textures/base/world/L4RA/textures/A21.toml](examples/textures/base/world/L4RA/textures/A21.toml)):

```toml
schema = "ob.texture-set/1.0"

[tpk]
version = 5
name = "TRACK"
path = 'Location4\TempTextures.tpk'

[[texture]]
hash = "0x636C197D"
name = "ANM_WATERA_"
image = "A21/636C197D.ktx2"     # BC2 (DXT3), 7 levels
width_px = 64
height_px = 64
alpha_usage = 2
blend = 1
u_0x57 = 0

[[texture]]
hash = "0xEA1CB863"
name = "SGN_DOCKS_NEW"
image = "A21/EA1CB863.ktx2"     # R8_UINT indices + OBpalette (P8)
width_px = 256
height_px = 64
alpha_usage = 1
rendering_order = 5
scroll = { kind = 1, speed_1024th = [256, 0] }
tpk = { data_offset = 0x2400, palette_offset = 0x0 }
```

### 3.3 Images: the KTX 2.0 profile

Every image the importer writes is a KTX 2.0 file (https://registry.khronos.org/KTX/specs/2.0/ktxspec.v2.html)
with these restrictions. Readers MUST refuse anything else (TEX.6).

| Original (+0x4A) | vkFormat (Vulkan `VkFormat`, https://registry.khronos.org/vulkan/specs/latest/man/html/VkFormat.html) | Bytes |
|---|---|---|
| `0x22` DXT1 | `VK_FORMAT_BC1_RGBA_UNORM_BLOCK` (133): DXT1 with its 1-bit-alpha mode, as D3D's DXT1 | the original 8-byte blocks, unchanged |
| `0x24` DXT3 | `VK_FORMAT_BC2_UNORM_BLOCK` (135) | the original 16-byte blocks, unchanged |
| `0x26` DXT5 (accepted by the parser; none in the reference install) | `VK_FORMAT_BC3_UNORM_BLOCK` (137) | unchanged |
| `0x20` 32-bit | `VK_FORMAT_B8G8R8A8_UNORM` (44) | the original texels (B, G, R, A), unchanged |
| `0x08`, `0x80`, `0x81` P8 | `VK_FORMAT_R8_UINT` (13) + the key `OBpalette` | the original indices, unchanged; the palette's 1,024 bytes (256 × B, G, R, A) unchanged in the key's value |
| – (mod packs and authored sets) | `VK_FORMAT_BC7_UNORM_BLOCK` (145), `VK_FORMAT_R8G8B8A8_UNORM` (37), and the `_SRGB` twins of 133, 135, 137, 145, 37, 44 (134, 136, 138, 146, 43, 50), read as the same bits | as written |

- **Header**: `typeSize` 1; `pixelWidth`, `pixelHeight` ≥ 1; `pixelDepth` 0; `layerCount` 0; `faceCount` 1;
  `levelCount` ≥ 1 and at most the full chain (`levelCount` 0, "generate mips", is refused). The original has no
  cube maps: a shop's or garage's environment map is six 2D textures named `XPOS`, `XNEG`, `YPOS`, `YNEG`, `ZPOS`,
  `ZNEG`, which the engine builds into a cube (0x5B9920).
- **Levels** are the original's levels, all of them (the entry's +0x4E; every texture of the reference install
  holds exactly the levels its size says), in KTX's order: the level index lists level 0 (largest) first, the
  level data is stored smallest first (KTX §3.9.7, §3.13). Level sizes are exact (`byteLength` and
  `uncompressedByteLength` equal the format's size for the level).
- **Supercompression**: 0 (none) or 2 (Zstandard, RFC 8878 https://www.rfc-editor.org/rfc/rfc8878). The importer
  writes 0 for images of plain packs (zero-copy, §7) and 2 for images of block-compressed packs (where the original
  itself kept them compressed: vinyls, car textures, the in-game HUD files, magazines, language, `LOC4DYNTEX`,
  `DYNTEX`); level 3 [guess at the best speed/size point]. KTX 2.0 is the open form for GPU textures
  (conventions §4.6 rule 2; rule 3 is for media bitstreams); it also costs less than the original's own
  compression: Zstandard of the decoded levels is 0.73× (vinyls) and 0.82× (car textures) of the original
  JDLZ/HUFF streams (§11).
- **Data format descriptor**: the basic descriptor Khronos's `vk2dfd` gives for the vkFormat; colour primaries
  BT.709, transfer function linear for `_UNORM` formats. The game samples its textures without sRGB decoding;
  the bytes are what it samples.
- **Key/value data**: `KTXorientation` = `rd` (absent means `rd`); `KTXwriter` SHOULD name the writer; `OBpalette`
  (binary, exactly 1,024 bytes) MUST be present for `R8_UINT` and absent otherwise (TEX.8). Unknown keys are
  ignored, as KTX requires.
- **Why indices and a palette, not RGBA**: KTX 2.0 has no paletted format and forbids emulating one
  (`VK_FORMAT_UNDEFINED` "must not be used as a workaround", KTX §4.2). Expanding to RGBA8 would lose the engine's
  model (`Texture::data` holds the indices, `Texture::palette` the palette: Eq1, §6; palettes with duplicate entries
  cannot be recovered) and grow the 18.2 GB of decoded vinyl indices four times. `R8_UINT` keeps every byte and
  views as grey in ordinary KTX viewers; Studio shows it through the palette.
- **Matching the record**: level-0 size equals `width_px` × `height_px` (TEX.7); the original compression byte is
  the format's usual one (`0x22`, `0x24`, `0x26`, `0x20`, `0x08`) unless `tpk.compression` says otherwise (TEX.10).

### 3.4 Texture animation (`ob.texture-anims/1.0`, `<unit>/textures/<ID>.anims.toml`)

Flip-books (water, blinking signs, the star layer): a record per animation in the original's order.

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.texture-anims/1.0` |
| `chunk.u_0x00` | u32 | – | no | 1 | Header `0x30300101` +0x00, **open** (1 in all nine chunks; not the record count). |
| `anim` | array<table> | – | yes | – | Records, below. |
| `anim.hash` | hash32 | – | yes | – | Record +0x18: the base hash, the key. References to this hash in the views that see the file are animated. |
| `anim.name` | name | – | no | none | Record +0x00 (≤ 23 characters). |
| `anim.fps` | u32 | 1/s | yes, except in `merge` entries | – | Record +0x20: frames per second. Frame `i = trunc(t · fps) mod n` (crates/ob-formats/src/formats/texture_anim.rs:35). |
| `anim.frames` | array<ref `texture`> | – | yes, except in `merge` entries | – | The frames in order (the frame table `0x30300103`); the record's count +0x1C is their number. Each resolves in the view like any reference; a view draws the animation only when more than one frame resolves (src/render/native/world/build/textures.rs:61). |
| `anim.op`, `anim.disabled` | | | mod packs | | §4. |

Two records with the same base hash in one view: the later in index order applies (the world loader's rule,
src/world/parse.rs:239; two such pairs exist in the reference install) [to verify against the original].
Example ([examples/textures/base/world/L4RA/textures/A21.anims.toml](examples/textures/base/world/L4RA/textures/A21.anims.toml)):

```toml
schema = "ob.texture-anims/1.0"

[[anim]]
hash = "0x636C197D"
name = "ANM_WATERA_"
fps = 12
frames = ["texture:0x636C197D", "texture:0xC9E6D6EE", "texture:0xC9E6D6EF"]
```

### 3.5 Overrides (`ob.texture-overrides/1.0`, mod packs)

An override changes what a hash resolves to in the views of its **scope**. It lives in
`<scope>/textures/overrides.toml`, with its images under `<scope>/textures/images/`.

| Scope folder | Applies in |
|---|---|
| `world/<REGION>` | the view `world/<REGION>` |
| `world/_any` | every `world/<REGION>` view (layering.md §3.5) |
| `cars/<NAME>`, `cars/_any` | the view `cars/<NAME>`, every car view |
| `scenes/<NAME>`, `scenes/_any` | the view `scenes/<NAME>`, every scene view |
| `frontend` | the three `frontend` views (the shops' included) |
| `global` | every view |

Other units (`cars/common`, `cars/WHEELS`, the shop sub-units `frontend/platforms/<X>` and `frontend/envmaps/<X>`)
are not views; overrides there are refused (TEX.13): the textures of `cars/common` and `cars/WHEELS` are changed
through `cars/_any` (every car view sees them), a shop's through `frontend`.

A hash **resolves below** a pack in a view when, with the layers under that pack, the view's search path (§2.3) or
an override entry supplies a texture for it there (a `disabled` one included). For a scope with several views
(`global`, a wildcard, `frontend`) the condition is taken over the scope's views: it resolves below when it does in
**at least one** of them.

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.texture-overrides/1.0` |
| `texture` | array<table> | – | yes | – | Entries, below. At most 65,536. |
| `texture.hash` / `texture.name` | hash32 / name | – | exactly one of them | – | The key; a name (≤ 63 characters, the full name) is hashed with `bin_hash` exactly as written (the game's names are upper case). |
| `texture.image` | string | – | for `add` and `replace` | the image of the same key in `images/`, if any | PNG, DDS or KTX 2.0 (§3.6), relative to this file's folder. Absent with no image of the key: the pixels stay the lower layers'. |
| `texture.draw` | array<enum> | – | no | all three | The renderers the entry applies to: `world`, `car`, `ui`. Two entries for one hash in one file MUST have disjoint `draw` (TEX.14). |
| `texture.compress` | enum | – | no | `bc7` | PNG only: `bc7` (compiled to BC7 with its mip chain, §7) or `none` (kept as RGBA8, mips made at load). |
| `texture.width_px`, `texture.height_px` | u16 | px | no | the image's size | `add` only: the new texture's record size. On `merge`/`replace` the base record's size stays (menus compute coordinates from it; an upscaled image keeps its atlas layout) and these fields are refused (TEX.18). |
| `texture.alpha_usage`, `blend`, `rendering_order`, `late_buckets`, `scroll` | as §3.2 | | no | the lower layers' value (`merge`); §3.2's defaults (`add`, `replace`) | Render state. Absent fields keep the record native: an override changes pixels only unless it names a field. |
| `texture.op` | enum | – | no | `merge` | layering.md §3.1: `merge` and `replace` need the hash to resolve below in the scope, in at least one of its views (`MRG.1`); `add` needs it to resolve below in none of them (TEX.16). Legacy packs (§4.1) skip and count an entry that resolves nowhere, as today (conventions §11). `add` and `replace` need an image (TEX.15). |
| `texture.disabled` | bool | – | no | false | The hash becomes a miss in the scope's views (the original's miss: `DEFAULTALPHA`, which draws nothing). |
| `extras`, `ext` | table | – | no | – | conventions §9. |

**Image-only entries.** Every file under `images/` (sub-folders allowed, at most 8 deep and 65,536 files per scope,
names starting with `.` skipped) is an entry of its own for the key its stem names, with all defaults (`op =
"merge"`, every renderer), unless an entry of `overrides.toml` names that key. A stem is read as a hash when it is
`0x` and 1–8 hex digits, or exactly 8 hex digits, in either case (`0x5fb8bcd1`, `5FB8BCD1`, `0xABC` = `0x00000ABC`);
any other stem is a name, hashed as written (src/obpack/index.rs:22). This relaxed hash form is a **legacy
relaxation for image file stems only**, kept so that today's packs' file names keep their meaning; every other
hash in the format, `texture.hash` included, is a strict `hash32` (conventions §3.2). Two images for one key in one
scope (`ROAD.png` and `ROAD.dds`, or a name and its hash) are refused (TEX.14).

**Resolution (normative).** For a reference to hash `h` drawn by renderer `r` in view `V`, start from the record
the search path gives (§2.3), or a miss. Then, **for each layer from the bottom of the stack to the top**, take that
layer's entries for `h` whose `draw` holds `r` in this order: `global`, then `V`'s wildcard scope, then `V`'s own
scope; apply each entry's fields in turn, a `replace` entry first resetting the record's render state to §3.2's
defaults, a `disabled = true` entry turning the result into a miss and a later `disabled = false` turning it back.
So inside one pack the specific scope wins over the wildcard, which wins over `global`, and an entry of a stronger
pack, of any scope, wins over every entry of a weaker one (layering.md §3.5). The image of the last applied entry
that has one supplies the pixels.

Example ([examples/textures/mod/global/textures/overrides.toml](examples/textures/mod/global/textures/overrides.toml)
and [examples/textures/mod/world/L4RA/textures/overrides.toml](examples/textures/mod/world/L4RA/textures/overrides.toml)):

```toml
schema = "ob.texture-overrides/1.0"

[[texture]]
name = "ROAD_MAIN"                  # key bin_hash("ROAD_MAIN") = 0x652CF9A9
image = "images/roads/ROAD_MAIN.png"
draw = ["world"]                    # only where the world renderer draws it

[[texture]]
hash = "0x1A33CBB5"                 # 3RDPERSON_TURBORED_1
image = "images/hud/turbo.ktx2"
draw = ["ui"]
blend = 2                           # an explicit render-state change

[[texture]]
name = "SGN_HARBOUR_NEW"            # a texture no lower layer has
op = "add"
image = "images/signs/harbour.png"
alpha_usage = 1
```

### 3.6 Image inputs (overrides and authored sets)

| Input | Accepted | Becomes (cooked, §7) |
|---|---|---|
| PNG, 8 bit (any colour type), ≤ 4,096 × 4,096, ≤ 128 MiB decoded | yes | BC7 with its full mip chain (alpha-weighted 2 × 2 boxes; src/obpack/dds.rs:83) when both sides are whole 4 × 4 blocks and `compress = "bc7"`; otherwise RGBA8, mips made at load |
| DDS, DX9 FourCC `DXT1` / `DXT3` / `DXT5`; DX10 DXGI 71, 72, 74, 75, 77, 78, 98, 99 (BC1/BC2/BC3/BC7, UNORM or SRGB); whole 4 × 4 blocks; its stored levels | yes | as it is (src/obpack/dds.rs:28) |
| KTX 2.0 per §3.3 (any listed vkFormat, supercompression 0 or 2) | yes | as it is |
| TGA, JPEG, BMP, DDS arrays or cube maps, other formats | no (TEX.12) | – |

Sides above 4,096 or the GPU's limit are refused for overrides (TEX.11); sets allow 8,192. An override for a P8
texture may use any input: the engine then draws it as an RGBA texture [open: vinyl recolouring through the palette,
[cars.md](cars.md)].

## 4. Layering

| Item | `merge` | `add` | `replace` | `disabled` | Clash key | Wildcards |
|---|---|---|---|---|---|---|
| texture set (index entry) | refused (TEX.3) | yes: a new set in the unit, searched after the base set's (§2.3); every hash in it must be new in that unit's views (TEX.16) | refused (TEX.3) | refused (TEX.3) | (unit, set id) | no |
| texture record in a base set | refused: a pack cannot hold a file of a base set's set (TEX.3); records change through overrides | – | – | – | – | – |
| override entry | yes (§3.5) | yes | yes | yes | (scope, hash, renderer) | `world/_any`, `cars/_any`, `scenes/_any`, and `global` |
| texture animation | yes (`fps`, `frames` replaced whole; an animation of that base hash must exist below in at least one of the scope's views, `MRG.1`) | yes (`fps`, `frames` required, TEX.19) | yes (as `merge`) | yes (the frames stay still at the reference's own texture) | (scope, base hash) | as overrides |

- Animation records in a mod pack live in an `<ID>.anims.toml` of a view scope (a wildcard and `global`
  included), listed in that scope's `index.toml` with `op = "add"` (§3.1); each record carries its own `op`.
- **Clashes** are reported per (scope, hash, renderer) between packs, and between a pack's `global` or wildcard
  entry and another pack's specific entry for the same view (layering.md §4).
- **Native by construction**: the base set has no overrides; with "Use mods" off every view resolves through the
  search paths of §2.3 alone.

### 4.1 Today's texture registries as legacy packs (layering.md §7)

Today's packs select replacements by **renderer**, not by file: a world pack applies to whatever the world renderer
draws (regions, shops, scenes; GLOBAL's textures included), a car pack to the car renderer, the HUD mod to the
interface (src/render/enh/assets/texture_pack/mod.rs:25, roots.rs:30; src/tpf_import.rs:32). The exact mapping is
therefore to the `global` scope with a `draw` filter:

| Today | Mounted as |
|---|---|
| Overbright pack format 1 (`packs/<id>/`, src/obpack/) | mod pack `<id>`: `world/**` → image-only entries in `global/textures` with `draw = ["world"]`; `car/**` → `draw = ["car"]`; `any/**` → both, where `world/` or `car/` has no image of the key (src/obpack/index.rs:15); `compress` from the manifest |
| Imported TexMod / TexWizard packages (`texture-pack/imported/<id>/{world,car}`, src/texture_mods.rs:119) | one mod pack each, as above |
| The HUD mod (`texture-pack/hud-mod`, src/hud_mod.rs:40) | mod pack `hud-mod`: entries with `draw = ["ui"]` (its map calibration: [career.md](career.md)) |
| Generated upscale art (`texture-pack/{world,car,ui}`) | mod pack `upscale-art`, `world`/`car`/`ui` draw filters |

Their order in the stack is today's loader order: Overbright packs (their `order.txt`), then the launcher's
packages, then the upscale art. The developer variables (`NFSU2_*_TEXTURE_PACK`) are not packs: harness builds
mount them as a temporary top layer. Conversion writes exactly these entries; a test compares the merged overrides
of both forms.

## 5. Import

### 5.1 Decoding

The importer walks every chunk tree (whole-file JDLZ unwrapped first: `FrontB.lzc`, `GlobalB.lzc`,
`InGameCommon.lzc`), and for each `0xB3300000` runs Overbright's parser (crates/ob-formats/src/formats/texture.rs:183
`parse_texture_pack`; blocks: :230 with crates/ob-formats/src/formats/jdlz.rs and crates/ob-formats/src/formats/huff.rs), giving the engine's model
(`Texture`, :157), the **Eq1 reference** (§6). It then writes one set file and one image per texture: the record's
fields from the entry, the image's levels from the pack's data (`data[0..size]`, levels largest first, re-ordered
smallest first into the KTX file), the palette into `OBpalette`. Packs of two files of one unit whose textures are
identical (`GlobalB.lzc`/`GLOBALB.BUN`, `InGameCommon.lzc`/`.bun`) are written once; both files' residual maps name
the same items. For each `0xB0300100` it runs crates/ob-formats/src/formats/texture_anim.rs:43.

### 5.2 The texture entry (0x7C bytes, `0x33310004` and the end of each version-5 block)

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 12 | `u8[12]` | run-time | – | Zero in every file (checked). At run time: the platform texture (+0x00, read at bind, 0x5BCC53) and list links. |
| 0x0C | 24 | `char[24]` | `name` | – | NUL-terminated, the rest zero: +0x23 is 0 in all 62,001 entries; the bytes after the NUL are zero in all 10,179 entries checked (every plain pack and every non-vinyl block). |
| 0x24 | 4 | `u32` | `hash` | – | |
| 0x28 | 4 | `u32` | `u_0x28` | – | |
| 0x2C | 4 | `u32` | `u_0x2c` | – | |
| 0x30 | 4 | `u32` | data offset | byte | `tpk.data_offset` or the rule (§5.4). |
| 0x34 | 4 | `u32` | palette offset | byte | `tpk.palette_offset` or the rule. |
| 0x38 | 4 | `u32` | data size | byte | Derived: the sum of the levels' sizes (holds for all 62,001). |
| 0x3C | 4 | `u32` | palette size | byte | Derived: 1,024 for `R8_UINT`, else 0. |
| 0x40 | 4 | `u32` | level-0 size | byte | Derived: the largest level's byte size (all 10,179 entries checked; the vinyl entries' values fit the same rule). |
| 0x44 | 2 | `u16` | `width_px` | px | |
| 0x46 | 2 | `u16` | `height_px` | px | |
| 0x48 | 1 | `u8` | log2 width | – | Derived (every side is a power of two in the reference install; others: TEX.10 when a `tpk` table asks for an encoding). |
| 0x49 | 1 | `u8` | log2 height | – | Derived. |
| 0x4A | 1 | `u8` | compression | – | From the vkFormat (§3.3) or `tpk.compression`. |
| 0x4B | 1 | `u8` | pad | – | 0 (checked). |
| 0x4C | 2 | `u16` | palette entries | – | Derived: 256 for `R8_UINT`, else 0. |
| 0x4E | 1 | `u8` | mip levels | – | Derived: the KTX `levelCount`. |
| 0x4F | 1 | `u8` | `u_0x4f` | – | |
| 0x50 | 1 | `u8` | `u_0x50` | – | |
| 0x51 | 1 | `u8` | `rendering_order` | – | |
| 0x52 | 1 | `u8` | `scroll.kind` | – | |
| 0x53 | 1 | `u8` | run-time "used" | – | 0 in files (checked); set by 0x4901D0 on a hit. |
| 0x54 | 1 | `u8` | `late_buckets` | – | |
| 0x55 | 1 | `u8` | `alpha_usage` | – | |
| 0x56 | 1 | `u8` | `blend` | – | |
| 0x57 | 1 | `u8` | `u_0x57` | – | |
| 0x58 | 2 | `i16` | `scroll.step_256th_s` | 1/256 s | |
| 0x5A | 4 | `i16[2]` | `scroll.speed_1024th` | 1/1024 per s | |
| 0x5E | 4 | `i16[2]` | `scroll.fixed_1024th` | 1/1024 | |
| 0x62 | 2 | `i16` | `u_0x62` | – | |
| 0x64 | 2 | `i16` | `u_0x64` | – | |
| 0x66 | 2 | `u8[2]` | pad | – | 0 (checked). |
| 0x68 | 20 | `u8[20]` | run-time | – | Zero in files (checked): the owning pack (+0x68, 0x48FDCA), the texture resolved from +0x2C (+0x6C), the platform texture and palette (+0x70, +0x74; 0x48FDCD, 0x48FDD4; tested by 0x48FE00), +0x78 [open]. |

Size 0x7C, alignment 4. What the engine reads (crates/ob-formats/src/formats/texture.rs:295): +0x0C, +0x24, +0x30–+0x3C (to cut the
data), +0x44, +0x46, +0x4A, +0x4E, +0x51, +0x52, +0x54, +0x55 (`== 1`), +0x56, +0x58–+0x61.

**Format record** (0x20 bytes, `0x33310005`, and after the entry in each version-5 block). Every field is derived;
the derivation held for all 10,639 records checked (every plain pack, every non-vinyl block, a vinyl sample). The
engine does not read it (it binds from +0x56, 0x5BCC40).

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u8[8]` | pad | – | 0 |
| 0x08 | 4 | `u32` | blend on | – | 1 when `blend` ≠ 0, else 0 |
| 0x0C | 4 | `u32` | source blend | – | 5 (`D3DBLEND_SRCALPHA`) |
| 0x10 | 4 | `u32` | destination blend | – | 2 (`D3DBLEND_ONE`) when `blend` = 2, else 6 (`D3DBLEND_INVSRCALPHA`) |
| 0x14 | 4 | `u32` | D3D format | – | `DXT1`/`DXT3`/`DXT5` FourCC, 21 (`D3DFMT_A8R8G8B8`) or 41 (`D3DFMT_P8`) by the image format |
| 0x18 | 8 | `u8[8]` | pad | – | 0 |

Size 0x20, alignment 4.

### 5.3 The pack header and data

| Chunk | Contents | Source |
|---|---|---|
| `0x33310001` info | one info record (below) | set `tpk` |
| `0x33310002` hash table | per texture a hash slot (below), ascending (all 917 non-empty packs); the entries follow the same order (all 845 packs with an entry table) | derived from the records' order |
| `0x33310004`, `0x33310005` | the entries (§5.2) and format records, record order | records |
| `0x33320001` | one data header (below) | derived (all 921 packs) |
| `0x33320002` | `0x11` bytes up to the next 128-byte file offset, then the images (§5.4), zero bytes between them | records + images |

**Info record** (`0x33310001`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `tpk.version` | – | |
| 0x04 | 28 | `char[28]` | `tpk.name` | – | NUL, then zeros |
| 0x20 | 64 | `char[64]` | `tpk.path` | – | NUL, then zeros |
| 0x60 | 4 | `u32` | path hash | – | Derived: `bin_hash(tpk.path)` (all 921 packs) |
| 0x64 | 24 | `u8[24]` | run-time | – | 0 in files |

Size 0x7C, alignment 4.

**Hash slot** (`0x33310002`, one per texture):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | hash | – | the record's `hash` |
| 0x04 | 4 | `u32` | run-time | – | 0 in files (the run-time pointer slot) |

Size 8, alignment 4.

**Data header** (`0x33320001`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u8[8]` | pad | – | 0 |
| 0x08 | 4 | `u32` | constant | – | 1 in all 921 packs; written as 1 [open: meaning] |
| 0x0C | 4 | `u32` | path hash | – | Derived: `bin_hash(tpk.path)` |
| 0x10 | 8 | `u8[8]` | pad | – | 0 |

Size 0x18, alignment 4.

Padding chunks (id 0, zero-filled) before `0xB3310000` and `0xB3320000` and inside `0xB3320000` align the parts
to file offsets; they are residual `fill`/`kept` nodes (residual.md §3.1), about 100 bytes per pack.

### 5.4 The data layout rule (plain packs)

Palettes first, 1,024 bytes each, in entry order, from offset 0; then each texture's pixel data in entry order, each
at the next multiple of 128 after the previous one (the first after the last palette). An entry without a palette
records the palette cursor (the offset where the next palette would start) in +0x34. The rule holds for 843 of the
845 packs with an entry table; the two others and every block-compressed pack (whose entries keep the offsets of an
uncompressed layout the files do not contain) record `tpk.data_offset` and `tpk.palette_offset`.

### 5.5 Block-compressed packs (`0x33310003`)

Per texture one block record:

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | hash | – | Equal to the entry's in all 54,904 version-5 blocks |
| 0x04 | 4 | `u32` | block offset | byte | File offset of the compressed block |
| 0x08 | 4 | `u32` | packed size | byte | |
| 0x0C | 4 | `u32` | unpacked size | byte | |
| 0x10 | 4 | `u32` | flags | – | 0x100 in all |
| 0x14 | 4 | `u32` | pad | – | 0 |

Size 0x18, alignment 4.

Each block is JDLZ (2,520) or HUFF (52,384; the twelve version-4 blocks are HUFF too) and decodes to: the pixels at
0; for P8 the palette at `palette_offset − data_offset` (zero bytes between); then the 0x7C entry and the 0x20
format record. The six **version-4** packs (vinyls of NEON, MIRRORS, S2000, SPINNER — empty — and IMPREZA, LANCER,
six textures each) hold the entries in a plain `0x33310004`/`0x33310005` table and only pixels and palette in the
blocks (layout `blocks_and_table`). Today's parser reads the compressed path for them and takes pixel bytes for the
entry, so those twelve vinyl images are dropped as "unknown compression" (crates/ob-formats/src/formats/texture.rs:198, :298); see
§10.

### 5.6 Residual

Plain packs are fully re-encoded (§6): their nodes are `decoded` with `proof = "bytes"`, apart from the padding
chunks (§5.3).

In a block-compressed pack, **each block is a compressed node** (residual.md §3.1): `fate = "container"`,
`wrap = "jdlz"` or `"huff"`, `wrap_size` and `wrap_digest` identifying the compressed stream, and as children the
decoded block content — pixels, palette, entry and format record (pixels and palette only in the version-4 packs),
each `decoded` with `proof = "bytes"` (Eq2, §6), and `fill` nodes for the zero bytes between them. The blocks are the children of the `0x33320002` node, in file order, with `fill` nodes
for the bytes between them. The file therefore rebuilds to its expanded content, as residual.md §3.1 defines; the
compressed streams themselves are not kept. Only the `0x33310003` block records stay as `kept` bytes, since their
offsets and packed sizes describe the compressed streams, which no Overbright encoder produces: 54,916 × 0x18 bytes
≈ 1.3 MB.

### 5.7 Texture animation chunk

| Chunk | Contents | Source |
|---|---|---|
| `0x30300101` | one animation header (below) | file |
| `0x30300102` | one animation record per animation (below) | records |
| `0x30300103` | one frame slot per frame, the records' frames back to back | `frames` |

**Animation header** (`0x30300101`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `chunk.u_0x00` | – | |
| 0x04 | 12 | `u8[12]` | pad | – | 0 |

Size 0x10, alignment 4.

**Animation record** (`0x30300102`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 24 | `char[24]` | `name` | – | NUL, then zeros |
| 0x18 | 4 | `u32` | `hash` | – | |
| 0x1C | 4 | `u32` | frame count | – | Derived: the length of `frames` |
| 0x20 | 4 | `u32` | `fps` | 1/s | |
| 0x24 | 16 | `u8[16]` | run-time | – | 0 in files |

Size 0x34, alignment 4.

**Frame slot** (`0x30300103`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | frame hash | – | one element of `frames` |
| 0x04 | 12 | `u8[12]` | run-time | – | 0 in files |

Size 0x10, alignment 4.

## 6. Equality and verification

| Kind | Level | Proof |
|---|---|---|
| texture set | **Eq1** per set: `H(parse_texture_pack(original pack)) == H(set loaded from the base set)`, `H` over the model's flat encoding (crates/ob-formats/src/formats/texture.rs:470: name, hash, size, format, levels, pixel bytes, palette, render state, scroll) | at import, every set |
| texture set, plain pack | **Eq2**: the encoder (§5.2–5.4) gives back the pack chunk's bytes except the padding chunks (residual fill nodes) | at import, every pack |
| texture set, block-compressed pack | **Eq2** for each decoded block (entry, format record, pixels, palette, fills) and for the pack's other chunks; each block is a compressed node, only the block records are kept (§5.6) | at import |
| texture animation | **Eq1** (crates/ob-formats/src/formats/texture_anim.rs:64) and **Eq2** (fixed records, §5.7) | at import |
| search paths, overrides | **Eq3**: the shot, load and cache gates (parity.md §3); a check at import that no view of §2.3 holds two different textures under one hash (a warning, and no promise, for `custom` editions) | gates |

- The image bytes are the original bytes (BC blocks, BGRA texels, P8 indices, palettes): equality of images is
  equality of bytes; Zstandard is lossless and its output is checked by the Eq1 read-back.
- Eq1 compares the engine's model, so it would bake in today's dropped version-4 vinyl images (§5.5): the parser is
  fixed and checked against the original before the import's reference is taken (§10).
- Tests: per set, the round trip original → set → model and set → encoder → original bytes on the reference install;
  KTX 2.0 files checked by an outside validator in CI (Khronos `ktx validate`,
  https://github.com/KhronosGroup/KTX-Software) [guess at the tool's name and mode]; the cache test cooked ≡ fresh
  (cooked.md §6) for the texture sections.

## 7. Cooked form

- **Cook unit**: the load unit (`world/<REGION>`, `cars/<NAME>`, `cars/common`, `cars/WHEELS`, `global`,
  `frontend`, the shop sub-units `frontend/platforms/<X>` and `frontend/envmaps/<X>`, `scenes/<NAME>`), for one
  layer stack and target. Overrides are cooked with the unit of their scope;
  `global` and wildcard scopes into a small entry of their own per stack.
- **Model sections**: `tex.index` (sets, groups, stream sections, animation files); `tex.records` (the `Texture` records
  without pixels, in set order); `tex.levels` (per texture and level: an external reference — bundle, offset,
  length — plus the supercompression scheme and the uncompressed length); `tex.palettes` (P8 palettes, copied);
  `tex.anims`; `tex.overrides` (per scope: the merged entries, their payloads).
- **Zero-copy**: every level of a KTX 2.0 image with supercompression 0 in a stored bundle entry (the BC images of
  plain packs: regions, track maps, front end, global, shops, scenes, about 240 MB) is referenced, never copied
  (cooked.md §5.1). Zstandard levels are referenced compressed and inflated at upload. This needs the model to hold
  pixel data as per-level slices (KTX stores the smallest level first); Eq1 hashes them concatenated largest first,
  which is the original's byte order.
- **Override payloads**: DDS and KTX 2.0 images from stored entries are referenced; PNG images are compiled once
  (BC7 or RGBA8, the encoder id `bc7-fast-1` as today, src/obpack/mod.rs) and the result copied into the entry.
- **Build-specific (prepared, not normative)**: the world texture arrays and their map (`gpu.a*`, `w.tdmap`,
  crates/ob-core/src/cache/track.rs:24), P8 expanded through the palette at upload, BC decoded to RGBA8 for `web-nobc`, opaque
  DXT3/DXT5 override images turned into BC1 (src/upscale.rs:887), surface classification and HD/upscale data.
  Today's equivalents (`w.tex.*`, `w.td.<hash>` in the track cache, src/world/cache.rs:77; the cars' `textures`
  file caches; the menus' `ui2d` cache) become these sections.

## 8. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| TEX.1 | `index.toml`: an entry naming a missing file, a duplicate `id`, `section` missing where §3.1 requires it or present where it refuses it, a `set` entry in a wildcard scope's index. |
| TEX.2 | A set or `.anims.toml` file in `textures/` that the index does not list. |
| TEX.3 | An index entry in a mod pack without `op = "add"`, or a mod pack holding a file of a base set's set. |
| TEX.4 | Two records with one hash in one set; two animation records with one base hash in one file. |
| TEX.5 | An `image` that does not exist, lies outside the file's folder, or is named by two records. |
| TEX.6 | A KTX 2.0 file outside the profile (§3.3): identifier, vkFormat, `typeSize`, depth, layers, faces, `levelCount` 0 or above the full chain, supercompression other than 0 or 2, a level's length, a data format descriptor that does not match the vkFormat, `KTXorientation` other than `rd`, a level that does not inflate to its length. |
| TEX.7 | An image whose level-0 size differs from `width_px` × `height_px`. |
| TEX.8 | `R8_UINT` without a 1,024-byte `OBpalette`, or `OBpalette` on another format. |
| TEX.9 | `full_name` whose `bin_hash` is not `hash`, or whose first 23 characters are not `name`; a `name` longer than 23 characters or not a `name` (conventions §3.2). |
| TEX.10 | A `tpk` table the image contradicts: `tpk.compression` not P8 on a non-`R8_UINT` image (or the reverse), a side that is not a power of two, a `tpk` table on a PNG or DDS image. |
| TEX.11 | A PNG or DDS image that does not decode, exceeds 4,096 per side (overrides) or 8,192 (sets) or 128 MiB decoded, or (BC) is not whole 4 × 4 blocks. |
| TEX.12 | An image type that is not PNG, DDS or KTX 2.0, or a DDS array or cube map. |
| TEX.13 | `overrides.toml` or `images/` outside a view scope (§3.5). |
| TEX.14 | Two entries for one (scope, hash, renderer) in one pack, or two images for one key in one scope. |
| TEX.15 | An `add` or `replace` entry without an image. |
| TEX.16 | A texture a mod pack adds (an `add` entry or a record of an added set) whose hash already resolves below in at least one view of its scope (§3.5). |
| TEX.17 | An added or overridden hash equal to a name the engine registers for a run-time texture. |
| TEX.18 | `width_px` / `height_px` on a `merge` or `replace` entry. |
| TEX.19 | A scroll value outside `i16`, or an animation `add` without `fps` and `frames`. |
| TEX.20 | A view whose search path holds two different textures under one hash (import check; an error for known editions, a warning for `custom`). |
<!-- /generated -->

## 9. Versioning and extension

- Minor versions MAY add: accepted vkFormats for images (BC4/BC5 for extra maps), `draw` values, optional record
  fields with native defaults (a `merge` that changes a texture's record size, with the menus' coordinate rules
  stated), new view scopes, more `tpk` encoder fields.
- Major: any change to the key or to how a reference resolves (§2.3), to the KTX profile's required parts, or to the
  meaning of a record field.
- Extension point: `[ext.OB_*]` on records and override entries. Expected first: maps for the opt-in enhanced
  renderer (normal, roughness) attached to a texture by hash, ignored by the native path [proposal].

## 10. Open points

- **Meanings**: `u_0x28`, `u_0x2c` (the loader resolves it inside the pack; what reads +0x6C is not traced), `u_0x4f`,
  `u_0x50`, `u_0x57`, `u_0x62`, `u_0x64`, `chunk.u_0x00`, entry +0x78 at run time; what `alpha_usage` = 2 changes
  beyond "no test" (the reflection mask is documented for the world effect, not traced to this byte).
- **Views to verify in the original**: whether `FrontB` is unloaded while the in-game screens show, which shop and
  environment-map packs are resident together, the per-car HUD files against `HUD_CustomTextures_ALL`, the two
  `TRACK4000_MAP` copies, a scene's fallback to the region's path, and the "later animation wins" rule.
- **Version-4 vinyl packs**: the original's loader for version 4 is not traced; the fix (entries from the plain
  table, pixels and palette from the blocks) must be checked against the original's garage before it becomes the
  Eq1 reference.
- **Which `GLOBALB` the original loads** (both `GLOBAL\GLOBALB.BUN` and `GLOBAL\GLOBALB.LZC` are named in the
  executable): irrelevant for textures while their textures are identical; the importer checks that and keeps two
  sets when an edition differs.
- **Overrides of P8 textures** (vinyls): whether an RGBA replacement must keep the palette semantics depends on how
  vinyls are recoloured ([cars.md](cars.md)).

## 11. Sizes and timing

Reference install (measured with read-only scripts over every content file):

| What | Number |
|---|---|
| Files holding texture packs | 270: 8 region stream files, 129 track maps, `LOC4DYNTEX`, 24 in `GLOBAL`, `FrontB`, 2 magazine files, 8 shop files, 2 environment maps, `LanguageTextures`, 31 car texture files (with `CARS/TEXTURES.BIN` and the rims'), 31 vinyl files with packs, 32 scene bundles |
| Texture packs | 921: 840 plain (one of them an empty version-4 pack), 76 block-compressed, 5 version-4 packs with both tables (3 of them empty) |
| Textures | 62,001 (51,834 vinyl images, 10,167 others); 57,959 distinct hashes |
| By format (count, pixel bytes) | DXT1 5,944 / 134.8 MB; DXT3 3,471 / 146.3 MB; DXT5 0; 32-bit 537 / 296.6 MB; P8 `0x08` 26,132 / 9.07 GB, `0x80` 24,077 / 8.42 GB, `0x81` 1,840 / 0.64 GB; 52,049 palettes / 53.3 MB |
| Sizes, mips | every side a power of two, largest 1,024; 52,302 textures with 7 levels (vinyls, 512 down to 8) |
| Compressed blocks | 54,916: 54,904 in version-5 packs (52,384 HUFF, 2,520 JDLZ) and 12 in version-4 packs; ≈ 482 MB packed, 18.5 GB decoded (vinyls 430 MB → 18.2 GB) |
| Texture-pack bytes as stored (whole-file JDLZ counted unpacked) | 717 MB: regions 126.9, track maps 45.2, `LOC4DYNTEX` 3.5, global 23.0, front end 4.7, magazines 3.6, shops and environment maps 30.6, language 1.2, car textures 28.6, vinyls 433.5, scenes 16.4 |
| Texture animation | 9 chunks, 14 records, 75 frames, 2,360 bytes (three regions and one shop) |
| Duplicated hashes | 2,166 (2,106 identical, 27 same pixels, 33 different; §2.3) |
| Zstandard level 3 per level vs. the original codec | vinyls 0.73× (1,804 sampled blocks), car textures 0.82× (all 2,700); on a 460-block vinyl sample of whole textures: level 19 0.58×, zlib 9 0.83× |

**Base size** [guess, ±15 %]: plain-pack images stored raw ≈ 240 MB; block-compressed images under Zstandard ≈ 345 MB
(vinyls ≈ 316 MB, cars ≈ 23 MB, the rest ≈ 7 MB) plus the vinyl palettes in `OBpalette` 53 MB (stored); KTX headers
≈ 15 MB; set records ≈ 20 MB of TOML, deflated in the bundle as text entries are ([layout.md](layout.md) §4) to
about 3 MB; ZIP headers and 64-byte alignment ≈ 6 MB [estimate]: **≈ 675 MB against 717 MB**. The palettes stay stored: they
are inside KTX 2.0 files, which are payload entries (layout.md §4).

**Residual**: ≈ 0.1 MB for plain packs (the padding chunks, §5.3); for block-compressed packs only the `0x33310003`
block records, 54,916 × 0x18 bytes ≈ 1.3 MB (§5.6). No shadow copies: every decoded node has a byte proof.

**Import time** [guess]: 10–20 s on the reference desktop for all textures, dominated by the vinyls (18.2 GB through
HUFF and then Zstandard, in parallel per image), and about as long again for the verification (inflate, Eq1 hash);
everything else is a copy of 240 MB of blocks plus 62,001 small records. Memory stays per image (at most about
6 MB decoded: a 1,024 × 1,024 32-bit texture with its levels).
