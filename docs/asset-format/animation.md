# Asset format: animation (animation banks, skeletons, part animations, camera tracks, in-engine scenes)

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md), [layering.md](layering.md) and [residual.md](residual.md) apply and are not repeated.

The original animates with EA's EAGL 4 tool library: **animation banks** and **skeletons** are small ELF objects
holding compressed channels. Base sets keep each of them byte for byte in a `.eagl` file (codec passthrough) with a
TOML index, decoded by Overbright's own decoder; new animation (mod packs, a community base set) is written as plain
per-frame key arrays. Camera tracks, part-animation pivots, the in-engine scenes' headers and event tracks become
TOML. glTF 2.0 animation is offered as an export for viewing, never as stored data (§3.9).

## 1. Scope

| Original | Reference install | Kind here | Engine reader today |
|---|---|---|---|
| Animation banks, chunk 0x00E34010, in `TRACKS/L4R?.BUN` (world animations), `NIS/Scene_*_BundleB.bun`, `CARS/<CAR>/PARTS_ANIMATIONS.bin` (29 cars), `GLOBAL/GLOBALB.BUN` and `GlobalB.lzc` | 806 banks (103 / 116 / 585 / 2), 2,328 channels, 5,897,184 bytes | `ob.anim-banks` + `.eagl` | `src/formats/eagl.rs:451` from `src/formats/world_anim.rs:200`, `src/character.rs:501`, `src/shop_scene.rs:166`; `src/formats/zam.rs:200` |
| Skeletons, chunk 0x00E34009, in NIS bundles and the two GLOBAL files | 85 (83 / 2), 452,832 bytes | `ob.anim-banks` + `.eagl` | `src/character.rs:119-139` |
| Part-animation pivots, chunk 0x00037100, in `PARTS_ANIMATIONS.bin` | 29 × 21 matrices, 39,904 bytes | `ob.part-pivots` | `src/formats/zam.rs:168-181` |
| Camera tracks: 0x8003B200 route (`L4RA`, `L4RD`, `L4RG.BUN`), 0x8003B201 generic (`InGameRace`, `InGameDrag`, `InGameDrift`, `InGameFreeRoam.bun`), 0x8003B202 typed (`InGameCommon.bun`; its JDLZ twin `.lzc` holds the same) | 9 chunks, 210 groups, 486 tracks, 1,146 entries, 174,840 bytes (twin counted) | `ob.camera-tracks` | `src/formats/camera_tracks.rs:141`, `src/shop_scene.rs:241`, the replay director (docs/formats/movies-replays.md) |
| Scene headers, chunk 0x80037020 (0x37030 header, 0x37040 characters), one per NIS bundle | 42 scenes, 410 characters, 70,340 bytes | `ob.scene` | `src/shop_scene.rs:149-176`, `src/character.rs:496-511` |
| Event tracks `NIS/Scene_<NAME>_EvtTrk.txt` | 35 files, 452 events, 34,097 bytes | `ob.scene-events` | `src/shop_scene.rs:546`, `src/shop_scene/{wheels,effects,audio}.rs`, `src/render/native/fx/effect_controller.rs:90` |
| The scene table, chunk 0x80037050 in `GLOBAL/InGameCommon.bun` (0x37060, 0x37070) | 1, 22,236 bytes | `ob.scene-table` | `src/shop_scene.rs:373-420` (0x37070 only) |

Not on this page: the world-animation trees and instances (0x37080 / 0x37090) that play bank channels on scenery
and smackables ([world.md](world.md); they reference channels as `anim:`, §2.3); the scenes' skinned models and
props ([meshes.md](meshes.md)) and textures ([textures.md](textures.md)); scene speech (`nisaudio`,
[audio.md](audio.md)); movies ([movies.md](movies.md)); the director, mover and scene-flow logic (engine code); the
scene's world origin (from track markers, [roads-ai.md](roads-ai.md)).

## 2. Files and keys

### 2.1 Files

| Source | Files (`<src>` = the original file name with extension and case) |
|---|---|
| banks and skeletons of a file | `<unit>/anim/<src>.banks.toml` (index) and `<unit>/anim/<src>.<n>.eagl`, *n* counting the file's EAGL objects (banks and skeletons together) from 0 in chunk order; in a scene unit `<src>` is `bundle` |
| part-animation pivots | `cars/<CAR>/anim/PARTS_ANIMATIONS.bin.pivots.toml` |
| camera tracks of a file | `<unit>/anim/<src>.cameras.toml` (one file per camera chunk) |
| a scene | `scenes/<NAME>/scene.toml`, `scenes/<NAME>/events.toml` |
| the scene table | `global/anim/InGameCommon.bun.scene-table.toml` |
| a pack's added banks | `<unit>/anim/<stem>.banks.toml` (any stem); its objects are keyed `<pack id>:<stem>.<n>` (§2.3), so they never share a key with a base bank or another pack's ([layering.md](layering.md) §3.1) |
| authored key arrays | `<unit>/anim/<name>.keys.bin` (any name) |

```text
world/L4RA/anim/L4RA.BUN.banks.toml            the 36 banks of L4RA.BUN, in order
world/L4RA/anim/L4RA.BUN.0.eagl                one bank, the original object byte for byte
world/L4RA/anim/L4RA.BUN.cameras.toml          route cameras (0x8003B200)
cars/240SX/anim/PARTS_ANIMATIONS.bin.banks.toml
cars/240SX/anim/PARTS_ANIMATIONS.bin.0.eagl … .20.eagl
cars/240SX/anim/PARTS_ANIMATIONS.bin.pivots.toml
global/anim/InGameFreeRoam.bun.cameras.toml    generic cameras (the shop scenes' among them)
global/anim/InGameCommon.bun.cameras.toml      typed cameras
global/anim/InGameCommon.bun.scene-table.toml
global/anim/GLOBALB.BUN.banks.toml
scenes/IntroNis08/scene.toml                   header and characters
scenes/IntroNis08/events.toml
scenes/IntroNis08/anim/bundle.banks.toml       its banks and skeletons
scenes/IntroNis08/anim/bundle.0.eagl …
scenes/IntroNis08/meshes/bundle.0.gltf         (meshes.md)
scenes/ShopEnter1/scene.toml
scenes/ShopEnter1/events.toml
scenes/ShopEnter1/anim/bundle.banks.toml
```

### 2.2 Why one unit per scene

