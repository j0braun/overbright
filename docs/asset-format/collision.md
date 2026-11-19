# Asset format: collision (per-stream-section collision packs, barrier groups, prop volumes)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md), [layering.md](layering.md) and [residual.md](residual.md) apply and are not repeated.
Schemas: `schemas/collision.schema.json`, `schemas/collision-shared.schema.json`. Examples: `examples/collision/`.
Terms (region, stream section, scenery section, detail limit, placement) as in [conventions.md](conventions.md) §2
and [world.md](world.md).

The original's static collision is a **face soup per stream section**: quads and triangles in quantised
coordinates, a grid of 32 m cells listing them with 8 m sub-cell masks, floors before walls. Nothing ties a face to
the placement it was built from (docs/formats/cars-1.md "Collision"). This page keeps that data exactly as the
engine reads it, in its own binary layouts, and reserves the shape of a later collision delta keyed by placement.

## 1. Scope

| Original | What | Where |
|---|---|---|
| `STREAM<REGION>.BUN` `0x80034130`, one per stream section (L4RA 255; all regions 599) | collision pack: header `0x34131`, faces `0x34134`, sub-cell references `0x34136`, grid cells `0x34137` | `collision/<n>.toml` + `collision/<n>.bin` |
| `0x80034130` / `0x34135` (u16s, named in docs/formats/cars-1.md) | absent from every shipped pack; another edition's would be kept | residual |
| `<REGION>.BUN` `0x0003412F` | barrier groups (L4RA 126 entries) | `collision/shared.toml` |
| `<REGION>.BUN` `0x80034020` (`0x39200`, `0x39201`, `0x39202` per volume) | the knock-over props' collision volumes (L4RA 89) | `collision/shared.toml` |

**Not here:** the cars' own collision volumes (`0x80034020` in `CARS/*/GEOMETRY.BIN`): no kind owns them yet; they
are imported byte for byte into the residual (kept) until a page defines them ([README.md](README.md) §3). The
behaviour (the height query 0x5E72F0, the polygon gather 0x5E0D40, barrier switching, contacts): engine code,
documented in docs/formats/cars-1.md "Collision" and "Contacts" (`src/ground.rs`, `src/collision.rs`). The engine's
own 16 m triangle grid (`collision::Collision`, built from these faces, or from render triangles for a region
without packs) is derived data (§7).

## 2. Files and keys

```text
world/<REGION>/collision/
  <n>.toml        ob.collision/1.0          one per stream section that has a pack
  <n>.bin                                   its faces, references and cells
  shared.toml     ob.collision-shared/1.0   barrier groups, prop volumes
```

`<n>` is the stream section's number. The pack header's own number (+0x00) equals it in all 599 packs (checked);
a pack whose header names another number is an import error (`COL.1`).

| Item | Key | Reference form | Example |
|---|---|---|---|
| collision pack | stream section number | `collision:<REGION>/<n>` | `collision:L4RA/1427` |
| face | (pack, index) | `collision-face:<REGION>/<n>/<i>` | `collision-face:L4RA/1427/12` |
| barrier group | its name; its **index** is what faces store | `barrier:<REGION>/<NAME>` | `barrier:L4RA/BARRIERS_4001` |
| prop volume | its name | `volume:<REGION>/<NAME>` | `volume:L4RA/ZCS_PylonA` |

**Which drivable stream section a pack serves** (0x5D3C20, `src/ground.rs:192`): pack `n` serves drivable stream
section `n − limit` when `limit ≤ n mod 100 < 2·limit` (`limit` = `region.toml` `detail_limit`), otherwise stream
section `n`. So `collision:L4RA/1427` serves drivable stream section 1407. This is a rule of the engine, not stored.

## 3. Source form

