# Asset format: meshes (solids)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md), [layering.md](layering.md) and [residual.md](residual.md) apply and are not repeated.

Every mesh of the original is a **solid**, kept in **solid lists** (chunk 0x80134000). The format stores each
solid list as one glTF 2.0 file (https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html) whose binary buffer
holds the original vertex and index arrays **byte for byte**, interleaved as the original interleaves them; every
header field the original keeps beside them is carried in `OB_` glTF extensions, so the import is lossless and the
original chunks can be rebuilt exactly (Eq2). Outside tools (Blender, any glTF viewer) open the files as they are.

## 1. Scope

| Original | Solid lists / solids / with geometry (reference install) | Engine reader today |
|---|---|---|
| `TRACKS/STREAM<REGION>.BUN`, the lists inside each stream section | 2,704 / 28,985 / 28,906 (79 placeholders) | `src/world/parse.rs:117` → `crates/ob-formats/src/formats/solid.rs:111` |
| `CARS/<CAR>/GEOMETRY.BIN` and the shared part groups (`EXHAUST`, `BRAKES`, `MIRRORS_*`, `SPOILER*`, `ROOF`, `AUDIO`, `PLATES`, `SPINNER`, `MIRRORS`) | 56 / 18,728 / 18,728 | `src/car/load.rs:182`, `src/car/assets.rs:176-229` |
| `CARS/WHEELS/GEOMETRY*.BIN` (rims) | 24 / 5,074 / 5,074 | `src/car/assets.rs:203` |
| `NIS/Scene_<NAME>_BundleB.bun` (skinned characters) | 140 / 140 / 140 | `src/character.rs:313-353` |
| `FRONTEND/PLATFORMS/*.BIN` (garage platforms) | 8 / 48 / 48 | `src/garage.rs:578` |
| `FRONTEND/FrontB.lzc` | 1 / 15 / 15 | – (not read) |
| `GLOBAL/InGameCommon.bun` (and its JDLZ twin `InGameCommon.lzc`, same content), `GLOBAL/GLOBALB.BUN`, `GLOBAL/GlobalB.lzc` | 4 / 30 / 30 | `src/world/parse.rs:429` (`MARKER_GPS_AID`) |
| **total** | **2,937 / 53,020 / 52,941** | |

The chunk tree (docs/formats/world.md "Chunk container", "Solids"; docs/formats/cars-1.md "Cars"):

```text
SOLID_LIST 0x80134000
├─ (id 0 padding chunk)
├─ SOLID_LIST_HEADER 0x80134001
│  ├─ 0x00134002  list info (0x90 bytes)          §3.7.6
│  ├─ 0x00134003  member hashes, sorted           derived
│  ├─ 0x00134004  member offset table (80 lists)  derived
│  └─ 0x80134008  empty container                 derived
└─ SOLID 0x80134010, one per solid, padding chunks between them
   ├─ 0x00134011  solid header (0xC0, 16-aligned)          §3.7.1
   ├─ 0x00134012  texture table (u32 hash, u32 0)          §3.7.4
   ├─ 0x00134013  light-material table (optional)          §3.7.4
   ├─ 0x0013401A  markers (optional, 0x50 each, 16-aligned) §3.7.5
   ├─ SOLID_MESH 0x80134100
   │  ├─ 0x00134900  mesh header (0x44, 16-aligned)       §3.7.2
   │  ├─ 0x00134B01  vertices (128-aligned)                §3.2
   │  ├─ 0x00134B02  draw ranges, 0x3C each (16-aligned)   §3.7.3
   │  └─ 0x00134B03  u16 indices (16-aligned, padded to 4)
   └─ 0x00134017, 0x00134018, 0x00134019  (car solids only; not read)
```

Not on this page: textures and texture records ([textures.md](textures.md)); which solid a placement draws, LOD
choice by scenery infos ([world.md](world.md)); which solids a car or part uses, wheel positions
([cars.md](cars.md)); the light-material records (`GLOBALB` chunk 0x135200: not owned by a kind yet, kept in the
residual, [README.md](README.md) §3); collision ([collision.md](collision.md)); skeletons and
animation ([animation.md](animation.md)); the in-engine scenes that place skinned solids
([animation.md](animation.md) §3.6). Solids do not link their levels of detail: the LODs are separate solids that
scenery infos (three hashes) and car parts (`_A`…`_D` name suffixes) name.

The opt-in **model fixes** (`src/render/enh/assets/car_model/fixes.rs`) are not part of the base set; §4.4 says how
they become an opt-in pack.

## 2. Files and keys

### 2.1 One glTF file per solid list

Each original solid list becomes `<unit>/meshes/<list>.gltf` plus its buffer `<unit>/meshes/<list>.bin`. `<list>`
names the list's origin:

| Source | Unit | `<list>` | Example |
|---|---|---|---|
| a list inside stream section *N* of `STREAM<REGION>.BUN` | `world/<REGION>` | `<N>.<n>` | `world/L4RA/meshes/1427.0.gltf` |
| any other file | the file's unit ([layout.md](layout.md) §2) | `<FILE>.<n>`, `<FILE>` the original file name with its extension and case | `cars/240SX/meshes/GEOMETRY.BIN.0.gltf`, `cars/WHEELS/meshes/GEOMETRY_ENKEI.BIN.0.gltf`, `global/meshes/InGameCommon.bun.0.gltf` |
| a garage platform | its sub-unit `frontend/platforms/<X>` ([ui.md](ui.md)) | `<FILE>.<n>` | `frontend/platforms/CarLot01/meshes/CarLot01.BIN.0.gltf` |
| a scene bundle | `scenes/<NAME>` | `bundle.<n>` | `scenes/IntroNis08/meshes/bundle.0.gltf` |

*n* counts the file's (or stream section's) solid lists from 0 in chunk order. Lists are keyed by **stream**
section, the load unit that holds them; scenery sections ([world.md](world.md)) own no lists (stream section 1427
of L4RA, which holds scenery sections 1427 and 1407, has one list: `1427.0`). The file extension stays in `<FILE>`
because `GLOBALB.BUN` and `GlobalB.lzc` differ only in case without it ([conventions.md](conventions.md) §6.2).
A list found outside every stream section would be `stream.<n>` (none in the reference install [to verify]).
Files whose content is identical after unwrapping (`InGameCommon.bun` / `.lzc`) are imported once, from the name
the engine reads (`.bun`); both residual maps point at the same items.

```text
world/L4RA/meshes/1427.0.gltf        list 0 of stream section 1427: JSON (nodes, accessors, OB_ extensions)
world/L4RA/meshes/1427.0.bin         its buffer: vertex arrays, index arrays, unread chunks
world/L4RA/meshes/1427.1.gltf
cars/240SX/meshes/GEOMETRY.BIN.0.gltf
cars/WHEELS/meshes/GEOMETRY_ENKEI.BIN.0.gltf
scenes/IntroNis08/meshes/bundle.0.gltf     one skinned character model
frontend/platforms/CarLot01/meshes/CarLot01.BIN.0.gltf
global/meshes/InGameCommon.bun.0.gltf
cars/GOLF/meshes/model-fixes.mesh-patch.toml    (a mod pack: field and vertex patches of cars/GOLF's solids, §3.9)
cars/GOLF/meshes/kit-extras.gltf                 (a mod pack's own list: solid-list:cars/GOLF/<pack id>:kit-extras, §4.2)
```

### 2.2 Bundles: `.gltf` + `.bin`, not `.glb`

