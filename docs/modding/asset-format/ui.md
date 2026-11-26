# Asset format: menus (FEng packages), fonts, HUD and front-end scenes

Status: **draft 1 — design, not implemented yet.** Part of the [Overbright asset format](README.md); rules from [conventions.md](conventions.md),
[layout.md](layout.md), [layering.md](layering.md) and [residual.md](residual.md) apply and are not repeated.
Rule prefix: `UI`.

The original's menus and HUDs are **FEng packages**: trees of objects (images, strings, groups, movies) with
keyframed scripts and message responses, stored as tagged binary and mostly JDLZ- or HUFF-compressed. The format
keeps each package as one TOML object tree with every field, in the original object order, with the original ids
and hashes; scripts, events and responses are plain data. Values the original stores twice or can compute (sizes,
counts, type tables, name offsets) are not stored: a small encoder writes them, and the import proves that the
encoder gives back every package byte for byte. Fonts are glyph tables in TOML whose glyph pages are textures
([textures.md](textures.md)).

Schemas: [`fe-package`](schemas/fe-package.schema.json), [`font`](schemas/font.schema.json). Examples:
[`examples/ui/`](examples/ui/).

## 1. Scope

| Original | Reference install | Kind here | Engine reader today |
|---|---|---|---|
| FEng packages: chunk 0x30203 (plain) and 0x30210 (u32 hash, then a JDLZ or HUFF block) in `FRONTEND/FrontB.lzc` (137), `GLOBAL/GlobalB.lzc` (19), `GLOBAL/GLOBALA.BUN` (2), `GLOBAL/InGameCommon.lzc` (27), `GLOBAL/InGameRace.bun` (13), `InGameFreeRoam.bun` (1), `InGameDrag.bun` (14), `InGameDrift.bun` (14), `InGamePerformanceTuning.bun` (9) | 236 chunks (215 JDLZ, 6 HUFF, 15 plain), 206 distinct names; the 30 repeated copies are byte-identical (measured) | `ob.fe-package` | `src/frontend/feng/parse.rs:193` |
| Fonts: chunk 0x30201 in `GLOBAL/GLOBALA.BUN` (1) and in every `LANGUAGES/<Lang>.bin` (6 or 7 each) | 87 chunks, 377,872 bytes, 15 distinct fonts | `ob.font` | `src/frontend/feng/font.rs:55` |
| The front-end asset set: which files the engine merges, in which order | 12 files (`asset_files(language)`: 11 shared, then the active language's text file) | rules, §3.3 | `src/frontend/assets.rs` |
| HUD: packages, element tables, custom-gauge pictures | 4 single-player HUD packages, 8 split-screen ones, `HUD_World_Loading` | §3.4 (no kind of its own) | `src/frontend/hud/mod.rs:50` |
| Garage platforms `FRONTEND/PLATFORMS/*.BIN`, environment maps `FRONTEND/ENVMAPS/*.bin` | 8 + 2 files | units, §3.5 | `src/garage.rs:570`, `crates/ob-formats/src/formats/magazines.rs:88` |

`GLOBAL/InGameCommon.bun` holds exactly the unpacked content of `InGameCommon.lzc` (measured) and is not read by
Overbright; its nodes in the residual map name the same items.

Not on this page:

- Every texture the menus and the HUD use (TPK chunks 0xB3300000 in the files above, `GLOBAL/HUD_CustomTextures_*.bin`,
  `LANGUAGES/LanguageTextures.bin` with the font glyph pages, the environment faces): [textures.md](textures.md).
- Solids in `FrontB.lzc`, `InGameCommon.*` and the platforms (0x80134000): [meshes.md](meshes.md). Camera tracks
  (0x8003B201, 0x8003B202) and the NIS start table (0x80037050): [animation.md](animation.md). Magazines
  (0x30230): [career.md](career.md). The track minimaps (`TRACKS/TRACKMAP*.BIN`): their pictures are
  [textures.md](textures.md)'s, their calibration (map edges, zoom, World Map window) is [career.md](career.md)'s
  (`ob.track-maps`).
- Data no kind owns yet, kept byte for byte in the residual ([README.md](README.md) §3, "Not yet owned by a
  kind"): the collision-audio stitch records (0x8003B500), the named car light setups (0x3B700 in `FrontB.lzc`,
  `src/render/native/car/lighting.rs:40`) and the particle effect descriptions (0x35021) in `Showroom.BIN`.
  `Showroom.BIN`'s placed emitters (0x35020, `src/particles/mod.rs`) are kept with them until a page takes them:
  [world.md](world.md) §3.4.6 decodes the emitters of regions only.
- Strings, credits and every other text: [text.md](text.md). Menu behaviour (which screen does what): engine code
  (docs/formats/front-end.md).

## 2. Files and keys

```text
frontend/
  ui/packages/<NAME>.toml          ob.fe-package   one per package name (all 206, whichever file held them)
  ui/fonts/<name>.toml             ob.font         fonts shared by every language that holds them, and GLOBALA's
  textures/…                       textures.md     menu texture sets (FrontB, LanguageTextures)
  platforms/<NAME>/meshes/…        meshes.md       one garage platform (§3.5)
  platforms/<NAME>/textures/…      textures.md     its textures, its environment faces XPOS … ZNEG
  envmaps/<NAME>/textures/…        textures.md     one environment map's six faces
global/
  textures/…                       textures.md     GlobalB, GLOBALA, InGame*, HUD_CustomTextures_* sets
text/<language>/
  fonts/<name>.toml                ob.font         fonts only that language holds (text.md §2)
```

`<NAME>` of a package is its stored name without `.fng` (`UI_Main.fng` → `UI_Main.toml`); all shipped names match
`[A-Za-z0-9_]{1,35}\.fng` and none differ only in case (measured). Fonts keep their stored lower-case names.

| Key | Item | Notes |
|---|---|---|
| `fe-package:<NAME>` | a package | `NAME` without `.fng`, case as stored; the engine's package map uses the same key (`src/frontend/assets.rs:134`). |
| `fe-object:<NAME>/0xID` | an object of a package | `ID` = the object's `id` (`OP` word 0) as 8 upper-case hex digits. Used by references and by the layering extension point (§4). |
| `font:<name>` | a font | Matched without case, by `fe_hash` of the name (`crates/ob-formats/src/formats/hash.rs:158`), as the engine looks fonts up (`src/frontend/assets.rs:218`). Resolution order: the active language's `fonts/`, then `frontend/ui/fonts/` (§3.3). |
| `fe-platform:<NAME>` | the unit `frontend/platforms/<NAME>/` | `NAME` = the original file's stem (`Showroom`); the engine's table names (`SHOWROOM`, `src/garage.rs:52`) match it without case, as the original (choice: format rule; the original's layout is in docs/formats/)'s file lookup does (a file-name key, conventions §6.3, `CONV.13`). |
| `fe-envmap:<NAME>` | the unit `frontend/envmaps/<NAME>/` | Named by magazine records (`crates/ob-formats/src/formats/magazines.rs:77`). |

Object name hashes repeat inside 209 of the 236 package chunks (library instances share names); the `id` never
repeats within a package (measured), so it is the key.

## 3. Source form

### 3.1 Menu package (`ob.fe-package/1.0`)

**The original** (docs/formats/front-end.md "Front end", verified except noted): a tree of four-character tags
(`tag`, u32 size, body) whose `ObjD`, `Scrp`, `MsgR` and `PkgR` bodies are two-character tags (`tag`, u16 size,
body). Every shipped package follows one tag order (measured on all 236; the encoder writes it):

```text
FEn\xE7  PkHd  TypS  Res\xCC{ RsNm RsRq }  Obj\xCC{ Butn  FOb\xEA* }  [PkgR]  Targ
FOb\xEA  ObjD{ Ot Oh OP [PA] [If] [Sb St Sj Sl Sw SH] SA }  Scrp{ Sh [Sc] (FI To Kd)* [EV] }*  [MsgR]
MsgR, PkgR:  ( Mi MC (Ri Ru Rt)* )*
```

**Mapping.** Every tag and word maps to a field, a documented constant, or a derived value:

| Original | Field | Stored? |
|---|---|---|
| `FEn\xE7`, `Res\xCC`, `Obj\xCC`, `FOb\xEA` tag bytes | – | constants |
| `PkHd` words 0, 1 | – | constants 0x00020000, 0 |
| `PkHd` word 2 | – | derived: resource count |
| `PkHd` word 3 | `u_0x0c` | stored (open) |
| `PkHd` words 4, 5 | – | derived: name and path lengths including their NUL |
| `PkHd` +0x18 | `name`, `path` | stored; NUL-terminated, then zero bytes to a multiple of 4 (derived) |
| `TypS` | – | derived: (kind code, state bytes) for each object kind, in the order of each kind's first object |
| `RsNm` | `resource[].name` | stored; NUL-terminated in resource order, zero bytes to a multiple of 4 (derived) |
| `RsRq` | `resource[]` | count and name offsets derived; per entry `hash`, `kind`; three u32 words that are 0 (constant) |
| `Butn` | – | derived: one u32, the number of objects whose flags have bit 0x10000000 |
| `FOb\xEA` | `object[]` | stored, in file order |
| `Ot`, `Oh`, `OP`, `PA`, `If`, `SA` | `kind`, `name_hash`, (`id`, –, `flags`, `resource`), `parent`, `image_flags`, `state` | stored; `OP` word 1 is derived (= `Oh`) |
| `Sb`, `St`, `Sj`, `Sl`, `Sw`, `SH` | `text.sb`, `text.text`, `text.justify`, `text.leading`, `text.wrap_width`, `text.label_hash` | stored; `St` is UTF-16LE, NUL-terminated, zero bytes to a multiple of 4 (derived) |
| `Sh` | `name_hash`, `length_ms`, `flags` | stored; word 3 (track count) derived |
| `Sc`, `EV` | `chain`, `events` | stored |
| `FI`, `To`, `Kd` | `param`, (value size), `interpolation`, `mode`, `length_ms`; `target_word`; `base`, `keys` | stored; the value size is derived (4 × the number of `base` values); `Kd`'s first time word is the constant 0xFFFFFFFF |
| `Mi`, `MC`, `Ri`, `Ru`, `Rt` | `message`, –, `entries[i][0..3]` | stored; `MC` (entry count) derived |
| `Targ` | `targ` | stored (open) |

**Top level**

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.fe-package/1.0` |
| `name` | string `[A-Za-z0-9_]{1,59}\.fng` | – | yes | – | The package's name as stored (`PkHd`). The file name is `name` without `.fng` (`UI.1`). |
| `path` | string ≤ 200, printable Latin-1 | – | no | `""` | The authoring path the original stores after the name. Not read by the engine. |
| `u_0x0c` | u32 | – | no | 0 | `PkHd` word 3. **Open.** |
| `targ` | array<u32> | – | yes | – | The `Targ` tag's words. **Open**; the first word is 0x00046354 in every shipped package. |
| `package_responses` | array<node> | – | no | absent | The `PkgR` message nodes (present in 208 packages). Absent = no `PkgR` tag. |
| `resource` | array<table> | – | no | `[]` | The resource requests; an object's `resource` is an index into this list. |
| `object` | array<table> | – | no | `[]` | The objects in file order. Order is data: the renderer and the message pump walk it (docs/formats/front-end.md "Layout"). |
| `extras` | table | – | no | – | conventions §9 |

**`resource[]`**

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `name` | string ≤ 255, printable ASCII | – | yes | – | The file name as authored (`MENU_BACK.tga`, `ConduitMdITC_TT21i.ffn`). Textures and fonts are found by `upper_hash` of the name without its extension (`src/frontend/assets.rs:212`, `:218`). |
| `kind` | enum | – | yes | – | `untextured` (0), `texture` (1), `font` (2), `movie` (4, **inferred** from the `.avi` names). |
| `hash` | hash32 | – | yes | – | `RsRq` word 0. For `untextured` it is `bin_hash` of the upper-case name; for the other kinds the derivation is **open** (228 of 2,978 match). Not read by Overbright. |

**`object[]`**

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `id` | u32 | – | yes | – | `OP` word 0; the object's key (`fe-object:`). Events and focus refer to objects by it. |
| `name_hash` | hash32 | – | yes | – | `Oh`: `fe_hash` of the object's name (the screens find objects by it, `crates/ob-formats/src/formats/hash.rs:158`). |
| `name` | string ≤ 63, printable ASCII | – | no | – | The name when known (Overbright's name list, `src/frontend/fe_ids.rs`); `fe_hash(name)` MUST equal `name_hash` (`UI.10`). Informational. |
| `kind` | enum | – | yes | – | `Ot`: `image` 1, `string` 2, `model` 3, `list` 4, `group` 5, `code_list` 6, `movie` 7, `effect` 8, `colored_image` 9, `anim_image` 10, `simple_image` 11, `multi_image` 12. |
| `flags` | u32 | – | no | 0 | `OP` word 2. Bit 0 hidden at run time, 0x10000000 a button, 0x100000 a library instance, bit 31 a skipped group (docs/formats/front-end.md "Object"). |
| `resource` | u32 | – | yes | – | `OP` word 3: index into `resource` (every shipped object has one; groups point at the untextured request). |
| `parent` | u32 | – | no | absent | `PA`: the `id` of the group this object hangs from. Absent = no `PA` tag (a top-level object). Every shipped parent is an earlier `group` (measured). |
| `image_flags` | u32 | – | no | absent | `If`. Absent = no `If` tag. |
| `state` | table | – | yes | – | `SA`, the animatable state (below). |
| `text` | table | – | iff `kind = "string"` | – | The string tags (below). |
| `responses` | array<node> | – | no | absent | `MsgR` (at most one per object, measured). Absent = no `MsgR` tag. |
| `script` | array<table> | – | no | `[]` | `Scrp` tags in order. |
| `extras` | table | – | no | – | conventions §9 |

**`state`** (`SA`: 17 words; 21 with `uv`; 37 with `uv` and `corner_colors_bgra`; scripts address it by word
offset). All objects of one kind have the same state size (`UI.5`).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `color_bgra` | array<i32>[4] | – | yes | – | Words 0–3: blue, green, red, alpha as integers, in the stored order. |
| `pivot` | vec3 | canvas px | yes | – | Words 4–6. |
| `position` | vec3 | canvas px | yes | – | Words 7–9 (the 640 × 480 canvas; z orders the draw). |
| `rotation` | vec4 | – | yes | – | Words 10–13, a quaternion x, y, z, w. |
| `size` | vec3 | – | yes | – | Words 14–16. |
| `uv` | vec4 | – | no | absent | Words 17–20: the texture rectangle (upper-left u, v, lower-right u, v). |
| `corner_colors_bgra` | array<array<i32>[4]>[4] | – | no | absent | Words 21–36: four corner colours of a coloured image. Requires `uv`. |

**Exception to conventions §5.3 and §5.4: values the original animates keep its own representation.** The state
is the block of words that scripts animate: a track names the first word it drives (`target_word`) and adds keyed
deltas to `base` word by word (colour tracks with negative integer steps, rotation tracks with quaternion
components). A `color` string or a 3×3 matrix would not be addressable by word, and converting a track's base and
deltas into them would not be exact in both directions (a matrix has no unique quaternion sign, a colour string has
no negative steps). So, normatively, and only for these fields:

- `color_bgra` and `corner_colors_bgra` are the stored `i32` words in the stored order (blue, green, red, alpha),
  not `color` strings; `rotation` is the stored quaternion (x, y, z, w), not a `mat3`. The same holds for the
  `base` and `delta` values of tracks (`track[]` below).
- Writers MUST write them as the original (choice: format rule; the original's layout is in docs/formats/)'s words; readers MUST NOT convert them before the scripts run, and
  refuse any other form (a `color` string, a matrix, Euler angles) with `UI.16`. Tools MAY show them as
  `#RRGGBBAA` colours and angles.

**`text`** (string objects)

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `text` | string ≤ 4,096 | – | yes | – | `St`, the text as authored (UTF-16 in the original; no shipped text has a surrogate), written as a basic string with escapes (conventions §3.2). The engine draws the screen's own text, else the label's string, else this text (`src/frontend/feng/layout/mod.rs:285`; docs/formats/front-end.md "Layout" for the original's flag). |
| `label_hash` | hash32 | – | yes | – | `SH`: the label looked up in the active language ([text.md](text.md) §3.8). |
| `label` | string `[A-Za-z0-9_]{1,63}` | – | no | – | The label name when known (from the labels table, [text.md](text.md) §2). |
| `justify` | u32 | – | no | 0 | `Sj` (`src/frontend/feng/text.rs:74`): 1 centres lines on the origin, 2 ends them there, 4 centres the block vertically, 8 ends it there. |
| `leading` | i32 | px | no | 0 | `Sl`. |
| `wrap_width` | u32 | px | no | 0 | `Sw`. |
| `sb` | u32 | – | yes | – | `Sb`. **Open**: values 0x10–0x1000, which look like a text buffer size [guess]; not read by Overbright. |

**`script[]`**

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `name_hash` | hash32 | – | yes | – | `Sh` word 0, `bin_hash` of the name (`INIT` = `0x001744B3`). Unique per object (measured): the script's key. |
| `name` | string ≤ 63 | – | no | – | The name when known; `fe_hash(name)` MUST equal `name_hash`. |
| `length_ms` | u32 | ms (package clock) | yes | – | `Sh` word 1. The package clock runs 1.2 × real time (`src/frontend/feng/script.rs:30`). |
| `flags` | u32 | – | no | 0 | `Sh` word 2; `flags & 3` is the end mode (0 hold, 1 loop, 2 ping-pong). |
| `chain` | hash32 | – | no | absent | `Sc`: the script of the same object that runs when this one ends. Absent = no `Sc` tag. |
| `events` | array<event> | – | no | absent | `EV`, in order. Absent = no `EV` tag. |
| `track` | array<table> | – | no | `[]` | `FI`, `To`, `Kd` triples in order. |

**`track[]`**

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `param` | u8 | – | yes | – | `FI` byte 0, the value type: 1 integer, 2 float, 3 vec2, 4 vec3, 5 quaternion (x, y, z, w), 6 colour (B, G, R, A integers). Shipped: 3, 4, 5, 6. |
| `interpolation` | u8 | – | no | 1 | `FI` byte 2: 0 step, 1 linear, 3 linear re-based at start (one shipped track). |
| `mode` | u8 | – | no | 0 | `FI` byte 3: bits 0–6 the loop mode (0 clamp, 1 loop, 2 ping-pong), bit 7 "done" (set on 5,028 key-less tracks). |
| `length_ms` | u32 < 2²⁴ | ms | yes | – | `FI` bytes 4–7 (24 bits read; the high byte is 0 in every shipped track). |
| `target_word` | u32 ≤ 36 | – | yes | – | `To`: the first `state` word the track drives. |
| `base` | array<number>[1..4] | – | yes | – | The value at time −1 (`Kd`'s first key). |
| `keys` | array<key> | – | no | `[]` | `{ time_ms = u32, delta = [..] }`: deltas from `base`, in stored order; `delta` has as many values as `base`. |

Value typing (`UI.7`): for `param` 1 and 6 every value is an `i32` (colour steps may be negative); for 2–5 an
`f32` (conventions §3.4: shortest round-trip decimal, `-0.0` kept: 1,373 shipped values are negative zero); any
other `param` stores the words as `u32`.

**`event`**: `{ message = hash32, target = i32, time_ms = u32 }` (`EV`, 12 bytes). `target` 0 broadcasts; −1
calls the screen's code; −4, −5, −6 pick other routes; any other value is an object `id` (docs/formats/front-end.md
"Scripts").

**`node`** (`responses`, `package_responses`): `{ message = hash32, entries = [[type, arg1, arg2], …] }`, each
entry three u32 (`Ri`, `Ru`, `Rt`). Messages are unique per list (measured). The entry types are documented in
docs/formats/front-end.md "Message responses".

**Limits** (beyond conventions §10): ≤ 65,535 objects (shipped maximum 375), ≤ 4,096 resources, ≤ 1,024 scripts per
object and tracks per script; every two-character tag MUST encode to ≤ 65,535 bytes (`UI.9`: a track's keys,
a script's events); a package file ≤ 16 MiB (the largest shipped one is ≈ 1.2 MB as TOML).

**Example** (shortened from [`examples/ui/frontend/ui/packages/UI_DemoDialog.toml`](examples/ui/frontend/ui/packages/UI_DemoDialog.toml)):

```toml
schema = "ob.fe-package/1.0"
name = "UI_DemoDialog.fng"
path = "fe\\ui_demodialog.fng"
targ = [0x00046354, 0]
package_responses = [
  { message = "0xBDFD2DF9", entries = [[0x00000001, 0x35F8620B, 0x00000000]] },
]

[[resource]]
name = "DEMO_BACKING.tga"
kind = "texture"
hash = "0x7A0C51E2"

[[object]]
id = 2
name_hash = "0xB94E5A33"
name = "DemoBacking"
kind = "image"
resource = 1
parent = 1
image_flags = 0x00000000

[object.state]
color_bgra = [255, 255, 255, 0]
pivot = [0.0, 0.0, 0.0]
position = [0.0, -12.5, 10.0]
rotation = [0.0, 0.0, 0.0, 1.0]
size = [400.0, 160.0, 1.0]
uv = [0.0, 0.0, 1.0, 0.625]

[[object.script]]
name_hash = "0x5B0D9106"
name = "FADEIN"
length_ms = 200
events = [{ message = "0x35F8620B", target = -1, time_ms = 200 }]

[[object.script.track]]
param = 6
length_ms = 200
target_word = 0
base = [255, 255, 255, 0]
keys = [
  { time_ms = 0, delta = [0, 0, 0, 0] },
  { time_ms = 200, delta = [0, 0, 0, 255] },
]
```

### 3.2 Font (`ob.font/1.0`)

**The original** chunk 0x30201 (verified against the renderer: 0x51BDC0, 0x51BFC0, 0x6F45F0; docs/formats/front-end.md
"Fonts"; the layout of the open bytes measured on all 87 chunks). `FNTF` offsets are counted from the `FNTF`
magic, as the docs count them, and name the `u_0xNN` fields.

| Offset | Size | Type | Field | Unit | Notes |
|---|---|---|---|---|---|
| 0x000 | 256 | char[256] | name block | – | NUL-separated: `name`, then the `source` strings; zero-filled (derived) |
| 0x100 | 256 | char[256] | texture block | – | `texture`, then the `texture_source` strings; zero-filled (derived) |
| 0x200 | 4 | char[4] | magic | – | `FNTF` |
| 0x204 | 4 | u32 | `u_0x04` | – | open |
| 0x208 | 2 | u16 | `u_0x08` | – | open (0x019E or 0x016E) |
| 0x20A | 2 | u16 | glyph count | – | derived |
| 0x20C | 4 | u32 | `flags` | – | bit 0x40000: 16-byte glyphs (set in every shipped font; required, `UI.11`) |
| 0x210 | 2 | u16 | `u_0x10` | – | open (0) |
| 0x212 | 2 | u16 | `u_0x12` | – | open; not the line height |
| 0x214 | 4 | u32 | glyph table offset | – | 0x80 (constant) |
| 0x218 | 4 | u32 | kerning table offset | – | 0x80 + 16 × glyphs when any glyph has pairs, else 0 (derived) |
| 0x21C | 4 | u32 | `FNTF` size | bytes | from the magic to the chunk's end (derived) |
| 0x220 | 4 | u32 | `u_0x20` | – | open (0xFFFFFFFF) |
| 0x224 | 4 | i32 | `line_height_px` | px | |
| 0x228 | 4 | u32 | `u_0x28` | – | open (0) |
| 0x22C | 4 | f32 | `u_0x2c` | – | open (1.0) |
| 0x230 | 4 | u32 | `u_0x30` | – | open: the `FNTF` offset of a 32-byte record in the tail, or the bits of 0.5 |
| 0x234 | 4 | u32 | `u_0x34` | – | open: 0x7F, or the bits of 0.5 |
| 0x238 | 4 | f32 | `u_0x38` | – | open (1.0) |
| 0x23C | 4 | f32 | `u_0x3c` | – | open (1.0) |
| 0x240 | 4 | f32 | `inv_texture_width` | 1/px | 1 / the glyph page's width |
| 0x244 | 4 | f32 | `inv_texture_height` | 1/px | 1 / its height |
| 0x248 | 4 | u32 | `u_0x48` | – | open (1) |
| 0x24C | 52 | u32[13] | `u_0x4c` | – | open (zero, except the word at `FNTF`+0x5C = 1 in most fonts) |
| 0x280 | 16·n | glyph[n] | `glyph` | – | ascending `code` |
| … | 4 | u32 | pair count | – | present iff any glyph has pairs (derived) |
| … | 4·k | pair[k] | `glyph[].kerning` | – | grouped per glyph, in glyph order (derived offsets) |
| … | rest | u8[] | `tail` | – | open; 0–144 bytes shipped |

Glyph (16 bytes, alignment 2): 0x0 u16 `code`; 0x2 u8 `width_px`; 0x3 u8 `height_px`; 0x4 u16 `x_px`; 0x6 u16
`y_px`; 0x8 u8 `pad` (0 in all 10,156 shipped glyphs); 0x9 i8 `x_offset_px`; 0xA i8 `y_offset_px`; 0xB u8 pair
count (derived); 0xC u16 first pair (derived: the running sum); 0xE i16 `advance_px`. Kerning pair (4 bytes):
u16 previous character, i8 adjust (px), u8 the low byte of the owning glyph's code (derived; true for all 3,233
shipped pairs).

| Field | Type | Unit | Req. | Default | Meaning |
|---|---|---|---|---|---|
| `schema` | string | – | yes | – | `ob.font/1.0` |
| `op` | enum | – | no | `merge` | layering.md §3.1 (packs) |
| `name` | name | – | yes | – | The font's name (`conduitmditc_tt14i`); the key `font:<name>`. |
| `source`, `texture_source` | array<string> | – | no | `[]` | The further strings of the two blocks (`bin` and a cut authoring file name; `bmp` and another). **Open**: not read. |
| `texture` | name | – | yes | – | The glyph page's texture name; it resolves to `texture:` + `upper_hash(texture)` ([textures.md](textures.md); the shipped pages are in `LANGUAGES/LanguageTextures.bin`). |
| `line_height_px` | i32 | px | yes | – | Read by the renderer (0x51BDC0). |
| `flags` | u32 | – | no | 0x00040009 | See the layout. |
| `inv_texture_width`, `inv_texture_height` | f32 | 1/px | yes | – | The reader takes 1 / value, 256 when not positive (`src/frontend/feng/font.rs:65`). |
| `u_0x04` … `u_0x48` | as the layout | – | no | the shipped constant shown in the layout, else 0 | Open words. |
| `u_0x4c` | array<u32>[13] | – | no | zeros | Open words. |
| `tail` | bytes | – | no | `"hex:"` | Open. |
| `glyph` | array<table> | – | yes | – | Keyed by `code` (u16, a UTF-16 code unit). Fields `width_px`, `height_px` (u8), `x_px`, `y_px` (u16), `x_offset_px`, `y_offset_px` (i8, default 0), `advance_px` (i16), `kerning` (array of `[previous u16, adjust_px i8]`, default `[]`). |

"Req." applies to a font that a base set holds or a pack adds (`op = "add"`); a `merge` entry holds only the
fields it changes (layering.md §3.1), so the schema requires only `schema` and `name`.

Example: [`examples/ui/frontend/ui/fonts/demo_font.toml`](examples/ui/frontend/ui/fonts/demo_font.toml).

### 3.3 The front-end asset set (normative for readers)

The engine builds one front-end asset set at start (`src/frontend/assets.rs:83`). With the base set alone it MUST
get exactly what it gets from the original files today:

1. **Packages**: every `fe-package` item, keyed by name without `.fng`. Today the first file in `asset_files` (`frontend::assets`)
   order wins a name; all duplicates are byte-identical, so the base set holds one copy (`UI.15` at import).
2. **Textures**: the UI texture sets of [textures.md](textures.md) in the original file order (`FrontB.lzc`,
   `GlobalB.lzc`, `GLOBALA.BUN`, `InGameCommon.lzc`, the four `InGame*.bun`, `InGamePerformanceTuning.bun`,
   `HUD_CustomTextures_ALL.bin`, `LanguageTextures.bin`): the first set holding a name wins, except that an in-game
   set's copy that differs in blend, size or pixels is kept under the alias `rotl(hash, 16) ^ 0x1A6E5EED`
   (`src/frontend/assets.rs:18`, `:145`). The set order and the in-game flag are therefore data the texture sets
   MUST keep.
3. **Fonts**: the shared fonts, then the active language's fonts; a language font replaces a shared one with the
   same `fe_hash` (today `English.bin`'s fonts are read after `GLOBALA.BUN`'s, `src/frontend/assets.rs:162`).
4. **Strings**: the active language's table with the fallback of [text.md](text.md) §3.8.
5. **Resources** resolve by `upper_hash` of the name without extension: textures among the merged sets, fonts
   among the merged fonts. A font that resolves to nothing falls back to *any* loaded font in the engine today
   (`src/frontend/assets.rs:220`); see §10 and the risks.

### 3.4 The HUD: what is data and what is engine

| Part | Where | Notes |
|---|---|---|
| Every HUD object's position, size, colour, rotation, texture rectangle, show/hide/flash scripts, label and font | FEng packages (this page): `HUD_SingleRace` (`InGameRace`, `InGameFreeRoam`), `HUD_Short_Track` (`InGameRace`), `HUD_Drag`, `HUD_Drift`, `2PHudTop`/`2PHudBot` and their `_DRAG`, `_DRIFT`, `_OUTRUN` forms, `HUD_World_Loading` | Native positions are package data, not code. |
| Pictures: gauges, needles, the custom-gauge sets | texture sets ([textures.md](textures.md)): the in-game sets and `GLOBAL/HUD_CustomTextures_00…10`, `_ALL` | |
| Which package a race mode uses | engine code: `src/frontend/hud/mod.rs:56` | |
| Which objects make up each HUD element and its feature mask; screen offsets such as the custom-gauge highlight's (21, 130) | engine code: `src/frontend/hud/elements.rs:53`, `src/frontend/custom_hud.rs:33` (constants traced from the executable; [engine-tables.md](engine-tables.md)) | |
| The minimap's calibration per track | career data ([career.md](career.md)) | The HUD mod changes these (§4). |
| Placement on screens wider than 4:3 | engine code: `src/frontend/enh/widescreen.rs:38` | An Overbright rule; nothing moves at 4:3. |

HUD layout as data with anchors is an extension point (§9), not part of 1.0.

### 3.5 Garage platforms and environment maps

Front-end data whose items are stored by other pages. Each is a sub-unit of `frontend/` that the engine loads
whole, as the original (choice: format rule; the original's layout is in docs/formats/) loads the whole file:

| Unit | Original | Bytes | Contents (measured) | Read by |
|---|---|---|---|---|
| `frontend/platforms/AudioShop01/` | `FRONTEND/PLATFORMS/AudioShop01.BIN` | 3,080,192 | 6 solids, 2 texture packs (26 textures) | `src/garage.rs:570` |
| `…/CarLot01/` | `CarLot01.BIN` | 3,797,632 | 6 solids, 2 packs (32) | same |
| `…/Crib01/` | `Crib01.BIN` | 3,161,088 | 6 solids, 2 packs (23) | same |
| `…/MegaloShop01/` | `MegaloShop01.BIN` | 3,760,640 | 6 solids, 2 packs (28) | same |
| `…/PaintShop01/` | `PaintShop01.BIN` | 3,042,304 | 6 solids, 2 packs (29) | same |
| `…/PartsShop01/` | `PartsShop01.BIN` | 4,075,520 | 6 solids, 2 packs (35) | same |
| `…/PerfShop01/` | `PerfShop01.BIN` | 2,910,848 | 6 solids, 2 packs (24) | same |
| `…/Showroom/` | `Showroom.BIN` | 3,989,168 | 6 solids, 2 packs (50), a texture animation, particle descriptions and emitters | same |
| `frontend/envmaps/ElaborareGT/` | `FRONTEND/ENVMAPS/ElaborareGT.bin` | 2,098,176 | 1 pack, the six faces | `crates/ob-formats/src/formats/magazines.rs:88` |
| `frontend/envmaps/Showroom/` | `FRONTEND/ENVMAPS/Showroom.bin` | 2,098,176 | 1 pack, the six faces | same |

The solids (DISC01–03, BACKDROP, BACKDROP_FLOOR, PLATFORMBASE) follow [meshes.md](meshes.md); the texture packs,
the six environment faces (named `XPOS` … `ZNEG` and found by name hash in the unit, `crates/ob-formats/src/formats/magazines.rs:83`)
and the texture animation follow [textures.md](textures.md). `FRONTEND/FRONTA.BUN` is empty (0 bytes): the
residual records it.

## 4. Layering

| Kind | `merge` | `add` | `replace` | `disabled` | Clash key |
|---|---|---|---|---|---|
| `fe-package` | refused (`MRG.3`) | refused (`MRG.3`): a new screen needs engine code | refused (`MRG.3`) | refused (`MRG.3`) | – |
| `font` | yes: fields; `glyph` is a keyed list (key `code`) | yes: a new name, in `frontend/ui/fonts/` or a language's `fonts/` | yes | refused for fonts; allowed for glyphs | (unit, font name, field or glyph `code`) |
| platforms, environment maps | their items per [meshes.md](meshes.md) and [textures.md](textures.md) | refused (`MRG.3`): a new platform needs engine code | – | – | – |

- **What packs change in menus today**: their pictures (texture overrides, [textures.md](textures.md)), their
  texts (string overrides, [text.md](text.md)), their fonts (this page).
- **Glyph list**: an added glyph (`op = "add"`) goes into code order, not to the end, because the table is sorted
  by code; a disabled glyph is left out of the cooked font (the character then draws nothing, as for any missing
  code). No wildcard units.
- **Extension point for menu edits** (refused in 1.0, planned as `ob.fe-package/1.1`): `merge` of a package by
  name, with `object` a keyed list (key `id`), `state` fields replaced per field, `script` keyed by `name_hash`,
  `track` keyed by (`param`, `target_word`), response lists keyed by `message`; objects a pack adds keyed by
  (pack id, local id) and appended after the base objects, so no base object moves in the draw order. The
  original's ids are kept for base objects; ids for added objects are given at cook time from a range no shipped
  package uses [guess: ≥ 0x80000000].
- **The HUD mod** (today's registry, layering.md §7) is mounted as the legacy pack `hud-mod`: its pictures are
  texture overrides by hash (595 HUD and 115 map pictures, `src/hud_mod.rs:45`); its map calibration rows
  (`map-overrides.tsv`: `TrackMapCalibrationOffsetX/Y`, `TrackMapCalibrationZoomIn`, `MenuMapZoomOffsetX/Y`,
  `MenuMapZoomWidth` per track number, `src/frontend/enh/map_pack.rs:8`) become field merges of the career data's
  track records ([career.md](career.md)), applied only for tracks whose map picture the pack also replaces (the
  rule of `src/frontend/enh/map_pack.rs:41`). Off unless the player enables it, as today.

## 5. Import

**Packages.** For each of the nine files (and the duplicate `InGameCommon.bun`): remove the file's JDLZ, walk its
chunks, and for each 0x30203 / 0x30210 chunk:

1. 0x30210: check the u32 against `bin_hash` of the upper-case package name (true for all 221) and unpack the JDLZ
   or HUFF block (`src/frontend/assets.rs:43`).
2. Decode the tag tree into the model of §3.1. Check the canonical tag order, every constant and every derived
   value (sizes, counts, `TypS`, `Butn`, `OP` word 1, `MC`, the `Sh` track count, the minimal zero padding of
   `PkHd`, `RsNm` and `St`, `Kd`'s first time word). Fill `label` from the labels table and `name` from
   Overbright's name list where the hashes match.
3. Encode the model again and compare with the unpacked bytes (Eq2). A package that fails any check is kept whole
   in the residual and reported (`UI.14`); none fails in the reference install.
4. The first chunk of a name writes `frontend/ui/packages/<NAME>.toml`; a later chunk of the same name MUST unpack
   to the same bytes (`UI.15`; true for all 30 repeats).
5. Residual map: a 0x30203 chunk is one `decoded` node, `items = ["fe-package:<NAME>"]`, `proof = "bytes"`. A
   0x30210 chunk is a **compressed node** ([residual.md](residual.md) §3.1 "Compressed nodes"): `fate =
   "container"`, `wrap = "jdlz"` or `"huff"`, `wrap_size` and `wrap_digest` (`xxh3-128`) of the compressed block;
   its children are the hash word (`decoded`, `items = ["fe-package:<NAME>"]`, `proof = "bytes"`: the encoder
   writes `bin_hash` of the upper-case name), the unpacked package (`decoded`, the same item, `proof = "bytes"`)
   and the 0–18 fill bytes (`fill`, or `kept` if they are not one repeated byte). No shadow is kept, and the
   file's map describes its expanded content, as residual.md §3.1 says. A repeated copy of a package (step 4)
   names the same item.

**Fonts.** Each 0x30201 chunk is decoded (§3.2), checked (name blocks in the canonical form, constants, derived
values, glyph byte 8 = 0, pairs grouped per glyph in order, byte 3 of each pair), encoded and compared. A font
whose name and bytes are the same in every file that holds it is written once to `frontend/ui/fonts/` (the six
European fonts and `GLOBALA.BUN`'s); a font only one language file holds goes to that language's `fonts/` (the
Korean, Chinese, Japanese and Thai faces). Each language's `language.toml` lists its fonts in file order
([text.md](text.md) §3.1). No shipped name has two different contents (measured).

**Platforms and environment maps**: their chunks go to [meshes.md](meshes.md) and [textures.md](textures.md);
the importer makes the units of §3.5.

## 6. Equality and verification

| Item | Level | Proof at import | Shadow |
|---|---|---|---|
| `fe-package` | Eq2 on the unpacked package, hence Eq1 | the encoder rebuilds the unpacked bytes; a prototype encoder written for this page rebuilt all 236 shipped chunks byte for byte | none: the 221 packed chunks (1,603,524 bytes) are compressed nodes (§5 step 5), whose compressed blocks are identified by `wrap_digest`, not kept |
| `font` | Eq2, hence Eq1 | the encoder rebuilds the chunk; the prototype rebuilt all 87 | none |
| the front-end asset set | Eq1 | `H(Assets)` over the package map, the fonts and the strings, from the original files and from the base set, for the active language | – |

Gates ([parity.md](parity.md) §3), baselines recorded from the original files first: every menu and HUD shot
(`--shot main|career|explore|race|qrstart|drift|drag|…` and the options pages) at every resolution of the matrix;
the Credits screen; a test that hashes each package's `Package` model (`src/frontend/feng/model.rs:177`), each
`Font` and the string map.

## 7. Cooked form

- **Cook unit** `ui-<language>`: the front-end asset set of §3.3 for one active language. Model sections:
  `fe_packages` (name → the `feng::Package` model the parser gives today), `fonts` (`fe_hash` → `Font`),
  `strings` ([text.md](text.md) §7), and the UI texture records of [textures.md](textures.md), whose BC blocks
  are referenced zero-copy from their bundle entries.
- **Inputs**: every file under `frontend/ui/`, the active language's `strings.toml`, `charmap.bin` and `fonts/`,
  the UI texture sets, plus the layout hash (cooked.md §3–4).
- **Cook units** `fe-platform-<NAME>` and `fe-envmap-<NAME>`: the unit's solids and texture records per their
  pages; the six environment faces in face order as one section.
- **Prepared data** (build-specific, not normative): uploaded textures, glyph atlases for the enhanced font
  rendering, HD UI art.

## 8. Errors

<!-- generated:rules -->
| Rule | Text |
|---|---|
| UI.1 | A package `name` that breaks its pattern, or a package file not named after it. |
| UI.2 | Two objects with one `id` in a package; two scripts with one `name_hash` on an object; two nodes with one `message` in a response list. |
| UI.3 | A `parent` that names no earlier `group` object of the package. |
| UI.4 | A `resource` index outside the resource list. |
| UI.5 | A `state` of the wrong shape: `corner_colors_bgra` without `uv`, or two objects of one kind with different state sizes. |
| UI.6 | `text` on an object that is not a string, or a string object without it. |
| UI.7 | A track whose `delta` lengths differ from `base`, whose values have the wrong type for its `param`, whose `target_word` plus value count passes the state's words, or whose `length_ms` is ≥ 2²⁴. |
| UI.8 | A `chain` that names no script of the same object. |
| UI.9 | A two-character tag that would encode to more than 65,535 bytes. |
| UI.10 | A `name` or `label` whose hash differs from the stored hash (`fe_hash` for objects and scripts; `bin_hash` of the label or of its upper-case form for labels). |
| UI.11 | A font with glyph codes not strictly ascending, more than 255 pairs on a glyph, `flags` without bit 0x40000, name or texture blocks over 256 bytes with their NULs, or a `tail` over 4,096 bytes. |
| UI.12 | A font `texture` that resolves to no texture in the layer stack. |
| UI.13 | One font name twice in one unit, or a language `fonts` entry that neither the language unit nor `frontend/ui/fonts/` holds. |
| UI.14 | (import) A package or font chunk the model cannot represent exactly: kept whole in the residual and reported. |
| UI.15 | (import) Two package chunks of one name whose unpacked bytes differ: the first in the original load order is decoded, the other kept and reported. |
| UI.16 | A `state` colour or rotation, or a track's `base` or `delta`, in another form than the original's words (§3.1, "Exception to conventions §5.3 and §5.4"): a `color` string, a matrix, Euler angles. |
<!-- /generated -->

Warning (never stops a pack): a package resource of kind `font` or `texture` that resolves to nothing. The shipped
`HUD_Drift` and `HUD_Short_Track` each have one such font resource (`FONT_IMPACTHUD`, used by two string objects
each); it is reported for the base set and not an error.

## 9. Versioning and extension

- Minor versions add optional fields: names for the open words when they are traced (`Sb`, `PkHd` word 3, `Targ`,
  the font header words), and the layering of §4's extension point (`ob.fe-package/1.1`).
- **HUD layout extension point**: a later kind `ob.hud-layout` in `frontend/ui/hud-layout.toml`, keyed by
  `fe-object:<PACKAGE>/0xID` of top-level HUD objects, giving an anchor (left, centre, right × top, middle,
  bottom) and an offset in canvas pixels. Absent means the engine's built-in placement (§3.4), so a base set
  never contains it and native placement is unchanged. Reserved: there is no pack form of it in 1.0 (package
  overrides are refused, `MRG.3`, §4; `OB_` extension names are Overbright's own, conventions §9).
- New screens (packages a pack adds) need engine code and are not planned.

## 10. Open points

- `Sb` (`text.sb`): the docs describe alignment flags under `Sb` while the reader takes alignment from `Sj`
  (`src/frontend/feng/text.rs:74`); the stored `Sb` values look like buffer sizes [guess]. To trace.
- `PkHd` word 3, the `Targ` words, the resource `hash` of textured, font and movie requests, resource kind 4
  (**inferred** "movie").
- The open font words (`u_0x04`, `u_0x08`, `u_0x10`, `u_0x12`, `u_0x30`, `u_0x34`, the trailing 32-byte record in
  `tail`, which begins with `{`).
- **Language fonts**: packages name `ConduitMdITC_TT14i`, while the Korean, Chinese, Japanese and Thai files hold
  `conduitmditc_tt14i_<language>` faces; how the original substitutes them (0x51BDC0's table 0x7F918C) is open
  until the original's language selection is traced. Until then the engine uses the language's fonts by exact
  name only.
- `FONT_IMPACTHUD` (HUD_Drift, HUD_Short_Track) exists in no font chunk; which font the original draws those four
  objects with is open. The engine today picks an arbitrary loaded font.
- Whether the original reads `InGameCommon.bun` [to verify]. Overbright reads only the `.lzc`.

## 11. Sizes and timing

| Kind | Original (reference install) | Base set (source form) |
|---|---|---|
| `fe-package` | 236 chunks in 9 files: 1,603,524 bytes packed + 503,816 plain; 9,690,064 bytes unpacked (all chunks: 19,580 objects, 51,159 scripts, 66,327 tracks, 22,716 response nodes); 8,382,312 bytes for the 206 distinct packages | 206 files, ≈ 24 MB of TOML [estimate from a prototype writer; largest ≈ 1.2 MB, median ≈ 80 KB]; ≈ 2.9 × the unpacked binary |
| `font` | 87 chunks, 377,872 bytes (15 distinct) | 15 files, ≈ 1.0 MB [estimate] |
| platforms, environment maps | 8 files, 27,817,392 bytes; 2 files, 4,196,352 bytes | per meshes.md, textures.md |

Import: unpacking and decoding all packages took ≈ 5 s in an unoptimised prototype; ≈ 1 s in the importer
[guess]. A cold start that parses all package TOML before the first cook: ≈ 0.3–1 s [guess]; later starts load the
cooked unit (today's uncached asset load: well under a second).