### 3.1 `collision/<n>.toml` (`ob.collision/1.0`)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.collision/1.0` |
| `section` | u16 | – | yes | – | Header +0x00; MUST equal the file name and a stream section of `region.toml` (`COL.1`). |
| `width`, `height` | u16 | 32 m cells | yes | – | The grid (header +0x04, +0x06). |
| `origin` | vec2 | m | yes | – | The grid's corner (header +0x08). |
| `bounds_min`, `bounds_max` | vec3 | m | yes | – | Header +0x10 / +0x20. Not read by Overbright. |
| `faces` | array descriptor, layout `collision_face` | – | yes | – | `0x34134`, in order. |
| `refs` | array descriptor, layout `collision_ref` | – | yes | – | `0x34136`, in order. |
| `cells` | array descriptor, layout `collision_cell` | – | yes | – | `0x34137`, row-major, `width × height` (`COL.2`). |
| `extras` | table | – | no | – | |

The three arrays are external only (no inline form): they are large (up to 15,696 faces in one pack) and are the
engine's input as they are. Writers place them in `<n>.bin` in the order faces, refs, cells, each at a multiple of
256 (conventions §4.3).

Layout `collision_header` (0x30 bytes, alignment 16; the encoder's view of the fields above):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 2 | `u16` | `section` | – | |
| 0x02 | 2 | `u16` | `pad` | – | Zero in all 599. |
| 0x04 | 2 | `u16` | `width` | cell | |
| 0x06 | 2 | `u16` | `height` | cell | |
| 0x08 | 8 | `f32[2]` | `origin` | m | |
| 0x10 | 12 | `f32[3]` | `bounds_min` | m | |
| 0x1C | 4 | `u32` | `pad` | – | |
| 0x20 | 12 | `f32[3]` | `bounds_max` | m | |
| 0x2C | 4 | `u32` | `pad` | – | |

Layout `collision_face` (0x20 bytes, alignment 2; decoded by 0x5D7F20 and 0x5D2CD0; `crates/ob-formats/src/formats/collision.rs`):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 1 | `u8` | `group` | – | Barrier group index into `shared.toml`'s `barrier_group` list; 0 = always present (`COL.6`). |
| 0x01 | 1 | `u8` | `bounds` | – | 2 bits each, low first: the corners giving x-min, y-min, x-max, y-max (compared in 1/65536 m, 0x5E7500). |
| 0x02 | 1 | `u8` | `surface` | – | Surface id (polygon +0x0B): the road surface for grip and sound (ids 0–28 seen; L4RA uses 16). |
| 0x03 | 1 | `u8` | `flags` | – | Below. |
| 0x04 | 2 | `i16` | `base_m` | m | Base height. |
| 0x06 | 2 | `u16` | `cache_slot` | – | The game's cache slot at run time; `0xFFFF` on disk in all 417,572 faces; MUST be `0xFFFF` (`COL.5`). |
| 0x08 | 8 | `i16[4]` | `x` | 1/8 m | Corners 0–3. |
| 0x10 | 8 | `i16[4]` | `y` | 1/8 m | |
| 0x18 | 8 | `i16[4]` | `z` | 1/256 m | Above `base_m`. |

A corner is `(x/8, y/8, base_m + z/256)` in binary32 as `crates/ob-formats/src/formats/collision.rs:68` computes it; with flag 0x04
corners 0 and 3 are at z = −2048 and corners 1 and 2 at +2048.

Face flags (byte 3; docs/formats/cars-1.md "Collision", **verified** unless marked):

| Bit | Meaning |
|---|---|
| 0x01 | A floor on one of several levels (crossing roads): such floors compete by height in the height query. |
| 0x02 | Wall. |
| 0x04 | Wall reaching ±2048 m. |
| 0x08 | Not ground for the wheels; ends a cell's floor run like a wall. |
| 0x10 | Four corners (else three). |
| 0x40 | Not kept among the car's last four floors, and not taken at once within 4 m (height query). |
| 0x20, 0x80 | **open**; on no face. |

Combinations in L4RA: 0x00, 0x01, 0x08, 0x10, 0x11, 0x12, 0x16, 0x18, 0x41, 0x51.

Layout `collision_ref` (4 bytes, alignment 2):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 2 | `u16` | `face` | – | Index into `faces` (`COL.3`). |
| 0x02 | 2 | `u16` | `mask` | – | The 4×4 eight-metre sub-cells of the cell the face touches: bit `sy·4 + sx` (0x5E72F0). Non-zero in all 620,291 references. |

Layout `collision_cell` (4 bytes, alignment 2):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 2 | `u16` | `first` | – | First reference of the cell. |
| 0x02 | 2 | `u16` | `count` | – | Its references. |

**Invariants** a reader checks (each holds in all 37,671 cells of the reference install): `cells` has `width ×
height` entries (`COL.2`); every cell's range lies inside `refs` and every reference's face inside `faces`
(`COL.3`); within a cell, every face without flags 0x02 / 0x08 comes before the first face with one of them
(`COL.4`): the height query stops at the first wall or 0x08 face, so a cell out of this order would lose floors.

Example (`world/L4RA/collision/1427.toml`, made-up values):

```toml
schema = "ob.collision/1.0"
section = 1427
width = 4
height = 3
origin = [-288.0, -1120.0]
bounds_min = [-288.0, -1120.0, -127.0]
bounds_max = [-160.0, -1024.0, 131.5]
faces = { file = "1427.bin", offset = 0, count = 745, layout = "collision_face", digest = "xxh3-128:0f1e2d3c4b5a69788796a5b4c3d2e1f0" }
refs = { file = "1427.bin", offset = 24064, count = 1102, layout = "collision_ref" }
cells = { file = "1427.bin", offset = 28672, count = 12, layout = "collision_cell" }
```

### 3.2 `collision/shared.toml` (`ob.collision-shared/1.0`)

| Field | Type | Req. | Meaning |
|---|---|---|---|
| `schema` | string | yes | `ob.collision-shared/1.0` |
| `barrier_group` | array of tables | yes | `0x3412F` in order: the index is the face `group` byte (≤ 256 entries; `COL.8`). |
| `volume` | array of tables | yes | `0x80034020` volumes in order (values level, §6). |
| `extras` | table | no | |

Layout `barrier_group` (32 bytes; registered by 0x5E78B0; always TOML):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `hash32` | `hash` | – | The name's hash, by which 0x5D3C70 finds a group (entries 1..). Stored, not derived: entry 0 (`NONE`) holds another value in every region. |
| 0x04 | 4 | `u32` | `u_0x04` | – | **open**. |
| 0x08 | 1 | `u8` | `on` | – | 1 in every entry on disk; the region loader sets it to (index == 0) once the bundle is in (0x5D8360), and races switch their own on. |
| 0x09 | 23 | `char[23]` | `name` | – | `NONE`, `BARRIERS_<race>`, `PLAYER_BARRIERS_<race>`, `BARRIERS_CAREER<n>`, `BARRIERS_DEMO`; NUL-terminated, zero-filled. |

Volume fields (the props' convex hulls, 0x5A1760, 0x5AFBB0; values as `parse_collision_volumes` gives them):

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `name` | string ≤ 31 | – | yes | – | `ZCS_PylonA` (header name). |
| `hash` | hash32 | – | yes | – | Header +0x18: the volume's own key; a prop kind's `key` finds its volume by it (0x5A18D0). |
| `vertices` | array<vec3> | m | yes | – | Vertex record +0x10, prop space about the centre of mass; as many as the header's count and the records allow (the smaller). |
| `support` | array<u16> | – | yes | – | Per vertex: the index it reports as a support (record +0x0C; 0 for a vertex with no links). |
| `first`, `count` | array<u16> | – | yes | – | Per vertex: its first link and its link count (record +0x0A, +0x08). |
| `links` | array<u16> | – | yes | – | The link list (`0x39202`, i16 values, negatives read as 0). |

Example (`world/L4RA/collision/shared.toml`, made-up values):

```toml
schema = "ob.collision-shared/1.0"

[[barrier_group]]
hash = "0x00000000"
name = "NONE"
on = 1

[[barrier_group]]
hash = "0x5C0FFEE1"
name = "BARRIERS_4001"
on = 1

[[volume]]
name = "ZCS_PylonA"
hash = "0x2B6E0F11"
vertices = [[-0.2, -0.2, -0.3], [0.2, -0.2, -0.3], [0.0, 0.2, -0.3], [0.0, 0.0, 0.45]]
support = [0, 1, 2, 3]
first = [0, 3, 6, 9]
count = [3, 3, 3, 3]
links = [1, 2, 3, 0, 2, 3, 0, 1, 3, 0, 1, 2]
```

## 4. Layering

- **Mod packs may not change collision in this version.** Every operation on `ob.collision` and
  `ob.collision-shared` items is refused (`MRG.3`), and a mod pack holding a file of this page is refused
  (`COL.7`). Barrier groups' `on` is run-time state, not a pack value.
- **Placements and collision.** Moving, disabling or adding a placement ([world.md](world.md) §4) does not change
  any face: a moved or disabled building keeps its collision where the original has it, and an added one has none.
  Knock-over props are the exception: their collision is their kind's volume, so a moved prop is hit where it now
  stands, a disabled one is gone, and an added placement with `prop` is a prop with that volume. This is today's
  behaviour of map mods (docs/map-editor.md "Limits").

### 4.1 Extension point: collision deltas (reserved, refused for now)

The shape is fixed now so that pack keys stay stable; the content waits until it is traced how the original
builds its packs (cells, sub-cell masks, the `bounds` byte, wall/floor flags, surface ids, and whether faces can be
matched to an object's footprint). Until then a reader refuses any file with schema
`ob.collision-delta/…` (`MRG.3`), so no pack can depend on an unfinished rule.

Reserved (non-normative sketch):

```toml
# world/<REGION>/collision/deltas.toml  (a mod pack)
schema = "ob.collision-delta/1.0"

[[delta]]
placement = "placement:L4RA/1407/12"   # the key of world.md; follows the placement's merged state
cut = "footprint"                      # remove the faces inside the original placement's footprint
bake = "solid"                         # add faces from the drawn solid at its merged transform
surface = "from-cut"                   # surface ids of the removed faces, or a given id
```

Rules the version that enables it must give: which packs a placement touches (its footprint against each pack's
grid, also across the `limit` rule of §2); the order new faces take in a cell (floors first, `COL.4`); how
references and masks are rebuilt; that a delta marks its pack `affects_play` ([manifest.md](manifest.md)); clashes
per (placement key); and an "approximate" warning in the pack check.

## 5. Import

- Decoders: `formats::collision::{parse_collision_pack, parse_collision_grid, parse_barrier_groups,
  parse_collision_volumes}` for the values proof; the packs' and barrier groups' records are copied as found.
- Faces, references and cells are copied whole into `<n>.bin`; the header goes to the TOML file. Constants are
  checked (`cache_slot` = 0xFFFF, zero `pad`; `CONV.11`, `COL.5`), and so are the invariants of §3.1 (a breach in an
  original file is an import error naming the pack, so an edition that differs is caught).
- Residual: any `0x34135` chunk (none in the reference install); the volumes' chunks as a shadow (proof `values`);
  padding as `fill`.

## 6. Equality and verification

| Kind | Level | Encoder (Eq2) | Shadow |
|---|---|---|---|
| collision packs | Eq2 bytes | header from TOML; faces, refs, cells copied (every byte is a named field) | none |
| barrier groups | Eq2 bytes | records from TOML | none |
| prop volumes | Eq1 values | none: the parser keeps the smaller of the header's vertex count and the records (they differ in 106 of 646 volumes), clamps negative links to 0, and the vertex records hold 24 more bytes it does not read | the whole `0x80034020` chunk (L4RA 59,124 B; all regions 426,584 B) |

Proofs (per region, at import and by `base verify`): Eq1, the `xxh3-128` of the flat encoding
(`crates/ob-core/src/cache/persist.rs`) of each pack's `(section, faces)` and `CollisionGrid`, of the barrier group list and of the
volume list, from the original and from the base set, are equal; Eq2, every pack and the barrier table re-encode to
their original bytes.

Tests:

1. `collision_round_trip` (every region): import → the parsed packs, grids, barrier groups and volumes equal the
   originals'; the packs and the barrier table rebuild byte-identical.
2. `ground_is_the_same_from_the_base_set` (L4RA): `TrackGround` built from both sources answers the height query and
   the gather bit-identically over a grid of points every 2 m across every drivable outline, with barrier groups
   off and on; the native-history test of `src/ground.rs` (drivable stream section 716, pack 736, 745 faces)
   passes on the base set.
3. `collision_invariants_hold`: every pack of every region passes §3.1's checks; a pack with a floor after a wall,
   a reference out of range or a wrong `cache_slot` is refused with its rule id.
4. `collision_files_in_a_mod_pack_are_refused`: a mod pack with `collision/1427.toml` or with a
   `collision/deltas.toml` is not mounted (`COL.7`, `MRG.3`).

## 7. Cooked form

- **Cook unit**: the region, together with [world.md](world.md) §7 (one track cache entry).
- **Model**: the ground (`ground::TrackGround`: per pack its grid and its polygons with normal and plane offset), the
  barrier groups and their switches, and the engine's triangle list (`collision_faces`), all in today's `w.rest`.
- **Zero-copy**: faces, references and cells are the engine's input layout; a cooked version MAY read them in place
  from the stored bundle entry. The polygons' normals and plane offsets are prepared data, build-specific; they must
  be computed in the engine's documented operation order (`src/ground.rs:32`: z·nz + y·ny first, then x·nx), or the
  heights move by a last bit.
- **Build-specific**: the 16 m triangle grid of `collision::Collision` (built on first use), and the stand-in from
  render triangles for a region without packs.

## 8. Errors

| Rule | Text |
|---|---|
| COL.1 | A pack file whose `section` differs from its file name or names no stream section of the region; two packs for one stream section. |
| COL.2 | `cells` does not have `width × height` entries. |
| COL.3 | A cell's range outside `refs`, or a reference's face outside `faces`. |
| COL.4 | A cell lists a floor after a wall or a 0x08 face. |
| COL.5 | A face whose `cache_slot` is not 0xFFFF. |
| COL.6 | A face whose `group` has no barrier group. |
| COL.7 | A mod pack holding a file of this page. |
| COL.8 | More than 256 barrier groups, or two with the same name. |
| COL.9 | A volume whose `support`, `first` or `count` length differs from its vertices, or whose links run outside `links` or name a missing vertex. |

## 9. Versioning and extension

- `ob.collision-delta/1.0` (§4.1), once the pack build is traced; with it, packs may also disable single faces by key
  (`collision-face:<REGION>/<n>/<i>`), e.g. to open a wall.
- Surface ids as names once the surface table is described in [engine-tables.md](engine-tables.md).
- Names for the open fields (barrier group +0x04, face flags 0x20 / 0x80).
- An inline form for small packs (tests, hand-made tracks), with the same layouts as field tables, as world.md
  §3.1 does.

## 10. Open points

- How the original builds a pack (cell assignment, sub-cell masks, the `bounds` byte, the floor/wall order inside a
  cell, which faces get 0x01 / 0x40): needed for deltas; the format carries the results as they are.
- Whether a triangle's stored corner 3 repeats corner 2 (the outline test treats it so, `src/ground.rs:37`)
  [to verify over all faces].
- `bounds_min`/`bounds_max` of the header: not read by Overbright; the original's use is open.
- Barrier group +0x04; the volumes' header words and the vertex records' bytes +0x00..+0x07 and +0x20..+0x2F
  (kept only in the shadow until an encoder exists).
- `0x34135`: named in the docs, absent from every shipped pack.

## 11. Sizes and timing

Reference install (L4RA in brackets):

| Data | Original | Base set |
|---|---|---|
| Packs | 599 (255) packs, 16.05 MB (5.91 MB) | `.bin` 16.0 MB (5.9 MB): 417,572 (153,625) faces × 32 B, 620,291 (228,061) refs × 4 B, 37,671 (14,550) cells × 4 B; TOML ~0.2 MB |
| Faces per pack | L4RA median 514, maximum 2,922; largest 15,696 (L4RG) | – |
| Barrier groups | 170 (126) entries, 5.5 KB | ~10 KB TOML |
| Prop volumes | 646 (89), 0.43 MB (59 KB) | ~0.3 MB TOML [guess] + the same as shadow |

Import: a copy and a check of 16 MB, ~0.1–0.2 s [guess]. Test 2's sweep over L4RA: ~1 s [guess].