Base sets and packs use the two-file form. The `.bin` is a stored (uncompressed) bundle entry at a 64-byte
aligned offset ([layout.md](layout.md) §4), so vertex and index arrays are zero-copy slices of the mapped bundle; the JSON
is parsed once per cook and is better deflated (measured on the reference install: 119.7 MB of JSON for all lists,
13.8 MB deflated). A `.glb` would put both into one stored entry and lose either the zero-copy buffer or the
compression. Authoring tools may hand `.glb` to `overbright-studio-cli mesh import` (§4.5), which rewrites it.

### 2.3 Keys

| Short name | Key | Example | Item |
|---|---|---|---|
| `solid-list` | `<unit>/<list>`; a pack's own list `<unit>/<pack id>:<stem>` (§4.2) | `solid-list:cars/240SX/GEOMETRY.BIN.0`, `solid-list:cars/GOLF/golf-kits:kit-extras` | one list (one file) |
| `solid` (item key) | `<unit>/<list>/<NAME>`; `<unit>/<list>/0xHASH` when the name is truncated (§3.3) | `solid:world/L4RA/1427.0/XO_CONE_01` | one solid |
| `solid` (scoped reference) | `<NAME>` or `0xHASH` | `solid:XO_CONE_01`, or by its hash `solid:0x608A650A` | the solid the engine finds by that hash in the referencing item's scope |
| draw range | (solid, index) | – | one record of 0x134B02 |
| marker | (solid, index) | – | one record of 0x13401A |

- **Hashes repeat across lists.** In one region the same hash is in several stream sections' lists (L4RA: 1,794 hashes in
  more than one list, 713 of them with different bytes; measured), so the item key includes the list. A hash never
  repeats inside one list (measured: 0 of 2,937 lists).
- **Scoped references** are what other kinds store (scenery infos, car parts, scene characters, world-animation
  entities): `bin_hash(NAME)` or the hash, resolved like the original, by the referencing kind's scope and order:
  the world keeps the first solid of a hash in stream-section load order (`src/world/parse.rs:483-489`); a car takes its
  own geometry, then shared part solids it does not have (`src/car/assets.rs:216-229`); a scene its own lists. The
  referencing kind's page states its scope; this page states only that the order of lists and of solids in a
  list is data ([conventions.md](conventions.md) §6.5).
- Solid order inside a list, draw-range order and marker order are the original order and part of the keys.

## 3. Source form

### 3.1 glTF profile

A mesh file is glTF 2.0 JSON ([conventions.md](conventions.md) §3.5 for floats) with these restrictions:

