# Asset format: roads, AI routes, traffic data, track markers, custom races

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md), [layering.md](layering.md) and [residual.md](residual.md) apply and are not repeated.

This page defines how the original's route folders (`TRACKS/ROUTES<REGION>/`), the AI tuning of the global data,
and the custom races of mod packs are stored. Field meanings come from `docs/formats/ai.md` ("AI route
layer", "Traffic"; verified except where noted) and `docs/formats/career-1.md` ("Routes", "Markers", "Triggers",
"Race route and progress"); the tags **verified**, **inferred** and **open** are theirs.

## 1. Scope

| Original data | Chunk ids | Kind (this page) |
|---|---|---|
| `TRACKS/ROUTES<R>/Paths<trk>.bin`: road network | container `0x80034147` → `0x34148` nodes, `0x34149` segments | `ob.road-net` |
| the same files and `PathsFreeRoam.bin`: route triggers, barriers, checkpoint lists (one copy per region, see §5.2) | `0x3414A`, `0x3414D`, `0x3414C` | `ob.route-paths` |
| `TRACKS/ROUTES<R>/Routes<trk>F.bin`, `Routes<trk>B.bin`, `RoutesFreeRoam.bin`, `RoutesAll.bin` | `0x34121` records + control points, `0x34122` junctions, `0x34123` traffic intersections (`0x34124` race traffic teleporters: in no shipped file) | `ob.ai-routes` |
| `TRACKS/ROUTES<R>/TrackPosMarkersAll.bin`, `TrackPosMarkersFreeRoam.bin` | `0x34146` | `ob.track-markers` |
| `GLOBAL/GlobalB.lzc` (and the identical unpacked `GLOBAL/GLOBALB.BUN`): the AI tuning | `0x34B00` | `ob.ai-tuning` |
| map mods' `[race]` sections (`maps/<folder>/map.obmap`) and new races of mod packs | – | `ob.race` |

Not here: the race tracks' records (GlobalB `0x34201`) and career events, which are [career.md](career.md); the
barrier *scenery* groups and collision (`0x3412F`, [collision.md](collision.md)); the track bundle's world-animation
instances (`0x37090`, [world.md](world.md)); the track maps' pictures (`TRACKS/TRACKMAP*.BIN`, [textures.md](textures.md)). The
traffic cars' models and caps are car records, and the AI's upgrade presets (`0x34A31`), rating ranges (`0x34A32`)
and class curves (`0x34A33`) are performance data ([cars.md](cars.md)). Traffic, AI and race *behaviour* (the walk,
the free-roam post-load 0x5E9E90, the race road build 0x5E5BC0, the locators) is engine code: it is not data and
does not appear here, but it decides what the data must keep (order, record ids, junction slots).

**Reference install**: 355 files, 31,132,472 bytes in 8 region folders (`L4RA` 185 files, `L4RB` 32, `L4RC` 41,
`L4RD` 14, `L4RF` 29, `L4RG` 44, `L4RH` 5, `L4RR` 5). Every file is a plain chunk file (no JDLZ), chunks packed
without gaps; every byte is covered by the chunks listed above.

## 2. Files and keys

```text
world/<REGION>/routes/
  paths.toml                   ob.route-paths   the region's triggers, barriers, checkpoint lists
  roads/<trk>.toml             ob.road-net      Paths<trk>.bin's road network …
  roads/<trk>.bin                               … nodes and segments
  ai/<name>.toml               ob.ai-routes     Routes<name>.bin (<name>: 4001F, 4001B, FreeRoam, All) …
  ai/<name>.bin                                 … records, control points, junctions, intersections
  markers/<name>.toml          ob.track-markers TrackPosMarkers<name>.bin (<name>: All, FreeRoam)
global/ai/
  tuning.toml                  ob.ai-tuning     the AI tuning …
  tuning.bin                                    … its one record
races/<id>.toml                ob.race          one custom race (mod packs only)
```

File names keep the original's suffix (`4001F`, `FreeRoam`, `All`) as conventions §6.2 allows for original names.
A region with no road networks (`L4RH`, `L4RR`) has no `roads/` folder; `PathsFreeRoam.bin` has no network of its own
and is fully described by `paths.toml`.

Item keys (short names as in the registry, [README.md](README.md) §3):

| Short name | Key | Item |
|---|---|---|
| `road` | `road:<REGION>/<trk>` (`road:L4RA/4001`) | one road network |
| `road-node`, `road-segment` | `road:<REGION>/<trk>#n<i>`, `…#s<i>` | a node or segment by its index (order is data: segments and nodes name each other by index) |
| `route` | `route:<REGION>/<name>` (`route:L4RA/4001F`, `route:L4RA/FreeRoam`) | one AI route file |
| `route-record` | `route:<REGION>/<name>#<id>` | a route record by its id (`+0x0A`; unique and ascending in every file) |
| `route-trigger` | `route-trigger:<REGION>/<i>` | the *i*-th trigger of the region's table (sorted by kind; first match wins) |
| `route-barrier` | `route-barrier:<REGION>/<i>` | the *i*-th barrier record (not a barrier group of [collision.md](collision.md), whose key is `barrier:<REGION>/<NAME>`) |
| `checkpoints` | `checkpoints:<REGION>/<trk>` | a track's checkpoint list (track numbers are unique in each table) |
| `marker` | `marker:<REGION>/<name>/<i>` | the *i*-th record of a markers file |
| `ai-tuning` | `ai-tuning:global` | the one AI tuning record |
| `race` | `race:<pack>:<id>` (`race:docks-remix:docks-sprint`) | a custom race; the id is unique in its pack |

## 3. Source form

### 3.1 Names or hashes (`nameref`)

Several fields hold a `bin_hash` of a name. They are **`nameref`**s ([conventions.md](conventions.md) §6.3): the
name itself when the importer knows it (`"EV_01"`, `"BARRIERS_4001"`, `"ForwardStartingPosition"`), else the hash as
`hash32` (`"0x8E55C895"`); a string starting with `0x` is a hash, any other string a name to hash (conventions §6.4).
The importer's dictionary of names is fixed per importer version: the barrier group names (`BARRIERS_<trk>`,
`PLAYER_BARRIERS_<trk>`, `BARRIERS_CAREER<n>`) for every track number, the marker names of
`crates/ob-map/src/frontend/career/routes.rs`, `PCDEMO`, the neighbourhood names (`src/frontend/career/triggers.rs:75`), `EV_nn`,
and the career's own names ([career.md](career.md)).

