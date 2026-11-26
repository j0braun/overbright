# Asset format: world (regions, stream sections, scenery placements, visibility, lights, props, world animation, fog)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md), [layering.md](layering.md) and [residual.md](residual.md) apply and are not repeated.
Schemas: `schemas/region.schema.json`, `visibility.schema.json`, `section.schema.json`, `smackables.schema.json`,
`world-anims.schema.json`, `fog.schema.json`, `placements.schema.json`. Examples: `examples/world/`.

This page defines the `world/<REGION>/` unit apart from its meshes, textures, animation banks, routes and collision:
the stream-section table, the scenery placements and the data that hangs off them. Its central rule is **placement
identity**: every original placement keeps its key `(region, scenery section, index)` from the import to the last
pack in the stack, and the original's order stays the order the engine draws and culls in.

Terms used on this page (the original's own words where it has them, docs/formats/world.md, docs/formats/cars-1.md):

| Term | Meaning |
|---|---|
| **region** | One pair of track files, `TRACKS/<REGION>.BUN` (the resident *track bundle*) and `TRACKS/STREAM<REGION>.BUN` (the *stream file*). The reference install has eight: `L4RA` (the city), `L4RB`, `L4RC`, `L4RD` (a race copy of the city), `L4RF`, `L4RG`, `L4RH`, `L4RR`. |
| **stream section** | One record of the track bundle's `0x34110` table: a name (`A21`, `N27`, `Z0`), a number (`121`, `1427`, `2600`) and a byte range of the stream file. What the original streams in and out; the load unit of this page's stream-section files. |
| **scenery section** | One `0x80034100` chunk inside a stream section: its own number (header +0x0C), a table of *infos* and a table of *instances*. A stream section holds zero, one or two (L4RA: 238 stream sections hold two, e.g. stream section `1427` holds scenery sections `1427` and `1407`). Scenery section numbers are unique in a region (measured in all eight). The original names scenery by these numbers: its scenery-link table (`0x34107`) and its visible lists (`0x34153`) use them. |
| **placement** | One scenery instance: a prop or building placed in the world. |
| **detail limit** | The first u32 of `0x34151` (20 in every region): the stream sections of letters A–T numbered 1 to limit − 1 are the **drivable stream sections**; the stream sections numbered limit to 2·limit − 1 of a letter are the same places at another level of detail and hold their scenery and collision. |

The word *section* alone is not used in this page's rules: a number is always a stream section's or a scenery
section's (conventions §2). Where both share a number (stream section 1427 holds scenery section 1427), the two are
still different items.

## 1. Scope

Every chunk of the two track files maps to exactly one place. "Residual" means the chunk is kept as found
([residual.md](residual.md)) because no engine code reads it today.

**Track bundle `TRACKS/<REGION>.BUN`** (top-level chunks):

| Chunk | What | Where |
|---|---|---|
| `0x00034110` | stream-section table, 0x50-byte records (393 in L4RA) | `region.toml` (§3.2) |
| `0x80034150` / `0x34151` | detail limit (first u32); the 804 bytes after it | `region.toml` `detail_limit`; the rest residual |
| `0x80034150` / `0x34152`, `0x34153` | drivable outlines, visible lists | `visibility.toml` (§3.3) |
| `0x80034150` / `0x34154`, `0x34155` | not read | residual |
| `0x00034026`, `0x00034107`, `0x00034108` | knock-over prop kinds, scenery links, scenery groups | `smackables.toml` + `smackables.bin` (§3.5) |
| `0x80034020` | the props' collision volumes | [collision.md](collision.md) |
| `0x0003412F` | barrier groups | [collision.md](collision.md) |
| `0x00037080`, `0x00037090` | world-animation entities and instances | `anims.toml` (§3.6) |
| `0x00037110`, `0x00037150` | not read | residual |
| `0x00E34010` | animation banks | [animation.md](animation.md) |
| `0x8003B200` | camera tracks | [animation.md](animation.md) |
| `0x8003B600` | parameter maps (fog and other values by position) | `fog.toml` (§3.7) |
| `0x00034250` | horizon fog sources | `fog.toml` (§3.7) |
| `0x00035021` | the region's particle effect descriptions (the records placed emitters name) | `effects.bin`, carried opaque: no kind owns them yet ([README.md](README.md) §3; §3.8) |
| `0x00034111`, `0x00034112`, `0x00034113`, `0x8003B000` | not read | residual |
| id 0 | alignment padding | residual `fill` nodes |

**Stream file `TRACKS/STREAM<REGION>.BUN`**: every top-level chunk lies inside a stream section's range (checked in
all eight regions). Per stream section:

| Chunk | What | At most per stream section | Where |
|---|---|---|---|
| `0x80034100` / `0x34101`, `0x34102`, `0x34103` | scenery section: header, infos, instances | 2 | `sections/<n>.toml` + `sections/<n>.bin` (§3.4) |
| `0x80034100` / `0x34104`, `0x34105`, `0x34106` | not read (L4RA: 5.8 MB, mostly `0x34106`) | 1 each | residual |
| `0x80135000` | light pack | 1 | stream-section file (§3.4.3) |
| `0x80135100` | flare pack (lamp glows) | 1 | stream-section file (§3.4.4) |
| `0x00034027` | knock-over prop instances | 1 | stream-section file (§3.4.5) |
| `0x00035020` | placed particle emitters | 1 | stream-section file (§3.4.6) |
| `0x80034130` | collision pack | 1 | [collision.md](collision.md) |
| `0x80134000` | solid lists | 18 | [meshes.md](meshes.md) |
| `0xB3300000`, `0xB0300100` | texture packs, texture animation | 2, 1 | [textures.md](textures.md) |
| `0x80036000` (`0x36001`–`0x36003`) | not read (L4RA: 0.41 MB) | 1 | residual |
| id 0 | alignment padding between stream sections | – | residual `fill` nodes |

**Stream-section ranges overlap.** Every region's first record (`--`, number 0, the only one with `u_0x10` = 0)
starts at offset 0 like `X0`. In L4RA and L4RD its range holds no whole chunk; in the other six it holds whole solid
lists of `X0`; in **L4RH and L4RR it also holds the whole scenery section 2600** (`Z0`, the SKYDOME) of stream
section 2600. The original never loads this record: sections are loaded only by number (0x5DACC0, fed by 0x5E5120's
fixed list, the visible list and the extra numbers, each taken only when not zero, and by 0x5E5230…0x5E52C8, positive
numbers only), and no visible list holds 0 (verified, docs/formats/world.md "Tracks, streaming and visibility").
Overbright's loader skips it too (`world::parse::loads`), so each scenery section is placed once.

**Other files of `TRACKS/`.** `ROUTES<REGION>/*`: [roads-ai.md](roads-ai.md). `LOC4DYNTEX.BIN` (the sky's and other
dynamic textures) and `TRACKMAP<n>.BIN` (129 files, each one texture pack holding the texture `TRACKMAP<n>`, the
minimap of track `n`, loaded by the front end by track number) belong to the `global/` unit ([layout.md](layout.md)
§2): the textures in [textures.md](textures.md), the track maps' minimap and World Map calibration in
[career.md](career.md) (`ob.track-maps`). world.md defines no data for them. `PrecullerBooBooScript.hoo`: no
reader; kept whole ([residual.md](residual.md) §5).

**The sky** is not a kind of its own: it is the placement of `SKYDOME` in scenery section `2600` (`Z0`), whose
textures live in `LOC4DYNTEX.BIN`. The original never draws that placement through the scenery path (0x5DFAC6 skips
the SKYDOME's hash); its drawing, cycle and texture pairs are engine behaviour (docs/formats/world.md "Sky").

**Effects.** Placed emitters (`0x35020`) are decoded by this page (§3.4.6). The particle effect descriptions they
name (`0x35021`) are not owned by any kind yet ([README.md](README.md) §3): a region's own records are carried opaque
in `world/<REGION>/effects.bin` (§3.8); those of `GLOBALB.BUN` and `FRONTEND/PLATFORMS/Showroom.BIN` stay in the
residual until an effects kind exists.

**Not on this page:** solids, textures, routes, collision, animation banks; light materials (`0x135200` in
`GLOBALB.BUN`, not owned by a kind yet, README §3); the engine's own placements (world-animation pieces, the GPS aid
marker, the dyno rollers), which the engine makes at load from this data (§7).

## 2. Files and keys

```text
world/<REGION>/
  region.toml            ob.region/1.0       the stream-section table, the detail limit
  visibility.toml        ob.visibility/1.0   drivable outlines, visible lists
  sections/<n>.toml      ob.section/1.0      one per stream section with scenery, lights, flares, props or emitters
  sections/<n>.bin                           that stream section's external arrays (§3.4)
  smackables.toml        ob.smackables/1.0   knock-over prop kinds, scenery groups
  smackables.bin                             the scenery-link table
  anims.toml             ob.world-anims/1.0  world-animation entities and instances
  fog.toml               ob.fog/1.0          parameter maps, horizon fog sources
  effects.bin                                particle effect descriptions, opaque (§3.8)
  placements.toml        ob.placements/1.0   mod packs only: moved, disabled, added placements (§3.9)
  collision/…                                collision.md
  meshes/…, textures/…, anim/…, routes/…     meshes.md, textures.md, animation.md, roads-ai.md
```

`<n>` is the stream section's number in decimal without leading zeros (`sections/1427.toml`). A base set has one
`region.toml`, `visibility.toml`, `smackables.toml`, `anims.toml` and `fog.toml` per region (empty tables allowed);
`effects.bin` and `smackables.bin` are absent when they would be empty.

**Item keys** (conventions §6.5; references §6.6):