A scene loads as one piece (`Scene::load`, `src/shop_scene.rs:143-147`; the crowd's bundle, 
`src/bin/game/flow/scenes.rs:332`), so `scenes/<NAME>/` ([layout.md](layout.md) §2) holds everything of
`NIS/Scene_<NAME>_*`: `scene.toml`, `events.toml`, `anim/`, `meshes/`, `textures/`. `<NAME>` is the original's
(`ShopEnter1`, `IntroNis08`, `EndingNis03`, `GenericBurnout`, …). Seven scenes have no event track; they have no
`events.toml`.

### 2.3 Keys

| Short name | Key | Example | Item |
|---|---|---|---|
| `anim-bank` | `<unit>/<src>.<n>`; a pack's added bank `<unit>/<pack id>:<stem>.<n>` | `anim-bank:world/L4RA/L4RA.BUN.3`, `anim-bank:scenes/ShopEnter1/shop-remix:car1-path.0` | one EAGL object (bank or skeleton), or one authored bank |
| `anim` (item key) | `<bank key>/<NAME>` | `anim:cars/240SX/PARTS_ANIMATIONS.bin.0/240SX_ZAM_HOOD_REG_q` | one channel |
| `anim` (scoped reference) | `<NAME>`, or `0xHASH` = `bin_hash` of the name (or of the name without `_q` / `_t` / `_s` where the consumer groups channels so) | `anim:ZPM_Car1_t`, `anim:0x5165B588` | the channel the consumer finds in the referencing unit |
| `skeleton` (scoped) | `<NAME>` (from `__Skeleton:::<NAME>`) or `0xHASH` | `skeleton:Bip11` | one skeleton of the unit |
| `part-pivots` | `<CAR>` | `part-pivots:240SX` | a car's 21 pivots |
| `camera-group` (item key) | `<unit>/<src>/<index>` | `camera-group:global/InGameFreeRoam.bun/12` | one group (chunk 0x3B210), by position |
| `camera-group` (scoped) | `<kind>/0xKEY` | `camera-group:generic/0xE39FF628` | the first group of that kind and key, as the camera manager finds it |
| `camera-track` | `<group item key>/<index>` | `camera-track:global/InGameFreeRoam.bun/12/0` | one track |
| `scene` | `<NAME>` (or `0xHASH` = `bin_hash(NAME)`) | `scene:ShopEnter1` | one scene unit |
| scene character, scene event | (scene, index) | – | one record / one event line |

- **Lookup order is data.** Consumers search a unit's channels across all its banks in bank order and decide
  differently on repeats: world animations keep the last channel of a name (`src/formats/world_anim.rs:200-215`),
  the shop and race scenes the last (`src/shop_scene.rs:166-178`), the scene characters the first
  (`src/character.rs:501-507`, `posed`). So the order of objects and of channels in each object is kept
  ([conventions.md](conventions.md) §6.5), and replacements keep the replaced channel's place (§4.1).
- Channel names are the original's, case-sensitive, printable ASCII without spaces, 1–127 bytes (bone names inside
  skeletons have spaces, `Bip11 Spine1`; they are not keys).

## 3. Source form

### 3.1 `.eagl` (EAGL object, passthrough)

A `.eagl` file is the body of chunk 0x00E34010 or 0x00E34009 from its first 16-aligned file offset to the chunk's
end (the bytes before it are `0x11` fill in all 891 chunks), byte for byte: an ELF32 little-endian relocatable
object (System V ABI, https://refspecs.linuxfoundation.org/elf/gabi4+/contents.html) written by EA's tool library
(symbol `__EAGL_TOOLLIB_VERSION:::EAGL_TOOLLIB_VERSION-4`). Sections: `.data`, `.shstrtab`, `.strtab`,
`.symtab`, and in most objects `.rel.data`; relocated words are `.data` offsets. Two NIS objects carry 40 bytes past
the ELF's last section, kept as they are. What the decoder reads (`src/formats/eagl.rs`, `src/character.rs`,
`src/formats/zam.rs`; tags as there):

| Structure | Where | Layout | Tag |
|---|---|---|---|
| Bank | symbol `__AnimationBank:::bank` | +0x04 channel count, +0x10 → channel pointers, +0x14 → name pointers (`src/formats/eagl.rs:451-484`) | verified |
| Skeleton | symbol `__Skeleton:::<name>` | +0x00 hash, +0x08 bone count, +0x10 bones of 0x70 bytes: scale xyz, parent `i32` (−1 root), rotation xyzw, translation xyz, the bone's index, inverse bind matrix (row-vector 4×4) (`src/character.rs:119-139`) | verified |
| Compound, type 0x0F | channel pointer | +0x08 child count, +0x0A frame count, +0x0C child pointers; children evaluated last first, the earlier child wins a slot (`eagl.rs:151-189`) | verified (+0x04 inferred) |
| Delta stream, types 0x0A (scalars) / 0x0B (quaternions) | child | +0x04 → stream (count, bits 4/8/16, per component offset/scale/initial, rows of quanta), +0x08 frame count, +0x0A pose slots (`eagl.rs:201-272`) | verified |
| Block-keyed, types 0x15 (scalars) / 0x14 (vec3) | child | +0x04 → slots, +0x08 → key times or 0, +0x0C keys, +0x0E items, +0x10 log₂ keys a block, +0x11 constants; per-component min/range/delta tables, blocks of u16 + u8 deltas, then constants (`eagl.rs:274-415`) | verified (constant alignment inferred) |
| ZAM key block | compound child (type 0) of a part-animation bank | +0x08 components per key (4 `_q`, 3 `_t`), +0x0C keys, raw f32 keys from +0x20, 30 a second (`zam.rs:1-23`, `:200-248`) | verified |
| Types 0x00, 0x02, 0x11 | children in 22 L4RB banks, 42 NIS banks (`XO_Spotlight…`), the 2 GLOBAL banks | not decoded: the engine's decoder fails on them and the consumer gets **no channel of that object** (`parse_bank(...).unwrap_or_default()`, `src/character.rs:502`; `src/formats/world_anim.rs:201`) | open |