### 3.2 `ob.route-paths` (`world/<REGION>/routes/paths.toml`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.route-paths/1.0` |
| `region` | string | – | yes | – | The region (`L4RA`); MUST equal the unit's name. |
| `trigger` | array<table> | – | no | `[]` | Route triggers (chunk `0x3414A`) in original order, which is sorted by `kind` (RTE.4). |
| `barrier` | array<table> | – | no | `[]` | Barriers (`0x3414D`) in original order. |
| `checkpoints` | array<table> | – | no | `[]` | Checkpoint lists (`0x3414C`) in original order. |
| `extras` | table | – | no | – | conventions §9. |

**`trigger`** — one record of `0x3414A` (verified layout, `src/frontend/career/triggers.rs:44`; record size
`0x44 + 8·points`):

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `kind` | u32 | – | yes | – | `+0x00`. 0–20. Known uses: 3 car block (not named `PCDEMO`), 4 reset zone, 5 and 6 rain cover and drive-by sound zones, 7 and 20 drive-by sound zones (`src/audio/world/tunnel.rs:6`, `src/bin/game/loads/drive_in.rs:619`), 11 shortcut, 13 event marker / photo spot, 14 shop, 16 traffic zone, 17 neighbourhood, 18 engage tip, 19 cash pickup; 12 and 15 **open**. |
| `position` | vec2 | m | yes | – | `+0x04` x, y. |
| `direction` | vec2 | – | no | `[0.0, 0.0]` | `+0x0C`. Kinds 3 and 4: the heading the car is placed with (`src/ai/route/mod.rs:117`); other kinds **open** (EV_01 reads 1, 0). |
| `z_m` | f32 | m | no | `0.0` | `+0x14`. |
| `u_0x18` | u32 | – | no | `0` | **open** (0, 1 or 2). |
| `u_0x1c` | u32 | – | no | `0` | **open** (0 in every shipped record). |
| `box_min`, `box_max` | vec2 | m | yes | – | `+0x20`, `+0x28`: the ground box (edges inside). |
| `name` | nameref | – | no | `"0x00000000"` | `+0x30`: the name the kind is looked up by (all kinds except 16). |
| `gps` | array<i32>[3] | m | no | `[0, 0, 0]` | `+0x34`: the road point the GPS routes to (read with `fild`, `src/frontend/career/triggers.rs:11`). All kinds except 16. |
| `traffic` | table | – | kind 16 | – | `+0x30`: `rows` (array<u32>[3], the AI tuning's spacing row per traffic level 1–3) and `caps_zone` (u32, `+0x3C`, the car types' traffic-cap column), `src/ai/traffic/place.rs:415`. |
| `polygon` | array<vec2> | m | no | `[]` | `+0x44`: the ground polygon (even-odd); at most 255 points. Empty: the box alone. |

The point count (`+0x40`) and record size (`+0x42`) are derived; the importer checks `size = 0x44 + 8·count` (true for
all 1,036 shipped records).