| Item | Key | Reference form | Example |
|---|---|---|---|
| region | its name | `region:<REGION>` | `region:L4RA` |
| stream section | number | `section:<REGION>/<n>` | `section:L4RA/1427` |
| scenery section | number | `scenery:<REGION>/<n>` | `scenery:L4RA/1407` |
| original placement | (scenery section, index) | `placement:<REGION>/<scenery>/<index>` | `placement:L4RA/1407/12` |
| added placement | (pack id, local id) | `placement:<REGION>/<pack>:<id>` | `placement:L4RA/docks-remix:a12` |
| light, flare, prop instance, placed emitter | (stream section, index) | `light:<REGION>/<n>/<i>`, `flare:…`, `prop:…`, `emitter:…` | `light:L4RA/1427/3` |
| knock-over prop kind | its key (bin hash of its name) | `prop-kind:<REGION>/<NAME>` | `prop-kind:L4RA/ZCS_PylonA` |
| scenery group | its name | `scenery-group:<REGION>/<NAME>` | `scenery-group:L4RA/SMOKEABLE` |
| scenery link | index in the table | `scenery-link:<REGION>/<i>` | `scenery-link:L4RA/40` |
| world-animation entity, instance | index in file order | `anim-entity:<REGION>/<i>`, `anim-instance:<REGION>/<i>` | `anim-instance:L4RA/0` |
| parameter map | its name hash | `fog-map:<REGION>/0x<HASH>` | `fog-map:L4RA/0x1A2B3C4D` |
| horizon fog source | index | `horizon-fog:<REGION>/<i>` | `horizon-fog:L4RA/2` |

`index` always counts from 0 in the original's order, **including** entries the engine skips (an instance whose
solid is missing or empty keeps its index). Keys never change once a base set is made.

Example tree (L4RA, base set):

```text
world/L4RA/region.toml
world/L4RA/visibility.toml
world/L4RA/sections/121.toml      scenery sections 121 and 101; lights, flares
world/L4RA/sections/121.bin
world/L4RA/sections/1427.toml     scenery sections 1427 and 1407; lights, flares, props
world/L4RA/sections/1427.bin
world/L4RA/sections/2600.toml     scenery section 2600: the SKYDOME
world/L4RA/smackables.toml
world/L4RA/smackables.bin
world/L4RA/anims.toml
world/L4RA/fog.toml
world/L4RA/effects.bin
```

## 3. Source form

### 3.1 Rules common to this page