| Rule | Value |
|---|---|
| `asset` | `version = "2.0"`; `generator` names the writer (`overbright-import 0.4.0`) |
| buffers | exactly one, `uri` = the `.bin` of the same stem; no data URIs; `byteLength` = the file's length |
| `extensionsUsed` | lists the `OB_` extensions present. `extensionsRequired` MUST NOT list them: glTF clients refuse files that require extensions they do not know (glTF 2.0 §3.12), and outside tools must open these files. The engine requires them by this page's rules instead (`MSH.2`). |
| versions | **The file's format marker** is the scene's `OB_solid_list.schema` = `"ob.mesh/<major>.<minor>"` (§3.6), required: the file's kind and the lowest version of this profile and of `OB_solid_list` that reads the file, as a TOML file's `schema` key ([conventions.md](conventions.md) §3.3, [versioning.md](versioning.md) §2.1). **Every other `OB_` extension** in `extensionsUsed` carries its own version in the glTF root object's `extensions`, once per file: `"OB_solid": { "version": "1.0" }`, the lowest version of that extension whose fields the file uses. These versions follow the kind-schema rules ([versioning.md](versioning.md) §1, §2): a reader refuses a file without them (`MSH.1`), of a major it does not read (`VER.2`) or of a newer minor (`VER.1`). The root `extensions` holds nothing else. |
| scenes | exactly one, `scene = 0`; its `extensions.OB_solid_list` (§3.6); its only root node is `OB_Z_UP` |
| `OB_Z_UP` | `rotation = [-0.70710677, 0.0, 0.0, 0.70710677]` (−90° about X: the original's +Z up becomes glTF's +Y up, [conventions.md](conventions.md) §5.1); no other property but `name` and `children`; children = the solid nodes in list order |
| solid node | `name` = the solid's name; `mesh` (absent for a placeholder); `children` = its marker nodes in record order; `extensions.OB_solid` (§3.3); no `matrix`/TRS (vertices are already in the solid's space) |
| marker node | `matrix` (§3.5) and `extensions.OB_marker`; `name` = the marker's name when known |
| meshes | one per solid with vertices; primitives in draw-range order, one per draw range with `index_count > 0`; `mode` 4 (triangles); all primitives of a solid share its attribute accessors |
| bufferViews | per solid: one vertex view (`byteStride` = the layout's stride, `target` 34962), one index view (no stride, `target` 34963), one view per large unread chunk (no `target`); each `byteOffset` a multiple of 16 |
| accessors | per solid: the layout's attributes (§3.2) over the vertex view; per primitive one `UNSIGNED_SHORT` `SCALAR` accessor over the index view at `byteOffset = first_index · 2`, `count = index_count` |
| `POSITION` `min`/`max` | the exact bounds of the vertex data (glTF 2.0 §3.6.2.5 requires them equal to the data); the header's own box is a separate field (§3.3) |
| materials | §3.8; informative |
| anything else | no animations, skins, cameras, images, textures, samplers, sparse accessors, morph targets; `extras` allowed anywhere and ignored |

The original's index buffer is one array per solid, global to the solid's vertex buffer; each draw range is a slice
of it (docs/formats/world.md "Solids"). The index view holds exactly `max(first_index + index_count)` indices; the
original's 2-byte zero pad after an odd count (15,612 solids) is not stored (the encoder writes it, §6.2).

### 3.2 Vertex layouts

The vertex view holds the original vertex chunk's payload unchanged. Three layouts exist; the importer picks one
with the engine's rule (`crates/ob-formats/src/formats/solid.rs:248-251`, `src/character.rs:325-327`): mesh flag 0x01 →
`skinned60`; flag 0x80 or 0x100 and at least 36 bytes a vertex → `lit36`; else `world24`.

`world24`, 24 bytes, 4-aligned (STREAM, platforms, GLOBAL, FrontB; 28,672 + 48 + 30 + 14 meshes, and one car mesh):

| Offset | Size | Type | Field | Unit | glTF attribute | Notes |
|---|---|---|---|---|---|---|
| 0x00 | 12 | `f32[3]` | position | m | `POSITION` VEC3 float | solid space, +Z up |
| 0x0C | 4 | `u8[4]` | `color_bgra` | – | `_COLOR_BGRA` VEC4 unsigned byte, `normalized: true` | baked lighting, D3DCOLOR byte order B, G, R, A |
| 0x10 | 8 | `f32[2]` | uv | – | `TEXCOORD_0` VEC2 float | D3D convention (v down), the same as glTF's |

`lit36`, 36 bytes, 4-aligned (car meshes, rims, 234 world meshes, 1 FrontB mesh):

| Offset | Size | Type | Field | Unit | glTF attribute | Notes |
|---|---|---|---|---|---|---|
| 0x00 | 12 | `f32[3]` | position | m | `POSITION` | |
| 0x0C | 12 | `f32[3]` | normal | – | `NORMAL` VEC3 float | unit length on all 5,546,209 lit vertices (measured) |
| 0x18 | 4 | `u8[4]` | `color_bgra` | – | `_COLOR_BGRA` | |
| 0x1C | 8 | `f32[2]` | uv | – | `TEXCOORD_0` | |

`skinned60`, 60 bytes, 4-aligned (the 140 NIS character models, mesh flags 0x4081):

| Offset | Size | Type | Field | Unit | glTF attribute | Notes |
|---|---|---|---|---|---|---|
| 0x00 | 12 | `f32[3]` | position | m | `POSITION` | bind space |
| 0x0C | 12 | `f32[3]` | normal | – | `NORMAL` | |
| 0x18 | 4 | `u8[4]` | `color_bgra` | – | `_COLOR_BGRA` | |
| 0x1C | 8 | `f32[2]` | uv | – | `TEXCOORD_0` | |
| 0x24 | 12 | `f32[3]` | weights | – | `_WEIGHTS3` VEC3 float | three weights summing to 1 (`src/character.rs` header) |
| 0x30 | 12 | `f32[3]` | bones | – | `_BONES3` VEC3 float | three bone numbers stored as floats, indices into the draw range's `palette` |

Why custom attributes: glTF's `COLOR_0` is RGBA and a linear multiplier (glTF 2.0 §3.7.2.1); the original's colour
is BGRA and used as stored, and a view cannot swizzle. glTF skinning needs `JOINTS_0`/`WEIGHTS_0` as VEC4 integer
joints over one joint list per mesh; the original has three float bone numbers indexing a palette that changes
per draw range, which cannot be expressed without duplicating vertices. Application-specific attribute names start
with `_` as glTF 2.0 §3.7.2.1 requires. Viewers that want colours or a rig get them from an export (§4.6), never
from the stored form.

### 3.3 `OB_solid` (node extension)

Schema: [`schemas/gltf/OB_solid.schema.json`](schemas/gltf/OB_solid.schema.json). Fields of the solid header
(§3.7.1), the mesh header (§3.7.2), the tables and the draw ranges. Every original byte is a field here, a
derived value (§3.7), or in the buffer.

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `hash` | hash32 | – | yes | – | Header +0x10, the solid's hash. Equals `bin_hash(node.name)` unless `name_truncated` (`MSH.8`). **verified** |
| `name_truncated` | bool | – | no | `false` | The header holds 27 characters, and no completion of up to 4 characters matched the hash (`crates/ob-formats/src/formats/hash.rs:25`): `node.name` holds those 27. Completed names are stored whole and not flagged (the engine uses the completion, `crates/ob-formats/src/formats/solid.rs:181-186`). |
| `layout` | enum | – | yes | – | `world24`, `lit36`, `skinned60` (§3.2), or `none` for a placeholder (vertex count 0: no mesh, no vertex or index view). |
| `bbox_min`, `bbox_max` | vec3 | m, solid space | yes | – | Header +0x20 / +0x30 (**verified**; what the engine reads). Not equal to the vertex bounds in any of the 52,941 solids with vertices (measured) [inferred: padded]. |
| `axes` | mat3 | – | no | identity | Header +0x40, +0x50, +0x60 (rows) [inferred: the solid's frame]. |
| `translation` | vec3 | m | no | `[0.0, 0.0, 0.0]` | Header +0x70: the engine reads it (car render info +0x30, `crates/ob-formats/src/formats/solid.rs:163`; the light flares' origin, `:212-221`). |
| `triangle_count` | u32 | – | yes | – | Header +0x14 [inferred: triangles; equals the draw ranges' indices ÷ 3 in 52,533 of 53,020 solids]. |
| `u_0x0c` | u32 | – | no | `0x00000016` | **open** (0x16, 0x400016, 0x800016 occur). |
| `u_0x18` | u32 | – | no | `0` | **open** (101 values; low 16 bits often 0x100 × draw-range count). |
| `u_0x88`, `u_0x8c` | u32 | – | no | `0x000EE580` | **open**; that value in every solid. |
| `u_0x94`, `u_0x98` | f32 | – | no | `0.0` | **open**; −∞ and −0.0 occur. This field allows −∞, written `"bits:0xFF800000"` (JSON has no literal for it, [conventions.md](conventions.md) §3.4); +∞ and NaN are refused (`CONV.7`). −0.0 is written `-0.0` and differs from the default (floats compare by bits). |
| `u_0x7c` | f32 | – | no | `1.0` | **open**; 1.0 in every solid (the matrix's w). |
| `u_0x00`, `u_0x04`, `u_0x08`, `u_0x1c`, `u_0x2c`, `u_0x3c`, `u_0x4c`, `u_0x5c`, `u_0x6c`, `u_0x80`, `u_0x84`, `u_0x90`, `u_0x9c`, `u_0xa0` | u32 | – | no | `0` | **open**; zero in every solid. |
| `mesh` | table | – | yes | – | The mesh header, below. |
| `textures` | array<ref `texture`> | – | yes | – | Chunk 0x134012 in slot order (draw ranges index it). At most 255. |
| `light_materials` | array<hash32> | – | no | absent | Chunk 0x134013 in order (draw ranges index it). Absent = no chunk; `[]` = an empty chunk. At most 255. **Bare hashes** ([conventions.md](conventions.md) §6.6): light materials are not owned by a kind yet ([README.md](README.md) §3); the engine resolves these hashes against the light-material records of `GLOBAL/GLOBALB.BUN` (chunk 0x135200), which stay in the residual. |
| `draw_ranges` | array<table> | – | yes | – | Chunk 0x134B02 records in order, below. At most 255 (`texture_slot` is a byte). |
| `extra_chunks` | array<table> | – | no | `[]` | The solid's chunks no reader reads, in file order (0x134017, 0x134018, 0x134019 on 15,658 car solids, 43.6 MB): `{ id, bufferView }` for a body of 65 bytes or more, `{ id, bytes }` (≤ 64 bytes, may be empty) otherwise. **open** |
| `mesh_error` | bool | – | no | `false` | The original mesh did not parse; the engine drops its geometry and flags (`crates/ob-formats/src/formats/solid.rs:222-231`). The node has no mesh, `layout = "none"`, `mesh.flags = 0`, and the solid's chunk is kept in the residual. None in the reference install. |

`mesh` (mesh header 0x134900):

| Field | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `flags` | u32 | yes | – | +0x0C: 0x01 skinned, 0x40 world reflection, 0x80 car effect, 0x100 environment map (**verified**, `crates/ob-formats/src/formats/solid.rs:77-80, 90-103`). |
| `triangle_count` | u32 | no | = `OB_solid.triangle_count` | +0x24 (differs in 487 solids). |
| `u_0x08` | u32 | no | `0x10` | **open**; 0x10 in every mesh. |
| `u_0x00`, `u_0x04`, `u_0x14`, `u_0x18`, `u_0x1c`, `u_0x20`, `u_0x28`, `u_0x2c`, `u_0x30`, `u_0x38`, `u_0x3c`, `u_0x40` | u32 | no | `0` | **open**; zero everywhere. |

Draw range (record of 0x134B02, 0x3C bytes):

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `bbox_min`, `bbox_max` | vec3 | m | yes | – | +0x00 / +0x10 [inferred: the range's bounds]; not read by the engine. |
| `index_count` | u32 | – | yes | – | +0x0C; 0 in 5 records (no primitive then). A multiple of 3 (all records). |
| `first_index` | u32 | – | yes | – | +0x34, into the solid's index array. |
| `texture_slot` | u8 | – | yes | – | +0x1C, index into `textures`. |
| `u_0x1d` | bytes (3) | – | no | zeros | +0x1D..+0x1F. |
| `light_material` | u8 | – | no | `0xFF` | +0x20, index into `light_materials`; 0xFF = the default material (docs/formats/cars-1.md). |
| `palette` | bytes (19) | – | no | zeros | +0x21..+0x33: for skinned meshes the palette's bone numbers (`src/character.rs:343-347`); zero on every other mesh. |
| `flags` | u32 | – | no | `0` | +0x38: the car draw's flags (0x100 env map, 0x80 diffuse/specular; **verified**, docs/formats/cars-1.md). |

The *k*-th draw range with `index_count > 0` is the mesh's *k*-th primitive (`MSH.6`).

### 3.4 Placeholders

79 world solids have a mesh header with vertex count 0 (docs/formats/world.md "Solids"). They are solid nodes
without `mesh`, `layout = "none"`, and their `mesh` table, `textures` and (empty) `draw_ranges` as stored. They keep
their place and key: scenery infos name them.

### 3.5 `OB_marker` (node extension)

Schema: [`schemas/gltf/OB_marker.schema.json`](schemas/gltf/OB_marker.schema.json). A solid's position markers
(chunk 0x13401A; 31,322 records: 8,943 chunks in car files, 143 in tracks) are child nodes of the solid node, in
record order, so tools show them as empties (exhaust tips, mirror, spoiler and plate mounts, flares).

| Field | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `hash` | hash32 | yes | – | Record +0x00: the marker's name hash (`EXHAUST_LEFT`, `SPOILER`, `REAR_LICENSE_PLATE`, …; docs/formats/cars-1.md "Shared part models"). `node.name` holds the name when it is known. |
| `kind` | u32 | no | `0` | Record +0x04 (0, 1, 3 occur): the light flares' kind (`crates/ob-formats/src/formats/solid.rs:84-87`). |
| `u_0x08`, `u_0x0c` | u32 | no | `0` | **open**; zero everywhere. |

`node.matrix` holds record +0x10..+0x4F, the 16 floats in file order: the original's row-vector matrix (rows =
axes, translation last) read in that order is exactly glTF's column-major matrix of the same transform, so no
float is reordered or recomputed. All 31,322 matrices are affine (w column 0, 0, 0, 1) with orthogonal axes,
734 of them mirrored (measured): decomposable to TRS as glTF 2.0 §3.5.3 requires.

### 3.6 `OB_solid_list` (scene extension)

Schema: [`schemas/gltf/OB_solid_list.schema.json`](schemas/gltf/OB_solid_list.schema.json). The file's format
marker and the list header's info chunk 0x134002 (§3.7.6); the rest of the header is derived.

| Field | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `schema` | string | yes | – | `"ob.mesh/1.0"`: the format marker (§3.1 "versions"); the version of the profile and of `OB_solid_list`. |
| `list_version` | u32 | no | `29` | +0x08, the original list header's version; 29 everywhere. Not the format's version. |
| `path` | string (≤ 63 bytes, ASCII) | yes | – | +0x10, NUL-terminated: the original tool's pipeline path (`Location4\RegionA\Pipeline\PC\eLabScenery_…`). |
| `path_tail` | bytes (≤ 63) | no | zeros | The bytes after the path's NUL up to +0x4F (stale tool bytes occur). |
| `u_0x68`, `u_0x6c` | u32 | no | `0` | **open**. |
| `u_0x70` | u32 | no | `0x80` | **open**; 0x80 everywhere [inferred: an alignment]. |
| `u_0x00`, `u_0x04`, `u_0x50` … `u_0x64`, `u_0x74` … `u_0x8c` | u32 | no | `0` | **open**; zero everywhere. |
| `offset_table` | bool | no | `false` | The header has chunk 0x134004 (80 car lists). |

Derived, never stored: +0x0C (the solid count), chunk 0x134003 (each solid's hash and a zero word, ascending by
hash: all 2,937 lists), chunk 0x134004 (per solid, ascending by hash: hash, the solid chunk's file offset, its size
with header twice, two zero words: all 80 tables), the empty container 0x80134008, and the order info, hashes,
[offset table], container (all lists).

### 3.7 Original layouts (normative for the encoder)

#### 3.7.1 Solid header (chunk 0x134011; from the first 16-aligned file offset of the body)

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 12 | `u32[3]` | `u_0x00`, `u_0x04`, `u_0x08` | – | |
| 0x0C | 4 | `u32` | `u_0x0c` | – | |
| 0x10 | 4 | `u32` | `hash` | – | |
| 0x14 | 4 | `u32` | `triangle_count` | – | |
| 0x18 | 4 | `u32` | `u_0x18` | – | |
| 0x1C | 4 | `u32` | `u_0x1c` | – | |
| 0x20 | 12 | `f32[3]` | `bbox_min` | m | |
| 0x2C | 4 | `u32` | `u_0x2c` | – | |
| 0x30 | 12 | `f32[3]` | `bbox_max` | m | |
| 0x3C | 4 | `u32` | `u_0x3c` | – | |
| 0x40 | 12 | `f32[3]` | `axes` row x | – | |
| 0x4C | 4 | `u32` | `u_0x4c` | – | |
| 0x50 | 12 | `f32[3]` | `axes` row y | – | |
| 0x5C | 4 | `u32` | `u_0x5c` | – | |
| 0x60 | 12 | `f32[3]` | `axes` row z | – | |
| 0x6C | 4 | `u32` | `u_0x6c` | – | |
| 0x70 | 12 | `f32[3]` | `translation` | m | |
| 0x7C | 4 | `f32` | `u_0x7c` | – | |
| 0x80 | 8 | `u32[2]` | `u_0x80`, `u_0x84` | – | |
| 0x88 | 8 | `u32[2]` | `u_0x88`, `u_0x8c` | – | |
| 0x90 | 4 | `u32` | `u_0x90` | – | |
| 0x94 | 8 | `f32[2]` | `u_0x94`, `u_0x98` | – | |
| 0x9C | 8 | `u32[2]` | `u_0x9c`, `u_0xa0` | – | |
| 0xA4 | 28 | `char[28]` | name | – | the first 27 characters of `node.name`, NUL, zero-filled (all 53,020: zero after the NUL) |

Size 0xC0, alignment 16. The body before it is `0x11` fill up to the 16-aligned offset (20,154 headers) or nothing.

#### 3.7.2 Mesh header (chunk 0x134900; from the first 16-aligned file offset of the body)

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `u_0x00`, `u_0x04` | – | |
| 0x08 | 4 | `u32` | `u_0x08` | – | |
| 0x0C | 4 | `u32` | `flags` | – | |
| 0x10 | 4 | `u32` | draw-range count | – | derived: the number of `draw_ranges` |
| 0x14 | 16 | `u32[4]` | `u_0x14` … `u_0x20` | – | |
| 0x24 | 4 | `u32` | `triangle_count` | – | `mesh.triangle_count` |
| 0x28 | 12 | `u32[3]` | `u_0x28`, `u_0x2c`, `u_0x30` | – | |
| 0x34 | 4 | `u32` | vertex count | – | derived: the vertex accessors' `count`, 0 for `none` |
| 0x38 | 12 | `u32[3]` | `u_0x38`, `u_0x3c`, `u_0x40` | – | |

Size 0x44, alignment 16.

#### 3.7.3 Draw range (record of chunk 0x134B02; the array from the first 16-aligned file offset of the body)

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 12 | `f32[3]` | `bbox_min` | m | |
| 0x0C | 4 | `u32` | `index_count` | – | |
| 0x10 | 12 | `f32[3]` | `bbox_max` | m | |
| 0x1C | 1 | `u8` | `texture_slot` | – | |
| 0x1D | 3 | `u8[3]` | `u_0x1d` | – | |
| 0x20 | 1 | `u8` | `light_material` | – | |
| 0x21 | 19 | `u8[19]` | `palette` | – | |
| 0x34 | 4 | `u32` | `first_index` | – | in indices, not bytes |
| 0x38 | 4 | `u32` | `flags` | – | |

Size 0x3C, alignment 4; records back to back, the array starting at a 16-aligned file offset.

#### 3.7.4 Texture and light-material tables (chunks 0x134012, 0x134013; from the body's first byte)

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | hash | – | the texture's or light material's hash |
| 0x04 | 4 | `u32` | `pad` | – | zero (all 80,382 tables) |

Size 8, alignment 4; records back to back. No hash repeats inside a table (measured), so slot ↔ hash is one to one.

#### 3.7.5 Marker (record of chunk 0x13401A; the array from the first 16-aligned file offset of the body)

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `hash` | – | |
| 0x04 | 4 | `u32` | `kind` | – | |
| 0x08 | 8 | `u32[2]` | `u_0x08`, `u_0x0c` | – | |
| 0x10 | 64 | `f32[16]` | `node.matrix` | m (translation row) | file order = glTF's column-major order (§3.5) |

Size 0x50, alignment 16; records back to back.

#### 3.7.6 List info (chunk 0x134002; from the body's first byte, a 16-aligned file offset in every list)

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 8 | `u32[2]` | `u_0x00`, `u_0x04` | – | |
| 0x08 | 4 | `u32` | `list_version` | – | |
| 0x0C | 4 | `u32` | solid count | – | derived |
| 0x10 | 64 | `char[64]` | `path`, NUL, `path_tail` | – | |
| 0x50 | 24 | `u32[6]` | `u_0x50` … `u_0x64` | – | |
| 0x68 | 8 | `u32[2]` | `u_0x68`, `u_0x6c` | – | |
| 0x70 | 4 | `u32` | `u_0x70` | – | |
| 0x74 | 28 | `u32[7]` | `u_0x74` … `u_0x8c` | – | |

Size 0x90, alignment 16.

### 3.8 glTF materials and `OB_material` (informative)

The engine ignores glTF materials. The writer adds one per distinct (texture, light material, draw flags) of the
file, named after the texture (its name when [textures.md](textures.md) knows it, else `0xHASH`), and points each
primitive at its material, so tools show material slots. Schema:
[`schemas/gltf/OB_material.schema.json`](schemas/gltf/OB_material.schema.json).

| Field | Type | Req. | Default | Meaning |
|---|---|---|---|---|
| `texture` | ref `texture` | yes | – | The draw range's texture. |
| `light_material` | hash32 | no | absent (the default material) | The light material's bare hash, as in `OB_solid.light_materials` (§3.3). |
| `flags` | u32 | no | `0` | The draw range's flags. |

They are the bridge for authoring: the import path (§4.5) builds draw ranges and tables from them.

### 3.9 Mesh patch (`ob.mesh-patch/1.0`, mod packs only)

Field-level changes to existing solids without shipping their geometry. File: `<unit>/meshes/<name>.mesh-patch.toml`.
Schema: [`schemas/mesh-patch.schema.json`](schemas/mesh-patch.schema.json).

**A patch file changes only solids of its own unit** (`MSH.16`): `cars/GOLF/meshes/*.mesh-patch.toml` patches the
solids of `cars/GOLF/meshes/`, never those of another car or of a shared part group (whose patches go in that
group's unit, `cars/SPOILER/meshes/…`). A unit's patches are cooked with the unit itself (§7), so loading one car
never depends on another car's files. Several patch files in one folder apply in byte order of their file names
([layering.md](layering.md) §3.1).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.mesh-patch/1.0` |
| `solid` | array<table> | – | yes | – | One entry per patched solid. |
| `solid.target` | ref `solid` (item key) | – | yes | – | The solid, in this file's unit (`MSH.16`): a base solid, or a solid a pack this pack depends on added (`<unit>/<pack id>:<stem>/<NAME>`, [layering.md](layering.md) §3.1). |
| `solid.guard` | table | – | no | – | `hash` (hash32), `vertex_count` (u32), `vertices` (digest of the vertex view): checked against the stack below (`MSH.13`). |
| `solid.op` | `"merge"` | – | no | `"merge"` | Only `merge`; replacing a solid is a glTF node (§4.2). |
| `solid.disabled` | bool | – | no | `false` | §4.1. A higher layer MAY set it back to `false`. |
| `solid.texture` | array<table> | – | no | – | Keyed by `slot` (u8): `texture` = the new scoped reference. |
| `solid.draw_range` | array<table> | – | no | – | Keyed by `index` (u32): `texture_slot`, `light_material`, `flags` replace; `disabled = true` hides the range. |
| `solid.marker` | array<table> | – | no | – | Keyed by `index`: `disabled = true` drops the marker from the engine's list; `matrix` (16 f32) replaces it. |
| `solid.vertex_patch` | array<table> | – | no | – | `attribute` (`TEXCOORD_0`, `_COLOR_BGRA`, `POSITION`, `NORMAL`), `vertices` (array<u16>, vertex numbers; below the solid's vertex count, `MSH.13`), `from` (the exact current value: a guard), `to` (the new value). Shapes per attribute: `TEXCOORD_0` 2 × f32; `POSITION`, `NORMAL` 3 × f32; `_COLOR_BGRA` 4 × u8 (B, G, R, A). Floats compare by bits ([conventions.md](conventions.md) §3.4). |
| `solid.hide_triangles` | array<table> | – | no | – | `triangles` (array<u32>, index ÷ 3 into the solid's index array), optional `when_part_fitted` (u8, a car slot number whose fitted part above level 0 makes the hiding apply; [cars.md](cars.md) defines slots) [guess: the shape the antenna fix needs]. |

The model-fixes pack (§4.4) therefore has one patch file per car it touches. Its GOLF file
([`examples/meshes/model-fixes/cars/GOLF/meshes/model-fixes.mesh-patch.toml`](examples/meshes/model-fixes/cars/GOLF/meshes/model-fixes.mesh-patch.toml);
the IS300 and PEUGOT106 files are beside it):

```toml
schema = "ob.mesh-patch/1.0"

# GOLF KIT01 front bumper: the grille slats sample a painted texel; move them to the trim box.
[[solid]]
target = "solid:cars/GOLF/GEOMETRY.BIN.0/GOLF_KIT01_FRONT_BUMPER_A"
guard = { hash = "0x34886201", vertex_count = 983 }

[[solid.vertex_patch]]
attribute = "TEXCOORD_0"
vertices = [118, 119, 131, 132]
from = [0.043, 0.854]
to = [0.795, 0.719]
```

and `cars/IS300/meshes/model-fixes.mesh-patch.toml`:

```toml
schema = "ob.mesh-patch/1.0"

# IS300 body kit 11 rear bumper: a duplicated exhaust marker.
[[solid]]
target = "solid:cars/IS300/GEOMETRY.BIN.0/IS300_KIT11_REAR_BUMPER_A"
guard = { hash = "0xFA8AA3AA" }

[[solid.marker]]
index = 3
disabled = true
```

(The hashes and vertex counts are the reference install's; the vertex numbers, the marker index and the triangle
numbers in these examples are made up.)

### 3.10 Limits

| What | Limit |
|---|---|
| Vertices per solid | 65,535 (u16 indices; glTF forbids index 65535, §3.1; largest original: 15,618) |
| Draw ranges, textures, light materials per solid | 255 |
| Solids per list | 65,535 (largest original: 1,438) |
| Index array per solid | 2³¹ − 1 bytes |
| A `.gltf` file | 64 MiB; a `.bin` file 1 GiB |
| Markers per solid | 4,096 |
| `extra_chunks` per solid | 16; each body < 16 MiB |

### 3.11 Example

A list with one `world24` solid of one triangle (made-up content: the real list `1427.0` holds ten solids; the full file is
[`examples/meshes/1427.0.gltf`](examples/meshes/1427.0.gltf) with its 96-byte buffer `1427.0.bin`; in a base set it
would be `world/L4RA/meshes/1427.0.gltf`):

```json
{
  "asset": { "version": "2.0", "generator": "overbright-import 0.4.0" },
  "extensionsUsed": ["OB_solid", "OB_solid_list", "OB_material"],
  "extensions": { "OB_solid": { "version": "1.0" }, "OB_material": { "version": "1.0" } },
  "scene": 0,
  "scenes": [{ "name": "1427.0", "nodes": [0],
    "extensions": { "OB_solid_list": { "schema": "ob.mesh/1.0",
      "path": "Location4\\RegionA\\Pipeline\\PC\\eLabScenery_EXAMPLE" } } }],
  "nodes": [
    { "name": "OB_Z_UP", "rotation": [-0.70710677, 0.0, 0.0, 0.70710677], "children": [1] },
    { "name": "XO_CONE_01", "mesh": 0, "extensions": { "OB_solid": {
        "hash": "0x608A650A", "layout": "world24",
        "bbox_min": [-0.51, -0.51, -0.01], "bbox_max": [0.51, 0.51, 1.01], "triangle_count": 1,
        "mesh": { "flags": 0 }, "textures": ["texture:0x1A2B3C4D"],
        "draw_ranges": [{ "bbox_min": [-0.5, -0.5, 0.0], "bbox_max": [0.5, 0.5, 1.0],
                          "index_count": 3, "first_index": 0, "texture_slot": 0 }] } } }
  ],
  "meshes": [{ "primitives": [{ "attributes": { "POSITION": 0, "_COLOR_BGRA": 1, "TEXCOORD_0": 2 },
                                "indices": 3, "material": 0, "mode": 4 }] }],
  "materials": [{ "name": "0x1A2B3C4D", "extensions": { "OB_material": { "texture": "texture:0x1A2B3C4D" } } }],
  "accessors": [
    { "bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [-0.5, -0.5, 0.0], "max": [0.5, 0.5, 1.0] },
    { "bufferView": 0, "byteOffset": 12, "componentType": 5121, "normalized": true, "count": 3, "type": "VEC4" },
    { "bufferView": 0, "byteOffset": 16, "componentType": 5126, "count": 3, "type": "VEC2" },
    { "bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR" }
  ],
  "bufferViews": [
    { "buffer": 0, "byteOffset": 0, "byteLength": 72, "byteStride": 24, "target": 34962 },
    { "buffer": 0, "byteOffset": 80, "byteLength": 6, "target": 34963 }
  ],
  "buffers": [{ "uri": "1427.0.bin", "byteLength": 96 }]
}
```

## 4. Layering

| Item | merge | add | replace | disabled | Clash key |
|---|---|---|---|---|---|
| `solid` | yes (`ob.mesh-patch`) | yes (a new name: a glTF node with `OB_solid.op = "add"` in the pack's own list, §4.2) | yes (glTF node with `op = "replace"` and `target`) | yes (`disabled = true` in a mesh patch; default `false`) | the solid's item key |
| `solid-list` | refused | yes (a pack's own mesh file, keyed `<unit>/<pack id>:<stem>`) | refused | refused | – |
| draw range, marker | yes (keyed by index, via the patch) | refused (`MRG.3`; replace the solid) | – | yes | (solid, index) |
| texture slot | yes (keyed by slot) | refused | – | – | (solid, slot) |

- No wildcard units for meshes: a hash means different solids in different units and lists (§2.3).
- **One unit per file.** A pack file changes only solids of its own unit: a mesh patch's targets and a `replace`
  node's `target` lie in the unit the file is in (`MSH.16`, §3.9).
- Refused operations fail with `MRG.3`.

### 4.1 Disabled solids

A disabled solid stays in its list with its key and position, and loads as a placeholder (no vertices, no draw
ranges, markers kept): every reference still resolves, nothing is drawn. A disabled draw range is skipped by the
draw (its indices stay); a disabled marker is left out of the solid's marker list.

### 4.2 Solids in pack mesh files

A pack's mesh file has the profile of §3.1 with two more fields in `OB_solid` on each solid node:

| Field | Type | Req. | Meaning |
|---|---|---|---|
| `op` | `"add"` \| `"replace"` | yes in packs, absent in base sets | `add`: a new solid named `node.name`, keyed `<unit>/<pack id>:<stem>/<NAME>` (below). `replace`: the whole solid at `target` becomes this node's (geometry, header fields, tables, markers); key and position stay the base's. |
| `target` | ref `solid` (item key) | with `replace` | The replaced solid, in the file's own unit (`MSH.16`). The node's `hash` MUST equal the target's (`MSH.12`). |

**Keys of a pack's lists.** A pack's mesh file `<unit>/meshes/<stem>.gltf` is a list of its own, keyed
`<unit>/<pack id>:<stem>` (`solid-list:cars/GOLF/golf-kits:kit-extras` for the file
`cars/GOLF/meshes/kit-extras.gltf` of the pack `golf-kits`), so it never shares a key with a base list (`1427.0`,
`GEOMETRY.BIN.0`) or with another pack's list ([layering.md](layering.md) §3.1, §3.4). Its added solids are keyed
`<unit>/<pack id>:<stem>/<NAME>`; only that pack defines them, and a `merge` (a mesh patch) or `replace` of them is
allowed only from a higher pack that lists it in `[dependencies]` (`ORD.2`). In a pack's file `OB_solid_list.path`
MAY be empty.

Merged order: each base list in order with replaced solids in place; then the packs' added lists, lowest layer
first, each pack's files in byte order of their logical paths ([layering.md](layering.md) §3.1, §3.3). Scoped references resolve
through that order, so a base name always finds the base solid (or its replacement) first; added names are new by
[conventions.md](conventions.md) §6.4 (`CONV.15`).

### 4.3 What a replacement or addition may change

Anything the profile allows: vertex layout (`world24` in world units, `lit36` for car parts, as the engine's
effect needs; `MSH.14`), counts, tables, markers. With "Use mods" off nothing of it is read: base-only is the
original ([parity.md](parity.md) §1).

### 4.4 The model fixes as an opt-in pack

The fixes (`src/render/enh/assets/car_model/fixes.rs`) are corrections of the shipped car models, applied today in
code when the Enhancements row is on (off = untouched files). In the format they become a pack Overbright ships
and the launcher offers as an opt-in (`origin = "own-work"`: the pack holds only patch data, no original bytes;
[manifest.md](manifest.md)):

| Fix today | In the pack |
|---|---|
| Duplicate markers on `IS300_KIT11_REAR_BUMPER_A` (`DUPLICATE_MARKERS`, `fixes.rs:27`) | `solid.marker` entries with `disabled = true` |
| Grille UVs on `GOLF_KIT01_FRONT_BUMPER_A/B/C` (`UV_FIXES`, `fixes.rs:44`) | `solid.vertex_patch` on `TEXCOORD_0` with `from` guards (the fix's own check that the data still has the error becomes the guard) |
| PEUGOT106 antenna under a roof scoop (`ANTENNA_UNDER_SCOOP`, `fixes.rs:79`) | `solid.hide_triangles` with `when_part_fitted = 7` (the roof-scoop slot) |
| Front wheel shifts (`WHEEL_SHIFTS`, `fixes.rs:62`) | not a mesh change: a per-car drawn-wheel offset, an override on [cars.md](cars.md) |

Equality test: the pack applied to the base set gives the same `Solid`s as `fixes::apply_always` on the original
(`fixes.rs:137`), and with the pack off the base `Solid`s are unchanged.

### 4.5 New solids from outside tools (add-on props and cars, later)

Packs never hold raw tool output: `overbright-studio-cli mesh import <file.gltf|.glb> --unit <unit>` (and Studio)
normalises a glTF 2.0 file from any tool into the profile of §3.1:

- input: triangle primitives (`mode` 4) with `POSITION`, `TEXCOORD_0`; optional `NORMAL`, `COLOR_0`
  (converted to `_COLOR_BGRA` by quantising the stored 0–1 values to bytes, no transfer function, as the original's
  colour is used as stored); node transforms are applied to the vertices; +Y up is turned back to +Z up;
- output layout: `world24` for world and global units, `lit36` for car units (`NORMAL` required), never
  `skinned60` until characters can be added;
- draw ranges: one per primitive, grouped by glTF material; `textures` from the materials' `OB_material.texture` or,
  without it, from the material name (a texture name or `0xHASH`), resolved in the stack (`CONV.9`);
- markers: child empties whose name is a marker name, or with `OB_marker`;
- header: `bbox_*` and per-range boxes computed from the vertices; `triangle_count` set; all `u_*` at defaults;
- checks, refusing the file: more than 65,535 vertices in one solid (`MSH.5`; the tool offers to split by
  material); an index ≥ the vertex count or a degenerate count (`MSH.4`); more than 255 materials or textures
  (`MSH.6`); a texture reference that resolves to nothing (`CONV.9`); a name outside `[A-Za-z0-9_]{1,63}` or one
  whose hash collides (`CONV.8`, `CONV.15`); missing UVs or (cars) normals (`MSH.14`).

Round trips through Blender keep geometry, UVs, names, material names and `extras` (Blender imports and exports
`extras` as custom properties; its exporter writes attributes whose names start with `_` when asked,
https://docs.blender.org/manual/en/4.2/addons/import_export/scene_gltf2.html) but drop `OB_` extensions; the import
path above rebuilds them from names and materials. Whether Blender's importer keeps `_COLOR_BGRA` as a mesh
attribute is [to verify].

### 4.6 Exports (views, never read back as base data)

`overbright-studio-cli mesh export` writes a viewing copy: `COLOR_0` (RGBA) from `_COLOR_BGRA`, textures as PNG
materials, skinned models with a glTF skin built from their skeleton ([animation.md](animation.md) §3.2; vertices
duplicated per draw range so each range's palette becomes the joint list). Exports carry
`extras.ob_view = true`; the import path refuses them as base data (`MSH.15`).

## 5. Import

| Source | Decoder | Output |
|---|---|---|
| any solid list | `parse_solid_list` (`crates/ob-formats/src/formats/solid.rs:111`) for the model; the chunk walk of §3.7 for every other byte | one `.gltf` + `.bin` per list |
| NIS lists | `SkinnedModel::parse` (`src/character.rs:317`) for the model | `skinned60` solids |

Steps per list: walk the chunks; for each solid fill `OB_solid` from §3.7, copy the vertex payload (from the
128-aligned start, `Chunk::payload`, `crates/ob-formats/src/formats/bundle.rs:91`) and the index array (`max(first + count)` indices)
into the buffer at 16-aligned offsets; complete names as the engine does (`complete_truncated_name`, up to 4
characters); build markers, primitives, accessors (computing `POSITION` bounds), materials; then run the encoder
(§6.2) and compare.

Residual map ([residual.md](residual.md) §3.1, §3.1.1), per list:

```text
0x80134000 (the list)                    container
├─ padding chunk before the header       fill (0x00)
├─ 0x80134001 (the list header)          container
│  ├─ 0x00134002, 0x00134003, [0x00134004]   decoded, proof = "bytes", items = ["solid-list:<unit>/<list>"]
│  └─ 0x80134008                         container (no children)
├─ [padding chunk before the first solid, car lists]   fill (0x00)
└─ 0x80134010 … (every solid, with the padding chunks between them)
                                         one run node: decoded, proof = "bytes", count = the solid count,
                                         items = ["solid:<unit>/<list>/*"]
```

The run node's `path` names the first solid and its `size` reaches the end of the list; the encoder rebuilds every
solid of the run and the padding chunks between them (§6.2 rule 5). A solid whose encoder output differs, or with
`mesh_error`, splits the run: it becomes a node of its own (a run of one solid) with `proof = "values"` and its
bytes kept, and the solids before and after it form runs of their own (none expected: §6.2).

## 6. Equality and verification

### 6.1 Eq1 (values)

`H(model from original) == H(model from base set)` per load unit, where the model is what the engine consumes:
`Vec<Solid>` with its persisted fields (`crates/ob-formats/src/formats/solid.rs:301-302`, including the derived `light_markers`), the
part-file solid index (`src/car/assets.rs:213`, the lists' hashes), `SkinnedModel`s (`src/character.rs`), and, for
the world, the solids after the region's first-match merge. Vertex and index bytes are equal by construction (they
are the same bytes; their digests are compared too). The car loader also reads markers and header +0x70 straight from the
chunks (`solid_markers`, `solid_translation`, `src/car/load.rs:471-599`); those reads move onto the markers and
`translation` of the model, so Eq1 covers them ([parity.md](parity.md) §2 "What the model must contain").

### 6.2 Eq2 (bytes): the encoder

Cheap, because everything is fixed records and the layout rules hold everywhere (measured on all 2,937 lists and
53,020 solids of the reference install):

1. Solid children in the order header, textures, [light materials], [markers], mesh, [extra chunks]; mesh children
   header, vertices, draw ranges, indices (all 53,020).
2. Header, mesh header, draw ranges and markers start at the first 16-aligned **file** offset of their chunk body,
   the vertex payload at the first 128-aligned one; the gap is `0x11` bytes; the fill is always the shortest
   (all solids). Tables (0x134012/13) start at the body's first byte.
3. The index chunk ends with two zero bytes when the index count is odd (15,612), else at the last index.
4. The list header: §3.6 derived rules (all lists).
5. Padding chunks (id 0, zero bytes; measured on the 2,934 lists of the uncompressed files): every list's body
   starts with one, the shortest (8 bytes or more) that puts the header chunk at a 16-aligned file offset. In car
   files (`CARS/`, the units `cars/<NAME>`) every solid chunk starts at a 128-aligned file offset, after the
   shortest padding chunk when it is not already aligned (all 23,802 car solids: 22,993 after a padding chunk, 809
   without); in every other file solids follow each other with no padding chunk (all 29,188 measured). The padding
   chunk before the header and, in car lists, the one before the first solid (when present) are `fill` nodes; those
   between solids are rebuilt by the encoder as part of the run node (§5). [to verify: the three lists of the
   whole-file JDLZ files `FrontB.lzc`, `GlobalB.lzc`, `InGameCommon.lzc`, not yet measured.]

So the base set needs no shadow for meshes: 0 bytes, if every node passes at import (a node that does not is kept,
§5).

### 6.3 Tests

- Eq1 and Eq2 over the reference install in CI (`overbright-import --verify`).
- glTF validity: every written file passes the Khronos glTF Validator with no errors (warnings allowed only for
  the unknown `OB_` extensions) [to verify: the validator's handling of `_`-attributes with `normalized`].
- The shot, load and cache gates ([parity.md](parity.md) §3) with the engine reading solids from the base set.
- Packs: a test pack replacing one solid, adding one, disabling one and patching one (the model-fixes pack); with
  the pack off the cooked model equals the base model.

## 7. Cooked form

| Cook unit | Model sections | Zero-copy payloads | Prepared (not normative) |
|---|---|---|---|
| `world/<REGION>` | `solids` (all lists in stream-section load order, first-match merged as today) | `world24` vertex arrays (the engine's `Vertex` is the same 24-byte layout, `crates/ob-formats/src/formats/solid.rs:28-35`) and all index arrays, as `extref`s into the stored `.bin` entries ([cooked.md](cooked.md) §5.1) | GPU buffers; the renderer's merged draw lists |
| `cars/<NAME>` (+ each shared part file) | `solids`; `solid-index` (per list, the solids' hashes: taken from the JSON alone, without reading the `.bin`) | index arrays; `lit36` vertex arrays once the engine reads them strided (today it splits them into `Vertex` + normals: prepared data) | split vertex arrays; the optional smoothing (`render/enh/assets/car_model/smooth.rs`) |
| `scenes/<NAME>` | `skinned_models` | index arrays; `skinned60` vertex arrays once read strided | posed copies |
| `frontend/platforms/<X>` (a garage platform's sub-unit) | `solids` | as world | |
| `global` | `solids` (the lists of `InGameCommon.bun` and `GLOBALB.BUN` / `GlobalB.lzc`; the engine reads `MARKER_GPS_AID`, `src/world/parse.rs:429`) | as world | |

FrontB's list (§1: not read today) is imported into `frontend/meshes/` and cooked with `frontend` only once a reader
uses it. A unit's mesh patches and pack mesh files (§4) are merged into that unit's `solids` entry; since they
change only their own unit's solids (`MSH.16`), no unit's entry depends on another unit's files. A pack's replaced
or added solid is copied into the model entry when it comes from a folder-form pack or a deflated entry (zero-copy
needs a stored bundle entry).

## 8. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| MSH.1 | A mesh file outside the profile (§3.1): not glTF 2.0, more or fewer than one buffer or scene, a data URI, a `.bin` of another stem, `OB_` names in `extensionsRequired`, no `OB_solid_list.schema` or one of another kind, an `OB_` extension in `extensionsUsed` without its `version` in the root `extensions` (or a root entry for one not used), no `OB_Z_UP` root or another rotation, a property the profile excludes. |
| MSH.2 | A child of `OB_Z_UP` without `OB_solid`, or a child of a solid node without `OB_marker`. |
| MSH.3 | Vertex data off its layout: wrong stride, attribute offset, component type or `normalized`; a view not 16-aligned; an attribute missing or extra for the layout. |
| MSH.4 | An index ≥ the vertex count or equal to 65535; an index accessor outside the index view; a count that is 0 or not a multiple of 3 on a primitive. |
| MSH.5 | More than 65,535 vertices in a solid; a vertex count that is not the same for all of a solid's attributes. |
| MSH.6 | Draw ranges and primitives disagree (number, `first_index`/`index_count` vs. the accessor); a `texture_slot` past `textures`; a `light_material` other than 0xFF past `light_materials`; over 255 ranges, textures or light materials. |
| MSH.7 | `POSITION` `min`/`max` that differ from the data. |
| MSH.8 | A hash that is not `bin_hash(node.name)` without `name_truncated`; `name_truncated` with a name not 27 characters long; two solids of one list with the same hash. |
| MSH.9 | A marker node whose matrix is missing, not affine (w column ≠ 0, 0, 0, 1) or not decomposable. |
| MSH.10 | An `extra_chunks` entry with both or neither of `bufferView` and `bytes`, `bytes` over 64, or an id with bit 31 set. |
| MSH.11 | A buffer smaller than its `byteLength`, overlapping views, or a view outside the buffer. |
| MSH.12 | In a pack: a solid node without `op`; `replace` without `target` or with another hash; `add` of a name the stack has (also `MRG.2`). |
| MSH.13 | A mesh-patch guard or `from` value that does not match the stack below; a patched index past the solid's vertices, ranges, markers or triangles. |
| MSH.14 | A layout the unit's consumer cannot draw (`skinned60` outside scenes, `world24` on a car part that needs normals); missing UVs. |
| MSH.15 | An export view (`extras.ob_view`) used as a pack's mesh file. |
| MSH.16 | A mesh-patch target, or a `replace` node's `target`, outside the unit of the file that holds it. |
<!-- /generated -->

## 9. Versioning and extension

- **Versions.** The mesh profile and `OB_solid_list` are versioned by `OB_solid_list.schema` (`ob.mesh/1.0`);
  `OB_solid`, `OB_marker` and `OB_material` each by their own `version` in the root `extensions` (§3.1). All follow
  the kind-schema rules of [versioning.md](versioning.md) §2, and a writer declares the lowest version each part of
  the file needs (§2.1 there). `ob.mesh-patch` is an ordinary TOML kind.
- A minor may add: optional fields for meanings found later (a `u_*` field given its name: the new name is an
  alias, writers write it, readers accept the old `u_*` name forever, a file holding both is refused,
  [versioning.md](versioning.md) §2); `skinned60` additions once scene characters can be added; a `lit36` strided
  read in the cooked form (no format change).
- Extension points: `OB_solid` and `OB_marker` take further fields only by a minor version of that extension;
  third parties use their own glTF extension names on nodes and materials, which readers ignore.
- KTX 2.0 textures are not referenced from glTF materials (glTF has no core way to reference BC-compressed KTX
  files); if a Khronos extension for that appears, materials may use it for viewing only.

## 10. Open points

- Header words `u_0x0c`, `u_0x18`, `u_0x94`, `u_0x98`, `u_0x88`/`u_0x8c`, the per-range boxes, `axes`: **open** or
  **inferred**; carried as fields.
- Chunks 0x134017 (12 bytes, zero in samples), 0x134018 (8-byte records: a hash and four bytes), 0x134019 (12-byte
  records: a hash, a word, a count or offset) on 15,658 car solids, 43.6 MB: **open** [guess: damage or part
  mapping]; carried as `extra_chunks`.
- The list info's `u_0x68`, `u_0x6c`, `u_0x70`: **open**.
- Whether the original game reads FrontB's 15 solids and GLOBALB's / GlobalB's single solids: not traced.
- glTF's "rotation is a unit quaternion": `±0.70710677` as binary32 has a norm within 1e-7 of 1; validators accept
  it [to verify].
- Whether all of a region's solid lists lie inside stream sections (§2.1): [to verify].

## 11. Sizes and timing

Reference install, solid-list chunks only (measured):

| Group | Lists | Solids | Vertices | Original bytes | Vertex bytes | Index bytes |
|---|---|---|---|---|---|---|
| `TRACKS/STREAM*.BUN` | 2,704 | 28,985 | 3,667,007 | 120,774,268 | 88,095,288 | 15,264,924 |
| `CARS/*/GEOMETRY.BIN` | 56 | 18,728 | 4,078,406 | 225,829,540 | 146,818,800 | 19,053,516 |
| `CARS/WHEELS` | 24 | 5,074 | 1,456,807 | 63,412,928 | 52,445,052 | 7,102,800 |
| `NIS` | 140 | 140 | 125,128 | 8,585,044 | 7,507,680 | 955,524 |
| `FRONTEND/PLATFORMS` | 8 | 48 | 44,366 | 1,349,120 | 1,064,784 | 246,444 |
| `FRONTEND/FrontB.lzc` | 1 | 15 | 4,969 | 200,832 | 167,904 | 24,984 |
| `GLOBAL` (4 lists, one a JDLZ twin) | 4 | 30 | 1,878 | 68,816 | 45,072 | 8,016 |
| **total** | **2,937** | **53,020** | **9,378,561** | **420,220,548** | **296,144,580** | **42,656,208** |

Base set: `.bin` files ≈ 382 MB (vertices, indices, 43.6 MB of `extra_chunks`, 16-byte alignment); `.gltf` JSON
≈ 120 MB, ≈ 14 MB deflated (estimated by generating the JSON of every list in the profile's shape). So ≈ 396 MB in
bundles with deflated JSON (≈ 502 MB if JSON entries must be stored), against 420 MB of original chunks; shadow 0.

Import time: reading and copying ≈ 420 MB, writing JSON and running the encoder per solid: ≈ 3–6 s on one core of
a desktop CPU, ≈ 1–2 s in parallel per file [guess]. Loading from the cooked form is unchanged; a cook miss parses
the JSON of a region (≈ 24 MB for L4RA's 10,735 solids, from the measured 2.3 KB a solid) in ≈ 0.1–0.2 s [guess].