**`barrier`** — one `0x3414D` record (0x24 bytes, `crates/ob-formats/src/formats/roads.rs:11`; item key `route-barrier:<REGION>/<i>`):

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `ends` | array<vec2>[2] | m | yes | – | `+0x00`, `+0x08`: the barrier's two ends. |
| `closed` | u8 | – | no | `0` | `+0x10`: set at load by the barrier switch (0x5D5020); 0 on disk in every shipped file. |
| `held` | u8 | – | no | `1` | `+0x12`: set with `closed`, never cleared (0x5D4FF0); 1 in 2,506 of 2,551 shipped records. |
| `u_0x13` | u8 | – | no | `0` | **open**. |
| `groups` | array<nameref> | – | yes | – | `+0x14` (`u32[4]`): the barrier-group names it answers to (count `+0x11`, 0–4). Slots past the count are zero (checked at import; true for all 118,729 records in the reference install's 113 Paths files); a file with other bytes there is kept whole (§5.2). |

**`checkpoints`** — one `0x3414C` record (0x110 bytes, read at 0x5D9AA0):

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `track` | u16 | – | add | – | `+0x00`: the track number. |
| `loops` | u8 | – | no | `0` | `+0x03`: the route closes on its first point (read as `!= 0`). |
| `u_0x04` | u8 | – | no | `0` | **open** (0 or 1). |
| `width_m` | u8 | m | no | `0` | `+0x05`: **inferred** a road width (20–60). |
| `u_0x06` | u16 | – | no | `0` | **open**. |
| `points` | array<vec2> | m | yes | – | `+0x08`: the checkpoints in driving order, at most 33 (count `+0x02`). |

Example (shortened; [examples/roads-ai/paths.toml](examples/roads-ai/paths.toml)):

```toml
schema = "ob.route-paths/1.0"
region = "L4RA"

[[trigger]]
kind = 4
position = [-1288.5, 806.25]
direction = [0.0, 1.0]
box_min = [-1300.0, 790.0]
box_max = [-1277.0, 822.5]
polygon = [[-1300.0, 790.0], [-1277.0, 790.0], [-1277.0, 822.5], [-1300.0, 822.5]]

[[trigger]]
kind = 16
position = [0.0, 0.0]
box_min = [-2600.0, -2200.0]
box_max = [400.0, 1900.0]
traffic = { rows = [0, 1, 2], caps_zone = 0 }

[[barrier]]
ends = [[-1021.75, 455.5], [-1009.25, 470.0]]
groups = ["BARRIERS_4001", "PLAYER_BARRIERS_4002"]

[[checkpoints]]
track = 4001
loops = 1
u_0x04 = 1
width_m = 50
points = [[-1203.5, 455.25], [-980.0, 610.75], [-702.125, 640.5], [-1180.0, 300.0]]
```

### 3.3 `ob.road-net` (`world/<REGION>/routes/roads/<trk>.toml` + `.bin`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.road-net/1.0` |
| `region` | string | – | yes | – | The region. |
| `track` | u16 | – | add | – | The track number (`Paths<trk>.bin`); MUST equal the file name. |
| `nodes` | array descriptor | – | yes | – | Layout `road_node`. |
| `segments` | array descriptor | – | yes | – | Layout `road_segment`. |
| `paths` | enum | – | no | `"region"` | Where the triggers, barriers and checkpoint lists this track's Paths file carries are: `region` (`../paths.toml`, the shared file) or `own` (`roads/<trk>.paths.toml`, same kind). Only an import of an edition whose Paths files differ within a region writes `own`. |

The arrays are the original's records byte for byte (Eq2 by identity, §6). Every byte (conventions §4.2):

`road_node` — 0x18 bytes, align 4 (`0x34148`, `crates/ob-formats/src/formats/roads.rs:4`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `f32[2]` | `at` | m | x, y |
| 0x08 | 2 | `u16` | `segment` | – | the segment holding the node |
| 0x0A | 1 | `u8` | `keep` | 0.5 m | twice the distance that keeps the node for a locator; `0xFF` not usable |
| 0x0B | 1 | `u8` | `try_links` | 0.5 m | twice the distance within which its links are tried |
| 0x0C | 6 | `i16[3]` | `links` | – | other nodes, −1 none |
| 0x12 | 2 | `u16` | `u_0x12` | – | **open**; 0 in all 18,064 shipped nodes |
| 0x14 | 4 | `f32` | `along` | m | the node's place along its road as exported |

`road_segment` — 0xDC bytes, align 4 (`0x34149`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 16 | `u8[16]` | `u_0x00` | – | **inferred** a name (`SH_Main0…`); not read |
| 0x10 | 4 | `u32` | `u_0x10` | – | **open** |
| 0x14 | 4 | `u32` | `u_0x14` | – | **open** (small integers) |
| 0x18 | 4 | `u32` | `u_0x18` | – | **open**; 0 everywhere |
| 0x1C | 2 | `u16` | `first` | – | first node |
| 0x1E | 2 | `u16` | `end` | – | end node (exclusive) |
| 0x20 | 2 | `u16` | `u_0x20` | – | **open** |
| 0x22 | 1 | `u8` | `exit_count_first` | – | connections at the first node's end (at most 4 read) |
| 0x23 | 1 | `u8` | `exit_count_last` | – | connections at the last node's end |
| 0x24 | 32 | `road_link[4]` | `exits_first` | – | slots past the count are not read (some hold old values) |
| 0x44 | 32 | `road_link[4]` | `exits_last` | – | |
| 0x64 | 4 | `f32` | `length` | m | |
| 0x68 | 4 | `u32` | `u_0x68` | – | **open** (0 or 1) |
| 0x6C | 4 | `f32` | `spline_length` | m | as exported (rebuilt for a race by 0x5D50A0) |
| 0x70 | 56 | `f32[14]` | `u_0x70` | – | **open** (positions and bounds, **inferred**) |
| 0xA8 | 4 | `u32` | `u_0xa8` | – | **open**; 0 everywhere |
| 0xAC | 12 | `u8[12]` | `u_0xac` | – | **open** (set in 12 segments) |
| 0xB8 | 4 | `u32` | `u_0xb8` | – | **open**; 0 everywhere |
| 0xBC | 2 | `i16` | `barrier_count` | – | read clamped to 0–14 |
| 0xBE | 28 | `u16[14]` | `barriers` | – | indices into the region's barriers |
| 0xDA | 2 | `u16` | `u_0xda` | – | **open**; 0 everywhere |

`road_link` — 8 bytes, align 4 (one exit at a segment's end):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 2 | `u16` | `segment` | – | the segment the exit leads to |
| 0x02 | 1 | `u8` | `direction` | – | 0 first→last, 1 back |
| 0x03 | 1 | `u8` | `u_0x03` | – | **open** |
| 0x04 | 4 | `u32` | `u_0x04` | – | **open**; 0 everywhere |

Example: [examples/roads-ai/road-4001.toml](examples/roads-ai/road-4001.toml) (the example's `.bin` is not shipped).

```toml
schema = "ob.road-net/1.0"
region = "L4RA"
track = 4001
nodes = { file = "4001.bin", offset = 0, count = 233, layout = "road_node" }
segments = { file = "4001.bin", offset = 5632, count = 41, layout = "road_segment" }
```

### 3.4 `ob.ai-routes` (`world/<REGION>/routes/ai/<name>.toml` + `.bin`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.ai-routes/1.0` |
| `region` | string | – | yes | – | The region. |
| `name` | string | – | yes | – | `<trk>F`, `<trk>B`, `FreeRoam` or `All`; MUST equal the file name. |
| `engine_reads` | bool | – | no | `true` | `false` for `All`: no reader in the original executable names `RoutesAll.bin` (it holds no such string; **inferred**) and Overbright never opens it. Kept so nothing of the original is lost. |
| `records` | array descriptor | – | yes | – | Layout `route_record`, in original order (ids ascending). |
| `points` | array descriptor | – | yes | – | Layout `route_point`: every record's control points, record after record; record *k* owns the next `point_count` points. |
| `junctions` | array descriptor | – | no | absent | Layout `route_junction`. Absent: the file has no `0x34122` chunk (four shipped files); `count = 0`: an empty one. |
| `intersections` | array descriptor | – | no | absent | Layout `route_intersection`; absent: no `0x34123` chunk (104 of 226 files). |

`route_record` — 0x8C bytes, align 4 (`0x34121` header, `crates/ob-formats/src/formats/ai_routes.rs:65`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `u_0x00` | – | **open**; 11 everywhere |
| 0x04 | 4 | `u32` | `u_0x04` | – | **open**; 11 everywhere |
| 0x08 | 2 | `u16` | `u_0x08` | – | **open**; 1 everywhere |
| 0x0A | 2 | `u16` | `id` | – | junctions and point flags name it; < 2048 (RTE.2) |
| 0x0C | 4 | `u32` | `kind` | – | 0 for the race records (0x5D9440) |
| 0x10 | 32 | `u8[32]` | `name` | – | text, `TrackRoutesA31`, NUL-terminated; bytes after the NUL are not read (old text in 70 records) |
| 0x30 | 4 | `f32` | `u_0x30` | – | **open**; 1.0 everywhere |
| 0x34 | 2 | `i16` | `point_count` | – | |
| 0x36 | 2 | `i16` | `u_0x36` | – | **inferred** a capacity; equals `point_count` in all 17,226 records |
| 0x38 | 8 | `f32[2]` | `coord_range` | m | the first and last points' race coordinates |
| 0x40 | 4 | `f32` | `u_0x40` | – | **open** |
| 0x44 | 8 | `i16[4]` | `tracks` | – | track numbers; free roam unlists a record naming 4000 (0x5E41B0) |
| 0x4C | 8 | `i16[4]` | `u_0x4c` | – | **open** (−1 mostly) |
| 0x54 | 4 | `u32` | `u_0x54` | – | **open** |
| 0x58 | 8 | `u8[8]` | `u_0x58` | – | **open** (text fragments in 12 records) |
| 0x60 | 16 | `f32[4]` | `bounds` | m | min x, min y, max x, max y |
| 0x70 | 4 | `u32` | `flags` | – | bit 6 listed, bit 10 never branched into by racers; others set at load |
| 0x74 | 2 | `u8[2]` | `fade_forward` | – | either set: lateral adjustments fade going forward |
| 0x76 | 2 | `u16` | `u_0x76` | – | **open**; 0 |
| 0x78 | 4 | `f32` | `u_0x78` | – | **open**; 999.0 everywhere |
| 0x7C | 2 | `u8[2]` | `fade_backward` | – | |
| 0x7E | 2 | `u16` | `u_0x7e` | – | **open** |
| 0x80 | 4 | `f32` | `u_0x80` | – | **open**; 999.0 in all but one record |
| 0x84 | 8 | `u8[8]` | `lanes` | – | lanes 2–9 exist (0x410CA0). Today's parser reads its lane array from 0x82, so its lanes 0 and 1 are the high bytes of `u_0x80`; nothing reads them. |

`route_point` — 0x38 bytes, align 4 (control point, `crates/ob-formats/src/formats/ai_routes.rs:1072`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `f32[2]` | `at` | m | x, y |
| 0x08 | 4 | `f32` | `coord` | m | the race coordinate |
| 0x0C | 4 | `u32` | `flags` | – | bits 0–10 the record id, bit 11 racer start point, 19–24 traffic intersection (0x3F none), 25, 27 |
| 0x10 | 2 | `u16` | `length` | 1/256 m | to the next point |
| 0x12 | 2 | `i16` | `z` | 1/8 m | |
| 0x14 | 4 | `u16[2]` | `half_width` | 1/256 m | left, right |
| 0x18 | 4 | `i16[2]` | `tangent` | 1/32767 | unit direction |
| 0x1C | 4 | `u16[2]` | `junctions` | – | `0xFFFF` none |
| 0x20 | 1 | `i8` | `bank` | 1/128 | |
| 0x21 | 1 | `i8` | `grade` | 1/128 | dz per metre |
| 0x22 | 10 | `i8[10]` | `lane_position` | 1/128 | of the wider half width; lanes 0/1 racing lines, 2–5 / 6–9 traffic |
| 0x2C | 10 | `u8[10]` | `lane_speed` | m/s | the racing lines read 100 |
| 0x36 | 2 | `u16` | `u_0x36` | – | **inferred** old bytes: 0 in 258,161 of 259,044 points |

`route_junction` — 0x20 bytes, align 4 (`0x34122`; the chunk starts with 4 bytes that are 0 in every file, checked
at import and not stored):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `u_0x00` | – | **open** |
| 0x04 | 4 | `i16[2]` | `target_point` | – | the second for type 3 |
| 0x08 | 4 | `i16[2]` | `target_record` | – | ids |
| 0x0C | 4 | `u32` | `flags` | – | type nibble; bits 18 direction, 20 not for racers, 23 flips, 26 turn, 27 one-in-three branch |
| 0x10 | 4 | `i32` | `u_0x10` | – | **open** (−1 or 0) |
| 0x14 | 4 | `u32` | `u_0x14` | – | **open**; 0 everywhere |
| 0x18 | 8 | `i16[4]` | `u_0x18` | – | **open** |

`route_intersection` — 0x114 bytes, align 4 (`0x34123`, `crates/ob-formats/src/formats/ai_routes.rs:139`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `kind` | – | 1 no stop, 2 road 1 stops, 3 every road stops |
| 0x04 | 4 | `i16[2]` | `roads` | – | record ids |
| 0x08 | 2 | `i16` | `u_0x08` | – | run-time state; −1 on disk |
| 0x0A | 4 | `i16[2]` | `points` | – | each road's point at the crossing |
| 0x0E | 2 | `u16` | `u_0x0e` | – | 0 on disk |
| 0x10 | 4 | `i32` | `u_0x10` | – | run-time right of way; −1 on disk |
| 0x14 | 256 | `u8[256]` | `slots` | – | 16 run-time slots; every shipped slot holds `00000000 FFFFFFFF −1.0f −1.0f` |

In the `.bin`, each array starts at a multiple of 256; gaps are zero. Example
([examples/roads-ai/ai-4001F.toml](examples/roads-ai/ai-4001F.toml)):

```toml
schema = "ob.ai-routes/1.0"
region = "L4RA"
name = "4001F"
records = { file = "4001F.bin", offset = 0, count = 412, layout = "route_record" }
points = { file = "4001F.bin", offset = 57856, count = 6120, layout = "route_point" }
junctions = { file = "4001F.bin", offset = 400640, count = 1210, layout = "route_junction" }
intersections = { file = "4001F.bin", offset = 439552, count = 38, layout = "route_intersection" }
```

### 3.5 `ob.track-markers` (`world/<REGION>/routes/markers/<name>.toml`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.track-markers/1.0` |
| `region` | string | – | yes | – | The region. |
| `name` | string | – | yes | – | `All` or `FreeRoam`; MUST equal the file name. |
| `marker` | array<table> | – | no | `[]` | The records in original order. |

`marker` (0x30 bytes, `crates/ob-map/src/frontend/career/routes.rs:5`):

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `name` | nameref | – | yes | – | `+0x08`: `ForwardStartingPosition`, `ForwardStartLine`, `ForwardFinishLine`, the `Backward…` set, `SplitTimeBoundary1–3`, shop names in free roam. |
| `track` | u16 | – | add | – | `+0x24`. |
| `slot` | u32 | – | no | `0` | `+0x0C`: a starting position's grid slot 0–7 (0x5EB5E0). |
| `position` | vec3 | m | yes | – | `+0x10`. The grid is put on the ground below it at load. |
| `heading_bam16` | u16 | bam16 | no | `0` | `+0x20`: around +z from +x. |
| `u_0x22` | u16 | – | no | `0` | **inferred** old bytes (text in 96 records). |
| `u_0x28` | bytes | – | no | 8 zero bytes | **inferred** old bytes (text in 68 records); exactly 8 bytes. |

Constants checked at import: `+0x00` and `+0x04` u32 11, `+0x1C` u32 0, `+0x26` u16 0; the chunk body starts with
8 bytes of `0x11` (alignment fill). A file that breaks one is kept whole in the residual (§5.3), as route files are
(§5.1).

```toml
schema = "ob.track-markers/1.0"
region = "L4RA"
name = "All"

[[marker]]
name = "ForwardStartingPosition"
track = 4001
slot = 0
position = [-1190.25, 461.5, 13.5625]
heading_bam16 = 16384
```

### 3.6 `ob.ai-tuning` (`global/ai/tuning.toml` + `.bin`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.ai-tuning/1.0` |
| `tuning` | array descriptor | – | yes | – | Layout `ai_tuning`, count 1 (`0x34B00`, kept at 0x81CB60). |

`ai_tuning` — 0xAE0 bytes, align 4 (`src/ai/route/mod.rs:263`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x000 | 0x370 | `ai_curve_point[110]` | `curves` | – | ten catch-up curves of 11 points each; curve *k* starts at 0x58·*k* (`src/ai/route/target_speed.rs:177`) |
| 0x370 | 0x1B4 | `u8[436]` | `u_0x370` | – | **open** |
| 0x524 | 0x410 | `ai_skill_row[10]` | `skill` | – | by skill row 0–9 |
| 0x934 | 0x18 | `ai_block_row[3]` | `block` | – | by block row 0–2 |
| 0x94C | 0x30 | `f32[12]` | `catch_up_scale` | – | by difficulty and class: element 4·difficulty + class |
| 0x97C | 0x78 | `u8[120]` | `u_0x97c` | – | **open** |
| 0x9F4 | 0x78 | `f32[30]` | `random_rows` | – | cumulative odds, ten per difficulty: element 10·difficulty + *i* |
| 0xA6C | 0x20 | `u8[32]` | `u_0xa6c` | – | **open** |
| 0xA8C | 0x48 | `f32[18]` | `traffic_spacing_m` | m | per zone row (9 rows): min, max |
| 0xAD4 | 4 | `f32` | `jitter_period_s` | s | how often the lateral offset is drawn again |
| 0xAD8 | 4 | `f32` | `catch_up_period_s` | s | how often the catch-up is refreshed |
| 0xADC | 4 | `u32` | `u_0xadc` | – | **open** |

`ai_curve_point` — 8 bytes, align 4:

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `f32` | `x` | – | the curve's input (for the catch-up curves, the gap) |
| 0x04 | 4 | `f32` | `y` | – | the value at `x` |

`ai_skill_row` — 0x68 bytes, align 4 (row *r* at 0x524 + 0x68·*r*):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 12 | `f32[3]` | `junction_odds` | – | |
| 0x0C | 4 | `f32` | `pass_odds` | – | |
| 0x10 | 4 | `u32` | `u_0x10` | – | **open** |
| 0x14 | 4 | `f32` | `speed_factor` | – | the route speed factor |
| 0x18 | 20 | `u8[20]` | `u_0x18` | – | **open** |
| 0x2C | 8 | `f32[2]` | `shift_down` | – | drag: min, max of the random down-shift offset |
| 0x34 | 8 | `f32[2]` | `shift_up` | – | drag: the same for up-shifts |
| 0x3C | 8 | `f32[2]` | `shift_second` | – | drag: the same in 2nd gear |
| 0x44 | 4 | `f32` | `nitrous_gap_s` | s | least time between two shots |
| 0x48 | 4 | `f32` | `last_lap_gap_s` | s | the same on the last lap |
| 0x4C | 4 | `f32` | `nitrous_min_s` | s | fewest seconds of nitrous a shot starts with |
| 0x50 | 4 | `f32` | `longest_shot_s` | s | |
| 0x54 | 8 | `u8[8]` | `u_0x54` | – | **open** |
| 0x5C | 4 | `f32` | `catch_up_hold_s` | s | race time before the catch-up starts |
| 0x60 | 4 | `f32` | `jitter_m` | m | size of the lateral offset |
| 0x64 | 1 | `u8` | `nitrous_gear` | – | drag: the lowest gear a shot fires in, minus 1 |
| 0x65 | 1 | `u8` | `keeps_reserve` | – | non-zero: no shot with under 1 s in the tank before the end |
| 0x66 | 2 | `u8[2]` | `u_0x66` | – | **open** |

`ai_block_row` — 8 bytes, align 4 (row *r* at 0x934 + 8·*r*):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `i32` | `priority` | – | |
| 0x04 | 1 | `u8` | `bumper` | – | |
| 0x05 | 3 | `u8[3]` | `u_0x05` | – | **open** |

```toml
schema = "ob.ai-tuning/1.0"
tuning = { file = "tuning.bin", offset = 0, count = 1, layout = "ai_tuning" }
```

### 3.7 `ob.race` (`races/<id>.toml`, mod packs)

A custom race runs on a **base track**: the track's road network (`road:<REGION>/<trk>`), its race barriers
(`BARRIERS_<trk>`, `PLAYER_BARRIERS_<trk>`), AI lanes and traffic, and the set-up of a career event of the same mode
on that track. The race file gives the rest. The fields map 1:1 to today's map-mod `[race]` section
(`crates/ob-map/src/tools/map/format.rs:141`, `:439`), so converting a `map.obmap` is mechanical (§5.5).

| Field | Type | Unit | Req. | Default | Meaning (`map.obmap` key) |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.race/1.0` |
| `op` | enum | – | no | `add` with `id`, else `merge` | `merge` or `add` (layering.md §3.1). |
| `key` | string | – | merge | – | In a merge only: the race it changes, `<pack>:<id>` of a pack this one lists in `[dependencies]` (`ORD.2`). |
| `disabled` | bool | – | no | `false` | The race is not offered (layering.md §3.1). |
| `id` | string | – | add | – | `id`: `[a-z0-9_-]{1,40}`; MUST equal the file name. Key `race:<pack>:<id>`. In an addition only. |
| `name` | string | – | no | the id | `name`: shown in lists; at most 200 characters, no control characters. |
| `track` | u16 | – | add | – | `track`: the base track's number (`track:<n>`, [career.md](career.md)). |
| `backward` | bool | – | no | `false` | `backward`: use the base track's backward barriers and lanes (`Routes<trk>B`). |
| `mode` | enum | – | add | – | `mode`: `circuit`, `sprint`, `streetx`, `drift`. Circuit and street X loop. |
| `laps` | u32 | – | no | 2 looping, 1 otherwise | `laps`: 1–20; exactly 1 for `sprint` and `drift`. |
| `checkpoints` | array<vec2> | m | no | `[]` | `checkpoint` (repeated): world x, y in driving order; at most 64. Fewer than 2: kept, not raceable. A loop closes from the last back to the first. |
| `grid` | array<table> | – | no | `[]` | `grid` (repeated): spots, pole first, at most 8. Empty: 8 slots behind the start, rows of two, 7 m between rows, 4 m across (`crates/ob-map/src/tools/map/race.rs:59`). |
| `start` | table | – | no | from the first two checkpoints | `start`: the start line spot. |
| `finish` | table | – | no | loop: the start; else the last checkpoint | `finish`: the finish line spot. |
| `extras` | table | – | no | – | conventions §9 (e.g. the editor's notes). |

A pack adds its own races (an entry with `id` and no `op` is an addition, layering.md §3.1). A pack that lists the
race's pack in `[dependencies]` may merge into it (`key`, and only the fields it changes; the rest keep the lower
layer's values) or disable it (`disabled = true`); its `grid` merges slot by slot (layering.md §3.2: every slot of
a pack's own race is that pack's addition, keyed `index = "<pack>:<n>"`; `op = "add"` appends one, `disabled`
hides one). "add" in the Req. column: required in an addition; "merge": required in a merge. The field table is
`crates/ob-asset-format/src/kinds.rs`, which also gives `schemas/race.schema.json` and Studio's inspector rows.

A **spot** is `{ position = vec3 (m), heading_deg = f32 }`: heading in degrees around +z from +x (the map format's
fourth number). Every coordinate and heading MUST be finite and within ±100,000 (`MAX_COORD`). Heights of grid slots
and markers are put on the ground at load, as the game does with its own markers.

**Race digest.** The *race digest* is the `xxh3-128` of this UTF-8 text, built from the fields that change how the
race drives (the same set the map editor's fingerprint takes, `validate::fingerprint` in `crates/ob-studio-gui/src/editor/validate.rs`; the name is not part of
it): a first line `ob.race-digest/1`, then `track=<n>`, `backward=<true|false>`, `mode=<mode>`, `laps=<n>`, one
`checkpoint=<x> <y>` per checkpoint, one `grid=<x> <y> <z> <h>` per grid slot, `start=<x> <y> <z> <h>` and
`finish=<x> <y> <z> <h>` when present; each line ends in LF; floats in the shortest form that round-trips as binary32
(conventions §3.4); defaults are applied (`laps`, `backward`) but the default grid, start and finish are not
expanded. Studio's **tested** mark ([manifest.md](manifest.md) §3, `[[tested]]`) stores `race_digest`: this race
digest followed by the content digests of the pack's play-affecting files, hashed together. The mark holds only while
that value is unchanged, so an edit that changes how the race drives, or any play-affecting file of the pack, clears
it. The race's `name` is not part of the race digest (the map editor's `tested.txt` rule).

```toml
schema = "ob.race/1.0"
id = "docks-sprint"
name = "Docks sprint"
track = 4001
mode = "sprint"
checkpoints = [[-1203.5, 455.25], [-980.0, 610.75], [-702.125, 640.5]]
start = { position = [-1210.0, 450.0, 13.5], heading_deg = 33.5 }
finish = { position = [-700.0, 641.0, 12.0], heading_deg = 2.0 }
grid = [
  { position = [-1214.0, 447.5, 13.5], heading_deg = 33.5 },
  { position = [-1211.75, 443.0, 13.5], heading_deg = 33.5 },
]
```

### 3.8 Limits

| What | Limit |
|---|---|
| Triggers per region / polygon points per trigger | 65,535 / 255 |
| Barriers per region; groups per barrier | 65,535; 4 |
| Checkpoint lists per region; points per list | 4,096; 33 |
| Route records per file; points per record; record id | 4,096; 32,767; < 2,048 |
| Junctions per file | 65,534 (slot `0xFFFF` means none) |
| Intersections per file | 63 (a point's 6-bit index; `0x3F` means none) |
| Nodes and segments per network | 65,535 |
| Markers per file | 65,536 |
| Races per pack; checkpoints; grid slots; laps | 256; 64; 8; 20 |

## 4. Layering

| Kind | merge | add | replace | disable | Notes |
|---|---|---|---|---|---|
| `ob.route-paths`, `ob.road-net`, `ob.ai-routes`, `ob.track-markers` | MRG.3 | MRG.3 | MRG.3 | MRG.3 | Read-only in 1.0: what lanes, junctions and barriers may change without breaking the AI is not known until the original's AI route choice is traced. A mod pack with any file under `world/<REGION>/routes/` is refused. |
| `ob.ai-tuning` | MRG.3 | MRG.3 | MRG.3 | MRG.3 | Read-only in 1.0 (the opaque ranges of `ai_tuning` cannot be merged field by field yet). |
| `ob.race` | MRG.3 | yes (implicit) | MRG.3 | own races only | A race file defines a new item in its pack's own namespace; `op` is absent or `"add"`. A disabled race is kept but not offered. Races of other packs cannot be changed in 1.0. |

- **No clashes are possible** between races: keys carry the pack id. Two packs' races may run on the same base track.
- **Wildcards**: none.
- **Mods off**: no race of any pack is listed; Quick Race and the career show the native lists.
- **Extension point (routes).** Once the original's AI route choice is traced, a minor version adds the kind
  `ob.route-edits` (`world/<REGION>/routes/edits.toml`, name reserved) with keyed lists: barrier records keyed
  `route-barrier:<REGION>/<i>` (merge of `groups`; disable) and added barriers keyed by pack; added route records keyed
  `route:<REGION>/<name>#<pack>:<id>` with their own points, and added junctions joining them to original records
  by `(record id, point index)` with guards (the original point's position); route triggers keyed by index (merge,
  disable) and added triggers keyed by pack (event markers for pack career events, [career.md](career.md)). Added
  items go after the originals (layering §3.3), so original record ids, junction slots and trigger order keep their
  meaning. `ob.race` 1.1 may then add `close_groups` / `open_groups` (barrier groups switched for the race) and
  `network` (another track's network). Until then these names are reserved and a reader refuses them (CONV.4).
- **Extension point (AI tuning).** A minor version may add named TOML fields for the known parts of `ai_tuning`
  (skill rows, block rows, catch-up scales, traffic spacing); a pack then merges those fields, and the `.bin`
  record stays the base. Until then the field names are reserved.

## 5. Import

### 5.1 Route files

Per `Routes<name>.bin`: walk the chunks in order (`0x34121`, `0x34122`, `0x34123`, always in this order in known
editions); copy each record's 0x8C header into `records` and its points into `points`; copy the junctions after the
4-byte zero prefix (checked) and the intersections; write `ai/<name>.toml`. A cut-off record, a chunk of another id,
a different chunk order or a junction prefix that is not 0 sends the whole file to the residual as `kept` (the
import's edition becomes `custom`). `0x34124` (no shipped file has it) is kept the same way.

### 5.2 Paths files

Per region:

1. Read every `Paths*.bin`. The container `0x80034147` holds, in this order, `0x34148`, `0x34149`, `0x3414C`,
   `0x3414A`, `0x3414D` (track files) or `0x3414A`, `0x3414D` (`PathsFreeRoam.bin`).
2. **Shared chunks.** In known editions `0x3414A` and `0x3414D` are byte-identical in every Paths file of a region
   (61 files in `L4RA`), and the *read part* of `0x3414C` (each record's first 8 bytes and its `count` points) is
   identical in every track file of a region, while the unread rest of each record differs from file to file. The
   importer writes one `paths.toml` from `PathsFreeRoam.bin` (`0x3414A`, `0x3414D`) and the first track file in
   byte order of the file names (`Paths4001.bin` in `L4RA`; `0x3414C`'s read part), then checks every other file
   against it. Every file's residual map names the same items (shared items, [residual.md](residual.md) §3.1.1;
   [import.md](import.md) §4). A file that differs gets its own `roads/<trk>.paths.toml` and `paths = "own"`; a file
   whose barrier records hold non-zero bytes in unused group slots is kept whole.
3. **Networks.** `0x34148` and `0x34149` of each track file are copied into `roads/<trk>.bin`.
4. **Names.** Hashes are resolved with the dictionary of §3.1; unknown ones stay hashes.

Residual nodes per Paths file: `0x34148`, `0x34149`, `0x3414A`, `0x3414D` decoded with proof `bytes`; `0x3414C`
decoded with proof `values`, its bytes kept (shadow, §6).

### 5.3 Markers

`0x34146`: the 8-byte `0x11` fill, then records of 0x30 bytes to the chunk's end; each becomes a `marker` in order.
A file whose fill, record size or constants (§3.5) differ is kept whole in the residual (the import's edition becomes
`custom`).

### 5.4 AI tuning

From the unpacked `GLOBAL/GlobalB.lzc`: `0x34B00` (first 0xAE0 bytes; a longer body sends the rest to the residual)
into `tuning`. `GLOBALB.BUN` is byte-identical to the unpacked `GlobalB.lzc` in known editions; its residual map
points at the same items.

### 5.5 Legacy map-mod races

Today's map mods are mounted as legacy packs (layering §7); each `[race]` section becomes `races/<id>.toml`: `id`,
`name`, `track`, `backward` (`yes`/`no` → bool), `mode`, `laps` as they are; each `checkpoint = x y` line an element of
`checkpoints` in line order; each `grid = x y z h` line an element of `grid`; `start` / `finish` likewise. Numbers are
already written in shortest round-trip form by today's writer, so they convert bit for bit. The editor's
`tested.txt` (`<id> <fnv64>`) is not converted: its fingerprint is not the race digest; a converted race starts
untested. The conversion test reads both forms and compares the `CustomRace` values.

## 6. Equality and verification

| Kind | Level | Proof at import | Shadow |
|---|---|---|---|
| `ob.ai-routes` | **Eq2** | the encoder interleaves records and points, re-adds the junction prefix and the chunk headers: the original file exactly | none |
| `ob.road-net` | **Eq2** | identity on `0x34148` / `0x34149` | none |
| `ob.route-paths` triggers, barriers | **Eq2** | every byte is a field, a count or size derived from one, or a checked constant (unused barrier-group slots: zero); the encoder writes the chunk from `paths.toml` and compares with each file's chunk | none |
| `ob.route-paths` checkpoint lists | **Eq1** | the read part equals; the unread rest of the 0x110-byte records differs per file | `0x3414C` of every track file: 1,106,224 bytes (≈ 20 KB deflated) |
| `ob.track-markers` | **Eq2** | every byte is a field or a checked constant | none |
| `ob.ai-tuning` | **Eq2** | identity | none |
| `ob.race` | – | no original data; the legacy conversion is tested lossless | – |

**Eq1 model** (what the engine consumes, hashed with the cache's flat encoding): `RoadNet` (`crates/ob-formats/src/formats/roads.rs:42`,
including `repaired`), `RouteFile` (`crates/ob-formats/src/formats/ai_routes.rs:147`, including `record_tracks`), the trigger sets
(`RouteTriggers`, `ResetZone`, `TrafficZone`), the barriers, each track's checkpoint list and loop flag, the marker
lists and `AiTuning`. Because every route chunk except `0x3414C` is Eq2, a transitional reader can rebuild the
original chunk and run today's parsers on it; Eq1 is still computed and compared for every file. Post-load products (the free-roam pruning per stage, `src/bin/game/loads/cache.rs:52`; the
racing speeds; the point grid) are computed from the model and are not stored.

**Readers that open route files themselves today** and must take the model instead (parity.md §2): the AI
routes' load (`src/ai/route/mod.rs:202`, a `std::fs::read` after `game_fs::path`); the route triggers' load, which
takes the first `Paths*` file of `ROUTESL4RA` in directory order (`src/frontend/career/triggers.rs:102`, harmless only
because every Paths file of a region carries the same triggers); the light-probe field, which lists the route folder
(`src/render/enh/lighting/probes/field.rs:255`); the free-roam reset zones, which join the root without `game_fs`
(`src/bin/game/input/dispatch.rs:403`); `load_route` and `load_markers` (`crates/ob-map/src/frontend/career/routes.rs:23`, `:85`);
the traffic load (`src/bin/game/sim/traffic/mod.rs:144`, `:147`); the free-roam route memo
(`src/bin/game/loads/cache.rs:64`, `:68`); the custom races' network (`crates/ob-map/src/tools/map/race.rs:16`); the read-ahead
list (`src/bin/game/loads/read_ahead.rs:36`), which becomes a bundle read-ahead.

Tests: Eq1 and Eq2 over all 355 route files and the AI chunks of the reference install; the region-dedupe check
(§5.2) as a test that fails if an edition breaks it; a property test that `ob.race` written and read again gives the
same `CustomRace` and the same race digest; the legacy conversion test (§5.5).

## 7. Cooked form

- **Cook units**: `world/<REGION>` for everything under `routes/`; `global` for `ob.ai-tuning`. Races are part of
  the unit of their base track's region (they are small and need its network).
- **Model sections**: `routes.paths` (triggers by kind, barriers, checkpoint lists), `routes.road.<trk>`,
  `routes.ai.<name>`, `routes.markers.<name>`, `races` (the merged custom races of the stack); `ai.tuning`.
- **Zero-copy**: the `.bin` arrays (records, points, junctions, intersections, nodes, segments, the tuning record) are
  stored entries of the region's bundle and are referenced, not copied (cooked.md §5.1). A route file is decoded when a
  race or free roam asks for it, as today.
- **Prepared, not normative**: the free-roam pruned network per career stage, the racing-speed tables, the
  `PointGrid`, the junction cache.

## 8. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| RTE.1 | The points array does not hold exactly the sum of the records' `point_count`s, or an array descriptor's layout does not match its field. |
| RTE.2 | Route record ids not unique, not ascending, or ≥ 2048. |
| RTE.3 | A `.bin` array whose bytes break a checked constant (junction prefix, marker constants) when rebuilt. |
| RTE.4 | Triggers not sorted by `kind`, or a `kind` above 20. |
| RTE.5 | A kind-16 trigger without `traffic`, or `traffic` on another kind; `name`/`gps` together with `traffic`. |
| RTE.6 | A barrier with more than 4 groups; a checkpoint list with more than 33 points or a track number twice in one table. |
| RTE.7 | A `nameref` that starts with `0x` but is not a `hash32` (8 upper-case hex digits), or a name that is not a valid game name ([conventions.md](conventions.md) §6.3). |
| RTE.8 | A race `id` that breaks `[a-z0-9_-]{1,40}` or differs from its file name. |
| RTE.9 | `laps` outside 1–20, or not 1 for a sprint or drift race. |
| RTE.10 | More than 64 checkpoints or more than 8 grid slots. |
| RTE.11 | A race coordinate or heading not finite or beyond ±100,000. |
| RTE.12 | A race `name` over 200 characters or with a control character. |
| RTE.13 | (warning) A race that cannot be raced: fewer than 2 checkpoints, a track number with no road network, a checkpoint leg with no way through the open network, or no career event of its mode on the base track. It is kept and shown with the reason, but not offered. |
| RTE.14 | A `name` / `region` / `track` field that does not match the file's path. |
<!-- /generated -->

MRG.3 covers the refused operations of §4.

## 9. Versioning and extension

- 1.x minors may add optional fields with defaults: named meanings for `u_*` bytes (the bytes stay where they are),
  `ob.race` 1.1's barrier and network fields (§4), a `drag` mode once drag lanes are traced.
- Reserved: the kind `ob.route-edits`, the file name `routes/edits.toml`, the AI tuning's field names (§4), the race
  fields `close_groups`, `open_groups`, `network`.
- `[ext.<NAME>]` tables are allowed in every file of this page (conventions §9).

## 10. Open points

1. Trigger kinds 12 and 15, what the sound and rain zones (5, 6, 7, 20) read beyond box and polygon, the
   `direction` of kinds other than 3 and 4; `u_0x18`.
2. Most of `road_segment` (`u_0x00`–`u_0x20`, `u_0x70`, `u_0xac`) and the road nodes' `u_0x12`.
3. Route record fields `u_0x00`–`u_0x08`, `u_0x40`, `u_0x4c`–`u_0x58`, the 999.0 pair; junction `u_0x00`, `u_0x10`,
   `u_0x18`.
4. Checkpoint list `u_0x04` and the meaning of `width_m`.
5. `RoutesAll.bin` is not read by any code found; if a reader turns up, `engine_reads` changes for that edition only.
6. The ranges `u_0x370`, `u_0x97c`, `u_0xa6c`, `u_0xadc` of `ai_tuning` and the open bytes of its rows.

## 11. Sizes and timing

| | Original (reference install) | Base set |
|---|---|---|
| Route files (`Routes*`) | 226 files, 20,136,336 bytes | 226 TOML + `.bin`, ≈ 20.1 MB (headers dropped, arrays 256-aligned) |
| Road networks (`0x34148`, `0x34149`) | 105 copies, 1,076,596 bytes | 105 TOML + `.bin`, ≈ 1.1 MB |
| Triggers, barriers (`0x3414A`, `0x3414D`) | 113 copies each, 8,706,356 bytes | one per region: 1,036 triggers and 2,551 barriers, ≈ 0.6 MB of TOML [guess] |
| Checkpoint lists (`0x3414C`) | 105 copies, 1,106,224 bytes | one per region, ≈ 40 KB of TOML [guess]; shadow 1.1 MB (≈ 20 KB deflated) |
| Markers | 16 files, 101,728 bytes (2,114 records) | ≈ 0.3 MB of TOML [guess] |
| AI tuning (GlobalB `0x34B00`) | 2,784 bytes | 2,784 bytes `.bin` + a small TOML |
| Total | 31.1 MB (+ 2.7 KB) | ≈ 22.3 MB stored (the shared chunks are stored once) |

Import: reading 31 MB and writing ≈ 22 MB, all Eq2 checks included, well under a second per region on one core
[guess]. Custom races: a few kilobytes each.