**Why passthrough** ([conventions.md](conventions.md) §4.6 rule 3; compressed animation curves are media
bitstreams, and the rule is applied per kind, over all its items): the decoded frames of the NIS banks would be
24,300,152 bytes against 4,801,328 stored (5.06×; computed from the channels' frame counts and widths on the
reference install), and of all 806 banks ≈ 24.8 MB against 5,897,184 bytes (≈ 4.2×). EAGL's quantised streams
cannot be rebuilt from decoded floats, so a decoded base would also keep the original as a shadow (≈ 5.2× in all).
Kept as they are, the objects need no shadow, the engine's values are equal by construction (the same decoder reads
the same bytes, including the objects it rejects today), and a later decoder for types 0x00, 0x02 and 0x11 needs no
re-import. The track and part-animation banks alone would stay under the threshold if decoded (≈ 0.62× and 0.42×,
≈ 1.62× and 1.42× with the shadow), but one kind has one representation: one decoder, and no ELF writer for Eq2.
glTF animation is not an alternative: a glTF sampler interpolates rotations by slerp (glTF 2.0 Appendix C) where
the engine interpolates components linearly without normalising (`eagl.rs:83-112`) or normalises a linear blend
(`zam.rs:89-116`), and its key times are floats, not the original's integer frames.

### 3.2 Animation-bank index (`ob.anim-banks/1.0`)

One file per original file (base sets), or per bank file of a pack (`<stem>.banks.toml`: each `[[object]]` is a new
bank keyed `<pack id>:<stem>.<n>`, §2.3, defined only by that pack; its channels carry their own `op`, §4.1).
Schema: [`schemas/anim-banks.schema.json`](schemas/anim-banks.schema.json).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.anim-banks/1.0` |
| `source` | string | – | no | – | The original file name (base sets; informative). |
| `object` | array<table> | – | yes | – | The EAGL objects in original order. |
| `object.kind` | enum | – | yes | – | `bank`, `skeleton`. |
| `object.encoding` | enum | – | yes | – | `eagl`: an original object kept whole in `file`. `keys`: an authored bank (channels with `values`, §3.3). `bones`: an authored skeleton (§3.3). |
| `object.file` | string | – | with `eagl` | – | The `.eagl` file, relative to this file's folder. |
| `object.digest` | digest | – | no | – | `xxh3-128` of `file`; writers SHOULD set it in base sets. |
| `object.decodes` | bool | – | no | `true` | `false`: the object holds a channel type this version has no decoder for; the engine reads nothing from it (as today). Set by the importer. |
| `object.name` | string | – | skeletons | – | The skeleton's name (`Bip11`). |
| `object.bones` | u16 | – | no | – | `eagl` skeletons: the bone count (informative, checked). |
| `object.channel` | array<table> | – | banks | – | The bank's channels in bank order. For `eagl` objects an index the importer derives (checked, `ANI.2`); for `keys` banks the data. |
| `channel.name` | string | – | yes | – | The channel's name. |
| `channel.op` | enum | – | no | – | Packs: `add`, `replace`, `merge` ([layering.md](layering.md) §3.1; §4.1). |
| `channel.disabled` | bool | – | no | – | §4.1. |
| `channel.kinds` | array<u16> | – | no | – | `eagl`: the sub-channel types (informative). |
| `channel.slots` | array<u16> | – | `keys` | – | Pose float indices the channel writes, one per value of a key (the EAGL consumers; scale 12–14 / quaternion 16–19 / translation 20–22 in the world banks, 12 floats a bone in the character banks, **inferred**, `eagl.rs:25-31`). |
| `channel.frames` | u16 | frame (1/30 s) | `keys` | – | The channel's frame count (compound +0x0A; ZAM: header +0x0A, the part animation's length). `eagl`: informative. |
| `channel.width` | u8 | – | `keys` | – | Values per key: 1–64 (ZAM: 4 for `_q`, 3 for `_t`). |
| `channel.values` | array descriptor | – | `keys` | – | Layout `anim_f32` (§3.3): keys × `width` floats, keys ≥ 1. |

For an authored skeleton (`encoding = "bones"`), `object.bone` is an array in bone order:

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `name` | string | – | no | – | The bone's name (informative). |
| `parent` | i32 | – | yes | – | Parent bone index, −1 for the root; less than the bone's own index. |
| `scale` | vec3 | – | no | `[1.0, 1.0, 1.0]` | Bind pose. |
| `rotation` | vec4 | – | no | `[0.0, 0.0, 0.0, 1.0]` | Bind pose quaternion x, y, z, w (stored as given; the engine normalises). |
| `translation` | vec3 | m | no | `[0.0, 0.0, 0.0]` | Bind pose. |
| `inverse_bind_axes` | mat3 | – | yes | – | Rows of the inverse bind matrix (row vectors, [conventions.md](conventions.md) §5.3). |
| `inverse_bind_translation` | vec3 | m | yes | – | Its translation row. |

Example (an excerpt of a region's index, one bank kept whole; and a pack's bank file
`scenes/ShopEnter1/anim/car1-path.banks.toml`, whose bank is `anim-bank:scenes/ShopEnter1/shop-remix:car1-path.0`
in the pack `shop-remix`, replacing one channel):

```toml
schema = "ob.anim-banks/1.0"
source = "L4RA.BUN"

# … objects 0 to 2 …
[[object]]
kind = "bank"
encoding = "eagl"
file = "L4RA.BUN.3.eagl"
digest = "xxh3-128:0f4c2a91b7e3d5c86a1b2c3d4e5f6071"

[[object.channel]]
name = "ANM_DoorAnim_RB_00_XO_DoorPieceA_1a_RB_00_q"
kinds = [0x0B]
slots = [16, 17, 18, 19]
frames = 61
```

```toml
schema = "ob.anim-banks/1.0"

[[object]]
kind = "bank"
encoding = "keys"