- **One table, two forms.** Every original fixed-size record and chunk header has one layout table below. The same
  table is the field table of its TOML form: field names and types are the layout rows' (`T[n]` is a TOML array of
  n values), and the rows' classes below say what is written. A list of such records is stored either as an external
  array (`{ file, offset, count, layout }`, conventions §4.4, layout = the table's name) or inline as a TOML array of
  tables with those fields. Each list has two field names, one per form: `instances` / `placement` (scenery),
  `nodes` / `node` and `records` / `light` (lights), `records` / `flare`, `records` / `prop`, `records` /
  `emitter`, `links` / `link`. A reader MUST accept both forms and decode them to the same values; exactly one of
  the two names is present (`WLD.4`). The importer writes the external form for the large arrays named below;
  community base sets and tools may write either.
- **Order is data.** Every list keeps the original's order (conventions §6.5). Readers MUST NOT sort.
- **Row classes.** Every row of a layout table is exactly one of:
  - a **field**: a name, or `u_<offset>` when its meaning is **open** ([conventions.md](conventions.md) §8). Stored
    in TOML. A `u_` field's default is 0 (all zeros for an array) unless its Notes give another; a named field's
    default is the one its Notes give, and a named field whose Notes give none is required. Writers leave out a
    field equal to its default (floats compared by bits, conventions §3.4); a reader takes the default for an
    absent one.
  - **`pad`**: zero on disk, not stored; non-zero is refused (`CONV.11`).
  - **`const`**: one value in every record of the reference install, given in the Notes, that Overbright's engine
    does not read as a value: list links and loaded flags the original's loader overwrites, version words it only
    tests, and words of open meaning that never vary. Not stored; the encoder writes the value. The importer checks
    it: a version word with another value means the original drops the chunk, so the importer keeps that chunk in
    the residual and writes nothing for it; any other `const` with another value is an import error (`WLD.6`): the
    edition differs from this specification, and the importer keeps the file whole ([import.md](import.md) §3). In
    the external form, where records are stored as laid out, a reader refuses a `const` row with another value
    (`WLD.6`) as it refuses non-zero `pad` (`CONV.11`).
  - ***derived***: a count, or a copy of the stream-section file's `number`. Not stored; the encoder computes it;
    the importer checks it (`WLD.4`).
- **Hashes.** A `hash32` field copied from an original record that points at another item is a **bare hash**
  (conventions §6.6): its row names the kind and the scope it resolves in, and `0x00000000` means none unless the
  row says otherwise. Items of kinds no page owns yet are named the same way (particle effect descriptions,
  `0x35021`; light materials, `0x135200`, which no field of this page is known to name; [README.md](README.md) §3).
  The other `hash32` fields are an item's own key or values the engine compares (a switch name, a message key), and
  their rows say so.
- **Fill.** The alignment bytes the original puts between a chunk's 8-byte id-and-size header and the first
  16-aligned byte of its body (12 bytes of `0x11` before every scenery instance array; 8 before the light tree, the
  light and flare records, the headers of every prop, emitter, description and horizon chunk, each world-animation
  record, and the prop kinds in two regions) are not data. Their length follows from the chunk's file offset (its
  residual node's offset); the chunk's encoder writes them as `0x11` bytes, and the importer checks them
  (`WLD.6`).
- Floats follow conventions §3.4 (exact bits). Positions are world space (conventions §5.1), metres. Rotations are
  row-major, rows the local axes, row vectors (conventions §5.3).
- **Limits** (beyond conventions §10): a stream-section file ≤ 8 MiB; a scenery section ≤ 65,535 infos and ≤ 65,535
  instances (u16 indices); a stream section ≤ 2 scenery sections; `placements.toml` ≤ 50,000 entries (today's
  map-mod limit, `crates/ob-map/src/tools/map/format.rs` `MAX_PROPS`).

### 3.2 `region.toml` (`ob.region/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.region/1.0` |
| `region` | string `[A-Za-z0-9]{2,8}` | – | yes | – | The region's name; MUST equal the unit's folder name (`WLD.1`). |
| `detail_limit` | u32 | – | yes | – | `0x34151` +0x00 (verified, 0x800DFC). |
| `section` | array of tables | – | yes | – | The stream-section table in original order; each entry is a `stream_section` record. |
| `effects` | array descriptor, layout `emitter_description` | – | no | none | The region's particle effect descriptions, opaque (§3.8); absent when its `0x35021` chunk holds none. |
| `extras` | table | – | no | – | conventions §9. |

Layout `stream_section` (0x50 bytes, alignment 4; the engine reads `name`, `number` and, in the original files,
the byte range, `crates/ob-formats/src/formats/track.rs:28`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `char[8]` | `name` | – | NUL-terminated, zero-filled (verified on all 946 records). `--` for the empty first entry. |
| 0x08 | 4 | `u32` | `number` | – | Unique in the region (`WLD.2`). |
| 0x0C | 4 | `u32` | `pad` | – | |
| 0x10 | 4 | `u32` | `u_0x10` | – | **open** (not read by the loaders traced); 0 for `--`, the record never loaded (§1), 1 for all others. |
| 0x14 | 4 | `u32` | `stream_offset` | byte | The stream section's range in the original stream file. Import-only: written by the importer for the rebuild, ignored by readers; absent in community sets (default 0). |
| 0x18 | 4 | `u32` | `stream_size` | byte | As `stream_offset` (default 0). |
| 0x1C | 4 | `u32` | `u_0x1c` | byte | **open**; a size, equal to `stream_size` in 200 of L4RA's 393 records. |
| 0x20 | 4 | `u32` | `u_0x20` | – | **open**; looks like an order key (10 000 + number for city stream sections) [guess]. |
| 0x24 | 12 | `f32[3]` | `u_0x24` | m | **open**; a point inside the stream section [guess]. |
| 0x30 | 4 | `u32` | `u_0x30` | – | **open**; hash-like, distinct per record. |
| 0x34 | 28 | `u8[28]` | `pad` | – | |

```toml
schema = "ob.region/1.0"
region = "L4RA"
detail_limit = 20

[[section]]
name = "--"
number = 0
stream_offset = 0
stream_size = 4096
u_0x1c = 4096
u_0x20 = 12345
u_0x30 = 0x00C0FFEE

[[section]]
name = "N27"
number = 1427
u_0x10 = 1
stream_offset = 0x00123000
stream_size = 65536
u_0x1c = 40960
u_0x20 = 54321
u_0x24 = [-200.0, -1000.0, 150.0]
u_0x30 = 0x12345678
```

### 3.3 `visibility.toml` (`ob.visibility/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.visibility/1.0` |
| `outline` | array of `outline` tables | – | yes | – | `0x34152`: the drivable stream sections' outlines, in file order (the engine lists the drivable stream sections in this order, 0x5E9C62). |
| `list` | array of `visible_list` tables | – | yes | – | `0x34153`: the visible lists, in file order. |
| `extras` | table | – | no | – | conventions §9. |

Both arrays may be empty; their entries are always inline (no external form).

Layout `outline` (variable size, 0x24 + 8·n bytes, alignment 4):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `const` | – | 11, 11 in all 802 outlines: the game's list link, overwritten at run time. |
| 0x08 | 2 | `u16` | `section` | – | A drivable stream section's number (letters A–T, 1 ≤ n < limit). |
| 0x0A | 2 | `u16` | `count` | – | *derived*: number of points (≤ 65,535). |
| 0x0C | 16 | `f32[4]` | `bbox` | m | min x, min y, max x, max y. |
| 0x1C | 8 | `f32[2]` | `inside` | m | A point inside the outline (read by 0x5D7280). |
| 0x24 | 8·n | `f32[2][n]` | `points` | m | The outline, in order (TOML: an array of `[x, y]`). |

Layout `visible_list` (0xA4 bytes, alignment 4):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `const` | – | 11, 11 in all 599 lists: as above. |
| 0x08 | 4 | `u32` | `section` | – | The drivable stream section the list is for. |
| 0x0C | 4 | `u32` | `pad` | – | |
| 0x10 | 4 | `u32` | `count` | – | *derived*; ≤ 72 (`WLD.8`). |
| 0x14 | 144 | `u16[72]` | `sections` | – | What to load and draw, in order (TOML: only the first `count`; the rest are zero on disk). Each number is a stream section's or a scenery section's. |

```toml
schema = "ob.visibility/1.0"

[[outline]]
section = 1407
bbox = [-240.0, -1080.0, -170.0, -1000.0]
inside = [-205.0, -1040.0]
points = [[-240.0, -1080.0], [-170.0, -1080.0], [-170.0, -1000.0], [-240.0, -1000.0]]

[[list]]
section = 1407
sections = [1407, 1408, 1427, 1428, 1499, 2401, 2501]
```

### 3.4 Stream-section files (`ob.section/1.0`)

One file per stream section that holds at least one of: scenery sections, a light pack, a flare pack, prop
instances, placed emitters. Its arrays live in `sections/<n>.bin` when the external form is used.

| Field | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `schema` | string | yes | – | `ob.section/1.0` |
| `number` | u32 | yes | – | The stream section; MUST be in `region.toml` and match the file name (`WLD.1`). |
| `scenery` | array of tables (§3.4.1) | no | `[]` | The scenery sections, in chunk order (≤ 2). |
| `also` | array<u32> | no | `[]` | Scenery sections of **other** stream sections whose chunks lie whole inside this stream section's original range (§1, "Stream-section ranges overlap"). The engine places nothing for them: the only ranges that hold such chunks belong to the `--` record (stream section 0), which is never loaded. Kept so that files written by earlier importers still read; the importer writes none. |
| `lights` | table (§3.4.3) | no | – | The light pack. |
| `flares` | table (§3.4.4) | no | – | The flare pack. |
| `props` | table (§3.4.5) | no | – | The knock-over prop instances. |
| `emitters` | table (§3.4.6) | no | – | The placed emitters. |
| `extras` | table | no | – | |

#### 3.4.1 Scenery sections

| Field | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `number` | u32 | yes | – | Header `0x34101` +0x0C. Unique in the region (`WLD.3`). |
| `info` | array of `scenery_info` tables | yes | – | The infos (`0x34102`), in order; always inline. |
| `instances` | array descriptor, layout `scenery_instance` | one of | – | The instances (`0x34103`), external form. |
| `placement` | array of `scenery_instance` tables | one of | – | The same, inline form. |

Layout `scenery_header` (chunk `0x34101`, 0x3C bytes, alignment 4; the body starts aligned in all 1,378 headers):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 12 | `u32[3]` | `pad` | – | Zero in all 1,378 headers. |
| 0x0C | 4 | `u32` | `number` | – | The scenery section's number (the field above). |
| 0x10 | 44 | `u8[44]` | `pad` | – | Zero in all 1,378 headers. |

Layout `scenery_info` (0x44 bytes, alignment 4; always TOML):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 32 | `char[32]` | `name` | – | NUL-terminated, zero-filled (verified on all 31,593 infos); not read by the engine. Names repeat within a scenery section (142 repeats in L4RA). |
| 0x20 | 12 | `hash32[3]` | `lod` | – | Bare hashes of kind `solid`: LOD A (full detail), B, C; `0x00000000` = none. Scope: the solids of this region's unit (`world/<REGION>`), in [meshes.md](meshes.md)'s lookup order (the first solid of a hash wins). A hash naming no solid of the region is allowed and means none (the original's rule; 2 instances in L4RB). |
| 0x2C | 4 | `u32` | `u_0x2c` | – | **open** (two u16 values [guess]). |
| 0x30 | 12 | `u32[3]` | `pad` | – | The loader's solid pointers at run time (0x5D6717, 0x5D672A); zero on disk. |
| 0x3C | 4 | `f32` | `radius_m` | m | Bounding radius; the cull adds 6 (0x5D6580, 0x7A47D4). |
| 0x40 | 4 | `u32` | `u_0x40` | – | **open**; not the name's hash (checked). |

Layout `scenery_instance` (0x40 bytes, alignment 4; external in base sets made by the importer):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 12 | `f32[3]` | `bbox_min` | m | World-space box as stored. Not read by Overbright; **open** whether the original reads it. Default `[0.0, 0.0, 0.0]`. |
| 0x0C | 12 | `f32[3]` | `bbox_max` | m | As `bbox_min` (same default). |
| 0x18 | 2 | `u16` | `info` | – | Index into this scenery section's `info` (`WLD.5`). |
| 0x1A | 2 | `u16` | `flags` | – | The flag bits below. |
| 0x1C | 4 | `u32` | `u_0x1c` | – | **open**; 0–545 in L4RA, 0–513 in L4RD, 0 in the other six regions. |
| 0x20 | 12 | `f32[3]` | `position` | m | |
| 0x2C | 18 | `i16[9]` | `rotation` | 1/8192 | Rows = local x, y, z axes, row vectors (docs/formats/world.md "Scenery", verified). TOML form: a `mat3` of the exact values k/8192 (conventions §3.4: every k/8192 is a binary32 value); the TOML form also accepts values off that grid, which only the TOML form can store. |
| 0x3E | 2 | `u16` | `pad` | – | Zero in all 122,178 instances. |

Instance flags (u16 +0x1A; read by the cull 0x5D6580 and the draw 0x5D6AE0; docs/formats/world.md "Scenery",
**verified** unless marked). The cull skips an instance when `(flags ^ 0x60) & view cull bits` is non-zero:

| Bit | Meaning | Instances with it (L4RA, on disk) |
|---|---|---|
| 0x0001 | Culled in split-screen layouts 3 and 4 (cull bit 0x01). | 0 (no region) |
| 0x0002 | Not drawn in views 1–2 (traffic lights, banners). | 78 |
| 0x0004 | Drawn only while the race runs forward. | 89 |
| 0x0008 | Drawn only while the race runs reversed. | 77 |
| 0x0010 | Hidden: in every view's cull bits, so drawn only while a scenery group that lists it is on (0x5710B0 clears it, 0x5710F0 sets it). The engine's `HIDDEN_FLAG`. | 5,999 |
| 0x0020 | Needed in view 3 (the rear-view mirror) (inverted by `^ 0x60`). | 17,613 |
| 0x0040 | Needed in the environment cube, the road reflection and view 3 (inverted). | 14,897 |
| 0x0080 | A cull bit no view sets. | 0 (no region) |
| 0x0100 | No matrix: the draw entry gets a null transform (0x5D6752); every such instance has the identity rotation and position 0. | 4,393 |
| 0x0200 | Race-only barrier (`XO_TRACKBARRIER*`): mirrored for a race run backward (0x5DB4C0). | 5,622 |
| 0x0400 | Draw flag 0x2000: left out of the road reflection; picks LOD B instead of C for cull base 0x1800 (0x5D672A). | 2,942 |
| 0x0800–0x4000 | **open**; on no instance of any region. | 0 |
| 0x8000 | Draw flag 0x200, cleared once drawn (0x5D6CD8): a run-time bit. | 0 (no region) |

External form, as the importer writes it:

```toml
[[scenery]]
number = 1407
instances = { file = "1427.bin", offset = 0, count = 229, layout = "scenery_instance", digest = "xxh3-128:5f0c2a7e9b1d4c3a8e6f7a0b1c2d3e4f" }

[[scenery.info]]
name = "XO_CONE_01"
lod = ["0x608A650A", "0x00000000", "0x00000000"]   # bin_hash("XO_CONE_01")
radius_m = 0.75
```

#### 3.4.2 The engine's placements from this data

What the engine consumes (`src/world/parse.rs:338-377`), and so what the equality proof covers: for each stream
section in table order, each scenery section in chunk order (stream section 0, `--`, is not loaded: §1), each instance in index order: the drawn solid is the
first `lod` hash naming a solid of the region (A in every instance of the reference install); an instance whose
`lod` names none is counted as missing and not placed; one whose solid has no triangles is not placed (47 in L4RA);
otherwise one placement with that solid, LOD B (`mid`), the low LOD (C, or B with flag 0x0400), the transform from
`rotation` and `position`, the stream section's number, `flags`, and cull radius `radius_m + 6`. **The draw and cull
order is this order**; added placements come after all of them (§4.2).

#### 3.4.3 Light pack (`0x80135000`)

Children `0x135001` (header), `0x135002` (box tree) and `0x135003` (lights), read by the `0x80135000` case of
0x48E0B0 (0x48E1AC…0x48E253, verified). The `lights` table:

| Field | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `u_tree_0x06`, `u_tree_0x0a` | u16 | no | 0 | `light_tree_header` +0x06 / +0x0A (**open**; named `u_tree_…` because this table holds fields of two structs). |
| `nodes` / `node` | array descriptor, layout `light_node` / array of `light_node` tables | one of | – | The tree's nodes, in order. |
| `records` / `light` | array descriptor, layout `light` / array of `light` tables | one of | – | The lights, in order. |
| `extras` | table | no | – | |

Layout `light_pack_header` (chunk `0x135001`, 0x20 bytes, alignment 16; the body starts aligned in all 544 packs):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `pad` | – | The pack's list links, set at load (0x48E22F…0x48E248). |
| 0x08 | 2 | `u16` | `const` | – | 4: the version. The loader drops a pack with another (0x48E219), so the importer keeps such a pack in the residual and writes no `lights` table. |
| 0x0A | 1 | `u8` | `pad` | – | The loaded flag, set at load (0x48E215). |
| 0x0B | 1 | `u8` | `pad` | – | |
| 0x0C | 4 | `u32` | `section` | – | *derived*: the file's `number` (equal in all 544 packs); the loader files the pack under it (0x5DB690). |
| 0x10 | 4 | `u32` | `pad` | – | The tree's address, set at load (0x48E1F5). |
| 0x14 | 4 | `u32` | `pad` | – | |
| 0x18 | 4 | `u32` | `pad` | – | The lights' address, set at load (0x48E1E5). |
| 0x1C | 4 | `u32` | `light_count` | – | *derived*: the number of lights. |

Layout `light_tree_header` (the first 0x10 bytes of chunk `0x135002`, after 8 bytes of fill; alignment 16; the
nodes follow from +0x10):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `pad` | – | The nodes' address, set at load to +0x10 (0x48E1F8). |
| 0x04 | 2 | `u16` | `light_count` | – | *derived*: the number of lights (equal in all 544 packs). |
| 0x06 | 2 | `u16` | `u_tree_0x06` | – | **open**; 1–65 in the reference install. |
| 0x08 | 2 | `u16` | `node_count` | – | *derived*: the number of nodes (equal in all 544 packs). |
| 0x0A | 2 | `u16` | `u_tree_0x0a` | – | **open**; 2–5 in the reference install. |
| 0x0C | 4 | `u32` | `pad` | – | |

The lights (`0x135003`) start after 8 bytes of fill; the chunk holds exactly `light_count` records in all 544 packs.

Layout `light_node` (0x30 bytes, alignment 4; read by 0x5747F0 / 0x574740):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 12 | `f32[3]` | `centre` | m | |
| 0x0C | 2 | `u16` | `u_0x0c` | – | **open**. |
| 0x0E | 2 | `i16` | `count` | – | > 0: that many children; ≤ 0: minus the lights listed (a leaf). |
| 0x10 | 12 | `f32[3]` | `half` | m | Half extents. |
| 0x1C | 20 | `u16[10]` | `indices` | – | Child node or light indices; unused slots zero (verified on all 6,990 nodes). TOML: the used slots only. |

Layout `light` (0x60 bytes, alignment 4; 0x48E3D0):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `u_0x00` | – | **open**; equals `bin_hash` of the name at +0x3E in all 5,597 lights (measured); matches none of `GLOBALB.BUN`'s 186 light-material hashes (checked). |
| 0x04 | 1 | `u8` | `kind` | – | 0 ambient, 2 point, 3 spot. |
| 0x05 | 1 | `u8` | `sub` | – | Default 2 (every light of the reference install). A kind-0 light with 2 is summed into the weighted ambient, another kind-0 light is the nearest ambient (docs/formats/cars-1.md "Light packs"). |
| 0x06 | 1 | `u8` | `const` | – | 2 in every light; meaning open. |
| 0x07 | 1 | `u8` | `on` | – | 1 = on. |
| 0x08 | 4 | `u32` | `pad` | – | |
| 0x0C | 3 | `u8[3]` | `colour` | – | Red, green, blue (0–255). |
| 0x0F | 1 | `u8` | `const` | – | 255 in every light; meaning open. |
| 0x10 | 12 | `f32[3]` | `position` | m | |
| 0x1C | 4 | `f32` | `outer_m` | m | Outer radius. |
| 0x20 | 12 | `f32[3]` | `direction` | – | Spot direction. |
| 0x2C | 4 | `f32` | `intensity` | – | |
| 0x30 | 4 | `f32` | `inner_m` | m | Inner radius. |
| 0x34 | 4 | `f32` | `cone` | – | Spot cone. |
| 0x38 | 4 | `f32` | `const` | – | 65354.0 (bits `0x477F4A00`) in every light; meaning open. |
| 0x3C | 32 | `u32[8]` | `u_0x3c` | – | **open**. Holds a u16 at +0x3C and from +0x3E the light's name (`EL_XO_StreetLightC_08`), NUL-terminated and zero-filled in all 5,597 lights (measured). |
| 0x5C | 4 | `u32` | `pad` | – | |

#### 3.4.4 Flare pack (`0x80135100`)

Children `0x135101` (header) and `0x135102` (flares), read by the `0x80135100` case of 0x48E0B0 (0x48E0EB…0x48E1AB,
verified). The `flares` table:

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `box_min`, `box_max` | vec3 | m | yes | – | Header +0x30 / +0x40: the frustum test's box (0x5BC310). |
| `records` / `flare` | array descriptor, layout `flare` / array of `flare` tables | – | one of | – | The flares, in order. |
| `extras` | table | – | no | – | |

Layout `flare_pack_header` (chunk `0x135101`, 0x60 bytes, alignment 16; the body starts aligned in all 541 packs):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `const` | – | 3, 3 in all 541 packs: the pack's list links, set at load (0x48E17B…0x48E194). |
| 0x08 | 4 | `u32` | `const` | – | 3: the version. The loader drops a pack with another (0x48E15A), so the importer keeps such a pack in the residual and writes no `flares` table. |
| 0x0C | 36 | `u8[36]` | `pad` | – | |
| 0x30 | 12 | `f32[3]` | `box_min` | m | |
| 0x3C | 4 | `u32` | `pad` | – | |
| 0x40 | 12 | `f32[3]` | `box_max` | m | |
| 0x4C | 4 | `u32` | `pad` | – | |
| 0x50 | 2 | `u16` | `flare_count` | – | *derived*: the number of flares (the chunk `0x135102` holds exactly that many in all 541 packs). |
| 0x52 | 1 | `u8` | `pad` | – | The loaded flag, set at load (0x48E156). |
| 0x53 | 1 | `u8` | `pad` | – | |
| 0x54 | 4 | `u32` | `section` | – | *derived*: the file's `number` (equal in all 541 packs); the loader files the pack under it (0x5DB690). |
| 0x58 | 8 | `u32[2]` | `const` | – | `0x0011E59C`, `0x0011E59C` in all 541 packs: the flare list's head, set to itself at load (0x48E164). |

Layout `flare` (0x30 bytes, alignment 4; after 8 bytes of fill; drawn by 0x491860):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `const` | – | 11, 11 in all 5,359 flares: list links, set at load (0x48E134…0x48E13F). |
| 0x08 | 4 | `hash32` | `name` | – | A name hash. Overbright does not resolve it; what it names is open (none of `GLOBALB.BUN`'s light materials, checked). |
| 0x0C | 4 | `u32` | `const` | – | `0x00FFFFFF` in every flare; meaning open. |
| 0x10 | 12 | `f32[3]` | `position` | m | |
| 0x1C | 4 | `f32` | `ground_m` | m | The reflection's ground height cache: default 999.0 (every flare on disk) = not cached yet; 0x490920 fills it. |
| 0x20 | 12 | `f32[3]` | `direction` | – | |
| 0x2C | 1 | `u8` | `kind` | – | Index into the flare types (0x7F3CC8). |
| 0x2D | 1 | `u8` | `flags` | – | |
| 0x2E | 2 | `u16` | `u_0x2e` | – | **open**. |

#### 3.4.5 Knock-over prop instances (`0x34027`)

The `props` table:

| Field | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `first_bit` | i16 | yes | – | Header +0x0A. |
| `records` / `prop` | array descriptor, layout `prop_instance` / array of `prop_instance` tables | one of | – | The prop instances, in order. |
| `extras` | table | no | – | |

Layout `prop_pack_header` (the first 0x10 bytes of chunk `0x34027`, after 8 bytes of fill; alignment 16; the records
follow from +0x10; docs/formats/cars-1.md "Knock-over props"):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `const` | – | 11, 11 in all 231 headers; meaning open. |
| 0x08 | 2 | `i16` | `section` | – | *derived*: the file's `number` (equal in all 231). |
| 0x0A | 2 | `i16` | `first_bit` | – | The first index into the "already made" bit set 0x86AD78. |
| 0x0C | 2 | `u16` | `prop_count` | – | *derived*: the number of prop instances (the chunk holds exactly that many in all 231). |
| 0x0E | 2 | `u16` | `pad` | – | |

Layout `prop_instance` (0x70 bytes, alignment 4; 0x5B0E40):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `hash32` | `kind` | – | Bare hash of kind `prop-kind`: the `key` of one of this region's prop kinds (`smackables.toml`, §3.5). |
| 0x04 | 2 | `u16` | `link` | – | Index into the scenery-link table: the placement it stands in for. |
| 0x06 | 2 | `u16` | `pad` | – | |
| 0x08 | 4 | `u32` | `u_0x08` | – | **open**. |
| 0x0C | 4 | `u32` | `pad` | – | |
| 0x10 | 12 | `f32[3]` | `bbox_min` | m | World bounds (the unhit body's box, 0x5AD900). |
| 0x1C | 4 | `u32` | `pad` | – | |
| 0x20 | 12 | `f32[3]` | `bbox_max` | m | |
| 0x2C | 4 | `u32` | `pad` | – | |
| 0x30 | 64 | `f32[16]` | `transform` | – | 4×4, row vectors: rows local axes and position (re-orthonormalised on first load, 0x5BB0E0). |

#### 3.4.6 Placed emitters (`0x35020`)

The `emitters` table:

| Field | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `records` / `emitter` | array descriptor, layout `emitter` / array of `emitter` tables | one of | – | The emitters, in order. |
| `extras` | table | no | – | |

Layout `emitter_pack_header` (the first 0x10 bytes of chunk `0x35020`, after 8 bytes of fill; alignment 16; the
records follow from +0x10; 0x491C70):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `section` | – | *derived*: the file's `number` (equal in all 36 chunks). |
| 0x04 | 4 | `u32` | `pad` | – | |
| 0x08 | 4 | `u32` | `const` | – | 6: the version. 0x491C70 reads nothing else, so the importer keeps a chunk of another version in the residual and writes no `emitters` table. |
| 0x0C | 4 | `u32` | `emitter_count` | – | *derived*: the number of emitters (the chunk holds exactly that many in all 36). |

Layout `emitter` (0xD0 bytes, alignment 4; `src/particles/mod.rs:280`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `const` | – | 11, 11 in all 148 emitters: list links. |
| 0x08 | 4 | `u32` | `u_0x08` | – | **open**. |
| 0x0C | 4 | `hash32` | `description` | – | Bare hash of a particle effect description's name (the description's +0x08); no kind owns descriptions yet (§1 "Effects"). Scope: the resident description list in load order, `GLOBALB.BUN`'s first, then this region's (`effects.bin`); the first match wins (0x48BC00). |
| 0x10 | 4 | `u32` | `state` | – | Default 1 (every emitter): 1 emitting, 0 waiting; any other value never runs. |
| 0x14 | 20 | `u8[20]` | `pad` | – | |
| 0x28 | 4 | `u32` | `u_0x28` | – | **open**. |
| 0x2C | 4 | `hash32` | `switch` | – | `bin_hash` of a switch name (`FREE_ROAM`, `BARRIERS_<race>`, `PLAYER_BARRIERS_<race>`): the emitter runs only while that name is switched on (0x492E10 → 0x5D7450; names are switched by 0x571170). Compared with switched names, never resolved to an item. `0x00000000` = always runs. |
| 0x30 | 16 | `u8[16]` | `pad` | – | |
| 0x40 | 64 | `f32[16]` | `transform` | – | 4×4, row vectors. |
| 0x80 | 16 | `u8[16]` | `pad` | – | |
| 0x90 | 32 | `u32[8]` | `u_0x90` | – | **open**; holds the emitter's name as text in the reference install (`XEE_Event_BODYSHOP_IconA_01`; docs/formats/cars-1.md "Particle effects"). |
| 0xB0 | 32 | `u8[32]` | `pad` | – | |

### 3.5 `smackables.toml` (`ob.smackables/1.0`) and `smackables.bin`

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.smackables/1.0` |
| `kind` | array of `prop_kind` tables | – | yes | – | `0x34026`, in order (L4RA: 79); always inline. |
| `links` / `link` | array descriptor, layout `scenery_link` / array of `scenery_link` tables | – | one of | – | `0x34107`, in order (L4RA: 6,431). |
| `group` | array of `scenery_group` tables | – | yes | – | `0x34108`, in order (L4RA: 130); always inline. |
| `extras` | table | – | no | – | conventions §9. |

Layout `prop_kind` (0x100 bytes, alignment 16; 0x5B0E40; docs/formats/cars-1.md "Knock-over props"):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `pad` | – | List links, zero on disk. |
| 0x08 | 24 | `char[24]` | `name` | – | `ZCS_PylonA`; NUL-terminated, zero-filled. |
| 0x20 | 4 | `f32` | `mass_tonne` | tonne | The rigid-body description's mass (0x5B1550 → 0x59B950). |
| 0x24 | 12 | `f32[3]` | `const` | – | 1.0, 1.0, 1.0 in all 566 kinds; meaning open. |
| 0x30 | 64 | `f32[16]` | `inertia` | – | 4×4; the 3×3 part is inverted into the body (0x43C7C0). |
| 0x70 | 4 | `f32` | `yaw_factor` | – | × the yaw term of the ground inverse inertia; default 1.0 (every kind). |
| 0x74 | 12 | `u8[12]` | `pad` | – | |
| 0x80 | 4 | `hash32` | `key` | – | What instances refer to; default `bin_hash(name)` (equal in all 566 kinds). |
| 0x84 | 4 | `u32` | `u_0x84` | – | **open**. |
| 0x88 | 4 | `u32` | `u_0x88` | – | The attached effects' list (+0x88) [inferred]. |
| 0x8C | 4 | `u32` | `u_0x8c` | – | **open**. |
| 0x90 | 16 | `u32[4]` | `pad` | – | Resolved kinds at run time. |
| 0xA0 | 16 | `hash32[4]` | `linked` | – | Bare hashes of kind `prop-kind`: `key`s of this region's prop kinds, resolved into +0x90 (the `XO_BulletSS_*` kinds name `ZCS_BulletSS`); `0x00000000` = none. Default: four times `0x00000000`. |
| 0xB0 | 12 | `f32[3]` | `centre_of_mass` | m | Prop space. |
| 0xBC | 4 | `u32` | `pad` | – | |
| 0xC0 | 4 | `u32` | `u_0xc0` | – | **open**. |
| 0xC4 | 4 | `f32` | `break_impulse` | – | The impulse that knocks the prop over (0 = loose). |
| 0xC8 | 4 | `u32` | `pad` | – | |
| 0xCC | 4 | `f32` | `restitution` | – | As stored; the engine holds it to ≤ 0.005 (0x5B1550). |
| 0xD0 | 4 | `u32` | `pad` | – | Resolved at run time. |
| 0xD4 | 4 | `hash32` | `u_0xd4` | – | A hash resolved by 0x5D7B60 into +0xD0 [inferred]; its kind is open, so it is carried as an open field. |
| 0xD8 | 4 | `u32` | `pad` | – | The material index at run time. |
| 0xDC | 4 | `hash32` | `material` | – | `bin_hash` of one of the engine's 19 material names (`TRAFFIC_CONE`, …; table 0x7FC808), not an item; another hash means `NONE`. |
| 0xE0 | 4 | `f32` | `friction` | – | As stored; the engine holds it to ≥ 0.3. |
| 0xE4 | 12 | `u8[12]` | `pad` | – | |
| 0xF0 | 16 | `u8[16]` | `u_0xf0` | – | **open**; different in every kind. TOML: `bytes`. |

The stored `restitution` and `friction` are kept raw; the engine's clamps are behaviour, applied after reading, so
the model equals the original's.

Layout `scenery_link` (8 bytes, alignment 2; 0x5DF8B0, 0x5D6440):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 2 | `i16` | `scenery` | – | A scenery section's number. |
| 0x02 | 2 | `i16` | `index` | – | The instance's index there: the link names `placement:<REGION>/<scenery>/<index>` (`WLD.9`). |
| 0x04 | 2 | `u16` | `instance_flags` | – | What 0x5DB0F0 writes into the instance's flags when the entry changes; equal to the instance's flags on disk in L4RA. Bit 0x10 of byte +0x04: a prop stands in for it. |
| 0x06 | 1 | `u8` | `kind` | – | 1: the placed prop becomes an unhit body (0x5B0C40). |
| 0x07 | 1 | `u8` | `const` | – | 0 in all 21,342 links of the eight regions; meaning open. |

The table is sorted by the scenery section's low byte (0x5D63F0); writers keep the given order.

Layout `scenery_group` (variable: (count·2 + 0x37) & ~3 bytes, alignment 4; 0x5DB2F0, 0x5DB350; always TOML):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `const` | – | 11, 11 in every group: list links. |
| 0x08 | 32 | `char[32]` | `name` | – | `BARRIERS_4001`, `SMOKEABLE`, `FREE_ROAM`; zero-filled. |
| 0x28 | 4 | `hash32` | `hash` | – | Default `bin_hash(name)` (equal in all 181). |
| 0x2C | 4 | `u32` | `on` | – | Default 0 (every group on disk; switched at run time). |
| 0x30 | 4 | `u32` | `count` | – | *derived*: the number of `members`. |
| 0x34 | 2·count | `u16[count]` | `members` | – | Scenery-link indices. |
| … | 0 or 2 | `u8[]` | `u_tail` | – | The bytes up to the 4-byte end; non-zero in 4 groups of the reference install, so kept (`bytes`, default zero). |

### 3.6 `anims.toml` (`ob.world-anims/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.world-anims/1.0` |
| `entity` | array of `anim_entity` tables | – | yes | – | `0x37080`, one record per chunk, in file order (may be empty). |
| `instance` | array of `anim_instance` tables | – | yes | – | `0x37090`, likewise. |
| `extras` | table | – | no | – | conventions §9. |

The animations themselves are the bank channels of [animation.md](animation.md) (`0xE34010`), found by `animation`
hash. Each record follows 8 bytes of fill in its chunk.

Layout `anim_entity` (0x70 bytes, alignment 16; 0x435A94, 0x438C24):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `const` | – | 11 in all 159 entities; meaning open. |
| 0x04 | 4 | `hash32` | `hash` | – | The entity's own hash; `parent` of other entities names it. |
| 0x08 | 4 | `hash32` | `parent` | – | The parent entity's `hash`, in this file; `0x00000000` for the root. |
| 0x0C | 4 | `hash32` | `solid` | – | Bare hash of kind `solid`: the model; scope as `scenery_info.lod` (the region's solids, [meshes.md](meshes.md)'s lookup order); `0x00000000` = none. |
| 0x10 | 16 | `u8[16]` | `pad` | – | |
| 0x20 | 4 | `hash32` | `tree` | – | The tree the entity belongs to; instances name trees by it. |
| 0x24 | 4 | `hash32` | `animation` | – | Bare hash of kind `anim`: `bin_hash` of the animation's name less `_q`/`_t`/`_s`; scope: the region's animation banks ([animation.md](animation.md)). |
| 0x28 | 4 | `u32` | `u_0x28` | – | **open**. |
| 0x2C | 4 | `u32` | `pad` | – | |
| 0x30 | 64 | `f32[16]` | `rest` | – | 4×4 rest matrix, row vectors, tree space. |

Layout `anim_instance` (0xA0 bytes, alignment 16; 0x43A460 → 0x43A040, 0x43A4A0):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `id` | – | The key sound objects find it by (0x432170). |
| 0x04 | 4 | `hash32` | `tree` | – | An entity tree of this file (`anim_entity.tree`). |
| 0x08 | 4 | `u32` | `u_0x08` | – | **open**. |
| 0x0C | 4 | `u32` | `flags` | – | The player's flags; the engine adds 0x40 when `range_flag` ≠ 0 (0x43A72C). |
| 0x10 | 4 | `f32` | `speed` | – | 0 → 1 (0x4389AE). |
| 0x14 | 8 | `f32[2]` | `delay_s` | s | |
| 0x1C | 4 | `u16[2]` | `range` | – | Frame range used with flag 0x40. |
| 0x20 | 8 | `hash32[2]` | `keys` | – | The hashes world messages address it by (a shop door: its shop's trigger); compared, never resolved to an item. |
| 0x28 | 8 | `u8[8]` | `pad` | – | |
| 0x30 | 4 | `u32` | `range_flag` | – | Default 0. |
| 0x34 | 4 | `u32` | `pad` | – | |
| 0x38 | 4 | `u8[4]` | `window` | – | Track-number window (0x43A4E7). |
| 0x3C | 2 | `u16` | `hold` | – | Set: starts in state 3 when `keys[0]` is 0 (0x43A763). |
| 0x3E | 2 | `u16` | `const` | – | `0x6969` in all 100 instances; meaning open. |
| 0x40 | 64 | `f32[16]` | `transform` | – | 4×4, row vectors, world space. |
| 0x80 | 32 | `u32[8]` | `u_0x80` | – | **open**. |

### 3.7 `fog.toml` (`ob.fog/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.fog/1.0` |
| `map` | array of tables (below) | – | yes | – | The parameter maps of `0x8003B600`, in file order (may be empty). |
| `horizon` | array of `horizon_source` tables | – | yes | – | The horizon fog sources of `0x34250`, in file order (may be empty). |
| `extras` | table | – | no | – | conventions §9. |

`map`: one entry per `0x8003B601` (docs/formats/world.md "Parameter maps and fog", verified;
`crates/ob-formats/src/formats/parameter_map.rs`). This kind is stored at the **values** level (§6): the fields below are what the
engine reads; the chunks' exact bytes are a shadow in the residual.

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `name` | hash32 | – | yes | – | The map's name hash ("Normal Fog", "Night Normal Fog", …: names are not in the file). A later map with the same hash replaces an earlier one (the engine's rule). |
| `min`, `max` | vec2 | m | yes | – | The quadtree's box (header +0x04..+0x10). |
| `types` | array<u32> | – | yes | – | Field types (`0x3B603`; 0 = float). |
| `rows` | array<array<f32>> | – | yes | – | The value rows, one float per field in field order (≥ 1 row, each `len(types)` long; `WLD.18`). |
| `tree` | array<array<u16>> | – | no | `[]` | The quadtree nodes (`0x3B608`, or `0x3B607` widened from u8), each an array of exactly 4: a leaf is `[0, row, ·, ·]`, any other node the child indices for (x < mid, y < mid), (x < mid, y ≥ mid), (x ≥ mid, y < mid), (x ≥ mid, y ≥ mid) (0x5F0A40, 0x5F0940). Empty = one value everywhere. |

`horizon`: chunk `0x34250`, read by 0x5E01A0; all records kept, the engine uses those of type 6. Six regions have the
chunk with no records; the encoder writes their header from the empty `horizon` list.

Layout `horizon_header` (the first 0x10 bytes of chunk `0x34250`, after 8 bytes of fill; alignment 16; the records
follow from +0x10):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `const` | – | 3, 3 in all eight regions; meaning open. |
| 0x08 | 4 | `u32` | `const` | – | 2: the version. 0x5E01A0 reads nothing else, so the importer keeps a chunk of another version in the residual and writes no sources. |
| 0x0C | 4 | `u32` | `source_count` | – | *derived*: the number of sources. |

Layout `horizon_source` (0x64 bytes, alignment 4; `src/render/native/fx/horizon_fog.rs:66`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `const` | – | 11, 11 in all 16 records: list links (0x5E01A0 links each record into the list of its type). |
| 0x08 | 4 | `u32` | `type` | – | Default 6 (every record); the engine reads type 6 only. |
| 0x0C | 12 | `f32[3]` | `colour` | – | 0–255 per channel. |
| 0x18 | 8 | `u8[8]` | `pad` | – | |
| 0x20 | 12 | `f32[3]` | `position` | m | |
| 0x2C | 4 | `f32` | `full_m` | m | From here on the weight is 1. |
| 0x30 | 4 | `f32` | `u_0x30` | – | **open**; 0.2 in 12 records, 0.4 in 4 (0x5E01A0 files a type-0 record with 0 here as type 5). |
| 0x34 | 4 | `f32` | `start_m` | m | Nearer than this the weight is 0. |
| 0x38 | 4 | `f32` | `const` | – | 0.5 in every record; meaning open. |
| 0x3C | 4 | `u32` | `pad` | – | |
| 0x40 | 4 | `f32` | `const` | – | 1.0 in every record; meaning open. |
| 0x44 | 16 | `f32[4]` | `u_0x44` | – | **open** on disk: +0x44 / +0x48 are the screen position the engine writes each frame (0x5DB910); the stored bytes are left-over text in most records, so floats here often need the `bits:` form (conventions §3.4). |
| 0x54 | 4 | `f32` | `const` | – | 1.0 in every record; the weight the engine writes each frame (0x5DB910). |
| 0x58 | 12 | `f32[3]` | `u_0x58` | – | **open** on disk: +0x60 is the visibility the engine writes each frame; left-over text as above. |

### 3.8 `effects.bin` (stopgap)

The region's particle effect descriptions (`0x35021`, 0x220-byte records; L4RA 37, L4RD 25; the other six regions
have the chunk with no records) are read by the engine (`src/particles/mod.rs:204`), but no kind owns them yet
([README.md](README.md) §3). Until one does, `region.toml` MAY carry `effects = { file = "effects.bin", offset,
count, layout = "emitter_description" }`, layout `emitter_description` = one row `u8[0x220] record`, the record
exactly as found, decoded by the engine's existing parser. Without `effects` the region has no descriptions of its
own; the encoder then writes the chunk's header with count 0.

Layout `emitter_description_header` (the first 0x10 bytes of chunk `0x35021`, after 8 bytes of fill; alignment 16;
the records follow from +0x10):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `const` | – | 7, 7 in all eight regions; meaning open. |
| 0x08 | 4 | `u32` | `const` | – | 4: the version. The engine reads no other (`src/particles/mod.rs:204`), so the importer keeps a chunk of another version in the residual and writes no `effects`. |
| 0x0C | 4 | `u32` | `description_count` | – | *derived*: `effects.count`, or 0 without `effects`. |

Eq2 by construction: the header from this table, the records copied. This stopgap is replaced, not extended, by the
effects kind.

### 3.9 `placements.toml` (`ob.placements/1.0`, mod packs only)

The file Studio and the map editor write: moves, disables and additions of placements in one region. One file per
region per pack, entries sorted by writers (originals by key, then additions in their order) so diffs stay small.

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.placements/1.0` |
| `placement` | array of tables | – | yes | – | The entries below. |
| `extras` | table | – | no | – | |

Entry fields:

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `op` | enum `merge`, `add` | – | no | `add` with `id`, else `merge` | layering.md §3.1: an entry carrying this pack's own local id (`id`) defines a pack-scoped item and defaults to `add`; an entry with `key` defaults to `merge`. `replace` is refused (`MRG.3`). |
| `key` | string | – | merge | – | `"<scenery>/<index>"` for an original placement (scenery section, index); `"<pack>:<id>"` for one another pack added, which this pack MUST list in `[dependencies]` (`ORD.2`, §4.2). |
| `guard` | table {`solid`: ref solid, `position`: vec3} | – | merge of an original | – | The original's drawn solid and position as the base set has them (§4.3). |
| `id` | string `[a-z0-9][a-z0-9_-]{0,31}` | – | add | – | Local id; the item's key is `<this pack id>:<id>`. |
| `scenery` | u32 | – | add | – | The scenery section the placement belongs to: it is drawn exactly when that scenery section is drawn (`WLD.14`). |
| `solid` | ref solid | – | add | – | LOD A: a scoped reference (`solid:<NAME>` or `solid:0x<HASH>`), resolved in this region's unit by [meshes.md](meshes.md)'s lookup order. |
| `lod_b`, `lod_c` | ref solid | – | no | none | Lower LODs, resolved as `solid` (none: nothing drawn at that detail, as the original (choice: format rule; the original's layout is in docs/formats/)). |
| `radius_m` | f32 | m | no | half the solid's box diagonal | Bounding radius (the cull adds 6). |
| `flags` | u16 | – | no | `0x0000` | Instance flags (§3.4.1); add only. |
| `prop` | ref prop-kind | – | no | none | Add only: the placement is also a knock-over prop of this kind (an item key of this region, `prop-kind:<REGION>/<NAME>`). |
| `position` | vec3 | m | add | – | Merge: the new position. |
| `rotation` | mat3 | – | no | identity (add) | Merge: the new rotation. Rows unit within 0.01, \|det\| ≥ 0.9 (mirrors allowed) (`WLD.12`). |
| `disabled` | bool | – | no | `false` | layering.md §3.1. |
| `extras` | table | – | no | – | |

On an original placement, `merge` may set `position`, `rotation` and `disabled` only; any other field is `MRG.3`.
Coordinates lie within ±100,000 m (`WLD.12`).

```toml
schema = "ob.placements/1.0"

# Two cones at exactly the same spot in the original: indices 0 and 1 of scenery section 1407
# (examples/world/sections/1427.toml).
[[placement]]
key = "1407/0"
guard = { solid = "solid:XO_CONE_01", position = [-215.25, -1037.5, 146.0] }
position = [-214.0, -1036.0, 146.0]
rotation = [0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0]

[[placement]]
key = "1407/1"
guard = { solid = "solid:XO_CONE_01", position = [-215.25, -1037.5, 146.0] }
disabled = true

# A new cone (no `op`: an entry with `id` is an addition).
[[placement]]
id = "a1"
scenery = 1407
solid = "solid:XO_CONE_01"
flags = 0x0060
radius_m = 0.75
prop = "prop-kind:L4RA/ZCS_PylonA"
position = [-212.0, -1035.0, 146.0]
```

## 4. Layering

### 4.1 What a mod pack may do

| Kind | merge | add | replace | disable | Clash key |
|---|---|---|---|---|---|
| placement (original) | `position`, `rotation` | – | `MRG.3` | yes | (region, scenery, index) |
| placement (added by a pack) | `position`, `rotation`, from a pack that depends on the adding pack | yes, in `placements.toml` (by the adding pack only) | `MRG.3` | yes | (region, pack, id) |
| region, stream section, scenery section, info | `MRG.3` | `MRG.3` | `MRG.3` | – | |
| visibility, lights, flares, prop instances, emitters, prop kinds, links, scenery groups, world animations, fog, effects | `MRG.3` | `MRG.3` | `MRG.3` | `MRG.3` | |

A mod pack holding any file of this page but `placements.toml` is refused (`WLD.10`); a base set holding
`placements.toml` is refused likewise. No wildcard units (`world/_any/`) for this page. Everything marked `MRG.3`
is refused for now and is the subject of later minor versions (§9).

### 4.2 Merged order, slots, keys

- The merged placements of a region are: the base set's placements in the order of §3.4.2, each kept in its slot
  whatever is merged into it or disabled; then the added placements, the lowest layer's first, each layer's in
  entry order. This order is the order the engine places, draws and culls; an added placement never shifts an
  original's index or key, so scenery links, scenery groups and existing packs keep naming the same items.
- **Disabled** (layering.md §3.1): the placement is not drawn; its knock-over prop is removed; scenery-link entries
  naming it no longer reach it, so no scenery group shows it again (`World::hide_placement`,
  `src/world/map_overlay.rs:165`). Its slot and key stay. Its share of the stream section's static collision
  **stays**: collision is baked per stream section with no link to placements ([collision.md](collision.md) §4); a
  pack that moves or disables a building therefore leaves its collision where the original has it (today's
  documented limit, docs/modding/map-editor.md "Limits").
- **Moved**: the placement takes the new transform; a knock-over prop standing in for it takes it as its rest pose
  (`set_placement_transform`). `bbox_min`/`bbox_max` and the original's unread scenery chunks are not updated (the
  engine reads neither).
- **Added**: the engine makes one placement from the entry: drawn solid `solid`, `lod_b`/`lod_c`, the transform,
  the stream section that holds `scenery`, `flags`, cull radius `radius_m + 6`, and a knock-over prop of kind `prop`
  at the same pose when given. Added placements have no static collision ([collision.md](collision.md) §4).
- A merge of `key = "<pack>:<id>"` addresses another pack's addition. Only the adding pack defines that key, so a
  pack that merges it MUST list the adding pack in its `[dependencies]` (which keeps it below and enabled, `ORD.2`;
  layering.md §3.1 "Pack-scoped keys"), and the item MUST exist there (`MRG.1`). A higher pack that merely happens
  to be above the adding pack is refused (`ORD.2`).
- Clashes (layering.md §4) are per placement key and field; a disable in one pack against a move in another is a
  clash shown with both.

### 4.3 Guards

A `merge` of an original placement carries `guard`. The reader compares it with the **base set's** placement of
that key (before any layer): `guard.solid` MUST name the placement's drawn solid (§3.4.2), and `guard.position`
MUST lie within 0.05 m of its position (distance computed in binary32 as in §4.4 step 4). Otherwise the guard is
stale (`WLD.11`): the pack was made against another base set, and editing whatever now has that index would be
wrong. The tolerance is the legacy match distance (`MATCH_DISTANCE`, `src/world/map_overlay.rs:14`), so a converted
legacy pack is accepted exactly when its legacy form resolves.

In the code: the load records every original placement's key (`World::original_placement`,
`World::original_key`: 12 bytes a placement, once in the world and once in its loaded state, so 1.2 MB and about
0.2 ms of a 110 ms parse on L4RA), and `ob_studio::keys::guard_holds` checks a guard on a loaded world against the
key's drawn solid and its position before any map mod moved it (`World::original_position`). A key the base set
places nothing at (no such instance, or one whose solid is missing or empty) is `MRG.1`.

**One stale reference refuses the whole pack** (conventions §11). A stale guard (`WLD.11`), a merge of a key no
lower layer has (`MRG.1`), an addition whose `scenery` names no scenery section (`WLD.14`) and a `solid`, `lod_b`,
`lod_c` or `prop` that resolves to nothing (`CONV.9`) are all errors against the stack: the pack is not mounted,
nothing of it is played, and the launcher's message names how many items are stale and the first of them ("made for
another game version: 3 placements not found, first L4RA/1407/12"). Studio still opens the pack and lists every
stale item for repair. Only legacy format-1 map mods (§4.4) keep today's rule instead: an edit that resolves to
nothing is skipped and counted (`WLD.15`).

### 4.4 Legacy references (today's map mods)

Today's map mods (`maps/<folder>/map.obmap`, format 1, docs/modding/map-editor.md "File format") are mounted as legacy packs
(layering.md §7) and can be converted to `placements.toml`. A format-1 edit names an original placement by
`PropRef` (`crates/ob-map/src/tools/map/format.rs:63`): `region`, `section` (a **stream** section's number), `solid` (the name of
the drawn solid) and `at` (its original position), matched within 5 cm. The map editor's **next format-1
revision** (not released; no file has it yet) adds the additive key `nth` (an integer ≥ 1, written only when
> 0), which tells exact duplicates apart by their rank in the original order; every file without it reads as
`nth` = 0, which is today's behaviour. Resolution into a key is deterministic:

1. The region: the unit whose name equals `region` ignoring ASCII case. None: unresolved.
2. The stream section numbered `section` in that region's `region.toml`. None: unresolved.
3. The candidates C: the **base set's** original placements of that stream section, in the order of §3.4.2 (its
   scenery sections in chunk order, instances in index order), that the engine places (a solid resolved and not
   empty) and whose drawn solid's name equals `solid` exactly.
4. For each c in C: d(c) = |c.position − at|, computed in binary32 like the engine (`glam::Vec3::distance`:
   component differences, squares summed x, y, z in that order, square root). Keep d ≤ 0.05 (as binary32).
5. `nth` = 0: the first c in C order with the smallest d. `nth` = k > 0: the list E of c with d = 0 in C order;
   E[k] when k < len(E), else unresolved.
6. The result is that placement's key (region, scenery section, index). An unresolved edit is skipped and counted,
   with a warning (`WLD.15`), never a refusal: legacy packs keep today's "skipped and counted in the log" rule. The
   converter writes no entry for an unresolved edit (it changes nothing in the legacy mount either) and lists it,
   so the converted pack merges to the same world and is not refused for it.

The revised map editor and the converter write `nth` as p's rank among C's placements with d = 0 to p's own
position, only when > 0. Additions (`[prop.add]`) convert to `add` entries: `id` from the revision's additive key
`id = a<k>` where present, else `a<k>` from the edit's position in the file; `scenery`, `lod_b`, `lod_c`, `flags`,
`radius_m` and `prop` taken from the nearest base-set placement of the same solid without flag 0x0100 (flags less
0x0010, 0x0100, 0x0200, 0x8000; first nearest wins), `scenery` from the nearest placement of any solid when none,
then no lower LODs, flags 0 and the half diagonal; the template is fixed at conversion (or mount) time and written
out. This is today's rule (`add_placement`, `src/world/map_overlay.rs:188`) taken over the base set.

**Where this differs from today's apply** (`apply_map_mods` applies edits one after another on the live world):
(a) when two enabled map mods change the same original placement, today the second mod looks it up at the first
mod's new position and usually misses it; resolved against the base set, both name the same key and the stronger
mod wins as a shown clash; (b) today's template search for an addition also sees placements moved or added before
it and the engine's own world-animation pieces; the base-set search does not. Both cases are listed by the
converter and covered by tests (§6.3).

Measured on L4RA (the base-set instance arrays, the engine's placement rule; read-only script): 48,223 placements;
1,372 pairs of the same stream section and solid within 5 cm, 295 of them at exactly the same position, in 229
groups (213 pairs, 6 triples, 9 quadruples, 1 quintuple). Without `nth`, exactly 256 placements (the non-first
members of those groups) resolve to another placement; every other placement resolves to itself. (An earlier count on the
loaded world gave 1,375 and 296 [to verify which placements it included].)

## 5. Import

| Original | Decoder (existing) | Writes |
|---|---|---|
| `0x34110` | `formats::track::parse_streaming_sections` + record copy | `region.toml` `section` |
| `0x34151` +0, `0x34152`, `0x34153` | `formats::visibility::Visibility::parse` + record copy | `region.toml` `detail_limit`, `visibility.toml` |
| `0x80034100` (`0x34101`–`0x34103`) | `formats::scenery::parse_scenery_section` + record copy | `sections/<n>.toml` `scenery` (infos inline, instances external) |
| `0x80135000`, `0x80135100` | `formats::lights::{LightPack, FlarePack}::parse` + record copy | `lights`, `flares` (nodes, lights, flares external) |
| `0x34027` | `formats::smackable::parse_instances` + record copy | `props` (external) |
| `0x35020` | `particles::parse_placed` + record copy | `emitters` (external) |
| `0x34026`, `0x34107`, `0x34108` | `formats::smackable::{parse_infos, parse_scenery_links, parse_scenery_groups}` + record copy | `smackables.toml`, `smackables.bin` |
| `0x37080`, `0x37090` | `formats::world_anim::WorldAnimations::parse` + record copy | `anims.toml` |
| `0x8003B600` | `formats::parameter_map::parse_parameter_maps` | `fog.toml` `map` (values) |
| `0x34250` | `horizon_fog::parse_sources` + record copy | `fog.toml` `horizon` |
| `0x35021` | record copy (the engine's parser, `particles::parse_descriptions`, for Eq1) | `effects.bin`, `region.toml` `effects` |

- The importer walks the stream-section table in order and, inside each range, the chunks in file order, skipping the
  `--` record (number 0) as `World::parse` does (§1): every chunk is then in exactly one loaded record's range, and
  the importer writes no `also` entry.
- Every field is copied as found; `pad`, `const` and *derived* rows and the fill are checked (§3.1; `CONV.11`,
  `WLD.6`, `WLD.4`). In every chunk with an Eq2 encoder (§6.1) each byte is a row of a layout table of §3 or fill,
  so the encoder rebuilds the chunk whole: id and size, fill, header (fields, `const` values, *derived* counts and
  stream-section numbers) and records. In particular the headers of `0x34027`, `0x35020`, `0x35021` and `0x34250`
  (their `const` words included) are rebuilt from §3's tables, not kept.
- **Residual** (`fate = "kept"`): `0x34104`–`0x34106`, `0x80036000`, `0x34111`–`0x34113`, `0x34154`, `0x34155`,
  `0x34151` after its first u32, `0x37110`, `0x37150`, `0x8003B000`; any light, flare, emitter, description or
  horizon chunk of a version the original drops (§3.1 `const`); padding chunks and alignment runs between chunks as
  `fill`. `0x8003B600` is decoded with proof `values` and kept as a shadow (residual.md §3.2).
- Container chunks are rebuilt from their children (`fate = "container"`).

## 6. Equality and verification

### 6.1 Levels

| Kind | Level | Encoder (Eq2) | Shadow |
|---|---|---|---|
| stream-section table | Eq2 bytes | records from `region.toml` (incl. `stream_offset`/`stream_size`) | none |
| visibility (outlines, lists) | Eq2 bytes | records from `visibility.toml` | none |
| scenery sections | Eq2 bytes | header from `number`; infos from TOML; instances copied | none |
| lights, flares, prop instances, emitters | Eq2 bytes | headers from §3's tables (TOML fields, `const`, *derived*), records copied | none |
| prop kinds, links, scenery groups | Eq2 bytes | records from TOML / copied | none |
| world-animation entities, instances | Eq2 bytes | records from TOML | none |
| horizon fog | Eq2 bytes | header from §3.7's table, records from TOML | none |
| parameter maps | Eq1 values | none yet (the values chunk's stride and field offsets are not kept) | the whole `0x8003B600` (L4RA 36,916 B; all regions 104,892 B) |
| particle effect descriptions | Eq2 bytes | header from §3.8's table, records copied | none |

### 6.2 Proofs (per region, at import; repeated by `overbright-studio-cli base verify`)

- **Eq1, values**: for each decoder of §5, the `xxh3-128` of its result encoded with the asset cache's flat encoding
  (`crates/ob-core/src/cache/persist.rs`, the `persist!` lists of the parsed structs) from the original bytes equals the same from
  the base set's files. And region-wide: the world model the engine builds (`World::parse` up to the
  `w.rest` cache section, with solids and textures from their pages) is byte-equal from both sources, including
  `missing_solids` and the order of every list.
- **Eq2, bytes**: each encoder above reproduces its residual-map node byte for byte (residual.md §3.2), then the
  rebuild of both track files gives their `content_digest`.

### 6.3 Tests

1. `world_import_round_trip` (every region): import → load from the base set → the cache sections `w.rest`
   (and the parser hashes) equal a load from the original files; rebuild both track files byte-identical.
2. `placement_keys_are_original_indices`: for every scenery-link entry of every region, `placement:<scenery>/<index>`
   resolves to the instance the original's link names; skipped instances keep their index (L4RB's 2 missing).
3. `legacy_refs_resolve_every_original_placement` (L4RA): for each of the 48,223 placements, its legacy reference
   (stream section, drawn solid's name, original position, `nth` as §4.4 writes it) resolves to its own key;
   exactly 256 references carry `nth` > 0; without `nth` exactly those 256 resolve to the first of their group; the
   1,372 / 295 pair counts are asserted so a change in the base data is noticed.
4. `legacy_duplicates_edit_independently` (the 295 exact pairs): a `map.obmap` that moves copy 0 and disables
   copy 1 of an exact duplicate group gives two keys, one moved and one disabled placement; the file reads and writes
   byte-identical; its conversion to `placements.toml` merges to the same world; every existing format-1 test file
   (none has `nth`) reads and writes byte-identical.
5. `added_placements_keep_original_indices`: two packs adding placements and a third disabling an original: every
   original key and the draw order of originals are unchanged; added ones follow in layer order.
6. `guard_mismatch_refuses_the_pack`: a merge whose guard solid differs, or whose position is 0.051 m off, refuses
   the whole pack with `WLD.11`; 0.049 m is accepted. A pack with three stale entries is refused once, and its
   message names the count (3) and the first stale key; the same edits as a legacy format-1 map mod are skipped
   and counted (`WLD.15`).
7. `legacy_differences_are_listed`: the two cases of §4.4 give the documented results.

## 7. Cooked form

- **Cook unit**: the region (today's track cache entry, `crate::cache::TrackCache`). Key: the digests of every file
  of `world/<REGION>/**` in each contributing layer and of the shared texture sets, the layer ids and versions
  (layering.md §1), and the layout hash of the model below.
- **Model**: today's `w.rest` cache section (`src/world/cache.rs:245`: stream sections, placements, visibility,
  parameter maps, horizon fog, light and flare packs, knock-over props, scenery groups, links, world animations,
  placed emitters, particle effect descriptions, barrier groups and the ground, [collision.md](collision.md) §7),
  next to the meshes' and textures' cache sections. Merging (§4) happens before the cook, so the cooked placements
  already hold the pack edits, and `World::reset_to_loaded` returns to the merged world as the map overlay does
  today.
- **Zero-copy**: none for this page's kinds today (instances become transforms, lights and props become engine
  structs). The external arrays are 64-aligned in stored bundle entries (conventions §4.3), so a later cooked version MAY read them in
  place.
- **Build-specific** (not normative): the engine's own placements (world-animation pieces, the GPS aid marker, the
  four dyno rollers, made at load after the original placements and before the added ones, today's order), the
  light-placement lists, the look-ahead caches, door and loop players.

## 8. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| WLD.1 | A file of this page in the wrong place: `region` not the unit's name; a stream-section file whose `number` is not in `region.toml` or not its file name. |
| WLD.2 | Two stream sections with the same number, or a stream section's name that is empty or longer than 7 characters. |
| WLD.3 | A scenery section number used twice in a region, or more than two scenery sections in a stream section. |
| WLD.4 | A list given in both forms or in neither where one is required; an external array of the wrong layout for its field; at import, a *derived* row (§3.1: a count or a copy of the stream section's number) that does not match. |
| WLD.5 | An instance's `info` beyond its scenery section's infos. |
| WLD.6 | A `const` row (§3.1) with another value: in an external array, or at import (other than a version word, whose chunk is kept in the residual instead); alignment fill other than `0x11` at import. At import it means the edition differs from this specification. |
| WLD.7 | A rotation or position that is not finite. |
| WLD.8 | A visible list longer than 72, or an outline with fewer than 3 points. |
| WLD.9 | A scenery link (or a prop instance's `link`) naming no original placement. |
| WLD.10 | A mod pack holding a file of this page other than `placements.toml`, or a base set holding `placements.toml`. |
| WLD.11 | A guard that does not match the base set's placement (solid differs or position more than 0.05 m off). Refuses the whole pack (§4.3). |
| WLD.12 | In `placements.toml`: a rotation whose rows are not unit within 0.01 or whose \|det\| < 0.9; a coordinate beyond ±100,000 m. |
| WLD.13 | In `placements.toml`: two entries with the same key; an entry with neither `key` nor `id`; `merge` of an original without `guard`; `add` without `id`, `scenery`, `solid` or `position`, or with `key`; an invalid `id`. |
| WLD.14 | An addition's `scenery` names no scenery section of the region. Refuses the whole pack (§4.3). |
| WLD.15 | Warning, legacy format-1 map mods only: an edit that resolves to no placement is skipped and counted in the log; the pack stays mounted (§4.4). |
| WLD.18 | A parameter map without rows, or a row whose length differs from its `types`. |
<!-- /generated -->

(`WLD.16` and `WLD.17` are reserved for the lights and flares merges of §9.)

Errors against the stack in a mod pack (`WLD.11`, `WLD.14`, and `MRG.1`, `ORD.2`, `CONV.9` on this page's entries)
follow conventions §11: **one stale item refuses the whole pack**, and the launcher names how many items are stale
and the first key. `WLD.15` is the only warning: legacy format-1 map mods keep today's skip-and-count rule. The
same rules found in a base set (`WLD.1`…`WLD.9`, `WLD.18`) make it damaged (conventions §11).

## 9. Versioning and extension

Minor versions MAY add, with defaults that keep today's behaviour:

- merges and disables of lights, flares, placed emitters and prop kinds (keyed `light:<REGION>/<n>/<i>`, …), each
  in a pack file `world/<REGION>/lights.toml`-style next to `placements.toml`;
- additions of lights with the light tree rebuilt by the reader (needs the tree's build rule traced);
- visibility edits (adding a stream section or scenery section to visible lists), additions of whole stream
  sections (new areas), and added placements that belong to several scenery sections (an addition drawn from every
  scenery section that can see it);
- fog edits: merges of parameter-map rows by (map, row) and of horizon fog sources;
- names for the `u_` fields once traced (a rename keeps the key's position and the stored value), and a
  `const` row found to be read as a value becomes a field whose default is that constant (no stored file
  changes).

New features before a schema version takes them use `[ext.<NAME>]` (conventions §9). A collision delta tied to
placement keys is reserved in [collision.md](collision.md) §9.

## 10. Open points

- **Open fields** (carried losslessly as `u_` fields): `stream_section` +0x10, +0x1C, +0x20, +0x24, +0x30;
  `scenery_info` +0x2C, +0x40; `scenery_instance` +0x1C and whether the original reads the instance box;
  `light_tree_header` +0x06, +0x0A; `light_node` +0x0C; `light` +0x00 (the name's hash) and +0x3C..+0x5B (a u16 and
  the light's name); `flare` +0x2E; `prop_instance` +0x08; `emitter` +0x08, +0x28, +0x90..+0xAF (the emitter's
  name); `prop_kind` +0x84..+0x8C, +0xC0, +0xD4, +0xF0; `anim_entity` +0x28; `anim_instance` +0x08, +0x80..+0x9F;
  `horizon_source` +0x30, +0x44..+0x53, +0x58..+0x63.
- **Constants of open meaning** (`const` rows, rebuilt from the values in §3): `light` +0x06, +0x0F, +0x38; `flare`
  +0x0C; `prop_kind` +0x24; `scenery_link` +0x07; `anim_entity` +0x00; `anim_instance` +0x3E; `horizon_source`
  +0x38, +0x40; the first two words of the prop, description and horizon headers. An edition with other values is
  refused at import (`WLD.6`) rather than misread.
- **Unread chunks** in the residual: `0x34104`–`0x34106` (5.8 MB in L4RA, likely the original's precomputed
  culling or spatial data [guess]; a moved or added placement does not update them, and Overbright does not read
  them), `0x80036000`, `0x34154`, `0x34155`, the rest of `0x34151`, `0x37110`, `0x37150`, `0x34111`–`0x34113`,
  `0x8003B000`.
- **Particle effect descriptions** (`0x35021`) have no kind (§1 "Effects", §3.8): a future effects kind should own
  them, `GLOBALB.BUN`'s and `Showroom.BIN`'s included; the region's `effects.bin` is then replaced by it.
- **The `--` entry** (`u_0x10` = 0): resolved. The original never loads it (§1) and Overbright's loader skips it, so
  L4RH's and L4RR's scenery section 2600 is placed once; `also` is no longer written (no format change).
- **Parameter maps** at values level: an encoder needs the values chunk's stride and field offsets kept; until
  then the shadow stays.
- The legacy pair counts (1,372 / 295 on the instance arrays, 1,375 / 296 in an earlier count on the loaded
  world) [to verify which placements that count included]; the resolution rule does not depend on them.

## 11. Sizes and timing

Reference install, all eight regions (L4RA in brackets). Original sizes are chunk bytes including headers.

| Data | Original | Base set | Notes |
|---|---|---|---|
| Stream-section tables | 76 KB (31 KB) | ~120 KB TOML [guess] | 946 records |
| Visibility | 195 KB (84 KB) | ~250 KB TOML [guess] | |
| Scenery sections | 21.5 MB (9.8 MB) | instances 7.8 MB `.bin` (3.1 MB) + infos ~4.5 MB TOML (1.8 MB) [guess] | 1,378 scenery sections, 31,593 infos, 122,178 instances (L4RA: 606, 12,858, 48,270); 11.4 MB (5.8 MB) of unread chunks go to the residual |
| Lights, flares | 1.25 MB (0.51 MB) | ~1.2 MB `.bin` | 5,597 lights, 5,359 flares |
| Prop instances, emitters | 0.21 MB (0.09 MB) | same, `.bin` | 1,535 props, 148 emitters |
| Prop kinds, links, groups | ~0.27 MB (0.10 MB) | ~0.3 MB | |
| World animations | ~29 KB (15 KB) | ~40 KB TOML | |
| Parameter maps, horizon fog | 106 KB (38 KB) | ~150 KB TOML + 105 KB shadow | |
| `0x80036000` and other unread | 1.0 MB (0.43 MB) | residual | |

Instances per scenery section (L4RA): median 29, 90th percentile 229, maximum 618; infos per scenery section:
median 17, maximum 89. A whole-region load reads the external arrays as stored bundle entries; the TOML parts of
L4RA (~2.5 MB) parse in ~0.05–0.1 s [guess] on the first cook only. Import of all eight regions' world data:
~2–4 s [guess], dominated by the residual's deflate.