[[object.channel]]
name = "ZPM_Car1_t"
op = "replace"
slots = [20, 21, 22]
frames = 71
width = 3
values = { file = "car1-path.keys.bin", offset = 0, count = 213, layout = "anim_f32" }
```

### 3.3 Authored keys (`anim_f32`) and how the engine reads them

`anim_f32`: `f32` values, row-major, `width` per key, 4-aligned; no header. A `keys` channel gives each consumer the
same model an EAGL channel gives it:

- EAGL consumers (world animations, scenes, characters): `Channel { kinds: [], slots, frame_count: frames,
  frames: rows }` with row *f* = key *f*, and keys past the last repeated up to `frames` rows (the clamp of
  `eagl.rs:259-270`). Sampling between rows is the engine's (`Channel::sample`, `eagl.rs:92-129`).
- The part-animation consumer: `(width, values, frames)` as `zam.rs:200-248` returns it.

So an authored channel is exact at every integer frame; what happens between frames is the original's own
interpolation, for base and authored channels alike.

### 3.4 Part-animation pivots (`ob.part-pivots/1.0`)

Chunk 0x37100 (`src/formats/zam.rs:17-23`, `:168-181`): which rest matrix pairs with which animation is fixed by
the engine (`zam::NAMES`). Schema: [`schemas/part-pivots.schema.json`](schemas/part-pivots.schema.json).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.part-pivots/1.0` |
| `car_hash` | hash32 | – | yes | – | +0x04: `bin_hash` of the car's name (all 29). |
| `u_0x00` | u32 | – | no | `0x0000000C` | **open** (12 in the 240SX's). |
| `u_0x08`, `u_0x0c` | u32 | – | no | `0` | **open**; zero. |
| `pivot` | array<table> | – | yes | – | Exactly 21, in order: hood REG, FRONT, LEFT / RIGHT / FRONT / BACK_HOOD_SPLIT, left door REG / SCISSOR / SUICIDE, right door REG / SCISSOR / SUICIDE, trunk REG, left door WIDE1–4, right door WIDE1–4. |
| `pivot.label` | string | – | no | – | Informative (`HOOD_REG`). |
| `pivot.axes` | mat3 | – | yes | – | Rows +0x00, +0x10, +0x20 of the 4×4 (car space). |
| `pivot.translation` | vec3 | m | yes | – | Row +0x30 (the hinge). |
| `pivot.u_w` | vec4 | – | no | `[0.0, 0.0, 0.0, 1.0]` | The matrix's w column (+0x0C, +0x1C, +0x2C, +0x3C); (0, 0, 0, 1) in every pivot. |

Layout of the chunk body, after `0x11` fill to its first 16-aligned file offset:

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `u_0x00` | – | |
| 0x04 | 4 | `u32` | `car_hash` | – | |
| 0x08 | 4 | `u32` | `u_0x08` | – | |
| 0x0C | 4 | `u32` | `u_0x0c` | – | |
| 0x10 | 0x540 | `pivot[21]` | `pivot` | – | in the order of the field table |

Size 0x550, alignment 16. `pivot` (a 4×4 matrix, row vectors, translation last):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 12 | `f32[3]` | `axes` row 0 | – | |
| 0x0C | 4 | `f32` | `u_w[0]` | – | |
| 0x10 | 12 | `f32[3]` | `axes` row 1 | – | |
| 0x1C | 4 | `f32` | `u_w[1]` | – | |
| 0x20 | 12 | `f32[3]` | `axes` row 2 | – | |
| 0x2C | 4 | `f32` | `u_w[2]` | – | |
| 0x30 | 12 | `f32[3]` | `translation` | m | |
| 0x3C | 4 | `f32` | `u_w[3]` | – | |

Size 0x40, alignment 16.

### 3.5 Camera tracks (`ob.camera-tracks/1.0`)

One file per camera chunk (docs/formats/movies-replays.md "Replays"; `src/formats/camera_tracks.rs`). Schema:
[`schemas/camera-tracks.schema.json`](schemas/camera-tracks.schema.json).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.camera-tracks/1.0` |
| `kind` | enum | – | yes | – | `route` (0x8003B200, keyed by road section and direction, ≤ 3,000 groups), `generic` (0x8003B201, by id, ≤ 256), `typed` (0x8003B202, by camera type, ≤ 100). |
| `group` | array<table> | – | yes | – | Chunks 0x3B210 in order. |
| `group.key` | u32 | – | yes | – | Route: road section \| direction bit << 16; generic: an id (a scene's header hash for scene cameras); typed: a camera type. A field like the others: groups are keyed by position (`index`, §4), not by `key`. |
| `group.index`, `group.op`, `group.disabled` | – | – | no | – | Packs (§4). |
| `group.track` | array<table> | – | yes | – | The tracks in order (`count` derived). |
| `track.index`, `track.op`, `track.disabled` | – | – | no | – | Packs (§4). |
| `track.name` | string | – | yes | – | +0x12, ≤ 13 characters (`Track 0`); scenes pick `Track %d` by name. |
| `track.name_tail` | bytes | – | no | zeros | Bytes after the name's NUL up to +0x1F (stale bytes in 6 tracks). |
| `track.duration_s` | f32 | s | yes | – | +0x0C: a typed track's run time. |
| `track.u_0x00`, `track.u_0x04`, `track.u_0x08` | u32 | – | no | `0` | The runtime link words, which the file holds stale (tool pointers). **open** as data. |
| `track.u_0x11` | u8 | – | no | `0` | **open**; zero. |
| `track.entry` | array<table> | – | yes | – | The 0x88-byte entries in order (count derived, ≤ 255). |
| `entry.index`, `entry.op` | – | – | no | – | Packs (§4). |
| `entry.mover` | u8 | – | yes | – | +0x00: camera type (0, 0x12, 0x13 occur; above 1 makes a track camera). verified |
| `entry.blend` | u8 | – | no | `0` | +0x01: blend from the previous shot (non-zero = on). |
| `entry.spline` | u8 | – | no | `0` | +0x02: Hermite eye / look paths. |
| `entry.u_0x03` | u8 | – | no | `0` | +0x03: **open** (0 / 1). |
| `entry.eye_space`, `entry.look_space` | u8 | – | yes | – | +0x04 / +0x05: 0 car frame, 1 world, 2 world axes at the car, 3 scene frame. |
| `entry.avoid` | array<u8> (2) | – | no | `[0, 0]` | +0x06 / +0x07: the collision pushes. |
| `entry.frozen_frame` | u8 | – | no | `0` | +0x08. |
| `entry.shake` | u8 | – | no | `0` | +0x09. |
| `entry.u_0x0a` | u16 | – | no | `0` | **open**; zero. |
| `entry.start` | f32 | fraction | yes | – | +0x0C: start fraction along the track. |
| `entry.tangent` | vec2 | – | yes | – | +0x10 / +0x14. |
| `entry.eye`, `entry.look` | array<vec3> (2) | m, in the entry's space | yes | – | +0x18 / +0x24, +0x30 / +0x3C: start and end points. |
| `entry.fov` | vec2 | – | yes | – | +0x48 / +0x4C, start and end (units **open**: `camera_tracks.rs:86-88` treats them as a focal length in mm, docs/formats/movies-replays.md as degrees). |
| `entry.params` | array<vec2> (4) | – | yes | – | +0x50..+0x6F: four start / end pairs (depth of field, shake). |
| `entry.tail` | bytes (24) | – | no | zeros | +0x70..+0x87: **open** (the engine keeps them raw, `camera_tracks.rs:92-93`). |

The group body (chunk 0x3B210) is a head, then per track a 0x20-byte header and its entries, back to back:

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `group.key` | – | |
| 0x04 | 4 | `u32` | track count | – | derived: the number of `track` elements |

Size 8. Track header:

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `track.u_0x00` | – | run-time link words, stale in the files |
| 0x04 | 4 | `u32` | `track.u_0x04` | – | |
| 0x08 | 4 | `u32` | `track.u_0x08` | – | |
| 0x0C | 4 | `f32` | `track.duration_s` | s | |
| 0x10 | 1 | `u8` | entry count | – | derived: the number of `entry` elements, ≤ 255 (`camera_tracks.rs:9-11`) |
| 0x11 | 1 | `u8` | `track.u_0x11` | – | |
| 0x12 | 14 | `char[14]` | `track.name` | – | ≤ 13 characters, NUL, then `track.name_tail` (zeros when absent) |

Size 0x20. The entry (0x88 bytes) is the `entry` rows above, whose offsets cover it completely (+0x00 to +0x87).
Groups, tracks and entries follow each other without padding (alignment 1).
Example: [`examples/animation/InGameFreeRoam.bun.cameras.toml`](examples/animation/InGameFreeRoam.bun.cameras.toml).

### 3.6 Scene (`ob.scene/1.0`)

The header chunk 0x37030 and the character records 0x37040 of a scene bundle (docs/formats/front-end.md "Shops",
`src/shop_scene.rs:15-21`, `src/character.rs:13-24`). Schema: [`schemas/scene.schema.json`](schemas/scene.schema.json).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.scene/1.0` |
| `name` | string | – | yes | – | +0x04 (`char[16]`): the scene's name, 1–15 characters `[A-Za-z0-9_]`; the unit's `<NAME>`. |
| `hash` | hash32 | – | no | `bin_hash(name)` | +0x00 (equal in all 42): the key of the scene's camera group. |
| `type` | u32 | – | yes | – | +0x18: 0 race start (and DemoFlyIn), 1 race ending, 2 dyno and the four `Generic*`, 3 shop entry, 4 shop exit and CareerNis1 (as stored; meanings verified for 0, 2, 3, 4 in docs/formats/front-end.md, 1 [inferred] from the four endings). |
| `u_0x1c` | u32 | – | no | `1` | **open** (1 in 32 scenes, 0 in 10). |
| `u_0x20` | u32 | – | no | `1` | **open**. |
| `cars` | u32 | – | yes | – | +0x24: the scene's car count. |
| `length_frames` | u32 | frame (1/30 s) | yes | – | +0x28. |
| `marks_frames` | array<u32> (2) | frame | yes | – | +0x2C / +0x30: the race start's two frames (the cut at +0x30, `src/shop_scene.rs:215-230`). |
| `description` | string | – | no | `""` | +0x34, NUL-terminated, printable ASCII, ≤ 200 characters (60 at most in the reference install). |
| `character` | array<table> | – | no | `[]` | The 0x37040 records in order. |
| `character.name` | hash32 | – | yes | – | +0x04 (render info +0xD0). |
| `character.flags` | u32 | – | yes | – | +0x0C: 0x20 loop, 0x40 loop over `loop_range_frames`, 0x10 back and forth. |
| `character.axes` | mat3 | – | yes | – | +0x10, +0x20, +0x30: rows of its matrix in scene space. |
| `character.translation` | vec3 | m | yes | – | +0x40. |
| `character.solids` | array (3) | – | yes | – | +0x50..+0x58: each a scoped reference `solid:<NAME or 0xHASH>` into the scene's solid lists ([meshes.md](meshes.md)), or the bare `"0x00000000"` for an empty slot ([conventions.md](conventions.md) §6.6; no empty slot in the reference install). |
| `character.channels` | array<hash32> (3) | – | no | zeros | +0x60..+0x68: bare hashes copied from the record ([conventions.md](conventions.md) §6.6): channels of the scene's banks, resolved as `anim:` references (§2.3), for unskinned objects; `0x00000000` = none (zero in all 410). |
| `character.anim` | ref `anim` | – | yes | – | +0x6C: scoped, `anim:0xHASH` of the bank entry's name. |
| `character.skeleton` | ref `skeleton` | – | yes | – | +0x70: scoped, `skeleton:0xHASH`. |
| `character.rate` | f32 | – | no | `1.0` | +0x74: play rate (0 means 1). |
| `character.delay_s` | f32 | s | no | `0.0` | +0x78. |
| `character.loop_range_frames` | array<u32> (2) | frame | no | `[0, 0]` | +0x80 / +0x84. |
| `character.u_0x00` | u32 | – | no | `1` | **open**; 1 in all. |
| `character.u_0x08`, `u_0x5c`, `u_0x7c`, `u_0x88`, `u_0x8c` | u32 | – | no | `0` | **open**; zero in all. |
| `character.u_w` | vec4 | – | no | `[0.0, 0.0, 0.0, 1.0]` | The matrix's w column; (0, 0, 0, 1) in all. |

Header (the body of chunk 0x37030; checked on all 42):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `hash` | – | `bin_hash(name)` in all 42 |
| 0x04 | 16 | `char[16]` | `name` | – | NUL-terminated, the rest zero (all 42); the longest is 14 characters (`GenericBurnout`) |
| 0x14 | 4 | `u32` | run-time | – | 0 in all 42 files; the loader stores the description's address there (0x437790: header +0x14 = header + 0x34, **verified**). Not a field: the encoder writes 0 |
| 0x18 | 4 | `u32` | `type` | – | |
| 0x1C | 4 | `u32` | `u_0x1c` | – | |
| 0x20 | 4 | `u32` | `u_0x20` | – | |
| 0x24 | 4 | `u32` | `cars` | – | |
| 0x28 | 4 | `u32` | `length_frames` | frame | |
| 0x2C | 8 | `u32[2]` | `marks_frames` | frame | |
| 0x34 | *n* | `char[n]` | `description` | – | the text, its NUL, then zero bytes up to the next multiple of 4 |

Size `align4(0x34 + len(description) + 1)` (all 42), alignment 4.

Character record (the body of each chunk 0x37040: `0x11` fill up to the body's first 16-aligned file offset, 8
bytes in all 410, then one record):

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x00 | 4 | `u32` | `u_0x00` | – | |
| 0x04 | 4 | `u32` | `name` | – | |
| 0x08 | 4 | `u32` | `u_0x08` | – | |
| 0x0C | 4 | `u32` | `flags` | – | |
| 0x10 | 12 | `f32[3]` | `axes` row 0 | – | the 4×4 matrix's rows are row vectors |
| 0x1C | 4 | `f32` | `u_w[0]` | – | |
| 0x20 | 12 | `f32[3]` | `axes` row 1 | – | |
| 0x2C | 4 | `f32` | `u_w[1]` | – | |
| 0x30 | 12 | `f32[3]` | `axes` row 2 | – | |
| 0x3C | 4 | `f32` | `u_w[2]` | – | |
| 0x40 | 12 | `f32[3]` | `translation` | m | |
| 0x4C | 4 | `f32` | `u_w[3]` | – | |
| 0x50 | 12 | `u32[3]` | `solids` | – | hashes; 0 = empty slot |
| 0x5C | 4 | `u32` | `u_0x5c` | – | |
| 0x60 | 12 | `u32[3]` | `channels` | – | |
| 0x6C | 4 | `u32` | `anim` | – | |
| 0x70 | 4 | `u32` | `skeleton` | – | |
| 0x74 | 4 | `f32` | `rate` | – | |
| 0x78 | 4 | `f32` | `delay_s` | s | |
| 0x7C | 4 | `u32` | `u_0x7c` | – | |
| 0x80 | 8 | `u32[2]` | `loop_range_frames` | frame | |
| 0x88 | 4 | `u32` | `u_0x88` | – | |
| 0x8C | 4 | `u32` | `u_0x8c` | – | |

Size 0x90, alignment 16.

### 3.7 Scene events (`ob.scene-events/1.0`)

The event track, a text file the original parses at run time (0x432D00). Schema:
[`schemas/scene-events.schema.json`](schemas/scene-events.schema.json).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.scene-events/1.0` |
| `event` | array<table> | – | yes | – | The track's `EVENT:` lines in file order. |
| `event.frame` | `token` | frame (1/30 s) | yes | – | The line's second word. |
| `event.kind` | string | – | yes | – | The third word, as written: any word of 1–200 characters without whitespace (always a string, whatever its form). The reference install's tracks use `CAR_AUDIO_PATTERN`, `SEQ_CAR_NITRO`, `CAR_TRIGGER_EFFECT`, `SCREEN_FLASH`, `CAR_HYDRAULIC_EXTEND`, `CAR_HYDRAULIC_BOUNCE`, `SEQ_CAR_BURNOUT`, `WORLD_MESSAGE`, `SEQ_CAR_TORQUE`, `CAR_BURNOUT_ADDITIVE`, `CAR_BRAKE_LOCK`; the engine also reads `CAR_ENGINE_TORQUE`. Unknown kinds are kept and ignored, as the original's consumers ignore them. |
| `event.args` | array<`token`> | – | no | `[]` | The remaining words. At most 16. |
| `event.index`, `event.op`, `event.disabled` | – | – | no | – | Packs (§4). |

**The type `token`** (this page only): a TOML integer, a TOML float, or a string of 1–200 characters without
whitespace that matches neither number form below. Which of the three a word becomes is fixed by the token rule.

**Tokens.** The importer cuts each line at its first `//`, splits it at whitespace, and makes an event of every
line whose first word is `EVENT:`. Each word becomes a TOML value: an **integer** if it matches `-?[0-9]+` (and fits
`i64`); a **float** if it matches `-?([0-9]+\.?[0-9]*|\.[0-9]+)f?` (its value is the word without the `f` parsed as
binary32, written as [conventions.md](conventions.md) §3.4 requires); otherwise the **string** as written. A reader
hands the consumers the same words back: an integer as its decimal text, a float as its binary32 value, a string as
is. The consumers parse as they parse the text today (`trim_end_matches('f').parse::<f32>()` for numbers,
`parse::<usize>()` for car slots), so a float token is not a slot, an integer token is a number, and `NaN` stays a
string the float parse rejects. A string token in a number form is refused (`ANI.7`): the importer would have made
it a number, so it cannot come back as written. Comment lines (`CAR_AUDIO_REV` occurs only in such lines), blank
lines, inline comments, tabs and line ends are not kept (the residual's shadow keeps the text, §6).

```toml
schema = "ob.scene-events/1.0"

[[event]]
frame = 0.0
kind = "WORLD_MESSAGE"
args = ["DOOR_POSITION", 0.0, 0.0]

[[event]]
frame = 21.0
kind = "WORLD_MESSAGE"
args = ["DOOR_POSITION", 1.0, 140.0]

[[event]]
frame = 35.0
kind = "CAR_AUDIO_PATTERN"
args = [1, 3, 1.0]
```

### 3.8 Scene table (`ob.scene-table/1.0`)

Which race start and ending scenes the game may pick (chunk 0x37070 in `GLOBAL/InGameCommon.bun`, `IntroTable`,
`src/shop_scene.rs:296-420`): the fields the engine reads. The chunk's other bytes and its sibling 0x37060 (no
reader) stay in the residual (§5). Schema: [`schemas/scene-table.schema.json`](schemas/scene-table.schema.json).

| Field | Type | Req. | Meaning |
|---|---|---|---|
| `schema` | string | yes | `ob.scene-table/1.0` |
| `race_start` | array<table> | yes | +0x1C, 12-byte records, count +0x00: `scene` (ref), `variant` u8 (+4, the camera track "Track %d"), `flags` u8 (+5: 0x40 and the grid slots a human may start from), `min_racers`, `max_racers` u8 (+6, +7), `start_kind` u8 (+8). |
| `ending` | array<table> | yes | +0x25C, 8-byte records, count +0x04: `scene`, `variant` u8, `flags` u8, `rest` (bytes 2). |
| `multiplayer_start` | array<table> | yes | +0x2BC, 0x14-byte records, count +0x0C: `scene`, `flags`, `min_racers`, `max_racers`, `start_kind` u8 (+5…+8), `slot_cars` (4 × u8, +0x0C). |
| `track_list` | array of 2 arrays of tables | yes | +0x37C (count +0x14) and +0x91C (count +0x18), 12-byte records: `track` u16 (+0), `scene_mask` u16 (+2), `entries_low` u32 (+4), `entries_high` u16 (+8), `endings` u16 (+10). |

`scene` is `scene:<NAME>` when the hash is a known scene's, else `scene:0xHASH`.

### 3.9 glTF exports (views)

`overbright-studio-cli anim export --gltf` writes viewing copies for Blender and other tools: a scene's characters
as skinned meshes with their skeletons (glTF skins, vertices duplicated per draw range, [meshes.md](meshes.md)
§4.6) and their animations, the `ZPM_Car<n>` paths and part animations (about their pivots) as node animations, all
sampled at every frame (input = frame / 30 s, `LINEAR`), camera tracks as animated cameras [guess: sampled with the
engine's own camera code]. Exports carry `extras.ob_view = true` and are never read as stored data.
`anim import --gltf` turns an edited export back into `keys` channels (exact at the sampled frames).

## 4. Layering

| Item | merge | add | replace | disabled | Clash key |
|---|---|---|---|---|---|
| channel | `disabled` only | yes: `keys`, a new name, in a pack index of the unit | yes: `keys` for a named channel | yes | (unit, channel name) |
| EAGL object | refused | yes (a pack's `keys` bank, keyed `<pack id>:<stem>.<n>`) | refused | refused | – |
| skeleton | refused | refused (needs new scenes) | refused | refused | – |
| part pivots | yes (`pivot` keyed by index) | refused | – | – | (car, index) |
| camera group | yes (by `index`; `key` and the fields replaced, `track` keyed by `index`) | yes (appended; its `key` must be new in the file) | yes | yes: the group is absent | (file, index) |
| camera track | yes (fields; `entry` keyed by index) | yes (appended; a new `name`) | yes | yes | (group, index) |
| scene | yes (header fields; `character` keyed by index) | refused (`MRG.3`; needs `add` on the scene table) | refused | yes | `scene:<NAME>` |
| scene event | yes (keyed by index) | yes (appended) | – | yes | (scene, index) |
| scene-table entry | yes (keyed by list and index) | refused | – | refused | (list, index) |

No wildcard units. Refused operations fail with `MRG.3`.

**Keyed lists in packs.** A pack's file names base elements by their position: `index` (u32) on `group`, `track`,
`entry`, `pivot`, `character`, `event` and the scene-table lists, counted in the **base set's** list (base
elements are never removed, only disabled); an element a pack added is addressed as `"<pack id>:<n>"` (its position
among that pack's own additions), only by that pack and packs that depend on it ([layering.md](layering.md) §3.2);
`op = "add"` appends where the table allows it; an element with neither is refused (`MRG.4`).
Channels are keyed by `name` within the unit. Base files hold complete elements and no `index`.

### 4.1 Channel overrides

- A replaced channel takes the base channel's place (its bank, its index), so every consumer's first- or
  last-match lookup finds the replacement exactly where it found the original. Added channels go into a bank after
  all of the unit's banks, lowest layer first. A disabled channel is left out of its bank's decoded list; lookups
  then fall through to the next match or to nothing, which each consumer already handles (a shop scene without
  `ZPM_Car1_t` does not play, `src/shop_scene.rs:189`; a part animation without channels stays shut,
  `src/formats/zam.rs:128-131`).
- An override of a channel in an object with `decodes = false` is refused (`ANI.8`): the engine has no channels of
  it to replace.
- A replacement's `slots` and `width` may differ from the original's; the consumer reads what it finds.

### 4.2 Scenes

- `disabled = true` on a scene makes the engine behave as when the scene's files are missing: the shop opens its
  front end at once (`src/shop_scene.rs:12-14`), a race starts without its start scene, the ending plays none
  [to verify for the race paths against the original's missing-scene handling].
- Character and event merges keep order; added events are appended and each consumer sorts its own events by frame
  as it does today (`src/shop_scene/effects.rs:77`, `src/shop_scene/audio.rs:51`), door events stay in file order.

## 5. Import

| Source | Decoder | Output | Residual |
|---|---|---|---|
| 0x00E34010 / 0x00E34009 chunks | chunk walk; `eagl::parse_bank` and `Skeleton::parse` for the index; a tolerant listing for objects the decoder rejects | `.eagl` per object, `<src>.banks.toml` per file | `decoded`, proof `bytes` (chunk = header + `0x11` fill + the `.eagl` bytes) |
| 0x00037100 | `zam.rs:168` | `PARTS_ANIMATIONS.bin.pivots.toml` | `decoded`, `bytes` |
| 0x8003B20x | `parse_camera_tracks` | `<src>.cameras.toml` | a `container` node; its 0x3B210 groups one run node (`count`), `decoded`, `bytes` (the encoder writes each group's head, tracks and entries) |
| 0x80037020 | `Scene::parse`, `SceneCharacters::parse` | `scene.toml` | a `container` node; 0x37030 and each 0x37040 `decoded`, `bytes`; the id-0 padding chunks between them (26 in the 42 scenes, zero, 0–12 bytes) `fill` |
| `Scene_*_EvtTrk.txt` | the token rule of §3.7 | `events.toml` | `text`: one `decoded` node, proof `values`, kept (shadow) |
| 0x80037050 | `IntroTable::parse` | `InGameCommon.bun.scene-table.toml` | a `container` node; 0x37070 `decoded`, `values`, kept (shadow); 0x37060 `kept` (no decoder) |

`InGameCommon.lzc` (JDLZ) holds the same content as `InGameCommon.bun` (measured); its items are imported once and
both residual maps refer to them ([residual.md](residual.md) §3.1.1). The scene name comes from the file name and
must equal the header's (`ANI.6`). A header whose run-time word +0x14 is not zero (none in the reference install)
cannot be rebuilt by the encoder alone: its node is `patched` ([residual.md](residual.md) §3.2).

## 6. Equality and verification

| Kind | Level | Proof at import | Shadow |
|---|---|---|---|
| EAGL objects | Eq2 (the bytes are the original's) | the `.eagl` digest = the digest of the chunk's aligned body; Eq1 follows (same decoder, same bytes): `H(parse_bank(original)) == H(parse_bank(.eagl))` is checked anyway | 0 |
| bank index | derived | regenerated from the objects and compared (`ANI.2`) | – |
| part pivots | Eq2 | encoder: fill, 0x10-byte head, 21 matrices | 0 |
| camera tracks | Eq2 | encoder: per group key, count, then per track the 0x20-byte header and 0x88-byte entries | 0 |
| scene header, characters | Eq2 | encoder (§3.6 layouts): the header from its fields, +0x14 written 0, zero-filled to `align4(0x34 + len + 1)`; each record after `0x11` fill to its 16-aligned offset; checked against all 42 headers and 410 records | 0 |
| event tracks | Eq1 | for each of the five consumers (doors, wheels, effects, audio patterns, flashes) `H(parse(text)) == H(parse(tokens))` | 34,097 bytes |
| scene table | Eq1 | `H(IntroTable::parse(original))` = `H(from TOML)` | 3,772 bytes (0x37070); 0x37060 is `kept` (18,440 bytes, no decoder), not a shadow |

Total shadow: 37,869 bytes (an event-track encoder is not worth having: the text's whitespace and comments are the
file); kept without a decoder: 18,440 bytes (0x37060). Gates ([parity.md](parity.md) §3): the shots that show scenes
(shop entry and exit, race starts with the crowd, endings, the career start), the world-animation doors, the part
animations in the garage, the replay cameras; the audio captures of scenes (event-driven engine patterns); the
playthrough. Pack tests: one channel replaced and one disabled in a shop scene and in a car's part animations, one
camera entry merged, one event added; with the pack off every model equals the base model.

## 7. Cooked form

| Cook unit | Model sections | Zero-copy | Prepared (not normative) |
|---|---|---|---|
| `world/<REGION>` | `world_anim_channels` (the decoded `Channel`s by name, as the region's world animations group them), `route_cameras` | – (`.eagl` objects are small; copied) | – |
| `cars/<NAME>` | `part_animations` (pivots, animations by name) | – | – |
| `scenes/<NAME>` | `scene` (header, characters, each consumer's event list), `characters` (skeletons, decoded channels) | – | posed matrices |
| `global` | `cameras/<src>` (groups per file), `scene_table` | – | – |

Decoded channels are model data (what Eq1 compares). Scenes are small (the largest bank decodes to 300,888 floats,
`src/formats/reader.rs:65`); caching them is optional, decoding on load takes milliseconds [guess].

## 8. Errors

| Rule | Text |
|---|---|
| ANI.1 | An `.eagl` file that is not an ELF32 little-endian relocatable object, lacks the `__AnimationBank` (banks) or `__Skeleton:::` (skeletons) symbol, or whose digest differs from `digest`. |
| ANI.2 | A bank index that differs from its object: channel count, names or order; a skeleton name or bone count. |
| ANI.3 | A `keys` channel whose `values` count is not a positive multiple of `width`, whose `width` is 0 or over 64, whose `slots` length differs from `width`, or whose `frames` is 0. |
| ANI.4 | A bank over the decode limit (4,194,304 values, `src/formats/reader.rs:94`), or a channel name outside §2.3. |
| ANI.5 | Camera tracks over the limits: more groups than the kind allows (3,000 / 256 / 100), more than 255 entries in a track, a name over 13 characters; a group a pack adds with a `key` the merged file already has. |
| ANI.6 | A scene whose `name` differs from its unit's `<NAME>`, is over 15 characters or outside `[A-Za-z0-9_]`, or whose `hash` is not `bin_hash(name)`; a description over 200 characters or outside printable ASCII. |
| ANI.7 | An event with more than 16 arguments, a token outside the three forms (a string token in an integer or float form included), or a float token that is not finite. |
| ANI.8 | An override of a channel in an object with `decodes = false`. |
| ANI.9 | An authored skeleton with a parent not below its own index, more than 255 bones, or no root. |
| ANI.10 | Pivots that are not exactly 21, or a `car_hash` that is not `bin_hash` of the unit's car name. |

## 9. Versioning and extension

- 1.x may add: decoders for channel types 0x00, 0x02, 0x11 (no format change: `decodes` flips to `true` at the
  next import epoch, [versioning.md](versioning.md) §4.2); `add` of scenes together with `add` on the scene table
  (new race starts, endings, shop scenes) and of skeletons; names for `u_*` fields once traced, as aliases
  ([versioning.md](versioning.md) §2).
- Extension point: `[ext.OB_*]` tables on scene characters and events (e.g. an Overbright-only event kind), none
  defined. An event kind the engine does not know is already carried and ignored.

## 10. Open points

- Channel types 0x00 and 0x02 (22 L4RB world banks) and 0x11 (the NIS `XO_Spotlight…` banks, the GLOBAL banks):
  **open**; carried, not decoded; what the original does with them is not traced.
- The NIS time base: two scene frames a key is **inferred** (`src/shop_scene.rs:192-212`); the format stores
  frames as the original does and leaves the rate to the engine.
- Camera `fov` units, `u_0x03`, the entry tail, the track link words: **open**.
- Scene header `u_0x1c`, `u_0x20`, type 1 = ending: **open** / **inferred**.
- Chunk 0x37060 (18,440 bytes, next to the scene table): **open**, kept.
- Event kinds with no consumer in the engine today (`CAR_HYDRAULIC_*`, `SEQ_CAR_TORQUE`): carried.
- Whether the GLOBAL skeletons and banks are used by the original: not traced.

## 11. Sizes and timing

| Kind | Original | Base set | Shadow | Kept (no decoder) |
|---|---|---|---|---|
| banks: NIS / parts / tracks / GLOBAL | 4,801,328 / 866,160 / 226,880 / 2,816 | ≈ 5.88 MB `.eagl` + ≈ 0.25 MB index TOML [guess] | 0 | 0 |
| skeletons | 452,832 | ≈ 0.45 MB `.eagl` | 0 | 0 |
| part pivots | 39,904 | ≈ 0.15 MB TOML [guess] | 0 | 0 |
| camera tracks | 174,840 (incl. the twin) | ≈ 0.6 MB TOML [guess: ≈ 450 bytes an entry] | 0 | 0 |
| scene headers | 70,340 | ≈ 0.25 MB TOML [guess] | 0 | 0 |
| event tracks | 34,097 | ≈ 40 KB TOML | 34,097 | 0 |
| scene table | 22,236 | ≈ 10 KB TOML | 3,772 (0x37070) | 18,440 (0x37060) |
| **total** | **≈ 6.69 MB** | **≈ 7.6 MB** (TOML deflates ≈ 5–10×) | **37,869** | **18,440** |

Import time: well under 1 s (copying 6.4 MB, decoding the banks once for the index and the Eq1 check) [guess].
